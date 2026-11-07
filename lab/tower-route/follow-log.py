"""Follow a run's log for matching lines, then exit when the run ends.

    follow-log.py RUN_DIR [--log boot/activity-host.log] [--pattern REGEX] [--minutes N]

Prints each new line that matches PATTERN (trimmed to 220 characters) and exits when RUN_DIR/boot
gains servers-stopped.txt, the log is gone, or N minutes pass. It polls with Python instead of
`tail -F`: on Windows a stopped Monitor left its `tail -F` running, and by 7 November 2026 there were
79 of them, each re-opening files on the F: hard disk every second.
"""
import argparse
import re
import sys
import time
from pathlib import Path

ap = argparse.ArgumentParser()
ap.add_argument('run')
ap.add_argument('--log', default='boot/activity-host.log')
ap.add_argument('--pattern', default=r'mission step|incident (2681|891|1726)|world table: activity|teleport|ended|left|panic|ERROR')
ap.add_argument('--minutes', type=float, default=40)
ap.add_argument('--every', type=float, default=2.0, help='seconds between polls')
a = ap.parse_args()
# The host's lines carry emoji markers; a Windows console's cp1252 cannot print them.
sys.stdout.reconfigure(encoding='utf-8', errors='replace')

run = Path(a.run)
log = run / a.log
pat = re.compile(a.pattern)
stop = run / 'boot' / 'servers-stopped.txt'
deadline = time.time() + a.minutes * 60
pos = 0
while time.time() < deadline:
    if log.exists():
        with open(log, 'rb') as f:
            f.seek(pos)
            chunk = f.read()
        # Only whole lines; keep a partial last line for the next poll.
        cut = chunk.rfind(b'\n') + 1
        pos += cut
        for line in chunk[:cut].decode('utf-8', 'replace').splitlines():
            if pat.search(line):
                print(line[11:231] if line[:4].isdigit() else line[:220], flush=True)
    if stop.exists():
        print(f'run ended: {stop.read_text().strip()[:200]}', flush=True)
        sys.exit(0)
    time.sleep(a.every)
print('follow-log: time limit reached', flush=True)
