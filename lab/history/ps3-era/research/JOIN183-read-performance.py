import datetime,json,struct,sys
from pathlib import Path
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from rpcs3_owned_memory import OwnedMemory
root=Path(__file__).parent
out={'at':datetime.datetime.now(datetime.timezone.utc).isoformat(),
     'scope':'Read-only exact owned client; non-atomic native performance dispatch sample'}
with OwnedMemory(root.parent/'SIGNON_LOCAL/logs/FINAL124_JOIN183-processes.json') as m:
    u32=lambda a:struct.unpack('>I',m.read(a,4))[0]
    def resolve(h):
        signed=h-(1<<32) if h&0x80000000 else h
        shifted=(signed>>13)&0xFFFFFFFF
        index=shifted & ((shifted>>18)|0x3FF)
        table=u32(u32(0x1896d50))
        entry=table+index*64
        stride=u32(entry+0x30)
        if not 0<stride<=0x100000:raise ValueError('Invalid resource stride')
        p=(u32(entry+0x10)+(h&0x1FFF)*stride)&0xFFFFFFFF
        return (p-(u32(p+8)&u32(entry+0x34)))&0xFFFFFFFF
    manager=u32(0x1acd25c)
    count=u32(manager+0x1350)
    out.update(manager=hex(manager),count=count,records=[])
    if count>32:raise ValueError('Unexpected native performance count')
    for index in range(count):
        raw=m.read(manager+0x1354+index*32,32)
        fields=struct.unpack('>8I',raw)
        row={'index':index,'record':raw.hex()}
        try:
            # Exact 01.24 0x42478c resource resolution and generic 0x291870
            # dispatch, rather than treating the salted data handle as an offset.
            definition=resolve(fields[4])
            data=(resolve(fields[5])+fields[6])&0xFFFFFFFF
            meta=(definition+u32(definition+0x10))&0xFFFFFFFF
            slot=u32(meta+0x38)
            instance=(data+u32(meta+0x3c))&0xFFFFFFFF
            descriptor=u32((u32(instance)+slot-1)&0xFFFFFFFF) if slot&1 else slot
            target=u32(descriptor)
            row.update(definition=hex(definition),data=hex(data),meta=hex(meta),
                       slot=hex(slot),instance=hex(instance),descriptor=hex(descriptor),
                       target=hex(target),definition_bytes=m.read(definition,128).hex(),
                       data_bytes=m.read(data,256).hex(),meta_bytes=m.read(meta,96).hex())
        except (ValueError,OSError) as exc:row['error']=str(exc)
        out['records'].append(row)
    out['machine_cast']=m.read(manager+0x17d4,104).hex()
    out['manager_unchanged']=u32(0x1acd25c)==manager and u32(manager+0x1350)==count
with (root/'JOIN183-performance-samples.jsonl').open('a',encoding='utf-8') as stream:
    stream.write(json.dumps(out)+'\n')
(root/'JOIN183-performance-last.json').write_text(json.dumps(out,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'at':out['at'],'count':out['count'],'manager_unchanged':out['manager_unchanged'],
                  'records':[{k:v for k,v in row.items() if not k.endswith('_bytes')}
                             for row in out['records']]}))
