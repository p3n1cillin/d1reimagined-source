from pathlib import Path
import importlib.util,sys,json,time
run=Path(r'F:\d1\d1preservation-main\cfw-staging\sense34')
assert not (run/'replay.lock').exists()
shots=run/'kill-fight';shots.mkdir()
p=Path(r'F:\d1\public-work\d1reimagined-stats\tools\ps4_setup\d1-waypoint.py')
s=importlib.util.spec_from_file_location('fight',p);m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
# Measured from the current 1936x1119 capture: the D1 hip reticle is lower than window centre.
m.RY=652
with (run/'kill-control-events.jsonl').open('a') as f:f.write(json.dumps({'utc':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),'action':'existing waypoint fight driver, private measured hip reticle Y652 diagnostic','seconds':150,'stick_nudge_each_loop':True})+'\n')
sys.argv=[str(p),'--tracked',str(run/'tracked.json'),'--seconds','150','--shots',str(shots),'--fight']
m.main()
