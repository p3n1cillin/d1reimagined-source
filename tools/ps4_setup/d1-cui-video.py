# SPDX-License-Identifier: GPL-2.0-or-later
"""Prepare private native-video option definitions from your decoded 56578 settings tag.

This is not an installer. UI graph bindings, localized string injection, and setting-id hooks
are separate work; a successful command does not mean native menu entries work in the game.
"""
import argparse
import hashlib
import json
from pathlib import Path

from d1_cui_settings import video_candidate


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('input', type=Path)
    p.add_argument('--template-group', type=lambda v: int(v, 0), required=True,
                   help='authored hash of the decoded Helmet group, not a hash of its English label')
    p.add_argument('--out', type=Path, required=True, help='new private directory, outside the repository')
    a = p.parse_args()
    repo = Path(__file__).resolve().parents[2]
    dest = a.out.resolve()
    if dest.is_relative_to(repo):
        p.error('outputs contain game data; choose a private directory outside this checkout')
    original = a.input.read_bytes()
    changed, manifest = video_candidate(original, a.template_group)
    manifest.update(source_sha256=hashlib.sha256(original).hexdigest(),
                    candidate_sha256=hashlib.sha256(changed).hexdigest())
    dest.mkdir(parents=True, exist_ok=False)
    (dest / 'settings-candidate.bin').write_bytes(changed)
    (dest / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf8')
    print('Prepared definitions only; not installed or native-UI validated.')


if __name__ == '__main__':
    main()
