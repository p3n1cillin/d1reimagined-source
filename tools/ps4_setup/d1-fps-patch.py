"""Write the 56578 client patch for a frame rate N (the 60 Hz patch, generalised).

    d1-fps-patch.py N [--out FILE] [--base ps4/patches/CUSA00219-56578-lab-orbit-60hz.xml]

The 60 Hz patch sets the engine's time control (0x592A03..0x592A21): period 1001/60 ms, rate 60,
step 1000/60 ms. Read live (d1-frame-timing.py, 2 November 2026), the presentation cadence table is
built from those values: +0x38 holds the rate and +0x08 the period in microseconds (16,683 at 60).
The engine then ticks at the rate (60.0 ticks/s). This rewrites the three values for N, with
period 1001/N ms (the NTSC 1.001 of the original 33.3667 ms at 30) and step 1000/N ms. Everything
else is kept as in the base.

For any rate but 60 the base's outer cadence entry is disabled (see patch()). Above 60 the emulator
must present that fast too: GPU.vblank_frequency >= N in config.json, and
D1_FLIP_RATE=0 (a flip on every vblank).
"""
import argparse
import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
BASE = os.path.join(REPO, 'ps4', 'patches', 'CUSA00219-56578-lab-orbit-60hz.xml')

LINES = {   # address -> (prefix, original value at 60 Hz)
    '992A03': ('C785BCFEFFFF', '77778541'),   # period ms (float)
    '992A17': ('C785C4FEFFFF', '3C000000'),   # rate (u32)
    '992A21': ('C785C8FEFFFF', '55558541'),   # step ms (float)
}


def values(n):
    f = lambda x: struct.pack('<f', x).hex().upper()
    return {'992A03': f(1001.0 / n), '992A17': struct.pack('<I', n).hex().upper(), '992A21': f(1000.0 / n)}


def patch(text, n):
    new = values(n)
    for addr, (prefix, old) in LINES.items():
        line = f'Address="{addr}" Value="{prefix}{old}"'
        if text.count(line) != 1:
            sys.exit(f'{addr}: expected one line {line!r} in the base patch')
        text = text.replace(line, f'Address="{addr}" Value="{prefix}{new[addr]}"')
    text = text.replace('60 Hz engine time control', f'{n} Hz engine time control')
    if n != 60:
        # The outer cadence entry points the present deadline at the table's +0x28, which stays
        # 16,683 us at every rate: at 120 the engine ticked 119.5/s but presented 57 fps (fps-b120,
        # 2 Nov). The original instruction reads +0x08, the period the rate sets, so it is left off.
        block = '<Metadata Name="D1 56578: outer cadence 59.94 (synthetic)"\n            isEnabled="true"'
        if text.count(block) != 1:
            sys.exit('expected one outer cadence entry in the base patch')
        text = text.replace(block, block.replace('isEnabled="true"', 'isEnabled="false"'))
    return text


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('fps', type=int)
    ap.add_argument('--base', default=BASE)
    ap.add_argument('--out')
    a = ap.parse_args()
    if not 10 <= a.fps <= 1000:
        sys.exit('fps must be 10..1000')
    text = patch(open(a.base, encoding='utf-8').read(), a.fps)
    out = a.out or os.path.join(os.path.dirname(a.base), f'CUSA00219-56578-lab-orbit-{a.fps}hz.xml')
    open(out, 'w', encoding='utf-8', newline='\n').write(text)
    print(f'{a.fps} Hz: period {1001.0 / a.fps:.4f} ms, step {1000.0 / a.fps:.4f} ms -> {out}')


if __name__ == '__main__':
    main()
