"""Combat controller v2 (56578): vision confirms a live enemy (nameplate), memory gives its position,
native yaw/pitch aim locks on and tracks it. In-process keys only (no PowerShell per action).

    combat2.py --seconds 120 [--shots DIR] [--log FILE]
Stops (exit 3) when the Guardian's unit changes (death). Every loop moves a stick.
"""
import argparse, ctypes, importlib.util, json, math, os, struct, sys, time
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, HERE); import routepaths  # noqa: E402
SENSE = str(routepaths.SETUP)   # d1-winguest.py, d1_unit_tracking.py and d1_squad_tracking.py
sys.path.insert(0, SENSE)
def L(n, p=None):
    s = importlib.util.spec_from_file_location(n, p or os.path.join(HERE, n + '.py')); m = importlib.util.module_from_spec(s); s.loader.exec_module(m); return m
g = L('g', os.path.join(SENSE, 'd1-winguest.py')); pl = L('plates'); hp = L('health'); fight = L('fight')
import d1_unit_tracking as ut
from d1_squad_tracking import SquadReader, Unavailable   # PR #68: live squad members' native body positions
VITALS = L('native_vitals', os.path.join(HERE, 'read_vitals.py'))  # read-only

u32 = ctypes.WinDLL('user32')
_s = importlib.util.spec_from_file_location('gamefocus', os.path.join(os.path.dirname(os.path.abspath(__file__)), 'gamefocus.py')); _gf = importlib.util.module_from_spec(_s); _s.loader.exec_module(_gf)
VK = {'W': 0x57, 'A': 0x41, 'S': 0x53, 'D': 0x44, 'I': 0x49, 'J': 0x4A, 'K': 0x4B, 'L': 0x4C,
      'FIRE': 0x4F, 'ADS': 0x45, 'GREN': 0x51, 'MELEE': 0x55, 'RELOAD': 0x64, 'JUMP': 0x62}
def key(k, down):
    if down: _gf.ensure()
    u32.keybd_event(VK[k], u32.MapVirtualKeyW(VK[k], 0), 0 if down else 2, None)
def tap(k, ms=60): key(k, True); time.sleep(ms / 1000); key(k, False)
def hold(k, ms): key(k, True); time.sleep(max(0, ms) / 1000); key(k, False)
def release_all():
    for k in VK: key(k, False)

YAW_RATE, PITCH_RATE = 2.76e-3, 1.2e-3          # rad per ms of right-stick hold
HFOV, VFOV = math.radians(78), math.radians(48)   # hip view, 1936x1119 capture
CX, CY, HALF_W, HALF_H = 968, 652, 960, 540       # reticle and half extents
FEET = (b'\xf6\xce\x9d\x80', b'\xe5\xcc\x9d\x80')     # body root at floor height (Dreg/Vandal, Captain)
TORSO = (b'\x37\xe1\x9d\x80', b'\x8b\xdc\x9d\x80')    # ~1.3 m above its root while upright; floor height as a corpse
ENEMY_HDR = FEET + TORSO
def wrap(a): return (a + math.pi) % (2 * math.pi) - math.pi


class World:
    def __init__(self):
        self.p, self.base = g.open_client(); self.cands, self.scanned = [], 0; self.handle = None
        self.first, self.moved = {}, {}          # unit -> first position seen, unit -> last time it had moved
        self.squads = SquadReader(self.p, self.base); self.tmap = {}
    def vital(self, key):
        """Sum of the member's native region fractions (drops when hits land), or None if unavailable."""
        t = self.tmap.get(key)
        if t is None: return None
        try: rows = VITALS.read_vitals(self.squads, t)
        except (OSError, RuntimeError, AssertionError, ValueError): return None
        return sum(r['fraction'] for row in rows for r in row['regions']) if rows else None
    def members(self, me, radius=60):
        """Native squad members (any group) with a body position: [(dist, key, pos)]. Body origin; aim adds an offset."""
        try: snap = self.squads.snapshot()
        except (OSError, Unavailable): return []
        out = []
        for t in snap['targets']:
            for part in t['parts']:
                pos = tuple(part['position'])
                if all(math.isfinite(v) for v in pos) and math.dist(pos, me) < radius:
                    key = ('sq', t['group'], t['squad'], t['member_handle']); self.tmap[key] = t
                    out.append((math.dist(pos, me), key, pos, part['role']))
                    break
        return sorted(out)
    def me(self):
        u = ut.read_unit_position(self.p, self.base)
        if u is None: return None
        if self.handle is None: self.handle = u.handle
        if u.handle != self.handle: raise RuntimeError('unit changed (death)')
        x, y, z = struct.unpack('<3f', self.p.read(u.address + ut.FORWARD_VECTORS[1], 12))
        return u.position, math.atan2(y, x), math.asin(max(-1, min(1, z)))
    def scan(self, me, radius=60):
        if time.time() - self.scanned < 2.0: return
        table = self.base + ut.OBJECT_TABLE
        entries = self.p.u(table + 8, 'Q'); stride = self.p.u(table + 0x10, 'I'); mask = self.p.u(table + 0x28, 'i') & ut.U64_MASK
        raw = self.p.read(entries, 8192 * stride); out = []
        for slot in range(8192):
            e8 = struct.unpack_from('<Q', raw, slot * stride + 8)[0]
            if not e8: continue
            unit = (entries + slot * stride - (mask & e8)) & ut.U64_MASK
            try:
                if self.p.u(unit + 0x24, 'I') & 0x1FFF != slot: continue
                hdr = self.p.read(unit, 4)
                if hdr not in ENEMY_HDR: continue
                pos = struct.unpack('<3f', self.p.read(unit + 0x1A0, 12))
            except OSError:
                continue
            if all(math.isfinite(v) for v in pos) and math.dist(pos, me) < radius:
                out.append((unit, hdr in TORSO))
        self.cands, self.scanned = out, time.time()
    def positions(self):
        out, now = [], time.time()
        for unit, torso in self.cands:
            if not torso: continue
            try: pos = struct.unpack('<3f', self.p.read(unit + 0x1A0, 12))
            except OSError: continue
            f = self.first.setdefault(unit, pos)
            if math.dist(f, pos) > 0.3:
                self.moved[unit] = now; self.first[unit] = pos
            out.append((unit, pos))
        return out
    def feet(self):
        out = []
        for unit, torso in self.cands:
            if torso: continue
            try: out.append(struct.unpack('<3f', self.p.read(unit + 0x1A0, 12)))
            except OSError: pass
        return out
    def upright(self, pos, feet):
        near = [f for f in feet if math.dist(f[:2], pos[:2]) < 1.5]
        return bool(near) and max(pos[2] - f[2] for f in near) > 0.8
    def live(self, objs, me, window=8.0):
        """Objects that moved in the last `window` s (corpses do not), nearest first."""
        now = time.time()
        feet = self.feet()
        return sorted(((math.dist(me, pos), unit, pos) for unit, pos in objs
                       if now - self.moved.get(unit, 0) < window or self.upright(pos, feet)))


def bearing(me, yaw, pitch, t, eye=0.35):   # unit z sits ~1.3 m above the floor; the camera ~0.35 m above that
    dx, dy, dz = t[0] - me[0], t[1] - me[1], t[2] - (me[2] + eye)
    return wrap(math.atan2(dy, dx) - yaw), math.atan2(dz, math.hypot(dx, dy)) - pitch, math.hypot(dx, dy)


def aim(world, t, tries=4):
    """Native yaw/pitch onto world point t. Tolerance is the target's half-width at its distance
    (about 0.35 m), so close targets need far less settling. Returns (on_target, distance)."""
    d = 1.0
    for _ in range(tries):
        s = world.me()
        if s is None: return False, d
        ey, ep, d = bearing(*s, t)
        tol = max(0.03, math.atan2(0.35, max(d, 0.5)))
        if abs(ey) < tol and abs(ep) < tol * 1.5: return True, d
        if abs(ey) >= tol: hold('L' if ey < 0 else 'J', max(25, abs(ey) / YAW_RATE))
        if abs(ep) >= tol * 1.5: hold('I' if ep > 0 else 'K', max(25, abs(ep) / PITCH_RATE))
    s = world.me()
    if s is None: return False, d
    ey, ep, d = bearing(*s, t)
    return math.hypot(ey, ep) < math.atan2(0.6, max(d, 0.5)), d


def main():
    ap = argparse.ArgumentParser(); ap.add_argument('--seconds', type=float, default=120)
    ap.add_argument('--shots'); ap.add_argument('--log')
    ap.add_argument('--squads', help='only these squad numbers, e.g. 8,11')
    # The 2014 reference player takes hits as big as ours and survives by breaking line of sight while
    # the shield comes back (notes, 28 Oct). Backing off only below 0.35 for 3.5 s kept the Guardian
    # dying in the Breach's waves (sense105-108).
    ap.add_argument('--retreat-at', type=float, default=0.55, help='back off below this health fraction')
    ap.add_argument('--recover-s', type=float, default=5.0, help='seconds to wait out of sight for the shield')
    ap.add_argument('--idle-passes', type=int, default=8,
                    help='end the fight after this many passes in a row with nothing to shoot at (0: never)')
    a = ap.parse_args()
    only = {int(x) for x in a.squads.split(',')} if a.squads else None
    W = World(); fight.pad('NumPad4')                    # focus the emulator once
    log = open(a.log, 'a') if a.log else None
    t_end, k, lock, lock_seen, strafe, last_gren, bursts = time.time() + a.seconds, 0, None, 0, 1, 0, 0
    sight = time.time()                                   # last time a plate sat near the reticle
    misses, hit_at = 0, 0                                 # aimed bursts with no native pool drop; last confirmed hit
    # Line of sight: an enemy's nameplate shows only when it is in view under the reticle. A target
    # aimed at with no plate near the reticle is behind a wall: do not fire at it (sense108 emptied
    # magazines into walls at members it could locate but not see). Peek once, then skip it a while.
    blind, blocked = 0, {}                                # blind aims at this lock; lock -> blocked until
    idle = 0                                              # passes in a row with no target
    try:
        while time.time() < t_end:
            k += 1; vit = ''
            s = W.me()
            if s is None: time.sleep(0.15); continue
            me, yaw, pitch = s
            W.scan(me)
            img = fight.grab(os.path.join(a.shots, f'c-{k:03d}.png') if a.shots else str(routepaths.probes() / 'c2.png'))
            h = hp.health(img); plates = pl.plates(img)
            objs = W.positions()
            # 1. Match each visible plate to the enemy object along its bearing.
            livemap = {unit: pos for _, unit, pos in W.live(objs, me)}
            for cx, cy, wd, ht in plates:
                py = -(cx - CX) / HALF_W * HFOV / 2          # screen right = clockwise = negative yaw offset
                pp = -(cy + 40 - CY) / HALF_H * VFOV / 2      # the torso sits just below its plate
                best = None
                for unit, pos in livemap.items():
                    ey, ep, d = bearing(me, yaw, pitch, pos)
                    err = math.hypot(wrap(ey - py), ep - pp)
                    if unit == lock: err *= 0.5               # hysteresis: keep the current lock
                    if err < 0.15 and (best is None or err < best[0]): best = (err, unit)
                if best:
                    lock, lock_seen = best[1], time.time(); break
            if lock in livemap: lock_seen = max(lock_seen, time.time() - 2)   # a live lock survives a blink without its plate
            target = next((pos for unit, pos in objs if unit == lock), None) if lock and time.time() - lock_seen < 4 else None
            now = time.time()
            members = [m for m in W.members(me) if (only is None or m[1][2] in only) and blocked.get(m[1], 0) < now]
            if members:
                pick = next((m for m in members if m[1] == lock), members[0])   # keep the current member lock
                if pick[1] != lock:
                    blind = 0
                lock, target = pick[1], pick[2]
                torso = [pos for _, pos in objs if math.dist(pos[:2], target[:2]) < 1.2 and 0.6 < pos[2] - target[2] < 2.2]
                if torso:                                 # the member's own upright torso object (taller for Captains)
                    target = min(torso, key=lambda q: math.dist(q[:2], target[:2]))
                else:
                    target = (target[0], target[1], target[2] + (1.1 if pick[3] == 'body' else 0.0) - 0.2)  # body origin -> chest (aim adds +0.2)
            if target is None:
                lv = [x for x in W.live(objs, me) if x[0] < 35]
                if lv:
                    lock, target = lv[0][1], lv[0][2]
            # 2. Hurt: break line of sight and recover.
            if h is not None and h < a.retreat_at:
                release_all(); hold('S', 1300); hold('A' if strafe > 0 else 'D', 600); tap('RELOAD')
                time.sleep(max(0.0, a.recover_s - 2.0))
                print(k, f'health {h:.2f}: back off and recover', flush=True); continue
            if target:
                idle = 0
                # In view now? This frame was taken after the previous pass aimed at the same lock.
                in_view = any(abs(px - CX) < 220 and py < CY + 40 for px, py, _, _ in plates)
                aim_pt = (target[0], target[1], target[2] + 0.2)   # torso object sits low in the chest; aim at upper chest
                ok, d = aim(W, aim_pt)
                # Plates are hard to see in the dark hall: a hit confirmed by the target's health in the
                # last 3 s, or a target this close, counts as in view too.
                in_view = in_view or time.time() - hit_at < 3 or d < 8
                if in_view: sight, blind = time.time(), 0
                if d < 2.2:
                    tap('MELEE', 100); time.sleep(0.4)          # point blank: melee (R1)
                elif ok and not in_view:
                    # Not walking at it: closing in on an unseen target walked the Guardian into walls.
                    blind += 1
                    if blind == 2:
                        hold('A' if strafe > 0 else 'D', 450)    # peek round the corner once
                    elif blind >= 3:
                        blocked[lock], blind = time.time() + 8, 0   # behind a wall: leave it for 8 s
                        print(k, f'lock {lock} out of sight at {d:.1f} m: skipped for 8 s', flush=True)
                elif ok:
                    if 6 < d < 14 and time.time() - last_gren > 20:
                        tap('GREN', 120); last_gren = time.time()
                    ads = d > 12
                    v0 = W.vital(lock) if isinstance(lock, tuple) else None
                    if ads: key('ADS', True); time.sleep(0.25)
                    key('FIRE', True); key('A' if strafe > 0 else 'D', True); time.sleep(0.7)
                    key('FIRE', False); key('A', False); key('D', False); key('ADS', False)
                    v1 = W.vital(lock) if v0 is not None else None
                    if v0 is not None and v1 is not None:
                        misses = 0 if v1 < v0 - 1e-4 else misses + 1
                        if v1 < v0 - 1e-4: hit_at = time.time()
                        if misses >= 3:                    # aimed and in view, but no damage: cover between us
                            blocked[lock], misses = time.time() + 8, 0
                        vit = f' vit {v0:.3f}->{v1:.3f}'
                strafe = -strafe; bursts += 1
                if bursts % 8 == 0: tap('RELOAD')
                msg = f'{k}: lock {lock if isinstance(lock, tuple) else hex(lock)} d {d:.1f} aimed {ok} plates {len(plates)} health {h}' + vit
            else:
                # No lock: turn toward the radar threat or sweep, nudging forward.
                if abs(pitch) > 0.06: hold('I' if pitch < 0 else 'K', max(25, abs(pitch) / PITCH_RATE))   # level the view
                hold('L', 250); hold('W', 200); msg = f'{k}: no live enemy within 35 m; level and sweep'
                if k % 15 == 0: tap('RELOAD')
                idle += 1
                if a.idle_passes and idle >= a.idle_passes:
                    # Sweeping an empty room for the rest of a 30 s round (sense108) only costs time.
                    print(msg, flush=True); print(k, 'nothing to fight: ending early', flush=True)
                    break
            print(msg, flush=True)
            if log: log.write(json.dumps({'t': time.time(), 'k': k, 'msg': msg}) + '\n')
    except RuntimeError as e:
        print('stopped:', e, flush=True); release_all(); sys.exit(3)
    finally:
        release_all()
    print('bursts', bursts, flush=True)


if __name__ == '__main__':
    main()
