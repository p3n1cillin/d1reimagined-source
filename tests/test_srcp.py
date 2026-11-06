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

    def test_investment_messages_pair_requests_and_replies_by_sequence(self):
        req = struct.pack('>HI', M.WS_REQUEST, 990) + struct.pack('>H', 901) + b'\x80\x00'
        rsp = struct.pack('>HI', M.WS_RESPONSE, 990) + struct.pack('>HH', 200, 901) + b'\x18'
        other = struct.pack('>HI', 0x2a, 991) + b'\x08\x01'
        data = capture(record(10.0, 0, (10, 0, 0, 1), 1, req), record(10.1, 1, (10, 0, 0, 1), 1, rsp),
                       record(10.2, 0, (10, 0, 0, 1), 1, other))
        got = M.investment_messages(data)
        self.assertEqual([(d, seq, st, mid) for _, d, seq, st, mid, _ in got], [('req', 990, None, 901), ('rsp', 990, 200, 901)])
        self.assertEqual(got[0][5][:2], b'\x03\x85', 'the bitstream starts with the service id')

    def test_queuez_updates_walk_objects_and_drop_a_truncated_push(self):
        def push(objs, cut=0):
            s = struct.pack('>I', 1) + struct.pack('>IQIBI', 4, 0xD1, 590, 0, len(objs))
            for it, key, body in objs:
                s += struct.pack('>IQI', it, key, len(body)) + body
            return struct.pack('>HI', M.QUEUEZ_UPDATE, 5) + (s[:-cut] if cut else s)
        data = capture(record(1.0, 1, (10, 0, 0, 1), 1, push([(0x0EB6AC38, 7, b''), (0xB5DF2907, 8, b'\x01\x02')])),
                       record(2.0, 1, (10, 0, 0, 1), 1, push([(0xB5DF2907, 8, b'\x01\x02')], cut=1)))
        [(t, [(fam, ver, flags, objs)])] = M.queuez_updates(data)
        self.assertEqual((t, fam, ver, flags), (0.0, 4, 590, 0))
        self.assertEqual(objs, [(0x0EB6AC38, 7, b''), (0xB5DF2907, 8, b'\x01\x02')])
        self.assertEqual(CLI.queuez_summary([(4, 590, 0, objs)]), 'f4 0eb6ac38:0 b5df2907:2', 'no keys or roots')

    def test_payload_tail_reads_the_flag_length_and_unaligned_bytes(self):
        # the arguments end at bit 3: flag 1, length 2, then 0xAB 0xCD, all from bit 4
        bits = 0b101 << 37 | 1 << 36 | 2 << 20 | 0xABCD << 4
        body = bits.to_bytes(5, 'big')
        self.assertEqual(CLI.payload_tail(body, 3), (True, b'\xab\xcd', 36))
        self.assertEqual(CLI.payload_tail(b'\x00', 3), (False, b'', 4))
        self.assertEqual(CLI.payload_tail(b'\x00', 8), (False, b'', 8))

    def test_render_fields_follows_nested_tags_and_redacts_big_values(self):
        fields = {0x10: [(0, 1, 0x20), (0x18, 5, None)], 0x20: [(0, 2, None), (8, 6, None), (0x10, 4, None)]}
        im = bytearray(0x1C)
        im[0] = 1
        struct.pack_into('<q', im, 8, 0x1234_5678_9ABC)
        struct.pack_into('<h', im, 0x10, 4168)
        struct.pack_into('<i', im, 0x18, -1)
        self.assertEqual(CLI.render_fields(0x10, bytes(im), fields),
                         [('f0.f0', 1), ('f0.f1', '#..9abc'), ('f0.f2', 4168), ('f1', -1)])

    def test_investment_rows_decode_from_bit_16_and_say_when_no_schema(self):
        req = struct.pack('>HI', M.WS_REQUEST, 1) + struct.pack('>H', 403) + b'\x00' * 9
        unknown = struct.pack('>HI', M.WS_REQUEST, 2) + struct.pack('>H', 4242) + b'\x00'
        data = capture(record(1.0, 0, (10, 0, 0, 1), 1, req), record(2.0, 0, (10, 0, 0, 1), 1, unknown))
        seen = []

        def decode(rows):
            rows = list(rows)
            seen.extend(rows)
            return [{'end': 16 + 64, 'object': (7).to_bytes(8, 'little').hex()}]
        rows = CLI.investment_rows(data, {403: (0x80801942, 0x80801834)}, {0x80801942: [(0, 0xA, None)]}, decode)
        self.assertEqual([(tag, at) for tag, _, at in seen], [(0x80801942, 16)])
        self.assertEqual(rows[0][4:], (403, [('f0', 7)], None, 88 - 81))
        self.assertEqual(rows[1][4:6], (4242, 'no schema'))

    def test_slot_changes_use_only_the_bytes_a_body_wrote(self):
        base = 0x10
        zeros, ones = bytearray(base + 32 * 3), bytearray(b'\xff' * (base + 32 * 3))
        for im in (zeros, ones):  # slot 1 writes only its quantity; slot 2 its content and instance
            struct.pack_into('<i', im, base + 32 + 0x10, 950)
            struct.pack_into('<H', im, base + 64, 714)
            im[base + 64 + 8:base + 64 + 16] = b'\x00' * 8
        present = CLI.present_bytes(zeros, ones)
        state = {1: (869, 1900)}
        self.assertEqual(CLI.slot_changes(bytes(zeros), present, base, 3, state),
                         [(1, 869, 950, 1900, False), (2, 714, None, None, True)])
        self.assertEqual(state, {1: (869, 950), 2: (714, None)})

    def test_stack_rows_decode_each_object_twice_and_keep_a_partial_profile(self):
        def push(objs):
            s = struct.pack('>I', 1) + struct.pack('>IQIBI', 4, 0xD1, 7, 0, len(objs))
            for it, body in objs:
                s += struct.pack('>IQI', it, 1, len(body)) + body
            return struct.pack('>HI', M.QUEUEZ_UPDATE, 5) + s
        data = capture(record(1.0, 1, (10, 0, 0, 1), 1, push([(0xB5DF2907, b'a'), (0xEB4B8439, b'p'), (0x1, b'x')])))
        acct = bytearray(0x4C8 + 32 * 326)
        struct.pack_into('<i', acct, 0x4C8 + 32 * 308 + 0x10, 24050)
        acct_ff = bytearray(b'\xff' * len(acct))
        struct.pack_into('<i', acct_ff, 0x4C8 + 32 * 308 + 0x10, 24050)
        calls = []

        def decode(rows):
            rows = list(rows)
            calls.extend((tag, fill) for tag, _, _, fill in rows)
            return [{'object': acct.hex()}, {'object': acct_ff.hex()},
                    {'error': 'runs past', 'partial': '00' * 0x38}, {'error': 'runs past', 'partial': 'ff' * 0x38}]
        rows = CLI.stack_rows(data, decode)
        self.assertEqual(calls, [(0x808018C7, '00'), (0x808018C7, 'ff'), (0x80801743, '00'), (0x80801743, 'ff')])
        self.assertEqual(rows[0][:4], (0.0, 'account', 7, [(308, None, 24050, None, False)]))
        self.assertEqual(rows[1][1:], ('profile', 7, [], 'partial'), 'a profile walk may stop early')

    def test_refuses_other_link_types(self):
        data = struct.pack('<IHHiIII', 0xa1b2c3d4, 2, 4, 0, 0, 0x40000, 1)
        with self.assertRaises(ValueError):
            list(M.records(data))


if __name__ == '__main__':
    unittest.main()
