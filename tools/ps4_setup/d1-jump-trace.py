"""Trace a jump a tick at a time in the running 56578 client: d1-jump-trace.py RATE HOLD_TICKS [TICKS].

Slow motion through the time scale ([0x26BC888]+0x04, which paces the engine: 0.05 x 30/RATE gives
about 1.5 ticks a second), the rate written with d1_rate.py, jump (NumPad2) pressed for HOLD_TICKS
ticks exactly. Each tick prints the Guardian's height, the vertical velocity over the tick and its
change in m/s^2. While jump is held the change is gravity (-12.71 m/s^2) plus the hold boost;
compare a rate with the same hold in seconds at 30 (d1-jump-trace.py 30 6 / 60 12). The rate goes
back to 30 and the time scale to 1.0 at the end. The Guardian must stand still on flat ground.
"""
import argparse
import ctypes
import importlib.util
import os
import struct
import time

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location('d1_winguest', os.path.join(HERE, 'd1-winguest.py'))
winguest = importlib.util.module_from_spec(spec)
spec.loader.exec_module(winguest)
import d1_keys  # noqa: E402
import d1_rate  # noqa: E402
import d1_unit_tracking as ut  # noqa: E402

JUMP = 0x62  # NumPad2


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('rate', type=int)
    ap.add_argument('hold', type=int, help='ticks jump is held')
    ap.add_argument('ticks', type=int, nargs='?', default=0, help='ticks to trace (default hold + 10)')
    a = ap.parse_args()
    n = a.ticks or a.hold + 10
    p, base = winguest.open_client()
    _, clock, scale = d1_rate.pointers(p, base)
    d1_rate.set_rate(p, base, a.rate)
    time.sleep(1)
    p.write(scale + 4, struct.pack('<f', 0.05 * 30 / a.rate))
    time.sleep(1.5)
    user32 = ctypes.windll.user32
    sc = user32.MapVirtualKeyW(JUMP, 0)
    d1_keys._focus(d1_keys._window())

    def tick():
        k = p.u(clock + 0xC, 'I')
        while p.u(clock + 0xC, 'I') == k:
            time.sleep(0.003)
        time.sleep(0.05)

    heights = []
    try:
        tick()
        heights.append(ut.read_unit_position(p, base).position[2])
        user32.keybd_event(JUMP, sc, 0, 0)
        for i in range(n):
            tick()
            if i + 1 == a.hold:
                user32.keybd_event(JUMP, sc, 2, 0)
            heights.append(ut.read_unit_position(p, base).position[2])
    finally:
        user32.keybd_event(JUMP, sc, 2, 0)
        p.write(scale + 4, struct.pack('<f', 1.0))
        d1_rate.set_rate(p, base, 30)
    prev = None
    for i in range(1, len(heights)):
        v = (heights[i] - heights[i - 1]) * a.rate
        acc = '' if prev is None else f'{(v - prev) * a.rate:+8.2f} m/s2'
        held = 'held' if i <= a.hold else '    '
        print(f'{i:3d} t={i / a.rate:.3f}s {held} height {heights[i] - heights[0]:6.3f} m  vz {v:6.3f} m/s {acc}')
        prev = v


if __name__ == '__main__':
    main()
