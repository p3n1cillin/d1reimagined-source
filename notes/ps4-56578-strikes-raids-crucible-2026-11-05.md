# PS4 56578: strikes, raids, arenas and the Crucible as data (5 November 2026)

The activity host now has a framework for the multi-stage and competitive activities, and data
for every one Bungie's final manifest lists. Nothing was run in the game: this is built,
unit-tested and wired in behind a knob. It does not change what the host sends yet.

## What was built

All of it is in `d1-activity-host`:

- **`checkpoints.rs`**
  - Per-character checkpoints and weekly loot lockouts, behind a `CheckpointStore` trait.
  - The weekly reset is Tuesday 09:00 UTC.
  - Two stores implement the trait: one in memory, and one that writes stream C's store control
    file (see "The store", below).
- **`encounter_framework.rs`**
  - An activity is a list of encounters. Each one opens on something the world reports:
    - a script phase;
    - a slice set the client reports;
    - a trigger it crosses;
    - the previous encounter finishing;
    - a manual open.
  - Each encounter can be a checkpoint, a darkness zone (where the whole fireteam dying is a
    wipe), a boss fight, or timed.
  - The skulls are the modifiers:
    - **Nightfall** returns the fireteam to orbit when every player is dead. The rule follows
      the skull's own manifest text, which has no darkness condition.
    - **Heroic** is noted on the run only: it changes combat, and the clients simulate combat.
  - `ActivityRun` turns events into effects and never touches the wire.
  - `ActivityLibrary` loads and checks the data.
- **`pvp.rs`**
  - The Crucible match state machine: lobby, countdown, play, rounds, sudden death, end and
    results.
  - The scoreboard row (row type 15), with BullShcript's semantics (MIT):
    - content rules keyed by incident name hash;
    - an audience and a scope mask (player 1, team 2, global 4);
    - a small stack bytecode;
    - `add_player_score`, `is_tied` and the leading team;
    - at most two notifications.
  - `ScoreboardState` serialises with BullShcript's field names, so the script bridge and a row
    encoder can take it unchanged.
  - Trials Passage cards, Iron Banner and match rewards.
- **The hooks** (two, both marked in `main.rs`):
  - `d1-activity-host activities [--verbose]` checks the data and prints its coverage.
  - With `D1_AH_ACTIVITY_FRAMEWORK=1` (or a folder), `serve` loads and checks the data.
    Off by default.

**Who drives a run.** The framework takes the same events from two sources:
- **The script bridge** (streams A1/A2): `phase`, `all_dead`, `activity_complete`.
- **The host's own reports**, for activities with no restored script: `host_events` turns udt 21
  slice sets and incident 3403 trigger reports into the same events.

**Who owns what in the Crucible.** Combat is peer-to-peer on a console "physics host", as Bungie
built it. The server owns the match state, the scoring, the scoreboard row and the rewards.

## The data (`ps4/fixtures/56578/activities/`)

Three tools write the data. They need these local inputs, none of which is kept in the
repository:
- Bungie's final D1 manifest (DIM's copy) and the mobile world database;
- the private live captures;
- Blam Network's restored scripts;
- the Cryptarch guide.

| File | Written by | What |
|---|---|---|
| `catalog.json` | `tools/ps4_setup/d1-activities-export.py` | 407 manifest rows: 138 strike, 50 Nightfall, 27 weekly Heroic, 33 strike playlist, 17 raid, 4 Prison of Elders, 15 challenge arena, 86 Crucible playlist and 36 Crucible map rows, plus the Plaguelands patrol (for Archon's Forge). Also the skulls, the activity bundles, the modes, and the strike playlists with their members |
| `sequences/*.json` | `tools/ps4_setup/d1-activity-sequence.py CAPTURE` | Nine strikes and Crota's End as the live server ran them: slice sets, the groups served, triggers crossed and cinematics, in time order, plus the encounter segments |
| `definitions/{strikes,raids,arenas}.json` | `tools/ps4_setup/d1-activities-defs.py` | 17 strikes, 4 raids, Prison of Elders, Challenge of the Elders, 6 challenge arenas and Archon's Forge |
| `crucible.json` | the same tool | 16 modes, 36 maps, 86 playlists, Iron Banner, Trials and the reward rules |

### Strikes: which restored script is which strike

The live captures settle it. A mission's activity host serves its root bundle's group, and that
group's hash is the FNV-1 of the bundle name. The tool checks this for each strike, and it
records the check in the definition's evidence.

| Strike | Script (`@activity`) | Group served live |
|---|---|---|
| The Devils' Lair | `cosmo_portal_1` (8 phases) | `0x3c3ec728` = fnv1(`cosmo_portal1_activity`) |
| The Summoning Pits | `moon_portal_2` (6) | `0x3e3c20e0` |
| The Nexus | `venus_portal_1` (5) | `0x0a302429` = fnv1(`strike_1_activity`) |
| The Undying Mind | `mars_portal_3` (6) | `0x2762ddea` |
| **The Shadow Thief** | **`strike_vampire`** (8) | `0xc33d6bb8`, and its three `unknown_*` bundles |

`strike_vampire` is **The Shadow Thief**, not The Will of Crota. The gap analysis had guessed
The Will of Crota from the Omnigul filters. The Will of Crota has no restored script.

For these five strikes:
- each encounter is a script phase;
- each phase is a checkpoint;
- the phases marked `@hard_wipe` are darkness zones.

Only identifiers are taken from the scripts: activity, bundle and phase names, regions and the
`@hard_wipe` flags.

### Strikes with no restored script: from the captures

Four strikes run from their live captures:
- The Will of Crota (no script);
- Winter's Run, Cerberus Vae III and Dust Palace (start-only stubs upstream).

How the capture becomes encounters:
- **Segments.** A segment opens at a slice set the player had not reported before, or at a
  trigger that follows 45 s of quiet (a fight in between).
- **Encounters.** Each segment becomes an encounter, opened by exactly that slice set or trigger.
- **Checkpoints and darkness.** The new-slice-set segments are taken as checkpoints, and the last
  segment as the boss and a darkness zone. Both are stated as guesses in each definition.

Each run is one solo player on the Xbox 360 build 51539. The ids are the same on 56578: slice
sets and trigger names are content hashes, and the trigger report's layout is the same.

### Strikes with neither

Eight strikes have neither a restored script nor a capture: Fallen S.A.B.E.R., Shield Brothers,
The Sunless Cell, Blighted Chalice, Echo Chamber, Sepiks Perfected, The Wretched Eye and The
Abomination Heist.
- Each is one encounter, marked `needs-evidence`.
- The boss is named only where D1 knowledge is sure of it.

`cosmo_portal_2` (start slice `array`) is a stub that may be one of them. That is unconfirmed, so
it is not mapped.

### Raids

Raids keep checkpoints per character for the week:
- **At launch** the run loads the leader's checkpoint.
- **When an encounter completes**, every present member's checkpoint is saved at the next
  checkpointed encounter, and the encounter's loot locks for that character for the week.
- **A full clear** spends the checkpoint.

The encounter lists are D1 knowledge. The manifest holds no encounters.

**Crota's End** opens its encounters at the capture's own reports:
- the Abyss from the start;
- the Bridge at slice set 32;
- Ir Yût at trigger `0x94b164df/29/86`;
- Crota at `0x51d22aa9/29/58`.

Which segment is which encounter is our reading of the run.

Encounter loot comes from the Cryptarch guide's loot tables, as item hashes only. It is labelled
`community`.

### Arenas and Archon's Forge

- **Prison of Elders:** five rounds, the last a boss round.
- **The challenge arenas:** five rounds and the named boss.
- **Challenge of the Elders:** three scored rounds, with a stand-in threshold of 30000.
- **Archon's Forge:** a public event inside Patrol the Plaguelands. It has no manifest row of its
  own, so it is labelled `community` and `needs-evidence`.

### Matchmaking playlists

The manifest lists no playlist members. The export infers them, and each inference is labelled:
- **Vanguard Legacy (level 20), Taken War (36) and Taken War Heroic (41):** the per-strike
  playlist-typed rows at the playlist's level.
- **SIVA Crisis:** the level-42 matchmade strike rows.
- **Vanguard Nightfall:** the per-strike Nightfall rows.

The year-one Weekly Heroic and Weekly Nightfall rows name no strike, so they are `needs-evidence`.

### The Crucible

**From the manifest:**
- the team sizes (each playlist's `maxPlayers`);
- the party sizes, including Trials' fireteam of 3;
- Iron Banner's level 40;
- the playlist reward items (Legendary Marks, Treasure of Ages).

**Modes.** Control, Zone Control, Clash, Mayhem, Rumble, Skirmish, Doubles, Salvage, Rift,
Supremacy, Rumble Supremacy, Elimination, Trials (Elimination), Combined Arms, Sparrow Racing and
Arsenal. Every manifest Crucible playlist maps to one of them, with flavours (inferno, mayhem,
classic, freelance, iron_banner, trials).

**Stand-ins.** Every score limit, time limit and point value is one, labelled so. The authority is
the content's scoreboard definition, which is not exported yet. The stand-ins are written in that
definition's rule shape, so an export can replace them value for value.

**Trials and Iron Banner** (D1 knowledge):
- A Trials card completes at nine wins and ends at three losses. A flawless card goes to the
  Lighthouse.
- The boons: Favor (the card starts at one win), Mercy (the first loss is forgiven) and Boon (the
  first win counts twice).
- Trials rewards come at five, seven and nine wins.
- Iron Banner and Trials playlists have level advantages on.

**Maps.** The eleven restored Crucible roots are all start-only stubs upstream. They are mapped to
their maps and flagged.

## The store (stream C) and rewards (stream M)

**The store.** Stream C's `player_store::PlayerStore` lives in `d1-responder`, which is another
process. Until there is a direct call, `ControlFileCheckpointStore` writes the store's control
file (`D1_STORE_CONTROL`). Each save is one line:

```text
checkpoint INDEX ENCOUNTER_INDEX ENCOUNTER_ID week=WEEK
```

- `INDEX` is the client's activity index, from the manifest hash through
  `characters/activity-table.tsv`. Every raid hash maps.
- The store reads the line as `Command::Checkpoint(INDEX, step)`.
- `checkpoints::format_step` and `parse_step` are the two directions of `step`.

**What the responder side still has to provide:**
1. **A clear.** Until there is one, a spent checkpoint is sent as `checkpoint INDEX -`.
2. **Loot lockouts.** There is no store field for them, so they are only in the host's mirror.
3. **A way to read a checkpoint at launch.** Until there is one, `seed` takes what the launcher
   knows.

**Rewards.** Rewards go through stream M's `rewards::grant_activity_rewards(&mut store,
character, activity_hash, &ActivityOutcome { completed, community_page, roll_engrams, seed })`
(#311), once per member and only when `rewards::enabled()`:
- `ActivityRun::outcome()` gives the inputs once a run has ended: the activity hash, `completed`
  (true after `ActivityCompleted`, false after `ReturnedToOrbit`), the definition's
  `community_page` and the members present;
- the raids, Prison of Elders, Challenge of the Elders and Archon's Forge carry their Cryptarch
  page; for a strike the launcher passes the playlist's page (for example
  `siva-crisis-heroic-strike-playlist`), since the page belongs to the playlist, not the strike;
- `Rewards` and `RewardsLocked` carry the encounter loot and the weekly lockout.

## Matchmaking (stream H)

`encounter_framework::matchmaking_playlists` gives one entry per matchmade playlist, in the shape
of stream H's `Matchmaker::add_playlist(Playlist { hash, kind, activities })` (#312, `d1-mp`):
- **Strike playlists** (Vanguard Legacy, Taken War, SIVA Crisis, their Heroics, Vanguard
  Nightfall): `Cooperative { max_players }` from the manifest row, the inferred members as
  `activities`. The year-one weekly rows have no members and are left out.
- **Crucible playlists**: `Versus { teams, team_size, min_players }` from the mode (team sizes from
  the manifest's `maxPlayers`; free-for-all is one-player teams), with `min_players` a stand-in of
  one full team. The rotation is every Crucible map, a stand-in: the manifest names no playlist's
  maps.
- A single activity (a raid, an arena) goes through `Services::launch_activity(leader, ActivityRef
  { activity_hash, party, .. })`, with `party` the catalog row's `min_party`/`max_party`.

`d1-activity-host` does not depend on `d1-mp`; the caller copies the entries across.

## Tests

`cargo test -p d1-activity-host` covers the following.

**Unit tests:**
- checkpoint save, load and restore;
- wipe and restart at the last checkpoint;
- the leader's checkpoint at launch, and the weekly reset;
- spending a checkpoint on a full clear, and the loot lockout;
- Nightfall;
- script phases;
- time limits;
- the Crucible lobby, score limit, mercy, sudden death, Elimination rounds, forfeits and
  free-for-all places;
- the scoreboard's rules, bytecode and notifications;
- Trials cards;
- rewards;
- the control-file store.

**Integration tests** (`tests/activities.rs`), over the real data:
- the data validates, including coverage of every PvE manifest row;
- every capture replays through its definition;
- a scripted strike follows its phases, and Nightfall sends a wipe to orbit;
- Crota's End wipes, keeps its checkpoint for the week and locks its loot;
- every Crucible mode plays to an end;
- the Trials and Iron Banner rules match the manifest;
- the scoreboard JSON has BullShcript's shape;
- the playlists map onto the matchmaker, and a finished run onto the reward call.

## Needs evidence

**56578 content export (offline, 5 November).** `d1-oracle content-tables` now writes
`ps4/fixtures/56578/activities/scoreboard-definitions.json`: 23 definitions, 1,064 incident
programs and 264 derived-stat programs. Incident/audience/stat hashes, scope flags,
instruction numbers, constants and variable identifiers retain the existing scoreboard rule
shape. Each entry's source tag and SHA-256 are recorded; private-input tests regenerate and
compare the output, skipping explicitly when the inputs are absent. The package reader is the
project's existing reader; the gibbon layout credit is in `d1-server/ATTRIBUTION.md`.

This closes the availability of the numeric scoring programs, **not the mode association**.
The 272 activity-options tags (also exported as `activity-options.json`, with each activity's
base/conditional option tags in `activity-rule-programs.json`) contain `score_to_win` and
`time_limit` overrides, but the defaults and selection path for each mode have not been
established. Scoreboard tags alone do
not identify Control versus Clash, and limits/mercy/round counts cannot safely be guessed from
them. `crucible.json` therefore retains its stand-in labels and values; no mode is marked fully
verified and the unmatched definitions are not silently activated. Score/time limits, mercy,
round counts and per-mode program selection remain open. No native or lab run was made.

**Per-mode limits: where they are, and where they are not (6 November, offline).** Option names
are FNV-1 hashes: `score_to_win` is 0xBE4DB480 and `time_limit` is 0xF5EC68CA. The results are in
`crucible.json`'s `content_limits`, written by `d1-activities-defs.py` from the committed exports,
and checked by `tests/test_crucible_limits.py`.

What the evidence settles:

- **No playlist or map sets a limit.** Each of the 86 Crucible playlists and 36 maps has a rule
  program. None of their base or conditional option tags sets `score_to_win` or `time_limit`. Most
  playlists have no base option tag at all. The rest share two generic PvP tags (19 and 12
  options, names unknown), and neither tag holds a limit.
- **The 46 preset tags are referenced by no activity.** These are the tags that do set a limit:
  0x80A60003–0x80A60023, 0x80A60064–0x80A60070 and 0x80A63A7C. They come in ladders:
  - scores 25–300 with a companion of 8 or 20;
  - scores 750–5,000, 1,000–3,000 and 2,500–30,000;
  - scores 50,000–500,000;
  - 3, 5, 7 and 10;
  - times 60–1,800 s.

  Each score preset carries companion options (0x537EDFBE, 0x8D445B3A, and 0x0F5D1CE6 in the
  25–300 ladder). They look like the private-match settings, but that is unverified.
- **The client reads only `score_to_win`.** Searching the owned 56578 eboot with `d1-ghidra.ps1
  -Find` and `-Decompile`, its hash occurs once, in the static table of activity-option names. That
  table's only reader is `0x4001a0`, which looks the option up through `0x118870` (the current
  activity's options). It turns the integer into the float that the mode state keeps (+0x46B8), and
  leaves 0 when the option is absent. `0x3DB100` calls it at mode set-up.
- **`time_limit` and the companions are server-side.** Their hashes occur nowhere in the eboot, so
  the client never reads them. They were the activity host's values, which fits the framework
  owning the match clock and the points.
- **No exported table references the 23 scoreboard definitions.** So the mode-to-scoreboard
  selection is not in what has been exported. It lives in content not yet exported, or on the
  server.
- **The 11 Crucible map scripts in Fukugen Daihon say nothing about limits.** Each only names its
  activity and start bubble: no limits, scoring, rounds or mercy. (Read locally; nothing is
  quoted.)

So matchmade Crucible's score limit, time limit, mercy rule and round count are not in the client's
content. The modes' values stay labelled stand-ins, and each mode's evidence now says why.

Two leads are still open:
- Whether the server (the activity host's session or options) supplies `score_to_win` for the
  client's display. With content alone the client's target is 0.
- What references the presets and the scoreboard definitions. A whole-package scan for those tags
  is the next step.

Each definition's `needs_evidence` lists what is missing for it. Across them:
- Selecting the exported scoreboard programs for each Crucible mode, and the limits the server
  sets (the content holds none for matchmade playlists; above).
- **A captured Crucible match** (6 November, stream R): one private capture of a whole matchmade
  match, Control then Elimination or Trials, ideally from two consoles, read per activity host.
  It should record:
  - who reports each kill (the `kill` chain's and the `player_died`'s consoles, and both
    participants of the selector-1 body);
  - the lifetime's values and times: 2 for the countdown? 5 or 9? the 6 → 7 and 7 → 8 delays,
    and a round boundary;
  - the kind-15 scoreboard's player and team rows and their columns, the global block, the
    notifications and `+0x4D3C`/`+0x4D40`;
  - whether `score_to_win` is in the lifetime value's `+0x10` list or the global state's `+0x64`
    list, with its type tag;
  - the final scores, time and rounds;
  - which zone-capture incidents Control sends;
  - the membership's physics host;
  - what the host sends at and after the results.

  The full list, with how each item is read: [the Crucible wire note](ps4-56578-crucible-wire-2026-11-06.md#placeholders-and-what-one-captured-match-would-settle).
- The encounter boundaries inside a bubble for the capture-run strikes.
- The checkpoints and darkness zones of every activity without a script.
- The raid difficulties and Age of Triumph challenge rules.
- Prison of Elders' and Challenge of the Elders' waves, scoring and bosses.
- Archon's Forge's tiers and timer.
- Which strike each year-one weekly row ran.
- The eight strikes with neither a script nor a capture.
