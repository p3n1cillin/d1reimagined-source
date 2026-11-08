# SPDX-License-Identifier: GPL-2.0-or-later
"""What the content and the 56578 client hold of the Crucible's limits (crucible.json's content_limits).

- The committed content_limits agrees with the committed content exports: recomputed here from
  activity-options.json and activity-rule-programs.json with the generator's own functions, so a
  re-export that starts setting score_to_win for a playlist fails this test until crucible.json is
  regenerated.
- With D1_EBOOT (the owned 56578 executable), the client facts it records are re-checked: the
  score_to_win hash is in the code exactly once, and time_limit and the preset companions are
  nowhere. Without it that part skips and says so.
"""
import importlib.util
import json
import os
from pathlib import Path
import struct
import unittest

REPO = Path(__file__).resolve().parents[1]
ACTIVITIES = REPO / 'ps4' / 'fixtures' / '56578' / 'activities'
PROGRAMS = REPO / 'ps4' / 'fixtures' / '56578' / 'characters' / 'activity-rule-programs.json'


def load_generator():
    spec = importlib.util.spec_from_file_location('activities_defs', REPO / 'tools' / 'ps4_setup' / 'd1-activities-defs.py')
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


class CrucibleLimits(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.gen = load_generator()
        cls.crucible = json.loads((ACTIVITIES / 'crucible.json').read_text(encoding='utf-8'))
        cls.options = {e['tag']: e['options'] for e in
                       json.loads((ACTIVITIES / 'activity-options.json').read_text(encoding='utf-8'))['data']}
        cls.programs = {e['hash']: e for e in json.loads(PROGRAMS.read_text(encoding='utf-8'))['data']}

    def test_option_names_are_the_contents_hashes(self):
        self.assertEqual(self.gen.SCORE_TO_WIN, 0xBE4DB480)
        self.assertEqual(self.gen.TIME_LIMIT, 0xF5EC68CA)
        names = {o['name_hash'] for opts in self.options.values() for o in opts}
        self.assertIn(self.gen.SCORE_TO_WIN, names, 'some option tag sets score_to_win')
        self.assertIn(self.gen.TIME_LIMIT, names, 'some option tag sets time_limit')

    def test_every_playlist_and_map_matches_its_option_tags(self):
        rows = self.crucible['playlists'] + self.crucible['maps']
        self.assertEqual(len(rows), 86 + 36)
        for r in rows:
            self.assertIn(r['hash'], self.programs, f"{r['name']} has no rule program")
            self.assertEqual(r['content_limits'], self.gen.content_limits(self.programs, self.options, r['hash']), r['name'])
        setting = [r['hash'] for r in rows if r['content_limits']['score_to_win'] is not None
                   or r['content_limits']['time_limit'] is not None]
        self.assertEqual(setting, [], 'no Crucible playlist or map sets a limit in its own option tags')
        limits = self.crucible['content_limits']
        self.assertEqual((limits['playlists_or_maps_setting_score_to_win'], limits['playlists_or_maps_setting_time_limit']), ([], []))

    def test_the_presets_are_every_limit_tag_and_unreferenced(self):
        referenced = {p['options'] for p in self.programs.values()} | \
                     {s['tag'] for p in self.programs.values() for s in p['option_sets']}
        presets = self.crucible['content_limits']['presets']
        self.assertEqual(presets, self.gen.presets(self.options, referenced))
        self.assertEqual(len(presets), 46)
        self.assertFalse(any(p['referenced_by_an_activity'] for p in presets))

    def test_modes_stay_labelled_stand_ins(self):
        for m in self.crucible['modes']:
            self.assertIn(m['evidence']['kind'], ('stand-in', 'needs-evidence'), m['id'])
            self.assertIn('content_limits', m['evidence']['source'], m['id'])

    def test_the_client_reads_score_to_win_only(self):
        eboot = os.environ.get('D1_EBOOT')
        if not eboot or not Path(eboot).is_file():
            self.skipTest('D1_EBOOT (the owned 56578 executable) is not set: the client facts are not re-checked')
        data = Path(eboot).read_bytes()
        client = self.crucible['content_limits']['client']
        self.assertEqual(data.count(struct.pack('<I', client['score_to_win']['name_hash'])), 1)
        self.assertEqual(data.count(struct.pack('<I', client['time_limit']['name_hash'])), 0)
        for h in client['preset_companions']['name_hashes']:
            self.assertEqual(data.count(struct.pack('<I', h)), 0, h)


if __name__ == '__main__':
    unittest.main()
