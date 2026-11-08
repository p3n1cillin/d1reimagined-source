#!/usr/bin/env node
// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// One activity, run on its world's content export under the coverage driver
// (tools/coverage.mjs), written as a transcript: one JSON line per message,
// `{"in": ...}` for what the driver sent and `{"out": ...}` for what the
// sidecar answered, in order. The same inputs give the same transcript, so it
// is a deterministic offline replay of the scripts: the host's tests feed its
// `out` lines through a fake sidecar (crates/d1-activity-host/tests/script_replay.rs).
//
//   D1_SCRIPTS_DIR=<scripts> node tools/replay.mjs ACTIVITY --bundles D1_BUNDLES_DIR [--seconds 300] [--out FILE]
//
// The transcript holds row names and script log lines: it is derived from the
// unlicensed scripts and the game's content, so it stays outside the repository
// (the default --out is stdout).

import { writeFileSync } from "node:fs";
import { loadBullshcript } from "../src/bullshcript.mjs";
import { IncidentTable } from "../src/incidents.mjs";
import { ScriptSet, scriptsDirFromEnv } from "../src/scripts.mjs";
import { bundlesPathFor, readBundlesFile } from "../src/session.mjs";
import { runOne } from "./coverage.mjs";

export async function replay({ activity: name, bundlesDir, scriptsDir, seconds = 300 }) {
  const bs = await loadBullshcript();
  const scripts = ScriptSet.fromDir(bs.api, scriptsDir);
  const activity = scripts.activity(name);
  if (!activity) throw new Error(`no script starts activity ${name}`);
  const path = bundlesPathFor(name, { D1_BUNDLES_DIR: bundlesDir });
  if (!path) throw new Error(`no content export for ${name} under ${bundlesDir}`);
  const transcript = [];
  const row = await runOne({
    activity,
    bs,
    bundles: readBundlesFile(path),
    incidents: IncidentTable.load(),
    scripts,
    seconds,
    transcript,
  });
  return { row, transcript };
}

const self = process.argv[1]?.replaceAll("\\", "/");
if (self && import.meta.url.endsWith(self.slice(self.lastIndexOf("/")))) {
  const argv = process.argv.slice(2);
  const opt = (k, d) => {
    const i = argv.indexOf(k);
    return i < 0 ? d : argv[i + 1];
  };
  const activity = argv[0];
  const bundlesDir = opt("--bundles", process.env.D1_BUNDLES_DIR);
  const scriptsDir = scriptsDirFromEnv();
  if (!activity || !bundlesDir || !scriptsDir) {
    console.error("usage: D1_SCRIPTS_DIR=<scripts> replay.mjs ACTIVITY --bundles DIR [--seconds N] [--out FILE]");
    process.exit(2);
  }
  const { row, transcript } = await replay({ activity, bundlesDir, scriptsDir, seconds: Number(opt("--seconds", 300)) });
  const text = transcript.map((t) => JSON.stringify(t)).join("\n") + "\n";
  const out = opt("--out");
  if (out) writeFileSync(out, text);
  else process.stdout.write(text);
  console.error(`${activity}: ${transcript.length} lines, phases ${row.phases_reached}, lifetime ${row.lifetime.join(" -> ")}, ended ${row.ended}`);
}
