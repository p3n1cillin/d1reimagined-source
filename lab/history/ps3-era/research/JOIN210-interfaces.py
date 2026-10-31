"""Private latest-package component interface/configuration inspection."""
import json,struct,sys
from pathlib import Path
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from d1_pkg import PackageSet,Oodle,split_tag_hash,dyn_array
rt=Path(__file__).parent.parent
out=Path(__file__).parent/'JOIN210-static'
out.mkdir(exist_ok=False)
ps=PackageSet(list((rt/'RESEARCH/ps3_updates/final/packages').glob('*.pkg'))+list((rt/'01_WORKING/ps3_destiny/PS3_GAME/USRDIR/packages').glob('*.pkg')),Oodle(rt/'TOOLS/oodle/oo2core_3_win64_x86-64/oo2core_3_win64.dll'))
def get(tag):
    pid,idx=split_tag_hash(tag);return ps.by_id[pid].entry_data(idx,ps.oodle)
rows=[]
for unit in (0x80cb828c,0x80cb82a0,0x80cc0528,0x80ccb1dc):
    raw=get(unit);n,offset,cls=dyn_array(raw,0x10)
    components=[]
    for i in range(n):
        tag=struct.unpack_from('>I',raw,offset+i*12)[0]
        cfg=get(tag);(out/f'{tag:08x}.bin').write_bytes(cfg)
        root=12+struct.unpack_from('>i',cfg,12)[0]
        refs=[]
        for off in range(0,len(cfg)-3,4):
            value=struct.unpack_from('>I',cfg,off)[0]
            if 0x80800000<=value<0x80806000:refs.append([hex(off),hex(value)])
        components.append({'tag':hex(tag),'root':hex(root),'root_header':cfg[root:root+0x100].hex(),'type_refs':refs})
    rows.append({'unit':hex(unit),'components':components})
(out/'interfaces.json').write_text(json.dumps(rows,indent=2)+'\n')
for row in rows:
    print(json.dumps({'unit':row['unit'],'components':[{'tag':c['tag'],'root':c['root'],'type_refs':c['type_refs']} for c in row['components']]}))
