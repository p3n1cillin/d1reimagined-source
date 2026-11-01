// d1-bundle-rows.mjs: name every sensor row of build 56578's activity bundles.
//
//   node tools/ps4_setup/d1-bundle-rows.mjs <dump>/CUSA00219/packages [GROUP_HASH ...] > rows.csv
//
// An activity bundle is a sensor group: its fnv1 name hash is the group the host's sensor lists
// use (0xFC0E45A3 = enc_breach_m0), and a row's index is its position in the bundle's table, the
// sensor row's index. Each row keeps its full name, so this maps every index we serve to a name.
//
// Reads packages with gibbon, from Blam Network's tiger libraries
// (https://github.com/Blam-Network/tiger, MIT). Build that repository (npm ci && npm run build)
// and point D1_TIGER at its folder (default F:/d1/third-party/Blam-Network_tiger). Output is CSV:
// group,bundle_tag,world_hash,slotted_rows,index,type,name. It is game data: keep it outside the repo.
import { createRequire } from "node:module";
import path from "node:path";

const tiger = process.env.D1_TIGER ?? "F:/d1/third-party/Blam-Network_tiger";
// The CommonJS build: the ES-module one imports cstruct without file extensions, which Node 22 refuses.
const g = createRequire(import.meta.url)(
  path.join(tiger, "packages/gibbon/dist-cjs/versions/destiny1/v56578_17_04_12_1251_evil.js")
);

const [dir, ...want] = process.argv.slice(2);
if (!dir) {
  console.error("usage: d1-bundle-rows.mjs <packages dir> [GROUP_HASH ...]");
  process.exit(2);
}
const groups = new Set(want.map((h) => Number.parseInt(h, 16) >>> 0));
const set = g.c_package_set.open_directory(dir);
const hex = (n) => `0x${(n >>> 0).toString(16).padStart(8, "0")}`;

console.log("group,bundle_tag,world_hash,slotted_rows,index,type,name");
for (const { hash } of set.entries_of_class(g.k_activity_bundle_class)) {
  const data = set.read_tag(hash);
  const head = g.read_definition(data, g.s_activity_bundle_head);
  if (groups.size && !groups.has(head.bundle >>> 0)) continue;
  const bundle = g.read_definition(data, g.s_activity_bundle);
  bundle.rows.forEach((row, index) => {
    console.log([hex(bundle.bundle), hex(hash), hex(bundle.world_hash), bundle.slotted_row_count,
      index, row.type, JSON.stringify(row.name)].join(","));
  });
}
