"""Bounded READ+QUERY cinematic/cue probe for owned build56578. No input or publication.
Default: one snapshot. --seconds 180 monitors across CONTINUE and world registration.
Writes private JSON/JSONL beside this script. Resolves fresh identities on every sample.
"""
import argparse,ctypes,datetime as dt,json,re,runpy,struct,time
from pathlib import Path
P=Path(__file__).resolve().parent
EXPECTED=0x80cb802b
ERRORS=(AssertionError,ValueError,OSError,struct.error,UnicodeDecodeError)

def decode_cine(raw,active):
 assert len(raw)==0xd4
 n=struct.unpack_from('<I',raw,0x9c)[0]
 return {'counter':struct.unpack_from('<I',raw,4)[0],'requested_playing':raw[8],'preload':raw[9],
         'optional_ref':{'group':hex(struct.unpack_from('<I',raw,12)[0]),'kind':raw[16],'sub':struct.unpack_from('<h',raw,18)[0]},
         'team_filter':struct.unpack_from('<b',raw,20)[0],'cast_count':n,
         'cast':[hex(struct.unpack_from('<Q',raw,0xa0+8*i)[0]) for i in range(min(n,6))],
         'native_start_accepted':active,'a_image':raw.hex()}

def self_test():
 raw=bytearray(0xd4);struct.pack_into('<I',raw,4,3);raw[8]=1;raw[20]=255
 struct.pack_into('<IBxh',raw,12,0x811c9dc5,255,-1);struct.pack_into('<I',raw,0x9c,1);struct.pack_into('<Q',raw,0xa0,0xd100000000000201)
 d=decode_cine(raw,1);assert d['counter']==3 and d['team_filter']==-1 and d['optional_ref']['sub']==-1 and d['cast']==['0xd100000000000201']
 struct.pack_into('<I',raw,0x9c,0xffffffff);assert len(decode_cine(raw,0)['cast'])==6
 print('PASS: 56578 cinematic offsets, signed filter/absent reference, cast[0] at+A0, bounded invalid cast count')

class Probe:
 def __init__(self):
  g=runpy.run_path(str(P/'monitor-recall-departure-readonly.py'));self.n=g['Native']();self.b,self.u,self.rd=self.n.b,self.n.u,self.n.rd
  for address,expected in {'0x403140': '554889e54157415641554154534883ec', '0x403770': '554889e54156534883ec204889fb4c8b', '0x1072c0': '31c90fb7052ffd4c024885c07414488d'}.items():
   want=bytes.fromhex(expected);assert self.rd(self.b+int(address,16),len(want))==want,'56578 cinematic code control failed'
  self.definition_cache={}
  self.names={};f=P/'hangar-owned-cue-names.json'
  if f.exists():
   for r in json.loads(f.read_text()):
    key=(int(r['group'],16),r['kind'],r['sub']);self.names.setdefault(key,set()).add(r['name'])
  self.definition_seen=set();self.sensor_seen=set()
 def labels(self,h,k,s):return sorted(self.names.get((h,k,s),set()))
 def pool(self):
  u,rd,b=self.u,self.rd,self.b;w=u(b+0x26556d8);assert w,'world pool absent'
  pool=u(w+0x28);assert pool,'world-0 sensor pool absent (world not resident)'
  n=u(pool+0x14,'i');assert 0<=n<=8192
  bitmap=u(u(u(pool)+8)+0x10);data=u(pool+8);off=u(pool+0x1c,'I');size=u(pool+0x20,'I');mask=u(pool+0x24,'I');flags=u(pool+0x34,'I')
  assert bitmap and data and 4<=size<=0x100000
  rows=[]
  for slot in range(n):
   if not ((u(bitmap+4*(slot//32),'I')>>(slot%32))&1):continue
   hi=u(data+off+size*slot,'I')&mask
   tag=(0x80000000|((hi&7)<<27)|((flags<<13)&0x7ffe000)|(slot&0x1fff)) if flags&0x40000000 else (((hi<<23)&0x7f800000)|((flags<<13)&0x7fe000)|(slot&0x1fff))
   a=self.n.resolve(tag);h,k,sub,tb,ta=struct.unpack('<IBxhII',rd(a,16))
   row={'group':hex(h),'kind':k,'sub':sub,'b_type':hex(tb),'a_type':hex(ta),'pending':u(a+0x15,'B'),'owner':u(a+0x60,'I'),'labels':self.labels(h,k,sub)}
   if k in (5,6,7,16,0x1d,0x20,0x27,0x28) and (tag,a) not in self.sensor_seen:
    row['sensor_tag']=hex(tag);row['sensor_address']=hex(a);row['sensor_head']=rd(a,0x70).hex();self.sensor_seen.add((tag,a))
   rows.append(row)
  assert u(w+0x28)==pool and u(pool+0x14,'i')==n,'sensor pool changed during sample'
  life=[r for r in rows if r['kind']==16];cines=[r for r in rows if r['kind']==6]
  return {'world_generation':u(w+0x1d,'B'),'clients':rows,'pending':sum(bool(r['pending']) for r in rows),'lifetime_names':life,'cinematic_names':cines}
 def components(self):
  u,rd,b=self.u,self.rd,self.b;ix=u(b+0x25d6ff8,'H');assert ix!=0xffff
  table=b+0x1ad5000;stride=u(table+16);assert 0<stride<=0x100000
  c=u(table+u(table,'q')+ix*stride+0x30);assert c
  rows=[];errors=[]
  for kind in (5,6,7,16,0x1d,0x20,0x27,0x28):
   start,count=struct.unpack('<hh',rd(c+0x2bd1c+kind*4,4));assert 0<=count<=512 and start>=0
   for j in range(count):
    dx=u(c+0x2a51c+2*(start+j),'H');assert dx<4096
    rec=c+0x510+dx*0x38
    try:
     tag=u(rec+16,'I');offset=u(rec+24);a=self.n.resolve(tag)+offset;sensor_tag=u(a+0x1a8,'I');sensor=self.n.resolve(sensor_tag)
     name_head=rd(sensor,16);h,k,sub,tb,ta=struct.unpack('<IBxhII',name_head);assert k==kind
     row={'group':hex(h),'kind':k,'sub':sub,'labels':self.labels(h,k,sub),'component':hex(a),'a_type':hex(ta),'b_type':hex(tb)}
     if kind==6:
      assert ta==0x8080254d,'unexpected cinematic A type'
      row['cinematic']=decode_cine(rd(a+0x1ac,0xd4),u(a+0x280,'B'))
      row['native_tail_280']=rd(a+0x280,0x40).hex()
     elif kind==16:row['phase']=u(a+0x1ac,'b')
     else:row['state_area_1ac']=rd(a+0x1ac,0x100).hex()
     row['definition_ref']=f'{tag:x}:{offset:x}:{a:x}'
     if (tag,offset,a) not in self.definition_seen:
      row['runtime_head']=rd(a,0x1ac).hex();row['owner_file']=hex(tag);row['owner_offset']=hex(offset)
      try:
       definition_tag=u(a,'I');definition_off=u(a+8);definition=self.n.resolve(definition_tag)+definition_off
       row['definition']={'tag':hex(definition_tag),'offset':hex(definition_off),'head':rd(definition,0x100).hex()}
       if kind==6:row['definition']['cinematic_key_at68']=hex(u(definition+0x68,'I'))
      except ERRORS as exc:row['definition']={'unavailable':str(exc)}
      self.definition_cache[row['definition_ref']]={key:row[key] for key in ('group','kind','sub','component','runtime_head','owner_file','owner_offset','definition')}
      if 'unavailable' not in row['definition']:self.definition_seen.add((tag,offset,a))
     assert u(a+0x1a8,'I')==sensor_tag and rd(sensor,16)==name_head,'component identity changed'
     rows.append(row)
    except ERRORS as e:errors.append({'kind':kind,'slot':dx,'unavailable':str(e)})
  assert u(b+0x25d6ff8,'H')==ix,'content changed during sample'
  return {'content_index':ix,'rows':rows,'errors':errors}
 def sample(self):
  u,b=self.u,self.b;out={}
  for name,fn in [('world',lambda:self.n.slow()['world']),('sensors',self.pool),('components',self.components)]:
   try:out[name]=fn()
   except ERRORS as e:out[name]={'unavailable':str(e)}
  try:
   a=u(b+0x26ca078);out['world_state']=u(a+0x1b0,'i') if a else None
   sel=u(b+0x26bc068);out['selected_activity']=u(sel+2,'h') if sel else None
  except ERRORS as e:out['selection_unavailable']=str(e)
  out['expected_hangar_scenario']=out.get('world',{}).get('scenario_tag')==f'0x{EXPECTED:08x}'
  out['expected_hangar_resident']=out['expected_hangar_scenario'] and 'clients' in out.get('sensors',{})
  return out

def main():
 ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--run',default='sense58');ap.add_argument('--seconds',type=float,default=0);ap.add_argument('--interval',type=float,default=.25);ap.add_argument('--self-test',action='store_true');args=ap.parse_args()
 if args.self_test:self_test();return
 assert 0<=args.seconds<=180 and .1<=args.interval<=2
 assert re.fullmatch(r'[A-Za-z0-9_-]{1,64}',args.run)
 p=Probe();stamp=time.time_ns();path=P/f'hangar-cues-{args.run}-{stamp}.jsonl';summary=P/f'hangar-cues-{args.run}-{stamp}.summary.json'
 started=time.monotonic();last=None;changes=0;lastsample={};had_hangar=False;had_in_world=False;active=set()
 try:
  with path.open('w',encoding='utf8') as f:
   while True:
    if not p.n.alive():break
    sample=p.sample();lastsample=sample;had_hangar|=sample['expected_hangar_resident'];had_in_world|=sample['expected_hangar_resident'] and sample.get('world_state')==36
    for row in sample.get('components',{}).get('rows',[]):
     if sample['expected_hangar_resident'] and row.get('cinematic',{}).get('native_start_accepted'):active.add((row['group'],row['sub']))
    packed=json.dumps(sample,sort_keys=True)
    if packed!=last:
     f.write(json.dumps({'utc':dt.datetime.now(dt.timezone.utc).isoformat(),'elapsed':round(time.monotonic()-started,3),'sample':sample})+'\n');f.flush();changes+=1;last=packed
    if time.monotonic()-started>=args.seconds:break
    time.sleep(min(args.interval,max(0,args.seconds-(time.monotonic()-started))))
 finally:p.n.close()
 out={'jsonl':str(path),'seconds':round(time.monotonic()-started,2),'changed_samples':changes,'saw_expected_hangar':had_hangar,'saw_hangar_world_state36':had_in_world,'accepted_cinematics_observed_in_hangar':sorted(active),'last_sample':lastsample,'definitions':p.definition_cache};summary.write_text(json.dumps(out,indent=2))
 print(summary);print(json.dumps({k:v for k,v in out.items() if k not in ('last_sample','definitions')},indent=2))

if __name__=='__main__':main()
