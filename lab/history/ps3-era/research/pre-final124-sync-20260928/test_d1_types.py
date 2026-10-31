import struct
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import d1_types  # noqa: E402


def definition(handle, size, fields):
    """A type definition as compiled into the EBOOT: size at +0x14, a relative pointer at +0x54 to
    the field list {count, handle, size, 8 bytes, then 0x28-byte field entries}."""
    head = bytearray(0x58)
    struct.pack_into(">I", head, 0x14, size)
    struct.pack_into(">I", head, 0x54, 4)  # the list follows the pointer word
    body = struct.pack(">III", len(fields), handle, size) + bytes(8)
    for off, kind, opt, nested, minimum, width in fields:
        e = bytearray(0x28)
        struct.pack_into(">III", e, 0, off, off, 0)
        e[0x10], e[0x11] = kind, opt
        struct.pack_into(">IIII", e, 0x14, nested, minimum, width, 0)
        body += bytes(e)
    return bytes(head) + body


class ScanTypes(unittest.TestCase):
    def test_finds_definitions_and_signature(self):
        inner = definition(0x80801704, 4, [(0, 3, 1, 0xFFFFFFFF, 0x80, 8), (2, 4, 1, 0xFFFFFFFF, 0x8000, 16)])
        outer = definition(0x8080183C, 0x28, [(0, 6, 1, 0xFFFFFFFF, 0, 64), (0xE, 1, 1, 0x80801704, 0, 0)])
        data = bytes(12) + inner + bytes(8) + outer + bytes(64)
        types = d1_types.scan_types(data)
        self.assertEqual(set(types), {"0x80801704", "0x8080183c"})
        self.assertEqual(types["0x8080183c"]["size"], 0x28)
        self.assertEqual(types["0x8080183c"]["fields"][1]["nested"], 0x80801704)
        self.assertEqual(d1_types.signature(types, 0x8080183C), "u64? {u8? i16?}?")


if __name__ == "__main__":
    unittest.main()
