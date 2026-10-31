"""Resume junction after backing off the wall; use canonical fixed-rate CLI."""
import importlib.util, time
from pathlib import Path
s=importlib.util.spec_from_file_location('r',Path(__file__).with_name('replay-divide14.py'))
r=importlib.util.module_from_spec(s)
s.loader.exec_module(r)
r.TOOLS=r.REPO/'tools/ps4_setup'
fixed=('--turn-rate','2.26')
r.walk('junction-canonical-rate',r.PRIVATE/'jw-route-from4.json',330,extra=fixed)
r.publish(r.RUN/'breach-prop3-only.txt')
r.walk('garage-canonical-rate',r.ROUTES/'wall_garage_door.json',150,1.0,extra=fixed)
r.walk('fan-canonical-rate',r.ROUTES/'wall_fan_tunnel_to_divide.json',420,extra=fixed)
logs=list((r.STAGING/'contained-boot').glob('*2026-10-27-divide14/activity-host.log'))
assert len(logs)==1
deadline=time.monotonic()+45
while 'client is now in slice set 96' not in logs[0].read_text(errors='replace'):
    if time.monotonic()>deadline:raise RuntimeError('No slice-96 crossing')
    time.sleep(1)
print('DIVIDE reached; no manual slice-96 sensor publish. Check scheduled step and native states.',flush=True)
