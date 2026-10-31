"""One bounded read-only native snapshot; no input, debugger or game calls."""
from pathlib import Path
import importlib.util,json,struct,sys,time
sys.path.insert(0,r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup')
sys.path.insert(0,r'lab/history\2026-10-28-static-combat-codex')
from d1_squad_tracking import SquadReader
import static56578 as st
spec=importlib.util.spec_from_file_location('g',r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-winguest.py');g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
p,base=g.open_client();reader=SquadReader(p,base)
for va in (0x17add0,0x139230,0x324870,0x105330):assert p.read(base+va,32)==st.read(va,32)
dest=Path(__file__).parent/('snapshot-%d'%time.time());dest.mkdir()
out={'unix':time.time(),'base':hex(base),'geometry':{},'rows':[],'cells':[]}
for va in (0x25d79a0,0x25d7a20,0x2692220):
 out['geometry'][hex(va)]={'data':hex(p.u(base+va+8,'Q')),'stride':p.u(base+va+16,'I'),'capacity':p.u(base+va+20,'I')}
for cellva in (0x19b1a30,0x19b3058,0x191a2d0,0x1974d90):
 cell=p.u(base+cellva,'Q');tag=p.u(cell,'I');out['cells'].append({'slot':hex(cellva),'tag':hex(tag),'key':hex(p.u(reader.resolve(tag)+8,'I'))})
T=base+g.LT_TABLE;index,content=reader.content()
out['content']=[index,hex(content)]
start,n=struct.unpack('<hh',p.read(content+0x2bd20,4))
for j in range(n):
 dx=p.u(content+0x2a51c+2*(start+j),'H');rec=content+0x510+dx*0x38
 comp=reader.resolve(p.u(rec+16,'I'))+p.u(rec+24,'Q')
 sensor=reader.resolve(p.u(comp+0x1a8,'I'))
 h,k,sub=struct.unpack('<IBxh',p.read(sensor,8))
 row={'group':hex(h),'index':sub,'component':hex(comp),'members':[]}
 label='%08x-%d'%(h,sub)
 (dest/(label+'-squad.bin')).write_bytes(p.read(comp,0x490))
 slot=p.u(sensor+0x40,'H');off=p.u(sensor+0x38,'Q');state=p.u(T+p.u(T,'q')+slot*p.u(T+16,'Q')+0x30,'Q')+off
 (dest/(label+'-A.bin')).write_bytes(p.read(state,124))
 cnt=p.u(comp+0x278,'i');assert 0<=cnt<=32
 for mi in range(cnt):
  rawpair=p.read(comp+0x27c+mi*8,8)
  try:
   mh=reader.pair(rawpair);data=p.u(base+0x25d79a0+8,'Q');stride=p.u(base+0x25d79a0+16,'I');entity=data+(mh&0x1fff)*stride
   b=p.read(entity,stride);assert p.u(entity+0x38,'I')==mh
   ml=label+'-%d'%mi;(dest/(ml+'-member.bin')).write_bytes(b)
   sd=p.u(base+0x25d7a20+8,'Q');ss=p.u(base+0x25d7a20+16,'I');(dest/(ml+'-sense.bin')).write_bytes(p.read(sd+(mh&0x1fff)*ss,ss))
   th=p.u(entity+0x3c,'I');ed=p.u(base+0x2692220+8,'Q');es=p.u(base+0x2692220+16,'I');e=ed+(th&0x1fff)*es
   ph=p.u(e+0x54,'I');root=reader.resolve(ph);offsets=reader.component_offsets(root)
   (dest/(ml+'-root.bin')).write_bytes(p.read(root,max(offsets)+0x1000))
   comps=[{'offset':hex(o),'tag':hex(p.u(root+o,'I')),'self':hex(p.u(root+o+0x24,'I'))} for o in offsets]
   record={'handle':hex(mh),'flags':hex(p.u(entity,'I')),'entity':hex(entity),'active':p.u(entity+0x50,'i')==-1,'root':hex(root),'transform':hex(th),'parent':hex(ph),'components':comps,'reader':reader.member(rawpair)}
   row['members'].append(record)
  except Exception as exc:row['members'].append({'error':str(exc)})
 out['rows'].append(row)
(dest/'index.json').write_text(json.dumps(out,indent=2))
print(dest);print(json.dumps({**out,'rows':[{**r,'members':[{k:v for k,v in m.items() if k not in ('components','reader')} for m in r['members']]} for r in out['rows']]},indent=2))
