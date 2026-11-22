# SPDX-License-Identifier: GPL-2.0-or-later
"""No tracked text file names a maintainer's lab folder (check_pr.py's LAB_PATH).

check_pr.py checks the lines a pull request adds; this test checks the whole tree, so an old path can't
survive a rename or a bulk import. Fix a failure with `.github/scripts/scrub_lab_paths.py --write PATH`
for records, or by reading the location from tools/ps4_setup/d1paths.py (or an environment variable) in code.
"""
import importlib.util
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("check_pr", ROOT / ".github/scripts/check_pr.py")
check_pr = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(check_pr)


class NoLabPaths(unittest.TestCase):
    def test_tracked_text_files_have_none(self):
        out = subprocess.run(["git", "ls-files", "-z"], cwd=ROOT, capture_output=True, check=True).stdout
        found = []
        for rel in filter(None, out.decode("utf-8").split("\0")):
            try:
                data = (ROOT / rel).read_bytes()
            except OSError:   # listed but absent (sparse checkout, deleted in the work tree)
                continue
            if b"\0" in data[:8192]:
                continue
            m = check_pr.LAB_PATH.search(data.decode("utf-8", "replace"))
            if m:
                found.append(f"{rel}: {m.group(0)!r} -> {check_pr.lab_placeholder(m)}")
        self.assertEqual(found, [], f"{len(found)} file(s) with a local lab path:\n" + "\n".join(found[:40]))


if __name__ == "__main__":
    unittest.main()
