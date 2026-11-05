# SPDX-License-Identifier: GPL-2.0-or-later
"""Summarise or decode the activity-host traffic in SRCP message captures (d1_srcp.py).

    d1-srcp.py summary CAPTURE.pcap
    d1-srcp.py incidents CAPTURE.pcap [--dir s2c|c2s]   (udt 18, decoded by d1-oracle)
    d1-srcp.py sensors CAPTURE.pcap                     (udt 5 sensor_auth_update frames)
    d1-srcp.py export CAPTURE.pcap --out PRIVATE.jsonl  (every activity-host message, hex body)

Decoding uses the 56578 client's own codec (`d1-oracle activity-host decode`, with D1_EBOOT and
D1_ORACLE as for the server); the last-gen captures' struct layouts decode with it, but content
indices such as incident types are numbered per build. Identical bodies are decoded once, all in
one d1-oracle start (`decode-batch`). The captures carry account data: write exports outside the
repository.
"""
import argparse
import collections
import json
import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import d1_srcp  # noqa: E402
import d1paths  # noqa: E402

INCIDENT, SENSOR = 18, 5


def oracle():
    return os.environ.get('D1_ORACLE') or str(Path(d1paths.workspace()) / 'd1-server' / 'target' /
                                              'release' / 'd1-oracle.exe')


def decode_all(pairs):
    """{(udt, body): words or None} for every distinct (udt, body), with one d1-oracle start
    (`activity-host decode-batch`). words is {offset: u64}; None where the codec refused."""
    pairs = list(dict.fromkeys(pairs))
    if not pairs:
        return {}
    batch = ''.join(f'{udt} {body.hex()}\n' for udt, body in pairs)
    r = subprocess.run([oracle(), 'activity-host', 'decode-batch'], input=batch, capture_output=True,
                       text=True, cwd=d1paths.workspace(), timeout=60 + len(pairs))
    lines = r.stdout.splitlines()
    if r.returncode or len(lines) != len(pairs):
        raise RuntimeError(f'd1-oracle decode-batch: {(r.stderr or r.stdout).strip()[-300:]}')
    out = {}
    for pair, line in zip(pairs, lines):
        d = json.loads(line)
        out[pair] = None if 'error' in d else {int(k, 16): int(v, 16) for k, v in d['words'].items()}
    return out


def sensor_frames(bodies):
    """{body: frame dict or None} for udt 5 payloads (`d1-oracle sensor decode-batch`)."""
    bodies = list(dict.fromkeys(bodies))
    if not bodies:
        return {}
    batch = ''.join(b.hex() + '\n' for b in bodies)
    r = subprocess.run([oracle(), 'sensor', 'decode-batch'], input=batch, capture_output=True, text=True,
                       cwd=d1paths.workspace(), timeout=60 + len(bodies))
    lines = r.stdout.splitlines()
    if r.returncode or len(lines) != len(bodies):
        raise RuntimeError(f'd1-oracle sensor decode-batch: {(r.stderr or r.stdout).strip()[-300:]}')
    out = {}
    for body, line in zip(bodies, lines):
        d = json.loads(line)
        out[body] = None if 'error' in d else d
    return out


def incident_types(words):
    """udt 18: {u32 count, u32 types[count], ...}; the struct's words are 64-bit little-endian."""
    count = words.get(0, 0) & 0xFFFFFFFF
    flat = []
    for off in range(0, 8 + 4 * count, 8):
        w = words.get(off, 0)
        flat += [w & 0xFFFFFFFF, w >> 32]
    return flat[1:1 + count]


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('command', choices=('summary', 'incidents', 'sensors', 'export'))
    ap.add_argument('capture', type=Path)
    ap.add_argument('--dir', choices=('s2c', 'c2s'), default='s2c')
    ap.add_argument('--out', type=Path)
    a = ap.parse_args()
    msgs = d1_srcp.activity_messages(a.capture.read_bytes())
    if a.command == 'summary':
        c = collections.Counter((d, peer, udt) for _, d, peer, udt, _ in msgs)
        first = {}
        for t, d, peer, udt, _ in msgs:
            first.setdefault((d, peer, udt), t)
        for (d, peer, udt), n in sorted(c.items(), key=lambda kv: (kv[0][1], kv[0][0], kv[0][2])):
            print(f'{peer:>21} {d} udt {udt:3}  x{n:<5} first {first[(d, peer, udt)]:7.1f} s')
    elif a.command == 'incidents':
        mine = [m for m in msgs if m[1] == a.dir and m[3] == INCIDENT]
        words = decode_all((udt, body) for _, _, _, udt, body in mine)
        for t, d, peer, udt, body in mine:
            w = words[(udt, body)]
            print(f'{t:8.1f} {peer:>21} types {incident_types(w) if w else "(refused)"}  body {len(body)} B')
    elif a.command == 'sensors':
        mine = [m for m in msgs if m[1] == 's2c' and m[3] == SENSOR]
        frames = sensor_frames(body for *_, body in mine)
        for t, d, peer, udt, body in mine:
            f = frames[body]
            if f is None:
                print(f'{t:8.1f} {peer:>21} (refused) {len(body)} B')
                continue
            names = ' '.join(f"{e['hash']}/{e['i8']:#x}/{e['sub']}:{e['body_bits']}" for e in f['entries'])
            auth = f" authority {f['authority']}" if f['authority'] else ''
            print(f"{t:8.1f} {peer:>21} gen {f['gen']} stamp {f['stamp']}{auth} {names}")
    else:
        if not a.out:
            sys.exit('export needs --out (a private path)')
        with open(a.out, 'w', encoding='utf-8') as f:
            for t, d, peer, udt, body in msgs:
                f.write(json.dumps({'t': round(t, 6), 'dir': d, 'peer': peer, 'udt': udt, 'body': body.hex()}) + '\n')
        print(f'{len(msgs)} messages -> {a.out}')


if __name__ == '__main__':
    main()
