"""The Breach Ghost's member and performance states (tools/ps4_setup/d1-ghost-states.py)."""
import importlib.util
from pathlib import Path
import struct
import unittest

ROOT = Path(__file__).parents[1]
SPEC = importlib.util.spec_from_file_location('ghost_states', ROOT / 'tools/ps4_setup/d1-ghost-states.py')
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)
LIST = (ROOT / 'ps4/fixtures/56578/mission/agr-b-lights.txt').read_text().splitlines(keepends=True)


class GhostStates(unittest.TestCase):
    def test_the_member_image_is_d1_member_states_with_the_ps3_test_fields(self):
        b = M.member(1)
        self.assertEqual(b, M.member_states.member(1))
        self.assertEqual(len(b), 2312)                                  # d1-eboot descriptors 0x80800868
        self.assertEqual(struct.unpack_from('<Ibb?', b, 0), (1, -1, 0, True))
        # the binding stays absent: index -1 at +0x58, never zero (an unguarded lookup crashed on one)
        self.assertEqual(b[0x50:0x5C].hex(), 'c59d1c81ff00ffffffffffff')

    def test_the_performance_image_carries_the_fly_to_the_gun_action(self):
        b = M.performance(3)
        self.assertEqual(len(b), 48)                                    # 0x80800556
        self.assertEqual(struct.unpack_from('<IIi', b, 0), (0x9B1D6A71, 0x811C9DC5, 3))
        self.assertEqual(b[0xC:], bytes(36))

    def test_both_rows_get_their_states_and_nothing_else_changes(self):
        out = M.apply(LIST, {('0x2', 95): (M.MEMBER_TAG, M.member(1)),
                             ('0x28', 125): (M.PERFORM_TAG, M.performance(1))})
        changed = [(a.split()[:3], b.split()[5]) for a, b in zip(LIST, out) if a != b]
        self.assertEqual(changed, [(['0xfc0e45a3', '0x2', '95'], M.MEMBER_TAG),
                                   (['0xfc0e45a3', '0x28', '125'], M.PERFORM_TAG)])

    def test_a_row_missing_from_the_list_is_refused(self):
        with self.assertRaises(SystemExit):
            M.apply(LIST, {('0x2', 9999): (M.MEMBER_TAG, M.member(1))})


if __name__ == '__main__':
    unittest.main()
