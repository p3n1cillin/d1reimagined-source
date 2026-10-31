"""One read-only namespace/state snapshot of instantiated Divide pilots and paths."""
import ctypes,importlib.util,struct,json,datetime
import static56578 as st
spec=importlib.util.spec_from_file_location('g',r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-winguest.py');g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
p,base=g.open_client()
for va in (0x1be5c0,0x1683d0,0x169a60,0x1696d0,0x107ed0):assert p.read(base+va,24)==st.read(va,24)
reg=p.u(p.u(base+g.REGISTRY,'Q'),'Q')
def resolve(tag):
 v=ctypes.c_int32(tag).value>>13;sl=(((v&0xffffffff)>>18)|0x3ff)&v&0xffffffff;r=reg+sl*64
 d,es,m=p.u(r+16,'Q'),p.u(r+48,'i'),p.u(r+52,'i');assert d and 0<es<0x100000
 e=d+(tag&0x1fff)*es;return e-ctypes.c_int64(m&p.u(e+8,'Q')).value
T=base+g.LT_TABLE;idx=p.u(base+g.LT_COUNT,'H');content=p.u(T+p.u(T,'q')+idx*p.u(T+16,'Q')+0x30,'Q')
rows=[]
for kind in (2,0x25,0x38):
 start,n=struct.unpack('<hh',p.read(content+0x2bd1c+kind*4,4));assert 0<=n<500
 for j in range(n):
  dx=p.u(content+0x2a51c+2*(start+j),'H');r=content+0x510+dx*0x38;tag=p.u(r+16,'I');off=p.u(r+24,'Q')
  try:
   obj=resolve(tag)+off;owned=p.u(obj,'I')
   if owned not in set(range(0x80cb8425,0x80cb842b))|set(range(0x80cb8419,0x80cb8422))|{0x80cb8435}:continue
   row={'kind':kind,'dx':dx,'runtime_tag':hex(tag),'component':hex(obj),'owned_tag':hex(owned),'head':p.read(obj,0x80).hex()}
   if kind==2:
    h=p.u(obj+0x1a8,'I');wrapper=resolve(h)
    lifetime=p.u(wrapper+0x40,'H')
    state=p.u(T+p.u(T,'q')+lifetime*p.u(T+16,'Q')+0x30,'Q')+p.u(wrapper+0x38,'Q')
    raw=p.read(state,2312)
    assert struct.unpack_from('<I',raw,0xb8)[0]<=32
    row.update({'state_handle':hex(h),'state':hex(state),'state_hex':raw.hex(),'program_revision':struct.unpack_from('<I',raw,0xb0)[0],'program_start':struct.unpack_from('<I',raw,0xb4)[0],'program_count':struct.unpack_from('<I',raw,0xb8)[0],'entity_handle':hex(p.u(obj+0x218,'I')),'progress':p.read(obj+0x1b8,16).hex()})
   else:row['component_hex']=p.read(obj,0x400).hex()
   rows.append(row)
  except (AssertionError,OSError) as e:rows.append({'kind':kind,'dx':dx,'error':str(e)})
out={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'base':hex(base),'content':hex(content),'rows':rows}
out['type_cells']=[]
for slot in range(0x1974be0,0x1974c58,8):
 cell=p.u(base+slot,'Q');raw=p.read(cell,16)
 out['type_cells'].append({'slot':hex(slot),'cell':hex(cell-base),'tag':hex(struct.unpack_from('<I',raw)[0]),'descriptor':hex(struct.unpack_from('<Q',raw,8)[0]-base)})
with open(__import__('sys').argv[1],'w',encoding='utf8') as f:json.dump(out,f,indent=2)
print(json.dumps({**out,'rows':[{k:v for k,v in r.items() if not k.endswith('hex')} for r in rows]},indent=2))
