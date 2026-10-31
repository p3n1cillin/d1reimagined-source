"""Address of a sensor client's state-A image (group kind sub), using the run-14 readback layout. Read-only."""
import importlib.util, struct, sys
s = importlib.util.spec_from_file_location('g', r'F:\d1\public-work\d1reimagined-source\tools\ps4_setup\d1-winguest.py')
g = importlib.util.module_from_spec(s); s.loader.exec_module(g)
import io, contextlib
p, base = g.open_client()
want = (int(sys.argv[1], 0), int(sys.argv[2], 0), int(sys.argv[3], 0))
buf = io.StringIO()
with contextlib.redirect_stdout(buf):
    sys.argv = ['x', 'worldset']; g.main()
import re
table = base + g.LT_TABLE
for line in buf.getvalue().splitlines():
    m = re.search(r'tag (0x[0-9a-f]+) obj (0x[0-9a-f]+)', line)
    if not m: continue
    obj = int(m.group(2), 16)
    name, kind, sub = struct.unpack('<IBxh', p.read(obj, 8))
    if (name, kind, sub) != want: continue
    arena = p.u(table + p.u(table, 'q') + p.u(obj + 0x40, 'H') * p.u(table + 0x10, 'Q') + 0x30, 'Q')
    img = arena + p.u(obj + 0x38, 'Q')
    raw = p.read(img, 0x70)
    print(f'obj {obj:#x} image {img:#x}'); print(raw[0:8].hex(), raw[0x5c:0x68].hex())
