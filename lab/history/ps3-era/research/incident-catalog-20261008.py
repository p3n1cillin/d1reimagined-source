import json
import re
import struct
import sys
from pathlib import Path
sys.path.insert(0, r"F:\d1\d1xenia\scripts")
from d1_pkg import PackageSet,Oodle,split_tag_hash,dyn_array
root=Path(__file__).parent
rt=root.parent
tag=int(sys.argv[1],0) if len(sys.argv)>1 else 0x80b1a06b
out=root/f'incident-catalog-{tag:08x}-20261008'
out.mkdir(exist_ok=False)
ps=PackageSet(list((root/'ps3_updates/final/packages').glob('*.pkg'))+
              list((rt/'01_WORKING/ps3_destiny/PS3_GAME/USRDIR/packages').glob('*.pkg')),
              Oodle(rt/'TOOLS/oodle/oo2core_3_win64_x86-64/oo2core_3_win64.dll'))
pid,index=split_tag_hash(tag)
p=ps.by_id[pid]
raw=p.entry_data(index,ps.oodle)
(out/f'{tag:08x}.bin').write_bytes(raw)
arrays=[]
for field in range(0,min(len(raw),0x80),4):
    try:
        count,offset,kind=dyn_array(raw,field)
    except (ValueError,struct.error):
        continue
    if 0<count<5000 and 0<=offset<len(raw):
        arrays.append({"field":hex(field),"count":count,"first":hex(offset),"kind":hex(kind),
                       "sample":raw[offset:offset+min(count*8,64)].hex()})
result={"tag":hex(tag),"class":hex(p.entries[index]['reference']),"size":len(raw),
        "header":raw[:128].hex(),"arrays":arrays,
        "strings":[s.decode('ascii') for s in re.findall(rb'[\x20-\x7e]{4,}\x00',raw)][:8],
        "known_hash_locations":{hex(h):[hex(i) for i in range(0,len(raw)-3,4) if struct.unpack_from('>I',raw,i)[0]==h]
                                for h in (0x790c2a95,0x7a0fd954,0x1124697d,0x84f68b44,0xaeca3822,0x324e9a20)}}
(out/'root.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result))
