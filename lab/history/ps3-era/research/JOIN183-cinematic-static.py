import sys,struct
from pathlib import Path
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from d1_eboot import Eboot
root=Path(__file__).parent
out=root/'JOIN183-cinematic-static';out.mkdir(exist_ok=True)
eb=Eboot(root/'ps3_updates/01.24/USRDIR/EBOOT.elf')
for pc,count in ((0x580928,200),(0x425654,80),(0x425120,150),(0x42478c,120),(0x291870,120)):
    # Decode each instruction separately so unsupported VMX instructions cannot
    # silently truncate the rest of the selected static range.
    lines=[]
    for address in range(pc,pc+count*4,4):
        decoded=list(eb.disasm(address,1))
        lines.extend(decoded or [f'{address:08x}: .long {eb.u32(address):08x}'])
    (out/f'{pc:x}.txt').write_text('\n'.join(lines)+'\n',encoding='utf-8')
    print(hex(pc),'instructions',len(lines),'saved')
for tag,base in ((0x80cb8d5b,0x4c0),(0x80cb8d65,0x1ed0)):
    raw=(root/f'JOIN182-performance-assets/{tag:08x}.bin').read_bytes()
    lines=[]
    for off in range(base,min(len(raw),base+0x210),16):
        lines.append(f'{off:04x}: {raw[off:off+16].hex()}')
    (out/f'{tag:08x}-definition.txt').write_text('\n'.join(lines)+'\n',encoding='utf-8')
    print(hex(tag),'definition bytes saved')
