"""Offline, read-only: resource tags referenced by the revival performances (private)."""
import sys, struct, collections
sys.path.insert(0, 'F:/d1/d1xenia/scripts')
from pathlib import Path
from d1_pkg import PackageSet, Oodle, tag_hash, split_tag_hash
runtime = Path('..')
paths = list((runtime/'RESEARCH/ps3_updates/final/packages').glob('*.pkg')) + list((runtime/'01_WORKING/ps3_destiny/PS3_GAME/USRDIR/packages').glob('*.pkg'))
ps = PackageSet(paths, Oodle(runtime/'TOOLS/oodle/oo2core_3_win64_x86-64/oo2core_3_win64.dll'))
def entry(tag):
    pid, idx = split_tag_hash(tag)
    pk = ps.by_id.get(pid)
    if not pk or idx >= len(pk.entries): return None, None
    return pk, pk.entries[idx]
def refs(raw):
    out = []
    for o in range(0, len(raw) - 3, 4):
        v = struct.unpack_from('>I', raw, o)[0]
        if 0x80800000 <= v < 0x81000000 and v & 0xFFFF0000 != 0x80800000:
            s = split_tag_hash(v)
            if s and s[0] in ps.by_id and s[1] < len(ps.by_id[s[0]].entries): out.append(v)
    return out
for root in (0x80cb8d65, 0x80cb8d5b):
    pk, e = entry(root)
    raw = pk.entry_data(e['index'], ps.oodle)
    rs = collections.Counter(refs(raw))
    print(hex(root), 'ref type', hex(e['reference']), len(raw), 'refs', len(rs))
    for r, n in sorted(rs.items()):
        p2, e2 = entry(r)
        print('  ', hex(r), 'pkg', hex(split_tag_hash(r)[0]), 'type', hex(e2['reference']), 'size', e2.get('size', '?'), 'x', n)
