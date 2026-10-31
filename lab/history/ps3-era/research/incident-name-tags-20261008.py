import json,re,sys
from pathlib import Path
sys.path.insert(0,r"F:\d1\d1xenia\scripts")
from d1_pkg import PackageSet,Oodle,split_tag_hash
root=Path(__file__).parent;rt=root.parent
out=root/'incident-name-tags-20261008';out.mkdir(exist_ok=False)
ps=PackageSet(list((root/'ps3_updates/final/packages').glob('*.pkg'))+list((rt/'01_WORKING/ps3_destiny/PS3_GAME/USRDIR/packages').glob('*.pkg')),Oodle(rt/'TOOLS/oodle/oo2core_3_win64_x86-64/oo2core_3_win64.dll'))
rows=[]
for tag in (0x80aa3eb1,0x80aa3eb2):
    pid,index=split_tag_hash(tag);p=ps.by_id[pid];raw=p.entry_data(index,ps.oodle)
    (out/f'{tag:08x}.bin').write_bytes(raw)
    strings=[s[:-1].decode('ascii') for s in re.findall(rb'[\x20-\x7e]{5,}\x00',raw)]
    matches=[s for s in strings if any(k in s.lower() for k in ('loot','weapon','trigger','performance','actor'))]
    rows.append({"tag":hex(tag),"class":hex(p.entries[index]['reference']),"bytes":len(raw),"header":raw[:64].hex(),"matched_strings":matches[:60],"strings":strings[:8]})
(out/'names.json').write_text(json.dumps(rows,indent=2)+'\n')
print(json.dumps(rows))
