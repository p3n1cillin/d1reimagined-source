#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Generate the compact reward and growth tables d1-responder's `rewards` module reads.

    gen_rewards_tables.py --manifest <dim-d1-manifest dir> [--cryptarch <cryptarch-guide data dir>]
                          [--out <dir>]

Inputs (local only; neither is committed):
  --manifest   Bungie's final D1 API manifest as archived by DIM: `d1-manifest-base.json.gz`.
  --cryptarch  nruble/cryptarch-guide `public/data` (GPL-3.0): community-collated engram contents
               and per-activity Legendary Mark limits. Optional; without it the community tables
               are written empty.

Output: tab-separated tables under `ps4/d1-server/crates/d1-responder/src/rewards/tables/`
(the default `--out`). Each starts with `#` lines naming its source and the SHA-256 of the input
file it was read from. Only facts the services need are kept (hashes, tiers, buckets, counts,
thresholds, point values); no names, descriptions or other text from either source.

The Grimoire comes from the cryptarch-guide copy of the manifest
(`d1_manifest/DestinyGrimoireDefinition.json` and `DestinyGrimoireCardDefinition.json`), because
DIM's archive has no Grimoire tables. Those files are Bungie's data too.
"""
import argparse
import gzip
import hashlib
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, '..', '..'))
DEFAULT_OUT = os.path.join(REPO, 'ps4', 'd1-server', 'crates', 'd1-responder', 'src', 'rewards', 'tables')

# Buckets whose items the reward services hand out, decrypt or count (DestinyInventoryBucketDefinition).
GEAR_BUCKETS = {
    3284755031,  # Subclass
    1498876634, 2465295065, 953998645,  # Primary, Special, Heavy
    3448274439, 3551918588, 14239492, 20886954, 1585787867,  # Helmet, Gauntlets, Chest, Legs, Class
    4023194814, 434908299,  # Ghost, Artifacts
    2973005342, 3313201758,  # Shaders, Ornaments
    2025709351, 284967655, 3054419239, 4274335291, 3796357825,  # Vehicle, Ships, Emotes, Emblems, Horn
}
STACK_BUCKETS = {
    3865314626,  # Materials
    1469714392,  # Consumables
    2689798304, 2689798308, 2689798309, 2689798310, 2689798311,  # Legendary Marks, Glimmer, Marks, Silver
}
ITEM_TYPE_ENGRAM = 8
# Armour stats an instance rolls: STAT_INTELLECT, STAT_DISCIPLINE, STAT_STRENGTH.
ROLLED_STATS = (144602215, 1735777505, 4244567218)
QUEST_BUCKET = 1801258597


def sha256(path):
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        for chunk in iter(lambda: f.read(1 << 20), b''):
            h.update(chunk)
    return h.hexdigest()


def write(out, name, header, rows):
    path = os.path.join(out, name)
    with open(path, 'w', encoding='utf-8', newline='\n') as f:
        for line in header:
            f.write('# ' + line + '\n')
        for row in rows:
            f.write('\t'.join(str(c) for c in row) + '\n')
    print(f'{name}: {len(rows)} rows, {os.path.getsize(path)} bytes')


def joined(values, sep=','):
    values = list(values)
    return sep.join(str(v) for v in values) if values else '-'


def items_table(m):
    rows = []
    for v in m['DestinyInventoryItemDefinition'].values():
        bucket = v.get('bucketTypeHash', 0)
        engram = v.get('itemType') == ITEM_TYPE_ENGRAM
        if not (bucket in GEAR_BUCKETS or bucket in STACK_BUCKETS or engram):
            continue
        prim = v.get('primaryBaseStatHash', 0) or 0
        stat = (v.get('stats') or {}).get(str(prim)) or {}
        rolls = []
        for h in ROLLED_STATS:
            r = (v.get('stats') or {}).get(str(h))
            if r and r.get('maximum', 0) > 0:
                rolls.append(f"{h}:{r.get('minimum', 0)}:{r['maximum']}")
        rows.append((
            v['itemHash'], v.get('tierType', 0), bucket, v.get('itemType', 0), v.get('classType', 3),
            int(bool(v.get('instanced'))), int(bool(v.get('equippable'))), v.get('talentGridHash', 0) or 0,
            v.get('maxStackSize', 1), prim, stat.get('minimum', 0), stat.get('maximum', 0), joined(rolls),
        ))
    rows.sort()
    return rows


def progressions_table(m):
    rows = []
    for v in m['DestinyProgressionDefinition'].values():
        steps = [s.get('progressTotal', 0) for s in v.get('steps') or []]
        rows.append((v['progressionHash'], v.get('name') or '-', v.get('scope', 0),
                     int(bool(v.get('repeatLastStep'))), joined(steps)))
    rows.sort()
    return rows


def grids_table(m, wanted):
    """One row per talent grid; nodes as `index:row:column:gridLevel:flags:steps:prereqs:exclusiveWith`,
    `;`-separated. flags: 1 autoUnlocks, 2 isRandom, 4 lastStepRepeats, 8 isRandomRepurchasable."""
    rows = []
    for v in m['DestinyTalentGridDefinition'].values():
        if v['gridHash'] not in wanted:
            continue
        nodes = []
        for n in v.get('nodes') or []:
            steps = n.get('steps') or []
            level = steps[0]['activationRequirement']['gridLevel'] if steps else 0
            flags = (1 if n.get('autoUnlocks') else 0) | (2 if n.get('isRandom') else 0) \
                | (4 if n.get('lastStepRepeats') else 0) | (8 if n.get('isRandomRepurchasable') else 0)
            nodes.append(':'.join(str(x) for x in (
                n['nodeIndex'], n.get('row', 0), n.get('column', 0), level, flags, len(steps),
                joined(n.get('prerequisiteNodeIndexes') or [], '.'),
                joined(n.get('exlusiveWithNodes') or [], '.'),
            )))
        rows.append((v['gridHash'], v.get('progressionHash', 0) or 0, v.get('maxGridLevel', 0), joined(nodes, ';')))
    rows.sort()
    return rows


def activities_table(m):
    rows = []
    for v in m['DestinyActivityDefinition'].values():
        rewards = []
        for group in v.get('rewards') or []:
            for r in group.get('rewardItems') or []:
                rewards.append(f"{r['itemHash']}:{r.get('value', 0)}")
        rows.append((v['activityHash'], v.get('activityLevel', 0), v.get('activityPower', 0) or 0,
                     v.get('tier', 0), v.get('activityTypeHash', 0), joined(rewards)))
    rows.sort()
    return rows


def reward_display_map(m, strings):
    """Activity reward lists name display items (type 0, stack 1) such as 'Legendary Marks'
    3510203897. Map each to the real stackable item with the same English name and a stack above 1."""
    items = m['DestinyInventoryItemDefinition']
    names = strings['DestinyInventoryItemDefinition']
    by_name = {}
    for h, v in items.items():
        name = (names.get(h) or {}).get('itemName')
        if name and v.get('maxStackSize', 1) > 1 and v.get('bucketTypeHash') in STACK_BUCKETS:
            by_name.setdefault(name, []).append(v['itemHash'])
    used = set()
    for v in m['DestinyActivityDefinition'].values():
        for group in v.get('rewards') or []:
            for r in group.get('rewardItems') or []:
                used.add(r['itemHash'])
    rows = []
    for h in sorted(used):
        name = (names.get(str(h)) or {}).get('itemName')
        targets = by_name.get(name, [])
        if len(targets) == 1 and targets[0] != h:
            rows.append((h, targets[0]))
    return rows


def weekly_bonus_markers(m, strings):
    """Activity reward lists carry marker items named '(Weekly Bonus Remaining: N)'. Keep the hash
    and N only."""
    names = strings['DestinyInventoryItemDefinition']
    rows = []
    for h, v in m['DestinyInventoryItemDefinition'].items():
        mm = re.fullmatch(r'\(Weekly Bonus Remaining: (\d+)\)', (names.get(h) or {}).get('itemName') or '')
        if mm:
            rows.append((v['itemHash'], int(mm.group(1))))
    return sorted(rows)


def records_tables(m):
    books, records, objectives = [], [], {}
    for v in m['DestinyRecordBookDefinition'].values():
        recs, rewards = [], []
        for page in v.get('pages') or []:
            recs += [r['recordHash'] for r in page.get('records') or []]
            for r in page.get('rewards') or []:
                rewards.append(f"{r.get('requirementProgressionLevel', 0)}:{r['itemHash']}:{r.get('quantity', 1)}")
        books.append((v['hash'], v.get('progressionHash', 0) or 0, v.get('itemHash', 0) or 0,
                      v.get('recordCount', 0), joined(recs), joined(rewards)))
    for v in m['DestinyRecordDefinition'].values():
        objs = [o['objectiveHash'] for o in v.get('objectives') or []]
        records.append((v['hash'], joined(objs)))
    for v in m['DestinyObjectiveDefinition'].values():
        objectives[v['objectiveHash']] = (v['objectiveHash'], v.get('completionValue', 0),
                                          int(bool(v.get('isCountingDownward'))),
                                          int(bool(v.get('allowNegativeValue'))))
    return sorted(books), sorted(records), sorted(objectives.values())


def quests_table(m):
    """Quest steps: items in the Quests bucket that carry objectives, with their questline and the
    questline's step order (the questline item's setItemHashes)."""
    items = m['DestinyInventoryItemDefinition']
    rows = []
    for v in items.values():
        if v.get('bucketTypeHash') != QUEST_BUCKET or not v.get('objectiveHashes'):
            continue
        line = v.get('questlineItemHash', 0) or 0
        order = (items.get(str(line)) or {}).get('setItemHashes') or []
        pos = order.index(v['itemHash']) if v['itemHash'] in order else -1
        rows.append((v['itemHash'], line, pos, joined(v['objectiveHashes'])))
    rows.sort()
    return rows


def grimoire_tables(path):
    g = json.load(open(os.path.join(path, 'DestinyGrimoireDefinition.json'), encoding='utf-8'))
    cards = json.load(open(os.path.join(path, 'DestinyGrimoireCardDefinition.json'), encoding='utf-8'))
    root = g['0'] if '0' in g else g
    rows, brief_total = [], 0
    for ti, theme in enumerate(root['themeCollection']):
        for pi, page in enumerate(theme['pageCollection']):
            for b in page['cardBriefs']:
                brief_total += b.get('totalPoints', 0)
                c = cards.get(str(b['cardId'])) or {}
                stats = []
                for s in c.get('statisticCollection') or []:
                    ranks = ','.join(f"{r['threshold']}:{r.get('points', 0)}" for r in s.get('rankCollection') or [])
                    stats.append(f"{s['statNumber']}={ranks or '-'}")
                rows.append((b['cardId'], ti, pi, c.get('points', b.get('points', 0)), joined(stats, ';')))
    return rows, brief_total


ENGRAM_SLOTS = {
    'helmet': 3448274439, 'guantlets': 3551918588, 'chest': 14239492, 'leg': 20886954,
    'class-item': 1585787867, 'primary': 1498876634, 'special': 2465295065, 'heavy': 953998645,
}


# Engrams bought for Silver Dust (patch 2.6.0: their items dismantle into Silver Dust); tier 0 rows.
SILVER_DUST_BOXES = {'dusty-iron-engram', 'triad-engram', 'arena-engram', 'exodus-engram', 'warpath-engram',
                     'unity-engram', 'icy-engram'}


def engram_pools(data):
    """Community decrypt pools: `<tier>-<slot>-engram` pages (and their legacy- variants), and the
    Silver Dust engrams' contents as tier-0, bucket-0 rows."""
    box = json.load(open(os.path.join(data, 'rewardBoxData.json'), encoding='utf-8'))
    rows = []
    for slug, page in box.items():
        mm = re.fullmatch(r'(legacy-)?(legendary|exotic)-(.+)-engram', slug)
        if slug in SILVER_DUST_BOXES:
            tier, bucket, legacy = 0, 0, 0
        elif mm and mm.group(3) in ENGRAM_SLOTS:
            tier = 5 if mm.group(2) == 'legendary' else 6
            bucket, legacy = ENGRAM_SLOTS[mm.group(3)], int(bool(mm.group(1)))
        else:
            continue
        hashes = set()
        for section in page.get('sections') or []:
            displays = [section.get('wideItemDisplay') or {}] + list(section.get('dividedItemDisplay') or [])
            for disp in displays:
                for s in disp.get('sets') or []:
                    hashes.update(int(h) for h in s.get('setItems') or [] if h and h != '0')
                hashes.update(int(h) for h in disp.get('items') or [] if h and h != '0')
        rows.append((slug, tier, bucket, legacy, joined(sorted(hashes))))
    rows.sort()
    return rows


def mark_limits(data):
    """Per-activity Legendary Mark grants and their limits, parsed from the guide's reward lines
    (`Legendary Marks +10 (Account Weekly Limit: 3 | Shared with ...)`). Only the numbers are kept."""
    pages = json.load(open(os.path.join(data, 'activityPageData.json'), encoding='utf-8'))
    pat = re.compile(r'Legendary Marks \+(\d+)(?: \(([^)]*)\))?')
    rows = []
    for slug, page in pages.items():
        if not slug:
            continue
        for section in page.get('sections') or []:
            for r in section.get('summaryRewards') or []:
                mm = pat.search(r.get('rewardText') or '')
                if not mm:
                    continue
                note = mm.group(2) or ''
                lim = re.search(r'(Daily|Weekly)[^:]*Limit: (\d+)', note)
                period = lim.group(1).lower() if lim else '-'
                count = lim.group(2) if lim else '-'
                scope = 'account' if 'Account' in note else '-'
                rows.append((slug, mm.group(1), period, count, scope))
    rows.sort()
    return rows


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--manifest', required=True, help='directory holding d1-manifest-base.json.gz')
    ap.add_argument('--cryptarch', help='nruble/cryptarch-guide public/data directory')
    ap.add_argument('--out', default=DEFAULT_OUT)
    a = ap.parse_args()

    base = os.path.join(a.manifest, 'd1-manifest-base.json.gz')
    strings = os.path.join(a.manifest, 'd1-manifest-strings-en.json.gz')
    m = json.load(gzip.open(base))
    st = json.load(gzip.open(strings))
    os.makedirs(a.out, exist_ok=True)
    src = f"Bungie's final D1 manifest (DIM archive), d1-manifest-base.json.gz sha256 {sha256(base)}"
    gen = 'generated by tools/rewards_tables/gen_rewards_tables.py; Bungie data, facts only; do not edit'

    items = items_table(m)
    write(a.out, 'items.tsv', [src, gen, 'hash\ttier\tbucket\titemType\tclassType\tinstanced\tequippable'
                               '\ttalentGrid\tmaxStack\tprimaryStat\tprimaryMin\tprimaryMax\trolledStats (hash:min:max)'], items)
    write(a.out, 'progressions.tsv', [src, gen, 'hash\tscope\trepeatLastStep\tstepCosts'], progressions_table(m))
    wanted = {r[7] for r in items if r[7]}
    write(a.out, 'talent_grids.tsv', [src, gen, 'grid\tprogression\tmaxGridLevel\tnodes (index:row:column:'
                                      'gridLevel:flags:steps:prereqs:exclusiveWith; flags 1 auto 2 random '
                                      '4 lastStepRepeats 8 randomRepurchasable)'], grids_table(m, wanted))
    write(a.out, 'activities.tsv', [src, gen, 'hash\tactivityLevel\tactivityPower\ttier\tactivityType'
                                    '\trewards (displayItem:value)'], activities_table(m))
    write(a.out, 'reward_display.tsv', [src, 'strings: d1-manifest-strings-en.json.gz sha256 ' + sha256(strings),
                                        gen, 'a reward display item and the stackable item of the same name',
                                        'display\titem'], reward_display_map(m, st))
    write(a.out, 'weekly_bonus.tsv', [src, 'strings: d1-manifest-strings-en.json.gz sha256 ' + sha256(strings),
                                      gen, 'reward marker item and the weekly bonus count its name states',
                                      'marker	remaining'], weekly_bonus_markers(m, st))
    books, records, objectives = records_tables(m)
    write(a.out, 'record_books.tsv', [src, gen, 'book\tprogression\titem\trecordCount\trecords'
                                      '\trewards (rank:item:quantity)'], books)
    write(a.out, 'records.tsv', [src, gen, 'record\tobjectives'], records)
    write(a.out, 'objectives.tsv', [src, gen, 'objective\tcompletionValue\tcountingDown\tallowNegative'], objectives)
    write(a.out, 'quests.tsv', [src, gen, 'step\tquestline\tposition\tobjectives'], quests_table(m))

    if a.cryptarch:
        gdir = os.path.join(a.cryptarch, 'd1_manifest')
        gsrc = ("Bungie's D1 manifest Grimoire tables as copied in nruble/cryptarch-guide, "
                f"DestinyGrimoireCardDefinition.json sha256 {sha256(os.path.join(gdir, 'DestinyGrimoireCardDefinition.json'))}")
        cards, brief_total = grimoire_tables(gdir)
        write(a.out, 'grimoire.tsv', [gsrc, gen, f'manifest cardBriefs totalPoints sum = {brief_total}',
                                      'card\ttheme\tpage\tpoints\tstats (number=threshold:points,...;...)'], cards)
        csrc = ('COMMUNITY STAND-IN: nruble/cryptarch-guide (GPL-3.0) rewardBoxData.json sha256 '
                + sha256(os.path.join(a.cryptarch, 'rewardBoxData.json')))
        write(a.out, 'engram_pools.tsv', [csrc, gen.replace('Bungie data', 'community data'),
                                          'slug\ttier\tbucket\tlegacy\titems'], engram_pools(a.cryptarch))
        msrc = ('COMMUNITY STAND-IN: nruble/cryptarch-guide (GPL-3.0) activityPageData.json sha256 '
                + sha256(os.path.join(a.cryptarch, 'activityPageData.json')))
        write(a.out, 'mark_limits.tsv', [msrc, gen.replace('Bungie data', 'community data'),
                                         'activity\tmarks\tperiod\tlimit\tscope'], mark_limits(a.cryptarch))
    else:
        print('no --cryptarch: grimoire, engram_pools and mark_limits not written', file=sys.stderr)


if __name__ == '__main__':
    main()
