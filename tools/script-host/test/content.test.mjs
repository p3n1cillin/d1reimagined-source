// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// The content export and what the sidecar does with it: the exporter's
// helpers (on synthetic structures, no packages), the export's two shapes,
// D1_BUNDLES_DIR, the scripts' archetypes and name aliases on content rows,
// host-side incident listeners, and the single public event messages. The
// test that reads the owned packages skips without them; the one that runs
// the real export skips without it and the 56578 scripts.

import assert from "node:assert/strict";
import { existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { findBullshcript, loadBullshcript } from "../src/bullshcript.mjs";
import { IncidentTable } from "../src/incidents.mjs";
import { aliasContent, annotateContent, parseBundles, ScriptSet, standInBundles } from "../src/scripts.mjs";
import { ActivitySession, bundlesPathFor, isHostSideBinding, readBundlesFile } from "../src/session.mjs";
import { insideRepo, objectiveTasks, sliceStatesTsv, squadCells, worldBundles } from "../tools/export-bundles.mjs";
import { SYNTHETIC_FILES } from "./synthetic.mjs";

const skip = findBullshcript() ? false : "no built BullShcript checkout (set D1_BULLSHCRIPT_DIR)";
const fnv1 = (s) => {
  let h = 0x811c9dc5;
  for (const b of Buffer.from(s)) h = (Math.imul(h, 0x01000193) ^ b) >>> 0;
  return h >>> 0;
};

describe("the exporter's helpers", () => {
  it("reads a squad definition's cells and member names", () => {
    const def = { members: [{ archetype: 0x7be5ce1a, variants: [[1, 2, 3], [1, 2, 3], [1, 2, 3]] }, { archetype: 5, variants: [[], [], []] }] };
    assert.deepEqual(squadCells(def), { cells: [3, 0], memberNames: [0x7be5ce1a, 5] });
  });

  it("flattens an objective's tasks as BullShcript's TU0 rows do", () => {
    const def = { task_names: [{ hash: 0x811c9dc5 }, { hash: 10 }, { hash: 11 }], tasks: [{ unknown0c: 1 }, { unknown0c: 2 }] };
    const tasks = objectiveTasks(def);
    assert.deepEqual(tasks.map((t) => [t.name, t.influencer]), [[10, 1], [11, 2]]);
    assert.ok(tasks.every((t) => t.capacity === -1 && t.parent === -1));
  });

  it("keeps one tag per bundle hash, first in slice-set order, and counts conflicts", () => {
    const tags = {
      1: { bundle: 0xa, rows: [{ name: "x", typeId: 4 }], tag: 1 },
      2: { bundle: 0xa, rows: [{ name: "x", typeId: 4 }], tag: 2 },
      3: { bundle: 0xa, rows: [{ name: "y", typeId: 4 }], tag: 3 },
      4: { bundle: 0xb, rows: [], tag: 4 },
    };
    const reader = { bundle: (t) => tags[t] };
    const { bundles, conflicts } = worldBundles(reader, { slices: [{ tags: [1, 4] }, { tags: [2] }, { tags: [3] }] });
    assert.deepEqual(bundles.map((b) => b.tag), [1, 4]);
    assert.equal(conflicts, 1, "a same-named bundle with another row table");
  });

  it("writes identifier-only slice states", () => {
    const tsv = sliceStatesTsv([{ name: "w", slices: [{ bubble: 3, region_hash: 0x1ad5e415, slice_set: 25, state_hash: 0x77a6931d, valid: true }] }]);
    assert.match(tsv, /^# GENERATED/);
    assert.ok(tsv.includes("w\t25\t3\t1\t0x77a6931d\t0x1ad5e415"));
  });

  it("refuses to write the export inside the repository", () => {
    assert.equal(insideRepo(join(process.cwd(), "x")), true);
    assert.equal(insideRepo(tmpdir()), false);
  });
});

describe("the export on the sidecar", () => {
  it("accepts an ActivityBundleTag[] or the per-world document", () => {
    const tag = { bundle: "0x2fe7dbe6", rows: [{ name: "a", nameHash: "0x10", typeId: 6, typeIndex: 0 }], tag: "0x80c3e70e" };
    const a = parseBundles([tag]);
    const b = parseBundles({ bundles: [tag], world: "cine_reveal" });
    assert.deepEqual(a, b);
    assert.equal(a[0].bundle, 0x2fe7dbe6);
    assert.equal(a[0].rows[0].nameHash, 0x10);
    assert.throws(() => parseBundles({ world: "x" }));
  });

  it("finds an activity's export under D1_BUNDLES_DIR", () => {
    const dir = mkdtempSync(join(tmpdir(), "k-bundles-"));
    try {
      writeFileSync(join(dir, "cine_reveal.json"), JSON.stringify({ bundles: [] }));
      assert.equal(bundlesPathFor("cine_reveal", { D1_BUNDLES_DIR: dir }), join(dir, "cine_reveal.json"));
      assert.equal(bundlesPathFor("act_x", { D1_BUNDLES_DIR: dir }), undefined);
      assert.equal(bundlesPathFor("cine_reveal", {}), undefined);
      assert.deepEqual(readBundlesFile(join(dir, "cine_reveal.json")), { bundles: [] });
    } finally {
      rmSync(dir, { force: true, recursive: true });
    }
  });

  it("keeps incident listeners host-side, as BullShcript does", () => {
    assert.equal(isHostSideBinding("sp_objective_vent._incident_listener"), true);
    assert.equal(isHostSideBinding("phase.il_boss_dead"), true);
    assert.equal(isHostSideBinding("obj._object"), false);
    assert.equal(isHostSideBinding("pt_bubble"), false);
  });

  it("names an Encounter row's archetype from the scripts and aliases a prefixed member", () => {
    const types = { Encounter: 59, Object: 4 };
    const content = [{ bundle: 9, rows: [
      { name: "phase.enc_a", nameHash: 1, typeId: 59, typeIndex: 0 },
      { name: "thorium_dome_a", nameHash: 2, typeId: 4, typeIndex: 1 },
      { name: "chest_dome_c", nameHash: 3, typeId: 4, typeIndex: 2 },
      { name: "chest_dome_d", nameHash: 4, typeId: 4, typeIndex: 3 },
    ], tag: 1 }];
    const standIn = [{ bundle: 9, rows: [
      { childScript: "simple_encounter", name: "phase.enc_a", typeId: 59 },
      { name: "thorium_dome", typeId: 4 },
      { name: "chest_dome", typeId: 4 },
    ], tag: 1 }];
    assert.equal(annotateContent(content, standIn, types), 1);
    assert.equal(content[0].rows[0].childScript, "simple_encounter");
    assert.equal(aliasContent(content, standIn, fnv1), 1, "chest_dome prefixes two rows: no alias");
    const alias = content[0].rows.find((r) => r.name === "thorium_dome");
    assert.deepEqual([alias.typeId, alias.typeIndex, alias.aliasOf, alias.nameHash], [4, 1, "thorium_dome_a", fnv1("thorium_dome")]);
    assert.equal(content[0].rows.filter((r) => r.typeIndex === 1).length, 2, "the content row itself stays");
  });
});

describe("single public events", { skip }, () => {
  async function session() {
    const bs = await loadBullshcript();
    const scripts = new ScriptSet(bs.api, SYNTHETIC_FILES);
    const s = await ActivitySession.create({ activity: "test_cine", bs, emit: () => undefined, incidents: new IncidentTable(), scripts });
    s.handle({ type: "start" });
    return s;
  }

  it("starts and stops one bundle's threads and answers each", async () => {
    const s = await session();
    const bundle = s.bundles[0].bundle;
    const others = s.bundles.slice(1).map((t) => t.bundle);
    const stop = s.handle({ bundle, event_id: 3, type: "event_stop" });
    assert.deepEqual(stop.find((m) => m.type === "event"), { bundle, event_id: 3, state: "stopped", type: "event" });
    assert.deepEqual([...s.activeBundles].sort(), [...others].sort(), "the other bundles keep running");
    const start = s.handle({ bundle, event_id: 3, type: "event_start" });
    assert.equal(start.find((m) => m.type === "event").state, "started");
    assert.ok(s.activeBundles.has(bundle));
    const refused = s.handle({ bundle: 0x1234, event_id: 4, type: "event_start" });
    assert.equal(refused.find((m) => m.type === "event").state, "refused");
  });

  it("starts from the active set a slice change named", async () => {
    const s = await session();
    const [a, b] = s.bundles.map((t) => t.bundle);
    s.handle({ bundles: [a], type: "active_bundles" });
    s.handle({ bundle: b, event_id: 1, type: "event_start" });
    assert.deepEqual([...s.activeBundles].sort(), [a, b].sort());
  });

  it("content bundles carry no stand-in flag and keep the content's indices", async () => {
    const bs = await loadBullshcript();
    const scripts = new ScriptSet(bs.api, SYNTHETIC_FILES);
    const types = { ...bs.api.ActivityBundleItemType };
    const made = standInBundles(scripts, scripts.activity("test_cine"), types);
    // A content export with the same rows at other indices.
    const content = made.bundles.map((t) => ({ ...t, rows: t.rows.map((r) => ({ ...r, typeIndex: r.typeIndex + 100 })) }));
    const s = await ActivitySession.create({ activity: "test_cine", bs, bundles: content, emit: () => undefined, incidents: new IncidentTable(), scripts });
    assert.equal(s.loaded().stand_in, false);
    const out = s.handle({ type: "start" });
    const changes = out.filter((m) => m.type === "rows").flatMap((m) => m.changes);
    assert.ok(changes.length > 0 && changes.every((c) => c.item.typeIndex >= 100), "every row is the content's");
  });
});

const realSkip =
  !findBullshcript() || !process.env.D1_SCRIPTS_DIR || !process.env.D1_BUNDLES_DIR || !existsSync(join(process.env.D1_BUNDLES_DIR ?? "", "cine_reveal.json"))
    ? "needs D1_SCRIPTS_DIR (the 56578 scripts) and D1_BUNDLES_DIR (the private export)"
    : false;

describe("the real export", { skip: realSkip }, () => {
  it("binds cine_reveal and A Guardian Rises with nothing unresolved", async () => {
    const bs = await loadBullshcript();
    const scripts = ScriptSet.fromDir(bs.api, process.env.D1_SCRIPTS_DIR);
    for (const name of ["cine_reveal", "act_cosmo_m0_highway"]) {
      const bundles = readBundlesFile(bundlesPathFor(name));
      const s = await ActivitySession.create({ activity: name, bs, bundles, emit: () => undefined, incidents: IncidentTable.load(), scripts });
      assert.equal(s.loaded().stand_in, false);
      const ready = s.handle({ type: "start" }).find((m) => m.type === "ready");
      assert.deepEqual(ready.unresolved, [], `${name}: no unresolved bindings on content`);
    }
  });
});

