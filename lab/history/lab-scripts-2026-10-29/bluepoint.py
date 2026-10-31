"""Steer toward the HUD's green waypoint diamond: screenshot, turn toward it, walk; repeat (pad input only)."""
import argparse, importlib.util, math, os, subprocess, time
import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location('fight', os.path.join(HERE, 'fight.py'))
fight = importlib.util.module_from_spec(spec); spec.loader.exec_module(fight)
RATE = 2.26          # rad/s of right-stick turn (fight.py)
HALF_FOV = math.radians(37)


def pad(keys):
    subprocess.run(['powershell', '-NoProfile', '-File', r'F:\d1\tools\d1-pad.ps1', '-Keys', keys], capture_output=True)


_vm = importlib.util.spec_from_file_location('vendormark', os.path.join(HERE, 'vendormark.py'))
vendormark = importlib.util.module_from_spec(_vm); _vm.loader.exec_module(vendormark)


def marker(path):
    # The marker's flat UI blue, its white icon and its name text (vendormark.py). A plain "blue pixels"
    # test followed blue-lit walls and a blue remote-desktop panel in sense63.
    w = Image.open(path).size[0]
    hit = vendormark.find(path) or vendormark.find(path, require_text=False)
    return (float(hit[0]) if hit else None), w


ap = argparse.ArgumentParser()
ap.add_argument('--steps', type=int, default=12)
ap.add_argument('--walk-ms', type=int, default=2000)
ap.add_argument('--shots', default=HERE)
a = ap.parse_args()
last = None
for i in range(a.steps):
    shot = os.path.join(a.shots, f'bp-{i:02d}.png')
    fight.grab(shot)
    x, w = marker(shot)
    if x is None:
        print(i, 'no marker; turning 60 deg'); pad('Hold:L:%d' % int(math.radians(60) / RATE * 1000)); continue
    if x < 150 or x > w - 150:                              # the off-screen edge arrow: turn, don't walk
        print(i, f'edge arrow at x {x:.0f}; turning 45 deg', flush=True)
        pad('Hold:%s:%d' % ('J' if x < w / 2 else 'L', int(math.radians(45) / RATE * 1000)))
        continue
    if last is not None and abs(x - last) < 3 and abs(x - w / 2) < w * 0.12:
        print(i, f'arrived: marker straight ahead at x {x:.0f} and no longer moving', flush=True)
        break
    if last is not None and abs(x - last) < 3:              # no progress: back off and go round
        side = 'L' if (i // 2) % 2 else 'J'
        print(i, f'stuck at x {x:.0f}; backing off, turning 60 deg {side}', flush=True)
        pad('Hold:S:1200'); pad('Hold:%s:%d' % (side, int(math.radians(60) / RATE * 1000))); pad('Hold:W:%d' % a.walk_ms)
        last = None
        continue
    last = x
    ang = (x - w / 2) / (w / 2) * HALF_FOV
    print(i, f'marker x {x:.0f}/{w} -> {math.degrees(ang):+.0f} deg', flush=True)
    if abs(ang) > math.radians(6):
        pad('Hold:%s:%d' % ('L' if ang > 0 else 'J', int(abs(ang) / RATE * 1000)))
    pad('Hold:W:%d' % a.walk_ms)
    time.sleep(0.5)
