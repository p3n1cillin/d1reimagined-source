# SPDX-License-Identifier: GPL-2.0-or-later
"""Head check in the Tower (issue #128): grab the normal third-person view, then wave (d-pad up, the
pad config's Up arrow) and grab again, saving a brightened crop of the Guardian's shoulders and head
from each. A stick nudge comes first, as in every driving loop.

    tower-head-check.py OUTDIR
"""
import ctypes
import importlib.util
import os
import sys
import time

os.environ['D1_LAB_DRIVER'] = '1'
HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location('c', os.path.join(HERE, 'combat2.py'))
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)
from PIL import Image, ImageEnhance  # noqa: E402

u = ctypes.WinDLL('user32')
UP = 0x26   # an extended key: flag 1


def tap_up():
    u.keybd_event(UP, u.MapVirtualKeyW(UP, 0), 1, None)
    time.sleep(0.15)
    u.keybd_event(UP, u.MapVirtualKeyW(UP, 0), 3, None)


def save(out, name):
    path = os.path.join(out, name + '.png')
    c.fight.grab(path)
    im = Image.open(path)
    w, h = im.size
    crop = im.crop((int(w * 0.15), int(h * 0.45), int(w * 0.55), h))   # the Guardian sits lower left
    ImageEnhance.Brightness(crop).enhance(2.2).save(os.path.join(out, name + '-crop.png'))


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    c._gf.ensure()
    time.sleep(0.5)
    c.hold('S', 500)
    time.sleep(1.0)
    save(out, 'head-normal')
    tap_up()
    time.sleep(1.6)
    save(out, 'head-wave')
    time.sleep(1.5)
    save(out, 'head-wave2')
    print('saved head-normal, head-wave, head-wave2 (and -crop) in', out)
    return 0


if __name__ == '__main__':
    sys.exit(main())
