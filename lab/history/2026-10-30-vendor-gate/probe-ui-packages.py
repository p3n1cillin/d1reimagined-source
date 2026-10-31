"""Read-only: the client's loaded UI packages (0xac5c90 container, iterated by 0xac47e0) and whether a
screen hash is registered in any of them (0xac5ff0's sorted 24-byte entries). READ + QUERY only."""
import runpy, struct, sys, json, time
from pathlib import Path
P = Path(__file__).parent
v = runpy.run_path(str(P / 'attach-readonly.py')); u, rd, b = v['u'], v['rd'], v['base']
WANT = [int(x, 16) for x in (sys.argv[1:] or ['c2ddef93'])]

def resolve(h):
    e = h >> 13 if h < 0x80000000 else (h - (1 << 32)) >> 13
    s = ((e & 0xffffffff) >> 18 | 0x3ff) & e & 0xffffffff
    ent = u(u(b + 0x1a238a0)) + s * 64; data = u(ent + 0x10); stride = u(ent + 0x30, 'i'); mask = u(ent + 0x34, 'i') & 0xffffffffffffffff
    p = data + (h & 0x1fff) * stride; return p - (mask & u(p + 8))

assert u(b + 0x279d290, 'B'), 'UI package container not initialised'
C = (b + 0x279d15f) & ~7
n = u(C + 0xb8)
first = (C + 0x1f) & ~7
out = {'time': time.time(), 'count': n, 'packages': []}
for i in range(n):
    e = first + 32 * i
    h = u(e + 0x18, 'I'); live = u(e + 0x1c, 'B') == 0
    if h == 0xffffffff:
        continue
    pkg = {'slot': i, 'handle': hex(h), 'live': live}
    try:
        p = resolve(h) + 0x10
        cnt = u(p); base = u(p + 8) + (p + 8) + 0x10
        hashes = [u(base + 24 * k, 'I') for k in range(min(cnt, 4096))]
        pkg['screens'] = cnt; pkg['has'] = {f'{w:08x}': (w in hashes) for w in WANT}
        pkg['sample'] = [f'{x:08x}' for x in hashes[:6]]
    except (AssertionError, OSError) as ex:
        pkg['error'] = str(ex)
    out['packages'].append(pkg)
print(json.dumps(out, indent=1))
(P / f'ui-packages-{time.time_ns()}.json').write_text(json.dumps(out, indent=1))
