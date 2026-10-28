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


SOURCE = Path(__file__).resolve().parents[1] / 'tools/ps4_setup/d1-walk.py'


def extract(*names):
    """The named top-level functions of d1-walk.py, without opening the client at import."""
    tree = ast.parse(SOURCE.read_text())
    nodes = [n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name in names]
    scope = dict(math=math)
    exec(compile(ast.Module(body=nodes, type_ignores=[]), str(SOURCE), 'exec'), scope)
    return scope


class MemoryFacing(unittest.TestCase):
    def setUp(self):
        self.facing_from = extract('wrap', 'facing_from')['facing_from']

    def test_majority_heading_ignores_an_outlier_and_non_unit_values(self):
        h = 0.5
        good = [(math.cos(h), math.sin(h))] * 4
        got = self.facing_from(good + [(math.cos(2.5), math.sin(2.5)), (3.0, 4.0), (float('nan'), 0)])
        self.assertAlmostEqual(got, h, places=6)

    def test_too_few_unit_vectors_gives_no_heading(self):
        self.assertIsNone(self.facing_from([(1.0, 0.0), (1.0, 0.0), (5.0, 1.0)]))

    def test_disagreeing_vectors_give_no_heading(self):
        spread = [(math.cos(a), math.sin(a)) for a in (0, 0, 2, 2, -2, -2)]
        self.assertIsNone(self.facing_from(spread))

    def test_walks_straight_when_memory_facing_points_at_the_waypoint(self):
        node = next(n for n in ast.parse(SOURCE.read_text()).body if isinstance(n, ast.FunctionDef) and n.name == 'main')
        calls = []

        class Clock:
            now = 0
            def time(self):
                self.now += .01
                return self.now
            def sleep(self, _):
                pass

        scope = dict(argparse=argparse, json=json, math=math, time=Clock(), Client=object,
                     active=lambda **_: None, position=lambda *args: (0, 0, 0),
                     wrap=lambda x: (x + math.pi) % (2 * math.pi) - math.pi,
                     facing=lambda *_: math.pi / 2, pad=lambda *keys: calls.append(keys))
        exec(compile(ast.Module(body=[node], type_ignores=[]), str(SOURCE), 'exec'), scope)
        with tempfile.TemporaryDirectory() as directory:
            route, tracked = Path(directory)/'route.json', Path(directory)/'tracked.json'
            route.write_text('[[0,10,0]]')
            tracked.write_text('[]')
            args = ['walk', '--route', str(route), '--tracked', str(tracked), '--max-seconds', '.05',
                    '--turn-sign', '1', '--turn-rate', '2.26', '--memory-facing']
            with patch.object(sys, 'argv', args), contextlib.redirect_stdout(io.StringIO()):
                scope['main']()
        self.assertTrue(calls)
        self.assertTrue(calls[0][0].startswith('Hold:W:'), calls[0])
        self.assertFalse(any(k.startswith(('Hold:J:', 'Hold:L:')) for c in calls for k in c))


class RetrackFallback(unittest.TestCase):
    """retrack_or_rescan: a thin retrack falls back to scans around known respawns."""

    def setUp(self):
        scope = extract('retrack_or_rescan')
        scope['position'] = lambda c, tracked: (0.0, 0.0, 0.0)
        self.fn = scope['retrack_or_rescan']

    def test_a_good_retrack_is_kept_without_scanning(self):
        scans = []
        got = self.fn(None, [1], [[319, -664, 7]], 25, 30, retrack_fn=lambda *_: list(range(200)),
                      scan_fn=lambda c, centre, radius: scans.append(centre) or [])
        self.assertEqual(len(got), 200)
        self.assertEqual(scans, [])

    def test_a_thin_retrack_scans_respawns_in_order_until_one_is_good(self):
        scans = []
        def scan(c, centre, radius):
            scans.append(centre)
            return list(range(362)) if centre[0] == 319 else list(range(5))
        got = self.fn(None, [1], [[189, -606, 5], [319, -664, 7], [277, -625, 6]], 25, 30,
                      retrack_fn=lambda *_: list(range(12)), scan_fn=scan)
        self.assertEqual(len(got), 362)
        self.assertEqual([c[0] for c in scans], [189, 319])

    def test_nothing_better_keeps_the_retrack(self):
        got = self.fn(None, [1], [[189, -606, 5]], 25, 30, retrack_fn=lambda *_: [7, 8, 9],
                      scan_fn=lambda *_: [])
        self.assertEqual(got, [7, 8, 9])


if __name__ == '__main__':
    unittest.main()
