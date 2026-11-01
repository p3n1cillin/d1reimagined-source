# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Build unbound squad-member activation states for PS4 build 56578.

    d1-member-states.py BASE_LIST OUT_LIST GROUP SUB:GEN[:ACTION] [SUB:GEN[:ACTION] ...]

Targets kind 2 / A 0x80800868 (2312 bytes). Use a fresh captured list for the
current world and retain its @authority header. Generation is the activation
counter; byte +4 is -1 and bool +6 is set, as in the tested diagnostic image.

ACTION gives the member the command script a mission script's
`enter(member, action, audio)` sends: `__cs.scripted_sequence(action)` then
`member.execute(__cs)` (Blam Network's Fukugen Daihon `enc_breach_m0.es`;
the sequence argument is only its audio). The script is A's `0x80800512` at
+0xB0: cookie (a new one starts it; the generation is used), pc, atom count,
then 64-byte atoms at +0xC0 whose kind 1 (scripted_sequence) carries the
action's FNV-1 hash at +0x10 and end condition 0 (its own duration). ACTION
is a name (`act_fallen_vault_right`) or a hash (`0x2AC3BDBA`).

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


SCRIPT, ATOMS, ATOM_SIZE = 0xB0, 0xC0, 0x40
SCRIPTED_SEQUENCE = 1


def fnv1(text):
    h = 0x811C9DC5
    for b in text.encode():
        h = (h * 0x01000193) & 0xFFFFFFFF
        h ^= b
    return h


def action_hash(action):
    return int(action, 16) if action.lower().startswith('0x') else fnv1(action)


def member(generation, action=None):
    if not 0 <= generation <= 0xFFFFFFFF:
        raise ValueError('generation must fit u32')
    image = bytearray(SIZE)
    struct.pack_into('<I', image, 0, generation)
    image[4], image[6] = 0xFF, 1
    struct.pack_into('<IBxH', image, 0x50, 0x811C9DC5, 0xFF, 0xFFFF)
    struct.pack_into('<I', image, 0x58, 0xFFFFFFFF)
    if action is not None:
        # cookie (31 bits), pc 0, one atom: scripted_sequence(action), ending on its own duration
        struct.pack_into('<III', image, SCRIPT, generation & 0x7FFFFFFF, 0, 1)
        struct.pack_into('<II', image, ATOMS, SCRIPTED_SEQUENCE, 0)
        struct.pack_into('<I', image, ATOMS + 0x10, action_hash(action))
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
        spec = states[sub] if isinstance(states[sub], tuple) else (states[sub],)
        replacement = ' '.join(fields[:5] + [TAG, member(*spec).hex()] + fields[7:])
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
    parser.add_argument('specs', nargs='+', metavar='SUB:GEN[:ACTION]')
    args = parser.parse_args()
    try:
        if not 0 <= args.group <= 0xFFFFFFFF:
            raise ValueError('group must fit u32')
        states = {}
        for spec in args.specs:
            fields = spec.split(':', 2)
            sub, generation = int(fields[0], 0), int(fields[1], 0)
            action = fields[2] if len(fields) > 2 else None
            if not -32768 <= sub <= 32767 or sub in states:
                raise ValueError('member index must fit i16 and occur once')
            member(generation, action)  # validate every spec before writing
            states[sub] = (generation, action)
        result = patched(args.base.read_text(), args.group, states)
    except ValueError as error:
        parser.error(str(error))
    args.out.write_text(result, newline='\n')
    print(f'{args.out}: {len(states)} member activation state(s); optional binding absent')


if __name__ == '__main__':
    main()
