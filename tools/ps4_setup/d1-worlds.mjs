// d1-worlds.mjs: read every world's scenario record, bubbles, sensor bundles and insertion points
// from build 56578's own packages, for the activity host's generated world tables
// (tools/ps4_setup/d1-world-table.py turns this into ps4/fixtures/56578/worlds/).
//
//   node tools/ps4_setup/d1-worlds.mjs <dump>/CUSA00219/packages OUT.json [WORLD ...]
//
// For each `<world>:scenario_client` named tag (224 on 56578) it writes the record's facts and the
// chain the client walks when a slice set loads:
//
//   scenario_client                    +0x08 fnv1(world name), +0x0C PRIVATE, +0x10 PUBLIC checksum,
//     .bubbles[b].states[s]            slice set 8*b+s; `mode` != 0 is a PUBLIC region
//       .descriptor -> slice descriptor (808008F0): the static world's hash and bubble index
//         .blueprint_set -> client blueprint set: global / authoritative / private bundle tags
//           -> activity bundle (8080076E): the sensor group (fnv1 of the bundle name), its rows and
//              the lookup groups that register sensor clients per (bubble, phase), each row's host
//              definition naming its A (auth) and B (sense) value types.
//
// and, per static world (a manifest root, 8080052E) and bubble, the name hashes of the spawn points
// the bubble loads: the insertion points the client picks among by the transition name the server
// sends (else `default`, 0x2EA8FB98). Positions are not kept.
//
// And the client's activity table (one tag of class 80801915, the investment globals' activities):
// per activity index its hash and the name its definition carries at +0x58, the activity's bundle
// name, which is the world it loads: `<name>:scenario_client` (see readActivityTable).
//
// The bundles section is keyed by bundle TAG (one bundle name can have several tags). Row names are
// kept, so the output is game data: write it outside the repository. d1-world-table.py reads it and
// commits identifiers only.
//
// Reads packages with gibbon, from Blam Network's tiger libraries
// (https://github.com/Blam-Network/tiger, MIT), whose 56578 definitions match our build. Build that
// repository (npm ci && npm run build) and point D1_TIGER at its folder (default
// F:/d1/third-party/Blam-Network_tiger). All 224 worlds take about a minute.
import { createRequire } from "node:module";
import { writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const BUILD = "56578.17.04.12.1251.evil";
export const EMPTY = 0xffffffff;
const SUFFIX = ":scenario_client";

export const hex = (n) => `0x${(n >>> 0).toString(16).padStart(8, "0")}`;

// FNV-1 (multiply, then xor), as every Tiger name hash.
export function fnv1(s) {
  let h = 0x811c9dc5;
  for (const b of Buffer.from(s, "utf8")) h = (Math.imul(h, 0x01000193) ^ b) >>> 0;
  return h >>> 0;
}

export function loadGibbon(tiger = process.env.D1_TIGER ?? "F:/d1/third-party/Blam-Network_tiger") {
  // The CommonJS build: the ES-module one imports cstruct without file extensions, which Node 22 refuses.
  return createRequire(import.meta.url)(
    path.join(tiger, "packages/gibbon/dist-cjs/versions/destiny1/v56578_17_04_12_1251_evil.js")
  );
}

export function readWorlds(g, set, only = []) {
  const bundles = {};
  const problems = [];

  // A row's host definition: the sensor head names the row it serves and its value types.
  function host_definition(tag) {
    const data = set.try_read_tag(tag);
    if (!data) return undefined;
    const off = g.checked_host_offsets(data, g.k_sensor_host_definition_size);
    if (!off) return undefined;
    const d = g.read_definition(data.subarray(off.definition), g.s_sensor_host_definition);
    return {
      bundle: hex(d.sensor.bundle),
      type: d.sensor.type,
      index: d.sensor.index,
      slice_set: d.slice_set,
      authority: d.authority,
      a: d.auth_schema === EMPTY ? null : hex(d.auth_schema),
      b: d.sense_schema === EMPTY ? null : hex(d.sense_schema),
    };
  }

  function bundle(tag) {
    const key = hex(tag);
    if (key in bundles) return bundles[key];
    const data = set.try_read_tag(tag);
    if (!data) {
      problems.push(`bundle ${key}: no tag`);
      return (bundles[key] = null);
    }
    const b = g.read_definition(data, g.s_activity_bundle);
    const lookups = b.lookup_groups.map((lg) => ({
      slice_set: lg.slice_set,
      phase: hex(lg.phase_hash),
      rows: lg.lookups.flatMap((l) => {
        const ld = set.try_read_tag(l.tag);
        if (!ld) return [];
        const h = host_definition(g.read_definition(ld, g.s_activity_lookup).host_tag);
        // A row group (firing areas, trigger volumes ...) has no sensor head: no client.
        return h && h.bundle === hex(b.bundle) ? [[h.index, h.type, h.a, h.b, h.authority, h.slice_set]] : [];
      }),
    }));
    return (bundles[key] = {
      bundle: hex(b.bundle),
      world_hash: hex(b.world_hash),
      slotted: b.slotted_row_count,
      rows: b.rows.map((r) => [r.type, r.name ?? ""]),
      lookups,
    });
  }

  function blueprint(descriptor) {
    const sd = set.try_read_tag(descriptor);
    if (!sd) return { error: `descriptor ${hex(descriptor)}: no tag` };
    const d = g.read_definition(sd, g.s_slice_descriptor);
    const bs = set.try_read_tag(d.blueprint_set);
    if (!bs) return { world_hash: hex(d.world_hash), bubble_index: d.bubble_index, error: "no blueprint set" };
    const s = g.read_definition(bs, g.s_client_blueprint_set);
    const tags = (list) => list.map((r) => (bundle(r.tag) ? hex(r.tag) : null)).filter(Boolean);
    return {
      world_hash: hex(d.world_hash),
      bubble_index: d.bubble_index,
      blueprint_set: hex(d.blueprint_set),
      global: tags(s.global_bundles),
      authoritative: tags(s.authoritative_bundles),
      private: tags(s.private_bundles),
    };
  }

  const worlds = [];
  const named = set.named_tags().filter((n) => n.name.endsWith(SUFFIX));
  named.sort((a, b) => a.hash - b.hash);
  for (const n of named) {
    const name = n.name.slice(0, -SUFFIX.length);
    if (only.length && !only.includes(name)) continue;
    const r = g.read_definition(set.read_tag(n.hash), g.s_scenario_client);
    if (r.family_checksum >>> 0 !== fnv1(name)) {
      problems.push(`${name}: +0x08 ${hex(r.family_checksum)} is not fnv1(name) ${hex(fnv1(name))}`);
    }
    worlds.push({
      name,
      scenario_tag: hex(n.hash),
      family: hex(r.family_checksum),
      private: hex(r.private_checksum),
      public: hex(r.public_checksum),
      destination: r.destination_name ?? "",
      world_hash: hex(r.world_hash),
      bubbles: r.bubbles.map((b, bi) => ({
        region_name_hash: hex(b.region_name_hash),
        states: b.states.map((s, si) => ({
          slice_set: 8 * bi + si,
          valid: s.valid,
          public: s.mode !== 0,
          region_name_hash: hex(s.region_name_hash),
          descriptor: hex(s.descriptor),
          ...blueprint(s.descriptor),
        })),
      })),
    });
  }
  return { worlds, bundles, problems };
}

// Per static world hash, per bubble index: [[spawn point name hash, count], ...], most points
// first. Only the worlds the scenarios use (their descriptors' world hashes) are read.
export function readInsertionPoints(g, set, worldHashes, problems = []) {
  const roots = g.find_worlds(set);
  const out = {};
  for (const wh of worldHashes) {
    const root = roots.get(parseInt(wh, 16) >>> 0);
    if (root === undefined) continue;
    const w = g.read_definition(set.read_tag(root), g.s_world);
    out[wh] = w.bubbles.map(({ tag }) => {
      const counts = new Map();
      try {
        for (const sp of g.find_world_bubble_spawn_point_sets(set, tag)) {
          for (const p of g.read_definition(set.read_tag(sp), g.s_spawn_point_set).points) {
            counts.set(p.name_hash >>> 0, (counts.get(p.name_hash >>> 0) ?? 0) + 1);
          }
        }
      } catch (e) {
        problems.push(`world ${wh} bubble ${hex(tag)}: ${e.message}`);
        return null;
      }
      return [...counts].sort((a, b) => b[1] - a[1] || a[0] - b[0]).map(([h, n]) => [hex(h), n]);
    });
  }
  return out;
}

// The client's activity table (investment globals, class 80801915): a tag block (+0x08) of 16-byte
// entries {u32 activity hash, u32 0, i64 pointer to the definition}. The definition is the 360's
// s_activity_definition (gibbon v41375 activity_definition.ts) widened to 64 bits: it starts with
// the activity hash, and its `name` (the activity's bundle name, a self-relative string pointer)
// sits at +0x58, its `fallback_name` at +0x60. On 56578 the name is the world the activity loads:
// every live host the captures cross-check served the world its activity names. No name means the
// definition names no world (playlists, the Crucible's modes, the credits).
export const k_activity_table_class = 0x80801915;
export const k_activity_table_entry_class = 0x808018a7;
export const k_activity_definition_name = 0x58;
export const k_activity_definition_fallback_name = 0x60;

export function readActivityTable(g, set, problems = []) {
  const refs = set.entries_of_class(k_activity_table_class);
  if (refs.length !== 1) {
    problems.push(`activity table: ${refs.length} tags of class ${hex(k_activity_table_class)}, not 1`);
    return [];
  }
  const data = set.read_tag(refs[0].hash);
  const header = g.tag_block_header(data, 8);
  if (header === undefined || data.readUInt32LE(header + 8) !== k_activity_table_entry_class) {
    problems.push(`activity table ${hex(refs[0].hash)}: no ${hex(k_activity_table_entry_class)} block at +0x08`);
    return [];
  }
  const count = Number(data.readBigUInt64LE(header));
  const str = g.tag_relative_string();
  const out = [];
  for (let index = 0; index < count; index++) {
    const at = header + 16 + 16 * index;
    const hash = data.readUInt32LE(at);
    const def = at + 8 + Number(data.readBigInt64LE(at + 8));
    if (def <= 0 || def + 0x68 > data.length || data.readUInt32LE(def) !== hash) {
      problems.push(`activity ${index} ${hex(hash)}: definition does not start with its hash`);
      out.push({ index, hash: hex(hash), name: null, fallback_name: null });
      continue;
    }
    out.push({
      index,
      hash: hex(hash),
      name: str.read(data, def + k_activity_definition_name) ?? null,
      fallback_name: str.read(data, def + k_activity_definition_fallback_name) ?? null,
    });
  }
  return out;
}

function main(argv) {
  const [dir, out, ...only] = argv;
  if (!dir || !out) {
    console.error("usage: d1-worlds.mjs <packages dir> OUT.json [WORLD ...]");
    process.exit(2);
  }
  const g = loadGibbon();
  const set = g.c_package_set.open_directory(dir);
  const { worlds, bundles, problems } = readWorlds(g, set, only);
  const hashes = new Set();
  for (const w of worlds) for (const b of w.bubbles) for (const s of b.states) if (s.world_hash) hashes.add(s.world_hash);
  const insertion_points = readInsertionPoints(g, set, [...hashes].sort(), problems);
  const activities = only.length ? [] : readActivityTable(g, set, problems);
  writeFileSync(
    out,
    JSON.stringify({
      build: BUILD,
      generator: "tools/ps4_setup/d1-worlds.mjs",
      worlds,
      bundles,
      insertion_points,
      activities,
      problems,
    }) + "\n"
  );
  console.error(
    `${worlds.length} worlds, ${Object.keys(bundles).length} bundle tags, ${activities.length} activities, ` +
      `${Object.keys(insertion_points).length} static worlds with insertion points, ${problems.length} problems -> ${out}`
  );
  for (const p of problems.slice(0, 20)) console.error("  " + p);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2));
}
