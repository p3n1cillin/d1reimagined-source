"""Scan the live client's eboot image for 8-byte pointers to the given eboot-relative targets."""
import sys, struct
src = open(r'F:\d1\tools\d1-winguest.py').read()
wg = {'__name__': 'lib'}; exec(compile(src, 'd1-winguest.py', 'exec'), wg)
p, base = wg['open_client']()
targets = {base + int(a, 16): a for a in sys.argv[1:]}
lo, hi = int(__import__("os").environ.get("LO","0x1800000"),16), int(__import__("os").environ.get("HI","0x2A00000"),16)
step = 0x10000
for off in range(lo, hi, step):
    try:
        b = p.read(base + off, step)
    except Exception:
        continue
    for t, name in targets.items():
        pat = struct.pack('<Q', t); j = b.find(pat)
        while j >= 0:
            if (j & 7) == 0: print(name, 'at', hex(off + j))
            j = b.find(pat, j + 1)
