from pathlib import Path
import sys,struct,json,re
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from d1_pkg import PackageSet,Oodle,dyn_array,split_tag_hash
from d1_eboot import Eboot
rt=Path(__file__).parent.parent
ps=PackageSet(list((rt/'RESEARCH/ps3_updates/final/packages').glob('*.pkg'))+list((rt/'01_WORKING/ps3_destiny/PS3_GAME/USRDIR/packages').glob('*.pkg')),Oodle(rt/'TOOLS/oodle/oo2core_3_win64_x86-64/oo2core_3_win64.dll'))
eb=Eboot(rt/'RESEARCH/ps3_updates/01.24/USRDIR/EBOOT.elf')
out=rt/'RESEARCH/ghost-profile-tables-20261008'
dye=(out/'globals-178-80a5f22c.bin').read_bytes()
count,first,kind=dyn_array(dye,4)
targets={struct.unpack_from('>I',dye,first+i*4)[0]:i for i in range(4)}
results=[]
for tag in (0x80a7d644,0x80a7d645):
    data=ps.tag(tag);count,first,kind=dyn_array(data,4)
    for index in range(count):
        name,resource=struct.unpack_from('>II',data,first+index*8)
        if name in targets:
            package_id,entry_index=split_tag_hash(resource)
            p=ps.by_id[package_id]
            raw=ps.tag(resource)
            row={'dye_index':targets[name],'name':hex(name),'library':hex(tag),'resource':hex(resource),'class':hex(p.entries[entry_index]['reference']),'length':len(raw),'head':raw[:128].hex()}
            results.append(row)
            (out/f'dye-{targets[name]}-{resource:x}.bin').write_bytes(raw)
print(json.dumps(results))
for marker in (0x808033fa,0x8080340e):
    print('type-refs',hex(marker),eb.code_refs(marker),eb.data_refs(marker)[:20])
for tag in (0x80a6a008,0x80cb8243):
    raw=ps.tag(tag)
    print('unit/member',hex(tag),len(raw),raw[:128].hex())
    refs=[]
    for off in range(0,len(raw)-3,4):
        handle=struct.unpack_from('>I',raw,off)[0]
        if not 0x80802000<=handle<0x81800000:continue
        key=split_tag_hash(handle)
        package=ps.by_id.get(key[0]) if key else None
        if package and key[1]<len(package.entries):
            refs.append((hex(off),hex(handle),hex(package.entries[key[1]]['reference'])))
    print('references',refs[:60])
