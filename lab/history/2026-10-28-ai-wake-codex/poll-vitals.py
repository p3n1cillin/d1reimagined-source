"""Bounded 30-second read-only observation, no input or native calls."""
from pathlib import Path
import importlib.util,json,time,struct
sp=importlib.util.spec_from_file_location('v',Path(__file__).with_name('read-vitals.py'));v=importlib.util.module_from_spec(sp);sp.loader.exec_module(v)
import d1_unit_tracking as ut
p,base=v.g.open_client();r=v.SquadReader(p,base)
out=Path(__file__).with_name('pool-monitor-%d.jsonl'%time.time())
with out.open('w') as f:
 for _ in range(150):
  row={'time':time.time(),'targets':[]};me=ut.read_unit_position(p,base);row['guardian']=me.position if me else None
  try:
   for t in r.snapshot(0x062dd95d)['targets']:
    entry=p.u(base+0x25d79a0+8,'Q')+(t['member_handle']&8191)*p.u(base+0x25d79a0+16,'I')
    row['targets'].append({'squad':t['squad'],'position':t['parts'][0]['position'],'awareness_bytes':p.read(entry+0x2340,8).hex(),'entity_flags':p.read(entry,24).hex(),'vitals':v.read_vitals(r,t)})
   f.write(json.dumps(row)+'\n');f.flush()
  except (OSError,AssertionError,ValueError,RuntimeError) as e:f.write(json.dumps({'time':time.time(),'error':str(e)})+'\n')
  time.sleep(.2)
print(out)
