"""Find the Guardian's facing near its tracked position copies: unit (x, y) pairs that rotate with a turn.

    find-yaw.py TRACKED.json   (turns the camera right about 90 degrees once; read-only otherwise)
"""
import importlib.util, json, math, struct, sys, time
spec = importlib.util.spec_from_file_location('walk', r'F:\d1\public-work\d1reimagined-source\tools\ps4_setup\d1-walk.py')
walk = importlib.util.module_from_spec(spec); spec.loader.exec_module(walk)
c = walk.Client()
tracked = json.load(open(sys.argv[1]))
SPAN = 0x400


def snap():
    out = {}
    for a in tracked:
        try:
            out[a] = c.p.read(a - SPAN, 2 * SPAN)
        except OSError:
            pass
    return out


def pairs(buf):
    f = struct.unpack(f'<{len(buf) // 4}f', buf)
    return {i: (f[i], f[i + 1]) for i in range(len(f) - 1)
            if all(math.isfinite(v) for v in f[i:i + 2]) and abs(math.hypot(f[i], f[i + 1]) - 1) < 0.02}


before = snap()
walk.pad('Hold:L:%d' % int(math.radians(90) / (2.26 / 1000 * 1.14)))
time.sleep(0.6)
after = snap()
hits = {}
for a, b0 in before.items():
    if a not in after:
        continue
    p0, p1 = pairs(b0), pairs(after[a])
    for i in p0.keys() & p1.keys():
        a0, a1 = math.atan2(p0[i][1], p0[i][0]), math.atan2(p1[i][1], p1[i][0])
        d = math.degrees((a1 - a0 + math.pi) % (2 * math.pi) - math.pi)
        if 40 < abs(d) < 140:
            off = i * 4 - SPAN
            hits.setdefault(off, []).append((round(d), a))
for off, v in sorted(hits.items(), key=lambda kv: -len(kv[1]))[:12]:
    print(f'offset {off:+#x}: {len(v)} copies, rotation {sorted(set(d for d, _ in v))[:6]}')
