"""The pre-review report's parsing (.github/scripts/pr_report.py), without git."""
import importlib.util
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location("pr_report", Path(__file__).resolve().parents[1] / ".github/scripts/pr_report.py")
pr_report = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(pr_report)

BODY = """<!-- Title: `area: summary` -->

### What
<!-- what changed -->
Adds round-trip tests.

### Tested
<!-- commands -->

### Checklist
- [x] Based on current `main`
- [ ] AI-assisted: <!-- yes / no --> no
"""


class Sections(unittest.TestCase):
    def test_filled_empty_and_missing(self):
        self.assertEqual(pr_report.section(BODY, "What"), "Adds round-trip tests.")
        self.assertEqual(pr_report.section(BODY, "Tested"), "")          # only the template's comment
        self.assertIsNone(pr_report.section(BODY, "Seen in the game"))  # heading removed


class Heuristics(unittest.TestCase):
    def test_test_files(self):
        self.assertTrue(pr_report.is_test("ps4/d1-server/crates/d1-live/tests/readside.rs", ""))
        self.assertTrue(pr_report.is_test("server/test_ah_session.py", ""))
        self.assertTrue(pr_report.is_test("ps4/d1-server/crates/d1-bap/src/loot.rs", "    #[test]\n    fn x() {}"))
        self.assertFalse(pr_report.is_test("ps4/d1-server/crates/d1-bap/src/loot.rs", "pub fn grant() {}"))

    def test_magic_constants_are_large_hex_only(self):
        self.assertEqual(pr_report.MAGIC.findall("let tag = 0x80a5ffcf; let off = 0x578; let a = 0x1017530;"),
                         ["0x80a5ffcf", "0x1017530"])

    def test_wire_paths(self):
        self.assertTrue(pr_report.WIRE.search("ps4/d1-server/crates/d1-bap/src/matchmaking.rs"))
        self.assertTrue(pr_report.WIRE.search("ps4/d1-server/crates/d1-activity-host/src/protocol.rs"))
        self.assertFalse(pr_report.WIRE.search("tools/ps4_setup/d1-record.py"))


if __name__ == "__main__":
    unittest.main()
