"""Find the best fight moments of a run in its recording, for clips.

    python -I fight-moments.py RUN_DIR --start EPOCH [--length 45] [--count 3] [--end EPOCH]

Reads the combat logs the replay writes (RUN_DIR/*combat.jsonl, *skirmish.jsonl: one line per lock
with the target's health) and scores each second by what happened in it:
- damage dealt: the locked target's health falling, scaled by how far;
- a kill: a target that took damage and is never locked again (health readings stop before a death,
  so the last one seen is often well above zero);
- shooting: a second with the crosshair on a locked target. Members placed by their own state (the
  Breach's entrances) report no health, so for them a kill is a target aimed at three times or more
  and never locked again.

It prints the best non-overlapping windows of --length seconds as offsets into a recording that
began at --start (epoch seconds): `start_s length_s kills damage`. The production side cuts Shorts
from these, so the report names each moment with its start time.
"""
import argparse
import glob
import json
import os
import re

LOCK = re.compile(r"lock \('sq', (\d+), (\d+), (\d+)\) d [\d.]+ aimed (\w+) plates \d+ health ([\d.]+|None)")


def events(run_dir):
    """[(t, unit, health, aimed)] for every lock, in time order; health is None when unknown."""
    out = []
    for path in glob.glob(os.path.join(run_dir, '*combat.jsonl')) + glob.glob(os.path.join(run_dir, '*skirmish.jsonl')):
        for line in open(path, encoding='utf-8', errors='replace'):
            try:
                rec = json.loads(line)
            except ValueError:
                continue
            m = LOCK.search(rec.get('msg', ''))
            if m:
                health = None if m.group(5) == 'None' else float(m.group(5))
                out.append((rec['t'], (int(m.group(1)), int(m.group(2)), int(m.group(3))), health,
                            m.group(4) == 'True'))
    return sorted(out)


def score(evts):
    """{second: (kills, damage)} from the lock stream; aimed seconds count 0.05 damage each."""
    per, last, first, aims = {}, {}, {}, {}
    final = {unit: k for k, (_, unit, _, _) in enumerate(evts)}
    for k, (t, unit, health, aimed) in enumerate(evts):
        s = int(t)
        kills, dmg = per.get(s, (0, 0.0))
        if aimed:
            dmg += 0.05
            aims[unit] = aims.get(unit, 0) + 1
        if health is not None:
            first.setdefault(unit, health)
            if unit in last and health < last[unit]:
                dmg += last[unit] - health
            last[unit] = health
            if final[unit] == k and health < first[unit] - 0.1:
                kills += 1
        elif final[unit] == k and aims.get(unit, 0) >= 3:
            kills += 1
        per[s] = (kills, dmg)
    return per


def windows(per, length, count, lo, hi):
    """The best non-overlapping windows inside [lo, hi): [(start, kills, damage)]."""
    picks = []
    starts = [s for s in range(int(lo), int(hi) - length)] if hi > lo + length else []
    value = lambda s: sum(3 * per.get(x, (0, 0))[0] + per.get(x, (0, 0))[1] for x in range(s, s + length))
    ranked = sorted(starts, key=value, reverse=True)
    for s in ranked:
        if len(picks) == count or value(s) <= 0:
            break
        if all(abs(s - p) >= length for p, _, _ in picks):
            k = sum(per.get(x, (0, 0))[0] for x in range(s, s + length))
            d = sum(per.get(x, (0, 0))[1] for x in range(s, s + length))
            picks.append((s, k, d))
    return sorted(picks)


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('run_dir')
    ap.add_argument('--start', type=float, required=True, help='epoch seconds when the recording began')
    ap.add_argument('--end', type=float, help='epoch seconds when it stopped (default: the last event)')
    ap.add_argument('--length', type=int, default=45)
    ap.add_argument('--count', type=int, default=3)
    a = ap.parse_args()
    evts = events(a.run_dir)
    per = score(evts)
    hi = a.end or (evts[-1][0] if evts else a.start)
    for s, k, d in windows(per, a.length, a.count, a.start, hi):
        print(f'{s - a.start:.0f} {a.length} kills={k} damage={d:.2f}')


if __name__ == '__main__':
    main()
