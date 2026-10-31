import sys,json,struct,datetime
from pathlib import Path
sys.path.insert(0,r'F:\d1\d1xenia\scripts')
from rpcs3_owned_memory import OwnedMemory
root=Path(r'F:\d1\<pc>-import\d1xenia\local')
def u32(read,a): return struct.unpack('>I',read(a,4))[0]
with OwnedMemory(root/'SIGNON_LOCAL/logs/FINAL124_JOIN169-processes.json') as p:
    engine=u32(p.read,0x1abc8ac)
    current=p.read(engine+0x90,0x40); target=p.read(engine+0x130,0x40)
    record={
      'at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'engine':hex(engine),
      'flags_88_8b':list(p.read(engine+0x88,4)),
      'current_words':list(struct.unpack('>16I',current)),
      'target_words':list(struct.unpack('>16I',target)),
      'world_group_490':p.read(engine+0x490,0x40).hex(),
    }
    table=u32(p.read,0x1a1dc80)
    record['slice_table']=hex(table)
    record['active_loading']=list(struct.unpack('>2i',p.read(table+0x110c,8)))
    record['loaded_words']=list(struct.unpack('>16I',p.read(table+0x1114,0x40)))
(root/'SIGNON_LOCAL/logs/JOIN169-engine-snapshot.json').write_text(json.dumps(record,indent=2))
print(json.dumps(record))