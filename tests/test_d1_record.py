# SPDX-License-Identifier: GPL-2.0-or-later
"""d1-record.py pure logic: stale tap cleanup, the wait-for-boot predicate, flag removal."""
import importlib.util
import json
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

    def test_old_log_is_not_ready_until_rewritten(self):
        with tempfile.TemporaryDirectory() as d:
            f = os.path.join(d, 'log')
            with open(f, 'wb') as fh:
                fh.write(b'x\n' * 500)
            os.utime(f, (1000, 1000))
            self.assertEqual(rec.log_lines_since(f, 2000, 200), 0)
            c = Clock()
            r = rec.wait_for_boot(lambda: rec.log_lines_since(f, 2000, 200), lambda: True, 200, 5,
                                  sleep=c.sleep, clock=c.now)
            self.assertEqual(r, 'timeout')
            os.utime(f, (3000, 3000))  # the new emulator rewrote it
            self.assertGreaterEqual(rec.log_lines_since(f, 2000, 200), 200)

    def test_count_lines_stops_early_and_warns_once(self):
        with tempfile.TemporaryDirectory() as d:
            f = os.path.join(d, 'log')
            with open(f, 'wb') as fh:
                fh.write(b'x\n' * (3 << 20))
            self.assertGreaterEqual(rec.count_lines(f, 10), 10)
            with mock.patch('builtins.open', side_effect=PermissionError(13, 'denied')),                     mock.patch('builtins.print') as pr:
                self.assertEqual(rec.count_lines('locked-log'), 0)
                self.assertEqual(rec.count_lines('locked-log'), 0)
            self.assertEqual(pr.call_count, 1)


class FakeStdin:
    def __init__(self, write_exc=None, flush_exc=None, close_exc=None):
        self.write_exc, self.flush_exc, self.close_exc = write_exc, flush_exc, close_exc
        self.written = []

    def write(self, s):
        if self.write_exc:
            raise self.write_exc
        self.written.append(s)

    def flush(self):
        if self.flush_exc:
            raise self.flush_exc

    def close(self):
        if self.close_exc:
            raise self.close_exc


class FakeProc:
    def __init__(self, stdin, running=True, hang=False):
        self.stdin, self.running, self.hang = stdin, running, hang
        self.terminated = False
        self.waits = 0

    def poll(self):
        return None if self.running else 0

    def wait(self, timeout=None):
        self.waits += 1
        if self.hang and not self.terminated:
            raise rec.subprocess.TimeoutExpired('ffmpeg', timeout)
        return 0

    def terminate(self):
        self.terminated = True


class StopFfmpeg(unittest.TestCase):
    def test_running_process_gets_q(self):
        p = FakeProc(FakeStdin())
        self.assertTrue(rec.stop_ffmpeg(p))
        self.assertEqual(p.stdin.written, ['q'])

    def test_write_oserror_22_is_tolerated(self):
        p = FakeProc(FakeStdin(write_exc=OSError(22, 'Invalid argument')))
        self.assertTrue(rec.stop_ffmpeg(p))
        self.assertEqual(p.waits, 1)

    def test_flush_oserror_is_tolerated(self):
        p = FakeProc(FakeStdin(flush_exc=OSError(22, 'Invalid argument')))
        rec.stop_ffmpeg(p)
        self.assertEqual(p.waits, 1)

    def test_broken_pipe_and_closed_file(self):
        rec.stop_ffmpeg(FakeProc(FakeStdin(write_exc=BrokenPipeError())))
        rec.stop_ffmpeg(FakeProc(FakeStdin(flush_exc=ValueError('closed'))))

    def test_close_error_is_tolerated(self):
        rec.stop_ffmpeg(FakeProc(FakeStdin(close_exc=OSError(22, 'Invalid argument'))))

    def test_already_exited_skips_write_and_reports_it(self):
        p = FakeProc(FakeStdin(write_exc=OSError(22, 'x')), running=False)
        self.assertFalse(rec.stop_ffmpeg(p))
        self.assertEqual(p.stdin.written, [])

    def test_wait_timeout_terminates_that_process(self):
        p = FakeProc(FakeStdin(), hang=True)
        rec.stop_ffmpeg(p)
        self.assertTrue(p.terminated)
        self.assertEqual(p.waits, 2)


class T0Sidecar(unittest.TestCase):
    def test_sidecar_content(self):
        with tempfile.TemporaryDirectory() as d:
            video = os.path.join(d, 'run.mp4.video.mp4')
            rec.write_t0(video, 1234.5, 7)
            with open(video + '.t0.json') as f:
                self.assertEqual(json.load(f), {'t_first': 1234.5, 'frames': 7})
            rec.write_t0(video, None, 0)
            with open(video + '.t0.json') as f:
                self.assertEqual(json.load(f), {'t_first': None, 'frames': 0})


if __name__ == '__main__':
    unittest.main()
