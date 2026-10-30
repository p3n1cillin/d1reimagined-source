"""Turn linter output into GitHub annotations, keeping only findings on lines the change adds.

Existing code has warnings of its own; a pull request should only hear about the ones it introduces.
Reads `path:line[:col]: message` lines (ruff concise, clippy --message-format=short) or rustfmt's
`Diff in <path> at line N:` from stdin, and prints ::warning annotations. Never fails the job.

    ruff check --output-format=concise FILES | python3 new_findings.py --base origin/main --tool ruff
    cargo clippy --message-format=short ... 2>&1 | python3 new_findings.py --base ... --tool clippy --prefix ps4/d1-server
"""
import argparse
import os
import re
import subprocess
import sys

LOCATED = re.compile(r"^(?P<path>[^\s:][^:]*?):(?P<line>\d+)(?::\d+)?:\s*(?P<msg>.+)$")
RUSTFMT = re.compile(r"^Diff in (?P<path>.+?) at line (?P<line>\d+):")


def added_lines(base, head):
    """{path: set(line numbers added or changed)} from the diff against the merge base."""
    out = subprocess.run(["git", "diff", "-U0", "--no-color", f"{base}...{head}"], capture_output=True, text=True,
                         check=True).stdout
    lines, path = {}, None
    for row in out.splitlines():
        if row.startswith("+++ "):
            path = row[6:] if row.startswith("+++ b/") else None
        elif path and row.startswith("@@"):
            m = re.search(r"\+(\d+)(?:,(\d+))?", row)
            start, count = int(m.group(1)), int(m.group(2) or 1)
            lines.setdefault(path, set()).update(range(start, start + count))
    return lines


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", default="origin/main")
    ap.add_argument("--head", default="HEAD")
    ap.add_argument("--tool", required=True)
    ap.add_argument("--prefix", default="", help="directory the tool's paths are relative to")
    a = ap.parse_args()
    added = added_lines(a.base, a.head)
    root = os.getcwd()
    seen, kept = set(), 0
    for raw in sys.stdin:
        raw = raw.rstrip()
        m = RUSTFMT.match(raw) or LOCATED.match(raw)
        if not m:
            continue
        path = m.group("path")
        # Absolute paths (rustfmt) are relative to the checkout; relative ones (clippy) to --prefix.
        path = os.path.relpath(path, root) if os.path.isabs(path) else os.path.join(a.prefix, path)
        path = os.path.normpath(path).replace(os.sep, "/")
        line = int(m.group("line"))
        msg = m.groupdict().get("msg") or "rustfmt would reformat this"
        if line not in added.get(path, ()) or (path, line, msg) in seen:
            continue
        seen.add((path, line, msg))
        kept += 1
        print(f"::warning file={path},line={line},title={a.tool}::{msg}")
    print(f"{a.tool}: {kept} finding(s) on added lines")


if __name__ == "__main__":
    main()
