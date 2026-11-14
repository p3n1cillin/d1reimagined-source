# SPDX-License-Identifier: GPL-2.0-or-later
import importlib.util
import math
from pathlib import Path
import sys
import unittest

TOOLS = Path(__file__).parents[1] / 'tools/ps4_setup'
sys.path.insert(0, str(TOOLS))
SPEC = importlib.util.spec_from_file_location('d1_walk', TOOLS / 'd1-walk.py')
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)


def line(n, spacing=1.5):
    return [[k * spacing, 0, 0] for k in range(n)]


class LoadRoute(unittest.TestCase):
    def test_three_and_four_elements(self):
        r = M.load_route([[0, 0, 0], [1, 0, 0, 1], [2, 0, 0, 0], [3, 0, 0, True]])
        self.assertEqual([p[3] for p in r], [False, True, False, True])
        self.assertTrue(all(len(p) == 4 for p in r))
        self.assertEqual(math.dist(r[0][:2], r[1][:2]), 1.0)

    def test_invalid(self):
        for bad in ([[0, 0]], [[0, 0, 0, 1, 2]], [[0, 'a', 0]], [[0, float('nan'), 0]], [5]):
            with self.assertRaises(ValueError):
                M.load_route(bad)

    def test_default_unchanged(self):
        r = M.load_route(line(10))
        self.assertEqual(len(r), 10)


class ThinRoute(unittest.TestCase):
    def test_thins_keeps_ends(self):
        r = M.load_route(line(40), 6.0)
        self.assertEqual(r[0][:3], [0, 0, 0])
        self.assertEqual(r[-1][:3], [39 * 1.5, 0, 0])
        self.assertLess(len(r), 40)
        for a, b in zip(r[:-2], r[1:-1]):
            self.assertGreaterEqual(math.dist(a[:2], b[:2]), 6.0)

    def test_keeps_jump_and_previous(self):
        pts = line(40)
        pts[13] = pts[13] + [1]
        pts[27] = pts[27] + [1]
        r = M.load_route(pts, 6.0)
        xs = [p[0] for p in r]
        for k in (12, 13, 26, 27):
            self.assertIn(k * 1.5, xs)
        self.assertEqual([p[0] for p in r if p[3]], [13 * 1.5, 27 * 1.5])

    def test_short_routes(self):
        self.assertEqual(len(M.thin_route(line(2), 100)), 2)


class ShouldJump(unittest.TestCase):
    def setUp(self):
        self.route = M.load_route([[0, 0, 0], [10, 0, 0, 1], [20, 0, 0]])

    def test_distance(self):
        self.assertFalse(M.should_jump(1, self.route, (6.0, 0, 0), set()))
        self.assertTrue(M.should_jump(1, self.route, (10 - M.JUMP_AT + 0.1, 0, 0), set()))
        self.assertFalse(M.should_jump(1, self.route, (10 - M.JUMP_AT, 0, 0), set()))

    def test_once(self):
        jumped = set()
        here = (9.0, 0, 0)
        self.assertTrue(M.should_jump(1, self.route, here, jumped))
        jumped.add(1)
        self.assertFalse(M.should_jump(1, self.route, here, jumped))

    def test_unflagged_and_range(self):
        self.assertFalse(M.should_jump(0, self.route, (0, 0, 0), set()))
        self.assertFalse(M.should_jump(3, self.route, (0, 0, 0), set()))


if __name__ == '__main__':
    unittest.main()
