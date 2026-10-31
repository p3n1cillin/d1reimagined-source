"""Read-only: decode the unlock expressions an activity's permission check evaluates.
Activity row +8 count / +0x10 relative list of u32 ids; each id is binary-searched in the content
root's +0x28 table (40-byte entries, key u32 at +0x20); an entry holds two lists of 16-byte expressions
(+0x00 and +0x10: {u64 count, relative ops}); ops are 8 bytes {u8 op, u32 operand at +4}, evaluated by
0x28e590 (1 flag[i]==2, 10 value[i], 11 literal, 2 not, 21 negate, 3..20 binary). READ + QUERY only."""
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
def rel(a):
    o = u(a); return (a + o) & M if o else 0
root = resolve(0x80a58001)
t28 = resolve(u(root + 0x28, 'I'))
T = t28 + 0x30                      # 0x25cd60 returns tag+8; the iterator adds 0x28
cnt = u(T); base = rel(T + 8) + 0x10
print(f'root+0x28 tag {u(root + 0x28, "I"):#x}: {cnt} entries at {base:#x}')
keys = [u(base + 40 * i + 0x20, 'I') for i in range(cnt)]
acts = u(b + 0x26527f8 + 0x20) + 8
for idx in map(int, sys.argv[1:] or ['0', '165']):
    rows = u(acts + 8) + acts + 0x18
    row = (u(rows + 16 * idx + 8) + rows + 16 * idx + 8) & M
    ids = [u(rel(row + 0x10) + 0x10 + 4 * k, 'I') for k in range(u(row + 8, 'i'))]
    print(f'activity {idx} {u(row, "I"):#010x}: ids {[f"{x:08x}" for x in ids]}')
    for key in ids:
        if key not in keys:
            print(f'  {key:08x}: not in table'); continue
        ent = base + 40 * keys.index(key)
        print(f'  {key:08x}: entry {keys.index(key)}')
        for part in (0, 0x10):
            n = u(ent + part); arr = rel(ent + part + 8) + 0x10 if n else 0
            for j in range(min(n, 16)):
                ex = arr + 16 * j; m = u(ex); ops = rel(ex + 8) + 0x10 if m else 0
                code = [(u(ops + 8 * q, 'B'), u(ops + 8 * q + 4, 'I')) for q in range(min(m, 32))]
                print(f'    list {part // 16} expr {j}: ' + ' '.join(f'{op}:{arg:#x}' for op, arg in code))
