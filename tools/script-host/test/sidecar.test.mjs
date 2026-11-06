// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// The sidecar as the host runs it: a child process speaking JSON lines,
// over the synthetic scripts written to a temporary folder.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { after, before, describe, it } from "node:test";
import { fileURLToPath } from "node:url";
import { findBullshcript } from "../src/bullshcript.mjs";
import { Sidecar } from "../src/sidecar.mjs";
import { writeSyntheticDir } from "./synthetic.mjs";

const skip = findBullshcript() ? false : "no built BullShcript checkout (set D1_BULLSHCRIPT_DIR)";
const SIDECAR = join(dirname(fileURLToPath(import.meta.url)), "../src/sidecar.mjs");

describe("sidecar process", { skip }, () => {
  let dir;
  before(() => {
    dir = writeSyntheticDir();
  });
  after(() => dir?.cleanup());

  const run = (lines, env = {}) => {
    const input = `${lines.map((l) => (typeof l === "string" ? l : JSON.stringify(l))).join("\n")}\n`;
    const r = spawnSync(process.execPath, [SIDECAR], {
      encoding: "utf8",
      env: { ...process.env, D1_SCRIPT_HOST_LOG: "quiet", ...env },
      input,
    });
    assert.equal(r.status, 0, r.stderr);
    return r.stdout.trim().split("\n").map((l) => JSON.parse(l));
  };

  it("answers hello, start and ticks in order, and says bye", () => {
    const ticks = Array.from({ length: 12 }, (_, i) => ({ now: i + 1, type: "tick" }));
    const out = run([
      { activity: "test_cine", build: "56578", proto: 1, scripts_dir: dir.dir, type: "hello" },
      { type: "start" },
      { key: "1", type: "player_spawned" },
      ...ticks,
      { type: "end" },
    ]);
    const types = out.map((m) => m.type);
    assert.equal(types[0], "loaded");
    assert.equal(types[1], "ready");
    assert.equal(types.at(-1), "bye");
    assert.equal(types.filter((t) => t === "tick_done").length, 12);
    assert.ok(!types.includes("log"), "Lua notes stay off the wire unless asked");
    const rows = out.filter((m) => m.type === "rows").flatMap((m) => m.changes);
    assert.ok(rows.some((c) => c.kind === "cinematic"));
    // every rows message comes before the tick_done of its tick
    const lastRows = types.lastIndexOf("rows");
    assert.ok(types.indexOf("tick_done", lastRows) > lastRows);
  });

  it("sends Lua notes when hello asks", () => {
    const out = run([
      { activity: "test_cine", log: "lua", scripts_dir: dir.dir, type: "hello" },
      { type: "start" },
      { type: "end" },
    ]);
    assert.ok(out.some((m) => m.type === "log"));
  });

  it("refuses a wrong build or protocol, and messages before hello", () => {
    const out = run([
      { type: "tick" },
      { activity: "test_cine", build: "41375", scripts_dir: dir.dir, type: "hello" },
      { activity: "test_cine", proto: 99, scripts_dir: dir.dir, type: "hello" },
      { activity: "nope", scripts_dir: dir.dir, type: "hello" },
      "not json",
      { type: "end" },
    ]);
    const errors = out.filter((m) => m.type === "error").map((m) => m.text);
    assert.match(errors[0], /tick before hello/);
    assert.match(errors[1], /build 41375 not supported/);
    assert.match(errors[2], /protocol 99 not supported/);
    assert.match(errors[3], /no script starts activity nope/);
    assert.match(errors[4], /not JSON/);
  });

  it("makes a hello that throws fatal, and a later failure not", async () => {
    const out = [];
    const sidecar = new Sidecar({ logLua: false, write: (m) => out.push(m) });
    await sidecar.line(
      JSON.stringify({ activity: "test_stub", bundles_path: join(dir.dir, "no-such-export.json"), scripts_dir: dir.dir, type: "hello" }),
    );
    const hello = out.find((m) => m.type === "error");
    assert.ok(hello, "the unreadable export is reported");
    assert.equal(hello.fatal, true, "a hello failure is fatal");
    assert.equal(out.some((m) => m.type === "loaded"), false);
    out.length = 0;
    await sidecar.line(JSON.stringify({ activity: "test_stub", scripts_dir: dir.dir, type: "hello" }));
    await sidecar.line(JSON.stringify({ type: "trigger", name: 12 }));
    for (const m of out.filter((x) => x.type === "error")) {
      assert.equal(m.fatal, false, "after hello, errors are not fatal");
    }
  });

  it("drives a session in-process through the same line handler", async () => {
    const out = [];
    const sidecar = new Sidecar({ logLua: false, write: (m) => out.push(m) });
    await sidecar.line(JSON.stringify({ activity: "test_stub", scripts_dir: dir.dir, type: "hello" }));
    await sidecar.line(JSON.stringify({ type: "start" }));
    assert.equal(out[0].type, "loaded");
    assert.equal(out[0].stub, true);
    assert.equal(out[0].drives_lifetime, false, "a start-only stub leaves the lifetime to the host");
    assert.equal(await sidecar.line(JSON.stringify({ type: "end" })), false);
  });
});
