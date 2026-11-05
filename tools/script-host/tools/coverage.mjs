#!/usr/bin/env node
// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// Compile coverage for every activity in a script folder: does it compile,
// does it run to its first phase, and which natives does it call that
// nothing answers. Prints a Markdown table (names and counts only, never
// script text) and, with --json, the same as JSON.
//
//   D1_SCRIPTS_DIR=<scripts> node tools/coverage.mjs [--seconds 60] [--json out.json] [--bundles D1_BUNDLES_DIR]
//
// With --bundles, an activity runs on its world's content export (tools/export-bundles.mjs) when one
// is there, and on stand-ins otherwise.
//
// Each activity runs on stand-in bundles (src/scripts.mjs), plus the area
// encounters guessed from names (--no-area-guess to leave them out), at
// 10 Hz for --seconds of script time with one player spawned at the start.
// A synthetic driver stands in for the client (--idle turns it off): a
// cinematic that starts playing finishes 2 s later, an armed player trigger
// fires 3 s later, and a squad that is placed reports one member alive, then
// none 5 s later. On content (--bundles) the driver also reports the losses of
// a squad with an import identifier as the client's tally (the runtime trusts
// a tally or kill incidents over the live count for such squads), and walks into a player trigger the client
// arms from its own content once a thread has polled it for 3 s. Nothing else is simulated, so the phases reached measure
// the scripts and natives, not the game.

import { writeFileSync } from "node:fs";
import { loadBullshcript } from "../src/bullshcript.mjs";
import { IncidentTable } from "../src/incidents.mjs";
import { guessAreaBundles, ScriptSet, scriptsDirFromEnv } from "../src/scripts.mjs";
import { STUB_VERBS, staticMissingNatives, unansweredByName } from "../src/natives56578.mjs";

const STUB_NAMES = new Set(Object.keys(STUB_VERBS));
import { ActivitySession, bundlesPathFor, readBundlesFile } from "../src/session.mjs";
import { lifetimeWirePhase } from "../src/types56578.mjs";

function args(argv) {
  const out = { areaGuess: true, drive: true, json: undefined, only: undefined, seconds: 60 };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--seconds") out.seconds = Number(argv[++i]);
    else if (a === "--json") out.json = argv[++i];
    else if (a === "--idle") out.drive = false;
    else if (a === "--waits") out.waits = true;
    else if (a === "--no-area-guess") out.areaGuess = false;
    else if (a === "--only") out.only = argv[++i].split(",");
    else if (a === "--scripts") out.scripts = argv[++i];
    else if (a === "--bundles") out.bundlesDir = argv[++i];
    else throw new Error(`unknown argument ${a}`);
  }
  return out;
}

export async function coverage({ scriptsDir, seconds = 60, drive = true, areaGuess = true, only, bundlesDir } = {}) {
  const bs = await loadBullshcript();
  const scripts = ScriptSet.fromDir(bs.api, scriptsDir);
  const incidents = IncidentTable.load();
  const results = [];
  for (const activity of scripts.activities()) {
    if (only && !only.includes(activity.name)) continue;
    const path = bundlesDir ? bundlesPathFor(activity.name, { D1_BUNDLES_DIR: bundlesDir }) : undefined;
    const bundles = path ? readBundlesFile(path) : undefined;
    results.push(await runOne({ activity, areaGuess, bs, bundles, drive, incidents, scripts, seconds }));
  }
  return { files: scripts.files.length, results };
}

export async function runOne({ activity, areaGuess = true, bs, bundles, drive = true, incidents, scripts, seconds, transcript }) {
  // Content names the bundles an activity loads; the name guess is for stand-ins only.
  const area = areaGuess && !bundles ? guessAreaBundles(scripts, activity) : [];
  const row = {
    activity: activity.name,
    area_bundles: area,
    content: bundles !== undefined,
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
      bundles,
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
  // Typed: self.<native member>:<verb>() with no verb. By name: any
  // `.name(` call nobody answers. Their union is what the scripts could call.
  const typed = staticMissingNatives(bs, scripts, session.closure);
  const byName = unansweredByName(bs, scripts, session.closure);
  row.static_missing = [
    ...typed,
    ...byName
      .filter((m) => !typed.some((t) => t.method === m.method))
      .map((m) => ({ ...m, stubbed: STUB_NAMES.has(m.method), types: [] })),
  ];
  // key -> {at, msgs}: what the driver sends at tick `at`
  const pending = new Map();
  const later = (key, after, msgs) => {
    if (drive && !pending.has(key)) pending.set(key, { at: session.ticks + after, msgs });
  };
  const ref = (i) => ({ bundle: i.bundle, type_index: i.typeIndex });
  // Content only: a squad's import identifier (its kills come as incidents the
  // runtime counts), and the player triggers by name, for the triggers the
  // client's own content arms (no script enables them).
  const idents = new Map();
  const triggers = new Map();
  for (const tag of bundles?.bundles ?? (Array.isArray(bundles) ? bundles : [])) {
    for (const r of tag.rows) {
      if (r.importIdent !== undefined && r.typeId === 1 /* Squad */) idents.set(`${tag.bundle}:${r.typeIndex}`, r.importIdent);
      if (r.typeId === 29 /* PlayerTrigger */ && r.name) {
        const last = r.name.slice(r.name.lastIndexOf(".") + 1);
        triggers.set(last, triggers.has(last) ? null : { bundle: tag.bundle, type_index: r.typeIndex });
      }
    }
  }
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
            // A squad with an import identifier counts its losses from the
            // client's tally (an identifier shared by several squads credits
            // a kill incident to none without a position).
            const tally = idents.has(`${c.item.bundle}:${c.item.typeIndex}`)
              ? [{ ...ref(c.item), tally: c.state.roster ?? [c.state.bodies ?? 1], type: "squad_tally" }]
              : [];
            later(`${key}:${c.state.placedAt}:${c.state.wave}:dead`, 50, [
              { ...ref(c.item), count: 0, type: "squad_alive" },
              ...tally,
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
  // With `transcript`, every message in and out is recorded in order ({in} / {out} lines): a run
  // that replays exactly (tools/replay.mjs).
  if (transcript) {
    const handle = session.handle.bind(session);
    session.handle = (msg) => {
      transcript.push({ in: msg });
      const out = handle(msg);
      for (const m of out) transcript.push({ out: m });
      return out;
    };
  }
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
      // Content only: a trigger the client arms from content (no script
      // enables it) that a thread has polled for 3 s is walked into.
      if (drive && triggers.size > 0 && t % 30 === 0) {
        for (const th of session.runtime.debugSnapshot().threads) {
          for (const m of (th.waitingOn ?? "").matchAll(/([A-Za-z_]\w*)\.is_triggered\(\)/g)) {
            const at = triggers.get(m[1]);
            if (at) later(`content-trigger:${at.bundle}:${at.type_index}`, 0, [{ ...at, type: "trigger" }]);
          }
        }
      }
    }
  } catch (error) {
    row.errors.push(`run: ${error.message}`);
  }
  // What each live thread waits on at the end (names only).
  row.waits = session.runtime
    .debugSnapshot()
    .threads.filter((t) => t.state !== "dead" && t.waitingOn)
    .map((t) => `${t.name}: ${t.waitingOn}`);
  row.missing = session.missingNatives();
  row.stub_calls = session.stubCalls();
  row.row_gaps = session.rowGapCalls();
  return row;
}

export function markdown({ files, results }) {
  const yes = (b) => (b ? "yes" : "no");
  const lines = [
    "| Activity | Compiles | Phases | First phase | Phases reached | Lifetime (wire) | Ended | Missing natives (static / run) | Unresolved bindings | Verbs on unplaced rows | Thread failures |",
    "|---|---|---:|---|---:|---|---|---:|---:|---:|---:|",
  ];
  for (const r of results) {
    lines.push(
      `| ${r.activity}${r.stub ? " (stub)" : ""}${r.content === false && results.some((x) => x.content) ? " (stand-in)" : ""} | ${yes(r.compiles)} | ${r.phases_authored} | ${r.stub ? "n/a" : yes(r.first_phase)} | ${r.phases_reached} | ${r.lifetime.join(" → ") || "none"} | ${yes(r.ended)} | ${r.static_missing.length} / ${r.missing.length + r.stub_calls.length} | ${r.unresolved} | ${r.row_gaps.reduce((n, g) => n + g.calls, 0)} | ${r.thread_failures} |`
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
    for (const m of [...r.missing, ...r.stub_calls]) {
      const e = entry(m.method);
      e.ran += 1;
      e.calls += m.calls;
      for (const on of m.on ?? []) e.types.add(`(${on.split(".").at(-1)})`);
    }
  }
  const summary = [
    `Files: ${files}. Activities: ${total} (${count((r) => r.stub)} start-only stubs).`,
    `Compile: ${count((r) => r.compiles)}/${total}. Load: ${count((r) => r.loads)}/${total}.`,
    `First phase: ${count((r) => !r.stub && r.first_phase)}/${count((r) => !r.stub)} written activities.`,
    `Content bundles: ${count((r) => r.content)} activities. Area encounters loaded by name guess: ${count((r) => r.area_bundles.length > 0)} activities.`,
    `Ended within the run: ${count((r) => r.ended)}. Distinct missing natives: ${missing.size}.`,
  ];
  const natives = [
    "| Missing native | On | Activities calling it | Activities hitting it in the run (calls) | Stubbed |",
    "|---|---|---:|---:|---|",
    ...[...missing]
      .sort((a, b) => b[1].activities - a[1].activities || a[0].localeCompare(b[0]))
      .map(([m, e]) => `| \`${m}\` | ${[...e.types].filter((t) => !t.startsWith("(")).join(", ") || "-"} | ${e.activities} | ${e.ran} (${e.calls}) | ${e.stubbed || STUB_NAMES.has(m) ? "yes" : "no"} |`),
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
  if (a.waits) {
    for (const r of report.results) {
      console.log(`\n${r.activity} waits:\n  ${r.waits.join("\n  ") || "(none)"}`);
    }
  }
  if (a.json) writeFileSync(a.json, `${JSON.stringify(report, null, 2)}\n`);
}
