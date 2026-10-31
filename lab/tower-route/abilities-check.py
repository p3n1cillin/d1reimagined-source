# SPDX-License-Identifier: GPL-2.0-or-later
"""Ability check in a match (issue #134): walk with W, then sprint (W, clicking L3 as sprint wants),
and compare the distances from the game's own position; then grab the HUD before and after a grenade
(L1) and a melee (R1). Uses the default pad config (W = stick up, X = L3, Q = L1, U = R1).

    abilities-check.py OUTDIR
"""
import importlib.util
import math
import os
import sys
import time

os.environ['D1_LAB_DRIVER'] = '1'
HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location('c', os.path.join(HERE, 'combat2.py'))
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)
import ctypes  # noqa: E402

u = ctypes.WinDLL('user32')


def vk(code, down):
    u.keybd_event(code, u.MapVirtualKeyW(code, 0), 0 if down else 2, None)


W = c.World()


def pos(wait=4.0):
    t0 = time.time()
    while time.time() - t0 < wait:
        p = W.me()
        if p:
            return p[0]
        time.sleep(0.1)


def run(label, sprint):
    a = pos()
    vk(0x57, True)                      # W: stick up
    time.sleep(0.15)
    if sprint:
        vk(0x58, True); time.sleep(0.1); vk(0x58, False)   # X: L3, click to sprint
    time.sleep(2.35)
    vk(0x57, False)
    time.sleep(0.6)
    b = pos()
    d = math.dist(a[:2], b[:2]) if a and b else float('nan')
    print(f'{label}: {d:.2f} m in 2.5 s', flush=True)
    c.hold('S', 2500)                    # walk back to keep the space
    time.sleep(0.5)
    return d


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    t0 = time.time()
    while pos(1.0) is None and time.time() - t0 < 240:
        time.sleep(2)
    c._gf.ensure()
    time.sleep(3)
    walk = run('walk', False)
    sprint = run('sprint (W + L3)', True)
    print(f'sprint/walk = {sprint / walk:.2f}' if walk else 'no walk distance', flush=True)
    c.fight.grab(os.path.join(out, 'hud-before.png'))
    for name, key in (('grenade-L1', 0x51), ('melee-R1', 0x55)):
        c.hold('A', 150)
        vk(key, True); time.sleep(0.15); vk(key, False)
        time.sleep(0.5)
        c.fight.grab(os.path.join(out, f'{name}-a.png'))
        time.sleep(1.5)
        c.fight.grab(os.path.join(out, f'{name}-b.png'))
        time.sleep(2)
    print('done', flush=True)
    return 0


if __name__ == '__main__':
    sys.exit(main())
