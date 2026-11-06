# SPDX-License-Identifier: GPL-2.0-or-later
"""Regenerate the 56578 identifier/number tables using the existing Rust oracle.

    python -I tools/ps4_setup/d1-content-tables.py --oracle <d1-oracle>

The owned inputs resolve from D1_EBOOT / D1_PKGDIR, otherwise from the selected build's
relative locations under d1paths.workspace(). No emulator is launched. Only the derived
identifier/number tables are written to --out (default: this repository's 56578 fixtures).
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import d1paths


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('--oracle', required=True, type=Path)
    ap.add_argument('--out', type=Path, default=d1paths.fixtures())
    args = ap.parse_args()
    repo = Path(__file__).resolve().parents[2]
    builds = json.loads((repo / 'ps4/capture-rig/builds.json').read_text(encoding='utf-8'))
    build = builds['builds']['later-56578']
    env = os.environ.copy()
    env['D1_BUILD'] = 'later-56578'
    for key, field in [('D1_EBOOT', 'eboot'), ('D1_PKGDIR', 'packages')]:
        if not env.get(key):
            env[key] = str(d1paths.workspace() / build[field])
    return subprocess.call([str(args.oracle.resolve()), 'content-tables', '--out', str(args.out)], env=env)


if __name__ == '__main__':
    sys.exit(main())
