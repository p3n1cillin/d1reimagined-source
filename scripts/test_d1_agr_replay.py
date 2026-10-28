"""Portable failure controls for the pad replay; no game input or guest access."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT/'tools/ps4_setup'/f'{name}.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


replay = load('d1-agr-replay')
steps = load('d1-agr-steps')


class ReplayGates(unittest.TestCase):
    def test_boot_can_complete_after_old_seven_minute_deadline(self):
        now = [0]
        def sleep(seconds):
            now[0] += seconds
        with patch.object(replay, 'contains', side_effect=lambda *_: now[0] >= 450):
            self.assertTrue(replay.wait_for(Path('boot'), 'gameplay', 900,
                                           clock=lambda: now[0], sleep=sleep))

    def test_wait_timeout_does_not_report_success(self):
        now = [0]
        with patch.object(replay, 'contains', return_value=False):
            self.assertFalse(replay.wait_for(Path('boot'), 'gameplay', 3,
                clock=lambda: now[0], sleep=lambda seconds: now.__setitem__(0, now[0] + seconds)))

    def test_boot_milestone_is_found_in_either_launcher_log_or_redirected_boot_log(self):
        with tempfile.TemporaryDirectory() as directory:
            run = Path(directory)
            logs = (run/'launcher.log', run/'boot.log')
            (run/'launcher.log').write_text('launch\ncutscenes skipped\nin gameplay (112)\n')
            self.assertTrue(replay.wait_for(logs, 'in gameplay (112)', 0, sleep=lambda _: None))
            (run/'launcher.log').unlink()
            (run/'boot.log').write_text('in gameplay (112)\n')
            self.assertTrue(replay.wait_for(logs, 'in gameplay (112)', 0, sleep=lambda _: None))
            (run/'boot.log').write_text('cutscenes skipped\n')
            self.assertFalse(replay.wait_for(logs, 'in gameplay (112)', 0, sleep=lambda _: None))

    def test_waiting_for_rifle_trigger_is_not_the_trigger(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'host.log'
            path.write_text('mission step 14 waiting for player trigger 0x992bb471/0x1d/35\n')
            self.assertFalse(replay.contains(path, replay.RIFLE_EVENT))
            with path.open('a') as stream:
                stream.write('x'*65530 + replay.RIFLE_EVENT + '\n')
            self.assertTrue(replay.contains(path, replay.RIFLE_EVENT))

    def test_old_acceptance_cannot_satisfy_new_publish(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'host.log'
            path.write_text('control: sensor clients garage.txt (123 bytes)\n')
            offset = path.stat().st_size
            self.assertFalse(replay.contains(path, 'control: sensor clients garage.txt (', offset))
            with path.open('a') as stream:
                stream.write('control: sensor clients garage.txt (123 bytes)\n')
            self.assertTrue(replay.contains(path, 'control: sensor clients garage.txt (', offset))

    def test_short_first_approach_retries_then_accepts_actual_trigger(self):
        player = replay.Replay(Path('run'), Path('host'), Path('garage'), Path('pad'))
        with patch.object(player, 'walk', return_value=False) as walk, \
             patch.object(replay, 'contains', return_value=False), \
             patch.object(replay, 'wait_for', side_effect=[False, True]):
            player.rifle()
        self.assertEqual(walk.call_count, 2)
        self.assertEqual(walk.call_args.args[3], 1.0)

    def test_completed_route_without_pickup_is_rejected(self):
        player = replay.Replay(Path('run'), Path('host'), Path('garage'), Path('pad'))
        with patch.object(player, 'walk', return_value=True) as walk, \
             patch.object(replay, 'contains', return_value=False), \
             patch.object(replay, 'wait_for', return_value=False):
            with self.assertRaisesRegex(RuntimeError, 'Rifle trigger 35 not observed'):
                player.rifle()
        self.assertEqual(walk.call_count, 2)

    def test_boot_failure_sends_no_scan_or_movement(self):
        player = replay.Replay(Path('run'), Path('host'), Path('garage'), Path('pad'))
        with patch.object(player, 'command') as command, patch.object(replay, 'wait_for', return_value=False):
            with self.assertRaisesRegex(RuntimeError, 'no route input sent'):
                player.play(900, False)
        command.assert_not_called()

    def test_unstable_first_spawn_scan_can_recover_without_starting_route(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            player = replay.Replay(root, root/'host', root/'garage', root/'pad')
            def scan(name, *_):
                player.tracked.write_text('[]' if name.endswith('-0') else str(list(range(10))))
            with patch.object(player, 'command', side_effect=scan) as command, \
                 patch.object(replay.time, 'sleep') as sleep:
                player.scan_spawn()
            self.assertEqual(command.call_count, 2)
            self.assertEqual(sleep.call_args_list, [unittest.mock.call(20), unittest.mock.call(20)])

    def test_two_empty_spawn_scans_refuse_route_input(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            player = replay.Replay(root, root/'host', root/'garage', root/'pad')
            player.tracked.write_text('[]')
            with patch.object(player, 'command') as command, patch.object(replay.time, 'sleep'):
                with self.assertRaisesRegex(RuntimeError, 'no route input sent'):
                    player.scan_spawn()
            self.assertEqual(command.call_count, 2)

    ROUTE = [[0, 0, 0], [10, 0, 0], [20, 0, 0], [30, 0, 0]]

    def route_player(self, root, outcomes):
        (root/'route.json').write_text(str(self.ROUTE))
        player = replay.Replay(root, root/'host', root/'garage', root/'pad')
        return player, patch.object(player, 'command', side_effect=[[o] for o in outcomes])

    def test_stalled_leg_resumes_once_from_nearest_waypoint(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            player, command = self.route_player(root, ['stopped (19.0, 1.0, 0.0)', 'done (30, 0, 0)'])
            with command as walked:
                self.assertTrue(player.walk('leg', root/'route.json', 60))
            resumed = walked.call_args_list[1].args
            self.assertEqual(resumed[0], 'leg-resume')
            route = resumed[1][resumed[1].index('--route') + 1]
            self.assertEqual(replay.json.loads(Path(route).read_text()), self.ROUTE[2:])

    def test_second_stall_still_stops_input(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            player, command = self.route_player(root, ['stopped (19, 1, 0)', 'stopped (21, 0, 0)'])
            with command as walked, self.assertRaisesRegex(RuntimeError, 'no subsequent input sent'):
                player.walk('leg', root/'route.json', 60)
            self.assertEqual(walked.call_count, 2)

    def test_unparsed_walker_output_does_not_resume(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            player, command = self.route_player(root, ['Traceback (most recent call last):'])
            with command as walked, self.assertRaisesRegex(RuntimeError, 'no subsequent input sent'):
                player.walk('leg', root/'route.json', 60)
            self.assertEqual(walked.call_count, 1)


class SchedulePromotion(unittest.TestCase):
    def test_preserves_prior_timing_and_incident_payload(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, divide, output = root/'steps.txt', root/'candidate.txt', root/'stable'
            divide.write_text('@authority 12,2,0\n# synthetic control\n')
            prior = '164 +10 0x4786c0e0 prior.txt incident=1851:1234\n'
            source.write_text(prior + f'164 %96|900 0x4786c0e0 {divide.as_posix()}\n')
            steps.promote(source, divide, output)
            result = (output/'agr-steps-divide.txt').read_text()
            self.assertTrue(result.startswith(prior))
            self.assertIn('%96|900', result)
            self.assertEqual((output/'agr-divide-objective17.txt').read_text(), divide.read_text())
            steps.promote(source, divide, output)  # identical promotion is idempotent

    def test_refuses_nonfinal_crossing_without_writing(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, divide, output = root/'steps.txt', root/'candidate.txt', root/'stable'
            divide.write_text('synthetic')
            source.write_text(f'164 %96|900 0x4786c0e0 {divide.as_posix()}\n164 +4 other later.txt\n')
            with self.assertRaisesRegex(ValueError, 'final slice-96'):
                steps.promote(source, divide, output)
            self.assertFalse(output.exists())

    def test_refuses_to_replace_existing_private_schedule(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, divide, output = root/'steps.txt', root/'candidate.txt', root/'stable'
            divide.write_text('synthetic')
            source.write_text(f'164 %96|900 0x4786c0e0 {divide.as_posix()}\n')
            output.mkdir()
            (output/'agr-steps-divide.txt').write_text('older evidence')
            with self.assertRaisesRegex(ValueError, 'Refusing to replace'):
                steps.promote(source, divide, output)
            self.assertFalse((output/'agr-divide-objective17.txt').exists())


if __name__ == '__main__':
    unittest.main()
