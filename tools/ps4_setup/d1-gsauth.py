"""SYNTHETIC diagnosis patch: make 0xBB89F0 ("this machine is game-state authority") return true.

    gsauth.py apply | revert | show
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import d1paths  # sibling: local paths from D1_ROOT or paths.json
src = open(d1paths.tools() / 'd1-winguest.py', encoding='utf-8').read().split('def main():')[0]
g = {}
exec(compile(src, 'wg', 'exec'), g)
p, base = g['open_client']()
SITE = 0xBB89F0
ORIG = bytes.fromhex('554889e5e8')     # push rbp; mov rbp,rsp; call ...
NEW = bytes.fromhex('b001c39090')      # mov al,1; ret; nop; nop
cur = p.read(base + SITE, 5)
print(f'{SITE:#x}: {cur.hex()}')
cmd = sys.argv[1] if len(sys.argv) > 1 else 'show'
if cmd == 'apply' and cur == ORIG:
    g['patch_code'](p, base + SITE, NEW)
elif cmd == 'revert' and cur == NEW:
    g['patch_code'](p, base + SITE, ORIG)
elif cmd != 'show' and cur not in (ORIG, NEW):
    sys.exit('unexpected bytes; nothing written')
print(f'{SITE:#x}: now {p.read(base + SITE, 5).hex()}')
