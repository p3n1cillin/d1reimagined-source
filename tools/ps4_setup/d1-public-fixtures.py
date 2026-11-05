#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Derive the public-bubble and social-space fixtures from the private live captures (facts only).

    d1-public-fixtures.py --raw RAW_DIR --out ps4/fixtures/56578/public

For every capture it knows the destination of, this reads each activity host the client joined
beside the mission host (the PAH), the region the PAH advertised that host for (the region
record's +0xD0), and the host's sensor frames (udt 5). It writes, per (destination, region):

* `public-<destination>-r<region>.txt`: the first full sensor list of a freshly started instance
  of that bubble, as served-list lines (`hash kind sub has_a has_b`, values left at their
  defaults), then the records that only appeared later, commented with their first offset;
* `bubbles.tsv`: one row per sensor group (bundle), with its record counts and the captures it
  was seen in;
* `social-<space>.txt` for the two social spaces the captures reach (Tower, Reef).

**What is kept is the record names and counts, never a value.** The live values carry per-player
data (the player objects' values hold the player's identity), so every line is value-less, which
the client default-initialises. The captures themselves, and anything exported from them, stay in
the private engineering folder. Whether a record kind carries an A or a B value is a property of
the kind (one answer per kind in every list read off the native client), taken from the served
lists in `ps4/fixtures/56578/mission`; a kind none of them has is commented out and named.

Decoding uses the 56578 client's own codec through d1-srcp.py (D1_EBOOT, D1_ORACLE as for it).
"""
import argparse
import collections
import importlib.util
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location('d1srcp_cli', HERE / 'd1-srcp.py')
srcp = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(srcp)
d1_srcp = srcp.d1_srcp

# The capture stems (after the date) whose destination we know, by the mission they record.
DESTINATIONS = {
    'earth': ['Restoration', 'The-Dark-Within', 'The-Warmind', 'The-Last-Array', 'Siege-of-the-Warmind',
              'The-Devils-Lair'],
    'moon': ['The-Dark-Beyond', 'The-Worlds-Grave', 'Sword-of-Crota', 'Shrine-of-Oryx', 'Chamber-of-Night',
             'The-Wakening', 'The-Summoning-Pits'],
    'venus': ['A-Strangers-Call', 'Ishtar-Collective', 'Eye-of-a-Gate-Lord', 'The-Archive', 'Patrol.Scattered-Pack'],
    'mars': ['Exclusion-Zone', 'The-Gardens-Spire', 'The-Buried-City', 'A-Rising-Tide', 'The-Black-Garden'],
    'dreadnaught': ['Patrol.Silent-Scream'],
}
# Social spaces: (space, capture stem, the activity the host's join incident names).
SOCIAL = [('tower', 'A-Guardian-Rises', 'Tower'), ('reef', 'Eye-of-a-Gate-Lord', 'Vestian Outpost')]
PLAYERS_GROUP = '0xeaaf16e2'
FRESH_STAMP = 100  # a first full list stamped under 10 s of host clock is a freshly started instance


def kind_presence(mission_dir):
    """{kind: (has_a, has_b)} from the native lists; refuses a kind read two ways."""
    seen = collections.defaultdict(set)
    for p in sorted(mission_dir.glob('*.txt')):
        for line in p.read_text().splitlines():
            f = line.split('#')[0].split()
            if len(f) >= 5 and f[0].startswith('0x'):
                seen[int(f[1], 16)].add((int(f[3]), int(f[4])))
    out = {}
    for k, v in seen.items():
        if len(v) == 1:
            out[k] = v.pop()
    return out


def stem(cap):
    return cap.stem[20:]


def host_regions(msgs):
    """{host: region} for each public instance the PAH advertised (region record +0xD0)."""
    mine = [m for m in msgs if m[1] == 's2c' and m[4] == 11]
    out = {}
    for m, w in zip(mine, srcp.decode_ordered((m[3], 11, m[5]) for m in mine)):
        for _, rec in (srcp.region_records(w) if w else []):
            region, host = int.from_bytes(rec[0:4], 'little'), int.from_bytes(rec[0xD0:0xD8], 'little')
            if host:
                out.setdefault(host, region)
    return out


def host_lists(msgs, host):
    """(start time, [(t, stamp, [(hash, kind, sub)])]) for one host's non-empty udt 5 frames."""
    mine = [m for m in msgs if m[3] == host and m[1] == 's2c' and m[4] == srcp.SENSOR]
    if not mine:
        return None, []
    frames = srcp.sensor_frames(m[5] for m in mine)
    t0 = min(m[0] for m in msgs if m[3] == host)
    out = []
    for m in mine:
        f = frames.get(m[5])
        if f and f['entries']:
            out.append((m[0] - t0, f['stamp'], [(e['hash'], e['i8'], e['sub']) for e in f['entries']]))
    return t0, out


def describe(records):
    groups = collections.OrderedDict()
    for h, k, s in records:
        groups.setdefault(h, collections.Counter())[k] += 1
    return groups


def list_lines(records, presence):
    lines = []
    for h, k, s in records:
        if k in presence:
            a, b = presence[k]
            lines.append(f'{h} {k:#x} {s} {a} {b}')
        else:
            lines.append(f'# kind {k:#x} not in any native list yet (has_a/has_b unread): {h} {k:#x} {s}')
    return lines


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--raw', type=Path, required=True, help='the private captures folder')
    ap.add_argument('--out', type=Path, required=True)
    a = ap.parse_args()
    root = HERE.parent.parent
    presence = kind_presence(root / 'ps4' / 'fixtures' / '56578' / 'mission')
    caps = {stem(p): p for p in sorted(a.raw.glob('*.pcap'))}
    a.out.mkdir(parents=True, exist_ok=True)

    # (destination, region) -> [(capture, host, start, [(t, stamp, records)])]
    bubbles = collections.defaultdict(list)
    for dest, names in DESTINATIONS.items():
        for name in names:
            cap = caps.get(name)
            if cap is None:
                sys.exit(f'no capture named {name} in {a.raw}')
            msgs = d1_srcp.host_messages(cap.read_bytes())
            for host, region in host_regions(msgs).items():
                t0, lists = host_lists(msgs, host)
                if lists:
                    bubbles[(dest, region)].append((name, host, t0, lists))
            print(f'{name}: {len(host_regions(msgs))} public instance(s)', file=sys.stderr)

    rows = []
    table = []
    for (dest, region), seen in sorted(bubbles.items()):
        # The served list: the largest first list of a freshly started instance (else of any).
        fresh = [s for s in seen if s[3][0][1] < FRESH_STAMP] or seen
        name, host, t0, lists = max(fresh, key=lambda s: len(s[3][0][2]))
        first_t, first_stamp, first = lists[0]
        later = collections.OrderedDict()
        for t, _, recs in lists[1:]:
            for r in recs:
                if r not in first and r not in later:
                    later[r] = t
        fname = f'public-{dest}-r{region}.txt'
        groups = describe(first + list(later))
        head = [
            f'# {dest.capitalize()} region {region} (bubble {region // 8}) as its public instance served it on the',
            f'# live servers (Xbox 360 build 51539): the first full list of a freshly started instance in the',
            f'# {name} capture, {first_t:.1f} s after the client joined it (host stamp {first_stamp}), then',
            f'# the records that only appeared later. Seen in {len(seen)} instance(s), in: '
            + ', '.join(sorted({s[0] for s in seen})) + '.',
            '# Names and counts only; every value is left at its default (the live values carry player data).',
            '# Generated by tools/ps4_setup/d1-public-fixtures.py; notes/ps4-56578-public-instances-2026-11-05.md.',
            '# Groups (first list / with later): ' + ', '.join(
                f'{g} {sum(1 for r in first if r[0] == g)}/{sum(c.values())}' for g, c in groups.items()),
        ]
        body = list_lines(first, presence)
        tail = [f'# later +{t:.0f} s: {h} {k:#x} {s}' for (h, k, s), t in later.items()]
        (a.out / fname).write_text('\n'.join(head + body + tail) + '\n')
        table.append(f'{dest}\t{region}\tpublic\t{fname}\t{len(first)}')
        for g, c in groups.items():
            kinds = ','.join(f'{k:#x}:{n}' for k, n in sorted(c.items()))
            rows.append(f'{dest}\t{region}\t{g}\t{sum(1 for r in first if r[0] == g)}\t{sum(c.values())}\t{kinds}\t'
                        + ','.join(sorted({s[0] for s in seen if any(r[0] == g for _, _, rs in s[3] for r in rs)})))

    for space, name, activity in SOCIAL:
        cap = caps[name]
        data = cap.read_bytes()
        names = srcp.activity_names()
        host = next(h for _, h, _, acts, _ in srcp.session_table(data, names)
                    if any(names.get(x) == activity for x in acts))
        msgs = d1_srcp.host_messages(data)
        t0, lists = host_lists(msgs, host)
        recs = list(collections.OrderedDict.fromkeys(r for _, _, rs in lists for r in rs))
        groups = describe(recs)
        fname = f'social-{space}.txt'
        head = [
            f'# {activity} (the {space} social space) as its host served it on the live servers, from the',
            f'# {name} capture: every record the host named over {lists[-1][0]:.0f} s, in order of first',
            f'# appearance (the host\'s first frames carried one record each). Its first stamp was {lists[0][1]}',
            '# (a host already running that long). Names only, values at their defaults.',
            '# Generated by tools/ps4_setup/d1-public-fixtures.py; notes/ps4-56578-public-instances-2026-11-05.md.',
            '# Groups: ' + ', '.join(f'{g} {sum(c.values())}' for g, c in groups.items()),
        ]
        (a.out / fname).write_text('\n'.join(head + list_lines(recs, presence)) + '\n')
        table.append(f'{space}\t-\tsocial\t{fname}\t{len(recs)}')

    (a.out / 'bubbles.tsv').write_text(
        'destination\tregion\tgroup\tfirst_list\twith_later\tkinds\tcaptures\n' + '\n'.join(rows) + '\n')
    (a.out / 'public-table.txt').write_text(
        '# destination region kind list first_list_records (generated by tools/ps4_setup/d1-public-fixtures.py)\n'
        + '\n'.join(table) + '\n')
    print(f'{len(table)} lists -> {a.out}', file=sys.stderr)


if __name__ == '__main__':
    main()
