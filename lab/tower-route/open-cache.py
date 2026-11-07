# SPDX-License-Identifier: GPL-2.0-or-later
"""Open A Guardian Rises' loot cache (Fight through the Wall) by pad input, as a player does.

At the end of the replay's chest leg the cache is close by, but the bot never interacted with it
(F49, 7 November). A player holds Square at the cache. This turns the Guardian in eight 45-degree
steps, looking slightly down, and holds Square (the lab's numpad 4) for 1.6 s at each, until the
activity host logs the client's report that the cache opened: `incident 1131 reported by the
client` (`tutorial_weapon_special`, with `gather_material` 1851), the report the live capture's
client sent at 684.5 s.

    python open-cache.py --host-log <activity-host.log>

Exit 0 once the report is seen, 1 if not after a full turn. Holding Square away from the cache only
reloads the rifle.
"""
import argparse
import importlib.util
import math
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.argv, _argv = [sys.argv[0]], sys.argv
_spec = importlib.util.spec_from_file_location('combat2', os.path.join(HERE, 'combat2.py'))
c2 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(c2)
sys.argv = _argv

REPORT = 'incident 1131 reported by the client'


def reported(log, offset):
    try:
        with open(log, 'rb') as f:
            f.seek(offset)
            return REPORT.encode() in f.read()
    except OSError:
        return False


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--host-log', required=True)
    ap.add_argument('--steps', type=int, default=8)
    ap.add_argument('--hold-ms', type=int, default=1600)
    a = ap.parse_args()
    start = 0
    if reported(a.host_log, 0):
        print('cache already opened (report in the host log)', flush=True)
        return 0
    start = os.path.getsize(a.host_log)
    c2.release_all()
    c2.hold('K', int(math.radians(12) / c2.PITCH_RATE))     # look a little down: the cache sits low
    step_ms = int(math.radians(360 / a.steps) / c2.YAW_RATE)
    for k in range(a.steps):
        c2.hold('RELOAD', a.hold_ms)
        time.sleep(0.6)
        if reported(a.host_log, start):
            print(f'cache opened at step {k} (incident 1131 reported)', flush=True)
            c2.hold('I', int(math.radians(12) / c2.PITCH_RATE))
            return 0
        c2.hold('L', step_ms)
    c2.hold('I', int(math.radians(12) / c2.PITCH_RATE))
    print(f'no cache report after {a.steps} steps', flush=True)
    return 1


if __name__ == '__main__':
    sys.exit(main())
