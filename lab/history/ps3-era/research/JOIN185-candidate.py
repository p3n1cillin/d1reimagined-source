import json
from pathlib import Path
root=Path(__file__).parent
mission=json.loads(Path('F:/d1/d1xenia/missions/a_guardian_rises.json').read_text(encoding='utf-8'))
removed=[step for step in mission['steps'] if step.get('at') in (90,135,140)]
assert len(removed)==3
mission['steps']=[step for step in mission['steps'] if step not in removed]
mission['comment'].append('PRIVATE JOIN185 diagnostic: no fixed revival stops or 113->112 teleport. Third-person authored completion observed before any manual handover. Not production mission acceptance.')
path=root/'JOIN185-mission-candidate.json'
with path.open('x',encoding='utf-8') as output:json.dump(mission,output,indent=2);output.write('\n')
with (root/'JOIN185-removed-timed-steps.json').open('x',encoding='utf-8') as output:json.dump(removed,output,indent=2);output.write('\n')
