"""Read-only: the live bucket table the item count 0x26D6E0 walks: obj=[base+0x26527E0], table=obj->vtbl[0x98](obj)
is static code, so read the vtable slot, disassemble nothing - just print the function address; then scan for rows."""
import runpy, struct, sys
from pathlib import Path
v = runpy.run_path(str(Path(r'lab/history\2026-10-30-vendor-gate') / 'attach-readonly.py')); u, b = v['u'], v['base']
obj = u(b + 0x26527e0); vt = u(obj); fn = u(vt + 0x98)
print('globals', hex(obj), 'vtbl', hex(vt), 'vfunc[0x98]', hex(fn), 'img', hex(fn - b))
t = u(b + 0x26527f8 + 0x80) + 8; n = u(t + 0x84, 'i')
print('table', hex(t), 'rows', n)
for k in range(n):
    r = t + 0x88 + 0x14 * k
    print(f'  ord {u(r, "B"):#04x}  first {u(r + 4, "i"):3d}  n {u(r + 8, "i"):3d}  scope {u(r + 0x10, "b")}  raw {bytes(u(r + i, "B") for i in range(0x14)).hex()}')
