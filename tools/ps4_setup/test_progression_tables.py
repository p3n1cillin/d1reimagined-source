# SPDX-License-Identifier: GPL-2.0-or-later
"""d1-progression-tables.py on a synthetic manifest (no Bungie data needed)."""
import gzip
import importlib.util
import json
import os
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location('tables', os.path.join(HERE, 'd1-progression-tables.py'))
tables = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tables)

BASE = {
    'DestinyProgressionDefinition': {
        '1716568313': {'hash': 1716568313, 'name': 'character_level', 'scope': 1, 'repeatLastStep': False,
                       'steps': [{'progressTotal': 0}, {'progressTotal': 900}, {'progressTotal': 4100}]},
        '3233510749': {'hash': 3233510749, 'name': 'faction_fotc_vanguard', 'scope': 1, 'repeatLastStep': True,
                       'steps': [{'progressTotal': 1500}, {'progressTotal': 2000}, {'progressTotal': 2500}]},
        '19601097': {'hash': 19601097, 'name': 'weapon_rare_auto_rifle', 'scope': 2, 'steps': [{'progressTotal': 0}]},
        '372013324': {'hash': 372013324, 'name': 'test_faction', 'scope': 1, 'steps': []},
    },
    'DestinyVendorDefinition': {
        '1527174714': {'summary': {'vendorHash': 1527174714, 'resetIntervalMinutes': 1440, 'resetOffsetMinutes': -540,
                                   'progressionHash': 0}},
        '1203215': {'summary': {'vendorHash': 1203215, 'resetIntervalMinutes': 0, 'resetOffsetMinutes': 0}},
    },
}
STRINGS = {'DestinyVendorDefinition': {'1527174714': {'summary': {'vendorName': 'Bounty Tracker'}}}}


class Tables(unittest.TestCase):
    def test_writes_the_curves_and_schedules(self):
        with tempfile.TemporaryDirectory() as d:
            base, strings, out = os.path.join(d, 'b.json.gz'), os.path.join(d, 's.json'), os.path.join(d, 'out')
            with gzip.open(base, 'wt', encoding='utf-8') as f:
                json.dump(BASE, f)
            with open(strings, 'w', encoding='utf-8') as f:
                json.dump(STRINGS, f)
            self.assertEqual(tables.main([base, '--strings', strings, '--out', out]), 0)
            curves = [l.split('\t') for l in open(os.path.join(out, 'progression-curves.tsv')).read().splitlines()
                      if not l.startswith('#')]
            self.assertEqual(curves[0], list(tables.CURVES_HEADER))
            self.assertEqual(curves[1], ['1716568313', 'character_level', '1', '0', '117', '0,900,4100'])
            self.assertEqual(curves[2], ['3233510749', 'faction_fotc_vanguard', '1', '1', '-', '1500,2000,2500'])
            self.assertEqual(len(curves), 3, 'weapon progressions and step-less factions are left out')
            resets = [l.split('\t') for l in open(os.path.join(out, 'vendor-resets.tsv')).read().splitlines()
                      if not l.startswith('#')]
            self.assertEqual(resets[1:], [['1527174714', 'Bounty Tracker', '1440', '-540', '0']])


if __name__ == '__main__':
    unittest.main()
