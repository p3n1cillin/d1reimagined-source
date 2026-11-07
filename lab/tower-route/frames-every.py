# SPDX-License-Identifier: GPL-2.0-or-later
"""Grab the game's frame every few seconds for a run, through fight.grab (the desktop grab, or the
emulator's own screenshot when the desktop reads black: the monitor off, 7 November).

    python frames-every.py --out DIR [--every 10] [--minutes 80] [--stop RUN/boot/stop]

Writes DIR/f0000.png, f0001.png, ... and DIR/frames.tsv (index, UTC time, mean brightness, source
size). Frames hold game pictures: keep DIR outside the repository (F:/d1/private).
"""
import argparse
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import fight  # noqa: E402


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--out', required=True)
    ap.add_argument('--every', type=float, default=10.0)
    ap.add_argument('--minutes', type=float, default=80.0)
    ap.add_argument('--stop', help='stop once this file exists (a run\'s boot/stop)')
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    end = time.time() + a.minutes * 60
    i = 0
    with open(os.path.join(a.out, 'frames.tsv'), 'a', encoding='utf-8') as log:
        while time.time() < end and not (a.stop and os.path.exists(a.stop)):
            t0 = time.time()
            path = os.path.join(a.out, f'f{i:04d}.png')
            try:
                img = fight.grab(path)
                log.write(f'{i}\t{time.strftime("%H:%M:%S", time.gmtime(t0))}\t{img.mean():.1f}\t{img.shape[1]}x{img.shape[0]}\n')
                log.flush()
                i += 1
            except Exception as e:                  # no window yet, or it closed
                log.write(f'-\t{time.strftime("%H:%M:%S", time.gmtime(t0))}\t{type(e).__name__}\n')
                log.flush()
            time.sleep(max(0.0, a.every - (time.time() - t0)))
    print(f'{i} frames in {a.out}')


if __name__ == '__main__':
    main()
