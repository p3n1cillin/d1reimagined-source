import sys, capstone, bisect
sys.path.insert(0, 'scripts')
from d1_eboot import Eboot
eb = Eboot('../d1xenia-data/ps3_updates/01.24/USRDIR/EBOOT.elf')
md = capstone.Cs(capstone.CS_ARCH_PPC, capstone.CS_MODE_64 | capstone.CS_MODE_BIG_ENDIAN); md.skipdata = True
starts = sorted(eb.function_starts())
for arg in sys.argv[1:]:
    a = int(arg.split(':')[0], 16)
    if ':' in arg:
        n = int(arg.split(':')[1]); s = a
    else:
        s = starts[bisect.bisect_right(starts, a) - 1]
        e = starts[bisect.bisect_right(starts, a)]
        n = (e - s) // 4
    print(f'== {s:#x} ({n} insns)')
    for i in md.disasm(eb.read(s, n * 4), s):
        print(f'{i.address:08x}  {i.mnemonic:8s} {i.op_str}')
