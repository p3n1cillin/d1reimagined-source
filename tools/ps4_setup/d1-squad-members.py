"""Where the Divide squads' members are (56578): resolve each member's object handles and print world positions.

    d1-watchwrite.py --exec 0x104000 --peek rbx+rcx+0x0 ... --peek rbx+rcx+0x7c --out members.jsonl
    d1-squad-members.py members.jsonl

The break at 0x104000 is inside the squad live-member counter (0x103F70), which walks a squad's
member list and counts entries of the entity table at 0x25D79A0 whose +0x50 is -1. Each peeked
entry carries the member's type handle at +0x1C and four object handles at +0x38..+0x44; this
resolves them through the client's object registry and prints float triples inside the Divide's
bounds. Read-only.
"""
import importlib.util, json, os, struct, sys
s = importlib.util.spec_from_file_location('g', os.path.join(os.path.dirname(os.path.abspath(__file__)), 'd1-winguest.py'))
g = importlib.util.module_from_spec(s); s.loader.exec_module(g)
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
    return e - (mk & p.u(e + 8, 'Q'))

def positions(obj, n=0x1000):
    try:
        raw = p.read(obj, n)
    except OSError:
        return []
    out = []
    for o in range(0, n - 12, 4):
        x, y, z = struct.unpack_from('<fff', raw, o)
        if 150 < x < 520 and -820 < y < -420 and -30 < z < 90:
            out.append((hex(o), round(x, 1), round(y, 1), round(z, 1)))
    return out

rows = [json.loads(l) for l in open(sys.argv[1]) if '"exec"' in l]
seen = set()
for r in rows:
    pk = r['peek']; vals = [int(pk[f'rbx+rcx+{hex(o)}'], 16) for o in range(0, 128, 4)]
    if vals[14] in seen: continue
    seen.add(vals[14])
    print(f'type {vals[7]:08x}', end='')
    for h in vals[14:18]:
        obj = resolve(h)
        pos = positions(obj) if obj else []
        print(f' | {h:08x}->{obj:#x}' if obj else f' | {h:08x}->None', pos[:2], end='')
    print()
