"""Bounded pad-only probe along Claude's known low-area centre chain; no guest writes."""
from pathlib import Path
import subprocess,sys,time
run=Path(r'F:\d1\d1preservation-main\cfw-staging\weapon26-base-stats')
tools=Path(r'F:\d1\public-work\d1reimagined-source\tools\ps4_setup')
assert 'DIVIDE REACHED' in (run/'replay-console.log').read_text(encoding='utf-8')
lock=run/'replay.lock'
with lock.open('x') as f:f.write('Codex bounded Divide centre probe owns pad.\n')
try:
 for i,goal in enumerate(['262,-640','243,-619','225,-600','195,-573','170,-591','153,-606']):
  log=run/f'divide-centre-{i}.log';out=run/f'divide-centre-{i}.json'
  print(time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),goal,'starting',flush=True)
  with log.open('x',encoding='utf-8') as f:
   r=subprocess.run([sys.executable,'-u','-I',str(tools/'d1-explore.py'),'--tracked',str(run/'tracked.json'),'--goal',goal,'--out',str(out),'--max-seconds','150','--stop-on-death'],stdout=f,stderr=subprocess.STDOUT,timeout=190)
  lines=log.read_text(encoding='utf-8',errors='replace').splitlines();last=lines[-1] if lines else ''
  print('exit',r.returncode,last,flush=True)
  if r.returncode or not last.startswith('reached'):
   raise RuntimeError('Probe stopped before reaching centre; inspect scene before further input')
 print('Stopped at pre-gate centre for supervised combat/HUD inspection.',flush=True)
finally:lock.unlink()
