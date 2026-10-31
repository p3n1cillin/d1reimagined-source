"""Offline reader for Destiny 1 Tiger packages (.pkg, format version 24) as shipped on the PS3
disc (big-endian, platform 4). Reads package headers, file entries, block tables and the named
tag table without the game running; extracts entries when their blocks are stored uncompressed
or when a user-supplied Oodle runtime is given (--oodle, `oo2core_3_win64.dll`; never commit or
download one). A compressed block decodes to exactly its used size (the furthest entry end in it,
rounded up to 0x4000), which is what the decoder is asked for.

Layout (tiger-pkg `d1_legacy`, cross-checked against a PS4 v24 package study in
github.com/lizardpeter/destiny-1 spec/D1_TIGER_PACKAGE_v24.md; PS3 byte order and the flag bit
from tiger-pkg; the package id at +4 and the u64 at +8 match what the client itself checks,
notes/ps3-route.md 021-022):

  header  +0x00 u16 version (24)  +0x02 u16 platform (4 = PS3)  +0x04 u16 package id
          +0x08 u64 registration value  +0x10 u64 build time  +0x20 u16 patch id
          +0x22 u16 language  +0x24 char[128] tool string  +0xb0 u32 signature offset
          +0xb4/+0xb8/+0xbc entry table count, offset, SHA-1
          +0xd0/+0xd4/+0xd8 block table count, offset, SHA-1
          +0xec/+0xf0/+0xf4 named tag table count, offset, SHA-1   +0x13c u32 file size
  entry   16 bytes: u32 reference, u32 type word, u64 block info
          (start block = bits 0-13, start offset = bits 14-27 * 16, size = bits 28-57)
  block   32 bytes: u32 offset, u32 stored size, u16 patch id, u16 flags, SHA-1 of stored bytes;
          blocks are 0x40000 bytes once decompressed; a block with a different patch id lives in
          <name>_<patch>.pkg next to this one
  named   68 bytes: u32 tag hash, u32 class hash, char[60] name

Tag hashes: 0x80800000 + (package id << 13) + entry index for ids below 0x400; ids up to 0xbff
move to the next banks (bits 23-24 hold 1 + id // 0x400).

Usage:
  python scripts/d1_pkg.py info PKG
  python scripts/d1_pkg.py entries PKG [--type T] [--subtype S]
  python scripts/d1_pkg.py blocks PKG [--verify]
  python scripts/d1_pkg.py named [DIR|PKG ...] [--grep TEXT]   (default DIR: the disc packages)
  python scripts/d1_pkg.py locate TAGHASH [DIR]                (which package holds a tag)
  python scripts/d1_pkg.py extract PKG INDEX|TAGHASH OUT [--oodle DLL]
  python scripts/d1_pkg.py scan [DIR] --find TEXT|--hex HEX [--oodle DLL]  (search entry data)
  python scripts/d1_pkg.py bubbles 80cda002 [DIR] --oodle DLL   (a destination's bubbles and names)
  python scripts/d1_pkg.py groups PKG --oodle DLL [--key K] [--objects]  (replicated entity groups)
  python scripts/d1_pkg.py strings DIR --oodle DLL [--grep TEXT|HASH]   (English localized strings)
"""

import argparse
import ctypes
import hashlib
import struct
import subprocess
import sys
from pathlib import Path

BLOCK_SIZE = 0x40000
PLATFORMS = ["tool32", "win32", "win64", "x360", "ps3", "tool64", "win64v1", "ps4", "xboxone"]
LANGUAGES = ["none", "en", "fr", "it", "de", "es", "ja", "pt", "ru", "pl", "zh-hans", "zh-hant",
             "es-mx", "ko"]
FLAG_COMPRESSED = 0x100  # tiger-pkg d1_legacy (big-endian builds)
RAW_QUANTUM = 0x4000  # compressed blocks decode to exactly align_up(last entry end, 0x4000)
# Oodle LZ block header byte 1 (low 7 bits) -> codec, for the newer header (low nibble 0xC) that
# the open-source ooz (github.com/powzix/ooz) reads. D1 PS4 packages instead start with 0xB7
# (lizardpeter/destiny-1 findings/LOG.md), an older framing no open-source decoder is known to
# handle: those need a user-supplied oo2core_3 runtime.
OODLE_DECODERS = {5: "LZNA", 6: "Kraken", 10: "Mermaid/Selkie", 11: "BitKnit", 12: "Leviathan"}


def default_dir():
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "server"))
    from local_paths import LOCAL
    return LOCAL / "01_WORKING" / "ps3_destiny" / "PS3_GAME" / "USRDIR" / "packages"


def tag_hash(pkg_id, index):
    if pkg_id >= 0xc00:
        raise ValueError(f"package id 0x{pkg_id:x} has no tag hash (banks 1-3 cover 0-0xbff)")
    return 0x80000000 | ((pkg_id // 0x400 + 1) << 23) | ((pkg_id % 0x400) << 13) | (index & 0x1fff)


def split_tag_hash(value):
    """(package id, entry index) of a tag hash, or None for a value outside the tag namespace."""
    if not value & 0x80000000 or not (value >> 23) & 3:
        return None
    return ((value >> 13) & 0x3ff) + (((value >> 23) & 3) - 1) * 0x400, value & 0x1fff


def parse_int(text):
    """0x-prefixed or 8-digit values are hex (tag hashes), short digit strings decimal (indices)."""
    if text.lower().startswith("0x") or len(text) == 8 or not text.isdigit():
        return int(text, 16)
    return int(text)


class Package:
    def __init__(self, path):
        self.path = Path(path)
        self.data = self.path.read_bytes()
        d = self.data
        (self.version, self.platform, self.pkg_id, self.unk6, self.reg_value, self.build_time,
         self.build_id, self.unk1c, self.patch_id, self.language) = struct.unpack_from(">HHHHQQIIHH", d, 0)
        if self.version != 24:
            raise ValueError(f"{self.path.name}: package version {self.version}, expected 24")
        self.tool = d[0x24:0xa4].split(b"\0")[0].decode("latin-1")
        self.signature_offset = struct.unpack_from(">I", d, 0xb0)[0]
        self.tables = {}
        for name, off in (("entries", 0xb4), ("blocks", 0xd0), ("named", 0xec)):
            count, offset = struct.unpack_from(">II", d, off)
            self.tables[name] = (count, offset, d[off + 8:off + 28])
        self.file_size = struct.unpack_from(">I", d, 0x13c)[0]

        count, offset, _ = self.tables["entries"]
        self.entries = []
        for i in range(count):
            ref, word, info = struct.unpack_from(">IIQ", d, offset + 16 * i)
            self.entries.append({
                "index": i, "reference": ref, "word": word,
                "type": (word >> 16) & 0xff, "subtype": word & 0xff,
                "block": info & 0x3fff, "offset": ((info >> 14) & 0x3fff) << 4,
                "size": (info >> 28) & 0x3fffffff})
        count, offset, _ = self.tables["blocks"]
        self.blocks = []
        for i in range(count):
            off, size, patch, flags = struct.unpack_from(">IIHH", d, offset + 32 * i)
            self.blocks.append({"index": i, "offset": off, "size": size, "patch": patch,
                                "flags": flags, "sha1": d[offset + 32 * i + 12:offset + 32 * i + 32]})
        count, offset, _ = self.tables["named"]
        self.named = []
        for i in range(count):
            h, cls = struct.unpack_from(">II", d, offset + 68 * i)
            name = d[offset + 68 * i + 8:offset + 68 * i + 68].split(b"\0")[0].decode("latin-1")
            self.named.append({"hash": h, "class": cls, "name": name})

    def table_hash_ok(self, name):
        count, offset, stored = self.tables[name]
        size = {"entries": 16, "blocks": 32, "named": 68}[name] * count
        if not count:
            return stored == bytes(20) or stored == hashlib.sha1(b"").digest()
        return hashlib.sha1(self.data[offset:offset + size]).digest() == stored

    def stored_block(self, block):
        if block["patch"] == self.patch_id:
            return self.data[block["offset"]:block["offset"] + block["size"]]
        stem = self.path.name.rsplit("_", 1)[0]
        other = self.path.with_name(f"{stem}_{block['patch']}.pkg")
        with open(other, "rb") as f:
            f.seek(block["offset"])
            return f.read(block["size"])

    def raw_size(self, index):
        """Decompressed size of block `index`: the furthest end of any entry data in it, rounded
        up to 0x4000 and capped at 0x40000. OodleLZ_Decompress needs the exact size; the full
        0x40000 fails for a partly used block (lizardpeter/destiny-1, Oodle block sizing note)."""
        if not hasattr(self, "_raw_sizes"):
            ends = {}
            for e in self.entries:
                start = e["block"] * BLOCK_SIZE + e["offset"]
                end = start + e["size"]
                for b in range(e["block"], (end - 1) // BLOCK_SIZE + 1 if e["size"] else e["block"]):
                    ends[b] = max(ends.get(b, 0), min(BLOCK_SIZE, end - b * BLOCK_SIZE))
            self._raw_sizes = {b: min(BLOCK_SIZE, -(-end // RAW_QUANTUM) * RAW_QUANTUM)
                               for b, end in ends.items()}
        return self._raw_sizes.get(index, BLOCK_SIZE)

    def block_data(self, index, oodle=None):
        block = self.blocks[index]
        raw = self.stored_block(block)
        if not block["flags"] & FLAG_COMPRESSED:
            return raw
        if oodle is None:
            raise RuntimeError(f"block {index} is compressed ({oodle_codec(raw)}); pass --oodle")
        cache = self.__dict__.setdefault("_decoded", {})
        if index in cache:
            return cache[index]
        size = self.raw_size(index)
        try:
            data = oodle.decompress(raw, size)
        except RuntimeError:
            if size == BLOCK_SIZE:
                raise
            data = oodle.decompress(raw, BLOCK_SIZE)  # in case the sizing rule differs
        if len(cache) >= 32:  # entries span consecutive blocks; keep the recent ones
            cache.pop(next(iter(cache)))
        cache[index] = data + bytes(BLOCK_SIZE - len(data))
        return cache[index]

    def entry_data(self, index, oodle=None):
        e = self.entries[index]
        out = bytearray()
        block, skip = e["block"], e["offset"]
        while len(out) < e["size"]:
            chunk = self.block_data(block, oodle)[skip:]
            out += chunk[:e["size"] - len(out)]
            block, skip = block + 1, 0
        return bytes(out)


def oodle_codec(raw):
    if raw[:1] == b"\xb7":
        return "legacy Oodle framing 0xB7 as in D1 PS4 packages (needs oo2core_3)"
    if len(raw) < 2 or raw[0] & 0x0f != 0x0c:
        return f"unknown block header {raw[:4].hex()}"
    kind = raw[1] & 0x7f
    return f"Oodle decoder {kind} ({OODLE_DECODERS.get(kind, 'not in ooz')})"


class Oodle:
    """OodleLZ_Decompress from a user-supplied oo2core runtime. On Windows the DLL is loaded
    directly; elsewhere a Windows DLL goes through scripts/oodle_pe_shim.c (built on first use
    into local/liboodle_shim.so with gcc), a contained loader that stubs every import."""

    def __init__(self, path):
        path = Path(path)
        if sys.platform != "win32" and path.suffix.lower() == ".dll":
            shim = Path(__file__).resolve().parents[1] / "local" / "liboodle_shim.so"
            source = Path(__file__).resolve().parent / "oodle_pe_shim.c"
            if not shim.exists() or shim.stat().st_mtime < source.stat().st_mtime:
                shim.parent.mkdir(exist_ok=True)
                subprocess.run(["gcc", "-O2", "-shared", "-fPIC", "-o", str(shim), str(source)], check=True)
            self.lib = ctypes.CDLL(str(shim))
            self.lib.d1_oodle_decompress.restype = ctypes.c_longlong
            self.lib.d1_oodle_decompress.argtypes = [ctypes.c_char_p, ctypes.c_longlong,
                                                     ctypes.c_char_p, ctypes.c_longlong]
            result = self.lib.d1_oodle_load(str(path).encode())
            if result:
                raise RuntimeError(f"oodle shim could not load {path} ({result})")
            self.fn = lambda raw, n, out, size: self.lib.d1_oodle_decompress(raw, n, out, size)
            return
        self.lib = ctypes.CDLL(str(path))
        f = self.lib.OodleLZ_Decompress
        f.restype = ctypes.c_int64
        f.argtypes = [ctypes.c_char_p, ctypes.c_int64, ctypes.c_char_p, ctypes.c_int64,
                      ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_void_p, ctypes.c_void_p,
                      ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int]
        # fuzz-safe, no CRC check, no verbosity, all thread phases (as tiger-pkg does)
        self.fn = lambda raw, n, out, size: f(raw, n, out, size, 1, 0, 0, None, None, None, None,
                                              None, None, 3)

    def decompress(self, raw, size):
        out = ctypes.create_string_buffer(size)
        n = self.fn(raw, len(raw), out, size)
        if n != size:
            raise RuntimeError(f"OodleLZ_Decompress returned {n} for a {len(raw)}-byte block "
                               f"(expected {size})")
        return out.raw[:n]


def packages(args_paths):
    paths = [Path(p) for p in args_paths] or [default_dir()]
    for p in paths:
        if p.is_dir():
            yield from sorted(p.glob("*.pkg"))
        else:
            yield p


def cmd_info(a):
    p = Package(a.pkg)
    plat = PLATFORMS[p.platform] if p.platform < len(PLATFORMS) else p.platform
    lang = LANGUAGES[p.language] if p.language < len(LANGUAGES) else p.language
    print(f"{p.path.name}: version {p.version}, platform {plat}, package id 0x{p.pkg_id:04x}, "
          f"patch {p.patch_id}, language {lang}")
    print(f"registration value 0x{p.reg_value:016x}, build time 0x{p.build_time:016x}, "
          f"build id 0x{p.build_id:08x}")
    print(f"tool: {p.tool}")
    for name, (count, offset, _) in p.tables.items():
        print(f"{name}: {count} at 0x{offset:x}, SHA-1 {'ok' if p.table_hash_ok(name) else 'MISMATCH'}")
    print(f"file size field {p.file_size}, actual {len(p.data)}")
    flags = {}
    for b in p.blocks:
        flags[b["flags"]] = flags.get(b["flags"], 0) + 1
    print("block flags: " + ", ".join(f"0x{k:04x} x{v}" for k, v in sorted(flags.items())))
    first = next((b for b in p.blocks if b["flags"] & FLAG_COMPRESSED and b["patch"] == p.patch_id), None)
    if first:
        print(f"first compressed block: {oodle_codec(p.stored_block(first))}")
    types = {}
    for e in p.entries:
        types[(e["type"], e["subtype"])] = types.get((e["type"], e["subtype"]), 0) + 1
    print("entry type:subtype: " + ", ".join(f"{t}:{s} x{n}" for (t, s), n in sorted(types.items())))


def cmd_entries(a):
    p = Package(a.pkg)
    for e in p.entries:
        if (a.type is None or e["type"] == a.type) and (a.subtype is None or e["subtype"] == a.subtype):
            print(f"{e['index']:5d} 0x{tag_hash(p.pkg_id, e['index']):08x} ref 0x{e['reference']:08x} "
                  f"word 0x{e['word']:08x} type {e['type']}:{e['subtype']} block {e['block']}"
                  f"+0x{e['offset']:x} size {e['size']}")


def cmd_blocks(a):
    p = Package(a.pkg)
    bad = 0
    for b in p.blocks:
        line = (f"{b['index']:5d} off 0x{b['offset']:08x} size {b['size']:7d} patch {b['patch']} "
                f"flags 0x{b['flags']:04x}")
        if a.verify:
            try:
                ok = hashlib.sha1(p.stored_block(b)).digest() == b["sha1"]
            except OSError as exc:
                ok, line = False, line + f" ({exc.strerror})"
            bad += not ok
            line += " sha1 ok" if ok else " sha1 MISMATCH"
        print(line)
    if a.verify:
        print(f"{len(p.blocks) - bad}/{len(p.blocks)} blocks match their SHA-1")


def cmd_named(a):
    needle = a.grep.lower() if a.grep else None
    for path in packages(a.paths):
        try:
            p = Package(path)
        except (ValueError, struct.error) as exc:
            print(f"# {path.name}: {exc}", file=sys.stderr)
            continue
        for n in p.named:
            if needle is None or needle in n["name"].lower():
                print(f"{path.name} 0x{n['hash']:08x} class 0x{n['class']:08x} {n['name']}")


def cmd_locate(a):
    value = parse_int(a.hash)
    where = split_tag_hash(value)
    if where is None:
        sys.exit(f"0x{value:08x} is not a package tag hash")
    pkg_id, index = where
    print(f"0x{value:08x} = package 0x{pkg_id:04x}, entry {index}")
    for path in packages(a.paths):
        try:
            p = Package(path)
        except (ValueError, struct.error):
            continue
        if p.pkg_id == pkg_id and index < len(p.entries):
            e = p.entries[index]
            print(f"  {path.name}: type {e['type']}:{e['subtype']}, size {e['size']}, "
                  f"reference 0x{e['reference']:08x}")


def resolve_entry(p, text):
    value = parse_int(text)
    if value < len(p.entries):
        return value
    where = split_tag_hash(value)
    if not where or where[0] != p.pkg_id:
        sys.exit(f"{text} is neither an entry index nor a tag of package 0x{p.pkg_id:04x}")
    return where[1]


def cmd_extract(a):
    p = Package(a.pkg)
    oodle = Oodle(a.oodle) if a.oodle else None
    data = p.entry_data(resolve_entry(p, a.entry), oodle)
    Path(a.out).write_bytes(data)
    print(f"wrote {len(data)} bytes to {a.out}")


# --- tag layouts (PS3, big-endian, 32-bit fields) ---------------------------------------------
# A DynamicArray field is {u32 count, u32 offset from the count field} pointing at a 0x14-byte
# header {0x80800184, count, element class, 0, 0} followed by the elements. The Cosmodrome
# destination root (map:cosmo:root, class 0x8080052e) is {u32 size, u32 names tag, u32 tag,
# array @0xc of u32 bubble definition tags}; its names tag (0x80800636) holds an array @0xc of
# 0x18-byte location rows {u32, u32, u32 name hash, u8 kind + u16 row index, i32 bubble index
# (-1 = not a bubble), u32 flags}. Names are FNV-1 hashes of short dev names (bubble 14 =
# "highway"); kind 0 = bubble, 1 and 2 = other named locations.
KNOWN_NAMES = """array blast breach graveyard hangar highway refinery rocketyard ship dome_a dome_b dome_c
dome_d dome_hive hive_shore breach_sky sky_dome_hive""".split()


def fnv1(data, h=0x811c9dc5):
    for c in data:
        h = ((h * 0x01000193) & 0xffffffff) ^ c
    return h


def dyn_array(data, field):
    count, rel = struct.unpack_from(">II", data, field)
    head = field + rel
    marker, count2, elem_class = struct.unpack_from(">III", data, head)
    if marker != 0x80800184 or count2 != count:
        raise ValueError(f"no array header at +{field:#x}")
    return count, head + 0x14, elem_class


class PackageSet:
    """Packages of one folder by id, so tags can be read wherever they live."""

    def __init__(self, paths, oodle=None):
        self.oodle, self.by_id = oodle, {}
        for path in packages(paths):
            try:
                p = Package(path)
            except (ValueError, struct.error):
                continue
            have = self.by_id.get(p.pkg_id)
            # one package per id: language-neutral first, and the newest patch of a patch chain
            # (a patch _N file reads its unchanged blocks from the _0.._N-1 files beside it)
            if have is None or (p.language == 0, p.patch_id) > (have.language == 0, have.patch_id):
                self.by_id[p.pkg_id] = p

    def tag(self, value):
        where = split_tag_hash(value)
        p = self.by_id.get(where[0]) if where else None
        if not p or where[1] >= len(p.entries):
            return None
        return p.entry_data(where[1], self.oodle)


def cmd_bubbles(a):
    ps = PackageSet(a.paths, Oodle(a.oodle) if a.oodle else None)
    root = ps.tag(parse_int(a.root))
    if root is None:
        sys.exit(f"{a.root}: package not found")
    count, off, _ = dyn_array(root, 0xc)
    bubbles = struct.unpack_from(f">{count}I", root, off)
    names = {fnv1(n.encode()): n for n in KNOWN_NAMES + (a.names.split(",") if a.names else [])}
    table = ps.tag(struct.unpack_from(">I", root, 4)[0])
    rows = []
    if table is not None:
        n, off, _ = dyn_array(table, 0xc)
        rows = [struct.unpack_from(">IIIIiI", table, off + 0x18 * i) for i in range(n)]
    by_bubble = {r[4]: r for r in rows if r[4] >= 0}
    for i, tag in enumerate(bubbles):
        row = by_bubble.get(i)
        name = names.get(row[2], f"{row[2]:#010x}") if row else "?"
        where = split_tag_hash(tag)
        pkg = ps.by_id.get(where[0])
        detail = f"package {where[0]:#06x} not here"
        if pkg:
            d = ps.tag(tag)
            detail = f"{dyn_array(d, 4)[0]} map containers"
        print(f"bubble {i:2d} {tag:#010x} {name:14} {detail}")
    others = [r for r in rows if r[4] < 0]
    if others:
        print(f"{len(others)} other named locations: " + ", ".join(
            names.get(r[2], f"{r[2]:#010x}") + f" (kind {r[3] >> 24})" for r in others[:12]) + " ...")


# Replicated entity groups (class 0x8080076e): {u32 size, u32 destination hash, u32 group key
# (FNV-1 of the group's name, the key of its replicated object ids), u32, array @0x10 of objects
# {u8 kind, 3 bytes, u32}, u32, array @0x1c, ...}, then one NUL-terminated dev name per object in
# the same order. An object's replicated id is (group key, kind, position in this list): bubble
# 14's group 0xecf574b5 has kind 4 at 0-27 (o_ghost_*, pigeon_1-22), kind 1 sq_ghost at 28, as in
# the live client's records (TEST362).
GROUP_CLASS = 0x8080076E


def entity_group(data):
    key = struct.unpack_from(">I", data, 8)[0]
    count, off, _ = dyn_array(data, 0x10)
    kinds = [data[off + 8 * i] for i in range(count)]
    names = [m.group(1).decode("latin-1")
             for m in __import__("re").finditer(rb"([\x20-\x7e]+)\x00", data[off + 8 * count:])]
    names = names[-count:] if len(names) >= count else names + [""] * (count - len(names))
    return key, list(zip(kinds, names))


def cmd_groups(a):
    oodle = Oodle(a.oodle) if a.oodle else None
    want = {parse_int(k) for k in a.key.split(",")} if a.key else None
    for path in packages(a.paths):
        p = Package(path)
        for e in p.entries:
            if e["reference"] != GROUP_CLASS:
                continue
            try:
                key, objects = entity_group(p.entry_data(e["index"], oodle))
            except (ValueError, struct.error, RuntimeError) as exc:
                print(f"# {path.name} entry {e['index']}: {exc}", file=sys.stderr)
                continue
            if want and key not in want:
                continue
            print(f"group {key:#010x} ({len(objects)} objects) tag {tag_hash(p.pkg_id, e['index']):#010x} {path.name}")
            if a.objects:
                for i, (kind, name) in enumerate(objects):
                    print(f"  {key:#010x}/{kind:#x}/{i} {name}")


# Baselines: the replicated objects the host must list (with a state-B bit) before the client
# applies any replicated state (0x1ce1b8). The client creates a record for an object once the
# object's definition is loaded (objectives and sequences always; triggers, props and squads with
# their bubble's map containers) and skips ids it has no record for, so a superset of a mission's
# groups is safe while a missing object blocks everything. State B presence is per kind (TEST350,
# TEST362: the kinds below); kinds 0x2c/0x37/0x3a/0x3b/0x3c/0x3d are not replicated.
KINDS_WITH_B = {0x1, 0x2, 0x3, 0x4, 0xC, 0x1C, 0x27}
KINDS_NOT_REPLICATED = {0x2C, 0x37, 0x3A, 0x3B, 0x3C, 0x3D}


def baselines(ps, keys):
    """[(key, kind, index, has_b)] for every replicated object of the groups `keys`."""
    out = []
    for p in ps.by_id.values():
        for e in p.entries:
            if e["reference"] != GROUP_CLASS:
                continue
            try:
                key, objects = entity_group(p.entry_data(e["index"], ps.oodle))
            except (ValueError, struct.error, RuntimeError):
                continue
            if key in keys and not any(o[0] == key for o in out):
                out += [(key, kind, i, kind in KINDS_WITH_B) for i, (kind, _) in enumerate(objects)
                        if kind not in KINDS_NOT_REPLICATED]
    return out


def cmd_baselines(a):
    ps = PackageSet(a.paths, Oodle(a.oodle) if a.oodle else None)
    keys = [parse_int(k) for k in a.keys.split(",")]
    found = baselines(ps, set(keys))
    missing = [k for k in keys if not any(o[0] == k for o in found)]
    if missing:
        print("# groups not found: " + ", ".join(f"{k:#010x}" for k in missing), file=sys.stderr)
    print(",".join(f"{k:#010x}/{kind:#x}/{i}" + ("/1" if b else "") for k, kind, i, b in found))


# Localized strings. An index tag (class 0x8080035a) is {u32 size, u32 count, u32, 12 language
# table tags from +0xc (English first), array of `count` sorted u32 string hashes}. A language
# table (class 0x808008be) has an array @0x4 of 0x14-byte parts {u32, u32 offset from the
# offset field, u32, u16 byte length, u16 char length, u16 shift, u16}, one per hash in order;
# each text byte is stored minus `shift` (0x1f). Objective definitions refer to their title and
# description by these hashes ("Enter the Wall" = 0x52ff1516).
STRING_INDEX_CLASS = 0x8080035A


def string_table(index, table):
    count = struct.unpack_from(">I", index, 4)[0]
    hashes = struct.unpack_from(f">{count}I", index, index.find(struct.pack(">II", 0x80800184, count)) + 0x14)
    n, off, _ = dyn_array(table, 0x4)
    texts = []
    for i in range(n):
        base = off + 0x14 * i
        _, rel, _, blen, _, shift, _ = struct.unpack_from(">IIIHHHH", table, base)
        raw = table[base + 4 + rel:base + 4 + rel + blen]
        texts.append(bytes((c + shift) & 0xff for c in raw).decode("utf-8", "replace"))
    return dict(zip(hashes, texts))


def all_strings(ps):
    """{hash: text} from every localized string index in the package set."""
    found = {}
    for p in ps.by_id.values():
        for e in p.entries:
            if e["reference"] != STRING_INDEX_CLASS:
                continue
            try:
                index = p.entry_data(e["index"], ps.oodle)
                table = ps.tag(struct.unpack_from(">I", index, 0xc)[0])
                if table:
                    for h, text in string_table(index, table).items():
                        found.setdefault(h, text)
            except (ValueError, struct.error, RuntimeError):
                continue
    return found


def cmd_strings(a):
    ps = PackageSet(a.paths, Oodle(a.oodle) if a.oodle else None)
    needle = a.grep.lower() if a.grep else None
    found = all_strings(ps)
    for h, text in sorted(found.items()):
        if needle is None or needle in text.lower() or needle == f"{h:08x}":
            print(f"{h:#010x} {text!r}")


# Investment globals (root tag named "investment_globals", class 0x80803389): +0xb0 the 14
# equippable buckets (28-byte entries: u32 bucket hash, {i16, u32} name, {i16, u32}
# description), +0xd8 the item list whose position is the i16 item index of the character data
# (0x28a304; 8-byte entries, u32 definition tag at +4). An item definition (class 0x80803484)
# holds {i16, u32 string hash} name / type / description at +0x24 / +0x2c / +0x34 and, for
# armour, weapons, ships, vehicles and ghosts, its bucket hash at +0x44 or +0x48. The bucket
# hashes are the ones Bungie's public Destiny 1 API used (Helmet 0xcd887e07, ...).
INVESTMENT_ROOT_NAME = "investment_globals"
ITEM_NAME_REFS = (0x28, 0x30, 0x38)


def investment_items(ps):
    """(buckets, items) of the investment globals: buckets = [(hash, name)] in equippable order,
    items = [{"index", "tag", "slot", "bucket", "type", "name", "description"}] in item index
    order; "slot" is the item's position in the equippable bucket list (None: not equippable
    through a bucket hash, e.g. subclasses, class items, emblems, shaders)."""
    root_hash = next((n["hash"] for p in ps.by_id.values() for n in p.named
                      if n["name"] == INVESTMENT_ROOT_NAME), None)
    root = ps.tag(root_hash) if root_hash else None
    if root is None:
        raise ValueError("investment_globals root tag not found (ps3_investment_globals_012c)")
    strings = all_strings(ps)
    text = lambda data, off: strings.get(struct.unpack_from(">I", data, off)[0], "")
    bucket_list = ps.tag(struct.unpack_from(">I", root, 0xb0)[0])
    n, first, _ = dyn_array(bucket_list, 4)
    buckets = []
    for i in range(n):
        entry = first + 0x1c * i
        buckets.append((struct.unpack_from(">I", bucket_list, entry)[0], text(bucket_list, entry + 8)))
    bucket_slot = {h: i for i, (h, _) in enumerate(buckets)}
    item_list = ps.tag(struct.unpack_from(">I", root, 0xd8)[0])
    n, first, _ = dyn_array(item_list, 4)
    items = []
    for i in range(n):
        tag = struct.unpack_from(">I", item_list, first + 8 * i + 4)[0]
        d = ps.tag(tag) or b""
        name, kind, desc = (text(d, o) if len(d) >= o + 4 else "" for o in ITEM_NAME_REFS)
        slot = next((bucket_slot[v] for o in (0x44, 0x48) if len(d) >= o + 4
                     for v in [struct.unpack_from(">I", d, o)[0]] if v in bucket_slot), None)
        items.append({"index": i, "tag": tag, "slot": slot, "bucket": buckets[slot][1] if slot is not None else "",
                      "type": kind.strip(), "name": name.strip(), "description": desc})
    return buckets, items


def cmd_items(a):
    ps = PackageSet(a.paths, Oodle(a.oodle) if a.oodle else None)
    buckets, items = investment_items(ps)
    if a.buckets:
        for i, (h, name) in enumerate(buckets):
            print(f"{i:2}  0x{h:08x}  {name}")
        return
    needle = a.grep.lower() if a.grep else None
    for it in items:
        slot = "-" if it["slot"] is None else it["slot"]
        line = (f"{it['index']:5}  0x{it['tag']:08x}  {slot:>2} {it['bucket'] or '-':<16} "
                f"{it['type'] or '-':<22} {it['name']}")
        if needle is None or needle in (line + " " + it["description"]).lower():
            print(line + (f"  | {it['description'][:90]}" if a.descriptions else ""))


def cmd_scan(a):
    needle = bytes.fromhex(a.hex) if a.hex else a.find.encode()
    oodle = Oodle(a.oodle) if a.oodle else None
    for path in packages(a.paths):
        try:
            p = Package(path)
        except (ValueError, struct.error):
            continue
        for e in p.entries:
            try:
                data = p.entry_data(e["index"], oodle)
            except (RuntimeError, OSError):
                continue
            pos = data.find(needle)
            while pos >= 0:
                ctx = data[max(0, pos - 16):pos + len(needle) + 16]
                print(f"{path.name} entry {e['index']} (0x{tag_hash(p.pkg_id, e['index']):08x}, "
                      f"type {e['type']}:{e['subtype']}) +0x{pos:x}: {ctx.hex()}")
                pos = data.find(needle, pos + 1)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("info"); s.add_argument("pkg"); s.set_defaults(fn=cmd_info)
    s = sub.add_parser("entries"); s.add_argument("pkg")
    s.add_argument("--type", type=int); s.add_argument("--subtype", type=int); s.set_defaults(fn=cmd_entries)
    s = sub.add_parser("blocks"); s.add_argument("pkg"); s.add_argument("--verify", action="store_true")
    s.set_defaults(fn=cmd_blocks)
    s = sub.add_parser("named"); s.add_argument("paths", nargs="*"); s.add_argument("--grep")
    s.set_defaults(fn=cmd_named)
    s = sub.add_parser("locate"); s.add_argument("hash"); s.add_argument("paths", nargs="*")
    s.set_defaults(fn=cmd_locate)
    s = sub.add_parser("extract"); s.add_argument("pkg"); s.add_argument("entry"); s.add_argument("out")
    s.add_argument("--oodle"); s.set_defaults(fn=cmd_extract)
    s = sub.add_parser("bubbles"); s.add_argument("root", help="destination root tag, e.g. 80cda002")
    s.add_argument("paths", nargs="*"); s.add_argument("--oodle"); s.add_argument("--names")
    s.set_defaults(fn=cmd_bubbles)
    s = sub.add_parser("groups"); s.add_argument("paths", nargs="*"); s.add_argument("--oodle")
    s.add_argument("--key", help="only these group keys (comma-separated)")
    s.add_argument("--objects", action="store_true", help="list each object: key/kind/index name")
    s.set_defaults(fn=cmd_groups)
    s = sub.add_parser("strings"); s.add_argument("paths", nargs="*"); s.add_argument("--oodle")
    s.add_argument("--grep", help="text substring or an 8-digit hash"); s.set_defaults(fn=cmd_strings)
    s = sub.add_parser("baselines", help="--ah-baselines list (KEY/KIND/INDEX[/1]) for replicated groups")
    s.add_argument("paths", nargs="*"); s.add_argument("--oodle")
    s.add_argument("--keys", required=True, help="group keys, e.g. 0x992bb471,0x3c2f5ed5,0x4786c0e0,0xecf574b5")
    s.set_defaults(fn=cmd_baselines)
    s = sub.add_parser("items", help="the investment item list: item index, bucket, type, name")
    s.add_argument("paths", nargs="*"); s.add_argument("--oodle"); s.add_argument("--grep")
    s.add_argument("--buckets", action="store_true", help="list the equippable buckets instead")
    s.add_argument("--descriptions", action="store_true"); s.set_defaults(fn=cmd_items)
    s = sub.add_parser("scan"); s.add_argument("paths", nargs="*")
    g = s.add_mutually_exclusive_group(required=True); g.add_argument("--find"); g.add_argument("--hex")
    s.add_argument("--oodle"); s.set_defaults(fn=cmd_scan)
    a = ap.parse_args(argv)
    a.fn(a)


if __name__ == "__main__":
    main()
