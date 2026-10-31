"""Read-only: every MOTD-selector group entry with its dialog handle, unlock-expression ops and flag
bytes. Group data addresses come from 0x48a570 (r7) in JOIN247. usage: dialogflags.py RUN"""
import struct, sys
sys.path.insert(0, r"F:\d1\d1xenia\scripts")
from rpcs3_owned_memory import OwnedMemory
GROUPS = [0x70090704, 0x70181014, 0x70090dc4, 0x70181d34, 0x70091634, 0x70093ff4, 0x700942b4, 0x70096ba4]
m = rf"F:\d1\<pc>-import\d1xenia\local\SIGNON_LOCAL\logs\FINAL124_{sys.argv[1]}-processes.json"
u32 = lambda mem, a: struct.unpack(">I", mem.read(a, 4))[0]
def ops(mem, obj):
    out = []
    cnt, off = struct.unpack(">II", mem.read(obj, 8))
    if not cnt or cnt > 8: return out
    arr = obj + off
    hdr = mem.read(arr, 0x14)
    if hdr[8:12] == bytes.fromhex("808016e4"):   # wrapper: array of expression objects
        for i in range(struct.unpack(">I", hdr[4:8])[0]):
            out += ops(mem, arr + 0x14 + 12 * i)
        return out
    for i in range(cnt):
        e = mem.read(arr + 0x14 + 8 * i, 8)
        out.append((e[0], struct.unpack(">h", e[6:8])[0]))
    return out
SEEDS = [0x70092668, 0x7011e874 - 0x1c, 0x702a0854 - 0x1c, 0x702a10c4 - 0x1c, 0x702a01cc - 0x1c]
def valid(mem, e):
    try: d = mem.read(e, 0x4c)
    except OSError: return False
    return d[0x44] == 0x80 and d[0x48:0x4c] == bytes.fromhex("811c9dc5")
with OwnedMemory(m) as mem:
    table = mem.read(0x600a1b9c, 0x2000)
    seen = set()
    for s in SEEDS:
        lo = s
        while valid(mem, lo - 0x38): lo -= 0x38
        e = lo; run = []
        while valid(mem, e): run.append(e); e += 0x38
        if lo in seen: continue
        seen.add(lo)
        print(f"group run @{lo:#x}: {len(run)} entries")
        for e in run:
            d = mem.read(e, 0x48)
            handle = struct.unpack(">I", d[0x44:0x48])[0]
            ctx = struct.unpack(">h", d[0x18:0x1a])[0]
            slot = d[0x30] if d[0x30] != 0xff else -1
            cool = struct.unpack(">Q", d[0x38:0x40])[0]
            try: o = ops(mem, e + 0x1c)
            except Exception as x: o = [("err", str(x)[:30])]
            flags = [(op, idx, table[idx] if isinstance(idx, int) and 0 <= idx < 0x2000 else None) for op, idx in o]
            print(f"  {e:#x} {handle:#010x} ctx {ctx} slot {slot} cooldown {cool} ops {flags}")
