"""Pad key names to virtual keys, as tools/d1-pad.ps1 maps them (tools/ps4_setup/d1_keys.py)."""
import importlib.util
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location('d1_keys', Path(__file__).parents[1] / 'tools/ps4_setup/d1_keys.py')
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)


class Keys(unittest.TestCase):
    def test_names_map_to_their_virtual_keys(self):
        self.assertEqual([M.vk(n) for n in ('W', 'NumPad2', 'NumPad6', 'Return', 'Up', 'L', 'D0', 'F5')],
                         [0x57, 0x62, 0x66, 0x0D, 0x26, 0x4C, 0x30, 0x74])

    def test_holds_and_comma_joined_keys_parse_like_the_script(self):
        self.assertEqual(M.parse(['Hold:S:600,Hold:A:500', 'NumPad2']),
                         [('S', 0x53, 600), ('A', 0x41, 500), ('NumPad2', 0x62, 120)])

    def test_an_unknown_key_is_refused(self):
        with self.assertRaises(ValueError):
            M.parse(['Hold:Banana:100'])


if __name__ == '__main__':
    unittest.main()
