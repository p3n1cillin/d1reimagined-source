import importlib.util
import math
from pathlib import Path
import sys
import unittest
from unittest.mock import Mock

TOOLS = Path(__file__).parents[1] / 'tools/ps4_setup'
sys.path.insert(0, str(TOOLS))
SPEC = importlib.util.spec_from_file_location('agr_replay', TOOLS / 'd1-agr-replay.py')
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)

LINE = [(float(x), 0.0, 0.0) for x in range(0, 50, 5)]     # ten waypoints along x


class Fake:
    """walk_fighting's surroundings: a Guardian that walks each route to its end unless told not to."""

    def __init__(self, at, stall=0, lose=0, hurt=0):
        self.at, self.stall, self.lose, self.walks, self.pads = at, stall, lose, [], []
        self.hurt, self.fights, self.stance = hurt, [], 'stand'

    def wait_for_unit(self, seconds=90):
        return True

    def where(self):
        return self.at

    def skirmish(self, name, recover=True):
        self.fights.append(name if recover else name + ' (no wait)')

    def write_route(self, name, points):
        return list(points)

    def walk_once(self, tag, route, *rest, **kw):
        self.walks.append((tag, route))
        if self.lose:
            self.lose -= 1
            return [M.UNIT_LOST + '; no stale position fallback']
        if self.hurt and not tag.endswith('-back'):
            self.hurt -= 1
            return ['hurt: health 0.20 at wp 1/4', f'stopped ({self.at[0]}, {self.at[1]}, 0)']   # as d1-walk prints them
        if self.stall and not tag.endswith('-back'):
            self.stall -= 1
            return [f'stopped ({self.at[0]}, {self.at[1]}, 0)']
        self.at = route[-1]
        return [f'done {route[-1]}']

    def pad(self, *keys):
        self.pads.append(keys)

    def recover(self, name):
        pass

    away, lost = M.Replay.away, M.Replay.lost
    fixed_cover = M.Replay.fixed_cover
    walk_fighting = M.Replay.walk_fighting


class WalkFightingTest(unittest.TestCase):
    def test_crouch_disables_jump_in_walk_and_retrack(self):
        replay = M.Replay.__new__(M.Replay)
        replay.native_unit = True
        replay.tracked = Path('synthetic-tracking')
        replay.command = Mock()
        for stance in ('crouch', 'stand'):
            replay.stance = stance
            replay.command.reset_mock()
            replay.walk_once('mine', Path('route.json'), 30, 1.3, ())
            replay.retrack()
            for call in replay.command.call_args_list:
                self.assertEqual('--no-jump' in call.args[1], stance == 'crouch')

    def test_off_the_leg_is_a_respawn(self):
        # sense108: respawned 40 m from the junction leg, which walked straight into a wall.
        with self.assertRaises(M.Respawned):
            Fake((20.0, 40.0, 0.0)).walk_fighting('leg', LINE, (), 300, chunk=4)

    def test_chosen_leg_rejoins(self):
        f = Fake((20.0, 40.0, 0.0))
        f.walk_fighting('leg', LINE, (), 300, chunk=4, rejoin=True)
        self.assertEqual(f.at, LINE[-1])

    def test_starts_at_the_nearest_stretch(self):
        f = Fake((31.0, 0.5, 0.0))
        f.walk_fighting('leg', LINE, (), 300, chunk=4)
        self.assertEqual(f.walks[0][1][0], LINE[6])

    def test_route_aligned_fixed_cover_ends_the_stretch_before_the_drop(self):
        for cover, expected in (((10.0, 0.2), LINE[2]), ((10.0, 3.0), LINE[4])):
            with self.subTest(cover=cover):
                f = Fake(LINE[0])
                f.fixed_covers = {'junction-west': cover}
                f.walk_fighting('junction-west-r2', LINE, (), 300, chunk=4)
                self.assertEqual(f.walks[0][1][-1], expected)
                self.assertEqual(f.fights[:2], ['junction-west-r2-0-pre', 'junction-west-r2-0'])
                self.assertEqual(f.at, LINE[-1])

    def test_stall_backs_off_before_retrying(self):
        f = Fake((0.0, 0.0, 0.0), stall=1)
        f.walk_fighting('leg', LINE, (), 300, chunk=4)
        tags = [t for t, _ in f.walks]
        self.assertEqual(tags[:3], ['leg-0', 'leg-0-back', 'leg-0-retry1'])
        self.assertTrue(f.pads)
        self.assertEqual(f.at, LINE[-1])


    def test_a_crouched_stall_does_not_jump(self):
        # sense124: a jump in the crouch-only corridor stands the Guardian up under the lasers.
        f = Fake((0.0, 0.0, 0.0), stall=1)
        f.stance = 'crouch'
        f.walk_fighting('leg', LINE, (), 300, chunk=4)
        self.assertTrue(f.pads)
        self.assertTrue(all('NumPad2' not in keys for keys in f.pads))

    def test_hurt_fights_before_walking_on_without_a_stall(self):
        # sense118 was shot from the side mid-walk and walked on (or stalled) until it died.
        f = Fake((0.0, 0.0, 0.0), hurt=1)
        f.walk_fighting('leg', LINE, (), 300, chunk=4)
        tags = [t for t, _ in f.walks]
        self.assertEqual(tags[:2], ['leg-0', 'leg-0-retry1'])     # no walk back
        self.assertFalse(f.pads)                                  # no sidestep
        self.assertEqual(f.fights[:2], ['leg-0-pre', 'leg-0-retry1-pre (no wait)'])   # straight to the fight
        self.assertEqual(f.at, LINE[-1])

    def test_endless_hurt_falls_to_the_stall_path(self):
        f = Fake((0.0, 0.0, 0.0), hurt=5)
        f.walk_fighting('leg', LINE, (), 300, chunk=4)
        self.assertTrue(f.pads)
        self.assertEqual(f.at, LINE[-1])

    def test_a_lost_unit_does_not_use_up_the_tries(self):
        # sense110: three unit-lost walks in 30 s ended the replay during a reload.
        f = Fake((0.0, 0.0, 0.0), lose=4)
        f.walk_fighting('leg', LINE, (), 300, chunk=4, tries=3)
        self.assertEqual(f.at, LINE[-1])


class CoverTest(unittest.TestCase):
    def test_route_aligned_cover_is_replaced_after_a_drop(self):
        replay = M.Replay.__new__(M.Replay)
        replay.leg = [(0.0, 0.0, 5.0), (5.0, 0.0, 5.0), (10.0, 0.0, 2.5),
                      (15.0, 0.0, 2.5), (20.0, 0.0, 2.5)]
        replay.fixed_covers = {'junction-west': (5.0, 0.0)}
        name = 'junction-west-r1-1'
        self.assertEqual(replay.cover((5.0, 0.0, 5.6), name=name), (5.0, 0.0))
        self.assertIsNone(replay.cover((10.0, 0.0, 3.1), name=name))
        self.assertEqual(replay.cover((20.0, 0.0, 3.1), name=name), (10.0, 0.0))

    def test_only_explicit_hiding_point_gets_combat_priority(self):
        for name, expected in (('junction-west-r2-pre', True), ('other-leg-pre', False)):
            with self.subTest(name=name):
                replay = M.Replay.__new__(M.Replay)
                replay.leg, replay.wave, replay.waves = LINE, '8', False
                replay.fixed_covers = {'junction-west': (45.0, 8.0)}
                replay.run, replay.tracked = Path('synthetic-run'), Path('synthetic-tracking')
                replay.where = lambda: (31.0, 0.5, 0.0)
                replay.select_retracked_position = lambda: None
                replay.command = Mock(side_effect=lambda tag, *args, **kw:
                                      ['32,0'] if 'skirmish-target' in tag else ['bursts 0'])
                replay.skirmish(name, recover=False)
                fights = [call.args[1] for call in replay.command.call_args_list
                          if any(str(arg).endswith('combat2.py') for arg in call.args[1])]
                self.assertEqual(len(fights), 1)
                self.assertEqual('--cover-first' in fights[0], expected)

    def test_fixed_point_applies_to_named_leg_and_its_retries_only(self):
        f = Fake((31.0, 0.5, 0.0)); f.leg = LINE
        f.fixed_covers = {'junction-west': (45.0, 8.0)}
        self.assertEqual(M.Replay.cover(f, f.at, name='junction-west-r6-1-pre'), (45.0, 8.0))
        self.assertEqual(M.Replay.cover(f, f.at, name='mine-crouched-r2'), LINE[4][:2])
        self.assertEqual(M.Replay.cover(f, f.at, name='junction-western'), LINE[4][:2])
        self.assertIsNone(M.Replay.cover(f, None, name='junction-west'))

    def test_fixed_cover_input_rejects_malformed_or_nonfinite_points(self):
        self.assertEqual(M.parse_fixed_cover('junction-west:45,-8.5'), ('junction-west', (45.0, -8.5)))
        for value in ('junction-west:nan,2', 'junction-west:1,inf', '1,2', 'x:1,2,3', 'x;bad:1,2'):
            with self.subTest(value=value), self.assertRaises(M.argparse.ArgumentTypeError):
                M.parse_fixed_cover(value)

    def test_cover_is_ten_metres_back_along_the_leg(self):
        f = Fake((31.0, 0.5, 0.0)); f.leg = LINE
        self.assertEqual(M.Replay.cover(f, f.at), LINE[4][:2])

    def test_no_cover_at_the_start_of_a_leg(self):
        f = Fake((0.5, 0.0, 0.0)); f.leg = LINE
        self.assertIsNone(M.Replay.cover(f, f.at))


if __name__ == '__main__':
    unittest.main()
