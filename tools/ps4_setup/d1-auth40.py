"""Hold global-state authority key 0x40 as LOCAL on activity client 0 (a70 bit set, a7c bit clear).

0xBA0B40 -> 0x2A1A80(ac, key 0x40): local authority = bit set in ac+0xA70 and clear in ac+0xA7C.
With it, 0x907930 (omaha world-controller shim [4], at state transitions) starts the game engine
as host (0x90A9A0, 0x907A40) when engine globals +0x700 == 0.
    auth40.py [seconds] [--read-only]
"""
import os, struct, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import d1paths  # sibling: local paths from D1_ROOT or paths.json
src = open(d1paths.tools() / 'd1-winguest.py', encoding='utf-8').read().split('def main():')[0]
g = {}
exec(compile(src, 'wg', 'exec'), g)
p, base = g['open_client']()
secs = float(sys.argv[1]) if len(sys.argv) > 1 and not sys.argv[1].startswith('-') else 120
ro = '--read-only' in sys.argv
KEY = 0x40
word, bit = (KEY >> 5) * 4, 1 << (KEY & 31)
last = None
end = time.time() + secs
while time.time() < end:
    try:
        pool = p.u(base + 0x26556D8, 'Q')
        ac = pool
        a70, a7c = p.u(ac + 0xA70 + word, 'I'), p.u(ac + 0xA7C + word, 'I')
        gl = p.u(base + 0x273E5C0, 'Q')
        v700 = p.u(gl + 0x700, 'I') if gl else None
    except OSError:
        time.sleep(0.05); continue
    cur = (hex(ac), hex(a70), hex(a7c), v700)
    if cur != last:
        print(time.strftime('%X'), 'ac', cur[0], 'a70[2]', cur[1], 'a7c[2]', cur[2], 'engine+0x700', cur[3], flush=True)
        last = cur
    if not ro and pool and (not a70 & bit or a7c & bit):
        try:
            p.write(ac + 0xA70 + word, struct.pack('<I', a70 | bit))
            p.write(ac + 0xA7C + word, struct.pack('<I', a7c & ~bit))
        except OSError:
            pass
    time.sleep(0.01)
