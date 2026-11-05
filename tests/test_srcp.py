# SPDX-License-Identifier: GPL-2.0-or-later
"""Synthetic SRCP captures only: the reader must find the activity-host messages in them."""
import importlib.util
from pathlib import Path
import struct
import unittest

spec = importlib.util.spec_from_file_location('srcp', Path(__file__).parents[1] / 'tools/ps4_setup/d1_srcp.py')
M = importlib.util.module_from_spec(spec)
spec.loader.exec_module(M)


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

    def test_splits_sessions_at_the_plugin_notes(self):
        push = struct.pack('>HI', M.BC_NOT, 7) + struct.pack('>BQII', 1, 2, 4, 1) + b'j'
        data = capture(record(10.0, 2, (0, 0, 0, 0), 0, b'10.0.0.1:30001 state 2'),
                       record(10.5, 1, (10, 0, 0, 1), 30001, push),
                       record(20.0, 2, (0, 0, 0, 0), 0, b'10.0.0.1:30001 state 2'),
                       record(20.5, 1, (10, 0, 0, 1), 30001, push))
        self.assertEqual([(t, [m[0] for m in msgs]) for t, msgs in M.sessions(data)],
                         [(0.0, [0.5]), (10.0, [10.5])])

    def test_refuses_other_link_types(self):
        data = struct.pack('<IHHiIII', 0xa1b2c3d4, 2, 4, 0, 0, 0x40000, 1)
        with self.assertRaises(ValueError):
            list(M.records(data))


if __name__ == '__main__':
    unittest.main()
