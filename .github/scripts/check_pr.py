"""Pull request conventions, run from the trusted base checkout against the PR's head commit.

Never imports or executes anything from the pull request: it only reads git objects.
Errors fail the check; notices are informational.

    python3 -I .github/scripts/check_pr.py --base BASE_SHA --head HEAD_SHA --title TITLE [--author LOGIN --owner LOGIN]
"""
import argparse
import posixpath
import re
import subprocess
import sys
from urllib.parse import unquote

# "area: summary" (this repository's style) or "type(scope): summary".
TITLE = re.compile(r"^[A-Za-z0-9][\w ./+-]*(\([^)]+\))?!?: \S")
SECRETS = [
    (re.compile(r"-----BEGIN (?:RSA |EC |OPENSSH |DSA |PGP )?PRIVATE KEY-----"), "a private key"),
    (re.compile(r"\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{36,}\b|\bgithub_pat_[A-Za-z0-9_]{60,}\b"), "a GitHub token"),
    (re.compile(r"\bAKIA[0-9A-Z]{16}\b"), "an AWS access key"),
    (re.compile(r"\bxox[abprs]-[A-Za-z0-9-]{10,}\b"), "a Slack token"),
    (re.compile(r"\bsk-(?:ant-)?[A-Za-z0-9_-]{32,}\b"), "an API secret key"),
]
LINK = re.compile(r"\]\(([^)\s#]+)(?:#[^)\s]*)?\)")


def git(*args):
    return subprocess.run(["git", "-c", "core.quotePath=false", *args], capture_output=True, check=True).stdout.decode("utf-8", "replace")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", required=True)
    ap.add_argument("--head", required=True)
    ap.add_argument("--title", required=True)
    ap.add_argument("--author", default="")
    ap.add_argument("--owner", default="")
    a = ap.parse_args()
    errors, notices = [], []
    if not a.author.endswith("[bot]") and not TITLE.match(a.title):
        errors.append(("", f"Title {a.title!r} should read 'area: summary', e.g. 'host: serve the Tower's sensor list'"))
    base = git("merge-base", a.base, a.head).strip()
    changed = [l.split("\t", 1) for l in git("diff", "--name-status", "--no-renames", base, a.head).splitlines() if l]
    if a.author and a.author != a.owner and any(p.startswith(".github/") for _, p in changed):
        notices.append((".github", "Changes workflows or repository automation: a maintainer merges these by hand"))
    current = None
    for line in git("diff", "-U0", "--no-color", base, a.head).splitlines():
        if line.startswith("+++ "):
            current = line[6:] if line.startswith("+++ b/") else None
        elif current and line.startswith("+") and not line.startswith("+++"):
            for pattern, what in SECRETS:
                if pattern.search(line):
                    errors.append((current, f"Looks like {what}. Remove it, revoke it, and never commit credentials"))
    for status, path in changed:
        if status == "D" or not path.endswith(".md"):
            continue
        text = git("show", f"{a.head}:{path}")
        for target in LINK.findall(text):
            if re.match(r"^[a-z][a-z0-9+.-]*:", target) or target.startswith("/"):
                continue
            resolved = posixpath.normpath(posixpath.join(posixpath.dirname(path), unquote(target)))
            if subprocess.run(["git", "cat-file", "-e", f"{a.head}:{resolved}"], capture_output=True).returncode:
                errors.append((path, f"Relative link to {target}, which does not exist"))
    for path, msg in notices:
        print(f"::notice file={path}::{msg}")
    for path, msg in errors:
        print(f"::error file={path}::{msg}" if path else f"::error::{msg}")
    print(f"{len(errors)} error(s), {len(notices)} notice(s); {len(changed)} file(s) checked")
    sys.exit(1 if errors else 0)


if __name__ == "__main__":
    main()
