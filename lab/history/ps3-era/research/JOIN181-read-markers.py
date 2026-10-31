import json, sys, struct, datetime
from pathlib import Path
sys.path.insert(0, 'F:/d1/d1xenia/scripts')
from rpcs3_owned_memory import OwnedMemory
root=Path(__file__).parent
out={'at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'scope':'Read-only exact owned process; non-atomic samples'}
with OwnedMemory(root.parent/'SIGNON_LOCAL/logs/FINAL124_JOIN181-processes.json') as m:
    u32=lambda a:struct.unpack('>I',m.read(a,4))[0]
    objective=0x607bbc30
    state=m.read(objective+0x110,0x2f0)
    out['enter_wall_component']={'address':hex(objective),'identity':m.read(objective,16).hex(),'state':state.hex(),'marker_0':state[0x40:0x80].hex(),'tail':m.read(objective+0x400,16).hex()}
    manager=u32(0x1acd25c)
    count=u32(manager+0x1350)
    out['cinematic_manager']={'address':hex(manager),'performance_count':count}
    if count <= 256:
        out['cinematic_manager']['records']=m.read(manager+0x1354,count*32).hex()
    out['cast_machine_list']=m.read(manager+0x17d4,104).hex()
    out['event_globals']={'start':hex(u32(0x1aa4944)),'stop':hex(u32(0x1aa4940))}
    out['position_codec']={'quantization_enabled':m.read(0x1aa9460,1)[0],'parameters':m.read(0x1aa9440,48).hex()}
path=root/'JOIN181-marker-owned-sample.json'
path.write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps(out))
