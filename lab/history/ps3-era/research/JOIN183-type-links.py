import sys,json,struct
from pathlib import Path
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from d1_eboot import Eboot
from d1_pkg import PackageSet,Oodle,split_tag_hash
root=Path(__file__).parent;rt=root.parent
eb=Eboot(root/'ps3_updates/01.24/USRDIR/EBOOT.elf')
for h in (0x80802b49,0x80802b63,0x80aa9a07,0x808021a9,0x80802001):
    print(hex(h),'data references',list(eb.data_refs(h))[:32])
pkg,index=split_tag_hash(0x80aa9a07)
paths=list((root/'ps3_updates/final/packages').glob(f'*_{pkg:04x}_*.pkg'))
paths+=list((rt/'01_WORKING/ps3_destiny/PS3_GAME/USRDIR/packages').glob(f'*_{pkg:04x}_*.pkg'))
print('package',hex(pkg),'files',len(paths),'entry',hex(index))
if paths:
    ps=PackageSet(paths,Oodle(rt/'TOOLS/oodle/oo2core_3_win64_x86-64/oo2core_3_win64.dll'))
    data=ps.tag(0x80aa9a07)
    if data is not None:
        out=root/'JOIN183-cinematic-static/80aa9a07.bin';out.write_bytes(data)
        print('type-tag bytes',len(data))
        for off in range(0x700,min(0x760,len(data)),16):print(hex(off),data[off:off+16].hex())
