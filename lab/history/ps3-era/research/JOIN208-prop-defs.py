from pathlib import Path
import sys,struct,json,collections
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from d1_pkg import PackageSet,Oodle,split_tag_hash,dyn_array
rt=Path(__file__).parent.parent
ps=PackageSet(list((rt/'RESEARCH/ps3_updates/final/packages').glob('*.pkg'))+list((rt/'01_WORKING/ps3_destiny/PS3_GAME/USRDIR/packages').glob('*.pkg')),Oodle(rt/'TOOLS/oodle/oo2core_3_win64_x86-64/oo2core_3_win64.dll'))
out=Path(__file__).parent/'JOIN208-static'
classes=collections.Counter()
rows=[]
def get(tag):
    pid,idx=split_tag_hash(tag);pk=ps.by_id[pid]
    return pk.entry_data(idx,ps.oodle),pk.entries[idx]['reference']
for tag in range(0x80cb817b,0x80cb8203):
    data,cls=get(tag)
    subtype=struct.unpack_from('>I',data,0x14)[0] if len(data)>=24 else 0
    classes[hex(subtype)]+=1
    if subtype != 0x8080092e: continue
    child=struct.unpack_from('>I',data,8)[0]
    raw,childcls=get(child)
    (out/f'{child:08x}.bin').write_bytes(raw)
    arrays=[]
    for field in range(4,min(len(raw),0x90),4):
        try:
            n,off,arrcls=dyn_array(raw,field)
            arrays.append({'field':hex(field),'count':n,'offset':hex(off),'class':hex(arrcls),'first':raw[off:off+128].hex()})
        except (ValueError,struct.error):pass
    row={'tag':hex(tag),'header':data.hex(),'type':hex(subtype),'child':hex(child),'child_class':hex(childcls),'child_header':raw[:160].hex(),'arrays':arrays}
    rows.append(row)
    print(json.dumps(row))
print('classes',dict(classes))
(out/'props.json').write_text(json.dumps(rows,indent=2))
