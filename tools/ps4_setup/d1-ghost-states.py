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

The 56578 layouts are from `d1-eboot descriptors --tag 0x80800868 --tag 0x80800556`:
- the member is 2312 bytes. Its leading fields are the same four, then nested parts left empty
  except an object reference at `+0x50` (absent: `0x811C9DC5, 0xFF, 0xFFFF`) and a hash at `+0x28`
  (the client's empty `0x811C9DC5`);
- the performance is 48 bytes: the primary state at `+0`, and an empty bounded list at `+0xC`.

Rows that already carry a state are replaced. A new generation re-runs the state, so a list that
keeps an earlier state must keep its generation.
"""
import argparse
import struct
import sys

BREACH = '0xfc0e45a3'
EMPTY_HASH = 0x811C9DC5
ACTION = 0x9B1D6A71          # the performance asset's only action key (JOIN111)
MEMBER_TAG, PERFORM_TAG = '0x80800868', '0x80800556'


def member(gen):
    b = bytearray(2312)
    struct.pack_into('<I', b, 0x00, gen)
    struct.pack_into('<bbB', b, 0x04, -1, 0, 1)
    struct.pack_into('<I', b, 0x28, EMPTY_HASH)
    struct.pack_into('<IBxH', b, 0x50, EMPTY_HASH, 0xFF, 0xFFFF)
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
