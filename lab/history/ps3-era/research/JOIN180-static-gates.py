import json, sys, struct
from pathlib import Path
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from d1_eboot import Eboot
root=Path(__file__).parent
eb=Eboot(str(root/'ps3_updates/01.24/USRDIR/EBOOT.elf'))
for pc in [0x57a6e8,0x57a70c,0x580b54,0x581000,0x425654]:
    start,end=eb.function_of(pc)
    (root/f'JOIN180-static-{pc:x}.txt').write_text('\n'.join(eb.disasm(start,(end-start)//4))+'\n')
    print(hex(pc),'callers',[hex(x) for x in eb.callers(pc)])
