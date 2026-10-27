"""Portable tests for the explorer's fatal-move guard; no game input or Windows APIs."""
import importlib.util
import itertools
import json
import math
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch


TOOL = Path(__file__).resolve().parents[1] / 'tools' / 'ps4_setup' / 'd1-explore.py'
spec = importlib.util.spec_from_file_location('d1_explore_policy', TOOL)
explore = importlib.util.module_from_spec(spec)
spec.loader.exec_module(explore)


class ExplorerLethalMoves(unittest.TestCase):
    def test_dead_end_retry_keeps_lethal_moves_and_reopens_wall_moves(self):
        cell = (12, -4)
        lethal = {(cell, 3), (cell, 7)}
        blocked = {(cell, k): 2 for k in range(explore.DIRS)}
        allowed = explore.reset_transient_blocks(cell, blocked, lethal)
        self.assertEqual(set(allowed), set(range(explore.DIRS)) - {3, 7})
        self.assertEqual(blocked, {(cell, 3): 99, (cell, 7): 99})

    def test_fatal_move_stays_excluded_even_if_its_transient_entry_is_missing(self):
        cell = (0, 100)
        self.assertNotIn(5, explore.available_directions(cell, {}, {(cell, 5)}))

    def test_recovery_does_not_erase_another_cells_evidence(self):
        cell, other = (3, 4), (4, 4)
        blocked = {(cell, 2): 2, (other, 2): 2, (other, 8): 99}
        explore.reset_transient_blocks(cell, blocked, {(other, 8)})
        self.assertEqual(blocked, {(other, 2): 2, (other, 8): 99})

    def test_all_fatal_directions_have_no_recovery_move(self):
        cell = (3, 4)
        lethal = {(cell, k) for k in range(explore.DIRS)}
        blocked = {}
        self.assertEqual(explore.reset_transient_blocks(cell, blocked, lethal), [])
        self.assertEqual(set(blocked), lethal)

    def test_a_single_remaining_safe_direction_can_be_retried(self):
        cell = (3, 4)
        lethal = {(cell, k) for k in range(explore.DIRS) if k != 9}
        blocked = {(cell, 9): 2}
        self.assertEqual(explore.reset_transient_blocks(cell, blocked, lethal), [9])
        self.assertEqual(explore.available_directions(cell, blocked, lethal), [9])

    def test_main_stops_before_keepalive_or_heading_probe_in_an_all_fatal_cell(self):
        here = (100.0, -100.0, 10.0)
        cell = explore.cell_of(here)
        driver = SimpleNamespace(Client=Mock(return_value=object()),
                                 position=Mock(return_value=here), pad=Mock(), active=Mock())
        with tempfile.TemporaryDirectory() as td:
            td = Path(td)
            tracked, lethal, output = td / 'tracked.json', td / 'lethal.json', td / 'out.json'
            tracked.write_text('[]')
            lethal.write_text(json.dumps([[*cell, k] for k in range(explore.DIRS)]))
            args = ['d1-explore.py', '--tracked', str(tracked), '--lethal', str(lethal),
                    '--goal', '200,-100', '--out', str(output)]
            with patch.object(explore, 'load_walk', return_value=driver), patch('sys.argv', args):
                explore.main()
            driver.pad.assert_not_called()
            driver.active.assert_not_called()
            self.assertEqual(json.loads(output.read_text()), [list(here)])

    def test_help_does_not_load_the_windows_driver(self):
        with patch.object(explore, 'load_walk') as load, patch('sys.argv', ['d1-explore.py', '--help']):
            with self.assertRaises(SystemExit) as result:
                explore.main()
        self.assertEqual(result.exception.code, 0)
        load.assert_not_called()

    def test_main_dead_end_uses_filtered_recovery_without_a_blind_back_jump(self):
        class RecoveryObserved(Exception):
            pass

        start, stopped = (100.0, -100.0, 10.0), (100.0, -98.0, 10.0)
        cell = explore.cell_of(stopped)
        samples = itertools.chain([start], itertools.repeat(stopped))
        driver = SimpleNamespace(Client=Mock(return_value=object()),
            position=Mock(side_effect=lambda *args: next(samples)), pad=Mock(), active=Mock(),
            retrack=Mock(return_value=[]), plausible=lambda point: True,
            wrap=lambda angle: (angle + math.pi) % (2 * math.pi) - math.pi)
        reset = explore.reset_transient_blocks

        def observe_recovery(where, blocked, lethal):
            self.assertEqual(where, cell)
            self.assertEqual(blocked[(cell, 0)], 99)
            self.assertNotIn(0, reset(where, blocked, lethal))
            raise RecoveryObserved()

        with tempfile.TemporaryDirectory() as td:
            td = Path(td)
            tracked, lethal, output = td / 'tracked.json', td / 'lethal.json', td / 'out.json'
            tracked.write_text('[]')
            lethal.write_text(json.dumps([[*cell, 0]]))
            args = ['d1-explore.py', '--tracked', str(tracked), '--lethal', str(lethal),
                    '--goal', '100,-50', '--out', str(output), '--max-seconds', '200']
            with patch.object(explore, 'load_walk', return_value=driver), patch('sys.argv', args), \
                    patch.object(explore.time, 'time', side_effect=itertools.count()), \
                    patch.object(explore, 'reset_transient_blocks', side_effect=observe_recovery):
                with self.assertRaises(RecoveryObserved):
                    explore.main()
        self.assertNotIn(unittest.mock.call('NumPad2', 'Hold:S:800'), driver.pad.call_args_list)


if __name__ == '__main__':
    unittest.main()
