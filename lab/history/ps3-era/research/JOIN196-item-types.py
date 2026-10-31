from pathlib import Path
import sys,struct,json,datetime
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from rpcs3_owned_memory import OwnedMemory
from rpcs3_final124_performance import Resources,u32
root=Path(__file__).parent
out=[]
with OwnedMemory(root.parent/'SIGNON_LOCAL/logs/FINAL124_JOIN196-processes.json') as mem:
    resources=Resources(mem.read)
    for handle in (0x808033fa,0x8080340e,0x808033cc):
        p=resources.resolve(handle)
        head=mem.read(p,0x58)
        size=struct.unpack_from('>I',head,0x14)[0]
        relative=struct.unpack_from('>I',head,0x54)[0]
        fields_address=p+0x54+relative
        count=u32(mem.read,fields_address)
        if size>32768 or count>128:raise ValueError(f'unsupported type {handle:#x}: {size}/{count}')
        raw=mem.read(fields_address+0x14,count*0x28)
        fields=[]
        for i in range(count):
            row=raw[i*0x28:(i+1)*0x28]
            fields.append({'off':struct.unpack_from('>I',row)[0],'kind':row[0x10],
                           'nested':hex(struct.unpack_from('>I',row,0x14)[0]),
                           'min':struct.unpack_from('>I',row,0x18)[0],
                           'width':struct.unpack_from('>I',row,0x1c)[0],
                           'raw':row.hex()})
        out.append({'handle':hex(handle),'address':hex(p),'size':size,'fields':fields,
                    'header_unchanged':mem.read(p,0x58)==head})
    result={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'types':out,'resources_unchanged':resources.unchanged()}
with (root/'JOIN196-item-types.json').open('x',encoding='utf-8') as stream:json.dump(result,stream,indent=2)
print(json.dumps(result))
