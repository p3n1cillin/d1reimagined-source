from pathlib import Path
import sys,struct,json
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from d1_eboot import Eboot
from d1_pkg import PackageSet,Oodle,dyn_array
rt=Path(__file__).parent.parent
eb=Eboot(rt/'RESEARCH/ps3_updates/01.24/USRDIR/EBOOT.elf')
for pc,count in ((0x2a64c4,58),(0x4c5ec8,36)):
    (rt/f'RESEARCH/JOIN195-static-{pc:x}.txt').write_text('\n'.join(line for address in range(pc,pc+4*count,4) for line in eb.disasm(address,1))+'\n',encoding='utf-8')
ps=PackageSet(list((rt/'RESEARCH/ps3_updates/final/packages').glob('*.pkg'))+list((rt/'01_WORKING/ps3_destiny/PS3_GAME/USRDIR/packages').glob('*.pkg')),Oodle(rt/'TOOLS/oodle/oo2core_3_win64_x86-64/oo2core_3_win64.dll'))
out=rt/'RESEARCH/ghost-profile-tables-20261008'
root=(out/'asset-root.bin').read_bytes()
for off in (0xd8,0xe0,0x178,0x180,0x188):
    tag=struct.unpack_from('>I',root,off)[0]
    data=ps.tag(tag)
    if data is None:
        print('unavailable',hex(off),hex(tag))
        continue
    (out/f'globals-{off:x}-{tag:x}.bin').write_bytes(data)
    print('table',hex(off),hex(tag),len(data),data[:100].hex())
model=(out/'assets-170-80a5f22b.bin').read_bytes()
count,first,kind=dyn_array(model,4)
print('model2706',count,hex(first),hex(kind),model[first+2706*16:first+2707*16].hex())
for handle in (0x80a74483,):
    data=ps.tag(handle);(out/f'ghost-model-{handle:x}.bin').write_bytes(data)
    print('ghostmodel',hex(handle),len(data),data[:200].hex())
dye=(out/'globals-178-80a5f22c.bin').read_bytes()
count,first,kind=dyn_array(dye,4)
names=[struct.unpack_from('>I',dye,first+i*4)[0] for i in range(count)]
for target in (0x17f84eca,0xefcd14be,0x0b30b1ec):
    print('name-index',hex(target),[i for i,name in enumerate(names) if name==target])
assets=ps.tag(0x80a7d644)
count,first,kind=dyn_array(assets,4)
for i in range(count):
    name,handle=struct.unpack_from('>II',assets,first+i*8)
    if name in (0x17f84eca,0xefcd14be):
        print('named-resource',hex(name),hex(handle),i)
