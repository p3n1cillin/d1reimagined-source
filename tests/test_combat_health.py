import importlib.util
from pathlib import Path
import unittest

import numpy as np

SPEC = importlib.util.spec_from_file_location('combat_health', Path(__file__).parents[1] / 'lab/tower-route/health.py')
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)


def frame(edge_rgb, mid_rgb=(40, 40, 40), h=1119, w=1936):
    a = np.empty((h, w, 3), int)
    a[:] = edge_rgb
    a[h // 5:4 * h // 5, w // 5:4 * w // 5] = mid_rgb
    return a


class Critical(unittest.TestCase):
    def test_red_edges_are_critical(self):
        a = frame((200, 40, 40))
        a[173:176, M.X0:M.X1] = (75, 75, 75)        # the bar's outline: the bar is drawn
        a[166:171, M.X0:M.X1] = a[178:183, M.X0:M.X1] = (40, 40, 40)
        self.assertTrue(M.critical(a))

    def test_red_light_without_the_bar_is_not_critical(self):
        # sense124: the tripmine corridor's red lasers, health full (bar hidden).
        self.assertFalse(M.critical(frame((200, 40, 40))))

    def test_rust_and_dark_frames_are_not(self):
        self.assertFalse(M.critical(frame((150, 80, 40))))      # orange rust: red leads by 70
        self.assertFalse(M.critical(frame((30, 30, 30), (230, 20, 20))))   # red only in the middle

    def test_no_bar_reads_none(self):
        self.assertIsNone(M.health(frame((30, 30, 30))))


def bar(fill_px, fill_rgb=(200, 30, 30)):
    """A dark frame with the health bar's outline drawn (row 174) and `fill_px` of fill."""
    a = frame((30, 30, 30), (30, 30, 30))
    a[173:176, M.X0:M.X1] = (75, 75, 75)
    a[177:187, M.X0:M.X1] = (50, 30, 30)              # the empty track, faintly red
    a[177:187, M.X0:M.X0 + fill_px] = fill_rgb
    return a


class Sliver(unittest.TestCase):
    # sense116's hangar deaths: a drawn bar with a few px of red read None (full health).
    def test_a_red_sliver_reads_low(self):
        h = M.health(bar(5))
        self.assertIsNotNone(h)
        self.assertLess(h, 0.03)

    def test_a_drawn_empty_bar_reads_zero(self):
        self.assertEqual(M.health(bar(0)), 0.0)

    def test_no_outline_still_reads_none(self):
        a = bar(5)
        a[173:176, M.X0:M.X1] = (30, 30, 30)
        self.assertIsNone(M.health(a))


class Fill(unittest.TestCase):
    def test_a_glow_on_the_left_end_does_not_cut_the_fill(self):
        # sense123: a light behind the bar's left end read a three-quarters bar as 0.30.
        a = frame((20, 20, 20), (20, 20, 20))
        a[176:188, M.X0:M.X0 + 340] = (157, 157, 157)
        a[176:188, M.X0:M.X0 + 20] = (198, 198, 198)
        self.assertAlmostEqual(M.health(a), 340 / (M.X1 - M.X0), delta=0.02)


if __name__ == '__main__':
    unittest.main()
