# SPDX-License-Identifier: GPL-2.0-or-later
"""d1-record.py pure logic: stale tap cleanup, the wait-for-boot predicate, flag removal."""
import importlib.util
import os
import sys
import tempfile
import unittest
from unittest import mock

HERE = os.path.dirname(os.path.abspath(__file__))
PATH = os.path.join(HERE, '..', 'tools', 'ps4_setup', 'd1-record.py')
spec = importlib.util.spec_from_file_location('d1_record', PATH)
rec = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rec)


class Clock:
    def __init__(self):
        self.t = 0.0

    def now(self):
        return self.t

    def sleep(self, s):
        self.t += s


class StaleTaps(unittest.TestCase):
    def test_locked_file_is_skipped_not_fatal(self):
        with tempfile.TemporaryDirectory() as d:
            a, b = (os.path.join(d, n) for n in ('port0-48000-2-1.f32', 'port1-48000-2-2.f32'))
            for f in (a, b):
                open(f, 'wb').close()
            real = os.remove

            def fake(p):
                if p == a:
                    raise PermissionError(32, 'in use')
                real(p)
            with mock.patch.object(rec.os, 'remove', fake):
                removed, skipped = rec.cleanup_stale_taps(d)
            self.assertEqual(skipped, [a])
            self.assertEqual(removed, [b])
            self.assertTrue(os.path.exists(a) and not os.path.exists(b))

    def test_flag_removed_after_exception(self):
        with tempfile.TemporaryDirectory() as d:
            argv = ['d1-record.py', '--out', os.path.join(d, 'o.mp4'), '--tap', d]
            with mock.patch.object(sys, 'argv', argv), \
                    mock.patch.object(rec, 'record', side_effect=RuntimeError('boom')):
                with self.assertRaises(RuntimeError):
                    rec.main()
            self.assertFalse(os.path.exists(os.path.join(d, 'rec.flag')))

    def test_flag_removed_after_keyboard_interrupt(self):
        with tempfile.TemporaryDirectory() as d:
            argv = ['d1-record.py', '--out', os.path.join(d, 'o.mp4'), '--tap', d]
            with mock.patch.object(sys, 'argv', argv), \
                    mock.patch.object(rec, 'record', side_effect=KeyboardInterrupt):
                with self.assertRaises(KeyboardInterrupt):
                    rec.main()
            self.assertFalse(os.path.exists(os.path.join(d, 'rec.flag')))


class WaitForBoot(unittest.TestCase):
    def test_predicate(self):
        self.assertTrue(rec.boot_ready(200, 200))
        self.assertTrue(rec.boot_ready(500, 200))
        self.assertFalse(rec.boot_ready(199, 200))
        self.assertFalse(rec.boot_ready(500, 200, window_present=False))

    def test_ready_when_lines_reach_n(self):
        c = Clock()
        counts = iter([0, 59, 150, 200])
        r = rec.wait_for_boot(lambda: next(counts), lambda: True, 200, 60,
                              sleep=c.sleep, clock=c.now)
        self.assertEqual(r, 'ready')

    def test_times_out_below_n(self):
        c = Clock()
        r = rec.wait_for_boot(lambda: 59, lambda: True, 200, 30, sleep=c.sleep, clock=c.now)
        self.assertEqual(r, 'timeout')
        self.assertGreaterEqual(c.t, 30)

    def test_times_out_without_window(self):
        c = Clock()
        r = rec.wait_for_boot(lambda: 900, lambda: False, 200, 10, sleep=c.sleep, clock=c.now)
        self.assertEqual(r, 'timeout')

    def test_stop_file_aborts(self):
        with tempfile.TemporaryDirectory() as d:
            stop = os.path.join(d, 'stop')
            open(stop, 'w').close()
            c = Clock()
            r = rec.wait_for_boot(lambda: 900, lambda: True, 200, 600, stop_file=stop,
                                  sleep=c.sleep, clock=c.now)
            self.assertEqual(r, 'stop')

    def test_count_lines(self):
        with tempfile.TemporaryDirectory() as d:
            f = os.path.join(d, 'log')
            self.assertEqual(rec.count_lines(f), 0)
            with open(f, 'wb') as fh:
                fh.write(b'a\nb\nc\n')
            self.assertEqual(rec.count_lines(f), 3)


if __name__ == '__main__':
    unittest.main()
