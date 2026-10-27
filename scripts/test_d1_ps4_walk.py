"""Run the Windows walker's route loop with a synthetic position and no OS input."""
import argparse
import ast
import contextlib
import io
import json
import math
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch


class RouteProgress(unittest.TestCase):
    def test_tolerant_arrival_off_next_segment_still_sends_movement(self):
        source = Path(__file__).resolve().parents[1] / 'tools/ps4_setup/d1-walk.py'
        # Extract the actual main, avoiding Windows process access at module import.
        node = next(n for n in ast.parse(source.read_text()).body if isinstance(n, ast.FunctionDef) and n.name == 'main')
        calls = []

        class Clock:
            now = 0
            def time(self):
                self.now += .01
                return self.now
            def sleep(self, _):
                pass

        scope = dict(argparse=argparse, json=json, math=math, time=Clock(), Client=object,
                     active=lambda **_: None, position=lambda *args: (3.5, 0, 0),
                     wrap=lambda x: (x + math.pi) % (2 * math.pi) - math.pi,
                     pad=lambda *keys: calls.append(keys))
        exec(compile(ast.Module(body=[node], type_ignores=[]), str(source), 'exec'), scope)
        with tempfile.TemporaryDirectory() as directory:
            route, tracked = Path(directory)/'route.json', Path(directory)/'tracked.json'
            route.write_text('[[0,0,0],[0,10,0],[0,20,0]]')
            tracked.write_text('[]')
            args = ['walk', '--route', str(route), '--tracked', str(tracked), '--tolerance', '4',
                    '--max-seconds', '.2', '--turn-sign', '1', '--turn-rate', '2.26']
            with patch.object(sys, 'argv', args), contextlib.redirect_stdout(io.StringIO()):
                scope['main']()
        self.assertTrue(calls, 'the old arrival/retreat loop emits no movement at this position')


if __name__ == '__main__':
    unittest.main()
