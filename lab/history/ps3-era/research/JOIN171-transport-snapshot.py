import sys,json,struct
sys.path.insert(0,r'F:\d1\d1xenia\scripts')
from rpcs3_owned_memory import OwnedMemory
from rpcs3_final124_session import u32,MANAGER_PTR
manifest=r'F:\d1\<pc>-import\d1xenia\local\SIGNON_LOCAL\logs\FINAL124_JOIN171-processes.json'
with OwnedMemory(manifest) as m:
    h=u32(m.read,u32(m.read,MANAGER_PTR)+0x550)
    d=m.read(0x1a20b70,16)
    storage,stride=struct.unpack_from('>II',d,8)
    address=(storage+(h&0x1fff)*stride)&0xffffffff
    baseline=m.read(address+0x66e0,0x220)
    pending=m.read(address+0x6900,0x220)
    result={'handle':h,'descriptor':d.hex(),'address':hex(address),'stride':stride,'vtable':hex(u32(m.read,address)),'baseline_slice':baseline[0xb8:0x220].hex(),'pending_slice':pending[0xb8:0x220].hex(),'state_dirty':baseline!=pending,'header_6b50':m.read(address+0x6b50,0x58).hex()}
    print(json.dumps(result))