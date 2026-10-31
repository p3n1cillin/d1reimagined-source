from pathlib import Path
import importlib.util,hashlib,struct,json,subprocess,sys
root=Path(r'lab/history\2026-10-28-character-stats')
tools=Path(r'F:\d1\public-work\d1reimagined-source\tools\ps4_setup')
s=importlib.util.spec_from_file_location('g',tools/'d1-winguest.py');g=importlib.util.module_from_spec(s);s.loader.exec_module(g)
import time
start=time.time();deadline=start+1200
log=Path(g.LOG)
while True:
 if log.exists() and log.stat().st_mtime>=start:
  text=log.read_text(encoding='utf-8',errors='replace')
  if "Entering state 'setup:activity_world_transition'" in text:break
 if time.time()>deadline:raise RuntimeError('No fresh world transition; no observer attached')
 time.sleep(2)
p,base=g.open_client();b=Path(r'F:\d1\d1preservation-main\dumps\later-56578-CUSA00219\eboot.elf').read_bytes();assert hashlib.sha256(b).hexdigest()==g.ELF_SHA
phoff=struct.unpack_from('<Q',b,0x20)[0];size,n=struct.unpack_from('<HH',b,0x36)
def code(va,length):
 for i in range(n):
  typ,flags,off,vaddr,paddr,fs,ms,align=struct.unpack_from('<II6Q',b,phoff+i*size)
  if typ==1 and vaddr<=va and va+length<=vaddr+fs:return b[off+va-vaddr:off+va-vaddr+length]
 raise ValueError(hex(va))
controls=[]
for va in [0x9128ec,0x9128f1,0x91297a,0x912991,0x553580,0x5dd050,0x281c40,0x26f7e0,0x281a20]:
 assert p.read(base+va,16)==code(va,16),hex(va)
 controls.append({'va':hex(va),'bytes':16,'native_matches_owned':True})
(root/'run27-character-code-controls.json').write_text(json.dumps({'base':hex(base),'sha256':g.ELF_SHA,'controls':controls},indent=2))
args=[sys.executable,'-I',str(tools/'d1-watchwrite.py'),'--seconds','360','--max-hits','8']
for va in ['0x9128f1','0x91297a','0x912991']:args+=['--exec',va]
for e in ['rbp-0x130!','rbp-0x128!','rbx=','r14=','r15=','r12!','r13=','r13+0x20!','r13+0x40!','r13+0x60!','r13+0x80!','r13+0xa0!','r13+0xc0!']:args+=['--peek',e]
args+=['--out',str(root/'run27-character-baseline.jsonl')]
with (root/'run27-character-baseline-watch.log').open('x',encoding='utf-8') as out:
 result=subprocess.run(args,stdout=out,stderr=subprocess.STDOUT,timeout=405)
assert result.returncode==0,result.returncode
print('Character observer completed and cleanly detached; inspect hits.',flush=True)
