"""Offline, read-only: which activity-package entries reference tv_flashlight (private output)."""
from pathlib import Path
import json, sys, struct, hashlib
sys.path.insert(0, 'F:/d1/d1xenia/scripts')
from d1_pkg import PackageSet, Oodle, tag_hash
root = Path(__file__).parent; runtime = root.parent
paths = list((runtime/'RESEARCH/ps3_updates/final/packages').glob('*025c*.pkg'))
paths += list((runtime/'01_WORKING/ps3_destiny/PS3_GAME/USRDIR/packages').glob('*025c*.pkg'))
ps = PackageSet(paths, Oodle(runtime/'TOOLS/oodle/oo2core_3_win64_x86-64/oo2core_3_win64.dll'))
package = ps.by_id[0x25c]
patterns = {'tv_flashlight': struct.pack('>IBBH', 0x3c2f5ed5, 0x3a, 0, 76),
            'tv_nux_light_start': struct.pack('>IBBH', 0xfc0e45a3, 0x3a, 0, 173),
            'pt_fallen_above': struct.pack('>IBBH', 0x3c2f5ed5, 0x1d, 0, 47)}
out = root/'JOIN190-flashlight'; out.mkdir(exist_ok=True)
rows = []
for entry in package.entries:
    raw = package.entry_data(entry['index'], ps.oodle)
    hits = {k: [i for i in range(len(raw) - 7) if raw.startswith(p, i)] for k, p in patterns.items()}
    hits = {k: v for k, v in hits.items() if v}
    if hits:
        tag = tag_hash(package.pkg_id, entry['index'])
        (out/f'{tag:08x}.bin').write_bytes(raw)
        row = {'tag': hex(tag), 'reference': hex(entry['reference']), 'size': len(raw), 'hits': hits}
        rows.append(row); print(json.dumps(row), flush=True)
(out/'matches.json').write_text(json.dumps(rows, indent=1))
print(len(package.entries), 'entries scanned')
