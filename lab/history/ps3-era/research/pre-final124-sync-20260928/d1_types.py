"""Destiny (BLES01857 01.00) runtime type definitions read statically from the decrypted EBOOT,
plus the web-service table from the investment globals package - the same JSON schema that
`scripts/rpcs3_tiger_types.py schema` exports from a running game, without running it.

Every serializable type definition is compiled into the EBOOT. A definition holds its struct size
at +0x14 and, at def+0x54+[def+0x54], its field list: u32 count, u32 own type handle
(0x8080xxxx), u32 size, then (after a 0x14-byte header) 0x28-byte field entries: +0 struct
offset, +8 presence-bit index, +0x10 kind, +0x11 optional, +0x14 nested type handle, +0x18 array
mode byte / biased minimum, +0x1c bit width, +0x20 extra. The field list names its own handle, so
a scan of the file finds every definition (4,387 in 01.00; all 816 types of the schemas exported
at run time match field for field). Types without fields (raw blobs such as scenario_client) are
not in the list.

The web services are a tag of the investment globals (class 0x80803389 package, e.g.
0x80a5806b): a dynamic array of 36-byte records of element class 0x80801820 - u32 name hash,
u32, u16 wire id, u16, u32 request type, u32 response type.

  python scripts/d1_types.py schema OUT.json [--packages DIR --oodle DLL]  # full schema (+ services)
  python scripts/d1_types.py type 0x80801855 [--depth 3]                    # one type tree
  python scripts/d1_types.py services --packages DIR --oodle DLL            # wire, request, response
"""

import argparse
import json
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import d1_pkg  # noqa: E402

DEFAULT_ELF = Path(__file__).resolve().parents[1] / "local" / "RESEARCH" / "ps3_decrypt" / "EBOOT.elf"
TYPE_LO, TYPE_HI = 0x80800000, 0x80810000
SERVICE_RECORD_CLASS = 0x80801820
KIND_NAMES = {1: "struct", 2: "bool", 3: "u8", 4: "i16", 5: "i32", 6: "u64", 7: "u8", 8: "u16", 9: "u32",
              10: "u64", 11: "f32", 13: "k13", 0x23: "machine_id"}


def scan_types(data):
    """{"0x8080....": {"size", "fields"}} for every type definition in the EBOOT bytes."""
    types, end = {}, len(data)
    for p in range(0, end - 0x58, 4):
        rel = struct.unpack_from(">I", data, p + 0x54)[0]
        if rel == 0 or rel > 0x100000 or rel % 4:
            continue
        q = p + 0x54 + rel
        if q + 0x14 > end:
            continue
        count, handle, size = struct.unpack_from(">III", data, q)
        if not TYPE_LO <= handle < TYPE_HI or count > 4096 or q + 0x14 + count * 0x28 > end:
            continue
        if struct.unpack_from(">I", data, p + 0x14)[0] != size:
            continue
        fields = []
        for i in range(count):
            e = q + 0x14 + i * 0x28
            off, _, bit = struct.unpack_from(">III", data, e)
            nested, minimum, width, extra = struct.unpack_from(">IIII", data, e + 0x14)
            fields.append({"off": off, "bit": bit, "kind": data[e + 0x10], "opt": data[e + 0x11],
                           "nested": nested, "arr": data[e + 0x18], "min": minimum, "width": width,
                           "extra": extra})
        types[f"{handle:#x}"] = {"size": size, "fields": fields}
    return types


def service_table(ps):
    """[(wire, name hash, request handle, response handle)] from the investment globals."""
    for p in ps.by_id.values():
        for e in p.entries:
            if e["type"] != 16:
                continue
            try:
                data = p.entry_data(e["index"], ps.oodle)
            except (ValueError, struct.error, RuntimeError):
                continue
            if struct.pack(">I", SERVICE_RECORD_CLASS) not in data[:0x40]:
                continue
            for field in range(4, 0x28, 4):
                try:
                    count, first, elem = d1_pkg.dyn_array(data, field)
                except (ValueError, struct.error):
                    continue
                if elem == SERVICE_RECORD_CLASS:
                    break
            else:
                continue
            out = []
            for i in range(count):
                name, _, wire, _, request, response = struct.unpack_from(">IIHHII", data, first + 36 * i)
                out.append((wire, name, request, response))
            return sorted(out)
    return []


def signature(types, handle, depth=0, max_depth=2):
    """Compact field summary of a type, e.g. "u64 i16 {u8 i16}x6"."""
    entry = types.get(f"{handle:#x}")
    if entry is None:
        return "?"
    parts = []
    for f in entry["fields"]:
        if f["kind"] == 1:
            inner = signature(types, f["nested"], depth + 1, max_depth) if depth < max_depth else f"{f['nested']:#x}"
            part = f"[{inner}]" if f["arr"] == 1 else f"{{{inner}}}"
        else:
            name = KIND_NAMES.get(f["kind"], f"k{f['kind']}")
            standard = f["width"] in (8, 16, 32, 64) or f["kind"] in (2, 6, 10, 11, 0x23)
            part = name if standard else f"{name}:{f['width']}"
        if f["opt"]:
            part += "?"
        if parts and parts[-1][0] == part:
            parts[-1][1] += 1
        else:
            parts.append([part, 1])
    return " ".join(p if n == 1 else f"{p}x{n}" for p, n in parts)


def print_tree(types, handle, depth=0, max_depth=3):
    entry = types.get(f"{handle:#x}")
    if entry is None:
        print("  " * depth + f"{handle:#x} (not in the EBOOT)")
        return
    print("  " * depth + f"{handle:#x} size {entry['size']:#x}, {len(entry['fields'])} fields")
    previous = None
    for f in entry["fields"]:
        name = KIND_NAMES.get(f["kind"], f"k{f['kind']}")
        line = (f"+{f['off']:#x} {name} w{f['width']} {'opt ' if f['opt'] else ''}"
                f"{'array ' if f['kind'] == 1 and f['arr'] == 1 else ''}"
                f"{'min ' + hex(f['min']) + ' ' if f['kind'] != 1 and f['min'] else ''}")
        if f["kind"] == 1:
            if f["nested"] == previous:
                print("  " * depth + "  " + line + f"(same {f['nested']:#x})")
                continue
            previous = f["nested"]
            print("  " * depth + "  " + line + "->")
            if depth < max_depth:
                print_tree(types, f["nested"], depth + 2, max_depth)
        else:
            previous = None
            print("  " * depth + "  " + line)


def package_set(a):
    if not a.packages:
        return None
    return d1_pkg.PackageSet([a.packages], d1_pkg.Oodle(a.oodle) if a.oodle else None)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--elf", type=Path, default=DEFAULT_ELF, help="decrypted EBOOT")
    ap.add_argument("--packages", help="folder with ps3_investment_globals_012c (for the services)")
    ap.add_argument("--oodle", help="oo2core_3_win64.dll (the packages' blocks are compressed)")
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("schema"); s.add_argument("out", type=Path)
    s = sub.add_parser("type"); s.add_argument("handle"); s.add_argument("--depth", type=int, default=3)
    sub.add_parser("services")
    a = ap.parse_args(argv)
    types = scan_types(a.elf.read_bytes())
    if a.cmd == "type":
        print_tree(types, int(a.handle, 16), max_depth=a.depth)
        return
    ps = package_set(a)
    services = service_table(ps) if ps else []
    if a.cmd == "schema":
        out = dict(types)
        if services:
            out["services"] = {str(w): [f"{rq:#x}", f"{rs:#x}"] for w, _, rq, rs in services}
        a.out.write_text(json.dumps(out, indent=0), encoding="utf-8")
        print(f"{len(types)} types, {len(services)} services -> {a.out}")
        return
    if not services:
        sys.exit("services need --packages (and --oodle) with the investment globals")
    for wire, name, request, response in services:
        print(f"{wire:5}  {name:#010x}  request {request:#x}: {signature(types, request)}")
        print(f"{'':19}response {response:#x}: {signature(types, response)}")


if __name__ == "__main__":
    main()
