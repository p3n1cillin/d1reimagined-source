"""Bounded read-only acceptance probe; READ+QUERY rights, no calls/input/writes."""
import runpy,json,time
from pathlib import Path
P=Path(__file__).parent
import sys; sys.path.insert(0,str(P)); import routepaths  # noqa: E402
g=runpy.run_path(str(P/'attach-readonly.py'));b=g['base'];u=g['u']
if not all(u(b+x) for x in [0x2652848,0x26bc130+0x18,0x26bc130+0x38,0x26ca4e0,0x26bc068,0x26ca078]):
 print('NOT_READY: the current client has not populated the required profile/session/content pointers. No gate prediction.');raise SystemExit(0)
glob=u(b+0x2652848)+8;ci=u(glob+0x192,'H');assert ci==2838
interface=b+0x26bc130;profile=u(interface+0x18);plane=u(interface+0x38)+8
root=u(b+0x26ca4e0);slot=u(root+8,'i');assert slot in (0,1)
session=root+slot*0x98e0+0x10;selection=u(b+0x26bc068)
out={'time':time.time(),'base':hex(b),'unlock_index':ci,'profile':hex(profile),
     'profile_completion673':u(profile+0x5661,'B'),'plane':hex(plane),
     'plane_state':u(plane+4+ci,'B'),'plane_valid':u(plane,'B'),
     'leader628':u(session+0x628,'i'),'leader62c':u(session+0x62c,'i'),
     'leader632c':u(session+0x632c,'i'),'selection_type':u(selection,'B'),
     'selection_activity':u(selection+2,'H'),'world_state':u(u(b+0x26ca078)+0x1b0,'i')}
out['leader_predicate']=out['leader628']!=-1 and out['leader62c']==out['leader632c']
out['mode_predicate']=out['plane_state']==2
out['ordinary_branch_predicted']=out['leader_predicate'] and not out['mode_predicate']
dest=routepaths.probes()/f'recall-gate-readonly-{int(time.time())}.json';dest.write_text(json.dumps(out,indent=2));print(json.dumps(out,indent=2));print(dest)
