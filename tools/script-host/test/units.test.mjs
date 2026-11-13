// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// The pieces that need no runtime: the lifetime mapping, the incident
// catalogue, the unanswered-native log parser.

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { IncidentTable, KNOWN_56578 } from "../src/incidents.mjs";
import { missingNativeOf } from "../src/natives56578.mjs";
import { scriptsDirFromEnv } from "../src/scripts.mjs";
import { LIFETIME_WIRE_PHASE, lifetimeWirePhase, types56578 } from "../src/types56578.mjs";
import { SYNTHETIC_TSV } from "./synthetic.mjs";

describe("lifetime wire phase", () => {
  it("maps the runtime's states to 56578's decoded values (#304)", () => {
    assert.equal(lifetimeWirePhase(0), 1);
    assert.equal(lifetimeWirePhase(2), 3);
    assert.equal(lifetimeWirePhase(3), 4);
    assert.equal(lifetimeWirePhase(5), 6);
    assert.equal(lifetimeWirePhase(6), 8);
  });
  it("says null for a state with no evidence", () => {
    assert.equal(lifetimeWirePhase(4), null);
    assert.equal(lifetimeWirePhase(7), null);
    assert.ok(Object.isFrozen(LIFETIME_WIRE_PHASE));
  });
  it("adds 56578's type 39 to 01.29's table without changing the rest", () => {
    const t = types56578({ Lifetime: 16, Squad: 1 });
    assert.deepEqual(t, { Lifetime: 16, NewUserExperience: 39, Squad: 1 });
  });
});

describe("incident catalogue", () => {
  it("knows the four ids resolved from the 56578 registry without a file", () => {
    const t = new IncidentTable();
    assert.equal(t.idOf("activity_complete"), 965);
    assert.equal(t.resolve({ id: 891 }).name, "cinematic_finished");
    assert.equal(t.resolve({ name_hash: "0x7352dafe" }).id, 1726);
    assert.equal(Object.keys(KNOWN_56578).length, 4);
  });
  it("parses Codex's TSV layout (#306) both ways", () => {
    const t = IncidentTable.parse(SYNTHETIC_TSV);
    assert.equal(t.resolve({ id: 1200 }).hash, 0xd4a3f1b2);
    assert.equal(t.resolve({ name: "test_pickup" }).id, 1200);
    assert.equal(t.resolve({ name_hash: 0x12345678 }).id, 7);
    assert.equal(t.resolve({ id: 7 }).name, undefined, "uncracked names stay unnamed");
    assert.equal(t.resolve({ id: 5 }), undefined);
  });
  it("hashes a name it has no row for, when given fnv1", () => {
    const t = new IncidentTable();
    const got = t.resolve({ name: "x" }, () => 0xabc);
    assert.deepEqual(got, { hash: 0xabc, id: undefined, name: "x" });
  });
  it("refuses a file that is not a catalogue", () => {
    assert.throws(() => IncidentTable.parse("a\tb\n1\t2\n"), /not an incident catalogue/);
  });
});

describe("unanswered natives in the runtime log", () => {
  it("reads BullShcript's unbound and unknown notes", () => {
    assert.deepEqual(missingNativeOf("lua phase.sq_x:frob(1, 2) (unbound)"), {
      method: "frob",
      on: "phase.sq_x",
      why: "unbound",
    });
    assert.equal(missingNativeOf("lua a.b:is_x() (unknown, false)").method, "is_x");
    assert.equal(missingNativeOf("lua lifetime_timer:get_y() (unbound, 0)").method, "get_y");
    assert.equal(missingNativeOf("lua thread main start"), undefined);
  });
});

describe("script folder per activity", () => {
  const env = { D1_SCRIPTS_DIR: "base", D1_SCRIPTS_DIR_FOR: " act_a = other/a ;act_b=;bad" };
  it("names another folder for one activity (D1_SCRIPTS_DIR_FOR)", () => {
    assert.equal(scriptsDirFromEnv(env, "act_a"), "other/a");
  });
  it("keeps D1_SCRIPTS_DIR for the others, an empty entry and no activity", () => {
    assert.equal(scriptsDirFromEnv(env, "act_c"), "base");
    assert.equal(scriptsDirFromEnv(env, "act_b"), "base");
    assert.equal(scriptsDirFromEnv(env), "base");
    assert.equal(scriptsDirFromEnv({}, "act_a"), undefined);
  });
});
