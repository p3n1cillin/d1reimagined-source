"""Wire regression for the opt-in inactive bubble-roster reset experiment."""

import json
import tempfile
import unittest
from pathlib import Path

import bap_server as bap


def field(offset, kind, width=0, *, nested=0xFFFFFFFF, optional=0, minimum=0):
    return {"off": offset, "kind": kind, "width": width, "nested": nested,
            "opt": optional, "min": minimum, "arr": 0}


SCHEMA = {
    "0x80800cf7": {"size": 0x310, "fields": [
        field(0, 5, 32, optional=1, minimum=0x80000000),
        field(4, 1, nested=0x8080540E),
    ]},
    "0x8080540e": {"size": 0x30C, "fields": [
        field(12 * slot, 1, nested=0x80800B83) for slot in range(65)
    ]},
    "0x80800b83": {"size": 12, "fields": [
        field(0, 10, 64, optional=1), field(8, 8, 16, optional=1), field(0xA, 2),
    ]},
}


def decode_roster_packet(payload):
    reader = bap.investment.BitReader(payload)
    epoch = reader.get(8)
    present = reader.get(1)
    changed = [slot for slot in range(65) if reader.get(1)]
    delta_present = reader.get(1)
    roster = bap.investment.decode_from(reader, SCHEMA, bap.ROSTER_TYPE)
    tick = reader.get(32)
    objects_present = reader.get(1)
    return epoch, present, changed, delta_present, roster, tick, objects_present


class BubbleResetWire(unittest.TestCase):
    def test_only_requested_inactive_slots_and_client_slot_change(self):
        resets = bap.bubble_reset_spec("14,0x20:0xffff,64:7")
        packet = bap.sim_packet(SCHEMA, 0x123456789ABCDEF0, 0, 2, epoch=3,
                                tick=0x10203040, bubble_resets=resets)
        epoch, present, changed, delta_present, roster, tick, objects = decode_roster_packet(packet)
        self.assertEqual((epoch, present, changed, delta_present, tick, objects),
                         (3, 1, [0, 14, 32, 64], 1, 0x10203040, 0))
        self.assertEqual(roster[4][0], {0: 0x123456789ABCDEF0, 8: 2, 0xA: True})
        for slot, generation in ((14, 1), (32, 65535), (64, 7)):
            self.assertEqual(roster[4][12 * slot], {8: generation, 0xA: False})
            self.assertNotIn(0, roster[4][12 * slot])  # no peer is assigned to a reset slot
        self.assertEqual(roster[4][12 * 13], {0xA: False})

    def test_heartbeat_retains_generation_while_clock_advances(self):
        packets = [bap.sim_packet(SCHEMA, 5, tick=tick, bubble_resets=((14, 4),))
                   for tick in (100, 130, 160)]
        decoded = [decode_roster_packet(packet) for packet in packets]
        self.assertEqual([entry[5] for entry in decoded], [100, 130, 160])
        self.assertTrue(all(entry[4][4][168] == {8: 4, 0xA: False} for entry in decoded))
        self.assertTrue(all(entry[2] == [0, 14] for entry in decoded))

    def test_default_wire_contains_only_the_client_roster_slot(self):
        decoded = decode_roster_packet(bap.sim_packet(SCHEMA, 9, slot=2, generation=1))
        self.assertEqual(decoded[2], [2])
        self.assertEqual(decoded[4][4][24], {0: 9, 8: 1, 0xA: True})
        self.assertEqual(decoded[4][4][168], {0xA: False})

    def test_invalid_and_colliding_slots_are_rejected(self):
        for text in ("", "14,14:2", "-1", "65", "14:0", "14:65536", "14:1:2"):
            with self.subTest(text=text), self.assertRaises(ValueError):
                bap.bubble_reset_spec(text)
        for resets in (((0, 1),), ((14, 1), (14, 2)), ((14, 0),), ((65, 1),)):
            with self.subTest(resets=resets), self.assertRaises(ValueError):
                bap.sim_packet(SCHEMA, 9, bubble_resets=resets)


class BubbleResetControl(unittest.TestCase):
    def test_complete_lines_parse_independently_without_changing_object_spec_api(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "control.jsonl"
            lines = [json.dumps({"bubble_reset": "14:2,3:7"}),
                     json.dumps({"bubble_reset": "14:2,14:3"}),
                     json.dumps({"bubble_reset": [14]}),
                     json.dumps({"teleport": 112})]
            path.write_text("\n".join(lines) + '\n{"bubble_reset":"14:3"}', encoding="utf-8")
            specs, teleports, offset, errors = bap.control_steps(path, 0, {}, {})
            self.assertEqual(specs, [])
            self.assertEqual(specs.bubble_resets, [((14, 2), (3, 7))])
            self.assertEqual(teleports, [(112, None)])
            self.assertEqual(len(errors), 2)
            with path.open("a", encoding="utf-8") as stream:
                stream.write("\n")
            more, _, offset, errors = bap.control_steps(path, offset, {}, {})
            self.assertEqual(more.bubble_resets, [((14, 3),)])
            self.assertEqual(errors, [])
            self.assertEqual(bap.control_steps(path, offset, {}, {})[0].bubble_resets, [])

    def test_update_retains_other_slots_and_rejects_client_collision_atomically(self):
        startup = ((14, 1), (3, 1))
        one = bap.update_bubble_resets(startup, ((14, 2),), 0)
        self.assertEqual(dict(one), {14: 2, 3: 1})
        self.assertEqual(startup, ((14, 1), (3, 1)))
        with self.assertRaisesRegex(ValueError, "collides"):
            bap.update_bubble_resets(one, ((14, 3), (0, 2)), 0)
        self.assertEqual(dict(one), {14: 2, 3: 1})
        # Another connection using the same startup options still begins at generation 1.
        self.assertEqual(dict(bap.update_bubble_resets(startup, ((3, 4),), 0)), {14: 1, 3: 4})

    def test_encrypted_session_control_preserves_generations_across_heartbeats(self):
        from test_ah_session import ActivityHostSession, PEER

        fixture = ActivityHostSession()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        path = Path(fixture.tmp.name) / "control.jsonl"
        path.write_text('{"bubble_reset":"14:9"}\n', encoding="utf-8")
        client = fixture.start("--ah-heartbeat", "0.025:-1", "--ah-sim-clock", "100",
                               "--ah-bubble-reset", "14:1,3:7", "--ah-control", str(path))

        def rosters(pushes):
            result = []
            for _, subtype, payload in pushes:
                if subtype != bap.AH_SIMULATION:
                    continue
                reader = bap.investment.BitReader(payload)
                reader.get(8 + 1)
                changed = [slot for slot in range(65) if reader.get(1)]
                reader.get(1)
                roster = bap.investment.decode_from(reader, fixture.schema, bap.ROSTER_TYPE)
                result.append((changed, roster[4], reader.get(32)))
            return result

        # Windows CI (PR #342) once read no push at all in a 0.14 s window; the windows are a
        # minimum wait for the next heartbeat push, so longer ones only cost time.
        before = rosters(fixture.run_client(client, 0.5)[1])
        self.assertTrue(before)
        self.assertTrue(all(roster[168][8] == 1 for _, roster, _ in before))  # stale startup line skipped
        with path.open("a", encoding="utf-8") as stream:
            stream.write('{"bubble_reset":"14:2"}\n')
        after = rosters(fixture.run_client(client, 0.6)[1])
        updated = [entry for entry in after if entry[1][168].get(8) == 2]
        self.assertGreaterEqual(len(updated), 2)
        for changed, roster, _ in updated:
            self.assertEqual(changed, [0, 3, 14])
            self.assertEqual(roster[0], {0: PEER, 8: 1, 0xA: True})
            self.assertEqual(roster[36], {8: 7, 0xA: False})
            self.assertEqual(roster[168], {8: 2, 0xA: False})
        self.assertGreater(updated[-1][2], updated[0][2])
        with path.open("a", encoding="utf-8") as stream:
            stream.write('{"bubble_reset":"14:3,0:2"}\n')
        rejected = rosters(fixture.run_client(client, 0.5)[1])
        self.assertTrue(rejected)
        self.assertTrue(all(roster[168].get(8) == 2 and roster[0][0xA]
                            for _, roster, _ in rejected))
        self.assertTrue(any("ignored bubble reset" in line for line in fixture.logs))


class MissionBubbleReset(unittest.TestCase):
    OBJECTS = {"trig": "0x10:3:2:0x80809002"}

    def test_reactive_step_expands_to_a_bubble_reset_reaction(self):
        from test_ah_session import schema
        mission = {"objects": self.OBJECTS,
                   "steps": [{"on": {"type": 17, "object": "trig"}, "delay": 2, "bubble_reset": "3:4"}]}
        timed, reactive = bap.expand_mission(mission, schema())
        self.assertEqual(timed, [])
        self.assertEqual(len(reactive), 1)
        self.assertEqual(reactive[0][2], 2.0)
        self.assertEqual(reactive[0][3], bap.BubbleResetReaction(((3, 4),)))
        bap.validate_mission_states(schema(), timed, reactive)

    def test_timed_combined_and_malformed_resets_are_rejected(self):
        from test_ah_session import schema
        for step in ({"at": 5, "bubble_reset": "3:1"},
                     {"on": {"type": 17, "object": "trig"}, "bubble_reset": "3:1",
                      "set": {"trig": {"0": 1}}},
                     {"on": {"type": 17, "object": "trig"}, "bubble_reset": "3:1", "teleport": 24},
                     {"on": {"type": 17, "object": "trig"}, "bubble_reset": "99:1"},
                     {"on": {"type": 17, "object": "trig"}, "bubble_reset": 3}):
            with self.subTest(step=step), self.assertRaises(ValueError):
                bap.expand_mission({"objects": self.OBJECTS, "steps": [step]}, schema())

    def test_shipped_mission_resets_bubble_three_after_the_breach_member_state(self):
        mission = json.loads((Path(__file__).resolve().parents[1] / "missions" /
                              "a_guardian_rises.json").read_text(encoding="utf-8"))
        steps = [s for s in mission["steps"] if "bubble_reset" in s]
        self.assertEqual(len(steps), 1)
        self.assertEqual(steps[0]["on"], {"type": 21, "slice_set": 24})
        self.assertEqual(bap.bubble_reset_spec(steps[0]["bubble_reset"]), ((3, 1),))
        member = [s for s in mission["steps"] if "sq_nux_ghost__ghost" in s.get("set", {})]
        self.assertEqual(len(member), 1)
        self.assertGreater(steps[0]["delay"], member[0]["delay"])

    def test_encrypted_session_applies_the_reset_when_the_trigger_reports(self):
        from test_ah_session import ActivityHostSession, PEER

        fixture = ActivityHostSession()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        path = Path(fixture.tmp.name) / "mission.json"
        path.write_text(json.dumps({"objects": self.OBJECTS, "steps": [
            {"on": {"type": 17, "object": "trig"}, "bubble_reset": "3:5"}]}), encoding="utf-8")
        client = fixture.start("--ah-heartbeat", "0.025:-1", "--ah-sim-clock", "100",
                               "--ah-baselines", "0x10/3/0,0x10/3/1,0x10/3/2",
                               "--ah-bubble-reset", "14:1", "--ah-mission", str(path))

        def rosters(pushes):
            result = []
            for _, subtype, payload in pushes:
                if subtype != bap.AH_SIMULATION:
                    continue
                reader = bap.investment.BitReader(payload)
                reader.get(8 + 1)
                changed = [slot for slot in range(65) if reader.get(1)]
                reader.get(1)
                result.append((changed, bap.investment.decode_from(reader, fixture.schema, bap.ROSTER_TYPE)[4]))
            return result

        before = rosters(fixture.run_client(client, 0.2)[1])
        self.assertTrue(before)
        self.assertTrue(all(changed == [0, 14] for changed, _ in before))
        w = bap.investment.BitWriter()
        w.put(1, 3)
        bap.investment.encode_into(w, fixture.schema, bap.OBJECT_ID_TYPE, {0: 0x10, 4: 3, 6: 2})
        client.notify(17, w.data())
        after = rosters(fixture.run_client(client, 0.3)[1])
        updated = [roster for changed, roster in after if changed == [0, 3, 14]]
        self.assertGreaterEqual(len(updated), 2)  # heartbeats keep the new generation
        for roster in updated:
            self.assertEqual(roster[0], {0: PEER, 8: 1, 0xA: True})
            self.assertEqual(roster[36], {8: 5, 0xA: False})
            self.assertEqual(roster[168], {8: 1, 0xA: False})
        self.assertTrue(any("mission: simulation bubble reset" in line for line in fixture.logs))


if __name__ == "__main__":
    unittest.main()
