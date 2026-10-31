"""Bounded read-only content/selector snapshot. Does not execute guest code."""
import importlib.util,json,struct,datetime
import static56578 as st
spec=importlib.util.spec_from_file_location('g',r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-winguest.py');g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
p,base=g.open_client()
for va in (0x529c40,0x528f90,0x511dc0,0x5142c0):assert p.read(base+va,32)==st.read(va,32)
registry=p.u(p.u(base+g.REGISTRY,'Q'),'Q')
def resolve(tag):
    v=struct.unpack('<i',struct.pack('<I',tag))[0]>>13
    slot=(((v&0xffffffff)>>18)|0x3ff)&v&0xffffffff
    rec=registry+slot*64;data=p.u(rec+16,'Q');stride=p.u(rec+48,'i');mask=p.u(rec+52,'i')
    assert data and 0<stride<0x100000
    e=data+(tag&0x1fff)*stride
    return e-struct.unpack('<q',struct.pack('<Q',mask&p.u(e+8,'Q')))[0]
tag=p.u(base+0x26b0a78,'I');addr=resolve(tag)
raw=p.read(addr,0x300)
out={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'base':hex(base),'handle':hex(tag),'address':hex(addr),'header':raw[:64].hex(),'tables':[],'typecells':[]}
for off in (0x40,0xe8,0x190,0x238):
    count=p.u(addr+off+8,'q');assert 0<=count<=128
    values=p.read(addr+off,8)
    relative=p.u(addr+off+16,'q');start=addr+off+16+relative+16
    rows=[list(struct.unpack('<3f',p.read(start+12*i,12))) for i in range(count)]
    out['tables'].append({'offset':hex(off),'limits':list(struct.unpack('<2f',values)),'count':count,'rows':rows,'header':raw[off:off+0xa8].hex()})
for off in (0x19b3058,0x19b4518):
    ptr=p.u(base+off,'Q');out['typecells'].append({'cell':hex(off),'address':hex(ptr),'bytes':p.read(ptr,32).hex()})
for n in range(0x26b0a5c,0x26b0a78,4):
    h=p.u(base+n,'I')
    try:r=resolve(h);out.setdefault('configuration',[]).append({'slot':hex(n),'handle':hex(h),'head':p.read(r,32).hex()})
    except (OSError,AssertionError):out.setdefault('configuration',[]).append({'slot':hex(n),'handle':hex(h),'unresolved':True})
with (st.ROOT/'power-globals-native.json').open('x',encoding='utf8') as f:json.dump(out,f,indent=2,allow_nan=False)
print(json.dumps(out,indent=2))
