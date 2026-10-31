# SPDX-License-Identifier: GPL-2.0-or-later
"""Import the project's private working folders into the repository, minus what must stay out.

Everything used to get the project this far belongs in the repository, except:
  - game files and content extracted from them (package entries saved by tag or as "owned" items,
    executables, firmware);
  - videos and screenshots (run evidence is summarised in the notes and daily reports instead);
  - build outputs and caches (they are rebuilt from the sources that are imported);
  - keys and credentials;
  - personal data (checked by a scan; a hit stops the import of that file).

    import_lab_history.py SOURCE DEST [--dry-run]

SOURCE is a folder of dated working folders; each lands at DEST/<name> with the same layout.
"""
import argparse
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
# Replaced in text files on import: local network addresses, the Windows profile folder, the lab
# machines' names, the old commit email, and the private folder these files came from (which is
# lab/history in the repository).
SEP = rb'(?:\\\\|\\|/)+'
SCRUB = [(re.compile(rb'192\.168\.\d+\.\d+'), b'<lan>'),
         (re.compile(rb'C:' + SEP + rb'Users' + SEP + rb'lukem', re.I), rb'%USERPROFILE%'),
         (re.compile(rb'(?:[A-Za-z]:|/[a-z])' + SEP + rb'd1' + SEP + rb'lukespc-import' + SEP + rb'd1xenia' + SEP +
                     rb'local' + SEP + rb'PRIVATE_ENGINEERING', re.I), b'lab/history'),
         (re.compile(rb'lcmcclellan12@gmail\.com', re.I), b'<old-email>'),
         (re.compile(rb'lukespc', re.I), b'<pc>'),
         (re.compile(rb'matildaspc', re.I), b'<lab-pc>'),
         (re.compile(rb'/home' + rb'/runner/'), b'<home>/')]   # CI runner paths in old logs
# A Markdown link (or image) and its target, as the repository's link check reads them.
MD_LINK = re.compile(r'(!?)\[([^\]]*)\]\(([^)\s#]+)((?:#[^)\s]*)?)\)')
TEXT = SOURCE | {'.txt', '.json', '.jsonl', '.log', '.tsv', '.csv', '.xml', '.yml', '.yaml', '.ini', '.patch'}
PERSONAL = re.compile(rb'(lcmcclellan|lukespc|matildaspc|C:[\\/]+Users[\\/]+lukem|192\.168\.\d+\.\d+)', re.I)
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
    if PERSONAL.search(data):
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
    a = ap.parse_args()
    counts, sizes, held = {}, {}, []
    for top, dirs, files in os.walk(a.source):
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
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
