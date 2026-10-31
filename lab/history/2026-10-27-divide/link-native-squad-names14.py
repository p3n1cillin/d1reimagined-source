"""Link native squad ids to their owned content labels; check native bytes independently."""
import importlib.util,json,re,struct
from pathlib import Path
pvt=Path(__file__).resolve().parent
run=Path(r'F:\d1\d1preservation-main\cfw-staging\2026-10-27-divide14')
s=importlib.util.spec_from_file_location('g',r'F:\d1\public-work\d1reimagined-source\tools\ps4_setup\d1-winguest.py')
g=importlib.util.module_from_spec(s);s.loader.exec_module(g)
p,base=g.open_client()
reg=p.u(p.u(base+g.REGISTRY,'Q'),'Q')
def resolve(tag):
    esi=struct.unpack('<i',struct.pack('<I',tag))[0]>>13
    slot=(((esi&0xFFFFFFFF)>>18)|0x3FF)&esi&0xFFFFFFFF
    r=reg+slot*64
    data,esz,mask=p.u(r+0x10,'Q'),p.u(r+0x30,'i'),p.u(r+0x34,'i')&0xFFFFFFFFFFFFFFFF
    assert 0<esz<=0x100000
    e=data+(tag&0x1FFF)*esz
    return (e-(mask&p.u(e+8,'Q')))&0xFFFFFFFFFFFFFFFF
watch=[json.loads(l) for l in (run/'squad-name-links.jsonl').read_text().splitlines()]
components=sorted({int(x['rdi'],0) for x in watch if x.get('event')=='exec'})
rows=[]
for component in components:
    tag=p.u(component,'I')
    client=resolve(p.u(component+0x1A8,'I'))
    group,kind,sub=struct.unpack('<IBxh',p.read(client,8))
    assert (group,kind)==(0x9243DFFB,1)
    file=pvt/('squad4-owned-entry.bin' if tag==0x80CB8454 else f'owned-entry-{tag:08x}.bin')
    data=file.read_bytes()
    names=re.findall(rb'sq_[a-zA-Z0-9_]+',data)
    assert len(names)==1
    label=names[0]
    root=resolve(tag)
    offset=data.index(label)
    match=p.read(root+offset,len(label))==label
    row={'sub':sub,'tag':hex(tag),'label':label.decode(),'native_label_matches':match,
         'header_matches':p.read(root,8)==data[:8]}
    rows.append(row)
rows.sort(key=lambda x:x['sub'])
assert len(rows)==12
(run/'squad-label-links.json').write_text(json.dumps(rows,indent=2)+'\n')
for row in rows: print(row)
