#!/usr/bin/env node
// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// The mission-script sidecar: one process per activity instance, JSON lines
// on stdin (host -> sidecar) and stdout (sidecar -> host), a human log on
// stderr. PROTOCOL.md is the contract.
//
//   node tools/script-host/src/sidecar.mjs
//
// Environment: D1_SCRIPTS_DIR (the script folder, unless `hello` names one),
// D1_BULLSHCRIPT_DIR or D1_TIGER_DIR (the BullShcript checkout),
// D1_SCRIPT_HOST_LOG=quiet|lua (stderr detail; default: no Lua notes).

import { createInterface } from "node:readline";
import { loadBullshcript } from "./bullshcript.mjs";
import { IncidentTable } from "./incidents.mjs";
import { guessAreaBundles, ScriptSet, scriptsDirFromEnv } from "./scripts.mjs";
import {
  ActivitySession,
  bundlesPathFor,
  DEFAULT_TICK_HZ,
  PROTOCOL_VERSION,
  readBundlesFile,
} from "./session.mjs";

// The sidecar's state machine over one input stream. `write` takes each
// output message; `logLua` says whether Lua notes go out as `log` messages.
export class Sidecar {
  constructor({ write, env = process.env, logLua = true }) {
    this.write = write;
    this.env = env;
    this.logLua = logLua;
    this.session = undefined;
    this.scriptSets = new Map();
  }

  send(msg) {
    // Lua notes only when asked; stub calls and unanswered natives always.
    if (msg.type === "log" && !this.logLua && !msg.stub && !msg.missing) {
      return;
    }
    this.write(msg);
  }

  error(text, fatal = false) {
    this.write({ fatal, text, type: "error" });
  }

  async scriptSet(bs, dir) {
    if (!this.scriptSets.has(dir)) {
      this.scriptSets.set(dir, ScriptSet.fromDir(bs.api, dir));
    }
    return this.scriptSets.get(dir);
  }

  async hello(msg) {
    if (msg.proto !== undefined && Number(msg.proto) !== PROTOCOL_VERSION) {
      return this.error(`protocol ${msg.proto} not supported (this is ${PROTOCOL_VERSION})`, true);
    }
    if (msg.build !== undefined && String(msg.build) !== "56578") {
      return this.error(`build ${msg.build} not supported (56578 only)`, true);
    }
    if (msg.log === "lua") this.logLua = true;
    const env = { ...this.env };
    if (msg.tiger_dir) env.D1_TIGER_DIR = msg.tiger_dir;
    if (msg.bullshcript_dir) env.D1_BULLSHCRIPT_DIR = msg.bullshcript_dir;
    const bs = await loadBullshcript(env);
    const dir = msg.scripts_dir ?? scriptsDirFromEnv(env);
    if (!dir) {
      return this.error("no script folder: set D1_SCRIPTS_DIR or send scripts_dir", true);
    }
    const scripts = await this.scriptSet(bs, dir);
    const incidents = IncidentTable.load(msg.incidents_path);
    const bundlesPath = msg.bundles_path ?? bundlesPathFor(msg.activity, env);
    const bundles = msg.bundles ?? (bundlesPath ? readBundlesFile(bundlesPath) : undefined);
    const activity = scripts.activity(msg.activity);
    if (!activity) {
      return this.error(`no script starts activity ${msg.activity}`, true);
    }
    const extraBundles =
      msg.extra_bundles === "guess" ? guessAreaBundles(scripts, activity) : (msg.extra_bundles ?? []);
    this.session = await ActivitySession.create({
      activity: msg.activity,
      bs,
      bundles,
      emit: (m) => this.send(m),
      extraBundles,
      incidents,
      phaseIndex: msg.phase_index,
      scripts,
      seed: msg.seed,
      tickHz: msg.tick_hz ?? DEFAULT_TICK_HZ,
    });
    const loaded = this.session.loaded();
    loaded.extra_bundles = extraBundles;
    loaded.incidents = incidents.source;
    this.send(loaded);
  }

  // One input line. Returns false once the stream should end.
  async line(text) {
    if (text.trim() === "") {
      return true;
    }
    let msg;
    try {
      msg = JSON.parse(text);
    } catch (error) {
      this.error(`not JSON: ${error.message}`);
      return true;
    }
    try {
      if (msg.type === "hello") {
        await this.hello(msg);
        return true;
      }
      if (msg.type === "end") {
        this.write({ type: "bye" });
        return false;
      }
      if (!this.session) {
        this.error(`${msg.type} before hello`);
        return true;
      }
      for (const out of this.session.handle(msg)) {
        this.send(out);
      }
    } catch (error) {
      // A hello that throws (BullShcript missing, an unreadable bundles file, a script that does
      // not load) means the session did not start: fatal, as PROTOCOL.md says, so the host
      // halts with the reason instead of waiting for a `loaded` that never comes (integration
      // review, finding 17). Later failures leave the session running.
      this.error(`${msg.type}: ${error.stack ?? error.message}`, msg.type === "hello");
    }
    return true;
  }
}

async function main() {
  const mode = process.env.D1_SCRIPT_HOST_LOG ?? "";
  const sidecar = new Sidecar({
    logLua: mode === "lua",
    write: (msg) => {
      process.stdout.write(`${JSON.stringify(msg)}\n`);
      if (mode !== "quiet" && (msg.type === "error" || msg.type === "phase" || msg.type === "ended")) {
        process.stderr.write(`script-host: ${msg.type} ${msg.text ?? msg.name ?? ""}\n`);
      }
    },
  });
  const rl = createInterface({ crlfDelay: Number.POSITIVE_INFINITY, input: process.stdin });
  for await (const text of rl) {
    if (!(await sidecar.line(text))) {
      break;
    }
  }
  rl.close();
}

const self = process.argv[1]?.replaceAll("\\", "/");
if (self && import.meta.url.endsWith(self.slice(self.lastIndexOf("/")))) {
  await main();
}
