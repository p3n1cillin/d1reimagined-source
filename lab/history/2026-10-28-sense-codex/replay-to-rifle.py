from pathlib import Path
import importlib.util,sys
tools=Path(r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup')
spec=importlib.util.spec_from_file_location('replay',tools/'d1-agr-replay.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
run,host=map(Path,sys.argv[1:3])
class ReachedRifle(Exception):pass
class RifleReplay(m.Replay):
 def command(self,name,args,seconds):
  if name=='rifle-movement-control':raise ReachedRifle()
  return super().command(name,args,seconds)
lock=run/'replay.lock'
with lock.open('x') as f:f.write('Canonical partial replay owns pad input through natural rifle trigger35.\n')
try:
 try:RifleReplay(run,host,run/'breach-prop3-only.txt',Path(r'F:\d1\tools\d1-pad.ps1')).play(900,False)
 except ReachedRifle:print('RIFLE REACHED; input released, no combat input sent',flush=True)
finally:lock.unlink()
