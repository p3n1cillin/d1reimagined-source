# SPDX-License-Identifier: GPL-2.0-or-later
"""Generate the script bridge's activity table (`D1_AH_SCRIPTS_ACTIVITIES`) for every story mission,
cinematic and quest mission of build 56578.

    d1-scripts-table.py --manifest DIR [--table-2014 activities.csv] [--worlds DIR] [--out FILE]

Inputs:
  - stream E's generated world tables (`ps4/fixtures/56578/worlds/`, committed): each activity's
    world and start slice set, and each slice set's bundles and sensor clients;
  - Bungie's final D1 manifest (DIM's `d1-manifest-base.json.gz`, a LOCAL input): each activity's
    type and place hashes, for the `activity_complete` body;
  - Blam Network's 2014 activity table (tiger's dictionary, `activities.csv`, a LOCAL input): the
    next activity of the story arc (cine_reveal -> the Tower), matched by world and mapped to 56578
    by hash.

Output, identifiers only: one line per activity,
  `INDEX WORLD GROUP @generated start=N hash=H [kind=cinematic] type=H type2=H place=H [next=I:H]`
with the activity's world as the `@activity` the sidecar runs (the script folder names each
activity after its world). `@generated` makes the bridge take the world's sensor list for the start
slice set from the world tables; GROUP is the group of that list's lifetime record.

The body's hashes, from the live completions (notes/ps4-56578-wire-catalog-2026-11-05.md):
  +0x48 type   the manifest's activity type, except a cinematic-only activity (STORY_CINEMATIC,
               CREDITS), whose live bodies carry Story (ACTIVITY_TYPE_STORY) there;
  +0x4c type2  the manifest's activity type (STORY_CINEMATIC 0x4d78882e on the cinematic bodies);
  +0x54 place  the manifest's place hash.
"""
import argparse
import csv
import gzip
import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
BUILD = '56578.17.04.12.1251.evil'
STORY = 'ACTIVITY_TYPE_STORY'
CINEMATIC_ONLY = {'STORY_CINEMATIC', 'CREDITS'}
KINDS = {STORY, 'QUEST', 'QUEST_SOLO'} | CINEMATIC_ONLY
LIFETIME = 0x10


def tsv(path):
    lines = [l for l in Path(path).read_text(encoding='utf-8').splitlines() if l and not l.startswith('#')]
    header = lines[0].split('\t')
    return [dict(zip(header, l.split('\t'))) for l in lines[1:]]


def num(s):
    return int(s, 16) if s.startswith('0x') else int(s)


def lifetime_group(world, start, slices, bundles):
    """The group of the lifetime record in `start`'s sensor list (generated.rs `plan`)."""
    s = slices.get((world, start))
    if s is None:
        return None
    bubble = start // 8
    for tag in s:
        for group, lb, clients in bundles.get(tag, []):
            if lb not in (-1, bubble):
                continue
            if any(c[0] == LIFETIME for c in clients):
                return group
    return None


def load_world_tables(d):
    slices = {}
    for r in tsv(Path(d) / 'slice-sets.tsv'):
        tags = []
        for col in ('global', 'authoritative', 'private'):
            if r[col] != '-':
                tags += [num(t) for t in r[col].split(',')]
        slices[(r['world'], int(r['slice_set']))] = tags
    bundles = {}
    for r in tsv(Path(d) / 'bundles.tsv'):
        clients = [tuple(int(x) for x in tok.split(':')[:2]) for tok in r['clients'].split()]
        bundles.setdefault(num(r['bundle_tag']), []).append((num(r['group']), int(r['lookup_bubble']), clients))
    return slices, bundles, tsv(Path(d) / 'activities.tsv')


def load_manifest(d):
    data = json.load(gzip.open(Path(d) / 'd1-manifest-base.json.gz'))
    types = {int(k): v.get('identifier') for k, v in data['DestinyActivityTypeDefinition'].items()}
    story = next(h for h, i in types.items() if i == STORY)
    acts = {int(k) & 0xFFFFFFFF: v for k, v in data['DestinyActivityDefinition'].items()}
    return acts, types, story


def next_activities(table_2014, activities):
    """World -> (56578 index, hash) of the 2014 table's next activity for that world."""
    if not table_2014:
        return {}
    rows = list(csv.DictReader(open(table_2014, encoding='utf-8')))
    by_index = {}
    for r in rows:
        by_index.setdefault(r['activity_index'], r)
    by_hash = {num('0x' + a['hash'][2:]): int(a['index']) for a in activities}
    out = {}
    for r in rows:
        nxt = r.get('next_activity', '-1')
        if nxt in ('', '-1') or r['codename'] in out:
            continue
        target = by_index.get(nxt)
        if not target:
            continue
        h = int(target['activity_hash'], 16)
        if h in by_hash:
            out[r['codename']] = (by_hash[h], h)
    return out


def generate(worlds_dir, manifest_dir, table_2014):
    slices, bundles, activities = load_world_tables(worlds_dir)
    acts, types, story = load_manifest(manifest_dir)
    nexts = next_activities(table_2014, activities)
    lines, skipped = [], []
    for a in activities:
        if a['type'] not in KINDS:
            continue
        index, h = int(a['index']), num(a['hash'])
        if a['world'] == '-' or a['start_region'] == '-':
            skipped.append((index, a['name'], a['type'], 'no world in the world tables (' + a['status'] + ')'))
            continue
        world, start = a['world'], int(a['start_region'])
        group = lifetime_group(world, start, slices, bundles)
        if group is None:
            skipped.append((index, a['name'], a['type'], f'{world} slice set {start} has no lifetime record'))
            continue
        m = acts.get(h)
        fields = [str(index), world, f'{group:#010x}', '@generated', f'start={start}', f'hash={h:#010x}']
        if a['type'] in CINEMATIC_ONLY:
            fields.append('kind=cinematic')
        if m:
            t2 = m['activityTypeHash'] & 0xFFFFFFFF
            t = story if types.get(t2) in CINEMATIC_ONLY else t2
            fields += [f'type={t:#010x}', f'type2={t2:#010x}', f'place={m["placeHash"] & 0xFFFFFFFF:#010x}']
        if world in nexts and a['type'] in CINEMATIC_ONLY | {STORY}:
            ni, nh = nexts[world]
            fields.append(f'next={ni}:{nh:#010x}')
        lines.append((index, a['name'], a['type'], a['status'], ' '.join(fields), m is not None))
    return lines, skipped


def render(lines, skipped):
    out = [
        f'# GENERATED by tools/ps4_setup/d1-scripts-table.py from build {BUILD}; do not edit',
        '# The script bridge\'s activity table (D1_AH_SCRIPTS_ACTIVITIES): every story mission, cinematic and',
        '# quest mission with a world in ps4/fixtures/56578/worlds/. Identifiers only. Needs D1_AH_WORLDS',
        '# (the world tables, for @generated) and ps4/fixtures/56578/scripts/slice-states.tsv.',
        f'# {len(lines)} activities; {len(skipped)} more have no world yet (listed at the end).',
    ]
    for index, name, kind, status, line, has_manifest in lines:
        note = '' if has_manifest else '   # no manifest definition: no completion body'
        out.append(f'# {name} ({kind}, world {status})')
        out.append(line + note)
    out.append('#')
    out.append('# Without a world (not served by the bridge until the world tables bind them):')
    for index, name, kind, why in skipped:
        out.append(f'#   {index} {name} ({kind}): {why}')
    return '\n'.join(out) + '\n'


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--manifest', required=True, help='the folder with d1-manifest-base.json.gz (local)')
    ap.add_argument('--table-2014', help="tiger's dictionary activities.csv (local)")
    ap.add_argument('--worlds', default=str(REPO / 'ps4/fixtures/56578/worlds'))
    ap.add_argument('--out', default=str(REPO / 'ps4/fixtures/56578/scripts/activities.txt'))
    args = ap.parse_args(argv)
    lines, skipped = generate(args.worlds, args.manifest, args.table_2014)
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    Path(args.out).write_text(render(lines, skipped), encoding='utf-8', newline='\n')
    print(f'{args.out}: {len(lines)} activities, {len(skipped)} without a world', file=sys.stderr)


if __name__ == '__main__':
    main()
