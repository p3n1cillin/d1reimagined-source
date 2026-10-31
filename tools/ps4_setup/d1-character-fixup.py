# SPDX-License-Identifier: GPL-2.0-or-later
"""Give a served character its subclass abilities, its helmet or profile stacks, editing the encoded objects
through the client's own codec (d1-oracle object). Writes a new file; never edits in place.

    d1-character-fixup.py peer    IN OUT [--grid 132] [--helmet]   family 0/1 peer view (0x808017FD)
    d1-character-fixup.py item    IN OUT --grid 132                inventory item (0x80801732): the subclass
    d1-character-fixup.py profile IN OUT [--helmet] [--stack 278:870:100 ...] [--item 93:3598:0xd100000000000401@7 ...]
                                                                   profile (0x80801743)

Abilities (issue #134): the subclass item names its talent grid (+0x58) and its active nodes (36
node-state bytes at +0x5A, 1 = active). The peer view carries the five ability rows at +0x40 (5 x
{u8 row, 16 x u32 option hash}, block 0x80805513) and 32 x u16 perk indices at +0x798 (block
0x8080599A). Rows, options and perks come from d1-talent-grid.py. Unused perk slots get 256, the
content's own "no perk".

Helmet (issue #128): the customization block 0x8080181A sits at +0x0C of the peer view, the card and
the profile; its last field (+0x2C) is the wear-helmet flag. At 0, the Tower draws no helmet, and
with an empty face, no head.

Stacks: a profile inventory slot (32 bytes from +0x38: u16 content, u64 instance at +8, u32 quantity
at +0x10) holding a stack of a non-instanced item, e.g. a character-held currency: Vanguard Marks
(870) in slot 278, Crucible Marks (871) in 279 (notes/ps4-56578-vendor-purchase-2026-10-30.md). The
slot must be empty or already hold that content.

Items: an instanced item in a profile slot (content, instance at +8, quantity 0), and with @N the
equipped array (20 x u64 instance at +0x2B78) entry N pointing at it. From the geared fixture: 1 =
subclass (bucket 0x06, slot 40), 2..6 = the armour (0x07..0x0B, slots 43/53/63/73/83), 7/8/9 = the
weapons (0x02/0x03/0x04, slots 93/103/113), 11 = vehicle (0x0D, slot 143), 12 = ghost (0x0E, slot
153). The item itself must be served too (D1_INVENTORY, as a 0x80801732 element).

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
PROFILE_SLOTS, SLOT_SIZE, EMPTY = 0x38, 32, 0xFFFF
EQUIPPED, EQUIPPED_COUNT = 0x2B78, 20


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


def stack(text):
    slot, content, quantity = (int(x, 0) for x in text.split(':'))
    return slot, content, quantity


def item(text):
    entry, _, index = text.partition('@')
    slot, content, instance = (int(x, 0) for x in entry.split(':'))
    return slot, content, instance, int(index) if index else None


def fix(kind, data, grid, helmet, stacks=(), items=()):
    data = bytearray(data)
    for slot, content, instance, index in items:
        if kind != 'profile':
            sys.exit('--item is for a profile')
        at = PROFILE_SLOTS + SLOT_SIZE * slot
        held = struct.unpack_from('<H', data, at)[0]
        if held != EMPTY:
            sys.exit(f'profile slot {slot} already holds content {held}')
        data[at:at + SLOT_SIZE] = bytes(SLOT_SIZE)
        struct.pack_into('<H', data, at, content)
        struct.pack_into('<Q', data, at + 8, instance)
        if index is not None:
            if not 0 <= index < EQUIPPED_COUNT:
                sys.exit(f'equipped index {index} is outside 0..{EQUIPPED_COUNT - 1}')
            struct.pack_into('<Q', data, EQUIPPED + 8 * index, instance)
    for slot, content, quantity in stacks:
        if kind != 'profile':
            sys.exit('--stack is for a profile')
        at = PROFILE_SLOTS + SLOT_SIZE * slot
        held = struct.unpack_from('<H', data, at)[0]
        if held not in (EMPTY, content):
            sys.exit(f'profile slot {slot} holds content {held}, not {content}')
        struct.pack_into('<H', data, at, content)
        struct.pack_into('<Q', data, at + 8, 0)
        struct.pack_into('<I', data, at + 0x10, quantity)
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
    ap.add_argument('--stack', type=stack, action='append', default=[], metavar='SLOT:CONTENT:QUANTITY')
    ap.add_argument('--item', type=item, action='append', default=[], metavar='SLOT:CONTENT:INSTANCE[@EQUIPPED]')
    a = ap.parse_args()
    if a.output.exists():
        sys.exit(f'{a.output} exists; choose a new file')
    if a.grid is None and not a.helmet and not a.stack and not a.item:
        sys.exit('nothing to do: give --grid, --helmet, --stack and/or --item')
    tag = f'{TAGS[a.kind]:#010x}'
    with tempfile.TemporaryDirectory() as tmp:
        obj, new = Path(tmp) / 'in.obj', Path(tmp) / 'out.obj'
        run('object', '--tag', tag, '--decode-file', str(a.input), '--out-obj', str(obj))
        new.write_bytes(fix(a.kind, obj.read_bytes(), a.grid, a.helmet, a.stack, a.item))
        out = run('object', '--tag', tag, '--obj-file', str(new), '--out', str(a.output))
    if 'IDENTICAL' not in out:
        a.output.unlink(missing_ok=True)
        sys.exit(f'the client codec did not round-trip the edited object: {out[-400:]}')
    print(f'{a.output}: {a.kind} edited (grid {a.grid}, helmet {a.helmet}, stacks {a.stack}, items {a.item}); round trip identical')
    return 0


if __name__ == '__main__':
    sys.exit(main())
