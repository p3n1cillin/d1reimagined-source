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


def sessions(data):
    """Activity-host sessions in order of their first message: [(start, host id, [(t, dir, peer,
    udt, body), ...])]. A session is one activity-host id. The client keeps several at once: in the
    live Restoration capture its private host stays connected all mission while a public one
    joins beside it, so the messages are told apart by host id, not by when a connection opened."""
    by_host = {}
    for t, d, peer, host, udt, body in host_messages(data):
        by_host.setdefault(host, (t, host, []))[2].append((t, d, peer, udt, body))
    return list(by_host.values())
