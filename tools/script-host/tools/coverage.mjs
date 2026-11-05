#!/usr/bin/env node
// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// Compile coverage for every activity in a script folder: does it compile,
// does it run to its first phase, and which natives does it call that
// nothing answers. Prints a Markdown table (names and counts only, never
// script text) and, with --json, the same as JSON.
//
//   D1_SCRIPTS_DIR=<scripts> node tools/coverage.mjs [--seconds 60] [--json out.json]
//
// Each activity runs on stand-in bundles (src/scripts.mjs), plus the area
// encounters guessed from names (--no-area-guess to leave them out), at
// 10 Hz for --seconds of script time with one player spawned at the start.
// A synthetic driver stands in for the client (--idle turns it off): a
// cinematic that starts playing finishes 2 s later, an armed player trigger
// fires 3 s later, and a squad that is placed reports one member alive, then
// none 5 s later. Nothing else is simulated, so the phases reached measure
// the scripts and natives, not the game.

import { writeFileSync } from "node:fs";
import { loadBullshcript } from "../src/bullshcript.mjs";
import { IncidentTable } from "../src/incidents.mjs";
import { guessAreaBundles, ScriptSet, scriptsDirFromEnv } from "../src/scripts.mjs";
import { staticMissingNatives } from "../src/natives56578.mjs";
import { ActivitySession } from "../src/session.mjs";
import { lifetimeWirePhase } from "../src/types56578.mjs";

function args(argv) {
  const out = { areaGuess: true, drive: true, json: undefined, only: undefined, seconds: 60 };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--seconds") out.seconds = Number(argv[++i]);
    else if (a === "--json") out.json = argv[++i];
    else if (a === "--idle") out.drive = false;
    else if (a === "--no-area-guess") out.areaGuess = false;
    else if (a === "--only") out.only = argv[++i].split(",");
    else if (a === "--scripts") out.scripts = argv[++i];
    else throw new Error(`unknown argument ${a}`);
  }
  return out;
}

export async function coverage({ scriptsDir, seconds = 60, drive = true, areaGuess = true, only } = {}) {
  const bs = await loadBullshcript();
  const scripts = ScriptSet.fromDir(bs.api, scriptsDir);
  const incidents = IncidentTable.load();
  const results = [];
  for (const activity of scripts.activities()) {
    if (only && !only.includes(activity.name)) continue;
    results.push(await runOne({ activity, areaGuess, bs, drive, incidents, scripts, seconds }));
  }
  return { files: scripts.files.length, results };
}

async function runOne({ activity, areaGuess, bs, drive, incidents, scripts, seconds }) {
  const area = areaGuess ? guessAreaBundles(scripts, activity) : [];
  const row = {
    activity: activity.name,
    area_bundles: area,
    compiles: false,
    ended: false,
    errors: [],
    first_phase: false,
    lifetime: [],
    loads: false,
    missing: [],
    phases_authored: activity.phases.length,
    phases_reached: 0,
    root: activity.root,
    stub: activity.stub,
    thread_failures: 0,
    unresolved: 0,
  };
  let session;
  try {
    session = await ActivitySession.create({
      activity: activity.name,
      bs,
      emit: () => undefined,
      extraBundles: area,
      incidents,
      scripts,
    });
  } catch (error) {
    row.errors.push(`create: ${error.message}`);
    return row;
  }
  const loaded = session.loaded();
  row.compiles = loaded.compile_errors.length === 0;
  row.errors.push(...loaded.compile_errors.slice(0, 3));
  row.drives_lifetime = loaded.drives_lifetime;
  row.static_missing = staticMissingNatives(bs, scripts, session.closure);
  // key -> {at, msgs}: what the driver sends at tick `at`
  const pending = new Map();
  const later = (key, after, msgs) => {
    if (drive && !pending.has(key)) pending.set(key, { at: session.ticks + after, msgs });
  };
  const ref = (i) => ({ bundle: i.bundle, type_index: i.typeIndex });
  const seen = (msgs) => {
    for (const m of msgs) {
      if (m.type === "phase") {
        row.phases_reached = Math.max(row.phases_reached, m.index + 1);
        if (m.index === 0) row.first_phase = true;
      } else if (m.type === "rows") {
        for (const c of m.changes) {
          if (c.kind === "lifetime") {
            const w = lifetimeWirePhase(c.state.state);
            if (row.lifetime.at(-1) !== w) row.lifetime.push(w);
          }
          const key = `${c.kind}:${c.item.bundle}:${c.item.typeIndex}`;
          if (c.kind === "cinematic" && c.state.playing) {
            later(`${key}:${c.state.playing.startedAt}`, 20, [{ ...ref(c.item), type: "cinematic_finished" }]);
          } else if (c.kind === "playerTrigger" && c.state.enabled) {
            later(`${key}:${c.state.armedAt}`, 30, [{ ...ref(c.item), type: "trigger" }]);
          } else if (c.kind === "squad" && c.state.placedAt !== undefined) {
            later(`${key}:${c.state.placedAt}:${c.state.wave}`, 1, [
              { ...ref(c.item), count: 1, type: "squad_alive" },
            ]);
            later(`${key}:${c.state.placedAt}:${c.state.wave}:dead`, 50, [
              { ...ref(c.item), count: 0, type: "squad_alive" },
            ]);
          }
        }
      } else if (m.type === "ended") {
        row.ended = true;
      } else if (m.type === "log" && / failed: /.test(m.text)) {
        row.thread_failures += 1;
        if (row.errors.length < 3) row.errors.push(m.text.slice(0, 160));
      } else if (m.type === "log" && /did not load/.test(m.text)) {
        if (row.errors.length < 3) row.errors.push(m.text.slice(0, 160));
      }
    }
  };
  try {
    const started = session.handle({ type: "start" });
    seen(started);
    const ready = started.find((m) => m.type === "ready");
    row.unresolved = ready?.unresolved.length ?? 0;
    row.loads = !started.some((m) => m.type === "log" && /did not load/.test(m.text));
    seen(session.handle({ key: "0", type: "player_spawned" }));
    const ticks = Math.round(seconds * session.tickHz);
    for (let t = 1; t <= ticks && !row.ended; t++) {
      for (const p of pending.values()) {
        if (p.at <= session.ticks && p.msgs) {
          p.msgs.forEach((msg) => seen(session.handle(msg)));
          p.msgs = undefined; // keep the key: each state is driven once
        }
      }
      seen(session.handle({ now: t, type: "tick" }));
    }
  } catch (error) {
    row.errors.push(`run: ${error.message}`);
  }
  row.missing = session.missingNatives();
  return row;
}

export function markdown({ files, results }) {
  const yes = (b) => (b ? "yes" : "no");
  const lines = [
    "| Activity | Compiles | Phases | First phase | Phases reached | Lifetime (wire) | Ended | Missing natives (static / run) | Unresolved | Thread failures |",
    "|---|---|---:|---|---:|---|---|---:|---:|---:|",
  ];
  for (const r of results) {
    lines.push(
      `| ${r.activity}${r.stub ? " (stub)" : ""} | ${yes(r.compiles)} | ${r.phases_authored} | ${r.stub ? "n/a" : yes(r.first_phase)} | ${r.phases_reached} | ${r.lifetime.join(" → ") || "none"} | ${yes(r.ended)} | ${r.static_missing.length} / ${r.missing.length} | ${r.unresolved} | ${r.thread_failures} |`
    );
  }
  const total = results.length;
  const count = (f) => results.filter(f).length;
  const missing = new Map();
  const entry = (method) => {
    const e = missing.get(method) ?? { activities: 0, calls: 0, ran: 0, stubbed: false, types: new Set() };
    missing.set(method, e);
    return e;
  };
  for (const r of results) {
    for (const m of r.static_missing) {
      const e = entry(m.method);
      e.activities += 1;
      e.stubbed ||= m.stubbed;
      for (const t of m.types) e.types.add(t);
    }
    for (const m of r.missing) {
      const e = entry(m.method);
      e.ran += 1;
      e.calls += m.calls;
      for (const on of m.on) e.types.add(`(${on.split(".").at(-1)})`);
    }
  }
  const summary = [
    `Files: ${files}. Activities: ${total} (${count((r) => r.stub)} start-only stubs).`,
    `Compile: ${count((r) => r.compiles)}/${total}. Load: ${count((r) => r.loads)}/${total}.`,
    `First phase: ${count((r) => !r.stub && r.first_phase)}/${count((r) => !r.stub)} written activities.`,
    `Area encounters loaded by name guess: ${count((r) => r.area_bundles.length > 0)} activities.`,
    `Ended within the run: ${count((r) => r.ended)}. Distinct missing natives: ${missing.size}.`,
  ];
  const natives = [
    "| Missing native | On | Activities calling it | Activities hitting it in the run (calls) | Stubbed |",
    "|---|---|---:|---:|---|",
    ...[...missing]
      .sort((a, b) => b[1].activities - a[1].activities || a[0].localeCompare(b[0]))
      .map(([m, e]) => `| \`${m}\` | ${[...e.types].filter((t) => !t.startsWith("(")).join(", ") || "-"} | ${e.activities} | ${e.ran} (${e.calls}) | ${e.stubbed ? "yes" : "no"} |`),
  ];
  return [...summary, "", ...lines, "", ...natives].join("\n");
}

if (import.meta.url === `file://${process.argv[1].replaceAll("\\", "/").replace(/^(?=[A-Za-z]:)/, "/")}`) {
  const a = args(process.argv.slice(2));
  const scriptsDir = a.scripts ?? scriptsDirFromEnv();
  if (!scriptsDir) {
    console.error("set D1_SCRIPTS_DIR (or --scripts) to the 56578 script folder");
    process.exit(2);
  }
  const report = await coverage({ ...a, scriptsDir });
  console.log(markdown(report));
  if (a.json) writeFileSync(a.json, `${JSON.stringify(report, null, 2)}\n`);
}
