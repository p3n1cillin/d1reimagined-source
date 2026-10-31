"""Resume this owned run after the bounded junction retry; approved driver only."""
import importlib.util
from pathlib import Path
import time
import sys

spec = importlib.util.spec_from_file_location('replay', Path(__file__).with_name('replay-divide12.py'))
r = importlib.util.module_from_spec(spec)
spec.loader.exec_module(r)
if '--resume-garage' not in sys.argv and '--resume-fan' not in sys.argv:
    r.walk('junction-west-retry', r.PRIVATE/'jw-route-from4.json', 330, extra=('--turn-sign', '1'))
    r.publish(r.RUN/'breach-prop3-only.txt')
if '--resume-fan' not in sys.argv:
    r.walk('garage-door-prop3-only-fixed-turn', r.ROUTES/'wall_garage_door.json', 150, 1.0, extra=('--turn-sign', '1'))
r.walk('fan-to-divide-retry', r.ROUTES/'wall_fan_tunnel_to_divide.json', 420, extra=('--turn-sign', '1'))
hosts = list((r.STAGING/'contained-boot').glob('*2026-10-27-divide12/activity-host.log'))
assert len(hosts) == 1
deadline = time.monotonic() + 45
while 'client is now in slice set 96' not in hosts[0].read_text(errors='replace'):
    if time.monotonic() >= deadline:
        raise RuntimeError('No natural slice-96 crossing; authority not changed')
    time.sleep(1)
r.publish(r.STAGING/'agr-s96-clients-auth.txt')
print('DIVIDE READY: authority baseline published; squads unspawned for inspection', flush=True)
