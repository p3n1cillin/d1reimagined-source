"""Pad key names to virtual keys, as tools/d1-pad.ps1 maps them (tools/ps4_setup/d1_keys.py)."""
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch, Mock, mock_open
from types import SimpleNamespace

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

    def test_arrow_and_numpad_same_scan_have_distinct_down_and_up_flags(self):
        # Both keys map to scan 0x50 on Windows; the E0 flag distinguishes them for SDL.
        user = Mock()
        user.MapVirtualKeyW.return_value = 0x50
        with patch.object(M.ctypes, 'windll', SimpleNamespace(user32=user), create=True), \
             patch.object(M, '_window', return_value=1), \
             patch.object(M, '_focus', return_value=True), \
             patch.object(M.time, 'sleep'), patch('builtins.open', mock_open()):
            M.press('Down', 'NumPad2', gap=0)
        self.assertEqual([call.args for call in user.keybd_event.call_args_list],
                         [(0x28, 0x50, 1, 0), (0x28, 0x50, 3, 0),
                          (0x62, 0x50, 0, 0), (0x62, 0x50, 2, 0)])


if __name__ == '__main__':
    unittest.main()
