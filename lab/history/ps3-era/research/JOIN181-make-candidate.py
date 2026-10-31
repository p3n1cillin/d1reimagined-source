"""Private marker registration candidate, pending native visibility acceptance."""
import json,sys,datetime,hashlib
from pathlib import Path
root=Path(__file__).parent
repo=Path('F:/d1/d1xenia')
mission=json.loads((repo/'missions/a_guardian_rises.json').read_text())
# Static 57b4ec compares marker+c against (active slice-set >>3)&63 when root+4 is null.
# Native JOIN180 retains active_loading112 outside and24 inside: buckets14 and3.
# Candidate uses the authored nav target at entry+20; root+4 stays null.
targets={'obj_enter_wall':(87,14),'obj_investigate':(65,3),
         'obj_get_weapon':(78,3),'obj_graveyard':(88,3)}
for step in mission['steps']:
    for name,state in step.get('set',{}).items():
        if name not in targets:continue
        target,bucket=targets[name]
        entry={'0':0}
        if state['0'] in (1,5):
            entry={'0':1,'0xc':bucket,'0x20':{'0':0x992bb471,'4':0x2c,'6':target}}
        state['0x40']={'0':entry}
mission['name']=mission.get('name','A Guardian Rises')+' — private JOIN181 marker test'
mission.setdefault('notes',[]).append('Private static-derived marker candidate: nonzero type, entry+20 authored nav ref, entry+c current slice bucket. Root reference stays null. No cinematic timing/cast changes; native visibility NOT ACCEPTED.')
path=root/'JOIN181-mission-marker-candidate.json'
assert not path.exists()
path.write_text(json.dumps(mission,indent=2)+'\n')
print(str(path),hashlib.sha256(path.read_bytes()).hexdigest())
