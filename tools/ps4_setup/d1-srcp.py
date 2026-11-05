# SPDX-License-Identifier: GPL-2.0-or-later
"""Summarise or decode the activity-host traffic in SRCP message captures (d1_srcp.py).

    d1-srcp.py summary CAPTURE.pcap
    d1-srcp.py incidents CAPTURE.pcap [--dir s2c|c2s]   (udt 18, decoded by d1-oracle)
    d1-srcp.py sensors CAPTURE.pcap                     (udt 5 sensor_auth_update frames)
    d1-srcp.py sessions CAPTURE.pcap [...]              (each host session's activity and slice sets)
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
import sqlite3
import struct
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


def activity_names():
    """{activity hash: name} from the public manifest (DestinyActivityDefinition)."""
    db = Path(d1paths.workspace()) / 'manifest' / 'data' / 'world_en.sqlite'
    names = {}
    for (js,) in sqlite3.connect(db).execute('select json from DestinyActivityDefinition'):
        d = json.loads(js)
        names[d['activityHash'] & 0xFFFFFFFF] = d.get('activityName', '?')
    return names


def hashes_in(image, known):
    """The known 32-bit values anywhere in an image, at any bit offset, either byte order."""
    n, bits, found = int.from_bytes(image, 'big'), len(image) * 8, []
    for off in range(bits - 31):
        v = (n >> (bits - off - 32)) & 0xFFFFFFFF
        for h in (v, int.from_bytes(v.to_bytes(4, 'big'), 'little')):
            if h in known and h not in found:
                found.append(h)
    return found


def image(words):
    b = bytearray(max(words) + 8 if words else 0)
    for off, v in words.items():
        struct.pack_into('<Q', b, off, v)
    return bytes(b)


def session_table(data, names):
    """Per host session: start, the session type byte the client's join request carries
    (join_result +0x38: 3 private, 2 public), the activities named by the host's join incident
    2426 (51539's numbering), and the slice sets the client reported as current (udt 21 +0xB8)."""
    sess = d1_srcp.sessions(data)
    pick = [(u, b) for _, msgs in sess for _, d, _, u, b in msgs
            if (d, u) in (('s2c', INCIDENT), ('s2c', 4), ('c2s', 21))]
    words = decode_all(pick)
    rows = []
    for start, msgs in sess:
        kind, acts, slices = None, [], []
        for _, d, _, u, b in msgs:
            w = words.get((u, b))
            if not w:
                continue
            if u == 4 and kind is None and 0x38 in w:
                kind = w[0x38] & 0xFF
            elif u == INCIDENT and not acts and incident_types(w)[:1] == [2426]:
                acts = [names[h] for h in hashes_in(image(w)[0xD8:], names)]
            elif u == 21 and 0xB8 in w and (w[0xB8] & 0x3FF) not in (0, 0x3FF):
                if (w[0xB8] & 0x3FF) not in slices:
                    slices.append(w[0xB8] & 0x3FF)
        rows.append((start, kind, acts, slices))
    return rows


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('command', choices=('summary', 'incidents', 'sensors', 'sessions', 'export'))
    ap.add_argument('capture', type=Path, nargs='+')
    ap.add_argument('--dir', choices=('s2c', 'c2s'), default='s2c')
    ap.add_argument('--out', type=Path)
    a = ap.parse_args()
    if a.command == 'sessions':
        names = activity_names()
        for cap in a.capture:
            for start, kind, acts, slices in session_table(cap.read_bytes(), names):
                kinds = {2: 'public', 3: 'private'}
                print(f"{cap.stem[20:]:28} +{start:6.0f} s {kinds.get(kind, kind)!s:8} {', '.join(acts[:2]) or '-':28} slices {slices}")
        return
    if len(a.capture) != 1:
        sys.exit(f'{a.command} takes one capture')
    msgs = d1_srcp.activity_messages(a.capture[0].read_bytes())
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
