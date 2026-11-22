# SPDX-License-Identifier: GPL-2.0-or-later
"""Rewrite local lab paths in tracked text files to placeholders.

check_pr.py's LAB_PATH finds them and LAB_PLACEHOLDERS names each one's replacement. Only the machine
root is replaced; the rest of the path stays as written, for example the d1 lab root followed by
\\tools\\x becomes <D1_ROOT>\\tools\\x. Use it for records (lab history, notes). Code should read its
locations from tools/ps4_setup/d1paths.py, D1Paths.ps1 or an environment variable instead, because a
placeholder in code breaks the tool.

    python .github/scripts/scrub_lab_paths.py [--write] PATHSPEC...

Without --write it only lists what it would change. Binary files (NUL in the first 8 KiB) are skipped, and
line endings and other bytes are kept as they are.
"""
import argparse
import importlib.util
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("check_pr", HERE / "check_pr.py")
check_pr = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(check_pr)


def scrub(text):
    """(new text, number of replacements)."""
    return check_pr.LAB_PATH.subn(check_pr.lab_placeholder, text)


def tracked(pathspecs):
    out = subprocess.run(["git", "ls-files", "-z", "--", *pathspecs], capture_output=True, check=True).stdout
    return [p for p in out.decode("utf-8").split("\0") if p]


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--write", action="store_true", help="rewrite the files (default: list only)")
    ap.add_argument("pathspec", nargs="+")
    a = ap.parse_args(argv)
    files = total = 0
    for path in tracked(a.pathspec):
        data = Path(path).read_bytes()
        if b"\0" in data[:8192]:
            continue
        # surrogateescape keeps any non-UTF-8 bytes exactly as they were.
        text = data.decode("utf-8", "surrogateescape")
        new, n = scrub(text)
        if not n:
            continue
        files += 1
        total += n
        print(f"{n:6d}  {path}")
        if a.write:
            Path(path).write_bytes(new.encode("utf-8", "surrogateescape"))
    print(f"{total} path(s) in {files} file(s){'' if a.write else ' (dry run; pass --write)'}")


if __name__ == "__main__":
    sys.exit(main())
