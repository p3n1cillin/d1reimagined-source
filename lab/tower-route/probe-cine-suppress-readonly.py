# SPDX-License-Identifier: GPL-2.0-or-later
"""Why the client uses the common loading UI instead of its loading cinematics (F48: no ship flight
when launching from orbit). READ + QUERY process rights only, no input.

    python probe-cine-suppress-readonly.py [--every 2] [--seconds 600]

The 56578 client logs "loading cinematics are suppressed -- using the common loading UI" from
FUN_00788e80 (and its two twins) when FUN_00766750 is true: `*(u8*)[DAT_026bc068] == 1` OR
FUN_0078c710 (a session condition: the session object's +0x59c against DAT_026cf2b0). This prints
the first input, the flag byte, each time it changes, so a run tells whether the global flag is
the cause. Addresses are eboot vaddrs, added to the eboot base from the emulator log.
"""
import argparse
import ctypes
import ctypes.wintypes as wt
import re
import struct
import subprocess
import sys
import time
from pathlib import Path

P = Path(__file__).parent
sys.path.insert(0, str(P))
import routepaths  # noqa: E402

FLAG_PTR = 0x26BC068          # DAT_026bc068: a pointer to the suppress byte
SESSION_KEY = 0x26CF2B0       # DAT_026cf2b0: compared with the session object's +0x59c


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--every', type=float, default=2.0)
    ap.add_argument('--seconds', type=float, default=600.0)
    a = ap.parse_args()
    listing = subprocess.run(['tasklist', '/FI', 'IMAGENAME eq shadPS4.exe', '/FO', 'CSV', '/NH'],
                             capture_output=True, text=True).stdout
    ids = [int(line.split('","')[1]) for line in listing.splitlines() if line.startswith('"shadPS4')]
    if len(ids) != 1:
        sys.exit(f'expected one shadPS4 process, found {len(ids)}')
    log = routepaths.shad_log().read_bytes()[:3000000].decode('utf8', 'replace')
    bases = re.findall(r'Loading module eboot\.bin to (0x[0-9a-f]+)', log)
    if len(bases) != 1:
        sys.exit('eboot base not found in the emulator log')
    base = int(bases[0], 16)
    k = ctypes.WinDLL('kernel32', use_last_error=True)
    k.OpenProcess.restype = wt.HANDLE
    k.ReadProcessMemory.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t,
                                    ctypes.POINTER(ctypes.c_size_t)]
    h = k.OpenProcess(0x410, False, ids[0])
    if not h:
        sys.exit('OpenProcess failed')

    def rd(address, size):
        buf = ctypes.create_string_buffer(size)
        got = ctypes.c_size_t()
        if not k.ReadProcessMemory(h, address, buf, size, ctypes.byref(got)) or got.value != size:
            return None
        return buf.raw

    last = object()
    end = time.time() + a.seconds
    while time.time() < end:
        raw = rd(base + FLAG_PTR, 8)
        ptr = struct.unpack('<Q', raw)[0] if raw else None
        flag = rd(ptr, 1)[0] if ptr and rd(ptr, 1) else None
        key = rd(base + SESSION_KEY, 4)
        key = struct.unpack('<I', key)[0] if key else None
        now = (ptr, flag, key)
        if now != last:
            print(time.strftime('%H:%M:%S'), f'flag ptr {ptr:#x}' if ptr else 'flag ptr -',
                  f'flag {flag}', f'session key {key:#x}' if key is not None else 'session key -', flush=True)
            last = now
        time.sleep(a.every)


if __name__ == '__main__':
    main()
