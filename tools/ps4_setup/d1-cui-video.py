# SPDX-License-Identifier: GPL-2.0-or-later
"""Prepare private native-video option definitions from your decoded 56578 settings tag.

This is not an installer. UI graph bindings, localized string injection, and setting-id hooks
are separate work; a successful command does not mean native menu entries work in the game.
"""
import argparse
import hashlib
import json
import struct
from pathlib import Path

from d1_cui_settings import array, video_candidate, video_graph_candidate, video_strings_candidate


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('input', type=Path)
    p.add_argument('--template-group', type=lambda v: int(v, 0), required=True,
                   help='authored hash of the decoded Helmet group, not a hash of its English label')
    p.add_argument('--out', type=Path, required=True, help='new private directory, outside the repository')
    placement = p.add_mutually_exclusive_group()
    placement.add_argument('--graph', type=Path, help='prepare a separate group in the native settings UI graph')
    placement.add_argument('--existing-group', type=lambda v: int(v, 0),
                           help='append rows to an existing authored settings group without changing the graph')
    p.add_argument('--group-title', help='display title for --existing-group; retain its authored identifier')
    p.add_argument('--string-index', type=Path, help='decoded settings string-index tag')
    p.add_argument('--string-index-tag', type=lambda v: int(v, 0), help='authored string-index tag id')
    p.add_argument('--english', type=Path, help='decoded English payload referenced by that index')
    a = p.parse_args()
    if a.group_title is not None and a.existing_group is None:
        p.error('--group-title requires --existing-group')
    repo = Path(__file__).resolve().parents[2]
    dest = a.out.resolve()
    if dest.is_relative_to(repo):
        p.error('outputs contain game data; choose a private directory outside this checkout')
    original = a.input.read_bytes()
    string_data = None
    if any((a.string_index, a.string_index_tag is not None, a.english)):
        if not all((a.string_index, a.string_index_tag is not None, a.english)):
            p.error('provide --string-index, --string-index-tag and --english together')
        si, en = a.string_index.read_bytes(), a.english.read_bytes()
        names = array(si, 8, 4)
        first_label = max(struct.unpack_from('<I', si, o)[0] for o in names) + 1
    else:
        first_label = None
    changed, manifest = video_candidate(original, a.template_group, first_label,
                                        existing_group=a.existing_group, group_title=a.group_title)
    if a.existing_group is not None:
        manifest['existing_group'] = True
    if first_label is not None:
        index_candidate, english_candidate = video_strings_candidate(si, en, manifest['strings'])
        string_data = (si, index_candidate, en, english_candidate)
        manifest['strings_index_tag'] = a.string_index_tag
        manifest['english_tag'] = struct.unpack_from('<I', si, 0x18)[0]
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
    if string_data is not None:
        for stem, data in zip(('string-index-original', 'string-index-candidate',
                               'english-original', 'english-candidate'), string_data):
            (dest / (stem + '.bin')).write_bytes(data)
    (dest / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf8')
    print('Prepared definitions only; not installed or native-UI validated.')


if __name__ == '__main__':
    main()
