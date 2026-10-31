import datetime,json,sys,time
from pathlib import Path
sys.path.insert(0,r'F:\d1\d1xenia\scripts')
from rpcs3_owned_memory import OwnedMemory
from rpcs3_final124_transition import snapshot
root=Path(r'F:\d1\<pc>-import\d1xenia\local')
manifest=root/'SIGNON_LOCAL/logs/FINAL124_JOIN142-processes.json'
output=root/'SIGNON_LOCAL/logs/JOIN142-transition-timeline.jsonl'
until=time.monotonic()+750
with OwnedMemory(manifest) as memory,output.open('a',encoding='utf-8') as stream:
    previous=None
    while time.monotonic()<until and not Path(str(manifest)+'.stop').exists():
        try:
            state=snapshot(memory.read)
            key={k:v for k,v in state.items() if k!='captured_utc'}
            if key!=previous:
                stream.write(json.dumps(state)+'\n');stream.flush()
                previous=key
        except (OSError,ValueError) as exc:
            stream.write(json.dumps({'at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'unavailable':str(exc)})+'\n');stream.flush()
        time.sleep(1)
