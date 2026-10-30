import os, struct, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import d1paths  # sibling: local paths from D1_ROOT or paths.json
src=open(d1paths.tools() / 'd1-winguest.py').read(); wg={'__name__':'lib'}; exec(compile(src,'w','exec'),wg)
p,base=wg['open_client']()
mgr=p.u(base+0x26CA088,'Q'); tag=p.u(mgr+0x278,'I')
print('mgr',hex(mgr),'scenario tag',hex(tag))
esi=struct.unpack('<i',struct.pack('<I',tag))[0]>>13
sl=(((esi&0xFFFFFFFF)>>0x12)|0x3FF)&esi&0xFFFFFFFF
reg=p.u(p.u(base+0x1A238A0,'Q'),'Q')
rec=reg+sl*0x40
d,es,mk=p.u(rec+0x10,'Q'),p.u(rec+0x30,'i'),p.u(rec+0x34,'i')&0xFFFFFFFFFFFFFFFF
e=d+(tag&0x1fff)*es; obj=e-(mk&p.u(e+8,'Q'))
cnt=p.u(obj+0x48,'q'); arr=obj+0x50+p.u(obj+0x50,'q')
print('scenario obj',hex(obj),'checksum',hex(p.u(obj+0xc,'I')),'bubbles',cnt)
for i in range(cnt):
    be=arr+0x10+i*0x18
    n=p.u(be+0x8,'i')
    if n==0: print(i,'subs 0'); continue
    sa=be+0x10+p.u(be+0x10,'q')
    subs=[p.read(sa+j*0x3c,0x3c) for j in range(n)]
    print(i,'subs',n,'public',[s[0x1c] for s in subs])
