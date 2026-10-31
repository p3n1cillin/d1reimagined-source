import sys
src = open(r'F:\d1\tools\d1-winguest.py').read()
wg = {'__name__': 'lib'}; exec(compile(src, 'd1-winguest.py', 'exec'), wg)
p, base = wg['open_client']()
a = int(sys.argv[1], 16); n = int(sys.argv[2], 16) if len(sys.argv) > 2 else 0x100
rel = len(sys.argv) <= 3 or sys.argv[3] != 'abs'
b = p.read(base + a if rel else a, n)
for o in range(0, n, 8):
    q = int.from_bytes(b[o:o+8], 'little')
    tag = f'  -> +{q-base:#x}' if base <= q < base + 0x4000000 else ''
    print(f'{a+o:#x}: {q:016x}{tag}')
