"""Aim at enemy nameplates and fire (56578 prototype, pad input only).

    aim.py --seconds 120 [--shots DIR] [--auto]
Each look: find nameplates (plates.py); aim 60 px below the one nearest the reticle (its body), turning
and pitching by the pixel offset; fire when within 30 px. With no plate, turn toward the radar's red
sector (fight.py); with nothing on the radar, sweep 60 degrees.
"""
import argparse, importlib.util, math, os, subprocess, time
import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))


def load(name):
    s = importlib.util.spec_from_file_location(name, os.path.join(HERE, name + '.py'))
    m = importlib.util.module_from_spec(s); s.loader.exec_module(m)
    return m


plates, fight = load('plates'), load('fight')
RATE = 2.26 * 1.14 * 110 / 90 / 1000     # rad per ms of stick, measured (a 90-degree command turned 110)
RX, RY = 968, 571                          # the reticle in the 1936x1119 window capture
HALF_W, HALF_H = 960, 540
HFOV, VFOV = math.radians(78), math.radians(48)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--seconds', type=float, default=120)
    ap.add_argument('--shots')
    ap.add_argument('--auto', action='store_true', help='hold fire (auto rifle) instead of tapping')
    a = ap.parse_args()
    t_end, step, kills = time.time() + a.seconds, 0, 0
    while time.time() < t_end:
        step += 1
        path = os.path.join(a.shots or HERE, f'aim-{step:03d}.png' if a.shots else 'aim.png')
        img = fight.grab(path)
        found = plates.plates(img)
        if found:
            cx, cy, w = min(found, key=lambda p: math.hypot(p[0] - RX, p[1] - RY))[:3]
            ax, ay = cx, cy + 60
            yaw = (ax - RX) / HALF_W * HFOV / 2
            pitch = (ay - RY) / HALF_H * VFOV / 2
            keys = []
            if abs(ax - RX) > 30:
                keys.append(f'Hold:{"L" if yaw > 0 else "J"}:{max(15, int(abs(yaw) / RATE))}')
            if abs(ay - RY) > 30:
                keys.append(f'Hold:{"K" if pitch > 0 else "I"}:{max(15, int(abs(pitch) / RATE))}')
            if keys:
                fight.pad(','.join(keys))
            fight.pad('Hold:O:700' if a.auto else 'O,O')
            print(f'{step}: plate at ({cx}, {cy}) w {w}; aim {",".join(keys) or "on"}; fire', flush=True)
            continue
        ang, n, near = fight.enemy_angle(img)
        if ang is None and near < 300:
            fight.pad('Hold:L:%d' % int(math.radians(60) / RATE))
            print(f'{step}: nothing; sweep', flush=True)
        elif ang is not None and abs(ang) > math.radians(12):
            fight.pad('Hold:%s:%d' % ('L' if ang > 0 else 'J', int(abs(ang) / RATE)))
            print(f'{step}: radar enemy at {math.degrees(ang):+.0f} deg; turn', flush=True)
        else:
            fight.pad('Hold:O:700' if a.auto else 'O,O')
            print(f'{step}: radar enemy ahead or close; fire', flush=True)


if __name__ == '__main__':
    main()
