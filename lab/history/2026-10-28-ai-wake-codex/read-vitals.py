"""Read-only experimental native damage pools. No native function calls."""
from pathlib import Path
import importlib.util,json,struct,sys,time,math
sys.path.insert(0,r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup')
from d1_squad_tracking import SquadReader
spec=importlib.util.spec_from_file_location('g',r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-winguest.py');g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)

CONTROLS={
 0x4f61b0:'554889e54157415641554154534881ec980000004989f74c',
 0x50e800:'554889e54883ec20488b05f1085101488b00488945f8488b',
 0x50ebb0:'488b07488b4f18488b5108488d8af802000048038af80200',
 0x146e00:'554889e54157415641554154534881ec980900004889fb48',
 0x1adb60:'554889e54157415641554154534881eca8000000899548ff',
 0x1ac160:'8a47432404c0e802c30f1f8000000000554889e541574156',
}

def require(ok,message):
 if not ok:raise RuntimeError(message)

def verify_reader(r):
 for va,expected in CONTROLS.items():
  require(r.p.read(r.base+va,24)==bytes.fromhex(expected),'native control mismatch at %x'%va)

def read_awareness(r,t):
 """Native fields only; category numbers are deliberately not friendly labels."""
 p=r.p;b=r.base;mh=t['member_handle'];table=b+0x25d79a0
 require((mh&8191)<p.u(table+20,'I'),'member outside table')
 ent=p.u(table+8,'Q')+(mh&8191)*p.u(table+16,'I')
 require(p.u(ent+0x38,'I')==mh and p.u(ent+0x50,'i')==-1,'member changed')
 hd=p.u(ent+0x2344,'I')
 row={'member_awareness':p.u(ent+0x2342,'B'),'selected_contact':None}
 if hd!=0xffffffff:
  tab=b+0x25d8ef0;stride=p.u(tab+16,'I')
  require(0<stride<=0x1000 and (hd&8191)<p.u(tab+20,'I'),'contact outside table')
  cp=p.u(tab+8,'Q')+(hd&8191)*stride
  require(p.u(cp+12,'I')==mh,'contact owner mismatch')
  row['selected_contact']={'handle':hd,'kind':p.u(cp,'B'),'level':p.u(cp+1,'B'),'state_20':p.u(cp+0x20,'B'),'evidence_21':p.u(cp+0x21,'B'),'flags_42':p.u(cp+0x42,'H'),'acquired_bit_0400':bool(p.u(cp+0x42,'H')&0x400),'target_transform':p.u(cp+0x44,'I')}
 require(p.u(ent+0x38,'I')==mh and p.u(ent+0x50,'i')==-1 and p.u(ent+0x2344,'I')==hd,'member/contact changed')
 return row

def read_vitals(r,t):
 verify_reader(r)
 p=r.p;root=r.resolve(t['parent_handle']);d=r.resolve(p.u(root+8,'I'))+p.u(root+16,'Q');c=d+0x38
 sz=p.u(c+8,'I');assert 0<sz<=4096
 slots=c+0x20+p.u(c+16,'q');records=c+0x30+p.u(c+32,'q')
 matches=[]
 for j in range(sz):
  key,start,flags=struct.unpack('<Ihh',p.read(slots+j*8,8))
  if key!=0xe52389ae:continue
  assert 0<=start and 0<(flags&0x7fff)<=16
  for i in range(flags&0x7fff):
   rec=records+(start+i)*40;cp=root+p.u(rec+4,'i')
   require(p.u(cp,'I') in (0x80cd0037,0x80cd0554),'unsupported damageable component')
   self_handle=p.u(cp+0x24,'I');require(r.object(self_handle)==cp,'damageable identity mismatch')
   binding=p.read(cp,16)
   de=r.resolve(p.u(cp,'I'))+p.u(cp+8,'Q')
   nr=p.u(de+0x1c8,'Q');np=p.u(de+0x2e8,'Q');assert 0<nr<=256 and 0<np<=256
   regions=de+0x1d0+p.u(de+0x1d0,'q')+16
   pools=de+0x2f0+p.u(de+0x2f0,'q')+16
   fractions=cp+0x2f8+p.u(cp+0x2f8,'q')+16
   states=cp+0x320+p.u(cp+0x320,'q')+16
   row={'squad':t['squad'],'member_handle':t['member_handle'],'component':hex(cp),'flags_30c':p.u(cp+0x30c,'I'),'regions':[],'pools':[],'awareness':read_awareness(r,t)}
   for ri in range(nr):
    region=regions+ri*0xd0;frac=p.u(fractions+ri*4,'f');assert math.isfinite(frac)
    row['regions'].append({'index':ri,'category':p.u(region+0xa0,'b'),'pool':p.u(region+0xa4,'i'),'fraction':frac})
   for pi in range(np):
    pd=pools+pi*0x3c;ps=states+pi*0x28;flags=p.u(pd+12,'B');ri=p.u(pd,'i')
    capacity=p.u(cp+(0xc4 if flags&2 else 0x34),'f') if flags&6 else p.u(pd+4,'f')*p.u(cp+0x310,'f')
    row['pools'].append({'index':pi,'region':ri,'definition_flags':flags,'base_capacity':capacity,'current_base':capacity*row['regions'][ri]['fraction'] if 0<=ri<nr else None,'term_50ec70':p.u(ps+10,'H')*p.u(ps,'f')*p.u(ps+4,'f'),'term_50ec90':p.u(ps+22,'H')*p.u(ps+12,'f')*p.u(ps+16,'f')})
   require(r.resolve(t['parent_handle'])==root and r.object(self_handle)==cp and p.read(cp,16)==binding,'damageable binding changed')
   for part in t['parts']:require(r.object(part['handle'])==part['address'],'physical binding changed')
   matches.append(row)
 return matches

if __name__=='__main__':
 p,base=g.open_client();r=SquadReader(p,base)
 rows=[]
 for t in r.snapshot(0x062dd95d)['targets']:
  try:rows.extend(read_vitals(r,t))
  except (OSError,AssertionError,ValueError,RuntimeError) as e:rows.append({'squad':t['squad'],'error':repr(e)})
 result={'time':time.time(),'base':hex(base),'rows':rows}
 dest=Path(__file__).parent/('pool-read-%d.json'%time.time());dest.write_text(json.dumps(result,indent=2));print(json.dumps(result,indent=2))
