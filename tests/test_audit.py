# SPDX-License-Identifier: GPL-2.0-or-later
"""Offline controls for the audit; every frame and value below is synthetic."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('audit', Path(__file__).parents[1] / 'tools/ps4_setup/d1-audit.py')
A = importlib.util.module_from_spec(spec)
spec.loader.exec_module(A)


def entry(kind=1, index=0, body='c0', group=7, bits=8):
    return dict(hash=str(group), i8=kind, sub=index, body=body, body_bits=bits)


def stream(*entries):
    return [(dict(frame=i, stamp=i), dict(entries=e)) for i, e in enumerate(entries)]


class Audit(unittest.TestCase):
    def history(self, events, values=None, tags=None):
        return A.histories(events, tags or {1: 99}, {99: values or {}})[0]

    def test_union_reports_rows_and_values_separately(self):
        a = self.history(stream([entry(index=0), entry(index=1, body='80'), entry(index=2)]),
                         {'c0': {'image': '0000'}})
        b = self.history(stream([entry(index=0, body='80'), entry(index=1), entry(index=3)]),
                         {'c0': {'image': '0000'}})
        rows = A.compare_rows(a, b)
        self.assertEqual([r['row_presence'] for r in rows], ['both', 'both', 'ours_only', 'live_only'])
        self.assertEqual([r['value_presence'] for r in rows], ['ours_only', 'live_only', 'ours_only', 'live_only'])
        # Explicit all-zero bytes are a value. A default-only row is not one.
        self.assertEqual(rows[0]['ours']['changes'][0]['image'], '0000')
        self.assertIsNone(rows[0]['first_explicit'])
        self.assertFalse(rows[0]['sequence_equal'])

    def test_change_order_preserves_reversion_and_all_events(self):
        rows = self.history(stream([entry()], [entry()], [entry(body='80')],
                                   [entry(body='d0')], [entry()]),
                            {'c0': {'image': 'AB00'}, 'd0': {'image': '0100'}})
        row = rows[(7, 1, 0)]
        self.assertEqual([c['image'] for c in row['changes']], ['ab00', '0100', 'ab00'])
        self.assertEqual([c['frame'] for c in row['changes']], [0, 3, 4])
        self.assertEqual(len(row['events']), 5)
        self.assertEqual(row['events'][2]['status'], 'no_explicit_value')

    def test_missing_tags_and_refusals_cannot_look_like_absent_values(self):
        rows, refused = A.histories([(dict(frame=0), None), *stream([
            entry(kind=88), entry(body='c0'), entry(index=1, body='80')])], {1: 99}, {99: {'c0': None}})
        self.assertEqual(refused, [dict(frame=0)])
        self.assertEqual(rows[(7, 88, 0)]['events'][0]['status'], 'missing_tag')
        self.assertEqual(rows[(7, 1, 0)]['events'][0]['status'], 'decode_refused')
        self.assertEqual(rows[(7, 1, 1)]['events'][0]['status'], 'no_explicit_value')
        totals = A.totals(A.compare_rows(rows, {}))
        self.assertEqual(totals[0]['ours_decode_refused'], 1)
        self.assertIsNone(A.compare_rows(rows, {})[0]['sequence_equal'])
        self.assertFalse(A.coverage(rows, refused)['decoded_value_coverage_complete'])

    def test_decoder_cannot_read_past_the_row_budget_and_report_a_match(self):
        rows, _ = A.histories(stream([entry(bits=9)]), {1: 99},
                              {99: {'c0': dict(image='00', bits=8)}})
        row = rows[(7, 1, 0)]
        self.assertEqual(row['changes'], [])
        self.assertEqual(row['events'][0]['status'], 'decode_overrun')
        self.assertEqual(row['events'][0]['candidate_image'], '00')
        self.assertIsNone(A.compare_rows(rows, rows)[0]['sequence_equal'])
        self.assertFalse(A.coverage(rows, [])['decoded_value_coverage_complete'])

    def test_coverage_does_not_claim_success_for_empty_or_refused_frames(self):
        self.assertFalse(A.coverage({}, [])['decoded_value_coverage_complete'])
        rows = self.history(stream([entry()]), {'c0': {'image': '00'}})
        self.assertTrue(A.coverage(rows, [])['decoded_value_coverage_complete'])
        self.assertFalse(A.coverage(rows, [dict(frame=0)])['decoded_value_coverage_complete'])

    def test_every_ordinal_diff_and_landmark_including_unequal_lengths(self):
        a = self.history(stream([entry()], [entry(body='d0')], [entry(body='e0')]),
                         {'c0': {'image': '0102'}, 'd0': {'image': '0304'}, 'e0': {'image': '0506'}})
        b = self.history(stream([entry()], [entry(body='d0')]),
                         {'c0': {'image': '010203'}, 'd0': {'image': '0305'}})
        row = A.compare_rows(a, b)[0]
        self.assertEqual(row['first_explicit'], [2])
        self.assertEqual(row['first_changed'], [1])
        self.assertEqual(row['last_observed'], [0, 1])
        self.assertEqual(row['ordinal_differences'], [
            dict(ordinal=0, ours_present=True, live_present=True, offsets=[2]),
            dict(ordinal=1, ours_present=True, live_present=True, offsets=[1]),
            dict(ordinal=2, ours_present=True, live_present=False, offsets=None)])
        self.assertEqual(A.byte_diff('', '00'), [0])
        self.assertEqual(A.byte_diff('00', '00'), [])

    def test_equal_endpoints_do_not_hide_missing_intermediate_value(self):
        a = self.history(stream([entry()], [entry(body='d0')]),
                         {'c0': {'image': '00'}, 'd0': {'image': '02'}})
        b = self.history(stream([entry()], [entry(body='e0')], [entry(body='d0')]),
                         {'c0': {'image': '00'}, 'd0': {'image': '02'}, 'e0': {'image': '01'}})
        row = A.compare_rows(a, b)[0]
        self.assertEqual(row['last_observed'], [])
        self.assertFalse(row['sequence_equal'])
        self.assertEqual(len(row['ordinal_differences']), 2)

    def test_decode_batches_deduplicate_by_tag_and_filter_groups(self):
        inputs = [stream([entry(), entry(kind=2), entry(kind=3, group=8)]), stream([entry()])]
        calls = []
        def decode(tag, bodies):
            calls.append((tag, bodies))
            return {'c0': {'image': '00'}}
        decoded = A.decode_values(inputs, {1: 99, 2: 99, 3: 98}, {7}, decode)
        self.assertEqual(calls, [(99, ['c0'])])
        self.assertEqual(decoded, {99: {'c0': {'image': '00'}}})

    def test_group_filter_keeps_unrecognised_kinds_in_selected_area(self):
        rows, _ = A.histories(stream([entry(group=8), entry(kind=88)]), {}, {}, {7})
        self.assertEqual(set(rows), {(7, 88, 0)})

    def test_live_hosts_stay_separate_and_only_server_sensor_messages_decode(self):
        messages = [(1, 'c2s', 'private', 10, 5, b'client'),
                    (2, 's2c', 'private', 10, 18, b'incident'),
                    (3, 's2c', 'private', 10, 5, b'a'),
                    (4, 's2c', 'private', 11, 5, b'b')]
        with patch.object(A.srcp.d1_srcp, 'host_messages', return_value=messages), \
                patch.object(A.srcp, 'sensor_frames', side_effect=lambda bodies: {
                    b: dict(stamp=i, entries=[]) for i, b in enumerate(bodies)}) as decoder:
            streams = A.live_streams(b'fake')
            self.assertEqual(list(streams), ['0x000000000000000a', '0x000000000000000b'])
            self.assertEqual(streams['0x000000000000000a'][0][0]['seconds'], 3)
            filtered = A.live_streams(b'fake', host='b', start=4, end=5)
            self.assertEqual(list(filtered), ['0x000000000000000b'])
            self.assertEqual(decoder.call_count, 2)
            with self.assertRaisesRegex(ValueError, 'no live'):
                A.live_streams(b'fake', end=0)

    def test_local_input_reuses_loader_order_and_preserves_refusal(self):
        rows = [(dict(source='synthetic', full=True), dict(stamp=3, entries=[])), ({}, None)]
        with patch.object(A.frames_cli, 'load', return_value=rows) as loader:
            stream = A.local_stream('synthetic.jsonl')
        loader.assert_called_once_with('synthetic.jsonl')
        self.assertEqual(stream[0][0], dict(frame=0, stamp=3, source='synthetic', full=True))
        self.assertIsNone(stream[1][1])

    def test_tags_follow_canonical_constants_and_leave_missing_mapping_open(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            base = root / 'ps4/d1-server/crates'
            for crate in ('d1-oracle', 'd1-activity-host'):
                (base / crate / 'src').mkdir(parents=True)
            (base / 'd1-oracle/src/sensor.rs').write_text('pub const V: u32 = 0x1234;')
            (base / 'd1-oracle/src/sensor_objective.rs').write_text('')
            (base / 'd1-activity-host/src/row_encoders.rs').write_text('''
pub const KIND_ONE: u8 = 1;
pub const TAG_ONE: u32 = 0x12_35;
pub const KIND_TWO: u8 = 2;
pub const TAG_TWO: u32 = d1_oracle::sensor::V;
pub const KIND_UNKNOWN: u8 = 3;
pub const TAG_UNKNOWN: u32 = expression();
''')
            self.assertEqual(A.kind_tags(root), {1: 0x1235, 2: 0x1234})

    def test_private_report_guard(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / 'repo'
            root.mkdir()
            for path in (root / 'private.json', root / 'sub/../private.json'):
                with self.assertRaisesRegex(ValueError, 'outside'):
                    A.private_output(path, root)
            self.assertEqual(A.private_output(Path(temp) / 'private.json', root),
                             Path(temp).resolve() / 'private.json')

    def test_cli_writes_private_report_with_explicit_tag_override_and_separate_runs(self):
        events = stream([entry(kind=3)])
        with tempfile.TemporaryDirectory() as temp:
            out = Path(temp) / 'audit.json'
            argv = ['d1-audit.py', '--ours', 'one.jsonl', 'two.jsonl', '--live', 'live.pcap',
                    '--out', str(out), '--tag', '3=99']
            with patch.object(A.sys, 'argv', argv), patch.object(A, 'kind_tags', return_value={}), \
                    patch.object(A, 'low_priority'), patch.object(A, 'local_stream', return_value=events), \
                    patch.object(Path, 'read_bytes', return_value=b'fake'), \
                    patch.object(A, 'live_streams', return_value={'synthetic': events}), \
                    patch.object(A.srcp, 'sensor_values', return_value={'c0': dict(image='00', bits=6)}):
                A.main()
            report = json.loads(out.read_text())
            self.assertEqual(report['missing_tags'], [])
            self.assertEqual(report['tag_overrides'], {'3': '0x00000063'})
            self.assertEqual(len(report['comparisons']), 2)
            for comparison in report['comparisons']:
                self.assertTrue(comparison['coverage_ours']['decoded_value_coverage_complete'])
                self.assertTrue(comparison['rows'][0]['sequence_equal'])

    def test_cli_rejects_input_output_collision_before_decoding(self):
        with tempfile.TemporaryDirectory() as temp:
            path = str(Path(temp) / 'input.jsonl')
            argv = ['d1-audit.py', '--ours', path, '--live', 'live.pcap', '--out', path]
            with patch.object(A.sys, 'argv', argv), patch.object(A, 'local_stream') as loader, \
                    patch.object(A.sys, 'stderr'):
                with self.assertRaises(SystemExit) as error:
                    A.main()
            self.assertEqual(error.exception.code, 2)
            loader.assert_not_called()


if __name__ == '__main__':
    unittest.main()
