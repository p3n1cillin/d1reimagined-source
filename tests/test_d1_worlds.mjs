// SPDX-License-Identifier: GPL-2.0-or-later
// tools/ps4_setup/d1-worlds.mjs: hashes always; the owned packages when D1_PACKAGES (or the default
// dump) and Blam Network's tiger build (D1_TIGER) are present, skipped otherwise.
//
//   node --test tests/test_d1_worlds.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import path from "node:path";
import { fnv1, hex, loadGibbon, readWorlds, readInsertionPoints } from "../tools/ps4_setup/d1-worlds.mjs";

const tiger = process.env.D1_TIGER ?? "F:/d1/third-party/Blam-Network_tiger";
const packages =
  process.env.D1_PACKAGES ?? "F:/d1/d1preservation-main/dumps/later-56578-CUSA00219/CUSA00219/packages";
const gibbon = path.join(tiger, "packages/gibbon/dist-cjs/versions/destiny1/v56578_17_04_12_1251_evil.js");
const owned = existsSync(packages) && existsSync(gibbon);

test("world and bundle names hash as the client's records do", () => {
  assert.equal(fnv1("cine_speaker"), 0x796b3b8c);
  assert.equal(fnv1("cine_reveal"), 0x2fe7dbe6);
  assert.equal(fnv1("act_cosmo_m1_dome_a"), 0xff4a0c1a);
  assert.equal(fnv1("default"), 0x2ea8fb98);
  assert.equal(hex(0x80c7a005), "0x80c7a005");
  assert.equal(hex(-1), "0xffffffff");
});

test("the owned packages give the measured scenario records", { skip: !owned && "owned packages or tiger absent" }, () => {
  const g = loadGibbon(tiger);
  const set = g.c_package_set.open_directory(packages);
  const { worlds, bundles, problems } = readWorlds(g, set, ["cine_reveal", "city_tower_default1"]);
  assert.deepEqual(problems, []);
  const byName = Object.fromEntries(worlds.map((w) => [w.name, w]));
  assert.equal(byName.cine_reveal.private, "0x14cda256");
  assert.equal(byName.cine_reveal.bubbles.length, 4);
  assert.equal(byName.city_tower_default1.scenario_tag, "0x80c7a005");
  assert.equal(byName.city_tower_default1.private, "0x24340499");
  const s0 = byName.city_tower_default1.bubbles[0].states[0];
  assert.equal(s0.slice_set, 0);
  // Tower common, with its lifetime record (type 0x10) and 30 sim-player slots (type 12).
  const common = s0.global.map((t) => bundles[t]).find((b) => b.bundle === "0x40aebf90");
  const rows = common.lookups.flatMap((l) => l.rows);
  assert.equal(rows.filter((r) => r[1] === 12).length, 30);
  assert.ok(rows.some((r) => r[1] === 0x10));
  const points = readInsertionPoints(g, set, [s0.world_hash]);
  assert.ok(points[s0.world_hash][0].length > 0, "the Tower's bubble 0 has spawn points");
});
