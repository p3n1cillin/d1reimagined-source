"""Read-only observations for run 12. Fails on another boot/group; no guest writes."""
import importlib.util, json, struct
from pathlib import Path
spec = importlib.util.spec_from_file_location('guest', r'F:\d1\public-work\d1reimagined-source\tools\ps4_setup\d1-winguest.py')
g = importlib.util.module_from_spec(spec)
spec.loader.exec_module(g)
p, base = g.open_client()
assert base == 0x5130000, 'Another native boot; rederive addresses'
reg = p.u(p.u(base + g.REGISTRY, 'Q'), 'Q')
def resolve(tag):
    esi = struct.unpack('<i', struct.pack('<I', tag))[0] >> 13
    slot = (((esi & 0xFFFFFFFF) >> 0x12) | 0x3FF) & esi & 0xFFFFFFFF
    rec = reg + slot * 0x40
    data, esz = p.u(rec+0x10,'Q'), p.u(rec+0x30,'i')
    mask = p.u(rec+0x34,'i') & 0xFFFFFFFFFFFFFFFF
    assert 0 < esz <= 0x100000
    e = data + (tag & 0x1FFF)*esz
    return e - (mask & p.u(e+8,'Q'))
client = 0x380571500
assert p.u(client,'I') == 0x9243DFFB and p.u(client+6,'h') == 20
tag = p.u(client+0x68,'I')
print('squad20 client +0x68',hex(tag),'not an established object handle')
run = Path(r'F:\d1\d1preservation-main\cfw-staging\2026-10-27-divide12')
watch = [json.loads(line) for line in (run/'squad-authority-watch.jsonl').read_text().splitlines()]
components = sorted({int(row['rdi'],0) for row in watch if row.get('rip_img') == '0x102b60'})
table = base + g.LT_TABLE
for component in components:
    sensor = resolve(p.u(component+0x1A8,'I'))
    name = struct.unpack('<IBxh', p.read(sensor,8))
    arena = p.u(table+p.u(table,'q')+p.u(sensor+0x40,'H')*p.u(table+0x10,'Q')+0x30,'Q')
    state = arena + p.u(sensor+0x38,'Q')
    b = p.read(state,124)
    record = resolve(p.u(component,'I')) + p.u(component+8,'Q')
    definition = resolve(p.u(record+0x68,'I')) + p.u(record+0x70,'Q')
    print('native squad',name,'gen',struct.unpack_from('<I',b,0x44)[0],
          'variant count',struct.unpack_from('<I',b,0x2C)[0], 'desired',struct.unpack_from('<h',b,0x30)[0],
          'modes', b[0x74:0x76].hex(), 'state address',hex(state),
          'effective desired',struct.unpack('<8h',p.read(component+0x430,16)),
          'existing',struct.unpack('<8i',p.read(component+0x1CC,32)),
          'definition variants',p.u(definition+0x20,'Q'), 'definition',hex(definition))
cache = p.u(p.u(base+0x273EA80,'Q')+8,'Q')
print('cached primary equipment', struct.unpack('<QHHH',p.read(cache+0xE40+6*64,14)))
world = 0x37FB1A200
print('world authority held',p.read(world+0xA70,12).hex(), 'pending',p.read(world+0xA7C,12).hex())
for name, at in [('primary',0x14E0),('special',0x1560),('heavy',0x15E0)]:
    raw = p.read(cache+at,0x80)
    print(name,'cached stat pairs',[(struct.unpack_from('<b',raw,i)[0],struct.unpack_from('<I',raw,i+4)[0]) for i in range(0,0x80,8)])
