"""Private bounded native props and interface dispatch sample, read-only."""
import json, re, struct, sys, datetime
from pathlib import Path
sys.path.insert(0, 'F:/d1/d1xenia/scripts')
from d1_guardian_route import open_shared_read
from rpcs3_owned_memory import OwnedMemory
from rpcs3_final124_performance import Resources, u32
from rpcs3_final124_attachment import unit_membership
root=Path(__file__).parent
name=sys.argv[1]
dest=root/f'JOIN210-{name}.json'
if dest.exists(): raise ValueError('preserve existing sample')
with open_shared_read(root.parent/'RPCS3_FINAL124_SMOKE001/log/RPCS3.log') as f:
    lines=f.read(150*1024*1024).decode('utf-8',errors='replace').splitlines()
devices={}; dispatch={}
for line in lines:
    if 'pc=0x2151d0 ' in line:
        m=re.search(r'r3=(0x[0-9a-f]+).*?m3=([0-9a-f]+)',line)
        if m: devices[int(m[1],16)]=m[2][:32]
    if 'pc=0x290af8 ' in line or 'pc=0x290b64 ' in line:
        m=re.search(r'pc=(0x[0-9a-f]+).*?m3=([0-9a-f]{64})',line)
        if m: dispatch[(m[1],m[2][:16])]=m[2]
result={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(), 'scope':'Non-atomic exact-owned read-only sample; dispatch target inferred using native member-pointer layout', 'devices':[], 'dispatch':[]}
with OwnedMemory(root.parent/'SIGNON_LOCAL/logs/FINAL124_JOIN210-processes.json') as mem:
    read=mem.read; res=Resources(read)
    for ptr, observed in devices.items():
        row={'address':hex(ptr),'observed_header':observed}
        try:
            raw=read(ptr,0x2e8)
            if raw[:16].hex()!=observed: raise ValueError('instance header changed')
            asset,off,kind=struct.unpack_from('>III',raw)
            definition=(res.resolve(asset)+off)&0xffffffff
            datum=struct.unpack_from('>I',raw,0x2e4)[0]
            row.update(asset=hex(asset),definition=hex(definition),definition_bytes=read(definition,0x70).hex(),state_a=raw[0x110:0x1f0].hex(),state_b=raw[0x1f0:0x2e0].hex(),unit_pair=raw[0x2e0:0x2e8].hex(),datum=hex(datum))
            if datum!=0xffffffff: row['membership']=unit_membership(read,datum)
            row['header_unchanged']=read(ptr,16)==raw[:16]
        except (ValueError,OSError) as e: row['error']=str(e)
        result['devices'].append(row)
    for (pc,_),observed in dispatch.items():
        cls,instance=struct.unpack_from('>II',bytes.fromhex(observed))
        row={'dispatch':pc,'class':hex(cls),'instance':hex(instance),'observed':observed}
        try:
            meta=(cls+u32(read,cls+0x10))&0xffffffff
            off=0x28 if pc=='0x290af8' else 0x38
            slot=u32(read,meta+off); adjusted=(instance+u32(read,meta+off+4))&0xffffffff
            descriptor=u32(read,(u32(read,adjusted)+slot-1)&0xffffffff) if slot&1 else slot
            header=read(instance,32)
            row.update(meta=hex(meta),slot=hex(slot),adjusted=hex(adjusted),descriptor=hex(descriptor),target=hex(u32(read,descriptor)),instance_header=header.hex(),meta_bytes=read(meta,0x50).hex(),class_bytes=read(cls,0x20).hex())
            row['header_unchanged']=read(instance,32)==header
            if struct.unpack_from('>I',header,8)[0]==0x808025e3:
                raw=read(instance,0x3f8)
                row.update(command_counters=struct.unpack_from('>6I',raw,0x3e0),values_78_9f=raw[0x78:0xa0].hex(),config_3c_67=raw[0x3c:0x68].hex())
        except (ValueError,OSError) as e: row['error']=str(e)
        result['dispatch'].append(row)
    result['registry_unchanged']=res.unchanged()
dest.write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'file':str(dest),'devices':len(result['devices']),'dispatch':len(result['dispatch']),'registry_unchanged':result['registry_unchanged']}))
