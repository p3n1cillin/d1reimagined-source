"""List the render surfaces the running 56578 client asks for: d1-surfaces.py [--seconds 3].

An execute breakpoint (d1-watchwrite.py) at 0x81B96D, in the surface manager's acquire (0x81B920),
after 0x817F90 has decoded the request's size: the slot, the split, the width x height x depth, the
size class (+8: 0 for an explicit size, 1-4 for full, half, quarter, eighth of the render size) and the
format (+0xC). One line per distinct request, with how often it came. The slots and their sizes are in
notes/ps4-56578-resolution-2026-11-03.md; a surface wider than its slot shows the render size isn't
followed somewhere.
"""
import argparse
import collections
import json
import os
import struct
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--seconds', type=float, default=3.0)
    a = ap.parse_args()
    out = os.path.join(tempfile.mkdtemp(prefix='d1-surfaces-'), 'surfaces.jsonl')
    subprocess.run([sys.executable, os.path.join(HERE, 'd1-watchwrite.py'), '--seconds', str(a.seconds),
                    '--exec', '0x81b96d', '--peek', 'r12=', '--peek', 'r15=', '--peek', 'rbp-0x38!',
                    '--peek', 'r14+8', '--peek', 'r14+0xc', '--max-hits', '2000', '--out', out],
                   check=True, stdout=subprocess.DEVNULL)
    seen = collections.Counter()
    with open(out) as f:
        for row in f:
            hit = json.loads(row)
            peek = hit.get('peek')
            if not peek:
                continue
            depth, height, width = struct.unpack_from('<3H', bytes.fromhex(peek['rbp-0x38!'].split(':')[1]), 2)
            seen[(int(peek['r12='], 16), int(peek['r15='], 16), width, height, depth,
                  int(peek['r14+8'], 16), int(peek['r14+0xc'], 16))] += 1
    for key, n in sorted(seen.items()):
        print('slot %2d split %d  %4dx%-4d depth %d  class %d  format %2d  x%d' % (key + (n,)))


if __name__ == '__main__':
    main()
