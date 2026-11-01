"""Search the running shadPS4's committed memory for a byte pattern; READ + QUERY process rights only, no input.

    find-bytes-readonly.py HEX [--before N] [--after N] [--max 20]
Prints each hit's address and the bytes around it (--before/--after). Run sense91 used it to find the refund
record the client kept from account field [20]: its expiry and purchase stamp, two u64s side by side.
"""
import argparse, ctypes, ctypes.wintypes as wt, subprocess

ap = argparse.ArgumentParser()
ap.add_argument('hex')
ap.add_argument('--before', type=int, default=0)
ap.add_argument('--after', type=int, default=0)
ap.add_argument('--max', type=int, default=20)
a = ap.parse_args()
pattern = bytes.fromhex(a.hex)

listing = subprocess.run(['tasklist', '/FI', 'IMAGENAME eq shadPS4.exe', '/FO', 'CSV', '/NH'], capture_output=True, text=True).stdout
ids = [int(line.split('","')[1]) for line in listing.splitlines() if line.startswith('"shadPS4')]
assert len(ids) == 1, 'expected one shadPS4'
k = ctypes.WinDLL('kernel32', use_last_error=True)
k.OpenProcess.restype = wt.HANDLE
k.ReadProcessMemory.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]


class MBI(ctypes.Structure):
    _fields_ = [('BaseAddress', ctypes.c_void_p), ('AllocationBase', ctypes.c_void_p), ('AllocationProtect', wt.DWORD),
                ('PartitionId', wt.WORD), ('RegionSize', ctypes.c_size_t), ('State', wt.DWORD), ('Protect', wt.DWORD),
                ('Type', wt.DWORD)]


k.VirtualQueryEx.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.POINTER(MBI), ctypes.c_size_t]
k.VirtualQueryEx.restype = ctypes.c_size_t
handle = k.OpenProcess(0x410, False, ids[0])   # PROCESS_QUERY_INFORMATION | PROCESS_VM_READ
assert handle


def rd(address, size):
    buf = ctypes.create_string_buffer(size)
    got = ctypes.c_size_t()
    ok = k.ReadProcessMemory(handle, address, buf, size, ctypes.byref(got))
    return buf.raw[:got.value] if ok or got.value else b''


READABLE = 0x02 | 0x04 | 0x08 | 0x20 | 0x40 | 0x80   # PAGE_READONLY .. PAGE_EXECUTE_WRITECOPY
CHUNK = 64 << 20
hits, address, mbi = 0, 0, MBI()
while hits < a.max and k.VirtualQueryEx(handle, address, ctypes.byref(mbi), ctypes.sizeof(mbi)):
    start, size = mbi.BaseAddress or 0, mbi.RegionSize
    if mbi.State == 0x1000 and mbi.Protect & READABLE and not mbi.Protect & 0x100:   # committed, readable, no guard
        for off in range(0, size, CHUNK):
            data = rd(start + off, min(CHUNK + len(pattern) - 1, size - off))
            i = data.find(pattern)
            while i >= 0 and hits < a.max:
                at = start + off + i
                print(f'{at:#x}')
                if a.before or a.after:
                    ctx = rd(at - a.before, a.before + len(pattern) + a.after)
                    for row in range(0, len(ctx), 16):
                        print(f'  {at - a.before + row:#x}  {ctx[row:row + 16].hex(" ")}')
                hits += 1
                i = data.find(pattern, i + 1)
    address = start + size
print(f'{hits} hit(s)')
