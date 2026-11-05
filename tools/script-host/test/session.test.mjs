// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// The session on synthetic scripts: start, phases, rows, completion, the
// end; the inputs the protocol takes; and that the same messages replay to
// the same outputs. Skips when no built BullShcript checkout is found.

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { findBullshcript, loadBullshcript } from "../src/bullshcript.mjs";
import { IncidentTable } from "../src/incidents.mjs";
import { ScriptSet } from "../src/scripts.mjs";
import { ActivitySession } from "../src/session.mjs";
import { SYNTHETIC_FILES } from "./synthetic.mjs";

const skip = findBullshcript() ? false : "no built BullShcript checkout (set D1_BULLSHCRIPT_DIR)";

async function session(activity = "test_cine", extra = {}) {
  const bs = await loadBullshcript();
  const scripts = new ScriptSet(bs.api, SYNTHETIC_FILES);
  return ActivitySession.create({
    activity,
    bs,
    emit: () => undefined,
    incidents: new IncidentTable(),
    scripts,
    ...extra,
  });
}

// Ticks until `pred(msgs)` holds for a tick's output; returns every message.
function runUntil(s, pred, max = 200) {
  const all = [];
  for (let i = 0; i < max; i++) {
    const out = s.handle({ now: s.ticks + 1, type: "tick" });
    all.push(...out);
    if (pred(out, all)) return all;
  }
  throw new Error(`condition not met in ${max} ticks`);
}

const changesOf = (msgs, kind) =>
  msgs.filter((m) => m.type === "rows").flatMap((m) => m.changes).filter((c) => !kind || c.kind === kind);
const ref = (c) => ({ bundle: c.item.bundle, type_index: c.item.typeIndex });

describe("ActivitySession on synthetic scripts", { skip }, () => {
  it("loads the activity on stand-in bundles and says what will run", async () => {
    const s = await session();
    const loaded = s.loaded();
    assert.equal(loaded.type, "loaded");
    assert.equal(loaded.stand_in, true);
    assert.equal(loaded.drives_lifetime, true);
    assert.deepEqual(loaded.compile_errors, []);
    assert.deepEqual(
      loaded.phases.map((p) => [p.index, p.name, p.slice]),
      [
        [0, "test_phase_a", "test_plaza"],
        [1, "test_phase_b", "test_yard"],
      ]
    );
    assert.equal(loaded.start.start_slice, "test_plaza");
    assert.equal(loaded.tick_hz, 10);
    assert.deepEqual(
      loaded.bundles.map((b) => b.path),
      ["bs/activities/common/story_common.bs", "es/testland/test_cine/test_cine.es"]
    );
  });

  it("runs both phases to the end: rows, phases, completion, ended", async () => {
    const s = await session();
    const started = s.handle({ type: "start" });
    assert.equal(started[0].type, "ready");
    assert.deepEqual(started[0].unresolved, []);
    const phase0 = started.find((m) => m.type === "phase");
    assert.equal(phase0.index, 0);
    assert.equal(phase0.name, "test_phase_a");
    assert.equal(phase0.name_hash, s.api.fnv1("test_phase_a"));
    assert.equal(phase0.slice_hash, s.api.sliceRegionHash("test_plaza"));

    s.handle({ key: "7", type: "player_spawned" });
    // pregame, then the cutscene plays under the cinematic hold
    let out = runUntil(s, (o) => changesOf(o, "cinematic").length > 0);
    const lifetimes = changesOf(out, "lifetime").map((c) => [c.state.state, c.wire.phase]);
    assert.deepEqual(lifetimes, [
      [0, 1],
      [3, 4],
    ]);
    const cutA = changesOf(out, "cinematic")[0];
    assert.ok(cutA.state.playing);
    assert.equal(cutA.item.typeId, 6);

    // the client finishes it: the next phase starts, the activity completes
    s.handle({ ...ref(cutA), type: "cinematic_finished" });
    out = runUntil(s, (o) => o.some((m) => m.type === "phase"));
    const phase1 = out.find((m) => m.type === "phase");
    assert.equal(phase1.index, 1);
    assert.equal(phase1.slice, "test_yard");
    out.push(...runUntil(s, (o, all) => all.some((m) => m.type === "activity_complete") && changesOf(all, "cinematic").length > 0));
    const complete = out.find((m) => m.type === "activity_complete");
    assert.equal(complete.id, 965);
    assert.equal(complete.name_hash, 0x42660690);
    assert.ok(changesOf(out, "lifetime").some((c) => c.wire.phase === 3), "active is wire 3");
    const cutB = changesOf(out, "cinematic").find((c) => c.state.playing);
    assert.notEqual(cutB.item.typeIndex, cutA.item.typeIndex);

    s.handle({ ...ref(cutB), type: "cinematic_finished" });
    out = runUntil(s, (o, all) => changesOf(all, "playerTrigger").length > 0);
    const squad = changesOf(out, "squad").find((c) => c.state.placedAt !== undefined);
    assert.ok(squad, "the squad is placed");
    assert.deepEqual(squad.state.roster, [2], "two bodies from @member_counts");
    const trigger = changesOf(out, "playerTrigger").at(-1);
    assert.equal(trigger.state.enabled, true);

    s.handle({ ...ref(trigger), type: "trigger" });
    s.handle({ ...ref(squad), count: 2, type: "squad_alive" });
    runUntil(s, () => true, 20);
    s.handle({ ...ref(squad), count: 0, type: "squad_alive" });
    out = runUntil(s, (o) => o.some((m) => m.type === "ended"));
    assert.deepEqual(
      changesOf(out, "lifetime").map((c) => c.wire.phase),
      [6, 8],
      "results then ended"
    );
    assert.equal(out.filter((m) => m.type === "ended").length, 1);
    // ended stays sent once
    const after = runUntil(s, () => true, 5);
    assert.equal(after.filter((m) => m.type === "ended").length, 0);
  });

  it("replays the same messages to the same outputs", async () => {
    const script = (s) => {
      const out = [];
      out.push(...s.handle({ type: "start" }));
      out.push(...s.handle({ key: "7", type: "player_spawned" }));
      for (let t = 1; t <= 40; t++) out.push(...s.handle({ now: t * 3, type: "tick" }));
      return JSON.stringify(out);
    };
    assert.equal(script(await session()), script(await session()));
  });

  it("stamps rows with the host's clock from tick.now", async () => {
    const s = await session();
    s.handle({ type: "start" });
    s.handle({ key: "7", type: "player_spawned" });
    const out = [];
    for (let t = 1; t <= 20; t++) out.push(...s.handle({ now: 1000 + t, type: "tick" }));
    const cut = changesOf(out, "cinematic")[0];
    assert.ok(cut.state.playing.startedAt > 1000);
    const done = out.filter((m) => m.type === "tick_done");
    assert.equal(done.length, 20);
    assert.equal(done.at(-1).now, 1020);
    assert.ok(Math.abs(done.at(-1).time - 2.0) < 1e-9, "20 ticks at 10 Hz is 2 s of script time");
  });

  it("a skip request from the only player ends the cinematic", async () => {
    const s = await session();
    s.handle({ type: "start" });
    s.handle({ key: "7", type: "player_spawned" });
    const out = runUntil(s, (o) => changesOf(o, "cinematic").length > 0);
    s.handle({ ...ref(changesOf(out, "cinematic")[0]), key: "7", type: "cinematic_skipped" });
    runUntil(s, (o) => o.some((m) => m.type === "phase" && m.index === 1));
  });

  it("a skip waits for every player the host listed", async () => {
    const s = await session();
    s.handle({ type: "start" });
    s.handle({ players: [{ key: "1" }, { key: "2" }], type: "players" });
    s.handle({ key: "1", type: "player_spawned" });
    s.handle({ key: "2", type: "player_spawned" });
    const out = runUntil(s, (o) => changesOf(o, "cinematic").length > 0);
    const cut = ref(changesOf(out, "cinematic")[0]);
    s.handle({ ...cut, key: "1", type: "cinematic_skipped" });
    const waiting = runUntil(s, () => true, 1);
    for (let i = 0; i < 30; i++) waiting.push(...s.handle({ now: s.ticks + 1, type: "tick" }));
    assert.ok(!waiting.some((m) => m.type === "phase"), "one of two skips does not end it");
    s.handle({ ...cut, key: "2", type: "cinematic_skipped" });
    runUntil(s, (o) => o.some((m) => m.type === "phase" && m.index === 1));
  });

  it("translates a catalogued incident id to its name hash", async () => {
    const s = await session("test_cine", {
      incidents: IncidentTable.parse("index\tname_hash\tname\n891\t0x3c03f059\tcinematic_finished\n"),
    });
    s.handle({ type: "start" });
    s.handle({ key: "7", type: "player_spawned" });
    const out = runUntil(s, (o) => changesOf(o, "cinematic").length > 0);
    // incident 891 naming the row is cinematic_finished
    s.handle({ ...ref(changesOf(out, "cinematic")[0]), id: 891, type: "incident" });
    runUntil(s, (o) => o.some((m) => m.type === "phase" && m.index === 1));
    const unknown = s.handle({ id: 424242, type: "incident" });
    assert.equal(unknown[0].type, "error");
  });

  it("takes a host content export in place of stand-ins", async () => {
    const first = await session();
    const exported = JSON.parse(JSON.stringify(first.bundles)).map((b) => ({
      ...b,
      bundle: `0x${b.bundle.toString(16)}`,
    }));
    const s = await session("test_cine", { bundles: exported });
    assert.equal(s.loaded().stand_in, false);
    const started = s.handle({ type: "start" });
    assert.deepEqual(started[0].unresolved, []);
  });

  it("phase_index resumes at a later phase", async () => {
    const s = await session("test_cine", { phaseIndex: 1 });
    const started = s.handle({ type: "start" });
    assert.equal(started.find((m) => m.type === "phase").index, 1);
  });

  it("rejects an unknown message type without stopping", async () => {
    const s = await session();
    const out = s.handle({ type: "frobnicate" });
    assert.equal(out[0].type, "error");
  });
});
