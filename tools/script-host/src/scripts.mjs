// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// A folder of mission scripts, compiled once in memory with BullShcript, and
// what the sidecar needs of it: the activities it starts, the script source
// the runtime reads chunks from, and stand-in bundles for when the host has
// no content export to give.
//
// The 56578 scripts (Blam Network's Fukugen Daihon) are UNLICENSED: they are
// read from D1_SCRIPTS_DIR at run time and nothing derived from them is
// written to disk or to this repository. Only names and counts leave here.

import {
  BINDER_EXTRA_ROWS,
  BINDER_PLAYER_ROWS,
  HANDLE_TYPES,
  NATIVE_ROW_TYPE,
  STAND_IN_ROW_TYPE,
} from "./types56578.mjs";

const ARRAY_TYPE = /^(static|dynamic)_array<\s*([A-Za-z_]\w*)\s*(?:,\s*(\d+)\s*)?>$/;
const MAX_DEPTH = 8;

export function scriptsDirFromEnv(env = process.env) {
  return env.D1_SCRIPTS_DIR || undefined;
}

// A compiled script folder. `files` are BullShcript ScriptFiles (or any
// {path, text} list for synthetic tests).
export class ScriptSet {
  constructor(api, files, { dir } = {}) {
    this.api = api;
    this.dir = dir;
    this.files = files.map((file) =>
      file.chunkName ? file : api.parseScriptFile(file.path, file.text)
    );
    this.lookups = api.scriptLookups(this.files);
    this.compiled = api.compileScriptFiles(this.files);
    this.byPath = new Map(this.files.map((file) => [file.path, file]));
    this.errorsByPath = new Map();
    for (const d of this.compiled.diagnostics) {
      if (d.severity === "error") {
        const list = this.errorsByPath.get(d.path) ?? [];
        list.push(`${d.line}: ${d.message}`);
        this.errorsByPath.set(d.path, list);
      }
    }
    this.declsByPath = new Map();
    for (const decl of this.compiled.decls) {
      const list = this.declsByPath.get(decl.path) ?? [];
      list.push(decl);
      this.declsByPath.set(decl.path, list);
    }
  }

  static fromDir(api, dir) {
    const folder = api.loadScriptFolder(dir);
    return new ScriptSet(api, folder.files, { dir });
  }

  // The compiled class decl of a file (its first class), or undefined.
  classDecl(path) {
    return (this.declsByPath.get(path) ?? []).find((d) => d.kind === "class");
  }

  schemaOf(path) {
    return this.classDecl(path)?.schema;
  }

  // Every activity some root names, in path order.
  activities() {
    const out = [];
    for (const file of this.files) {
      for (const name of file.activityStart?.activities ?? []) {
        out.push(this.activity(name));
      }
    }
    return out.filter(Boolean);
  }

  activity(name) {
    const start = this.lookups.activityStart(name);
    if (!start) {
      return;
    }
    const root = this.byPath.get(start.script);
    const missions = root ? this.lookups.missionScripts(root) : [];
    return {
      bundle: root?.activityBundle,
      destination: start.destination,
      missions: missions.map((file) => file.path),
      name,
      phases: start.phases ?? [],
      root: start.script,
      start,
      stub: missions.length === 0,
    };
  }

  // The file whose class is named `decl`.
  fileOfDecl(decl) {
    return this.lookups.archetypeScript(decl);
  }

  // Every file a set of roots runs: members (archetypes), missions and their
  // members, transitively.
  closure(paths) {
    const seen = new Set();
    const queue = [...paths];
    while (queue.length > 0) {
      const path = queue.shift();
      if (seen.has(path)) {
        continue;
      }
      seen.add(path);
      const file = this.byPath.get(path);
      if (!file) {
        continue;
      }
      for (const next of [
        ...this.lookups.memberScripts(file),
        ...this.lookups.missionScripts(file),
      ]) {
        queue.push(next.path);
      }
    }
    return [...seen];
  }

  compileErrors(paths) {
    return paths.flatMap((path) =>
      (this.errorsByPath.get(path) ?? []).map((e) => `${path}:${e}`)
    );
  }

  // Whether any script of the set reaches the lifetime sensor itself. Without
  // one (a start-only stub) nothing moves the lifetime: the host decides.
  drivesLifetime(paths) {
    return paths.some((path) =>
      (this.declsByPath.get(path) ?? []).some((d) =>
        d.lua?.includes("get_lifetime_sensor")
      )
    );
  }

  scriptTagOf(path) {
    return this.api.fnv1(path) >>> 0;
  }

  // The file a bundle's own script is: by its @activity_bundle, preferring
  // the destination's folder.
  scriptForBundle(bundleHash, destination) {
    return (
      this.lookups.scriptByBundle(bundleHash >>> 0, destination) ??
      this.lookups.scriptByBundle(bundleHash >>> 0)
    );
  }

  // A BullShcript ScriptSource over the compiled set: one tag per file
  // (fnv1 of its path), its chunk padded to a word, then the tags of the
  // scripts it references as big-endian words (the layout
  // scriptReferences() scans). `refs` adds per-bundle archetypes.
  scriptSource() {
    const fmt = this.api.formatScriptTag;
    const entries = new Map();
    for (const file of this.files) {
      const decl = this.classDecl(file.path);
      if (!decl?.chunk) {
        continue;
      }
      const refs = [
        ...this.lookups.memberScripts(file),
        ...this.lookups.missionScripts(file),
      ]
        .filter((ref) => this.classDecl(ref.path)?.chunk)
        .map((ref) => this.scriptTagOf(ref.path));
      entries.set(fmt(this.scriptTagOf(file.path)), { decl, refs });
    }
    const tags = [...entries.keys()];
    return {
      read: (tagId) => {
        const entry = entries.get(tagId.toUpperCase());
        if (!entry) {
          return;
        }
        const chunk = entry.decl.chunk;
        const padded = Math.ceil(chunk.length / 4) * 4;
        const out = new Uint8Array(padded + entry.refs.length * 4);
        out.set(chunk);
        const view = new DataView(out.buffer);
        entry.refs.forEach((ref, i) => view.setUint32(padded + i * 4, ref));
        return out;
      },
      schema: (tagId) => entries.get(tagId.toUpperCase())?.decl.schema,
      tags: () => tags,
    };
  }
}

// Which binder a stand-in activity uses. Content says for real; this is the
// folder convention (tower, strikes, everything else).
export function standInBinder(scripts, activity) {
  const binders = scripts.files.filter(
    (f) => f.kind === "bs" && f.path.startsWith("bs/activities/common/")
  );
  const named = (stem) => binders.find((f) => f.path.endsWith(`/${stem}.bs`));
  if (activity.destination === "city_tower") {
    return named("tower_common") ?? named("story_common");
  }
  if (/portal|strike/.test(activity.name)) {
    return named("portal_common") ?? named("story_common");
  }
  return named("story_common") ?? binders[0];
}

// Stand-in bundles for an activity: rows named and typed from the scripts'
// own member declarations, the way the activity bundles name them
// (`<phase>.<member>`, `<phase>.<member>.<archetype member>`). They let a
// script run with no content export; every typeIndex is ours, not the game's,
// so they must never reach the wire. `extra` names more bundles to load
// (area encounters: enc_highway_m0, ...), which only content can list.
export function standInBundles(scripts, activity, types, { extra = [] } = {}) {
  const notes = [];
  const bundles = [];
  const api = scripts.api;
  const rowType = (spelling) => {
    const key = NATIVE_ROW_TYPE[spelling] ?? STAND_IN_ROW_TYPE[spelling];
    if (key && STAND_IN_ROW_TYPE[spelling] && !NATIVE_ROW_TYPE[spelling]) {
      notes.push(`stand-in row type: ${spelling} -> ${key}`);
    }
    return key === undefined ? undefined : types[key];
  };
  const unmapped = new Set();

  // @member_counts by squad row name: the stand-in squad definition.
  let counts;
  const addBundle = (file, build) => {
    if (!file || file.activityBundle === undefined) {
      return;
    }
    const rows = [];
    counts = new Map();
    const push = (name, typeId, extraFields = {}) => {
      rows.push({
        name,
        nameHash: api.fnv1(name),
        typeId,
        typeIndex: rows.length,
        ...extraFields,
      });
    };
    build(push);
    // A squad whose script authors its members gets those members as its
    // definition (cells and member names): content would give the real one.
    for (const row of rows) {
      const authored = row.typeId === types.Squad ? counts.get(row.name) : undefined;
      if (authored) {
        Object.assign(row, standInSquadDefinition(authored));
        notes.push("stand-in squad definitions from @member_counts");
      }
    }
    bundles.push({
      bundle: file.activityBundle >>> 0,
      path: file.path,
      rows,
      tag: scripts.scriptTagOf(file.path),
    });
  };

  const membersInto = (push, path, prefix, depth, stack) => {
    const schema = scripts.schemaOf(path);
    if (!schema || depth > MAX_DEPTH || stack.includes(path)) {
      return;
    }
    const at = (name) => (prefix ? `${prefix}.${name}` : name);
    for (const member of schema.members ?? []) {
      for (const { members, path: sub } of member.memberCounts ?? []) {
        const field =
          sub === undefined
            ? member.name
            : `${member.name}${sub.startsWith("[") ? "" : "."}${sub}`;
        if (!counts.has(at(field))) {
          counts.set(at(field), members);
        }
      }
      const array = ARRAY_TYPE.exec(member.type);
      const base = array ? array[2] : member.type;
      const count = array
        ? array[1] === "static"
          ? Number(array[3] ?? 0)
          : 1
        : undefined;
      const names =
        count === undefined
          ? [at(member.name)]
          : Array.from({ length: count }, (_, i) => at(`${member.name}[${i}]`));
      const script = scripts.fileOfDecl(base);
      if (script && (member.kind === "class_instance" || array)) {
        if (script.kind === "esp") {
          // A root's mission: the phase row its member names.
          for (const name of names) {
            push(name, types.Phase);
            membersInto(push, script.path, name, depth + 1, [...stack, path]);
          }
          continue;
        }
        for (const name of names) {
          push(name, types.Encounter, { childScript: base });
          membersInto(push, script.path, name, depth + 1, [...stack, path]);
        }
        continue;
      }
      if (HANDLE_TYPES.has(base) || !["object", "handle", "static_array", "dynamic_array"].includes(member.kind)) {
        continue;
      }
      const typeId = rowType(base);
      if (typeId === undefined) {
        unmapped.add(base);
        continue;
      }
      for (const name of names) {
        push(name, typeId);
      }
    }
  };

  const binder = standInBinder(scripts, activity);
  addBundle(binder, (push) => {
    membersInto(push, binder.path, "", 0, []);
    for (const [name, key] of BINDER_EXTRA_ROWS) {
      if (types[key] !== undefined) {
        push(name, types[key]);
      }
    }
    for (let i = 0; i < BINDER_PLAYER_ROWS; i++) {
      push(`players[${i}]`, types.Player);
    }
  });
  const root = scripts.byPath.get(activity.root);
  addBundle(root, (push) => membersInto(push, root.path, "", 0, []));
  // The activity's own folder's other bundles (music, Ghost lines), then
  // whatever the caller names.
  const folder = activity.root.slice(0, activity.root.lastIndexOf("/") + 1);
  const seen = new Set(bundles.map((b) => b.path));
  const more = scripts.files.filter(
    (f) =>
      f.kind === "es" &&
      f.path.startsWith(folder) &&
      f.path.slice(folder.length).indexOf("/") < 0 &&
      f.activityBundle !== undefined &&
      !seen.has(f.path)
  );
  for (const name of extra) {
    const hash = typeof name === "number" ? name : api.fnv1(name);
    const file = scripts.scriptForBundle(hash, activity.destination);
    if (file && !seen.has(file.path) && !more.includes(file)) {
      more.push(file);
    } else if (!file) {
      notes.push(`no script for extra bundle ${name}`);
    }
  }
  for (const file of more) {
    seen.add(file.path);
    addBundle(file, (push) => membersInto(push, file.path, "", 0, []));
  }
  for (const base of unmapped) {
    notes.push(`no row type for native type ${base} (rows left out)`);
  }
  return { bundles, notes: [...new Set(notes)] };
}

// The cells a squad's @member_counts names, in the order written: each key
// takes as many cells as it has counts.
function standInSquadDefinition(members) {
  const cells = [];
  const memberNames = [];
  for (const { counts: given, hash } of members) {
    for (const count of given) {
      cells.push(Number(count));
      memberNames.push(hash >>> 0);
    }
  }
  return { cells, memberNames };
}

// Area encounters an activity probably loads, guessed from names: the
// mission tag of `act_<destination>_<tag>_<area>` (m0, m0_5, m01, ...) or a
// strike's portalN / strikeN, matched against `enc_<area>_<tag>` stems in
// the same destination. Only content knows the real list; this is labelled
// a guess wherever it is used.
export function guessAreaBundles(scripts, activity) {
  const tags = [];
  const mission = /^act_[a-z]+_(m\d+(?:_\d+)?)_/.exec(activity.name);
  if (mission) tags.push(mission[1]);
  const portal = /portal_?(\d+)/.exec(activity.name) ?? /portal(\d+)/.exec(activity.root);
  if (portal) tags.push(`portal${portal[1]}`);
  const strike = /strike_?(\d+)/.exec(activity.root);
  if (strike) tags.push(`strike${strike[1]}`);
  if (tags.length === 0 || !activity.destination) return [];
  const prefix = `es/${activity.destination}/`;
  return scripts.files
    .filter((f) => f.kind === "es" && f.path.startsWith(prefix))
    .map((f) => f.path.slice(f.path.lastIndexOf("/") + 1, -3))
    .filter((stem) => stem.startsWith("enc_") && tags.some((t) => stem.endsWith(`_${t}`)));
}

// A host-supplied content export: ActivityBundleTag[] as JSON
// ({bundle, tag, rows:[{name?, nameHash, typeId, typeIndex, ...}]}), hashes
// as numbers or "0x" strings.
export function parseBundles(json) {
  const num = (v) =>
    typeof v === "string" ? Number.parseInt(v, v.startsWith("0x") ? 16 : 10) >>> 0 : v;
  return json.map((tag) => ({
    ...tag,
    bundle: num(tag.bundle),
    rows: tag.rows.map((row) => ({ ...row, nameHash: num(row.nameHash) })),
    tag: num(tag.tag),
  }));
}
