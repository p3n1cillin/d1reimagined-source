"""Read-only: dump activity condition records (class 0x8080182e) by tag. READ + QUERY only."""
import runpy, sys
from pathlib import Path
P = Path(__file__).parent
v = runpy.run_path(str(P / 'attach-readonly.py')); u, rd, b = v['u'], v['rd'], v['base']
def resolve(h):
    e = h >> 13 if h < 0x80000000 else (h - (1 << 32)) >> 13
    s = ((e & 0xffffffff) >> 18 | 0x3ff) & e & 0xffffffff
    ent = u(u(b + 0x1a238a0)) + s * 64; data = u(ent + 0x10); stride = u(ent + 0x30, 'i'); mask = u(ent + 0x34, 'i') & 0xffffffffffffffff
    p = data + (h & 0x1fff) * stride; return p - (mask & u(p + 8))
for t in sys.argv[1:]:
    a = resolve(int(t, 16)); raw = rd(a, int(0x100))
    print(f'== {t} at {a:#x}')
    for o in range(0, len(raw), 16):
        print(f'  +{o:03x} ' + ' '.join(raw[o + i:o + i + 4][::-1].hex() for i in range(0, 16, 4)))
