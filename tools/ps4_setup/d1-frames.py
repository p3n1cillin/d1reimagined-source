# SPDX-License-Identifier: GPL-2.0-or-later
"""Read the udt 5 frames an activity host dumped with D1_AH_DUMP_FRAMES=<file> (JSON lines).

    d1-frames.py summary DUMP                      (each frame: source, stamp, full/delta, entries, init, authority)
    d1-frames.py compare DUMP_A DUMP_B --group G   (the first full list naming group G in each dump, entry by entry:
                                                    rows only one names, and rows whose bodies differ, by group/kind)
    d1-frames.py values DUMP --group G --kind K --tag T
                                                   (each row's decoded value image as it changes, frame by frame,
                                                    as d1-srcp.py values reads a live capture)

Frames are decoded with the client's own codec (`d1-oracle sensor decode-batch`, D1_ORACLE and D1_EBOOT as for
d1-srcp.py). A dump holds the player's machine id: keep it outside the repository. Run F40 (6 November) was found
this way: the bridge's Breach list sent its squads without the B-presence bit, 2 bits where the step schedule's had 3.
"""
import argparse
import collections
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import importlib.util
_spec = importlib.util.spec_from_file_location('d1_srcp_cli', Path(__file__).resolve().parent / 'd1-srcp.py')
srcp = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(srcp)


def load(path):
    rows = [json.loads(line) for line in Path(path).read_text(encoding='utf-8').splitlines() if line.strip()]
    frames = srcp.sensor_frames(bytes.fromhex(r['frame']) for r in rows)
    return [(r, frames[bytes.fromhex(r['frame'])]) for r in rows]


def key(e):
    return (int(str(e['hash']), 0), e['i8'], e['sub'])


def first_full_naming(dump, group):
    for r, d in dump:
        if d and d.get('init') and any(key(e)[0] == group for e in d.get('entries', [])):
            return r, d
    return None, None


def compare(a, b):
    """(only in a, only in b, {(group, kind): count of differing bodies}, one example per (group, kind))."""
    ea = {key(e): e for e in a['entries']}
    eb = {key(e): e for e in b['entries']}
    diff, example = collections.Counter(), {}
    for k in sorted(set(ea) & set(eb)):
        x, y = ea[k], eb[k]
        if (x['body'], x['body_bits']) != (y['body'], y['body_bits']):
            diff[k[:2]] += 1
            example.setdefault(k[:2], (k, x['body_bits'], y['body_bits']))
    return sorted(set(ea) - set(eb)), sorted(set(eb) - set(ea)), diff, example


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('command', choices=('summary', 'compare', 'values'))
    ap.add_argument('dump', type=Path, nargs='+')
    ap.add_argument('--group', type=lambda v: int(v, 0))
    ap.add_argument('--kind', type=lambda v: int(v, 0), help='values: the row kind (1 = squads)')
    ap.add_argument('--tag', type=lambda v: int(v, 0), help='values: the value tag (0x8080071E = squads)')
    a = ap.parse_args()
    fmt = lambda k: f'{k[0]:#010x}/{k[1]:#x}/{k[2]}' if len(k) == 3 else f'{k[0]:#010x}/{k[1]:#x}'
    if a.command == 'summary':
        for r, d in load(a.dump[0]):
            if d is None:
                print(f"{r.get('source', 'bridge'):6} (refused) {len(r['frame']) // 2} B")
                continue
            print(f"{r.get('source', 'bridge'):6} stamp {d['stamp']:6} {'init' if d['init'] else '    '} "
                  f"{len(d.get('entries', [])):4} entries {d.get('bits', 0):7} bits authority {d.get('authority')}")
        return
    if a.command == 'values':
        if a.kind is None or a.tag is None:
            ap.error('values needs --kind and --tag')
        rows = [(i, d['stamp'], (int(str(e['hash']), 0), e['sub']), e['body'])
                for i, (r, d) in enumerate(load(a.dump[0])) if d
                for e in d.get('entries', [])
                if e['i8'] == a.kind and (a.group is None or int(str(e['hash']), 0) == a.group)]
        values = srcp.sensor_values(a.tag, [r[3] for r in rows])
        last = {}
        for i, stamp, (g, sub), b in rows:
            image = (values.get(b) or {}).get('image')
            if image is not None and last.get((g, sub)) != image:
                last[(g, sub)] = image
                print(f'frame {i:5} stamp {stamp:6} {g:#010x}/{a.kind:#x}/{sub} {image}')
        return
    if len(a.dump) != 2 or a.group is None:
        ap.error('compare needs two dumps and --group')
    (ra, da), (rb, db) = (first_full_naming(load(p), a.group) for p in a.dump)
    if da is None or db is None:
        sys.exit('no full list naming that group in one of the dumps')
    print(f"A stamp {da['stamp']} ({len(da['entries'])} entries), B stamp {db['stamp']} ({len(db['entries'])} entries)")
    only_a, only_b, diff, example = compare(da, db)
    print('only A:', ' '.join(map(fmt, only_a)) or '-')
    print('only B:', ' '.join(map(fmt, only_b)) or '-')
    for gk, n in sorted(diff.items()):
        k, bits_a, bits_b = example[gk]
        print(f'{fmt(gk)}: {n} bodies differ (e.g. {fmt(k)}: {bits_a} bits vs {bits_b})')


if __name__ == '__main__':
    main()
