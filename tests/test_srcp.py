# SPDX-License-Identifier: GPL-2.0-or-later
"""Synthetic SRCP captures only: the reader must find the activity-host messages in them."""
import importlib.util
from pathlib import Path
import struct
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('srcp', Path(__file__).parents[1] / 'tools/ps4_setup/d1_srcp.py')
M = importlib.util.module_from_spec(spec)
spec.loader.exec_module(M)
spec = importlib.util.spec_from_file_location('srcp_cli', Path(__file__).parents[1] / 'tools/ps4_setup/d1-srcp.py')
CLI = importlib.util.module_from_spec(spec)
spec.loader.exec_module(CLI)


def record(t, direction, ip, port, payload):
    head = bytearray(M.HEADER)
    head[:8] = b'SRCP' + struct.pack('<HH', 1, M.HEADER)
    head[16] = direction
    head[24:28] = bytes(ip)
    head[28:30] = struct.pack('>H', port)
    rec = bytes(head) + payload
    return struct.pack('<IIII', int(t), int(t % 1 * 1e6), len(rec), len(rec)) + rec


def capture(*recs):
    return struct.pack('<IHHiIII', 0xa1b2c3d4, 2, 4, 0, 0, 0x40000, M.LINK_SRCP) + b''.join(recs)


class SrcpTest(unittest.TestCase):
    def test_finds_notifications_and_client_messages(self):
        push = struct.pack('>HI', M.BC_NOT, 7) + struct.pack('>BQII', 1, 2, 18, 3) + b'abc'
        sent = struct.pack('>HI', M.AH_NOT, 8) + struct.pack('>QBIII', 2, 1, 6, 2, 0) + b'xy'
        data = capture(record(100.0, 2, (0, 0, 0, 0), 0, b'plugin note'),
                       record(100.5, 1, (10, 0, 0, 1), 30001, push),
                       record(101.0, 0, (10, 0, 0, 1), 30001, sent),
                       record(101.5, 0, (10, 0, 0, 1), 30001, struct.pack('>HI', 0x1e, 0) + b'DESTINY'))
        self.assertEqual(M.activity_messages(data), [(0.5, 's2c', '10.0.0.1:30001', 18, b'abc'),
                                                     (1.0, 'c2s', '10.0.0.1:30001', 6, b'xy')])

    def test_host_messages_carry_the_host_id(self):
        push = struct.pack('>HI', M.BC_NOT, 7) + struct.pack('>BQII', 1, 2, 18, 3) + b'abc'
        data = capture(record(100.0, 1, (10, 0, 0, 1), 30001, push))
        self.assertEqual(M.host_messages(data)[0][0], 0.0)
        data = capture(record(99.5, 2, (0, 0, 0, 0), 0, b'note'), record(100.0, 1, (10, 0, 0, 1), 30001, push))
        self.assertEqual(M.host_messages(data)[0], (0.5, 's2c', '10.0.0.1:30001', 2, 18, b'abc'))

    def test_sessions_are_one_per_host_even_when_they_overlap(self):
        def push(t, host):
            body = struct.pack('>HI', M.BC_NOT, 7) + struct.pack('>BQII', 1, host, 4, 1) + b'j'
            return record(t, 1, (10, 0, 0, 1), 30001, body)
        # The live Restoration capture: the private host stays on while a public one joins beside it.
        data = capture(record(10.0, 2, (0, 0, 0, 0), 0, b'10.0.0.1:30001 state 2'), push(10.5, 0xA),
                       record(20.0, 2, (0, 0, 0, 0), 0, b'10.0.0.1:30001 state 2'), push(20.5, 0xB),
                       push(21.0, 0xA), push(22.0, 0xB))
        self.assertEqual([(t, h, [m[0] for m in msgs]) for t, h, msgs in M.sessions(data)],
                         [(0.5, 0xA, [0.5, 11.0]), (10.5, 0xB, [10.5, 12.0])])

    def test_region_records_skip_unused_slots_and_never_print_the_session(self):
        im = bytearray(CLI.PAH_REGIONS + CLI.PAH_REGION_COUNT * CLI.PAH_REGION_STRIDE)
        for i in range(CLI.PAH_REGION_COUNT):
            struct.pack_into('<I', im, CLI.PAH_REGIONS + i * CLI.PAH_REGION_STRIDE, 0xFFFFFFFF)
        rec = CLI.PAH_REGIONS
        struct.pack_into('<II', im, rec, 56, 1)
        im[rec + 8:rec + 12] = bytes([1, 1, 0, 0])
        im[rec + 0x38] = 2
        struct.pack_into('<I', im, rec + 0x4C, 0x4E)
        im[rec + 0x50:rec + 0x90] = b'\xab' * 64
        struct.pack_into('<Q', im, rec + 0xD0, 0xF1343670EFD9)
        words = {o: struct.unpack_from('<Q', im, o)[0] for o in range(0, len(im), 8)}
        [(slot, got)] = CLI.region_records(words)
        self.assertEqual(slot, 0)
        text = CLI.describe_region(got)
        self.assertEqual(text, 'region 56 public 1 +08 01010000 +19 0 +20 0 +38 2 session 0x4e + 64 B host ..efd9')
        self.assertNotIn('abab', text)

    def test_mission_row_reads_the_first_hosts_world_and_groups(self):
        def push(host, body):
            return record(10.0, 1, (10, 0, 0, 1), 30001,
                          struct.pack('>HI', M.BC_NOT, 7) + struct.pack('>BQII', 1, host, CLI.SENSOR, len(body)) + body)
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        cap = Path(tmp.name) / 'synthetic.pcap'
        cap.write_bytes(capture(push(0xA, b'w'), push(0xB, b'p')))
        frames = {b'w': {'entries': [{'hash': '0x4786c0e0', 'i8': 0xF, 'sub': 0}, {'hash': '0x11111111', 'i8': 5, 'sub': 1},
                                     {'hash': '0x11111111', 'i8': 5, 'sub': 2}]},
                  b'p': {'entries': [{'hash': '0xeaaf16e2', 'i8': 0xC, 'sub': 0}]}}
        saved = CLI.session_table, CLI.sensor_frames
        CLI.session_table = lambda data, names: [(0.0, 0xA, 3, [0x6E0DAE89], [113, 112]), (5.0, 0xB, 3, [], [56])]
        CLI.sensor_frames = lambda bodies: {b: frames[b] for b in bodies}
        try:
            row = CLI.mission_row(cap, {0x6E0DAE89: 'A Guardian Rises'}, {0x6E0DAE89: 164})
        finally:
            CLI.session_table, CLI.sensor_frames = saved
        self.assertIn('A Guardian Rises 0x6e0dae89 (index 164) | world 0x4786c0e0 | start [113]', row)
        self.assertIn('groups 0x11111111:2 0x4786c0e0:1', row)
        self.assertNotIn('eaaf16e2', row, 'only the first host (the PAH)')

    def test_hosts_rows_name_every_activity_host_and_skip_public_ones(self):
        def push(host, body):
            return record(10.0, 1, (10, 0, 0, 1), 30001,
                          struct.pack('>HI', M.BC_NOT, 7) + struct.pack('>BQII', 1, host, CLI.SENSOR, len(body)) + body)
        data = capture(push(0x1EF5E, b'w'), push(0x2A7F6, b'c'), push(0x3B000, b'p'))
        frames = {b'w': {'entries': [{'hash': '0x4786c0e0', 'i8': 0xF, 'sub': 0}, {'hash': '0x992bb471', 'i8': 4, 'sub': 0},
                                     {'hash': '0x992bb471', 'i8': 6, 'sub': 1}]},
                  b'c': {'entries': [{'hash': '0x4786c0e0', 'i8': 0xF, 'sub': 0}, {'hash': '0x2fe7dbe6', 'i8': 6, 'sub': 0}]},
                  b'p': {'entries': [{'hash': '0xeaaf16e2', 'i8': 0xC, 'sub': 0}]}}
        saved = CLI.session_table, CLI.sensor_frames
        CLI.session_table = lambda data, names: [(0.0, 0x1EF5E, 3, [0x6E0DAE89], [113, 112]),
                                                 (5.0, 0x2A7F6, 3, [0x6EAF0959], [25]), (6.0, 0x3B000, 3, [], [56])]
        CLI.sensor_frames = lambda bodies: {b: frames[b] for b in bodies}
        try:
            rows = CLI.hosts_tsv_rows('AGR', data, {0x6E0DAE89: 'A Guardian Rises', 0x6EAF0959: 'A Guardian Rises'},
                                      {0x6E0DAE89: 164})
        finally:
            CLI.session_table, CLI.sensor_frames = saved
        self.assertEqual([r.split('\t') for r in rows], [
            ['AGR', 'ef5e', 'starts', '0x6e0dae89', '164', 'A Guardian Rises', '0x4786c0e0', '113', '113,112',
             '0x992bb471:2 0x4786c0e0:1'],
            ['AGR', 'a7f6', 'starts', '0x6eaf0959', '-', 'A Guardian Rises', '0x4786c0e0', '25', '25',
             '0x2fe7dbe6:1 0x4786c0e0:1']])
        self.assertEqual(len(CLI.HOSTS_COLUMNS.split('\t')), len(rows[0].split('\t')))

    def test_incident_body_reads_the_length_at_d4(self):
        img = bytearray(0x2D0)
        struct.pack_into('<II', img, 0, 2, 965)
        struct.pack_into('<I', img, 0xD4, 3)
        img[0xD8:0xDC] = b'\x01\x02\x03\x04'  # the fourth byte is past the length
        words = {o: struct.unpack_from('<Q', img, o)[0] for o in range(0, len(img), 8)}
        self.assertEqual(CLI.incident_types(words)[:1], [965])
        self.assertEqual(CLI.incident_body(words), b'\x01\x02\x03')
        self.assertEqual(CLI.incident_body({}), b'')

    def test_lifetime_values_are_signed_and_only_changes_print(self):
        self.assertEqual(CLI.lifetime_value('03ff01'), 3)
        self.assertEqual(CLI.lifetime_value('ff'), -1)
        self.assertIsNone(CLI.lifetime_value(''))
        rows = [(1, 0xA, 'g', 3, 0), (2, 0xA, 'g', 3, 0), (3, 0xA, 'g', 3, 1), (3, 0xB, 'g', 3, 1),
                (4, 0xA, 'g', 3, None), (5, 0xA, 'g', 3, 6)]
        self.assertEqual(CLI.lifetime_changes(rows),
                         [(1, 0xA, 'g', 3, 0), (3, 0xA, 'g', 3, 1), (3, 0xB, 'g', 3, 1), (5, 0xA, 'g', 3, 6)])

    def test_refuses_other_link_types(self):
        data = struct.pack('<IHHiIII', 0xa1b2c3d4, 2, 4, 0, 0, 0x40000, 1)
        with self.assertRaises(ValueError):
            list(M.records(data))


if __name__ == '__main__':
    unittest.main()
