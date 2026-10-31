from pathlib import Path
import sys
sys.path.insert(0, 'F:/d1/d1xenia/scripts')
from d1_eboot import Eboot
root = Path(__file__).parent
eb = Eboot(root / 'ps3_updates/01.24/USRDIR/EBOOT.elf')
out = root / 'ghost-profile-static-20261008'
out.mkdir(exist_ok=True)
for pc, count in ((0x71ec90,160), (0x71de7c,100), (0x71e86c,100),
                  (0x46ed4c,220), (0x2a4c84,70), (0x42edb4,90), (0x2d184c,150)):
    lines = []
    for address in range(pc, pc + count*4, 4):
        lines.extend(list(eb.disasm(address,1)) or [f'{address:08x}: .long {eb.u32(address):08x}'])
    (out / f'{pc:x}.txt').write_text('\n'.join(lines)+'\n', encoding='utf-8')
print(out)
