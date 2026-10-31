"""Build an account object (family 4 / type 0, tag 0x80801803) with account-inventory entries, encoded by
the client's own codec (d1-oracle object --obj). Field [6] @+0x4c0 = 0x80800fe6 {u32 serial; 0x80800fe8
array @+0x4c8, elements 0x808016e0 of 32 B from +0x20: u16 content, u64 soid, u32 a, u32 b, u32 c}.
    build-account.py OUT SLOT:CONTENT:QTY [...]      (QTY is written to element +0x10)"""
import json, os, subprocess, sys
ORACLE = r'F:\d1\public-work\target-claude\release\d1-oracle.exe'
ENV = dict(os.environ, D1_BUILDS=r'F:\d1\d1preservation-main\capture-rig\builds.json', D1_BUILD='later-56578')
ACCT, CHR, PROF = 0xD100000000000001, 0xD100000000000101, 0xD100000000000201

def bits_account():
    b = []
    put = lambda v, n: b.extend((v >> i) & 1 for i in range(n - 1, -1, -1))
    put(1, 1); put(ACCT, 64)                 # [0]
    put(1, 1); put(1, 4); put(CHR, 64)       # [1] count 1
    put(0, 1)                                # [2] absent
    put(1, 1); put(PROF, 64)                 # [3]
    b.extend([0] * 21)                       # [4..24] absent
    while len(b) % 8: b.append(0)
    return bytes(int(''.join(map(str, b[i:i + 8])), 2) for i in range(0, len(b), 8))

def oracle(*args, want_json=True):
    r = subprocess.run([ORACLE, '--json', 'object', '--tag', '0x80801803', *args], env=ENV, capture_output=True, text=True, timeout=300)
    line = next((l for l in r.stdout.splitlines() if l.startswith('{')), None)
    if not want_json:
        if r.returncode or 'IDENTICAL' not in r.stdout: sys.exit(f'encode failed: {r.stdout[-800:]} {r.stderr[-400:]}')
        return r.stdout
    if r.returncode or not line: sys.exit(f'oracle failed {r.returncode}: {r.stderr[-800:]} {r.stdout[-800:]}')
    return json.loads(line)

HERE = os.path.dirname(os.path.abspath(__file__))
base = oracle('--decode', bits_account().hex(), '--out-obj', os.path.join(HERE, 'acct-base.obj'))
obj = bytearray(open(os.path.join(HERE, 'acct-base.obj'), 'rb').read()); size = len(obj)
print('decoded baseline', size, 'bytes, nonzero', base['nonzero'][:8])
ARR = 0x4c8 + 0x20
RAW = [a for a in sys.argv[2:] if a.startswith('@')]
SLOTS = [a for a in sys.argv[2:] if not a.startswith('@')]
for spec in RAW:                      # @OFFSET:SIZE:VALUE, little-endian, into the C object
    off, size, val = (int(x, 0) for x in spec[1:].split(':'))
    obj[off:off + size] = val.to_bytes(size, 'little')
for spec in SLOTS:
    slot, content, qty = (int(x, 0) for x in spec.split(':'))
    e = ARR + 32 * slot
    obj[e:e + 2] = content.to_bytes(2, 'little'); obj[e + 0x10:e + 0x14] = qty.to_bytes(4, 'little')
# unused slots: content 0xFFFF like the profile's empty slots
for s in range(325):
    e = ARR + 32 * s
    if obj[e:e + 2] == b'\0\0' and not any(f'{s}:' == x.split(':')[0] + ':' for x in SLOTS): obj[e:e + 2] = b'\xff\xff'
open(os.path.join(HERE, 'acct-new.obj'), 'wb').write(obj)
if os.path.exists(sys.argv[1]): os.remove(sys.argv[1])
enc = oracle('--obj-file', os.path.join(HERE, 'acct-new.obj'), '--out', sys.argv[1], want_json=False); print(enc.strip().splitlines()[-1])
wire = open(sys.argv[1], 'rb').read()
back = oracle('--decode-file', sys.argv[1])
print('wire', len(wire), 'B; re-decoded nonzero entries', len(back['nonzero']))
for spec in RAW:
    off = int(spec[1:].split(':')[0], 0)
    print(' raw', hex(off), [(o, v) for o, v in back['nonzero'] if off - 8 < int(o, 0) <= off + 8])
for spec in SLOTS:
    slot = int(spec.split(':')[0], 0); e = ARR + 32 * slot
    hits = [(o, v) for o, v in back['nonzero'] if e <= int(o, 0) < e + 32]
    print(' slot', slot, hits)
print('wrote', sys.argv[1])
