# SPDX-License-Identifier: GPL-2.0-or-later
"""Read a subclass's talent grid from your own 56578 packages and derive what a character needs to have
its abilities: the default active nodes, the five ability rows with their option hashes, and the perk
indices. Served characters with these left empty have no sprint, grenade or melee (issue #134).

    d1-talent-grid.py GRID [--json]          e.g. 132 (Striker); 130-135 and 888-890 are the subclasses

The grid table is content tag 0x80A5E227: 24-byte rows after its header, the definition tag at row
+0x10. A definition holds its node count at +0x10 and, at +0x18, a self-relative offset to the nodes
(0x40 bytes each). A node's step data starts at node +0x20 + the u64 there; its exclusive set is the
u32 at node +0x38, where 0x811C9DC5 (the hash of nothing) means none. In the step data, a
0x808018E8 list carries ability entries of 16 bytes {u32 option hash, u32 whose low byte is the row
the option applies to, u32 with the slot's ability row in its top byte, u32 slot} and a 0x80804628
list the perk indices (u16; 256 is none).

Content entries are extracted with `d1-oracle oodle entry` into STAGING/content-cache (never the
repository): set D1_ORACLE, D1_PKGDIR, D1_BUILDS and D1_BUILD as for the oracle.
"""
import argparse
import json
import os
import struct
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import d1paths  # noqa: E402

GRID_TABLE = 0x80A5E227
NO_SET = 0x811C9DC5
ABILITY_LIST, PERK_LIST = 0x808018E8, 0x80804628
NODE_SIZE = 0x40
SLOTS = 5


def entry(tag):
    cache = d1paths.staging() / 'content-cache'
    cache.mkdir(parents=True, exist_ok=True)
    out = cache / f'{tag:08x}.bin'
    if not out.exists():
        oracle = os.environ.get('D1_ORACLE') or str(d1paths.workspace() / 'd1-server' / 'target' / 'release' / 'd1-oracle.exe')
        r = subprocess.run([oracle, 'oodle', 'entry', '--tag', f'{tag:#010x}', '-o', str(out)], capture_output=True, text=True)
        if r.returncode or not out.exists():
            sys.exit(f'd1-oracle could not extract {tag:#010x}: {r.stdout[-300:]} {r.stderr[-300:]}')
    return out.read_bytes()


def u32(b, o):
    return struct.unpack_from('<I', b, o)[0]


def u64(b, o):
    return struct.unpack_from('<Q', b, o)[0]


def definition_tag(grid):
    table = entry(GRID_TABLE)
    rows = 8 + 0x18 + u64(table, 0x10)
    count = u64(table, 8)
    if not 0 <= grid < count:
        sys.exit(f'grid {grid} is outside the table ({count} rows)')
    return u32(table, rows + 0x18 * grid + 0x10)


def lists_in(blob, start, end):
    """Typed lists {u64 count, u32 tag, ...} in a node's step data: (tag, first element offset, count)."""
    found, o = [], start
    while o + 12 <= end:
        count, tag = u64(blob, o), u32(blob, o + 8)
        if tag >> 16 == 0x8080 and 0 < count < 64:
            found.append((tag, o + 16, count))
        o += 4
    return found


def read_grid(grid):
    tag = definition_tag(grid)
    d = entry(tag)
    count = u64(d, 0x10)
    first = 0x18 + u64(d, 0x18) + 0x10
    nodes = []
    for i in range(count):
        n = first + NODE_SIZE * i
        nodes.append({'index': i, 'steps': n + 0x20 + u64(d, n + 0x20), 'set': u32(d, n + 0x38)})
    starts = sorted(n['steps'] for n in nodes)
    for n in nodes:
        n['end'] = next((s for s in starts if s > n['steps']), len(d))
    return tag, d, nodes


def default_active(nodes):
    """The first node of each exclusive set, and every node outside a set."""
    seen, active = set(), []
    for n in nodes:
        if n['set'] == NO_SET or n['set'] not in seen:
            active.append(n['index'])
            seen.add(n['set'])
    return active


def abilities(grid):
    tag, d, nodes = read_grid(grid)
    active = default_active(nodes)
    rows, options, perks = {}, [], []
    for i in active:
        n = nodes[i]
        for ltag, at, count in lists_in(d, n['steps'], n['end']):
            if ltag == ABILITY_LIST:
                for k in range(count):
                    e = at + 16 * k
                    option, target, row, slot = u32(d, e), d[e + 4], d[e + 0xB], u32(d, e + 0xC)
                    if row != 0xFF and slot < SLOTS:
                        rows.setdefault(slot, row)
                    if option != NO_SET:
                        options.append((option, target))   # an option names the row it applies to
            elif ltag == PERK_LIST:
                perks += [p for p in struct.unpack_from(f'<{count}H', d, at) if p != 256]
    slots = []
    for s in range(SLOTS):
        row = rows.get(s, 0)
        slots.append({'slot': s, 'row': row, 'options': [f'{o:#010x}' for o, r in options if r == row]})
    return {'grid': grid, 'definition': f'{tag:#010x}', 'nodes': len(nodes), 'active_nodes': active,
            'slots': slots, 'perks': perks}


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    ap.add_argument('grid', type=int)
    ap.add_argument('--json', action='store_true')
    a = ap.parse_args()
    r = abilities(a.grid)
    if a.json:
        print(json.dumps(r, indent=1))
        return 0
    print(f"grid {r['grid']} ({r['definition']}): {r['nodes']} nodes, active {r['active_nodes']}")
    for s in r['slots']:
        print(f"  slot {s['slot']}: row {s['row']:3}  {len(s['options'])} options  {' '.join(s['options'][:6])}{' ...' if len(s['options']) > 6 else ''}")
    print(f"  perks: {r['perks']}")
    return 0


if __name__ == '__main__':
    sys.exit(main())
