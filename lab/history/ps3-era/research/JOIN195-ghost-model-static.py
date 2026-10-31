from pathlib import Path
import struct,sys
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from d1_eboot import Eboot
root=Path(__file__).parent
eb=Eboot(root/'ps3_updates/01.24/USRDIR/EBOOT.elf')
for pc,count in ((0x2a65a8,75),(0x4c7158,60),(0x2d184c,180),(0x4c6480,190)):
    lines=[]
    for address in range(pc,pc+count*4,4):
        lines.extend(list(eb.disasm(address,1)) or [f'{address:08x}: .long {eb.u32(address):08x}'])
    (root/f'JOIN195-static-{pc:x}.txt').write_text('\n'.join(lines)+'\n',encoding='utf-8')
data=root/'ghost-profile-tables-20261008'
raw=(data/'asset-root.bin').read_bytes()
print('root',[(hex(i),hex(struct.unpack_from('>I',raw,i)[0])) for i in range(0x130,len(raw)-3,4)])
print('item862',(data/'item-862.bin').read_bytes().hex())
