"""Private, offline only. Does not publish control or write process memory."""
import importlib.util, json, struct, subprocess, os
from pathlib import Path
P=Path(__file__).parent
src=Path(r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-member-states.py')
spec=importlib.util.spec_from_file_location('member',src);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
base=Path(r'F:\d1\d1preservation-main\cfw-staging\agr-divide-objective20-m52-disarm39.txt')
(P/'skiff-candidate-base.txt').write_bytes(base.read_bytes())
body=bytearray(m.member(1))
activation=bytes(body)
struct.pack_into('<III',body,0xb0,1,0,2)
for i,(selector,marker) in enumerate([(0,0),(3,1)]):
 at=0xc0+i*64
 body[at]=selector
 struct.pack_into('<IBxH',body,at+0x10,0x9243dffb,0x38,79)
 struct.pack_into('<I',body,at+0x18,marker)
(P/'drop2-exit-pilot7-A.bin').write_bytes(body)
lines=[];hit=0
for line in base.read_text().splitlines():
 f=line.partition('#')[0].split()
 if len(f)>=5 and f[0].lower()=='0x9243dffb' and int(f[1],0)==2 and int(f[2],0)==7:
  hit+=1;f[5:7]=['0x80800868',body.hex()]
  line=' '.join(f)+' # UNTESTED pilot activation + snap exit2 marker0, move marker1'
 lines.append(line)
assert hit==1,hit
(P/'agr-drop2-exit-candidate.txt').write_text('\n'.join(lines)+'\n',encoding='utf8')
(P/'agr-drop2-activate-only-control.txt').write_text('\n'.join(line.replace(body.hex(),activation.hex()) for line in lines)+'\n',encoding='utf8')
env=dict(os.environ,D1_BUILD='later-56578',D1_BUILDS=r'F:\d1\d1preservation-main\capture-rig\builds.json')
cmd=[r'F:\d1\public-work\target-codex-static\release\d1-oracle.exe','object','--tag','0x80800868','--obj',body.hex()]
r=subprocess.run(cmd,env=env,capture_output=True,text=True,timeout=60)
(P/'drop2-exit-codec-check.txt').write_text(r.stdout+r.stderr,encoding='utf8')
print('candidate written privately; native codec exit',r.returncode);print(r.stdout[-1200:]);print(r.stderr[-1200:])
raise SystemExit(r.returncode)
