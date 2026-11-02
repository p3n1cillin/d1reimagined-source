"""Measure a jump at several frame rates in the running 56578 client: d1-jump-probe.py [--rates 30,60,120].

The fidelity check for the frame-rate presets: if the simulation step changes how the game plays,
a jump shows it first (peak height and airtime). For each rate the probe writes the four values
d1-frame-rate.py writes, presses jump (NumPad2, held --hold ms of game time, 120 by default; the
jump is variable-height, so a hold must cover the same game time at every rate) and samples the Guardian's position
every ~2 ms until it lands. Airtime is reported in real time and in game time (engine ticks times
the tick length), so a rate the PC cannot hold (game time below real time) is not mistaken for a
change in the jump. The Guardian must stand still on flat ground, in gameplay, with the window
focusable. The rate in force before the probe is put back at the end.
"""
import argparse
import importlib.util
import os
import statistics
import struct
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location('d1_winguest', os.path.join(HERE, 'd1-winguest.py'))
winguest = importlib.util.module_from_spec(spec)
spec.loader.exec_module(winguest)
import d1_keys  # noqa: E402
import d1_unit_tracking as ut  # noqa: E402

CADENCE_PTR, CLOCK_PTR = 0x26BCD08, 0x26BC880
LIFT = 0.03  # m above the standing height that counts as airborne


def set_rate(p, table, clock, fps):
    p.write(table + 8, struct.pack('<Q', round(1001000 / fps)))
    p.write(table + 0x38, struct.pack('<Q', fps))
    p.write(table + 0x40, struct.pack('<Q', round(1000000 / fps)))
    p.write(clock + 8, struct.pack('<f', 1.0 / fps))


def sample(p, base, clock, seconds):
    """[(real s, engine ticks, x, y, z)] at ~2 ms for the given time."""
    out, t0 = [], time.perf_counter()
    while (now := time.perf_counter()) - t0 < seconds:
        u = ut.read_unit_position(p, base)
        if u is not None:
            out.append((now - t0, p.u(clock + 0xC, 'I'), *u.position))
        time.sleep(0.002)
    return out


def jump(p, base, clock, tick_s, hold_ms, seconds):
    press = threading.Thread(target=d1_keys.press, args=(f'Hold:NumPad2:{hold_ms}',), kwargs={'gap': 0})
    rows = []
    sampler = threading.Thread(target=lambda: rows.extend(sample(p, base, clock, seconds)))
    sampler.start()
    time.sleep(0.2)  # standing height first
    press.start()
    sampler.join()
    press.join()
    if len(rows) < 50:
        raise RuntimeError('no position samples (not in gameplay?)')
    stand = [r for r in rows if r[0] < 0.18]
    # The up axis is the one that moves most.
    axis = max(range(2, 5), key=lambda i: max(r[i] for r in rows) - min(r[i] for r in rows))
    floor = statistics.median(r[axis] for r in stand)
    peak = max(r[axis] for r in rows) - floor
    air = [r for r in rows if r[axis] > floor + LIFT]
    if not air:
        return {'axis': 'xyz'[axis - 2], 'peak': peak, 'air_real': 0.0, 'air_game': 0.0, 'samples': len(rows)}
    up, down = air[0], air[-1]
    return {'axis': 'xyz'[axis - 2], 'peak': peak, 'air_real': down[0] - up[0],
            'air_game': (down[1] - up[1]) * tick_s, 'samples': len(rows),
            'landed': down is not rows[-1]}


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--rates', default='30,60,120')
    ap.add_argument('--jumps', type=int, default=3)
    ap.add_argument('--settle', type=float, default=3.0)
    ap.add_argument('--hold', type=int, default=120, help='jump held this many ms of game time')
    a = ap.parse_args()
    rates = [int(x) for x in a.rates.split(',')]
    p, base = winguest.open_client()
    table, clock = p.u(base + CADENCE_PTR, 'Q'), p.u(base + CLOCK_PTR, 'Q')
    was = p.u(table + 0x38, 'Q')
    try:
        for fps in rates:
            set_rate(p, table, clock, fps)
            time.sleep(a.settle)
            t0, k0 = time.monotonic(), p.u(clock + 0xC, 'I')
            time.sleep(2.0)
            ticks = (p.u(clock + 0xC, 'I') - k0) / (time.monotonic() - t0)
            tick_s = p.u(clock + 8, 'f')
            print(f'{fps:4d}: engine {ticks:.1f} ticks/s, game {ticks * tick_s:.2f}x real')
            for i in range(a.jumps):
                # A rate the PC cannot hold runs slow: sample long enough for 2 s of game time.
                window = max(2.5, 2.0 / max(ticks * tick_s, 0.1))
                # The hold in game time too: D1's jump rises higher the longer it is held.
                j = jump(p, base, clock, tick_s, round(a.hold / max(ticks * tick_s, 0.1)), window)
                print(f'      jump {i + 1}: peak {j["peak"]:.3f} m ({j["axis"]}), airtime {j["air_real"]:.3f} s real,'
                      f' {j["air_game"]:.3f} s game, {j["samples"]} samples'
                      + ('' if j.get('landed', True) else f', NOT LANDED in {window:.1f} s'))
                time.sleep(1.0)
    finally:
        set_rate(p, table, clock, was)
        print(f'rate put back to {was}')


if __name__ == '__main__':
    main()
