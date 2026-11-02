"""Read the 56578 client's frame timing live: the presentation cadence table and the engine clock.

    d1-frame-timing.py [--seconds S]

Read-only (the d1-winguest.py process reader). Prints:
- the cadence table behind the global pointer at 0x26BCD08: the code at 0x594A3B adds its +0x08 field
  (33,366 us at 30 fps) to the next present deadline. The 60 Hz patch (lab-orbit-60hz.xml) points that
  add at +0x28 (16,683 us), and `[table] == 2` takes another branch (0x594664);
- the engine's game-time clock at *(0x26BC880): tick count +0x0C and period +0x08. Two samples S seconds
  apart give the ticks per second the engine simulates.

For the frame-rate settings work (2 November 2026): whether those values follow the emulator's vblank
rate, or are fixed by the game.
"""
import argparse
import importlib.util
import os
import struct
import time

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location('d1_winguest', os.path.join(HERE, 'd1-winguest.py'))
winguest = importlib.util.module_from_spec(spec)
spec.loader.exec_module(winguest)

CADENCE_PTR = 0x26BCD08
CLOCK_PTR = 0x26BC880


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--seconds', type=float, default=1.0)
    a = ap.parse_args()
    p, base = winguest.open_client()
    table = p.u(base + CADENCE_PTR, 'Q')
    raw = p.read(table, 0x40)
    print(f'cadence table {table:#x}:')
    for off in range(0, 0x40, 8):
        lo, hi = struct.unpack_from('<II', raw, off)
        q = struct.unpack_from('<Q', raw, off)[0]
        print(f'  +{off:#04x}  u64 {q:<14d} u32 {lo:<10d} {hi:<10d}')
    clock = p.u(base + CLOCK_PTR, 'Q')
    t0, ticks0, period = time.monotonic(), p.u(clock + 0x0C, 'I'), p.u(clock + 0x08, 'f')
    time.sleep(a.seconds)
    t1, ticks1 = time.monotonic(), p.u(clock + 0x0C, 'I')
    rate = (ticks1 - ticks0) / (t1 - t0)
    print(f'engine clock {clock:#x}: period {period:.4f} (+0x08), {rate:.1f} ticks/s over {t1 - t0:.1f} s')


if __name__ == '__main__':
    main()
