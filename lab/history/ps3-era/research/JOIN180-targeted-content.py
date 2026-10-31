"""Read only the activity's verified local patch chain; keep extracted payloads private."""
from pathlib import Path
import json, sys, struct, hashlib
sys.path.insert(0, 'F:/d1/d1xenia/scripts')
from d1_pkg import PackageSet, Oodle, dyn_array
root=Path(__file__).parent
runtime=root.parent
paths=list((runtime/'RESEARCH/ps3_updates/final/packages').glob('*025c*.pkg'))
paths+=list((runtime/'01_WORKING/ps3_destiny/PS3_GAME/USRDIR/packages').glob('*025c*.pkg'))
ps=PackageSet(paths, Oodle(runtime/'TOOLS/oodle/oo2core_3_win64_x86-64/oo2core_3_win64.dll'))
out=root/'JOIN180-content-bindings'
out.mkdir(exist_ok=True)
for tag in [0x80cb911e]:
    raw=ps.tag(tag)
    (out/f'{tag:08x}.bin').write_bytes(raw)
    print(f'{tag:08x} bytes {len(raw)} SHA256 {hashlib.sha256(raw).hexdigest()}')
    for off in list(range(0, min(len(raw), 0x80), 0x10))+list(range(0x420, min(len(raw),0x590),0x10)):
        block=raw[off:off+16]
        print(f'{off:04x} {block.hex()} '+ ' '.join(f'{x:08x}' for x in struct.unpack('>4I',block)))
    for off in [4,0xc,0x10,0x1c,0x28,0x38,0x48]:
        try: print('array',hex(off),dyn_array(raw,off))
        except Exception as exc: print('array',hex(off),type(exc).__name__)
