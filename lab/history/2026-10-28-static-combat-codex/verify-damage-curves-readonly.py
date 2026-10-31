"""Bounded read-only verification, no debugger/guest calls or controller input."""
from pathlib import Path
import importlib.util,ctypes,json,hashlib,datetime,struct
import static56578 as st
P=Path(__file__).parent
spec=importlib.util.spec_from_file_location('g',r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-winguest.py')
g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g);p,base=g.open_client()
reg=p.u(p.u(base+g.REGISTRY,'Q'),'Q')
tag=p.u(base+0x26b0a78,'I');assert tag==0x80b1a005
v=ctypes.c_int32(tag).value>>13;slot=(((v&0xffffffff)>>18)|0x3ff)&v&0xffffffff
r=reg+slot*64;data=p.u(r+16,'Q');stride=p.u(r+48,'i');mask=p.u(r+52,'i')
assert data and 0<stride<0x100000
e=data+(tag&0x1fff)*stride;addr=e-ctypes.c_int64(mask&p.u(e+8,'Q')).value
owned=(P/'owned-80b1a005.bin').read_bytes();actual=p.read(addr,len(owned));assert actual==owned
checks=[]
for va,n in ((0x528f90,0x424),(0x529c40,0x3f8),(0x237710,0x1a28),(0x512b00,0x15a),(0x546fd0,0x89),(0x1616e0,0x35)):
    a=p.read(base+va,n);b=st.read(va,n);assert a==b,hex(va)
    checks.append({'va':hex(va),'bytes':n,'matches_owned':True})
# Seven optional variable hashes, exact 118870/118540 lookup table.
variables=base+0x26ae458
array=p.u(variables+8,'Q');count=p.u(array,'I');assert 0<count<1024
raw=p.read(array+4,count*24);present={struct.unpack_from('<I',raw,i*24)[0] for i in range(count)}
overrides=[]
for va in range(0x26b0a5c,0x26b0a78,4):
    key=p.u(base+va,'I');overrides.append({'slot':hex(va),'hash':hex(key),'present':key in present})
assert not any(x['present'] for x in overrides)
out={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'read_only':True,'base':hex(base),
     'owned_image_sha256':st.SHA,'globals_handle':hex(tag),'globals_bytes':len(owned),
     'globals_match':True,'globals_sha256':hashlib.sha256(owned).hexdigest(),'code_checks':checks,
     'optional_override_count':count,'overrides':overrides}
(P/'damage-curves-native-verification.json').write_text(json.dumps(out,indent=2));print(json.dumps(out,indent=2))
