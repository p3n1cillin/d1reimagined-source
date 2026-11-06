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
        '691336206': {'hash': 691336206, 'name': 'pvp_tournament0.wins', 'scope': 1, 'steps': [{'progressTotal': 1}]},
        '2576753410': {'hash': 2576753410, 'name': 'd15.winter.racing', 'scope': 1, 'repeatLastStep': True,
                       'steps': [{'progressTotal': 1000}, {'progressTotal': 1250}]},
        '2033897742': {'hash': 2033897742, 'name': 'weekly_pve', 'scope': 1, 'steps': [{'progressTotal': 1}]},
    },
    'DestinyVendorDefinition': {
        '1527174714': {'summary': {'vendorHash': 1527174714, 'resetIntervalMinutes': 1440, 'resetOffsetMinutes': -540,
                                   'progressionHash': 0}},
        '1203215': {'summary': {'vendorHash': 1203215, 'resetIntervalMinutes': 0, 'resetOffsetMinutes': 0}},
        '459708109': {'summary': {'vendorHash': 459708109, 'resetIntervalMinutes': 4320, 'resetOffsetMinutes': 3840,
                                  'progressionHash': 2576753410}},
        '3917130357': {'summary': {'vendorHash': 3917130357, 'resetIntervalMinutes': 10080, 'resetOffsetMinutes': 2340},
                       'sales': [{'itemHash': 11, 'refundPolicy': 2, 'refundLimit': 15},
                                 {'itemHash': 12, 'refundPolicy': 0, 'refundLimit': 15},
                                 {'itemHash': 10, 'refundPolicy': 2, 'refundLimit': 15}]},
    },
}
STRINGS = {'DestinyVendorDefinition': {'1527174714': {'summary': {'vendorName': 'Bounty Tracker'}},
                                       '3917130357': {'summary': {'vendorName': 'Eververse'}}}}


def table(path):
    with open(path, encoding='utf-8') as f:
        return [l.split('\t') for l in f.read().splitlines() if not l.startswith('#')]


class Tables(unittest.TestCase):
    def test_writes_the_curves_schedules_and_refunds(self):
        with tempfile.TemporaryDirectory() as d:
            base, strings, out = os.path.join(d, 'b.json.gz'), os.path.join(d, 's.json'), os.path.join(d, 'out')
            with gzip.open(base, 'wt', encoding='utf-8') as f:
                json.dump(BASE, f)
            with open(strings, 'w', encoding='utf-8') as f:
                json.dump(STRINGS, f)
            self.assertEqual(tables.main([base, '--strings', strings, '--out', out]), 0)
            curves = table(os.path.join(out, 'progression-curves.tsv'))
            self.assertEqual(curves[0], list(tables.CURVES_HEADER))
            self.assertEqual(curves[1], ['1716568313', 'character_level', '1', '0', '117', '0,900,4100'])
            self.assertEqual([c[1] for c in curves[2:]],
                             ['d15.winter.racing', 'faction_fotc_vanguard', 'pvp_tournament0.wins'],
                             'ranks, Trials and vendor progressions in; weapons, weeklies and step-less ones out')
            self.assertEqual(curves[3], ['3233510749', 'faction_fotc_vanguard', '1', '1',
                             str(tables.runtime_indices()[3233510749]), '1500,2000,2500'])
            resets = table(os.path.join(out, 'vendor-resets.tsv'))
            self.assertEqual(resets[1:], [['459708109', '-', '4320', '3840', '2576753410'],
                                          ['1527174714', 'Bounty Tracker', '1440', '-540', '0'],
                                          ['3917130357', 'Eververse', '10080', '2340', '0']])
            refunds = table(os.path.join(out, 'refund-policies.tsv'))
            self.assertEqual(refunds[0], list(tables.REFUNDS_HEADER))
            self.assertEqual(refunds[1:], [['3917130357', 'Eververse', '2', '15', '2', '3', '10,11']],
                             'only vendors with a refundable sale; their item hashes sorted')

    def test_the_committed_tables_cite_their_source(self):
        out = os.path.join(HERE, '..', '..', 'ps4', 'fixtures', '56578', 'progression')
        for name in ('progression-curves.tsv', 'vendor-resets.tsv', 'refund-policies.tsv'):
            with open(os.path.join(out, name), encoding='utf-8') as f:
                head = f.readline()
            self.assertTrue(head.startswith('# Derived by tools/ps4_setup/d1-progression-tables.py from Destiny'), name)
        level = [r for r in table(os.path.join(out, 'progression-curves.tsv')) if r[1] == 'character_level'][0]
        self.assertEqual(sum(map(int, level[5].split(','))), 346_000, 'level 40 = 346,000 XP')
        refunds = table(os.path.join(out, 'refund-policies.tsv'))[1:]
        self.assertEqual(sorted(r[1] for r in refunds), ['Eververse', 'Silver Dust Vendor'])
        self.assertEqual({r[3] for r in refunds}, {'15'})


if __name__ == '__main__':
    unittest.main()
