import datetime,json,struct,sys,time
from pathlib import Path
sys.path.insert(0,r'F:\d1\d1xenia\scripts')
from rpcs3_owned_memory import OwnedMemory
root=Path(r'F:\d1\<pc>-import\d1xenia\local')
manifest=root/'SIGNON_LOCAL/logs/FINAL124_JOIN142-processes.json'
with OwnedMemory(manifest) as memory,(root/'SIGNON_LOCAL/logs/JOIN142-host-world-timeline.jsonl').open('a') as stream:
    def u32(a): return struct.unpack('>I',memory.read(a,4))[0]
    for sample in range(320):
        if Path(str(manifest)+'.stop').exists(): break
        result={'at':datetime.datetime.now(datetime.timezone.utc).isoformat()}
        try:
            manager=u32(0x1abc8a4); indices=u32(0x1abc79c)
            index=u32(indices)
            if not manager or not 0<=index<64: raise ValueError('manager/session slot not initialized')
            entry=manager+index*0x170
            handle=u32(entry+0x550)
            if handle==0xffffffff: raise ValueError('no host session handle')
            base,stride=struct.unpack('>II',memory.read(0x1a20b78,8))
            if not base or not 0x5038+0xe08<=stride<=0x10000: raise ValueError('host pool layout is unverified')
            host=base+(handle&0x1fff)*stride
            world=host+0xe08
            req=memory.read(world+0x5010,16)
            result.update(host=hex(host),handle=hex(handle),world_version=u32(world),
                group_current=struct.unpack('>i',memory.read(world+0x2e48,4))[0],
                group_switch=struct.unpack('>i',memory.read(world+0x2e48+0xd8,4))[0],
                request_state=req[0],request_index=struct.unpack_from('>i',req,8)[0],
                request_hash=hex(struct.unpack_from('>I',req,12)[0]))
        except (OSError,ValueError) as exc: result['unavailable']=str(exc)
        stream.write(json.dumps(result)+'\n'); stream.flush();time.sleep(.75)
