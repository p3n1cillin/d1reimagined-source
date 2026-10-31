import json
from pathlib import Path

source=Path('F:/d1/d1xenia/missions/a_guardian_rises.json')
candidate=Path(__file__).with_name('JOIN182-mission-marker-candidate.json')
production=json.loads(source.read_text())
tested=json.loads(candidate.read_text())
targets={'obj_enter_wall':(87,14),'obj_investigate':(65,3),
         'obj_get_weapon':(78,3),'obj_graveyard':(88,3)}
changed=0
assert len(production['steps']) == len(tested['steps'])
for before, after in zip(production['steps'],tested['steps']):
    expected=json.loads(json.dumps(before))
    for name,(nav,bucket) in targets.items():
        if name not in expected.get('set',{}):continue
        state=expected['set'][name]
        phase=state.get('0')
        if phase in (1,5):
            marker={'0':1,'0xc':bucket,'0x20':{'0':0x992bb471,'4':0x2c,'6':nav}}
        elif phase in (0,3):
            marker={'0':0}
        else:
            continue
        state['0x40']={'0':marker}
        changed+=1
    assert expected == after, 'Candidate contains a non-marker change'
assert production['objects'] == tested['objects']
assert production['clock'] == tested['clock']
assert changed == 14, changed
production['steps']=tested['steps']
production['comment'] += [
    '01.24 JOIN182: raw kind-13 positions require 96 wire bits even when schema width is zero.',
    'Objective markers use entry 0 at +0x40, type 1, authored nav target at entry+0x20 and null root fallback slice bucket (112>>3=14, 24>>3=3).',
    'JOIN182 verifies the normal-view Enter the Wall diamond and disabled marker after completion; the next three target entries are configured but still need native visibility acceptance.',
    'Normal/Nav Mode objective text, complete revival shots/Ghost and stable Breach progression remain incomplete.'
]
source.write_text(json.dumps(production,indent=2)+'\n')
print('Promoted',changed,'marker lifecycle states; no cinematic, trigger, clock or inventory changes')
