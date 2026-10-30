"""The automatic pre-review report on a pull request: everything mechanical, so review can go to judgement.

Reads git objects and the PR body only; never runs pull request code. Writes markdown to stdout.

    python3 -I .github/scripts/pr_report.py --base BASE --head HEAD --body-file BODY.md
"""
import argparse
import re
import subprocess
import sys

SECTIONS = ["What", "Tested", "Seen in the game"]
# Code that speaks the game's protocols or reads its formats: a wrong byte here fails silently in game.
WIRE = re.compile(r"(^ps4/d1-server/crates/(d1-bap|shadnet-[a-z]+|d1-eboot)/|protocol|codec|/wire|schema)", re.I)
AREAS = [
    ("server", r"^ps4/d1-server/"), ("trace", r"^ps4/(d1-trace|d1-observe|capture-rig)/"),
    ("missions", r"^missions/"), ("tools", r"^(tools|scripts)/"), ("ps3", r"^(server|patches|data)/"),
    ("docs", r"(^docs/|^notes/|\.md$)"), ("ci", r"^\.github/"),
]
# A large hex literal in non-test code is often a hash, tag or address read from one trace.
MAGIC = re.compile(r"\b0x[0-9a-fA-F]{7,8}\b")


def git(*args):
    return subprocess.run(["git", "-c", "core.quotePath=false", *args], capture_output=True, text=True,
                          check=True).stdout


def is_test(path, added):
    return bool(re.search(r"(^|/)(tests?/|test_[^/]*\.py$|[^/]*_test\.py$)", path)) or "#[test]" in added


def section(body, name):
    """The text under '### name' (or '## name') with HTML comments removed, or None if the heading is missing."""
    m = re.search(rf"^#{{2,3}}\s*{re.escape(name)}\s*$(.*?)(?=^#{{2,3}}\s|\Z)", body, re.M | re.S | re.I)
    return None if m is None else re.sub(r"<!--.*?-->", "", m.group(1), flags=re.S).strip()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", required=True)
    ap.add_argument("--head", required=True)
    ap.add_argument("--body-file", required=True)
    a = ap.parse_args()
    sys.stdout.reconfigure(encoding="utf-8")
    body = open(a.body_file, encoding="utf-8").read() if a.body_file else ""
    base = git("merge-base", a.base, a.head).strip()
    stat = [l.split("\t") for l in git("diff", "--numstat", "--no-renames", base, a.head).splitlines() if l]
    patch = git("diff", "-U0", "--no-color", "--no-renames", base, a.head)
    added = {}
    current = None
    for line in patch.splitlines():
        if line.startswith("+++ "):
            current = line[6:] if line.startswith("+++ b/") else None
        elif current and line.startswith("+") and not line.startswith("+++"):
            added.setdefault(current, []).append(line[1:])
    files = [(p, int(i) if i != "-" else 0, int(d) if d != "-" else 0) for i, d, p in stat]
    rows = []

    # Template
    parts = []
    for name in SECTIONS:
        text = section(body, name)
        parts.append(f"{'✅' if text else '⚠️'} {name}{'' if text else (' missing' if text is None else ' empty')}")
    ai = re.search(r"AI-assisted:\s*(?:<!--.*?-->\s*)?(yes|no)\b", body, re.I | re.S)
    parts.append(f"AI-assisted: {ai.group(1).lower()}" if ai else "⚠️ AI-assisted not answered")
    unticked = len(re.findall(r"^\s*-\s\[ \]", body, re.M))
    if unticked:
        parts.append(f"⚠️ {unticked} unticked checklist item(s)")
    rows.append(("Template", " · ".join(parts)))

    # Issues
    closes = sorted(set(re.findall(r"\b(?:close[sd]?|fix(?:e[sd])?|resolve[sd]?)\s+#(\d+)", body, re.I)), key=int)
    related = sorted(set(re.findall(r"#(\d+)", body)) - set(closes), key=int)
    issues = (", ".join(f"closes #{n}" for n in closes) or "closes none") + (
        "; mentions " + ", ".join(f"#{n}" for n in related) if related else "")
    rows.append(("Issues", issues))

    # Areas and size
    counts = {}
    for p, _, _ in files:
        for name, pattern in AREAS:
            if re.search(pattern, p):
                counts[name] = counts.get(name, 0) + 1
                break
    plus, minus = sum(i for _, i, _ in files), sum(d for _, _, d in files)
    rows.append(("Scope", f"{len(files)} file(s), +{plus} −{minus}; " +
                 (", ".join(f"{k} ({v})" for k, v in sorted(counts.items())) or "other")))

    # Tests
    code = [p for p, _, _ in files if p.endswith((".rs", ".py")) and not is_test(p, "\n".join(added.get(p, [])))]
    tests = [p for p, _, _ in files if p.endswith((".rs", ".py")) and is_test(p, "\n".join(added.get(p, [])))]
    if code and not tests:
        rows.append(("Tests", "⚠️ source changed with no test changes: " + ", ".join(f"`{p}`" for p in code[:6])))
    elif tests:
        rows.append(("Tests", f"✅ {len(tests)} file(s) with test changes"))
    else:
        rows.append(("Tests", "no source changes"))

    # What review should look at hardest
    flags = []
    wire = [p for p, _, _ in files if WIRE.search(p)]
    if wire:
        flags.append("protocol or format code (check wire bytes against the client): " + ", ".join(f"`{p}`" for p in wire[:5]))
    # Rust keeps unit tests beside the code, so scan every file outside test paths, minus assertion lines.
    src = [p for p, _, _ in files if p.endswith((".rs", ".py")) and not is_test(p, "")]
    magic = sorted({m for p in src for line in added.get(p, []) if "assert" not in line for m in MAGIC.findall(line)})
    if magic:
        flags.append(f"{len(magic)} new large hex constant(s) outside tests, e.g. " + ", ".join(f"`{m}`" for m in magic[:5]) +
                     ": derived from content or the client, or copied from one trace?")
    deps = [p for p, _, _ in files if re.search(r"(Cargo\.(toml|lock)|requirements[^/]*\.txt|package(-lock)?\.json)$", p)]
    if deps:
        flags.append("dependencies change: " + ", ".join(f"`{p}`" for p in deps))
    if any(p.startswith(".github/") for p, _, _ in files):
        flags.append("repository automation (`.github/`) changes: merged by hand")
    rows.append(("Look closely", "<br>".join(flags) if flags else "nothing flagged"))

    print("<!-- pr-report -->")
    print("**Automatic pre-review report**, updated on every push and edit. A maintainer reviews after this; "
          "lint findings on added lines appear inline under *Files changed*.\n")
    print("| | |\n|---|---|")
    for k, v in rows:
        print(f"| {k} | {v} |")


if __name__ == "__main__":
    main()
