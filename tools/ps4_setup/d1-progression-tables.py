# SPDX-License-Identifier: GPL-2.0-or-later
"""Write the progression curves and vendor reset schedules the player store uses, from Bungie's D1 manifest.

    python -I d1-progression-tables.py MANIFEST_BASE [--strings MANIFEST_STRINGS] [--out DIR]

MANIFEST_BASE is Bungie's final D1 API manifest as Destiny Item Manager archives it
(`d1-manifest-base.json.gz`, with `d1-manifest-strings-en.json.gz` for vendor names). It is Bungie's
data and a local input: never commit it. The two tables this writes hold numbers derived from it,
each row citing its definition hash, and are committed under ps4/fixtures/56578/progression:

- progression-curves.tsv: DestinyProgressionDefinition steps (progressTotal per step) for
  character_level, character_prestige and every faction (reputation/rank) progression.
  `runtime_index` is the 56578 client's own u16 ordinal for the progression, which the manifest
  does not carry: 117 character_level and 118 character_prestige are the ids the client's
  completion path reads from the investment globals (+0x166, +0x16a;
  notes/ps4-56578-postgame-2026-10-29.md). The others are `-` until someone reads them.
- vendor-resets.tsv: DestinyVendorDefinition.summary resetIntervalMinutes / resetOffsetMinutes for
  every vendor with a schedule. A vendor resets at epoch minute m when (m + offset) mod interval == 0.

Read-only on the inputs; overwrites the two output files.
"""
import argparse
import gzip
import json
import os
import sys

RUNTIME_INDEX = {'character_level': 117, 'character_prestige': 118}
CURVES_HEADER = ('hash', 'name', 'scope', 'repeat_last', 'runtime_index', 'steps')
RESETS_HEADER = ('vendor_hash', 'name', 'reset_interval_minutes', 'reset_offset_minutes', 'progression_hash')


def load(path):
    opener = gzip.open if path.endswith('.gz') else open
    with opener(path, 'rt', encoding='utf-8') as f:
        return json.load(f)


def wanted(name):
    return name in RUNTIME_INDEX or 'faction' in name


def curves(base):
    rows = []
    for key, d in base['DestinyProgressionDefinition'].items():
        name = d.get('name') or ''
        steps = [int(s.get('progressTotal', 0)) for s in d.get('steps', [])]
        if not wanted(name) or not steps:
            continue
        index = RUNTIME_INDEX.get(name)
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
        name = (names.get(key, {}).get('summary') or {}).get('vendorName') or '-'
        rows.append((int(s.get('vendorHash', key)), name, interval, int(s.get('resetOffsetMinutes', 0)),
                     int(s.get('progressionHash', 0))))
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
    here = os.path.dirname(os.path.abspath(__file__))
    ap.add_argument('--out', default=os.path.join(here, '..', '..', 'ps4', 'fixtures', '56578', 'progression'))
    a = ap.parse_args(argv)
    base = load(a.manifest)
    strings = load(a.strings) if a.strings else None
    os.makedirs(a.out, exist_ok=True)
    c, r = curves(base), resets(base, strings)
    write(os.path.join(a.out, 'progression-curves.tsv'), CURVES_HEADER, c, 'DestinyProgressionDefinition')
    write(os.path.join(a.out, 'vendor-resets.tsv'), RESETS_HEADER, r, 'DestinyVendorDefinition.summary')
    print(f'{len(c)} progression curves, {len(r)} vendor schedules -> {os.path.normpath(a.out)}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
