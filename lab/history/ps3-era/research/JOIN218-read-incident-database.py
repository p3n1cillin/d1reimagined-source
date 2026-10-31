import json
import struct
import sys
from pathlib import Path
sys.path.insert(0,r"F:\d1\d1xenia\scripts")
from rpcs3_owned_memory import OwnedMemory
from d1_eboot import Eboot
root=Path(__file__).parent
e=Eboot(root/'ps3_updates/01.24/USRDIR/EBOOT.elf')
with OwnedMemory(root.parent/'SIGNON_LOCAL/logs/FINAL124_JOIN218-processes.json') as m:
    def u32(a):return struct.unpack('>I',m.read(a,4))[0]
    obj=u32(0x1a93300)
    raw=m.read(obj,0x40)
    vtable=struct.unpack_from('>I',raw)[0]
    desc=u32(vtable+0x18)
    function=u32(desc)
    lo,hi=e.function_of(function)
    rows={"singleton":hex(obj),"words":[hex(v) for v in struct.unpack('>16I',raw)],
          "vtable":hex(vtable),"database_getter_descriptor":hex(desc),"function":hex(function),
          "disassembly":list(e.disasm(lo,min((hi-lo)//4,80)))}
    if m.read(obj,0x40)!=raw or u32(0x1a93300)!=obj:raise ValueError('database changed')
with (root/'JOIN218-incident-database-verified.json').open('x',encoding='utf-8') as f:
    json.dump(rows,f,indent=2);f.write('\n')
print(json.dumps(rows))
