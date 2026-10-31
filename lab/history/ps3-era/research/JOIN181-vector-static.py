import sys
from pathlib import Path
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from d1_eboot import Eboot
root=Path(__file__).parent
eb=Eboot(root/'ps3_updates/01.24/USRDIR/EBOOT.elf')
for table in (0x1898778,0x18988c8):
    for kind in (11,12,13,14):
        descriptor=eb.u32(table+kind*4)
        pc=eb.u32(descriptor)
        print(hex(table),kind,hex(descriptor),hex(pc))
        lines=list(eb.disasm(pc,80))
        (root/f'JOIN181-reader-{kind}-{pc:x}.txt').write_text('\n'.join(lines)+'\n')
        print('\n'.join(lines[:38]))
