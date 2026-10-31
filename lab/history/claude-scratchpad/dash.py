"""Sprint through waypoints by native position and facing (56578, PR #57 reader).
    dash.py "x,y;x,y;..." [--sprint-ms 1100]   Stops (exit 3) when the unit changes (death)."""
import argparse, importlib.util, math, os, sys, time
HERE = os.path.dirname(os.path.abspath(__file__))
SENSE = os.path.join(HERE, 'pr57')   # private copy of PR 57's walker and reader
sys.path.insert(0, SENSE)
s = importlib.util.spec_from_file_location('w', os.path.join(SENSE, 'd1-walk.py')); w = importlib.util.module_from_spec(s); s.loader.exec_module(w)
s = importlib.util.spec_from_file_location('fo', os.path.join(HERE, 'follow.py')); fo = importlib.util.module_from_spec(s); s.loader.exec_module(fo)
RATE = 2.76e-3   # rad per ms of right-stick hold (90 deg command -> 110 deg, d1-walk notes)
ap = argparse.ArgumentParser(); ap.add_argument('points'); ap.add_argument('--sprint-ms', type=int, default=1100); ap.add_argument('--tol', type=float, default=3.0)
a = ap.parse_args()
pts = [tuple(map(float, p.split(','))) for p in a.points.split(';')]
c = w.Client(); c.native_unit = True
try:
    for tx, ty in pts:
        for _ in range(30):
            x, y, z = w.position(c, None)
            if math.dist((x, y), (tx, ty)) < a.tol:
                break
            for _ in range(3):
                h = w.facing(c, None)
                d = w.wrap(math.atan2(ty - y, tx - x) - h)
                if abs(d) < 0.12:
                    break
                fo.fight.pad('Hold:%s:%d' % ('L' if d < 0 else 'J', max(15, int(abs(d) / RATE))))
            dist = math.dist((x, y), (tx, ty))
            fo.sprint(min(a.sprint_ms, int(dist / 9 * 1000) + 250))
        print(f'at ({x:.1f}, {y:.1f}) for ({tx}, {ty})', flush=True)
    x, y, z = w.position(c, None); print(f'done ({x:.1f}, {y:.1f}, {z:.1f})')
except RuntimeError as e:
    print('stopped:', e); sys.exit(3)
