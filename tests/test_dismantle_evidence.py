# SPDX-License-Identifier: GPL-2.0-or-later
"""Synthetic transaction windows; no live records or universal reward amounts are embedded."""
import importlib.util
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import unittest
from unittest.mock import patch

REPO = Path(__file__).resolve().parents[1]


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, REPO / 'tools/ps4_setup' / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


CLI = load('srcp_cli_yields', 'd1-srcp.py')
ITEMS = load('item_actions_yields', 'd1-item-actions.py')


def request(t=1, seq=7, service=402):
    return (t, 'req', seq, None, service, b'')


def reply(t=2, seq=7, status=200):
    return (t, 'rsp', seq, status, 402, b'')


class ActionEvidence(unittest.TestCase):
    def report(self, extra=(), result=None, fields=None, stacks=None):
        req, rsp = request(), reply()
        fields = fields if fields is not None else {'f0.f0': 1, 'f0.f1': '#..abcd', 'f0.f2': 42, 'f1': 1}
        result = result if result is not None else {'f0.f0': 0, 'f0.f1': 0}
        stacks = stacks if stacks is not None else [(1.5, 'account', 3, [(9, 66, 15, 11, False)], '')]
        return CLI.correlate_item_actions([req, *extra, rsp], [(req, fields), (rsp, result)], stacks)[0]

    def test_matched_success_reports_observation_and_omits_identifiers(self):
        row = self.report()
        self.assertTrue(row['accepted'])
        self.assertEqual(row['candidates'][0]['delta'], 4)
        self.assertNotIn('abcd', json.dumps(row))
        self.assertNotIn('sequence', row)
        self.assertNotIn('yields', row)

    def test_missing_baseline_and_content_are_kept_unknown(self):
        row = self.report(stacks=[(1.5, 'profile', 3, [(9, None, 15, None, False)], 'partial')])
        self.assertIsNone(row['candidates'][0]['content'])
        self.assertIsNone(row['candidates'][0]['delta'])
        self.assertIn('inventory decode: partial', row['issues'])

    def test_failed_result_does_not_attribute_rewards(self):
        row = self.report(result={'f0.f0': 1, 'f0.f1': 5})
        self.assertFalse(row['accepted'])
        self.assertEqual(row['candidates'], [])

    def test_concurrent_requests_and_stack_actions_are_explicitly_ambiguous(self):
        row = self.report(extra=[request(1.2, 8, 901)], fields={'f0.f0': 0, 'f0.f2': 42, 'f1': 4})
        self.assertTrue(any('overlaps' in s for s in row['issues']))
        self.assertTrue(any('not necessarily a dismantle' in s for s in row['issues']))
        self.assertFalse(self.report(extra=[request(1.2, 8, 702)])['issues'])

    def test_missing_duplicate_or_wrong_sequence_never_matches(self):
        req = request()
        fields = {'f0.f0': 1, 'f0.f2': 42, 'f1': 1}
        for replies in ([], [(reply(seq=8), {})], [(reply(), {}), (reply(), {})]):
            row = CLI.correlate_item_actions([], [(req, fields), *replies], [])[0]
            self.assertFalse(row['accepted'])
            self.assertTrue(row['issues'])

    def test_undecodable_request_does_not_create_an_observation(self):
        row = CLI.correlate_item_actions([], [(request(), None)], [])[0]
        self.assertIsNone(row['content'])
        self.assertFalse(row['accepted'])

    def test_a_partial_or_overrunning_codec_walk_is_not_accepted(self):
        req = (*request()[:5], b'\0' * 16)
        rsp = (*reply()[:5], b'\0' * 16)
        for malformed in ({'object': '00' * 32, 'end': 129},
                          {'partial': '00' * 32, 'end': 20, 'error': 'short input'}):
            with patch.object(CLI.d1_srcp, 'investment_messages', return_value=[req, rsp]), \
                 patch.object(CLI, 'stack_rows', return_value=[]):
                rows = CLI.dismantle_rows(b'', decode=lambda todo: [malformed, malformed])
            self.assertEqual(len(rows), 1)
            self.assertIsNone(rows[0]['content'])
            self.assertFalse(rows[0]['accepted'])

    def test_absent_table_is_not_a_zero_reward_table(self):
        b = bytearray(0x400)
        struct.pack_into('<Q', b, 0, len(b))
        struct.pack_into('<I', b, 0x3E8, 0xFFFFFFFF)
        self.assertEqual(ITEMS.reward_table_status(b)['status'], 'absent')
        self.assertIsNone(ITEMS.reward_table_status(b)['reward_table_tag'])
        struct.pack_into('<I', b, 0x3E8, 123)
        self.assertEqual(ITEMS.reward_table_status(b)['reward_table_tag'], 123)
        with self.assertRaises(ValueError):
            ITEMS.reward_table_status(b[:20])

    def test_owned_table_reference_is_checked_when_inputs_are_present(self):
        if not os.environ.get('D1_EBOOT') or not os.environ.get('D1_REPO'):
            self.skipTest('set D1_EBOOT and D1_REPO for the owned 56578 table-reference check')
        r = subprocess.run([sys.executable, '-I', str(REPO / 'tools/ps4_setup/d1-item-actions.py'),
                            '--reward-table-status'], check=True, capture_output=True, text=True)
        result = json.loads(r.stdout)
        self.assertEqual(result['status'], 'absent')
        self.assertIsNone(result['reward_table_tag'])
        sources = json.loads((REPO / 'ps4/fixtures/56578/characters/starting-kit-lists.json').read_text())['sources']
        expected = next(s['sha256'] for s in sources if s['tag'] == result['root_tag'])
        self.assertEqual(result['root_sha256'], expected)


if __name__ == '__main__':
    unittest.main()
