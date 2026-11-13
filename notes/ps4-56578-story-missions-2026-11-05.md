# PS4 56578: every story mission from its own scripts and content (5 November 2026)

**Result.** The script bridge can now run every story mission, cinematic and quest mission the world
tables bind, from Destiny's own mission scripts, on the game's own content:

- the sidecar's rows carry the client's real type ids and indices;
- each activity's table row is generated;
- the reveal (165) completes and hands off to the Tower.

**Offline only.** This was built and unit-tested, with a deterministic replay of cine_reveal and
A Guardian Rises through the bridge. No game, emulator or lab run was made, so nothing here is an
in-game result. Claude runs the integrated native test next.

This is stream K. It closes the open items of [the script bridge](ps4-56578-script-bridge-2026-11-05.md)
and [the sidecar](ps4-56578-script-sidecar-2026-11-05.md). It builds on stream E's
[world tables](ps4-56578-worlds-2026-11-05.md) and stream G's [public host](ps4-56578-public-instances-2026-11-05.md).

## Licences

- **The 56578 scripts** (Fukugen Daihon `v56578_17_04_12_1251_evil`) are unlicensed. They stay a
  local input (`D1_SCRIPTS_DIR`), and this note names activities and counts only.
- **The content export** holds row names, which are game data. It is written outside the repository
  (`D1_BUNDLES_DIR`), and the exporter refuses a path inside it.
- **Blam Network's gibbon and BullShcript** are MIT, read in place from the tiger checkout.
- **The D1 manifest** (DIM's copy) **and the 2014 activity table** (tiger's dictionary) are local
  inputs. Only hashes and indices from them are committed.

## What was built

| Piece | What it does |
|---|---|
| [export-bundles.mjs](../tools/script-host/tools/export-bundles.mjs) | Writes the private content export (below). `--tables` writes the identifier-only `slice-states.tsv`. |
| The sidecar ([PROTOCOL.md](../tools/script-host/PROTOCOL.md)) | Runs on the export (below). New messages: `event_start` / `event_stop`, answered by `event`. |
| [d1-scripts-table.py](../tools/ps4_setup/d1-scripts-table.py) | Generates [scripts/activities.txt](../ps4/fixtures/56578/scripts/activities.txt), the `D1_AH_SCRIPTS_ACTIVITIES` table. |
| [script_bridge.rs](../ps4/d1-server/crates/d1-activity-host/src/script_bridge.rs) | Reads the generated table. Takes phase regions and bundle lists from content. Grants rewards at 6, reports the hand-off and sends the single-event messages. |
| [rewards.rs](../ps4/d1-server/crates/d1-activity-host/src/rewards.rs) | One `ActivityRewards` trait for the bridge and the open world. |
| [row_encoders.rs](../ps4/d1-server/crates/d1-activity-host/src/row_encoders.rs) | Field maps for sequences, actors, machines and trigger groups. The 56578 layouts of all seven kinds. |
| [replay.mjs](../tools/script-host/tools/replay.mjs), [script_replay.rs](../ps4/d1-server/crates/d1-activity-host/tests/script_replay.rs) | Records a transcript of one activity on content under the coverage driver, and replays it through a fake sidecar into the bridge. |

**The export.** For each of the 224 worlds, `export-bundles.mjs` writes every activity bundle its
slice sets load, in BullShcript's `ActivityBundleTag` shape. That is 1,692 bundle tags, one per
bundle hash. Each row has its name and the game's type id and index. From the row's host
definition it also has:

- the import identifier;
- a squad's cells and member names, from its squad definition;
- an objective's task list;
- a sequence's duration.

The export takes about 90 s and 44 MB.

**The sidecar on the export.** It reads `bundles_path`, or else `D1_BUNDLES_DIR/<activity>.json`.
Two things come from the scripts on top of content:

- **Archetypes.** An Encounter row takes its archetype from the scripts, by name.
- **Aliases.** Suppose a restored member's name prefixes exactly one content row of its type. The
  sidecar adds an alias entry that binds the member to that row. The content row is unchanged.

`ready.host_side` lists the incident listeners and `activity_flag` members. No bundle has a row for
either.

## The content export and the slice tables

**What content gives that stand-ins could not.**

- The real row indices. Stand-in indices were ours and never reached the wire.
- The bundles each world actually loads. The stand-ins guessed area encounters from names for 17
  activities. Content names them for all 62 activities that have a world.
- The squads' real cells and the objectives' task lists.
- The import identifiers that bind interactables and objects by hash.

**Slice → region.** A scenario's bubble states carry a name hash at `+0x04` (gibbon's `unknown04`).
It is the FNV-1 of the mission phase the state belongs to. The control: cine_reveal's states 25 and
26 are `cine_106_trv_phase` and `cine_107_twr_phase`, as the Fukugen note found.

`slice-states.tsv` commits those hashes for all 2,545 slice sets. The bridge resolves a phase's
region in this order:

1. the table's `regions=`, kept as an override;
2. the state named by the phase;
3. the bubble whose region hash is the phase's slice (its first valid private state).

**Slice → bundle.** The world tables already list each slice set's bundle tags, and the bridge maps
them to groups. When the client reports a new slice set (udt 21 `+0xB8`), the bridge:

- sends `active_bundles` with that set's bundles;
- relists the row table with that set's own sensor list, keeping every value, so the new list goes
  out in full.

The live host did the same at cine_reveal's move from 25 to 26: a full list at phase 4
([live captures](d1-live-captures-2026-11-05.md)). `D1_AH_SCRIPTS_ACTIVE_BUNDLES=0` turns the
bundle switching off.

**One lifetime state per frame.** Two lifetime states set in one sidecar message (cine_reveal's
pregame and cinematic hold) now go out in frames of their own, 1 then 4, as the live hosts sent
them.

## The activity table

`activities.txt` has a row for **138 activities**. These are every `ACTIVITY_TYPE_STORY`,
`STORY_CINEMATIC`, `CREDITS`, `QUEST` and `QUEST_SOLO` activity whose world the world tables bind,
with a lifetime record in its start set. The other 78 have no world yet, because no capture or 2014
row binds them. The table lists them at the end. Each row reads:

```
INDEX WORLD GROUP @generated start=N hash=H [kind=cinematic] type=H type2=H place=H [next=I:H]
```

- **The activity is the world.** The scripts name every activity after its world. 62 of the 63
  script activities match a world name; `city_tower_iron_banner` has no world.
- **`@generated`.** The list is the world's sensor list for `start=`, built from the world tables
  the way `D1_AH_WORLDS` builds it. GROUP is the group of its lifetime record.
- **The body's hashes**, all from the manifest:
  - `type2` is the manifest's activity type;
  - `type` is the same, except for a cinematic-only activity (`STORY_CINEMATIC`, `CREDITS`), whose
    live bodies carry Story there;
  - `place` is the manifest's place hash.

  This reproduces the cinematic bodies' Story / `0x4d78882e` pair
  ([wire catalogue](ps4-56578-wire-catalog-2026-11-05.md)). All 138 rows have a manifest
  definition.
- **`next=`.** This is the 2014 table's next activity, matched by world and mapped to 56578 by
  hash. Only cine_reveal (165) has one: 0, the Tower.

**What the scripts cover.**

| Rows | Worlds | Scripts |
|---:|---:|---|
| 80 | 25 | written mission scripts |
| 35 | 12 | start-only stubs |
| 23 | 6 | none in the folder: `mission_deja`, `quest_deadflower`, `mission_omen`, `mission_souvenir`, `mission_topknot`, `mission_turing` |

For the last group the sidecar refuses the session at `hello`, and the bridge serves the activity as
before.

## The arc: cine_reveal hands off to the Tower

In retail, a new Guardian went from cine_reveal to the Tower. Ours went to orbit
([cine-live13](ps4-56578-cine-live13-2026-11-05.md)).

The client chooses what to launch next from the profile it is served. After A Guardian Rises,
CONTINUE launched 165 once 164 was recorded complete
([post-game to the Tower](ps4-56578-continue-tower-2026-10-29.md)). The step-driven host recorded
nothing for 165. The sequencing is now:

1. The scripts complete 165 at the Tower cinematic (`activity_complete` 965, the cinematic chain).
2. At the results (lifetime 6), the bridge grants `Completed` for each joined character.
3. Right after that, because the row says `next=0:0x8ae8da69`, it reports the hand-off:
   `ActivityRewards::activity_hand_off(character, 165's hash, 0, the Tower's hash)`.
4. Then the lifetime goes to 8, the client's own end-of-activity transition.

Steps 2 and 3 reach the responder through stream O's forwarder. It records the completion and the
next activity before the client reaches 8. Until it is plugged in, `LoggedRewards` logs both. The
native test has to show that the client then launches the Tower.

## Rewards at the results

The bridge grants when the lifetime reaches **6**, not at the end:

- **Gameplay activities.** The 7 opens the post-game screens 30 s after the 6. Those screens show
  what the profile holds by then, so a grant at the end would come too late for them.
- **Cinematic-only activities.** The 8 is the client's transition to what comes next, which must
  already be decided (the arc above).

The other outcomes:

- A run that reaches its end without passing 6 is granted at `ended`, as `EndedIncomplete`.
- A run that stops early is `Abandoned`.

Each character is granted once per run.

## One rewards trait

`rewards::ActivityRewards` replaces G's trait (open_world.rs) and the bridge's. Its methods:

- `grant_activity_rewards(&mut self, &CharacterRef, activity_hash, &ActivityOutcome)`;
- `activity_hand_off(...)`, whose default does nothing.

**Keyed by the identity.** The trait is keyed by the client's own 40-byte identity. Its u64 is the
identity's first qword (`CharacterRef::key`), the word the completion body carries as its player.

**Both callers still work.**

- G's call sites keep their u64 members through `CharacterRef::from_key`.
- `open_world` re-exports the types, so G's paths still work.
- `ActivityOutcome` is one enum: the bridge's three mission outcomes and G's four open-world ones.
- Both services take `Arc<Mutex<dyn ActivityRewards + Send>>`.

**HOOK (stream O):** plug the forwarder into `ScriptConfig::rewards` and
`OpenWorldServices::rewards`.

## Single public events

Protocol 1 had no message for one event. It now has two:

- **The messages.** `event_start {bundle, event_id}` and `event_stop {bundle, event_id}` make the
  event's bundle join or leave the active set. All loaded bundles count as active until
  `active_bundles` names a set.
- **Why a bundle.** An event runs from its own bundle's scripts.
- **The answer.** The sidecar answers `event {state: started | stopped | refused}`. It refuses a
  bundle the activity does not load.
- **The bridge.** `BubbleScriptBridge::start_event` and `stop_event` send these messages once the
  scripts run, and refuse before that.

Two things are not decided here:

- The event's rows keep their values when it stops. No 56578 evidence says they reset.
- Which event of a bundle's rotation is which is still G's placeholder schedule.

## Row kinds: the 56578 layouts and what is mapped

**Where the layouts come from.** Every layout below is the executable's, from the RTTI tables
(#308). A test checks `row_encoders::ROW_LAYOUTS` against `fields.tsv` and `schemas.tsv`.

**Checked through the client's codec.** With the eboot, each new image goes through the client's
codec and comes back with its mapped fields intact:

- sequence, 97 bits;
- actor, 587 bits;
- machine, 101 bits;
- trigger group, 57 bits.

| Kind (id, A tag) | 56578 layout | Mapped from the script | Evidence for the meaning |
|---|---|---|---|
| sequence (5, `0x808029A1`, 32 B) | i32 +0, u8 +4, a counted list +8, a name ref +0x18 | `playing.startedAt` → start +0. `playing.id` → step +4, never 0xFF and always a new value. `target` → +0x18 | 56578 apply `0x40CE50` and evaluator `0x40D000`, native ([breach sequences](ps4-56578-breach-sequences-2026-10-31.md)): a new step other than 0xFF plays, and start + duration expires. TU0's SequenceAuth [3] is the target, and the final build moved the reference to +0x18. `pairs` is not mapped (logged) |
| actor (2, `0x80800868`, 2312 B) | i32/31 +0, i8 +4 and +5, bool +6, optional structs at +8, +0x2C, +0xB0 and +0x8C0 | Not placed → the client's default. Placed → `incarnation` at +0 (1 the first time), +4 = -1, +6 set, the binding absent (+0x50, +0x58). `commandScript` → cookie +0xB0, pc +0xB4, count +0xB8, 64-byte atoms at +0xC0 | `d1-member-states.py`'s image, native: the Breach entrances play. Only `scripted_sequence` atoms with end condition 0 or 1 have a known shape; any other atom leaves the script out (logged). Behaviour, drop and kill are not mapped |
| machine (40, `0x80800556`, 48 B) | optional performance +0 (cue u32, override u32, generation i32), optional cues +0xC | `performance.name` → +0, the empty override at +4, `performance.generation` → +8. No performance → the empty cue, which the client reads as none | 56578 apply `0x287230` and updater `0x21e730`; JOIN111's dispatched image ([Ghost light](ghost-light-0124.md)). Cues are not mapped |
| trigger group (30, `0x8080040E`, 12 B) | name ref +0, i8 2-bit +8 | `state` → +8 (-1..2); the volume ref is kept | Structural: +8 is the only other field, and its range is BullShcript's (-1 until a script writes it) |
| timer (17, `0x80800725`, 36 B) | timer struct +0 (`0x80800ab3`: bool, five i32, f32), bool +0x1C, i32 +0x20 | nothing (refused) | None says which i32 is which |
| hard-wipe globals (34, `0x808004A4`, 32 B) | bool +0, bool +1, i8 2-bit +2, the same timer struct +4 | nothing (refused) | One of the two bools is `enabled`; which one is not known |
| object filter (33, `0x80800750`, 400 B) | a 4-bit count and up to 8 typed predicates (`0x808042e0`) | nothing (refused) | The predicates' typed encoding is not mapped |

## The completion body's counters and session id

Neither is resolved:

- **The counters.** The eboot sets the seven counter ids in `0x24d3a0`, but their names are not among
  the executable's strings ([wire catalogue](ps4-56578-wire-catalog-2026-11-05.md)). Nothing in the
  descriptors or content says which four the live 51539 values are.
- **The session id** at `+0x30` has no reader that names it.

Both stay labelled stand-ins in [completion.rs](../ps4/d1-server/crates/d1-activity-host/src/completion.rs).

## Coverage, before and after

```sh
D1_SCRIPTS_DIR=<scripts> node tools/script-host/tools/coverage.mjs --seconds 600            # before: stand-ins
D1_SCRIPTS_DIR=<scripts> node tools/script-host/tools/coverage.mjs --seconds 600 --bundles <D1_BUNDLES_DIR>   # after: content
```

Each run is 600 s of script time with one player, under a synthetic driver:

- a cinematic finishes 2 s after it starts;
- a trigger a script arms fires 3 s later;
- a placed squad reports one member alive, then none 5 s later.

On content the driver adds two rules, both labelled in the tool:

- A squad with an import identifier also reports its losses as a tally. The runtime counts those
  squads by kills or by tally, not by the live count.
- A player trigger that the client arms from its own content (no script enables it) fires once a
  thread has polled it for 3 s.

The phases reached measure what the driver allowed, not what a player would see.

| | Stand-ins (before) | Content (after) |
|---|---:|---:|
| Compile | 63/63 | 63/63 |
| First phase (written activities) | 30/30 | 30/30 |
| Reach the end | 3 | **5** |
| Phases reached (sum over the activities) | 71 | **85** |
| Unresolved bindings | 31 | **0** |
| Thread failures | 2 | 1 |
| Missing natives (distinct) | 1 | 1 |

**Unresolved bindings.** All 31 on stand-ins were host-side by BullShcript's own design:

- 29 incident listeners;
- 2 `activity_flag` members of `act_mars_m07_lowergarden`.

The content export confirms that no bundle has a row for any of them. They are now listed apart, in
`ready.host_side` (31 paths over 16 activities), not counted as gaps. Content did not have to fill
them. It explains them.

**What content did.**

- **The ends.** Two more activities reach their end: `mission_pact` and `mission_darksnow`.
- **The phases.** Ten more activities get further. For example, A Guardian Rises goes from phase 2
  to phase 5 of 7, and `mission_pact` from 1 to 5.
- **The failures.** Both stand-in thread failures are gone (`quest_infidel` and
  `act_mars_m07_lowergarden`). Their rows exist in other bundles of the same world, which the export
  loads.
- **The aliases.** Thirteen restored member names now bind the one content row they prefix.

**What is still open.**

- **A new thread failure, in `mission_pact`.** That activity now gets further. When a content trigger
  fires, one squad's `place()` raises "attempt to call a table value" inside the runtime. The
  activity still ends. The cause is open: BullShcript or a restored name, not a missing native.
- **The one missing native** is `import_only_clear_slice_set_assignment`, still a logged stub.
  Content says nothing of its effect.
- **A false positive, removed.** Content loads the ambient bundles, whose scripts call
  `encounter_manager:add_ambient_encounter`. The static check listed it as missing in 23
  activities. BullShcript answers it in its encounter manager's own dispatcher, and the check now
  knows those four verbs.
- **Verbs on unplaced rows: 82 calls in 14 activities.** These are restored member names with no
  content row, such as the chest and thorium ambient scripts' `thorium_low_traffic` and
  `chest_dome_c_high`. Their content rows have other names. Where exactly one row is prefixed by a
  member's name, the alias binds it. Where none or several are, nothing is guessed.

Each activity (names and counts only; the lifetime is the wire value the bridge sends):

| Activity | Compiles | Phases | First phase | Phases reached (stand-ins → content) | Ends (stand-ins → content) | Unresolved bindings | Thread failures | Lifetime on content (wire) |
|---|---|---:|---|---:|---|---:|---:|---|
| cine_hangar | yes | 1 | yes | 1 → 1 | yes → yes | 0 → 0 | 0 → 0 | 1 → 4 → 6 → 8 |
| act_cosmo_m0_5_hangar | yes | 3 | yes | 1 → 1 | no → no | 2 → 0 | 0 → 0 | 1 → 3 |
| act_cosmo_m0_highway | yes | 7 | yes | 2 → 5 | no → no | 0 → 0 | 0 → 0 | 1 → 4 |
| act_cosmo_m1_dome_a | yes | 3 | yes | 1 → 1 | no → no | 1 → 0 | 0 → 0 | 1 → 3 |
| act_cosmo_m2_dome_c | yes | 5 | yes | 3 → 3 | no → no | 2 → 0 | 0 → 0 | 1 → 3 |
| act_cosmo_m4_dome_c | yes | 4 | yes | 1 → 1 | no → no | 1 → 0 | 0 → 0 | 1 → 3 |
| cine_reveal | yes | 2 | yes | 2 → 2 | yes → yes | 0 → 0 | 0 → 0 | 1 → 4 → 6 → 8 |
| cosmo_portal_1 | yes | 8 | yes | 2 → 2 | no → no | 0 → 0 | 0 → 0 | 1 → 3 |
| mission_elevator | yes | 5 | yes | 4 → 4 | no → no | 1 → 0 | 0 → 0 | 1 → 3 |
| mission_pact | yes | 5 | yes | 1 → 5 | no → yes | 0 → 0 | 0 → 1 | 1 → 3 → 6 → 8 |
| quest_bonzo_dlc01 | yes | 2 | yes | 2 → 2 | no → no | 0 → 0 | 0 → 0 | 1 → 3 |
| quest_infidel | yes | 4 | yes | 3 → 3 | no → no | 0 → 0 | 1 → 0 | 1 → 3 |
| mission_touchdown | yes | 6 | yes | 4 → 4 | no → no | 1 → 0 | 0 → 0 | 1 → 4 → 3 |
| mission_darksnow | yes | 7 | yes | 4 → 7 | no → yes | 0 → 0 | 0 → 0 | 1 → 4 → 3 → 4 → 6 → 8 |
| act_mars_m01_valley | yes | 6 | yes | 2 → 3 | no → no | 4 → 0 | 0 → 0 | 1 → 4 → 3 |
| act_mars_m05_warbase | yes | 9 | yes | 3 → 3 | no → no | 4 → 0 | 0 → 0 | 1 → 3 |
| act_mars_m06_subway | yes | 8 | yes | 2 → 2 | no → no | 4 → 0 | 0 → 0 | 1 → 3 |
| act_mars_m07_lowergarden | yes | 10 | yes | 4 → 4 | no → no | 2 → 0 | 1 → 0 | 1 → 3 |
| mars_portal_3 | yes | 6 | yes | 1 → 3 | no → no | 0 → 0 | 0 → 0 | 1 → 3 |
| act_moon_m1_launchtubes | yes | 9 | yes | 2 → 2 | no → no | 4 → 0 | 0 → 0 | 1 → 4 → 3 |
| act_moon_m3_lowercitadel | yes | 5 | yes | 2 → 2 | no → no | 1 → 0 | 0 → 0 | 1 → 3 |
| act_moon_m5_strength | yes | 8 | yes | 7 → 7 | no → no | 1 → 0 | 0 → 0 | 1 → 3 → 4 |
| moon_portal_2 | yes | 6 | yes | 1 → 1 | no → no | 1 → 0 | 0 → 0 | 1 → 3 |
| strike_vampire | yes | 8 | yes | 1 → 1 | no → no | 0 → 0 | 0 → 0 | 1 → 3 |
| act_venus_m01_engineering | yes | 6 | yes | 3 → 4 | no → no | 1 → 0 | 0 → 0 | 1 → 3 → 4 |
| act_venus_m07_northern_tides | yes | 5 | yes | 1 → 1 | no → no | 0 → 0 | 0 → 0 | 1 → 3 |
| mission_pursuit | yes | 5 | yes | 5 → 5 | yes → yes | 0 → 0 | 0 → 0 | 1 → 3 → 6 → 8 |
| mission_traitor | yes | 6 | yes | 2 → 2 | no → no | 1 → 0 | 0 → 0 | 1 → 3 |
| quest_safari | yes | 5 | yes | 3 → 3 | no → no | 0 → 0 | 0 → 0 | 1 → 3 |
| venus_portal_1 | yes | 0 | yes | 1 → 1 | no → no | 0 → 0 | 0 → 0 | 1 → 3 |

Start-only stubs (no mission scripts; they compile and load, and the host decides their lifetime), 33: act_cinematic_queen_101, act_cinematic_queen_102, cine_end, cine_speaker, city_tower_default1, city_tower_iron_banner (no world), city_tower_queen, act_cosmo_m5_refinery, act_cosmo_m6_rasputin, cosmo_portal_2, act_mars_m03_penthouse, mars_portal_1, mars_portal_2, act_moon_m4_uppercitadel, act_moon_m6_reactor, act_moon_m7_selection, pvp_anomaly, pvp_cabal_camp, pvp_city_defense, pvp_colony_ship, pvp_factory, pvp_greenhouse, pvp_marsbattle, pvp_moonbase, pvp_rooftops, pvp_venus_hcc, pvp_vex_tube, social_landing, act_venus_m02_southern_tides, act_venus_m05_vault, act_venus_m06_ketch, venus_portal_2, venus_raid_1.

## Knobs

| Knob | Meaning |
|---|---|
| `D1_BUNDLES_DIR` | The private content export (`export-bundles.mjs`), outside the repository. The sidecar and the bridge look up `<world>.json` there when the table gives no `bundles=`. |
| `D1_AH_SCRIPTS_ACTIVITIES` | The bridge's table; `ps4/fixtures/56578/scripts/activities.txt` is the generated one. Rows may now say `@generated` for LIST, `world=`, `start=` and `next=I:H`. |
| `D1_AH_WORLDS` | The world tables (default `ps4/fixtures/56578/worlds`), for `@generated` lists and the slice tables. |
| `D1_AH_SCRIPTS_SLICE_STATES` | The slice states (default `ps4/fixtures/56578/scripts/slice-states.tsv`). |
| `D1_AH_SCRIPTS_START_PHASE=INDEX:PHASE[,...]` | Lab only, default off, process-wide (every join of that activity in the process is served the phase's region, later players and public instances too). Tests the end of a mission without walking it: the scripts start at that phase (`hello.phase_index`; earlier phases never run), the join serves the phase's `regions=` entry instead of `start=` (no entry: a warning, `start=` stands), and the lifetime is brought to gameplay after the scripts' first value. Seen offline-then-in-game on build 56578, emulator 1ac34e4: run gh62 (`164:5`) the Guardian spawned in the hangar area with the squads placed; run gh64 (`164:6`) phase 6 ran in 105, the real completion, the outro, the rewards screen, then the reveal and the Tower. The regions for AGR's phases 5 and 6 (104, 105) are the bridge's own phase log lines (computed from the game's content); they lived in a private table for those runs, not in the committed table. `164:5` is an example, not a profile setting. |
| `D1_AH_SCRIPTS_SCOREBOARD=1` | Off by default (baseline); on in the full-game profile. At the scripts' `activity_complete` the host sets the activity world's kind-15 scoreboard row (the activity's own sensor group, `0x4786C0E0` for A Guardian Rises; kind `0xF`, index 0; A tag `0x808008D0`) with one player row per joined player, each named by the player bundle's key (identity `+0x18`, byte-reversed): kills and deaths counted from the client's udt 18 reports of the run (the Crucible driver's classification: 1825 `kill`/734 `any_kill`, 2079 `player_died`; with exactly one player joined, else not attributed and logged once). Precision kills are not counted (no client incident for them found in the catalogue); the rewards array stays empty. The row goes before the completion incidents and is kept set; the sense49 order rule needs it at least ~10 s before lifetime 7, and the host only logs a warning when the 7 follows sooner. Seen in game on build 56578, emulator 1ac34e4: after A Guardian Rises' scripted ending (`D1_AH_SCRIPTS_START_PHASE=164:6`) the ACTIVITY REWARDS screen had no rows without the row (gh64) and with a row naming the player by identity `+0x08` (sb1); with the bundle's key it lists the player with kills 0, precision kills 0, deaths 0 (sb2; that run fought nothing). Counting during a fight is unit-tested only. |
| `D1_AH_SCRIPTS_INTERACTABLE=1` | Lab, off by default (empty in the baseline and full-game profiles). Restoration's first objective (activity 166) needs the placement the script binds to be usable. When the scripts raise or clear an activity flag, the host writes it into every served player bundle row (A `0x808022A5`: count at `+0x4C`, raw u32 hashes from `+0x50`, capacity 8) as a delta, in the scripts' own order. When a script makes a placement interactable for a filter (`set_enabled_player_filter` / `set_disabled_player_filter`), the object row carries two commands, the mission-interactable `0x80802140` (mode 1 enabled for the filter's players, 2 disabled, and the filter row's reference) and the device command at rest, and the filter row (kind `0x21`) holds the flag predicate `0x80800952` with the flag's raw hash; the pairing comes from the script's own calls, nothing is hard-coded. Evidence: a private capture of the live servers serving Restoration, in which both objects and filters and the player rows have exactly this shape, and the 56578 descriptors for every offset (a test per offset). Offline only: unit tests and the client's codec round trip; nothing seen in game yet. Other predicates, interactables without a filter, cross-host routing and the new-user-experience row are not encoded. |
| `D1_AH_SCRIPTS_ACTIVE_BUNDLES=0` | Do not switch the active bundles on slice changes. |

## How to rebuild and test

```sh
node tools/script-host/tools/export-bundles.mjs <dump>/CUSA00219/packages <D1_BUNDLES_DIR>        # private, ~90 s
node tools/script-host/tools/export-bundles.mjs <dump>/CUSA00219/packages --tables ps4/fixtures/56578/scripts
python tools/ps4_setup/d1-scripts-table.py --manifest <dim-d1-manifest> --table-2014 <tiger>/packages/dictionary/destiny1/v41375_14_07_03_1501_live/activities.csv
cd tools/script-host && node --test                                                              # 41; the content ones skip without inputs
python -m unittest tests/test_scripts_table.py                                                   # regeneration with D1_MANIFEST_DIR and D1_TABLE_2014
cargo test -p d1-activity-host                                                                   # the replay needs D1_SCRIPTS_DIR, D1_BUNDLES_DIR, D1_NODE
```

## Open

- The native run: cine_reveal and A Guardian Rises script-driven on content, then the Tower after 165.
- The timer, hard-wipe globals and object filter meanings; the actor's other command atoms; sequence pairs; machine cues.
- The restored names with no content row (82 calls). `mission_pact`'s thread failure is a hash collision in BullShcript's member lookup, not ours ([the wiring note](ps4-56578-integration-wiring-2026-11-05.md), row 21).
- The 78 story, cinematic and quest activities with no world, and the six worlds with no script.
- The completion body's counters and session id.
