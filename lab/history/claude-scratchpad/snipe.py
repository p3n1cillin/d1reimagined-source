"""Hold a position and shoot the nearest nameplate with tracked ADS bursts; back off to recover shields.
    snipe.py --seconds 90 [--shots DIR]   (every loop nudges the stick)"""
import argparse, importlib.util, os, time
HERE = os.path.dirname(os.path.abspath(__file__))
def L(n):
    s = importlib.util.spec_from_file_location(n, os.path.join(HERE, n + '.py')); m = importlib.util.module_from_spec(s); s.loader.exec_module(m); return m
fo = L('follow'); f = fo.fight; pl = L('plates'); hp = L('health')
AIM_X, AIM_Y, RATE = 968, 652, 0.00315
ap = argparse.ArgumentParser(); ap.add_argument('--seconds', type=float, default=90); ap.add_argument('--shots'); a = ap.parse_args()
t_end, k, hits = time.time() + a.seconds, 0, 0
while time.time() < t_end:
    k += 1
    img = f.grab(os.path.join(a.shots or HERE, f'sn-{k:03d}.png' if a.shots else 'sn.png'))
    h = hp.health(img); p = pl.plates(img)
    if h is not None and h < 0.5:
        f.pad('Hold:S:900'); time.sleep(5); f.pad('Hold:W:700')
        print(k, f'health {h:.2f}: back off, recover, return', flush=True); continue
    if not p:
        # Plates only show near the crosshair: sweep under ADS (2 deg steps, +-8 deg about the start).
        found = None
        try:
            fo.key(0x45, True); time.sleep(0.35)
            for step in ['J'] * 4 + ['L'] * 8 + ['J'] * 4:
                f.pad('Hold:%s:12' % step)
                img = f.grab(os.path.join(HERE, 'sn-scan.png')); q = pl.plates(img)
                if q:
                    found = q; break
        finally:
            fo.key(0x45, False)
        if not found:
            f.pad('Hold:A:50,Hold:D:50'); print(k, 'no plate after ADS sweep', flush=True); continue
        p = found
        print(k, 'plate found by ADS sweep', flush=True)
    cx, cy, w = min(p, key=lambda q: abs(q[0] - AIM_X) + abs(q[1] - AIM_Y))[:3]
    ax, ay = cx - w // 2 + int(0.3 * max(w, 150)), cy + int(0.6 * max(w, 150))
    keys = []
    if abs(ax - AIM_X) > 12: keys.append('Hold:%s:%d' % ('L' if ax > AIM_X else 'J', max(12, int(abs(ax - AIM_X) / 960 * 0.68 / RATE))))
    if abs(ay - AIM_Y) > 12: keys.append('Hold:%s:%d' % ('K' if ay > AIM_Y else 'I', max(12, int(abs(ay - AIM_Y) / 540 * 0.42 / RATE))))
    if keys: f.pad(','.join(keys))
    try:
        fo.key(0x45, True); time.sleep(0.35); fo.key(0x4F, True); time.sleep(0.9)
    finally:
        fo.key(0x4F, False); fo.key(0x45, False)
    hits += 1
    print(k, f'plate ({cx},{cy}) w {w}; aim {keys}; burst', flush=True)
print('bursts', hits, flush=True)
