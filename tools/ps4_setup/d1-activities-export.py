# SPDX-License-Identifier: GPL-2.0-or-later
"""Export Bungie's final Destiny 1 manifest's strikes, raids, arenas and Crucible activities.

    d1-activities-export.py [--dim DIR] [--world SQLITE] [--out ps4/fixtures/56578/activities/catalog.json]

Two local inputs, both Bungie's own public data and neither kept in the repository:

- `--dim`: the final D1 manifest as DIM ships it (`d1-manifest-base.json.gz` and
  `d1-manifest-strings-en.json.gz`; default `F:/d1/third-party/trawl-2026-11-05/data/dim-d1-manifest`).
  It is the source of every activity row: hash, name, type, level, light, party sizes,
  matchmaking, playlist flag, skulls and rewards.
- `--world`: the mobile world database (`manifest/data/world_en.sqlite` of the workspace) for what
  DIM's copy leaves out: activity bundles (which activities are one strike), destinations, places
  and the stats modes. Its activity names fill the few DIM's strings lack.

The output is `catalog.json`, read by `d1_activity_host::encounter_framework::Catalog`. It holds
only identifiers, numbers and Bungie's English names; the framework definitions beside it say
which encounters each activity has and where every value came from.
"""
import argparse
import gzip
import json
import sqlite3
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import d1paths  # noqa: E402

DIM = Path('F:/d1/third-party/trawl-2026-11-05/data/dim-d1-manifest')

# Activity type identifier -> catalog category. The types with no identifier are told apart by
# their English name (below) and destination.
TYPE_CATEGORY = {
    'ACTIVITY_TYPE_STRIKE': 'strike',
    'ACTIVITY_TYPE_STRIKE_PLAYLIST': 'strike_playlist',
    'STRIKE_WEEKLY': 'weekly_heroic',
    'ACTIVITY_TYPE_NIGHTFALL': 'nightfall',
    'ACTIVITY_TYPE_RAID': 'raid',
    'RAID_MOON1': 'raid',
    'ARENA': 'arena',
    'ARENA_CHALLENGE': 'arena_challenge',
}
CRUCIBLE_DESTINATION = 'The Crucible'


def load_dim(path):
    base = json.load(gzip.open(path / 'd1-manifest-base.json.gz'))
    strings = json.load(gzip.open(path / 'd1-manifest-strings-en.json.gz'))
    return base, strings


def load_world(path):
    c = sqlite3.connect(path)

    def table(name, key):
        return {json.loads(j)[key]: json.loads(j) for (j,) in c.execute(f'select json from {name}')}
    return {
        'activities': table('DestinyActivityDefinition', 'activityHash'),
        'bundles': table('DestinyActivityBundleDefinition', 'bundleHash'),
        'destinations': table('DestinyDestinationDefinition', 'destinationHash'),
        'places': table('DestinyPlaceDefinition', 'placeHash'),
        'modes': table('DestinyActivityModeDefinition', 'hash'),
    }


def category(type_ident, type_name, dest_name, row, name):
    if type_ident in TYPE_CATEGORY:
        return TYPE_CATEGORY[type_ident]
    if dest_name == CRUCIBLE_DESTINATION:
        # The maps are the Rumble-typed rows that are neither playlists nor matchmade.
        if type_name == 'Rumble' and not row['isPlaylist'] and not row['isMatchmade']:
            return 'crucible_map'
        return 'crucible_playlist'
    if type_name == 'Strike':
        return 'strike'
    # Archon's Forge is a public event inside this patrol, not an activity of its own.
    if type_ident == 'ACTIVITY_TYPE_EXPLORE' and name and 'Plaguelands' in name:
        return 'patrol_event'
    return None


# Strike playlist rows and the member rows they draw from. The manifest lists no members, so they
# are inferred and labelled: the per-strike rows typed as the playlist at the playlist's level.
PLAYLIST_LEVEL = {'Vanguard Legacy': 20, 'Taken War': 36, 'Taken War Heroic': 41}
GENERIC = {'Vanguard Legacy', 'Taken War', 'Taken War Heroic', 'SIVA Crisis', 'SIVA Crisis Heroic',
           'Vanguard Nightfall', 'Weekly Heroic Strike', 'Weekly Nightfall Strike'}


def playlists(rows):
    named = [r for r in rows if r['name'] not in GENERIC]
    out = []
    for r in rows:
        if r['name'] not in GENERIC:
            continue
        n = r['name']
        if n in PLAYLIST_LEVEL:
            members = [m['hash'] for m in named if m['category'] == 'strike_playlist' and m['level'] == PLAYLIST_LEVEL[n]]
            how = f"inferred: the per-strike '{n}'-typed rows at level {PLAYLIST_LEVEL[n]}"
        elif n.startswith('SIVA Crisis'):
            members = [m['hash'] for m in named if m['category'] == 'strike' and m['level'] == 42 and m['matchmade']
                       and not m['playlist']]
            how = 'inferred: the per-strike matchmade strike rows at level 42'
        elif n == 'Vanguard Nightfall':
            members = [m['hash'] for m in named if m['category'] == 'nightfall' and not m['playlist']]
            how = 'inferred: the per-strike Nightfall rows'
        else:
            members = []
            how = "needs evidence: the year-one weekly rows name no strike, and the week's strike is not in the manifest"
        out.append({'hash': r['hash'], 'name': n, 'category': r['category'], 'members': members,
                    'membership': {'kind': 'needs-evidence' if not members else 'inferred', 'source': how}})
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--dim', type=Path, default=DIM)
    ap.add_argument('--world', type=Path,
                    default=Path(d1paths.workspace()) / 'manifest' / 'data' / 'world_en.sqlite')
    ap.add_argument('--out', type=Path,
                    default=HERE.parents[1] / 'ps4' / 'fixtures' / '56578' / 'activities' / 'catalog.json')
    a = ap.parse_args()
    base, strings = load_dim(a.dim)
    world = load_world(a.world)
    acts, s_acts = base['DestinyActivityDefinition'], strings['DestinyActivityDefinition']
    types, s_types = base['DestinyActivityTypeDefinition'], strings['DestinyActivityTypeDefinition']
    skull_defs = base['DestinyScriptedSkullDefinition']
    s_skulls = strings['DestinyScriptedSkullDefinition']

    bundle_of = {}
    for b in world['bundles'].values():
        for h in b.get('activityHashes', []):
            bundle_of.setdefault(h, b['bundleHash'])

    out, used_bundles = [], set()
    for key, x in sorted(acts.items(), key=lambda kv: int(kv[0])):
        h = int(key)
        t = types.get(str(x['activityTypeHash']), {})
        type_ident = t.get('identifier')
        type_name = s_types.get(str(x['activityTypeHash']), {}).get('activityTypeName')
        w = world['activities'].get(h, {})
        dest = world['destinations'].get(x.get('destinationHash'), {})
        place = world['places'].get(x.get('placeHash'), {})
        name = s_acts.get(key, {}).get('activityName') or w.get('activityName')
        cat = category(type_ident, type_name, dest.get('destinationName'), x, name)
        if cat is None:
            continue
        skull_names = [s.get('displayName') for s in s_acts.get(key, {}).get('skulls', [])]
        bundle = bundle_of.get(h)
        if bundle:
            used_bundles.add(bundle)
        out.append({
            'hash': h,
            'name': name,
            'category': cat,
            'type_hash': x['activityTypeHash'],
            'type_identifier': type_ident,
            'type_name': type_name,
            'bundle_hash': bundle,
            'destination': dest.get('destinationName'),
            'place': place.get('placeName'),
            'level': x['activityLevel'],
            'light': x.get('activityPower', 0),
            'tier': x['tier'],
            'min_party': x['minParty'],
            'max_party': x['maxParty'],
            'max_players': x['maxPlayers'],
            'matchmade': x['isMatchmade'],
            'playlist': x['isPlaylist'],
            'skulls': skull_names,
            'rewards': [[r['itemHash'] for r in g.get('rewardItems', [])] for g in x.get('rewards', [])],
        })
    skulls = [{'hash': int(k), 'identifier': v.get('identifier'), 'name': s_skulls.get(k, {}).get('skullName'),
               'description': s_skulls.get(k, {}).get('description')}
              for k, v in sorted(skull_defs.items(), key=lambda kv: int(kv[0]))]
    bundles = [{'hash': b, 'name': world['bundles'][b].get('activityName'),
                'activity_hashes': world['bundles'][b].get('activityHashes', [])}
               for b in sorted(used_bundles)]
    modes = [{'hash': m['hash'], 'name': m.get('modeName'), 'mode_type': m.get('modeType'),
              'friendly_name': m.get('friendlyName'), 'category': m.get('activityModeCategory'),
              'aggregate': m.get('isAggregateMode', False)}
             for m in sorted(world['modes'].values(), key=lambda m: m.get('modeType', 0))]
    doc = {
        'source': {
            'activities': "Bungie's final D1 manifest (DIM's copy: DestinyActivityDefinition, "
                          'DestinyActivityTypeDefinition, DestinyScriptedSkullDefinition)',
            'bundles_destinations_modes': 'Bungie mobile world database (world_en.sqlite)',
            'tool': 'tools/ps4_setup/d1-activities-export.py',
        },
        'activities': out,
        'playlists': playlists(out),
        'bundles': bundles,
        'skulls': skulls,
        'modes': modes,
    }
    a.out.parent.mkdir(parents=True, exist_ok=True)
    a.out.write_text(json.dumps(doc, indent=1, ensure_ascii=False) + '\n', encoding='utf-8')
    counts = {}
    for r in out:
        counts[r['category']] = counts.get(r['category'], 0) + 1
    print(f'{len(out)} activities -> {a.out}: ' + ', '.join(f'{k} {v}' for k, v in sorted(counts.items())))


if __name__ == '__main__':
    main()
