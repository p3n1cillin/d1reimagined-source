"""One bounded read-only action snapshot. No input, debugger or process writes.

Native derivation: 3B67A0 walks26AD110; its entries resolve action components.
Component+2C is the unit/object handle (36C910). Queue base+30, active mask+34
(3BC740), selector records via3BC560, state via3BC480, selector->implementation
via3B6360 table1935910. Reject identity changes rather than reuse stale units.
"""
import ctypes,importlib.util,struct,json,datetime
import static56578 as st
spec=importlib.util.spec_from_file_location('g',r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-winguest.py');g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
p,base=g.open_client()
for va in (0x36c910,0x3bc480,0x3bc560,0x3b6360,0x3b67a0):assert p.read(base+va,24)==st.read(va,24)
reg=p.u(p.u(base+g.REGISTRY,'Q'),'Q')
def resolve(tag):
 v=ctypes.c_int32(tag).value>>13;sl=(((v&0xffffffff)>>18)|0x3ff)&v&0xffffffff;r=reg+sl*64
 d,es,m=p.u(r+16,'Q'),p.u(r+48,'i'),p.u(r+52,'i');assert d and 0<es<0x100000
 e=d+(tag&0x1fff)*es;return e-ctypes.c_int64(m&p.u(e+8,'Q')).value
def rel(a):return a+p.u(a,'q')+16
player=p.u(p.u(base+g.PLAYER_POOL,'Q')+8,'Q');unit=p.u(player+0x644,'I')
pool=base+0x26ad110;data=p.u(pool+8,'Q');stride=p.u(pool+0x10,'I');capacity=p.u(pool+0x14,'I')
assert stride==8 and capacity==256
catalogue=json.loads((st.ROOT/'native-action-catalogue.json').read_text())
rows=[]
for i in range(capacity):
 tag=p.u(data+i*stride,'I')
 try:
  obj=resolve(tag)
  if p.u(obj+0x2c,'I')!=unit:continue
  raw=p.read(obj,0x200);q=obj+0x30;mask=p.u(q+0x34,'I');active=[]
  for j in range(32):
   if not mask>>j&1:continue
   record=rel(q+8)+j*4;slot=p.u(record,'h');selector=p.u(record+2,'b')
   assert 0<=slot<32 and str(selector) in catalogue
   offset=p.u(rel(q+0x18)+slot*6,'h');assert 0<=offset<0x10000
   state=rel(q+0x28)+offset
   active.append({'index':j,'selector':selector,'implementation':catalogue[str(selector)]['name'],'state':hex(state),'state_hex':p.read(state,0xa0).hex()})
  assert p.u(obj+0x2c,'I')==unit and p.u(player+0x644,'I')==unit
  rows.append({'registry_index':i,'runtime_tag':hex(tag),'component':hex(obj),'unit':hex(unit),'active_mask':hex(mask),'active':active,'component_hex':raw.hex()})
 except (OSError,AssertionError):continue
out={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'base':hex(base),'unit':hex(unit),'rows':rows}
(st.ROOT/'current-actions-native-latest.json').write_text(json.dumps(out,indent=2))
print(json.dumps({**out,'rows':[{k:v for k,v in row.items() if k!='component_hex'} for row in rows]},indent=2))
