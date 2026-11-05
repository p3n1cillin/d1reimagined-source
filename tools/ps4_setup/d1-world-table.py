# SPDX-License-Identifier: GPL-2.0-or-later
"""Generate build 56578's world tables, for every activity, from the client's own content.

    d1-world-table.py [--worlds-json PRIVATE.json | --packages DIR] [--hosts HOSTS.tsv ...]
                      [--legacy-table activities.csv] [--out DIR] [--coverage-md FILE]
    d1-world-table.py --sensor-list WORLD:SLICE_SET     (a sensor-client list from the committed tables)

Inputs, all local:
  - the worlds JSON `d1-worlds.mjs` writes from the owned packages (`--packages` runs it; game data,
    kept outside the repository): every world's scenario record, slice sets, bundles and insertion
    points;
  - the client's activity table (`ps4/fixtures/56578/characters/activity-table.tsv`, index -> hash);
  - Bungie's final D1 manifest (`world_en.sqlite`, through d1paths): each activity's name, type,
    destination and whether it is a playlist;
  - Blam Network's 2014 activity table (build 41375, `activities.csv` in tiger's dictionary, a local
    external input): the host-side activity -> world binding the 56578 client does not carry;
  - optional `d1-srcp.py hosts` TSVs from the live captures (private): the world each live host
    actually served, and the slice set its client started in.

Outputs, identifiers only (hashes, indexes, names; no game bytes), in `ps4/fixtures/56578/worlds/`:
  worlds.tsv            every world's scenario facts (the activity host's rung-33 values)
  slice-sets.tsv        every slice set: region, public flag, static world bubble and bundle tags
  bundles.tsv           every bundle tag's sensor clients per lookup bubble
  insertion-points.tsv  spawn point name hashes per static world bubble
  activities.tsv        every activity index: its world, start slice set, sensor group and evidence

The activity host reads them with `D1_AH_WORLDS=<folder>` (crates/d1-activity-host/src/generated.rs).
The format is documented in ps4/fixtures/56578/README.md and notes/ps4-56578-worlds-2026-11-05.md.
"""
import argparse
import collections
import csv
import json
import os
import sqlite3
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import d1paths  # noqa: E402  sibling: local paths from D1_ROOT or paths.json

BUILD = '56578.17.04.12.1251.evil'
LIFETIME_TYPE = 0x10
DEFAULT_INSERTION = 0x2EA8FB98  # fnv1("default"): the spawn the client picks with no transition name
LEGACY_TABLE = 'packages/dictionary/destiny1/v41375_14_07_03_1501_live/activities.csv'
CINEMATIC_TYPES = {'STORY_CINEMATIC', 'CREDITS'}
STATUS = ('cross-checked', 'generated', 'missing evidence')

WORLDS_COLUMNS = 'world\tscenario_tag\tfamily\tprivate_checksum\tpublic_checksum\tbubble_count\tchosen\tpublic_slice_sets\tdestination\tstatic_world'
SLICE_COLUMNS = 'world\tslice_set\tbubble\tvalid\tpublic\tregion_hash\tdescriptor\tstatic_world\tstatic_bubble\tglobal\tauthoritative\tprivate'
BUNDLE_COLUMNS = 'bundle_tag\tgroup\tlookup_bubble\tclients'
INSERTION_COLUMNS = 'static_world\tbubble\tinsertion_points'
ACTIVITY_COLUMNS = ('index\thash\tname\ttype\tworld\tstart_region\tstart_source\tstart_public\tprivate_start\t'
                    'sensor_group\tstatus\tevidence')


def fnv1(s):
    h = 0x811C9DC5
    for b in s.encode('utf-8'):
        h = ((h * 0x01000193) & 0xFFFFFFFF) ^ b
    return h


def h32(v):
    return int(v, 16) if isinstance(v, str) else v


def hx(v):
    return f'{h32(v):#010x}'


def is_cinematic_world(name):
    return name.startswith(('cine_', 'cinematic_', 'act_cinematic_'))


# ---------------------------------------------------------------- inputs

def read_activity_table(path):
    rows = []
    for line in Path(path).read_text().splitlines()[1:]:
        if line.strip():
            i, h = line.split('\t')[:2]
            rows.append((int(i), int(h, 16)))
    return rows


def read_manifest(path):
    """{activity hash: {name, description, type, destination, playlist}} from world_en.sqlite."""
    if not path or not Path(path).exists():
        return {}
    db = sqlite3.connect(f'file:{Path(path).as_posix()}?mode=ro', uri=True)

    def table(name, key):
        return {json.loads(j)[key] & 0xFFFFFFFF: json.loads(j) for (j,) in db.execute(f'select json from {name}')}
    acts = table('DestinyActivityDefinition', 'activityHash')
    types = table('DestinyActivityTypeDefinition', 'activityTypeHash')
    dests = table('DestinyDestinationDefinition', 'destinationHash')
    out = {}
    for h, a in acts.items():
        t = types.get(a.get('activityTypeHash', 0) & 0xFFFFFFFF, {})
        out[h] = {'name': a.get('activityName', ''), 'description': a.get('activityDescription', ''),
                  'type': t.get('identifier') or t.get('activityTypeName') or '',
                  'destination': dests.get(a.get('destinationHash', 0) & 0xFFFFFFFF, {}).get('destinationName', ''),
                  'playlist': bool(a.get('isPlaylist'))}
    return out


def read_legacy(path):
    """Blam Network's 2014 activity table: [{hash, codename, name, description, start_bubble}]."""
    if not path or not Path(path).exists():
        return []
    out = []
    with open(path, encoding='utf-8', newline='') as f:
        for r in csv.DictReader(f):
            if r.get('codename'):
                out.append({'hash': int(r['activity_hash'], 16) if r.get('activity_hash') else None,
                            'codename': r['codename'], 'name': r.get('name', ''),
                            'description': r.get('description', ''),
                            'start_bubble': int(r['start_bubble']) if (r.get('start_bubble') or '').isdigit() else None})
    return out


def read_hosts(paths):
    """{activity hash: [(capture, start slice set or None, {group: records})]} from `d1-srcp.py hosts`."""
    out = collections.defaultdict(list)
    for p in paths:
        lines = Path(p).read_text().splitlines()
        if not lines:
            continue
        cols = lines[0].split('\t')
        for line in lines[1:]:
            r = dict(zip(cols, line.split('\t')))
            if not r.get('activity_hash'):
                continue
            groups = {}
            for tok in (r.get('groups') or '').split():
                if ':' in tok:
                    g, n = tok.split(':')
                    groups[int(g, 16)] = int(n)
            start = int(r['start']) if r.get('start', '-').isdigit() else None
            out[int(r['activity_hash'], 16)].append((r.get('capture', ''), start, groups))
    return out


# ---------------------------------------------------------------- content model

class Content:
    """The worlds JSON, indexed."""

    def __init__(self, data):
        if data.get('build') not in (None, BUILD):
            raise SystemExit(f"worlds JSON is for build {data.get('build')}, not {BUILD}")
        self.worlds = {w['name']: w for w in data['worlds']}
        self.bundles = data['bundles']
        self.insertion = data.get('insertion_points', {})
        self.problems = data.get('problems', [])
        self.world_groups = {}
        self.group_worlds = collections.defaultdict(set)
        for name, w in self.worlds.items():
            gs = set()
            for st in self.states(w):
                for tag in self.state_bundles(st):
                    gs.add(h32(self.bundles[tag]['bundle']))
            self.world_groups[name] = gs
            for g in gs:
                self.group_worlds[g].add(name)

    @staticmethod
    def states(w):
        for b in w['bubbles']:
            yield from b['states']

    def state(self, world, slice_set):
        w = self.worlds[world]
        b, s = divmod(slice_set, 8)
        if b < len(w['bubbles']) and s < len(w['bubbles'][b]['states']):
            return w['bubbles'][b]['states'][s]
        return None

    def state_bundles(self, st):
        return [t for t in st.get('global', []) + st.get('authoritative', []) + st.get('private', [])
                if self.bundles.get(t)]

    def chosen(self, world):
        """Bubbles that may be marked chosen: those with a valid state (scenario sub-bubble flag)."""
        return [i for i, b in enumerate(self.worlds[world]['bubbles']) if any(s['valid'] for s in b['states'])]

    def valid_sets(self, world):
        return [s['slice_set'] for s in self.states(self.worlds[world]) if s['valid']]

    def bundle_clients(self, tag):
        """[(lookup bubble, [(type, index, has_a, has_b)])], merged per bubble, lookups' order kept."""
        merged = collections.OrderedDict()
        for lg in self.bundles[tag]['lookups']:
            rows = merged.setdefault(lg['slice_set'], [])
            for idx, ty, a, b, _auth, _sset in lg['rows']:
                c = (ty, idx, int(a is not None), int(b is not None))
                if (c[2] or c[3]) and c not in rows:
                    rows.append(c)
        return [(k, v) for k, v in merged.items() if v]

    def sensor_clients(self, world, slice_set):
        """The sensor clients the client registers when `slice_set` loads: every row of the slice
        set's bundles whose lookup group names its bubble or every bubble (-1), with an A or B value
        type. [(group, type, index, has_a, has_b)]: reproduces the client's own lists (see tests)."""
        st = self.state(world, slice_set)
        if st is None:
            return []
        bubble = slice_set // 8
        out, seen = [], set()
        for tag in self.state_bundles(st):
            g = h32(self.bundles[tag]['bundle'])
            for lb, rows in self.bundle_clients(tag):
                if lb not in (-1, bubble):
                    continue
                for ty, idx, a, b in rows:
                    if (g, ty, idx) not in seen:
                        seen.add((g, ty, idx))
                        out.append((g, ty, idx, a, b))
        return out

    def sensor_group(self, world, slice_set):
        """The group holding the lifetime record (type 0x10) in the start slice set's clients."""
        for g, ty, _idx, _a, _b in self.sensor_clients(world, slice_set):
            if ty == LIFETIME_TYPE:
                return g
        return None

    def first_private(self, world):
        for s in self.states(self.worlds[world]):
            if s['valid'] and not s['public']:
                return s['slice_set']
        return None

    def capture_world(self, groups, start):
        """The world a live host served: the one whose bundles explain most of its sensor groups,
        each group weighted by how few worlds have it. None unless one world wins outright and has
        the client's start slice set."""
        scores = collections.Counter()
        for g in groups:
            ws = self.group_worlds.get(g, ())
            for w in ws:
                scores[w] += 1.0 / len(ws)
        ranked = scores.most_common()
        if not ranked:
            return None, 0.0
        top = [w for w, s in ranked if abs(s - ranked[0][1]) < 1e-9]
        if len(top) != 1:
            return None, ranked[0][1]
        w = top[0]
        if start is not None and start not in self.valid_sets(w):
            return None, ranked[0][1]
        return w, ranked[0][1]


# ---------------------------------------------------------------- binding

def legacy_candidates(legacy, worlds, ahash, meta):
    """(worlds, how) from the 2014 table: same activity hash, else same name and description,
    else same name; only worlds 56578 has. Cinematic activity types prefer cinematic worlds."""
    for how, match in (('2014 table, same hash', lambda r: r['hash'] == ahash),
                       ('2014 table, same name and description',
                        lambda r: meta and meta['name'] and r['name'] == meta['name']
                        and r['description'] == meta['description']),
                       ('2014 table, same name', lambda r: meta and meta['name'] and r['name'] == meta['name'])):
        rows = [r for r in legacy if match(r) and r['codename'] in worlds]
        names = sorted({r['codename'] for r in rows})
        if not names:
            continue
        if len(names) > 1 and meta:
            cine = meta['type'] in CINEMATIC_TYPES
            pick = [n for n in names if is_cinematic_world(n) == cine]
            if pick:
                names = pick
        starts = {r['start_bubble'] for r in rows if r['codename'] in names}
        return names, how, (starts.pop() if len(starts) == 1 else None)
    return [], None, None


def bind(content, activities, manifest, legacy, hosts):
    out = []
    for index, ahash in activities:
        meta = manifest.get(ahash)
        names, how, legacy_bubble = legacy_candidates(legacy, content.worlds, ahash, meta)
        cap_world, cap_start, cap_label = None, None, None
        for label, start, groups in hosts.get(ahash, []):
            w, _ = content.capture_world(groups, start)
            if w:
                cap_world, cap_start, cap_label = w, start, label
                break
        row = {'index': index, 'hash': ahash, 'name': meta['name'] if meta else '',
               'type': meta['type'] if meta else '', 'world': None, 'start_region': None,
               'start_source': '-', 'status': 'missing evidence', 'evidence': ''}
        if cap_world:
            row['world'] = cap_world
            row['status'] = 'cross-checked'
            ev = f'live capture {cap_label}'
            if names and cap_world in names:
                ev += f'; {how}'
            elif names:
                ev += f'; {how} said {"/".join(names)}'
            row['evidence'] = ev
        elif len(names) == 1:
            row['world'] = names[0]
            row['status'] = 'generated'
            row['evidence'] = how
        elif names:
            row['evidence'] = f'{how}: ambiguous {"/".join(names)}'
        elif meta and meta['playlist']:
            row['evidence'] = 'playlist: launches another activity'
        elif meta and not meta['name']:
            row['evidence'] = 'unnamed in the manifest'
        else:
            row['evidence'] = 'no binding in the 2014 table or the captures'
        row['_start'] = (cap_start if cap_world else None, 'capture', legacy_bubble)
        out.append(row)
    siblings(out, manifest)
    seen = {}
    for row in out:
        meta = manifest.get(row['hash'])
        if row['status'] == 'cross-checked' and row['_start'][0] is not None and meta and meta['name']:
            seen.setdefault((meta['name'], meta['destination'], row['world']), (row['index'], row['_start'][0]))
    for row in out:
        cap_start, cap_source, legacy_bubble = row.pop('_start')
        w = row['world']
        meta = manifest.get(row['hash'])
        twin = seen.get((meta['name'], meta['destination'], w)) if meta and w else None
        if cap_start is None and twin is not None and twin[0] != row['index']:
            cap_start, cap_source = twin[1], f'capture of {twin[0]}'
        if w:
            if cap_start is not None:
                row['start_region'], row['start_source'] = cap_start, cap_source
            elif legacy_bubble is not None and any(s // 8 == legacy_bubble for s in content.valid_sets(w)):
                row['start_region'] = min(s for s in content.valid_sets(w) if s // 8 == legacy_bubble)
                row['start_source'] = '2014 start bubble'
            else:
                sets = content.valid_sets(w)
                private = content.first_private(w)
                if len(sets) == 1:
                    row['start_region'], row['start_source'] = sets[0], 'only slice set'
                elif private is not None:
                    row['start_region'], row['start_source'] = private, 'stand-in: first private slice set'
    return out


def siblings(rows, manifest):
    """An unbound activity takes the world of the bound activities with its name and destination
    (a mission's heroic and playlist copies, a strike's nightfall), when one of them was seen on a
    live host and none disagrees. Generic names that rotate between worlds ("Weekly Nightfall
    Strike") are never seen whole on one host, so they stay unbound. Status `generated`: the copy
    itself was never seen."""
    by_key = collections.defaultdict(list)
    for r in rows:
        meta = manifest.get(r['hash'])
        if r['world'] and meta and meta['name'] and r['status'] != 'missing evidence':
            by_key[(meta['name'], meta['destination'])].append(r)
    for r in rows:
        meta = manifest.get(r['hash'])
        if r['world'] or not meta or not meta['name'] or meta['playlist']:
            continue
        known = by_key.get((meta['name'], meta['destination']), [])
        worlds = {k['world'] for k in known}
        if len(worlds) != 1 or not any(k['status'] == 'cross-checked' for k in known):
            continue
        src = min(known, key=lambda k: (k['status'] != 'cross-checked', k['index']))
        r['world'] = src['world']
        r['status'] = 'generated'
        r['evidence'] = f"same name and destination as {src['index']} ({src['status']})"
        if src['_start'][0] is not None:
            # The live host's start for the activity it was seen on, not for this copy.
            r['_start'] = (src['_start'][0], f"capture of {src['index']}", None)
        else:
            r['_start'] = (None, None, src['_start'][2])


# ---------------------------------------------------------------- outputs

def tsv(path, header, rows):
    text = f'# GENERATED by tools/ps4_setup/d1-world-table.py from build {BUILD}; do not edit\n{header}\n'
    text += ''.join('\t'.join(str(c) for c in r) + '\n' for r in rows)
    Path(path).write_text(text, encoding='utf-8', newline='\n')


def join(items, sep=','):
    items = [str(i) for i in items]
    return sep.join(items) if items else '-'


def write_tables(content, bound, out):
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    wrows = []
    for name in sorted(content.worlds):
        w = content.worlds[name]
        pub = [s['slice_set'] for s in content.states(w) if s['public']]
        statics = sorted({s.get('world_hash') for s in content.states(w) if s.get('world_hash')})
        wrows.append((name, w['scenario_tag'], w['family'], w['private'], w['public'], len(w['bubbles']),
                      join(content.chosen(name)), join(pub), w['destination'] or '-', join(statics)))
    tsv(out / 'worlds.tsv', WORLDS_COLUMNS, wrows)
    srows = []
    for name in sorted(content.worlds):
        for s in content.states(content.worlds[name]):
            srows.append((name, s['slice_set'], s['slice_set'] // 8, s['valid'], int(s['public']), s['region_name_hash'],
                          s['descriptor'], s.get('world_hash', '-'), s.get('bubble_index', '-'),
                          join(s.get('global', [])), join(s.get('authoritative', [])), join(s.get('private', []))))
    tsv(out / 'slice-sets.tsv', SLICE_COLUMNS, srows)
    brows = []
    for tag in sorted(content.bundles, key=h32):
        if not content.bundles[tag]:
            continue
        for lb, rows in content.bundle_clients(tag):
            brows.append((tag, content.bundles[tag]['bundle'], lb, ' '.join(f'{t}:{i}:{a}{b}' for t, i, a, b in rows)))
    tsv(out / 'bundles.tsv', BUNDLE_COLUMNS, brows)
    irows = []
    for wh in sorted(content.insertion, key=h32):
        for bi, pts in enumerate(content.insertion[wh]):
            irows.append((wh, bi, join(f'{h}:{n}' for h, n in pts) if pts is not None else '?'))
    tsv(out / 'insertion-points.tsv', INSERTION_COLUMNS, irows)
    arows = []
    for r in bound:
        w, s = r['world'], r['start_region']
        st = content.state(w, s) if w and s is not None else None
        group = content.sensor_group(w, s) if st else None
        private = content.first_private(w) if w else None
        arows.append((r['index'], hx(r['hash']), r['name'] or '-', r['type'] or '-', w or '-',
                      s if s is not None else '-', r['start_source'],
                      int(st['public']) if st else '-', private if private is not None else '-',
                      hx(group) if group is not None else '-', r['status'], r['evidence'] or '-'))
    tsv(out / 'activities.tsv', ACTIVITY_COLUMNS, arows)
    return arows


class Tables:
    """The committed tables, read back: what the activity host's loader reads (generated.rs)."""

    def __init__(self, folder):
        folder = Path(folder)

        def rows(name):
            lines = [x for x in (folder / name).read_text(encoding='utf-8').splitlines() if x and not x.startswith('#')]
            cols = lines[0].split('\t')
            return [dict(zip(cols, x.split('\t'))) for x in lines[1:]]
        self.worlds = {r['world']: r for r in rows('worlds.tsv')}
        self.slices = {(r['world'], int(r['slice_set'])): r for r in rows('slice-sets.tsv')}
        self.bundles = collections.defaultdict(list)
        for r in rows('bundles.tsv'):
            clients = []
            for tok in r['clients'].split():
                t, i, ab = tok.split(':')
                clients.append((int(t), int(i), int(ab[0]), int(ab[1])))
            self.bundles[r['bundle_tag']].append((h32(r['group']), int(r['lookup_bubble']), clients))
        self.activities = {int(r['index']): r for r in rows('activities.tsv')}

    def sensor_clients(self, world, slice_set):
        s = self.slices.get((world, slice_set))
        if s is None:
            return []
        out, seen = [], set()
        for kind in ('global', 'authoritative', 'private'):
            for tag in ([] if s[kind] == '-' else s[kind].split(',')):
                for g, lb, clients in self.bundles.get(tag, []):
                    if lb not in (-1, slice_set // 8):
                        continue
                    for t, i, a, b in clients:
                        if (g, t, i) not in seen:
                            seen.add((g, t, i))
                            out.append((g, t, i, a, b))
        return out

    def sensor_list(self, world, slice_set):
        """A sensor-client list file (`hash i8 sub has_a has_b` per line), as D1_AH_WORLD_TABLE and
        the mission steps read, with default values."""
        head = f'# {world} slice set {slice_set}: generated from build {BUILD} content (d1-world-table.py)\n'
        return head + ''.join(f'{g:#010x} {t:#x} {i} {a} {b}\n' for g, t, i, a, b in self.sensor_clients(world, slice_set))


def coverage(bound):
    c = collections.Counter(r['status'] for r in bound)
    return {s: c.get(s, 0) for s in STATUS}


def coverage_markdown(bound):
    c = coverage(bound)
    lines = [f'{len(bound)} activities: ' + ', '.join(f'**{c[s]} {s}**' for s in STATUS) + '.', '',
             '| Index | Activity | Type | World | Start | Status | Evidence |', '|---:|---|---|---|---:|---|---|']
    for r in bound:
        start = f"{r['start_region']} ({r['start_source']})" if r['start_region'] is not None else '-'
        lines.append(f"| {r['index']} | {r['name'] or '-'} | {r['type'] or '-'} | {r['world'] or '-'} | {start} | "
                     f"{r['status']} | {r['evidence'] or '-'} |")
    return '\n'.join(lines) + '\n'


def run_node(packages, out):
    node = os.environ.get('D1_NODE', 'node')
    r = subprocess.run([node, str(HERE / 'd1-worlds.mjs'), str(packages), str(out)], capture_output=True, text=True)
    if r.returncode:
        raise SystemExit(f'd1-worlds.mjs failed: {(r.stderr or r.stdout).strip()[-400:]}')
    print(r.stderr.strip(), file=sys.stderr)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    src = ap.add_mutually_exclusive_group()
    src.add_argument('--worlds-json', type=Path, help='d1-worlds.mjs output (private)')
    src.add_argument('--packages', type=Path, help='owned 56578 packages: run d1-worlds.mjs on them')
    ap.add_argument('--activity-table', type=Path, default=None)
    ap.add_argument('--manifest', type=Path, default=None)
    ap.add_argument('--legacy-table', type=Path, default=None, help="Blam Network's 2014 activities.csv")
    ap.add_argument('--hosts', type=Path, action='append', default=[], help='d1-srcp.py hosts TSV (private)')
    ap.add_argument('--out', type=Path, default=None)
    ap.add_argument('--coverage-md', type=Path)
    ap.add_argument('--sensor-list', metavar='WORLD:SLICE_SET',
                    help='print the sensor-client list the committed tables give a slice set, and stop')
    a = ap.parse_args(argv)
    fixtures = d1paths.fixtures()
    a.activity_table = a.activity_table or fixtures / 'characters' / 'activity-table.tsv'
    a.out = a.out or fixtures / 'worlds'
    if a.sensor_list:
        world, _, slice_set = a.sensor_list.rpartition(':')
        t = Tables(a.out)
        if (world, int(slice_set)) not in t.slices:
            raise SystemExit(f'{world} has no slice set {slice_set} in {a.out}')
        sys.stdout.write(t.sensor_list(world, int(slice_set)))
        return
    if a.manifest is None:
        a.manifest = Path(d1paths.workspace()) / 'manifest' / 'data' / 'world_en.sqlite'
    if a.legacy_table is None:
        a.legacy_table = Path(os.environ.get('D1_TIGER', 'F:/d1/third-party/Blam-Network_tiger')) / LEGACY_TABLE
    with tempfile.TemporaryDirectory() as tmp:
        path = a.worlds_json
        if path is None:
            packages = a.packages or (Path(d1paths.workspace()) / 'dumps' / 'later-56578-CUSA00219' / 'CUSA00219' / 'packages')
            if not packages.exists():
                raise SystemExit(f'no packages at {packages}: pass --packages or --worlds-json')
            path = Path(tmp) / 'worlds.json'
            run_node(packages, path)
        content = Content(json.loads(Path(path).read_text(encoding='utf-8')))
    manifest = read_manifest(a.manifest)
    if not manifest:
        print(f'warning: no manifest at {a.manifest}; activities will be unnamed', file=sys.stderr)
    legacy = read_legacy(a.legacy_table)
    if not legacy:
        print(f'warning: no 2014 activity table at {a.legacy_table}', file=sys.stderr)
    bound = bind(content, read_activity_table(a.activity_table), manifest, legacy, read_hosts(a.hosts))
    write_tables(content, bound, a.out)
    if a.coverage_md:
        a.coverage_md.write_text(coverage_markdown(bound), encoding='utf-8', newline='\n')
    c = coverage(bound)
    print(f'{len(content.worlds)} worlds, {len(bound)} activities: ' + ', '.join(f'{c[s]} {s}' for s in STATUS)
          + f' -> {a.out}', file=sys.stderr)


if __name__ == '__main__':
    main()
