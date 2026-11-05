# SPDX-License-Identifier: GPL-2.0-or-later
"""Play cine_reveal's two cinematics the way the live host did: d1-cine-watch.py RUN

For a run served with ps4/fixtures/56578/mission/world-table-cine-live.txt (activity 165 with its
two cinematic records and start region 25; notes/d1-live-captures-2026-11-05.md). Follows the run's
activity-host log; once the client is in 165 and reports slice set 25 it serves
cine_reveal-play-0.txt through the host's control file (`clients GROUP PATH`), then on each
"incident 891 reported by the client" (a cinematic stopped, 56578) the next state: play-1, done.
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


def due(stage, line):
    """Whether this host-log line is the cue for STATES[stage]."""
    if stage == 0:
        return 'client is now in slice set 25' in line
    return 'incident 891 reported by the client' in line


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('run')
    ap.add_argument('--control', default=os.path.join(str(d1paths.staging()), 'ah-control.txt'),
                    help="the activity host's D1_AH_CONTROL_FILE (d1-agr-run.ps1's default)")
    a = ap.parse_args()
    staging = str(d1paths.staging())
    while True:
        d = sorted(glob.glob(os.path.join(staging, 'contained-boot', f'*-{a.run}')))
        if d and os.path.exists(os.path.join(d[-1], 'activity-host.log')):
            path = os.path.join(d[-1], 'activity-host.log')
            break
        time.sleep(5)
    log(f'host log {path}')
    pos, joined, stage = 0, False, 0
    while stage < len(STATES):
        with open(path, encoding='utf-8', errors='replace') as f:
            f.seek(pos)
            new = f.read()
            pos = f.tell()
        for line in new.splitlines():
            if not joined:
                if 'activity index 165 -> world' in line:
                    joined = True
                    log(line.strip()[:160])
                continue
            if stage < len(STATES) and due(stage, line):
                log(line.strip()[:160])
                with open(a.control, 'a', encoding='utf-8') as c:
                    c.write(f'clients {GROUP} {MISSION / STATES[stage]}\n')
                log(f'control: clients {GROUP} {STATES[stage]}')
                stage += 1
            elif 'slice set' in line or 'incident' in line:
                log(line.strip()[:160])
        time.sleep(1)
    log('all cinematic states served')


if __name__ == '__main__':
    main()
