"""Bounded, read-only lookup of candidate component bindings, not a health claim."""
from pathlib import Path
import importlib.util,json,struct,sys,time
sys.path.insert(0,r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup')
from d1_squad_tracking import SquadReader
sys.path.insert(0,r'lab/history\2026-10-28-static-combat-codex')
import static56578 as st
spec=importlib.util.spec_from_file_location('g',r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-winguest.py');g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
p,base=g.open_client();r=SquadReader(p,base)
rows=[];dest=Path(__file__).parent/('vitals-%d'%time.time());dest.mkdir()
for t in r.snapshot(0x062dd95d)['targets']:
 root=r.resolve(t['parent_handle']);d=r.resolve(p.u(root+8,'I'))+p.u(root+16,'Q');c=d+0x38
 sz=p.u(c+8,'I');slots=c+0x20+p.u(c+16,'q');records=c+0x30+p.u(c+32,'q')
 row={'squad':t['squad'],'root':hex(root),'components':[],'position':t['parts'][0]['position']}
 for j in range(sz):
  key,start,flags=struct.unpack('<Ihh',p.read(slots+j*8,8))
  if key not in (0xe52389ae,0x1939b49e):continue
  for i in range(flags&0x7fff):
   rec=records+(start+i)*40;offset=p.u(rec+4,'i');cp=root+offset
   tag=p.u(rec+16,'I');off=p.u(rec+24,'Q');deft=r.resolve(tag)+off
   data=p.read(cp,0x2000)
   (dest/('%d-%x.bin'%(t['squad'],key))).write_bytes(data)
   (dest/('%d-%x-definition.bin'%(t['squad'],key))).write_bytes(p.read(deft,0x1000))
   methods=[]
   template=r.resolve(p.u(rec+32,'I'));shift=p.u(template+24,'q')
   for mo in range(0x20,0x210,16):
    try:
     method=p.u(template+shift+mo,'Q');adjust=p.u(template+shift+mo+8,'q');this=cp+adjust
     fn=p.u(p.u(this,'Q')+method-1,'Q') if method&1 else method
     va=fn-base;lo,hi=st.func(va)
     if not 0<=va<0x18d8000:continue
     assert p.read(fn,24)==st.read(va,24)
     (dest/('native-%x.txt'%lo)).write_text(st.dis(lo,hi-lo))
     methods.append({'slot':hex(mo),'fn':hex(va),'this':hex(this)})
    except (OSError,ValueError,AssertionError):continue
   row['components'].append({'key':hex(key),'offset':hex(offset),'component':hex(cp),'native_tag':hex(p.u(cp,'I')),'definition':hex(deft),'record':p.read(rec,40).hex(),'methods':methods})
 rows.append(row)
(dest/'index.json').write_text(json.dumps({'base':hex(base),'rows':rows},indent=2));print(dest);print(json.dumps(rows,indent=2))
