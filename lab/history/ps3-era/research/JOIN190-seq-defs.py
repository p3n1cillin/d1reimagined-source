"""Offline, read-only: object IDs referenced by each sequence-group definition (private)."""
from pathlib import Path
import json, sys, struct
sys.path.insert(0, 'F:/d1/d1xenia/scripts')
from d1_pkg import PackageSet, Oodle, tag_hash
root = Path(__file__).parent; runtime = root.parent
paths = list((runtime/'RESEARCH/ps3_updates/final/packages').glob('*025c*.pkg'))
paths += list((runtime/'01_WORKING/ps3_destiny/PS3_GAME/USRDIR/packages').glob('*025c*.pkg'))
ps = PackageSet(paths, Oodle(runtime/'TOOLS/oodle/oo2core_3_win64_x86-64/oo2core_3_win64.dll'))
package = ps.by_id[0x25c]
names = {}
for line in open(r'F:\d1\d1xenia\notes\mission-objects-highway.md', encoding='utf-8'):
    p = line.split()
    if len(p) >= 2 and p[0].count('/') == 2 and p[0].startswith('0x'):
        g, k, i = p[0].split('/'); names[(int(g, 16), int(k, 16), int(i))] = p[1]
groups = {g for g, _, _ in names}
out = []
for entry in package.entries:
    if entry['reference'] != 0x80800861: continue
    raw = package.entry_data(entry['index'], ps.oodle)
    if len(raw) > 4000 or raw[0x30:0x34] != b'\x00\x00\x80\x80': pass
    ids = []
    for off in range(0, len(raw) - 7):
        g, k, z, i = struct.unpack_from('>IBBH', raw, off)
        if g in groups and z == 0 and (g, k, i) in names: ids.append((off, names[(g, k, i)]))
    own = [n for o, n in ids if n.startswith('seq_')]
    if any(n.startswith('seq_') for _, n in ids) and len(raw) < 4000:
        cls = raw[0x38:0x3c].hex()
        out.append({'tag': hex(tag_hash(package.pkg_id, entry['index'])), 'size': len(raw), 'class': cls, 'ids': ids})
for r in sorted(out, key=lambda r: r['ids'][0][1]): print(r['tag'], r['size'], r['class'], [n for _, n in r['ids']])
