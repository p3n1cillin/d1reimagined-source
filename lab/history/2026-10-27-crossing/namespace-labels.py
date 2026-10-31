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
records=[]
for kind in range(0x40):
 start,count=struct.unpack('<hh',p.read(content+0x2bd1c+4*kind,4))
 if not 0<count<1000: continue
 labels=[]
 for j in range(count):
  dx=p.u(content+0x2a51c+2*(start+j),'H'); row=p.read(content+0x510+dx*0x38,0x38)
  group,k,sub=struct.unpack_from('<IBxh',row)
  if dx < 209: continue
  for tag in (p.u(content+0x510+dx*0x38+0x10,'I'),p.u(content+0x510+dx*0x38+0x28,'I')):
   root=resolve(tag)
   if not root: continue
   try:
    off=p.u(content+0x510+dx*0x38+(0x18 if tag == p.u(content+0x510+dx*0x38+0x10,'I') else 0x30),'Q')
    component=root+off
    asset=p.u(component,'I')
    if 0x80000000 <= asset < 0x90000000:
     owned=resolve(asset)
     if owned: root=owned; tag=asset
    data=p.read(root,0x4000)
   except OSError: continue
   words=[x.decode() for x in re.findall(rb'(?:fa_|nav_|path_|sq_|o_)[a-zA-Z0-9_]{3,}',data)]
   item={'kind':kind,'sub':sub,'tag':hex(tag),'root':hex(root),'head':data[:16].hex(),'labels':words}
   if words: labels.append(item)
   records.append(item)
 print(hex(kind),'count',count,'labels',labels[:10])
Path(__file__).with_name('namespace-labels.json').write_text(json.dumps(records,indent=2))
