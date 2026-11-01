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
            self.assertEqual(desired(rows[('0x1', i)][1]), (1, 2))
        for i in M.OPENING + (21,):
            self.assertEqual(desired(rows[('0x1', i)][1]), (0, 2), i)
        self.assertEqual(rows[('0x1d', 108)][1], M.UNARMED)
        self.assertEqual(rows[('0x1d', 110)][1], M.UNARMED)
        self.assertEqual(rows[('0x1d', 111)][1], M.ARMED)
        self.assertNotIn(('0x1', 72), rows)

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
