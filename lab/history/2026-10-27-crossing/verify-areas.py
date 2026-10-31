"""Read-only 56578 namespace probe; current process and base are verified by d1-winguest."""
import ctypes, importlib.util, json, struct, sys
from pathlib import Path
src = Path(r'F:\d1\public-work\d1reimagined-source\tools\ps4_setup\d1-winguest.py')
s = importlib.util.spec_from_file_location('winguest', src)
g = importlib.util.module_from_spec(s); s.loader.exec_module(g)
p, base = g.open_client()
T = base + g.LT_TABLE
index = p.u(base + g.LT_COUNT, 'H')
content = p.u(T + p.u(T, 'q') + index * p.u(T+0x10, 'Q') + 0x30, 'Q')
print(f'content {content:#x} index {index}; header {p.read(content, 16).hex(" ")}')
reg = p.u(p.u(base+g.REGISTRY, 'Q'), 'Q')
def resolve(tag):
    if tag == 0xffffffff: return None
    esi = struct.unpack('<i', struct.pack('<I',tag))[0] >> 13
    slot = (((esi & 0xffffffff)>>18)|0x3ff) & esi & 0xffffffff
    rec = reg + slot*0x40
    data, stride, mask = p.u(rec+0x10,'Q'), p.u(rec+0x30,'i'), p.u(rec+0x34,'i')
    if not data or not 0 < stride <= 0x100000: return None
    e = data + (tag & 0x1fff)*stride
    return e - ctypes.c_int64(mask & p.u(e+8,'Q')).value

import re
root=Path(__file__).parent
b=(root/'owned-firing-areas-80cb8422.bin').read_bytes(); native=p.read(resolve(0x80cb8422),len(b))
assert b[:8]==native[:8]
areas=[]
for i in range(18):
 off=0x640+0x80*i; nameoff=off+struct.unpack_from('<q',b,off)[0]
 label=b[nameoff:b.index(0,nameoff)].decode() if 0<=nameoff<len(b) and any(b[off:off+8]) else '(unnamed)'
 low=struct.unpack_from('<3f',b,off+0x10); high=struct.unpack_from('<3f',b,off+0x20)
 assert b[off:off+0x30]==native[off:off+0x30]
 areas.append({'index':i,'name':label,'low':low,'high':high,'record':hex(off),'native_match':True})
for m in re.finditer(rb'fa_[a-zA-Z0-9_]+',b): assert b[m.start():m.end()]==native[m.start():m.end()]
for tag in (0x80cb844e,0x80cb8450,0x80cb8452):
 owned=(root.parent/'2026-10-27-divide'/f'owned-label-{tag:#x}.bin').read_bytes()
 assert owned[0x510:0x51c]==p.read(resolve(tag)+0x510,12)
 print(hex(tag),'position matched',struct.unpack_from('<3f',owned,0x510))
elf=Path(r'F:\d1\d1preservation-main\dumps\later-56578-CUSA00219\eboot.elf').read_bytes()
phoff=struct.unpack_from('<Q',elf,0x20)[0]; phsize,phnum=struct.unpack_from('<HH',elf,0x36)
def code(va,n):
 for i in range(phnum):
  typ,flags,foff,vaddr,paddr,filesz,memsz,align=struct.unpack_from('<II6Q',elf,phoff+i*phsize)
  if typ==1 and vaddr<=va and va+n<=vaddr+filesz: return elf[foff+va-vaddr:foff+va-vaddr+n]
 raise ValueError(hex(va))
for va,n in ((0x105840,0x6c),(0x17950e,5),(0x1795a0,0x12),(0x125270,0x38)):
 expected=code(va,n); assert expected==p.read(base+va,n); print('native code matches owned',hex(va),n)
call=code(0x17950e,5); assert call[0]==0xe8 and 0x17950e+5+struct.unpack_from('<i',call,1)[0]==0x105840
(root/'firing-areas-verified.json').write_text(json.dumps(areas,indent=2))
print('all18 area records,15 label bytes,3 prop positions and4 native code blocks independently matched')
for a in areas: print(a['index'],a['name'],tuple(round(x,2) for x in a['low']),tuple(round(x,2) for x in a['high']))
