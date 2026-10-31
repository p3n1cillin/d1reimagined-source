import argparse,ctypes,ctypes.wintypes as wt,datetime,importlib.util,json,struct
from pathlib import Path
a=argparse.ArgumentParser();a.add_argument('--pid',type=int,required=True);a.add_argument('--out',type=Path,required=True);args=a.parse_args()
s=importlib.util.spec_from_file_location('g',r'F:\d1\public-work\d1reimagined-source\tools\ps4_setup\d1-winguest.py');g=importlib.util.module_from_spec(s);s.loader.exec_module(g)
p,base=g.open_client();getpid=g.k32.GetProcessId;getpid.argtypes=[wt.HANDLE];getpid.restype=wt.DWORD;pid=getpid(p.h);assert pid==args.pid,(pid,args.pid)
players=p.u(p.u(base+g.PLAYER_POOL,'Q')+8,'Q')
r={'pid':pid,'base':hex(base),'players':hex(players),'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'tables':[]}
for off in (0x1460,0x14e0,0x1560,0x15e0):
 raw=p.read(players+off,0x80);pairs=[struct.unpack_from('<b3xI',raw,i*8) for i in range(16)];r['tables'].append({'offset':hex(off),'address':hex(players+off),'pairs':pairs})
with args.out.open('x',encoding='utf-8') as f:json.dump(r,f,indent=2)
print(json.dumps(r))
