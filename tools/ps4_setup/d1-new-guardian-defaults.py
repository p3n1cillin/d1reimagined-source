# SPDX-License-Identifier: GPL-2.0-or-later
"""Export numeric creation defaults from owned 56578 content; no game payload is retained.

    d1-new-guardian-defaults.py --out DEFAULTS.json

Set D1_ORACLE, D1_EBOOT and D1_PKGDIR (or use the selected build registry).
The three entries are read by d1-oracle's existing package reader and client decoder.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
import d1paths
from d1_new_guardian import creation_defaults


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('--out', type=Path, required=True)
    args = ap.parse_args()
    oracle = os.environ.get('D1_ORACLE') or str(d1paths.workspace() / 'd1-server/target/release/d1-oracle.exe')
    env = dict(os.environ, D1_BUILD='later-56578')
    registry = Path(env.get('D1_BUILDS') or d1paths.workspace() / 'capture-rig/builds.json')
    env['D1_BUILDS'] = str(registry)
    selected = json.loads(registry.read_text(encoding='utf-8'))['builds']['later-56578']
    eboot = Path(env.get('D1_EBOOT') or registry.parent.parent / selected['eboot'])
    pinned = json.loads((Path(__file__).resolve().parents[2] / 'ps4/capture-rig/builds.json').read_text())
    expected = pinned['builds']['later-56578']['eboot_sha256']
    if hashlib.sha256(eboot.read_bytes()).hexdigest() != expected:
        ap.error('the executable is not the pinned 56578 build')
    env['D1_EBOOT'] = str(eboot)
    with tempfile.TemporaryDirectory() as tmp:
        def read(tag, expected):
            path = Path(tmp) / 'entry.bin'
            r = subprocess.run([oracle, 'oodle', 'entry', '--tag', hex(tag), '--out', str(path), '--hex', '0'],
                               env=env, capture_output=True, text=True, check=True)
            if not re.search(rf'class 0x{expected:08x}\b', r.stdout, re.I):
                raise ValueError(f'{tag:#x}: unexpected entry class')
            return path.read_bytes()
        document = creation_defaults(read)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(document, indent=2) + '\n', encoding='utf-8')
    print(f'{len(document["data"])} creation records; numeric facts only')


if __name__ == '__main__':
    main()
