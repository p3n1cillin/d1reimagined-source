"""Account object (tag 0x80801803) with ONLY fields [0] [1] [3] (as d1_bap::account_rsat) plus [6] (account
inventory 0x80800FE6) and optionally [7] (0x808019FF); every other field ABSENT, so the client keeps its own
defaults (a full re-encode sends them present-and-zero: HUD opacity 0). Sub-objects are encoded by the client's
codec (d1-oracle object --tag SUB --obj-file) and spliced in bit by bit.
    build-account2.py OUT [SLOT:CONTENT:QTY ...] [f7:IDX:CONTENT:QTY ...]"""
import os, re, subprocess, sys
ORACLE = r'F:\d1\public-work\target-claude\release\d1-oracle.exe'
ENV = dict(os.environ, D1_BUILDS=r'F:\d1\d1preservation-main\capture-rig\builds.json', D1_BUILD='later-56578')
HERE = os.path.dirname(os.path.abspath(__file__))
QTY_FIELDS = tuple(int(x, 0) for x in os.environ.get('QTY_FIELDS', '0x10').split(','))  # entry offsets given the quantity
ACCT, CHR, PROF = 0xD100000000000001, 0xD100000000000101, 0xD100000000000201

def run(*a):
    r = subprocess.run([ORACLE, *a], env=ENV, capture_output=True, text=True, timeout=300)
    if r.returncode: sys.exit(f'oracle {a[:3]} failed: {r.stdout[-600:]} {r.stderr[-400:]}')
    return r.stdout

def sub_bits(tag, obj):
    src, out = os.path.join(HERE, f'sub-{tag}.obj'), os.path.join(HERE, f'sub-{tag}.bin')
    open(src, 'wb').write(obj)
    if os.path.exists(out): os.remove(out)
    o = run('object', '--tag', tag, '--obj-file', src, '--out', out)
    if 'IDENTICAL' not in o: sys.exit(f'{tag} round trip failed: {o[-400:]}')
    nbits = int(re.search(r'/ (\d+) bits', o).group(1))
    data = open(out, 'rb').read()
    return [(data[i // 8] >> (7 - i % 8)) & 1 for i in range(nbits)]

bits = []
put = lambda v, n: bits.extend((v >> i) & 1 for i in range(n - 1, -1, -1))
put(1, 1); put(ACCT, 64)                     # [0]
put(1, 1); put(1, 4); put(CHR, 64)           # [1]
put(0, 1)                                    # [2]
put(1, 1); put(PROF, 64)                     # [3]
put(0, 1); put(0, 1)                         # [4] [5]
inv = bytearray(10440)                       # [6] 0x80800FE6 {u32 serial; 0x80800FE8 @+8 = 326 x 32 B elements}
# 0x26D6E0 reads account slot s at account+0x4C8+32*s: elements start at +8; the descriptor's +0x20 is the stride.
for s in range(326): inv[8 + 32 * s:8 + 32 * s + 2] = b'\xff\xff'
f7 = bytearray(196); n7 = 0                  # [7] 0x808019FF {u16 count?; u16; 0x808058CB @+4, elements @+0xC of 12 B}
for spec in sys.argv[2:]:
    p = spec.split(':')
    if p[0] == 'f7':
        i, c, q = (int(x, 0) for x in p[1:]); e = 4 + 0xC + 12 * i
        f7[e:e + 2] = c.to_bytes(2, 'little'); f7[e + 4:e + 8] = q.to_bytes(4, 'little'); n7 = max(n7, i + 1)
    else:
        s, c, q = (int(x, 0) for x in p); e = 8 + 32 * s
        inv[e:e + 2] = c.to_bytes(2, 'little'); inv[e + 8:e + 16] = b'\0' * 8
        for f in QTY_FIELDS: inv[e + f:e + f + 4] = q.to_bytes(4, 'little')
put(1, 1); bits.extend(sub_bits('0x80800fe6', bytes(inv)))
if n7:
    f7[0:2] = n7.to_bytes(2, 'little')
    put(1, 1); bits.extend(sub_bits('0x808019ff', bytes(f7)))
else:
    put(0, 1)
bits.extend([0] * (25 - 8))                  # [8..24] absent
while len(bits) % 8: bits.append(0)
wire = bytes(int(''.join(map(str, bits[i:i + 8])), 2) for i in range(0, len(bits), 8))
open(sys.argv[1], 'wb').write(wire)
o = run('--json', 'object', '--tag', '0x80801803', '--decode-file', sys.argv[1])
import json
d = json.loads(next(l for l in o.splitlines() if l.startswith('{')))
nz = d['nonzero']
print(len(wire), 'B;', len(nz), 'nonzero qwords; outside fields 0-7:',
      [x for x in nz if int(x[0], 0) >= 0x2e4c][:5])
for spec in sys.argv[2:]:
    p = spec.split(':')
    off = 0x2d88 + 4 + 0xC + 12 * int(p[1], 0) if p[0] == 'f7' else 0x4c0 + 8 + 32 * int(p[0], 0)
    print(' ', spec, [x for x in nz if off <= int(x[0], 0) < off + 24])
