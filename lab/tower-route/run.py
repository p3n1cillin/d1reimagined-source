"""Continuous sprint through waypoints, steering on the move by native position and facing.
    run.py "x,y;x,y;..." [--tol 2.5]   (W stays held, sprint re-tapped; keys released on exit)"""
import argparse, importlib.util, math, os, sys, time
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE); import routepaths  # noqa: E402
def L(n, p=None):
    s = importlib.util.spec_from_file_location(n, p or os.path.join(HERE, n + '.py')); m = importlib.util.module_from_spec(s); s.loader.exec_module(m); return m
w = L('w', routepaths.WALK); fo = L('follow')
RATE = 2.76e-3
VK_W, VK_X, VK_J, VK_L = 0x57, 0x58, 0x4A, 0x4C
ap = argparse.ArgumentParser(); ap.add_argument('points'); ap.add_argument('--tol', type=float, default=2.5); ap.add_argument('--max-s', type=float, default=60); ap.add_argument('--walk', action='store_true', help='no sprint (L3) taps')
a = ap.parse_args()
pts = [tuple(map(float, p.split(','))) for p in a.points.split(';')]
c = w.Client(); c.native_unit = True
fo.fight.pad('NumPad4')                      # focus the emulator
t0 = time.time(); last_sprint = 0; i = 0; stuck = 0; prev = None
try:
    fo.key(VK_W, True)
    while i < len(pts) and time.time() - t0 < a.max_s:
        try:
            x, y, z = w.position(c, None); h = w.facing(c, None); gap = 0
        except RuntimeError as e:
            if 'changed' in str(e): raise          # a different unit: death/respawn
            gap = locals().get('gap', 0) + 1       # transient unavailable read: ride it out
            if gap > 25: raise
            time.sleep(0.12); continue
        tx, ty = pts[i]
        if math.dist((x, y), (tx, ty)) < a.tol:
            i += 1; continue
        if not a.walk and time.time() - last_sprint > 1.5:  # L3 tap (re)starts the sprint
            fo.key(VK_X, True); time.sleep(0.06); fo.key(VK_X, False); last_sprint = time.time()
        d = w.wrap(math.atan2(ty - y, tx - x) - h)
        if abs(d) > 0.12:
            k = VK_L if d < 0 else VK_J
            fo.key(k, True); time.sleep(min(0.4, abs(d) / RATE / 1000)); fo.key(k, False)
        if prev and math.dist(prev, (x, y)) < 0.15:
            stuck += 1
            if stuck >= 6:                   # sprinting double jump over whatever blocks us
                fo.key(VK_W, False); fo.sprint(900, jump=True); fo.key(VK_W, True); stuck = 0
        else:
            stuck = 0
        prev = (x, y)
        time.sleep(0.12)
    x, y, z = w.position(c, None)
    print(f'{"done" if i >= len(pts) else "timeout"} ({x:.1f}, {y:.1f}, {z:.1f}) after {time.time() - t0:.1f} s', flush=True)
except RuntimeError as e:
    print(f'stopped after {time.time() - t0:.1f} s:', e); sys.exit(3)
finally:
    for k in (VK_W, VK_X, VK_J, VK_L):
        fo.key(k, False)
