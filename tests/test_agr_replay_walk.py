import importlib.util
import math
from pathlib import Path
import sys
import unittest

TOOLS = Path(__file__).parents[1] / 'tools/ps4_setup'
sys.path.insert(0, str(TOOLS))
SPEC = importlib.util.spec_from_file_location('agr_replay', TOOLS / 'd1-agr-replay.py')
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)

LINE = [(float(x), 0.0, 0.0) for x in range(0, 50, 5)]     # ten waypoints along x


class Fake:
    """walk_fighting's surroundings: a Guardian that walks each route to its end unless told not to."""

    def __init__(self, at, stall=0):
        self.at, self.stall, self.walks, self.pads = at, stall, [], []

    def where(self):
        return self.at

    def skirmish(self, name):
        pass

    def write_route(self, name, points):
        return list(points)

    def walk_once(self, tag, route, *rest, **kw):
        self.walks.append((tag, route))
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
    walk_fighting = M.Replay.walk_fighting


class WalkFightingTest(unittest.TestCase):
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

    def test_stall_backs_off_before_retrying(self):
        f = Fake((0.0, 0.0, 0.0), stall=1)
        f.walk_fighting('leg', LINE, (), 300, chunk=4)
        tags = [t for t, _ in f.walks]
        self.assertEqual(tags[:3], ['leg-0', 'leg-0-back', 'leg-0-retry1'])
        self.assertTrue(f.pads)
        self.assertEqual(f.at, LINE[-1])


if __name__ == '__main__':
    unittest.main()
