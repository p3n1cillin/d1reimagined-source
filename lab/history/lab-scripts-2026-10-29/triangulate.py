"""Locate the on-screen vendor (its HUD marker) in world coordinates from two bearings.

Takes a bearing to the marker, strafes sideways, takes a second, and intersects the two rays. Prints
"X,Y" for seek.py (which follows walls, so it finds stairs up to a raised vendor). Screen right is
clockwise, a negative yaw offset (as in combat2.py). Exit 1 if the marker isn't visible both times.
    triangulate.py [--strafe-ms 1500] [--shots DIR] [--waypoint]
"""
import argparse, importlib.util, math, os, sys, time

HERE = os.path.dirname(os.path.abspath(__file__))
def L(n):
    s = importlib.util.spec_from_file_location(n, os.path.join(HERE, n + '.py')); m = importlib.util.module_from_spec(s); s.loader.exec_module(m); return m
c = L('combat2'); vm = L('vendormark')
HALF_FOV = math.radians(37)                         # bluepoint.py's horizontal half field of view


LOOSE = False                                      # --loose: accept a vendor marker without its name text
WAYPOINT = False                                   # --waypoint: the green objective marker instead of a vendor's


def bearing(W, shot):
    for _ in range(20):                             # the unit read refuses while moving (transform copies diverge)
        s = W.me()
        if s is not None: break
        time.sleep(0.15)
    if s is None:
        return None
    me, yaw, _ = s
    c.fight.grab(shot)
    hit = vm.find_waypoint(shot) if WAYPOINT else vm.find(shot, require_text=not LOOSE)
    if hit is None:
        return None
    w = 1936
    off = (hit[0] - w / 2) / (w / 2) * HALF_FOV
    return me, yaw - off


def main():
    ap = argparse.ArgumentParser(); ap.add_argument('--strafe-ms', type=int, default=1500); ap.add_argument('--shots', default=HERE)
    ap.add_argument('--waypoint', action='store_true'); ap.add_argument('--loose', action='store_true')
    a = ap.parse_args()
    global WAYPOINT, LOOSE; WAYPOINT = a.waypoint; LOOSE = a.loose
    W = c.World()
    for turn in range(9):                           # look round in 45 degree steps until the marker is in view
        b1 = bearing(W, os.path.join(a.shots, f'tri-1-{turn}.png'))
        if b1 is not None: break
        c.hold('L', math.radians(45) / c.YAW_RATE / 1000 * 1000); time.sleep(0.5)
    if b1 is None: print('no marker (first)'); sys.exit(1)
    c.hold('D', a.strafe_ms); time.sleep(0.6)
    b2 = bearing(W, os.path.join(a.shots, 'tri-2.png'))
    if b2 is None:
        c.hold('A', 2 * a.strafe_ms); time.sleep(0.6); b2 = bearing(W, os.path.join(a.shots, 'tri-2.png'))
    c.release_all()
    if b2 is None: print('no marker (second)'); sys.exit(1)
    (p1, t1), (p2, t2) = b1, b2
    d1 = (math.cos(t1), math.sin(t1)); d2 = (math.cos(t2), math.sin(t2))
    den = d1[0] * d2[1] - d1[1] * d2[0]
    base = math.dist(p1[:2], p2[:2])
    if abs(den) < 1e-3 or base < 0.8:
        print(f'rays nearly parallel (baseline {base:.1f} m)'); sys.exit(1)
    s = ((p2[0] - p1[0]) * d2[1] - (p2[1] - p1[1]) * d2[0]) / den
    if s <= 0 or s > 80:
        print(f'implausible range {s:.1f} m'); sys.exit(1)
    x, y = p1[0] + s * d1[0], p1[1] + s * d1[1]
    print(f'from {p1[0]:.1f},{p1[1]:.1f} and {p2[0]:.1f},{p2[1]:.1f} (baseline {base:.1f} m): range {s:.1f} m', file=sys.stderr)
    print(f'{x:.1f},{y:.1f}')


if __name__ == '__main__':
    main()
