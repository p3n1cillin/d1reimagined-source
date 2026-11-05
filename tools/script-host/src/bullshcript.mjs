// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// Finds Blam Network's BullShcript (MIT) in a local checkout and loads its
// built ESM output in place. Nothing of BullShcript is vendored here: the
// sidecar depends on the checkout by path, the way THIRD_PARTY.md records.
//
// Where it looks, first match wins:
//   D1_BULLSHCRIPT_DIR   the package folder itself (…/packages/bullshcript)
//   D1_TIGER_DIR         a tiger checkout; the package is packages/bullshcript
//   ../third-party/Blam-Network_tiger and ../../third-party/Blam-Network_tiger
//                        beside the repository (the usual local layout)
//
// The package must be built (`npm install && npm run build -w
// @blamnetwork/bullshcript` in the tiger checkout) so dist/ exists.

import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

// BullShcript's nearest build to ours: PS4 01.29. Our 56578 differences are
// layered on top in types56578.mjs and natives56578.mjs.
export const BULLSHCRIPT_BUILD = "v52553_16_09_28_1551_evil";

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(HERE, "../../..");

export function bullshcriptCandidates(env = process.env) {
  const out = [];
  if (env.D1_BULLSHCRIPT_DIR) {
    out.push(resolve(env.D1_BULLSHCRIPT_DIR));
  }
  if (env.D1_TIGER_DIR) {
    out.push(resolve(env.D1_TIGER_DIR, "packages/bullshcript"));
  }
  for (const up of ["..", "../.."]) {
    out.push(
      resolve(REPO_ROOT, up, "third-party/Blam-Network_tiger/packages/bullshcript")
    );
  }
  return out;
}

function entryOf(dir) {
  return join(dir, "dist/versions/destiny1", `${BULLSHCRIPT_BUILD}.js`);
}

// The package folder, or undefined when no built checkout is found.
export function findBullshcript(env = process.env) {
  return bullshcriptCandidates(env).find((dir) => existsSync(entryOf(dir)));
}

let loaded;

// { dir, api, nativeRegistry, shape, support } where api is the 01.29 barrel and
// nativeRegistry the bound-object verb table its natives register into (one
// module instance: imported by the same file URL the barrel resolves).
export async function loadBullshcript(env = process.env) {
  if (loaded) {
    return loaded;
  }
  const dir = findBullshcript(env);
  if (!dir) {
    throw new Error(
      "BullShcript not found: set D1_BULLSHCRIPT_DIR (or D1_TIGER_DIR) to a " +
        "built checkout; looked in " +
        bullshcriptCandidates(env).join(", ")
    );
  }
  const url = (path) => pathToFileURL(join(dir, "dist", path)).href;
  const api = await import(url(`versions/destiny1/${BULLSHCRIPT_BUILD}.js`));
  const bound = await import(url("host/bound/bound_object.js"));
  const support = await import(url("host/natives/native_support.js"));
  const shape = await import(url("hks/env/script_shape.js"));
  loaded = {
    api,
    dir,
    nativeRegistry: bound.nativeRegistry,
    shape,
    support,
  };
  return loaded;
}
