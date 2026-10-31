from pathlib import Path
import runpy,struct,ctypes,json,datetime,math
P=Path(__file__).parent;s=runpy.run_path(str(P/'read-damage-unit.py'));p,base,resolve,key=s['p'],s['base'],s['resolve'],s['key'];cache={}
def component_offset(root):
 h=p.u(root+8,'I');off=p.u(root+0x10,'Q');ident=(h,off)
 if ident in cache:return cache[ident]
 c=resolve(h)+off+0x38;size=p.u(c+8,'I');assert 0<size<=4096 and size&(size-1)==0
 a=((key<<4)+(p.u(c,'I')^key))&0xffffffff;a^=a>>10;a=(a*0x81)&0xffffffff;a^=a>>13;slot=a&(size-1);slots=c+0x20+p.u(c+0x10,'q');found=None
 for j in range(size):
  k,start,flags=struct.unpack('<Ihh',p.read(slots+slot*8,8))
  if k==key:
   assert flags&0x7fff==1
   rec=c+0x30+p.u(c+0x20,'q')+start*0x28;found=p.u(rec+4,'i');assert 0<=found<0x100000;break
  if flags>=0:break
  slot=(slot+1)&(size-1)
 cache[ident]=found;return found
T=base+0x2692220;stride=p.u(T+16,'I');n=p.u(T+20,'I');data=p.u(T+8,'Q');assert stride==240 and 0<n<=8192
raw=p.read(data,stride*n);tab=base+0x26897f0;entries=p.u(tab+8,'Q');objmask=p.u(tab+40,'i');assert p.u(tab+16,'I')==32
rows=[];errors=0
for i in range(n):
 ph=struct.unpack_from('<I',raw,i*stride+0x54)[0]
 if ph in (0,0xffffffff):continue
 try:
  e=entries+(ph&0x1fff)*32;root=e-ctypes.c_int64(objmask&p.u(e+8,'Q')).value
  if not root:continue
  off=component_offset(root)
  if off is None:continue
  comp=root+off;eh=p.u(comp+0x2c,'I')
  if eh&0x1fff!=i or p.u(data+i*stride+0x54,'I')!=ph:continue
  power=struct.unpack('<3f',p.read(comp+0x94,12));mode=p.u(comp+0xb4,'B');cat=p.u(comp+0xa6,'B')
  if not all(math.isfinite(x) and -10000<=x<=10000 for x in power):continue
  rows.append({'index':i,'entity':hex(eh),'parent':hex(ph),'component':hex(comp),'owned':hex(p.u(comp,'I')),'power':power,'category_a6':cat,'mode_b4':mode})
 except (OSError,AssertionError,ValueError):errors+=1
out={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'capacity':n,'rows':rows,'refused_rows':errors,'read_only':True};(P/'sense40-all-power-blocks.json').write_text(json.dumps(out,indent=2));print(json.dumps(out,indent=2))
