"""Read-only code controls, then a bounded hardware trace at natural rifle pickup.

The canonical replay and idle guard own all pad input. This observer sends none.
"""
import hashlib
import importlib.util
import json
from pathlib import Path
import struct
import subprocess
import sys
import time

ROOT=Path(__file__).parent
RUN=Path(r'F:\d1\d1preservation-main\cfw-staging\weapon25-mag44')
TOOLS=Path(r'F:\d1\public-work\d1reimagined-source\tools\ps4_setup')
deadline=time.monotonic()+1500
while not (RUN/'replay-rifle-0-0.log').exists():
    if time.monotonic()>deadline:
        raise RuntimeError('Rifle approach did not start; no debugger attached')
    time.sleep(1)
spec=importlib.util.spec_from_file_location('g',TOOLS/'d1-winguest.py')
g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
p,base=g.open_client()
elf=Path(r'F:\d1\d1preservation-main\dumps\later-56578-CUSA00219\eboot.elf').read_bytes()
assert hashlib.sha256(elf).hexdigest()==g.ELF_SHA
phoff=struct.unpack_from('<Q',elf,0x20)[0]
phsize,phnum=struct.unpack_from('<HH',elf,0x36)
def code(va,n):
    for i in range(phnum):
        typ,flags,off,vaddr,paddr,size,memsz,align=struct.unpack_from('<II6Q',elf,phoff+i*phsize)
        if typ==1 and vaddr<=va and va+n<=vaddr+size:
            return elf[off+va-vaddr:off+va-vaddr+n]
    raise ValueError(hex(va))
controls=[]
for va,n in ((0x54dfc0,0x30),(0x54e14e,0x18),(0x910c60,0x29),(0x910da7,5),(0x910dac,8),(0x28a2f0,0x40)):
    assert p.read(base+va,n)==code(va,n),f'Owned/native code mismatch at {va:#x}'
    controls.append({'address':hex(va),'bytes':n,'native_matches_owned':True})
(ROOT/'run25-code-controls.json').write_text(json.dumps({'base':hex(base),'sha256':g.ELF_SHA,'controls':controls},indent=2))
args=[sys.executable,'-I',str(TOOLS/'d1-watchwrite.py'),'--seconds','240','--max-hits','12']
for va in ('0x54e14e','0x910c60','0x910dac','0x28a2f0'):
    args.extend(['--exec',va])
for expr in ('r8=','r8!','rsi=','rsi+8','r12=','r12+0x888!','rdi=','rdi!',
             'rbp-0xe0!','rsp!','r9=','r14=','r15='):
    args.extend(['--peek',expr])
args.extend(['--out',str(ROOT/'run25-pickup-trace.jsonl')])
print('Arming four read-only execution points for natural pickup',flush=True)
with (ROOT/'run25-pickup-trace.log').open('x') as output:
    result=subprocess.run(args,stdout=output,stderr=subprocess.STDOUT,timeout=280)
assert result.returncode==0,'Native trace failed; inspect preserved output'
print('Native watch completed and detached; inspect actual hits before making claims',flush=True)
