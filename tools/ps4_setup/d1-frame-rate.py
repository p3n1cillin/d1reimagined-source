"""Set the running 56578 client's frame rate live: d1-frame-rate.py N [--seconds S].

Writes the three values the game's rate lives in, then measures the engine's ticks per second:
- the cadence table (pointer at 0x26BCD08): +0x08 the present period in microseconds (1001000/N)
  and +0x38 the rate N;
- the engine clock (pointer at 0x26BC880): +0x08 the tick period in seconds (1/N).
Run sense fps-d120-game (2 November 2026) went from 120 to exactly 60.0 ticks/s this way, without
a restart. The emulator has to present that fast: GPU.vblank_frequency >= N (read at its start)
and D1_FLIP_RATE=0.

The engine takes at most two ticks per frame, so game time runs at real speed only while the screen
holds at least N/2 FPS. At 120 and 41 FPS it ticked 84/s: 0.7x speed.
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

CADENCE_PTR, CLOCK_PTR = 0x26BCD08, 0x26BC880


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('fps', type=int)
    ap.add_argument('--seconds', type=float, default=3.0)
    a = ap.parse_args()
    if not 10 <= a.fps <= 1000:
        raise SystemExit('fps must be 10..1000')
    p, base = winguest.open_client()
    table, clock = p.u(base + CADENCE_PTR, 'Q'), p.u(base + CLOCK_PTR, 'Q')
    print(f'was: period {p.u(table + 8, "Q")} us, rate {p.u(table + 0x38, "Q")}, tick {p.u(clock + 8, "f"):.5f} s')
    p.write(table + 8, struct.pack('<Q', round(1001000 / a.fps)))
    p.write(table + 0x38, struct.pack('<Q', a.fps))
    p.write(clock + 8, struct.pack('<f', 1.0 / a.fps))
    t0, k0 = time.monotonic(), p.u(clock + 0xC, 'I')
    time.sleep(a.seconds)
    t1, k1 = time.monotonic(), p.u(clock + 0xC, 'I')
    rate = (k1 - k0) / (t1 - t0)
    print(f'now: rate {a.fps}, engine {rate:.1f} ticks/s ({rate / a.fps:.2f}x real time)')


if __name__ == '__main__':
    main()
