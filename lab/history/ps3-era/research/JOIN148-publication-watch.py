import datetime,json,struct,sys,time
from pathlib import Path
sys.path.insert(0,r'F:\d1\d1xenia\scripts')
from rpcs3_owned_memory import OwnedMemory
from rpcs3_final124_transition import snapshot
root=Path(r'F:\d1\<pc>-import\d1xenia\local')
manifest=root/'SIGNON_LOCAL/logs/FINAL124_JOIN148-processes.json'
until=time.monotonic()+900
with OwnedMemory(manifest) as memory,(root/'SIGNON_LOCAL/logs/JOIN148-publication-timeline.jsonl').open('a',encoding='utf-8') as stream:
    def u32(a): return struct.unpack('>I',memory.read(a,4))[0]
    def zone(a):
        raw=memory.read(a,0x148)
        return {'current':struct.unpack_from('>iIi',raw),'target':struct.unpack_from('>iIi',raw,0xa0),'peer':raw[0x140],'state':raw[0x142]}
    previous=None
    while time.monotonic()<until and not Path(str(manifest)+'.stop').exists():
        result={'at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'atomic':False}
        try:
            engine=u32(0x1abc8ac)
            if not engine: raise ValueError('engine not initialized')
            state=snapshot(memory.read)
            state.pop('captured_utc',None)
            current=memory.read(engine+0x88,0x168)
            cached=memory.read(engine+0x1f0,0x168)
            result.update(engine=state,publication_differs=current!=cached,
                publication_cache=zone(engine+0x1f8),entry_guards=memory.read(engine+0x80,2).hex())
            guard=u32(0x1b28860)
            flags=memory.read(guard,2) if guard else bytes(2)
            gate=u32(0x1b28864) if any(flags) else 0
            result.update(publication_guard_flags=flags.hex(),publication_guard_gate=u32(gate) if gate else 0)
            pool=u32(0x1abca14)
            if pool and memory.read(pool,1)!=b'\0':
                contexts=[]
                for slot in range(3):
                    record=pool+slot*0x12208
                    selected=u32(record+8)
                    if selected not in (0,1): raise ValueError('context bank selector is unverified')
                    for bank in (0,1):
                        context=record+bank*0x9100+0x10
                        context_state=u32(context+0x8468)
                        active=struct.unpack('>iIi',memory.read(context+0x400,12))
                        contexts.append({'slot':slot,'bank':bank,'selected':selected,'address':hex(context),
                            'state_8468':context_state,'record_400':active,
                            'getter_8fed64_admits':context_state!=0 and active[2]!=-1})
                result['activity_contexts']=contexts
            manager=u32(0x1abc8a4);indices=u32(0x1abc79c)
            index=u32(indices) if indices else 0xffffffff
            if not manager or not 0<=index<64: raise ValueError('manager/session slot not initialized')
            handle=u32(manager+index*0x170+0x550)
            if handle==0xffffffff: raise ValueError('no host session handle')
            base,stride=struct.unpack('>II',memory.read(0x1a20b78,8))
            if not base or not 0x6ba8<=stride<=0x10000: raise ValueError('host pool layout is unverified')
            host=base+(handle&0x1fff)*stride
            result.update(host=hex(host),client_sent_baseline=zone(host+0x6798),client_pending=zone(host+0x69b8),dirty_words=memory.read(host+0x6b20,8).hex(),send_role_flags=memory.read(host+0x128,2).hex())
        except (OSError,ValueError,struct.error) as exc: result['unavailable']=str(exc)
        key={k:v for k,v in result.items() if k!='at'}
        if key!=previous:
            stream.write(json.dumps(result)+'\n');stream.flush();previous=key
        time.sleep(.75)

