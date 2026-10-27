# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Build unbound squad-member activation states for PS4 build 56578.

    d1-member-states.py BASE_LIST OUT_LIST GROUP SUB:GEN [SUB:GEN ...]

Targets kind 2 / A 0x80800868 (2312 bytes). Use a fresh captured list for the
current world and retain its @authority header. Generation is the activation
counter; byte +4 is -1 and bool +6 is set, as in the tested diagnostic image.

The optional binding at +0x50 must be ABSENT, with index -1 at +0x58. Zero
names a real reference: 0x1BCEC3..0x1BCF34 caches it for 0x1BDCC1's unguarded
lookup and an all-zero binding crashed a native Divide test. This utility
does not set a flight path, performance, encounter order or objective.

Inputs and generated images stay private. A state readback or crash-free run
alone does not establish visible AI, correct placement or mission completion.
"""
import argparse
from pathlib import Path
import struct

TAG = '0x80800868'
SIZE = 2312


def member(generation):
    if not 0 <= generation <= 0xFFFFFFFF:
        raise ValueError('generation must fit u32')
    image = bytearray(SIZE)
    struct.pack_into('<I', image, 0, generation)
    image[4], image[6] = 0xFF, 1
    struct.pack_into('<IBxH', image, 0x50, 0x811C9DC5, 0xFF, 0xFFFF)
    struct.pack_into('<I', image, 0x58, 0xFFFFFFFF)
    return bytes(image)


def patched(text, group, states):
    """Replace selected A images; preserve authority, other clients and B images."""
    lines, hit = [], set()
    for line in text.splitlines(keepends=True):
        body, comment_mark, comment = line.rstrip('\r\n').partition('#')
        fields = body.split()
        try:
            name, kind, sub = (int(f, 0) for f in fields[:3])
        except ValueError:
            lines.append(line)
            continue
        if name != group or kind != 2 or sub not in states:
            lines.append(line)
            continue
        if sub in hit:
            raise ValueError(f'duplicate member {sub} in input list')
        if len(fields) not in (5, 7, 9) or int(fields[3], 0) != 1:
            raise ValueError(f'member {sub}: want a client with an A state')
        if len(fields) >= 7 and int(fields[5], 0) != int(TAG, 0):
            raise ValueError(f'member {sub}: existing A type is not {TAG}')
        replacement = ' '.join(fields[:5] + [TAG, member(states[sub]).hex()] + fields[7:])
        if comment_mark:
            replacement += ' #' + comment
        lines.append(replacement + '\n')
        hit.add(sub)
    if missing := set(states) - hit:
        raise ValueError(f'members not in input list: {sorted(missing)}')
    return ''.join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('base', type=Path)
    parser.add_argument('out', type=Path)
    parser.add_argument('group', type=lambda s: int(s, 0))
    parser.add_argument('specs', nargs='+', metavar='SUB:GEN')
    args = parser.parse_args()
    try:
        if not 0 <= args.group <= 0xFFFFFFFF:
            raise ValueError('group must fit u32')
        states = {}
        for spec in args.specs:
            sub, generation = (int(f, 0) for f in spec.split(':'))
            if not -32768 <= sub <= 32767 or sub in states:
                raise ValueError('member index must fit i16 and occur once')
            member(generation)  # validate every spec before writing
            states[sub] = generation
        result = patched(args.base.read_text(), args.group, states)
    except ValueError as error:
        parser.error(str(error))
    args.out.write_text(result, newline='\n')
    print(f'{args.out}: {len(states)} member activation state(s); optional binding absent')


if __name__ == '__main__':
    main()
