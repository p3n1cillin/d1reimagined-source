"""serve_pending.py's group choice (no game access)."""
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('serve_pending', ROOT/'lab/tower-route/serve_pending.py')
serve = importlib.util.module_from_spec(spec)
spec.loader.exec_module(serve)


class Group(unittest.TestCase):
    def test_the_lifetime_rows_group_wins(self):
        # The Tower's pending list: the lifetime record (kind 0x10) is in the common group 0x40aebf90.
        lines = ['# world 0x1 gen 2 local id 1', '0xf2d88bd0 0x6 0 1 0', '0xf2d88bd0 0x6 1 1 0',
                 '0x40aebf90 0xf 0 1 0', '0x40aebf90 0x10 2 1 0']
        self.assertEqual(serve.lifetime_group(lines), '0x40aebf90')

    def test_without_a_lifetime_row_the_largest_group(self):
        self.assertEqual(serve.lifetime_group(['0xaa 0x1 0 1 0', '0xbb 0x1 1 1 0', '0xbb 0x2 2 1 0']), '0xbb')

    def test_an_empty_list_has_no_group(self):
        self.assertIsNone(serve.lifetime_group(['# world 0x1']))


if __name__ == '__main__':
    unittest.main()
