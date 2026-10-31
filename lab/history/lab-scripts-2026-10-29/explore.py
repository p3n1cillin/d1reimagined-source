"""Greedy local explorer toward a target by native position: try headings nearest the target bearing,
keep a step only if it moves and does not lose ground; remember failed (cell, heading) pairs.
    explore.py X Y [--max-s 180] [--log FILE]   Stops on arrival (3 m in x/y) or a unit change."""
import argparse, importlib.util, json, math, os, sys, time
HERE = os.path.dirname(os.path.abspath(__file__))
SENSE = os.path.join(HERE, 'pr57')
def L(n, p=None):
    s = importlib.util.spec_from_file_location(n, p or os.path.join(HERE, n + '.py')); m = importlib.util.module_from_spec(s); s.loader.exec_module(m); return m
w = L('w', os.path.join(SENSE, 'd1-walk.py')); fo = L('follow')
RATE = 2.76e-3
ap = argparse.ArgumentParser(); ap.add_argument('x', type=float); ap.add_argument('y', type=float)
ap.add_argument('--max-s', type=float, default=180); ap.add_argument('--log'); a = ap.parse_args()
c = w.Client(); c.native_unit = True
failed = set(); trail = []
def face(target_h):
    for _ in range(3):
        d = w.wrap(target_h - w.facing(c, None))
        if abs(d) < 0.08: return
        fo.fight.pad('Hold:%s:%d' % ('L' if d < 0 else 'J', max(12, int(abs(d) / RATE))))
t0 = time.time()
try:
    while time.time() - t0 < a.max_s:
        x, y, z = w.position(c, None); trail.append((round(x, 1), round(y, 1), round(z, 1)))
        dist = math.dist((x, y), (a.x, a.y))
        if dist < 3:
            print(f'arrived ({x:.1f}, {y:.1f}, {z:.1f})', flush=True); break
        bearing = math.atan2(a.y - y, a.x - x)
        cell = (round(x / 2), round(y / 2))
        options = sorted(range(12), key=lambda k: abs(w.wrap(k * math.pi / 6 - bearing)))
        moved = False
        for k in options:
            if (cell, k) in failed: continue
            face(k * math.pi / 6)
            fo.fight.pad('Hold:W:1100')
            nx, ny, nz = w.position(c, None)
            step = math.dist((x, y), (nx, ny)); nd = math.dist((nx, ny), (a.x, a.y))
            if step < 0.8:
                fo.fight.pad('NumPad2,Hold:W:700')          # jump over a lip
                nx, ny, nz = w.position(c, None); step = math.dist((x, y), (nx, ny)); nd = math.dist((nx, ny), (a.x, a.y))
            if step >= 0.8 and nd <= dist + 1.5:
                moved = True; break
            failed.add((cell, k))
        if not moved:
            print('no way forward from', cell, flush=True); break
        print(f'({nx:.1f}, {ny:.1f}, {nz:.1f}) dist {nd:.1f}', flush=True)
except RuntimeError as e:
    print('stopped:', e)
if a.log:
    json.dump(trail, open(a.log, 'w'))
