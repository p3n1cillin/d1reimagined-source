"""Bounded byte-pattern leads in owned executable; known instruction is the control."""
import struct
from pathlib import Path
b=Path(r'F:\d1\d1preservation-main\dumps\later-56578-CUSA00219\eboot.elf').read_bytes()
assert b[:4]==b'\x7fELF' and b[4:6]==b'\x02\x01'
phoff=struct.unpack_from('<Q',b,0x20)[0]
size,count=struct.unpack_from('<HH',b,0x36)
sites=[]
for i in range(count):
    typ,flags,off,addr,_,filesz,_,_=struct.unpack_from('<IIQQQQQQ',b,phoff+i*size)
    if typ!=1 or not flags&1:continue
    start=max(0,0x1B8000-addr)
    end=min(filesz,0x1C0000-addr)
    if start>=end:continue
    data=b[off+start:off+end]
    at=0
    while (hit:=data.find(b'\xa8\x0a\x00\x00',at))!=-1:
        sites.append(addr+start+hit)
        at=hit+1
assert 0x1BCF4D in sites, 'Known lea r14,[rbx+0xaa8] operand control failed'
print('Known instruction operand control PASS; candidate operands (not yet proven writes):')
print(' '.join(hex(s) for s in sites))
