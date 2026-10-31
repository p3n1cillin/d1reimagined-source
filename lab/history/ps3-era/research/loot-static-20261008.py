"""Private 01.24 loot call-chain analysis; source data stays outside Git."""
import json,sys
from pathlib import Path
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from d1_eboot import Eboot
root=Path(__file__).parent
out=root/'loot-static-20261008'
out.mkdir(exist_ok=False)
e=Eboot(root/'ps3_updates/01.24/USRDIR/EBOOT.elf')
funcs=(0x2b279c,0x2b2a60,0x2b3240,0x2b36f8,0x2b4070,0x5e5600,0xd0ecac,0xc43518,0xcbb6c4)
rows=[]
for target in funcs:
    lo,hi=e.function_of(target)
    (out/f'{target:08x}.asm').write_text('\n'.join(e.disasm(target,(hi-target)//4))+'\n')
    rows.append({'function':hex(target),'end':hex(hi),'callers':[{'pc':hex(p),'function':hex(e.function_of(p)[0])} for p in e.callers(target)],'data_refs':[hex(p) for p in e.data_refs(target)]})
(out/'calls.json').write_text(json.dumps(rows,indent=2)+'\n')
print(json.dumps(rows,indent=1))
