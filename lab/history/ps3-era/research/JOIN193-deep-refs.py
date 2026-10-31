import sys, struct, collections
sys.path.insert(0, 'F:/d1/d1xenia/scripts')
from pathlib import Path
from d1_pkg import PackageSet, Oodle, split_tag_hash
runtime = Path('..')
paths = list((runtime/'RESEARCH/ps3_updates/final/packages').glob('*.pkg')) + list((runtime/'01_WORKING/ps3_destiny/PS3_GAME/USRDIR/packages').glob('*.pkg'))
ps = PackageSet(paths, Oodle(runtime/'TOOLS/oodle/oo2core_3_win64_x86-64/oo2core_3_win64.dll'))
missing = collections.Counter()
def refs(raw):
    out = []
    for o in range(0, len(raw) - 3, 4):
        v = struct.unpack_from('>I', raw, o)[0]
        if 0x80800000 <= v < 0x81800000 and (v >> 16) != 0x8080:
            s = split_tag_hash(v)
            if s: out.append((v, s))
    return out
seen = set()
def walk(tag, depth, path):
    if tag in seen or depth > 4: return
    seen.add(tag)
    pid, idx = split_tag_hash(tag)
    pk = ps.by_id.get(pid)
    if pk is None:
        missing[hex(pid)] += 1; print('  ' * depth + f'{tag:#x} MISSING PACKAGE {pid:#x} via {path}'); return
    if idx >= len(pk.entries):
        print('  ' * depth + f'{tag:#x} index beyond package'); return
    e = pk.entries[idx]
    try: raw = pk.entry_data(idx, ps.oodle)
    except Exception as ex: print('  ' * depth + f'{tag:#x} unreadable {ex}'); return
    print('  ' * depth + f'{tag:#x} pkg {pid:#x} type {e["reference"]:#x} {len(raw)}')
    for v, s in sorted(set(refs(raw))):
        if v != tag: walk(v, depth + 1, path + [hex(tag)])
for t in (0x80bc6166, 0x80aa9a08, 0x80aa9a0e, 0x80bc635b):
    walk(t, 0, [])
print('missing', dict(missing))
