# SPDX-License-Identifier: GPL-2.0-or-later
"""Write the activity framework's definitions: every strike, raid and arena, and the Crucible.

    d1-activities-defs.py [--scripts DIR] [--community FILE] [--dir ps4/fixtures/56578/activities]

Reads, beside `--dir`:
- `catalog.json` (tools/ps4_setup/d1-activities-export.py: Bungie's manifest);
- `sequences/*.json` (tools/ps4_setup/d1-activity-sequence.py: the live captures);

and two local inputs that never enter the repository:
- `--scripts`: Blam Network's restored 56578 scripts (Fukugen Daihon, unlicensed). Only
  identifiers are taken: each strike root's `@activity`, `@activity_bundle`, phase row names in
  lifetime order, the region each `@phase` names and which phase files carry `@hard_wipe()`. No
  script text. Without it the script blocks are left out.
- `--community`: the Cryptarch guide's `activityPageData.json` (nruble/cryptarch-guide, GPL-3.0).
  Only its raid loot tables are used (section titles and item hashes), labelled `community`.

Writes `definitions/strikes.json`, `definitions/raids.json`, `definitions/arenas.json` and
`crucible.json`. What is not the game's own data is labelled where it is written (the KNOWLEDGE
tables below), and `needs_evidence` lists what is missing. The note
notes/ps4-56578-strikes-raids-crucible-2026-11-05.md explains the sources.
"""
import argparse
import json
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
SCRIPTS = Path('F:/d1/third-party/Blam-Network_tiger/packages/fukugen-daihon/destiny1/v56578_17_04_12_1251_evil')
COMMUNITY = Path('F:/d1/third-party/nruble_cryptarch-guide/public/data/activityPageData.json')
CFG = {'scripts': SCRIPTS}


def fnv1(s):
    h = 0x811C9DC5
    for b in s.encode():
        h = ((h * 0x01000193) & 0xFFFFFFFF) ^ b
    return h


def ev(kind, source=''):
    return {'kind': kind, 'source': source}


COMMUNITY_SRC = 'nruble/cryptarch-guide activityPageData.json (GPL-3.0): a community stand-in'
STAND_IN_SRC = ('placeholder so the framework runs; the authority is the content (the scoreboard '
                "definition's rules, the activity's own tags), not exported yet")

# ---------------------------------------------------------------------------------------------
# Strikes
# ---------------------------------------------------------------------------------------------

# name -> (id, restored script folder under es/ or None, capture sequence or None, boss (D1
# knowledge) or None). The script mapping for the scripted five is checked below against the
# live capture: the PAH serves the root bundle's group, fnv1 of the bundle name.
STRIKES = {
    "The Devils' Lair": ('strike-the-devils-lair', 'cosmo/cosmo_portal1_activity', 'strike-the-devils-lair', 'Sepiks Prime'),
    'The Summoning Pits': ('strike-the-summoning-pits', 'moon/moon_portal_2', 'strike-the-summoning-pits', 'Phogoth, the Untamed'),
    'The Nexus': ('strike-the-nexus', 'venus/strike_1_activity', 'strike-the-nexus', 'Nexus Mind'),
    'The Undying Mind': ('strike-the-undying-mind', 'mars/mars_portal3_activity', 'strike-the-undying-mind', None),
    'The Shadow Thief': ('strike-the-shadow-thief', 'moon/strike_vampire', 'strike-the-shadow-thief', 'Taniks, the Scarred'),
    'The Will of Crota': ('strike-the-will-of-crota', None, 'strike-the-will-of-crota', 'Omnigul, Will of Crota'),
    "Winter's Run": ('strike-winters-run', 'venus/venus_portal_2', 'strike-winters-run', 'Draksis, the Winter Kell'),
    'Cerberus Vae III': ('strike-cerberus-vae-iii', 'mars/mars_portal_2', 'strike-cerberus-vae-iii', "Valus Ta'aurc"),
    'Dust Palace': ('strike-dust-palace', 'mars/mars_portal_1', 'strike-dust-palace', None),
    'Fallen S.A.B.E.R.': ('strike-fallen-saber', None, None, None),
    'Shield Brothers': ('strike-shield-brothers', None, None, "Valus Mau'ual and Valus Tlu'urn"),
    'The Sunless Cell': ('strike-the-sunless-cell', None, None, 'Alak-Hul, the Darkblade'),
    'Blighted Chalice': ('strike-blighted-chalice', None, None, 'Sylok, the Defiled'),
    'Echo Chamber': ('strike-echo-chamber', None, None, None),
    'Sepiks Perfected': ('strike-sepiks-perfected', None, None, 'Sepiks Perfected'),
    'The Wretched Eye': ('strike-the-wretched-eye', None, None, None),
    'The Abomination Heist': ('strike-the-abomination-heist', None, None, None),
}


def read_script(folder):
    """Identifiers of a restored root: @activity, bundle, phases (member order), regions,
    hard-wiped phases, start-only."""
    d = CFG['scripts'] / 'es' / folder
    root = next(p for p in sorted(d.glob('*.es')) if '@activity(' in p.read_text(encoding='utf-8', errors='replace'))
    text = root.read_text(encoding='utf-8', errors='replace')
    act = re.search(r'^@activity\("([^"]+)"', text, re.M).group(1)
    bundle = re.search(r'^@activity_bundle\("([^"]+)"', text, re.M)
    regions = dict(re.findall(r'^@phase\("([^"]+)",\s*"?([^")]+)"?\)', text, re.M))
    stems = {p.stem for p in d.glob('*.esp')}
    phases, wiped = [], set()
    for m in re.finditer(r'^(\w+)\s+(\w+)\s*$', text, re.M):
        if m.group(1) in stems:
            phases.append(m.group(2))
            if '@hard_wipe()' in (d / f'{m.group(1)}.esp').read_text(encoding='utf-8', errors='replace'):
                wiped.add(m.group(2))
    return {'activity': act, 'bundle': bundle.group(1) if bundle else root.stem, 'phases': phases,
            'start_only_stub': not phases}, regions, wiped


def strike_defs(catalog, sequences, scripts_ok):
    by_name = {}
    for r in catalog['activities']:
        if r['category'] in ('strike', 'nightfall', 'weekly_heroic', 'strike_playlist') and r['name'] in STRIKES:
            by_name.setdefault(r['name'], []).append(r)
    out = []
    for name, (sid, folder, seq_id, boss) in STRIKES.items():
        rows = by_name.get(name, [])
        hashes = sorted(r['hash'] for r in rows)
        bundles = sorted({r['bundle_hash'] for r in rows if r['bundle_hash']})
        # The skulls come with the playlist row a run is launched from; a strike allows Heroic or
        # Nightfall when one of its rows is in such a playlist (or is a Nightfall row).
        mine = set(hashes)
        heroic = any('Heroic' in p['name'] and mine & set(p['members']) for p in catalog.get('playlists', []))
        nightfall = any(r['category'] == 'nightfall' for r in rows)
        mods = (['Heroic'] if heroic else []) + (['Nightfall'] if nightfall else [])
        seq = sequences.get(seq_id) if seq_id else None
        need = []
        d = {'id': sid, 'name': name, 'kind': 'strike', 'activity_hashes': hashes, 'bundle_hashes': bundles,
             'fireteam': {'min': 1, 'max': max([r['max_party'] for r in rows] or [3])},
             'checkpoints': 'in_activity', 'modifiers': mods}
        script = regions = wiped = None
        if folder and scripts_ok:
            script, regions, wiped = read_script(folder)
            d['script'] = script
        if seq:
            d['sequence'] = seq_id
        encounters = []
        if script and not script['start_only_stub']:
            groups = {g for st in seq['stages'] for g in st['groups_first_served']} if seq else set()
            check = f"{fnv1(script['bundle']):#010x}"
            src = (f"restored 56578 script {script['bundle']} (identifiers only); the live PAH served its "
                   f"group {check} = fnv1('{script['bundle']}')" if check in groups else
                   f"restored 56578 script {script['bundle']} (identifiers only)")
            d['evidence'] = ev('script', src)
            for i, p in enumerate(script['phases']):
                encounters.append({
                    'id': p, 'name': p.replace('_', ' '), 'region': regions.get(p),
                    'begins_on': {'script_phase': p}, 'checkpoint': True, 'darkness': p in wiped,
                    'boss': boss if i == len(script['phases']) - 2 and boss else None,
                    'evidence': ev('script', f"@phase row {p}" + (', @hard_wipe' if p in wiped else '')),
                })
            if boss:
                need.append('which phase is the boss fight: taken as the next-to-last phase (the last is the outro)')
        elif seq:
            d['evidence'] = ev('live-capture', f"sequences/{seq_id}.json ({seq['evidence']['capture']}, 51539)")
            last = len(seq['segments']) - 1
            for s in seq['segments']:
                b = s['begins_on']
                encounters.append({
                    'id': f"segment-{s['index']}", 'name': f"capture segment {s['index']} ({s['duration_s']:.0f} s)",
                    'region': str(b['slice_entered']) if 'slice_entered' in b else None,
                    'begins_on': b, 'checkpoint': 'slice_entered' in b, 'darkness': s['index'] == last,
                    'boss': boss if s['index'] == last else None,
                    'evidence': ev('live-capture', f"opened at +{s['at_s']} s of the PAH session; segments split at "
                                   "new slice sets and at triggers after 45 s of quiet (a heuristic)"),
                })
            need += ['encounter boundaries inside a bubble (the segments are a heuristic over one solo run)',
                     'which segments are checkpoints and darkness zones (taken as: new slice sets; the last)']
            if script and script['start_only_stub']:
                need.append(f"the restored script {script['activity']} is a start-only stub upstream")
        else:
            d['evidence'] = ev('needs-evidence', 'no restored script and no live capture')
            encounters.append({'id': 'strike', 'name': name, 'begins_on': 'previous_complete', 'checkpoint': True,
                               'darkness': False, 'boss': boss,
                               'evidence': ev('needs-evidence', 'one encounter for the whole strike until a capture or script shows its stages')})
            need.append('the encounter sequence: no restored script, no live capture')
        if boss:
            need.append('boss name is D1 knowledge, not read from the game data')
        d['encounters'] = encounters
        d['rewards'] = [{'label': 'manifest activity rewards (union over the variants)',
                         'items': sorted({h for r in rows for g in r['rewards'] for h in g}),
                         'evidence': ev('manifest', 'DestinyActivityDefinition.rewards')}]
        d['needs_evidence'] = need
        out.append(d)
    return out


# ---------------------------------------------------------------------------------------------
# Raids
# ---------------------------------------------------------------------------------------------

# Encounter lists from D1 knowledge; `loot` names the Cryptarch guide section that holds the
# encounter's loot table (community). Crota's End's openings come from its live capture.
RAIDS = {
    'Vault of Glass': ('raid-vault-of-glass', None, 'Vault of Glass', [
        ('the-spire', 'Opening the Vault', None, None, False),
        ('confluxes', 'The Confluxes', None, 'Confluxes / Oracles', True),
        ('oracles', 'The Oracles', None, None, True),
        ('templar', 'The Templar', 'The Templar', 'Templar (Boss)', True),
        ('gorgons-labyrinth', "The Gorgons' Labyrinth", None, None, True),
        ('the-jump', 'The Glass Throne approach', None, None, False),
        ('gatekeepers', 'The Gatekeepers', None, 'Gatekeeper', True),
        ('atheon', "Atheon, Time's Conflux", "Atheon, Time's Conflux", 'Atheon (Boss)', True),
    ], 'vault-of-glass-reprised'),
    "Crota's End": ('raid-crotas-end', 'raid-crotas-end', "Crota's End", [
        ('the-abyss', 'The Abyss', None, 'The Abyss', True),
        ('the-bridge', 'The Bridge', None, 'The Bridge', True),
        ('ir-yut', 'Ir Yut, the Deathsinger', 'Ir Yut, the Deathsinger', 'Ir Y\u00fbt, the Deathsinger (Boss)', True),
        ('crota', 'Crota, Son of Oryx', 'Crota, Son of Oryx', 'Crota (Boss)', True),
    ], 'crotas-end-reprised'),
    "King's Fall": ('raid-kings-fall', None, "King's Fall", [
        ('hall-of-souls', 'The Hall of Souls', None, 'Hall of Souls (Chest)', True),
        ('tomb-ships', 'The Tomb Ships', None, None, False),
        ('annihilator-totems', 'The Annihilator Totems', None, 'Annihilator Totems (Chest)', True),
        ('warpriest', 'The Warpriest', 'The Warpriest', 'The Warpriest (Boss)', True),
        ('golgoroth', 'Golgoroth', 'Golgoroth', 'Golgoroth (Boss)', True),
        ('daughters', 'The Daughters of Oryx', 'The Daughters of Oryx', 'Daughters of Oryx (Boss)', True),
        ('oryx', 'Oryx, the Taken King', 'Oryx, the Taken King', 'Oryx, the Taken King (Boss)', True),
    ], 'kings-fall-reprised'),
    'Wrath of the Machine': ('raid-wrath-of-the-machine', None, 'Wrath of the Machine', [
        ('entering-the-wall', 'Entering the Wall', None, 'Entering the Wall (Chest)', True),
        ('vosik', 'Vosik, the Archpriest', 'Vosik, the Archpriest', 'Vosik, the Archpriest (Boss)', True),
        ('siege-engine', 'The Siege Engine', None, 'Siege Engine (Chest)', True),
        ('perfection-complex', 'The Perfection Complex', None, 'The Perfection Complex (Chest)', False),
        ('aksis', 'Aksis, Archon Prime', 'Aksis, Archon Prime', 'Aksis, Archon Prime (Boss)', True),
    ], 'wrath-of-the-machine-featured'),
}

# Crota's End: which capture segment opens which encounter (our reading of the run: the Abyss from
# the start, the Bridge with the new slice set, Ir Yut and Crota at the triggers that opened the
# last two long segments).
CROTA_SEGMENTS = {'the-abyss': 0, 'the-bridge': 2, 'ir-yut': 4, 'crota': 5}


def community_loot(page):
    """{section title: [item hashes]} from a Cryptarch page; titles normalised for matching."""
    out = {}
    for s in (page or {}).get('sections', []):
        items = []
        for dv in s.get('dividedItemDisplay', []) or []:
            items += [int(i) for i in dv.get('items', []) or [] if str(i).isdigit()]
        out[s.get('sectionTitle', '').replace('\ufffd', '\u00fb')] = items
    return out


def raid_defs(catalog, sequences, community):
    out = []
    for name, (rid, seq_id, _, encs, page_key) in RAIDS.items():
        rows = [r for r in catalog['activities'] if r['category'] == 'raid' and r['name'] == name]
        loot = community_loot(community.get(page_key)) if community else {}
        seq = sequences.get(seq_id) if seq_id else None
        d = {'id': rid, 'name': name, 'kind': 'raid', 'activity_hashes': sorted(r['hash'] for r in rows),
             'bundle_hashes': sorted({r['bundle_hash'] for r in rows if r['bundle_hash']}),
             'fireteam': {'min': 1, 'max': max(r['max_party'] for r in rows)}, 'checkpoints': 'weekly',
             'modifiers': [], 'community_page': page_key,
             'evidence': ev('d1-knowledge', 'encounter list; the manifest holds no encounters')}
        if seq:
            d['sequence'] = seq_id
        need = ['checkpoint placement (D1 knowledge)',
                'each difficulty and Age of Triumph challenge rule (activity hashes: '
                + ', '.join(f"{r['hash']} level {r['level']} tier {r['tier']}" for r in sorted(rows, key=lambda r: r['level'])) + ')']
        encounters = []
        for i, (eid, ename, boss, section, checkpoint) in enumerate(encs):
            e = {'id': eid, 'name': ename, 'begins_on': 'previous_complete', 'checkpoint': checkpoint or i == 0,
                 'darkness': True, 'boss': boss, 'evidence': ev('d1-knowledge', 'encounter name and order')}
            if seq and eid in CROTA_SEGMENTS:
                s = seq['segments'][CROTA_SEGMENTS[eid]]
                e['begins_on'] = s['begins_on']
                e['evidence'] = ev('live-capture', f"opened at +{s['at_s']} s of the live run ({seq['evidence']['capture']}); "
                                   'which segment is which encounter is our reading')
            if section and section in loot:
                e['rewards'] = [{'label': section, 'items': loot[section], 'evidence': ev('community', COMMUNITY_SRC)}]
            encounters.append(e)
        if not community:
            need.append('encounter loot (community loot tables not read)')
        d['encounters'] = encounters
        d['rewards'] = [{'label': 'manifest activity rewards (union over the difficulties)',
                         'items': sorted({h for r in rows for g in r['rewards'] for h in g}),
                         'evidence': ev('manifest', 'DestinyActivityDefinition.rewards')}]
        d['needs_evidence'] = need
        out.append(d)
    return out


# ---------------------------------------------------------------------------------------------
# Arenas and Archon's Forge
# ---------------------------------------------------------------------------------------------

def arena_defs(catalog):
    rows = catalog['activities']
    out = []

    def rounds(n, boss_last, label):
        return [{'id': f'round-{i}', 'name': f'Round {i}', 'begins_on': 'previous_complete', 'checkpoint': True,
                 'darkness': True, 'boss': (boss_last if i == n else None),
                 'evidence': ev('d1-knowledge', label)} for i in range(1, n + 1)]

    poe = [r for r in rows if r['category'] == 'arena']
    out.append({'id': 'arena-prison-of-elders', 'name': 'Prison of Elders', 'kind': 'arena', 'community_page': 'prison-of-elders',
                'activity_hashes': sorted(r['hash'] for r in poe), 'fireteam': {'min': 1, 'max': 3},
                'checkpoints': 'in_activity', 'modifiers': [],
                'encounters': rounds(5, None, 'House of Wolves Prison of Elders: four rounds of waves, a boss round fifth'),
                'rewards': [{'label': 'manifest activity rewards', 'items': sorted({h for r in poe for g in r['rewards'] for h in g}),
                             'evidence': ev('manifest', 'DestinyActivityDefinition.rewards')}],
                'evidence': ev('d1-knowledge', 'round structure'),
                'needs_evidence': ['round count and waves per level (28, 32, 34, 35, 41)', 'the bosses per round',
                                   "whether a wipe restarts the round (taken as: yes)"]})
    challenge = {}
    for r in rows:
        if r['category'] == 'arena_challenge':
            challenge.setdefault(r['name'], []).append(r)
    for name, rs in sorted(challenge.items()):
        if name == 'Challenge of the Elders':
            out.append({'id': 'arena-challenge-of-the-elders', 'name': name, 'kind': 'arena_challenge',
                        'community_page': 'challenge-of-the-elders',
                        'activity_hashes': sorted(r['hash'] for r in rs), 'fireteam': {'min': 1, 'max': 3},
                        'checkpoints': 'in_activity', 'modifiers': sorted({s for r in rs for s in r['skulls'] if s}),
                        'encounters': rounds(3, None, 'The Taken King Challenge of the Elders: three scored rounds, each ending on a boss'),
                        'score_thresholds': [30000],
                        'rewards': [{'label': 'manifest activity rewards', 'items': sorted({h for r in rs for g in r['rewards'] for h in g}),
                                     'evidence': ev('manifest', 'DestinyActivityDefinition.rewards')}],
                        'evidence': ev('d1-knowledge', 'scored rounds'),
                        'needs_evidence': ['the scoring (points per kill, modifiers, the bonus timer)',
                                           'the score threshold for the end reward (30000 is a stand-in)']})
            continue
        sid = 'arena-' + re.sub(r'[^a-z0-9]+', '-', name.lower()).strip('-')
        out.append({'id': sid, 'name': name, 'kind': 'arena_challenge', 'activity_hashes': sorted(r['hash'] for r in rs),
                    'fireteam': {'min': 1, 'max': 3}, 'checkpoints': 'in_activity',
                    'modifiers': sorted({s for r in rs for s in r['skulls'] if s}),
                    'encounters': rounds(5, name, 'House of Wolves challenge arena: four rounds and the named boss'),
                    'rewards': [{'label': 'manifest activity rewards', 'items': sorted({h for r in rs for g in r['rewards'] for h in g}),
                                 'evidence': ev('manifest', 'DestinyActivityDefinition.rewards')}],
                    'evidence': ev('d1-knowledge', 'round structure'),
                    'needs_evidence': ['round count and the boss (named after the activity here)']})
    forge = [r for r in rows if r['category'] == 'patrol_event']
    out.append({'id': 'event-archons-forge', 'name': "Archon's Forge", 'kind': 'public_event', 'community_page': 'archons-forge',
                'activity_hashes': sorted(r['hash'] for r in forge), 'fireteam': {'min': 1, 'max': 6},
                'checkpoints': 'in_activity', 'modifiers': [],
                'encounters': [
                    {'id': 'offering', 'name': 'A SIVA Offering is placed', 'begins_on': 'previous_complete', 'checkpoint': False,
                     'darkness': False, 'evidence': ev('community', COMMUNITY_SRC + ': the Forge starts from an offering')},
                    {'id': 'waves', 'name': 'The Forge: waves', 'begins_on': 'previous_complete', 'checkpoint': False,
                     'darkness': False, 'evidence': ev('needs-evidence', 'wave count and timer')},
                    {'id': 'champion', 'name': "The Forge's champion", 'begins_on': 'previous_complete', 'checkpoint': False,
                     'darkness': False, 'evidence': ev('needs-evidence', 'boss per offering tier')}],
                'rewards': [],
                'evidence': ev('community', "a public event inside Patrol the Plaguelands; the manifest has no row for it"),
                'needs_evidence': ['offering tiers and their bosses', 'the timer and failure rule', 'the rewards per tier',
                                   'Rise of Iron Plaguelands capture']})
    return out


# ---------------------------------------------------------------------------------------------
# The Crucible
# ---------------------------------------------------------------------------------------------

def kill_rules(points):
    return [
        {'incident': 'player_kill', 'points': points, 'stat': 'score', 'credit': 'both'},
        {'incident': 'player_kill', 'points': 1, 'stat': 'kills', 'credit': 'player'},
        {'incident': 'player_death', 'points': 1, 'stat': 'deaths', 'credit': 'player'},
        {'incident': 'player_assist', 'points': 1, 'stat': 'assists', 'credit': 'player'},
    ]


def mode(mid, name, teams, size, *, limit=None, time=None, rounds=None, tie='draw', kill=100, extra=(),
         objective=None, mercy=None, kind='stand-in'):
    players = size if teams == 0 else teams * size
    return {'id': mid, 'name': name, 'teams': teams, 'team_size': size, 'max_players': players,
            'score_limit': limit, 'time_limit_s': time, 'rounds': rounds, 'mercy_lead': mercy, 'tie': tie,
            'scoring': kill_rules(kill) + [dict(e) for e in extra], 'objective': objective, 'type_hashes': [],
            'evidence': ev(kind, 'team sizes: the playlists\' manifest maxPlayers; limits and points: ' + STAND_IN_SRC)}


MODES = [
    mode('control', 'Control', 2, 6, limit=15000, time=720, objective='zones',
         extra=[{'incident': 'zone_captured', 'points': 200, 'stat': 'score', 'credit': 'both'}]),
    mode('zone_control', 'Zone Control', 2, 6, limit=15000, time=720, kill=0, objective='zones',
         extra=[{'incident': 'zone_captured', 'points': 200, 'stat': 'score', 'credit': 'both'},
                {'incident': 'zone_held', 'points': 10, 'stat': 'score', 'credit': 'team'}]),
    mode('clash', 'Clash', 2, 6, limit=10000, time=600),
    mode('mayhem', 'Mayhem Clash', 2, 6, limit=20000, time=720),
    mode('rumble', 'Rumble', 0, 6, limit=2000, time=600),
    mode('skirmish', 'Skirmish', 2, 3, limit=5000, time=600),
    mode('doubles', 'Doubles', 2, 2, limit=3000, time=600),
    mode('salvage', 'Salvage', 2, 3, limit=5000, time=600, objective='relic',
         extra=[{'incident': 'relic_captured', 'points': 1000, 'stat': 'score', 'credit': 'both'}]),
    mode('rift', 'Rift', 2, 6, limit=10000, time=720, objective='sparks',
         extra=[{'incident': 'spark_scored', 'points': 1000, 'stat': 'score', 'credit': 'both'}]),
    mode('supremacy', 'Supremacy', 2, 6, limit=10000, time=600, kill=0, objective='crests',
         extra=[{'incident': 'crest_collected', 'points': 100, 'stat': 'score', 'credit': 'both'}]),
    mode('supremacy_rumble', 'Rumble Supremacy', 0, 6, limit=2000, time=600, kill=0, objective='crests',
         extra=[{'incident': 'crest_collected', 'points': 100, 'stat': 'score', 'credit': 'player'}]),
    mode('elimination', 'Elimination', 2, 3, rounds={'wins_needed': 5, 'time_limit_s': 120, 'respawn': False},
         tie='most_alive', extra=[{'incident': 'player_revive', 'points': 0, 'stat': 'revives', 'credit': 'player'}]),
    mode('trials', "Trials of Osiris (Elimination)", 2, 3, rounds={'wins_needed': 5, 'time_limit_s': 120, 'respawn': False},
         tie='most_alive', extra=[{'incident': 'player_revive', 'points': 0, 'stat': 'revives', 'credit': 'player'}]),
    mode('combined_arms', 'Combined Arms', 2, 6, limit=15000, time=720, objective='zones',
         extra=[{'incident': 'zone_captured', 'points': 200, 'stat': 'score', 'credit': 'both'}]),
    mode('sparrow_racing', 'Sparrow Racing', 0, 6, limit=None, time=600, kill=0, objective='race',
         extra=[{'incident': 'race_finished', 'points': 1, 'stat': 'score', 'credit': 'player'}], kind='needs-evidence'),
    mode('arsenal', 'Arsenal', 0, 6, limit=2000, time=600, kind='needs-evidence'),
]

# Manifest Crucible playlist type name -> (mode id, flavours).
PLAYLIST_MODE = {
    'Control': ('control', []), 'Inferno Control': ('control', ['inferno']), 'Iron Banner Control': ('control', ['iron_banner']),
    'Zone Control': ('zone_control', []), 'Inferno Zone Control': ('zone_control', ['inferno']),
    'Clash': ('clash', []), 'Inferno Clash': ('clash', ['inferno']), 'Iron Banner Clash': ('clash', ['iron_banner']),
    'Mayhem Clash': ('mayhem', ['mayhem']), 'Iron Banner Mayhem Clash': ('mayhem', ['mayhem', 'iron_banner']),
    'Rumble': ('rumble', []), 'Inferno Rumble': ('rumble', ['inferno']), 'Mayhem Rumble': ('rumble', ['mayhem']),
    'Classic Free-For-All': ('rumble', ['classic']), 'Arsenal': ('arsenal', []),
    'Skirmish': ('skirmish', []), 'Inferno Skirmish': ('skirmish', ['inferno']), 'Classic 3v3': ('skirmish', ['classic', 'mixed']),
    'Freelance 3v3': ('skirmish', ['freelance', 'mixed']), 'Inferno 3v3': ('skirmish', ['inferno', 'mixed']),
    'Doubles': ('doubles', []), 'Inferno Doubles': ('doubles', ['inferno']), 'Crimson Doubles': ('doubles', ['crimson']),
    'Salvage': ('salvage', []), 'Inferno Salvage': ('salvage', ['inferno']),
    'Rift': ('rift', []), 'Inferno Rift': ('rift', ['inferno']), 'Iron Banner Rift': ('rift', ['iron_banner']),
    'Supremacy': ('supremacy', []), 'Inferno Supremacy': ('supremacy', ['inferno']), 'Mayhem Supremacy': ('supremacy', ['mayhem']),
    'Iron Banner Supremacy': ('supremacy', ['iron_banner']), 'Rumble Supremacy': ('supremacy_rumble', []),
    'Elimination': ('elimination', []), 'Inferno Elimination': ('elimination', ['inferno']),
    'Trials of Osiris': ('trials', ['trials']),
    'Classic 6v6': ('clash', ['classic', 'mixed']), 'Freelance 6v6': ('clash', ['freelance', 'mixed']),
    'Inferno 6v6': ('clash', ['inferno', 'mixed']),
    'Combined Arms': ('combined_arms', []), 'Sparrow Racing': ('sparrow_racing', []),
}

# The eleven Crucible roots of the restored scripts (all start-only stubs) and their maps.
PVP_SCRIPTS = {
    'pvp_anomaly': 'The Anomaly', 'pvp_cabal_camp': 'Firebase Delphi', 'pvp_city_defense': 'Twilight Gap',
    'pvp_colony_ship': 'Exodus Blue', 'pvp_factory': 'Rusted Lands', 'pvp_greenhouse': 'Shores of Time',
    'pvp_marsbattle': 'Bastion', 'pvp_moonbase': 'First Light', 'pvp_rooftops': 'Blind Watch',
    'pvp_venus_hcc': 'Asylum', 'pvp_vex_tube': 'The Burning Shrine',
}


def crucible(catalog, scripts_ok):
    rows = catalog['activities']
    by_map = {v: k for k, v in PVP_SCRIPTS.items()}
    maps = []
    for r in rows:
        if r['category'] != 'crucible_map':
            continue
        script = by_map.get(r['name'])
        if script and scripts_ok and not (CFG['scripts'] / 'es' / script).exists():
            script = None
        maps.append({'hash': r['hash'], 'name': r['name'] or f"(unnamed {r['hash']})", 'script': script,
                     'evidence': ev('manifest', 'DestinyActivityDefinition (Rumble-typed map rows)'
                                    + ('; restored script is a start-only stub' if script else ''))})
    playlists, modes = [], {m['id']: m for m in json.loads(json.dumps(MODES))}
    for r in rows:
        if r['category'] != 'crucible_playlist':
            continue
        mid, fl = PLAYLIST_MODE[r['type_name']]
        modes[mid]['type_hashes'] = sorted(set(modes[mid]['type_hashes']) | {r['type_hash']})
        playlists.append({'hash': r['hash'], 'name': r['name'], 'mode': mid, 'flavours': fl,
                          'level_advantages': 'iron_banner' in fl or 'trials' in fl, 'matchmade': r['matchmade'],
                          'evidence': ev('manifest', f"type {r['type_name']}; level {r['level']}; "
                                         f"party {r['min_party']}-{r['max_party']}, {r['max_players']} players; "
                                         'mode and flavour mapping is ours')})
    ib = [p['hash'] for p in playlists if 'iron_banner' in p['flavours']]
    trials = [p['hash'] for p in playlists if 'trials' in p['flavours']]
    marks = 3510203897  # Legendary Marks, in most playlists' manifest rewards
    return {
        'comment': ('Crucible rules for the server-owned match state. Team sizes, levels, party sizes and playlist '
                    'rewards are the manifest\'s; scoring and limits are stand-ins (see each mode). Generated by '
                    'tools/ps4_setup/d1-activities-defs.py.'),
        'modes': list(modes.values()),
        'maps': maps,
        'playlists': playlists,
        'iron_banner': {'playlists': ib, 'level_advantages': True,
                        'evidence': ev('manifest', 'Iron Banner playlists are level 40 in the manifest; level advantages '
                                       'are D1 knowledge')},
        'trials': {'mode': 'trials', 'fireteam': 3, 'wins_for_lighthouse': 9, 'losses_to_end': 3,
                   'reward_wins': [5, 7, 9], 'boons': ['favor', 'mercy', 'boon'], 'playlists': trials,
                   'evidence': ev('d1-knowledge', 'fireteam of 3 is the manifest\'s (party 3-3); the Passage rules '
                                  '(nine wins, three losses, the Lighthouse for a flawless card, the three boons, '
                                  'rewards at 5/7/9 wins) are D1 knowledge')},
        'rewards': {'reputation_completed': {'Crucible': 75, 'Iron Banner': 100},
                    'reputation_win_bonus': {'Crucible': 25, 'Iron Banner': 50},
                    'drop_items': [marks, 2807593482], 'ffa_winning_places': 3,
                    'evidence': ev('stand-in', 'reputation amounts: ' + STAND_IN_SRC + '; drop items: the playlists\' '
                                   'manifest rewards (Legendary Marks, Treasure of Ages); free-for-all top three win: D1 knowledge')},
    }


def write(path, doc):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(doc, indent=1, ensure_ascii=False) + '\n', encoding='utf-8')


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--dir', type=Path, default=ROOT / 'ps4' / 'fixtures' / '56578' / 'activities')
    ap.add_argument('--scripts', type=Path, default=SCRIPTS)
    ap.add_argument('--community', type=Path, default=COMMUNITY)
    a = ap.parse_args()
    CFG['scripts'] = a.scripts
    scripts_ok = a.scripts.exists()
    catalog = json.loads((a.dir / 'catalog.json').read_text(encoding='utf-8'))
    sequences = {p.stem: json.loads(p.read_text(encoding='utf-8')) for p in sorted((a.dir / 'sequences').glob('*.json'))}
    community = json.loads(a.community.read_text(encoding='utf-8', errors='replace')) if a.community.exists() else {}
    gen = 'Generated by tools/ps4_setup/d1-activities-defs.py; see notes/ps4-56578-strikes-raids-crucible-2026-11-05.md.'
    strikes = strike_defs(catalog, sequences, scripts_ok)
    raids = raid_defs(catalog, sequences, community)
    arenas = arena_defs(catalog)
    write(a.dir / 'definitions' / 'strikes.json', {'comment': gen, 'activities': strikes})
    write(a.dir / 'definitions' / 'raids.json', {'comment': gen, 'activities': raids})
    write(a.dir / 'definitions' / 'arenas.json', {'comment': gen, 'activities': arenas})
    cr = crucible(catalog, scripts_ok)
    write(a.dir / 'crucible.json', cr)
    print(f'{len(strikes)} strikes, {len(raids)} raids, {len(arenas)} arenas and events, '
          f"{len(cr['modes'])} Crucible modes, {len(cr['maps'])} maps, {len(cr['playlists'])} playlists -> {a.dir}")


if __name__ == '__main__':
    main()
