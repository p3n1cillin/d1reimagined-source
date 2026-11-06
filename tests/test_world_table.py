# SPDX-License-Identifier: GPL-2.0-or-later
"""The worlds generator (tools/ps4_setup/d1-world-table.py) and its committed tables.

Synthetic content for the binding rules; the committed tables against the client's own sensor
lists; and, with D1_WORLDS_JSON pointing at a private `d1-worlds.mjs` output, a regeneration that
must reproduce the committed content tables byte for byte (skipped without it).
"""
import importlib.util
import json
import os
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('world_table', ROOT / 'tools/ps4_setup/d1-world-table.py')
M = importlib.util.module_from_spec(spec)
spec.loader.exec_module(M)
FIXTURES = ROOT / 'ps4/fixtures/56578'
TABLES = FIXTURES / 'worlds'
TAB, NL = chr(9), chr(10)


def state(slice_set, glob=(), auth=(), priv=(), public=False, valid=1):
    return {'slice_set': slice_set, 'valid': valid, 'public': public, 'region_name_hash': '0x11111111',
            'descriptor': f'0x8000{slice_set:04x}', 'world_hash': '0xaaaaaaaa', 'bubble_index': slice_set // 8,
            'blueprint_set': '0x80001111', 'global': list(glob), 'authoritative': list(auth), 'private': list(priv)}


def bundle(group, lookups):
    return {'bundle': group, 'world_hash': '0x811c9dc5', 'slotted': 0, 'rows': [],
            'lookups': [{'slice_set': lb, 'phase': '0x811c9dc5', 'rows': rows} for lb, rows in lookups]}


def synthetic():
    common = bundle('0x4786c0e0', [(-1, [[0, 15, '0x808008d0', None, 0, -1], [1, 16, '0x808008a3', None, 0, -1],
                                         [2, 12, '0x808022a5', '0x80802c30', 4, -1]])])
    mission = bundle('0x99999999', [(-1, [[0, 6, '0x8080254d', None, 0, -1], [1, 61, None, None, 0, -1]]),
                                    (1, [[2, 4, '0x808007d9', '0x80800387', 2, 1]]), (2, [[3, 4, '0x808007d9', None, 2, 2]])])
    cine = bundle('0x2fe7dbe6', [(-1, [[0, 6, '0x8080254d', None, 0, -1]])])
    other = bundle('0x55555555', [(-1, [[0, 5, '0x808029a1', None, 0, -1]])])
    worlds = [
        {'name': 'act_test_mission', 'scenario_tag': '0x80cb800d', 'family': hex(M.fnv1('act_test_mission')),
         'private': '0x8fe07299', 'public': '0xe354f1ea', 'destination': 'cosmo', 'world_hash': '0xaaaaaaaa',
         'bubbles': [{'region_name_hash': '0x1', 'states': [state(0, ['0x80000001'], public=True)]},
                     {'region_name_hash': '0x2', 'states': [state(8, ['0x80000001'], ['0x80000002'])]},
                     {'region_name_hash': '0x3', 'states': []},
                     {'region_name_hash': '0x4', 'states': [state(24, ['0x80000001']), state(25, ['0x80000001'], ['0x80000002'])]}]},
        {'name': 'cine_test', 'scenario_tag': '0x80c3e00a', 'family': hex(M.fnv1('cine_test')),
         'private': '0x14cda256', 'public': '0xd56dca6e', 'destination': 'cinematic_trv', 'world_hash': '0xbbbbbbbb',
         'bubbles': [{'region_name_hash': '0x5', 'states': [state(0, ['0x80000001', '0x80000003'])]}]},
        {'name': 'pvp_test', 'scenario_tag': '0x80f08003', 'family': hex(M.fnv1('pvp_test')),
         'private': '0x5966e491', 'public': '0xc4647343', 'destination': 'pvp_test', 'world_hash': '0xcccccccc',
         'bubbles': [{'region_name_hash': '0x6', 'states': [state(0, ['0x80000004'])]}]},
    ]
    return {'build': M.BUILD, 'worlds': worlds,
            'bundles': {'0x80000001': common, '0x80000002': mission, '0x80000003': cine, '0x80000004': other},
            'insertion_points': {'0xaaaaaaaa': [[['0x2ea8fb98', 3]], None, [], [['0x12345678', 1]]]}, 'problems': []}


def meta(name, typ='ACTIVITY_TYPE_STORY', dest='Old Russia, Earth', desc='', playlist=False):
    return {'name': name, 'description': desc, 'type': typ, 'destination': dest, 'playlist': playlist}


class ContentTest(unittest.TestCase):
    def setUp(self):
        self.c = M.Content(synthetic())

    def test_sensor_clients_take_every_bubble_and_the_slice_sets_own_and_need_a_value_type(self):
        self.assertEqual(self.c.sensor_clients('act_test_mission', 8), [
            (0x4786C0E0, 15, 0, 1, 0), (0x4786C0E0, 16, 1, 1, 0), (0x4786C0E0, 12, 2, 1, 1),
            (0x99999999, 6, 0, 1, 0), (0x99999999, 4, 2, 1, 1)])
        self.assertNotIn(61, [t for _, t, _, _, _ in self.c.sensor_clients('act_test_mission', 8)], 'no A or B type')
        self.assertEqual(self.c.sensor_group('act_test_mission', 8), 0x4786C0E0)
        self.assertEqual(self.c.sensor_group('pvp_test', 0), None)

    def test_chosen_bubbles_skip_empty_ones_and_first_private_skips_public(self):
        self.assertEqual(self.c.chosen('act_test_mission'), [0, 1, 3])
        self.assertEqual(self.c.first_private('act_test_mission'), 8)

    def test_capture_world_weights_rare_groups_and_needs_the_start(self):
        self.assertEqual(self.c.capture_world({0x4786C0E0: 21, 0x99999999: 4}, 8)[0], 'act_test_mission')
        self.assertEqual(self.c.capture_world({0x4786C0E0: 21, 0x2FE7DBE6: 1}, 0)[0], 'cine_test')
        self.assertIsNone(self.c.capture_world({0x4786C0E0: 21}, 0)[0], 'a common group alone is a tie')
        self.assertIsNone(self.c.capture_world({0x99999999: 4}, 16)[0], 'no such slice set')

    def test_binding_prefers_the_capture_then_the_2014_table_then_twins(self):
        acts = [(164, 0x6E0DAE89), (165, 0x6EAF0959), (169, 0x7EEA7680), (300, 0x1), (301, 0x2), (410, 0xFB5D13A3),
                (500, 0x3)]
        manifest = {0x6E0DAE89: meta('A Test Rises', desc='d'), 0x6EAF0959: meta('A Test Rises', 'STORY_CINEMATIC', desc='d'),
                    0x7EEA7680: meta('A Test Rises', desc='d'), 0x1: meta('Clash', 'PVP', 'The Crucible', playlist=True),
                    0x2: meta('First Test', 'ACTIVITY_TYPE_PVP_FFA', 'The Crucible', desc='Moon'),
                    0xFB5D13A3: meta('Skirmish', 'PVP', 'The Crucible'), 0x3: meta('A Test Rises', dest='Mars')}
        legacy = [{'hash': 0xB8AD0AE6, 'codename': 'act_test_mission', 'name': 'A Test Rises', 'description': 'd', 'start_bubble': None},
                  {'hash': 0xF3B1798E, 'codename': 'cine_test', 'name': 'A Test Rises', 'description': 'd', 'start_bubble': 3},
                  {'hash': 0x2, 'codename': 'pvp_test', 'name': 'Old Name', 'description': '', 'start_bubble': None},
                  {'hash': 0x9, 'codename': 'not_in_56578', 'name': 'Skirmish', 'description': '', 'start_bubble': None}]
        hosts = {0x6E0DAE89: [('AGR', 25, {0x4786C0E0: 21, 0x99999999: 9})]}
        rows = {r['index']: r for r in M.bind(self.c, acts, manifest, legacy, hosts)}
        self.assertEqual((rows[164]['world'], rows[164]['status'], rows[164]['start_region'], rows[164]['start_source']),
                         ('act_test_mission', 'cross-checked', 25, 'capture'))
        self.assertIn('2014 table, same name and description', rows[164]['evidence'])
        # A cinematic activity type picks the cinematic world among the 2014 table's candidates.
        self.assertEqual((rows[165]['world'], rows[165]['status']), ('cine_test', 'generated'))
        self.assertEqual(rows[165]['start_source'], 'only slice set')
        # A heroic copy: same name and destination as the captured one, and its start.
        self.assertEqual((rows[169]['world'], rows[169]['start_region'], rows[169]['start_source']),
                         ('act_test_mission', 25, 'capture of 164'))
        self.assertEqual((rows[300]['world'], rows[300]['status'], rows[300]['evidence']),
                         (None, 'missing evidence', 'playlist: launches another activity'))
        self.assertEqual((rows[301]['world'], rows[301]['evidence']), ('pvp_test', '2014 table, same hash'))
        self.assertEqual(rows[410]['status'], 'missing evidence', 'a 2014 world the build lacks is no binding')
        self.assertEqual(rows[500]['world'], 'act_test_mission', 'the 2014 table binds by name on any destination')
        self.assertEqual(rows[500]['start_source'], 'stand-in: first private slice set', 'a twin needs the same destination')
        self.assertEqual(M.coverage(rows.values()), {'cross-checked': 1, 'generated': 4, 'missing evidence': 2})

    def test_the_clients_own_activity_table_binds_and_outranks_the_2014_table(self):
        data = synthetic()
        data['worlds'].append({'name': 'social_test', 'scenario_tag': '0x81540003', 'family': hex(M.fnv1('social_test')),
                               'private': '0xbfcedaa5', 'public': '0x72730527', 'destination': 'mercury',
                               'world_hash': '0xdddddddd',
                               'bubbles': [{'region_name_hash': '0x7', 'states': [state(0, ['0x80000001'], public=True),
                                                                                  state(1, ['0x80000001'], public=True)]}]})
        data['activities'] = [
            {'index': 1, 'hash': '0x00000011', 'name': 'act_test_mission', 'fallback_name': None},
            {'index': 2, 'hash': '0x00000012', 'name': 'act_test_mission', 'fallback_name': None},
            {'index': 3, 'hash': '0x00000013', 'name': 'old_patrols_1', 'fallback_name': 'action_mock'},
            {'index': 4, 'hash': '0x00000014', 'name': None, 'fallback_name': None},
            {'index': 5, 'hash': '0x00000015', 'name': 'social_test', 'fallback_name': None},
            {'index': 6, 'hash': '0x00000016', 'name': 'cine_test', 'fallback_name': None},
            {'index': 7, 'hash': '0x99999999', 'name': 'pvp_test', 'fallback_name': None},
        ]
        c = M.Content(data)
        self.assertEqual((c.client_name(1, 0x11), c.client_name(4, 0x14), c.client_name(7, 0x17)), ('act_test_mission', '', None))
        acts = [(1, 0x11), (2, 0x12), (3, 0x13), (4, 0x14), (5, 0x15), (6, 0x16)]
        manifest = {0x11: meta('A Test Rises'), 0x12: meta('Weekly Test'), 0x13: meta('A Test Rises'),
                    0x14: meta('Clash', 'PVP', 'The Crucible'), 0x15: meta('Lighthouse', 'SOCIAL', 'Mercury'),
                    0x16: meta('A Test Cinematic', 'STORY_CINEMATIC')}
        legacy = [{'hash': 0x12, 'codename': 'pvp_test', 'name': 'Weekly Test', 'description': '', 'start_bubble': None},
                  {'hash': 0x77, 'codename': 'act_test_mission', 'name': 'Other', 'description': '', 'start_bubble': 3}]
        captured = {0x11: ('AGR', 'act_test_mission', 25)}
        rows = {r['index']: r for r in M.bind(c, acts, manifest, legacy, {}, captured)}
        # The committed table's cross-checked row stands in for the capture; the client agrees.
        self.assertEqual((rows[1]['status'], rows[1]['start_region'], rows[1]['start_source']), ('cross-checked', 25, 'capture'))
        self.assertEqual(rows[1]['evidence'], f"live capture AGR; {M.CLIENT} agrees")
        # The client's own name outranks the 2014 table, and the world's other captured start is its start.
        self.assertEqual((rows[2]['world'], rows[2]['status']), ('act_test_mission', 'generated'))
        self.assertEqual(rows[2]['evidence'], f'{M.CLIENT} names it; 2014 table, same hash said pvp_test')
        self.assertEqual((rows[2]['start_region'], rows[2]['start_source']), (25, 'capture of 1 (same world)'))
        # A bundle the build has no scenario for: the twin the client's table binds.
        self.assertEqual(rows[3]['world'], 'act_test_mission')
        self.assertIn('names old_patrols_1, which has no scenario', rows[3]['evidence'])
        self.assertEqual((rows[4]['world'], rows[4]['evidence']), (None, f'{M.CLIENT} names no world'))
        # A world with public slice sets only starts in its first.
        self.assertEqual((rows[5]['world'], rows[5]['start_region'], rows[5]['start_source']),
                         ('social_test', 0, 'stand-in: first slice set (all public)'))
        self.assertEqual(rows[6]['world'], 'cine_test')

    def test_captured_rows_read_back_from_a_table(self):
        rows = [M.ACTIVITY_COLUMNS.split(TAB),
                ['1', '0x00000011', 'A', 'T', 'act_test_mission', '25', 'capture', '0', '8', '0x1', 'cross-checked',
                 'live capture AGR; 2014 table, same hash'],
                ['2', '0x00000012', 'B', 'T', 'social_landing', '0', 'stand-in: first private slice set', '0', '0', '0x1',
                 'cross-checked', 'live capture Eye'],
                ['3', '0x00000013', 'C', 'T', 'act_test_mission', '8', 'capture of 1', '0', '8', '0x1', 'generated', 'same name']]
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'activities.tsv'
            path.write_text(''.join(TAB.join(r) + NL for r in [['# x']] + rows), encoding='utf-8')
            self.assertEqual(M.read_captured(path), {0x11: ('AGR', 'act_test_mission', 25), 0x12: ('Eye', 'social_landing', None)})
        self.assertEqual(M.read_captured(None), {})

    def test_tables_round_trip_to_the_same_sensor_lists(self):
        rows = M.bind(self.c, [(0, 0x1)], {0x1: meta('A Test Rises')},
                      [{'hash': 0x1, 'codename': 'act_test_mission', 'name': '', 'description': '', 'start_bubble': 1}], {})
        with tempfile.TemporaryDirectory() as tmp:
            arows = M.write_tables(self.c, rows, tmp)
            t = M.Tables(tmp)
            for world, s in (('act_test_mission', 8), ('act_test_mission', 25), ('cine_test', 0), ('pvp_test', 0)):
                self.assertEqual(t.sensor_clients(world, s), self.c.sensor_clients(world, s), (world, s))
            self.assertEqual(arows[0][4:7], ('act_test_mission', 8, '2014 start bubble'))
            self.assertEqual(t.activities[0]['sensor_group'], '0x4786c0e0')
            self.assertEqual(t.worlds['act_test_mission']['chosen'], '0,1,3')
            points = (Path(tmp) / 'insertion-points.tsv').read_text().splitlines()[2:]
            self.assertEqual(points, ['0xaaaaaaaa\t0\t0x2ea8fb98:3', '0xaaaaaaaa\t1\t?', '0xaaaaaaaa\t2\t-',
                                      '0xaaaaaaaa\t3\t0x12345678:1'])
            self.assertTrue(t.sensor_list('cine_test', 0).splitlines()[1].startswith('0x4786c0e0 0xf 0 1 0'))


def fixture_list(name):
    out = set()
    for line in (FIXTURES / 'mission' / name).read_text().splitlines():
        f = line.split('#')[0].split()
        if len(f) >= 5 and not f[0].startswith('@'):
            out.add((int(f[0], 16), int(f[1], 0), int(f[2], 0), int(f[3]), int(f[4])))
    return out


class CommittedTablesTest(unittest.TestCase):
    """The committed tables against what the client and the live hosts did."""

    @classmethod
    def setUpClass(cls):
        cls.t = M.Tables(TABLES)

    def test_the_generated_lists_are_the_clients_own(self):
        # Each list was read from the PS4 client's memory once its world was resident (or served by
        # the live host); the generator reproduces every record, values aside.
        for world, s, name in (('city_tower_default1', 0, 'sensor-clients-city_tower_default1-squads.txt'),
                               ('cine_reveal', 25, 'sensor-clients-cine_reveal-native.txt'),
                               ('cine_speaker', 1, 'sensor-clients-cine_speaker-live.txt'),
                               ('act_cosmo_m1_dome_a', 8, 'sensor-clients-act_cosmo_m1_dome_a-bundle.txt'),
                               ('act_cosmo_m0_highway', 113, 'sensor-clients-act_cosmo_m0_highway-r113-cine.txt')):
            self.assertEqual(set(self.t.sensor_clients(world, s)), fixture_list(name), (world, s))

    def test_first_lights_list_is_the_clients_plus_its_lifetime_record(self):
        got = set(self.t.sensor_clients('pvp_moonbase', 0))
        want = fixture_list('sensor-clients-pvp_moonbase-bundle7-team0.txt')
        self.assertEqual(got - want, {(0xC2DACAF7, M.LIFETIME_TYPE, 3, 1, 0)})
        self.assertEqual(want - got, set())

    def test_worlds_match_the_hosts_measured_scenario_records(self):
        # crates/d1-activity-host/src/worlds.rs: live-captured or read before this generator.
        for name, tag, checksum, bubbles in (('pvp_moonbase', 0x80F08003, 0x5966E491, 1), ('orbit', 0x80B52003, 0x33F044D3, 16),
                                             ('pvp_city_defense', 0x80DFA003, 0x929D89ED, 1),
                                             ('act_cosmo_m0_highway', 0x80CB800D, 0x8FE07299, 22),
                                             ('act_cosmo_m1_dome_a', 0x80CB804B, 0x42FE77A5, 22),
                                             ('city_tower_default1', 0x80C7A005, 0x24340499, 5),
                                             ('cine_reveal', 0x80C3E00A, 0x14CDA256, 4), ('cine_speaker', 0x80C3E011, 0xA3198561, 4),
                                             ('act_cinematic_queen_101', 0x80C02004, 0x14CDA256, 3)):
            w = self.t.worlds[name]
            self.assertEqual((int(w['scenario_tag'], 16), int(w['private_checksum'], 16), int(w['bubble_count'])),
                             (tag, checksum, bubbles), name)
        self.assertEqual(self.t.worlds['orbit']['chosen'], '0,1,2,3,4,9,10,11,12,13,14', 'measured live, 16/16')

    def test_every_activity_row_is_consistent(self):
        self.assertEqual(sorted(self.t.activities), list(range(653)))
        for i, r in self.t.activities.items():
            self.assertIn(r['status'], M.STATUS, i)
            if r['world'] == '-':
                self.assertEqual(r['status'], 'missing evidence', i)
                continue
            self.assertIn(r['world'], self.t.worlds, i)
            s = int(r['start_region'])
            self.assertIn((r['world'], s), self.t.slices, i)
            self.assertEqual(r['start_public'], self.t.slices[(r['world'], s)]['public'], i)

    def test_the_hand_built_activities_agree(self):
        a = self.t.activities
        self.assertEqual((a[164]['world'], a[164]['start_region'], a[164]['status']), ('act_cosmo_m0_highway', '113', 'cross-checked'))
        self.assertEqual((a[165]['world'], a[165]['start_region']), ('cine_reveal', '25'))
        self.assertEqual((a[167]['world'], a[167]['start_region']), ('cine_speaker', '1'))
        self.assertEqual(a[0]['world'], 'city_tower_default1')
        self.assertEqual(a[168]['world'], 'act_cosmo_m1_dome_a')

    def test_the_clients_activity_table_binds_the_later_content(self):
        a = self.t.activities
        # Every live host the captures cross-check served the world the client's own definition names.
        for i, r in a.items():
            if r['status'] == 'cross-checked' and 'names no world' not in r['evidence']:
                self.assertIn(f'{M.CLIENT} agrees', r['evidence'], i)
        for i, world in ((160, 'raid_epiphany'), (380, 'raid_promise'), (27, 'arena_champion'), (336, 'arena_goldfish'),
                         (337, 'arena_angelfish'), (398, 'social_observatory'), (40, 'mercury_social'),
                         (424, 'plaguelands_patrol'), (391, 'mission_hound')):
            self.assertEqual((a[i]['world'], a[i]['status']), (world, 'generated'), i)
            self.assertIn(f'{M.CLIENT} names it', a[i]['evidence'], i)
        # What stays unbound names no world of this build: playlists, the Crucible's modes, credits.
        for i, r in a.items():
            if r['world'] == '-':
                self.assertRegex(r['evidence'], r'^(playlist|client.s activity definition names)', i)


@unittest.skipUnless(os.environ.get('D1_WORLDS_JSON'), 'private: set D1_WORLDS_JSON to a d1-worlds.mjs output')
class RegenerateTest(unittest.TestCase):
    def test_the_content_tables_regenerate_byte_for_byte(self):
        content = M.Content(json.loads(Path(os.environ['D1_WORLDS_JSON']).read_text(encoding='utf-8')))
        with tempfile.TemporaryDirectory() as tmp:
            M.write_tables(content, [], tmp)
            for name in ('worlds.tsv', 'slice-sets.tsv', 'bundles.tsv', 'insertion-points.tsv'):
                self.assertEqual((Path(tmp) / name).read_bytes(), (TABLES / name).read_bytes(), name)

    def test_the_activity_bindings_regenerate_byte_for_byte(self):
        # The bindings also need the client's activity table in the worlds JSON (d1-worlds.mjs since
        # stream Q), Bungie's manifest and the 2014 table (local inputs). The captures are not needed:
        # the committed cross-checked rows stand in for them.
        data = json.loads(Path(os.environ['D1_WORLDS_JSON']).read_text(encoding='utf-8'))
        manifest = Path(os.environ.get('D1_MANIFEST_SQLITE', 'F:/d1/d1preservation-main/manifest/data/world_en.sqlite'))
        legacy = Path(os.environ.get('D1_TIGER', 'F:/d1/third-party/Blam-Network_tiger')) / M.LEGACY_TABLE
        if not data.get('activities') or not manifest.exists() or not legacy.exists():
            self.skipTest('private: a worlds JSON with the activity table, the manifest and the 2014 table')
        content = M.Content(data)
        acts = M.read_activity_table(FIXTURES / 'characters' / 'activity-table.tsv')
        bound = M.bind(content, acts, M.read_manifest(manifest), M.read_legacy(legacy), {},
                       M.read_captured(TABLES / 'activities.tsv'))
        with tempfile.TemporaryDirectory() as tmp:
            M.write_tables(content, bound, tmp)
            self.assertEqual((Path(tmp) / 'activities.tsv').read_bytes(), (TABLES / 'activities.tsv').read_bytes())


if __name__ == '__main__':
    unittest.main()
