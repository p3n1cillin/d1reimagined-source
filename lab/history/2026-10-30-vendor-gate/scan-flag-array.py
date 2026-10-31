"""Read-only: find the unlock-expression flag array (8192 tri-state bytes, 2 = set, then 2048 ints) in the
client by signature: every byte in {0,1,2}, and A Guardian Rises' requirement true
((f[0x1a3] or f[0x1b8]) and (f[0xa6c] or f[0x1b8]), f[0x135] and f[0xd40] clear). Prints the Tower's flags.
READ + QUERY only."""
import runpy, ctypes, ctypes.wintypes as wt, sys
import numpy as np
from pathlib import Path
P = Path(__file__).parent
v = runpy.run_path(str(P / 'attach-readonly.py')); k, h = v['k'], v['handle']
class MBI(ctypes.Structure):
    _fields_ = [('BaseAddress', ctypes.c_void_p), ('AllocationBase', ctypes.c_void_p), ('AllocationProtect', wt.DWORD),
                ('PartitionId', wt.WORD), ('RegionSize', ctypes.c_size_t), ('State', wt.DWORD), ('Protect', wt.DWORD), ('Type', wt.DWORD)]
k.VirtualQueryEx.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.POINTER(MBI), ctypes.c_size_t]
k.VirtualQueryEx.restype = ctypes.c_size_t
def read(addr, n):
    buf = ctypes.create_string_buffer(n); got = ctypes.c_size_t()
    ok = k.ReadProcessMemory(h, ctypes.c_void_p(addr), buf, n, ctypes.byref(got))
    return buf.raw[:got.value] if ok else b''
N = 0x2000; CH = 1 << 25
addr, hits, scanned = 0, [], 0
while addr < (1 << 47):
    m = MBI()
    if not k.VirtualQueryEx(h, ctypes.c_void_p(addr), ctypes.byref(m), ctypes.sizeof(m)): break
    base, size = m.BaseAddress or 0, m.RegionSize
    if m.State == 0x1000 and m.Protect & 0xcc and not m.Protect & 0x100:    # committed, writable, not guard
        off = 0
        while off < size:
            n = min(CH + N, size - off); raw = read(base + off, n)
            if len(raw) > N:
                a = np.frombuffer(raw, np.uint8); scanned += len(raw)
                L = len(a) - N
                two = a == 2
                ok = (two[0x1a3:0x1a3 + L] | two[0x1b8:0x1b8 + L]) & (two[0xa6c:0xa6c + L] | two[0x1b8:0x1b8 + L]) \
                     & (a[0:L] <= 2) & (a[N - 1:N - 1 + L] <= 2)
                for p in np.flatnonzero(ok)[:2000]:
                    w = a[p:p + N]
                    if w.max() <= 2 and (w == 2).sum() >= 4:
                        hits.append((base + off + p, w))
            off += CH
    addr = base + size
print(f'scanned {scanned >> 20} MiB, {len(hits)} candidates')
for a, w in hits[:20]:
    sets = np.flatnonzero(w == 2)
    print(f'{a:#x}: set {len(sets)} ones {(w == 1).sum()}  tower 0x134={w[0x134]} 0xd3f={w[0xd3f]}  agr 0x135={w[0x135]} 0xd40={w[0xd40]}  '
          f'0x1a3={w[0x1a3]} 0x1b8={w[0x1b8]} 0xa6c={w[0xa6c]}  first set {[hex(x) for x in sets[:12]]}')
