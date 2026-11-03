# SPDX-License-Identifier: GPL-2.0-or-later
"""Synthetic tag data only: relocation must preserve existing settings and references."""
import importlib.util
from pathlib import Path
import struct
import unittest

spec = importlib.util.spec_from_file_location('settings', Path(__file__).parents[1] /
                                             'tools/ps4_setup/d1_cui_settings.py')
M = importlib.util.module_from_spec(spec)
spec.loader.exec_module(M)


def fixture():
    b = bytearray(0x100)
    struct.pack_into('<Q', b, 0, len(b))
    for field, head, count, cls in ((8, 0x30, 1, 10), (0x18, 0x90, 1, 11),
                                    (0x58, 0x70, 1, 12), (0xb8, 0xd0, 2, 13)):
        struct.pack_into('<Qq', b, field, count, head - field - 8)
        struct.pack_into('<IQII', b, head - 4, M.ARRAY, count, cls, 0)
    struct.pack_into('<IIIIII', b, 0x40, 0x1234, 0x5678, 0x9012, 0xffffffff, 0xffffffff, 0)
    struct.pack_into('<q', b, 0x80, 0x20)
    struct.pack_into('<IIIIII', b, 0xa0, 4, 0x3456, 0x7890, 0xffffffff, 0x1111, 0)
    struct.pack_into('<8I', b, 0xe0, 0, 15, 16, 17, 1, 18, 19, 20)
    return bytes(b)


class CuiSettingsTest(unittest.TestCase):
    def test_addition_relocates_groups_without_changing_existing_choices(self):
        before = fixture()
        changed, manifest = M.video_candidate(before, 0x1234)
        tag = M.Settings(changed)
        self.assertEqual(tag.summary(), {'groups': 2, 'settings': 3, 'ids': [4, 5, 6]})
        self.assertEqual(manifest['setting_ids'], {'frame_rate': 5, 'resolution': 6})
        old_group = next(o for o in tag.groups if tag.u32(o) == 0x1234)
        ref, = M.array(changed, old_group + 24, 8)
        row = ref + struct.unpack_from('<q', changed, ref)[0]
        self.assertEqual(changed[row:row + 24], before[0xa0:0xb8])
        a, b = M.array(changed, row + 24, 16)
        self.assertEqual(changed[a:b + 16], before[0xe0:0x100])
        self.assertEqual([len(M.array(changed, r + 24, 16)) for r in tag.rows], [2, 6, 3])
        self.assertFalse(manifest['installed'])
        with self.assertRaisesRegex(ValueError, 'already present'):
            M.video_candidate(changed, 0x1234)

    def test_corrupt_references_and_counts_fail_closed(self):
        for offset, fmt, value in ((0x80, '<q', -0x80), (8, '<Q', 4097),
                                   (0x2c, '<I', 0), (0, '<Q', 1)):
            with self.subTest(offset=offset):
                b = bytearray(fixture())
                struct.pack_into(fmt, b, offset, value)
                with self.assertRaises(ValueError):
                    M.Settings(b)


if __name__ == '__main__':
    unittest.main()
