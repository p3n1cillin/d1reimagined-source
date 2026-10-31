"""Face a world point by native facing and fight there: ADS bursts, plate refinement, back off when hurt.
    pointfight.py X Y --seconds 90 [--shots DIR]"""
import argparse, importlib.util, math, os, sys, time
HERE = os.path.dirname(os.path.abspath(__file__))
SENSE = os.path.join(HERE, 'pr57')   # private copy of PR 57's walker and reader
sys.path.insert(0, SENSE)
def L(n, p=None):
    s = importlib.util.spec_from_file_location(n, p or os.path.join(HERE, n + '.py')); m = importlib.util.module_from_spec(s); s.loader.exec_module(m); return m
w = L('w', os.path.join(SENSE, 'd1-walk.py')); fo = L('follow'); f = fo.fight; pl = L('plates'); hp = L('health')
RATE, PIX = 2.76e-3, 0.00315
ap = argparse.ArgumentParser(); ap.add_argument('x', type=float); ap.add_argument('y', type=float)
ap.add_argument('--seconds', type=float, default=90); ap.add_argument('--shots'); a = ap.parse_args()
c = w.Client(); c.native_unit = True
def face():
    for _ in range(3):
        x, y, _z = w.position(c, None); h = w.facing(c, None)
        d = w.wrap(math.atan2(a.y - y, a.x - x) - h)
        if abs(d) < 0.04: return
        f.pad('Hold:%s:%d' % ('L' if d < 0 else 'J', max(12, int(abs(d) / RATE))))
t_end, k = time.time() + a.seconds, 0
try:
    face()
    while time.time() < t_end:
        k += 1
        img = f.grab(os.path.join(a.shots or HERE, f'pf-{k:03d}.png' if a.shots else 'pf.png'))
        h = hp.health(img); p = pl.plates(img)
        if h is not None and h < 0.5:
            f.pad('Hold:S:1000'); time.sleep(5.5); f.pad('Hold:W:900'); face()
            print(k, f'health {h:.2f}: cover, recover, return', flush=True); continue
        keys = []
        if p:
            cx, cy, wd = min(p, key=lambda q: abs(q[0] - 968) + abs(q[1] - 652))[:3]
            full = wd if wd < 120 else max(wd, 150)   # far plates are small; near ones shrink with damage
            ax, ay = cx - wd // 2 + int(0.3 * full), cy + int(0.5 * full)
            if abs(ax - 968) > 10: keys.append('Hold:%s:%d' % ('L' if ax > 968 else 'J', max(12, int(abs(ax - 968) / 960 * 0.68 / PIX))))
            if abs(ay - 652) > 10: keys.append('Hold:%s:%d' % ('K' if ay > 652 else 'I', max(12, int(abs(ay - 652) / 540 * 0.42 / PIX))))
            if keys: f.pad(','.join(keys))
        try:
            fo.key(0x45, True); time.sleep(0.35); fo.key(0x4F, True); time.sleep(1.0)
        finally:
            fo.key(0x4F, False); fo.key(0x45, False)
        print(k, f'plate {p[0] if p else None} aim {keys}; burst', flush=True)
        if not p and k % 3 == 0: face()
except RuntimeError as e:
    print('stopped:', e)
