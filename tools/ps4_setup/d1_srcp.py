# SPDX-License-Identifier: GPL-2.0-or-later
"""Read SRCP message captures: classic pcap files whose records (link type 147) are plaintext
Destiny messages, each behind a 40-byte SRCP header, as a community member's console capture
plugin writes them (notes/d1-live-captures-2026-11-05.md).

The captures hold account data (the sign-on request carries an Xbox Live token), so they and
everything read from them stay private: nothing here writes into the repository.

Record layout (little-endian pcap; header fields as measured, not documented):
    +0   'SRCP'   +4 u16 version 1   +6 u16 header length 0x28
    +8   u64 timestamp (plugin time base)
    +16  u8 direction: 0 client -> server, 1 server -> client, 2 plugin annotation
    +24  IPv4 address, +28 u16 big-endian port (0 for the sign-on and content requests)
    +40  the message
Messages on the BAP connections are the plain frames our server builds before sealing them:
    u16 type, u32 id, then for an activity-host notification (type 9, b->c):
        u8 flag, u64 id, u32 udt, u32 length, body
    and for a client message to the host (type 8, c->ah):
        u64 host, u8 1, u32 udt, u32 length, u32 0, body
"""
import struct

LINK_SRCP = 147
HEADER = 40
BC_NOT, AH_NOT = 0x09, 0x08
DIRECTIONS = {0: 'c2s', 1: 's2c', 2: 'note'}


def records(data):
    """Yield every SRCP record of a pcap image as a dict."""
    if len(data) < 24:
        raise ValueError('not a pcap file')
    magic = struct.unpack_from('<I', data)[0]
    if magic != 0xa1b2c3d4:
        raise ValueError(f'unsupported pcap magic {magic:#x}')
    link = struct.unpack_from('<I', data, 20)[0]
    if link != LINK_SRCP:
        raise ValueError(f'link type {link}, not the SRCP plugin format ({LINK_SRCP})')
    off = 24
    while off + 16 <= len(data):
        sec, usec, incl, _ = struct.unpack_from('<IIII', data, off)
        rec = data[off + 16:off + 16 + incl]
        off += 16 + incl
        if len(rec) < HEADER or rec[:4] != b'SRCP':
            continue
        yield {'t': sec + usec / 1e6, 'dir': DIRECTIONS.get(rec[16], rec[16]),
               'ip': '.'.join(map(str, rec[24:28])), 'port': rec[28] << 8 | rec[29],
               'payload': rec[HEADER:]}


def message(payload):
    """(type, id, host, udt, body) for a BAP frame; host, udt and body are None for other types.
    host is the activity-host id the frame names (one connection per host)."""
    if len(payload) < 6:
        return None
    mtype, mid = struct.unpack_from('>HI', payload)
    if mtype == BC_NOT and len(payload) >= 23:
        _, host, udt, length = struct.unpack_from('>BQII', payload, 6)
        return mtype, mid, host, udt, payload[23:23 + length]
    if mtype == AH_NOT and len(payload) >= 27:
        host, _, udt, length, _ = struct.unpack_from('>QBIII', payload, 6)
        return mtype, mid, host, udt, payload[27:27 + length]
    return mtype, mid, None, None, None


def host_messages(data):
    """Activity-host notifications and client messages in time order:
    (seconds from the first record, 'c2s'/'s2c', server 'ip:port', host id, udt, body)."""
    out, t0 = [], None
    for rec in records(data):
        t0 = rec['t'] if t0 is None else t0
        if rec['dir'] not in ('c2s', 's2c'):
            continue
        m = message(rec['payload'])
        if m and m[3] is not None:
            out.append((rec['t'] - t0, rec['dir'], f"{rec['ip']}:{rec['port']}", m[2], m[3], m[4]))
    return out


def activity_messages(data):
    """host_messages without the host id: (t, 'c2s'/'s2c', 'ip:port', udt, body)."""
    return [(t, d, peer, udt, body) for t, d, peer, _, udt, body in host_messages(data)]


WS_REQUEST, WS_RESPONSE, QUEUEZ_UPDATE = 0x0a, 0x0b, 0x7b


def investment_messages(data):
    """The services host's investment traffic (BAP 10 client_to_world_server_request and its
    reply 11), in time order: [(t, 'req'/'rsp', sequence, status or None, service id, bits)].
    A request is `u16 type, u32 sequence`, then the bitstream `u16 service id || arguments ||
    1 bit has_payload || [u16 length || bytes]`; a reply has a u16 status before its bitstream,
    which starts with the same id (notes/ps4-56578-wire-catalog-2026-11-05.md)."""
    out, t0 = [], None
    for rec in records(data):
        t0 = rec['t'] if t0 is None else t0
        p = rec['payload']
        if len(p) < 8:
            continue
        mtype, seq = struct.unpack_from('>HI', p)
        if mtype == WS_REQUEST and rec['dir'] == 'c2s':
            status, bits = None, p[6:]
        elif mtype == WS_RESPONSE and rec['dir'] == 's2c' and len(p) >= 10:
            status, bits = struct.unpack_from('>H', p, 6)[0], p[8:]
        else:
            continue
        out.append((rec['t'] - t0, 'req' if status is None else 'rsp', seq, status,
                    struct.unpack_from('>H', bits)[0], bits))
    return out


def queuez_updates(data):
    """QueueZ object pushes (BAP 123, server to client) in time order: [(t, [(family, version,
    flags, [(instance type checksum, key, payload)]), ...])]. A push is `u16 type, u32 id`,
    `u32 family count`, then per family `u32 family, u64 root, u32 version, u8 flags, u32 object
    count` and per object `u32 instance type, u64 key, u32 length, payload` (all big-endian;
    d1_bap::queuez_span_objects). A push that does not walk to its end is left out."""
    out, t0 = [], None
    for rec in records(data):
        t0 = rec['t'] if t0 is None else t0
        p = rec['payload']
        if rec['dir'] != 's2c' or len(p) < 10 or struct.unpack_from('>H', p)[0] != QUEUEZ_UPDATE:
            continue
        s, fams = p[6:], []
        try:
            at = 4
            for _ in range(struct.unpack_from('>I', s)[0]):
                fam, _root, ver, flags, count = struct.unpack_from('>IQIBI', s, at)
                at += 21
                objs = []
                for _ in range(count):
                    itype, key, n = struct.unpack_from('>IQI', s, at)
                    if at + 16 + n > len(s):
                        raise struct.error('object runs past the push')
                    objs.append((itype, key, s[at + 16:at + 16 + n]))
                    at += 16 + n
                fams.append((fam, ver, flags, objs))
        except struct.error:
            continue
        if at == len(s):
            out.append((rec['t'] - t0, fams))
    return out


def sessions(data):
    """Activity-host sessions in order of their first message: [(start, host id, [(t, dir, peer,
    udt, body), ...])]. A session is one activity-host id. The client keeps several at once: in the
    live Restoration capture its private host stays connected all mission while a public one
    joins beside it, so the messages are told apart by host id, not by when a connection opened."""
    by_host = {}
    for t, d, peer, host, udt, body in host_messages(data):
        by_host.setdefault(host, (t, host, []))[2].append((t, d, peer, udt, body))
    return list(by_host.values())
