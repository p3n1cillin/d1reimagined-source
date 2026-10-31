import json,sys,datetime
from pathlib import Path
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from rpcs3_owned_memory import OwnedMemory
from rpcs3_final124_performance import snapshot
root=Path(__file__).parent
with OwnedMemory(root.parent/'SIGNON_LOCAL/logs/FINAL124_JOIN184-processes.json') as memory:
    state=snapshot(memory.read,revival_only=True,include_actors=True)
state['utc']=datetime.datetime.now(datetime.timezone.utc).isoformat()
(root/'JOIN184-probe-last.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'utc':state['utc'],'count':state['count'],'unchanged':state['unchanged_during_read'],
                  'performances':[{k:row.get(k) for k in ('asset','phase_raw','scene_count','scene_index','error')} for row in state['performances']]}))
