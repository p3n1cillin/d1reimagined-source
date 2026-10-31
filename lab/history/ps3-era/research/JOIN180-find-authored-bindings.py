"""Targeted local activity-package lookup, private output; no input mutation."""
from pathlib import Path
import json,sys,struct,hashlib
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from d1_pkg import PackageSet,Oodle,tag_hash
root=Path(__file__).parent;runtime=root.parent
paths=list((runtime/'RESEARCH/ps3_updates/final/packages').glob('*025c*.pkg'))
paths+=list((runtime/'01_WORKING/ps3_destiny/PS3_GAME/USRDIR/packages').glob('*025c*.pkg'))
ps=PackageSet(paths,Oodle(runtime/'TOOLS/oodle/oo2core_3_win64_x86-64/oo2core_3_win64.dll'))
package=ps.by_id[0x25c]
out=root/'JOIN180-content-bindings';out.mkdir(exist_ok=True)
patterns={f'{kind:x}:{index}':struct.pack('>IBBH',0x992bb471,kind,0,index)
          for kind,index in [(6,1),(6,2),(0x2c,65),(0x2c,87)]}
found=[]
for entry in package.entries:
    if entry['reference']!=0x80800861:continue
    raw=package.entry_data(entry['index'],ps.oodle)
    matches={key:[i for i in range(len(raw)) if raw.startswith(pattern,i)] for key,pattern in patterns.items()}
    matches={key:offsets for key,offsets in matches.items() if offsets}
    if not matches:continue
    tag=tag_hash(package.pkg_id,entry['index'])
    (out/f'{tag:08x}.bin').write_bytes(raw)
    row={'tag':hex(tag),'size':len(raw),'matches':matches,'sha256':hashlib.sha256(raw).hexdigest()}
    found.append(row);print(json.dumps(row),flush=True)
(out/'binding-matches.json').write_text(json.dumps(found,indent=2)+'\n')
