from pathlib import Path
import sys,json,datetime,struct,hashlib
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from rpcs3_owned_memory import OwnedMemory
from rpcs3_final124_performance import u32,Resources
from d1_eboot import Eboot
root=Path(__file__).parent
eb=Eboot(root/'ps3_updates/01.24/USRDIR/EBOOT.elf')
with OwnedMemory(root.parent/'SIGNON_LOCAL/logs/FINAL124_JOIN200-processes.json') as mem:
    manager=u32(mem.read,0x1acd25c)
    registry=u32(mem.read,0x1ab69ac)
    header=mem.read(registry,0xa0)
    first,stride=struct.unpack_from('>II',header,0x18)
    if not 0x120<=stride<=4096 or not first:raise ValueError('unsupported Ghost registry layout')
    slots=[]
    for index in range(6):
        slot=mem.read(manager+index*0x338,0x338)
        # 0x42edb4 stores category 4 at actor+0x1c+0x2f0+4*4.
        unit,gear=struct.unpack_from('>I',slot,0x14)[0],struct.unpack_from('>I',slot,0x31c)[0]
        row={'slot':index,'unit_tag':hex(unit),'ghost_gear_handle':hex(gear)}
        if gear!=0xffffffff:
            record_index=gear&0x1fff
            if record_index>=256:raise ValueError('Ghost record exceeds bounded capture')
            address=first+record_index*stride
            raw=mem.read(address,stride)
            row.update(record_index=record_index,record_address=hex(address),record=raw.hex(),
                       record_unchanged=mem.read(address,stride)==raw)
        slots.append(row)
    code=[{'address':hex(pc),'matches_elf':mem.read(pc,count*4)==eb.read(pc,count*4)}
          for pc,count in ((0x71ec90,129),(0x2086d8,150),(0x42fa68,31))]
    result={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'atomic':False,
            'registry':hex(registry),'header':header.hex(),'record_stride':stride,'slots':slots,
            'code_checks':code,'registry_unchanged':mem.read(registry,0xa0)==header,
            'manager_unchanged':u32(mem.read,0x1acd25c)==manager}
path=root/('JOIN200-ghost-cache-'+datetime.datetime.now(datetime.timezone.utc).strftime('%H%M%S')+'.json')
with path.open('x',encoding='utf-8') as stream:json.dump(result,stream,indent=2)
print(json.dumps(result))
