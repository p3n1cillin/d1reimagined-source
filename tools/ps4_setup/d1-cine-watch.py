# SPDX-License-Identifier: GPL-2.0-or-later
"""Play a cinematic activity's records the way the live host did: d1-cine-watch.py RUN

    d1-cine-watch.py RUN                                    (cine_reveal, activity 165)
    d1-cine-watch.py RUN --activity 167 --slice 1 \
        --states cine_speaker-phase-1.txt cine_speaker-play.txt+1 cine_speaker-done.txt

For a run served with a world table that serves the activity's cinematic records idle
(ps4/fixtures/56578/mission/world-table-cine-live.txt for 165, world-table-speaker-live.txt adds
The Speaker, 167; notes/d1-live-captures-2026-11-05.md). Follows the run's activity-host log; once
the client is in the activity and reports its start slice set (or `--start-after` seconds after
the join, whichever comes first: in run cine-live8 the host never logged the client's slice set in
165), it serves the first state through the host's control file (`clients GROUP PATH`), then on
each "incident 891 reported by the client" (a cinematic stopped, 56578) the next one. A state
written `FILE+SECONDS` is served that many seconds after the previous one instead.

The live hosts stepped the world's lifetime phase 0 -> 1 -> 4 and played each cinematic at phase 4,
then 6 and 8 at the end (notes/d1-live-speaker-timeline-2026-11-05.md); the lists carry it as
`@phase`. The defaults are cine_reveal's: slice set 25, then phase 1, play-0 one second later, and
done.
"""
import argparse
import glob
import os
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import d1paths  # noqa: E402

MISSION = HERE.parents[1] / 'ps4' / 'fixtures' / '56578' / 'mission'
GROUP = '0x4786c0e0'
STATES = ['cine_reveal-phase-1.txt', 'cine_reveal-play-0.txt+1', 'cine_reveal-done.txt']


def log(msg):
    print(time.strftime('%H:%M:%S'), msg, flush=True)


def due(stage, line, slice_set=25):
    """Whether this host-log line is the cue for the state at `stage`: the start slice set for the
    first, then each cinematic the client reports stopped."""
    if stage == 0:
        return f'client is now in slice set {slice_set} ' in line
    return 'incident 891 reported by the client' in line


def state(spec):
    """`FILE` (served on its cue) or `FILE+SECONDS` (served that long after the previous state)."""
    name, plus, after = spec.rpartition('+')
    if plus and name and after.replace('.', '', 1).isdigit():
        return name, float(after)
    return spec, None


def joined(line, activity):
    """Whether this host-log line is the host switching to the activity's world."""
    return f'activity index {activity} -> world' in line


def main():
    # The host's log lines carry emoji; a redirected Windows console would otherwise be cp1252
    # and the first one ends the watcher (run cine-live8).
    sys.stdout.reconfigure(encoding='utf-8', errors='replace')
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('run')
    ap.add_argument('--control', default=os.path.join(str(d1paths.staging()), 'ah-control.txt'),
                    help="the activity host's D1_AH_CONTROL_FILE (d1-agr-run.ps1's default)")
    ap.add_argument('--activity', type=int, default=165, help='the activity index (default 165)')
    ap.add_argument('--slice', type=int, default=25, help='its start slice set (default 25)')
    ap.add_argument('--group', default=GROUP, help=f'the sensor group to replace (default {GROUP})')
    ap.add_argument('--states', nargs='+', default=STATES, help='the lists to serve, in order')
    ap.add_argument('--start-after', type=float, default=15.0,
                    help='serve the first state this many seconds after the join if the slice-set cue '
                         'has not come (default 15; live, the first cinematic played 12.7 s in)')
    a = ap.parse_args()
    staging = str(d1paths.staging())
    while True:
        d = sorted(glob.glob(os.path.join(staging, 'contained-boot', f'*-{a.run}')))
        if d and os.path.exists(os.path.join(d[-1], 'activity-host.log')):
            path = os.path.join(d[-1], 'activity-host.log')
            break
        time.sleep(5)
    log(f'host log {path}')

    states = [state(s) for s in a.states]
    served_at = None

    def serve(stage):
        nonlocal served_at
        name = states[stage][0]
        with open(a.control, 'a', encoding='utf-8') as c:
            c.write(f'clients {a.group} {MISSION / name}\n')
        log(f'control: clients {a.group} {name}')
        served_at = time.monotonic()

    pos, joined_at, stage = 0, None, 0
    while stage < len(a.states):
        with open(path, encoding='utf-8', errors='replace') as f:
            f.seek(pos)
            new = f.read()
            pos = f.tell()
        for line in new.splitlines():
            if joined_at is None:
                if joined(line, a.activity):
                    joined_at = time.monotonic()
                    log(line.strip()[:160])
                continue
            if stage < len(a.states) and states[stage][1] is None and due(stage, line, a.slice):
                log(line.strip()[:160])
                serve(stage)
                stage += 1
            elif 'slice set' in line or 'incident' in line:
                log(line.strip()[:160])
        if stage == 0 and joined_at is not None and time.monotonic() - joined_at >= a.start_after:
            log(f'no slice-set cue {a.start_after:.0f} s after the join: serving the first state')
            serve(stage)
            stage += 1
        while (0 < stage < len(a.states) and states[stage][1] is not None
               and time.monotonic() - served_at >= states[stage][1]):
            serve(stage)
            stage += 1
        time.sleep(0.2)
    log('all cinematic states served')


if __name__ == '__main__':
    main()
