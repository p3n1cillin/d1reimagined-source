"""Bounded, read-only 56578 completion/profile/lifetime probe. No game calls."""
from pathlib import Path
import importlib.util,json,struct,sys,time
sys.path.insert(0,r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup')
from d1_squad_tracking import SquadReader
sys.path.insert(0,r'lab/history\2026-10-28-static-combat-codex')
import static56578 as st
sp=importlib.util.spec_from_file_location('g',r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-winguest.py');g=importlib.util.module_from_spec(sp);sp.loader.exec_module(g)
p,b=g.open_client();r=SquadReader(p,b)
for va in (0x56b260,0x56dc00,0x10c430,0x75e4c0): assert p.read(b+va,24)==st.read(va,24),hex(va)
out={'time':time.time(),'base':hex(b),'profiles':[],'lifetime':[]}
m=p.u(b+0x26bacc0,'Q')
out['investment_manager']=hex(m)
selection=p.u(b+0x26bc068,'Q');out['selection']={'address':hex(selection),'head':p.read(selection,0x20).hex(),'type':p.u(selection,'b'),'activity':p.u(selection+2,'h')}
out['in_world_handlers']=[{'address':hex(h),'flags_30_80':p.read(h+0x30,0x50).hex(),'postgame68':p.u(h+0x68,'Q'),'postgame70':p.u(h+0x70,'Q'),'postgame78':p.u(h+0x78,'Q')} for h in g.orbit_handler(p,b,0x1945ad0)]
for local in (0,1):
 e=m+4*96+local*16+0x4fd90
 off,count,stride=struct.unpack('<Ih2xi',p.read(e,12))
 assert count<=8 and (off==0xffffffff or 0<stride<0x10000)
 if off==0xffffffff:continue
 for i in range(count):
  q=m+0x40a0+off+i*stride
  z={'local':local,'address':hex(q),'key':hex(p.u(q,'Q')),'stride':stride}
  if stride>=0x3670:
   z.update(activity=p.u(q+0x2c20,'h'),completed=p.u(q+0x2c38,'B'),activity_raw=p.read(q+0x2c20,32).hex())
   n=p.u(q+0x3660,'I');assert n<=512
   arr=q+0x3664
   z['progressions']=[struct.unpack('<IIIH2x',p.read(arr+j*16,16)) for j in range(n)]
  out['profiles'].append(z)
ix,c=r.content();out['content']=ix
start,n=struct.unpack('<hh',p.read(c+0x2bd1c+16*4,4));assert 0<=n<=8
for j in range(n):
 dx=p.u(c+0x2a51c+2*(start+j),'H');rec=c+0x510+dx*0x38
 cp=r.resolve(p.u(rec+16,'I'))+p.u(rec+24,'Q')
 out['lifetime'].append({'component':hex(cp),'phase':p.u(cp+0x1ac,'b'),'record':p.read(rec,56).hex(),'state':p.read(cp+0x1a0,32).hex()})
 image=bytearray(p.read(cp+0x1ac,0x504));image[0]=7
 draft=Path(r'F:\d1\d1preservation-main\cfw-staging\agr-postgame-phase7.draft.txt')
 draft.write_text('# PRIVATE UNPUBLISHED DRAFT; requires source PR for @phase.\n# sense48 gen 0; lifetime group 4786c0e0, kind16, sub3; no pending clients at sample.\n# Publish using clients 4786c0e0 THIS_PATH after rebuilding only when lab stopped.\n# Native 75e4c0 requires phase 7 for 7976b0 Enter post game.\n# Retain the current authority sequence (from outro), not launch authority 3.\n@authority 13,2,0\n@phase 7\n')
 out['phase7_draft']=str(draft)
globals_obj=b+0x26527f8;glob=p.u(globals_obj+0x50,'Q')+8
out['globals']={'object':hex(globals_obj),'definition':hex(glob),'progression_ids':struct.unpack('<5h',p.read(glob+0x166,10))}
obj=p.u(b+0x26527e0,'Q');vt=p.u(obj,'Q')
out['investment_interface']={'object':hex(obj),'vtable':hex(vt-b),'slots':{hex(i):hex(p.u(vt+i,'Q')-b) for i in range(0,0x340,8)}}
table=p.u(globals_obj+0x20,'Q');ar=table+8
count=p.u(ar,'I');assert 164<count<4096
cell=ar+8+p.u(ar+8,'q')+24+164*16
de=cell+p.u(cell,'q')
out['investment_activity164']={'definition':hex(de),'entry':p.read(cell-8,16).hex(),'head':p.read(de,0xb0).hex(),'chain_relative':p.u(de+0x98,'q')}
chain=de+0x98+p.u(de+0x98,'q')
out['investment_activity164']['chain']={'address':hex(chain),'raw':p.read(chain,0x30).hex(),'expression_count':p.u(chain,'I'),'type':p.u(chain+0x10,'b'),'activity':p.u(chain+0x12,'h')}
cn=p.u(chain,'Q');assert cn<512
out['investment_activity164']['chain']['expression_raw']=p.read(chain+8+p.u(chain+8,'q')+16,cn*8).hex()
out['candidate_progression_arrays']=[]
for cache in (0x180,0x1e0):
 table=p.u(globals_obj+cache,'Q')
 if not table:continue
 ar=table+8;cnt=p.u(ar,'I');assert cnt<10000
 data=ar+8+p.u(ar+8,'q')+16
 row={'cache':hex(cache),'count':cnt,'records':[]}
 for idx in (117,118,124,249,250):
  if idx>=cnt:continue
  q=data+idx*56;raw=p.read(q,56)
  z={'id':idx,'address':hex(q),'hash':hex(p.u(q,'I')),'raw':raw.hex()}
  sn=p.u(q+8,'Q')
  if sn<1024:
   step=q+0x10+p.u(q+0x10,'q')+16
   z['steps']=[struct.unpack('<II',p.read(step+i*8,8)) for i in range(sn)]
  row['records'].append(z)
 out['candidate_progression_arrays'].append(row)
at=p.u(b+0x26b95b0,'Q')
if at:
 arr=at+8;count=p.u(arr,'I');assert 164<count<4096
 cell=arr+8+p.u(arr+8,'q')+16+164*8
 definition=cell+p.u(cell,'q')
 out['activity164']={'definition':hex(definition),'head':p.read(definition,0xb0).hex(),'field18':hex(p.u(definition+0x18,'I')),'chain_relative':p.u(definition+0x98,'q')}
dest=Path(__file__).parent/('completion-%d.json'%time.time());dest.write_text(json.dumps(out,indent=2));print(dest);print(json.dumps(out,indent=2))
