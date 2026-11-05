// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// Natives the 56578 scripts call that BullShcript's 01.29 host does not
// answer. Each one here is a STUB: there is no 56578 evidence for what it
// does yet. A stub never makes a script succeed on our guess: a command
// does nothing and answers true (as BullShcript's own fallback does), a
// predicate answers false so the script holds where it is. Every call logs
// `stub56578 <path>:<verb>`, so a run says which stubs it leaned on.
//
// Registered into BullShcript's bound-object verb table (MIT) as an
// extension; nothing in the checkout is modified. A verb BullShcript already
// has is never replaced.

import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

// The verbs, with what they answer and what is known. From the coverage
// report (tools/coverage.mjs): each was called by a 56578 activity and
// answered by nobody.
export const STUB_VERBS = Object.freeze({
  // Utility archetypes call it on their trigger volume at load time.
  // Command; no 56578 effect known (the name suggests it drops the volume's
  // slice-set assignment).
  import_only_clear_slice_set_assignment: {
    answer: true,
    why: "load-time wiring, effect unknown",
  },
});

let installed;

// Registers the stubs once per process (the registry is a module
// singleton). Returns the names registered. `log` receives each stub call.
export function installNatives56578(bs, log) {
  installed ??= { log, names: new Set() };
  installed.log = log;
  if (installed.registry === bs.nativeRegistry) {
    return installed.names;
  }
  installed.registry = bs.nativeRegistry;
  for (const [verb, { answer, why }] of Object.entries(STUB_VERBS)) {
    if (bs.nativeRegistry.lookup(verb)) {
      continue;
    }
    bs.nativeRegistry.verb(verb, (self, _args, method) => {
      installed.log(`stub56578 ${self.path}:${method} -> ${String(answer)} (${why})`);
      return answer;
    });
    installed.names.add(verb);
  }
  return installed.names;
}

// Row types whose verbs BullShcript answers per row type (rowVerb), which
// the verb table cannot list: a static check skips them.
const ROW_VERB_TYPES = new Set(["lifetime", "loot", "passenger", "scoreboard"]);
// The orders a command_script block takes, as BullShcript's command_script.ts
// builds them (names only).
const COMMAND_SCRIPT_ORDERS = new Set([
  "ability", "clear", "face", "follow_path", "move_to", "point_targeted_ability",
  "scripted_sequence", "set_channel_scalar", "set_flag", "sleep", "snap_to",
  "terminate_delay", "terminate_immediate",
]);
const COMMAND_SCRIPT_TYPES = new Set(["c_command_script", "command_script"]);
const ARRAY_OF = /^(?:static|dynamic)_array<\s*([A-Za-z_]\w*)/;

// Every verb a set of scripts calls on a native-typed member (self.<field>:
// verb()) that BullShcript's verb table does not have: what a run could hit
// that nothing answers. `nativeTypes` is the set of native spellings.
// Returns [{method, type, calls, files}], by method.
export function staticMissingNatives(bs, scripts, paths) {
  const shape = bs.shape;
  const nativeTypes = new Set(bs.api.NATIVE_TYPES.map((t) => t.spelling));
  // A verb some script defines (a def or thread) is the script's, reached
  // through a handle: encounter_handle.boss_is_dead() and the like.
  const scriptDefined = new Set();
  for (const decl of scripts.compiled.decls) {
    for (const m of decl.schema?.members ?? []) {
      if (m.kind === "method" || m.kind === "thread") scriptDefined.add(m.name);
    }
  }
  const found = new Map();
  for (const path of paths) {
    for (const decl of scripts.declsByPath.get(path) ?? []) {
      if (!decl.chunk) continue;
      const types = new Map(
        (decl.schema?.members ?? []).map((m) => [m.name, ARRAY_OF.exec(m.type)?.[1] ?? m.type])
      );
      const chunk = bs.api.readBytecode(decl.chunk, path);
      for (const [field, methods] of shape.fieldMethodCalls(chunk)) {
        const type = types.get(field);
        if (!type || !nativeTypes.has(type) || type === "thread" || ROW_VERB_TYPES.has(type)) {
          continue;
        }
        for (const method of methods) {
          if (COMMAND_SCRIPT_TYPES.has(type) ? COMMAND_SCRIPT_ORDERS.has(method) : scriptDefined.has(method)) {
            continue;
          }
          const stubbed = installed?.names.has(method) === true;
          if (bs.nativeRegistry.lookup(method) && !stubbed) continue;
          const entry = found.get(method) ?? { files: new Set(), method, stubbed, types: new Set() };
          entry.files.add(path);
          entry.types.add(type);
          found.set(method, entry);
        }
      }
    }
  }
  return [...found.values()]
    .map((e) => ({ files: e.files.size, method: e.method, stubbed: e.stubbed, types: [...e.types].sort() }))
    .sort((a, b) => a.method.localeCompare(b.method));
}

// Every name BullShcript's host answers somewhere, read off its built
// output: verb registrations and the string cases its dispatchers switch on
// (library methods, thread verbs, command-script orders, row verbs), plus
// its builtins and native base members. Generous on purpose: a name counts
// as answered if any receiver answers it.
let answeredCache;
export function answeredNames(bs) {
  if (answeredCache?.dir === bs.dir) return answeredCache.names;
  const names = new Set();
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) {
        walk(path);
      } else if (path.endsWith(".js")) {
        const text = readFileSync(path, "utf8");
        for (const m of text.matchAll(/(?:case |=== |verb\(|\[)\s*"([a-z_][a-z0-9_]*)"/g)) {
          names.add(m[1]);
        }
        for (const m of text.matchAll(/verb\(\s*\[([^\]]*)\]/g)) {
          for (const n of m[1].matchAll(/"([a-z_][a-z0-9_]*)"/g)) names.add(n[1]);
        }
      }
    }
  };
  walk(join(bs.dir, "dist/host"));
  walk(join(bs.dir, "dist/hks/env"));
  const api = bs.api;
  for (const n of [...api.BUILTINS, ...api.NATIVE_BASE_METHODS, ...api.NATIVE_DELEGATES]) {
    names.add(n);
  }
  for (const base of api.NATIVE_BASES) for (const n of base.members) names.add(n);
  answeredCache = { dir: bs.dir, names };
  return names;
}

// Method-call names (`.name(`) in a set of script files that no script
// defines and BullShcript answers nowhere, with how many files call each.
// Reads the sources locally; only names leave.
export function unansweredByName(bs, scripts, paths) {
  const answered = answeredNames(bs);
  const defined = new Set();
  for (const decl of scripts.compiled.decls) {
    defined.add(decl.name);
    for (const m of decl.schema?.members ?? []) defined.add(m.name);
  }
  const found = new Map();
  for (const path of paths) {
    const text = scripts.byPath.get(path)?.text ?? "";
    for (const m of text.replace(/#.*$/gm, "").matchAll(/\.([a-z_][a-z0-9_]*)\s*\(/g)) {
      const name = m[1];
      if (answered.has(name) || defined.has(name)) continue;
      const entry = found.get(name) ?? { calls: 0, files: new Set() };
      entry.calls += 1;
      entry.files.add(path);
      found.set(name, entry);
    }
  }
  return [...found].map(([method, e]) => ({ calls: e.calls, files: e.files.size, method }));
}

// A runtime log line saying nothing answered a native, as BullShcript writes
// them: `<path>:<verb>(<args>) (unbound…)` or `(unknown, false)`.
const UNANSWERED =
  /^lua (.*?):([A-Za-z_]\w*)\((.*)\) \((unbound[^,)]*|unknown)(?:, [^)]*)?\)$/;

export function missingNativeOf(line) {
  const match = UNANSWERED.exec(line);
  if (!match) {
    return;
  }
  return { method: match[2], on: match[1], why: match[4] };
}
