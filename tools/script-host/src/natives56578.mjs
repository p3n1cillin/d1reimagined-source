// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// Natives the 56578 scripts call that BullShcript's 01.29 host does not
// answer. Each one here is a STUB: there is no 56578 evidence for what it
// does yet, so it answers the way BullShcript's own fallback would (a
// predicate false, a getter 0, a command true) and logs `stub56578 <verb>`
// every time it runs, so a run says which stubs it leaned on.
//
// Registered into BullShcript's bound-object verb table (MIT) as an
// extension; nothing in the checkout is modified. A verb BullShcript already
// has is never replaced.

// The verbs, with what is known. Filled from the coverage report
// (tools/coverage.mjs): every name below was called by some 56578 activity
// and answered by nobody.
export const STUB_VERBS = Object.freeze({});

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
  const { unboundAnswer } = bs.support;
  for (const [verb, why] of Object.entries(STUB_VERBS)) {
    if (bs.nativeRegistry.lookup(verb)) {
      continue;
    }
    bs.nativeRegistry.verb(verb, (self, _args, method) => {
      const answer = unboundAnswer(method);
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
