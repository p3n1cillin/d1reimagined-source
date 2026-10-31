from pathlib import Path
import importlib.util,struct,json,time,subprocess
s=importlib.util.spec_from_file_location('g',r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-winguest.py');g=importlib.util.module_from_spec(s);s.loader.exec_module(g);p,b=g.open_client();a=p.u(p.u(b+g.PLAYER_POOL,'Q')+8,'Q');T=b+0x26897f0

def sample():
 h=p.u(a+0x644,'I');data=p.u(T+8,'Q');es=p.u(T+0x10,'I');mask=p.u(T+0x28,'i')&0xffffffffffffffff;e=data+(h&0x1fff)*es;obj=(e-(mask&p.u(e+8,'Q')))&0xffffffffffffffff
 assert p.u(obj+0x24,'I')==h
 points=[struct.unpack('<3f',p.read(obj+off,12)) for off in [0x1a0,0xb00,0xb50]]
 return {'unix':time.time(),'unit':hex(obj),'handle':hex(h),'points':points}
rows=[sample()]
for keys in ['Hold:W:300','Hold:S:300']:
 q=subprocess.run(['powershell','-NoProfile','-File',r'F:\d1\tools\d1-pad.ps1','-Keys',keys],capture_output=True,text=True);assert q.returncode==0
 rows.append(sample())
print(json.dumps(rows))
with Path(r'lab/history\2026-10-28-sense-codex\unit-position-control34.json').open('x') as f:json.dump(rows,f,indent=2)
