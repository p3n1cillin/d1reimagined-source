import json
from pathlib import Path
root=Path(__file__).parent
mission=json.loads(Path('F:/d1/d1xenia/missions/a_guardian_rises.json').read_text(encoding='utf-8'))
assert [step['at'] for step in mission['steps'] if 'at' in step]==[20]
with (root/'JOIN187-mission-candidate.json').open('x',encoding='utf-8') as output:
    json.dump(mission,output,indent=2);output.write('\n')