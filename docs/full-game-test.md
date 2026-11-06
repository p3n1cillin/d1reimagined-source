# The full-game test

One command starts the 56578 lab session with every subsystem built on 5 November switched on:
the player store, social services and vendors, rewards, multiplayer, the activity framework,
generated worlds, public instances, the native RSAT codec, the udt 5 stamp, the script bridge and
its sidecar, and the emulator fork's draw-cost and PC-client work.

Each of these was built and unit-tested on its own, behind an off-by-default knob. **None of them
has run in the game yet.** This test is the first time they run together. Nothing on this page is
an in-game result until a run shows it, and a quiet log is not a working subsystem.

- The launcher is [`tools/ps4_setup/d1-full-game.ps1`](../tools/ps4_setup/d1-full-game.ps1).
- The profiles are in [`ps4/profiles/`](../ps4/profiles):
  - [`full-game.env`](../ps4/profiles/full-game.env): everything on;
  - [`baseline.env`](../ps4/profiles/baseline.env): the same session with all of it off;
  - [`lab-base.env`](../ps4/profiles/lab-base.env): the session both include, the one
    `d1-lab-served.ps1` and `ending-run.ps1` serve. Its header describes the file format.
- `tests/test_full_game_profile.py` checks the profiles and the dry run.

## Before you start

You need the same inputs as any lab run. The launcher checks each one and names what is missing.

- **The owned build 56578.** The launcher reads its eboot, its `sce_sys/param.sfo` and the served
  manifest through `capture-rig/builds.json` in the workspace (`D1_WORKSPACE`), and refuses an image
  that is not the pinned one.
- **The emulator**, `D1_SHADPS4\shadPS4.exe`. The deployed build (98542a7, fork #6) does not have
  fork #7's or #8's knobs. The launcher drops those and says so (see [the A/B](#the-ab-for-fork-7-and-8)).
- **The contained lab network.** Set up once with `tools/ps4_setup/d1-lab-net.cmd`. The contained
  boot checks it on every start and refuses on any failure:
  - every firewall profile is on;
  - the emulator has its outbound block;
  - the lab address is on the KM-TEST adapter, with no default route;
  - the hosts redirects are applied.

  **The launcher never changes firewall rules, adapters or hosts entries.** If a check fails, fix
  the setup, not the check.
- **For the script bridge:**
  - the local Blam Network tiger checkout (`TIGER`, default `D1_ROOT\third-party\Blam-Network_tiger`),
    with BullShcript built (`npm install`, then `npm run build -w @blamnetwork/bullshcript`);
  - its Fukugen Daihon 56578 scripts (`D1_SCRIPTS_DIR`). These are local inputs. They are never
    vendored or committed ([Blam Network note](../notes/ps4-56578-fukugen-daihon-2026-11-05.md)).
  - Node 22 (`NODE`, default `D1_TOOLS\node\node.exe`, else `node` on PATH).
  - Stream K's private content bundle export, `D1_BUNDLES_DIR` (set it in the environment, or
    `-Set D1_BUNDLES_DIR=<folder>`). It is required while `D1_AH_SCRIPTS=1`; without it, the run
    refuses. To run without the bridge, pass `-Set D1_AH_SCRIPTS=`.
- **Cargo**, when the servers need building. The launcher builds into this checkout's own
  `ps4/d1-server/target` and ignores `CARGO_TARGET_DIR`, because one target folder shared between
  worktrees has corrupted builds. `-TargetDir` chooses another folder. d1-oracle's Unicorn bindings need libclang (`LIBCLANG_PATH`); when
  it is unset, the launcher uses `D1_TOOLS\llvm*\bin`.
- **A quiet machine.** Builds, disk-heavy jobs and Ghidra stall the client
  ([lab stalls](../notes/ps4-56578-lab-stalls-2026-11-05.md)). The launcher builds before it starts
  anything and refuses to start over a running lab.

## Running it

```powershell
# Print everything, start nothing: the knobs, the inputs, the build plan and the command.
pwsh -File tools/ps4_setup/d1-full-game.ps1 -DryRun

# The full-game session: build the servers if they are stale, then the servers and the emulator.
pwsh -File tools/ps4_setup/d1-full-game.ps1 -Name fg1

# The baseline: the same session with the new subsystems off.
pwsh -File tools/ps4_setup/d1-full-game.ps1 -Profile baseline -Name base1

# Servers only, then the emulator against them (two run folders).
pwsh -File tools/ps4_setup/d1-full-game.ps1 -Only servers -Name fg2
pwsh -File tools/ps4_setup/d1-full-game.ps1 -Only emulator -Name fg2e

# Override knobs for one run (later wins). A key the profile lacks needs its section.
pwsh -File tools/ps4_setup/d1-full-game.ps1 -Name fg3 -Set D1_RSAT_NATIVE=0,server:D1_MP_FIRETEAM_CAP=6
```

| Switch | What it does |
|---|---|
| `-Profile NAME\|PATH` | A profile in `ps4/profiles` (`full-game`, `baseline`) or any `.env` file. |
| `-Only all\|servers\|emulator` | `servers` holds the servers until `RUN\boot\stop` exists or `MINUTES` pass. `emulator` needs servers already listening on the lab address. |
| `-Set KEY=VALUE,...` | Per-knob overrides. An empty value unsets the knob. `[launch]` keys (`ACTIVITY`, `MINUTES`, ...) work too. |
| `-Build` / `-NoBuild` | Always run cargo / never build (refuse if stale). By default it builds only when stale. |
| `-DryRun` (`-Json`) | Resolve and check everything and print it (as JSON with `-Json`). Nothing is built, written or started. |
| `-TargetDir` | The cargo target folder (default this checkout's `ps4/d1-server/target`). |

**Stale servers.** The launcher reads each binary's cargo dep-info beside it. A binary is rebuilt
when it is missing, when a source file or `Cargo.toml` is newer than it, or when it was built from
another checkout. That last case happens when worktrees share a target folder (`-TargetDir`): cargo
finds this checkout's crates fresh and leaves the other checkout's binary in place. The launcher
touches that binary's `main.rs` and builds again. Builds run at BelowNormal priority. The built binaries are
copied into the run folder, and the servers run from that copy, so the shared served target is
never swapped.

**Knob checks.** Every knob's name must be in the code that reads it:
- `[server]` knobs in the servers' binaries;
- `[sidecar]` knobs in `tools/script-host`;
- `[emulator]` knobs in `shadPS4.exe`.

A plain knob that is missing stops the run, because it was renamed or the build is stale. A `?KEY`
knob belongs to code that is not merged or not deployed. When its name is absent, it is dropped and
reported as `dropped`.

**The run folder** is `D1_STAGING\full-game\<utc>-<Name>\`:

| File | What |
|---|---|
| `manifest.json` | Every knob with its value, its state (`on`, `unset`, `dropped`) and its source line; the checkout revision; each binary's path, origin and SHA-256; the emulator's SHA-256; the inputs checked; the command. |
| `plan.txt` | The dry-run printout of this run. |
| `server.env`, `emulator.env` | The knobs passed (an empty value unsets). |
| `launcher.log`, `build-1.log` (`build-2.log`) | The launcher's steps and cargo's output (a second build when a shared target held another checkout's binaries). |
| `bin\` | The servers this run uses. |
| `boot\` | The contained boot's evidence: `receipt.json`, `server-environment.txt`, `responder.log`, `activity-host.log`, `shadnet.log`, `emulator.log`, frames. |
| `launched-activity.txt`, `store-control.txt`, `profile-push.bin`, `account-push.bin`, `ah-control.txt` | The live files the servers read. Write `profile-push.bin` here to push a profile. |

The player store is the exception. It lives in `D1_STAGING\player-store-full-game`, so the Guardian
carries over between full-game runs. Delete that folder to start a new Guardian.

**Stopping.** A full run ends when the emulator exits or after `MINUTES` (240). The contained boot
then stops its servers. A `-Only servers` run stops when `RUN\boot\stop` is created.

## The profile

| Subsystem | From | Knobs on in `full-game.env` | Off in `baseline.env` |
|---|---|---|---|
| Player store | #307 | `D1_STORE=1`, `D1_STORE_DIR`, `D1_STORE_PLAYER=local`, `D1_STORE_CONTROL`, and the four progression tables from this checkout | all unset |
| Social services and vendors | #309 | `D1_SOCIAL=1`, `D1_DISMANTLE_PLACEHOLDER=1`; `D1_SOCIAL_STOCK=0` (the stock rotation is a stand-in and the client lists everything, so it stays off) | unset |
| Rewards and growth | #311 | `D1_REWARDS=1`, `D1_REWARDS_SEED=0xD1F0` | unset |
| Multiplayer | #312 | `D1_MP=1`, `D1_MM_CONFIG_FILE`, `D1_AH_SHARED_INSTANCES=1` | unset |
| Activity framework | #313 | `D1_AH_ACTIVITY_FRAMEWORK=<fixtures>/activities` | unset |
| Generated worlds | #314, stream Q | `D1_AH_WORLDS`, `D1_AH_WORLDS_ACTIVITIES=all` (every activity with a world), `D1_AH_WORLDS_PRIVATE_START=1` | unset |
| Public instances | #316 | `D1_AH_PUBLIC_HOST=1`, `D1_AH_PUBLIC_TABLE`, `D1_AH_DISTINCT_HOST_IDS=1`; stream O2: `D1_AH_PUBLIC_INSTANCES_FILE` (the host's instances for the responder's matchmaker) | unset |
| Native RSAT codec | #317 | `D1_RSAT_NATIVE=1` | unset |
| udt 5 stamp mode | #298 | `D1_AH_SENSOR_STAMP=1` | unset |
| Script bridge | #319 (stream A2) | `D1_AH_SCRIPTS=1`, `D1_AH_SCRIPTS_SIDECAR`, `D1_AH_SCRIPTS_ACTIVITIES` (a stand-in, [165 cine_reveal](../ps4/profiles/script-activities.txt), until stream K's table lands), `D1_AH_SCRIPTS_INCIDENTS`. Listed as off: `D1_AH_SCRIPTS_MISSION_PHASE` (unverified), `D1_AH_SCRIPTS_SCOREBOARD_COLUMNS`, `D1_AH_ROWS_UNLISTED`, `D1_AH_ROWS_DELTA_INIT` | unset |
| Content bundles | stream K (not merged) | `D1_BUNDLES_DIR` (`[sidecar]`, from the environment; required with the bridge on) | unset |
| Script sidecar | #310 | `D1_SCRIPTS_DIR`, `D1_TIGER_DIR` (`[sidecar]`) | unset |
| Draw cost | fork #7 (draft) | `?D1_DRAW_STATS=1`, `?D1_DRAW_FAST=1`, `?D1_GPU_WAIT_YIELD=1` | `D1_DRAW_FAST=0`, the rest unset |
| PC client | fork #8 (draft) | `?D1_EXCEPTION_HANDLERS=1`, `?D1_CLEAN_EXIT=1`, `?D1_HITCH_HALT=0`, `?D1_AUDIO_SYNC=1`, `?D1_PC_LAYOUT=1`, `?D1_PAD_KEEP_KEYBOARD=1` | `D1_EXCEPTION_HANDLERS=0`, `D1_CLEAN_EXIT=0`, the rest unset |
| PC controls | fork d1 (#113) | `?D1_MOUSE_CURSOR=1`, `?D1_MOUSE_BUTTONS=1`, `?D1_MOUSE_LOOK=1` | unset |

Some choices in this profile are worth knowing about:

- **`D1_AH_WORLDS_ACTIVITIES=all`.** Since stream Q the profile serves every activity the world
  tables bind: 534 of 653, from the client's own activity table
  ([worlds note](../notes/ps4-56578-worlds-2026-11-05.md)). Each distinct world and start costs two
  udt 5 frames before the host listens; `tests/all_worlds.rs` measures that cost (see the note).
  The hand table's rows (164, 165, 168, 0, 410, 416) still win over generated ones. To serve a few,
  pass `-Set D1_AH_WORLDS_ACTIVITIES=166,167`.
- **Public starts.** `D1_AH_WORLDS_PRIVATE_START=1` serves Restoration from its private stand-in.
  Public instances now encode their lists as entry-only frames ([integration
  note](../notes/ps4-56578-integration-wiring-2026-11-05.md)), but none has been served to the client
  yet. To try the public start, pass `-Set D1_AH_WORLDS_PRIVATE_START=`.
- **`D1_HITCH_HALT=0`** is the "on" value: the game reports the stall and carries on, where unset
  keeps the console's halt.
- **Session knobs.** `lab-base.env` serves the geared Guardian, the Director flags without 308 and
  3391 (the social Tower), the vendors and a day's idle limit. The boot world is `pvp_moonbase`, as
  `d1-lab-served.ps1` uses, and the person at the pad drives (`OPERATOR_INPUT=1`).
- **Not in the profile:**
  - The earlier public-session knobs (`D1_AH_PUBLIC_REGIONS`, `D1_MM_LIVE_REPLIES`,
    `D1_AH_SLOT_HANDOVER`).
  - `D1_AH_SCRIPTS_COMPLETE_BODY`. The 56578 body of incident 965 is unknown, so it is not sent.
  - `D1_KBM`. It doubles keys with the PC layout.
  - `D1_MP_ACCOUNTS`.

  Add any of them with `-Set server:KEY=VALUE`.

## What each subsystem should log

`boot\responder.log` starts with the responder's knob table (`=== d1-responder environment`). Every
`[server]` knob the profile sets should read `SET` there. `manifest.json` lists anything that was
dropped. **A start-up line only shows that a subsystem loaded.** The last column says what counts
as working.

| Subsystem | Log | Shows it loaded | Counts as working when |
|---|---|---|---|
| Player store (#307) | responder | `💾 player store ON: <dir> (player local, r<N>, schema 1, saved every 300s)`, then `💾 player store r<N>: <what>` on each important event. `⛔⛔ player store ... NOT USED` means it fell back to the fixtures. | A purchase, equip or dismantle in run 1 is still there after signing on in run 2 with the same store. `account-<soid>.json` changes in the store folder, and its `history` names the events. |
| Social and vendors (#309) | responder | `⭐ SOCIAL (<id>) decoded: <op>` for each social request (901, 902, 903, 405, ...) | A Tower purchase, a vault move and a dismantle are applied and pushed back, each with its `VendorItemStatus`/`TransferStatuses` result, and the client shows them. |
| Rewards (#311) | activity host, responder | Host: `⭐ rewards: grants go to the player store through <store-control.txt>`, then `⭐ rewards: forwarded to the store: grant ...` when an activity ends. Responder: `💾 control "grant ...": ... for 0x...: N currency line(s), ...`; `store-control.state.json` lists the grant. | A completed scripted activity's grant reaches the store (`rewards.grants` in the character's attributes) and a changed wallet is pushed. Only Glimmer has a content index ([integration note](../notes/ps4-56578-integration-wiring-2026-11-05.md)). |
| Multiplayer (#312) | responder, activity host | `⭐ D1_MP=1: multiplayer services on (...)`; with the store, `⭐ MP: accounts resolve through the player store's identities.json`; per connection, `⭐ MP: connection #N is '<id>', <account>, in <fireteam>`; on a 42 search, `⭐ MP matchmaking (connection #N): ...`. In the host: `⭐ shared instances on (D1_AH_SHARED_INSTANCES): one membership per named host`. | With one client: it signs on as its own account in a fireteam of one, and its 42 searches get answers. Fireteams and shared instances need a second client. |
| Activity framework (#313) | activity host | `activity framework: <coverage summary>`. Any `activity framework: ...` warning is a data problem. `... not loaded: ...` means it failed. | The summary line has the expected counts. A strike run would drive `ActivityRun` from the host's slice and trigger reports; no run has launched one. |
| Generated worlds (#314) | activity host | `⭐ D1_AH_WORLDS <dir>: N activities served from generated worlds (M distinct frames), K without one`. `⚠ D1_AH_WORLDS: <world> generated checksum ...` means the generated world disagrees with the measured one. | Launching Restoration (166) from the Director loads a world. Run sense125 got a black screen there. The same goes for any bound activity (King's Fall 160, Prison of Elders 27, Felwinter Peak 398, ...); none has been launched yet. |
| Public instances (#316) | activity host | `public lists encoded as entry-only frames (gen 0) in ...`, then `⭐ D1_AH_PUBLIC_HOST=1 - N public lists from <table>, N frame(s) built`; `⭐ public instances: bundles go to the script bridge`; `⭐ public instances: M worlds' PUBLIC checksums`. A join logs `⭐ public instance: publishing ...'s PUBLIC checksum` and `⭐ public instance <id> is <key>`; a Tower join logs `⭐ social space ...`. | A public instance's join is recognised, its region's list is served (`⭐ public instance: serving ...'s sensor list`), and its slot is freed when the client leaves. Its events still wait on stream K's sidecar message. |
| Native RSAT (#317) | activity host | One `native RSAT: udt N encode+decode native` line for each covered udt, and `native RSAT: ... stays on the client's codec: <why>` for the rest. | The client signs on, loads the boot world and the Tower, and stays connected with no native codec error. Compare the slow-turn warnings against the baseline. |
| udt 5 stamp (#298) | activity host | `⭐ D1_AH_SENSOR_STAMP=1 - udt 5 stamped at 10 Hz, init on full lists, 3 s heartbeats` | The client keeps the world through idle stretches (heartbeats), and the cinematics start on time. |
| Script bridge (#319) | activity host | `⭐ D1_AH_SCRIPTS=1 - scripted activities [165]`. On launching 165: `⭐ script bridge: cine_reveal scripts start`, then `⭐ script bridge: lifetime ...` lines, then `⭐ script bridge: cine_reveal ended (Completed)`. | The reveal cinematic plays and ends, driven by its script, in the live order 0 → 1 → 4 → 6 → 8, as the step schedule did in run cine-live13. |
| Script sidecar (#310) | activity host | Lines prefixed `sidecar:`; a `ready` with its unresolved bindings; `sidecar error:` lines are its errors | The sidecar starts once per scripted instance and answers every `tick` (no `no tick_done` warnings). |
| PC client (fork #8) | emulator | No `Stub resolved ... libkernel_exception` line, and four `Installing signal handler for` lines (4, 8, 10, 11). Under a stall: `halt skipped (D1_HITCH_HALT=0), the game carries on`. The PC layout writes `user\input_config\d1-pc.ini`. | The checklist in fork #8's description, below. |
| Draw cost (fork #7) | emulator | A `D1 draw stats: ...` line once a second | The A/B below. |

## The A/B for fork #7 and #8

The deployed emulator (98542a7) has neither fork PR. Build the PR's commit, or an integration of
both, and deploy it as usual: a committed fork revision, hash-recorded in `DEPLOYED.txt`. Then run
these. The launcher records the emulator's SHA-256 in each manifest. A native result must name
build 56578 and the emulator commit.

### Fork #7: the draw path

Run the same scene with the same setting both times:
- for example the Tower plaza, standing still for 60 s after the load settles;
- `D1_FPS=60` (or `0`, uncapped) on both sides;
- nothing heavy running.

```powershell
pwsh -File tools/ps4_setup/d1-full-game.ps1 -Name draw-a -Set D1_DRAW_FAST=0,D1_GPU_WAIT_YIELD=,D1_FPS=60
pwsh -File tools/ps4_setup/d1-full-game.ps1 -Name draw-b -Set D1_GPU_WAIT_YIELD=,D1_FPS=60
pwsh -File tools/ps4_setup/d1-full-game.ps1 -Name draw-c -Set D1_FPS=60        # with the experimental yield
```

1. From each run's `boot\emulator.log`, compare the medians of the `D1 draw stats` lines:
   - us a call, and its record phase;
   - the GPU thread's share in calls;
   - skipped binds and the pipeline memo (0% with `D1_DRAW_FAST=0`).
2. Compare the frame rate: the FPS counter, and `D1_FPS`'s "uncapped: N frames/s" and "ran at N
   ticks/s" lines.
3. Look for rendering differences. None are expected. One would mean a bind was skipped that should
   not have been; `D1_DRAW_FAST=0` is the workaround.
4. The frame rate can only rise where the GPU thread is near 100% in calls.
5. For `D1_GPU_WAIT_YIELD` (draw-c against draw-b): the guest wait spins and yields, and whether
   the frame rate falls.

### Fork #8: the PC client

Each knob can be turned off alone, against the full profile:

| What | A (off) | B (on, the profile) | Look for |
|---|---|---|---|
| Exception handlers | `-Set D1_EXCEPTION_HANDLERS=0` | default | The boot-log lines above. With them on, the game's own fatal path ends the process cleanly (`D1_CLEAN_EXIT`), with no Windows error dialog. |
| Hitch halt | `-Set D1_HITCH_HALT=` (halts, as on a console) | `0` (carries on) | Only under a stall. Don't load the disk on purpose during someone else's run. |
| Audio pacing | `-Set D1_AUDIO_SYNC=` | `1` | Dialogue stays in sync in the opening and the reveal cinematic. No drops or crackle over a long run. No "Clearing backed up audio queue". |
| PC layout | `-Set D1_PC_LAYOUT=,D1_PAD_KEEP_KEYBOARD=` | `1` | WASD, Space, Shift sprint, Ctrl crouch, R reload, E interact (hold), C melee, Q grenade, F super, 1 or the wheel to switch, hold 3 for heavy, I and Tab. Esc goes back in menus and opens the character screen in play. Mouse look turns the same at 30 and 60. Unplug the pad: the keyboard keeps player 1. |

The whole set is one A/B as well: `-Profile baseline` against `full-game`, on the same emulator
build and the same scene.

## Still stand-ins, or needing evidence

Collected from each stream's note. Each note has the full list.

- **Player store** ([note](../notes/ps4-56578-player-store-2026-11-05.md)):
  - A Guardian Rises' 900 XP and every `stand-in` row of `activity-rewards.tsv`.
  - The 901 stamp's unit (seconds is assumed), and the 905 refund path (decoded, not applied).
  - Runtime indices for reputation. Until they are read, ranks live only in the store.
  - Content index ↔ manifest hash, so refunds can apply per sale instead of per vendor.
  - One player per responder process.
- **Social and vendors** ([note](../notes/ps4-56578-social-and-vendors-2026-11-05.md)):
  - Which sales a stock refresh picks.
  - Every dismantle amount (`D1_DISMANTLE_PLACEHOLDER`).
  - 902 read as "take back".
  - The Lost Items bucket.
  - The infusion rule and its message.
  - The bounty turn-in message.
  - The rank-up package flow.
  - The Vault's and Postmaster's vendor rows.
  - The client's stock message (1401 is the candidate).
  - 903, 902, 405 and 404 have never been seen on the wire.
- **Rewards** ([note](../notes/ps4-56578-rewards-and-growth-2026-11-05.md)):
  - 19 placeholder rules (drop chances, the Light spread, decrypt upgrades, dismantle amounts, the
    Marks weekly cap, node costs, XP sharing and more) and 15 community rules.
  - Grants cross from the host through the store's command file ([integration
    note](../notes/ps4-56578-integration-wiring-2026-11-05.md)). Only Glimmer has a content index;
    other currencies stay in the kept overlay. The overlay's counters and slotless stacks are saved;
    its instances and grids are not.
- **Multiplayer** ([note](../notes/ps4-56578-multiplayer-2026-11-05.md)):
  - Never seen in the game.
  - The 56578 field numbers inside 42's `f7`/`f8` are inferred from 51539, and the search scores
    are stand-ins.
  - BAP 23's own-id rule is a guess.
  - Peer slots: the per-peer token sits at the configured index, and entity requests are not
    confined to a slot.
  - No reconcile on a physics-host migration.
- **Activity framework** ([note](../notes/ps4-56578-strikes-raids-crucible-2026-11-05.md)):
  - Every Crucible score limit, time limit and point value is a stand-in.
  - Strikes with neither a script nor a capture are one encounter each.
  - The raid encounter lists are D1 knowledge.
  - The checkpoint clear, the loot lockouts and the launch read-back go through the store
    (`store-control.state.json`); no run uses them yet.
  - Nothing drives a run on the wire yet.
- **Generated worlds** ([note](../notes/ps4-56578-worlds-2026-11-05.md)):
  - Generated lists carry default values.
  - 363 activities have no evidence.
  - None of the generated rows has been served in the lab.
- **Public instances** ([note](../notes/ps4-56578-public-instances-2026-11-05.md)):
  - The lists are entry-only frames with default values, never served to a client yet. The open
    world's bridge is the script bridge, which refuses until a bubble has a scripted run (stream K).
  - The public instance publishes the world's PUBLIC checksum from `worlds.tsv`; not measured.
  - The event schedules, patrol beacons, loot timers and social-space cap are placeholders.
  - The authority-reset exchange and peer reservations are not served.
  - One placement rule now (the emptiest first), but the responder's matchmaker cannot see the
    host's registry, so no search places into it yet.
- **Native RSAT** ([note §5](../notes/ps4-56578-native-rsat-plan-2026-11-05.md)):
  - Kinds 0x22, 0x23, 0x26 and 0x0D stay on the oracle, so most mission lists still go through the
    control worker.
  - No delta encoder, no capture comparison and no latency benchmark.
  - The responder still calls `d1-oracle`.
- **Stamp mode** ([note](../notes/ps4-56578-udt5-stamp-init-2026-11-05.md)): static analysis only.
  No stamp-age timeout was found, but the absence is not proven for every watchdog.
- **Script bridge** ([note](../notes/ps4-56578-script-bridge-2026-11-05.md), #319):
  - The scripted-activity table is a one-row stand-in (`ps4/profiles/script-activities.txt`) until
    stream K generates it. Point `D1_AH_SCRIPTS_ACTIVITIES` at K's table with `-Set` when it lands.
  - The 965 body is unknown, so completion is not sent.
  - The mission-phase tuple's meaning is unknown.
  - Whether the walker accepts a subset delta is unproven.
  - Many row kinds have no field map yet: timers, sequences, trigger groups, machines, actors, and
    the squad task and roster.
  - There is no slice → bundle table.
- **Script sidecar** ([note](../notes/ps4-56578-script-sidecar-2026-11-05.md)):
  - It runs on stand-in bundles: its row indices are ours, never the game's.
  - BullShcript answers natives with 01.29 semantics.
  - Two thread failures on stand-ins.
  - `import_only_clear_slice_set_assignment` is a logged stub.
- **Fork #7 and #8:** built and unit-tested only. The savings and the audio pacing are estimates
  until the A/B above.

## Pending code (`?` knobs)

Code that is not on `main` or not in the deployed emulator yet. The launcher drops these knobs
while their names are absent from the binaries. `tests/test_full_game_profile.py` lets a `?` knob
be missing from the code only while it has a row here.

| Knob | From | Status (5 November) | Present when |
|---|---|---|---|
| `D1_BUNDLES_DIR` | stream K, the content bundle export | being built | `tools/script-host` reads it (until then the launcher passes it on and warns) |
| `D1_DRAW_STATS` | fork #7, `d1-perf/draw-cost` | draft | the deployed shadPS4.exe includes #7 |
| `D1_DRAW_FAST` | fork #7 | draft | as above |
| `D1_GPU_WAIT_YIELD` | fork #7 | draft | as above |
| `D1_EXCEPTION_HANDLERS` | fork #8, `d1-client/input-audio` | draft | the deployed shadPS4.exe includes #8 |
| `D1_CLEAN_EXIT` | fork #8 | draft | as above |
| `D1_HITCH_HALT` | fork #8 | draft | as above |
| `D1_AUDIO_SYNC` | fork #8 | draft | as above |
| `D1_PC_LAYOUT` | fork #8 | draft | as above |
| `D1_PAD_KEEP_KEYBOARD` | fork #8 | draft | as above |

`D1_MOUSE_CURSOR`, `D1_MOUSE_BUTTONS` and `D1_MOUSE_LOOK` are in the deployed build already. They
are marked `?` only so an older emulator still starts. When code lands, remove its row here. Keep
the `?` until the deployed binaries have the knob.
