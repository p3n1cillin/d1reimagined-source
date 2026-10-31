"""Read-only: resolve item definition tags in the running client and print bytes +0x80..+0x90 (bucket ordinal
at +0x88, stackable flag at +0x8B per 0x26D360). Usage: probe-itemdef.py TAG [TAG ...]"""
import runpy, sys
from pathlib import Path
v = runpy.run_path(str(Path(r'lab/history\2026-10-30-vendor-gate') / 'attach-readonly.py')); u, b = v['u'], v['base']
M = 0xffffffffffffffff
def resolve(h):
    e = h >> 13 if h < 0x80000000 else (h - (1 << 32)) >> 13
    s = ((e & 0xffffffff) >> 18 | 0x3ff) & e & 0xffffffff
    ent = u(u(b + 0x1a238a0)) + s * 64; data = u(ent + 0x10); stride = u(ent + 0x30, 'i'); mask = u(ent + 0x34, 'i') & M
    p = data + (h & 0x1fff) * stride; return (p - (mask & u(p + 8))) & M
for t in sys.argv[1:]:
    a = resolve(int(t, 16))
    raw = bytes(u(a + i, 'B') for i in range(0x60, 0xa0))
    print(t, hex(a), 'def+0x60..0xa0:', raw.hex(' ', 4), ' +0x88 =', raw[0x28], ' +0x8b =', raw[0x2b])
