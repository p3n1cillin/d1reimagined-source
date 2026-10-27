"""Bounded pad-only replay from the 56578 tutorial spawn to the Divide.

Run alongside d1-agr-run.ps1, redirecting that launcher's output to RUN/boot.log.
Native fixtures and logs stay in RUN; this script neither boots nor patches the client.
See README.md for preparation and the limits of this diagnostic route.
"""
import argparse
import json
import math
import os
from pathlib import Path
import subprocess
import sys
import time

HERE = Path(__file__).resolve().parent
ROUTES = HERE.parents[1] / 'missions' / 'routes'
RIFLE_EVENT = 'player trigger 0x992bb471/0x1d/35 reported'


def contains(path, needle, offset=0):
    """Read a growing log in bounded chunks, including matches across chunk edges."""
    if not path.exists():
        return False
    needle = needle.encode('utf-8')
    overlap = b''
    with path.open('rb') as stream:
        stream.seek(offset)
        while chunk := stream.read(65536):
            chunk = overlap + chunk
            if needle in chunk:
                return True
            overlap = chunk[-len(needle):]
    return False


def wait_for(path, needle, seconds, *, offset=0, clock=time.monotonic, sleep=time.sleep):
    deadline = clock() + seconds
    while True:
        if contains(path, needle, offset):
            return True
        if clock() >= deadline:
            return False
        sleep(1)


class Replay:
    def __init__(self, run, host_log, garage_clients, pad, routes=ROUTES):
        self.run, self.host_log, self.garage_clients = run, host_log, garage_clients
        self.pad_path, self.routes = pad, routes
        self.tracked = run / 'tracked.json'

    def command(self, name, args, seconds):
        # Preserve earlier attempts and evidence, including explicit resumes.
        index = 0
        while True:
            path = self.run / f'replay-{name}-{index}.log'
            try:
                output = path.open('x', encoding='utf-8')
                break
            except FileExistsError:
                index += 1
        print(time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()), name, 'starting', flush=True)
        with output:
            result = subprocess.run(args, stdout=output, stderr=subprocess.STDOUT, timeout=seconds,
                                    env=dict(os.environ, D1_PAD=str(self.pad_path)))
        lines = path.read_text(encoding='utf-8', errors='replace').splitlines()
        print(name, 'exit', result.returncode, 'last:', lines[-1] if lines else '(empty)', flush=True)
        if result.returncode:
            raise RuntimeError(f'{name} failed; preserve {path}')
        return lines

    def walk(self, name, route, seconds, tolerance=1.3, extra=(), strict=True):
        lines = self.walk_once(name, route, seconds, tolerance, extra)
        done = bool(lines and lines[-1].startswith('done '))
        if strict and not done:
            # Run 18 stalled at the Wall's stair corner (waypoint 45 of enter_the_wall)
            # and ran out of time; the rest of the leg was walkable. Resume once from
            # the waypoint nearest the stop, then give up.
            resumed = self.resume_route(name, route, lines)
            if resumed:
                lines = self.walk_once(f'{name}-resume', resumed, seconds, tolerance, extra)
                done = bool(lines and lines[-1].startswith('done '))
        if strict and not done:
            raise RuntimeError(f'{name} stopped before completing its route; no subsequent input sent')
        return done

    def walk_once(self, name, route, seconds, tolerance, extra):
        return self.command(name, [sys.executable, '-u', '-I', str(HERE/'d1-walk.py'),
            '--tracked', str(self.tracked), '--route', str(route), '--max-seconds', str(seconds),
            '--tolerance', str(tolerance), '--turn-sign', '1', '--turn-rate', '2.26', *extra], seconds + 45)

    def resume_route(self, name, route, lines):
        """The route from the waypoint nearest the walker's final `stopped (x, y, z)`, or None."""
        last = lines[-1] if lines else ''
        if not last.startswith('stopped ('):
            return None
        try:
            stop = [float(v) for v in last[len('stopped ('):last.index(')')].split(',')]
        except ValueError:
            return None
        points = json.loads(Path(route).read_text())
        nearest = min(range(len(points)), key=lambda i: math.dist(points[i][:2], stop[:2]))
        index = 0
        while (self.run / f'replay-{name}-resume-{index}.json').exists():
            index += 1
        return self.write_route(f'{name}-resume-{index}', points[nearest:])

    def pad(self, *keys):
        self.command('pad', ['powershell', '-NoProfile', '-File', str(self.pad_path),
                            '-Keys', ','.join(keys)], 20)

    def rifle(self):
        # Route completion is not the pickup postcondition. Run 17 stopped two units
        # short; a tighter second walk reached trigger 35. Never proceed without it.
        for attempt in range(2):
            if contains(self.host_log, RIFLE_EVENT):
                return
            self.walk(f'rifle-{attempt}', self.routes/'breach_rifle.json', 150, 1.0, strict=False)
            if wait_for(self.host_log, RIFLE_EVENT, 20):
                return
        raise RuntimeError('Rifle trigger 35 not observed after two approaches; no subsequent input sent')

    def scan_spawn(self):
        # The teleport-completed log precedes stable player tracking. Preserve the
        # original replay's settling interval, then allow one fresh scan attempt.
        time.sleep(20)
        for attempt in range(2):
            self.command(f'scan-{attempt}', [sys.executable, '-u', '-I', str(HERE/'d1-walk.py'),
                '--scan', '383,-962,12.5', '--radius', '25', '--out', str(self.tracked)], 120)
            if len(json.loads(self.tracked.read_text())) >= 10:
                return
            if attempt == 0:
                time.sleep(20)
        raise RuntimeError('Insufficient fresh position tracking; no route input sent')

    def publish_garage(self):
        control = self.run.parent / 'ah-control.txt'
        offset = self.host_log.stat().st_size
        with control.open('x', encoding='utf-8') as output:
            output.write('clients 0x4786c0e0 ' + self.garage_clients.as_posix() + '\n')
        deadline = time.monotonic() + 20
        while control.exists() and time.monotonic() < deadline:
            time.sleep(.5)
        if control.exists():
            raise RuntimeError('AH did not consume the garage control; stopping')
        # Consumption is not acceptance. Require the host's success line as well.
        accepted = 'control: sensor clients ' + self.garage_clients.as_posix() + ' ('
        if not wait_for(self.host_log, accepted, 20, offset=offset):
            raise RuntimeError('AH did not log acceptance of the garage list; stopping')
        time.sleep(6)

    def sliced_route(self, name, source, start, stop):
        points = json.loads((self.routes/source).read_text())
        return self.write_route(name, points[start:stop])

    def write_route(self, name, points):
        path = self.run / f'replay-{name}.json'
        if not points:
            raise RuntimeError(f'Empty {name} route')
        text = json.dumps(points) + '\n'
        if path.exists():
            if json.loads(path.read_text()) != points:
                raise RuntimeError(f'Refusing to replace a different route: {path}')
        else:
            with path.open('x') as output:
                output.write(text)
        return path

    def play(self, boot_timeout, resume):
        if not resume:
            # The launcher owns input during boot. Do not race its cinematic skips.
            if not wait_for(self.run/'boot.log', 'in gameplay (112)', boot_timeout):
                raise RuntimeError('Boot did not reach gameplay; no route input sent')
            self.scan_spawn()
            self.walk('enter-wall', self.routes/'enter_the_wall.json', 650, 2.5)
            self.walk('breach', self.routes/'breach.json', 640, extra=('--avoid', '330,-837,340,-812'))
            self.walk('platform', self.routes/'breach_platform.json', 200)
            self.rifle()
        elif not contains(self.host_log, RIFLE_EVENT):
            raise RuntimeError('Resume requires the rifle trigger in this run; no route input sent')
        self.command('rifle-movement-control', [sys.executable, '-I', str(HERE/'d1-step.py'),
            '--tracked', str(self.tracked), '--trail', str(self.run/'replay-trail.json'),
            '--keys', 'Hold:S:1200,Hold:W:1300'], 20)
        self.command('retrack', [sys.executable, '-u', '-I', str(HERE/'d1-walk.py'),
                              '--tracked', str(self.tracked), '--retrack'], 60)
        # The proven session chain is the existing Wall route followed by the
        # chest trail, omitting its first point at the shared junction.
        wall = json.loads((self.routes/'fight_through_the_wall.json').read_text())
        chest = json.loads((self.routes/'wall_container_to_chest.json').read_text())
        self.walk('chest', self.write_route('to-chest', wall + chest[1:]), 840)
        self.walk('mine-stairs', self.sliced_route('mine-a', 'wall_tripmine_corridor.json', 0, 4), 90, 1.2)
        self.pad('NumPad6')
        # If this fails, stop crouched. Do not blindly toggle stance in a retry.
        self.walk('mine-crouched', self.sliced_route('mine-b', 'wall_tripmine_corridor.json', 3, 10), 240, 1.5)
        self.pad('NumPad6')
        self.walk('mine-junction', self.sliced_route('mine-c', 'wall_tripmine_corridor.json', 9, None), 150, 1.2)
        self.walk('junction-west', self.sliced_route('junction', 'wall_junction_west.json', 4, None), 330)
        self.publish_garage()
        self.walk('garage', self.routes/'wall_garage_door.json', 150, 1.0)
        self.walk('fan-to-divide', self.routes/'wall_fan_tunnel_to_divide.json', 420)
        if not wait_for(self.host_log, 'client is now in slice set 96', 45):
            raise RuntimeError('No natural slice-96 crossing observed; no Divide control published')
        print('DIVIDE REACHED: verify scheduled step 16, native authority, objective and scene.', flush=True)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--run-dir', type=Path, required=True)
    ap.add_argument('--host-log', type=Path, required=True)
    ap.add_argument('--garage-clients', type=Path, required=True,
                    help='Private list activating only Breach prop 3')
    ap.add_argument('--pad', type=Path, default=Path(r'F:\d1\tools\d1-pad.ps1'))
    ap.add_argument('--boot-timeout', type=int, default=900)
    ap.add_argument('--resume-after-rifle', action='store_true',
                    help='Explicit supervised resume; refresh tracking and verify stance first')
    a = ap.parse_args()
    if a.boot_timeout <= 0:
        ap.error('--boot-timeout must be positive')
    if not a.run_dir.is_dir() or not a.garage_clients.is_file() or not a.pad.is_file():
        ap.error('Run directory, private garage list and existing pad driver must exist')
    if not a.host_log.parent.name.endswith('-' + a.run_dir.name):
        ap.error('--host-log must belong to the selected contained run')
    lock = a.run_dir/'replay.lock'
    # Exclusive create refuses concurrent replay owners. On abnormal termination,
    # inspect processes and the scene before removing a stale lock manually.
    with lock.open('x') as output:
        output.write('Pad replay owns input; do not run another movement driver.\n')
    try:
        Replay(a.run_dir, a.host_log, a.garage_clients, a.pad).play(a.boot_timeout, a.resume_after_rifle)
    finally:
        lock.unlink()


if __name__ == '__main__':
    main()
