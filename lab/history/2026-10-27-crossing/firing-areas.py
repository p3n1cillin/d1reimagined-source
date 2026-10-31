import os,subprocess,struct,re,json
from pathlib import Path
root=Path(r'lab/history\2026-10-27-crossing')
out=root/'owned-firing-areas-80cb8422.bin'
env=dict(os.environ,D1_BUILDS=r'F:\d1\d1preservation-main\capture-rig\builds.json',D1_BUILD='later-56578')
r=subprocess.run([r'F:\d1\d1preservation-main\d1-server\target\release\d1-oracle.exe','oodle','entry','--tag','0x80cb8422','--out',str(out),'--hex','0'],env=env,capture_output=True,text=True,timeout=90)
(root/'owned-firing-areas-80cb8422.log').write_text(r.stdout+r.stderr)
assert r.returncode==0 and 'MATCHES' in r.stdout
b=out.read_bytes(); print('length',hex(len(b)),hex(struct.unpack_from('<I',b)[0]))
labels=[(hex(m.start()),m.group().decode()) for m in re.finditer(rb'fa_[a-zA-Z0-9_]+',b)]; print('labels',labels)
positions=[]
for o in range(0,len(b)-16,4):
 x,y,z,w=struct.unpack_from('<4f',b,o)
 if 70<x<390 and -700<y<-450 and -25<z<65:
  positions.append((hex(o),[round(v,3) for v in (x,y,z,w)]))
print('positions',positions)
(root/'firing-areas-candidates.json').write_text(json.dumps({'labels':labels,'positions':positions},indent=2))
