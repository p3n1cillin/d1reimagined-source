"""Find live item instances (level 40 at +0x18, content index at +0x24) and print their grid (+0x58). Read-only."""
import ctypes, ctypes.wintypes as w, importlib.util, struct, sys, time
spec = importlib.util.spec_from_file_location('g', r'F:\d1\public-work\d1reimagined-source\tools\ps4_setup\d1-winguest.py')
g = importlib.util.module_from_spec(spec); spec.loader.exec_module(g)
p, base = g.open_client()
class MBI(ctypes.Structure):
    _fields_ = [('BaseAddress', ctypes.c_uint64), ('AllocationBase', ctypes.c_uint64), ('AllocationProtect', w.DWORD),
                ('_a', w.DWORD), ('RegionSize', ctypes.c_uint64), ('State', w.DWORD), ('Protect', w.DWORD), ('Type', w.DWORD), ('_b', w.DWORD)]
k32 = ctypes.WinDLL('kernel32', use_last_error=True)
k32.VirtualQueryEx.argtypes = [w.HANDLE, ctypes.c_uint64, ctypes.POINTER(MBI), ctypes.c_size_t]
h = p.h if hasattr(p, 'h') else p.handle
cis = [int(x) for x in sys.argv[1:]] or [3598]
addr, m, hits, t0 = 0x100000000, MBI(), [], time.time()
while addr < 0x1000000000 and time.time() - t0 < 150:
    if not k32.VirtualQueryEx(h, addr, ctypes.byref(m), ctypes.sizeof(m)):
        break
    size = m.RegionSize
    if m.State == 0x1000 and m.Protect in (0x04, 0x40) and size <= 0x20000000:
        try:
            raw = p.read(m.BaseAddress, size)
        except OSError:
            raw = b''
        for ci in cis:
            needle = struct.pack('<H', ci)
            i = raw.find(needle)
            while i >= 0:
                o = i - 0x24
                if o >= 0 and o + 0xB8 <= len(raw) and struct.unpack_from("<i", raw, o + 0x18)[0] in (1, 40):
                    hits.append((m.BaseAddress + o, struct.unpack_from("<i", raw, o + 0x18)[0], ci, struct.unpack_from('<H', raw, o + 0x58)[0],
                                 struct.unpack_from('<8H', raw, o + 0x88), struct.unpack_from('<8I', raw, o + 0x98)))
                i = raw.find(needle, i + 1)
    addr = m.BaseAddress + size
for a, lv, ci, grid, keys, vals in hits[:30]:
    print(hex(a), "level", lv, ci, hex(grid), keys, vals)
print(len(hits), 'hits', round(time.time() - t0), 's')
