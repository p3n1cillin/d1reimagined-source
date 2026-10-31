"""Private, read-only RRC v6 command/resource inspection, derived from local RPCS3.

RSXThread.cpp serializers, rsx_replay.h bitcopy layouts and serialization.hpp VLE.
No extracted bytes are written. Native replay is a separate verification step.
"""
import argparse
from collections import Counter
from datetime import datetime, timezone
import gzip
import hashlib
import json
from pathlib import Path
import struct


class Reader:
    def __init__(self, data):
        self.data, self.pos = data, 0

    def take(self, size):
        if size < 0 or size > len(self.data) - self.pos:
            raise ValueError(f"truncated capture at {self.pos}, requested {size}")
        value = memoryview(self.data)[self.pos:self.pos + size]
        self.pos += size
        return value

    def unpack(self, fmt):
        return struct.unpack(fmt, self.take(struct.calcsize(fmt)))

    def count(self):
        value = 0
        for shift in range(0, 35, 7):
            byte = self.unpack('<B')[0]
            value |= (byte & 127) << shift
            if not byte & 128:
                if value > 1_000_000:
                    raise ValueError('private inspection count limit exceeded')
                return value
        raise ValueError('unterminated VLE')

    def blob_size(self):
        value = 0
        for shift in range(0, 35, 7):
            byte = self.unpack('<B')[0]
            value |= (byte & 127) << shift
            if not byte & 128:
                return value
        raise ValueError('unterminated blob VLE')


def inspect(path):
    # Run after the owned game closes: a large capture must not contend with it.
    limit = 2 * 1024**3
    with gzip.open(path, 'rb') as stream:
        data = stream.read(limit + 1)
    if len(data) > limit:
        raise ValueError('private decompression limit exceeded')
    r = Reader(data)
    magic, version, little = r.unpack('<4sII')
    if (magic, version, little) != (b'RRC\0', 6, 1):
        raise ValueError('requires little-endian native RRC v6')
    tiles = {}
    for _ in range(r.count()):
        payload = r.take(15 * 16 + 8 * 24)
        ident = r.unpack('<Q')[0]
        if ident in tiles:
            raise ValueError('duplicate tile ID')
        tiles[ident] = hashlib.sha256(payload).hexdigest()
    memory = {}
    for _ in range(r.count()):
        offset, location, data_id, ident = r.unpack('<IIQQ')
        if ident in memory:
            raise ValueError('duplicate memory ID')
        memory[ident] = (offset, location, data_id)
    blocks = {}
    for _ in range(r.count()):
        payload = r.take(r.blob_size())
        ident = r.unpack('<Q')[0]
        if ident in blocks:
            raise ValueError('duplicate data ID')
        blocks[ident] = {'bytes': len(payload), 'sha256': hashlib.sha256(payload).hexdigest()}
    displays = {}
    for _ in range(r.count()):
        buffers = [r.unpack('<IIII') for _ in range(8)]
        count = r.unpack('<I')[0]
        ident = r.unpack('<Q')[0]
        if count > 8 or ident in displays:
            raise ValueError('invalid display count or duplicate display ID')
        displays[ident] = {'count': count, 'buffers': buffers[:count]}
    commands = []
    unknown = []
    for _ in range(r.count()):
        method, arg = r.unpack('<II')
        refs = [r.unpack('<Q')[0] for _ in range(r.count())]
        tile, display = r.unpack('<QQ')
        unknown.extend(('memory', n) for n in refs if n not in memory)
        if tile and tile not in tiles:
            unknown.append(('tile', tile))
        if display and display not in displays:
            unknown.append(('display', display))
        commands.append({'command': method, 'argument': arg, 'memory_ids': refs,
                         'tile_id': tile, 'display_id': display})
    unknown.extend(('data', v[2]) for v in memory.values() if v[2] not in blocks)
    if unknown:
        raise ValueError(f'{len(unknown)} unresolved resource references')
    # Keep the version-dependent rsx_state tail opaque. Do not invent a full-parse
    # PASS from prefix success; native BootRsxCapture must validate/replay it.
    prefix_end = r.pos
    tail = r.take(len(data) - prefix_end)
    return {'at': datetime.now(timezone.utc).isoformat(), 'source': str(path),
            'source_bytes': path.stat().st_size,
            'source_sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
            'gzip_full_decode': 'PASS', 'version': version, 'little_endian': True,
            'decompressed_bytes': len(data), 'prefix_end': prefix_end,
            'opaque_register_tail_bytes': len(tail),
            'opaque_register_tail_sha256': hashlib.sha256(tail).hexdigest(),
            'native_full_parse': 'NOT RUN', 'replay': 'NOT RUN',
            'counts': {'tiles': len(tiles), 'memory': len(memory), 'data': len(blocks),
                       'displays': len(displays), 'commands': len(commands)},
            'resource_references': 'PASS', 'memory_location_counts': dict(Counter(v[1] for v in memory.values())),
            'data_blocks': blocks,
            'memory_blocks': {k: {'offset': v[0], 'location': v[1], 'data_id': v[2]} for k, v in memory.items()},
            'display_states': displays, 'commands': commands,
            'scope': 'Private capture prefix inspection. No game bytes exported; not graphics acceptance.'}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('capture', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    result = inspect(args.capture)
    with args.output.open('x', encoding='utf-8') as output:
        json.dump(result, output, indent=2)
    print(json.dumps({k: result[k] for k in ('counts', 'gzip_full_decode', 'resource_references',
                                            'opaque_register_tail_bytes', 'native_full_parse', 'replay')}))
