"""The conventions check's patterns (.github/scripts/check_pr.py), without git."""
import importlib.util
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location("check_pr", Path(__file__).resolve().parents[1] / ".github/scripts/check_pr.py")
check_pr = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(check_pr)


class PersonalPaths(unittest.TestCase):
    def test_real_home_directories_are_flagged(self):
        # Built from pieces so this file doesn't itself trip the check.
        for text in ["\\".join(["C:", "Users", "alice", "d1"]), "/".join(["C:", "Users", "bob", "src"]),
                     "/".join(["", "home", "kael", "src"]), "see " + "/".join(["", "Users", "sam", "code", ""])]:
            self.assertTrue(check_pr.PERSONAL.search(text), text)

    def test_placeholders_and_shared_folders_pass(self):
        for text in [r"C:\Users\<you>\d1", r"C:\Users\%USERNAME%\x", "/home/<user>/d1", "$HOME/d1", "~/d1",
                     r"C:\Users\Public\x", "path/home/x/", "D1_ROOT=<D1_ROOT>"]:
            self.assertFalse(check_pr.PERSONAL.search(text), text)


class Links(unittest.TestCase):
    def links(self, text):
        return check_pr.LINK.findall(check_pr.CODE.sub("", text))

    def test_code_is_not_a_link(self):
        text = "Call `vtable[0x18](obj)` here.\n```c\nobj->v[0](obj);\n```\nSee [the status](docs/STATUS.md)."
        self.assertEqual(self.links(text), ["docs/STATUS.md"])

    def test_anchor_is_dropped(self):
        self.assertEqual(self.links("[x](notes/a.md#part)"), ["notes/a.md"])


class Titles(unittest.TestCase):
    def test_area_summary(self):
        for title in ["d1-bap: cover loot round trips", "56578: derive item stats", "ci(sweep): merge green PRs", "docs!: rename"]:
            self.assertTrue(check_pr.TITLE.match(title), title)
        for title in ["Update loot.rs", "fix", ": nothing", "host:no space"]:
            self.assertFalse(check_pr.TITLE.match(title), title)


if __name__ == "__main__":
    unittest.main()
