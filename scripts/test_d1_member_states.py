# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Synthetic regression tests; no owned executable or captured list is required."""
import importlib.util
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest

TOOL = Path(__file__).resolve().parents[1] / 'tools/ps4_setup/d1-member-states.py'
spec = importlib.util.spec_from_file_location('member_states', TOOL)
states = importlib.util.module_from_spec(spec)
spec.loader.exec_module(states)


class MemberStatesTests(unittest.TestCase):
    def test_known_zero_binding_crash_cannot_enter_the_native_cache_guard(self):
        # 56578 0x1076C0 tests sub != -1 and kind != -1; 0x1BCED0
        # additionally requires index != -1 before caching the reference.
        def cached(image):
            return (struct.unpack_from('<H', image, 0x56)[0] != 0xFFFF
                    and image[0x54] != 0xFF
                    and struct.unpack_from('<I', image, 0x58)[0] != 0xFFFFFFFF)
        self.assertTrue(cached(bytes(states.SIZE)), 'the native failing case must be the negative control')
        self.assertFalse(cached(states.member(7)))
        with self.assertRaises(ValueError):
            states.member(1 << 32)

    def test_patching_preserves_authority_non_targets_and_existing_b_value(self):
        prefix = '# synthetic list\n@authority 12,2,0\n0x1234 0x1 4 1 0\n'
        text = prefix + '0x1234 0x2 5 1 1 0x80800868 00 0x1111 deadbeef # keep B\n'
        result = states.patched(text, 0x1234, {5: 3})
        self.assertTrue(result.startswith(prefix))
        self.assertTrue(result.endswith('0x1111 deadbeef # keep B\n'))
        self.assertEqual(struct.unpack_from('<I', bytes.fromhex(result.splitlines()[-1].split()[6]))[0], 3)

    def test_wrong_type_or_duplicate_member_is_refused(self):
        for text in ['1 2 5 1 0 0x9999 00\n', '1 2 5 1 0\n1 2 5 1 0\n']:
            with self.assertRaises(ValueError):
                states.patched(text, 1, {5: 1})

    def test_missing_member_does_not_overwrite_an_existing_output(self):
        with tempfile.TemporaryDirectory() as directory:
            base, out = Path(directory)/'base.txt', Path(directory)/'out.txt'
            base.write_text('@authority 12,2,0\n1 2 5 1 0\n')
            out.write_text('previous working list\n')
            result = subprocess.run([sys.executable, '-I', str(TOOL), str(base), str(out), '1', '6:1'],
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 2)
            self.assertIn('members not in input list', result.stderr)
            self.assertEqual(out.read_text(), 'previous working list\n')


if __name__ == '__main__':
    unittest.main()
