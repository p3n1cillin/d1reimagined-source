"""Prepare an untested next-run candidate; does not alter or publish current steps."""
import os,subprocess
from pathlib import Path
run=Path(r'F:\d1\d1preservation-main\cfw-staging\2026-10-27-divide16')
oracle=r'F:\d1\public-work\d1reimagined-source\ps4\d1-server\target\release\d1-oracle.exe'
env=dict(os.environ,D1_BUILDS=r'F:\d1\d1preservation-main\capture-rig\builds.json',D1_BUILD='later-56578')
output=run/'divide-gen2-objective17.txt'
subprocess.run([oracle,'sensor','objective','--input',str(run/'divide-squads-gen2.txt'),'--out',str(output),'--group','0x992bb471','--active','17','--clear','14','--marker','46','--machine','12'],env=env,check=True)
text=(run/'steps.txt').read_text()
original=(run/'divide-squads-gen2.txt').as_posix()
assert text.count(original)==1 and '%96|' in text
(run/'steps-objective17-candidate.txt').write_text(text.replace(original,output.as_posix()))
print('Prepared steps-objective17-candidate.txt; not replayed end to end or published')
