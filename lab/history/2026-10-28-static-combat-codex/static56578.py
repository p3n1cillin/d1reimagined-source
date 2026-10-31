"""Owned-image analysis only. Never attaches, writes memory, or sends game input."""
from pathlib import Path
import argparse, bisect, hashlib, json, re, struct
from capstone import Cs, CS_ARCH_X86, CS_MODE_64

ROOT=Path(__file__).parent
IMAGE=Path(r'F:\d1\d1preservation-main\dumps\later-56578-CUSA00219\eboot.elf')
SHA='672f03411c0503dfbcda804cbb9fb27cdd9a8d73a8bcb71bf0c4f688073b0833'
B=IMAGE.read_bytes()
assert hashlib.sha256(B).hexdigest()==SHA
phoff=struct.unpack_from('<Q',B,0x20)[0]
ents,n=struct.unpack_from('<HH',B,0x36)
SEGS=[]
for i in range(n):
    t,f,o,a,_,sz,_,_=struct.unpack_from('<IIQQQQQQ',B,phoff+i*ents)
    if t==1: SEGS.append((a,o,sz,f))
MD=Cs(CS_ARCH_X86,CS_MODE_64); MD.skipdata=True
RELOCS={}
dyn=dict(struct.unpack_from('<QQ',B,o) for o in range(0x1be30e0,0x1be30e0+0xd70,16))
rela=0x1a25580+dyn[0x6100002f]
for o in range(rela,rela+dyn[0x61000031],24):
    dest,info,value=struct.unpack_from('<QQq',B,o)
    if info==8:RELOCS[dest]=value
def read(va,n):
    for a,o,s,f in SEGS:
        if a<=va and va+n<=a+s:return B[o+va-a:o+va-a+n]
    raise ValueError(hex(va))
def cstr(va,limit=200):
    try:return read(va,limit).split(b'\0',1)[0].decode('utf8','replace')
    except ValueError:return ''
def pointer(va):return RELOCS.get(va,struct.unpack('<Q',read(va,8))[0])
def ptrrefs(target):return [a for a,v in RELOCS.items() if v==target]
EH=0x1869930
assert read(EH,4)==b'\x01\x1b\x03\x3b'
NF=struct.unpack('<I',read(EH+8,4))[0]
FUNCS=[EH+struct.unpack('<i',read(EH+12+8*i,4))[0] for i in range(NF)]
def func(va):
    i=bisect.bisect_right(FUNCS,va)-1
    return FUNCS[i],FUNCS[i+1] if i+1<len(FUNCS) else va+0x100
def dis(va,n):
    rows=[]
    for a,s,m,op in MD.disasm_lite(read(va,n),va):
        tail=''
        rip=re.search(r'\[rip ([+-]) (0x[0-9a-f]+)\]',op)
        if rip:
            target=a+s+int(rip[2],16)*(1 if rip[1]=='+' else -1)
            text=cstr(target)
            tail=f' ; [{target:#x}]'
            if text and text.isascii() and all(c.isprintable() for c in text):tail+=' '+repr(text)
        rows.append(f'{a:#010x} {m:9s} {op}{tail}')
    return '\n'.join(rows)
def index():
    p=ROOT/'owned-xrefs.json'
    if p.exists():return json.loads(p.read_text())
    rows=[];count=0
    for a,o,s,f in SEGS:
        if not f&1:continue
        for va,sz,m,op in MD.disasm_lite(B[o:o+s],a):
            count+=1
            if m in ('call','jmp') and op.startswith('0x'):
                rows.append([va,int(op,16),m])
            for match in re.finditer(r'\[rip ([+-]) (0x[0-9a-f]+)\]',op):
                target=va+sz+int(match[2],16)*(1 if match[1]=='+' else -1)
                rows.append([va,target,'rip'])
    p.write_text(json.dumps({'sha':SHA,'instructions':count,'refs':rows}))
    return json.loads(p.read_text())
def strings(pattern):
    q=re.compile(pattern,re.I);out=[]
    for a,o,s,f in SEGS:
        for hit in re.finditer(rb'[\x20-\x7e]{5,}',B[o:o+s]):
            txt=hit[0].decode()
            if q.search(txt):out.append((a+hit.start(),txt))
    return out
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('mode',choices=['dis','refs','strings','index']);p.add_argument('value',nargs='?');p.add_argument('--size',default='0x300');p.add_argument('--out')
    args=p.parse_args()
    if args.mode=='dis':out=dis(int(args.value,0),int(args.size,0))
    elif args.mode=='strings':out='\n'.join(f'{a:#x} {t}' for a,t in strings(args.value))
    elif args.mode=='index':
        x=index();out=json.dumps({'instructions':x['instructions'],'refs':len(x['refs']),'sha':x['sha']})
    else:
        target=int(args.value,0);out='\n'.join(f'{va:#x} {kind} {t:#x}' for va,t,kind in index()['refs'] if target<=t<target+int(args.size,0))
    if args.out:
        dest=ROOT/args.out
        if dest.exists():raise FileExistsError(dest)
        dest.write_text(out,encoding='utf8')
    else:print(out)
