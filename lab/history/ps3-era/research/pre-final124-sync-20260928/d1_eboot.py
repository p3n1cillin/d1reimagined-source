"""Static analysis of the decrypted Destiny EBOOT (BLES01857 01.00, `rpcs3.exe --decrypt` output,
ELF64 big-endian PPU): strings, code and data references to an address or a string, callers of a
function, the function around an address, and disassembly.

The SNC compiler forms addresses as `lis rX, hi` followed by `addic`/`addi`/`ori` or a D-form
load/store with base rX, or loads them from the TOC (`lwz/ld rX, off(r2)`; TOC base 0x175a5e8,
orbit checkpoint "Slice sets and cinematics"). Function starts are taken from `bl` targets and
frame-setup prologues after an unconditional branch (`mflr r0` for SNC's out-of-line register
save helpers, or `stdu r1`) and the .opd descriptors; a leaf function without a frame that is
only reached through a pointer can still merge with the one before it.

  python scripts/d1_eboot.py strings [--grep TEXT] [--min 6]
  python scripts/d1_eboot.py xref 0x11b7218 | "Change slice-set"    (code, TOC and data refs)
  python scripts/d1_eboot.py callers 0x13f8ec
  python scripts/d1_eboot.py func 0x1f6e98                           (containing function)
  python scripts/d1_eboot.py disasm 0x1f6aa4 [--count 40] [--func]   (needs `pip install capstone`)

Default EBOOT: local/RESEARCH/ps3_decrypt/EBOOT.elf (or --elf PATH). Addresses print with their
curated names from notes/eboot-labels.tsv. The first run builds a small index next to the ELF
(`.d1index-v2.npz`, function starts) to keep later queries fast.
"""

import argparse
import re
import struct
import sys
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[1]
TOC_BASE = 0x175a5e8
PT_LOAD = 1


def default_elf():
    sys.path.insert(0, str(REPO / "server"))
    from local_paths import LOCAL
    return LOCAL / "RESEARCH" / "ps3_decrypt" / "EBOOT.elf"


def labels():
    try:
        sys.path.insert(0, str(REPO / "scripts"))
        from d1_addresses import load_labels
        return {a: v["name"] for a, v in load_labels().items()}
    except (ImportError, OSError):
        return {}


class Eboot:
    def __init__(self, path, toc=TOC_BASE):
        self.path = Path(path)
        self.data = self.path.read_bytes()
        d = self.data
        if d[:4] != b"\x7fELF" or d[4] != 2 or d[5] != 2:
            raise ValueError(f"{path}: not a big-endian ELF64 (decrypt the EBOOT with rpcs3 --decrypt)")
        phoff, = struct.unpack_from(">Q", d, 0x20)
        phentsize, phnum = struct.unpack_from(">HH", d, 0x36)
        self.segments = []  # (vaddr, file offset, file size, flags)
        for i in range(phnum):
            p_type, p_flags, p_offset, p_vaddr, _, p_filesz = struct.unpack_from(">IIQQQQ", d, phoff + i * phentsize)
            if p_type == PT_LOAD and p_filesz:
                self.segments.append((p_vaddr, p_offset, p_filesz, p_flags))
        code = [s for s in self.segments if s[3] & 1] or self.segments[:1]
        self.code_va, off, size, _ = code[0]
        self.words = np.frombuffer(d, dtype=">u4", count=size // 4, offset=off).astype(np.uint32)
        self.toc = toc
        self._targets = None
        self.names = labels()

    # --- memory
    def read(self, va, size):
        for vaddr, off, filesz, _ in self.segments:
            if vaddr <= va and va + size <= vaddr + filesz:
                return self.data[off + va - vaddr:off + va - vaddr + size]
        return None

    def u32(self, va):
        raw = self.read(va, 4)
        return struct.unpack(">I", raw)[0] if raw else None

    def name(self, va):
        n = self.names.get(va)
        return f"0x{va:x}" + (f" <{n}>" if n else "")

    # --- strings
    def strings(self, minimum=6):
        pattern = re.compile(rb"[\x20-\x7e\t\n]{%d,}\x00" % minimum)
        for vaddr, off, filesz, flags in self.segments:
            for m in pattern.finditer(self.data, off, off + filesz):
                yield vaddr + m.start() - off, m.group()[:-1].decode("latin-1")

    def find_string(self, text):
        needle = text.encode("latin-1")
        for vaddr, off, filesz, flags in self.segments:
            pos = self.data.find(needle, off, off + filesz)
            while pos >= 0:
                start = pos
                while start > off and self.data[start - 1] not in (0,):
                    start -= 1
                yield vaddr + start - off, vaddr + pos - off
                pos = self.data.find(needle, pos + 1, off + filesz)

    # --- instruction fields (vectorized over the code segment)
    @property
    def fields(self):
        w = self.words
        return w >> 26, (w >> 21) & 31, (w >> 16) & 31, (w & 0xffff).astype(np.int32)

    def pc(self, index):
        return self.code_va + 4 * int(index)

    # D-form instructions that use rA as a base or source (addic/addic./addi and loads/stores);
    # 58 (ld/lwa) and 62 (std) are DS-form: the low two immediate bits are part of the opcode.
    BASE_OPS = {12, 13, 14, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 48, 49, 50, 51,
                52, 53, 54, 55, 58, 62}
    STORES = {36, 37, 38, 39, 44, 45, 52, 53, 54, 55, 62}  # the rD/rS slot is read, not written

    def code_refs(self, target, window=24):
        """(pc, lis pc) of instructions completing `lis rX, hi` + (addic/addi/ori/load/store
        with base rX) = target, searched up to `window` instructions after the lis."""
        op, rd, ra, imm = self.fields
        lo = target & 0xffff
        forms = {((target + 0x8000) >> 16) & 0xffff: "signed"}
        forms.setdefault(target >> 16 & 0xffff, "ori")
        if lo < 0x8000:
            forms[target >> 16 & 0xffff] = "both"
        out = []
        for hi, kind in forms.items():
            for i in np.nonzero((op == 15) & (ra == 0) & (imm == hi))[0]:
                reg = int(rd[i])
                for j in range(i + 1, min(i + 1 + window, len(self.words))):
                    o, a, d, v = int(op[j]), int(ra[j]), int(rd[j]), int(imm[j])
                    if kind != "ori" and o in self.BASE_OPS and a == reg and \
                            (v & 0xfffc if o in (58, 62) else v) == lo:
                        out.append((self.pc(j), self.pc(i)))
                        break
                    if kind != "signed" and o == 24 and d == reg and v == lo:
                        out.append((self.pc(j), self.pc(i)))  # ori rA, rS, lo (rS in the rD slot)
                        break
                    if (o in (24, 25, 26, 27, 28, 29) and a == reg) or \
                            (d == reg and o not in self.STORES and o not in (16, 17, 18, 19, 24, 25, 26,
                                                                             27, 28, 29, 31)):
                        break  # rX overwritten (X-form op 31 is not decoded: kept as a possible hit)
        return sorted(set(out))

    def toc_refs(self, target):
        """Loads through r2 of a TOC slot holding target: (pc, slot)."""
        op, rd, ra, imm = self.fields
        out = []
        for vaddr, off, filesz, flags in self.segments:
            words = np.frombuffer(self.data, dtype=">u4", count=filesz // 4, offset=off)
            for k in np.nonzero(words == target)[0]:
                slot = vaddr + 4 * int(k)
                delta = slot - self.toc
                if -0x8000 <= delta < 0x8000:
                    hits = np.nonzero(((op == 32) | (op == 58)) & (ra == 2) & (imm == (delta & 0xffff)))[0]
                    out += [(self.pc(i), slot) for i in hits]
        return out

    def data_refs(self, target):
        """Aligned 32-bit words equal to target in any segment (pointer tables, vtables; the
        executable segment also holds the read-only data)."""
        out = []
        for vaddr, off, filesz, flags in self.segments:
            words = np.frombuffer(self.data, dtype=">u4", count=filesz // 4, offset=off)
            out += [vaddr + 4 * int(k) for k in np.nonzero(words == target)[0]]
        return out

    # --- calls and functions
    def call_sites(self):
        """(pc array, target array) of every `bl`."""
        w = self.words
        is_bl = (w >> 26 == 18) & (w & 3 == 1)
        idx = np.nonzero(is_bl)[0]
        li = (w[idx] & 0x3fffffc).astype(np.int64)
        li = np.where(li & 0x2000000, li - 0x4000000, li)
        pcs = self.code_va + 4 * idx.astype(np.int64)
        return pcs, pcs + li

    def function_starts(self):
        if self._targets is None:
            cache = self.path.with_suffix(".d1index-v2.npz")  # bump when the start rules change
            if cache.exists() and cache.stat().st_mtime >= self.path.stat().st_mtime:
                self._targets = np.load(cache)["starts"]
            else:
                _, targets = self.call_sites()
                # Functions reached only through pointers: a frame setup (`mflr r0`, used with
                # SNC's out-of-line register-save helpers, or `stdu r1, -N(r1)`) right after an
                # unconditional b / blr / bctr or padding.
                w = self.words
                prev = np.concatenate([[0x4e800020], w[:-1]])
                after_end = ((prev == 0x4e800020) | (prev == 0x4e800420) | ((prev >> 26 == 18) & (prev & 3 == 0))
                             | (prev == 0x60000000) | (prev == 0))
                frame = (w == 0x7c0802a6) | ((w & 0xffff0003) == 0xf8210001)
                prologues = self.code_va + 4 * np.nonzero(after_end & frame)[0].astype(np.int64)
                opd = []
                for vaddr, off, filesz, flags in self.segments:
                    words = np.frombuffer(self.data, dtype=">u4", count=filesz // 4, offset=off)
                    # .opd descriptors: {u32 function, u32 toc}
                    pairs = np.nonzero((words[1:] == self.toc) & (words[:-1] >= self.code_va) &
                                       (words[:-1] < self.code_va + 4 * len(self.words)))[0]
                    opd += [int(words[k]) for k in pairs]
                starts = np.unique(np.concatenate([targets, prologues, np.array(opd, dtype=np.int64)]))
                self._targets = starts[(starts >= self.code_va) & (starts < self.code_va + 4 * len(self.words))]
                try:
                    np.savez(cache, starts=self._targets)
                except OSError:
                    pass
        return self._targets

    def function_of(self, va):
        starts = self.function_starts()
        k = np.searchsorted(starts, va, side="right") - 1
        if k < 0:
            return None, None
        start = int(starts[k])
        end = int(starts[k + 1]) if k + 1 < len(starts) else self.code_va + 4 * len(self.words)
        return start, end

    def callers(self, target):
        pcs, targets = self.call_sites()
        return [int(p) for p in pcs[targets == target]]

    def disasm(self, va, count):
        try:
            import capstone
        except ImportError:
            sys.exit("disasm needs capstone: pip install capstone")
        md = capstone.Cs(capstone.CS_ARCH_PPC, capstone.CS_MODE_64 | capstone.CS_MODE_BIG_ENDIAN)
        raw = self.read(va, 4 * count) or b""
        for i in md.disasm(raw, va):
            note = ""
            m = re.search(r"0x([0-9a-f]+)$", i.op_str)
            if i.mnemonic in ("bl", "b") and m:
                note = "  ; " + self.name(int(m.group(1), 16))
            yield f"{i.address:08x}  {i.bytes.hex()}  {i.mnemonic:8} {i.op_str}{note}"


def parse_target(eb, text):
    if re.fullmatch(r"0x[0-9a-fA-F]+", text):
        return [(int(text, 16), text)]
    return [(start, eb.read(start, 200).split(b"\0")[0].decode("latin-1")) for start, _ in eb.find_string(text)]


def describe(eb, pc):
    start, _ = eb.function_of(pc)
    return f"0x{pc:x} in {eb.name(start) if start else '?'}"


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--elf", help="decrypted EBOOT (default local/RESEARCH/ps3_decrypt/EBOOT.elf)")
    ap.add_argument("--toc", type=lambda v: int(v, 0), default=TOC_BASE)
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("strings"); s.add_argument("--grep"); s.add_argument("--min", type=int, default=6)
    s = sub.add_parser("xref"); s.add_argument("target")
    s = sub.add_parser("callers"); s.add_argument("target")
    s = sub.add_parser("func"); s.add_argument("target")
    s = sub.add_parser("disasm"); s.add_argument("target"); s.add_argument("--count", type=int, default=40)
    s.add_argument("--func", action="store_true", help="the whole function containing the address")
    a = ap.parse_args(argv)
    eb = Eboot(a.elf or default_elf(), a.toc)

    if a.cmd == "strings":
        needle = a.grep.lower() if a.grep else None
        for va, text in eb.strings(a.min):
            if needle is None or needle in text.lower():
                print(f"0x{va:08x} {text!r}")
    elif a.cmd == "xref":
        for target, label in parse_target(eb, a.target):
            print(f"== 0x{target:x} {label!r}")
            for pc, lis in eb.code_refs(target):
                print(f"  code {describe(eb, pc)} (lis at 0x{lis:x})")
            for pc, slot in eb.toc_refs(target):
                print(f"  toc  {describe(eb, pc)} (slot 0x{slot:x})")
            for va in eb.data_refs(target):
                print(f"  data 0x{va:x}")
    elif a.cmd == "callers":
        target = int(a.target, 16)
        for pc in eb.callers(target):
            print(f"  {describe(eb, pc)}")
    elif a.cmd == "func":
        start, end = eb.function_of(int(a.target, 16))
        print(f"{eb.name(start)} .. 0x{end:x} ({(end - start) // 4} instructions), "
              f"{len(eb.callers(start))} callers")
    elif a.cmd == "disasm":
        va = int(a.target, 16)
        count = a.count
        if a.func:
            va, end = eb.function_of(va)
            count = (end - va) // 4
        for line in eb.disasm(va, count):
            print(line)


if __name__ == "__main__":
    main()
