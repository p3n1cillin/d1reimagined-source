import datetime,sys,json,struct
sys.path.insert(0,r'F:\d1\d1xenia\scripts')
from rpcs3_owned_memory import OwnedMemory
from rpcs3_final124_session import u32,MANAGER_PTR
manifest=r'F:\d1\<pc>-import\d1xenia\local\SIGNON_LOCAL\logs\FINAL124_JOIN171-processes.json'
with OwnedMemory(manifest) as m:
    h=u32(m.read,u32(m.read,MANAGER_PTR)+0x550)
    d=m.read(0x1a20b70,16)
    storage,stride=struct.unpack_from('>II',d,8)
    address=(storage+(h&0x1fff)*stride)&0xffffffff
    writer=u32(m.read,address+0xdf0)
    slot=u32(m.read,writer+0x58)
    rows=u32(m.read,0x1ad5dec)
    row=rows+slot*24
    data=m.read(row,24)
    obj=u32(m.read,row+0x10)
    result={'at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'host':hex(address),'writer':hex(writer),'writer_vtable':hex(u32(m.read,writer)),'writer_fields_50':m.read(writer+0x50,0x20).hex(),'slot':slot,'queue_rows':hex(rows),'queue_row':hex(row),'queue_row_bytes':data.hex(),'object_handle':hex(obj)}
    print(json.dumps(result))