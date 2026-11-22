# SPDX-License-Identifier: GPL-2.0-or-later
import importlib.util
import struct
import subprocess
import sys
import unittest
from pathlib import Path

TOOL = Path(__file__).parents[1] / 'tools/ps4_setup/d1-world-probe.py'
SPEC = importlib.util.spec_from_file_location('world_probe', TOOL)
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)


class FakeProc:
    """A sparse guest: {address: bytes}; reads outside raise OSError."""

    def __init__(self, mem):
        self.mem = mem

    def read(self, addr, n):
        for a, b in self.mem.items():
            if a <= addr and addr + n <= a + len(b):
                return b[addr - a:addr - a + n]
        raise OSError('unmapped')

    def u(self, addr, fmt):
        return struct.unpack('<' + fmt, self.read(addr, struct.calcsize(fmt)))[0]


class Helpers(unittest.TestCase):
    def test_parse_ref(self):
        self.assertEqual(M.parse_ref('0xd7f68da0/0x21/71'), (0xD7F68DA0, 0x21, 71))
        self.assertEqual(M.parse_ref('0x1/4/-1'), (1, 4, -1))
        for bad in ('0x1/2', '0x1/2/3/4', 'zz/1/1', '0x1ffffffff/1/1', '0x1/256/1', '0x1/1/40000'):
            with self.assertRaises(ValueError):
                M.parse_ref(bad)

    def test_decode_ref(self):
        raw = struct.pack('<IBxh', 0xAABBCCDD, 0x21, -2)
        self.assertEqual(M.decode_ref(raw), (0xAABBCCDD, 0x21, -2))
        self.assertEqual(M.fmt_ref((0xAB, 4, 7)), '0x000000ab/0x4/7')

    def test_handle_math(self):
        self.assertEqual(M.handle_index(0x12345), 0x12345 & 0x1FFF)
        self.assertEqual(M.handle_slot((5 << 13) | 0x123), 5)
        self.assertEqual(M.handle_slot(0x123), 0)
        h = 0x80000000 | (3 << 13)      # a high-bit handle sign-extends the shift
        esi = (struct.unpack('<i', struct.pack('<I', h))[0] >> 13) & 0xFFFFFFFF
        self.assertEqual(M.handle_slot(h), ((esi >> 0x12) | 0x3FF) & esi)

    def test_object_address(self):
        self.assertEqual(M.object_address(0x1000, M.M64, 0x10), 0x1000 - 0x10)
        self.assertEqual(M.object_address(0x1000, 0, 0x10), 0x1000)
        self.assertEqual(M.object_address(0x10, M.M64, 0x20), (0x10 - 0x20) & M.M64)

    def test_shape_box(self):
        buf = bytearray(0x20)       # half at +0, centre at +0x10, w at +0x1C
        struct.pack_into('<3f', buf, 0, 1.5, 2.5, 3.5)
        struct.pack_into('<3f', buf, 0x10, 10, 20, 30)
        struct.pack_into('<f', buf, 0x1C, 1.0)
        half, centre, w = M.decode_shape_box(bytes(buf))
        self.assertEqual(half, (1.5, 2.5, 3.5))
        self.assertEqual(centre, (10.0, 20.0, 30.0))
        self.assertEqual(w, 1.0)
        with self.assertRaises(ValueError):
            M.decode_shape_box(b'\0' * 0x1F)

    def test_nav_decode(self):
        row = bytearray(0x38)
        struct.pack_into('<QQ', row, 0x28, 0xFFFF_0000_1234_5678, 0x40)
        self.assertEqual(M.decode_nav_row(bytes(row)), (0x12345678, 0x40))
        self.assertEqual(M.decode_nav_position(struct.pack('<3f', 1, -2, 3.5) + b'xx'), (1.0, -2.0, 3.5))

    def test_distance(self):
        self.assertEqual(M.distance((0, 0, 0), (3, 4, 12)), 13.0)
        self.assertEqual(M.distance((0, 0, 0), (3, 4, 12), planar=True), 5.0)
        self.assertIsNone(M.distance(None, (0, 0, 0)))
        self.assertIsNone(M.distance((float('nan'), 0, 0), (0, 0, 0)))

    def test_words_and_hx(self):
        self.assertEqual(M.words(struct.pack('<2I', 1, 0xFF), 2), '00000001 000000ff')
        self.assertEqual(M.words(None, 2), 'unreadable')
        self.assertEqual(M.words(b'\0' * 4, 2), 'unreadable')
        self.assertEqual(M.hx(None), 'unreadable')
        self.assertEqual(M.hx(255), '0xff')


class GuardedReads(unittest.TestCase):
    def test_unreadable_is_none(self):
        m = M.Mem(FakeProc({}), 0x1000)
        self.assertIsNone(m.u(0x10, 'I'))
        self.assertIsNone(m.read(0x10, 4))
        self.assertIsNone(m.f3(0x10))
        self.assertIsNone(m.resolve(0x1234))
        self.assertIsNone(m.resolve(0xFFFFFFFF))

    def test_resolve_synthetic_registry(self):
        base, reg, data = 0x10000, 0x20000, 0x30000
        h = 3                           # slot 0, index 3
        entry = data + 3 * 0x20
        mem = {
            base + M.REGISTRY: struct.pack('<Q', 0x5000),
            0x5000: struct.pack('<Q', reg),
            reg + 0x10: struct.pack('<Q', data),
            reg + 0x30: struct.pack('<ii', 0x20, 0x7),
            entry + 8: struct.pack('<Q', 0x4),
        }
        m = M.Mem(FakeProc(mem), base)
        self.assertEqual(m.resolve(h), entry - (0x7 & 0x4))

    def test_empty_list_is_none(self):
        m = M.Mem(FakeProc({0x1000 + M.INTERACTABLE_LIST_CELL: struct.pack('<Q', 0)}), 0x1000)
        self.assertIsNone(M.walk_list(m, M.INTERACTABLE_LIST_CELL))


class Cli(unittest.TestCase):
    def run_tool(self, *args):
        return subprocess.run([sys.executable, '-I', str(TOOL), *args], capture_output=True, text=True)

    def test_help(self):
        r = self.run_tool('--help')
        self.assertEqual(r.returncode, 0)
        self.assertIn('triggers', r.stdout)

    def test_bad_ref(self):
        self.assertEqual(self.run_tool('row', '--ref', 'nope').returncode, 2)

    def test_no_emulator_exits_1(self):
        if sys.platform != 'win32' or M.shadps4_running():
            self.skipTest('needs Windows with no shadPS4 running')
        r = self.run_tool('player')
        self.assertEqual(r.returncode, 1)
        self.assertIn('no shadPS4', r.stdout)


if __name__ == '__main__':
    unittest.main()
