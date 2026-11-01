"""The trigger-driven Breach lists (tools/ps4_setup/d1-breach-waves.py) against the real base list."""
import importlib.util
from pathlib import Path
import struct
import unittest

ROOT = Path(__file__).parents[1]
SPEC = importlib.util.spec_from_file_location('breach_waves', ROOT / 'tools/ps4_setup/d1-breach-waves.py')
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)
BASE = (ROOT / 'ps4/fixtures/56578/mission/agr-b-graveyard.txt').read_text().splitlines(keepends=True)


def breach_rows(text):
    """{(kind, index): (tag, hex)} for the Breach rows that carry a state."""
    rows = {}
    for line in text.splitlines():
        p = line.split()
        if len(p) >= 7 and p[0].lower() == M.BREACH:
            rows[(p[1].lower(), int(p[2]))] = (p[5], p[6])
    return rows


def desired(hex_):
    b = bytes.fromhex(hex_)
    return struct.unpack_from('<h', b, 0x30)[0], struct.unpack_from('<I', b, 0x44)[0]


def entries(hex_):
    return struct.unpack_from('<I', bytes.fromhex(hex_), 0x2C)[0]


def command(hex_):
    """(cookie, pc, count, first atom kind, its end kind, its name hash) of a member A image."""
    b = bytes.fromhex(hex_)
    return struct.unpack_from('<III', b, 0xB0) + struct.unpack_from('<II', b, 0xC0) + (struct.unpack_from('<I', b, 0xD0)[0],)


class BreachWaves(unittest.TestCase):
    def setUp(self):
        self.lists = dict(M.build(BASE, gen=2, count=1))

    def test_one_list_per_trigger_plus_the_armed_one(self):
        self.assertEqual(list(self.lists), ['agr-bw-armed.txt'] + [f'agr-bw-{t}.txt' for t, _ in M.WAVES])

    def test_the_armed_list_arms_every_wave_and_places_only_the_opening(self):
        rows = breach_rows(self.lists['agr-bw-armed.txt'])
        for t, _ in M.WAVES:
            self.assertEqual(rows[('0x1d', t)], ('0x80800463', M.ARMED))
        live = sorted(i for (k, i), (_, h) in rows.items() if k == '0x1' and desired(h)[0] > 0)
        self.assertEqual(live, sorted(M.OPENING))

    def test_a_wave_places_its_squads_and_stops_the_earlier_ones_refilling(self):
        rows = breach_rows(self.lists['agr-bw-110.txt'])
        for i in (24, 38):
            self.assertEqual(desired(rows[('0x1', i)][1]), (0, 2))
            self.assertEqual(entries(rows[('0x1', i)][1]), 1, i)  # an explicit zero, not the default
        for i in M.OPENING + (21,):
            self.assertEqual(desired(rows[('0x1', i)][1]), (0, 2), i)
        self.assertEqual(rows[('0x1d', 108)][1], M.UNARMED)
        self.assertEqual(rows[('0x1d', 110)][1], M.UNARMED)
        self.assertEqual(rows[('0x1d', 111)][1], M.ARMED)
        self.assertNotIn(('0x1', 72), rows)

    def test_names_hash_with_fnv1(self):
        self.assertEqual(M.member_states.fnv1('enc_breach_m0'), int(M.BREACH, 16))  # the bundle's group
        self.assertEqual(M.member_states.action_hash('0x2AC3BDBA'), M.member_states.fnv1('act_fallen_vault_right'))

    def test_a_wave_enters_its_named_members(self):
        rows = breach_rows(self.lists['agr-bw-110.txt'])
        for sub, action in ((25, 'act_fallen_intro_vandal_drop'), (39, 'act_fallen_vault_right')):
            tag, hex_ = rows[('0x2', sub)]
            self.assertEqual(tag, M.member_states.TAG)
            self.assertEqual(command(hex_), (2, 0, 1, 1, 0, M.member_states.fnv1(action)))
            # the optional binding stays absent (a present one is looked up unguarded at 0x1BDCC1)
            self.assertEqual(bytes.fromhex(hex_)[0x50:0x5C].hex(), 'c59d1c81ff00ffffffffffff')
        self.assertNotIn(('0x2', 43), rows)  # dropdown e's member waits for trigger 111

    def test_entered_members_and_their_squads_carry_into_later_lists(self):
        first = breach_rows(self.lists['agr-bw-110.txt'])
        last = breach_rows(self.lists['agr-bw-107.txt'])
        for key in (('0x1', 38), ('0x2', 39), ('0x1', 24), ('0x2', 25)):
            self.assertEqual(first[key], last[key], key)

    def test_every_entrance_member_is_a_member_row_of_the_base(self):
        rows = {(p[1].lower(), int(p[2])) for p in (l.split() for l in BASE) if len(p) >= 5 and p[0].lower() == M.BREACH}
        placed = {s for _, squads in M.WAVES for s in squads}
        for squad, entered in M.ENTRANCES.items():
            self.assertIn(squad, placed)
            for sub, _ in entered:
                self.assertIn(('0x2', sub), rows)
        self.assertNotIn(61, {m for e in M.ENTRANCES.values() for m, _ in e})  # 2__dreg_c is never entered

    def test_no_entrances_keeps_the_squads_only_lists(self):
        rows = breach_rows(dict(M.build(BASE, gen=2, count=1, entrances=False))['agr-bw-110.txt'])
        for i in (24, 38):
            self.assertEqual(desired(rows[('0x1', i)][1]), (1, 2))
        self.assertFalse([k for k in rows if k[0] == '0x2'])

    def test_member_counts_squads_place_their_script_bodies(self):
        rows = breach_rows(self.lists['agr-bw-107.txt'])
        for i, bodies in M.MEMBER_COUNTS.items():
            self.assertEqual(desired(rows[('0x1', i)][1])[1], 2, i)
        self.assertEqual(desired(breach_rows(self.lists['agr-bw-116.txt'])[('0x1', 55)][1]), (2, 2))
        self.assertEqual(desired(rows[('0x1', 76)][1]), (3, 2))

    def test_every_squad_placed_in_the_last_list_has_one_entry(self):
        # No entry (+0x2C = 0) makes the client place its default again: a spent squad must not refill.
        rows = breach_rows(self.lists['agr-bw-107.txt'])
        for i in {s for _, squads in M.WAVES for s in squads}:
            self.assertEqual(entries(rows[('0x1', i)][1]), 1, i)
            if i != 76:  # the last wave's own squad is still live
                self.assertEqual(desired(rows[('0x1', i)][1]), (0, 2), i)

    def test_lists_change_only_breach_rows(self):
        for text in self.lists.values():
            got = text.splitlines()
            self.assertEqual(len(got), len(BASE))
            for a, b in zip(BASE, got):
                if a.split()[:1] != [M.BREACH]:
                    self.assertEqual(a.rstrip('\n'), b)

    def test_the_steps_wait_on_each_trigger_in_route_order(self):
        lines = M.steps(300).splitlines()
        self.assertEqual(lines[0], '164 +2 0x4786c0e0 agr-bw-armed.txt')
        self.assertEqual(lines[1], '164 ^0xfc0e45a3/0x1d/108|300 0x4786c0e0 agr-bw-108.txt')
        self.assertEqual(len(lines), len(M.WAVES) + 1)


if __name__ == '__main__':
    unittest.main()
