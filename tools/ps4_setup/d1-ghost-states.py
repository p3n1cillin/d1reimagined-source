"""Give the Breach's Ghost its authored member and its fly-to-the-gun performance in a sensor list (56578).

    python -I d1-ghost-states.py IN_LIST OUT_LIST [--member GEN] [--perform GEN]

On the PS3 build (notes/ghost-light-0124.md, JOIN109 and JOIN111) the Ghost only moved once two
states were served:
- the authored member `sq_nux_ghost__ghost` (kind 2, row 95, state A `0x80800868`): generation, then
  `+4` = -1, `+5` = 0 and `+6` = true. That creates the Ghost through the member path, so it is bound
  to its performance, which a squad's desired count alone never does;
- the performance `nux_ghost` (kind 0x28, row 125, state A `0x80800556`): the primary state holds the
  action key `0x9B1D6A71`, the empty override `0x811C9DC5` and a generation. The client looked it up
  under category `0xAFB11A12`, dispatched it, and the Ghost moved about 21 m towards the gun.

The member state is `d1-member-states.py`'s image, which has been checked through the client's codec
and natively: the leading four fields, and the optional binding at `+0x50` absent with index -1 at
`+0x58` (a zero there names a real reference, and the client's unguarded lookup crashed on one).
The performance is 48 bytes on 56578 (`d1-eboot descriptors --tag 0x80800556`): the primary state at
`+0`, and an empty bounded list at `+0xC`.

Rows that already carry a state are replaced. A new generation re-runs the state, so a list that
keeps an earlier state must keep its generation.
"""
import argparse
import importlib.util
from pathlib import Path
import struct
import sys

BREACH = '0xfc0e45a3'
EMPTY_HASH = 0x811C9DC5
ACTION = 0x9B1D6A71          # the performance asset's only action key (JOIN111)
PERFORM_TAG = '0x80800556'

_spec = importlib.util.spec_from_file_location('member_states', Path(__file__).with_name('d1-member-states.py'))
member_states = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(member_states)
MEMBER_TAG = member_states.TAG
_spec = importlib.util.spec_from_file_location('squad_states', Path(__file__).with_name('d1-squad-states.py'))
squad_states = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(squad_states)
SQUAD_TAG = '0x8080071E'


def member(gen):
    return member_states.member(gen)


def squad(gen):
    """`sq_nux_ghost` (kind 1, row 94) with one entry of count 0. A named member then makes the only
    actor; with no entry the client adds its default body (DeRveXx, PR #193). Run sense106 served
    the member with no squad state, and the reader found no member of squad 94."""
    b = bytearray(squad_states.squad(gen, 0))
    struct.pack_into('<I', b, 0x2C, 1)
    struct.pack_into('<h', b, 0x30, 0)
    return bytes(b)


def performance(gen):
    b = bytearray(48)
    struct.pack_into('<IIi', b, 0x00, ACTION, EMPTY_HASH, gen)
    return bytes(b)


def apply(lines, states):
    """`states` maps (kind, index) to (tag, image); every one must be in the list."""
    out, hit = [], set()
    for line in lines:
        p = line.split()
        key = (p[1].lower(), int(p[2])) if len(p) >= 5 and p[0].lower() == BREACH else None
        if key in states:
            tag, image = states[key]
            line = ' '.join(p[:5] + [tag, image.hex()]) + '\n'
            hit.add(key)
        out.append(line)
    if set(states) - hit:
        sys.exit(f'not in the list: {sorted(set(states) - hit)}')
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('inp')
    ap.add_argument('out')
    ap.add_argument('--member', type=int, help='serve the authored member with this generation')
    ap.add_argument('--perform', type=int, help='serve the fly-to-the-gun performance with this generation')
    a = ap.parse_args()
    states = {}
    if a.member is not None:
        states[('0x1', 94)] = (SQUAD_TAG, squad(a.member))
        states[('0x2', 95)] = (MEMBER_TAG, member(a.member))
    if a.perform is not None:
        states[('0x28', 125)] = (PERFORM_TAG, performance(a.perform))
    if not states:
        ap.error('give --member, --perform or both')
    with open(a.inp, encoding='utf-8') as f:
        lines = apply(f.readlines(), states)
    with open(a.out, 'w', encoding='utf-8', newline='\n') as f:
        f.writelines(lines)
    print(f'{a.out}: {", ".join(f"{k}/{i}" for k, i in states)}')


if __name__ == '__main__':
    main()
