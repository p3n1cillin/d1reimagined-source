import struct
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import d1_eboot  # noqa: E402

CODE, DATA, TOC = 0x10000, 0x20000, 0x28400


def lis(rd, hi): return (15 << 26) | (rd << 21) | (hi & 0xffff)
def addic(rd, ra, v): return (12 << 26) | (rd << 21) | (ra << 16) | (v & 0xffff)
def ori(ra, rs, v): return (24 << 26) | (rs << 21) | (ra << 16) | (v & 0xffff)
def lwz(rd, ra, v): return (32 << 26) | (rd << 21) | (ra << 16) | (v & 0xffff)
def addi(rd, ra, v): return (14 << 26) | (rd << 21) | (ra << 16) | (v & 0xffff)
def bl(pc, target): return (18 << 26) | ((target - pc) & 0x3fffffc) | 1
NOP, BLR = 0x60000000, 0x4e800020


def build(path):
    s1, s2 = DATA + 0x10, DATA + 0x8010  # s2's low half has bit 15 set
    code = [
        lis(3, (s1 + 0x8000) >> 16), addic(3, 3, s1), bl(CODE + 8, CODE + 0x40), BLR,   # f0: refs s1
        lis(4, (s2 + 0x8000) >> 16), NOP, addic(5, 4, s2 & 0xffff), BLR,                 # refs s2 signed
        lis(6, s1 >> 16), ori(6, 6, s1 & 0xffff), BLR, NOP,                             # refs s1 via ori
        lis(7, (s1 + 0x8000) >> 16), addi(7, 0, 5), addic(8, 7, s1), BLR,              # r7 overwritten
        lwz(9, 2, (TOC + 0x10 - TOC)), bl(CODE + 0x44, CODE + 0x40), BLR, NOP,          # f @0x40: TOC ref
        0x7C0802A6, BLR,                                  # @0x50: pointer-only function (mflr r0)
    ]
    code_bytes = b"".join(struct.pack(">I", w) for w in code)
    data = bytearray(0x9000)
    data[0x10:0x10 + 15] = b"objective test\0"
    data[0x8010:0x8010 + 12] = b"second one\0\0"
    struct.pack_into(">I", data, TOC + 0x10 - DATA, s1)          # TOC slot -> s1
    struct.pack_into(">II", data, 0x100, CODE + 0x10, d1_eboot.TOC_BASE)  # an .opd-style pair
    ehdr = bytearray(0x40)
    ehdr[:6] = b"\x7fELF\x02\x02"
    struct.pack_into(">QHH", ehdr, 0x20, 0x40, 0, 0)
    struct.pack_into(">HH", ehdr, 0x36, 56, 2)
    ph = b""
    code_off, data_off = 0x100, 0x100 + len(code_bytes)
    for vaddr, off, size, flags in ((CODE, code_off, len(code_bytes), 5), (DATA, data_off, len(data), 6)):
        ph += struct.pack(">IIQQQQQQ", 1, flags, off, vaddr, vaddr, size, size, 0x10)
    blob = bytes(ehdr) + ph
    blob += bytes(code_off - len(blob)) + code_bytes + bytes(data)
    Path(path).write_bytes(blob)
    return s1, s2


class EbootTest(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.path = Path(self.dir.name) / "EBOOT.elf"
        self.s1, self.s2 = build(self.path)
        self.eb = d1_eboot.Eboot(self.path, toc=TOC)

    def tearDown(self):
        self.dir.cleanup()

    def test_strings(self):
        found = dict(self.eb.strings(6))
        self.assertEqual(found[self.s1], "objective test")
        self.assertEqual([va for va, _ in self.eb.find_string("test")], [self.s1])

    def test_code_refs_signed_ori_and_kill(self):
        refs = self.eb.code_refs(self.s1)
        self.assertIn((CODE + 4, CODE), refs)            # lis + addic
        self.assertIn((CODE + 0x24, CODE + 0x20), refs)  # lis + ori
        self.assertNotIn(CODE + 0x38, [pc for pc, _ in refs])  # r7 was overwritten first
        self.assertEqual(self.eb.code_refs(self.s2), [(CODE + 0x18, CODE + 0x10)])

    def test_toc_and_data_refs(self):
        self.assertEqual(self.eb.toc_refs(self.s1), [(CODE + 0x40, TOC + 0x10)])
        self.assertEqual(self.eb.data_refs(self.s1), [TOC + 0x10])

    def test_calls_and_functions(self):
        self.assertEqual(self.eb.callers(CODE + 0x40), [CODE + 8, CODE + 0x44])
        self.assertEqual(self.eb.function_of(CODE + 0x48)[0], CODE + 0x40)
        self.assertEqual(self.eb.function_of(CODE + 0x54)[0], CODE + 0x50)  # prologue after nop


if __name__ == "__main__":
    unittest.main()
