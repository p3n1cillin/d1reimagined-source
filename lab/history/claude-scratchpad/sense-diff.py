"""Bit-level diff of captured udt 6 (sensor_sense_update) payloads from an activity-host log.

    sense-diff.py HOST_LOG [--udt 6]
Prints each payload's time and length, then for consecutive payloads of equal length the changed
bit ranges (MSB-first bit offsets, the order the client's bitstreams use), and finally the bits
that change most often across the whole capture.
"""
import argparse, collections, re

LINE = re.compile(r'^(\S+)Z .*CAPTURED udt (\d+) \([^)]*\) payload(?: \(TRUNCATED\))?: ([0-9a-f]+)')


def bits(hexs):
    return ''.join(f'{int(hexs[i:i + 2], 16):08b}' for i in range(0, len(hexs), 2))


def ranges(diff):
    out, start = [], None
    for i, d in enumerate(diff + [False]):
        if d and start is None:
            start = i
        elif not d and start is not None:
            out.append((start, i)); start = None
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('log')
    ap.add_argument('--udt', type=int, default=6)
    a = ap.parse_args()
    rows = []
    for raw in open(a.log, encoding='utf-8', errors='replace'):
        line = re.sub(r'\x1b\[[0-9;]*m', '', raw)
        m = LINE.match(line)
        if m and int(m[2]) == a.udt:
            rows.append((m[1][11:23], m[3]))
    print(f'{len(rows)} udt {a.udt} payloads')
    counts = collections.Counter()
    prev = None
    for t, h in rows:
        b = bits(h)
        note = ''
        if prev and len(prev) == len(b):
            diff = [x != y for x, y in zip(prev, b)]
            for i, d in enumerate(diff):
                if d:
                    counts[i] += 1
            note = ' changed bits ' + ', '.join(f'{s}..{e}' for s, e in ranges(diff))
        elif prev:
            note = f' (length {len(prev) // 8} -> {len(b) // 8} bytes)'
        print(f'{t} {len(h) // 2:4d} B {h[:48]}{"..." if len(h) > 48 else ""}{note}')
        prev = b
    if counts:
        print('most-changed bits:', ', '.join(f'{i}x{n}' for i, n in sorted(counts.items(), key=lambda kv: -kv[1])[:24]))


if __name__ == '__main__':
    main()
