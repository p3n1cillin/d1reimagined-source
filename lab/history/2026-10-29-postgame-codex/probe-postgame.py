"""One bounded read-only 56578 sample. Never writes game/control/schedule files."""
import importlib.util,json,struct,sys,time,re
from pathlib import Path
sys.path.insert(0,r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup')
from d1_squad_tracking import SquadReader
sys.path.insert(0,r'lab/history\2026-10-28-static-combat-codex')
import static56578 as st
sp=importlib.util.spec_from_file_location('g',r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-winguest.py');g=importlib.util.module_from_spec(sp);sp.loader.exec_module(g)
p,b=g.open_client();r=SquadReader(p,b);out={'time':time.time(),'base':hex(b)}
for va in (0x56dc00,0x7976b0,0x574900,0x770ba0):assert p.read(b+va,24)==st.read(va,24),hex(va)
sel=p.u(b+0x26bc068,'Q')
out['selection']={'address':hex(sel),'raw':p.read(sel,0x20).hex(),'type':p.u(sel,'b'),'activity':p.u(sel+2,'h')}
ix,c=r.content();out['content']=ix
start,n=struct.unpack('<hh',p.read(c+0x2bd1c+16*4,4));assert 0<=n<=8
out['lifetime']=[]
for j in range(n):
 dx=p.u(c+0x2a51c+2*(start+j),'H');rec=c+0x510+dx*0x38
 cp=r.resolve(p.u(rec+16,'I'))+p.u(rec+24,'Q')
 out['lifetime'].append({'address':hex(cp),'phase':p.u(cp+0x1ac,'b')})
h=b+0x26ce420
if p.u(h,'Q')==b+0x1945ad0:
 out['in_world']={'address':hex(h),'flags30':p.read(h+0x30,0x50).hex(),'postgame68':p.u(h+0x68,'Q'),'postgame70':p.u(h+0x70,'Q'),'postgame78':p.u(h+0x78,'Q')}
col=p.u(b+0x26bc040,'Q')
if col:
 out['collector']={'address':hex(col),'state':p.u(col,'b'),'target':p.u(col+1,'b'),'activity':p.u(col+2,'h'),'level_xp':struct.unpack('<III',p.read(col+0x10,12)),'prestige_xp':struct.unpack('<III',p.read(col+0x1c,12)),'item_changes':p.u(col+0x14c,'I'),'queued_grants':p.u(col+0x254,'I'),'reward_count':p.u(col+0x258,'I'),'raw':p.read(col,0x300).hex()}
dat=p.u(b+0x26ca070,'Q')
out['datamine']={'address':hex(dat),'postgame':p.read(dat+0x1e60,16).hex(),'participant_count':p.u(dat+0x109c8,'Q'),'participant0':p.read(dat+0x109d0,0xc0).hex()}
out['skill_interface']={'vtable':hex(p.u(b+0x281d410,'Q')-b),'submit':hex(p.u(p.u(b+0x281d410,'Q')+0x40,'Q')-b)}
manager=p.u(b+0x26bacc0,'Q');e=manager+4*96+16+0x4fd90
off,count,stride=struct.unpack('<Ih2xi',p.read(e,12));assert 0<=count<=8 and 0x3670<=stride<0x10000
out['profiles']=[]
for i in range(count):
 q=manager+0x40a0+off+i*stride;n=p.u(q+0x3660,'I');assert n<=512
 out['profiles'].append({'address':hex(q),'key':hex(p.u(q,'Q')),'activity':p.u(q+0x2c20,'h'),'completed':p.u(q+0x2c38,'B'),'progressions':[struct.unpack('<IIIH2x',p.read(q+0x3664+j*16,16)) for j in range(n)]})
glob=b+0x26527f8;out['activities']=[]
ar=p.u(glob+0x20,'Q')+8;assert 165<p.u(ar,'I')<4096
ca=p.u(b+0x26b95b0,'Q')+8
out['activity_counts']={'investment':p.u(ar,'I'),'client':p.u(ca,'I'),'client_table_head':p.read(ca-8,48).hex()}
out['client_neighbours']=[]
for idx in range(162,169):
 ce=ca+8+p.u(ca+8,'q')+16+idx*8;cd=ce+p.u(ce,'q')
 out['client_neighbours'].append({'id':idx,'address':hex(cd),'head':p.read(cd,0x60).hex()})
out['owned_activity_files']={hex(tag):{'address':hex(r.resolve(tag)),'raw':p.read(r.resolve(tag),152).hex()} for tag in [0x80a585cb,0x80a585cc]}
for idx in (164,165):
 cell=ar+8+p.u(ar+8,'q')+24+idx*16;de=cell+p.u(cell,'q')
 row={'index':idx,'definition':hex(de),'hash':hex(p.u(de,'I')),'head':p.read(de,0x200).hex(),'family':p.u(de+0x8c,'B'),'skip_matchmaking':p.u(de+0xa9,'B')}
 chainrel=p.u(de+0x98,'q')
 if chainrel:
  chain=de+0x98+chainrel;row['chain']={'raw':p.read(chain,0x18).hex(),'expr_count':p.u(chain,'I'),'type':p.u(chain+0x10,'b'),'activity':p.u(chain+0x12,'h')}
 ce=ca+8+p.u(ca+8,'q')+16+idx*8;cd=ce+p.u(ce,'q')
 row['client']={'address':hex(cd),'raw':p.read(cd,0x200).hex()}
 out['activities'].append(row)
fn=st.pointer(0x190cf40+0x248);dis=st.dis(fn,0x80)
mt=re.search(r'mov\s+rdx, qword ptr \[r8 \+ (0x[0-9a-f]+)\]',dis);assert mt
msgfile=p.u(glob+int(mt[1],16),'Q')
if not msgfile:
 root=p.u(glob,'Q');msgfile=r.resolve(p.u(root+0x588,'I'))
tab=msgfile+0x18
nn=p.u(tab,'I');assert 20<=nn<=512
arr=tab+8+p.u(tab+8,'q')+16
out['messages']=[{'hash':hex(p.u(arr+i*64,'I')),'id':p.u(arr+i*64+0x10,'H'),'request':hex(p.u(arr+i*64+0x14,'I')),'response':hex(p.u(arr+i*64+0x18,'I')),'raw':p.read(arr+i*64,64).hex()} for i in range(nn)]
assert any(x['id']==501 and x['hash']=='0xe906221f' for x in out['messages'])
dest=Path(__file__).parent/('postgame-%d.json'%time.time());dest.write_text(json.dumps(out,indent=2))
print(dest)
print(json.dumps({k:v for k,v in out.items() if k not in ('messages','activities')},indent=2))
print('activities',[(x['index'],x['hash'],x['family'],x['skip_matchmaking']) for x in out['activities']])
