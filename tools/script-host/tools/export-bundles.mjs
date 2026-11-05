#!/usr/bin/env node
// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// The private content bundle export the sidecar runs on (PROTOCOL.md, `hello.bundles_path`): for
// every world of build 56578, the activity bundles its slice sets load, in BullShcript's
// `ActivityBundleTag` shape, read from the owned packages with Blam Network's MIT gibbon.
//
//   node tools/script-host/tools/export-bundles.mjs <packages> <D1_BUNDLES_DIR> [WORLD ...]
//   node tools/script-host/tools/export-bundles.mjs <packages> --tables <repo>/ps4/fixtures/56578/scripts
//
// The first form writes `<D1_BUNDLES_DIR>/<world>.json` and `index.json`. These hold row NAMES, which
// are game data: the directory must be OUTSIDE the repository (the tool refuses a path inside it).
// The second form writes only identifier tables for the repository (`slice-states.tsv`: each slice
// set's state name hash, which is the FNV-1 of the mission phase that state belongs to).
//
// What each row carries (ps4/design/licensing.md: content is read at run time, never committed):
//
//   name, nameHash, typeId, typeIndex      the bundle's row table: the row's position is its sub
//   importIdent                            the row host definition's import_only_bind label
//   cells, memberNames                     a Squad row: its squad definition's cells (bodies per
//                                          cell, 0 for a named actor) and each cell's archetype hash
//   tasks                                  an Objective row: its task names, flat (BullShcript's TU0
//                                          shape: tasks[i] is task_names[i + 1])
//   sequenceDurationSeconds, sequenceHasBlueprint   a Sequence row
//
// Reads packages with gibbon (tiger, MIT) through tools/ps4_setup/d1-worlds.mjs's loader, so
// there is one reader of the world chain. D1_TIGER (or D1_TIGER_DIR) names the tiger checkout.

import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { BUILD, EMPTY, fnv1, hex, loadGibbon } from "../../ps4_setup/d1-worlds.mjs";

const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
const SUFFIX = ":scenario_client";

// The definition classes by row kind (gibbon's activity_lookup.ts).
const DEFINITION = {
  squad: 0x808005a7,
  objective: 0x80800688,
  sequence: 0x808028da,
};

// Whether `dir` lies inside the repository (an export there would put game data under git).
export function insideRepo(dir, repo = REPO) {
  const rel = path.relative(path.resolve(repo), path.resolve(dir));
  return rel === "" || (!rel.startsWith("..") && !path.isAbsolute(rel));
}

// One squad definition's cells: bodies per cell (the first variant list's length; the three always
// agree) and each cell's archetype name hash.
export function squadCells(definition) {
  return {
    cells: definition.members.map((m) => m.variants[0]?.length ?? 0),
    memberNames: definition.members.map((m) => m.archetype >>> 0),
  };
}

// An objective host definition's tasks in BullShcript's flat TU0 shape.
export function objectiveTasks(definition) {
  return definition.tasks.map((task, i) => ({
    capacity: -1,
    firstChild: -1,
    influencer: task.unknown0c >>> 0,
    name: (definition.task_names[i + 1]?.hash ?? 0x811c9dc5) >>> 0,
    nextSibling: -1,
    parent: -1,
    priority: 0,
  }));
}

export class BundleReader {
  constructor(g, set) {
    this.g = g;
    this.set = set;
    this.cache = new Map();
    this.problems = [];
  }

  // The extras a row's host definition gives, keyed by row index.
  extras(bundle) {
    const { g, set } = this;
    const out = new Map();
    for (const lg of bundle.lookup_groups) {
      for (const l of lg.lookups) {
        const ld = set.try_read_tag(l.tag);
        if (!ld) continue;
        const host = set.try_read_tag(g.read_definition(ld, g.s_activity_lookup).host_tag);
        if (!host) continue;
        const off = g.checked_host_offsets(host, g.k_sensor_host_definition_size);
        if (!off) continue;
        const at = host.subarray(off.definition);
        const head = g.read_definition(at, g.s_sensor_host_definition);
        if (head.sensor.bundle >>> 0 !== bundle.bundle >>> 0 || out.has(head.sensor.index)) continue;
        const extra = {};
        if (head.identifier >>> 0 !== 0x811c9dc5 && head.identifier >>> 0 !== EMPTY) {
          extra.importIdent = head.identifier >>> 0;
        }
        const kind = host.readUInt32LE(off.definition - 4) >>> 0;
        try {
          if (kind === DEFINITION.squad) {
            const d = g.read_definition(at, g.s_squad_sensor_host_definition);
            const sd = set.try_read_tag(d.squad);
            if (sd) Object.assign(extra, squadCells(g.read_definition(sd, g.s_squad_definition)));
          } else if (kind === DEFINITION.objective) {
            const d = g.read_definition(at, g.s_objective_sensor_host_definition);
            if (d.tasks.length > 0) extra.tasks = objectiveTasks(d);
          } else if (kind === DEFINITION.sequence) {
            const d = g.read_definition(at, g.s_sequence_sensor_host_definition);
            extra.sequenceHasBlueprint = d.sequence >>> 0 !== EMPTY;
            extra.sequenceDurationSeconds = d.duration;
          }
        } catch (e) {
          this.problems.push(`bundle ${hex(bundle.bundle)} row ${head.sensor.index}: ${e.message}`);
        }
        out.set(head.sensor.index, extra);
      }
    }
    return out;
  }

  // One bundle tag as an ActivityBundleTag, or undefined.
  bundle(tag) {
    if (this.cache.has(tag)) return this.cache.get(tag);
    const { g, set } = this;
    const data = set.try_read_tag(tag);
    let out;
    if (!data) {
      this.problems.push(`bundle tag ${hex(tag)}: no tag`);
    } else {
      const b = g.read_definition(data, g.s_activity_bundle);
      const extras = this.extras(b);
      out = {
        bundle: b.bundle >>> 0,
        rows: b.rows.map((r, i) => {
          const name = r.name ?? "";
          return { name, nameHash: fnv1(name), typeId: r.type, typeIndex: i, ...(extras.get(i) ?? {}) };
        }),
        slotted: b.slotted_row_count,
        tag: tag >>> 0,
      };
    }
    this.cache.set(tag, out);
    return out;
  }
}

// Every world's scenario: its slice sets in order, each with its state name hash and bundle tags
// (global, authoritative, private: the client's registration order).
export function readScenarios(g, set, only = []) {
  const worlds = [];
  const named = set.named_tags().filter((n) => n.name.endsWith(SUFFIX));
  named.sort((a, b) => a.hash - b.hash);
  for (const n of named) {
    const name = n.name.slice(0, -SUFFIX.length);
    if (only.length && !only.includes(name)) continue;
    const r = g.read_definition(set.read_tag(n.hash), g.s_scenario_client);
    const slices = [];
    r.bubbles.forEach((b, bi) =>
      b.states.forEach((s, si) => {
        const slice = {
          bubble: bi,
          region_hash: s.region_name_hash >>> 0,
          slice_set: 8 * bi + si,
          state_hash: s.unknown04 >>> 0,
          tags: [],
          valid: s.valid !== 0,
        };
        const sd = set.try_read_tag(s.descriptor);
        if (sd) {
          const d = g.read_definition(sd, g.s_slice_descriptor);
          const bs = set.try_read_tag(d.blueprint_set);
          if (bs) {
            const c = g.read_definition(bs, g.s_client_blueprint_set);
            slice.tags = [...c.global_bundles, ...c.authoritative_bundles, ...c.private_bundles].map((x) => x.tag >>> 0);
          }
        }
        slices.push(slice);
      })
    );
    worlds.push({ name, slices });
  }
  return worlds;
}

// The bundles a world loads, one tag per bundle hash (BullShcript keys by hash): the first tag in
// slice-set order wins; a later tag of the same bundle with a different row table is counted.
export function worldBundles(reader, world) {
  const byHash = new Map();
  let conflicts = 0;
  for (const s of world.slices) {
    for (const tag of s.tags) {
      const b = reader.bundle(tag);
      if (!b) continue;
      const have = byHash.get(b.bundle);
      if (!have) {
        byHash.set(b.bundle, b);
      } else if (have.tag !== b.tag && have.rows.map((r) => `${r.typeId}:${r.name}`).join() !== b.rows.map((r) => `${r.typeId}:${r.name}`).join()) {
        conflicts += 1;
      }
    }
  }
  return { bundles: [...byHash.values()], conflicts };
}

// The repository's identifier table: world, slice set, bubble, state name hash, region hash.
export function sliceStatesTsv(worlds) {
  const lines = [
    `# GENERATED by tools/script-host/tools/export-bundles.mjs --tables from build ${BUILD}; do not edit`,
    "# state_hash: the scenario state's name hash, the FNV-1 of the mission phase it belongs to",
    "world\tslice_set\tbubble\tvalid\tstate_hash\tregion_hash",
  ];
  for (const w of worlds) {
    for (const s of w.slices) {
      lines.push([w.name, s.slice_set, s.bubble, s.valid ? 1 : 0, hex(s.state_hash), hex(s.region_hash)].join("\t"));
    }
  }
  return `${lines.join("\n")}\n`;
}

function main(argv) {
  const [packages, out, ...rest] = argv;
  if (!packages || !out) {
    console.error("usage: export-bundles.mjs <packages> <D1_BUNDLES_DIR> [WORLD ...]\n       export-bundles.mjs <packages> --tables <dir>");
    process.exit(2);
  }
  const g = loadGibbon(process.env.D1_TIGER ?? process.env.D1_TIGER_DIR);
  const set = g.c_package_set.open_directory(packages);
  if (out === "--tables") {
    const dir = rest[0];
    if (!dir) throw new Error("--tables needs a directory");
    mkdirSync(dir, { recursive: true });
    const worlds = readScenarios(g, set);
    writeFileSync(path.join(dir, "slice-states.tsv"), sliceStatesTsv(worlds));
    console.error(`${worlds.length} worlds -> ${dir}/slice-states.tsv`);
    return;
  }
  if (insideRepo(out)) {
    console.error(`refusing to write the export inside the repository (${out}): it holds row names, which are game data`);
    process.exit(2);
  }
  mkdirSync(out, { recursive: true });
  const reader = new BundleReader(g, set);
  const index = [];
  for (const world of readScenarios(g, set, rest)) {
    const { bundles, conflicts } = worldBundles(reader, world);
    const doc = {
      build: BUILD,
      bundles,
      generator: "tools/script-host/tools/export-bundles.mjs",
      slices: world.slices.map(({ slice_set, state_hash, region_hash, tags, valid }) => ({
        bundles: [...new Set(tags.map((t) => reader.bundle(t)?.bundle).filter((b) => b !== undefined))],
        region_hash,
        slice_set,
        state_hash,
        valid,
      })),
      world: world.name,
    };
    writeFileSync(path.join(out, `${world.name}.json`), `${JSON.stringify(doc)}\n`);
    index.push({ bundles: bundles.length, conflicts, rows: bundles.reduce((n, b) => n + b.rows.length, 0), world: world.name });
  }
  writeFileSync(path.join(out, "index.json"), `${JSON.stringify({ build: BUILD, problems: reader.problems, worlds: index }, null, 1)}\n`);
  console.error(`${index.length} worlds, ${reader.cache.size} bundle tags, ${reader.problems.length} problems -> ${out}`);
  for (const p of reader.problems.slice(0, 10)) console.error(`  ${p}`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2));
}
