import sys,struct,json,hashlib
from pathlib import Path
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from d1_pkg import PackageSet,Oodle
root=Path(__file__).parent;rt=root.parent
paths=list((root/'ps3_updates/final/packages').glob('*025c*.pkg'))
paths+=list((rt/'01_WORKING/ps3_destiny/PS3_GAME/USRDIR/packages').glob('*025c*.pkg'))
ps=PackageSet(paths,Oodle(rt/'TOOLS/oodle/oo2core_3_win64_x86-64/oo2core_3_win64.dll'))
out=root/'JOIN182-performance-assets';out.mkdir(exist_ok=True)
for tag,base in ((0x80cb8d5b,0x4c0),(0x80cb8d65,0x1ed0)):
    raw=ps.tag(tag)
    (out/f'{tag:08x}.bin').write_bytes(raw)
    print(hex(tag),'bytes',len(raw),'sha256',hashlib.sha256(raw).hexdigest(),'definition',hex(base))
    for off in range(base,min(len(raw)-16,base+0x120),16):
        block=raw[off:off+16]
        print(f'{off:04x}',block.hex(),struct.unpack('>4f',block))
