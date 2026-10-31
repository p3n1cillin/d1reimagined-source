from pathlib import Path
import sys,json,collections,struct
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from d1_eboot import Eboot
root=Path(__file__).parent
eb=Eboot(root/'ps3_updates/01.24/USRDIR/EBOOT.elf')
for pc,count in ((0x2086d8,260),(0x7ddb0,8)):
    lines=[]
    for address in range(pc,pc+count*4,4):
        lines.extend(list(eb.disasm(address,1)) or [f'{address:08x}: .long {eb.u32(address):08x}'])
    (root/f'JOIN196-static-{pc:x}.txt').write_text('\n'.join(lines)+'\n',encoding='utf-8')
samples=[json.loads(line) for line in (root/'JOIN196-performance-timeline.jsonl').read_text(encoding='utf-8').splitlines()]
transitions=[]
last=None
for sample in samples:
    performances=sample.get('performances',[])
    state=[(p.get('asset'),p.get('phase_raw'),p.get('scene_index')) for p in performances]
    slots=sample.get('actor_slots',[])
    cast=[slot[40:48] for slot in slots]
    current=(state,cast)
    if current!=last:
        transitions.append({'utc':sample.get('utc'),'state':state,'slot_resource_at_14':cast})
        last=current
result={'samples':len(samples),'read_errors':sum('error' in s for s in samples),
        'failed_rechecks':sum(s.get('unchanged_during_read') is False for s in samples),
        'transitions':transitions}
(root/'JOIN196-performance-summary.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
print(json.dumps(result))

