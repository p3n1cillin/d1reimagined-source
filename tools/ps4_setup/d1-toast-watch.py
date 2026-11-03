"""Watch an orbit load for the "Contacting Destiny Servers" toast: d1-toast-watch.py OUT [--seconds 90].

For a client started with d1-lab-served.ps1: presses cross at the title until the picture brightens,
then saves a frame every second (OUT/f-NNN.png, with the wall-clock time in OUT/frames.txt) so the
toast (bottom right: ATTENTION, Contacting Destiny Servers...) can be placed against the servers'
timestamped logs. Crops of the toast area go to OUT/toast-strip.png.
"""
import argparse
import os
import subprocess
import sys
import time

from PIL import Image, ImageStat

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import d1_keys  # noqa: E402


def shot(path):
    subprocess.run(['pwsh', '-NoProfile', '-File', os.path.join(HERE, 'd1-shot.ps1'), '-Out', path],
                   check=True, capture_output=True)
    return Image.open(path)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('out')
    ap.add_argument('--seconds', type=int, default=90)
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    probe = os.path.join(a.out, 'probe.png')
    while ImageStat.Stat(shot(probe).convert('L').resize((240, 140))).mean[0] <= 40:
        d1_keys.press('NumPad2')
        time.sleep(10)
    log = open(os.path.join(a.out, 'frames.txt'), 'w')
    crops = []
    start = time.time()
    n = 0
    while time.time() - start < a.seconds:
        n += 1
        path = os.path.join(a.out, f'f-{n:03d}.png')
        im = shot(path)
        log.write(f'{n} {time.strftime("%H:%M:%S", time.gmtime())}Z\n')
        log.flush()
        w, h = im.size
        crops.append(im.crop((int(w * 0.55), int(h * 0.88), w, int(h * 0.98))).resize((500, 60)))
        time.sleep(max(0.0, start + n - time.time()))
    strip = Image.new('RGB', (500, 62 * len(crops)), 'white')
    for k, c in enumerate(crops):
        strip.paste(c, (0, k * 62))
    strip.save(os.path.join(a.out, 'toast-strip.png'))
    print(f'{n} frames -> {a.out}')


if __name__ == '__main__':
    main()
