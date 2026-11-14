# SPDX-License-Identifier: GPL-2.0-or-later
"""Record a walkable route from the Guardian's world position (read-only guest memory, no input sent).

While a person plays (or the bot walks), sample the player unit's position and write a JSON list of
[x, y, z] waypoints that `d1-walk.py --route` accepts.

  d1-route-record.py --out route.json [--interval 0.25] [--min-step 1.5] [--seconds N]
                     [--stop-file PATH] [--split-on-gap 8.0] [--raw raw.jsonl]

A point is kept when it is at least --min-step metres (3-D) from the last kept one; the first and last
valid samples are always kept. A jump between consecutive valid samples above --split-on-gap metres
(teleport, respawn) starts a new segment: the first goes to --out, the rest to <stem>-2.json,
<stem>-3.json, ... Samples are buffered and written every few seconds and at exit (the lab disk is slow).
"""
import argparse, importlib.util, json, math, os, subprocess, sys, time

HERE = os.path.dirname(os.path.abspath(__file__))
FLUSH_S = 5.0


def thin(points, min_step):
    """Keep a point when it is >= min_step (3-D) from the last kept point; always keep the first and last."""
    kept = []
    for p in points:
        if not kept or math.dist(kept[-1], p) >= min_step:
            kept.append(tuple(p))
    if points and kept[-1] != tuple(points[-1]):
        kept.append(tuple(points[-1]))
    return kept


def segments(samples, gap):
    """Split samples (points, or None for a missed read) into runs; None is skipped and does not split.
    A jump above gap metres between consecutive valid samples starts a new run."""
    runs, last = [], None
    for s in samples:
        if s is None:
            continue
        s = tuple(s)
        if last is None or math.dist(last, s) > gap:
            runs.append([])
        runs[-1].append(s)
        last = s
    return runs


def output_paths(out, count):
    stem, ext = os.path.splitext(out)
    return [out] + [f'{stem}-{i}{ext}' for i in range(2, count + 1)]


def write_routes(out, samples, min_step, gap):
    """Write one file per segment; returns [(path, waypoints)]."""
    runs = [thin(r, min_step) for r in segments(samples, gap)]
    paths = output_paths(out, len(runs))
    for path, run in zip(paths, runs):
        # Temp file, then replace: a stop or kill mid-write never leaves a truncated route.
        tmp = path + '.tmp'
        with open(tmp, 'w') as f:
            json.dump([[round(v, 3) for v in p] for p in run], f)
        os.replace(tmp, path)
    # A segment file left by an earlier, longer recording is not this run's.
    stem, ext = os.path.splitext(out)
    i = len(runs) + 1
    while os.path.exists(f'{stem}-{i}{ext}'):
        os.remove(f'{stem}-{i}{ext}')
        i += 1
    return list(zip(paths, runs))


def _load(name, filename):
    spec = importlib.util.spec_from_file_location(name, os.path.join(HERE, filename))
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


def _require_shadps4():
    out = subprocess.run(['tasklist', '/FI', 'IMAGENAME eq shadPS4.exe', '/FO', 'CSV', '/NH'],
                         capture_output=True, text=True).stdout
    if not any(line.startswith('"shadPS4') for line in out.splitlines()):
        sys.exit('no shadPS4 process found: start the emulator and the game first')


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--out', required=True, help='route file to write (JSON list of [x, y, z])')
    ap.add_argument('--interval', type=float, default=0.25, help='seconds between samples')
    ap.add_argument('--min-step', type=float, default=1.5, help='metres between kept points')
    ap.add_argument('--seconds', type=float, help='stop after this many seconds')
    ap.add_argument('--stop-file', help='stop when this file appears')
    ap.add_argument('--split-on-gap', type=float, default=8.0, help='metres; a larger jump starts a new segment')
    ap.add_argument('--raw', help='append every raw sample {t, x, y, z} to this JSONL file')
    a = ap.parse_args()
    _require_shadps4()
    ut = _load('d1_unit_tracking', 'd1_unit_tracking.py')
    wg = _load('d1_winguest', 'd1-winguest.py')
    reader, base = wg.open_client()
    samples, raw_buf, missed = [], [], 0
    t0 = time.time()
    next_flush = t0 + FLUSH_S

    def flush():
        if a.raw and raw_buf:
            chunk = ''.join(raw_buf)
            raw_buf.clear()             # cleared first: an interrupted append is not written twice
            with open(a.raw, 'a') as f:
                f.write(chunk)
        return write_routes(a.out, samples, a.min_step, a.split_on_gap) if samples else []

    print('recording; Ctrl+C, --seconds or --stop-file to stop')
    try:
        while True:
            now = time.time()
            if a.seconds is not None and now - t0 >= a.seconds:
                break
            if a.stop_file and os.path.exists(a.stop_file):
                break
            s = ut.read_unit_position(reader, base)
            pos = tuple(float(v) for v in s.position) if s is not None and s.position is not None else None
            if pos is None:
                missed += 1
            else:
                samples.append(pos)
                if a.raw:
                    raw_buf.append(json.dumps({'t': round(now - t0, 3), 'x': pos[0], 'y': pos[1], 'z': pos[2]}) + '\n')
            if now >= next_flush:
                flush()
                next_flush = now + FLUSH_S
            time.sleep(max(0.0, a.interval))
    except KeyboardInterrupt:
        print('interrupted')
    finally:
        written = flush()
    for path, run in written:
        print(f'wrote {path}: {len(run)} waypoints')
    if not written:
        print('no valid samples; nothing written')
    print(f'{len(samples)} samples, {missed} missed (None) reads')
    return 0 if written else 1


if __name__ == '__main__':
    sys.exit(main())
