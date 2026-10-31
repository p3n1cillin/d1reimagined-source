# SPDX-License-Identifier: GPL-2.0-or-later
"""Give a served character its subclass abilities and/or its helmet, editing the encoded objects
through the client's own codec (d1-oracle object). Writes a new file; never edits in place.

    d1-character-fixup.py peer    IN OUT [--grid 132] [--helmet]   family 0/1 peer view (0x808017FD)
    d1-character-fixup.py item    IN OUT --grid 132                inventory item (0x80801732): the subclass
    d1-character-fixup.py profile IN OUT --helmet                  profile (0x80801743)

Abilities (issue #134): the subclass item names its talent grid (+0x58) and its active nodes (36
node-state bytes at +0x5A, 1 = active). The peer view carries the five ability rows at +0x40 (5 x
{u8 row, 16 x u32 option hash}, block 0x80805513) and 32 x u16 perk indices at +0x798 (block
0x8080599A). Rows, options and perks come from d1-talent-grid.py. Unused perk slots get 256, the
content's own "no perk".

Helmet (issue #128): the customization block 0x8080181A sits at +0x0C of the peer view, the card and
the profile; its last field (+0x2C) is the wear-helmet flag. At 0, the Tower draws no helmet, and
with an empty face, no head.

Needs D1_ORACLE (or the workspace build), D1_BUILDS and D1_BUILD, plus D1_PKGDIR for --grid.
"""
import argparse
import importlib.util
import os
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import d1paths  # noqa: E402

TAGS = {'peer': 0x808017FD, 'item': 0x80801732, 'profile': 0x80801743}
HELMET = 0x2C
PEER_ROWS, ROW_SIZE, OPTIONS = 0x40, 68, 16
PEER_PERKS, PERK_SLOTS, NO_PERK = 0x30 + 0x768, 32, 256
ITEM_GRID, ITEM_NODES, NODE_BYTES = 0x58, 0x5A, 36


def oracle():
    return os.environ.get('D1_ORACLE') or str(d1paths.workspace() / 'd1-server' / 'target' / 'release' / 'd1-oracle.exe')


def run(*args):
    r = subprocess.run([oracle(), *args], capture_output=True, text=True)
    if r.returncode:
        sys.exit(f'd1-oracle {args[0]} failed: {r.stdout[-400:]} {r.stderr[-400:]}')
    return r.stdout


def talent_grid():
    spec = importlib.util.spec_from_file_location('talent_grid', os.path.join(HERE, 'd1-talent-grid.py'))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def fix(kind, data, grid, helmet):
    data = bytearray(data)
    if helmet and kind in ('peer', 'profile'):
        data[HELMET] = 1
    if grid is not None:
        ab = talent_grid().abilities(grid)
        if kind == 'item':
            struct.pack_into('<H', data, ITEM_GRID, grid)
            nodes = bytearray(NODE_BYTES)
            for i in ab['active_nodes']:
                if i < NODE_BYTES:
                    nodes[i] = 1
            data[ITEM_NODES:ITEM_NODES + NODE_BYTES] = nodes
        elif kind == 'peer':
            for s in ab['slots']:
                at = PEER_ROWS + ROW_SIZE * s['slot']
                options = [int(o, 16) for o in s['options']][:OPTIONS]
                options += [0] * (OPTIONS - len(options))
                data[at] = s['row']
                struct.pack_into(f'<{OPTIONS}I', data, at + 4, *options)
            perks = (ab['perks'] + [NO_PERK] * PERK_SLOTS)[:PERK_SLOTS]
            struct.pack_into(f'<{PERK_SLOTS}H', data, PEER_PERKS, *perks)
    return bytes(data)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    ap.add_argument('kind', choices=sorted(TAGS))
    ap.add_argument('input', type=Path)
    ap.add_argument('output', type=Path)
    ap.add_argument('--grid', type=int)
    ap.add_argument('--helmet', action='store_true')
    a = ap.parse_args()
    if a.output.exists():
        sys.exit(f'{a.output} exists; choose a new file')
    if a.grid is None and not a.helmet:
        sys.exit('nothing to do: give --grid and/or --helmet')
    tag = f'{TAGS[a.kind]:#010x}'
    with tempfile.TemporaryDirectory() as tmp:
        obj, new = Path(tmp) / 'in.obj', Path(tmp) / 'out.obj'
        run('object', '--tag', tag, '--decode-file', str(a.input), '--out-obj', str(obj))
        new.write_bytes(fix(a.kind, obj.read_bytes(), a.grid, a.helmet))
        out = run('object', '--tag', tag, '--obj-file', str(new), '--out', str(a.output))
    if 'IDENTICAL' not in out:
        a.output.unlink(missing_ok=True)
        sys.exit(f'the client codec did not round-trip the edited object: {out[-400:]}')
    print(f'{a.output}: {a.kind} edited (grid {a.grid}, helmet {a.helmet}); round trip identical')
    return 0


if __name__ == '__main__':
    sys.exit(main())
