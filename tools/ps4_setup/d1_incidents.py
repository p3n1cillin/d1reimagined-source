# SPDX-License-Identifier: GPL-2.0-or-later
"""Export the owned 56578 client's incident identities, offline.

Requires Node and a locally built Blam Network tiger checkout (gibbon/poodle,
MIT). No emulator, native decoder, extracted files or proprietary DLL is used.
Only the requested fact tables are written; raw tags stay in subprocess pipes.
See notes/ps4-56578-incident-catalog-2026-11-05.md for layout and provenance.
"""

import argparse
import csv
import io
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from d1_incident_catalog import parse_catalog  # noqa: E402
from d1_pkg import fnv1  # noqa: E402

FIXTURES = ROOT / "ps4/fixtures/56578"
CATALOG = FIXTURES / "incidents-56578.tsv"
MAPPING = FIXTURES / "incidents-51539-to-56578.tsv"
LEGACY = ROOT / "lab/history/ps3-era/research/JOIN218-incident-catalog-report.json"
CATALOG_FIELDS = ("index", "name_hash", "name", "payload_selector", "aux")
MAPPING_FIELDS = ("index_51539", "index_56578", "name_hash", "name")

# Use gibbon's existing package, patch-chain and poodle readers. Discover the
# registry through the named client root and class, never a tag/address from a run.
READ_CLIENT = r"""
const path = require('node:path');
const [tiger, directory] = process.argv.slice(1);
const g = require(path.join(tiger,
  'packages/gibbon/dist-cjs/versions/destiny1/v56578_17_04_12_1251_evil.js'));
const set = g.c_package_set.open_directory(directory);
try {
  const roots = set.named_tags().filter(t => t.name === 'client_startup_globals');
  if (roots.length !== 1) throw Error('expected one named client startup root');
  const root = roots[0];
  const bytes = set.read_tag(root.hash);
  const candidates = new Map();
  for (let offset = 0; offset + 4 <= bytes.length; offset += 4) {
    const tag = bytes.readUInt32LE(offset);
    if (set.entry(tag)?.class_hash === 0x80800659) candidates.set(tag, offset);
  }
  if (candidates.size !== 1) throw Error('expected one client incident registry reference');
  const [[tag, offset]] = candidates;
  const data = set.read_tag(tag);
  console.error(JSON.stringify({root: root.hash, tag, reference_offset: offset}));
  process.stdout.write(data);
} finally {
  set.close();
}
"""


def read_client(packages, tiger, node="node"):
    result = subprocess.run([str(node), "-e", READ_CLIENT, str(Path(tiger).resolve()),
                             str(Path(packages).resolve())], capture_output=True, timeout=120)
    if result.returncode:
        raise ValueError("gibbon could not read the client registry: "
                         + result.stderr.decode("utf-8", "replace").strip())
    return parse_catalog(result.stdout, ps4=True), json.loads(result.stderr)


def identities(records):
    """Require a complete, unambiguous index -> hash table before joining builds."""
    result = {}
    if not records:
        raise ValueError("empty incident catalogue")
    for index, row in enumerate(records):
        value = int(row["name_hash"], 16)
        if row["index"] != index or not 0 <= value <= 0xFFFFFFFF:
            raise ValueError("incident indices must be contiguous and hashes must fit u32")
        if value in result:
            raise ValueError(f"ambiguous incident hash 0x{value:08x}")
        result[value] = row
    return result


def add_name(names, value, name):
    if not name or any(c in name for c in "\t\r\n") or fnv1(name.encode("utf-8")) != value:
        raise ValueError(f"name does not hash to 0x{value:08x}: {name!r}")
    previous = names.get(value)
    if previous is not None and previous != name:
        raise ValueError(f"name collision at 0x{value:08x}: {previous!r}, {name!r}")
    names[value] = name


def read_names(path):
    names = {}
    with Path(path).open(encoding="utf-8", newline="") as source:
        for row in csv.DictReader(source, delimiter="\t"):
            if row["name"]:
                add_name(names, int(row["name_hash"], 16), row["name"])
    return names


def dictionary_names(directory, wanted):
    """Read locally, retaining only verified hash -> name facts in our catalogues."""
    names = {}
    files = sorted(Path(directory).rglob("*.csv"))
    if not files:
        raise ValueError("dictionary contains no CSV files")
    for path in files:
        with path.open(encoding="utf-8-sig", newline="") as source:
            reader = csv.DictReader(source)
            if not {"hash", "string"}.issubset(reader.fieldnames or []):
                continue
            for row in reader:
                value = int(row["hash"], 16)
                if value in wanted and row["string"]:
                    add_name(names, value, row["string"])
    return names


def documented_names(path, wanted):
    names = {}
    for name in re.findall(r"`([a-z][a-z0-9_]*)`", Path(path).read_text(encoding="utf-8")):
        value = fnv1(name.encode("utf-8"))
        if value in wanted:
            add_name(names, value, name)
    return names


def fact_tables(records, legacy, names):
    target, old = identities(records), identities(legacy)
    catalogue = [{"index": row["index"], "name_hash": row["name_hash"],
                  "name": names.get(value, ""), "payload_selector": row["payload_selector"],
                  "aux": ",".join(str(v) for v in row["auxiliary_u16"])}
                 for value, row in target.items()]
    mapping = [{"index_51539": row["index"],
                "index_56578": target[value]["index"] if value in target else "",
                "name_hash": row["name_hash"], "name": names.get(value, "")}
               for value, row in old.items()]
    return catalogue, mapping


def tsv_text(rows, fields):
    output = io.StringIO(newline="")
    writer = csv.DictWriter(output, fieldnames=fields, delimiter="\t", lineterminator="\n")
    writer.writeheader()
    writer.writerows(rows)
    return output.getvalue()


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--packages", type=Path, required=True, help="owned 56578 packages directory")
    parser.add_argument("--tiger", type=Path, default=os.environ.get("D1_TIGER"),
                        help="built local tiger checkout (or D1_TIGER)")
    parser.add_argument("--node", default="node", help="Node executable")
    parser.add_argument("--legacy", type=Path, default=LEGACY, help="01.24 catalogue report")
    parser.add_argument("--names", type=Path, default=CATALOG,
                        help="existing fact TSV for reproducible names; hashes are checked")
    parser.add_argument("--dictionary", type=Path, help="optional local dictionary CSV directory")
    parser.add_argument("--incident-notes", type=Path, help="optional local bullshcript INCIDENTS.md")
    parser.add_argument("--out", type=Path, default=CATALOG)
    parser.add_argument("--mapping-out", type=Path, default=MAPPING)
    args = parser.parse_args(argv)
    if not args.tiger:
        parser.error("--tiger or D1_TIGER is required")
    try:
        if args.out.resolve() == args.mapping_out.resolve():
            raise ValueError("catalogue and mapping outputs must be distinct")
        result, provenance = read_client(args.packages, args.tiger, args.node)
        records = result["records"]
        previous = json.loads(args.legacy.read_text(encoding="utf-8"))
        legacy = previous["records"]
        if previous["record_count"] != len(legacy):
            raise ValueError("legacy report count differs from its records")
        wanted = identities(records).keys() | identities(legacy).keys()
        names = read_names(args.names) if args.names.exists() else {}
        for extra in (dictionary_names(args.dictionary, wanted) if args.dictionary else {},
                      documented_names(args.incident_notes, wanted) if args.incident_notes else {}):
            for value, name in extra.items():
                add_name(names, value, name)
        catalogue, mapping = fact_tables(records, legacy, names)
        outputs = ((args.out, tsv_text(catalogue, CATALOG_FIELDS)),
                   (args.mapping_out, tsv_text(mapping, MAPPING_FIELDS)))
        # Build and validate both fact tables before writing either.
        for path, contents in outputs:
            path.write_text(contents, encoding="utf-8", newline="")
    except (OSError, ValueError, KeyError, subprocess.TimeoutExpired) as exc:
        parser.error(str(exc))
    print(f"client registry 0x{provenance['tag']:08x}, root reference +0x{provenance['reference_offset']:x}")
    print(f"{len(catalogue)} incidents, {sum(bool(r['name']) for r in catalogue)} named; "
          f"{sum(r['index_56578'] != '' for r in mapping)}/{len(mapping)} legacy hashes mapped")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
