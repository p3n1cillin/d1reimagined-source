"""Check candidate facing offsets against the direction of a forward walk."""
import importlib.util, json, math, struct, sys
spec = importlib.util.spec_from_file_location('walk', r'F:\d1\public-work\d1reimagined-source\tools\ps4_setup\d1-walk.py')
walk = importlib.util.module_from_spec(spec); spec.loader.exec_module(walk)
c = walk.Client()
tracked = json.load(open(sys.argv[1]))
OFFS = [0x200, 0x110, 0x130, 0x38, 0xc0, -0x30, -0x10]


def facings():
    out = {}
    for a in tracked:
        for o in OFFS:
            try:
                x, y = struct.unpack('<2f', c.p.read(a + o, 8))
            except OSError:
                continue
            if math.isfinite(x) and math.isfinite(y) and abs(math.hypot(x, y) - 1) < 0.02:
                out.setdefault(o, []).append(math.degrees(math.atan2(y, x)))
    return {o: sorted(v)[len(v) // 2] for o, v in out.items()}


p0 = walk.position(c, tracked)
f = facings()
walk.pad('Hold:W:1500')
p1 = walk.position(c, tracked, p0)
move = math.degrees(math.atan2(p1[1] - p0[1], p1[0] - p0[0]))
print(f'moved {math.dist(p0[:2], p1[:2]):.1f} toward {move:.0f} deg')
for o, v in f.items():
    print(f'offset {o:+#x}: facing {v:.0f} deg, error {((v - move + 180) % 360) - 180:+.0f}')
