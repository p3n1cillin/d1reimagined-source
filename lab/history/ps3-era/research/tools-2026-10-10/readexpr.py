"""Read-only: decode the urgent selector entry's unlock expression and its flag byte. usage: readexpr.py RUN"""
import struct, sys
sys.path.insert(0, r"F:\d1\d1xenia\scripts")
from rpcs3_owned_memory import OwnedMemory
run = sys.argv[1]
m = rf"F:\d1\<pc>-import\d1xenia\local\SIGNON_LOCAL\logs\FINAL124_{run}-processes.json"
with OwnedMemory(m) as mem:
    for entry, name in ((0x70092668, "urgent 0x80a58505"), (0x700926a0, "next 0x80a5e9fb")):
        e = mem.read(entry, 0x40)
        count, off = struct.unpack(">II", e[0x1c:0x24])
        arr = entry + 0x1c + off
        a = mem.read(arr, 0x14 + 8 * max(count, 1) + 8)
        ops = [(a[0x14 + 8 * i], struct.unpack(">h", a[0x14 + 8 * i + 6:0x14 + 8 * i + 8])[0]) for i in range(count)]
        print(name, "entry", e[0x18:0x40].hex(" ", 4), "expr count", count, "array", hex(arr), a.hex(" ", 4), "ops", ops)
        table = mem.read(0x600a1b9c, 0x2000)
        for op, idx in ops:
            if 0 <= idx < 0x2000: print("   flag", idx, hex(idx), "byte", table[idx])
