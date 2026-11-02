"""Serve a stalled world its own sensor clients, read from the client, without a restart.

    python -I serve_pending.py OUT_LIST [--world N] [--group 0xHASH] [--dry]

A world served another world's sensor list loads but sits on the loading screen (world state 34)
and never answers the host's udt-5 frames: the Tower did with the Highway's list (sense56), and
cine_reveal did at its first try (sense124). The cure then was the world's own pending list. This
reads it from the resident world (`d1-winguest.py sensor-clients --all --bundle`), writes OUT_LIST,
and hands it to the running activity host through its control file (`clients GROUP PATH`, the line
d1-agr-replay.py and postgame5.sh use). GROUP defaults to the group holding the lifetime record
(kind 0x10), as the Tower's 0x40aebf90 does. Read-only on the client.
"""
import argparse
import collections
import os
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
TOOLS = os.path.join(HERE, '..', '..', 'tools', 'ps4_setup')


def staging():
    return subprocess.run([sys.executable, os.path.join(TOOLS, 'd1paths.py'), 'staging'],
                          capture_output=True, text=True, check=True).stdout.strip()


def lifetime_group(lines):
    """The group of the kind-0x10 (lifetime) row, else the group with the most rows."""
    rows = [l.split() for l in lines if l.strip() and not l.startswith('#')]
    for r in rows:
        if len(r) >= 2 and int(r[1], 16) == 0x10:
            return r[0]
    counts = collections.Counter(r[0] for r in rows)
    return counts.most_common(1)[0][0] if counts else None


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('out')
    ap.add_argument('--world', default='0')
    ap.add_argument('--group')
    ap.add_argument('--dry', action='store_true', help='read and write the list, serve nothing')
    ap.add_argument('--timeout', type=int, default=60)
    a = ap.parse_args()
    out = os.path.abspath(a.out)
    r = subprocess.run([sys.executable, '-I', os.path.join(TOOLS, 'd1-winguest.py'), 'sensor-clients', '--all', '--bundle',
                        '--world', a.world, '--out', out], capture_output=True, text=True)
    print(r.stderr.strip().splitlines()[-1] if r.stderr.strip() else '(no count)')
    if r.returncode or not os.path.exists(out):
        sys.exit(f'sensor-clients failed: {r.stdout[-300:]} {r.stderr[-300:]}')
    lines = open(out, encoding='utf-8').read().splitlines()
    group = a.group or lifetime_group(lines)
    if not group:
        sys.exit('the list is empty: is the world resident yet?')
    print(f'{len(lines) - 1} clients -> {out}; group {group}')
    if a.dry:
        return
    control = os.path.join(staging(), 'ah-control.txt')
    with open(control, 'w', encoding='utf-8') as f:
        f.write(f'clients {group} {out.replace(os.sep, "/")}\n')
    deadline = time.time() + a.timeout
    while os.path.exists(control):
        if time.time() > deadline:
            sys.exit(f'the activity host did not take {control} (is D1_AH_CONTROL_FILE set?)')
        time.sleep(1)
    print('served: the host took the control line')


if __name__ == '__main__':
    main()
