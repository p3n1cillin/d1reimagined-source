"""Write a sensor-client list with squad states (kind 1, state A 0x8080071E, 124 bytes, 56578).

    python -I d1-squad-states.py BASE_LIST OUT_LIST GROUP SUB:GEN[:COUNT] ...

e.g. the Breach Fallen, 30 s after the rifle (`agr-b-fallen5.txt`):

    d1-squad-states.py agr-b-got-weapon-c.txt agr-b-fallen5.txt 0xfc0e45a3 21:2:1 26:2:1 28:2:1 30:2:1

Each named squad entry of BASE_LIST gets a state image: the four object references absent
({0x811C9DC5, 0xFF, 0xFFFF}), the three hash fields +0x48/+0x4C/+0x78 at the client's own default
0x811C9DC5 (0 there is a real reference and the squad then finds no spawn location), desired
member count COUNT for variant 0 (+0x2C count, +0x30 i16), generation GEN at +0x44 (a new value
re-runs the squad) and existing-members mode -1 at +0x74 (keep). The per-frame squad update
(0x102B60 -> 0x102C10) creates members while desired - existing - live > 0. Layout from
`d1-eboot descriptors --tag 0x8080071e`; every image round-trips through the client's codec.

COUNT 0 writes no entry (+0x2C = 0): the client then places its own default, one body per member.
An explicit zero is one entry with count 0 (`SUB:GEN:=0`, `squad(gen, 0, entry=True)`): a squad whose
members are all named then places only the actors their own member states create (56578, native).
"""
import struct, sys

ABSENT = (0x811C9DC5, 0xFF, 0xFFFF)
NONE_HASH = 0x811C9DC5


def squad(gen, count=0, entry=False):
    b = bytearray(124)
    for ref in (0x00, 0x08, 0x50, 0x58):
        struct.pack_into('<IBxH', b, ref, *ABSENT)
    for off in (0x48, 0x4C, 0x78):
        struct.pack_into('<I', b, off, NONE_HASH)
    if count or entry:
        struct.pack_into('<I', b, 0x2C, 1)
        struct.pack_into('<h', b, 0x30, count)
    struct.pack_into('<I', b, 0x44, gen)
    b[0x74] = 0xFF
    return bytes(b)


def main():
    base, out, group = sys.argv[1], sys.argv[2], sys.argv[3].lower()
    states = {}
    for spec in sys.argv[4:]:
        parts = spec.split(':')
        entry = len(parts) > 2 and parts[2].startswith('=')
        f = [int(x.lstrip('='), 0) for x in parts]
        sub, gen, count = f[0], f[1], (f[2] if len(f) > 2 else 0)
        states[(group, '0x1', str(sub))] = squad(gen, count, entry)
    lines, hit = [], set()
    for line in open(base):
        p = line.split()
        key = tuple(p[:3])
        if len(p) >= 5 and key in states:
            line = ' '.join(p[:5] + ['0x8080071E', states[key].hex()]) + '\n'
            hit.add(key)
        lines.append(line)
    missing = set(states) - hit
    if missing:
        sys.exit(f'not in {base}: {sorted(missing)}')
    open(out, 'w', newline='\n').write(''.join(lines))
    print(f'{out}: {len(hit)} squad state(s)')


if __name__ == '__main__':
    main()
