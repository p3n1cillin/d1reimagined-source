"""Bounded read-only snapshot. No debugger, calls, input, or writes to the client."""
import importlib.util, json, struct, sys, time
from pathlib import Path
sys.path.insert(0, r'lab/history\2026-10-28-static-combat-codex')
import static56578 as st
spec=importlib.util.spec_from_file_location('g',r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-winguest.py')
g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
p,base=g.open_client()
for va in (0x105330,0x17b380,0x17a5a0,0x103f70,0x138190):
    assert p.read(base+va,32)==st.read(va,32),hex(va)
reg=p.u(p.u(base+g.REGISTRY,'Q'),'Q')
def resolve(tag):
    if tag in (0,0xffffffff):return None
    esi=struct.unpack('<i',struct.pack('<I',tag))[0]>>13
    sl=(((esi&0xffffffff)>>18)|0x3ff)&esi&0xffffffff
    rec=reg+sl*64
    d,es,mk=p.u(rec+0x10,'Q'),p.u(rec+0x30,'i'),p.u(rec+0x34,'i')&0xffffffffffffffff
    if not d or not 0<es<0x100000:return None
    e=d+(tag&0x1fff)*es
    return (e-(mk&p.u(e+8,'Q')))&0xffffffffffffffff
w=p.u(base+g.WORLD_POOL,'Q');P=p.u(w+0x28,'Q');cnt=p.u(P+0x14,'i')
bm=p.u(p.u(p.u(P,'Q')+8,'Q')+0x10,'Q')
data,bo,esz,mask,flags=p.u(P+8,'Q'),p.u(P+0x1c,'I'),p.u(P+0x20,'I'),p.u(P+0x24,'I'),p.u(P+0x34,'I')
rows=[]
T=base+g.LT_TABLE
content=p.u(T+p.u(T,'q')+p.u(base+g.LT_COUNT,'H')*p.u(T+16,'Q')+0x30,'Q')
components={}
for kind in range(64):
    start,num=struct.unpack('<hh',p.read(content+0x2bd1c+kind*4,4))
    if not (0<=num<500 and 0<=start<4096):continue
    for j in range(num):
        dx=p.u(content+0x2a51c+2*(start+j),'H')
        rr=content+0x510+dx*0x38
        try:
            cp=resolve(p.u(rr+16,'I'))+p.u(rr+24,'Q')
            sn=resolve(p.u(cp+0x1a8,'I'))
            if sn and p.u(sn+8,'I')==0x80800357 and p.u(sn+4,'B')==1:
                components[sn]=(cp,kind)
        except (OSError,TypeError):pass
for slot in range(min(cnt,8192)):
    if not p.u(bm+4*(slot//32),'I')>>(slot%32)&1:continue
    hi=p.u(data+bo+esz*slot,'I')&mask
    tag=0x80000000|((hi&7)<<27)|((flags<<13)&0x7ffe000)|(slot&0x1fff) if flags&0x40000000 else ((hi<<23)&0x7f800000)|((flags<<13)&0x7fe000)|(slot&0x1fff)
    obj=resolve(tag)
    h,k,i,tb,ta=struct.unpack('<IBxhII',p.read(obj,16))
    if k!=1 or tb!=0x80800357 or h==0xfc0e45a3:continue
    pool=base+0x1ad5000;stride=p.u(pool+0x10,'Q');idx=p.u(obj+0x58,'H');off=p.u(obj+0x50,'Q')
    if idx==65535:continue
    at=(pool+p.u(pool,'q'))&0xffffffffffffffff
    image=p.u(at+stride*idx+0x30,'Q')+off
    if obj not in components:continue
    comp,kind=components[obj]
    back=resolve(p.u(comp+0x1a8,'I'))
    row=dict(group=hex(h),index=i,client=hex(obj),component=hex(comp),back_matches=back==obj,reported=p.u(image+12,'i'),members=[],desired=list(struct.unpack('<8h',p.read(comp+0x430,16))),existing=list(struct.unpack('<8i',p.read(comp+0x1cc,32))))
    n=p.u(comp+0x278,'i');row['member_count']=n
    row['component_kind']=kind
    if not (0<=n<=32 and back==obj):continue
    for mi in range(n):
        pair=p.read(comp+0x27c+mi*8,8);a,b=struct.unpack('<II',pair)
        ent=p.u(base+0x25d79a0+8,'Q')+(b&0x1fff)*p.u(base+0x25d79a0+0x10,'I')
        raw=p.read(ent,0x80)
        member=dict(pair=[hex(a),hex(b)],entity=hex(ent),raw=raw.hex(),objects=[])
        for o in (0x38,0x3c,0x40,0x44):
            handle=struct.unpack_from('<I',raw,o)[0]
            try:
                if o in (0x40,0x44):
                    tab=base+0x26897f0;e=p.u(tab+8,'Q')+(handle&0x1fff)*p.u(tab+16,'I')
                    obj2=(e-((p.u(tab+0x28,'i')&0xffffffffffffffff)&p.u(e+8,'Q')))&0xffffffffffffffff
                else:obj2=resolve(handle)
                ob=p.read(obj2,0x520) if obj2 else b''
                member['objects'].append(dict(offset=hex(o),handle=hex(handle),address=hex(obj2) if obj2 else None,header=ob[:8].hex(),self_handle=hex(struct.unpack_from('<I',ob,0x24)[0]) if ob else None,position=list(struct.unpack_from('<3f',ob,0x1a0)) if ob else None))
            except OSError as e:member['objects'].append(dict(offset=hex(o),handle=hex(handle),error=str(e)))
        row['members'].append(member)
        for o in (0x3c,0x40,0x44):
            hh=struct.unpack_from('<I',raw,o)[0]
            oo=resolve(hh)
            if oo:
                dest=Path(__file__).parent/('object-%d-%x.bin'%(i,o))
                dest.write_bytes(p.read(oo,0x1000))
                if o==0x3c:member['actor_unit30']=hex(p.u(oo+0x30,'I'))
    rows.append(row)
out=dict(unix=time.time(),base=hex(base),rows=rows)
dest=Path(__file__).parent/('members-%d.json'%time.time())
dest.write_text(json.dumps(out,indent=2))
print(dest)
print(json.dumps(rows,indent=2))
