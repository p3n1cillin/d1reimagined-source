from pathlib import Path
import importlib.util,hashlib,struct,json,sys,time
r=Path(__file__).resolve().parent
s=importlib.util.spec_from_file_location('g',r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-winguest.py');g=importlib.util.module_from_spec(s);s.loader.exec_module(g)
p,base=g.open_client();b=Path(r'F:\d1\d1preservation-main\dumps\later-56578-CUSA00219\eboot.elf').read_bytes();assert hashlib.sha256(b).hexdigest()==g.ELF_SHA
ph=struct.unpack_from('<Q',b,0x20)[0];sz,n=struct.unpack_from('<HH',b,0x36);segs=[struct.unpack_from('<II6Q',b,ph+i*sz) for i in range(n)]
def code(v,l):
 for t,f,o,a,pa,fs,ms,al in segs:
  if t==1 and a<=v and v+l<=a+fs:return b[o+v-a:o+v-a+l]
 raise ValueError(hex(v))
controls=[]
for va in [0x630c70,0x630d20,0x66e740,0x66e817]:
 assert p.read(base+va,24)==code(va,24),hex(va)
 controls.append({'va':hex(va),'native_matches_owned':True,'bytes':24})
a=p.u(base+0x26be8a0,'Q')
out={'utc':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),'label':sys.argv[1],'base':hex(base),'pointer':hex(a),'values':list(struct.unpack('<4i',p.read(a,16))) if a else None,'code_controls':controls,'interpretation':'candidate ammo globals; correlate against HUD before calling this Guardian reserve'}
with (r/('ammo-'+sys.argv[1]+'.json')).open('x') as f:json.dump(out,f,indent=2)
print(json.dumps(out))
