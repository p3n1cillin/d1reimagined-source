"""The waypoint detector on synthetic captures (no game frames in the repo)."""
import importlib.util
from pathlib import Path
import unittest

import numpy as np

source = Path(__file__).resolve().parents[1] / 'tools/ps4_setup/d1-waypoint.py'
spec = importlib.util.spec_from_file_location('d1waypoint', source)
wp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wp)


def frame(background=(40, 45, 50)):
    img = np.zeros((1119, 1936, 3), dtype=np.uint8)
    img[:] = background
    return img


def diamond(img, cx, cy, core=9, ring=3, outer=8, grey=(150, 150, 150)):
    """Draw the waypoint: grey outer diamond, black ring, white core (L1 radii)."""
    ys, xs = np.mgrid[0:img.shape[0], 0:img.shape[1]]
    d = np.abs(xs - cx) + np.abs(ys - cy)
    img[d <= core + ring + outer] = grey
    img[d <= core + ring] = (0, 0, 0)
    img[d <= core] = (255, 255, 255)
    return img


class Markers(unittest.TestCase):
    def test_finds_a_centred_waypoint(self):
        found = wp.markers(diamond(frame(), 900, 600))
        self.assertEqual(len(found), 1)
        cx, cy, size = found[0]
        self.assertLessEqual(abs(cx - 900), 1)
        self.assertLessEqual(abs(cy - 600), 1)
        self.assertTrue(15 <= size <= 21)

    def test_finds_one_pinned_to_the_left_edge_over_a_dark_wall(self):
        found = wp.markers(diamond(frame((8, 8, 10)), 113, 591, grey=(60, 60, 60)))
        self.assertEqual([(c, r) for c, r, _ in found], [(113, 591)])

    def test_ignores_a_white_square_without_a_ring(self):
        img = frame((200, 200, 200))
        img[580:600, 880:900] = 255
        self.assertEqual(wp.markers(img), [])

    def test_ignores_a_bright_blob_of_the_wrong_shape(self):
        img = frame()
        img[590:596, 860:940] = 255          # a lamp streak: wide and flat
        img[586:600, 856:944][img[586:600, 856:944].min(axis=2) < 255] = 0
        self.assertEqual(wp.markers(img), [])

    def test_ignores_the_fixed_hud(self):
        img = diamond(frame(), 200, 200)    # inside the radar box
        img = diamond(img, 300, 1000)       # inside the weapon box
        self.assertEqual(wp.markers(img), [])

    def test_a_diamond_with_a_coloured_surround_is_not_the_waypoint(self):
        self.assertEqual(wp.markers(diamond(frame(), 900, 600, grey=(200, 60, 40))), [])


if __name__ == '__main__':
    unittest.main()
