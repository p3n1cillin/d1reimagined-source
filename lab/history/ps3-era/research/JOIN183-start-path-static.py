import sys
from pathlib import Path
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from d1_eboot import Eboot
root=Path(__file__).parent;eb=Eboot(root/'ps3_updates/01.24/USRDIR/EBOOT.elf')
out=root/'JOIN183-cinematic-static'
for pc,count in ((0x42c060,144),(0x42afec,190),(0x42b51c,380),(0x42b2f0,190),(0x430650,120)):
    lines=[]
    for address in range(pc,pc+4*count,4):
        lines.extend(list(eb.disasm(address,1)) or [f'{address:08x}: .long {eb.u32(address):08x}'])
    (out/f'{pc:x}.txt').write_text('\n'.join(lines)+'\n',encoding='utf-8')
    print(hex(pc),'saved',count)
