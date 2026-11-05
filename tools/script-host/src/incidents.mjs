// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// 56578 incident ids <-> FNV-1 name hashes, from Codex's catalogue
// (ps4/fixtures/56578/incidents-56578.tsv, PR #306: index, name_hash, name,
// payload_selector, aux). The four ids below were resolved from the owned
// 56578 registry in #304 and hold even with no catalogue on disk.

import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
export const DEFAULT_CATALOGUE = resolve(
  HERE,
  "../../../ps4/fixtures/56578/incidents-56578.tsv"
);

// name -> [id, name hash] (notes/ps4-56578-fukugen-daihon-2026-11-05.md).
export const KNOWN_56578 = Object.freeze({
  activity_complete: [965, 0x42660690],
  cinematic_finished: [891, 0x3c03f059],
  cinematic_skip: [1726, 0x7352dafe],
  cinematic_started: [2681, 0xaeca3822],
});

export class IncidentTable {
  constructor(rows = [], source = "built-in") {
    this.source = source;
    this.byId = new Map();
    this.byHash = new Map();
    this.byName = new Map();
    for (const [name, [id, hash]] of Object.entries(KNOWN_56578)) {
      this.add(id, hash, name);
    }
    for (const row of rows) {
      this.add(row.id, row.hash, row.name);
    }
  }

  add(id, hash, name) {
    const entry = { hash: hash >>> 0, id, name: name || undefined };
    this.byId.set(id, entry);
    if (!this.byHash.has(entry.hash) || entry.name) {
      this.byHash.set(entry.hash, entry);
    }
    if (entry.name) {
      this.byName.set(entry.name, entry);
    }
  }

  // TSV text with a header naming at least `index` and `name_hash`.
  static parse(text, source = "tsv") {
    const lines = text.split(/\r?\n/).filter((line) => line.length > 0);
    const header = (lines.shift() ?? "").split("\t");
    const col = (name) => header.indexOf(name);
    const iIndex = col("index");
    const iHash = col("name_hash");
    const iName = col("name");
    if (iIndex < 0 || iHash < 0) {
      throw new Error(`${source}: not an incident catalogue (no index/name_hash columns)`);
    }
    const rows = lines.map((line) => {
      const cells = line.split("\t");
      return {
        hash: Number.parseInt(cells[iHash], 16) >>> 0,
        id: Number(cells[iIndex]),
        name: iName >= 0 ? cells[iName] : undefined,
      };
    });
    return new IncidentTable(rows, source);
  }

  // The catalogue at `path`, else the repository's, else the built-in four.
  static load(path) {
    const at = path ?? (existsSync(DEFAULT_CATALOGUE) ? DEFAULT_CATALOGUE : undefined);
    if (!at) {
      return new IncidentTable();
    }
    return IncidentTable.parse(readFileSync(at, "utf8"), at);
  }

  // {id?, name_hash?, name?} -> {id, hash, name} or undefined.
  resolve({ id, name_hash: nameHash, name }, fnv1) {
    if (nameHash !== undefined) {
      const hash = (typeof nameHash === "string" ? Number.parseInt(nameHash, 16) : nameHash) >>> 0;
      return this.byHash.get(hash) ?? { hash, id: undefined, name: undefined };
    }
    if (id !== undefined) {
      return this.byId.get(Number(id));
    }
    if (name !== undefined) {
      return this.byName.get(name) ?? (fnv1 ? { hash: fnv1(name), id: undefined, name } : undefined);
    }
    return;
  }

  idOf(name) {
    return this.byName.get(name)?.id;
  }
}
