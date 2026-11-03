# SPDX-License-Identifier: GPL-2.0-or-later
"""Prepare private native-video option definitions from your decoded 56578 settings tag.

This is not an installer. UI graph bindings, localized string injection, and setting-id hooks
are separate work; a successful command does not mean native menu entries work in the game.
"""
import argparse
import hashlib
import json
from pathlib import Path

from d1_cui_settings import video_candidate, video_graph_candidate


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('input', type=Path)
    p.add_argument('--template-group', type=lambda v: int(v, 0), required=True,
                   help='authored hash of the decoded Helmet group, not a hash of its English label')
    p.add_argument('--out', type=Path, required=True, help='new private directory, outside the repository')
    p.add_argument('--graph', type=Path, help='also prepare the decoded native settings UI graph')
    a = p.parse_args()
    repo = Path(__file__).resolve().parents[2]
    dest = a.out.resolve()
    if dest.is_relative_to(repo):
        p.error('outputs contain game data; choose a private directory outside this checkout')
    original = a.input.read_bytes()
    changed, manifest = video_candidate(original, a.template_group)
    manifest.update(source_sha256=hashlib.sha256(original).hexdigest(),
                    candidate_sha256=hashlib.sha256(changed).hexdigest())
    graph = None
    if a.graph:
        graph_source = a.graph.read_bytes()
        graph, graph_info = video_graph_candidate(graph_source, a.template_group,
                                                  int(manifest['group_hash'], 16))
        graph_info.update(source_sha256=hashlib.sha256(graph_source).hexdigest(),
                          candidate_sha256=hashlib.sha256(graph).hexdigest())
        manifest['graph'] = graph_info
    dest.mkdir(parents=True, exist_ok=False)
    (dest / 'settings-original.bin').write_bytes(original)
    (dest / 'settings-candidate.bin').write_bytes(changed)
    if graph is not None:
        (dest / 'graph-original.bin').write_bytes(graph_source)
        (dest / 'graph-candidate.bin').write_bytes(graph)
    (dest / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf8')
    print('Prepared definitions only; not installed or native-UI validated.')


if __name__ == '__main__':
    main()
