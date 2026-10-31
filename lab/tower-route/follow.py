"""Follow the objective waypoint diamond (56578 prototype, pad input only).

    follow.py --seconds 120 [--shots DIR] [--run-ms 1500]
Each look: find the waypoint (marker.py). In view: turn to centre it, then sprint forward for --run-ms.
Not in view: turn 45 degrees and look again. Prints the Guardian's tracked position when --tracked is given.
"""
import argparse, ctypes, importlib.util, json, math, os, time

HERE = os.path.dirname(os.path.abspath(__file__))
import sys; sys.path.insert(0, HERE); import routepaths  # noqa: E402


def load(name, path=None):
    s = importlib.util.spec_from_file_location(name, path or os.path.join(HERE, name + '.py'))
    m = importlib.util.module_from_spec(s); s.loader.exec_module(m)
    return m


fight, mk, plates = load('fight'), load('marker'), load('plates')
walk = load('walk', str(routepaths.WALK))
u32 = ctypes.WinDLL('user32')
_s = importlib.util.spec_from_file_location('gamefocus', os.path.join(os.path.dirname(os.path.abspath(__file__)), 'gamefocus.py')); _gf = importlib.util.module_from_spec(_s); _s.loader.exec_module(_gf)
RATE = 2.26 * 1.14 * 110 / 90 / 1000
RX, RY, HALF_W, HALF_H = 968, 571, 960, 540
AIM_Y = 652                        # the reticle sits below the frame centre (measured, hip and ADS)
HFOV, VFOV = math.radians(78), math.radians(48)


def key(vk, down):
    if down: _gf.ensure()
    u32.keybd_event(vk, u32.MapVirtualKeyW(vk, 0), 0 if down else 2, None)


def sprint(ms, jump=False):
    fight.pad('NumPad4')                      # focus the emulator (and counts as activity)
    key(0x57, True); time.sleep(0.12); key(0x58, True); time.sleep(0.09); key(0x58, False)
    if jump:                                  # cross (NumPad2) twice: jump, then the second jump in the air
        time.sleep(0.25); key(0x62, True); time.sleep(0.09); key(0x62, False)
        time.sleep(0.25); key(0x62, True); time.sleep(0.09); key(0x62, False)
    time.sleep(max(0, ms / 1000 - (0.89 if jump else 0.21))); key(0x57, False)


def ads_burst(ms=600):
    """Aim down sights (L2 = E), fire (R2 = O) for ms, release."""
    fight.pad('NumPad4')
    key(0x45, True); time.sleep(0.3); key(0x4F, True); time.sleep(ms / 1000); key(0x4F, False); key(0x45, False)


def engage(found):
    cx, cy, w = min(found, key=lambda p: math.hypot(p[0] - RX, p[1] - AIM_Y))[:3]
    full = max(w, 150)                 # a damaged bar is shorter than its plate; the plate's left end does not move
    dx, dy = cx - w // 2 + int(0.3 * full) - RX, cy + int(0.6 * full) - AIM_Y   # the body is under the plate's left part
    keys = []
    if abs(dx) > 20:
        keys.append(f'Hold:{"L" if dx > 0 else "J"}:{max(15, int(abs(dx / HALF_W * HFOV / 2) / RATE))}')
    if abs(dy) > 20:
        keys.append(f'Hold:{"K" if dy > 0 else "I"}:{max(15, int(abs(dy / HALF_H * VFOV / 2) / RATE))}')
    if keys:
        fight.pad(','.join(keys))
    ads_burst()
    return cx, cy, w, keys


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--seconds', type=float, default=120)
    ap.add_argument('--shots')
    ap.add_argument('--run-ms', type=int, default=1500)
    ap.add_argument('--tracked')
    a = ap.parse_args()
    c = walk.Client() if a.tracked else None
    tracked = json.load(open(a.tracked)) if a.tracked else None
    t_end, step, missing, still, last = time.time() + a.seconds, 0, 0, 0, None
    while time.time() < t_end:
        step += 1
        fight.pad('Space,Space')               # touchpad twice: nav mode on and off leaves the waypoint shown
        time.sleep(0.4)
        img = fight.grab(os.path.join(a.shots or HERE, f'fo-{step:03d}.png' if a.shots else 'fo.png'))
        enemies = plates.plates(img)
        found = mk.markers(img)
        pos = walk.position(c, tracked) if c else None
        if pos and last and math.dist(pos[:2], last[:2]) < 0.3:
            still += 1
        else:
            still = 0
        last = pos
        if c and still >= 4:
            fight.pad('NumPad4')               # respawn prompt (square), else a harmless reload
            time.sleep(2)
            tracked = walk.retrack(c, tracked)
            still = 0
            print(f'{step}: no movement for 4 looks; square + retrack', flush=True)
            continue
        where = f' at ({pos[0]:.1f}, {pos[1]:.1f}, {pos[2]:.1f})' if pos else ''
        if enemies:
            cx, cy, w, keys = engage(enemies)
            still = 0
            print(f'{step}: enemy plate ({cx}, {cy}) w {w}; aim {",".join(keys) or "on"}; ADS burst{where}', flush=True)
            continue
        if found:
            missing = 0
            cx, cy, size = min(found, key=lambda m: abs(m[0] - RX))
            yaw = (cx - RX) / HALF_W * HFOV / 2
            if cx < 160 or cx > 2 * RX - 160:     # pinned to a screen edge: the objective is further round
                fight.pad('Hold:%s:%d' % ('L' if cx > RX else 'J', int(math.radians(60) / RATE)))
                print(f'{step}: marker at the edge ({cx}, {cy}); turn 60{where}', flush=True)
                continue
            if abs(cx - RX) > 40:
                fight.pad(f'Hold:{"L" if yaw > 0 else "J"}:{max(15, int(abs(yaw) / RATE))}')
            sprint(a.run_ms, jump=still >= 1)
            print(f'{step}: marker ({cx}, {cy}) size {size}; turn {math.degrees(yaw):+.0f}; sprint{" + jump" if still >= 1 else ""}{where}', flush=True)
        else:
            missing += 1
            fight.pad('NumPad4,Hold:L:%d' % int(math.radians(45) / RATE))
            print(f'{step}: no marker; turn 45{where}', flush=True)


if __name__ == '__main__':
    main()
