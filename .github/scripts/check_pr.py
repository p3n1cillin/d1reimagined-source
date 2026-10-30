"""Pull request conventions, run from the trusted base checkout against the PR's head commit.

Never imports or executes anything from the pull request: it only reads git objects.
Errors fail the check; notices are informational. Run it locally before pushing; it checks your branch
against origin/main, and the title too when you pass the one you'll use:

    python3 .github/scripts/check_pr.py [--title "area: summary"]
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
# A home directory with a real user name in it. Placeholders (<user>, %USERNAME%, $USER) are fine.
PERSONAL = re.compile(
    r"\b[A-Za-z]:[\\/]+Users[\\/]+(?![<%$]|Public\b|Default\b)[^\\/\s'\"`<>]+"
    r"|(?<![\w.~])/(?:home|Users)/(?![<$])[a-z][\w.-]*/",
    re.I,
)
LINK = re.compile(r"\]\(([^)\s#]+)(?:#[^)\s]*)?\)")
# Fenced blocks and inline code, where `a[i](x)` is code rather than a link.
CODE = re.compile(r"^(```|~~~).*?^\1|`[^`\n]*`", re.S | re.M)


def git(*args):
    return subprocess.run(["git", "-c", "core.quotePath=false", *args], capture_output=True, check=True).stdout.decode("utf-8", "replace")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", default="origin/main")
    ap.add_argument("--head", default="HEAD")
    ap.add_argument("--title", help="the pull request title; not checked when omitted")
    ap.add_argument("--author", default="")
    ap.add_argument("--owner", default="")
    a = ap.parse_args()
    errors, notices = [], []
    if a.title is not None and not a.author.endswith("[bot]") and not TITLE.match(a.title):
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
            personal = PERSONAL.search(line)
            if personal:
                errors.append((current, f"Personal path {personal.group(0)!r}: use a placeholder such as <home>, or an environment variable"))
    for status, path in changed:
        if status == "D" or not path.endswith(".md"):
            continue
        text = CODE.sub("", git("show", f"{a.head}:{path}"))
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
