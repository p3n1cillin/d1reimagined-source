import importlib.util
import math
from pathlib import Path
import sys
import unittest

import numpy as np

LAB = Path(__file__).parents[1] / 'lab/tower-route'
sys.path.insert(0, str(LAB))
try:
    SPEC = importlib.util.spec_from_file_location('combat2', LAB / 'combat2.py')
    M = importlib.util.module_from_spec(SPEC)
    SPEC.loader.exec_module(M)
except Exception as e:      # combat2 loads the Windows-only memory readers
    M, SKIP = None, str(e)


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


if __name__ == '__main__':
    unittest.main()
