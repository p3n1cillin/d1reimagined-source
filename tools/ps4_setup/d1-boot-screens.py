"""Record the screens an orbit boot shows before the player can act: d1-boot-screens.py OUT [--max 12].

For a client started with d1-lab-served.ps1 and waiting at (or on its way to) the title: presses cross
at the title until the picture brightens (orbit), then for each screen saves OUT/screen-N.png, presses
cross, and if the screen did not change presses circle (the Destiny 2 pre-order screen dismisses with
circle). Stops when a press changes nothing (the orbit menu itself) or after --max screens, and writes
OUT/sheet.png with every screen in order. Which announcements a profile shows depends on its completion
record (notes/ps4-56578-new-guardian-2026-11-03.md).
"""
import argparse
import os
import subprocess
import time

from PIL import Image, ImageChops, ImageStat

HERE = os.path.dirname(os.path.abspath(__file__))
import sys  # noqa: E402
sys.path.insert(0, HERE)
import d1_keys  # noqa: E402


def shot(path):
    subprocess.run(['pwsh', '-NoProfile', '-File', os.path.join(HERE, 'd1-shot.ps1'), '-Out', path],
                   check=True, capture_output=True)
    return Image.open(path).convert('L').resize((240, 140))


def brightness(im):
    return ImageStat.Stat(im).mean[0]


def changed(a, b):
    return ImageStat.Stat(ImageChops.difference(a, b)).mean[0] > 6


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('out')
    ap.add_argument('--max', type=int, default=12)
    ap.add_argument('--title-timeout', type=float, default=600)
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    tmp = os.path.join(a.out, 'probe.png')
    start = time.time()
    while True:  # the title (dark, "PRESS X TO PLAY") until orbit
        im = shot(tmp)
        if brightness(im) > 40:
            break
        if time.time() - start > a.title_timeout:
            raise SystemExit('no orbit within the timeout')
        d1_keys.press('NumPad2')
        time.sleep(15)
    time.sleep(5)
    screens = []
    for n in range(1, a.max + 1):
        path = os.path.join(a.out, f'screen-{n}.png')
        before = shot(path)
        screens.append(path)
        d1_keys.press('NumPad2')
        time.sleep(5)
        after = shot(tmp)
        if not changed(before, after):
            d1_keys.press('NumPad6')
            time.sleep(5)
            after = shot(tmp)
            if not changed(before, after):
                print(f'screen {n}: no change on cross or circle; stopping (the orbit menu)')
                break
        print(f'screen {n} saved')
    thumbs = [Image.open(p).resize((480, 280)) for p in screens]
    cols = 4
    sheet = Image.new('RGB', (480 * cols, 280 * ((len(thumbs) + cols - 1) // cols)))
    for k, t in enumerate(thumbs):
        sheet.paste(t, ((k % cols) * 480, (k // cols) * 280))
    sheet.save(os.path.join(a.out, 'sheet.png'))
    print(f'{len(screens)} screens -> {os.path.join(a.out, "sheet.png")}')


if __name__ == '__main__':
    main()
