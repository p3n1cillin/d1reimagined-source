# SPDX-License-Identifier: GPL-2.0-or-later
import importlib.util
from pathlib import Path
import tempfile
import unittest

TOOLS = Path(__file__).parents[1] / 'tools/ps4_setup'
SPEC = importlib.util.spec_from_file_location('route_record', TOOLS / 'd1-route-record.py')
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)


class ThinTests(unittest.TestCase):
    def test_empty(self):
        self.assertEqual(M.thin([], 1.5), [])

    def test_drops_close_points_keeps_first_and_last(self):
        pts = [(0.0, 0.0, 0.0), (0.5, 0.0, 0.0), (1.0, 0.0, 0.0), (2.0, 0.0, 0.0), (2.2, 0.0, 0.0)]
        self.assertEqual(M.thin(pts, 1.5), [(0.0, 0.0, 0.0), (2.0, 0.0, 0.0), (2.2, 0.0, 0.0)])

    def test_single_point(self):
        self.assertEqual(M.thin([(1.0, 2.0, 3.0)], 1.5), [(1.0, 2.0, 3.0)])

    def test_uses_height(self):
        self.assertEqual(len(M.thin([(0.0, 0.0, 0.0), (0.0, 0.0, 2.0)], 1.5)), 2)


class SegmentTests(unittest.TestCase):
    def test_split_on_jump(self):
        s = [(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (100.0, 0.0, 0.0), (101.0, 0.0, 0.0)]
        self.assertEqual(M.segments(s, 8.0), [[(0.0, 0.0, 0.0), (1.0, 0.0, 0.0)], [(100.0, 0.0, 0.0), (101.0, 0.0, 0.0)]])

    def test_none_skipped_and_does_not_split(self):
        s = [(0.0, 0.0, 0.0), None, None, (1.0, 0.0, 0.0)]
        self.assertEqual(len(M.segments(s, 8.0)), 1)
        self.assertEqual(M.segments([None, None], 8.0), [])

    def test_jump_across_none_still_splits(self):
        self.assertEqual(len(M.segments([(0.0, 0.0, 0.0), None, (50.0, 0.0, 0.0)], 8.0)), 2)


class WriteTests(unittest.TestCase):
    def test_paths_and_files(self):
        self.assertEqual(M.output_paths('r.json', 3), ['r.json', 'r-2.json', 'r-3.json'])
        with tempfile.TemporaryDirectory() as d:
            out = str(Path(d) / 'route.json')
            s = [(0.0, 0.0, 0.0), (3.0, 0.0, 0.0), (90.0, 0.0, 0.0), (93.0, 0.0, 0.0)]
            written = M.write_routes(out, s, 1.5, 8.0)
            self.assertEqual(len(written), 2)
            self.assertTrue((Path(d) / 'route-2.json').exists())


if __name__ == '__main__':
    unittest.main()
