"""Walk the Guardian along a recorded route on the 56578 client (shadPS4), by pad input only.

Position: the client keeps many float triples that track the player (rendering and simulation
copies). `--scan` finds them: candidates near a known point, kept when they move with the player
and stay still when it does; the position is the median of the tracked set. No memory writes.

Steering: heading from the last movement, camera turns with the right stick (J/L), walking with
the left stick (W). Turn rate and sign are calibrated on the first turn. If the direction has
already been measured, --turn-sign 1 (or -1) prevents a collision slide reversing that direction.
--turn-rate sets a measured rate in radians per second instead of recalibrating it near obstacles.

  d1-walk.py --scan 383,-962,12 --out tracked.json      (needs the player able to move)
  d1-walk.py --tracked tracked.json --route missions/routes/enter_the_wall.json [--until-incident ...]
  d1-walk.py --tracked tracked.json --route missions/routes/wall_garage_door.json --turn-sign 1 --turn-rate 2.26
"""
import argparse, ctypes, ctypes.wintypes as wt, json, math, os, statistics, struct, subprocess, sys, time

src = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'd1-winguest.py')).read()
wg = {'__name__': 'lib'}
exec(compile(src, 'd1-winguest.py', 'exec'), wg)
PAD = os.environ.get('D1_PAD', r'F:\d1\tools\d1-pad.ps1')


def pad(*keys, gap=150):
    subprocess.run(['powershell', '-NoProfile', '-File', PAD, '-Keys', ','.join(keys), '-GapMilliseconds', str(gap)],
                   capture_output=True)


class Client:
    def __init__(self):
        self.p, self.base = wg['open_client']()

    def triple(self, a):
        try:
            return struct.unpack('<3f', self.p.read(a, 12))
        except OSError:
            return None

    def scan(self, centre, radius):
        k32 = ctypes.WinDLL('kernel32', use_last_error=True)

        class MBI(ctypes.Structure):
            _fields_ = [('BaseAddress', ctypes.c_void_p), ('AllocationBase', ctypes.c_void_p),
                        ('AllocationProtect', wt.DWORD), ('PartitionId', wt.WORD), ('RegionSize', ctypes.c_size_t),
                        ('State', wt.DWORD), ('Protect', wt.DWORD), ('Type', wt.DWORD)]
        k32.VirtualQueryEx.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.POINTER(MBI), ctypes.c_size_t]
        import numpy as np
        out, addr, mbi = [], 0x100000000, MBI()
        while addr < 0x1000000000 and k32.VirtualQueryEx(self.p.h, ctypes.c_void_p(addr), ctypes.byref(mbi), ctypes.sizeof(mbi)):
            b0, sz = mbi.BaseAddress or 0, mbi.RegionSize
            if mbi.State == 0x1000 and mbi.Protect in (0x04, 0x40) and sz < (1 << 31):
                for c in range(0, sz, 1 << 24):
                    try:
                        buf = self.p.read(b0 + c, min(1 << 24, sz - c))
                    except OSError:
                        continue
                    a = np.frombuffer(buf[:len(buf) // 4 * 4], dtype='<f4')
                    m = ((abs(a[:-2] - centre[0]) < radius) & (abs(a[1:-1] - centre[1]) < radius)
                         & (abs(a[2:] - centre[2]) < radius))
                    out += [b0 + c + 4 * int(i) for i in np.nonzero(m)[0]]
            addr = b0 + sz
        return out


def keep(client, addrs, moved):
    """Re-read; keep addresses that moved 1..15 units (moved=True) or stayed within 0.05."""
    before = {a: client.triple(a) for a in addrs}
    return before


def filter_step(client, before, moved):
    kept = []
    for a, v in before.items():
        n = client.triple(a)
        if not v or not n:
            continue
        d = math.dist(v[:2], n[:2])
        if (moved and 1 <= d <= 15 and abs(n[2] - v[2]) < 3) or (not moved and d <= 0.05):
            kept.append(a)
    return kept


def plausible(t):
    """A world position, not a stray float: the Cosmodrome's coordinates are hundreds of units out."""
    return t is not None and all(math.isfinite(x) and abs(x) < 1e5 for x in t) and math.hypot(t[0], t[1]) > 50


def retrack(client, tracked, near=None, scan_radius=25):
    """Drop stale copies after a death or respawn: keep the addresses that follow a short move.

    A respawn leaves some tracked copies frozen at the old spot, and the median then flips between
    the frozen and live groups. Move forward, back and sideways in turn (a Guardian against a wall
    can still back off or sidestep) and keep the copies that moved 0.5..15 units. Only if none moved
    in any direction, re-scan around `near` (the last good position); never around stray values.
    Returns the new list, or the old one when nothing better is found.
    """
    for keys in (['Hold:W:700'], ['Hold:S:700'], ['Hold:A:700'], ['Hold:D:700'], ['NumPad2', 'Hold:W:700']):
        before = {a: client.triple(a) for a in tracked}
        pad(*keys)
        moved = []
        for a, v in before.items():
            n = client.triple(a)
            if plausible(v) and plausible(n) and 0.5 <= math.dist(v[:2], n[:2]) <= 15:
                moved.append(a)
        if len(moved) >= 10:
            return moved
    if not plausible(near):
        return tracked
    addrs = client.scan(near, scan_radius)
    if not addrs or len(addrs) > 100000:
        return tracked
    for keys in ('Hold:W:1200', 'Hold:S:1200'):
        before = keep(client, addrs, True)
        pad(keys)
        found = [a for a in filter_step(client, before, True) if plausible(client.triple(a))]
        if len(found) >= 10:
            return found
    return tracked


def position(client, tracked, near=None):
    pts = [t for t in (client.triple(a) for a in tracked) if plausible(t)]
    if near:
        pts = [t for t in pts if math.dist(t[:2], near[:2]) < 25] or pts
    if not pts:
        return None
    return tuple(statistics.median(t[k] for t in pts) for k in range(3))


# The client returns to Orbit after 15 minutes "inactive" (prerequisite 1, 0x793A00). In the world it
# takes the OLDER of two activity times: the last button change and the last stick-quadrant change
# (0xB1B0E0, slot 0x2803340 + user*0x140: +0x1A0 buttons, +0x1B0 sticks, +0x1B8 the result). Sticks
# alone never count, so press square (reload; harmless with a full magazine) and nudge the stick.
KEEPALIVE_S = 45
_last_active = 0.0


def active(force=True):
    global _last_active
    if force or time.time() - _last_active >= KEEPALIVE_S:
        pad('NumPad4', 'Hold:W:150', 'Hold:S:150')
        _last_active = time.time()


def wrap(a):
    return (a + math.pi) % (2 * math.pi) - math.pi


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--scan')
    ap.add_argument('--out')
    ap.add_argument('--tracked')
    ap.add_argument('--route')
    ap.add_argument("--tolerance", type=float, default=2.5)
    ap.add_argument("--radius", type=float, default=25)
    ap.add_argument('--max-seconds', type=float, default=600)
    ap.add_argument('--turn-sign', type=int, choices=(-1, 1),
                    help='hold a measured camera turn direction; do not infer it from collision slides')
    ap.add_argument('--turn-rate', type=float,
                    help='hold a measured camera turn rate in radians/second; skip movement calibration')
    ap.add_argument('--avoid', action='append', default=[],
                    help='x0,y0,x1,y1: a hazard box to keep out of (the pit by the Wall stairs: 331,-837,338,-819)')
    ap.add_argument('--keepalive', type=float, help='only keep the Guardian active for this many seconds')
    ap.add_argument('--retrack', action='store_true', help='only drop stale copies from --tracked (after a death) and rewrite it')
    a = ap.parse_args()
    if a.turn_rate is not None and (not math.isfinite(a.turn_rate) or a.turn_rate <= 0):
        ap.error('--turn-rate must be a finite positive number in radians/second')
    if a.keepalive:
        end = time.time() + a.keepalive
        while time.time() < end:
            active()
            time.sleep(min(KEEPALIVE_S, max(0.0, end - time.time())))
        return
    c = Client()
    if a.scan:
        centre = [float(v) for v in a.scan.split(',')]
        # A rough centre finds few copies; a second pass centred on the first fix finds the rest
        # (run agr-breach4: 1 tracked, then 253 around that fix).
        for attempt in range(3):
            addrs = c.scan(centre, a.radius)
            print(len(addrs), 'candidates')
            # Forward, still, back, still: backing up returns the player near the start (side-steps
            # walked it into the ditch by the cars, run agr-breach8). The last attempt side-steps
            # instead, which still moves the player when backing up is blocked.
            back = 'Hold:A:1500' if attempt == 2 else 'Hold:S:1500'
            for moved, keys in ((True, ['Hold:W:1500']), (False, []), (True, [back]), (False, [])):
                before = keep(c, addrs, moved)
                if keys:
                    pad(*keys)
                else:
                    time.sleep(2)
                addrs = filter_step(c, before, moved)
                print(('moved' if moved else 'still'), len(addrs))
            # Keep only triples near the scan area: transient buffers can pass the movement tests.
            addrs = [x for x in addrs if (t := c.triple(x)) and all(math.isfinite(v) for v in t)
                     and math.dist(t[:2], centre[:2]) < a.radius + 40]
            here = position(c, addrs)
            print(len(addrs), 'tracked; position', here)
            if len(addrs) >= 100 or not here:
                break
            centre = list(here)
        json.dump(addrs, open(a.out, 'w'))
        return
    with open(a.tracked) as tracked_file:
        tracked = json.load(tracked_file)
    if a.retrack:
        tracked = retrack(c, tracked, position(c, tracked))
        json.dump(tracked, open(a.tracked, 'w'))
        print(len(tracked), 'tracked; position', position(c, tracked))
        return
    with open(a.route) as route_file:
        route = json.load(route_file)
    here = position(c, tracked)
    print('start', here)
    rate, sign = (a.turn_rate or 3.5) / 1000, float(a.turn_sign or 1)  # rad/ms and camera direction
    heading, calibrated = None, a.turn_rate is not None
    t_end = time.time() + a.max_seconds
    i = min(range(len(route)), key=lambda k: math.dist(route[k][:2], here[:2]))
    stuck = 0
    best, since_best, skipped = None, 0, False
    turn_err, worse = None, 0
    def corner(k):
        # The turn at waypoint k, in radians (0 = straight on).
        if k <= 0 or k + 1 >= len(route):
            return 0.0
        h1 = math.atan2(route[k][1] - route[k - 1][1], route[k][0] - route[k - 1][0])
        h2 = math.atan2(route[k + 1][1] - route[k][1], route[k + 1][0] - route[k][0])
        return abs(wrap(h2 - h1))

    def cross_track(k, p):
        # Distance from p to the segment route[k-1] -> route[k].
        (x1, y1), (x2, y2) = route[k - 1][:2], route[k][:2]
        dx, dy = x2 - x1, y2 - y1
        L2 = dx * dx + dy * dy
        t = 0.0 if L2 == 0 else max(0.0, min(1.0, ((p[0] - x1) * dx + (p[1] - y1) * dy) / L2))
        return math.dist(p[:2], (x1 + t * dx, y1 + t * dy))

    boxes = [tuple(float(v) for v in b.split(',')) for b in a.avoid]

    def escape(p, margin=0.8):
        # The nearest point outside any hazard box (grown by margin) when p is inside one.
        for x0, y0, x1, y1 in boxes:
            x0, y0, x1, y1 = min(x0, x1) - margin, min(y0, y1) - margin, max(x0, x1) + margin, max(y0, y1) + margin
            if x0 < p[0] < x1 and y0 < p[1] < y1:
                exits = [(p[0] - x0, (x0 - 1.0, p[1])), (x1 - p[0], (x1 + 1.0, p[1])),
                         (p[1] - y0, (p[0], y0 - 1.0)), (y1 - p[1], (p[0], y1 + 1.0))]
                return min(exits)[1]
        return None

    retreats = 0
    while i < len(route) and time.time() < t_end:
        active(force=False)
        target = route[i]
        out = escape(here)
        if out:
            # Inside a hazard margin: step out first, short pulses, then resume the route.
            print(f'hazard: stepping out to ({out[0]:.1f}, {out[1]:.1f})')
            target = [out[0], out[1], here[2]]
        d = math.dist(here[:2], target[:2])
        # Sharp corners need the waypoint itself: overshooting the north-to-west turn at the
        # Wall's stairs (waypoint 43) drops the Guardian into a pit (runs agr-breach5 and 9).
        sharp = corner(i) > 0.6
        # While stepping out of a hazard the target is the escape point, not a waypoint: do not
        # count it as an arrival (rifle4 walked the whole route index off in place that way).
        if not out and d < (min(a.tolerance, 1.5) if sharp else a.tolerance):
            i += 1
            best, since_best, retreats = None, 0, 0
            continue
        # No progress toward this waypoint for a long while (a railing, a car): back off,
        # side-step and take the next waypoint instead - once; skipping on without progress walks
        # the whole route off in place (run agr-breach10 skipped 20 waypoints at the stairs).
        if best is None or d < best - 0.5:
            best, since_best, skipped = d, 0, False
        else:
            since_best += 1
        # Off the route by more than 3 units: steer back to the previous waypoint first (drifting
        # sideways off the Wall-stair ramp dropped the Guardian into the pit, run agr-breach19).
        # A tolerant arrival can leave us >3 units from the next segment.
        # Retreating to that already-reached point immediately advances again,
        # resets retreats and repeats forever without sending another pad input.
        if (i > 0 and retreats < 2 and cross_track(i, here) > 3.0
                and math.dist(here[:2], route[i - 1][:2]) >= a.tolerance):
            i, retreats = i - 1, retreats + 1
            best, since_best, heading = None, 0, None
            print(f'off the route: back to wp {i}')
            continue
        if since_best >= 16 and not skipped:
            skipped = True
            pad('Hold:S:600', 'Hold:A:500' if (i % 2) else 'Hold:D:500', 'NumPad2')
            # Never skip a corner (it is where the path turns around an obstacle): re-approach it.
            if sharp and i > 0 and retreats < 2:
                i, retreats = i - 1, retreats + 1
            else:
                i = min(i + 1, len(route) - 1)
            since_best, heading = 0, None
            here = position(c, tracked, here) or here
            best = math.dist(here[:2], route[i][:2])
            print(f'skip to wp {i}')
            continue
        want = math.atan2(target[1] - here[1], target[0] - here[0])
        if heading is not None:
            err = wrap(want - heading)
            if abs(err) > 0.3:
                ms = int(min(900, abs(err) / rate))
                key = 'L' if (err < 0) == (sign > 0) else 'J'
                h0 = heading
                pad(f'Hold:{key}:{ms}')
                turn_err = err
                if not calibrated:
                    # measure the turn: one short forward pulse gives the new heading
                    p0 = position(c, tracked, here)
                    pad('Hold:W:500')
                    p1 = position(c, tracked, here)
                    if p0 and p1 and math.dist(p0[:2], p1[:2]) > 0.3:
                        h1 = math.atan2(p1[1] - p0[1], p1[0] - p0[0])
                        turned = wrap(h1 - h0)
                        # A pulse that slid along an obstacle measures nonsense (agr-breach8: 10 rad/s);
                        # keep the default unless the rate is plausible (3.3 rad/s measured).
                        if abs(turned) > 0.05 and 0.0015 <= abs(turned) / ms <= 0.006:
                            measured_sign = (1.0 if turned * (-1 if key == 'L' else 1) > 0 else -1.0)
                            if a.turn_sign is None or measured_sign == a.turn_sign:
                                sign, rate = measured_sign, abs(turned) / ms
                                calibrated = True
                                print(f'calibrated: {rate * 1000:.2f} rad/s per key, sign {sign:+.0f}')
                            else:
                                print('calibration slide opposed the fixed turn sign; keeping direction')
                        heading, here = h1, p1
                    continue
                heading = None
        p0 = here
        pad('Hold:W:350' if (sharp or d < 4) else 'Hold:W:700')
        here = position(c, tracked, p0) or p0
        step = math.dist(p0[:2], here[:2])
        if step > 0.3:
            heading = math.atan2(here[1] - p0[1], here[0] - p0[0])
            stuck = 0
            # Turns that keep making the error worse mean the turn sign is wrong (run agr-breach12
            # spiralled off the cliff south of the spawn): flip it.
            if turn_err is not None and step > 1.5:   # only clean, unobstructed moves count
                worse = worse + 1 if abs(wrap(want - heading)) > abs(turn_err) + 0.3 else 0
                if worse >= 2 and a.turn_sign is None:
                    sign, worse = -sign, 0
                    print('turn sign flipped')
            turn_err = None
        else:
            stuck += 1
            # Blocked, so no heading and no way to steer. Step back (the way we came is usually
            # free) and take the heading from that: facing is opposite to the back-step.
            if stuck % 2 == 0:
                # A step or ledge (the ramp at the Wall's stairs, run agr-breach16): jump and push on.
                pad('NumPad2', 'Hold:W:600')
                q = position(c, tracked, here) or here
                if math.dist(q[:2], here[:2]) > 0.3:
                    heading, here = math.atan2(q[1] - here[1], q[0] - here[0]), q
                    print(f'wp {i}/{len(route)} jumped to ({here[0]:.1f}, {here[1]:.1f}, {here[2]:.1f})')
                    continue
            q0 = here
            pad('Hold:S:500')
            here = position(c, tracked, q0) or q0
            if math.dist(q0[:2], here[:2]) > 0.3:
                heading = math.atan2(q0[1] - here[1], q0[0] - here[0])
            else:
                heading = None
                pad('NumPad2', 'Hold:A:600' if stuck % 2 else 'Hold:D:600')   # jump and side-step out
                here = position(c, tracked, here) or here
        if here[2] < target[2] - 8:
            print(f'fell off the route at {here}')
            break
        print(f'wp {i}/{len(route)} at ({here[0]:.1f}, {here[1]:.1f}, {here[2]:.1f}) -> ({target[0]:.1f}, {target[1]:.1f})')
    print('done' if i >= len(route) else 'stopped', here)


if __name__ == '__main__':
    main()
