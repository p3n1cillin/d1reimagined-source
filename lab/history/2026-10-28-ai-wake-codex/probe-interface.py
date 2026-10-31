from pathlib import Path
import importlib.util,sys,struct,json,time
sys.path.insert(0,r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup')
sys.path.insert(0,r'lab/history\2026-10-28-static-combat-codex')
from d1_squad_tracking import SquadReader
import static56578 as st
spec=importlib.util.spec_from_file_location('g',r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-winguest.py');g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
p,base=g.open_client();r=SquadReader(p,base)
snap=json.loads((sorted(x for x in Path(__file__).parent.glob('snapshot-*') if x.is_dir())[-1]/'index.json').read_text())
rows=[]
for sq in snap['rows']:
 for m in sq['members']:
  if 'error' in m:continue
  ent=int(m['entity'],16)
  if p.u(ent+0x38,'I')!=int(m['handle'],16):continue
  for off,mo in ((0x78,0x48),(0x138,0x30),(0xa8,0x120)):
   try:
    template=r.resolve(p.u(ent+off+16,'I'))
    obj=r.resolve(p.u(ent+off+24,'I'))+p.u(ent+off+32,'Q')
    shift=p.u(template+24,'Q');method=p.u(template+shift+mo,'Q');adjust=p.u(template+shift+mo+8,'q');this=obj+adjust
    fn=p.u(p.u(this,'Q')+method-1,'Q') if method&1 else method
    va=fn-base
    assert p.read(fn,24)==st.read(va,24)
    lo,hi=st.func(va);(Path(__file__).parent/('native-%x.txt'%lo)).write_text(st.dis(lo,hi-lo))
    rows.append({'squad':sq['index'],'interface_offset':hex(off),'template':hex(template),'this':hex(this),'fn':hex(va),'function':hex(lo),'size':hi-lo})
   except Exception as ex:rows.append({'squad':sq['index'],'interface_offset':hex(off),'error':str(ex)})
dest=Path(__file__).parent/('interfaces-%d.json'%time.time());dest.write_text(json.dumps(rows,indent=2));print(json.dumps(rows,indent=2))
