"""List each live squad's template spawn points; read-only process access, no input.

    squad-templates-readonly.py [--group 0xfc0e45a3]
For every squad in the current content's squad list it prints the sensor name (group, index), the
template's inline spawn-point count (template +0x40), the squad's own spawn-node head (+0x3D8, -1 =
none) and its +0x250 reference. The template is what 0x1055E0 returns:
P = squad[+8] + resolve(u32 squad[+0]); template = resolve(u32 P[+0x68]) + u64 P[+0x70].
The spawn picker 0x179FB0 walks the node list at list +0x188 (squad +0x3D8), filled at squad init
(0x177990) from the template's points, unless the squad state A +0x50 names another object.
"""
import argparse, importlib.util, os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, HERE); import routepaths  # noqa: E402
SENSE = str(routepaths.SETUP)
sys.path.insert(0, SENSE)
s = importlib.util.spec_from_file_location('g', os.path.join(SENSE, 'd1-winguest.py')); g = importlib.util.module_from_spec(s); s.loader.exec_module(g)
from d1_squad_tracking import SquadReader  # noqa: E402

ap = argparse.ArgumentParser()
ap.add_argument('--group', type=lambda v: int(v, 0))
a = ap.parse_args()
p, base = g.open_client()
sr = SquadReader(p, base)
content = sr.content()[1]
start, count = struct.unpack('<hh', p.read(content + 0x2BD1C + 4, 4))
assert 0 <= start < 8192 and 0 <= count <= 512, 'invalid squad component list'
print(f'{count} squads')
for j in range(count):
    dx = p.u(content + 0x2A51C + 2 * (start + j), 'H')
    rec = content + 0x510 + dx * 0x38
    try:
        comp = sr.resolve(p.u(rec + 16, 'I')) + p.u(rec + 24, 'Q')
        h, kind, index = struct.unpack('<IBxh', p.read(sr.resolve(p.u(comp + 0x1A8, 'I')), 8))
        if a.group is not None and h != a.group:
            continue
        P = p.u(comp + 8, 'Q') + sr.resolve(p.u(comp, 'I'))
        tmpl = sr.resolve(p.u(P + 0x68, 'I')) + p.u(P + 0x70, 'Q')
        points = p.u(tmpl + 0x40, 'q')
        head = p.u(comp + 0x3D8, 'i')
        ref = struct.unpack('<IBxH', p.read(comp + 0x250, 8))
        print(f'{h:#010x} kind {kind} index {index:3d}  template points {points:3d}  head {head:#x}  '
              f'+0x250 ref {ref[0]:#x}/{ref[1]:#x}/{ref[2]:#x}  members {p.u(comp + 0x278, "i")}')
    except Exception as e:  # a squad mid-teardown; report and go on
        print(f'entry {j}: {e}')
