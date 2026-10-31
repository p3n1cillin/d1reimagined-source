import json,struct,sys
from pathlib import Path
sys.path.insert(0,r"F:\d1\d1xenia\scripts")
from rpcs3_owned_memory import OwnedMemory
root=Path(__file__).parent
inputs={}
with OwnedMemory(root.parent/'SIGNON_LOCAL/logs/FINAL124_JOIN218-processes.json') as m:
    def read(a,n):
        raw=m.read(a,n);inputs[(a,n)]=raw;return raw
    def u32(a):return struct.unpack('>I',read(a,4))[0]
    binding=u32(0x1ad5c94)
    handle=u32(binding+0x10)
    shifted=((handle-(1<<32) if handle&0x80000000 else handle)>>13)&0xffffffff
    index=shifted&((shifted>>18)|0x3ff)
    table=u32(u32(0x1896d50))
    entry=table+index*64
    cell=u32(entry+0x10)+(handle&0x1fff)*u32(entry+0x30)
    definition=(cell-(u32(cell+8)&u32(entry+0x34)))&0xffffffff
    raw=read(definition,0x80)
    words=struct.unpack('>32I',raw)
    result={"binding":hex(binding),"handle":hex(handle),"entry":hex(entry),
            "cell":hex(cell),"definition":hex(definition),"words":[hex(v) for v in words]}
    for field in (0x1c,0x20,0x24,0x28):
        count=u32(definition+field)
        rel=u32(definition+field+4)
        if 0<count<5000 and rel<0x100000:
            first=definition+field+4+rel+12
            result[f"array_{field:x}"]={"count":count,"first":hex(first),"sample":read(first,min(count*8,96)).hex()}
    if any(m.read(a,n)!=d for (a,n),d in inputs.items()):raise ValueError('tag metadata changed')
with (root/'JOIN218-incident-tag.json').open('x',encoding='utf-8') as f:
    json.dump(result,f,indent=2);f.write('\n')
print(json.dumps(result))
