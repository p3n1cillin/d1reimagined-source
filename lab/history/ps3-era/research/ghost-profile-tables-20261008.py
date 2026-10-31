from pathlib import Path
import struct, sys, json
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from d1_pkg import PackageSet, Oodle, dyn_array, split_tag_hash, investment_items
rt=Path(__file__).parent.parent
paths=list((rt/'RESEARCH/ps3_updates/final/packages').glob('*.pkg'))+list((rt/'01_WORKING/ps3_destiny/PS3_GAME/USRDIR/packages').glob('*.pkg'))
ps=PackageSet(paths,Oodle(rt/'TOOLS/oodle/oo2core_3_win64_x86-64/oo2core_3_win64.dll'))
roots=[n for p in ps.by_id.values() for n in p.named if 'investment' in n['name']]
out=rt/'RESEARCH/ghost-profile-tables-20261008'
out.mkdir(exist_ok=True)
print('roots',roots)
for library_tag in (0x80a7d644,0x80a7d645):
    data=ps.tag(library_tag)
    count,first,kind=dyn_array(data,4)
    for index in range(count):
        name,resource=struct.unpack_from('>II',data,first+index*8)
        if name==0x0b30b1ec:
            print('Generalist appearance resource',hex(library_tag),hex(kind),index,hex(resource))
            content=ps.tag(resource)
            (out/f'generalist-appearance-{resource:x}.bin').write_bytes(content)
            print('resource header',len(content),content[:64].hex())
for info in roots:
    if info['name']!='investment_globals':continue
    raw=ps.tag(info['hash'])
    print('investment globals',hex(info['hash']),len(raw))
    if len(raw)<0x194:
        print('asset wrapper',raw.hex())
        wrapper=raw
        for pos in range(0,len(wrapper)-3,4):
            reference=struct.unpack_from('>I',wrapper,pos)[0]
            if 0x80800000<=reference<0x81800000 and reference>>16!=0x8080:
                data=ps.tag(reference)
                print('wrapper reference',hex(pos),hex(reference),len(data) if data else None)
                if data and len(data)>=0x194:
                    print('candidate headers',data[:32].hex(),[hex(struct.unpack_from('>I',data,p)[0]) for p in (0x144,0x170,0x190)])
                    if 0x80800000<=struct.unpack_from('>I',data,0x190)[0]<0x81800000:raw=data
        if len(raw)<0x194:continue
    for off in (0x144,0x170,0x190):
        tag=struct.unpack_from('>I',raw,off)[0]
        try:data=ps.tag(tag)
        except Exception as ex: print(hex(off),hex(tag),str(ex));continue
        print(hex(off),hex(tag),len(data) if data else None)
        if data:
            (out/f'assets-{off:x}-{tag:x}.bin').write_bytes(data)
            print('header',data[:80].hex())
            if off==0x190:
                count,first,kind=dyn_array(data,4)
                print('appearance table',count,hex(first),hex(kind))
                for index in (0,32,42):
                    row=data[first+index*24:first+(index+1)*24]
                    print('appearance',index,row.hex())
    (out/'asset-root.bin').write_bytes(raw)
_,items=investment_items(ps)
for index in (862,204):
    item=items[index];raw=ps.tag(item['tag']);print('item',item,'bytes',len(raw));print(raw.hex())
    (out/f'item-{index}.bin').write_bytes(raw)
