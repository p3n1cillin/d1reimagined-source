"""Read-only pre-departure read of the live c_post_game_data_component (56578).

Attaches with READ + QUERY rights only (attach-readonly.py), finds component instances by their vtable
(image +0x1963c30, from sense54's UI walk: head 0x6d13c30 at base 0x53b0000), and records the fields
named in the 29 Oct world165-advancement hand-back plus a raw dump. No input, no writes to the game.
Usage: python pregame-component-read.py [--out DIR]
"""
import argparse, ctypes, ctypes.wintypes as wt, json, runpy, struct, time
from pathlib import Path

Q = Path(r'lab/history\2026-10-29-postgame-codex')
VTABLE = 0x1963c30
ap = argparse.ArgumentParser()
ap.add_argument('--out', default=str(Q))
args = ap.parse_args()

v = runpy.run_path(str(Q / 'attach-readonly.py'), run_name='attach')
rd, u, base, handle, k = v['rd'], v['u'], v['base'], v['handle'], v['k']
vt = base + VTABLE
first = u(vt)
assert base <= first < base + 0x18d8000, f'vtable +{VTABLE:#x} does not point into code ({first:#x})'


class MBI(ctypes.Structure):
    _fields_ = [('BaseAddress', ctypes.c_void_p), ('AllocationBase', ctypes.c_void_p), ('AllocationProtect', wt.DWORD),
                ('PartitionId', wt.WORD), ('RegionSize', ctypes.c_size_t), ('State', wt.DWORD), ('Protect', wt.DWORD), ('Type', wt.DWORD)]


k.VirtualQueryEx.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.POINTER(MBI), ctypes.c_size_t]
k.VirtualQueryEx.restype = ctypes.c_size_t
buf = ctypes.create_string_buffer(1 << 24)
got = ctypes.c_size_t()


def regions():
    a, m = 0, MBI()
    while a < 0x7fffffff0000 and k.VirtualQueryEx(handle, a, ctypes.byref(m), ctypes.sizeof(m)):
        size = m.RegionSize
        if m.State == 0x1000 and m.Protect in (0x04, 0x40):  # committed, READWRITE / EXECUTE_READWRITE
            yield (m.BaseAddress or 0), size
        a = (m.BaseAddress or 0) + size


t0 = time.time()
needle = struct.pack('<Q', vt)
hits, scanned = [], 0
for start, size in regions():
    for off in range(0, size, len(buf)):
        n = min(len(buf), size - off)
        if not k.ReadProcessMemory(handle, start + off, buf, n, ctypes.byref(got)):
            continue
        data = buf.raw[:got.value]
        scanned += got.value
        i = data.find(needle)
        while i != -1:
            if i % 8 == 0:
                hits.append(start + off + i)
            i = data.find(needle, i + 1)

out = {'time': time.time(), 'base': hex(base), 'vtable': hex(vt), 'scanned_mb': scanned >> 20,
       'scan_s': round(time.time() - t0, 1), 'components': []}
for c in hits:
    f = lambda o, fmt='I': u(c + o, fmt)
    out['components'].append({
        'address': hex(c),
        'identity_90_98_a0': [hex(f(0x90, 'Q')), hex(f(0x98, 'Q')), hex(f(0xa0, 'Q'))],
        'profile_version_628': f(0x628),
        'char_initial_level_ac': f(0xac), 'char_initial_frac_b0': struct.unpack('<f', rd(c + 0xb0, 4))[0],
        'char_final_level_b4': f(0xb4), 'char_final_frac_b8': struct.unpack('<f', rd(c + 0xb8, 4))[0],
        'char_xp_gained_bc': f(0xbc), 'char_xp_remaining_c0': f(0xc0),
        'prestige_c4_d8': [f(o) for o in range(0xc4, 0xdc, 4)],
        'raw_0_700': rd(c, 0x700).hex(),
    })
col = u(base + 0x26bc040)
if col:
    out['collector'] = {'address': hex(col), 'state': u(col, 'B'), 'target': u(col + 1, 'B'), 'activity': u(col + 2, 'h'),
                        'level_xp': struct.unpack('<III', rd(col + 0x10, 12)), 'prestige_xp': struct.unpack('<III', rd(col + 0x1c, 12))}
dest = Path(args.out) / f'pregame-component-{int(time.time())}.json'
dest.write_text(json.dumps(out, indent=2))
print(dest)
print(json.dumps({k2: v2 for k2, v2 in out.items() if k2 != 'components'}, indent=1))
for comp in out['components']:
    print({k2: v2 for k2, v2 in comp.items() if k2 != 'raw_0_700'})
