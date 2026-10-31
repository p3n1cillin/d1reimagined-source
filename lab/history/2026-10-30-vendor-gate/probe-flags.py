"""Read-only: look hashes up in the master flag table (tag 0x80a5ffcd: rows at +0x30, 8 B {hash,u16 kind,u16 sub})."""
import runpy, struct, sys
from pathlib import Path
P = Path(__file__).parent
v = runpy.run_path(str(P / 'attach-readonly.py')); u, rd, b = v['u'], v['rd'], v['base']
def resolve(h):
    e = h >> 13 if h < 0x80000000 else (h - (1 << 32)) >> 13
    s = ((e & 0xffffffff) >> 18 | 0x3ff) & e & 0xffffffff
    ent = u(u(b + 0x1a238a0)) + s * 64; data = u(ent + 0x10); stride = u(ent + 0x30, 'i'); mask = u(ent + 0x34, 'i') & 0xffffffffffffffff
    p = data + (h & 0x1fff) * stride; return p - (mask & u(p + 8))
a = resolve(0x80a5ffcd) & 0xffffffffffffffff
head = rd(a, 0x30); n = 7731
rows = rd(a + 0x30, 8 * n)
print('head', head.hex())
table = {}
for i in range(n):
    h, k, s = struct.unpack_from('<IHH', rows, 8 * i); table[h] = (i, k, s)
for t in sys.argv[1:]:
    print(t, table.get(int(t, 16)))
WANT = {0xd3f, 0xd40, 0x134, 0x135, 0x1a3, 0x1b8, 0xa6c}
kinds = {}
for h, (i, k, s) in table.items():
    kinds[k] = kinds.get(k, 0) + 1
    if s in WANT:
        print(f'sub {s:#x}: row {i} hash {h:08x} kind {k}')
print('kinds', sorted(kinds.items()))
