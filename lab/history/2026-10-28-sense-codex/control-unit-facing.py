from pathlib import Path
import importlib.util,struct,subprocess,json,math,time
s=importlib.util.spec_from_file_location('walk',r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-walk.py');m=importlib.util.module_from_spec(s);s.loader.exec_module(m);c=m.Client()
def sample():
 q=m._unit_tracking.read_unit_position(c.p,c.base);vs=[struct.unpack('<3f',c.p.read(q.address+o,12)) for o in [0x230,0x3a0]];return {'unix':time.time(),'unit':hex(q.address),'position':q.position,'vectors':vs,'yaw':[math.atan2(v[1],v[0]) for v in vs]}
rows=[sample()]
for k in ['Hold:L:200','Hold:J:200']:
 p=subprocess.run(['powershell','-NoProfile','-File',r'F:\d1\tools\d1-pad.ps1','-Keys','Hold:A:80,Hold:D:80,'+k],capture_output=True);assert p.returncode==0;rows.append(sample())
print(json.dumps(rows))
with Path(r'lab/history\2026-10-28-sense-codex\unit-facing-control34.json').open('x') as f:json.dump(rows,f,indent=2)
