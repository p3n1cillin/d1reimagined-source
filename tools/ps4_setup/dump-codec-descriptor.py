#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Dump a Destiny PS4 codec descriptor table (the 23-byte field records documented in the PS4
tree's design/signon-and-entitlement.md section 1.4), recursing into nested messages.

Pointers inside records (aux at +0x0f) are not 8-aligned, so they are resolved from the ELF's
R_X86_64_RELATIVE entries in DT_SCE_RELA rather than from the file bytes (which are 0).

Usage: dump-codec-descriptor.py <eboot.elf> <descriptor-vaddr> [--json]
"""
import json
import struct
import sys

CARD = {0x00: "required", 0x10: "optional", 0x20: "repeated", 0x30: "callback"}
TYPE = {0: "varint", 1: "zigzag", 2: "fixed32", 3: "fixed64", 4: "bytes", 5: "string",
        6: "message", 7: "invalid"}


class Image:
    def __init__(self, path):
        with open(path, "rb") as stream:
            self.b = stream.read()
        b = self.b
        phoff, = struct.unpack_from("<Q", b, 0x20)
        phnum, = struct.unpack_from("<H", b, 0x38)
        self.loads, dyn, dyndata = [], None, None
        for i in range(phnum):
            t, fl, off, va, pa, fs, ms, al = struct.unpack_from("<IIQQQQQQ", b, phoff + i * 56)
            if t == 1:
                self.loads.append((va, off, fs))
            elif t == 2:
                dyn = (off, fs)
            elif t == 0x61000000:
                dyndata = off
        rela_off = rela_sz = None
        for k in range(0, dyn[1], 16):
            tag, val = struct.unpack_from("<qQ", b, dyn[0] + k)
            if tag == 0x6100002F:      # DT_SCE_RELA
                rela_off = val
            elif tag == 0x61000031:    # DT_SCE_RELASZ
                rela_sz = val
        self.rel = {}
        base = dyndata + rela_off
        for k in range(0, rela_sz, 24):
            r_off, r_info, r_add = struct.unpack_from("<QQq", b, base + k)
            if r_info & 0xFFFFFFFF == 8:
                self.rel[r_off] = r_add

    def file(self, va):
        for lva, off, fs in self.loads:
            if lva <= va < lva + fs:
                return off + va - lva
        raise ValueError(f"vaddr {va:#x} is not file-backed")

    def ptr(self, va):
        if va in self.rel:
            return self.rel[va]
        return struct.unpack_from("<Q", self.b, self.file(va))[0]


def dump(img, va, depth=0, seen=None):
    seen = seen if seen is not None else set()
    rows = []
    o = img.file(va)
    for k in range(256):
        rec = va + k * 23
        fn, ty, pre, fd, size, cap = struct.unpack_from("<IBBbII", img.b, o + k * 23)
        if fn == 0:
            break
        aux = img.ptr(rec + 0x0F)
        row = {"field": fn, "card": CARD.get(ty & 0xF0, hex(ty & 0xF0)),
               "type": TYPE.get(ty & 0x0F, hex(ty & 0x0F)), "size": size, "capacity": cap,
               "pre_pad": pre, "flag_delta": fd, "aux": hex(aux) if aux else None}
        if ty & 0x0F == 6 and aux and aux not in seen and depth < 4:
            seen.add(aux)
            row["descriptor"] = hex(aux)
            row["fields"] = dump(img, aux, depth + 1, seen)
        rows.append(row)
    return rows


def show(rows, indent=""):
    for r in rows:
        extra = f" cap={r['capacity']}" if r["card"] == "repeated" else ""
        sub = f" -> {r['descriptor']}" if "descriptor" in r else ""
        print(f"{indent}f{r['field']:<3} {r['card']:<8} {r['type']:<8} size={r['size']}{extra}{sub}")
        if "fields" in r:
            show(r["fields"], indent + "    ")


if __name__ == "__main__":
    img = Image(sys.argv[1])
    rows = dump(img, int(sys.argv[2], 0))
    if "--json" in sys.argv:
        print(json.dumps(rows, indent=1))
    else:
        show(rows)
