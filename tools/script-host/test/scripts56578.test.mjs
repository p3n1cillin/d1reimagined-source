// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// Against the local 56578 script folder (D1_SCRIPTS_DIR), when there is one.
// The scripts are unlicensed and never in this repository: these tests read
// them in place and check only counts, names and states.

import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { describe, it } from "node:test";
import { coverage } from "../tools/coverage.mjs";
import { findBullshcript } from "../src/bullshcript.mjs";

const dir = process.env.D1_SCRIPTS_DIR;
const skip = !findBullshcript()
  ? "no built BullShcript checkout"
  : !(dir && existsSync(dir))
    ? "D1_SCRIPTS_DIR is not set to the 56578 script folder"
    : false;

describe("the 56578 scripts", { skip }, () => {
  it("compile and load every activity, and each written one reaches its first phase", async () => {
    const report = await coverage({ scriptsDir: dir, seconds: 5 });
    assert.equal(report.files, 380);
    const written = report.results.filter((r) => !r.stub);
    assert.equal(written.length, 30);
    assert.deepEqual(report.results.filter((r) => !r.compiles || !r.loads).map((r) => r.activity), []);
    assert.deepEqual(written.filter((r) => !r.first_phase).map((r) => r.activity), []);
  });

  it("runs the reveal (165) through 1, 4, 6, 8 with cinematics finished by the driver", async () => {
    const report = await coverage({ only: ["cine_reveal"], scriptsDir: dir, seconds: 120 });
    const [r] = report.results;
    assert.deepEqual(r.lifetime, [1, 4, 6, 8]);
    assert.equal(r.phases_reached, 2);
    assert.equal(r.ended, true);
    assert.equal(r.thread_failures, 0);
  });
});
