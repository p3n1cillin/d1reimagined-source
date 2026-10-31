from pathlib import Path
import importlib.util,struct,json,time
s=importlib.util.spec_from_file_location('g',r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-winguest.py');g=importlib.util.module_from_spec(s);s.loader.exec_module(g);p,b=g.open_client();a=p.u(p.u(b+g.PLAYER_POOL,'Q')+8,'Q');T=b+0x26897f0;data=p.u(T+8,'Q');es=p.u(T+0x10,'I');mask=p.u(T+0x28,'i')&0xffffffffffffffff;out=[]
for off in [0x644,0x690,0x6a0]:
 h=p.u(a+off,'I')
 if h in (0,0xffffffff):continue
 e=data+(h&0x1fff)*es;obj=(e-(mask&p.u(e+8,'Q')))&0xffffffffffffffff
 if p.u(obj+0x24,'I')!=h:continue
 raw=p.read(obj,0x1000)
 vals=[]
 for n in range(0,len(raw)-4,2):
  value=struct.unpack_from('<H',raw,n)[0]
  if value in [25,134,159,184,317,318]:vals.append((hex(n),value))
 out.append({'player_offset':hex(off),'handle':hex(h),'object':hex(obj),'definition':hex(p.u(obj,'I')),'class':hex(p.u(obj+0x10,'I')),'candidate_small_values':vals,'image':raw.hex()})
f=Path(r'lab/history\2026-10-28-sense-codex\weapon-objects35-before.json');f.write_text(json.dumps({'utc':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),'objects':out},indent=2));print(json.dumps([{k:v for k,v in r.items() if k!='image'} for r in out]))
