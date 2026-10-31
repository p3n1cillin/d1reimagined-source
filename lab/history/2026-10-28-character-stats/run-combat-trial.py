from pathlib import Path
import argparse,datetime,json,os,subprocess,sys,time

ap=argparse.ArgumentParser()
ap.add_argument('--run',type=Path,required=True)
ap.add_argument('--label',required=True)
a=ap.parse_args()
tools=Path(r'F:\d1\public-work\d1reimagined-stats\tools\ps4_setup')
out=a.run/a.label;out.mkdir()
if (a.run/'replay.lock').exists():raise RuntimeError('Replay owns input; no movement sent')
started=time.time();events=[]
def event(kind,**values):
 x=dict(utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),event=kind,**values)
 events.append(x);(out/'events.json').write_text(json.dumps(events,indent=2));print(json.dumps(x),flush=True)
def run(name,args,seconds):
 event('start',step=name)
 with (out/(name+'.log')).open('x') as log:
  r=subprocess.run(args,stdout=log,stderr=subprocess.STDOUT,timeout=seconds)
 text=(out/(name+'.log')).read_text(errors='replace')
 event('end',step=name,returncode=r.returncode,last=text.splitlines()[-1:] )
 assert r.returncode==0,name
 return text
event('trial',label=a.label,changes='See parent run plan; no memory writes; pad inputs only')
run('activity',['powershell','-NoProfile','-File',r'F:\d1\tools\d1-pad.ps1','-Keys','Hold:A:80,Hold:D:80'],15)
for i,goal in enumerate(['262,-640','243,-619','225,-600','195,-573','170,-591','153,-606','143,-609']):
 text=run('goal'+str(i),[sys.executable,'-u','-I',str(tools/'d1-explore.py'),'--tracked',str(a.run/'tracked.json'),'--goal',goal,'--out',str(out/f'goal{i}-trail.json'),'--max-seconds','120','--reach','5','--stop-on-death'],150)
 if not text.splitlines()[-1].startswith('reached '):
  event('incomplete',reason='goal not reached; no later goal or combat input sent',elapsed=time.time()-started)
  sys.exit(2)
shots=out/'fight';shots.mkdir()
run('fight',[sys.executable,'-u','-I',str(tools/'d1-waypoint.py'),'--tracked',str(a.run/'tracked.json'),'--seconds','120','--shots',str(shots),'--fight'],150)
event('complete',elapsed=time.time()-started,note='Inspect native logs and frames; completion is not proof of survival or kills')
