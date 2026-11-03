"""Set the running 56578 client's frame rate live: d1-frame-rate.py N [--seconds S].

Writes the values the game's rate lives in (d1_rate.py: the cadence table, the engine clock's ticks
a second and tick period, the time scale), then measures the engine's ticks per second.
Run sense fps-d120-game (2 November 2026) went from 120 to exactly 60.0 ticks/s this way, without
a restart. The emulator has to present that fast: GPU.vblank_frequency >= N (read at its start)
and D1_FLIP_RATE=0.

The engine simulates only so many ticks a second (CPU-bound: about 70 in the mission on the lab PC,
nearly 1000 in orbit). Past that the game slows down: at 120 in the mission it ticked 84/s, 0.7x
real time. The emulator's D1_FPS (the project's shadPS4 fork) holds a rate with a guard.
"""
import argparse
import importlib.util
import os
import time

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location('d1_winguest', os.path.join(HERE, 'd1-winguest.py'))
winguest = importlib.util.module_from_spec(spec)
spec.loader.exec_module(winguest)
import d1_rate  # noqa: E402


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('fps', type=int)
    ap.add_argument('--seconds', type=float, default=3.0)
    a = ap.parse_args()
    if not 10 <= a.fps <= 1000:
        raise SystemExit('fps must be 10..1000')
    p, base = winguest.open_client()
    _, clock, _ = d1_rate.pointers(p, base)
    n, tps, tick, scale = d1_rate.read(p, base)
    print(f'was: rate {n}, clock {tps} ticks/s, tick {tick:.5f} s, time scale {scale:.3f}')
    d1_rate.set_rate(p, base, a.fps)
    t0, k0 = time.monotonic(), p.u(clock + 0xC, 'I')
    time.sleep(a.seconds)
    t1, k1 = time.monotonic(), p.u(clock + 0xC, 'I')
    rate = (k1 - k0) / (t1 - t0)
    game = rate * p.u(clock + 8, 'f')
    print(f'now: rate {a.fps}, engine {rate:.1f} ticks/s, game time {game:.2f}x real')


if __name__ == '__main__':
    main()
