#!/usr/bin/env python3
"""SYNTHETIC test of gate leg A on build 56578: give one type-12 client bundle (the first) our
player's identity (entry+0x18 -> bundle+0x258) and set the gate/task-9 bytes (bundle+0x220/+0x221),
then report player-array entry+0x50, the spawn word and the biped handle. Dry run without --confirm."""
import os, sys, struct, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import d1paths  # sibling: local paths from D1_ROOT or paths.json
sys.argv += []
src = open(d1paths.tools() / 'd1-winguest.py', encoding='utf-8').read().split('def main():')[0]
g = {}; exec(src, g)
p, b = g['open_client']()
T = b + 0x1AD5000
idx = p.u(b + 0x25D6FF8, 'H')
content = p.u(T + p.u(T, 'Q') + idx * p.u(T + 0x10, 'Q') + 0x30, 'Q')
first = p.u(content + 0xc * 4 + 0x2bd1c, 'h')
reg = p.u(p.u(b + g['REGISTRY'], 'Q'), 'Q')
def resolve(tag):
    esi = struct.unpack('<i', struct.pack('<I', tag))[0] >> 13
    sl = (((esi & 0xFFFFFFFF) >> 0x12) | 0x3FF) & esi & 0xFFFFFFFF
    rec = reg + sl * 0x40
    d, es, mk = p.u(rec + 0x10, 'Q'), p.u(rec + 0x30, 'i'), p.u(rec + 0x34, 'i') & 0xFFFFFFFFFFFFFFFF
    e = d + (tag & 0x1FFF) * es
    return e - (mk & p.u(e + 8, 'Q'))
dx = p.u(content + first * 2 + 0x2a51c, 'H')
tag = p.u(content + dx * 0x38 + 0x520, 'I')
obj = resolve(tag)
PA = b + 0x2689580; entry = p.u(PA + 8, 'Q')
key = p.read(entry + 0x18, 8)
r13 = p.u(p.u(b + g['PLAYER_POOL'], 'Q') + 8, 'Q')
def state(tagline):
    print(f'{tagline}: entry+0x50 {p.u(entry + 0x50, "I"):#x}  bundle+0x218 {p.read(obj + 0x218, 16).hex()}  '
          f'+0x258 {p.read(obj + 0x258, 8).hex()}  spawn {p.u(r13 + 0xC, "I"):#x}  biped {p.u(r13 + 0x644, "I"):#x}')
print(f'bundle tag {tag:#x} obj {obj:#x}; player key {key.hex()}')
state('before')
if '--confirm' not in sys.argv:
    sys.exit('dry run (add --confirm)')
p.write(obj + 0x258, key); p.write(obj + 0x220, b'\x01\x01')
print('wrote bundle+0x258 = key, +0x220/+0x221 = 1  (SYNTHETIC)')
for i in range(6):
    time.sleep(5); state(f't+{5 * (i + 1)}s')
