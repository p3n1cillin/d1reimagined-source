# SPDX-License-Identifier: GPL-2.0-or-later
"""The item elements a live capture's QueueZ pushes carry, decoded, as the fields the lab serves.

    d1-capture-items.py CAPTURE.pcap [--from SECONDS] [--to SECONDS] [--type 0x0EB6AC38]

Each item object (instance type --type, default the inventory item 0x0EB6AC38) is decoded with the
client's codec as an item element (struct 0x80801732; d1-rsat, D1_RSAT, the build from D1_BUILDS /
D1_BUILD) and printed as the fields the served inventory fixtures set: the item index (+0x24), the
item level (+0x18), the talent grid (+0x58) and its active nodes (+0x5A, one byte per node), plus
the bytes the push writes as `offset:value` pairs. Instance ids (+0x08) are left out, so the output
carries none of the recorder's identifiers.

Used for F54 (7 November): a new Guardian's subclass at creation, against ours
(ps4/fixtures/56578/characters/inventory-tutorial/00-Subclass.bin). Limit found then: the Xbox 360
captures are build 51539, whose item element differs from 56578's after the item index. The index
(+0x24) and item level (+0x18) read sensibly (the Generalist Shell is 862 in both builds, and a new
character's subclass items are level 0, its starter armour level 1), but later fields, the talent
nodes among them, do not. A 56578 capture would read whole.
"""
import argparse
import importlib.util
import struct
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location('srcp_cli', HERE / 'd1-srcp.py')
srcp = importlib.util.module_from_spec(_spec)
sys.argv, _argv = [sys.argv[0]], sys.argv
_spec.loader.exec_module(srcp)
sys.argv = _argv
import d1_srcp  # noqa: E402

ITEM_TAG = 0x80801732
PRIVATE = set(range(0x08, 0x10))      # the instance id


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('capture')
    ap.add_argument('--from', dest='t0', type=float, default=0.0)
    ap.add_argument('--to', dest='t1', type=float, default=1e9)
    ap.add_argument('--type', default='0x0EB6AC38')
    a = ap.parse_args()
    kind = int(a.type, 16)
    data = open(a.capture, 'rb').read()
    objs = [(t, body) for t, fams in d1_srcp.queuez_updates(data) for fam, ver, flags, os_ in fams
            for it, _, body in os_ if it == kind and a.t0 <= t <= a.t1]
    res = srcp.rsat_decode([(ITEM_TAG, body, 0, fill) for _, body in objs for fill in ('00', 'ff')])
    for k, (t, body) in enumerate(objs):
        z, o = res[2 * k], res[2 * k + 1]
        if 'object' not in z or 'object' not in o:
            print(f'{t:7.1f}  {len(body)} B  not decoded: {z.get("error", "")[:80]}')
            continue
        zi, oi = bytes.fromhex(z['object']), bytes.fromhex(o['object'])
        present = [i for i in range(len(zi)) if zi[i] == oi[i] and i not in PRIVATE]
        u16 = lambda off: struct.unpack_from('<H', zi, off)[0] if {off, off + 1} <= set(present) else None
        nodes = [i - 0x5A for i in present if 0x5A <= i < 0x86 and zi[i]]
        print(f'{t:7.1f}  {len(body)} B  item {u16(0x24)}  level {zi[0x18] if 0x18 in present else None}  '
              f'grid {u16(0x58)}  active nodes {nodes}')
        print('         ' + ' '.join(f'{i:x}:{zi[i]:02x}' for i in present if zi[i])[:900])
    return 0


if __name__ == '__main__':
    sys.exit(main())
