import importlib.util
import math
from pathlib import Path
import sys
import os
import unittest
from unittest import mock

import numpy as np

LAB = Path(__file__).parents[1] / 'lab/tower-route'
sys.path.insert(0, str(LAB))
try:
    SPEC = importlib.util.spec_from_file_location('combat2', LAB / 'combat2.py')
    M = importlib.util.module_from_spec(SPEC)
    SPEC.loader.exec_module(M)
except Exception as e:      # combat2 loads the Windows-only memory readers
    M, SKIP = None, str(e)

PLATES_SPEC = importlib.util.spec_from_file_location('plates', LAB / 'plates.py')
PLATES = importlib.util.module_from_spec(PLATES_SPEC)
PLATES_SPEC.loader.exec_module(PLATES)


def chevron(angle_deg):
    """A window-sized frame with an orange-red chevron on the marker ring at `angle_deg` clockwise."""
    img = np.zeros((1119, 1936, 3), np.uint8)
    a = math.radians(angle_deg)
    x, y = int(M.CX + 175 * math.sin(a)), int(M.CY - 175 * math.cos(a))
    img[y - 12:y + 12, x - 12:x + 12] = (220, 50, 0)
    return img


@unittest.skipIf(M is None, 'combat2 needs its Windows readers')
class DamageDir(unittest.TestCase):
    def test_reads_the_marker_bearing(self):
        for deg in (0, 90, 135, -90, 180):
            got = math.degrees(M.damage_dir(chevron(deg)))
            self.assertAlmostEqual((got - deg + 180) % 360 - 180, 0, delta=6)

    def test_nothing_without_a_marker(self):
        self.assertIsNone(M.damage_dir(np.zeros((1119, 1936, 3), np.uint8)))

    def test_ignores_muzzle_flash_bloom(self):
        img = chevron(160)
        yy, xx = np.mgrid[:1119, :1936]
        r = np.hypot(xx - M.CX, yy - M.CY)
        img[r < 90] = (245, 220, 180)
        with mock.patch.dict(os.environ, {'D1_KEYS_POST': '1'}):
            self.assertIsNone(M.damage_dir(img))
        with mock.patch.dict(os.environ, {'D1_KEYS_POST': ''}):    # desktop grabs: unchanged
            self.assertIsNotNone(M.damage_dir(img))


class Plates(unittest.TestCase):
    def test_finds_nameplate_with_white_text_on_dark_background(self):
        img = np.full((1119, 1936, 3), (30, 30, 30), np.uint8)
        img[680:692, 1000:1120] = (190, 54, 42)
        img[658:665, 1005:1012] = (230, 230, 230)
        self.assertEqual(PLATES.plates(img), [(1059, 685, 120, 12)])

    def test_ignores_red_bar_under_muzzle_flash_glare(self):
        img = np.full((1119, 1936, 3), (30, 30, 30), np.uint8)
        img[680:692, 1000:1120] = (190, 54, 42)
        img[650:677, 1000:1120] = (255, 120, 60)
        img[655:665, 1005:1015] = (255, 230, 210)
        with mock.patch.dict(os.environ, {'D1_KEYS_POST': '1'}):
            self.assertEqual(PLATES.plates(img), [])
        with mock.patch.dict(os.environ, {'D1_KEYS_POST': ''}):    # desktop grabs: unchanged
            self.assertEqual(len(PLATES.plates(img)), 1)


if __name__ == '__main__':
    unittest.main()
