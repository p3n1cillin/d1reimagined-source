"""The 56578 client's simulation rate: the values the game derives from it at start, written live.

The cadence table (pointer at 0x26BCD08): +0x08 the present period in us (1001000/N), +0x38 the
rate N, +0x40 the tick step in us (1000000/N), which the scheduler ticks on.
The engine clock (pointer at 0x26BC880): +0x04 the ticks a second N (an integer; the helpers at
0x591BF0-0x591C80 turn seconds into ticks with it: a weapon's shot interval), +0x08 the tick period
in s (1/N), +0x0C the tick count.
The time scale (pointer at 0x26BC888): +0x04 a float the clock's set-up (0x591710) writes as the
configured rate over the clock's (1.0 in a normal start). It is left alone: written as 30/N it
paced the engine down (at 60, 0.5 gave 30 ticks a second, half real time).
Leaving out +0x04 kept shots on the start-up rate's ticks: a rifle booted at 120 fired every 16
ticks at every rate, 112 rounds a minute at 30 and 900 at 240 (notes/ps4-56578-frame-rate-2026-11-02.md).
"""
import struct

CADENCE_PTR, CLOCK_PTR, SCALE_PTR = 0x26BCD08, 0x26BC880, 0x26BC888


def pointers(p, base):
    return p.u(base + CADENCE_PTR, 'Q'), p.u(base + CLOCK_PTR, 'Q'), p.u(base + SCALE_PTR, 'Q')


def read(p, base):
    """(rate N, clock ticks a second, tick period s, time scale)."""
    table, clock, scale = pointers(p, base)
    return p.u(table + 0x38, 'Q'), p.u(clock + 4, 'I'), p.u(clock + 8, 'f'), p.u(scale + 4, 'f')


def set_rate(p, base, fps, ticks_per_second=True):
    """Write rate N. ticks_per_second=False leaves clock +0x04 as it was (what d1-frame-rate.py
    wrote before 3 November), for comparison."""
    table, clock, _ = pointers(p, base)
    if ticks_per_second:
        p.write(clock + 4, struct.pack('<I', fps))
    p.write(table + 8, struct.pack('<Q', round(1001000 / fps)))
    p.write(table + 0x38, struct.pack('<Q', fps))
    p.write(table + 0x40, struct.pack('<Q', round(1000000 / fps)))
    p.write(clock + 8, struct.pack('<f', 1.0 / fps))
