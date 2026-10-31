"""Read-only: the activity rows the permission check walks (content root +0x48 table, 16-byte rows,
row+8 relative pointer), and the root +0x28 table the condition iterators index. READ + QUERY only."""
import runpy, struct, sys
from pathlib import Path
P = Path(__file__).parent
v = runpy.run_path(str(P / 'attach-readonly.py')); u, rd, b = v['u'], v['rd'], v['base']
M = 0xffffffffffffffff
def resolve(h):
    e = h >> 13 if h < 0x80000000 else (h - (1 << 32)) >> 13
    s = ((e & 0xffffffff) >> 18 | 0x3ff) & e & 0xffffffff
    ent = u(u(b + 0x1a238a0)) + s * 64; data = u(ent + 0x10); stride = u(ent + 0x30, 'i'); mask = u(ent + 0x34, 'i') & M
    p = data + (h & 0x1fff) * stride; return (p - (mask & u(p + 8))) & M
def dump(a, n, label):
    raw = rd(a, n); print(f'== {label} at {a:#x}')
    for o in range(0, n, 16):
        print(f'  +{o:03x} ' + ' '.join(raw[o + i:o + i + 4][::-1].hex() for i in range(0, 16, 4)))
cache = b + 0x26527f8
root_tag = None
acts = u(cache + 0x20) + 8          # cached activity table (+8), as 0x25ce40 returns
print('activity table count', u(acts, 'i'))
for idx in map(int, sys.argv[1:] or ['0', '165']):
    rows = u(acts + 8) + acts + 0x18
    row = (u(rows + 16 * idx + 8) + rows + 16 * idx + 8) & M
    dump(row, 0x100, f'activity {idx} row')
t28 = u(cache + 0x10) + 8
print('root+0x28 table at', hex(t28), 'count', u(t28 + 8 - 8, 'i'))
dump(t28, 0x80, 'root+0x28 table head')
