from pathlib import Path
import sys,struct,json,re
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from d1_pkg import PackageSet,Oodle,split_tag_hash,dyn_array
rt=Path(__file__).parent.parent
ps=PackageSet(list((rt/'RESEARCH/ps3_updates/final/packages').glob('*.pkg'))+list((rt/'01_WORKING/ps3_destiny/PS3_GAME/USRDIR/packages').glob('*.pkg')),Oodle(rt/'TOOLS/oodle/oo2core_3_win64_x86-64/oo2core_3_win64.dll'))
out=Path(__file__).parent/'JOIN209-rifle-static'
out.mkdir(exist_ok=False)
def get(tag):
    where=split_tag_hash(tag)
    if not where:return None,None
    pid,idx=where;pk=ps.by_id.get(pid)
    if not pk or idx>=len(pk.entries):return None,None
    raw=pk.entry_data(idx,ps.oodle)
    return raw,pk.entries[idx]['reference']
rows=[]
for tag in (0x80cb828c,0x80cb82a8,0x80cc0528):
    raw,cls=get(tag)
    (out/f'{tag:08x}.bin').write_bytes(raw)
    n,off,kind=dyn_array(raw,0x10)
    print(hex(tag),'unit class',hex(cls),'components',n,'header',raw[:96].hex())
    components=[]
    for i in range(n):
        refs=struct.unpack_from('>III',raw,off+i*12)
        row={'index':i,'refs':[hex(t) for t in refs],'resources':[]}
        for t in refs:
            d,c=get(t)
            if d is None:continue
            names=[s.decode('latin1') for s in re.findall(rb'[\x20-\x7e]{5,}\x00',d)]
            sub=[]
            for field in range(0,min(len(d),512)-3,4):
                v=struct.unpack_from('>I',d,field)[0]
                if not 0x80810000<=v<0x81800000:continue
                sd,sc=get(v)
                if sd is None or len(sd)>65536:continue
                sn=[s.decode('latin1') for s in re.findall(rb'[\x20-\x7e]{5,}\x00',sd)]
                if sn:sub.append({'field':hex(field),'tag':hex(v),'names':sn[:15]})
            row['resources'].append({'tag':hex(t),'class':hex(c),'bytes':len(d),'header':d[:128].hex(),'names':names[:15],'children':sub})
            if names or sub:print('component',i,hex(t),names[:5],json.dumps(sub[:6]))
            (out/f'{t:08x}.bin').write_bytes(d)
        components.append(row)
    rows.append({'tag':hex(tag),'count':n,'components':components})
(out/'components.json').write_text(json.dumps(rows,indent=2))
