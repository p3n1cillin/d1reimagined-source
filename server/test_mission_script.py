import copy
import json
import tempfile
import unittest
from pathlib import Path

import bap_server as bap
from mission_script import ReactionQueue, reaction_scope


class MissionTypedValues(unittest.TestCase):
    def setUp(self):
        from test_investment_typed_values import SCHEMA, field

        self.schema = copy.deepcopy(SCHEMA)
        self.schema["0x80800a07"] = {"size": 8, "fields": [field(0, 1, nested=bap.OBJECT_ID_TYPE)]}
        self.objects = {"helper": "0xfc0e45a3:0x18:101:0x8080065c:1"}
        self.reference = {"0": 0xFC0E45A3, "4": 1, "6": 94}
        self.values = {"0": True, "4": 1, "8": self.reference,
                       "0x10": {"type": "0x80800a07", "values": {"0": self.reference}}}
        self.expected = {0: True, 4: 1, 8: {0: 0xFC0E45A3, 4: 1, 6: 94},
                         0x10: {"type": 0x80800A07, "values": {0: {0: 0xFC0E45A3, 4: 1, 6: 94}}}}

    def test_object_spec_retains_wrapper_and_encodes_its_nested_reference(self):
        import investment

        spec = bap.object_spec(self.objects["helper"] + "=" + json.dumps(self.values))
        self.assertEqual(spec[2], self.expected)
        encoded = investment.encode(self.schema, spec[1], spec[2])
        self.assertEqual(investment.decode(self.schema, spec[1], encoded), self.expected)
        self.assertTrue(spec[3])

    def test_timed_and_reactive_mission_states_pass_encoding_preflight(self):
        mission = {"objects": self.objects, "steps": [
            {"at": 0, "set": {"helper": self.values}},
            {"on": {"type": 18, "regex": "^01"}, "set": {"helper": self.values}}]}
        timed, reactive = bap.expand_mission(mission, self.schema)
        self.assertEqual(timed[0][1][2], self.expected)
        self.assertEqual(reactive[0][3][2], self.expected)
        self.assertEqual(bap.validate_mission_states(self.schema, timed, reactive), 2)

    def test_live_control_rejects_named_offsets_and_accepts_the_next_typed_line(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "control.jsonl"
            lines = [{"set": {"helper": {"ordinary_name": 1}}},
                     {"set": {"helper": self.values}}]
            path.write_text("".join(json.dumps(line) + "\n" for line in lines), encoding="utf-8")
            specs, teleports, offset, errors = bap.control_steps(path, 0, self.objects, self.schema)
            self.assertEqual(len(errors), 1)
            self.assertEqual(len(specs), 1)
            self.assertEqual(specs[0][2], self.expected)
            self.assertEqual(teleports, [])
            self.assertEqual(offset, path.stat().st_size)
            self.assertEqual(bap.validate_mission_states(self.schema, [(0, specs[0])], []), 1)

    def test_wrappers_normalize_recursively_in_lists_and_nested_payloads(self):
        wrapper = {"type": "0x102", "values": {
            "0": {"type": 0x80800A07, "values": {"0": self.reference}}}}
        normalized = bap.offset_keys([wrapper])[0]
        self.assertEqual(normalized, {"type": 0x102, "values": {
            0: {"type": 0x80800A07, "values": {0: {0: 0xFC0E45A3, 4: 1, 6: 94}}}}})
        spec = bap.object_spec("1:3:0:0x101=" + json.dumps({"8": wrapper}))
        self.assertEqual(bap.validate_mission_states(self.schema, [(0, spec)], []), 1)

    def test_only_complete_wrappers_with_nonnull_integer_handles_are_accepted(self):
        invalid = [{"ordinary_name": 1}, {"type": 1}, {"values": {}},
                   {"type": 1, "values": {}, "extra": 0},
                   {"type": 1, "values": []}, {"type": 1, "values": {"bad_offset": 0}}]
        invalid += [{"type": handle, "values": {}}
                    for handle in (None, True, 1.5, -1, "0xffffffff", 0x100000000, "unknown")]
        for value in invalid:
            with self.subTest(value=value), self.assertRaises(ValueError):
                bap.offset_keys({"0x10": value})
        self.assertEqual(bap.offset_keys({"type": "0", "values": {}}),
                         {"type": 0, "values": {}})

    def test_unknown_selected_type_still_fails_schema_preflight(self):
        values = {"0x10": {"type": "0xdeadbeef", "values": {}}}
        spec = bap.object_spec(self.objects["helper"] + "=" + json.dumps(values))
        with self.assertRaisesRegex(ValueError, "unknown type handle"):
            bap.validate_mission_states(self.schema, [(0, spec)], [])


class GhostAuthoredMember(unittest.TestCase):
    def test_breach_rule_activates_the_authored_member_without_a_second_count_path(self):
        import investment
        from test_investment_typed_values import SCHEMA, field

        # Final member State A: retain the four absent nested-state wire bits.
        # JOIN109's native gate requires +6 true and +5 outside {1, 3, 4}.
        fields = [field(0, 5, 31, optional=True), field(4, 3, 2, 1),
                  field(5, 3, 3, 1), field(6, 2)]
        fields += [field(off, 1, optional=True, nested=handle) for off, handle in
                   ((8, 0x808014e7), (0x2c, 0x80800836),
                    (0xb0, 0x80800512), (0x8c0, 0x808007a9))]
        schema = {
            "0x808004b1": copy.deepcopy(SCHEMA["0x808004b1"]),
            "0x80800868": {"size": 0x908, "fields": fields},
            "0x8080071e": {"size": 0x7c, "fields": [
                field(0x44, 5, 31, optional=True),
                field(0x74, 3, 2, 1), field(0x75, 3, 3, 1)]},
        }
        mission = json.loads((Path(__file__).resolve().parents[1] / "missions" /
                              "a_guardian_rises.json").read_text(encoding="utf-8"))
        from test_cinematic_completion import add_incident_schema
        _, reactions = bap.expand_mission(mission, add_incident_schema(schema))
        ghost = [rule for rule in reactions if not isinstance(rule[3], (bap.TeleportReaction, bap.BubbleResetReaction, bap.LootDropReaction)) and rule[3][0] ==
                 {0: 0xFC0E45A3, 4: 2, 6: 95}]
        self.assertEqual(len(ghost), 1)
        rule = ghost[0]
        self.assertEqual((rule[0], rule[1].index, rule[2]), (21, 24, 20))
        spec = rule[3]
        self.assertEqual(spec[1], 0x80800868)
        self.assertEqual(spec[2], {0: 1, 4: -1, 5: 0, 6: True})
        self.assertTrue(spec[3])
        self.assertFalse(any("sq_nux_ghost" in step.get("set", {})
                             for step in mission["steps"]))
        self.assertEqual(bap.validate_mission_states(schema, [(0, spec)], []), 1)
        writer = investment.BitWriter()
        investment.encode_into(writer, schema, spec[1], spec[2])
        self.assertEqual(len(writer.bits), 42)
        self.assertEqual(investment.decode(schema, spec[1], writer.data()),
                         spec[2])


class ReactionScopes(unittest.TestCase):
    def test_scopes_require_an_explicit_pair_and_reactive_state_changes(self):
        self.assertIsNone(reaction_scope({"at": 1, "set": {}}))
        step = {"on": {"type": 18}, "set": {"objective": {"0": 5}}}
        self.assertEqual(reaction_scope({**step, "lane": "objectives", "phase": "weapon"}),
                         ("objectives", "weapon"))
        for invalid in ({"lane": "objectives"}, {"phase": "weapon"},
                        {"lane": "", "phase": "weapon"}, {"lane": "objectives", "phase": 1},
                        {"lane": "objectives", "phase": "weapon", "at": 1},
                        {"lane": "objectives", "phase": "weapon", "set": {}}):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                reaction_scope({**step, **invalid})

    def test_new_phase_cancels_only_its_lane_and_keeps_parallel_siblings(self):
        queue = ReactionQueue()
        queue.schedule(10, "old objective", "first", ("objectives", "investigate"))
        queue.schedule(11, "Ghost dialogue", "parallel", ("dialogue", "ghost_light"))
        queue.schedule(12, "unscoped squad", "legacy")
        self.assertEqual(queue.schedule(4, "complete", "next", ("objectives", "weapon")), 1)
        self.assertEqual(queue.schedule(8, "clear/start", "next", ("objectives", "weapon")), 0)
        self.assertEqual(queue.schedule(14, "activate", "next", ("objectives", "weapon")), 0)
        self.assertEqual([spec for _, spec, _ in queue.pop_due(20)],
                         ["Ghost dialogue", "unscoped squad", "complete", "clear/start", "activate"])
        self.assertFalse(queue.has_due(20))

    def test_lane_state_is_per_session(self):
        one, two = ReactionQueue(), ReactionQueue()
        one.schedule(10, "first session", "one", ("objectives", "investigate"))
        two.schedule(10, "second session", "two", ("objectives", "investigate"))
        one.schedule(4, "next phase", "one", ("objectives", "weapon"))
        self.assertEqual([spec for _, spec, _ in one.pop_due(10)], ["next phase"])
        self.assertEqual([spec for _, spec, _ in two.pop_due(10)], ["second session"])


class RapidObjectiveHandover(unittest.TestCase):
    """Successive trigger reports must not reactivate an objective from an earlier phase."""

    def test_guardian_script_rapid_wall_weapon_reports_keep_only_latest_objective_active(self):
        from test_bap_server import MissionScript
        from test_cinematic_completion import add_incident_schema

        mission = json.loads((Path(__file__).resolve().parents[1] / "missions" /
                              "a_guardian_rises.json").read_text(encoding="utf-8"))
        _, reactions = bap.expand_mission(mission, add_incident_schema(MissionScript.SCHEMA))
        queue, states = ReactionQueue(), {}

        def apply(now):
            for _, spec, _ in queue.pop_due(now):
                states[tuple(spec[0][off] for off in (0, 4, 6))] = spec[2]

        def report(now, matches):
            for rule in reactions:
                if matches(rule):
                    _, _, delay, spec = rule
                    queue.schedule(now + delay, spec, "mission", getattr(rule, "scope", None))
            apply(now)

        def object_id(name):
            return tuple(bap.object_spec(mission["objects"][name] + "=")[0][off] for off in (0, 4, 6))

        def trigger(name):
            ident = object_id(name)
            return lambda rule: isinstance(rule[1], bap.ObjectIdMatcher) and rule[1].ident == ident

        # The next volume is reached just after each four-second handover arms its trigger,
        # before the preceding phase's delayed active-state update can run.
        report(0, lambda rule: isinstance(rule[1], bap.SliceSetMatcher) and rule[1].index == 112)
        apply(20)
        report(21, trigger("trig_enter_wall"))
        apply(25)
        report(25.1, trigger("trig_investigate"))
        apply(29.1)
        report(29.2, trigger("trig_get_weapon"))
        apply(40)
        self.assertEqual({name: states[object_id(name)][0] for name in
                          ("obj_enter_wall", "obj_investigate", "obj_get_weapon", "obj_graveyard")},
                         {"obj_enter_wall": 0, "obj_investigate": 0,
                          "obj_get_weapon": 0, "obj_graveyard": 5})

    def test_encrypted_session_rapid_triggers_cancel_stale_activation(self):
        from test_ah_session import ActivityHostSession, objects_in

        fixture = ActivityHostSession()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        mission = {"objects": {name: f"0x10:3:{index}:0x80809002"
                               for index, name in enumerate(("wall", "investigate", "weapon", "fight"))},
                   "steps": []}
        for code, old, new in (("41", "wall", "investigate"),
                               ("42", "investigate", "weapon"), ("43", "weapon", "fight")):
            common = {"on": {"type": 15, "regex": f"^{code}"},
                      "lane": "objectives", "phase": new}
            mission["steps"] += [
                {**common, "set": {old: {"0": 3}}},
                {**common, "delay": 0.15, "set": {old: {"0": 0}, new: {"0": 1}}},
                {**common, "delay": 1.2, "set": {new: {"0": 5}}}]
        path = Path(fixture.tmp.name) / "mission.json"
        path.write_text(json.dumps(mission), encoding="utf-8")
        client = fixture.start("--ah-heartbeat", "0.025:-1", "--ah-mission", str(path),
                               "--ah-baselines", "0x10/3/0,0x10/3/1,0x10/3/2,0x10/3/3")
        pushes = []
        # Each trigger comes 0.6 s after the last: well after the 0.15 s step (which must fire) and well
        # before the 1.2 s one (which the next trigger must cancel). The last waits 3 s for its 1.2 s
        # step: 1.5 s was not enough on a Windows runner (#191). At 0.21 s, a slow Windows runner
        # sometimes hadn't fired the 0.15 s step yet, and the final state kept the wall at 3.
        for report, wait in ((b"A", 0.6), (b"B", 0.6), (b"C", 3.0)):
            client.notify(15, report)
            pushes += fixture.run_client(client, wait)[1]
        states = [objects_in(fixture.schema, data) for _, subtype, data in pushes
                  if subtype == bap.AH_SIMULATION]
        self.assertTrue(states)
        self.assertEqual(states[-1], {(0x10, 3, 0): {0: 0}, (0x10, 3, 1): {0: 0},
                                      (0x10, 3, 2): {0: 0}, (0x10, 3, 3): {0: 5}})
        self.assertFalse(any(state[(0x10, 3, 1)] == {0: 5} or state[(0x10, 3, 2)] == {0: 5}
                             for state in states))
        self.assertTrue(any("cancelled" in line for line in fixture.logs))


if __name__ == "__main__":
    unittest.main()


class BreachDialogueRules(unittest.TestCase):
    """JOIN190: host-stepped dialogue sequences and the Fallen-above trigger."""

    def setUp(self):
        self.mission = json.loads((Path(__file__).resolve().parents[1] / "missions" /
                                   "a_guardian_rises.json").read_text(encoding="utf-8"))

    def steps_setting(self, name):
        return [s for s in self.mission["steps"] if name in s.get("set", {})]

    def test_right_above_us_line_plays_on_entering_the_breach(self):
        (step,) = self.steps_setting("seq_d_o2_20")
        self.assertEqual(step["on"], {"type": 21, "slice_set": 24})
        self.assertLessEqual(step["delay"], 5)
        self.assertEqual(step["set"]["seq_d_o2_20"], {"0": "$now", "4": 1})

    def test_light_up_chain_follows_the_reference_order(self):
        # JOIN205-207: reference 07:50-08:17 order from the light-up start trigger.
        armed = [s for s in self.steps_setting("trig_ghostlight")
                 if s["set"]["trig_ghostlight"].get("0") is True]
        (arm,) = armed
        (reset,) = [s for s in self.mission["steps"] if "bubble_reset" in s]
        self.assertEqual(arm["on"], {"type": 21, "slice_set": 24})
        self.assertGreater(arm["delay"], reset["delay"])
        chain = [s for s in self.mission["steps"]
                 if s.get("on") == {"type": 18, "object": "trig_ghostlight"}]
        order = sorted((s.get("delay", 0), name) for s in chain for name in s.get("set", {}))
        names = [name for _, name in order]
        self.assertEqual(names[:2], ["seq_d_o2_40", "trig_ghostlight"])
        self.assertLess(names.index("seq_d_o2_50"), names.index("o_breach_doors_lights"))
        self.assertLess(names.index("o_breach_gating_lights"), names.index("seq_d_o2_70"))
        self.assertLess(names.index("seq_d_o2_70"), names.index("seq_d_o2_80"))
        self.assertIn("o_weapon", names)
        self.assertNotIn("o_chest_primary_weapon", names)
        self.assertEqual(self.mission["objects"]["o_weapon"],
                         "0x992bb471:0x4:6:0x808007d9:1")
        (rifle,) = self.steps_setting("o_weapon")
        self.assertEqual(rifle["on"], {"type": 18, "object": "trig_ghostlight"})
        self.assertEqual(rifle["delay"], 30)
        self.assertIn("seq_d_o2_80", rifle["set"])
        # JOIN221-226: the starter rifle is announced as a world-loot drop with the weapon object.
        (drop,) = [s for s in chain if "loot_drop" in s]
        self.assertEqual((drop["loot_drop"], drop["delay"]), ("starter_rifle", rifle["delay"]))
        self.assertEqual(self.mission["loot"]["starter_rifle"],
                         {"item": 3598, "position": [410.644, -794.385, 17.723], "world": 3})
        created = {"0": 1, "4": True, "8": {"0": 0x811C9DC5, "4": -1, "6": -1}}
        props = ("o_breach_control", "o_breach_doors_lights", "o_breach_gating_lights")
        (create,) = [s for s in chain if set(props) <= set(s.get("set", {})) and "16" not in s["set"][props[0]]]
        (activate,) = [s for s in chain if set(props) <= set(s.get("set", {})) and "16" in s["set"][props[0]]]
        self.assertGreater(activate["delay"], create["delay"])
        self.assertLess(activate["delay"], [s["delay"] for s in chain if "seq_d_o2_70" in s.get("set", {})][0])
        # JOIN210: channels 0-2 at (1, 1, 1.0) open the barrier to the rifle.
        channels = {"0": 1, "4": 1, "8": 1.0, "12": 1, "16": 1, "20": 1.0, "24": 1, "28": 1, "32": 1.0}
        for name in props:
            self.assertEqual(create["set"][name], created)
            self.assertEqual(activate["set"][name], {**created, "16": {"0": 1, "16": [
                {"type": "0x8080279a", "values": channels}]}})

    def test_first_fallen_spawn_after_the_rifle_pickup(self):
        # JOIN231: one desired Vandal plus three authored Dreg members create attacking Fallen.
        (step,) = [s for s in self.mission["steps"] if "sq_fallen_first_vandal" in s.get("set", {})]
        self.assertEqual((step["on"], step["delay"]), ({"type": 18, "object": "trig_get_weapon"}, 30))
        self.assertEqual(step["set"]["sq_fallen_first_vandal"],
                         {"0x2c": {"0": 1, "4": [1]}, "0x44": 2, "squad_existing_members": -1})
        member = {"0": 1, "4": -1, "5": 0, "6": True}
        for name in ("sq_fallen_floor_a__dreg", "sq_fallen_floor_b__dreg", "sq_fallen_wall_l_a__dreg"):
            self.assertEqual(step["set"][name], member)
            self.assertTrue(self.mission["objects"][name].startswith("0xfc0e45a3:0x2:"))

    def test_fallen_above_is_armed_at_the_wall_and_disarmed_on_its_report(self):
        armed = [s for s in self.steps_setting("trig_fallen_above")
                 if s["set"]["trig_fallen_above"].get("0") is True]
        self.assertEqual(len(armed), 1)
        self.assertEqual(armed[0]["on"], {"type": 18, "object": "trig_enter_wall"})
        self.assertIn("seq_d_o2_10", armed[0]["set"])
        (report,) = [s for s in self.mission["steps"] if s.get("on", {}).get("object") == "trig_fallen_above"]
        self.assertEqual(report["set"]["trig_fallen_above"], {"0": False})
        self.assertEqual(report["set"]["seq_fallen_above"], {"0": "$now", "4": 1})

    def test_dialogue_objects_use_the_sequence_state_type(self):
        for name in ("seq_d_o2_10", "seq_d_o2_20", "seq_fallen_above"):
            self.assertTrue(self.mission["objects"][name].startswith("0x3c2f5ed5:0x5:"))
            self.assertTrue(self.mission["objects"][name].endswith(":0x808029a1"))
