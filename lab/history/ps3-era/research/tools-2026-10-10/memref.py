import sys, struct, bisect
sys.path.insert(0, 'scripts')
from d1_eboot import Eboot
eb = Eboot('../d1xenia-data/ps3_updates/01.24/USRDIR/EBOOT.elf')
starts = sorted(eb.function_starts()); fn = lambda a: starts[bisect.bisect_right(starts, a) - 1]
targets = {int(x, 16) for x in sys.argv[1:]}
start, end = 0x10000, 0xd98000
raw = eb.read(start, end - start)
his = {}
for off in range(0, len(raw), 4):
    w = struct.unpack_from('>I', raw, off)[0]
    op, rt, ra, imm = w >> 26, (w >> 21) & 31, (w >> 16) & 31, w & 0xffff
    if op == 15 and ra == 0: his[rt] = (imm << 16, start + off); continue
    if op in (32, 36, 14) and ra in his:   # lwz, stw, addi
        hi, at = his[ra]
        val = (hi + (imm - 0x10000 if imm & 0x8000 else imm)) & 0xffffffff
        if val in targets and start + off - at < 96:
            print(hex(val), {32: 'lwz', 36: 'stw', 14: 'addi'}[op], 'at', hex(start + off), 'fn', hex(fn(start + off)))
