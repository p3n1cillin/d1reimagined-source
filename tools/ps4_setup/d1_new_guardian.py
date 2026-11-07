# SPDX-License-Identifier: GPL-2.0-or-later
"""Build 56578's authored creation flags, using the existing tag-array reader."""
import hashlib
import struct

from d1_cui_settings import array

BUILD = '56578.17.04.12.1251.evil'


def block(data, field, stride, element):
    rows = array(data, field, stride)
    if rows and struct.unpack_from('<I', data, rows[0] - 8)[0] != element:
        raise ValueError(f'wrong element class at {field:#x}')
    return rows


def creation_defaults(read_entry):
    """read_entry(tag, expected_class) uses the existing oracle, never a second package codec.

    Offsets/strides below belong to 56578: root +E8, +538; creation records 176 B,
    flag list +30. Their array classes and all nine (class, race) keys are checked.
    """
    sources = {}

    def read(tag, expected):
        data = read_entry(tag, expected)
        if len(data) < 8 or struct.unpack_from('<Q', data)[0] != len(data):
            raise ValueError('tag self-length disagrees')
        sources[tag] = hashlib.sha256(data).hexdigest()
        return data

    root = read(0x80A58001, 0x80801750)
    defaults = read(struct.unpack_from('<I', root, 0xE8)[0], 0x8080190D)
    names = read(struct.unpack_from('<I', root, 0x538)[0], 0x8080179C)
    flags = [struct.unpack_from('<IBxH', names, at)
             for at in block(names, 8, 8, 0x808018B8)]
    rows, keys = [], set()
    for at in block(defaults, 8, 176, 0x8080170F):
        key = tuple(defaults[at:at + 2])
        if key in keys or any(k > 2 for k in key):
            raise ValueError('duplicate or invalid creation class/race')
        keys.add(key)
        grants, seen = [], set()
        for p in block(defaults, at + 0x30, 4, 0x8080185C):
            index, value = struct.unpack_from('<hB', defaults, p)
            if not 0 <= index < len(flags) or index in seen or value not in (0, 1, 2):
                raise ValueError('invalid creation flag')
            seen.add(index)
            name, kind, sub = flags[index]
            if kind not in (1, 2, 3) or sub == 0xFFFF:
                raise ValueError('creation flag has no known dense source')
            grants.append(dict(index=index, hash=name, kind=kind, sub=sub, value=value))
        rows.append(dict(character_class=key[0], race=key[1], flags=grants))
    if keys != {(c, r) for c in range(3) for r in range(3)}:
        raise ValueError('creation defaults do not cover all classes and races')
    return dict(build=BUILD, generator='d1-new-guardian-defaults.py',
                sources=[dict(tag=t, sha256=h) for t, h in sorted(sources.items())], data=rows)


def select_defaults(document, character_class, race):
    if document.get('build') != BUILD:
        raise ValueError('creation defaults must be from build 56578')
    rows = [r for r in document['data']
            if (r['character_class'], r['race']) == (character_class, race)]
    if len(rows) != 1:
        raise ValueError('expected one creation class/race record')
    return rows[0]['flags']
