"""Exercise debug-register teardown without attaching to a process or requiring Windows."""
import ast
from pathlib import Path
import types
import unittest


SOURCE = Path(__file__).parents[1] / 'tools/ps4_setup/d1-watchwrite.py'
TREE = ast.parse(SOURCE.read_text(encoding='utf-8'))
FUNCTION = next(node for node in TREE.body if isinstance(node, ast.FunctionDef) and node.name == 'disarm_threads')


class CleanupTests(unittest.TestCase):
    def setup_cleanup(self, dead=(), live_failure=(), clear_failure=(), dirty=()):
        events = []

        class Kernel:
            def SuspendThread(self, handle):
                events.append(('suspend', handle))
                return 0xFFFFFFFF if handle in dead or handle in live_failure else 0

            def GetExitCodeThread(self, handle, pointer):
                pointer.value = 0 if handle in dead else 259
                return True

            def ResumeThread(self, handle):
                events.append(('resume', handle))
                return 1

        class Context:
            def get(self, handle):
                self.handle = handle
                events.append(('verify', handle))
                return True

            def u64(self, offset):
                return int(self.handle in dirty)

        def arm(handle, addresses, on):
            events.append(('clear', handle))
            self.assertFalse(on)
            return handle not in clear_failure

        namespace = dict(k32=Kernel(), Ctx=Context, arm=arm, OFF_DR7=0,
                         wt=types.SimpleNamespace(DWORD=lambda: types.SimpleNamespace(value=0)),
                         ctypes=types.SimpleNamespace(byref=lambda value: value))
        exec(compile(ast.Module(body=[FUNCTION], type_ignores=[]), str(SOURCE), 'exec'), namespace)
        return namespace['disarm_threads'], events

    def test_suspends_entire_set_before_clear_and_resumes_only_after_verification(self):
        cleanup, events = self.setup_cleanup()
        self.assertEqual(cleanup({1: 11, '_attach_bp': 1, 2: 22}, [0x1000]), 2)
        self.assertEqual(events, [('suspend', 11), ('suspend', 22), ('clear', 11),
                                  ('verify', 11), ('clear', 22), ('verify', 22),
                                  ('resume', 22), ('resume', 11)])

    def test_exited_thread_is_skipped_but_live_suspend_failure_is_an_error(self):
        cleanup, events = self.setup_cleanup(dead=(22,))
        self.assertEqual(cleanup({1: 11, 2: 22}, []), 1)
        self.assertNotIn(('clear', 22), events)
        cleanup, events = self.setup_cleanup(live_failure=(22,))
        with self.assertRaisesRegex(RuntimeError, 'Cannot suspend live thread 2'):
            cleanup({1: 11, 2: 22}, [])
        self.assertEqual(events[-1], ('resume', 11))
        self.assertNotIn(('clear', 11), events)

    def test_clear_failure_still_releases_every_suspend(self):
        cleanup, events = self.setup_cleanup(clear_failure=(11,))
        with self.assertRaisesRegex(RuntimeError, 'Failed to clear'):
            cleanup({1: 11, 2: 22}, [])
        self.assertEqual(events[-2:], [('resume', 22), ('resume', 11)])

    def test_nonzero_dr7_is_not_reported_as_a_clean_detach(self):
        cleanup, events = self.setup_cleanup(dirty=(22,))
        with self.assertRaisesRegex(RuntimeError, 'DR7 clear was not verified'):
            cleanup({1: 11, 2: 22}, [])
        self.assertEqual(events[-2:], [('resume', 22), ('resume', 11)])


if __name__ == '__main__':
    unittest.main()
