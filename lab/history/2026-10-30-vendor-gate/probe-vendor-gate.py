"""Read-only sample of the gates in front of the vendor screen (0xa96110). READ + QUERY only, no input.
  0x9231e0: [0x273eb80] -> dword +0x1c+4*i must be <= 3
  0x906f70: [0x273dcd0] bytes 0/1 and [[0x273dcd8]] -> must read false (UI busy)
  0x58ab80: [0x26527e0] ready (vtable[0]), [0x26527e8] vtable+0x2d8 -> vendor table (count, 24-byte rows, hash at +0x10)
"""
from pathlib import Path
import runpy,struct,json,sys,time
P=Path(__file__).parent
v=runpy.run_path(str(P/'attach-readonly.py'));u,rd,b=v['u'],v['rd'],v['base']
out={'time':time.time()}
g=u(b+0x273eb80); out['players']=[u(g+0x1c+4*i,'i') for i in range(4)] if g else None
busy=u(b+0x273dcd0); out['busy']=None
if busy:
    c=u(b+0x273dcd8); out['busy']={'b0':u(busy,'B'),'b1':u(busy+1,'B'),'count':u(c,'i') if c else None}
r=u(b+0x26527e0); out['ready_obj']=hex(r)
if r:
    vt=u(r); out['ready']={'vtable':hex(vt-b),'fn0':hex(u(vt)-b),'field_c':u(r+0xc,'i')}
i=u(b+0x26527e8); out['investment']=hex(i)
if i:
    vt=u(i); out['investment_vtable']=hex(vt-b); out['fn_2d8']=hex(u(vt+0x2d8)-b)
print(json.dumps(out,indent=1))
(P/f'vendor-gate-{time.time_ns()}.json').write_text(json.dumps(out,indent=1))
t=u(b+0x26527f8+0x290); out2={'table':hex(t)}
if t:
    a=t+8; n=u(a,'i'); base=u(a+8)+(a+8)
    out2['count']=n; out2['hashes']=[f"{u(base+24*k+0x10-0x10+0x10,'I'):08x}" for k in range(min(n,128))]
print(json.dumps({k:(v if k!='hashes' else v[:8]+['...']) for k,v in out2.items()},indent=1))
(P/f'vendor-table-{time.time_ns()}.json').write_text(json.dumps(out2,indent=1))
# The cache above fills on first use; resolve the same tag handles directly (0x25cc50, 0x25fd30).
def resolve(h):
    e=h>>13 if h<0x80000000 else (h-(1<<32))>>13
    s=((e&0xffffffff)>>18|0x3ff)&e&0xffffffff
    ent=u(u(b+0x1a238a0))+s*64; data=u(ent+0x10); stride=u(ent+0x30,'i'); mask=u(ent+0x34,'i')&0xffffffffffffffff
    p=data+(h&0x1fff)*stride; return p-(mask&u(p+8))
inv=u(b+0x26527e8); defs=u(b+0x26527f8) or resolve(u(inv+0xc,'I'))
th=u(defs+0x578,'I'); t=resolve(th); a=t+8; n=u(a,'i'); base=u(a+8)+(a+8)
out3={'defs':hex(defs),'table_handle':hex(th),'count':n,'hashes':[f"{u(base+24*k+0x10,'I'):08x}" for k in range(min(n,128))]}
print('resolved', out3['table_handle'], out3['count'], out3['hashes'][:4])
(P/f'vendor-table-resolved-{time.time_ns()}.json').write_text(json.dumps(out3,indent=1))
