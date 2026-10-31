from pathlib import Path
import sys
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from d1_eboot import Eboot
root=Path(__file__).parent
eb=Eboot(root/'ps3_updates/01.24/USRDIR/EBOOT.elf')
for pc,count in ((0x42f144,150),(0x42f3a0,500),(0x2d4ed0,100),(0x4c6768,100),(0x2d1570,115)):
    lines=[]
    for address in range(pc,pc+count*4,4):
        lines.extend(list(eb.disasm(address,1)) or [f'{address:08x}: .long {eb.u32(address):08x}'])
    (root/f'JOIN195-static-{pc:x}.txt').write_text('\n'.join(lines)+'\n',encoding='utf-8')
