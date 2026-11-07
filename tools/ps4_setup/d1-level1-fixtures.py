# SPDX-License-Identifier: GPL-2.0-or-later
"""Build the level-1 variant of every object the A Guardian Rises tutorial serves (build 56578).

    python -I d1-level1-fixtures.py [--fixtures DIR] [--out DIR] [--light-native]

Retail A Guardian Rises is played by a new level-1 character; the served fixtures are level 40,
Light 385. Reads the served fixtures (ps4/fixtures/56578/characters, never written) and writes the
level-1 set to --out (default: <staging>/level1-test), with a README and checks/report.json.
Only level, Light and item level change (Defense and Attack follow from item level through
the client's own curves). Every edited object goes through the client's codec (d1-oracle object):
decode -> edit -> encode (the encoder must report IDENTICAL) -> decode the new payload again -> the
re-decoded object must equal the edited one, and differ from the source only at the intended bytes.
Defense and Attack are derived with `d1-oracle character derived-stat` (the client's item-level
curves) from the level-1 items, not typed in. checks/report.json records every check.
"""
import argparse
import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import d1paths  # sibling: local paths from D1_ROOT or paths.json

HERE = Path(d1paths.staging()) / 'level1-test'     # the output folder (main() sets it from --out)
WORKSPACE = Path(d1paths.workspace())   # derived-stat finds capture-rig/builds/<build>/item-content-index.tsv here
ORACLE = os.environ.get('D1_ORACLE') or str(WORKSPACE / 'd1-server' / 'target' / 'release' / 'd1-oracle.exe')
ENV = dict(os.environ, D1_BUILDS=os.environ.get('D1_BUILDS', str(WORKSPACE / 'capture-rig' / 'builds.json')),
           D1_BUILD=os.environ.get('D1_BUILD', 'later-56578'))

PEER, CARD2, ITEM, PROFILE = 0x808017FD, 0x80801810, 0x80801732, 0x80801743
LEVEL = 1
# TODO(Light): the client's character-Light aggregation (0x2660E0 -> 0x265EF0 over the 20 equipped
# items, without the override) is not decoded, and derived-stat refuses key 0. 5 is the value every
# contributing item derives at item level 1 (Attack 5, Defense 5 each), and the mission's own
# "Recommended Light 5". See README.md.
# --light-native avoids the assumption for the profile: it clears the override instead.
LIGHT = 5
SAINT14_HELMET = 1065   # the content index the helm-saint14 card names for SOID d1..0303 (inventory: 6514)


def run(*args, ok=True):
    r = subprocess.run([ORACLE, *map(str, args)], env=ENV, cwd=WORKSPACE, capture_output=True, text=True, timeout=600)
    if ok and r.returncode:
        sys.exit(f'd1-oracle {" ".join(map(str, args))} failed: {r.stdout[-600:]} {r.stderr[-600:]}')
    return r


def decode(path, tag):
    with tempfile.TemporaryDirectory() as t:
        obj = Path(t) / 'o.obj'
        run('object', '--tag', hex(tag), '--decode-file', path, '--out-obj', obj)
        return obj.read_bytes()


def encode(obj, tag, out):
    with tempfile.TemporaryDirectory() as t:
        f = Path(t) / 'o.obj'
        f.write_bytes(obj)
        r = run('object', '--tag', hex(tag), '--obj-file', f, '--out', out)
    if 'IDENTICAL' not in r.stdout:
        sys.exit(f'{out}: the client encoder did not round-trip: {r.stdout[-400:]}')
    return r.stdout.strip().splitlines()[-1]


def u32(b, at):
    return struct.unpack_from('<I', b, at)[0]


def f32(b, at):
    return struct.unpack_from('<f', b, at)[0]


REPORT = []


def check(src, out, tag, before, edited, fields, encoder_line):
    """Re-decode `out`; it must equal `edited`, and differ from `before` only inside `fields`."""
    after = decode(out, tag)
    if after != edited:
        sys.exit(f'{out}: re-decoded object differs from the edited object')
    allowed = set()
    for at, size, *_ in fields:
        allowed.update(range(at, at + size))
    diff = [i for i, (a, b) in enumerate(zip(before, after)) if a != b]
    if len(before) != len(after) or any(i not in allowed for i in diff):
        sys.exit(f'{out}: unintended bytes differ: {[hex(i) for i in diff if i not in allowed]}')
    rows = []
    for at, size, kind, name in fields:
        get = f32 if kind == 'f32' else u32
        rows.append({'offset': hex(at), 'field': name, 'old': get(before, at), 'new': get(after, at)})
    REPORT.append({'source': str(src), 'output': str(out.relative_to(HERE)), 'tag': hex(tag),
                   'encoder': encoder_line, 'redecoded_equals_edited': True,
                   'changed_bytes': [hex(i) for i in diff], 'fields': rows})
    print(f'{out.relative_to(HERE)}: ' + ', '.join(f"{r['field']} {r['old']} -> {r['new']}" for r in rows))


def edit(src, out, tag, changes):
    """changes: [(offset, kind, value, name, expected_old)]. Writes `out` and checks it."""
    before = decode(src, tag)
    b = bytearray(before)
    fields = []
    for at, kind, value, name, old in changes:
        cur = f32(b, at) if kind == 'f32' else u32(b, at)
        if old is not None and cur != old:
            sys.exit(f'{src}: {name} at {at:#x} is {cur}, expected {old}')
        struct.pack_into('<f' if kind == 'f32' else '<I', b, at, value)
        fields.append((at, 4, kind, name))
    out.parent.mkdir(parents=True, exist_ok=True)
    line = encode(bytes(b), tag, out)
    check(src, out, tag, before, bytes(b), fields, line)


# The subclass item's talent grid (+0x58, u16) and one state byte per node from +0x5A; served
# 00-Subclass.bin is the Striker grid 132 (30 nodes) with its 14 default-active nodes set to 1.
GRID_AT, NODES_AT = 0x58, 0x5A


def fresh_subclass(path, nodes):
    """Clear the talent node states of the subclass item at `path` in place: a new Guardian's grid
    before any subclass XP. The reference footage (A Guardian Rises at launch, to the loot cache)
    shows no grenade, melee or Super, and no ability icons (F54, 7 November)."""
    before = decode(path, ITEM)
    b = bytearray(before)
    grid = struct.unpack_from('<H', b, GRID_AT)[0]
    old = [i for i in range(nodes) if b[NODES_AT + i]]
    b[NODES_AT:NODES_AT + nodes] = bytes(nodes)
    line = encode(bytes(b), ITEM, path)
    after = decode(path, ITEM)
    if after != bytes(b):
        sys.exit(f'{path}: re-decoded subclass differs from the edited one')
    diff = [i for i, (x, y) in enumerate(zip(before, after)) if x != y]
    if any(not NODES_AT <= i < NODES_AT + nodes for i in diff):
        sys.exit(f'{path}: unintended bytes differ: {[hex(i) for i in diff]}')
    REPORT.append({'output': str(path.relative_to(HERE)), 'tag': hex(ITEM), 'encoder': line,
                   'redecoded_equals_edited': True, 'changed_bytes': [hex(i) for i in diff],
                   'fields': [{'offset': hex(NODES_AT), 'field': f'grid {grid} node states',
                               'old': f'active {old}', 'new': 'none active'}]})
    print(f'{path.relative_to(HERE)}: grid {grid} nodes {old} -> none active')


def copy(src, out, tag):
    out.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(src, out)
    REPORT.append({'source': str(src), 'output': str(out.relative_to(HERE)), 'tag': hex(tag), 'copied_unchanged': True})
    print(f'{out.relative_to(HERE)}: copied unchanged (no level, Light or item level)')


def peer_changes(light_old=385):
    # +0x30 level (the card's +0x68 shifted by 0x38), +0x38 derived level f32, +0x838 stat table entry 0.
    return [(0x30, 'u32', LEVEL, 'level', 40), (0x38, 'f32', float(LEVEL), 'derived level (f32)', 40.0),
            (0x83C, 'u32', LIGHT, 'stat key 0 (Light)', light_old)]


def derive(card, inventory, table, stat, scratch):
    out = scratch / f'derived-{table}-{stat}-{len(list(scratch.iterdir()))}.bin'
    r = run('--json', 'character', 'derived-stat', '--input', card, '--out', out, '--inventory', inventory,
            '--table', table, '--stat', stat)
    result = json.loads(next(l for l in r.stdout.splitlines() if l.startswith('{')))
    if result.get('round_trip') != 'IDENTICAL':
        sys.exit(f'derived-stat did not round-trip: {result}')
    return result, out


def main():
    global HERE
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    ap.add_argument('--fixtures', type=Path, default=Path(d1paths.fixtures()) / 'characters')
    ap.add_argument('--out', type=Path, default=HERE)
    ap.add_argument('--light-native', action='store_true',
                    help='clear the Light override in the profiles, so the client computes Light from the level-1 gear')
    ap.add_argument('--fresh-subclass', type=int, nargs='?', const=30, metavar='NODES',
                    help='also clear the subclass talent nodes (default 30, the Striker grid): no '
                         'grenade, melee, jump or Super, as a new Guardian (F54)')
    a = ap.parse_args()
    F, OUT = a.fixtures, a.out.resolve()
    HERE = OUT                                   # report paths are relative to the output folder

    # 1. Item instances: +0x18 item level 40 -> 1 (not `inventory rebuild`: it reads +0x18 as a hash).
    for src in sorted((F / 'inventory-tutorial').glob('*.bin')):
        edit(src, OUT / 'inventory-tutorial-l1' / src.name, ITEM, [(0x18, 'u32', LEVEL, 'item level', 40)])
    edit(F / 'grants' / 'khvostov-3598.bin', OUT / 'grants-l1' / 'khvostov-3598.bin', ITEM,
         [(0x18, 'u32', LEVEL, 'item level', 40)])
    if a.fresh_subclass:
        fresh_subclass(OUT / 'inventory-tutorial-l1' / '00-Subclass.bin', a.fresh_subclass)

    # 2. Profiles: the Light override (+0x6FDC present = 1, +0x6FE0 {key 0, value}) value 385 -> LIGHT.
    # The post-game pushes too (postgame5.sh): level1-run2 served the level-1 boot profiles, and
    # the outro's pushes then put Light 385 back. postgame-l1 is a whole post-game folder
    # (ending-run.ps1 -PostgameDir); its other files are copied unchanged.
    P, PL = F.parent / 'postgame', OUT / 'postgame-l1'
    PUSHES = ('profile-active164-xp0.draft.bin', 'profile-active164-xp900.draft.bin',
              'arcadia-profile-xp900-owned.draft.bin')
    PL.mkdir(parents=True, exist_ok=True)
    for f in sorted(P.iterdir()):
        if f.is_file() and f.name not in PUSHES:
            shutil.copyfile(f, PL / f.name)
    for src, out in ((F / 'profile-tutorial-t.bin', OUT / 'profile-tutorial-t-l1.bin'),
                     (F / 'grants' / 'profile-tutorial-th.bin', OUT / 'grants-l1' / 'profile-tutorial-th.bin'),
                     *((P / name, PL / name) for name in PUSHES)):
        b = decode(src, PROFILE)
        if b[0x6FDC] != 1 or u32(b, 0x6FE0) != 0:
            sys.exit(f'{src}: the Light override is not where expected')
        if a.light_native:
            # No override: the client sums Light over the equipped (level-1) items itself, and never
            # reads the override's value (ps4/design/items-and-gear.md section 10).
            edit(src, out, PROFILE, [(0x6FDC, 'u32', 0, 'stat override present', 1)])
        else:
            edit(src, out, PROFILE, [(0x6FE4, 'u32', LIGHT, 'stat override 0 value (Light)', 385)])

    # 3. Roster dirs: family-0 peer view (level, Light), family-2 card (level, Light), the rest copied.
    for src_dir, out_dir in ((F / 'rosters-f0-tutorial', OUT / 'rosters-f0-tutorial-l1'),
                             (F / 'grants' / 'rosters-f0-tutorial-khv', OUT / 'grants-l1' / 'rosters-f0-tutorial-khv'),
                             (F / 'roster-def1200-helm-saint14', OUT / 'roster-def-l1')):
        for src in sorted(src_dir.glob('*.bin')):
            out = out_dir / src.name
            if src.name.startswith('f2t1-'):
                edit(src, out, CARD2, [(0x10, 'u32', LEVEL, 'level', 40), (0x14, 'u32', LIGHT, 'Light', 385)])
            elif src.name.startswith('f0t1-') and src_dir.name != 'roster-def1200-helm-saint14':
                edit(src, out, PEER, peer_changes())
            elif src.name.startswith('f0t1-'):
                pass   # below, after the derivations
            else:
                copy(src, out, 0)

    # 4. Defense and Attack at item level 1, through the client's own curves (derived-stat).
    with tempfile.TemporaryDirectory() as t:
        scratch = Path(t) / 'derived'; scratch.mkdir()
        inv = Path(t) / 'inventory-l1-with-rifle'; inv.mkdir()
        for f in (OUT / 'inventory-tutorial-l1').glob('*.bin'):
            shutil.copyfile(f, inv / f.name)
        shutil.copyfile(OUT / 'grants-l1' / 'khvostov-3598.bin', inv / 'khvostov-3598.bin')
        # Defense: the served inventory (helmet 6514) against the khv card, which names the same items.
        defense, _ = derive(OUT / 'grants-l1' / 'rosters-f0-tutorial-khv' / 'f0t1-d100000000000501.bin', inv, 'character', 1, scratch)
        # The helm-saint14 card names helmet content 1065 for SOID d1..0303; derived-stat refuses the
        # mismatch, so a stand-in copy of the helmet with that content (scratch only, never served)
        # gives the card's own reading (Attack is the rifle's alone, so it is the same either way).
        stand = Path(t) / 'inventory-l1-saint14-standin'; shutil.copytree(inv, stand)
        h = bytearray(decode(stand / '02-Helmet.bin', ITEM)); struct.pack_into('<H', h, 0x24, SAINT14_HELMET)
        encode(bytes(h), ITEM, stand / '02-Helmet.bin')
        src = F / 'roster-def1200-helm-saint14' / 'f0t1-d100000000000501.bin'
        pre = decode(src, PEER)
        if pre[0x840] != 1 or pre[0x8B8] != 11:
            sys.exit(f'{src}: Defense/Attack keys are not where expected')
        staged = scratch / 'helm-level-light.bin'
        b = bytearray(pre)
        for at, kind, value, *_ in peer_changes():
            struct.pack_into('<f' if kind == 'f32' else '<I', b, at, value)
        encode(bytes(b), PEER, staged)
        saint_defense, _ = derive(staged, stand, 'character', 1, scratch)
        attack, attacked = derive(staged, stand, 'primary', 11, scratch)
        a_obj = decode(attacked, PEER)
        if u32(a_obj, 0x8BC) != attack['value'] or a_obj[0x8B8] != 11:
            sys.exit('derived-stat did not write Attack into the existing key-11 entry')
        D = defense['value']
        edit(src, OUT / 'roster-def-l1' / src.name, PEER, peer_changes() + [
            (0x844, 'u32', D, 'stat key 1 (Defense)', 1200),
            (0x8BC, 'u32', attack['value'], 'primary table key 11 (Attack)', 200)])
        final = decode(OUT / 'roster-def-l1' / src.name, PEER)
        if final != a_obj[:0x840] + final[0x840:0x848] + a_obj[0x848:]:
            sys.exit('roster-def-l1 differs from the derived-stat output outside the Defense entry')

    (OUT / 'checks').mkdir(exist_ok=True)
    (OUT / 'checks' / 'report.json').write_text(json.dumps({
        'level': LEVEL, 'light': LIGHT, 'light_status': 'TODO: aggregation not decoded; see README',
        'derived_defense_served_inventory': defense, 'derived_defense_saint14_standin': saint_defense,
        'derived_attack': attack, 'objects': REPORT}, indent=2))
    print(f'Defense {D} (served inventory; {saint_defense["value"]} with the card\'s Saint-14 helmet), '
          f'Attack {attack["value"]}, Light {LIGHT} (TODO); {len(REPORT)} objects; checks/report.json')
    return 0


if __name__ == '__main__':
    sys.exit(main())
