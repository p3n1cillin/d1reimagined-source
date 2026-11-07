# SPDX-License-Identifier: GPL-2.0-or-later
"""Creation state is content-derived; account flags must not become profile completions."""
import importlib.util
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest

REPO = Path(__file__).resolve().parents[1]
TOOLS = REPO / 'tools/ps4_setup'
sys.path.insert(0, str(TOOLS))
from d1_new_guardian import BUILD, creation_defaults, select_defaults

spec = importlib.util.spec_from_file_location('completions', TOOLS / 'd1-profile-completions.py')
completions = importlib.util.module_from_spec(spec)
spec.loader.exec_module(completions)


def tag(size):
    b = bytearray(size)
    struct.pack_into('<Q', b, 0, size)
    return b


def array(b, field, first, count, kind):
    struct.pack_into('<Qq', b, field, count, first - 16 - field - 8)
    struct.pack_into('<IQII', b, first - 20, 0x80800184, count, kind, 0)


def synthetic_entries():
    # Fictional ordinals and values, deliberately unrelated to the owned corpus.
    root, defaults, flags = tag(0x540), tag(2048), tag(72)
    struct.pack_into('<I', root, 0xE8, 101)
    struct.pack_into('<I', root, 0x538, 102)
    array(flags, 8, 48, 3, 0x808018B8)
    for i, (kind, sub) in enumerate(((1, 9), (3, 14), (3, 17))):
        struct.pack_into('<IBxH', flags, 48 + 8 * i, 1000 + i, kind, sub)
    array(defaults, 8, 64, 9, 0x8080170F)
    for i in range(9):
        at = 64 + 176 * i
        defaults[at:at + 2] = bytes(divmod(i, 3))
        first = 1680 + 40 * i
        array(defaults, at + 0x30, first, 3, 0x8080185C)
        for n, value in enumerate((2, 1, i % 3)):
            struct.pack_into('<hBB', defaults, first + 4 * n, n, value, 0)
    return {0x80A58001: root, 101: defaults, 102: flags}


class CreationDefaults(unittest.TestCase):
    def document(self, entries=None):
        entries = entries or synthetic_entries()
        return creation_defaults(lambda tag, expected: entries[tag])

    def test_all_identities_and_actual_values_are_retained(self):
        d = self.document()
        self.assertEqual(d['build'], BUILD)
        self.assertEqual(len(d['data']), 9)
        self.assertEqual([f['value'] for f in select_defaults(d, 2, 1)], [2, 1, 1])
        self.assertEqual(len(d['sources']), 3)

    def test_creation_is_not_an_empty_or_all_completed_record(self):
        d = self.document()
        b = bytearray([0xA5] * (completions.FIELD11 + completions.FIELD11_BYTES + 5))
        b[8], b[0xA] = 1, 2
        before = bytes(b)
        completions.apply_creation_defaults(b, d)
        self.assertEqual(completions.subs(b), {14: 1, 17: 1})
        self.assertEqual(b[:completions.FIELD11], before[:completions.FIELD11])
        self.assertEqual(b[-5:], before[-5:])

    def test_duplicate_identity_or_corrupt_array_is_refused(self):
        e = synthetic_entries()
        e[101][64 + 176:64 + 178] = b'\0\0'
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            self.document(e)
        e = synthetic_entries()
        struct.pack_into('<I', e[101], 64 - 8, 0x1234)
        with self.assertRaisesRegex(ValueError, 'class'):
            self.document(e)
        e = synthetic_entries()
        struct.pack_into('<q', e[101], 64 + 0x38, 999999)
        with self.assertRaisesRegex(ValueError, 'outside'):
            self.document(e)

    def test_other_build_and_invalid_sub_are_refused_before_editing(self):
        d = self.document()
        b = bytearray(completions.FIELD11 + completions.FIELD11_BYTES)
        d['build'] = 'another build'
        with self.assertRaisesRegex(ValueError, '56578'):
            completions.apply_creation_defaults(b, d)
        d['build'] = BUILD
        d['data'][0]['flags'][1]['sub'] = completions.FIELD11_BYTES
        before = bytes(b)
        with self.assertRaisesRegex(ValueError, 'invalid'):
            completions.apply_creation_defaults(b, d)
        self.assertEqual(b, before)


class OwnedCreationDefaults(unittest.TestCase):
    def test_owned_export_matches_the_committed_numeric_facts(self):
        if not os.environ.get('D1_EBOOT') or not os.environ.get('D1_REPO'):
            self.skipTest('set D1_EBOOT and D1_REPO for the owned 56578 export comparison')
        self.assertTrue(Path(os.environ['D1_EBOOT']).is_file())
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / 'defaults.json'
            subprocess.run([sys.executable, '-I', str(TOOLS / 'd1-new-guardian-defaults.py'),
                            '--out', str(out)], check=True, capture_output=True, text=True)
            expected = json.loads((REPO / 'ps4/fixtures/56578/characters/new-guardian-defaults.json').read_text())
            self.assertEqual(json.loads(out.read_text()), expected)
            self.assertTrue(all(row['flags'] for row in expected['data']))


if __name__ == '__main__':
    unittest.main()
