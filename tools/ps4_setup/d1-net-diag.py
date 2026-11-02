"""Read the emulator's D1_NET_DIAG socket summaries and flag where the client stopped reading.

    d1-net-diag.py [SHAD_LOG] [--sock N] [--all]

The project's shadPS4 fork (branch d1-net-diag) logs, with D1_NET_DIAG=1, one line per socket every
2 s: receive calls, bytes, receives that found nothing, sends, and the bytes Windows holds unread.
Destiny dropped the activity-host session in the Breach waves (runs sense125-130): the client
stopped reading that socket for about 20 s while it kept sending. This prints each socket's
totals and the intervals that look like that stall:
- NO-RECV: sends but no receive calls while bytes are waiting (the game stopped asking);
- STARVED: receive calls that all found nothing while bytes are waiting (the emulator's fault).
"""
import argparse
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import d1paths  # sibling: local paths from D1_ROOT or paths.json

LINE = re.compile(r'D1_NET_DIAG sock (\d+): (\d+) ms: recv (\d+) calls (\d+) B (\d+) empty; '
                  r'send (\d+) calls (\d+) B; waiting (\d+) B')


def parse(lines):
    """[(order, sock, ms, recv_calls, recv_bytes, recv_empty, send_calls, send_bytes, waiting)]"""
    out = []
    for i, line in enumerate(lines):
        m = LINE.search(line)
        if m:
            out.append((i, *map(int, m.groups())))
    return out


def verdict(row):
    _, _, _, rc, _, re_, sc, _, w = row
    if w and sc and not rc:
        return 'NO-RECV'
    if w and rc and rc == re_:
        return 'STARVED'
    return ''


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('log', nargs='?', default=str(d1paths.shadps4() / 'user' / 'log' / 'shad_log.txt'))
    ap.add_argument('--sock', type=int)
    ap.add_argument('--all', action='store_true', help='print every interval, not only the flagged ones')
    a = ap.parse_args()
    rows = parse(open(a.log, encoding='utf-8', errors='replace'))
    if a.sock is not None:
        rows = [r for r in rows if r[1] == a.sock]
    totals = {}
    for r in rows:
        t = totals.setdefault(r[1], [0, 0, 0, 0, 0, 0])
        for k, v in enumerate((r[2], r[3], r[4], r[6], r[7], r[8])):
            t[k] = max(t[k], v) if k == 5 else t[k] + v
    for s, (ms, rc, rb, sc, sb, w) in sorted(totals.items(), key=lambda kv: -kv[1][2]):
        print(f'sock {s}: {ms / 1000:.0f} s logged, recv {rc} calls {rb} B, send {sc} calls {sb} B, most waiting {w} B')
    flagged = 0
    for r in rows:
        v = verdict(r)
        if v or a.all:
            flagged += bool(v)
            print(f'  line {r[0]}: sock {r[1]} {r[2]} ms recv {r[3]}/{r[4]} B/{r[5]} empty send {r[6]}/{r[7]} B '
                  f'waiting {r[8]} B {v}')
    print(f'{flagged} flagged intervals')


if __name__ == '__main__':
    main()
