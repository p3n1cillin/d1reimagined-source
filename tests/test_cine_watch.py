# SPDX-License-Identifier: GPL-2.0-or-later
"""d1-cine-watch.py's cues, on activity-host log lines in the form the host writes them."""
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).parents[1]
spec = importlib.util.spec_from_file_location('cine_watch', ROOT / 'tools/ps4_setup/d1-cine-watch.py')
M = importlib.util.module_from_spec(spec)
spec.loader.exec_module(M)
MISSION = ROOT / 'ps4/fixtures/56578/mission'


class CineWatchTest(unittest.TestCase):
    def test_the_start_cue_is_the_exact_slice_set(self):
        line = '⭐ client is now in slice set {} (hash 0x68935472, was None)'
        self.assertTrue(M.due(0, line.format(25)))
        self.assertTrue(M.due(0, line.format(1), 1))
        self.assertFalse(M.due(0, line.format(113), 1), 'slice set 1 is not 113')
        self.assertFalse(M.due(0, line.format(250)))

    def test_later_cues_are_a_stopped_cinematic(self):
        self.assertTrue(M.due(1, 'incident 891 reported by the client'))
        self.assertFalse(M.due(1, 'incident 2681 reported by the client'))

    def test_the_join_names_the_activity(self):
        line = '⭐ world table: activity index {} -> world "cine_speaker" (checksum 0xa3198561, 4 bubble(s))'
        self.assertTrue(M.joined(line.format(167), 167))
        self.assertFalse(M.joined(line.format(165), 167))

    def test_the_speaker_lists_hold_one_cinematic_and_the_world_block(self):
        for name, state in (('sensor-clients-cine_speaker-live.txt', 1), ('cine_speaker-play.txt', 2),
                            ('cine_speaker-done.txt', 4)):
            rows = [r.split() for r in (MISSION / name).read_text(encoding='utf-8').splitlines()
                    if r and not r.startswith('#')]
            cine = [r for r in rows if r[0] == '0x796b3b8c']
            self.assertEqual(len(cine), 1, name)
            self.assertEqual(int(cine[0][6][8:16], 16), state << 24, name)
            self.assertEqual(int(cine[0][6][16:18], 16), int(state == 2), name + ': playing byte (+8)')
            self.assertEqual(sum(r[0] == '0x4786c0e0' for r in rows), 21, name)
        table = (MISSION / 'world-table-speaker-live.txt').read_text(encoding='utf-8')
        self.assertIn('167  cine_speaker          0x4786c0e0  sensor-clients-cine_speaker-live.txt  1', table)

    def test_the_awoken_lists_step_two_cinematics(self):
        for name, states in (('sensor-clients-cine_awoken-live.txt', (1, 1)), ('cine_awoken-play-0.txt', (2, 1)),
                             ('cine_awoken-play-1.txt', (4, 2)), ('cine_awoken-done.txt', (4, 4))):
            rows = [r.split() for r in (MISSION / name).read_text(encoding='utf-8').splitlines()
                    if r and not r.startswith('#')]
            cine = {int(r[2]): int(r[6][8:16], 16) >> 24 for r in rows if r[0] == '0x4af77757'}
            playing = {int(r[2]): int(r[6][16:18], 16) for r in rows if r[0] == '0x4af77757'}
            self.assertEqual((playing[0], playing[1]), tuple(int(x == 2) for x in states), name)
            self.assertEqual((cine[0], cine[1]), states, name)
            self.assertEqual(sum(r[0] == '0x4786c0e0' for r in rows), 21, name)
        table = (MISSION / 'world-table-speaker-live.txt').read_text(encoding='utf-8')
        self.assertIn('186  act_cinematic_queen_101  0x4786c0e0  sensor-clients-cine_awoken-live.txt  17', table)


if __name__ == '__main__':
    unittest.main()
