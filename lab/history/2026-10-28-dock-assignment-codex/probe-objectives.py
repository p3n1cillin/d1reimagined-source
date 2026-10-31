"""Single bounded native read-only kind-3 snapshot. No calls into the game."""
from pathlib import Path
import importlib.util,json,struct,sys,time
sys.path.insert(0,r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup')
from d1_squad_tracking import SquadReader
sys.path.insert(0,r'lab/history\2026-10-28-static-combat-codex')
import static56578 as st
spec=importlib.util.spec_from_file_location('g',r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-winguest.py');g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
p,b=g.open_client();r=SquadReader(p,b);index,c=r.content()
for va in (0x1c28d0,0x171610,0x1046b0):assert p.read(b+va,32)==st.read(va,32)
start,n=struct.unpack('<hh',p.read(c+0x2bd28,4));assert 0<=n<=128
dest=Path(__file__).parent/('native-%d'%time.time());dest.mkdir()
rows=[]
for j in range(n):
 dx=p.u(c+0x2a51c+2*(start+j),'H');rec=c+0x510+dx*0x38
 cp=r.resolve(p.u(rec+16,'I'))+p.u(rec+24,'Q')
 row={'component':hex(cp),'record':p.read(rec,56).hex(),'header':p.read(cp,32).hex(),'bindings':[]}
 sensor=r.resolve(p.u(cp+0x1a8,'I'));raw=p.read(sensor,16);h,k,sub,tb,ta=struct.unpack('<IBxhII',raw)
 row.update(group=hex(h),kind=k,index=sub,btag=hex(tb),atag=hex(ta))
 label=f'{h:08x}-{k}-{sub}'
 (dest/(label+'-component.bin')).write_bytes(p.read(cp,0x300))
 for at in (0,0x1b0):
  tag=p.u(cp+at,'I');off=p.u(cp+at+8,'Q')
  try:
   base=r.resolve(tag);de=base+off
   (dest/(label+f'-def-{at:x}.bin')).write_bytes(p.read(de,0x300))
   row['bindings'].append({'at':hex(at),'tag':hex(tag),'offset':hex(off),'file_base':hex(base),'definition':hex(de),'head':p.read(de,0x60).hex()})
  except (OSError,RuntimeError) as e:row['bindings'].append({'at':hex(at),'error':str(e)})
 rows.append(row)
(dest/'index.json').write_text(json.dumps({'time':time.time(),'base':hex(b),'content':index,'rows':rows},indent=2));print(dest);print(json.dumps(rows,indent=2))
