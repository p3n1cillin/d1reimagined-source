"""Bounded owned activity table reads; READ + QUERY process rights only, no input."""
import ctypes, ctypes.wintypes as wt, subprocess, re, struct, json, time
import sys
from pathlib import Path
P = Path(__file__).parent
sys.path.insert(0, str(P)); import routepaths  # noqa: E402
listing = subprocess.run(['tasklist','/FI','IMAGENAME eq shadPS4.exe','/FO','CSV','/NH'], capture_output=True, text=True).stdout
ids = [int(line.split('\",\"')[1]) for line in listing.splitlines() if line.startswith('\"shadPS4')]
assert len(ids) == 1
log = routepaths.shad_log().read_bytes()[:2000000].decode('utf8','replace')
bases = re.findall(r'Loading module eboot\.bin to (0x[0-9a-f]+)', log)
assert len(bases) == 1
base = int(bases[0],16)
k = ctypes.WinDLL('kernel32',use_last_error=True)
k.OpenProcess.restype = wt.HANDLE
k.ReadProcessMemory.argtypes = [wt.HANDLE,ctypes.c_void_p,ctypes.c_void_p,ctypes.c_size_t,ctypes.POINTER(ctypes.c_size_t)]
handle = k.OpenProcess(0x410,False,ids[0])
assert handle
def rd(address, size):
    buf = ctypes.create_string_buffer(size)
    got = ctypes.c_size_t()
    assert k.ReadProcessMemory(handle,address,buf,size,ctypes.byref(got)) and got.value == size, (hex(address),size)
    return buf.raw
def u(address, fmt='Q'):
    return struct.unpack('<'+fmt,rd(address,struct.calcsize(fmt)))[0]
assert base <= u(base+0x19474b0) < base+0x18d8000
