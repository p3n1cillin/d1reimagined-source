from pathlib import Path
import sys, struct, json
sys.path.insert(0, 'F:/d1/d1xenia/scripts')
from d1_pkg import PackageSet, Oodle, entity_group, dyn_array, tag_hash
from d1_eboot import Eboot
rt = Path(__file__).parent.parent
out = Path(__file__).parent / 'JOIN208-static'
out.mkdir(exist_ok=False)
ps = PackageSet(list((rt/'RESEARCH/ps3_updates/final/packages').glob('*.pkg')) + list((rt/'01_WORKING/ps3_destiny/PS3_GAME/USRDIR/packages').glob('*.pkg')), Oodle(rt/'TOOLS/oodle/oo2core_3_win64_x86-64/oo2core_3_win64.dll'))
rows = []
for pid, pk in ps.by_id.items():
    for e in pk.entries:
        if e['reference'] != 0x8080076e: continue
        raw = pk.entry_data(e['index'], ps.oodle)
        key, objects = entity_group(raw)
        if key not in (0xfc0e45a3, 0x992bb471): continue
        tag = tag_hash(pid, e['index'])
        (out/f'{tag:08x}-group.bin').write_bytes(raw)
        arrays = []
        for field in range(0x10, min(0x80, len(raw)), 4):
            try:
                n, off, cls = dyn_array(raw, field)
                arrays.append({'field':hex(field),'count':n,'offset':hex(off),'class':hex(cls),'first':raw[off:off+64].hex()})
            except (ValueError, struct.error): pass
        row = {'key':hex(key),'tag':hex(tag),'bytes':len(raw),'header':raw[:128].hex(),'arrays':arrays,'objects':[{'index':i,'kind':k,'name':name} for i,(k,name) in enumerate(objects)]}
        rows.append(row)
        print(json.dumps({k:v for k,v in row.items() if k!='objects'}))
(out/'groups.json').write_text(json.dumps(rows,indent=2))
eb = Eboot(rt/'RESEARCH/ps3_updates/01.24/USRDIR/EBOOT.elf')
data = eb.read(0x11e2520, 128)
print('type descriptor', data.hex())
for pc in (0x214478, 0x215bfc, 0x2000e0):
    (out/f'{pc:08x}.txt').write_text('\n'.join(eb.disasm(pc,90))+'\n')
