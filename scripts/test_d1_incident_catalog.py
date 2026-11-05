import struct
import tempfile
import unittest
from pathlib import Path

import d1_incident_catalog as catalog


def fixture():
    raw = bytearray(0x100)
    # Independent tag layout: root array descriptor at 0x10, metadata at 0x3c,
    # two 20-byte records at 0x50. Relative definition links start in each +4 field.
    struct.pack_into(">II", raw, 0x10, 2, 0x2C)
    struct.pack_into(">III", raw, 0x3C, 0x80800184, 2, 0x808003D3)
    struct.pack_into(">II4HB3x", raw, 0x50, 0xAECA3822, 0xC0 - 0x54,
                     0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 17)
    struct.pack_into(">II4HB3x", raw, 0x64, 0x7A0FD954, 0xD0 - 0x68,
                     0xFFFF, 17, 0xFFFF, 8, 2)
    struct.pack_into(">II", raw, 0xC0, 0, 0x808003C2)
    struct.pack_into(">II", raw, 0xD0, 0, 0x808008E5)
    return raw


class IncidentCatalogTest(unittest.TestCase):
    def ps4_fixture(self):
        raw = bytearray(0x180)
        # Independently constructed 64-bit layout. Two records at 0x80 and 0xa0;
        # one definition forwards at 0x100, the other backwards at 0x50.
        struct.pack_into("<Q", raw, 0, len(raw))
        struct.pack_into("<Qq", raw, 0x20, 2, 0x48)
        struct.pack_into("<IQI", raw, 0x6c, 0x80800184, 2, 0x808003d3)
        struct.pack_into("<I4xq4HB7x", raw, 0x80, 0x3c03f059, 0x100 - 0x88,
                         0x1234, 0xabcd, 0xffff, 8, 17)
        struct.pack_into("<I4xq4HB7x", raw, 0xa0, 0x42660690, 0x50 - 0xa8,
                         0xffff, 5, 3, 2, 27)
        struct.pack_into("<II", raw, 0x100, 0, 0x808003c2)
        struct.pack_into("<II", raw, 0x50, 0, 0x808003bc)
        return raw

    def test_ps4_word_sizes_endian_stride_and_signed_links(self):
        a, b = catalog.parse_catalog(self.ps4_fixture(), ps4=True)["records"]
        self.assertEqual((a["index"], a["name_hash"], a["payload_selector"]),
                         (0, "0x3c03f059", 17))
        self.assertEqual(a["auxiliary_u16"], [0x1234, 0xabcd, 0xffff, 8])
        self.assertEqual((a["definition_offset"], a["definition_class"]), (0x100, "0x808003c2"))
        self.assertEqual((b["index"], b["name_hash"], b["payload_selector"]),
                         (1, "0x42660690", 27))
        self.assertEqual(b["auxiliary_u16"], [0xffff, 5, 3, 2])
        self.assertEqual((b["definition_offset"], b["definition_class"]), (0x50, "0x808003bc"))

    def test_ps4_rejects_malformed_sizes_headers_counts_and_links(self):
        mutations = [("<Q", 0, 0x100), ("<Q", 0x20, 0), ("<Q", 0x20, 65537),
                     ("<q", 0x28, -0x100), ("<q", 0x28, 0x1000),
                     ("<I", 0x6c, 0), ("<Q", 0x70, 3), ("<I", 0x78, 0x80802468),
                     ("<q", 0x88, -0x1000), ("<q", 0x88, 0x100000000)]
        for fmt, offset, value in mutations:
            raw = self.ps4_fixture()
            struct.pack_into(fmt, raw, offset, value)
            with self.subTest(offset=offset, value=value), self.assertRaises(ValueError):
                catalog.parse_catalog(raw, ps4=True)
        raw = self.ps4_fixture()[:0xbf]
        struct.pack_into("<Q", raw, 0, len(raw))
        with self.assertRaises(ValueError):
            catalog.parse_catalog(raw, ps4=True)

    def test_ps4_absent_links(self):
        raw = self.ps4_fixture()
        struct.pack_into("<q", raw, 0x88, -1)
        struct.pack_into("<q", raw, 0xa8, 0)
        for row in catalog.parse_catalog(raw, ps4=True)["records"]:
            self.assertIsNone(row["definition_offset"])

    def test_identity_selector_and_field_relative_definition(self):
        result = catalog.parse_catalog(fixture())
        a, b = result["records"]
        self.assertEqual(result["record_count"], 2)
        self.assertEqual((a["index"], a["name_hash"], a["payload_selector"]),
                         (0, "0xaeca3822", 17))
        self.assertEqual(a["definition_offset"], 0xC0)
        self.assertEqual(a["definition_class"], "0x808003c2")
        self.assertEqual(b["auxiliary_u16"], [0xFFFF, 17, 0xFFFF, 8])
        self.assertEqual(b["payload_selector"], 2)

    def test_absent_and_backwards_definition_links(self):
        raw = fixture()
        struct.pack_into(">I", raw, 0x54, 0xFFFFFFFF)
        struct.pack_into(">I", raw, 0x68, (0x20 - 0x68) & 0xFFFFFFFF)
        struct.pack_into(">I", raw, 0x24, 0x808008C3)
        a, b = catalog.parse_catalog(raw)["records"]
        self.assertIsNone(a["definition_class"])
        self.assertEqual(b["definition_offset"], 0x20)
        self.assertEqual(b["definition_class"], "0x808008c3")

    def test_corrupt_headers_classes_and_counts_are_rejected(self):
        for offset, value in ((0x3C, 0), (0x40, 3), (0x44, 0x80802468), (0x10, 65537)):
            raw = fixture()
            struct.pack_into(">I", raw, offset, value)
            with self.subTest(offset=offset), self.assertRaises(ValueError):
                catalog.parse_catalog(raw)

    def test_truncation_and_definition_overflow_are_rejected(self):
        with self.assertRaises(ValueError):
            catalog.parse_catalog(fixture()[:0x75])
        for relative in (0x1000, 0xFFFFFF00):
            raw = fixture()
            struct.pack_into(">I", raw, 0x54, relative)
            with self.subTest(relative=relative), self.assertRaises(ValueError):
                catalog.parse_catalog(raw)

    def test_existing_and_tracked_outputs_are_rejected(self):
        with self.assertRaises(ValueError):
            catalog.private_output(Path(__file__).parent / "private-report.json")
        with tempfile.TemporaryDirectory() as folder:
            destination = Path(folder) / "report.json"
            destination.write_text("preserve")
            with self.assertRaises(ValueError):
                catalog.private_output(destination)


if __name__ == "__main__":
    unittest.main()
