"""Read-only: find the account inventory entries the test account serves (content 869 at slots 308..311 with
quantity 1000/2000/3000/4000 at +0x10, 32 bytes apart) in the running client, and print what surrounds them."""
import ctypes, ctypes.wintypes as w, importlib.util, struct, time
spec = importlib.util.spec_from_file_location('g', r'F:\d1\public-work\d1reimagined-source\tools\ps4_setup\d1-winguest.py')
g = importlib.util.module_from_spec(spec); spec.loader.exec_module(g)
p, base = g.open_client()
class MBI(ctypes.Structure):
    _fields_ = [('BaseAddress', ctypes.c_uint64), ('AllocationBase', ctypes.c_uint64), ('AllocationProtect', w.DWORD),
                ('_a', w.DWORD), ('RegionSize', ctypes.c_uint64), ('State', w.DWORD), ('Protect', w.DWORD), ('Type', w.DWORD), ('_b', w.DWORD)]
k32 = ctypes.WinDLL('kernel32', use_last_error=True)
k32.VirtualQueryEx.argtypes = [w.HANDLE, ctypes.c_uint64, ctypes.POINTER(MBI), ctypes.c_size_t]
h = p.h if hasattr(p, 'h') else p.handle
needle = struct.pack('<H', 869) + b'\0' * 14 + struct.pack('<I', 1000)
addr, m, t0, hits = 0x100000000, MBI(), time.time(), 0
while addr < 0x1000000000 and time.time() - t0 < 200:
    if not k32.VirtualQueryEx(h, addr, ctypes.byref(m), ctypes.sizeof(m)): break
    size = m.RegionSize
    if m.State == 0x1000 and m.Protect in (0x04, 0x40) and size <= 0x40000000:
        try: raw = p.read(m.BaseAddress, size)
        except OSError: raw = b''
        i = raw.find(needle)
        while i >= 0:
            q = [struct.unpack_from('<HxxxxxxQII', raw, i + 32 * k) if i + 32 * k + 24 <= len(raw) else None for k in range(4)]
            print(f'{m.BaseAddress + i:#x}: ' + ' | '.join(f'c{c} soid{s:#x} q{a} {b}' for c, s, a, b in filter(None, q)))
            hits += 1; i = raw.find(needle, i + 1)
    addr = m.BaseAddress + size
print('hits', hits, f'{time.time() - t0:.0f}s')
