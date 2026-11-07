# SPDX-License-Identifier: GPL-2.0-or-later
"""Summarise or decode the activity-host traffic in SRCP message captures (d1_srcp.py).

    d1-srcp.py summary CAPTURE.pcap                     (per activity-host id; --host ID filters any command)
    d1-srcp.py incidents CAPTURE.pcap [--dir s2c|c2s]   (udt 18, decoded by d1-oracle)
    d1-srcp.py sensors CAPTURE.pcap                     (udt 5 sensor_auth_update frames)
    d1-srcp.py lifetime CAPTURE.pcap                    (each host's activity lifetime values, as they change)
    d1-srcp.py sessions CAPTURE.pcap [...]              (each host session's activity and slice sets)
    d1-srcp.py mission CAPTURE.pcap [...] [--activity-table TSV]  (what a host needs: activity, world, start, groups)
    d1-srcp.py hosts CAPTURE.pcap [...] [--activity-table TSV]    (the same for every host naming an activity, as TSV)
    d1-srcp.py regions CAPTURE.pcap                     (the PAH region records, udt 11, as they change)
    d1-srcp.py moves CAPTURE.pcap                       (the host's slice-set requests, udt 11, and the
                                                        slice sets the client reports, udt 21: which moves
                                                        the host asked for and which the client walked)
    d1-srcp.py values CAPTURE.pcap --kind 0x1d [--group G] [--tag T]  (each sensor row's decoded value
                                                        as it changes; the tag defaults by kind)
    d1-srcp.py export CAPTURE.pcap --out PRIVATE.jsonl  (every activity-host message, hex body)
    d1-srcp.py matchmaking CAPTURE.pcap [...]           (message 42/43 protobuf shapes, values redacted)
    d1-srcp.py investment CAPTURE.pcap [...] [--skip 702]  (BAP 10/11 investment requests and replies,
                                                        arguments decoded by d1-rsat, with the QueueZ pushes)
    d1-srcp.py queuez CAPTURE.pcap [...]                (QueueZ 123 pushes: family, type checksum, length)
    d1-srcp.py stacks CAPTURE.pcap [...]                (the account and profile slots each push writes:
                                                        content, quantity and its change; d1-rsat)
    d1-srcp.py dismantles CAPTURE.pcap [...]            (402 requests, matched replies and candidate stack
                                                        changes; private JSONL, never a yields table)
    d1-srcp.py current CAPTURE.pcap [...]               (each profile push's current activity, field [6]:
                                                        its activity index and completed flag, and the
                                                        byte ranges the push writes, offsets only; d1-rsat)

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
LIFETIME_KIND, LIFETIME_TAG = 0x10, 0x808008A3  # sensor kind 0x10 carries the 0x808008A3 lifetime value


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


def sensor_values(tag, bodies):
    """{entry body: decoded value dict or None} (`d1-oracle sensor decode-values`, one start)."""
    bodies = list(dict.fromkeys(bodies))
    if not bodies:
        return {}
    batch = ''.join(f'{tag:#x} {b}\n' for b in bodies)
    r = subprocess.run([oracle(), 'sensor', 'decode-values'], input=batch, capture_output=True, text=True,
                       cwd=d1paths.workspace(), timeout=120 + 2 * len(bodies))
    lines = r.stdout.splitlines()
    if r.returncode or len(lines) != len(bodies):
        raise RuntimeError(f'd1-oracle sensor decode-values: {(r.stderr or r.stdout).strip()[-300:]}')
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


def incident_body(words):
    """udt 18's one metadata body: u32 length at +0xD4, bytes from +0xD8 (at most 500). The client
    decodes it as the RSAT type its FIRST incident's body selector names (0x2554B0)."""
    img = image(words) if words else b''
    if len(img) < 0xD8:
        return b''
    n = min(struct.unpack_from('<I', img, 0xD4)[0], 500)
    return (img + bytes(0xD8 + n))[0xD8:0xD8 + n]


def lifetime_value(image_hex):
    """The decoded lifetime value: the signed first byte of the 0x808008A3 image (wire nibble - 1)."""
    if not image_hex:
        return None
    v = int(image_hex[:2], 16)
    return v - 256 if v > 127 else v


def lifetime_changes(rows):
    """[(t, host, group, sub, value)] where each (host, group, sub) lifetime record changes value."""
    out, last = [], {}
    for t, host, group, sub, value in rows:
        key = (host, group, sub)
        if value is not None and last.get(key) != value:
            last[key] = value
            out.append((t, host, group, sub, value))
    return out


# The host's group slice-set request in replicate_membership (udt 11): {u3 state, {u8, i10 index,
# u32 name hash}} at group block +0x2E48, field +0x21C8 (messages.rs GROUP_SLICE_REQUEST). The
# client's own slice set in client_authoritative_data_update (udt 21): +0xB8 index, +0xBC hash.
SLICE_REQUEST, CLIENT_SLICE = 0x2E48 + 0x21C8, 0xB8
# Value tags by sensor kind, for `values` (row_encoders.rs).
KIND_TAGS = {0x1D: 0x80800463, 0x10: LIFETIME_TAG}


def slice_request(im):
    """(state, slice-set index, name hash) of the host's request in a udt 11 image, or None."""
    if len(im) < SLICE_REQUEST + 16:
        return None
    return (im[SLICE_REQUEST], struct.unpack_from('<I', im, SLICE_REQUEST + 8)[0] & 0x3FF,
            struct.unpack_from('<I', im, SLICE_REQUEST + 12)[0])


def client_slice(im):
    """(slice-set index, name hash) the client reports in a udt 21 image, or None."""
    if len(im) < CLIENT_SLICE + 8:
        return None
    return struct.unpack_from('<I', im, CLIENT_SLICE)[0] & 0x3FF, struct.unpack_from('<I', im, CLIENT_SLICE + 4)[0]


def value_changes(rows):
    """[(t, key, value)] where each key's value changes (None values are skipped)."""
    out, last = [], {}
    for t, key, value in rows:
        if value is not None and last.get(key) != value:
            last[key] = value
            out.append((t, key, value))
    return out


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


def host_rows(data, names):
    """Every host session's mission facts: [(start, host, kind, activity hashes, slice sets, world
    group, {group: {(i8, sub)}})]. The world group is the first group in the host's first non-empty
    sensor frame (the block our world tables call the world); the groups are every sensor record
    the host served."""
    rows = []
    msgs = d1_srcp.host_messages(data)
    sessions = session_table(data, names)
    frames = sensor_frames(b for _, d, _, _, u, b in msgs if d == 's2c' and u == SENSOR)
    for start, host, kind, acts, slices in sessions:
        world, records = None, collections.defaultdict(set)
        for _, d, _, h, u, b in msgs:
            if h != host or d != 's2c' or u != SENSOR:
                continue
            for e in (frames[b] or {}).get('entries', []):
                world = world or e['hash']
                records[e['hash']].add((e['i8'], e['sub']))
        rows.append((start, host, kind, acts, slices, world, records))
    return rows


def format_groups(records):
    """`group:records` pairs, most records first."""
    return ' '.join(f'{g}:{len(r)}' for g, r in sorted(records.items(), key=lambda kv: (-len(kv[1]), kv[0])))


def mission_row(cap, names, index):
    """What an activity host needs to serve a capture's mission, from its first host (the PAH):
    the activity (hash, name, the client's index when `index` maps it), the world group, the start
    slice set, and the sensor groups the PAH served, most records first (see `host_rows`)."""
    start, host, kind, acts, slices, world, records = host_rows(cap.read_bytes(), names)[0]
    act = ', '.join(f'{names[h]} {h:#010x}' + (f' (index {index[h]})' if h in index else '') for h in acts[:1])
    return (f'{cap.stem[20:]:24} {act or "-"} | world {world or "-"} | start {slices[:1] or "-"} '
            f'slices {slices} | groups {format_groups(records)}')


HOSTS_COLUMNS = 'capture\thost\tkind\tactivity_hash\tindex\tname\tworld_group\tstart\tslices\tgroups'


def hosts_tsv_rows(label, data, names, index):
    """`hosts`: one TSV row per host session that names an activity (the PAH and the follow-on
    activities' own PAHs: cine_reveal and the Tower after A Guardian Rises, A Key Awaits ...).
    Public instances, which name none, are left out. Facts only: no account, session or address
    data (those stay in the captures)."""
    out = []
    kinds = {2: 'joins', 3: 'starts'}
    for start, host, kind, acts, slices, world, records in host_rows(data, names):
        if not acts:
            continue
        h = acts[0]
        out.append('\t'.join([label, f'{host & 0xFFFF:04x}', kinds.get(kind, str(kind)), f'{h:#010x}',
                              str(index.get(h, '-')), names.get(h, '?'), world or '-',
                              str(slices[0]) if slices else '-', ','.join(map(str, slices)) or '-',
                              format_groups(records) or '-']))
    return out


def rsat():
    """The native codec's CLI (`d1-rsat`, ps4/d1-server/crates/d1-rsat): D1_RSAT, else beside
    the oracle."""
    return os.environ.get('D1_RSAT') or str(Path(oracle()).with_name('d1-rsat.exe' if os.name == 'nt'
                                                                     else 'd1-rsat'))


def rsat_decode(rows):
    """[decoded dict] for (tag, body, at[, fill]) rows, one `d1-rsat decode-batch` start: each body
    decoded as tag from bit `at` (into an image of `fill` bytes, hex); a dict has 'end' (the absolute bit after the walk) and 'object' (the
    struct image, hex), or 'error'."""
    rows = list(rows)
    if not rows:
        return []
    batch = ''.join(f'{r[0]:#x} {r[1].hex()} {r[2]}' + (f' {r[3]}' if len(r) > 3 else '') + '\n' for r in rows)
    r = subprocess.run([rsat(), 'decode-batch'], input=batch, capture_output=True, text=True,
                       timeout=60 + len(rows))
    lines = r.stdout.splitlines()
    if r.returncode or len(lines) != len(rows):
        raise RuntimeError(f'd1-rsat decode-batch: {(r.stderr or r.stdout).strip()[-300:]}')
    return [json.loads(line) for line in lines]


def rtti_dir():
    return Path(d1paths.fixtures()) / 'rtti'


def load_services(path=None):
    """{service id: (request tag, response tag)} from the 56578 RTTI export (services.tsv)."""
    out = {}
    for line in Path(path or rtti_dir() / 'services.tsv').read_text().splitlines()[1:]:
        p = line.split('\t')
        out[int(p[0])] = (int(p[2], 16), int(p[3], 16))
    return out


def load_fields(path=None):
    """{tag: [(offset, kind, child tag or None)]} from the RTTI export's fields.tsv."""
    out = collections.defaultdict(list)
    for line in Path(path or rtti_dir() / 'fields.tsv').read_text().splitlines()[1:]:
        p = line.split('\t')
        if not p[4]:
            continue  # the one incomplete schema's unmarked field (0x80802049 field 9)
        child = int(p[6])
        out[int(p[0], 16)].append((int(p[4]), int(p[5]), None if child == 0xFFFFFFFF else child))
    return dict(out)


# kind low byte -> (C width, signed): bool, biased i8/i16/i32, i64, raw u8/u16/u32/u64, f32
LEAF = {2: (1, False), 3: (1, True), 4: (2, True), 5: (4, True), 6: (8, True), 7: (1, False),
        8: (2, False), 9: (4, False), 0xA: (8, False), 0xB: (4, None)}


def render_fields(tag, image, fields, base=0, depth=0):
    """A decoded struct image's top-level (and nested) leaf fields as `[(path, value)]`, offsets
    from `fields`. 64-bit values of 2**20 or more (item instances, nonces, account ids) are shown
    as `#....` and their last four hex digits only, so the output can be shared."""
    out = []
    for i, (off, kind, child) in enumerate(fields.get(tag, [])):
        at = base + off
        if child is not None and depth < 4 and child in fields:
            out += [(f'f{i}.{p}', v) for p, v in render_fields(child, image, fields, at, depth + 1)]
            continue
        leaf = LEAF.get(kind & 0xFF)
        if leaf is None or at + leaf[0] > len(image):
            continue
        width, signed = leaf
        if signed is None:
            v = round(struct.unpack_from('<f', image, at)[0], 4)
        else:
            v = int.from_bytes(image[at:at + width], 'little', signed=signed)
            if width == 8 and abs(v) >= 1 << 20:
                v = f'#..{v & 0xFFFF:04x}'
        out.append((f'f{i}', v))
    return out


def payload_tail(bits, end):
    """After an investment message's arguments: its has_payload bit at `end` and, when set, the
    16-bit length and the payload bytes (MSB-first, not byte aligned). (flag, payload, end)."""
    def take(at, n):
        v = 0
        for k in range(n):
            i = at + k
            v = v << 1 | ((bits[i >> 3] >> (7 - (i & 7))) & 1 if i >> 3 < len(bits) else 0)
        return v
    if end >= len(bits) * 8:
        return False, b'', end
    if not take(end, 1):
        return False, b'', end + 1
    n = take(end + 1, 16)
    return True, bytes(take(end + 17 + 8 * k, 8) for k in range(n)), end + 17 + 8 * n


def investment_rows(data, services, fields, decode=rsat_decode):
    """Each investment message decoded: [(t, dir, seq, status, id, [(field, value)] or an error
    string, payload length or None, trailing bits)]. Arguments decode with the 56578 schemas: a
    row whose walk does not end in its last byte (`trailing bits` outside 0..7) was laid out
    differently on the capture's build."""
    msgs = d1_srcp.investment_messages(data)
    todo = [(i, services[m[4]][0 if m[1] == 'req' else 1], m[5]) for i, m in enumerate(msgs)
            if m[4] in services]
    decoded = dict(zip((i for i, _, _ in todo), decode((tag, body, 16) for _, tag, body in todo)))
    rows = []
    for i, (t, d, seq, status, mid, bits) in enumerate(msgs):
        r = decoded.get(i)
        if r is None:
            rows.append((t, d, seq, status, mid, 'no schema', None, None))
            continue
        if 'error' in r:
            rows.append((t, d, seq, status, mid, r['error'], None, None))
            continue
        tag = services[mid][0 if d == 'req' else 1]
        flag, payload, end = payload_tail(bits, r['end'])
        rows.append((t, d, seq, status, mid, render_fields(tag, bytes.fromhex(r['object']), fields),
                     len(payload) if flag else None, len(bits) * 8 - end))
    return rows


# QueueZ object types of the 51539 captures that the 56578 schemas read, as (checksum -> what, 56578
# tag, slot array offset, slots, whole): the account (`0x808018c7`, its field [6] inventory of 326
# 32-byte slots at +0x4C8: Glimmer at slot 308) walks to the end of most of its bodies; the ones
# that also carry a later field stop past field [6], so its slots are still read ("partial"). The
# character profile (`0x80801743`, 340 slots at +0x38 on 56578) walks only part-way on 51539, so
# only its first 320 slots are read (later ones decode as garbage: the 51539 layout differs after
# them). `whole` refuses a partial walk outright.
STACK_OBJECTS = {0xB5DF2907: ('account', 0x808018C7, 0x4C8, 326, False),
                 0xEB4B8439: ('profile', 0x80801743, 0x38, 320, False)}


def present_bytes(zeros, ones):
    """The bytes a sparse body wrote: equal in the decodes over a 0x00 image and a 0xFF image."""
    return {i for i, (a, b) in enumerate(zip(zeros, ones)) if a == b}


def slot_changes(image, present, base, count, state):
    """The 32-byte slots `{u16 content @0, u64 instance @8, i32 quantity @0x10}` a body carries:
    [(slot, content or None, quantity or None, previous quantity or None, instance written)].
    `state` ({slot: (content, quantity)}) carries what earlier bodies said and is updated."""
    out = []
    for s in sorted({(i - base) // 32 for i in present if base <= i < base + 32 * count}):
        e = base + 32 * s
        ci = struct.unpack_from('<H', image, e)[0] if e in present else None
        q = struct.unpack_from('<i', image, e + 0x10)[0] if e + 0x10 in present else None
        old_ci, old_q = state.get(s, (None, None))
        ci = old_ci if ci is None else ci
        out.append((s, ci, q, old_q, e + 8 in present))
        state[s] = (ci, old_q if q is None else q)
    return out


def stack_rows(data, decode=rsat_decode):
    """Every account and profile object the capture's QueueZ pushes carry, decoded twice (over
    0x00 and 0xFF) to see which slots it writes: [(t, what, version, slot changes, note)]."""
    objs = [(t, ver, it, body) for t, fams in d1_srcp.queuez_updates(data) for fam, ver, flags, os_ in fams
            for it, _, body in os_ if it in STACK_OBJECTS]
    res = decode((STACK_OBJECTS[it][1], body, 0, fill) for _, _, it, body in objs for fill in ('00', 'ff'))
    state, rows = collections.defaultdict(dict), []
    for k, (t, ver, it, body) in enumerate(objs):
        what, _, base, count, whole = STACK_OBJECTS[it]
        z, o = res[2 * k], res[2 * k + 1]
        zi, oi = z.get('object') or z.get('partial'), o.get('object') or o.get('partial')
        if zi is None or oi is None or (whole and 'error' in z):
            rows.append((t, what, ver, [], 'not read: ' + z.get('error', '')[:60]))
            continue
        zi, oi = bytes.fromhex(zi), bytes.fromhex(oi)
        note = 'partial' if 'error' in z else ''
        rows.append((t, what, ver, slot_changes(zi, present_bytes(zi, oi), base, count, state[what]), note))
    return rows


def correlate_item_actions(messages, actions, stacks):
    """Candidate deltas between a 402 and its matched reply, never inferred reward rules.

    actions is [(message, decoded-fields-or-None)]. Sequence numbers and item instances are
    used internally only. Other requests in the window make attribution ambiguous; even an
    isolated window is an observation, not proof of a tier/bucket formula.
    """
    out = []
    for k, (request, fields) in enumerate(actions):
        t, direction, sequence, _, service, _ = request
        if direction != 'req':
            continue
        row = dict(time=t, content=None, instanced=None, quantity=None, accepted=False,
                   issues=[], candidates=[])
        out.append(row)
        if fields is None or not all(key in fields for key in ('f0.f0', 'f0.f2', 'f1')):
            row['issues'].append('request schema did not consume the complete message')
            continue
        row.update(content=fields['f0.f2'], instanced=bool(fields['f0.f0']), quantity=fields['f1'])
        replies = [(m, f) for m, f in actions[k + 1:]
                   if m[1] == 'rsp' and m[2] == sequence]
        if len(replies) != 1 or sum(m[1] == 'req' and m[2] == sequence for m, _ in actions) != 1:
            row['issues'].append('missing or ambiguous reply sequence')
            continue
        reply, result = replies[0]
        end = reply[0]
        row['accepted'] = (reply[3] == 200 and result is not None
                           and result.get('f0.f0') == 0 and result.get('f0.f1') == 0)
        if not row['accepted']:
            row['issues'].append('reply refused or did not decode completely')
            continue
        if any(t <= m[0] <= end and m[1] == 'req' and m[2] != sequence and m[4] != 702
               for m in messages):
            row['issues'].append('another investment request overlaps this window')
        if not row['instanced']:
            row['issues'].append('stack item action; not necessarily a dismantle')
        for pushed, scope, version, changes, note in stacks:
            if not t <= pushed <= end:
                continue
            if note:
                issue = 'inventory decode: ' + note
                if issue not in row['issues']:
                    row['issues'].append(issue)
            for slot, content, quantity, previous, instance_written in changes:
                if quantity is not None and previous == quantity and not instance_written:
                    continue
                delta = quantity - previous if quantity is not None and previous is not None else None
                row['candidates'].append(dict(scope=scope, slot=slot, content=content,
                                              before=previous, after=quantity, delta=delta,
                                              instance_written=instance_written))
    return out


def dismantle_rows(data, decode=rsat_decode):
    """Decode 402 only using existing schemas, then correlate existing stack_rows.

    A complete message must end with only its 0..7 alignment bits after the payload tail.
    Partial decodes and different-build layouts cannot become silently accepted observations.
    """
    messages = d1_srcp.investment_messages(data)
    selected = [m for m in messages if m[4] == 402]
    if not selected:
        return []
    request_tag, response_tag = load_services()[402]
    todo = [(request_tag if m[1] == 'req' else response_tag, m[5], 16) for m in selected]
    decoded = decode(todo)
    if len(decoded) != len(todo):
        raise ValueError('native codec returned an incomplete batch')
    fields, actions = load_fields(), []
    for message, (tag, bits, _), result in zip(selected, todo, decoded):
        valid = 'object' in result and 'error' not in result and result.get('end', len(bits) * 8 + 1) < len(bits) * 8
        if valid:
            _, _, end = payload_tail(bits, result['end'])
            valid = 0 <= len(bits) * 8 - end < 8
        values = dict(render_fields(tag, bytes.fromhex(result['object']), fields)) if valid else None
        actions.append((message, values))
    return correlate_item_actions(messages, actions, stack_rows(data, decode=decode))


# The profile's field [6], the current activity (struct 0x808017b9 at +0x2c20): its activity index
# (u16 @+0x2c20, the build's own numbering) and its completed flag (+0x2c38).
CURRENT_ACTIVITY, CURRENT_COMPLETED = 0x2C20, 0x2C38


def byte_ranges(present):
    """Sorted byte offsets as [(first, last)] runs."""
    runs = []
    for i in sorted(present):
        if runs and i == runs[-1][1] + 1:
            runs[-1][1] = i
        else:
            runs.append([i, i])
    return [tuple(r) for r in runs]


def current_rows(data, decode=rsat_decode):
    """Each profile push's current activity: [(t, version, index or None, completed or None,
    [(first, last)] byte ranges written)]. Only offsets and the two field-[6] values are kept, so
    the output carries none of the capture's identifiers."""
    objs = [(t, ver, body) for t, fams in d1_srcp.queuez_updates(data) for fam, ver, flags, os_ in fams
            for it, _, body in os_ if it == 0xEB4B8439]
    res = decode((0x80801743, body, 0, fill) for _, _, body in objs for fill in ('00', 'ff'))
    rows = []
    for k, (t, ver, body) in enumerate(objs):
        z, o = res[2 * k], res[2 * k + 1]
        zi, oi = z.get('object') or z.get('partial'), o.get('object') or o.get('partial')
        if zi is None or oi is None:
            continue
        zi, oi = bytes.fromhex(zi), bytes.fromhex(oi)
        present = present_bytes(zi, oi)
        index = (struct.unpack_from('<H', zi, CURRENT_ACTIVITY)[0]
                 if {CURRENT_ACTIVITY, CURRENT_ACTIVITY + 1} <= present else None)
        done = zi[CURRENT_COMPLETED] if CURRENT_COMPLETED in present else None
        rows.append((t, ver, index, done, byte_ranges(present)))
    return rows


def queuez_summary(fams):
    """One push as `family:type-checksum:length` per object (no keys, no roots)."""
    return ' | '.join(f'f{fam}{"B" if flags & 1 else ""} ' + ' '.join(f'{it:08x}:{len(b)}' for it, _, b in objs)
                      for fam, _, flags, objs in fams)


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
    ap.add_argument('command', choices=('summary', 'incidents', 'sensors', 'lifetime', 'sessions', 'mission',
                                        'hosts', 'regions', 'export', 'matchmaking', 'investment', 'queuez',
                                        'stacks', 'current', 'moves', 'values', 'dismantles'))
    ap.add_argument('--skip', default='702',
                    help='investment: service ids to leave out (comma-separated; default 702, the periodic one)')
    ap.add_argument('capture', type=Path, nargs='+')
    ap.add_argument('--dir', choices=('s2c', 'c2s'), default='s2c')
    ap.add_argument('--out', type=Path)
    ap.add_argument('--host', help='only the activity host whose id ends in these hex digits')
    ap.add_argument('--kind', type=lambda v: int(v, 0), help='values: the sensor kind (0x1d = player triggers)')
    ap.add_argument('--group', type=lambda v: int(v, 0), help='values: only this group hash')
    ap.add_argument('--tag', type=lambda v: int(v, 0), help='values: the value tag (default by kind)')
    ap.add_argument('--activity-table', type=Path,
                    help='mission: the client\'s index -> activity hash TSV (activity-table.tsv)')
    a = ap.parse_args()
    if a.command == 'dismantles':
        for cap in a.capture:
            for row in dismantle_rows(cap.read_bytes()):
                print(json.dumps(dict(capture=cap.stem, **row), sort_keys=True))
        return
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
    if a.command == 'current':
        for cap in a.capture:
            for t, ver, index, done, ranges in current_rows(cap.read_bytes()):
                six = '' if index is None and done is None else \
                    f' field[6] activity {"ffff" if index == 0xFFFF else index} completed {done}'
                print(f'{cap.stem[20:]:20} {t:8.1f} profile v{ver}{six} writes '
                      + ' '.join(f'+{a:#x}' + (f'..+{b:#x}' if b != a else '') for a, b in ranges))
        return
    if a.command == 'stacks':
        for cap in a.capture:
            for t, what, ver, changes, note in stack_rows(cap.read_bytes()):
                body = '; '.join(f'slot {s}' + (f' content {ci}' if ci is not None else '')
                                 + (f' quantity {q}' if q is not None else '')
                                 + (f' ({q - old:+d})' if q is not None and old is not None and q != old else '')
                                 + (' instance' if inst else '') for s, ci, q, old, inst in changes)
                if body or note.startswith('not'):
                    print(f'{cap.stem[20:]:20} {t:8.1f} {what:7} v{ver} {note} {body}')
        return
    if a.command in ('investment', 'queuez'):
        skip = {int(x) for x in a.skip.split(',') if x}
        for cap in a.capture:
            data = cap.read_bytes()
            events = []
            if a.command == 'investment':
                for t, d, seq, status, mid, flds, plen, trail in investment_rows(data, load_services(), load_fields()):
                    if mid in skip:
                        continue
                    body = flds if isinstance(flds, str) else ' '.join(f'{p}={v}' for p, v in flds)
                    extra = (f' payload {plen} B' if plen is not None else '') + \
                        (f' trailing {trail} bits' if trail is not None and not 0 <= trail < 8 else '')
                    events.append((t, f'{d} {mid} seq {seq}' + (f' status {status}' if status is not None else '')
                                   + f' {body}{extra}'))
            for t, fams in d1_srcp.queuez_updates(data):
                events.append((t, 'queuez ' + queuez_summary(fams)))
            for t, e in sorted(events, key=lambda x: x[0]):
                print(f'{cap.stem[20:]:20} {t:8.1f} {e}')
        return
    if a.command in ('mission', 'hosts'):
        names = activity_names()
        index = {}
        if a.activity_table:
            for line in a.activity_table.read_text().splitlines()[1:]:
                i, h = line.split('\t')[:2]
                index.setdefault(int(h, 16), int(i))
        if a.command == 'hosts':
            print(HOSTS_COLUMNS)
        for cap in a.capture:
            if a.command == 'hosts':
                rows = hosts_tsv_rows(cap.stem[20:], cap.read_bytes(), names, index)
                if rows:
                    print('\n'.join(rows), flush=True)
            else:
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
    elif a.command == 'moves':
        rows = []
        for udt, d, label, read in ((11, 's2c', 'host request', slice_request), (21, 'c2s', 'client in', client_slice)):
            mine = [m for m in msgs if m[1] == d and m[4] == udt]
            for m, w in zip(mine, decode_ordered((f'{m[3]:x}', udt, m[5]) for m in mine)):
                if w:
                    rows.append((m[0], (m[3], label), read(image(w))))
        for t, (host, label), v in sorted(value_changes(sorted(rows, key=lambda r: r[0])), key=lambda r: r[0]):
            what = f'state {v[0]} slice set {v[1]} hash {v[2]:#010x}' if len(v) == 3 else f'slice set {v[0]} hash {v[1]:#010x}'
            print(f'{t:8.1f} host ..{host & 0xFFFF:04x} {label:12} {what}')
    elif a.command == 'values':
        if a.kind is None or (a.tag or KIND_TAGS.get(a.kind)) is None:
            ap.error('values needs --kind (and --tag for kinds without a default)')
        tag = a.tag or KIND_TAGS[a.kind]
        mine = [m for m in msgs if m[1] == 's2c' and m[4] == SENSOR]
        frames = sensor_frames(body for *_, body in mine)
        rows = [(t, host, int(str(e['hash']), 0), e['sub'], e['body'])
                for t, d, peer, host, udt, body in mine for e in (frames[body] or {}).get('entries', [])
                if e['i8'] == a.kind and (a.group is None or int(str(e['hash']), 0) == a.group)]
        values = sensor_values(tag, [r[4] for r in rows])
        for t, (host, group, sub), image_hex in value_changes(
                (t, (host, group, sub), (values.get(b) or {}).get('image')) for t, host, group, sub, b in rows):
            print(f'{t:8.1f} host ..{host & 0xFFFF:04x} {group:#010x}/{a.kind:#x}/{sub} {image_hex}')
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
            meta = f'metadata {len(incident_body(w))} B' if w else ''
            print(f'{t:8.1f} host ..{host & 0xFFFF:04x} types {incident_types(w) if w else "(refused)"}  '
                  f'body {len(body)} B {meta}')
    elif a.command == 'lifetime':
        mine = [m for m in msgs if m[1] == 's2c' and m[4] == SENSOR]
        frames = sensor_frames(body for *_, body in mine)
        entries = list(dict.fromkeys(e['body'] for f in frames.values() if f for e in f['entries']
                                     if e['i8'] == LIFETIME_KIND))
        values = sensor_values(LIFETIME_TAG, entries)
        rows = [(t, host, e['hash'], e['sub'], lifetime_value((values.get(e['body']) or {}).get('image')))
                for t, d, peer, host, udt, body in mine for e in (frames[body] or {}).get('entries', [])
                if e['i8'] == LIFETIME_KIND]
        for t, host, group, sub, value in lifetime_changes(rows):
            print(f'{t:8.1f} host ..{host & 0xFFFF:04x} {group}/0x10/{sub} lifetime {value}')
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
            # The changed blocks grouped by (peer, seq, applied): "peer 7 seq 1 applied 1 x19".
            blocks = f.get('authority_blocks') or []
            groups = {}
            for b in blocks:
                groups.setdefault((b['peer'], b['seq'], b['applied']), []).append(b['block'])
            auth += ''.join(f" [peer {p} seq {s} applied {ap} blocks {min(bs)}-{max(bs)} x{len(bs)}]"
                            for (p, s, ap), bs in sorted(groups.items()))
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
