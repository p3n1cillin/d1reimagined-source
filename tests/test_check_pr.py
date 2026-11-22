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


class LabPaths(unittest.TestCase):
    # Built from pieces so this file doesn't itself trip the check (tests/test_no_lab_paths.py).
    B = "\\"

    def sub(self, text):
        return check_pr.LAB_PATH.sub(check_pr.lab_placeholder, text)

    def test_lab_roots_in_every_spelling_get_their_placeholder(self):
        B, d = self.B, "d" + "1"
        cases = {
            "F:" + B + d + B + "tools": "<D1_ROOT>" + B + "tools",
            "F:" + B * 2 + d + B * 2 + "x": "<D1_ROOT>" + B * 2 + "x",
            "see f:/" + d + "/public-work": "see <D1_ROOT>/public-work",
            "/mnt/f/" + d + "/repo": "<D1_ROOT>/repo",
            "cd /f/" + d + "/x": "cd <D1_ROOT>/x",
            "E:" + B + d + "-lab" + B + "runs": "<LAB_ROOT>" + B + "runs",
            "E:" + B + d + "-game" + B + "x": "<GAME_ROOT>" + B + "x",
            "D:" + B + "Destiny" + "360" + B + "x": "<PS3_ROOT>" + B + "x",
            "C:/" + "dev" + "/rpcs3": "<DEV_ROOT>/rpcs3",
            "F:" + B + "VS" + B + "BuildTools (": "<VS_BUILDTOOLS> (",
            "Y:" + B + "incoming" + B + "x": "<NAS_INCOMING>" + B + "x",
            # After a switch, a URI scheme, or a word whose space was lost.
            "cl a.cpp /Fe:F:" + B + d + B + "a.exe": "cl a.cpp /Fe:<D1_ROOT>" + B + "a.exe",
            "sqlite3.connect('file:F:/" + d + "/m.db')": "sqlite3.connect('file:<D1_ROOT>/m.db')",
            "UpdatedF:" + B + d + B + "x": "Updated<D1_ROOT>" + B + "x",
            "/mnt/e/" + d + "-lab/runs": "<LAB_ROOT>/runs",
        }
        for text, want in cases.items():
            self.assertEqual(self.sub(text), want, text)

    def test_other_paths_pass(self):
        B = self.B
        for text in ["https://d1reimagined.com/a/d1", "C:" + B + "Program Files" + B + "Git", "c:" + B + "bungie" + B + "x",
                     "F:/store", "ps4/a/d1/x", "D:/Games/D1", "<D1_ROOT>/tools", "F:" + B + "d1xenia", "/f/d1x"]:
            self.assertFalse(check_pr.LAB_PATH.search(text), text)


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
