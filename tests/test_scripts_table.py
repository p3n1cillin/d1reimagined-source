# SPDX-License-Identifier: GPL-2.0-or-later
"""The script bridge's activity table generator (tools/ps4_setup/d1-scripts-table.py).

Synthetic world tables, manifest and 2014 table for the rules (the lifetime group, the body's
type hashes, cinematic-only kinds, the next activity); and the committed table against the
committed world tables. With D1_MANIFEST_DIR (DIM's D1 manifest, a local input) and D1_TABLE_2014
(tiger's dictionary activities.csv), a regeneration must reproduce the committed table byte for
byte (skipped without them).
"""
import csv
import gzip
import importlib.util
import json
import os
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('scripts_table', ROOT / 'tools/ps4_setup/d1-scripts-table.py')
M = importlib.util.module_from_spec(spec)
spec.loader.exec_module(M)
COMMITTED = ROOT / 'ps4/fixtures/56578/scripts/activities.txt'

STORY, CINE = 0x648999F4, 0x4D78882E


def write_tables(d):
    (d / 'slice-sets.tsv').write_text(
        '# x\nworld\tslice_set\tbubble\tvalid\tpublic\tregion_hash\tdescriptor\tstatic_world\tstatic_bubble\tglobal\tauthoritative\tprivate\n'
        'cine\t25\t3\t1\t0\t0x1\t0x1\t0x1\t3\t0x80000001,0x80000002\t-\t-\n'
        'tower\t24\t3\t1\t0\t0x1\t0x1\t0x1\t3\t0x80000003\t-\t-\n'
        'mission\t0\t0\t1\t0\t0x1\t0x1\t0x1\t0\t0x80000004\t-\t-\n', encoding='utf-8')
    (d / 'bundles.tsv').write_text(
        '# x\nbundle_tag\tgroup\tlookup_bubble\tclients\n'
        '0x80000001\t0x2fe7dbe6\t-1\t6:0:10 6:1:10\n'
        '0x80000002\t0x4786c0e0\t-1\t15:0:10 16:3:10\n'
        '0x80000003\t0x40aebf90\t-1\t16:2:10\n'
        '0x80000004\t0x99999999\t-1\t16:3:10\n', encoding='utf-8')
    (d / 'activities.tsv').write_text(
        '# x\nindex\thash\tname\ttype\tworld\tstart_region\tstart_source\tstart_public\tprivate_start\tsensor_group\tstatus\tevidence\n'
        '0\t0x8ae8da69\tTower\tACTIVITY_TYPE_SOCIAL\ttower\t24\tx\t0\t0\t0x40aebf90\tgenerated\tx\n'
        '165\t0x6eaf0959\tA Guardian Rises\tSTORY_CINEMATIC\tcine\t25\tx\t0\t0\t0x4786c0e0\tcross-checked\tx\n'
        '200\t0x00000200\tSomewhere\tACTIVITY_TYPE_STORY\tmission\t0\tx\t0\t0\t0x99999999\tgenerated\tx\n'
        '201\t0x00000201\tUnbound\tQUEST\t-\t-\t-\t-\t-\t-\tmissing evidence\tx\n', encoding='utf-8')


def write_manifest(d):
    data = {
        'DestinyActivityTypeDefinition': {str(STORY): {'identifier': 'ACTIVITY_TYPE_STORY'}, str(CINE): {'identifier': 'STORY_CINEMATIC'}},
        'DestinyActivityDefinition': {str(0x6EAF0959): {'activityTypeHash': CINE, 'placeHash': 0xDF617463}},
    }
    with gzip.open(d / 'd1-manifest-base.json.gz', 'wt') as f:
        json.dump(data, f)


def write_2014(path):
    with open(path, 'w', newline='', encoding='utf-8') as f:
        w = csv.writer(f)
        w.writerow(['activity_index', 'activity_hash', 'codename', 'next_activity'])
        w.writerow(['0', '8AE8DA69', 'tower', '-1'])
        w.writerow(['127', 'F3B1798E', 'cine', '0'])


class Rules(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        d = Path(self.tmp.name)
        write_tables(d)
        write_manifest(d)
        write_2014(d / 'activities.csv')
        self.lines, self.skipped = M.generate(d, d, d / 'activities.csv')

    def tearDown(self):
        self.tmp.cleanup()

    def test_story_cinematic_and_quest_rows_only(self):
        self.assertEqual([l[0] for l in self.lines], [165, 200], 'not the Tower (social), not the unbound quest')
        self.assertEqual([s[0] for s in self.skipped], [201])

    def test_the_reveal_row(self):
        line = self.lines[0][4]
        self.assertEqual(line, '165 cine 0x4786c0e0 @generated start=25 hash=0x6eaf0959 kind=cinematic '
                               'type=0x648999f4 type2=0x4d78882e place=0xdf617463 next=0:0x8ae8da69')

    def test_the_lifetime_group_respects_the_lookup_bubble(self):
        slices = {('w', 0): [1]}
        self.assertIsNone(M.lifetime_group('w', 0, slices, {1: [(0x99, 5, [(16, 3)])]}), 'a client of another bubble')
        self.assertEqual(M.lifetime_group('w', 0, slices, {1: [(0x99, -1, [(16, 3)])]}), 0x99)
        self.assertEqual(self.lines[1][4].split()[:4], ['200', 'mission', '0x99999999', '@generated'])
        self.assertNotIn('type=', self.lines[1][4], 'no manifest definition: no completion body hashes')

    def test_a_world_no_script_starts_is_marked_a_start_only_stub(self):
        d = Path(self.tmp.name) / 'scripts' / 'es' / 'x'
        d.mkdir(parents=True)
        (d / 'cine.es').write_text('@activity("cine")' + chr(10), encoding='utf-8')
        self.assertEqual(M.script_activities(d.parents[1]), {'cine'})
        self.assertIsNone(M.script_activities(None))
        lines, skipped = M.generate(Path(self.tmp.name), Path(self.tmp.name), Path(self.tmp.name) / 'activities.csv', d.parents[1])
        self.assertEqual([l[6] for l in lines], [True, False])
        rendered = M.render(lines, skipped)
        self.assertIn('# Somewhere (ACTIVITY_TYPE_STORY, world generated; no script: a start-only stub)', rendered)
        self.assertIn('# 1 of them name a world no restored script starts', rendered)
        self.assertNotIn('start-only', M.render(self.lines, self.skipped), 'no marks without the scripts')

    def test_the_rendered_table_lists_what_has_no_world(self):
        rendered = M.render(self.lines, self.skipped)
        self.assertIn('201 Unbound (QUEST)', rendered)
        self.assertIn('# no manifest definition', rendered)


class Committed(unittest.TestCase):
    def test_identifiers_only_and_every_row_generated(self):
        rows = [l for l in COMMITTED.read_text(encoding='utf-8').splitlines() if l and not l.startswith('#')]
        self.assertGreaterEqual(len(rows), 100)
        for l in rows:
            f = l.split('#')[0].split()
            self.assertEqual(f[3], '@generated', l)
            self.assertTrue(f[0].isdigit() and f[2].startswith('0x'), l)
            for t in f[4:]:
                self.assertRegex(t, r'^(start=\d+|hash=0x[0-9a-f]{8}|kind=cinematic|type2?=0x[0-9a-f]{8}|place=0x[0-9a-f]{8}|next=\d+:0x[0-9a-f]{8})$', l)
        reveal = next(l for l in rows if l.startswith('165 '))
        self.assertIn('next=0:0x8ae8da69', reveal)

    def test_strikes_raids_and_arenas_are_in_and_the_open_world_is_not(self):
        rows = {int(l.split()[0]): l for l in COMMITTED.read_text(encoding='utf-8').splitlines() if l[:1].isdigit()}
        for i, world in ((160, 'raid_epiphany'), (380, 'raid_promise'), (27, 'arena_champion'), (302, 'cosmo_portal_1')):
            self.assertEqual(rows[i].split()[1], world, i)
        for i in (0, 1, 93, 424):   # the Tower, patrols: the open world and the public host serve them
            self.assertNotIn(i, rows)

    @unittest.skipUnless(os.environ.get('D1_MANIFEST_DIR') and os.environ.get('D1_TABLE_2014') and os.environ.get('D1_SCRIPTS_DIR'),
                         'set D1_MANIFEST_DIR, D1_TABLE_2014 and D1_SCRIPTS_DIR (local inputs) to regenerate')
    def test_regeneration_is_byte_identical(self):
        lines, skipped = M.generate(ROOT / 'ps4/fixtures/56578/worlds', os.environ['D1_MANIFEST_DIR'], os.environ['D1_TABLE_2014'],
                                    os.environ['D1_SCRIPTS_DIR'])
        self.assertEqual(M.render(lines, skipped), COMMITTED.read_text(encoding='utf-8'))


if __name__ == '__main__':
    unittest.main()
