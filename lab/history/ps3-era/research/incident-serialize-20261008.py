import json,sys
from pathlib import Path
sys.path.insert(0,r"F:\d1\d1xenia\scripts")
from d1_eboot import Eboot
root=Path(__file__).parent
out=root/'incident-serialize-20261008'
out.mkdir(exist_ok=False)
e=Eboot(root/'ps3_updates/01.24/USRDIR/EBOOT.elf')
rows=[]
for target in (0x1cae2c,0x12f2e8,0x5ad874,0x13db80,0xc43fc0):
    lo,hi=e.function_of(target)
    (out/f'{target:08x}.asm').write_text('\n'.join(e.disasm(lo,min((hi-lo)//4,1800)))+'\n')
    rows.append({"target":hex(target),"end":hex(hi)})
print(json.dumps(rows))
