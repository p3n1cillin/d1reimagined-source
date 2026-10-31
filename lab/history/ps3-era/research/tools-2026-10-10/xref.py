import sys, capstone, bisect
sys.path.insert(0, 'scripts')
from d1_eboot import Eboot
eb = Eboot('../d1xenia-data/ps3_updates/01.24/USRDIR/EBOOT.elf')
md = capstone.Cs(capstone.CS_ARCH_PPC, capstone.CS_MODE_64 | capstone.CS_MODE_BIG_ENDIAN); md.skipdata = True
starts = sorted(eb.function_starts())
fn = lambda a: starts[bisect.bisect_right(starts, a) - 1]
targets = {int(x, 16) for x in sys.argv[1:]}
start, end = 0x10000, 0xd98000
raw = eb.read(start, end - start)
import struct
for off in range(0, len(raw), 4):
    w = struct.unpack_from('>I', raw, off)[0]
    if w >> 26 == 18:
        li = w & 0x3fffffc
        if li & 0x2000000: li -= 0x4000000
        a = start + off
        t = li if w & 2 else a + li
        if t in targets:
            print(f'{a:#x} -> {t:#x} (fn {fn(a):#x}){" link" if w & 1 else ""}')
