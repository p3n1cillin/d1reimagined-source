# SPDX-License-Identifier: GPL-2.0-or-later
"""Write the progression curves and vendor reset schedules the player store uses, from Bungie's D1 manifest.

    python -I d1-progression-tables.py MANIFEST_BASE [--strings MANIFEST_STRINGS] [--out DIR]

MANIFEST_BASE is Bungie's final D1 API manifest as Destiny Item Manager archives it
(`d1-manifest-base.json.gz`, with `d1-manifest-strings-en.json.gz` for vendor names). It is Bungie's
data and a local input: never commit it. The two tables this writes hold numbers derived from it,
each row citing its definition hash, and are committed under ps4/fixtures/56578/progression:

- progression-curves.tsv: DestinyProgressionDefinition steps (progressTotal per step) for
  character_level, character_prestige, every faction (reputation/rank) progression, the Crucible
  and Trials of Osiris rank and ticket progressions, Iron Banner's loss tokens, the record books,
  and every progression a vendor's summary names (e.g. the Shipwright's racing rank).
  `runtime_index` is the 56578 client's own u16 ordinal for the progression, which the manifest
  does not carry. The owned-content exporter supplies all 252 ordinals in
  progression-index.tsv; --indices selects that input. Unknown hashes stay labelled `-`.
- vendor-resets.tsv: DestinyVendorDefinition.summary resetIntervalMinutes / resetOffsetMinutes for
  every vendor with a schedule. A vendor resets at epoch minute m when (m + offset) mod interval == 0.
- refund-policies.tsv: every vendor with a refundable sale (DestinyVendorDefinition.sales[]
  refundPolicy != 0), how many of its sales are, their refundLimit (minutes) and their item hashes.
  In the final manifest only Eververse and its Silver Dust store qualify (policy 2, 15 minutes).

Read-only on the inputs; overwrites the three output files.
"""
import argparse
import gzip
import json
import os
import sys

CHARACTER_CURVES = {'character_level', 'character_prestige'}
INDEX_TABLE = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', 'ps4', 'fixtures',
                          '56578', 'progression', 'progression-index.tsv')


def runtime_indices(path=INDEX_TABLE):
    with open(path, encoding='utf-8') as f:
        rows = [l.rstrip('\n').split('\t') for l in f if l.strip() and not l.startswith('#')]
    return {int(h): int(i) for i, h in rows[1:]}
CURVES_HEADER = ('hash', 'name', 'scope', 'repeat_last', 'runtime_index', 'steps')
RESETS_HEADER = ('vendor_hash', 'name', 'reset_interval_minutes', 'reset_offset_minutes', 'progression_hash')
REFUNDS_HEADER = ('vendor_hash', 'name', 'refund_policy', 'refund_limit_minutes', 'refundable_sales', 'sales',
                  'refundable_item_hashes')
# Rank and ticket progressions kept besides the factions (name substrings).
RANKS = ('faction', 'trials_of_osiris', 'pvp_tournament', 'pvp_iron_banner', 'record_books')


def load(path):
    opener = gzip.open if path.endswith('.gz') else open
    with opener(path, 'rt', encoding='utf-8') as f:
        return json.load(f)


def wanted(name, key, vendor_progressions):
    return name in CHARACTER_CURVES or any(r in name for r in RANKS) or int(key) in vendor_progressions


def curves(base, indices=None):
    indices = runtime_indices() if indices is None else indices
    vendor_progressions = {int(d.get('summary', {}).get('progressionHash', 0))
                           for d in base.get('DestinyVendorDefinition', {}).values()} - {0}
    rows = []
    for key, d in base['DestinyProgressionDefinition'].items():
        name = d.get('name') or ''
        steps = [int(s.get('progressTotal', 0)) for s in d.get('steps', [])]
        if not wanted(name, d.get('hash', key), vendor_progressions) or not steps:
            continue
        index = indices.get(int(d.get('hash', key)))
        rows.append((int(d.get('hash', key)), name, int(d.get('scope', 0)), int(bool(d.get('repeatLastStep'))),
                     '-' if index is None else str(index), ','.join(map(str, steps))))
    return sorted(rows, key=lambda r: (r[4] == '-', r[1]))


def resets(base, strings):
    names = (strings or {}).get('DestinyVendorDefinition', {})
    rows = []
    for key, d in base['DestinyVendorDefinition'].items():
        s = d.get('summary', {})
        interval = int(s.get('resetIntervalMinutes', 0))
        if interval <= 0:
            continue
        name = vendor_name(names, key)
        rows.append((int(s.get('vendorHash', key)), name, interval, int(s.get('resetOffsetMinutes', 0)),
                     int(s.get('progressionHash', 0))))
    return sorted(rows, key=lambda r: (r[1], r[0]))


def vendor_name(names, key):
    return ((names.get(key, {}).get('summary') or {}).get('vendorName') or '-').replace('	', ' ')


def refunds(base, strings):
    names = (strings or {}).get('DestinyVendorDefinition', {})
    rows = []
    for key, d in base['DestinyVendorDefinition'].items():
        sales = d.get('sales', [])
        refundable = [s for s in sales if int(s.get('refundPolicy') or 0) != 0]
        if not refundable:
            continue
        policies = sorted({int(s['refundPolicy']) for s in refundable})
        limits = sorted({int(s.get('refundLimit') or 0) for s in refundable})
        items = sorted({int(s.get('itemHash', 0)) for s in refundable})
        rows.append((int(d.get('summary', {}).get('vendorHash', key)), vendor_name(names, key),
                     ','.join(map(str, policies)), ','.join(map(str, limits)), len(refundable), len(sales),
                     ','.join(map(str, items))))
    return sorted(rows, key=lambda r: (r[1], r[0]))


def write(path, header, rows, source):
    with open(path, 'w', encoding='utf-8', newline='\n') as f:
        f.write(f'# Derived by tools/ps4_setup/d1-progression-tables.py from {source}.\n')
        f.write("# Bungie's data (D1 API manifest, as archived by DIM); the numbers are cited to it, the manifest is not committed.\n")
        f.write('\t'.join(header) + '\n')
        for r in rows:
            f.write('\t'.join(map(str, r)) + '\n')


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    ap.add_argument('manifest')
    ap.add_argument('--strings')
    ap.add_argument('--indices', default=INDEX_TABLE, help='56578 progression-index.tsv from d1-oracle content-tables')
    here = os.path.dirname(os.path.abspath(__file__))
    ap.add_argument('--out', default=os.path.join(here, '..', '..', 'ps4', 'fixtures', '56578', 'progression'))
    a = ap.parse_args(argv)
    base = load(a.manifest)
    strings = load(a.strings) if a.strings else None
    os.makedirs(a.out, exist_ok=True)
    c, r, f = curves(base, runtime_indices(a.indices)), resets(base, strings), refunds(base, strings)
    write(os.path.join(a.out, 'progression-curves.tsv'), CURVES_HEADER, c, 'DestinyProgressionDefinition')
    write(os.path.join(a.out, 'vendor-resets.tsv'), RESETS_HEADER, r, 'DestinyVendorDefinition.summary')
    write(os.path.join(a.out, 'refund-policies.tsv'), REFUNDS_HEADER, f,
          'DestinyVendorDefinition.sales[] refundPolicy/refundLimit')
    print(f'{len(c)} progression curves, {len(r)} vendor schedules, {len(f)} refunding vendors'
          f' -> {os.path.normpath(a.out)}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
