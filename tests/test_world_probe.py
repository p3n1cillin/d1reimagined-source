# SPDX-License-Identifier: GPL-2.0-or-later
import importlib.util
import json
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
        # hand-computed vectors of the client's (h >> 13) arithmetic shift, folded with 0x3FF
        self.assertEqual(M.handle_index(0x0000A123), 0x0123)
        self.assertEqual(M.handle_slot(0x0000A123), 5)      # esi = 5
        self.assertEqual(M.handle_slot(0x00000123), 0)      # esi = 0
        self.assertEqual(M.handle_slot(0x80002000), 1)      # esi = 0xFFFC0001 (sign-extended), (esi>>18)|0x3ff = 0x3fff
        self.assertEqual(M.handle_index(0x80002000), 0)

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


BASE = 0x100000


def trigger_guest(box=(1.0, 2.0, 3.0, 10.0, 20.0, 30.0)):
    """A one-volume trigger list: handle 3 -> node -> component -> geometry handle 5 -> shape box."""
    ram = bytearray(0xA000)

    def put(addr, fmt, *v):
        struct.pack_into('<' + fmt, ram, addr, *v)

    put(0x5000, 'Q', 0x6000)                    # registry pointer -> registry
    put(0x6000 + 0x10, 'Q', 0x7000)             # slot 0 data
    put(0x6000 + 0x30, 'ii', 0x200, 0)          # stride, mask
    put(0x8000, 'I', 3)                         # list head: handle 3, node offset 0x100
    put(0x8008, 'Q', 0x100)
    node, comp = 0x7600 + 0x100, 0x7600 + 0x100 - M.TRIGGER_NODE_IN_COMP
    put(node, 'I', 0xFFFFFFFF)                  # next handle: end
    put(comp + 0x24, 'I', 3)
    put(comp + 0x50, 'I', 5)                    # geometry handle
    put(comp + 0x58, 'I', 1)                    # shape index
    put(comp + 0x5C, 'IBxh', 0xABCD, 0x21, 7)   # row ref
    G = 0x7000 + 5 * 0x200
    put(G + 0x1C, 'i', 0x100)
    put(G + 0x54, 'i', 0x40)                    # table = G + 0x100 + 0x40; entry 1 at +8
    put(G + 0x140 + 8, 'Q', 0x9000)
    put(0x9000 + M.SHAPE_BOX_OFF, '3f', *box[:3])
    put(0x9000 + M.SHAPE_BOX_OFF + 0x10, '3f', *box[3:])
    put(0x9000 + M.SHAPE_BOX_OFF + 0x1C, 'f', 1.0)
    cell = bytearray(8)
    struct.pack_into('<Q', cell, 0, 0x8000)
    reg = bytearray(8)
    struct.pack_into('<Q', reg, 0, 0x5000)
    return FakeProc({0: ram, BASE + M.TRIGGER_LIST_CELL: cell, BASE + M.REGISTRY: reg})


class Args:
    near = None
    ref = None
    json = True


class Ut:
    class _P:
        position = (0.0, 0.0, 0.0)
        handle = 1
        address = 0x1234

    def read_unit_position(self, reader, base):
        return self._P()


class SanityAndWalk(unittest.TestCase):
    def test_box_sane(self):
        self.assertTrue(M.box_sane((1, 2, 3), (0, 0, 0)))
        self.assertFalse(M.box_sane((0, 2, 3), (0, 0, 0)))
        self.assertFalse(M.box_sane((1, 2, 501), (0, 0, 0)))
        self.assertFalse(M.box_sane((1, 2, float('nan')), (0, 0, 0)))
        self.assertFalse(M.box_sane((1, 2, 3), (float('inf'), 0, 0)))
        self.assertFalse(M.box_sane(None, (0, 0, 0)))

    def test_trigger_walk(self):
        m = M.Mem(trigger_guest(), BASE)
        data, lines = M.cmd_triggers(m, Ut(), Args())
        (t,) = data['triggers']
        self.assertEqual(t['ref'], (0xABCD, 0x21, 7))
        self.assertEqual(t['centre'], [10.0, 20.0, 30.0])
        self.assertEqual(t['half_extents'], [1.0, 2.0, 3.0])
        self.assertTrue(t['self_ok'])
        self.assertEqual(t['distance'], round((10 ** 2 + 20 ** 2 + 30 ** 2) ** 0.5, 1))
        self.assertEqual(set(t), set(M.TRIGGER_KEYS))

    def test_trigger_junk_box_is_unreadable(self):
        m = M.Mem(trigger_guest(box=(0.0, 0.0, 0.0, 1e30, 0, 0)), BASE)
        data, lines = M.cmd_triggers(m, Ut(), Args())
        (t,) = data['triggers']
        self.assertIsNone(t['centre'])
        self.assertIsNone(t['half_extents'])
        self.assertIn('unreadable', ' '.join(lines))

    def test_trigger_ref_filter(self):
        m = M.Mem(trigger_guest(), BASE)
        a = Args()
        a.ref = (1, 2, 3)
        self.assertEqual(M.cmd_triggers(m, Ut(), a)[0]['triggers'], [])

    def test_readonly_view_has_no_write(self):
        v = M.Mem(trigger_guest(), BASE).p
        self.assertFalse(hasattr(v, 'write'))
        self.assertFalse(hasattr(v, 'h'))


class JsonShape(unittest.TestCase):
    def test_clean_non_finite(self):
        self.assertEqual(M.clean({'a': float('nan'), 'b': [float('inf'), 1.5], 'c': (1, float('-inf'))}),
                         {'a': None, 'b': [None, 1.5], 'c': [1, None]})

    def test_fill_stable_keys(self):
        self.assertEqual(M.fill({'handle': 1}, M.TRIGGER_KEYS), {k: (1 if k == 'handle' else None) for k in M.TRIGGER_KEYS})

    def test_unreadable_items_keep_keys(self):
        m = M.Mem(FakeProc({BASE + M.TRIGGER_LIST_CELL: struct.pack('<Q', 0x8000),
                            0x8000: struct.pack('<IxxxxQ', 3, 0x100)}), BASE)
        data, _ = M.cmd_triggers(m, Ut(), Args())
        (t,) = data['triggers']
        self.assertEqual(set(t), set(M.TRIGGER_KEYS))
        self.assertEqual(t['handle'], 3)
        self.assertIsNone(t['centre'])
        json.dumps(M.clean(data))

    def test_player_and_flags_keys_when_unreadable(self):
        class NoUt:
            def read_unit_position(self, r, b):
                return None
        m = M.Mem(FakeProc({}), BASE)
        self.assertEqual(set(M.cmd_player(m, NoUt(), Args())[0]['player']), set(M.PLAYER_KEYS))
        self.assertEqual(set(M.cmd_flags(m, NoUt(), Args())[0]['flags']), set(M.FLAGS_KEYS))
        self.assertEqual(set(M.cmd_row(m, NoUt(), Args())[0]), set(M.ROW_KEYS))


class Cli(unittest.TestCase):
    def run_tool(self, *args):
        return subprocess.run([sys.executable, '-I', str(TOOL), *args], capture_output=True, text=True)

    def test_help(self):
        r = self.run_tool('--help')
        self.assertEqual(r.returncode, 0)
        self.assertIn('triggers', r.stdout)

    def test_row_without_ref_json_error(self):
        r = self.run_tool('row', '--json')
        self.assertEqual(r.returncode, 2)
        self.assertIn('error', json.loads(r.stdout))

    def test_no_emulator_json_error(self):
        if sys.platform != 'win32' or M.shadps4_running():
            self.skipTest('needs Windows with no shadPS4 running')
        r = self.run_tool('player', '--json')
        self.assertEqual(r.returncode, 1)
        self.assertIn('no shadPS4', json.loads(r.stdout)['error'])

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
