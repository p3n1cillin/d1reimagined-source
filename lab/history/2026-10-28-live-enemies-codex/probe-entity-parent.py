import runpy,struct,json
from pathlib import Path
z=runpy.run_path(str(Path(__file__).with_name('probe-members.py')))
p,base,resolve=z['p'],z['base'],z['resolve']
for r in z['rows']:
 for m in r['members']:
  eh=int(m['objects'][1]['handle'],16);t=base+0x2692220
  e=p.u(t+8,'Q')+(eh&0x1fff)*p.u(t+16,'I');ph=p.u(e+0x54,'I')
  root=resolve(ph)
  print('PARENT',r['index'],hex(eh),hex(ph),hex(root))
  h=p.u(root+8,'I');off=p.u(root+0x10,'Q');definition=resolve(h)+off;c=definition+0x38
  size=p.u(c+8,'I');print('SIZE',size)
  slots=c+0x20+p.u(c+0x10,'q');records=c+0x30+p.u(c+0x20,'q')
  for j in range(min(size,512)):
   key,start,fl=struct.unpack('<Ihh',p.read(slots+j*8,8))
   if key==0xffffffff or start<0:continue
   for k in range(fl&0x7fff):
    rec=records+(start+k)*0x28;co=p.u(rec+4,'i')
    comp=root+co
    print('COMP',hex(key),hex(co),hex(p.u(comp,'I')),hex(p.u(comp+0x24,'I')),hex(p.u(comp+0x2c,'I')))
  break
 break
