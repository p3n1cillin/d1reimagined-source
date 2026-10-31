from pathlib import Path
import importlib.util,sys,subprocess,json,time,datetime,math
root=Path(__file__).resolve().parent;run=Path(r'F:\d1\d1preservation-main\cfw-staging\sense35');host=Path(r'F:\d1\d1preservation-main\cfw-staging\contained-boot\20261028-065948-sense35\activity-host.log');tools=Path(r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup')
stop_at=datetime.datetime(2026,10,28,7,40,tzinfo=datetime.timezone.utc).timestamp()
def load(n,p):
 s=importlib.util.spec_from_file_location(n,p);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
walk=load('walk35',tools/'d1-walk.py');replay=load('replay35',tools/'d1-agr-replay.py')
def event(kind,**data):
 v={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'event':kind,**data};print(json.dumps(v),flush=True)
 with (run/'divide-control-events.jsonl').open('a') as f:f.write(json.dumps(v)+'\n')
def command(name,args,seconds):
 if time.time()+seconds>stop_at:raise RuntimeError('Safe checkpoint deadline before another command')
 event('start',step=name)
 with (run/(name+'.log')).open('x') as f:r=subprocess.run(args,stdout=f,stderr=subprocess.STDOUT,timeout=seconds+20)
 lines=(run/(name+'.log')).read_text(encoding='utf-8',errors='replace').splitlines();event('end',step=name,code=r.returncode,last=lines[-1:] )
 if r.returncode:raise RuntimeError(name+' failed')
 return lines
while time.time()<stop_at:
 if not (run/'replay.lock').exists() and replay.contains(host,'client is now in slice set 96'):break
 time.sleep(2)
else:raise RuntimeError('No completed Divide replay before safe checkpoint')
event('replay_input_released')
c=walk.Client()
def check_pose():
 tracked=json.loads((run/'tracked.json').read_text());q=walk._unit_tracking.read_unit_position(c.p,c.base);p=walk.position(c,tracked)
 if q is None or p is None or math.dist(p[:2],q.position[:2])>1:raise RuntimeError('Heuristic/current-unit XY mismatch; no subsequent input')
 event('pose_control',tracked=len(tracked),heuristic=p,native=q.position,unit=q.handle)
check_pose()
for i,goal in enumerate(['170,-591','153,-606']):
 lines=command('divide-near-'+str(i),[sys.executable,'-u','-I',str(tools/'d1-explore.py'),'--tracked',str(run/'tracked.json'),'--goal',goal,'--out',str(run/f'divide-near-{i}-trail.json'),'--max-seconds','120','--reach','5','--stop-on-death'],130)
 if not lines or not lines[-1].startswith('reached '):raise RuntimeError('Goal incomplete; no next input')
 check_pose()
if time.time()+170>stop_at:raise RuntimeError('No combat budget before safe checkpoint')
g=load('guard35',root/'claude-combat-tools'/'guard.py');g.fo.RY=652
for m in [g.fight,g.fo.fight]:
 original=m.pad
 def pad(keys,_original=original):return _original('Hold:A:80,Hold:D:80,'+keys)
 m.pad=pad
base_burst=g.fo.ads_burst
def burst(ms=600):
 try:return base_burst(ms)
 finally:g.fo.key(0x4f,False);g.fo.key(0x45,False)
g.fo.ads_burst=burst
first=walk._unit_tracking.read_unit_position(c.p,c.base)
if first is None:raise RuntimeError('No current Guardian before combat')
original_grab=g.fight.grab
def grab(path):
 q=walk._unit_tracking.read_unit_position(c.p,c.base)
 if q is None or q.handle!=first.handle:raise RuntimeError('Combat stopped on Guardian unit loss/change')
 return original_grab(path)
g.fight.grab=grab
shots=run/'guard-near-control';shots.mkdir();event('combat',seconds=150,reticle_y=652,first_unit=first.handle)
sys.argv=['guard.py','--seconds','150','--shots',str(shots)]
try:g.main()
finally:
 for key in [0x4f,0x45,0x57,0x41,0x53,0x44]:g.fo.key(key,False)
event('combat_finished',note='Inspect actual kill evidence; driver completion is not acceptance')
