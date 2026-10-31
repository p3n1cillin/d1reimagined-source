from pathlib import Path
import sys,struct,json
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from d1_pkg import PackageSet,Oodle,split_tag_hash,dyn_array
rt=Path(__file__).parent.parent
ps=PackageSet(list((rt/'RESEARCH/ps3_updates/final/packages').glob('*.pkg'))+list((rt/'01_WORKING/ps3_destiny/PS3_GAME/USRDIR/packages').glob('*.pkg')),Oodle(rt/'TOOLS/oodle/oo2core_3_win64_x86-64/oo2core_3_win64.dll'))
out=Path(__file__).parent/'JOIN208-static'
for tag in (*range(0x80cb820c,0x80cb8216),*range(0x80cb8172,0x80cb817b)):
    pid,idx=split_tag_hash(tag);pk=ps.by_id[pid]
    raw=pk.entry_data(idx,ps.oodle)
    (out/f'{tag:08x}.bin').write_bytes(raw)
    print(hex(tag),'class',hex(pk.entries[idx]['reference']),'len',len(raw),'header',raw[:0x80].hex())
    for field in range(4,min(len(raw),0x90),4):
        try:
            n,off,cls=dyn_array(raw,field)
            print('array',hex(field),n,hex(off),hex(cls),raw[off:off+128].hex())
            if cls==0x80800406:
                for i in range(n):
                    rec=off+i*128
                    print('placed',i,'unit',hex(struct.unpack_from('>I',raw,rec)[0]),'xyz',struct.unpack_from('>fff',raw,rec+0x20),'last',raw[rec:rec+128].hex())
            if field==0x44:
                print('placements',json.dumps([{'i':i,'data':raw[off+i*128:off+(i+1)*128].hex()} for i in range(n)]))
        except (ValueError,struct.error):pass
group=(out/'80cb8167-group.bin').read_bytes()
print('group instances',group[0x800:0x910].hex())
