# SPDX-License-Identifier: GPL-2.0-or-later
"""Import the project's private working folders into the repository, minus what must stay out.

Everything used to get the project this far belongs in the repository, except:
  - game files and content extracted from them (package entries saved by tag or as "owned" items,
    executables, firmware);
  - videos and screenshots (run evidence is summarised in the notes and daily reports instead);
  - build outputs and caches (they are rebuilt from the sources that are imported);
  - keys and credentials;
  - personal data (checked by a scan; a hit stops the import of that file).

    import_lab_history.py SOURCE DEST [--dry-run] [--exclude FOLDER ...]

Personal scrub patterns come from tools/scrub-patterns.local.txt (untracked; D1_SCRUB_PATTERNS
overrides). An import without them is refused; scrub-patterns.example.txt shows the format.

SOURCE is a folder of dated working folders; each lands at DEST/<name> with the same layout.
"""
import argparse
import fnmatch
import os
import re
import shutil
import sys

MEDIA = {'.png', '.jpg', '.jpeg', '.gif', '.bmp', '.mp4', '.mkv', '.avi', '.mov', '.webm', '.wav', '.f32'}
BUILD = {'.exe', '.dll', '.pdb', '.lib', '.rlib', '.rmeta', '.d', '.o', '.pyc', '.timestamp', '.ilk', '.exp'}
GAME = {'.elf', '.self', '.sprx', '.prx', '.pkg', '.pup', '.sfo', '.edat', '.sdat', '.rap', '.rif', '.iso'}
SECRET = {'.key', '.pem', '.p12', '.pfx', '.crt'}
SKIP_DIRS = {'__pycache__', 'target', '.git', 'node_modules', 'CMakeFiles'}
# Package entries saved by content tag (e.g. 80cb8027.bin) or as owned items: extracted game content.
GAME_NAME = re.compile(r'(^owned|owned-|^[0-9a-f]{8}(\.|-)|-definition|\.pkgentry|^entry-|^blob-)', re.I)
GAME_DIR = re.compile(r'(^|[\\/])(owned|[a-z-]*-owned|dumps?|packages|entries)([\\/]|$)', re.I)
# Our own code and notes are kept even when they read game content; the content itself is not.
SOURCE = {'.py', '.rs', '.sh', '.ps1', '.cmd', '.bat', '.c', '.h', '.cpp', '.md', '.toml'}
# Replaced in text files on import. The generic rules live here: local network addresses and CI
# runner paths. The personal ones (your user folder, machine names, old e-mail addresses, the private
# folder the files came from) are read from an untracked file, scrub-patterns.local.txt beside this
# script or D1_SCRUB_PATTERNS, so they never enter the repository. See scrub-patterns.example.txt.
GENERIC_SCRUB = [(re.compile(rb'192\.168\.\d+\.\d+'), b'<lan>'),
                 (re.compile(rb'/home' + rb'/runner/'), b'<home>/')]   # CI runner paths in old logs
GENERIC_PERSONAL = [re.compile(rb'192\.168\.\d+\.\d+')]
LOCAL_PATTERNS = os.environ.get('D1_SCRUB_PATTERNS') or os.path.join(
    os.path.dirname(os.path.abspath(__file__)), 'scrub-patterns.local.txt')
# A Markdown link (or image) and its target, as the repository's link check reads them.
MD_LINK = re.compile(r'(!?)\[([^\]]*)\]\(([^)\s#]+)((?:#[^)\s]*)?)\)')
TEXT = SOURCE | {'.txt', '.json', '.jsonl', '.log', '.tsv', '.csv', '.xml', '.yml', '.yaml', '.ini', '.patch'}


def load_local_patterns(path):
    """`REGEX<TAB>REPLACEMENT` per line (Python syntax, case-insensitive); # starts a comment. Every
    pattern is also a personal-data check: a file that still matches after scrubbing is held."""
    rules = []
    with open(path, encoding='utf-8') as f:
        for n, line in enumerate(f, 1):
            line = line.rstrip('\r\n')
            if not line.strip() or line.lstrip().startswith('#'):
                continue
            if '\t' not in line:
                sys.exit(f'{path}:{n}: want REGEX<TAB>REPLACEMENT')
            pattern, replacement = line.split('\t', 1)
            rules.append((re.compile(pattern.encode(), re.I), replacement.encode()))
    return rules


SCRUB = list(GENERIC_SCRUB)
PERSONAL = list(GENERIC_PERSONAL)
MAX_BYTES = 5 * 1024 * 1024
# Binaries are kept only when they are the project's own encodings (served fixtures, decoded objects,
# wire messages, drafts). Everything else binary is a package entry or a game-memory snapshot.
OWN_BINARY = re.compile(
    r'^(f\dt\d-[0-9a-f]+\.bin|\d\d-[A-Za-z ]+\.bin|(profile|prof|account|acct|arcadia|emblem|grant|roster|'
    r'scoreboard|khvostov-\d+|instance-ci\d+|peer-|sub-|check|cmp-|sense\d+-|vendor-)[^/\\]*\.(bin|obj)|'
    r'[^/\\]*\.(decoded|wire|draft)\.bin|[^/\\]*-udt\d+\.bin|[^/\\]*\.obj)$', re.I)
NOT_OWN_BINARY = re.compile(r'(-def[-.]|def-|-component\.|original-state|definition)', re.I)
# Text over this size is a memory or executable extract (UI trees, reflection, descriptor dumps).
MAX_DATA_BYTES = 1024 * 1024


def verdict(path, rel):
    name = os.path.basename(path)
    ext = os.path.splitext(name)[1].lower()
    if ext in MEDIA:
        return 'media'
    if ext in BUILD:
        return 'build'
    if ext in GAME:
        return 'game'
    if ext in SECRET:
        return 'secret'
    if ext not in SOURCE and (GAME_NAME.search(name) or GAME_DIR.search(os.path.dirname(rel))):
        return 'game'
    if ext not in TEXT and (not OWN_BINARY.match(name) or NOT_OWN_BINARY.search(name)):
        return 'game'
    size = os.path.getsize(path)
    if size > MAX_BYTES or (ext in TEXT and ext not in SOURCE and size > MAX_DATA_BYTES):
        return 'large'
    with open(path, 'rb') as f:
        data = f.read()
    if ext in TEXT:
        data = scrub(data)
    if any(p.search(data) for p in PERSONAL):
        return 'personal'
    return 'keep'


def scrub(data):
    for pattern, replacement in SCRUB:
        data = pattern.sub(replacement, data)
    return data


def unlink_missing(dest):
    """Links to files that are not in the repository (the authors' own folders, or files left out)
    become plain text with the old target in code, so the record keeps it and the link check passes."""
    changed = 0
    for top, _, files in os.walk(dest):
        for name in files:
            if not name.lower().endswith('.md'):
                continue
            path = os.path.join(top, name)
            text = open(path, encoding='utf-8', errors='replace').read()

            def fix(m):
                image, label, target = m.group(1), m.group(2), m.group(3)
                if re.match(r'^[a-z][a-z0-9+.-]*:(?![\\/])', target, re.I) and not re.match(r'^[a-z]:', target, re.I):
                    return m.group(0)   # a URL (https:, mailto:, ...)
                if not re.match(r'^([a-z]:|[\\/])', target, re.I) and os.path.exists(os.path.join(top, target)):
                    return m.group(0)   # a link inside the repository that resolves
                kept = 'image not kept' if image else 'not in the repository'
                return f'{label} ({kept}: `{target}`)'

            new = MD_LINK.sub(fix, text)
            if new != text:
                with open(path, 'w', encoding='utf-8', newline='') as f:
                    f.write(new)
                changed += 1
    return changed


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('source')
    ap.add_argument('dest')
    ap.add_argument('--dry-run', action='store_true')
    # Top-level folders of SOURCE to leave out (shell-style patterns), e.g. copies of other projects'
    # source trees, or metadata extracted from game updates.
    ap.add_argument('--exclude', action='append', default=[])
    a = ap.parse_args()
    if os.path.exists(LOCAL_PATTERNS):
        local = load_local_patterns(LOCAL_PATTERNS)
        SCRUB.extend(local)
        PERSONAL.extend(p for p, _ in local)
    elif not a.dry_run:
        sys.exit(f'no personal scrub patterns at {LOCAL_PATTERNS}: copy scrub-patterns.example.txt there and '
                 'fill in your own details first (an import without them could publish personal data)')
    counts, sizes, held = {}, {}, []
    for top, dirs, files in os.walk(a.source):
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
        if os.path.normpath(top) == os.path.normpath(a.source):
            dirs[:] = [d for d in dirs if not any(fnmatch.fnmatch(d, p) for p in a.exclude)]
        for name in files:
            path = os.path.join(top, name)
            rel = os.path.relpath(path, a.source)
            v = verdict(path, rel)
            counts[v] = counts.get(v, 0) + 1
            sizes[v] = sizes.get(v, 0) + os.path.getsize(path)
            if v in ('personal', 'large'):
                held.append((v, rel))
            if v == 'keep' and not a.dry_run:
                out = os.path.join(a.dest, rel)
                os.makedirs(os.path.dirname(out), exist_ok=True)
                if os.path.splitext(name)[1].lower() in TEXT:
                    with open(path, 'rb') as src, open(out, 'wb') as dst:
                        dst.write(scrub(src.read()))
                else:
                    shutil.copy2(path, out)
    if not a.dry_run and os.path.isdir(a.dest):
        print(f'unlinked missing targets in {unlink_missing(a.dest)} Markdown files')
    for v in sorted(counts):
        print(f'{v:9} {counts[v]:6} files {sizes[v] / 1e6:9.1f} MB')
    for v, rel in held:
        print(f'held ({v}): {rel}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
