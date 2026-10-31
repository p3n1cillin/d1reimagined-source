"""Close-quarters fight: hip fire at the nearest nameplate, turn toward radar contacts, back off when hurt.
    cqb.py --seconds 90 [--shots DIR]   (stops when the Guardian's unit changes, i.e. on death)"""
import argparse, importlib.util, math, os, sys, time
HERE = os.path.dirname(os.path.abspath(__file__))
SENSE = os.path.join(HERE, 'pr57')
def L(n, p=None):
    s = importlib.util.spec_from_file_location(n, p or os.path.join(HERE, n + '.py')); m = importlib.util.module_from_spec(s); s.loader.exec_module(m); return m
w = L('w', os.path.join(SENSE, 'd1-walk.py')); fo = L('follow'); f = fo.fight; pl = L('plates'); hp = L('health')
PIX = 0.00315
ap = argparse.ArgumentParser(); ap.add_argument('--seconds', type=float, default=90); ap.add_argument('--shots'); a = ap.parse_args()
c = w.Client(); c.native_unit = True
t_end, k = time.time() + a.seconds, 0
try:
    while time.time() < t_end:
        k += 1
        w.position(c, None)            # raises on a unit change (death)
        img = f.grab(os.path.join(a.shots or HERE, f'cq-{k:03d}.png' if a.shots else 'cq.png'))
        h = hp.health(img); p = pl.plates(img)
        if p:
            cx, cy, wd, ht = min(p, key=lambda q: abs(q[0] - 968) + abs(q[1] - 652))
            full = wd if wd < 120 else max(wd, 150)
            if ht > 14:     # a slanted plate: the enemy is within a few metres; aim at the head below it
                ax, ay = cx - wd // 2, cy + int(1.1 * full)
            else:
                ax, ay = cx - wd // 2 + int(0.3 * full), cy + int(0.5 * full)
            keys = []
            if abs(ax - 968) > 15: keys.append('Hold:%s:%d' % ('L' if ax > 968 else 'J', max(12, int(abs(ax - 968) / 960 * 0.68 / PIX))))
            if abs(ay - 652) > 15: keys.append('Hold:%s:%d' % ('K' if ay > 652 else 'I', max(12, int(abs(ay - 652) / 540 * 0.42 / PIX))))
            if keys: f.pad(','.join(keys))
            try:
                fo.key(0x4F, True); time.sleep(0.8)
            finally:
                fo.key(0x4F, False)
            print(k, f'plate ({cx},{cy}) w {wd} health {h}; aim {keys}; hip fire', flush=True); continue
        if h is not None and h < 0.4:
            f.pad('Hold:S:700'); time.sleep(4); print(k, f'health {h:.2f}: back off', flush=True); continue
        ang, n, near = f.enemy_angle(img)
        if ang is not None and abs(ang) > math.radians(12):
            f.pad('Hold:%s:%d' % ('L' if ang > 0 else 'J', max(15, int(abs(ang) / (2.76e-3)))))
            print(k, f'radar {math.degrees(ang):+.0f}; turn', flush=True)
        else:
            f.pad('Hold:W:400'); print(k, 'advance', flush=True)
except RuntimeError as e:
    print('stopped:', e)
