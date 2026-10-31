"""Preflight a native-identified prop command, then send a diagnostic control."""
import datetime,json,sys
from pathlib import Path
sys.path.insert(0,'F:/d1/d1xenia/server')
import bap_server as bap, investment as inv
root=Path(__file__).parent
phase=int(sys.argv[1]); channel=int(sys.argv[2])
if phase not in (1,2,3,4,5) or channel not in (0,1,2): raise ValueError('bounded diagnostic')
dest=root/f'JOIN210-activation-{phase}.json'
if dest.exists(): raise ValueError('preserve existing request')
schema=json.loads(Path('F:/d1/d1xenia-data/ps3_updates/types_01.24.json').read_text())
mission=json.loads(Path('F:/d1/d1xenia/missions/a_guardian_rises.json').read_text())
payload={str(i):0 for i in range(0,36,4)}
for i in range(channel+1): payload.update({str(i*12):1,str(i*12+4):1,str(i*12+8):1.0})
if phase in (4,5): payload.update({'24':phase-2,'28':phase-2,'32':0.0 if phase==4 else 1.0})
values={'16':{'0':1,'16':[{'type':'0x8080279a','values':payload}]}}
line={'set':{name:{'0':3 if name=='o_breach_doors_lights' else 1,'4':True,'8':{'0':2166136261,'4':-1,'6':-1},**values} for name in ('o_breach_doors_lights','o_breach_control','o_breach_gating_lights')}}
timed,_=bap.expand_mission({'objects':mission['objects'],'steps':[{'at':0,**line}]},schema)
assert bap.validate_mission_states(schema,timed,[])==3
encoded=[]
for _,spec in timed:
    wire=inv.encode(schema,spec[1],spec[2]); decoded=inv.decode(schema,spec[1],wire)
    assert decoded==spec[2],(decoded,spec[2])
    encoded.append(wire.hex())
dest.write_text(json.dumps({'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'scope':'Manual native-identified State A component command. Values clamped by native handler to 0..1; no lifecycle-generation change, guest writes or teleport. Candidate requires visual and traversal verification.','control':line,'encoded':encoded},indent=2)+'\n')
with (root.parent/'SIGNON_LOCAL/FINAL124_JOIN210-control.jsonl').open('a',encoding='utf-8') as f: f.write(json.dumps(line)+'\n')
print(dest)
