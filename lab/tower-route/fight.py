"""Prototype: turn toward the radar's red enemy sector and fire aimed bursts (56578, pad input only).

    fight.py --seconds 90 [--shots DIR]
"""
import argparse, math, os, subprocess, time
import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
import sys; sys.path.insert(0, HERE); import routepaths  # noqa: E402
SHOT = str(routepaths.SHOT)
PAD = str(routepaths.PAD)
RATE = 2.26 / 1000 * 1.14          # rad per ms of right-stick hold (d1-navfollow.py)
CX, CY, R_IN, R_OUT = 185, 189, 14, 95   # radar centre and the ring searched, full-res window capture


def grab(path):
    subprocess.run(['powershell', '-NoProfile', '-File', SHOT, '-Out', path], capture_output=True)
    return np.asarray(Image.open(path).convert('RGB')).astype(int)


def enemy_angle(img):
    """Mean bearing (radians, + = right of facing) of red radar pixels, their count, and the near-disc count."""
    reg = img[CY - R_OUT:CY + R_OUT, CX - R_OUT:CX + R_OUT]
    yy, xx = np.mgrid[-R_OUT:R_OUT, -R_OUT:R_OUT]
    r = np.hypot(xx, yy)
    # The damage tint reddens the whole screen; measure red against the frame just outside the radar.
    outer = img[CY - R_OUT - 30:CY + R_OUT + 30, CX - R_OUT - 30:CX + R_OUT + 30]
    oy, ox = np.mgrid[-R_OUT - 30:R_OUT + 30, -R_OUT - 30:R_OUT + 30]
    bg = float(np.median(outer[(np.hypot(ox, oy) > R_OUT + 8) & (np.hypot(ox, oy) < R_OUT + 28)][:, 0]))
    red = ((reg[:, :, 0] - bg > 35) & (reg[:, :, 0] > 2.2 * reg[:, :, 1]) & (reg[:, :, 0] > 2.2 * reg[:, :, 2]))
    ring = red & (r > R_IN + 25) & (r < R_OUT)
    near = int((red & (r <= R_IN + 25)).sum())
    if ring.sum() < 100:   # a faint arc measured 177, a quiet frame 42
        return None, int(ring.sum()), near
    ang = np.arctan2(xx[ring], -yy[ring])          # 0 = up (facing), + = right
    return float(np.arctan2(np.sin(ang).mean(), np.cos(ang).mean())), int(ring.sum()), near


def pad(keys):
    subprocess.run(['powershell', '-NoProfile', '-File', PAD, '-Keys', keys], capture_output=True)


def burst(ms):
    # Hip fire: a held R2 through the pad driver empties the magazine (measured 12 -> reload).
    pad('Hold:O:%d' % ms)


def engage(a, state, budget):
    """Fight until the radar has been clear for three looks or the budget runs out; True if anything was seen."""
    t_end, clear, seen = time.time() + budget, 0, False
    while time.time() < t_end and clear < 3:
        state['step'] += 1
        step = state['step']
        img = grab(os.path.join(a.shots or HERE, f'fight-{step:03d}.png' if a.shots else 'fight.png'))
        ang, n, near = enemy_angle(img)
        if ang is None and near < 300:
            clear += 1
            if seen:
                pad('Hold:L:%d' % int(math.radians(60) / RATE))
            print(f'{step}: radar clear (ring {n}, near {near})', flush=True)
            continue
        seen, clear = True, 0
        if ang is not None and abs(ang) > math.radians(12):
            pad('Hold:%s:%d' % ('L' if ang > 0 else 'J', int(abs(ang) / RATE)))
            print(f'{step}: enemy at {math.degrees(ang):+.0f} deg (ring {n}, near {near}); turn', flush=True)
        burst(900)
        print(f'{step}: burst (ring {n}, near {near})', flush=True)
    return seen


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--seconds', type=float, default=90)
    ap.add_argument('--shots')
    ap.add_argument('--route', help='advance along this d1-walk route two waypoints at a time, fighting between')
    ap.add_argument('--tracked')
    a = ap.parse_args()
    state = {'step': 0}
    if not a.route:
        engage(a, state, a.seconds)
        return
    import json
    walk = str(routepaths.WALK)
    points = json.load(open(a.route))
    t_end, i = time.time() + a.seconds, 0
    while i < len(points) and time.time() < t_end:
        leg = os.path.join(a.shots or HERE, 'leg.json')
        json.dump(points[i:i + 2], open(leg, 'w'))
        out = subprocess.run(['python', '-I', '-u', walk, '--tracked', a.tracked, '--route', leg, '--max-seconds', '25',
                              '--tolerance', '1.5', '--turn-sign', '1', '--turn-rate', '2.26'],
                             capture_output=True, text=True).stdout.strip().splitlines()
        print(f'leg {i}: {out[-1] if out else "(no output)"}', flush=True)
        i += 2
        engage(a, state, 40)


if __name__ == '__main__':
    main()
