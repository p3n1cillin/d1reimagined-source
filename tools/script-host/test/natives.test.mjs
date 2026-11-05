// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// Missing natives: a call nothing answers is counted and logged, a 56578
// stub logs every call, and the static sweeps find both before a run.

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { findBullshcript, loadBullshcript } from "../src/bullshcript.mjs";
import { IncidentTable } from "../src/incidents.mjs";
import { installNatives56578, STUB_VERBS, staticMissingNatives, unansweredByName } from "../src/natives56578.mjs";
import { ScriptSet } from "../src/scripts.mjs";
import { ActivitySession } from "../src/session.mjs";
import { SYNTHETIC_FILES } from "./synthetic.mjs";

const skip = findBullshcript() ? false : "no built BullShcript checkout (set D1_BULLSHCRIPT_DIR)";

describe("56578 natives", { skip }, () => {
  it("logs the stub and counts the unanswered verb at run time", async () => {
    const bs = await loadBullshcript();
    const scripts = new ScriptSet(bs.api, SYNTHETIC_FILES);
    const s = await ActivitySession.create({
      activity: "test_odd", bs, emit: () => undefined, incidents: new IncidentTable(), scripts,
    });
    const out = s.handle({ type: "start" });
    s.handle({ key: "1", type: "player_spawned" });
    for (let t = 1; t <= 5; t++) out.push(...s.handle({ now: t, type: "tick" }));
    assert.ok(out[0].stubs.includes("import_only_clear_slice_set_assignment"));
    const stubLog = out.find((m) => m.type === "log" && m.stub);
    assert.equal(stubLog.stub, "import_only_clear_slice_set_assignment");
    assert.match(stubLog.text, /^stub56578 tv_odd:import_only_clear_slice_set_assignment -> true/);
    const missing = out.find((m) => m.type === "log" && m.missing);
    assert.equal(missing.missing, "frobnicate_for_test");
    assert.deepEqual(s.missingNatives().map((m) => m.method), ["frobnicate_for_test"]);
    assert.deepEqual(s.stubCalls(), [{ calls: 1, method: "import_only_clear_slice_set_assignment" }]);
    // the script carries on past both: the lifetime goes active
    assert.ok(out.some((m) => m.type === "rows" && m.changes.some((c) => c.kind === "lifetime" && c.wire.phase === 3)));
  });

  it("finds both statically, before any run", async () => {
    const bs = await loadBullshcript();
    const scripts = new ScriptSet(bs.api, SYNTHETIC_FILES);
    const paths = ["es/testland/test_odd/test_odd.es"];
    const typed = staticMissingNatives(bs, scripts, paths);
    assert.deepEqual(
      typed.map((m) => [m.method, m.types, m.stubbed]),
      [
        ["frobnicate_for_test", ["squad"], false],
        ["import_only_clear_slice_set_assignment", ["trigger_volume"], true],
      ]
    );
    const byName = unansweredByName(bs, scripts, paths).map((m) => m.method).sort();
    assert.deepEqual(byName, ["frobnicate_for_test", "import_only_clear_slice_set_assignment"]);
    // nothing in the cinematic activity is unanswered
    assert.deepEqual(unansweredByName(bs, scripts, scripts.closure(["es/testland/test_cine/test_cine.es"])), []);
  });

  it("never replaces a verb the table already has", () => {
    const registered = [];
    const table = { lookup: () => () => "theirs", verb: (name) => registered.push(name) };
    const names = installNatives56578({ nativeRegistry: table }, () => undefined);
    assert.deepEqual([...names], []);
    assert.deepEqual(registered, []);
    const empty = { lookup: () => undefined, verb: (name) => registered.push(name) };
    assert.deepEqual([...installNatives56578({ nativeRegistry: empty }, () => undefined)], Object.keys(STUB_VERBS));
  });
});
