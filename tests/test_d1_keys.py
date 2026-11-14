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
                         [(('S',), (0x53,), 600), (('A',), (0x41,), 500), (('NumPad2',), (0x62,), 120)])

    def test_chords_parse_with_and_without_a_hold(self):
        self.assertEqual(M.parse(['Hold:W+NumPad2:900', 'W+NumPad2']),
                         [(('W', 'NumPad2'), (0x57, 0x62), 900), (('W', 'NumPad2'), (0x57, 0x62), 120)])
        with self.assertRaises(ValueError):
            M.parse(['Hold:W+Banana:100'])

    def test_posted_chord_goes_down_in_order_and_up_in_reverse(self):
        user = Mock()
        user.MapVirtualKeyW.side_effect = lambda code, _: {0x57: 0x11, 0x62: 0x50}[code]
        with patch.object(M.ctypes, 'windll', SimpleNamespace(user32=user), create=True),              patch.object(M, '_window', return_value=7),              patch.object(M, '_focus', side_effect=AssertionError('focused')),              patch.object(M.time, 'sleep'), patch('builtins.open', mock_open()) as op:
            M.press('Hold:W+NumPad2:900', gap=0, post=True)
        self.assertEqual([c.args for c in user.PostMessageW.call_args_list],
                         [(7, 0x100, 0x57, 0x00110001), (7, 0x100, 0x62, 0x00500001),
                          (7, 0x101, 0x62, 0xC0500001), (7, 0x101, 0x57, 0xC0110001)])
        op.assert_called()

    def test_keybd_chord_focuses_once_and_releases_in_reverse(self):
        user = Mock()
        user.MapVirtualKeyW.side_effect = lambda code, _: {0x57: 0x11, 0x62: 0x50}[code]
        with patch.object(M.ctypes, 'windll', SimpleNamespace(user32=user), create=True),              patch.object(M, '_window', return_value=7),              patch.object(M, '_focus', return_value=True) as focus,              patch.object(M.time, 'sleep'), patch('builtins.open', mock_open()):
            M.press('W+NumPad2', gap=0, post=False)
        focus.assert_called_once()
        self.assertEqual([c.args for c in user.keybd_event.call_args_list],
                         [(0x57, 0x11, 0, 0), (0x62, 0x50, 0, 0), (0x62, 0x50, 2, 0), (0x57, 0x11, 2, 0)])

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

    def test_post_lparam_matches_the_script(self):
        self.assertEqual(M.post_lparam(0x57, 0x11, True), 0x00110001)
        self.assertEqual(M.post_lparam(0x57, 0x11, False), 0xC0110001)
        self.assertEqual(M.post_lparam(0x28, 0x50, True), 0x01500001)
        self.assertEqual(M.post_lparam(0x28, 0x50, False), 0xC1500001)
        self.assertEqual(M.post_lparam(0x62, 0x50, True), 0x00500001)

    def test_post_mode_selection(self):
        with patch.dict(M.os.environ, {}, clear=False):
            M.os.environ.pop('D1_KEYS_POST', None)
            self.assertFalse(M.post_mode())
            M.os.environ['D1_KEYS_POST'] = '1'
            self.assertTrue(M.post_mode())
            self.assertFalse(M.post_mode(False))
            M.os.environ['D1_KEYS_POST'] = '0'
            self.assertFalse(M.post_mode())
            self.assertTrue(M.post_mode(True))

    def test_posted_press_never_focuses_or_sends_real_input(self):
        user = Mock()
        user.MapVirtualKeyW.return_value = 0x50
        with patch.object(M.ctypes, 'windll', SimpleNamespace(user32=user), create=True),              patch.object(M, '_window', return_value=7),              patch.object(M, '_focus', side_effect=AssertionError('focused')),              patch.object(M.time, 'sleep'), patch('builtins.open', mock_open()) as op:
            M.press('Down', 'NumPad2', gap=0, post=True)
        self.assertEqual([c.args for c in user.PostMessageW.call_args_list],
                         [(7, 0x100, 0x28, 0x01500001), (7, 0x101, 0x28, 0xC1500001),
                          (7, 0x100, 0x62, 0x00500001), (7, 0x101, 0x62, 0xC0500001)])
        user.keybd_event.assert_not_called()
        op.assert_called()   # the idle-guard stamp is still written


if __name__ == '__main__':
    unittest.main()
