"""Measure a weapon's rate of fire at several frame rates in the running 56578 client:
d1-fire-probe.py --mag ADDR [--rates 30,60,120] [--hold 1.5]

The second fidelity check for the frame-rate presets: a shot interval rounds to whole ticks, so a
weapon could fire faster at a higher rate than on PS4 (30). ADDR is the equipped weapon's magazine
count (4 bytes; found with d1-memvalue.py: the HUD count, then one less after each shot). For each
rate the probe writes the rate (d1_rate.py), holds fire (O) for --hold seconds of
game time while it samples the count and the engine's tick count every ~1 ms, and reports the
interval between shots in ticks and game time, and the rounds a minute. Then it reloads (NumPad4).
The Guardian must hold an automatic weapon in gameplay with the window focusable. The rate in force
before the probe is put back at the end.
"""
import argparse
import importlib.util
import os
import statistics
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location('d1_winguest', os.path.join(HERE, 'd1-winguest.py'))
winguest = importlib.util.module_from_spec(spec)
spec.loader.exec_module(winguest)
import d1_keys  # noqa: E402
import d1_rate  # noqa: E402


def burst(p, clock, mag, hold_real):
    """[(engine tick, count)] at each shot (the count dropping) while fire is held."""
    press = threading.Thread(target=d1_keys.press, args=(f'Hold:O:{round(hold_real * 1000)}',), kwargs={'gap': 0})
    shots, last = [], p.u(mag, 'I')
    press.start()
    t0 = time.perf_counter()
    while time.perf_counter() - t0 < hold_real + 0.5:
        n = p.u(mag, 'I')
        if n < last:
            shots.append((p.u(clock + 0xC, 'I'), n))
        last = n
        time.sleep(0.001)
    press.join()
    return shots


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--mag', required=True, type=lambda s: int(s, 0))
    ap.add_argument('--rates', default='30,60,120')
    ap.add_argument('--hold', type=float, default=1.5, help='fire held this many seconds of game time')
    ap.add_argument('--bursts', type=int, default=2)
    ap.add_argument('--old-switch', action='store_true',
                    help='leave the clock ticks a second and the time scale as they are (the switch before 3 November)')
    a = ap.parse_args()
    p, base = winguest.open_client()
    _, clock, _ = d1_rate.pointers(p, base)
    was = d1_rate.read(p, base)[0]
    try:
        for fps in a.rates.split(','):
            fps = int(fps)
            d1_rate.set_rate(p, base, fps, not a.old_switch)
            time.sleep(2.0)
            t0, k0 = time.monotonic(), p.u(clock + 0xC, 'I')
            time.sleep(2.0)
            ticks = (p.u(clock + 0xC, 'I') - k0) / (time.monotonic() - t0)
            tick_s = p.u(clock + 8, 'f')
            game = max(ticks * tick_s, 0.1)
            n, tps, _, scale = d1_rate.read(p, base)
            print(f'{fps:4d}: engine {ticks:.1f} ticks/s, game {game:.2f}x real (clock {tps} ticks/s, scale {scale:.3f})')
            for i in range(a.bursts):
                fired = burst(p, clock, a.mag, a.hold / game)
                gaps = [b[0] - a_[0] for a_, b in zip(fired, fired[1:])]
                if len(gaps) < 2:
                    print(f'      burst {i + 1}: {len(fired)} shots seen, too few')
                else:
                    med = statistics.median(gaps)
                    print(f'      burst {i + 1}: {len(fired)} shots, ticks between {sorted(set(gaps))} (median {med}),'
                          f' {med * tick_s * 1000:.1f} ms, {60 / (statistics.mean(gaps) * tick_s):.0f} rounds/min')
                d1_keys.press('NumPad4')  # reload
                time.sleep(3.0 / game)
    finally:
        d1_rate.set_rate(p, base, was, not a.old_switch)
        print(f'rate put back to {was}')


if __name__ == '__main__':
    main()
