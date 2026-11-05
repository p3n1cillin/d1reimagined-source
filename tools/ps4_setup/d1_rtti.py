#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Export build-pinned type facts, never executable bytes or private paths.

Build `cargo build -p d1-eboot` in ps4/d1-server first (BelowNormal on Windows).
Then set D1_EBOOT and run this file with --output ps4/fixtures/56578/rtti.
All ELF, RTTI, relocation and RSAT parsing stays in d1-eboot. A service definition
table is optional because investment message ids live in content, not the eboot.
"""
import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def binary():
    explicit = os.environ.get("D1_EBOOT_BIN")
    if explicit:
        return Path(explicit)
    suffix = ".exe" if os.name == "nt" else ""
    for profile in ("debug", "release"):
        candidate = ROOT / "ps4/d1-server/target" / profile / ("d1-eboot" + suffix)
        if candidate.is_file():
            return candidate
    raise SystemExit("Build d1-eboot first, or set D1_EBOOT_BIN")


def validate(data):
    """Refuse vacuous/corrupt exports; missing schema slots remain explicitly unknown."""
    types = data["rtti"]["types"]
    schemas = data["schemas"]
    if not types or not schemas or not data["envelopes"]:
        raise ValueError("empty RTTI, schema or envelope export")
    tis = {t["ti"] for t in types}
    if len(tis) != len(types) or any(not t["name"] for t in types):
        raise ValueError("duplicate typeinfo or unnamed class")
    if any(base not in tis for t in types for base in t["bases"]):
        raise ValueError("unresolved RTTI base")
    tags = {s["tag"] for s in schemas}
    if len(tags) != len(schemas):
        raise ValueError("duplicate schema tag")
    for schema in schemas:
        if len(schema["fields"]) != schema["field_count"]:
            raise ValueError("truncated schema export")
    for vt in data["rtti"]["vtables"]:
        if vt["ti"] not in tis:
            raise ValueError("vtable without typeinfo")
    for row in data.get("investment_definitions") or []:
        if row["unexpected_nonzero_offsets"]:
            raise ValueError("service definition layout contains unrecognised words")


def table(path, columns, rows):
    with path.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.writer(stream, delimiter="\t", lineterminator="\n")
        writer.writerow(columns)
        writer.writerows(rows)


def export(data, output):
    validate(data)
    output.mkdir(parents=True, exist_ok=True)
    types = sorted(data["rtti"]["types"], key=lambda t: t["ti"])
    schemas = sorted(data["schemas"], key=lambda s: s["tag"])
    table(output / "classes.tsv", ("typeinfo", "kind", "name", "bases", "vtables"), (
        (hex(t["ti"]), t["kind"], t["name"],
         ",".join(map(hex, t["bases"])), ",".join(map(hex, t["vtables"]))) for t in types))
    table(output / "vtables.tsv", ("vptr", "typeinfo", "offset_to_top", "slot", "target"), (
        (hex(v["vptr"]), hex(v["ti"]), v["offset_to_top"], i, hex(target))
        for v in sorted(data["rtti"]["vtables"], key=lambda v: v["vptr"])
        for i, target in enumerate(v["slots"])))
    table(output / "schemas.tsv", ("tag", "anchor", "struct_size", "field_count", "complete",
                                    "array_capacity", "array_strides"), (
        (hex(s["tag"]), hex(s["anchor"]), s["struct_size"], s["field_count"],
         int(all(f["present"] for f in s["fields"])), s["array_capacity"],
         ",".join(map(str, s["array_strides"])) if s["array_capacity"] > 0 else "") for s in schemas))
    columns = ("tag", "index", "marker", "present", "offset", "kind", "child_tag",
               "raw_bits_or_count_offset", "serialized_start", "raw_parameter0", "raw_parameter2")
    table(output / "fields.tsv", columns, (
        [hex(s["tag"])] + [
            "" if f[k] is None else int(f[k]) if isinstance(f[k], bool) else f[k]
            for k in columns[1:]] for s in schemas for f in s["fields"]))
    table(output / "envelopes.tsv", ("id", "class", "vtable", "header_class", "encode", "decode"), (
        (e["type"], e["class"], hex(e["vtable"]), e["header_class"],
         hex(e["encode_payload"]) if e["encode_payload"] else "",
         hex(e["decode_payload"]) if e["decode_payload"] else "")
        for e in sorted(data["envelopes"].values(), key=lambda e: e["type"])))
    table(output / "instances.tsv", ("tag", "checksum", "record", "struct_size", "span_bits"), (
        (hex(s["tag"]), hex(s["checksum"]), hex(s["record"]), s["decoded_size"], s["span_bits"])
        for s in sorted(data["instances"], key=lambda s: s["tag"])))
    table(output / "services.tsv", ("id", "name_hash", "request_tag", "response_tag"), (
        (d["message_id"], hex(d["name_hash"]), hex(d["request_tag"]), hex(d["response_tag"]))
        for d in sorted(data.get("investment_definitions") or [], key=lambda d: d["message_id"])))
    files = sorted(output.glob("*.tsv"))
    manifest = {
        "format_version": 1, "eboot_sha256": data["eboot_sha256"],
        "evidence": "static executable metadata; no native game run",
        "counts": {"classes": len(types), "vtables": len(data["rtti"]["vtables"]),
                   "vtable_slots": sum(len(v["slots"]) for v in data["rtti"]["vtables"]),
                   "schemas": len(schemas), "fields": sum(s["field_count"] for s in schemas),
                   "incomplete_schemas": sum(not all(f["present"] for f in s["fields"]) for s in schemas),
                   "envelopes": len(data["envelopes"]), "instances": len(data["instances"]),
                   "services": len(data.get("investment_definitions") or [])},
        "tables_sha256": {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in files},
    }
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--eboot", default=os.environ.get("D1_EBOOT"))
    parser.add_argument("--definitions", help="optional private investment definition table")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not args.eboot:
        parser.error("set D1_EBOOT or pass --eboot")
    command = [str(binary()), "--eboot", args.eboot, "wire-catalog"]
    if args.definitions:
        command += ["--definitions", args.definitions]
    result = subprocess.run(command, check=True, capture_output=True, encoding="utf-8")
    manifest = export(json.loads(result.stdout), args.output)
    print(json.dumps(manifest["counts"], sort_keys=True))


if __name__ == "__main__":
    main()
