# SPDX-License-Identifier: GPL-2.0-or-later
"""Summarise or decode the activity-host traffic in SRCP message captures (d1_srcp.py).

    d1-srcp.py summary CAPTURE.pcap                     (per activity-host id; --host ID filters any command)
    d1-srcp.py incidents CAPTURE.pcap [--dir s2c|c2s]   (udt 18, decoded by d1-oracle)
    d1-srcp.py sensors CAPTURE.pcap                     (udt 5 sensor_auth_update frames)
    d1-srcp.py sessions CAPTURE.pcap [...]              (each host session's activity and slice sets)
    d1-srcp.py mission CAPTURE.pcap [...] [--activity-table TSV]  (what a host needs: activity, world, start, groups)
    d1-srcp.py regions CAPTURE.pcap                     (the PAH region records, udt 11, as they change)
    d1-srcp.py export CAPTURE.pcap --out PRIVATE.jsonl  (every activity-host message, hex body)
    d1-srcp.py matchmaking CAPTURE.pcap [...]           (message 42/43 protobuf shapes, values redacted)

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
        setup_error(d)
        out[pair] = None if 'error' in d else {int(k, 16): int(v, 16) for k, v in d['words'].items()}
    return out


def decode_ordered(rows):
    """[words or None] for (stream, udt, body) rows in message order, one d1-oracle start. The
    replicated udts 11 and 21 carry only what changed; each decodes against its stream's previous
    image (`decode-batch`'s STREAM column), so nothing is deduplicated here."""
    rows = list(rows)
    if not rows:
        return []
    batch = ''.join(f'{udt} {body.hex()} {stream}\n' for stream, udt, body in rows)
    r = subprocess.run([oracle(), 'activity-host', 'decode-batch'], input=batch, capture_output=True,
                       text=True, cwd=d1paths.workspace(), timeout=60 + len(rows))
    lines = r.stdout.splitlines()
    if r.returncode or len(lines) != len(rows):
        raise RuntimeError(f'd1-oracle decode-batch: {(r.stderr or r.stdout).strip()[-300:]}')
    out = []
    for line in lines:
        d = json.loads(line)
        setup_error(d)
        out.append(None if 'error' in d else {int(k, 16): int(v, 16) for k, v in d['words'].items()})
    return out


PAH_REGIONS, PAH_REGION_STRIDE, PAH_REGION_COUNT = 0x2E48, 0xD8, 40


def region_records(words):
    """The PAH region records in a replicate_membership image (udt 11): [(slot, record bytes)]
    for slots naming a region (an unused slot's region is 0xFFFFFFFF)."""
    im = image(words)
    recs = []
    for i in range(PAH_REGION_COUNT):
        rec = im[PAH_REGIONS + i * PAH_REGION_STRIDE:PAH_REGIONS + (i + 1) * PAH_REGION_STRIDE]
        rec = rec.ljust(PAH_REGION_STRIDE, b'\0')
        if any(rec) and rec[:4] != bytes([0xFF] * 4):
            recs.append((i, rec))
    return recs


def describe_region(rec):
    """A region record's fields without the advertised session itself (a platform session key
    and addresses: personal data). Offsets as notes/d1-live-captures-2026-11-05.md reads them."""
    region, public = struct.unpack_from('<II', rec, 0)
    session = struct.unpack_from('<I', rec, 0x4C)[0]
    host = struct.unpack_from('<Q', rec, 0xD0)[0]
    return (f'region {region} public {public} +08 {rec[8:12].hex()} +19 {rec[0x19]} +20 {rec[0x20]} '
            f'+38 {rec[0x38]} session {"%#x + 64 B" % session if session else "-"} '
            f'host {"..%04x" % (host & 0xFFFF) if host else "-"}')


def setup_error(d):
    """A missing or unreadable eboot fails every decode; say so instead of 'refused'."""
    if str(d.get('error', '')).startswith('eboot:'):
        raise RuntimeError(f"d1-oracle could not read the eboot (set D1_EBOOT): {d['error']}")


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
        setup_error(d)
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
    """Per host session: start, the host id, the session type byte the client's join request
    carries (join_result +0x38; 3 on the hosts the client starts, its mission host and the public
    sessions it creates, 2 where it joins an existing instance: another player's public session,
    the Tower), the activity hashes named by the host's join incident
    2426 (51539's numbering), and the slice sets the client reported as current (udt 21 +0xB8)."""
    sess = d1_srcp.sessions(data)
    pick = [(u, b) for _, _, msgs in sess for _, d, _, u, b in msgs
            if (d, u) in (('s2c', INCIDENT), ('s2c', 4), ('c2s', 21))]
    words = decode_all(pick)
    rows = []
    for start, host, msgs in sess:
        kind, acts, slices = None, [], []
        for _, d, _, u, b in msgs:
            w = words.get((u, b))
            if not w:
                continue
            if u == 4 and kind is None and 0x38 in w:
                kind = w[0x38] & 0xFF
            elif u == INCIDENT and not acts and incident_types(w)[:1] == [2426]:
                acts = hashes_in(image(w)[0xD8:], names)
            elif u == 21 and 0xB8 in w and (w[0xB8] & 0x3FF) not in (0, 0x3FF):
                if (w[0xB8] & 0x3FF) not in slices:
                    slices.append(w[0xB8] & 0x3FF)
        rows.append((start, host, kind, acts, slices))
    return rows


def mission_row(cap, names, index):
    """What an activity host needs to serve a capture's mission, from its first host (the PAH):
    the activity (hash, name, the client's index when `index` maps it), the world group (the
    first group in the PAH's first non-empty sensor frame, the block our world tables call the
    world), the start slice set, and the sensor groups the PAH served, most records first."""
    data = cap.read_bytes()
    start, host, kind, acts, slices = session_table(data, names)[0]
    bodies = [b for _, d, _, h, u, b in d1_srcp.host_messages(data) if h == host and d == 's2c' and u == SENSOR]
    frames = sensor_frames(bodies)
    world, records = None, collections.defaultdict(set)
    for b in bodies:
        for e in (frames[b] or {}).get('entries', []):
            world = world or e['hash']
            records[e['hash']].add((e['i8'], e['sub']))
    act = ', '.join(f'{names[h]} {h:#010x}' + (f' (index {index[h]})' if h in index else '') for h in acts[:1])
    groups = ' '.join(f'{g}:{len(r)}' for g, r in sorted(records.items(), key=lambda kv: -len(kv[1])))
    return (f'{cap.stem[20:]:24} {act or "-"} | world {world or "-"} | start {slices[:1] or "-"} '
            f'slices {slices} | groups {groups}')


MM_REQUEST, MM_RESPONSE, MM_HEADER = 42, 43, {42: 6, 43: 8}


def pb_shape(b, depth=0):
    """A protobuf body's shape: field numbers, wire types, small integers. Values of 4096 or
    more (nonces, ids, hashes), bytes and strings are redacted, so the output can be shared
    without the capture's personal data (sessions, names, tokens)."""
    def varint(i):
        v = s = 0
        while True:
            c = b[i]
            i += 1
            v |= (c & 0x7f) << s
            s += 7
            if not c & 0x80:
                return v, i
    out, i = [], 0
    try:
        while i < len(b):
            tag, i = varint(i)
            f, wt = tag >> 3, tag & 7
            if f == 0:
                return None
            if wt == 0:
                v, i = varint(i)
                out.append(f'{f}={v if v < 4096 else "BIG"}')
            elif wt == 2:
                n, i = varint(i)
                if i + n > len(b):
                    return None
                inner = pb_shape(b[i:i + n], depth + 1) if depth < 4 and n else None
                out.append(f'{f}{{{" ".join(inner)}}}' if inner else f'{f}:len{n}')
                i += n
            elif wt == 5:
                out.append(f'{f}:f32')
                i += 4
            elif wt == 1:
                out.append(f'{f}:f64')
                i += 8
            else:
                return None
    except IndexError:
        return None
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('command', choices=('summary', 'incidents', 'sensors', 'sessions', 'mission', 'regions', 'export', 'matchmaking'))
    ap.add_argument('capture', type=Path, nargs='+')
    ap.add_argument('--dir', choices=('s2c', 'c2s'), default='s2c')
    ap.add_argument('--out', type=Path)
    ap.add_argument('--host', help='only the activity host whose id ends in these hex digits')
    ap.add_argument('--activity-table', type=Path,
                    help='mission: the client\'s index -> activity hash TSV (activity-table.tsv)')
    a = ap.parse_args()
    if a.command == 'sessions':
        names = activity_names()
        for cap in a.capture:
            for start, host, kind, acts, slices in session_table(cap.read_bytes(), names):
                kinds = {2: 'joins', 3: 'starts'}
                acts = ', '.join(names[h] for h in acts[:2])
                print(f"{cap.stem[20:]:28} +{start:6.0f} s host ..{host & 0xFFFF:04x} "
                      f"{kinds.get(kind, kind)!s:8} {acts or '-':28} slices {slices}")
        return
    if a.command == 'matchmaking':
        for cap in a.capture:
            t0 = None
            for rec in d1_srcp.records(cap.read_bytes()):
                t0 = rec['t'] if t0 is None else t0
                p = rec['payload']
                mtype = int.from_bytes(p[:2], 'big') if len(p) >= 6 else None
                if rec['dir'] not in ('c2s', 's2c') or mtype not in MM_HEADER:
                    continue
                shape = pb_shape(p[MM_HEADER[mtype]:])
                print(f"{cap.stem[20:]:28} {rec['t'] - t0:8.1f} {mtype} "
                      f"{' '.join(shape) if shape is not None else 'undecodable'}")
        return
    if a.command == 'mission':
        names = activity_names()
        index = {}
        if a.activity_table:
            for line in a.activity_table.read_text().splitlines()[1:]:
                i, h = line.split('\t')[:2]
                index.setdefault(int(h, 16), int(i))
        for cap in a.capture:
            print(mission_row(cap, names, index))
        return
    if len(a.capture) != 1:
        sys.exit(f'{a.command} takes one capture')
    msgs = d1_srcp.host_messages(a.capture[0].read_bytes())
    if a.host:
        msgs = [m for m in msgs if f'{m[3]:x}'.endswith(a.host.lower().removeprefix('0x'))]
    if a.command == 'summary':
        c = collections.Counter((host, d, udt) for _, d, _, host, udt, _ in msgs)
        first, last = {}, {}
        for t, d, _, host, udt, _ in msgs:
            first.setdefault((host, d, udt), t)
            last[(host, d, udt)] = t
        for k, n in sorted(c.items()):
            host, d, udt = k
            print(f'host {host:#018x} {d} udt {udt:3}  x{n:<5} {first[k]:7.1f}-{last[k]:7.1f} s')
    elif a.command == 'regions':
        mine = [m for m in msgs if m[1] == 's2c' and m[4] == 11]
        last = {}
        for (t, d, peer, host, udt, body), w in zip(mine, decode_ordered((m[3], 11, m[5]) for m in mine)):
            recs = tuple(region_records(w)) if w else None
            if w is None or last.get(host) == recs:
                continue
            last[host] = recs
            print(f'{t:8.1f} host ..{host & 0xFFFF:04x} membership {w.get(0, 0) & 0xFFFFFFFF}: '
                  f'{len(recs)} region records')
            for slot, rec in recs:
                print(f'           [{slot}] {describe_region(rec)}')
    elif a.command == 'incidents':
        mine = [m for m in msgs if m[1] == a.dir and m[4] == INCIDENT]
        words = decode_all((udt, body) for *_, udt, body in mine)
        for t, d, peer, host, udt, body in mine:
            w = words[(udt, body)]
            print(f'{t:8.1f} host ..{host & 0xFFFF:04x} types {incident_types(w) if w else "(refused)"}  body {len(body)} B')
    elif a.command == 'sensors':
        mine = [m for m in msgs if m[1] == 's2c' and m[4] == SENSOR]
        frames = sensor_frames(body for *_, body in mine)
        for t, d, peer, host, udt, body in mine:
            f = frames[body]
            if f is None:
                print(f'{t:8.1f} host ..{host & 0xFFFF:04x} (refused) {len(body)} B')
                continue
            names = ' '.join(f"{e['hash']}/{e['i8']:#x}/{e['sub']}:{e['body_bits']}" for e in f['entries'])
            auth = f" authority {f['authority']}" if f['authority'] else ''
            print(f"{t:8.1f} host ..{host & 0xFFFF:04x} gen {f['gen']} stamp {f['stamp']}{auth} {names}")
    else:
        if not a.out:
            sys.exit('export needs --out (a private path)')
        with open(a.out, 'w', encoding='utf-8') as f:
            for t, d, peer, host, udt, body in msgs:
                f.write(json.dumps({'t': round(t, 6), 'dir': d, 'peer': peer, 'host': f'{host:#018x}',
                                    'udt': udt, 'body': body.hex()}) + '\n')
        print(f'{len(msgs)} messages -> {a.out}')


if __name__ == '__main__':
    main()
