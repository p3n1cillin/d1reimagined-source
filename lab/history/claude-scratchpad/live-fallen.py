"""Find which position fields of the Divide squad members move (live bodies). Read-only.

    live-fallen.py MEMBERS.jsonl OUT.json
Writes [{handle, type, addr}] for every member object field that holds a Divide position and changed
between two reads 1.5 s apart, plus the static ones (marked moved=false).
"""
import importlib.util, json, math, struct, sys, time
spec = importlib.util.spec_from_file_location('g', r'F:\d1\public-work\d1reimagined-source\tools\ps4_setup\d1-winguest.py')
g = importlib.util.module_from_spec(spec); spec.loader.exec_module(g)
p, base = g.open_client()
reg = p.u(p.u(base + g.REGISTRY, 'Q'), 'Q')


def resolve(tag):
    esi = struct.unpack('<i', struct.pack('<I', tag))[0] >> 13
    sl = (((esi & 0xFFFFFFFF) >> 0x12) | 0x3FF) & esi & 0xFFFFFFFF
    rec = reg + sl * 0x40
    d, es, mk = p.u(rec + 0x10, 'Q'), p.u(rec + 0x30, 'i'), p.u(rec + 0x34, 'i') & 0xFFFFFFFFFFFFFFFF
    if not d or es <= 0 or es > 0x100000:
        return None
    e = d + (tag & 0x1FFF) * es
    obj = (e - (mk & p.u(e + 8, 'Q'))) & 0xFFFFFFFFFFFFFFFF
    return obj if 0x100000000 <= obj < 0x1000000000 else None


def triples(obj, n=0x1000):
    try:
        raw = p.read(obj, n)
    except OSError:
        return {}
    out = {}
    for o in range(0, n - 12, 4):
        x, y, z = struct.unpack_from('<fff', raw, o)
        if 100 < x < 520 and -820 < y < -420 and -30 < z < 90:
            out[o] = (x, y, z)
    return out


rows = [json.loads(l) for l in open(sys.argv[1]) if '"exec"' in l]
members = {}
for r in rows:
    vals = [int(r['peek'][f'rbx+rcx+{hex(o)}'], 16) for o in range(0, 128, 4)]
    members[vals[14]] = (vals[7], vals[14:18])
objs = {}
for h, (typ, handles) in members.items():
    for k in handles:
        o = resolve(k)
        if o:
            objs[(h, k)] = (typ, o)
first = {key: triples(o) for key, (typ, o) in objs.items()}
time.sleep(1.5)
found = []
for key, (typ, o) in objs.items():
    now = triples(o)
    for off, a in first[key].items():
        b = now.get(off)
        if b:
            found.append({'member': hex(key[0]), 'handle': hex(key[1]), 'type': hex(typ), 'addr': o + off,
                          'pos': [round(v, 2) for v in b], 'moved': round(math.dist(a, b), 2)})
json.dump(found, open(sys.argv[2], 'w'), indent=1)
movers = [f for f in found if f['moved'] > 0.05]
print(f'{len(members)} members, {len(objs)} objects, {len(found)} position fields, {len(movers)} moving')
for f in sorted(movers, key=lambda f: -f['moved'])[:15]:
    print(f['type'], f['member'], hex(f['addr']), f['pos'], 'moved', f['moved'])
