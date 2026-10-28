"""Synthetic controls for the native squad-to-physical-component reader."""
import importlib.util
import math
from pathlib import Path
import struct
import unittest

SPEC = importlib.util.spec_from_file_location('squad_tracking', Path(__file__).parents[1] / 'tools/ps4_setup/d1_squad_tracking.py')
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)


class Memory:
    def __init__(self):
        self.data = {}

    def put(self, address, fmt, *values):
        self.bytes(address, struct.pack('<' + fmt, *values))

    def bytes(self, address, raw):
        self.data.update((address + i, b) for i, b in enumerate(raw))

    def read(self, address, size):
        try:
            return bytes(self.data[address + i] for i in range(size))
        except KeyError as exc:
            raise OSError('unmapped synthetic memory') from exc

    def u(self, address, fmt):
        return struct.unpack('<' + fmt, self.read(address, struct.calcsize('<' + fmt)))[0]


def fixture():
    p = Memory()
    content, comp, sensor = 0x100000, 0x200000, 0x300000
    member, root, body, definition = 0x400000, 0x500000, 0x500100, 0x600000
    handles = {0x101: comp, 0x102: sensor, 0x103: root, 0x104: definition, 0x105: 0x700000}

    class Reader(M.SquadReader):
        def resolve(self, handle):
            if handle not in handles:
                raise M.Unavailable('missing fixture handle')
            return handles[handle]

    reader = Reader(p, 0)
    p.put(M.CURRENT_CONTENT, 'H', 4)
    p.put(M.LIFETIME_TABLE, 'q', 0x100)
    p.put(M.LIFETIME_TABLE + 16, 'Q', 64)
    p.put(M.LIFETIME_TABLE + 0x100 + 4 * 64 + 0x30, 'Q', content)
    p.put(content + 0x2BD20, 'hh', 0, 1)
    p.put(content + 0x2A51C, 'H', 0)
    p.put(content + 0x520, 'I', 0x101)
    p.put(content + 0x528, 'Q', 0)
    p.put(comp + 0x1A8, 'I', 0x102)
    p.bytes(sensor, struct.pack('<IBxhII', 0x12345678, 1, 8, 0x80800357, 0x8080071E))
    p.put(comp + 0x278, 'i', 1)
    handle, serial, transform = 0x2001, 0x3456, 0x2002
    p.put(comp + 0x27C, 'II', serial, handle)
    # Native pair registry -> pool -> serial; capacity guards the low index.
    registry, entries, pool, pool_header, pool_data = 0x800000, 0x810000, 0x820000, 0x830000, 0x840000
    p.put(M.PAIR_REGISTRY, 'Q', registry)
    p.put(registry, 'Q', entries)
    p.put(registry + 16, 'I', 64)
    p.put(entries + 64 + 0x18, 'Q', pool)
    p.put(pool, 'Q', pool_header)
    p.put(pool_header + 0x1C, 'H', 16)
    p.put(pool + 8, 'Q', pool_data)
    p.put(pool + 0x1C, 'I', 0)
    p.put(pool + 0x20, 'I', 8)
    p.put(pool_data + 8, 'I', serial)
    p.put(M.MEMBER_TABLE + 8, 'Q', member - 0x100)
    p.put(M.MEMBER_TABLE + 16, 'I', 0x100)
    p.bytes(member, bytes(0x58))
    p.put(member + 0x28, 'I', 0x105)
    p.put(member + 0x38, 'II', handle, transform)
    p.put(member + 0x50, 'i', -1)
    p.put(0x700274, 'I', 7)
    p.put(0x25D7A28, 'Q', 0x900000)
    p.put(0x25D7A30, 'I', 0x200)
    p.put(0x900000 + 0x200 + 0x14C, 'I', 7)
    p.put(M.ENTITY_TABLE + 8, 'Q', 0xA00000)
    p.put(M.ENTITY_TABLE + 16, 'I', 240)
    entity = 0xA00000 + 2 * 240
    p.put(entity + 0x54, 'I', 0x103)
    p.put(root + 8, 'I', 0x104)
    p.put(root + 16, 'Q', 0)
    c = definition + 0x38
    p.put(c, 'I', 0)
    p.put(c + 8, 'I', 1)
    p.put(c + 0x10, 'q', 0x100)
    p.put(c + 0x20, 'q', 0x200)
    p.put(c + 0x120, 'Ihh', M.ALL_COMPONENTS_KEY, 0, 1)
    p.put(c + 0x230 + 4, 'i', 0x100)
    p.put(body, 'I', 0x809DCEF6)
    p.put(body + 0x24, 'II', 0x4003, 0)
    p.put(body + 0x2C, 'I', transform)
    p.put(body + 0x1A0, '3f', 100, -550, 8)
    object_entries = 0xB00000
    p.put(M.OBJECT_TABLE + 8, 'Q', object_entries)
    p.put(M.OBJECT_TABLE + 16, 'I', 32)
    p.put(M.OBJECT_TABLE + 40, 'i', -1)
    entry = object_entries + 3 * 32
    p.put(entry + 8, 'Q', (entry - body) & M.MASK64)
    return p, reader, dict(comp=comp, sensor=sensor, body=body, member=member,
                           entity=entity, serial_at=pool_data + 8, content=content)


class SquadTrackingTests(unittest.TestCase):
    def test_resolves_current_member_to_physical_position(self):
        p, reader, a = fixture()
        sample = reader.snapshot(0x12345678)
        self.assertEqual(sample['targets'][0]['parts'][0]['position'], (100, -550, 8))
        self.assertTrue(sample['targets'][0]['sense_matches'])
        self.assertIsNone(sample['targets'][0]['health'])

    def test_zero_members_is_not_a_target(self):
        p, reader, a = fixture()
        p.put(a['comp'] + 0x278, 'i', 0)
        self.assertEqual(reader.snapshot()['targets'], [])

    def test_inactive_or_sense_mismatch_is_not_targeted(self):
        for address, fmt, value in ((0x400050, 'i', 2), (0x700274, 'I', 9)):
            p, reader, a = fixture()
            p.put(address, fmt, value)
            self.assertEqual(reader.snapshot()['targets'], [])

    def test_recycled_pair_and_object_are_diagnostics(self):
        for key, offset, value in (('serial_at', 0, 99), ('body', 0x24, 99)):
            p, reader, a = fixture()
            p.put(a[key] + offset, 'I', value)
            sample = reader.snapshot()
            self.assertEqual(sample['targets'], [])
            self.assertEqual(len(sample['squads'][0]['errors']), 1)

    def test_absent_binding_and_nonfinite_position_are_unknown(self):
        for key, offset, fmt, values in (('entity', 0x54, 'I', (0xFFFFFFFF,)),
                                         ('body', 0x1A0, '3f', (math.nan, 1, 2))):
            p, reader, a = fixture()
            p.put(a[key] + offset, fmt, *values)
            sample = reader.snapshot()
            self.assertFalse(sample['targets'])
            self.assertTrue(sample['squads'][0]['errors'])

    def test_missing_sense_reference_does_not_use_permissive_native_fallback(self):
        p, reader, a = fixture()
        p.put(a['member'] + 0x28, 'I', 0xFFFFFFFF)
        self.assertTrue(reader.snapshot()['squads'][0]['errors'])

    def test_squad_list_change_refuses_whole_sample(self):
        p, reader, a = fixture()
        original = p.read
        calls = 0
        def changing(address, size):
            nonlocal calls
            if address == a['comp'] + 0x278:
                calls += 1
                if calls == 2:
                    return struct.pack('<i', 0)
            return original(address, size)
        p.read = changing
        with self.assertRaises(M.Unavailable):
            reader.snapshot()

    def test_member_liveness_change_during_position_read_is_refused(self):
        p, reader, a = fixture()
        original = p.read
        def changing(address, size):
            if address == a['body'] + 0x1A0:
                p.put(a['member'] + 0x50, 'i', 1)
            return original(address, size)
        p.read = changing
        self.assertTrue(reader.snapshot()['squads'][0]['errors'])

    def test_content_change_refuses_whole_sample(self):
        p, reader, a = fixture()
        original = reader.content
        calls = 0
        def changing():
            nonlocal calls
            calls += 1
            return original() if calls == 1 else (5, a['content'])
        reader.content = changing
        with self.assertRaises(M.Unavailable):
            reader.snapshot()

    def test_other_group_is_not_returned_and_malformed_count_is_refused(self):
        p, reader, a = fixture()
        self.assertEqual(reader.snapshot(1)['squads'], [])
        p.put(a['comp'] + 0x278, 'i', 33)
        with self.assertRaises(M.Unavailable):
            reader.snapshot()

    def test_native_registry_resolution_uses_signed_mask(self):
        p = Memory()
        reader = M.SquadReader(p, 0)
        p.put(M.REGISTRY, 'Q', 0x1000)
        p.put(0x1000, 'Q', 0x2000)
        p.put(0x2000 + 16, 'Q', 0x3000)
        p.put(0x2000 + 48, 'i', 32)
        p.put(0x2000 + 52, 'i', -1)
        p.put(0x3020 + 8, 'Q', (0x3020 - 0x4000) & M.MASK64)
        self.assertEqual(reader.resolve(1), 0x4000)


if __name__ == '__main__':
    unittest.main()
