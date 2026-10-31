from pathlib import Path
import importlib.util,struct,json,sys,time,hashlib
r=Path(__file__).resolve().parent
s=importlib.util.spec_from_file_location('g',r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-winguest.py');g=importlib.util.module_from_spec(s);s.loader.exec_module(g)
p,base=g.open_client()
b=Path(r'F:\d1\d1preservation-main\dumps\later-56578-CUSA00219\eboot.elf').read_bytes();assert hashlib.sha256(b).hexdigest()==g.ELF_SHA
ph=struct.unpack_from('<Q',b,0x20)[0];sz,n=struct.unpack_from('<HH',b,0x36)
for va in [0x2cfd00,0x2cfca0]:
 for j in range(n):
  typ,fl,off,v,pa,fs,ms,al=struct.unpack_from('<II6Q',b,ph+j*sz)
  if typ==1 and v<=va and va+29<=v+fs:assert p.read(base+va,29)==b[off+va-v:off+va-v+29];break
 else:raise ValueError('Code not in owned image')
print('Native B-image getter code controls passed',flush=True)
def clients():
 w=p.u(base+g.WORLD_POOL,'Q');P=p.u(w+0x28,'Q');cnt=p.u(P+0x14,'i');bm=p.u(p.u(p.u(P,'Q')+8,'Q')+0x10,'Q')
 data,bo,esz,mask,flags=p.u(P+8,'Q'),p.u(P+0x1c,'I'),p.u(P+0x20,'I'),p.u(P+0x24,'I'),p.u(P+0x34,'I');reg=p.u(p.u(base+g.REGISTRY,'Q'),'Q');out=[]
 for slot in range(min(cnt,8192)):
  if not p.u(bm+4*(slot//32),'I')>>(slot%32)&1:continue
  hi=p.u(data+bo+esz*slot,'I')&mask
  tag=0x80000000|((hi&7)<<27)|((flags<<13)&0x7ffe000)|(slot&0x1fff) if flags&0x40000000 else ((hi<<23)&0x7f800000)|((flags<<13)&0x7fe000)|(slot&0x1fff)
  esi=struct.unpack('<i',struct.pack('<I',tag))[0]>>13;sl=(((esi&0xffffffff)>>18)|0x3ff)&esi&0xffffffff;rec=reg+sl*64;d,es,mk=p.u(rec+0x10,'Q'),p.u(rec+0x30,'i'),p.u(rec+0x34,'i')&0xffffffffffffffff;e=d+(tag&0x1fff)*es;obj=e-(mk&p.u(e+8,'Q'))
  h,k,i,tb,ta=struct.unpack('<IBxhII',p.read(obj,16))
  if k==1 and tb==0x80800357:out.append((obj,h,k,i,tb))
 return out
label=sys.argv[1];seconds=float(sys.argv[2]) if len(sys.argv)>2 else 0;deadline=time.time()+seconds;prev={};count=0
with (r/('b-images-'+label+'.jsonl')).open('x') as f:
 while True:
  for obj,h,k,i,tb in clients():
   try:
    pool=base+0x1ad5000;stride=p.u(pool+0x10,'Q');slot=p.u(obj+0x58,'H');off=p.u(obj+0x50,'Q')
    if slot==65535:continue
    at=(pool+p.u(pool,'Q'))&0xffffffffffffffff;ptr=p.u(at+stride*slot+0x30,'Q')+off
    raw=p.read(ptr,160)
    if p.read(ptr,160)!=raw or p.u(obj+0x58,'H')!=slot or p.u(obj+0x50,'Q')!=off:continue
    key=(h,k,i)
    if prev.get(key)==raw:continue
    prev[key]=raw;count+=1
    row={'utc':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),'unix':time.time(),'name':{'group':h,'kind':k,'index':i},'tag':tb,'client':hex(obj),'image_ptr':hex(ptr),'revision':p.u(obj+0x10,'I'),'image':raw.hex(),'i32':list(struct.unpack_from('<4i',raw)),'scalar16':struct.unpack_from('<f',raw,16)[0],'state24':list(raw[24:28]),'consistent_double_read':True}
    f.write(json.dumps(row)+'\n');f.flush()
   except OSError:continue
  if time.time()>=deadline:break
  time.sleep(.25)
print('Read-only B snapshots',count,flush=True)
