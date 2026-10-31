from pathlib import Path
import sys,struct,json,re
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from d1_pkg import PackageSet,Oodle,split_tag_hash,dyn_array,tag_hash
rt=Path(__file__).parent.parent
ps=PackageSet(list((rt/'RESEARCH/ps3_updates/final/packages').glob('*.pkg'))+list((rt/'01_WORKING/ps3_destiny/PS3_GAME/USRDIR/packages').glob('*.pkg')),Oodle(rt/'TOOLS/oodle/oo2core_3_win64_x86-64/oo2core_3_win64.dll'))
out=Path(__file__).parent/'JOIN208-static'
pk=ps.by_id[split_tag_hash(0x80cb82a7)[0]]
rows=[]
for entry in pk.entries:
    if entry['reference']!=0x808003f6:continue
    wrapper=pk.entry_data(entry['index'],ps.oodle)
    if len(wrapper)<24 or struct.unpack_from('>I',wrapper,0x14)[0]!=0x8080092e:continue
    child=struct.unpack_from('>I',wrapper,8)[0]
    pid,idx=split_tag_hash(child)
    raw=ps.by_id[pid].entry_data(idx,ps.oodle)
    matches=[(name,raw.find(bytes.fromhex(ident))) for name,ident in [('weapon','992bb47104000006'),('chest','fc0e45a304000007'),('lights','fc0e45a304000004')]]
    matches=[(name,off) for name,off in matches if off>=0]
    if not matches:continue
    placements=[]
    for field in range(4,len(raw)-8,4):
        try:n,off,cls=dyn_array(raw,field)
        except (ValueError,struct.error):continue
        if cls!=0x80800406:continue
        for i in range(n):
            rec=off+i*128
            unit=struct.unpack_from('>I',raw,rec)[0]
            placements.append({'unit':hex(unit),'xyz':struct.unpack_from('>fff',raw,rec+0x20),'quaternion':struct.unpack_from('>ffff',raw,rec+0x10),'field':hex(field),'offset':hex(rec)})
    row={'wrapper':hex(tag_hash(pk.pkg_id,entry['index'])),'definition':hex(child),'object_matches':matches,'placements':placements,'names':[x.decode('latin1') for x in re.findall(rb'[\x20-\x7e]{8,}\x00',raw)]}
    rows.append(row);print(json.dumps(row))
    (out/f'{child:08x}.bin').write_bytes(raw)
(out/'weapon-defs.json').write_text(json.dumps(rows,indent=2))
