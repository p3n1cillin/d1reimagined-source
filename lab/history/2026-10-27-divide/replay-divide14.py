"""Private replay of Claude's run-11 chain; every movement uses the existing pad driver."""
import json
from pathlib import Path
import subprocess
import sys
import time

PYTHON = sys.executable
REPO = Path(r'F:\d1\public-work\d1reimagined-source')
TOOLS = Path('lab/history\\2026-10-27-divide\\fixed-rate-driver')
ROUTES = REPO / 'missions/routes'
PRIVATE = Path(__file__).resolve().parent
STAGING = Path(r'F:\d1\d1preservation-main\cfw-staging')
RUN = STAGING / '2026-10-27-divide14'
TRACKED = RUN / 'tracked.json'


def command(name, args, seconds):
    path = RUN / (name + '.log')
    print(time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()), name, 'starting', flush=True)
    with path.open('w') as output:
        result = subprocess.run(args, stdout=output, stderr=subprocess.STDOUT, timeout=seconds)
    lines = path.read_text(errors='replace').splitlines()
    print(name, 'exit', result.returncode, 'last:', lines[-1] if lines else '(empty)', flush=True)
    if result.returncode:
        raise RuntimeError(f'{name} failed; preserve {path}')
    return lines


def walk(name, route, seconds, tolerance=1.3, extra=()):
    args = [PYTHON, '-u', '-I', str(TOOLS/'d1-walk.py'), '--tracked', str(TRACKED),
            '--route', str(route), '--max-seconds', str(seconds), '--tolerance', str(tolerance), '--turn-sign', '1', *extra]
    lines = command(name, args, seconds + 45)
    if not lines or not lines[-1].startswith('done '):
        raise RuntimeError(f'{name} stopped before completing its route; no subsequent input sent')


def pad(key):
    command('pad-' + key.lower(), ['powershell', '-NoProfile', '-File', r'F:\d1\tools\d1-pad.ps1',
                                  '-Keys', key], 15)


def publish(path):
    control = STAGING/'ah-control.txt'
    # Exclusive create protects a command already waiting for the host.
    with control.open('x') as file:
        file.write('clients 0x4786c0e0 ' + path.as_posix() + '\n')
    deadline = time.monotonic() + 20
    while control.exists() and time.monotonic() < deadline:
        time.sleep(.5)
    if control.exists():
        raise RuntimeError('AH did not consume the control file; stopping')
    print('host consumed', path.name, flush=True)
    time.sleep(6)


def main():
    if '--resume-wall' not in sys.argv:
        deadline = time.monotonic() + 420
        while 'in gameplay (112)' not in (RUN/'boot.log').read_text(errors='replace'):
            if time.monotonic() >= deadline:
                raise RuntimeError('Boot did not reach gameplay; no route input sent')
            time.sleep(3)
        time.sleep(20)
        command('scan', [PYTHON, '-u', '-I', str(TOOLS/'d1-walk.py'), '--scan', '383,-962,12.5',
                         '--radius', '25', '--out', str(TRACKED)], 120)
        assert len(json.loads(TRACKED.read_text())) >= 10, 'Insufficient fresh tracking'
    walk('enter-wall-fixed-rate', ROUTES/'enter_the_wall.json', 650, 2.5)
    walk('breach', ROUTES/'breach.json', 640, extra=('--avoid', '330,-837,340,-812'))
    walk('platform', ROUTES/'breach_platform.json', 200)
    walk('rifle', PRIVATE/'rifle.json', 150, 1.5)
    command('rifle-movement-control', [PYTHON, '-I', str(TOOLS/'d1-step.py'), '--tracked', str(TRACKED),
            '--trail', str(RUN/'trail.json'), '--keys', 'Hold:S:1200,Hold:W:1300'], 20)
    command('retrack', [PYTHON, '-u', '-I', str(TOOLS/'d1-walk.py'), '--tracked', str(TRACKED), '--retrack'], 60)
    walk('chest', PRIVATE/'to-chest.json', 840)
    walk('mine-stairs', PRIVATE/'mine-a.json', 90, 1.2)
    pad('NumPad6')
    walk('mine-crouched', PRIVATE/'mine-b.json', 240, 1.5)
    pad('NumPad6')
    walk('mine-junction', PRIVATE/'mine-c.json', 150, 1.2)
    walk('junction-west', PRIVATE/'jw-route-from4.json', 330)
    publish(RUN/'breach-prop3-only.txt')
    walk('garage-door-prop3-only', ROUTES/'wall_garage_door.json', 150, 1.0)
    walk('fan-to-divide', ROUTES/'wall_fan_tunnel_to_divide.json', 420)
    hosts = list((STAGING/'contained-boot').glob('*2026-10-27-divide14/activity-host.log'))
    assert len(hosts) == 1, 'Expected one owned AH log'
    deadline = time.monotonic() + 45
    while 'client is now in slice set 96' not in hosts[0].read_text(errors='replace'):
        if time.monotonic() >= deadline:
            raise RuntimeError('No natural slice-96 crossing observed; authority not changed')
        time.sleep(1)
    print('Scheduled Divide frame must be verified in AH log and native state; no manual slice-96 publish', flush=True)
    print('DIVIDE READY: scheduled authority and squad frame awaits native readback', flush=True)
    

if __name__ == '__main__':
    main()
