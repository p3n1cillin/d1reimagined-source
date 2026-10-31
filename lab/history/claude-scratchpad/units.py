"""Read-only: list objects in the client's object table whose three transform copies agree (unit layout),
near the local Guardian. units.py [radius]"""
import importlib.util, math, os, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
SENSE = os.path.join(HERE, 'pr57')
sys.path.insert(0, SENSE)
s = importlib.util.spec_from_file_location('g', os.path.join(SENSE, 'd1-winguest.py')); g = importlib.util.module_from_spec(s); s.loader.exec_module(g)
import d1_unit_tracking as ut
p, base = g.open_client()
me = ut.read_unit_position(p, base)
table = base + ut.OBJECT_TABLE
entries = p.u(table + 8, 'Q'); stride = p.u(table + 0x10, 'I'); mask = p.u(table + 0x28, 'i') & ut.U64_MASK
raw = p.read(entries, 8192 * stride)
R = float(sys.argv[1]) if len(sys.argv) > 1 else 80
out = []
for slot in range(8192):
    e8 = struct.unpack_from('<Q', raw, slot * stride + 8)[0]
    if not e8: continue
    unit = (entries + slot * stride - (mask & e8)) & ut.U64_MASK
    try:
        h = p.u(unit + ut.SELF_HANDLE, 'I')
        if h & 0x1FFF != slot: continue
        pts = [struct.unpack('<3f', p.read(unit + o, 12)) for o in ut.TRANSFORMS]
    except OSError:
        continue
    if not all(math.isfinite(v) and abs(v) < 1e5 for pt in pts for v in pt): continue
    if any(math.dist(pts[0], q) > 0.05 for q in pts[1:]): continue
    d = math.dist(pts[0], me.position)
    if d > R: continue
    tag = p.u(unit + 0x1C, 'I') if True else 0
    hdr = p.read(unit, 0x20).hex()
    out.append((round(d, 1), hex(h), hex(unit), tuple(round(v, 1) for v in pts[0]), hdr[:48]))
print('me', hex(me.handle), tuple(round(v, 1) for v in me.position))
for r in sorted(out): print(*r)
