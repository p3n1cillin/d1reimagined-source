# PS4 56578: the mission-script sidecar (5 November 2026)

This is stream A1 of the [research record](ps4-56578-research-record-2026-11-05.md): a Node
process that runs the restored 56578 mission scripts on Blam Network's MIT BullShcript compiler
and VM. It talks to `d1-activity-host` over JSON lines. The code is in
[tools/script-host/](../tools/script-host/), and the wire contract is
[tools/script-host/PROTOCOL.md](../tools/script-host/PROTOCOL.md). Stream A2 (`scripts/host-bridge`)
builds the Rust side: the row table, the row encoders and the bridge.

**What these claims cover.** Everything here was built and unit-tested offline. No game,
emulator, lab or capture was run, so nothing here is an in-game result. The integrated test
comes after A2 lands. The coverage figures come from runs on **stand-in bundles** (below), not on
content. The phases reached are what a synthetic driver allowed, not what a player would see.

## Sources and licences

- **BullShcript** (MIT, Blam Network / Codie), from the local tiger checkout at `82c57cb`
  (1 November 2026). Its nearest build module is `v52553_16_09_28_1551_evil` (PS4 01.29), one patch
  before ours. The sidecar imports the checkout's built `dist/` in place and vendors none of it.
  [THIRD_PARTY.md](../THIRD_PARTY.md) records the dependency.
- **The 56578 scripts**: Fukugen Daihon `v56578_17_04_12_1251_evil`, 380 files. They are
  unlicensed, so [licensing rule 2's exception](../ps4/design/licensing.md) applies. The sidecar
  reads them from `D1_SCRIPTS_DIR` at run time and compiles them in memory. Nothing derived from
  them is written to disk or committed, and this note gives activity names and counts only. The
  tests run on scripts we wrote ourselves; the one test that reads the real folder skips without
  it.
- **The lifetime numbering** is Codex's mapping from #304.
- **The incident ids** come from Codex's catalogue (#306,
  `ps4/fixtures/56578/incidents-56578.tsv`), read when present. The four ids that #304 resolved
  (891, 965, 1726 and 2681) are built in.

## What the sidecar does

- At `hello`, it loads the whole folder and compiles it as one project, in about 0.3 s.
- It runs one BullShcript `ScriptRuntime` per activity instance, on 01.29's row type ids. It adds
  56578's type 39: our lists use that id, and BullShcript's TU0 binder table names it
  `new_user_experience`.
- Time moves only when the host sends `tick`: one script tick per message, at 10 Hz by default to
  match the udt 5 stamp. A run therefore replays exactly from its input lines (tested).
- Host events go in: `trigger`, `cinematic_finished`, `squad_alive`, catalogued `incident`s and
  the rest of BullShcript's `report*` inputs.
- Results come out:
  - `rows`: BullShcript's `ItemChange`, unchanged, except that lifetime changes gain `wire.phase`;
  - `phase`;
  - `activity_complete` (incident 965);
  - `ended`;
  - the hard-wipe signals.

**Stand-in bundles.** The runtime binds script members to bundle rows by name. Until A2's content
export exists, the sidecar builds the rows itself from the scripts' own declarations:

- **Names.** Rows are named as the activity bundles name them: `<phase>.<member>`, then
  `<phase>.<member>.<archetype member>`.
- **Types.** Each row is typed by its native type, from a table we derived by joining
  BullShcript's MIT TU0 row tables to the TU0 scripts.
- **Types with no TU0 pairing.** `device`, `interactable` and `capturable` are put on Object rows
  and logged as stand-ins.
- **Squads.** A squad with `@member_counts` gets those members as its definition.
- **Indices.** Stand-in `typeIndex` values are ours, never the game's, and must not reach the
  wire.

Area encounters (`enc_*`) are separate bundles, and only content says which ones an activity
loads. The coverage run guesses them from the mission tag in the name. For example,
`act_cosmo_m0_highway` loads the four `enc_*_m0` area encounters. The guess applies to 17
activities.

## Coverage

The command is `D1_SCRIPTS_DIR=<scripts> node tools/script-host/tools/coverage.mjs --seconds 600`.
It runs each activity for 600 s of script time, with one player spawned, under a **synthetic
driver** that stands in for the client:

- a cinematic finishes 2 s after it starts;
- an armed player trigger fires 3 s after it is armed;
- a placed squad reports one member alive, then none 5 s later.

Nothing else is simulated.

- **Activities:** 63 names from 61 roots (the Tower root names three). 30 are written missions.
  The other 33 are start-only: the 29 stubs, the three Tower names and the Reef.
- **Compile:** 63/63, with no diagnostics over all 380 files.
- **Load:** 63/63.
- **First phase:** all 30 written activities reach their first phase, with or without the driver.
- **End:** with the driver, three activities end, all through lifetime 1 → … → 6 → 8:
  `cine_reveal`, `cine_hangar` and `mission_pursuit`. The reveal goes 1 → 4 → 6 → 8, the order
  the live captures show (0, 1, 4, play, 6, 8).
- **Missing natives:** **one name**, `import_only_clear_slice_set_assignment`. Three strike
  activities call it through the dialogue-trigger archetype, 16 calls in the run. Two independent
  static sweeps agree:
  - every `self.<native member>:<verb>()` call, checked against BullShcript's verb table;
  - every `.name(` call in the scripts, checked against every name BullShcript answers anywhere.

  It is now a logged 56578 stub: a command with no known effect. It answers true and logs
  `stub56578` on each call.
- **`cinematic_midmission` is not a missing type.** It is a utility archetype (`.esu`), and it
  compiles and runs as one: the reveal plays both of its cinematic phases through it. Compiling a
  folder without `es/utility_scripts/` would fail on the type, but that is a missing input, not a
  missing 56578 type.

The table below is with the driver on. Two of its columns need explaining:

- **Unresolved bindings** are BullShcript's own unresolved import slots. On stand-ins these are
  mostly interactable listeners and objects bound by identifier hash, which only content can
  resolve.
- **Verbs on unplaced rows** counts calls to known verbs that answered "unknown" because their row
  is not where the stand-ins put it.

| Activity | Compiles | Phases | First phase | Phases reached | Lifetime (wire) | Ended | Missing natives (static / run) | Unresolved bindings | Verbs on unplaced rows | Thread failures |
|---|---|---:|---|---:|---|---|---:|---:|---:|---:|
| act_cinematic_queen_101 (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| act_cinematic_queen_102 (stub) | yes | 1 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| cine_end (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| cine_hangar | yes | 1 | yes | 1 | 1 → 4 → 6 → 8 | yes | 0 / 0 | 0 | 0 | 0 |
| cine_speaker (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| city_tower_default1 (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| city_tower_iron_banner (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| city_tower_queen (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| act_cosmo_m0_5_hangar | yes | 3 | yes | 1 | 1 → 3 | no | 0 / 0 | 2 | 0 | 0 |
| act_cosmo_m0_highway | yes | 7 | yes | 2 | 1 → 4 | no | 0 / 0 | 0 | 0 | 0 |
| act_cosmo_m1_dome_a | yes | 3 | yes | 1 | 1 → 3 | no | 0 / 0 | 1 | 0 | 0 |
| act_cosmo_m2_dome_c | yes | 5 | yes | 3 | 1 → 3 | no | 0 / 0 | 2 | 0 | 0 |
| act_cosmo_m4_dome_c | yes | 4 | yes | 1 | 1 → 3 | no | 0 / 0 | 1 | 0 | 0 |
| act_cosmo_m5_refinery (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| act_cosmo_m6_rasputin (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| cine_reveal | yes | 2 | yes | 2 | 1 → 4 → 6 → 8 | yes | 0 / 0 | 0 | 0 | 0 |
| cosmo_portal_1 | yes | 8 | yes | 2 | 1 → 3 | no | 0 / 0 | 0 | 0 | 0 |
| cosmo_portal_2 (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| mission_elevator | yes | 5 | yes | 4 | 1 → 3 | no | 0 / 0 | 1 | 0 | 0 |
| mission_pact | yes | 5 | yes | 1 | 1 → 3 | no | 0 / 0 | 0 | 0 | 0 |
| quest_bonzo_dlc01 | yes | 2 | yes | 2 | 1 → 3 | no | 0 / 0 | 0 | 0 | 0 |
| quest_infidel | yes | 4 | yes | 3 | 1 → 3 | no | 0 / 0 | 0 | 0 | 1 |
| mission_touchdown | yes | 6 | yes | 4 | 1 → 4 → 3 | no | 0 / 0 | 1 | 0 | 0 |
| mission_darksnow | yes | 7 | yes | 4 | 1 → 4 → 3 | no | 0 / 0 | 0 | 0 | 0 |
| act_mars_m01_valley | yes | 6 | yes | 2 | 1 → 4 | no | 0 / 0 | 4 | 0 | 0 |
| act_mars_m03_penthouse (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| act_mars_m05_warbase | yes | 9 | yes | 3 | 1 → 3 | no | 0 / 0 | 4 | 0 | 0 |
| act_mars_m06_subway | yes | 8 | yes | 2 | 1 → 3 | no | 0 / 0 | 4 | 0 | 0 |
| act_mars_m07_lowergarden | yes | 10 | yes | 4 | 1 → 3 | no | 0 / 0 | 2 | 0 | 1 |
| mars_portal_3 | yes | 6 | yes | 1 | 1 → 3 | no | 1 / 1 | 0 | 0 | 0 |
| mars_portal_1 (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| mars_portal_2 (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| act_moon_m1_launchtubes | yes | 9 | yes | 2 | 1 → 4 → 3 | no | 0 / 0 | 4 | 0 | 0 |
| act_moon_m3_lowercitadel | yes | 5 | yes | 2 | 1 → 3 | no | 0 / 0 | 1 | 0 | 0 |
| act_moon_m4_uppercitadel (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| act_moon_m5_strength | yes | 8 | yes | 7 | 1 → 3 → 4 | no | 0 / 0 | 1 | 0 | 0 |
| act_moon_m6_reactor (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| act_moon_m7_selection (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| moon_portal_2 | yes | 6 | yes | 1 | 1 → 3 | no | 1 / 1 | 1 | 0 | 0 |
| strike_vampire | yes | 8 | yes | 1 | 1 → 3 | no | 0 / 0 | 0 | 0 | 0 |
| pvp_anomaly (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| pvp_cabal_camp (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| pvp_city_defense (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| pvp_colony_ship (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| pvp_factory (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| pvp_greenhouse (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| pvp_marsbattle (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| pvp_moonbase (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| pvp_rooftops (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| pvp_venus_hcc (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| pvp_vex_tube (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| social_landing (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| act_venus_m01_engineering | yes | 6 | yes | 3 | 1 → 4 | no | 0 / 0 | 1 | 0 | 0 |
| act_venus_m02_southern_tides (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| act_venus_m05_vault (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| act_venus_m06_ketch (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| act_venus_m07_northern_tides | yes | 5 | yes | 1 | 1 → 3 | no | 0 / 0 | 0 | 1 | 0 |
| mission_pursuit | yes | 5 | yes | 5 | 1 → 3 → 6 → 8 | yes | 0 / 0 | 0 | 0 | 0 |
| mission_traitor | yes | 6 | yes | 2 | 1 → 3 | no | 0 / 0 | 1 | 0 | 0 |
| quest_safari | yes | 5 | yes | 3 | 1 → 3 | no | 0 / 0 | 0 | 0 | 0 |
| venus_portal_1 | yes | 0 | yes | 1 | 1 → 3 | no | 1 / 1 | 0 | 0 | 0 |
| venus_portal_2 (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |
| venus_raid_1 (stub) | yes | 0 | n/a | 0 | none | no | 0 / 0 | 0 | 0 | 0 |

| Missing native | On | Activities calling it | Activities hitting it in the run (calls) | Stubbed |
|---|---|---:|---:|---|
| `import_only_clear_slice_set_assignment` | - | 3 | 3 (16) | yes |

Without the driver (`--idle`), every written activity still reaches its first phase and then
stops at its first client gate. The lifetime reaches pregame (wire 1), then active (3) or the
cinematic hold (4).

## What this does not show

- **Content.** The runtime needs 56578 bundle tables in BullShcript's `ActivityBundleTag` shape:
  - row names and indices;
  - squad definitions;
  - objective task trees;
  - nav points;
  - command scripts;
  - sequence durations;
  - the slice → bundle lists.

  Stand-ins show that the scripts run. They say nothing about the indices the client expects.
- **Wire meaning.** Where BullShcript answers a native, it answers with 01.29 semantics. Whether
  the 56578 client reads the same state is for A2's encoders to establish, checked against the
  live captures.
- **Thread failures on stand-ins.** There are two: one in `quest_infidel` and one in
  `act_mars_m07_lowergarden`. In both, a script indexes a member that another bundle owns or that
  content would supply. Recheck both on content before treating either as a script bug.
- **Where runs stop.** Under the driver, AGR stops at phase 2. Its threads wait on player triggers
  that no script arms (`pt_bubble` and the Breach spawns), which the client arms from content. The
  driver only fires triggers that a script arms.

## How to run it

```sh
export D1_SCRIPTS_DIR=<tiger>/packages/fukugen-daihon/destiny1/v56578_17_04_12_1251_evil
export D1_TIGER_DIR=<tiger>            # or D1_BULLSHCRIPT_DIR, or a sibling third-party/ checkout
cd tools/script-host
node --test                            # 28 tests; the 56578 ones skip without D1_SCRIPTS_DIR
node tools/coverage.mjs --seconds 600  # the table above (add --idle, --waits, --json FILE)
node src/sidecar.mjs < session.jsonl   # one session, JSON lines in and out
```

First build BullShcript in the tiger checkout: `npm install`, then
`npm run build -w @blamnetwork/bullshcript`.
