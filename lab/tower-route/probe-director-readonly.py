"""Read-only probe of the Director's node-state object (56578). READ + QUERY process rights, no input.

    probe-director-readonly.py [--out FILE] [--samples N] [--every S]

Reads the translation manager's object (`*(eboot + 0x26BC830)`, FUN_0057e980(&DAT_026bc820)) that
FUN_0058f840 updates and FUN_0058fd40 fills with node states, and reports:
- `+0x114`, `+0x4580` (dirty bits: 1 = states to rebuild, 2 = translation retry, 0x18), `+0x4581`
  (states enabled) and `+0x4582` (a counter FUN_0058f840 bumps every call, so it shows it runs);
- the four ids at `+0x42E4/+0x42E6/+0x42EC/+0x42EE` and the translation result at `+0x104`;
- the node-state bytes at `+0x5E8 + dest*0x4C + node` (0 = hidden, 1-2 locked, 3+ available,
  4 new), as counts per destination.

Run it with the Director open on a profile that shows nodes and on one that doesn't, and compare.
Addresses from the 7 November 2026 trace (notes/ps4-56578-director-2026-11-07.md).
"""
import argparse
import ctypes
import ctypes.wintypes as wt
import json
import re
import struct
import subprocess
import time
from pathlib import Path

ap = argparse.ArgumentParser()
ap.add_argument('--out', default='')
ap.add_argument('--samples', type=int, default=3)
ap.add_argument('--every', type=float, default=1.0)
ap.add_argument('--destinations', type=int, default=24, help='how many 0x4C-byte destination rows to count')
a = ap.parse_args()

listing = subprocess.run(['tasklist', '/FI', 'IMAGENAME eq shadPS4.exe', '/FO', 'CSV', '/NH'],
                         capture_output=True, text=True).stdout
ids = [int(line.split('","')[1]) for line in listing.splitlines() if line.startswith('"shadPS4')]
assert len(ids) == 1, 'want exactly one shadPS4.exe'
log = Path(r'F:\d1\tools\shadPS4\0.19.0\user\log\shad_log.txt').read_bytes()[:2_000_000].decode('utf8', 'replace')
bases = re.findall(r'Loading module eboot\.bin to (0x[0-9a-f]+)', log)
assert len(bases) == 1, 'eboot base not found once in the log'
base = int(bases[0], 16)

k = ctypes.WinDLL('kernel32', use_last_error=True)
k.OpenProcess.restype = wt.HANDLE
k.ReadProcessMemory.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t,
                                ctypes.POINTER(ctypes.c_size_t)]
handle = k.OpenProcess(0x410, False, ids[0])  # PROCESS_VM_READ | PROCESS_QUERY_INFORMATION
assert handle


def rd(address, size):
    buf = ctypes.create_string_buffer(size)
    got = ctypes.c_size_t()
    ok = k.ReadProcessMemory(handle, ctypes.c_void_p(address), buf, size, ctypes.byref(got))
    assert ok and got.value == size, (hex(address), size)
    return buf.raw


def u(address, fmt='Q'):
    return struct.unpack('<' + fmt, rd(address, struct.calcsize(fmt)))[0]


samples = []
for i in range(a.samples):
    obj = u(base + 0x26BC830)
    s = {'time': time.time(), 'base': hex(base), 'object': hex(obj)}
    if obj:
        s.update({
            'b114': u(obj + 0x114, 'B'),
            'dirty4580': u(obj + 0x4580, 'B'),
            'enabled4581': u(obj + 0x4581, 'B'),
            'counter4582': u(obj + 0x4582, 'B'),
            'ids42e4': [u(obj + o, 'h') for o in (0x42E4, 0x42E6, 0x42EC, 0x42EE)],
            'translation104': rd(obj + 0x104, 16).hex(),
        })
        states = rd(obj + 0x5E8, 0x4C * a.destinations)
        rows = []
        for d in range(a.destinations):
            row = states[d * 0x4C:(d + 1) * 0x4C]
            counts = {v: row.count(v) for v in sorted(set(row)) if v}
            if counts:
                rows.append({'dest': d, 'states': {str(v): c for v, c in counts.items()}})
        s['node_states'] = rows
        s['nonzero_state_bytes'] = sum(1 for b in states if b)
    samples.append(s)
    if i + 1 < a.samples:
        time.sleep(a.every)

text = json.dumps(samples, indent=1)
print(text)
if a.out:
    Path(a.out).write_text(text)
