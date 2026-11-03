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


class CuiGraphTest(unittest.TestCase):
    def test_graph_adds_a_binding_and_preserves_existing_path_contents(self):
        b = bytearray(0x158)

        def arr(field, n, stride):
            head = len(b) + 8
            b.extend(struct.pack('<IIQII', 0, M.ARRAY, n, 5, 0))
            start = len(b)
            b.extend(b'\0' * (n * stride))
            struct.pack_into('<Qq', b, field, n, head - field - 8)
            return [start + i * stride for i in range(n)]

        name, = arr(0xc8, 1, 4)
        struct.pack_into('<I', b, name, 0x1234)
        b.extend(b'\0' * (-len(b) % 8))
        paths = arr(0x48, 2, 16)
        for pos, value in zip(paths, (4, 0x201)):
            token, = arr(pos, 1, 8)
            struct.pack_into('<Q', b, token, value)
        binding, = arr(0x138, 1, 64)
        struct.pack_into('<QqQQqQQQ', b, binding, 20, paths[0] - binding - 8,
                         1, 12, paths[1] - binding - 32, 0x30000, 0, 0)
        struct.pack_into('<Q', b, 0, len(b))
        changed, manifest = M.video_graph_candidate(bytes(b), 0x1234, 0x5678)
        self.assertEqual(manifest['target_list_node'], 12)
        self.assertEqual(manifest['target_slot'], 4)
        links = M.array(changed, 0x138, 64)
        self.assertEqual(len(links), 2)
        for link, expected in zip(links, (0, 1)):
            path = link + 8 + struct.unpack_from('<q', changed, link + 8)[0]
            token, = M.array(changed, path, 8)
            self.assertEqual(struct.unpack_from('<HHI', changed, token), (4, expected, 0))
        with self.assertRaisesRegex(ValueError, 'already present'):
            M.video_graph_candidate(changed, 0x1234, 0x5678)


class CuiStringsTest(unittest.TestCase):
    def test_new_labels_and_control_parts_round_trip_through_existing_decoder(self):
        index, english = bytearray(0x58), bytearray(0x58)

        def arr(b, field, n, stride):
            b.extend(b'\0' * (-len(b) % 8))
            head = len(b) + 8
            b.extend(struct.pack('<IIQII', 0, M.ARRAY, n, 5, 0))
            start = len(b)
            b.extend(b'\0' * (n * stride))
            struct.pack_into('<Qq', b, field, n, head - field - 8)
            return [start + i * stride for i in range(n)]

        english_tag, index_tag = 0x80800001, 0x80800002
        struct.pack_into('<I', index, 0x18, english_tag)
        for pos, value in zip(arr(index, 8, 2, 4), (10, 20)):
            struct.pack_into('<I', index, pos, value)
        parts = arr(english, 8, 2, 32)
        blob = arr(english, 0x38, 3, 1)
        english[blob[0]:blob[0] + 3] = b'One'
        control = len(english)
        english.extend(b'" ')
        english.extend(b'\0' * (-len(english) % 16))
        formatting = len(english)
        english.extend(bytes(range(80)))
        struct.pack_into('<q', english, parts[1], formatting - parts[1])
        for pos, target, length in zip(parts, (blob[0], control), (3, 2)):
            struct.pack_into('<q', english, pos + 8, target - pos - 8)
            struct.pack_into('<HHH', english, pos + 0x14, length, length, 0)
        for pos, part in zip(arr(english, 0x48, 2, 16), parts):
            struct.pack_into('<qQ', english, pos, part - pos, 1)
        for b in (index, english):
            struct.pack_into('<Q', b, 0, len(b))
        si, en = M.video_strings_candidate(bytes(index), bytes(english), {'0000001e': 'Frame Rate'})
        relocated = M.array(en, 8, 32)[1]
        fmt = relocated + struct.unpack_from('<q', en, relocated)[0]
        self.assertEqual(en[fmt:fmt + 80], bytes(range(80)))
        decoder_spec = importlib.util.spec_from_file_location('string_decoder', Path(__file__).parents[1] /
                                                             'tools/ps4_setup/d1-scenarios.py')
        decoder = importlib.util.module_from_spec(decoder_spec)
        decoder_spec.loader.exec_module(decoder)
        decoder._mem.update({index_tag: si, english_tag: en})
        self.assertEqual(decoder.library(index_tag), {10: 'One', 20: '" ', 30: 'Frame Rate'})
        with self.assertRaisesRegex(ValueError, 'follow every original hash'):
            M.video_strings_candidate(bytes(index), bytes(english), {'0000000f': 'Collision'})


if __name__ == '__main__':
    unittest.main()
