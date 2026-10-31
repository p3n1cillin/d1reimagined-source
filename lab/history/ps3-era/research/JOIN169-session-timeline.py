import datetime, json, struct, sys, time
from pathlib import Path
sys.path.insert(0, r'F:\d1\d1xenia\scripts')
from rpcs3_owned_memory import OwnedMemory
rt=Path(r'F:\d1\<pc>-import\d1xenia\local')
manifest=rt/'SIGNON_LOCAL/logs/FINAL124_JOIN169-processes.json'
output=rt/'RESEARCH/JOIN169-session-timeline.jsonl'
def u32(read, address): return struct.unpack('>I',read(address,4))[0]
def pair(read, address): return list(struct.unpack('>iI',read(address,8)))
with output.open('x',encoding='utf-8',buffering=1) as stream, OwnedMemory(manifest) as memory:
    end=time.monotonic()+70
    previous=None
    while time.monotonic()<end:
        read=memory.read
        engine=u32(read,0x1abc8ac)
        manager=u32(read,0x1abc8a4)
        mapping=u32(read,0x1abc79c)
        slots=list(struct.unpack('>4I',read(mapping,16))) if mapping else []
        state={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'engine':hex(engine),
               'flags_88_8b':list(read(engine+0x88,4)),
               'native_current':pair(read,engine+0x90),'native_target':pair(read,engine+0x130),
               'last_sent_current':pair(read,engine+0x1f8),'last_sent_target':pair(read,engine+0x298),
               'engine_version':u32(read,engine+0x3b0),'local_peer':u32(read,engine+0x3b4),
               'reported_peer_byte':read(engine+0x1d0,1)[0],
               'group0':read(engine+0x3b8,64).hex(),'group1':read(engine+0x490,64).hex(),
               'manager':hex(manager),'mapping':hex(mapping),'mapped_slots':slots,'sessions':[]}
        for slot in sorted(set(slots)):
            if slot<16 and manager:
                record=manager+0x4e0+slot*0x170
                state['sessions'].append({'slot':slot,'state':u32(read,record),
                    'context_handle':hex(u32(read,record+0x14))})
        table=u32(read,0x1a1dc80)
        state['active_loading']=list(struct.unpack('>2i',read(table+0x110c,8)))
        state['loaded_words']=list(struct.unpack('>16I',read(table+0x1114,64)))
        state['stable_pointers']=u32(read,0x1abc8ac)==engine and u32(read,0x1abc8a4)==manager
        stream.write(json.dumps(state)+'\n')
        significant={k:v for k,v in state.items() if k!='utc'}
        if significant!=previous:
            print(json.dumps(state),flush=True)
            previous=significant
        time.sleep(.5)
