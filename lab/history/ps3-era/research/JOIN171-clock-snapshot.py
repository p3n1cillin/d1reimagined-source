import datetime,sys,json,time
sys.path.insert(0,r'F:\d1\d1xenia\scripts')
from rpcs3_owned_memory import OwnedMemory
from rpcs3_final124_session import u32
with OwnedMemory(r'F:\d1\<pc>-import\d1xenia\local\SIGNON_LOCAL\logs\FINAL124_JOIN171-processes.json') as m:
    for _ in range(2):
        p=u32(m.read,0x1ab6710)
        print(json.dumps({'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'clock_pointer':hex(p),'tick_0c':u32(m.read,p+12),'clock_header':m.read(p,32).hex()}),flush=True)
        time.sleep(2)