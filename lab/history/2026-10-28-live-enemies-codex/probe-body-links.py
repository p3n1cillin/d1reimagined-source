import importlib.util,struct,json,time
from pathlib import Path
spec=importlib.util.spec_from_file_location('g',r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-winguest.py');g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
p,base=g.open_client()
T=base+0x26897f0;entries=p.u(T+8,'Q');stride=p.u(T+16,'I');mask=p.u(T+0x28,'i')&0xffffffffffffffff
assert stride==32
out=[]
for i in range(8192):
    e=entries+i*32
    try:
        o=(e-(mask&p.u(e+8,'Q')))&0xffffffffffffffff
        head=p.u(o,'I')
        if head not in (0x809dcef6,0x809dcce5,0x809de137,0x809ddc8b):continue
        b=p.read(o,0x1200)
        row=dict(address=hex(o),handle=hex(p.u(o+0x24,'I')),header=hex(head),position=list(struct.unpack_from('<3f',b,0x1a0)))
        (Path(__file__).parent/('body-%x.bin'%o)).write_bytes(b)
        out.append(row)
    except OSError:continue
dest=Path(__file__).parent/('bodies-%d.json'%time.time());dest.write_text(json.dumps(out,indent=2));print(dest);print(json.dumps(out,indent=2))
