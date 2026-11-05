# SPDX-License-Identifier: GPL-2.0-or-later
"""Play a cinematic activity's records the way the live host did: d1-cine-watch.py RUN

    d1-cine-watch.py RUN                                    (cine_reveal, activity 165)
    d1-cine-watch.py RUN --activity 167 --slice 1 --states cine_speaker-play.txt cine_speaker-done.txt

For a run served with a world table that serves the activity's cinematic records idle
(ps4/fixtures/56578/mission/world-table-cine-live.txt for 165, world-table-speaker-live.txt adds
The Speaker, 167; notes/d1-live-captures-2026-11-05.md). Follows the run's activity-host log; once
the client is in the activity and reports its start slice set, it serves the first state through
the host's control file (`clients GROUP PATH`), then on each "incident 891 reported by the client"
(a cinematic stopped, 56578) the next one. The defaults are cine_reveal's: slice set 25, then
play-0, play-1, done.
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
STATES = ['cine_reveal-play-0.txt', 'cine_reveal-play-1.txt', 'cine_reveal-done.txt']


def log(msg):
    print(time.strftime('%H:%M:%S'), msg, flush=True)


def due(stage, line, slice_set=25):
    """Whether this host-log line is the cue for the state at `stage`: the start slice set for the
    first, then each cinematic the client reports stopped."""
    if stage == 0:
        return f'client is now in slice set {slice_set} ' in line
    return 'incident 891 reported by the client' in line


def joined(line, activity):
    """Whether this host-log line is the host switching to the activity's world."""
    return f'activity index {activity} -> world' in line


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('run')
    ap.add_argument('--control', default=os.path.join(str(d1paths.staging()), 'ah-control.txt'),
                    help="the activity host's D1_AH_CONTROL_FILE (d1-agr-run.ps1's default)")
    ap.add_argument('--activity', type=int, default=165, help='the activity index (default 165)')
    ap.add_argument('--slice', type=int, default=25, help='its start slice set (default 25)')
    ap.add_argument('--group', default=GROUP, help=f'the sensor group to replace (default {GROUP})')
    ap.add_argument('--states', nargs='+', default=STATES, help='the lists to serve, in order')
    a = ap.parse_args()
    staging = str(d1paths.staging())
    while True:
        d = sorted(glob.glob(os.path.join(staging, 'contained-boot', f'*-{a.run}')))
        if d and os.path.exists(os.path.join(d[-1], 'activity-host.log')):
            path = os.path.join(d[-1], 'activity-host.log')
            break
        time.sleep(5)
    log(f'host log {path}')
    pos, inside, stage = 0, False, 0
    while stage < len(a.states):
        with open(path, encoding='utf-8', errors='replace') as f:
            f.seek(pos)
            new = f.read()
            pos = f.tell()
        for line in new.splitlines():
            if not inside:
                if joined(line, a.activity):
                    inside = True
                    log(line.strip()[:160])
                continue
            if stage < len(a.states) and due(stage, line, a.slice):
                log(line.strip()[:160])
                with open(a.control, 'a', encoding='utf-8') as c:
                    c.write(f'clients {a.group} {MISSION / a.states[stage]}\n')
                log(f'control: clients {a.group} {a.states[stage]}')
                stage += 1
            elif 'slice set' in line or 'incident' in line:
                log(line.strip()[:160])
        time.sleep(1)
    log('all cinematic states served')


if __name__ == '__main__':
    main()
