# SPDX-License-Identifier: GPL-2.0-or-later
"""combat2.key and fight.grab in posted mode (D1_KEYS_POST=1): offline, no emulator."""
import importlib.util
import os
import sys
import time
import unittest
from pathlib import Path
from unittest import mock

import numpy as np

ROOT = Path(__file__).parents[1]
TR = ROOT / 'lab' / 'tower-route'
sys.path.insert(0, str(ROOT / 'tools' / 'ps4_setup'))
sys.path.insert(0, str(TR))


def load(name):
    spec = importlib.util.spec_from_file_location(name, TR / (name + '.py'))
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


try:
    import d1_keys
    fight = load('fight')
    combat2 = load('combat2')
    ERR = None
except Exception as e:  # lab dependencies missing (not Windows, no game tooling)
    ERR = e


@unittest.skipIf(ERR is not None, 'combat2/fight not importable here: %s' % ERR)
class Posted(unittest.TestCase):
    def setUp(self):
        self.env = mock.patch.dict(os.environ, {}, clear=False)
        self.env.start()
        os.environ.pop('D1_KEYS_POST', None)
        combat2._post_hwnd = None
        combat2._stamped = 1e12   # skip the idle-guard stamp file write

    def tearDown(self):
        self.env.stop()

    def fake_u32(self, post_ok=True):
        u = mock.Mock()
        u.MapVirtualKeyW.return_value = 0x11
        u.PostMessageW.return_value = 1 if post_ok else 0
        return u

    def test_posted_key_down_and_up(self):
        os.environ['D1_KEYS_POST'] = '1'
        u = self.fake_u32()
        with mock.patch.object(combat2, 'u32', u), mock.patch.object(combat2, '_gf') as gf, \
                mock.patch.object(d1_keys, '_window', return_value=77) as win:
            combat2.key('W', True)
            combat2.key('W', False)
        gf.ensure.assert_not_called()
        u.keybd_event.assert_not_called()
        down, up = u.PostMessageW.call_args_list
        self.assertEqual(down.args, (77, 0x100, 0x57, d1_keys.post_lparam(0x57, 0x11, True)))
        self.assertEqual(up.args, (77, 0x101, 0x57, d1_keys.post_lparam(0x57, 0x11, False)))
        win.assert_called_once()   # cached

    def test_refinds_window_when_post_fails(self):
        os.environ['D1_KEYS_POST'] = '1'
        u = self.fake_u32()
        u.PostMessageW.side_effect = [0, 1]
        with mock.patch.object(combat2, 'u32', u), mock.patch.object(d1_keys, '_window', side_effect=[5, 6]) as win:
            combat2.key('A', True)
        self.assertEqual(win.call_count, 2)
        self.assertEqual(u.PostMessageW.call_args.args[0], 6)

    def test_default_key_unchanged(self):
        u = self.fake_u32()
        with mock.patch.object(combat2, 'u32', u), mock.patch.object(combat2, '_gf') as gf:
            combat2.key('W', True)
            combat2.key('W', False)
        gf.ensure.assert_called_once()
        self.assertEqual([c.args[1:3] for c in u.keybd_event.call_args_list], [(0x11, 0), (0x11, 2)])
        u.PostMessageW.assert_not_called()

    def test_posted_grab_uses_emulator_shot_only(self):
        os.environ['D1_KEYS_POST'] = '1'
        from PIL import Image
        img = Image.new('RGB', (4, 3), (9, 8, 7))
        out = os.path.join(os.environ.get('TEMP', '.'), 'combat_posted_grab.png')
        with mock.patch.object(d1_keys, '_window', return_value=1), \
                mock.patch.object(d1_keys, '_focus') as focus, \
                mock.patch('PIL.ImageGrab.grab') as grab, \
                mock.patch.object(fight, '_emulator_shot', return_value=img) as shot, \
                mock.patch.object(fight.subprocess, 'run') as run:
            a = fight.grab(out)
        focus.assert_not_called()
        grab.assert_not_called()
        run.assert_not_called()
        shot.assert_called_once()
        self.assertEqual(a.shape, (3, 4, 3))
        self.assertEqual(tuple(a[0, 0]), (9, 8, 7))
        os.remove(out)

    def test_posted_grab_failed_shot_reuses_last_then_fails(self):
        os.environ['D1_KEYS_POST'] = '1'
        from PIL import Image
        out = os.path.join(os.environ.get('TEMP', '.'), 'combat_posted_fail.png')
        fight._last_good[:] = [None, 0.0]
        with mock.patch.object(d1_keys, '_window', return_value=1),                 mock.patch.object(fight.subprocess, 'run') as run,                 mock.patch.object(fight, '_emulator_shot', side_effect=[Image.new('RGB', (4, 3), (1, 2, 3)), None]):
            first = fight.grab(out)
            again = fight.grab(out)          # shot fails: the last good frame
            self.assertEqual(tuple(again[0, 0]), (1, 2, 3))
            self.assertTrue((first == again).all())
        fight._last_good[:] = [None, 0.0]
        with mock.patch.object(d1_keys, '_window', return_value=1),                 mock.patch.object(fight.subprocess, 'run') as run,                 mock.patch.object(fight, '_emulator_shot', return_value=None):
            with self.assertRaises(FileNotFoundError):
                fight.grab(out)
        run.assert_not_called()
        os.remove(out)

    def _posted_grab_no_subprocess(self, **shot_kw):
        os.environ['D1_KEYS_POST'] = '1'
        out = os.path.join(os.environ.get('TEMP', '.'), 'combat_posted_x.png')
        with mock.patch.object(fight.subprocess, 'run') as run, \
                mock.patch.object(fight, '_emulator_shot', **shot_kw):
            with self.assertRaises(FileNotFoundError):
                fight.grab(out)
        run.assert_not_called()

    def test_posted_grab_without_window_never_runs_script(self):
        fight._last_good[:] = [None, 0.0]
        with mock.patch.object(d1_keys, '_window', return_value=None):
            self._posted_grab_no_subprocess(return_value=None)

    def test_posted_grab_shot_error_never_runs_script(self):
        fight._last_good[:] = [None, 0.0]
        with mock.patch.object(d1_keys, '_window', return_value=1):
            self._posted_grab_no_subprocess(side_effect=RuntimeError('No shadPS4 window'))

    def test_posted_grab_stale_frame_raises(self):
        from PIL import Image
        fight._last_good[:] = [Image.new('RGB', (4, 3)), time.time() - fight.MAX_FRAME_AGE - 1]
        with mock.patch.object(d1_keys, '_window', return_value=1):
            self._posted_grab_no_subprocess(return_value=None)
        fight._last_good[:] = [None, 0.0]

    def test_pad_posted_fallback_passes_post(self):
        os.environ['D1_KEYS_POST'] = '1'
        with mock.patch.object(d1_keys, 'press', side_effect=RuntimeError('No shadPS4 window')), \
                mock.patch.object(fight.subprocess, 'run') as run:
            fight.pad('Hold:O:100')
        self.assertIn('-Post', run.call_args.args[0])
        os.environ.pop('D1_KEYS_POST')
        with mock.patch.object(d1_keys, 'press', side_effect=RuntimeError('x')), \
                mock.patch.object(fight.subprocess, 'run') as run:
            fight.pad('Hold:O:100')
        self.assertNotIn('-Post', run.call_args.args[0])

    def test_posted_key_failure_raises(self):
        os.environ['D1_KEYS_POST'] = '1'
        u = self.fake_u32(post_ok=False)
        with mock.patch.object(combat2, 'u32', u), mock.patch.object(d1_keys, '_window', return_value=3):
            with self.assertRaises(RuntimeError):
                combat2.key('W', False)
            with self.assertRaises(RuntimeError):
                combat2.release_all()
        self.assertGreater(u.PostMessageW.call_count, len(combat2.VK))
        with mock.patch.object(combat2, 'u32', u), mock.patch.object(d1_keys, '_window', return_value=None):
            with self.assertRaises(RuntimeError):
                combat2.key('W', False)

    def test_default_grab_still_focuses_and_grabs(self):
        from PIL import Image
        out = os.path.join(os.environ.get('TEMP', '.'), 'combat_default_grab.png')
        with mock.patch.object(d1_keys, '_window', return_value=1), \
                mock.patch.object(d1_keys, '_focus', return_value=True) as focus, \
                mock.patch('PIL.ImageGrab.grab', return_value=Image.new('RGB', (4, 3), (200, 200, 200))) as grab, \
                mock.patch.object(fight, '_emulator_shot') as shot:
            a = fight.grab(out)
        focus.assert_called_once()
        grab.assert_called_once()
        shot.assert_not_called()
        self.assertEqual(a.shape, (3, 4, 3))
        os.remove(out)


if __name__ == '__main__':
    unittest.main()
