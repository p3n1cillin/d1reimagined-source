# SPDX-License-Identifier: GPL-2.0-or-later
"""Compare activity-host A-value histories, grouped by area, without copying a codec.

    d1-audit.py --ours LOCAL.jsonl [LOCAL2.jsonl ...] --live LIVE.pcap --out PRIVATE.json
    d1-audit.py ... --tag KIND=TAG --group GROUP --host HOST_SUFFIX

Set D1_ORACLE, D1_EBOOT and D1_WORKSPACE as for d1-srcp.py. Tags come from
row_encoders.rs; --tag supplies explicitly researched mappings missing there. Unknown
kinds remain in the report. Each dump and each live host is compared separately.
The output contains private decoded images and must stay outside this repository.
Windows runs and their decoder children use Idle priority; other systems use nice 19.

Comparisons use value-change ordinal, not wall time: different playthroughs are not
synchronised. "last_observed" is a capture endpoint, not proof of steady state or
mission completion. No generic value field establishes placement for every kind.
"first_changed" is the second distinct explicit image (if observed), not a guessed
placed state. All events, including absent values and refusals, remain available.
Last observed means last explicit image, even if a later entry carries no value.
"""
import argparse
import collections
import ctypes
import importlib.util
import json
import os
from pathlib import Path
import re
import sys

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
spec = importlib.util.spec_from_file_location('audit_frames', HERE / 'd1-frames.py')
frames_cli = importlib.util.module_from_spec(spec)
spec.loader.exec_module(frames_cli)
srcp = frames_cli.srcp


def low_priority():
    """Idle on Windows (also inherited by synchronous oracle/rsat children)."""
    if os.name == 'nt':
        kernel = ctypes.WinDLL('kernel32', use_last_error=True)
        kernel.GetCurrentProcess.restype = ctypes.c_void_p
        kernel.SetPriorityClass.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
        if not kernel.SetPriorityClass(kernel.GetCurrentProcess(), 0x40):
            raise ctypes.WinError(ctypes.get_last_error())
    else:
        os.nice(19)


def number(value):
    return int(str(value).replace('_', ''), 0)


def kind_tags(root=REPO):
    """Read named KIND_*/TAG_* pairs; resolve only the two canonical module aliases.

    An unrecognised Rust expression stays unresolved rather than being evaluated or
    replaced with a guessed tag. This reads definitions, not captured identifiers.
    """
    base = Path(root) / 'ps4/d1-server/crates'
    text = (base / 'd1-activity-host/src/row_encoders.rs').read_text(encoding='utf-8')
    constants = dict(re.findall(r'pub const (\w+):\s*\w+\s*=\s*([^;]+);', text))
    aliases = {}
    for module in ('sensor', 'sensor_objective'):
        source = (base / f'd1-oracle/src/{module}.rs').read_text(encoding='utf-8')
        for name, literal in re.findall(r'pub const (\w+):\s*u32\s*=\s*(0x[\da-fA-F_]+);', source):
            aliases[f'd1_oracle::{module}::{name}'] = number(literal)
    tags = {}
    for name, value in constants.items():
        if not name.startswith('KIND_'):
            continue
        expression = constants.get('TAG_' + name[5:], '')
        if re.fullmatch(r'0x[\da-fA-F_]+', expression):
            tags[number(value)] = number(expression)
        elif expression in aliases:
            tags[number(value)] = aliases[expression]
    return tags


def private_output(path, root=REPO):
    """Reject repository output, including paths through an existing directory link."""
    path = Path(path).resolve()
    if path.is_relative_to(Path(root).resolve()):
        raise ValueError('decoded audit reports must be outside the repository')
    return path


def local_stream(path):
    # Use the existing loader: it preserves JSON-line order, including retransmits.
    return [(dict(frame=i, stamp=(decoded or {}).get('stamp'),
                  source=record.get('source'), full=record.get('full')), decoded)
            for i, (record, decoded) in enumerate(frames_cli.load(path))]


def live_streams(data, host=None, start=None, end=None):
    messages = [m for m in srcp.d1_srcp.host_messages(data)
                if m[1] == 's2c' and m[4] == srcp.SENSOR
                and (host is None or f'{m[3]:x}'.endswith(host.lower().removeprefix('0x')))
                and (start is None or m[0] >= start) and (end is None or m[0] <= end)]
    decoded = srcp.sensor_frames(m[5] for m in messages)
    streams = collections.defaultdict(list)
    for i, m in enumerate(messages):
        frame = decoded[m[5]]
        streams[f'{m[3]:#018x}'].append((dict(frame=i, seconds=m[0], stamp=(frame or {}).get('stamp')),
                                      frame))
    if not streams:
        raise ValueError('no live server-to-client sensor frames matched the selection')
    return dict(streams)


def decode_values(streams, tags, groups=None, decode=None):
    """One synchronous decode per tag, deduplicated across every input stream."""
    bodies = collections.defaultdict(set)
    for stream in streams:
        for _, frame in stream:
            for entry in (frame or {}).get('entries', []):
                if (groups is None or frames_cli.key(entry)[0] in groups) and entry['i8'] in tags:
                    bodies[tags[entry['i8']]].add(entry['body'])
    decode = decode or srcp.sensor_values
    return {tag: decode(tag, sorted(values)) for tag, values in sorted(bodies.items())}


def histories(stream, tags, decoded, groups=None):
    rows, refused = {}, []
    for position, frame in stream:
        if frame is None:
            refused.append(position)
            continue
        for entry in frame.get('entries', []):
            key = frames_cli.key(entry)
            if groups is not None and key[0] not in groups:
                continue
            row = rows.setdefault(key, dict(events=[], changes=[]))
            event = dict(position, body_bits=entry['body_bits'])
            tag = tags.get(key[1])
            decoded_value = decoded.get(tag, {}).get(entry['body']) or {}
            value = decoded_value.get('image')
            if tag is None:
                event['status'] = 'missing_tag'
            elif value is not None and decoded_value.get('bits', 0) > entry['body_bits'] - 2:
                event.update(status='decode_overrun', value_bits=decoded_value['bits'],
                             candidate_image=value)
            elif value is not None:
                # Normalise hex case and validate the decoder's image.
                event.update(status='value', image=bytes.fromhex(value).hex())
                if 'bits' in decoded_value:
                    event['value_bits'] = decoded_value['bits']
                if not row['changes'] or row['changes'][-1]['image'] != event['image']:
                    row['changes'].append(dict(event))
            else:
                body = bytes.fromhex(entry['body'])
                # The same precondition used by sensor decode-values. A default-only
                # entry is not a failed decode and is never synthesised as zero bytes.
                event['status'] = ('no_explicit_value' if entry['body_bits'] < 2 or not body
                                   or body[0] & 0xC0 != 0xC0 else 'decode_refused')
            row['events'].append(event)
    return rows, refused


def byte_diff(a, b):
    """Offsets include length differences; no padding can hide a missing tail."""
    if a is None or b is None:
        return None
    a, b = bytes.fromhex(a), bytes.fromhex(b)
    return [i for i in range(max(len(a), len(b)))
            if i >= len(a) or i >= len(b) or a[i] != b[i]]


def compare_rows(ours, live):
    result = []
    for key in sorted(ours.keys() | live.keys()):
        a, b = ours.get(key), live.get(key)
        ac, bc = (r['changes'] if r else [] for r in (a, b))
        images_a, images_b = ([c['image'] for c in changes] for changes in (ac, bc))
        unresolved = any(e['status'] in ('missing_tag', 'decode_refused', 'decode_overrun')
                         for row in (a, b) if row for e in row['events'])
        def at(images, i):
            return images[i] if len(images) > i else None
        def landmark(images, i):
            return images[i] if images and (i == -1 or len(images) > i) else None
        result.append(dict(group=f'{key[0]:#010x}', kind=key[1], index=key[2],
                           row_presence='both' if a and b else 'ours_only' if a else 'live_only',
                           value_presence=('both' if ac and bc else 'ours_only' if ac else
                                           'live_only' if bc else 'neither'),
                           value_presence_uncertain=unresolved,
                           sequence_equal=None if unresolved else images_a == images_b,
                           observed_sequence_equal=images_a == images_b,
                           first_explicit=byte_diff(at(images_a, 0), at(images_b, 0)),
                           first_changed=byte_diff(at(images_a, 1), at(images_b, 1)),
                           last_observed=byte_diff(landmark(images_a, -1), landmark(images_b, -1)),
                           ordinal_differences=[dict(ordinal=i, ours_present=i < len(ac),
                                                     live_present=i < len(bc),
                                                     offsets=byte_diff(at(images_a, i), at(images_b, i)))
                                                for i in range(max(len(ac), len(bc)))
                                                if at(images_a, i) != at(images_b, i)],
                           ours=a, live=b))
    return result


def totals(rows):
    groups = collections.defaultdict(collections.Counter)
    for row in rows:
        count = groups[(row['group'], row['kind'])]
        count['rows'] += 1
        count['row_' + row['row_presence']] += 1
        count['value_' + row['value_presence']] += 1
        count['different_sequences'] += row['sequence_equal'] is False
        count['unresolved_sequences'] += row['sequence_equal'] is None
        for side in ('ours', 'live'):
            for event in (row[side] or {}).get('events', []):
                count[side + '_' + event['status']] += 1
    return [dict(group=group, kind=kind, **counts) for (group, kind), counts in sorted(groups.items())]


def coverage(rows, refused):
    statuses = collections.Counter(event['status'] for row in rows.values() for event in row['events'])
    return dict(rows=len(rows), refused_frames=len(refused), event_statuses=dict(statuses),
                decoded_value_coverage_complete=bool(rows) and not refused
                and not any(statuses[s] for s in ('missing_tag', 'decode_refused', 'decode_overrun')))


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--ours', type=Path, nargs='+', required=True)
    parser.add_argument('--live', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--tag', action='append', default=[], metavar='KIND=TAG')
    parser.add_argument('--group', action='append', type=number)
    parser.add_argument('--host', help='live host hex suffix; separate hosts are never merged')
    parser.add_argument('--start', type=float, help='live window start, seconds from first SRCP record')
    parser.add_argument('--end', type=float, help='live window end (inclusive)')
    args = parser.parse_args()
    try:
        out = private_output(args.out)
        if out in {path.resolve() for path in [*args.ours, args.live]}:
            raise ValueError('output would overwrite an input')
        if args.start is not None and args.end is not None and args.end < args.start:
            raise ValueError('--end precedes --start')
        tags = kind_tags()
        overrides = dict(tuple(map(number, pair.split('='))) for pair in args.tag)
        tags.update(overrides)
        low_priority()
        ours = {}
        for path in args.ours:
            if str(path.resolve()) in ours:
                raise ValueError('duplicate local dump')
            print(f'Loading local input {len(ours) + 1}', file=sys.stderr, flush=True)
            stream = local_stream(path)
            if not stream:
                raise ValueError('empty local dump')
            ours[str(path.resolve())] = stream
        print('Loading live input', file=sys.stderr, flush=True)
        live = live_streams(args.live.read_bytes(), args.host, args.start, args.end)
        groups = set(args.group) if args.group else None
        decoded = decode_values([*ours.values(), *live.values()], tags, groups)
        oh = {name: histories(stream, tags, decoded, groups) for name, stream in ours.items()}
        lh = {name: histories(stream, tags, decoded, groups) for name, stream in live.items()}
        comparisons = []
        for local_name, (a, ar) in oh.items():
            for host, (b, br) in lh.items():
                rows = compare_rows(a, b)
                comparisons.append(dict(ours=local_name, live_host=host, refused_ours=ar,
                                        refused_live=br, coverage_ours=coverage(a, ar),
                                        coverage_live=coverage(b, br), totals=totals(rows), rows=rows))
        report = dict(format_version=1, evidence='offline capture comparison; no emulator run',
                      decoder_target='56578; capture build is not inferred from its filename',
                      alignment='distinct explicit A-image ordinal, independently per input and host',
                      endpoint='last_observed is not proven steady state; capture coverage can differ',
                      scope='A images only; B values, authority and timing are not compared',
                      value_presence='successfully decoded explicit A images; uncertain when a tag or decode is missing',
                      live=str(args.live.resolve()), groups=sorted(groups) if groups else None,
                      live_window=[args.start, args.end],
                      tags={str(k): f'{v:#010x}' for k, v in sorted(tags.items())},
                      tag_overrides={str(k): f'{v:#010x}' for k, v in overrides.items()},
                      missing_tags=sorted({k[1] for rows, _ in [*oh.values(), *lh.values()]
                                           for k in rows if k[1] not in tags}),
                      comparisons=comparisons)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
        print(f'Wrote private report: {len(comparisons)} comparisons; '
              f'{len(report["missing_tags"])} kinds need tags')
    except (OSError, ValueError, RuntimeError) as error:
        parser.exit(2, f'audit: {error}\n')


if __name__ == '__main__':
    main()
