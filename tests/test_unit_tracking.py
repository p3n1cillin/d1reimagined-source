"""Synthetic memory controls for native-unit identity and transform guards."""
import importlib.util
from pathlib import Path
import struct
import sys
import unittest

SPEC = importlib.util.spec_from_file_location('d1_unit_tracking_test', Path(__file__).parents[1] / 'tools/ps4_setup/d1_unit_tracking.py')
M = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = M
SPEC.loader.exec_module(M)


class Memory:
    def __init__(self):
        self.values = {}
        self.bytes = {}

    def u(self, address, fmt):
        if address not in self.values:
            raise OSError('unmapped synthetic memory')
        return self.values[address]

    def read(self, address, size):
        if address not in self.bytes:
            raise OSError('unmapped synthetic memory')
        return self.bytes[address][:size]


def fixture():
    m = Memory()
    base, pool, player, entries, unit, handle = 0x100000, 0x200000, 0x300000, 0x400000, 0x500000, 0xA0200057
    entry = entries + (handle & 0x1FFF) * 32
    m.values.update({base + M.PLAYER_POOL: pool, pool + 8: player, player + M.UNIT_LINK: handle,
                     base + M.OBJECT_TABLE + 8: entries, base + M.OBJECT_TABLE + 0x10: 32,
                     base + M.OBJECT_TABLE + 0x28: -1, entry + 8: (entry - unit) & M.U64_MASK,
                     unit + M.SELF_HANDLE: handle})
    for offset in M.TRANSFORMS:
        m.bytes[unit + offset] = struct.pack('<3f', 300, -600, 12)
    for offset in M.FORWARD_VECTORS:
        m.bytes[unit + offset] = struct.pack('<3f', 1, 0, 0)
    return m, base, player, unit, handle


class UnitTrackingTests(unittest.TestCase):
    def test_resolves_unsigned_self_relative_pointer_and_agreed_transforms(self):
        memory, base, player, unit, handle = fixture()
        result = M.read_unit_position(memory, base)
        self.assertEqual((result.handle, result.address, result.position), (handle, unit, (300, -600, 12)))

    def test_refuses_absent_link_or_wrong_object_identity(self):
        for value in (0, 0xFFFFFFFF):
            memory, base, player, unit, handle = fixture()
            memory.values[player + M.UNIT_LINK] = value
            self.assertIsNone(M.read_unit_position(memory, base))
        memory, base, player, unit, handle = fixture()
        memory.values[unit + M.SELF_HANDLE] = handle + 1
        self.assertIsNone(M.read_unit_position(memory, base))

    def test_refuses_bad_geometry_and_divergent_or_nonfinite_transforms(self):
        memory, base, player, unit, handle = fixture()
        memory.values[base + M.OBJECT_TABLE + 0x10] = 128
        self.assertIsNone(M.read_unit_position(memory, base))
        for point in ((301, -600, 12), (float('nan'), -600, 12)):
            memory, base, player, unit, handle = fixture()
            memory.bytes[unit + M.TRANSFORMS[-1]] = struct.pack('<3f', *point)
            self.assertIsNone(M.read_unit_position(memory, base))

    def test_refuses_link_change_during_the_read(self):
        memory, base, player, unit, handle = fixture()
        original = memory.read
        def changing_read(address, size):
            memory.values[player + M.UNIT_LINK] = handle + 1
            return original(address, size)
        memory.read = changing_read
        self.assertIsNone(M.read_unit_position(memory, base))

    def test_unmapped_memory_is_unavailable_and_bad_player_index_is_an_error(self):
        self.assertIsNone(M.read_unit_position(Memory(), 0))
        with self.assertRaises(ValueError):
            M.read_unit_position(Memory(), 0, 4)

    def test_facing_requires_finite_normalized_agreed_vectors(self):
        memory, base, player, unit, handle = fixture()
        self.assertEqual(M.read_unit_facing(memory, unit), 0)
        for vector in ((0, 1, 0), (0, 0, 0), (float('nan'), 0, 0), (0, 0, 1)):
            memory.bytes[unit + M.FORWARD_VECTORS[-1]] = struct.pack('<3f', *vector)
            self.assertIsNone(M.read_unit_facing(memory, unit))


if __name__ == '__main__':
    unittest.main()
