from pathlib import Path
import hashlib,importlib.util,json,struct,subprocess,sys,time
root=Path(__file__).resolve().parent
tools=Path(r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup')
s=importlib.util.spec_from_file_location('g',tools/'d1-winguest.py');g=importlib.util.module_from_spec(s);s.loader.exec_module(g)
start=time.time()
while time.time()<start+1200:
 log=Path(g.LOG)
 if log.exists() and log.stat().st_mtime>=start and "Entering state 'setup:activity_world_transition'" in log.read_text(errors='replace'):break
 time.sleep(2)
else:raise RuntimeError('No fresh native world transition; no observer attached')
p,base=g.open_client()
b=Path(r'F:\d1\d1preservation-main\dumps\later-56578-CUSA00219\eboot.elf').read_bytes();assert hashlib.sha256(b).hexdigest()==g.ELF_SHA
ph=struct.unpack_from('<Q',b,0x20)[0];sz,n=struct.unpack_from('<HH',b,0x36)
for i in range(n):
 t,f,o,v,pa,fs,ms,a=struct.unpack_from('<II6Q',b,ph+i*sz)
 if t==1 and v<=0x299cb0 and 0x299cb0+64<=v+fs:
  assert p.read(base+0x299cb0,64)==b[o+0x299cb0-v:o+0x299cb0-v+64];break
else:raise RuntimeError('No mapped source code')
(root/'sense32-code-control.json').write_text(json.dumps({'base':hex(base),'va':'0x299cb0','bytes':64,'native_matches_owned':True}))
args=[sys.executable,'-u','-I',str(tools/'d1-watchwrite.py'),'--exec','0x299cb0','--seconds','300','--max-hits','1','--out',str(root/'sense32-send.jsonl')]
for expr in ['rdx=','rdx!','rdx+0x18!','rcx=','rsp!','rdi!','rsi=']:args+=['--peek',expr]
with (root/'sense32-send-watch.log').open('x') as f:r=subprocess.run(args,stdout=f,stderr=subprocess.STDOUT,timeout=335)
assert r.returncode==0
print('Sense-send observer finished; verify actual hit and clean detach',flush=True)
