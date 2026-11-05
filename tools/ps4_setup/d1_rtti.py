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
import importlib.util
import json
import os
from pathlib import Path
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def protobuf(eboot, data):
    """Find descriptor arguments in known RTTI methods; reuse the protobuf reader.

    Only accept a RIP-relative LEA into RSI (the descriptor argument of the native
    protobuf wrapper). No reference-build table addresses or global offset are used.
    Failure to find this narrow code shape does not establish absence of a body.
    """
    spec = importlib.util.spec_from_file_location("d1_proto_descriptor",
                                                Path(__file__).with_name("dump-codec-descriptor.py"))
    reader = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(reader)
    image = reader.Image(eboot)
    cache = {}
    roots, fields = [], {}

    def collect(descriptor, rows):
        if not rows or len(rows) >= 256:
            raise ValueError("empty or unterminated protobuf table")
        numbers = [r["field"] for r in rows]
        if numbers != sorted(set(numbers)) or numbers[-1] > 2048:
            raise ValueError("implausible protobuf field numbering")
        for row in rows:
            if row["card"] not in reader.CARD.values() or row["type"] not in reader.TYPE.values():
                raise ValueError("unknown protobuf codec")
            if row["type"] == "invalid" or row["size"] > 0x100000 or row["capacity"] > 0x100000:
                raise ValueError("implausible protobuf field")
            fields[(descriptor, row["field"])] = (descriptor, row["field"], row["card"], row["type"],
                row["size"], row["capacity"], row["pre_pad"], row["flag_delta"], row["aux"] or "")
            if "fields" in row:
                collect(int(row["descriptor"], 16), row["fields"])

    for envelope in sorted(data["envelopes"].values(), key=lambda e: e["type"]):
        for direction, key in (("encode", "encode_payload"), ("decode", "decode_payload")):
            method = envelope[key]
            if not method:
                continue
            if method not in cache:
                result = subprocess.run([str(binary()), "--eboot", str(eboot), "disasm", hex(method),
                                         "--force", "--count", "40", "--json"],
                                        capture_output=True, check=True, encoding="utf-8")
                candidates = []
                for insn in json.loads(result.stdout)["insns"]:
                    if insn["mnemonic"] == "ret":
                        break
                    if (insn["mnemonic"] == "lea" and insn["op_str"].startswith("rsi,")
                            and insn["rip_target"] is not None):
                        address = insn["rip_target"]
                        before = dict(fields)
                        try:
                            collect(address, reader.dump(image, address))
                            candidates.append(address)
                        except (ValueError, IndexError, UnicodeError, struct.error):
                            fields = before
                cache[method] = sorted(set(candidates))
            for address in cache[method]:
                roots.append((envelope["type"], direction, hex(method), hex(address)))
    # A known independently documented schema controls both method discovery and table parsing.
    control = [int(r[3], 16) for r in roots if r[0] == 42 and r[1] == "encode"]
    if len(control) != 1 or len([k for k in fields if k[0] == control[0]]) != 11:
        raise ValueError("matchmaking-core positive control failed; do not cite protobuf export")
    return {"roots": roots, "fields": [fields[k] for k in sorted(fields)]}


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
        if row["request_tag"] not in tags or row["response_tag"] not in tags:
            raise ValueError("service definition references a schema absent from this executable")


def table(path, columns, rows):
    with path.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.writer(stream, delimiter="\t", lineterminator="\n")
        writer.writerow(columns)
        writer.writerows(rows)


def field_layout(schema):
    """Display parsed facts only; this is not an encoder or a second schema reader."""
    primitives = {2: "bool", 3: "i8", 4: "i16", 5: "i32", 6: "u64",
                  7: "u8", 8: "u16", 9: "u32", 10: "u64", 11: "f32"}
    result = []
    for field in schema["fields"]:
        if not field["present"]:
            result.append(f"f{field['index']}:UNKNOWN")
            continue
        kind = field["kind"] & 255
        optional = "?" if field["kind"] >> 8 else ""
        if field["child_tag"] != 0xffffffff:
            value = hex(field["child_tag"])
        else:
            value = primitives.get(kind, f"kind{kind:#x}")
            if kind in (3, 4, 5, 6, 7, 8, 9, 10):
                value += f"(bits={field['raw_bits_or_count_offset']},p0={field['raw_parameter0']})"
        result.append(f"f{field['index']}@{field['offset']:#x}:{value}{optional}")
    return "; ".join(result) or "empty"


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
    by_tag = {s["tag"]: s for s in schemas}
    table(output / "services.tsv", ("id", "name_hash", "request_tag", "response_tag",
          "request_size", "request_fields", "response_size", "response_fields"), (
        (d["message_id"], hex(d["name_hash"]), hex(d["request_tag"]), hex(d["response_tag"]),
         by_tag[d["request_tag"]]["struct_size"], field_layout(by_tag[d["request_tag"]]),
         by_tag[d["response_tag"]]["struct_size"], field_layout(by_tag[d["response_tag"]]))
        for d in sorted(data.get("investment_definitions") or [], key=lambda d: d["message_id"])))
    if "protobuf" in data:
        table(output / "protobuf_roots.tsv", ("id", "direction", "method", "descriptor"),
              data["protobuf"]["roots"])
        table(output / "protobuf_fields.tsv", ("descriptor", "field", "cardinality", "codec",
              "size", "capacity", "pre_pad", "flag_delta", "aux"), (
              (hex(r[0]), *r[1:]) for r in data["protobuf"]["fields"]))
    else:
        for name in ("protobuf_roots.tsv", "protobuf_fields.tsv"):
            (output / name).unlink(missing_ok=True)
    names = ["classes.tsv", "vtables.tsv", "schemas.tsv", "fields.tsv", "envelopes.tsv",
             "instances.tsv", "services.tsv"]
    if "protobuf" in data:
        names += ["protobuf_roots.tsv", "protobuf_fields.tsv"]
    files = [output / name for name in sorted(names)]
    manifest = {
        "format_version": 1, "eboot_sha256": data["eboot_sha256"],
        "evidence": "static executable metadata; no native game run",
        "definitions_sha256": data.get("definitions_sha256"),
        "counts": {"classes": len(types), "vtables": len(data["rtti"]["vtables"]),
                   "vtable_slots": sum(len(v["slots"]) for v in data["rtti"]["vtables"]),
                   "schemas": len(schemas), "fields": sum(s["field_count"] for s in schemas),
                   "incomplete_schemas": sum(not all(f["present"] for f in s["fields"]) for s in schemas),
                   "envelopes": len(data["envelopes"]), "instances": len(data["instances"]),
                   "services": len(data.get("investment_definitions") or [])},
        "tables_sha256": {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in files},
    }
    if "protobuf" in data:
        manifest["counts"].update(protobuf_roots=len(data["protobuf"]["roots"]),
                                  protobuf_fields=len(data["protobuf"]["fields"]))
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--eboot", default=os.environ.get("D1_EBOOT"))
    parser.add_argument("--definitions", help="optional private investment definition table")
    parser.add_argument("--protobuf", action="store_true", help="also discover native protobuf tables")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not args.eboot:
        parser.error("set D1_EBOOT or pass --eboot")
    command = [str(binary()), "--eboot", args.eboot, "wire-catalog"]
    if args.definitions:
        command += ["--definitions", args.definitions]
    result = subprocess.run(command, check=True, capture_output=True, encoding="utf-8")
    data = json.loads(result.stdout)
    if args.definitions:
        data["definitions_sha256"] = hashlib.sha256(Path(args.definitions).read_bytes()).hexdigest()
    if args.protobuf:
        data["protobuf"] = protobuf(args.eboot, data)
    manifest = export(data, args.output)
    print(json.dumps(manifest["counts"], sort_keys=True))


if __name__ == "__main__":
    main()
