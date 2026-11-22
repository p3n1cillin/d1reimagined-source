# SPDX-License-Identifier: GPL-2.0-or-later
"""The combat readers on the geometry of the emulator's own frame (fight._emulator_shot): the
1920x1080 game image at (8, 31) inside a 1936x1119 capture, black band above, bar bottom-left.
Synthetic images only. The numbers were measured on build 56578 emulator frames from one lab run:
tuning values, not game data. The emulator readers apply in posted mode (D1_KEYS_POST=1)."""
import importlib.util
import os
import sys
import unittest
from unittest import mock
from pathlib import Path

import numpy as np

TR = Path(__file__).parents[1] / 'lab' / 'tower-route'
sys.path.insert(0, str(TR))


def load(name):
    spec = importlib.util.spec_from_file_location(name, TR / (name + '.py'))
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


HP, PL = load('health'), load('plates')


def frame(fill_px=468, color=(243, 220, 86)):
    """A dark game image in its place, the panel, and a slanted bar of `fill_px` px from x 101."""
    img = np.zeros((1119, 1936, 3), np.uint8)
    img[31:1111, 8:1928] = (30, 26, 20)
    img[838:928, 67:603] = (40, 33, 14)
    for x in range(101, 101 + fill_px):
        yc = int(round(891 - 20.5 * (x - 110) / 458))
        img[yc - 8:yc + 8, x] = color
    return img


class EmulatorFrameHealth(unittest.TestCase):
    def setUp(self):
        p = mock.patch.dict(os.environ, {'D1_KEYS_POST': '1'})
        p.start()
        self.addCleanup(p.stop)

    def test_full_bar(self):
        self.assertAlmostEqual(HP.health(frame()), 1.0, delta=0.01)

    def test_partial_bars_follow_the_slant(self):
        for px in (100, 234, 400):
            self.assertAlmostEqual(HP.health(frame(px)), px / 468, delta=0.02)

    def test_red_fill_counts(self):
        self.assertAlmostEqual(HP.health(frame(60, (200, 40, 30))), 60 / 468, delta=0.03)

    def test_no_bar_is_none(self):
        self.assertIsNone(HP.health(frame(0)))

    def test_empty_bar_with_the_hud_up_is_zero(self):
        img = frame(0)
        img[905:975, 101:250] = (60, 120, 170)             # the ability icons
        self.assertEqual(HP.health(img), 0.0)

    def test_uniform_scenes_are_not_a_bar(self):
        for colour in ((245, 225, 100), (255, 255, 255), (200, 40, 30)):
            img = np.zeros((1119, 1936, 3), np.uint8)
            img[:] = colour
            self.assertIsNone(HP.bottom_left(img.astype(int)), colour)

    def test_a_full_bar_in_a_red_lit_scene_is_not_critical(self):
        img = frame()
        keep = img[830:935, 60:610].copy()
        img[:, :200] = img[:, -200:] = (200, 20, 20)
        img[:200] = img[-200:] = (200, 20, 20)
        img[830:935, 60:610] = keep
        self.assertAlmostEqual(HP.health(img), 1.0, delta=0.01)
        self.assertFalse(HP.critical(img))

    def test_desktop_mode_ignores_the_bottom_left_bar(self):
        with mock.patch.dict(os.environ, {'D1_KEYS_POST': ''}):
            self.assertIsNone(HP.health(frame()))

    def test_scenery_yellow_beside_the_bar_is_not_a_bar(self):
        img = frame(0)
        img[860:900, 300:400] = (243, 220, 86)
        self.assertIsNone(HP.health(img))

    def test_critical_needs_the_bar_and_red_edges(self):
        img = frame(30, (200, 40, 30))
        keep = img[830:935, 60:610].copy()
        self.assertFalse(HP.critical(img))
        img[:, :200] = (200, 20, 20)
        img[:, -200:] = (200, 20, 20)
        img[:200] = (200, 20, 20)
        img[-200:] = (200, 20, 20)
        img[830:935, 60:610] = keep
        self.assertTrue(HP.critical(img))


class EmulatorFramePlates(unittest.TestCase):
    def test_a_nameplate_is_found_with_the_band_present(self):
        img = frame()
        img[686:698, 990:1170] = (190, 54, 42)             # the bar
        img[660:675, 990:1030] = (240, 240, 240)           # the name above it
        got = PL.plates(img)
        self.assertEqual(len(got), 1)
        self.assertAlmostEqual(got[0][0], 1080, delta=3)

    def test_the_bottom_left_bar_is_not_a_plate(self):
        self.assertEqual(PL.plates(frame(468, (200, 40, 30))), [])


if __name__ == '__main__':
    unittest.main()
