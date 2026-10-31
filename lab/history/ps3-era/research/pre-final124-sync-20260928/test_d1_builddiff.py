import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import d1_builddiff  # noqa: E402


def field(off, kind, nested=0xFFFFFFFF, width=32, opt=0):
    return {"off": off, "bit": 0, "kind": kind, "opt": opt, "nested": nested, "arr": 0, "min": 0,
            "width": width, "extra": 0}


class MatchTypes(unittest.TestCase):
    def test_renumbered_and_changed_types(self):
        old = {"0x80801000": {"size": 8, "fields": [field(0, 9), field(4, 9)]},
               "0x80801001": {"size": 16, "fields": [field(0, 1, 0x80801000), field(8, 6, width=64)]},
               "0x80801002": {"size": 12, "fields": [field(0, 9), field(4, 9), field(8, 5)]},
               "0x80801003": {"size": 4, "fields": [field(0, 2, width=0)]}}
        new = {"0x80802000": {"size": 8, "fields": [field(0, 9), field(4, 9)]},          # renumbered
               "0x80802001": {"size": 16, "fields": [field(0, 1, 0x80802000), field(8, 6, width=64)]},
               "0x80802002": {"size": 12, "fields": [field(0, 9), field(4, 9), field(8, 5, opt=1)]},  # changed
               "0x80802009": {"size": 64, "fields": [field(0, 11)]}}                    # added
        mapping, how, removed, added = d1_builddiff.match_types(old, new)
        self.assertEqual(mapping["0x80801000"], "0x80802000")
        self.assertEqual(mapping["0x80801001"], "0x80802001")
        self.assertEqual((mapping["0x80801002"], how["0x80801002"]), ("0x80802002", "changed"))
        self.assertEqual(removed, ["0x80801003"])
        self.assertEqual(added, ["0x80802009"])
        text = d1_builddiff.report(old, new, mapping, how, removed, added, names=("a", "b"))
        self.assertIn("2 structurally identical, 1 changed, 1 removed, 1 added", text)


if __name__ == "__main__":
    unittest.main()
