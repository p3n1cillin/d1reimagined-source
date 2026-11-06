# SPDX-License-Identifier: GPL-2.0-or-later
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('frames', Path(__file__).parents[1] / 'tools/ps4_setup/d1-frames.py')
F = importlib.util.module_from_spec(spec)
spec.loader.exec_module(F)


def entry(h, kind, sub, body, bits):
    return {'hash': h, 'i8': kind, 'sub': sub, 'body': body, 'body_bits': bits}


class Compare(unittest.TestCase):
    def test_compare_names_one_sided_rows_and_counts_differing_bodies_by_group_and_kind(self):
        # F40: the same squad, 3 bits with its B-presence bit and 2 without.
        a = {'entries': [entry('0xfc0e45a3', 1, 36, '80', 3), entry('0xfc0e45a3', 1, 37, '80', 3),
                         entry('0xfc0e45a3', 29, 120, 'f000000000', 35)]}
        b = {'entries': [entry('0xfc0e45a3', 1, 36, '80', 2), entry('0xfc0e45a3', 1, 37, '80', 2),
                         entry('0x992bb471', 6, 1, 'e0', 9)]}
        only_a, only_b, diff, example = F.compare(a, b)
        self.assertEqual(only_a, [(0xfc0e45a3, 29, 120)])
        self.assertEqual(only_b, [(0x992bb471, 6, 1)])
        self.assertEqual(dict(diff), {(0xfc0e45a3, 1): 2})
        self.assertEqual(example[(0xfc0e45a3, 1)], ((0xfc0e45a3, 1, 36), 3, 2))

    def test_first_full_naming_skips_deltas_and_other_groups(self):
        dump = [({}, {'init': False, 'entries': [entry('0x1', 1, 0, '', 2)]}),
                ({}, {'init': True, 'entries': [entry('0x2', 1, 0, '', 2)]}),
                ({}, None),
                ({'n': 3}, {'init': True, 'entries': [entry('0x1', 1, 0, '', 2)]})]
        self.assertEqual(F.first_full_naming(dump, 1)[0], {'n': 3})
        self.assertEqual(F.first_full_naming(dump, 9), (None, None))


if __name__ == '__main__':
    unittest.main()
