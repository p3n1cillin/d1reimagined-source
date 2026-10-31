import json
from pathlib import Path
root=Path(__file__).parent
mission=json.loads(Path('F:/d1/d1xenia/missions/a_guardian_rises.json').read_text(encoding='utf-8'))
for step in mission['steps']:
    at=step.get('at')
    if at==20:
        step['at']=90
    elif at in (90,135,140):
        del step['at']
        step['on']={'type':18,'object':'cine_revival' if at==90 else 'cine_revival_fp', 'cinematic_complete':True}
        if at==140:step['delay']=5
mission['comment'].append('PRIVATE JOIN186: completion-driven revival handovers. First start at90 solely for inspected dialog removal. Original production start20 unchanged; manual controls disqualify whole-flow acceptance.')
with (root/'JOIN186-mission-candidate.json').open('x',encoding='utf-8') as output:json.dump(mission,output,indent=2);output.write('\n')
