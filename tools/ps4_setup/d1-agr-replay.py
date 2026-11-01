"""Bounded pad-only replay from the 56578 tutorial spawn to the Divide.

Run alongside d1-agr-run.ps1. The launcher records its milestones in RUN/launcher.log; a redirected
RUN/boot.log is read too.
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
sys.path.insert(0, str(HERE))
import d1paths  # sibling: local paths from D1_ROOT or paths.json
ROUTES = HERE.parents[1] / 'missions' / 'routes'
LAB = HERE.parents[1] / 'lab' / 'tower-route'   # combat2.py, seek.py, target.py, release.py
RIFLE_EVENT = 'player trigger 0x992bb471/0x1d/35 reported'
DIVIDE_SLICE = 'client is now in slice set 96'
DIVIDE_STEP = 'mission step 16 at '   # agr-steps-divide.txt: the Divide's list (%96)
BREACH_SQUADS = '21,26,28,30'   # the squads ps4/fixtures/56578/mission/agr-b-graveyard-fallen.txt gives states
# --waves: the mission schedule serves the Breach by its own triggers (d1-breach-waves.py), so the
# replay publishes nothing and fights whatever those waves placed. The first group is the opening and the
# hall's waves (triggers 108-112), the second the corridor's and the garage's (114-116, 106, 107).
WAVE_SQUADS = ('26,28,30,21,24,38,42,56,58', '44,47,49,51,53,55,72,74,68,76')
JUNCTION_SQUADS = '64,68,76'   # agr-b-graveyard-junction.txt: on or by the garage route (run sense100's map).
# 72 and 74 stand a level below it; in sense101 the fight went down after them and the Guardian died twice.


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
    """True once `needle` appears in `path` (or in any of a tuple of paths), False at the deadline."""
    paths = path if isinstance(path, (list, tuple)) else (path,)
    deadline = clock() + seconds
    while True:
        if any(contains(one, needle, offset) for one in paths):
            return True
        if clock() >= deadline:
            return False
        sleep(1)


class Replay:
    def __init__(self, run, host_log, garage_clients, pad, routes=ROUTES, fallen=None, junction=None, waves=False):
        self.run, self.host_log, self.garage_clients = run, host_log, garage_clients
        self.waves = waves
        self.pad_path, self.routes, self.fallen, self.junction = pad, routes, fallen, junction
        self.tracked = run / 'tracked.json'
        self.native_unit = False
        self.wave = None                   # the squads of the last wave fought, for skirmish()

    def command(self, name, args, seconds, check=True):
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
        if result.returncode and check:
            raise RuntimeError(f'{name} failed; preserve {path}')
        return lines

    def walk(self, name, route, seconds, tolerance=1.3, extra=(), strict=True):
        lines = self.walk_once(name, route, seconds, tolerance, extra)
        done = bool(lines and lines[-1].startswith('done '))
        if strict and not done:
            self.skirmish(name)
            # Run 18 stalled at the Wall's stair corner (waypoint 45 of enter_the_wall)
            # and ran out of time; the rest of the leg was walkable. Resume once from
            # the waypoint nearest the stop, then give up.
            resumed = self.resume_route(name, route, lines)
            if resumed:
                lines = self.walk_once(f'{name}-resume', resumed, seconds, tolerance, extra)
                done = bool(lines and lines[-1].startswith('done '))
            # Run sense45 stalled twice at the same spot against the Wall's fan (x about 387);
            # a sidestep and a jump got past it. Nudge, alternating sides, and resume again.
            for side in ('D', 'A', 'D'):
                if done:
                    break
                resumed = self.resume_route(name, route, lines)
                if not resumed:
                    break
                self.pad(f'Hold:{side}:600', 'NumPad2')
                lines = self.walk_once(f'{name}-nudge', resumed, seconds, tolerance, extra)
                done = bool(lines and lines[-1].startswith('done '))
        if strict and not done:
            raise RuntimeError(f'{name} stopped before completing its route; no subsequent input sent')
        return done

    def walk_fighting(self, name, points, squads, seconds, chunk=10):
        """Walk `points` a few waypoints at a time, fighting `squads` after each stretch.

        With the Breach served by its triggers (--waves), each wave comes as the Guardian crosses its
        trigger, so one fight at the start of a leg meets only the first. Run sense105 fought the
        first Vandal, then walked the 64-waypoint chest leg through three more waves without
        fighting, died in the hall and could not find the route again from the respawn.
        """
        self.wave = squads
        for k, start in enumerate(range(0, len(points) - 1, chunk)):
            part = points[start:start + chunk + 1]
            me = self.where()
            if me and math.dist(me[:2], part[0][:2]) > 15:
                # A respawn (or a stretch that ended short): rejoin from the nearest waypoint so far.
                near = min(range(start + 1), key=lambda i: math.dist(points[i][:2], me[:2]))
                part = points[near:start + chunk + 1]
            share = max(90, seconds * len(part) // len(points))
            self.walk(f'{name}-{k}', self.write_route(f'{name}-{k}', part), share)
            self.skirmish(f'{name}-{k}')

    def skirmish(self, name, rounds=3):
        """Fight what is left of the last wave where a walk stalled.

        Run sense104 ended the junction wave with squad 76 still standing by the fan; the walk to
        the Divide then stopped against the fan rail under its fire and the nudges could not get
        past. So before resuming a stalled walk, fight any member of the last wave's squads that
        is within reach.
        """
        if not self.wave:
            return
        fought = False
        for round_ in range(rounds):
            lines = self.command(f'{name}-skirmish-target-{round_}', [sys.executable, '-I', str(LAB/'target.py'),
                                 self.wave], 30, check=False)
            if not any(l.count(',') == 1 for l in lines):
                break
            fought = True
            self.command(f'{name}-skirmish-{round_}', [sys.executable, '-u', '-I', str(LAB/'combat2.py'),
                         '--seconds', '30', '--squads', self.wave,
                         '--log', str(self.run/f'{name}-skirmish.jsonl')], 90, check=False)
            self.command(f'{name}-skirmish-release-{round_}', [sys.executable, '-I', str(LAB/'release.py')], 20, check=False)
        if fought:                         # as after a wave: the fight can move the tracked copies
            self.command('retrack', [sys.executable, '-u', '-I', str(HERE/'d1-walk.py'),
                                     '--tracked', str(self.tracked), '--retrack'], 60)
            self.select_retracked_position()

    def walk_once(self, name, route, seconds, tolerance, extra, check=True):
        tracking = (['--native-unit', '--memory-facing'] if self.native_unit else
                    ['--tracked', str(self.tracked)])
        return self.command(name, [sys.executable, '-u', '-I', str(HERE/'d1-walk.py'),
            *tracking, '--route', str(route), '--max-seconds', str(seconds),
            '--tolerance', str(tolerance), '--turn-sign', '1', '--turn-rate', '2.26', *extra], seconds + 45, check)

    def select_retracked_position(self):
        """A sparse heuristic retrack must not drive the fourteen-minute chest leg.

        The native walker validates the current unit and heading before any route
        input, and stops on unavailable data or unit change. Never recover from
        that failure by falling back to the sparse/stale heuristic copies.
        """
        copies = json.loads(self.tracked.read_text())
        if not isinstance(copies, list) or any(type(a) is not int or a <= 0 for a in copies):
            raise RuntimeError('Malformed retracking result; no route input sent')
        self.native_unit = len(copies) < 30
        if self.native_unit:
            print(f'Retrack kept {len(copies)} copies (<30): remaining walks require native unit/facing validation.', flush=True)

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
        # Once the Divide's step (16 in agr-steps-divide.txt) has served its list, the garage list
        # would replace it and the Divide's squads would never come. In runs sense102 and sense103
        # the fights pushed the route past that step's timeout, so it fired before the garage.
        if contains(self.host_log, DIVIDE_SLICE) or contains(self.host_log, DIVIDE_STEP):
            print('The Divide step has served its list; the garage list is out of date and not published.', flush=True)
            return
        self.publish(self.garage_clients, 'garage')
        time.sleep(6)

    def publish(self, clients, label):
        # The host opens the path itself: a Windows path (F:/...), never an MSYS one (/f/...).
        control = self.run.parent / 'ah-control.txt'
        offset = self.host_log.stat().st_size
        with control.open('x', encoding='utf-8') as output:
            output.write('clients 0x4786c0e0 ' + clients.as_posix() + '\n')
        deadline = time.monotonic() + 20
        while control.exists() and time.monotonic() < deadline:
            time.sleep(.5)
        if control.exists():
            raise RuntimeError(f'AH did not consume the {label} control; stopping')
        # Consumption is not acceptance. Require the host's answer as well: the frame served
        # ("... (N bytes)"), or dropped because a mission step installed a newer frame while it was
        # being built ("... superseded by a mission step"; the host builds lists off its tick).
        accepted = 'control: sensor clients ' + clients.as_posix() + ' '
        if not wait_for(self.host_log, accepted, 60, offset=offset):
            raise RuntimeError(f'AH did not log acceptance of the {label} list; stopping')

    def fight(self, route, clients, squads, tag, label, back=None):
        """Serve a wave (opt-in lists), fight it along `route`, and return the rest of the route.

        A list gives some squads a state with one desired member each, so they spawn (26 October run
        `fallen2`; traced through the spawn path in run sense94). The walk alone does not survive
        them. In run sense96 one came and was shot; the other three stood still on the route itself,
        and free seeking off the route lost the walk. So each round walks the route from the waypoint
        nearest the Guardian to the one nearest the target, then fights, until twice no member of
        those squads is within 400 m (run sense97: the Breach's four in 3.5 minutes). Gives up after
        the rounds and goes on: the route can still be walked.
        """
        if clients is not None:            # --waves: the schedule placed them
            self.publish(clients, label)
        self.wave = squads
        time.sleep(8)                      # the members appeared within seconds in sense94
        empty = 0
        for round_ in range(8):
            lines = self.command(f'{tag}-target-{round_}', [sys.executable, '-I', str(LAB/'target.py'),
                                 squads], 30, check=False)
            target = next((l for l in reversed(lines) if l.count(',') == 1), None)
            if target is None:
                empty += 1
                if empty == 2:             # twice nothing within 400 m: they are down
                    break
            else:
                empty = 0
                me, goal = self.where(), [float(v) for v in target.split(',')]
                if me:
                    a = min(range(len(route)), key=lambda k: math.dist(route[k][:2], me[:2]))
                    b = min(range(len(route)), key=lambda k: math.dist(route[k][:2], goal))
                    if b > a and math.dist(me[:2], goal) > 15:
                        self.walk_once(f'{tag}-approach-{round_}',
                                       self.write_route(f'{tag}-approach-{round_}', route[a:b + 1]),
                                       90, 1.5, (), check=False)
            self.command(f'{tag}-fight-{round_}', [sys.executable, '-u', '-I', str(LAB/'combat2.py'),
                         '--seconds', '30', '--squads', squads,
                         '--log', str(self.run/f'{tag}-combat.jsonl')], 90, check=False)
            self.command(f'{tag}-release-{round_}', [sys.executable, '-I', str(LAB/'release.py')], 20, check=False)
        else:
            if not empty:                  # only if the last round still found one
                print(f'{label} still standing after 8 rounds; continuing the route.', flush=True)
        self.command('retrack', [sys.executable, '-u', '-I', str(HERE/'d1-walk.py'),
                                 '--tracked', str(self.tracked), '--retrack'], 60)
        self.select_retracked_position()
        me = self.where()
        if me and back and min(math.dist(p[:2], me[:2]) for p in route) > 15:
            # A death respawns the Guardian at the last checkpoint (sense101: back in the tripmine
            # corridor). Walk the leg before the fight again, from its waypoint nearest the
            # Guardian, rather than straight at the route.
            b = min(range(len(back)), key=lambda k: math.dist(back[k][:2], me[:2]))
            self.walk_once(f'{tag}-back', self.write_route(f'{tag}-back', back[b:]), 330, 1.3, (), check=False)
            me = self.where()
        a = min(range(len(route)), key=lambda k: math.dist(route[k][:2], me[:2])) if me else 0
        return route[a:]

    def where(self):
        """The Guardian's position from memory (lab/tower-route/where.py), or None."""
        lines = self.command('where', [sys.executable, '-I', str(LAB/'where.py')], 30, check=False)
        try:
            return [float(v) for v in lines[-1].split(',')]
        except (IndexError, ValueError):
            return None

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
            if not wait_for((self.run/'launcher.log', self.run/'boot.log'), 'in gameplay (112)', boot_timeout):
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
        self.select_retracked_position()
        # The proven session chain is the existing Wall route followed by the
        # chest trail, omitting its first point at the shared junction.
        wall = json.loads((self.routes/'fight_through_the_wall.json').read_text())
        chest = json.loads((self.routes/'wall_container_to_chest.json').read_text())
        to_chest = wall + chest[1:]
        if self.waves:
            to_chest = self.fight(to_chest, None, WAVE_SQUADS[0], 'waves-hall', 'the hall waves')
        elif self.fallen:
            to_chest = self.fight(to_chest, self.fallen, BREACH_SQUADS, 'fallen', 'Breach Fallen')
        if self.waves:
            self.walk_fighting('chest', to_chest, WAVE_SQUADS[0], 840)
        else:
            self.walk('chest', self.write_route('to-chest', to_chest), 840)
        self.walk('mine-stairs', self.sliced_route('mine-a', 'wall_tripmine_corridor.json', 0, 4), 90, 1.2)
        self.pad('NumPad6')
        # If this fails, stop crouched. Do not blindly toggle stance in a retry.
        self.walk('mine-crouched', self.sliced_route('mine-b', 'wall_tripmine_corridor.json', 3, 10), 240, 1.5)
        self.pad('NumPad6')
        self.walk('mine-junction', self.sliced_route('mine-c', 'wall_tripmine_corridor.json', 9, None), 150, 1.2)
        if self.waves:                     # the corridor's waves (114-116) came during the mine legs
            self.wave = WAVE_SQUADS[1]
            self.skirmish('mine-end')
        junction = self.sliced_route('junction', 'wall_junction_west.json', 4, None)
        self.walk('junction-west', junction, 330)
        garage = self.routes/'wall_garage_door.json'
        if self.waves:
            garage = self.write_route('garage-rest', self.fight(json.loads(garage.read_text()), None,
                                      WAVE_SQUADS[1], 'waves-garage', 'the corridor and garage waves',
                                      back=json.loads(junction.read_text())))
        elif self.junction:
            garage = self.write_route('garage-rest', self.fight(json.loads(garage.read_text()), self.junction,
                                      JUNCTION_SQUADS, 'junction', 'junction Fallen',
                                      back=json.loads(junction.read_text())))
        if not self.waves:                 # the wave lists carry the open garage door
            self.publish_garage()
        self.walk('garage', garage, 150, 1.0)
        self.walk('fan-to-divide', self.routes/'wall_fan_tunnel_to_divide.json', 420)
        if not wait_for(self.host_log, DIVIDE_SLICE, 45):
            raise RuntimeError('No natural slice-96 crossing observed; no Divide control published')
        print('DIVIDE REACHED: verify scheduled step 16, native authority, objective and scene.', flush=True)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--run-dir', type=Path, required=True)
    ap.add_argument('--host-log', type=Path, required=True)
    ap.add_argument('--garage-clients', type=Path, required=True,
                    help='Private list activating only Breach prop 3')
    ap.add_argument('--pad', type=Path, help='pad driver (default: D1_TOOLS/d1-pad.ps1)')
    ap.add_argument('--boot-timeout', type=int, default=900)
    ap.add_argument('--resume-after-rifle', action='store_true',
                    help='Explicit supervised resume; refresh tracking and verify stance first')
    ap.add_argument('--breach-fallen', type=Path,
                    help='after the rifle, publish this list (Breach squad states) and fight the Fallen')
    ap.add_argument('--junction-fallen', type=Path,
                    help='after the junction, publish this list (squads 64, 68, 76) and fight them on the way to the garage')
    ap.add_argument('--waves', action='store_true',
                    help='the schedule serves the Breach by its triggers (agr-steps-divide-waves.txt): publish nothing, fight the waves')
    a = ap.parse_args()
    if a.waves and (a.breach_fallen or a.junction_fallen):
        ap.error('--waves replaces --breach-fallen and --junction-fallen')
    if a.pad is None:
        a.pad = d1paths.tools() / 'd1-pad.ps1'
    if a.boot_timeout <= 0:
        ap.error('--boot-timeout must be positive')
    if not a.run_dir.is_dir() or not a.garage_clients.is_file() or not a.pad.is_file():
        ap.error('Run directory, private garage list and existing pad driver must exist')
    for opt, value in (('--breach-fallen', a.breach_fallen), ('--junction-fallen', a.junction_fallen)):
        if value is not None and not value.is_file():
            ap.error(f'{opt} must name an existing list')
    if not a.host_log.parent.name.endswith('-' + a.run_dir.name):
        ap.error('--host-log must belong to the selected contained run')
    lock = a.run_dir/'replay.lock'
    # Exclusive create refuses concurrent replay owners. On abnormal termination,
    # inspect processes and the scene before removing a stale lock manually.
    with lock.open('x') as output:
        output.write('Pad replay owns input; do not run another movement driver.\n')
    try:
        Replay(a.run_dir, a.host_log, a.garage_clients, a.pad, fallen=a.breach_fallen,
               junction=a.junction_fallen, waves=a.waves).play(a.boot_timeout, a.resume_after_rifle)
    finally:
        lock.unlink()


if __name__ == '__main__':
    main()
