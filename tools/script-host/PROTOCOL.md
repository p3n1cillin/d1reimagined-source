<!-- SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project -->
<!-- SPDX-License-Identifier: GPL-2.0-or-later -->
# Script sidecar protocol, version 1

`d1-activity-host` runs one sidecar process per activity instance:

```sh
node tools/script-host/src/sidecar.mjs
```

The host writes JSON lines to the sidecar's stdin and reads JSON lines from its stdout. Stderr is
a human log and carries nothing the host must parse. Each line is one object with a `type` field.
The sidecar owns script semantics only. The host keeps the whole wire: RSAT encoding, udt 5
framing, memberships, slices and BAP.

This follows section 4.3 of the Fukugen Daihon gap analysis (kept outside the repository, because
it names unlicensed paths). Where this document differs, the difference is marked **Differs from
§4.3** and says why.

## Clock and ordering

- **The host owns the clock.** The sidecar never reads wall time. One `tick` message advances the
  scripts by one script tick, which is `1 / tick_hz` seconds.
- **The tick rate is 10 Hz by default**, to match the udt 5 sensor stamp: the live host's
  10 Hz clock (#295). **Differs from §4.3**, which proposed a 30 Hz `tick` carrying 30 Hz activity
  ticks and a 1 s script beat. Send one `tick` per udt 5 stamp step.
- `tick.now` is the host's clock value, in whatever unit the host stamps rows with; the udt 5
  stamp is the intended one. The sidecar copies it into every state that records a time
  (`startedAt`, `armedAt`, `placedAt`). Script time (`tick_done.time`) is separate: it counts
  ticks / `tick_hz`.
- The sidecar handles messages strictly in order. Every output a message causes is written before
  the sidecar reads the next line. Event messages between two ticks apply before the next tick.
- **One run replays exactly.** The same input lines give the same output lines (the scripts' random
  numbers come from `seed`). A host log of the input stream is therefore enough to replay a run
  offline.
- Within one message's output, `rows` comes first. Then `phase`, `activity_complete`,
  `hard_wipe_armed`, `all_dead` and `ended` follow in that order. After a `tick`, `tick_done`
  comes last.

## Identifiers

- **A row** is `{bundle, typeId, typeIndex}`: BullShcript's `ItemRef`. `bundle` is the bundle's
  FNV-1 hash, `typeId` the row type (the `i8` of our name refs) and `typeIndex` the row's index
  (`sub`). The host keys its row table by `(group hash, i8, sub)`. Group 0's live hash is per
  load, so the host translates `bundle` into the live group hash, as it already does.
- Host-to-sidecar messages that name a row use `bundle` and `type_index`. `typeIndex` is also
  accepted.
- Hashes are unsigned 32-bit numbers. Inputs accept a number or a `"0x…"` string.
- Player keys (`key`) are strings. A u64 character id does not fit a JSON number.

## Host → sidecar

| `type` | Fields | When | Effect |
|---|---|---|---|
| `hello` | see below | first | loads and compiles the scripts, builds the session, answers `loaded` |
| `start` | none | after the join apply is built | binds rows, runs load-time code, starts every `@startup` thread and the first phase; answers `ready` |
| `tick` | `now`, `threads?` | every udt 5 stamp step | one script tick; answers rows etc., then `tick_done` |
| `active_bundles` | `bundles[]` (hashes) | the current slice changes (udt 21) | only threads of these bundles run |
| `event_start` | `bundle`, `event_id` | a public bubble's host starts one event of its rotation (stream G's `BubbleScriptBridge::start_event`) | the event's bundle joins the active set (all loaded bundles until `active_bundles` names a set), so its threads run; answers `event` |
| `event_stop` | `bundle`, `event_id` | the event's time ran out or the instance goes away | the bundle leaves the active set, so its threads stop; its rows keep their values; answers `event` |
| `players` | `players[]`: `{key, slot?, power?, team?, ready?}` | the membership roster changes | the roster the scripts see; until it is sent, the sidecar assumes one player who becomes ready on `player_spawned` |
| `player_joined` / `player_left` | `key` | join / eviction | the host-synthesized `player_joined` / `player_left` incidents |
| `player_spawned` | `key?` | client incident `player_spawned` | marks the player ready; the scripts start waiting on players |
| `player_died` / `player_respawned` | `key` | kill classification / player sense | dead players, for a hard wipe |
| `trigger` | `bundle, type_index` | incident 3403 (`player_trigger`) | the player trigger fired |
| `cinematic_finished` | `bundle, type_index` | incident 891, with the row from its body | the cinematic row is done |
| `cinematic_skipped` | `bundle, type_index, key` | incident 1726 | counted per player; the cinematic finishes once every listed player asked (one player: at once). The host may instead decide and send `cinematic_finished` |
| `incident` | `id` or `name_hash` or `name`; `bundle`/`type_index`/`key` where the name needs them | any other client incident | translated through the 56578 catalogue (below); `player_spawned`, `cinematic_finished` and `cinematic_skip` with a row go where the messages above go, and anything else goes to the scripts' incident listeners by name hash |
| `squad_alive` | `bundle, type_index, count` | udt 6, squad live count (`sense.rs`) | `alive_count()` |
| `squad_combat` | `…, count` | udt 6 | in-combat count |
| `squad_tally` | `…, tally[]` | udt 6 | failed placements per cell |
| `squad_kill` | `ident, at?` | kill incidents | a kill credited to the squads with that identifier |
| `kill` | `details` (BullShcript's `KillFilterDetails`) | kill incidents | kill listeners |
| `object_alive` | `…, alive` | udt 6 (later) | |
| `player_monitor` | `…, count` | udt 6 (later) | |
| `health` | `…, current, max` | udt 6 (later) | |
| `objective_count` | `…, slot?, count` | udt 6 (later) | objective slot counts |
| `actor_bound` | `…, flag7, flag8` | udt 6 (later) | |
| `actor_health` | `…, fraction` | udt 6 (later) | |
| `actor_drop_ack` | `…, drop_id, in_progress` | udt 6 (later) | |
| `actor_standing` | `…, epoch, standing` | udt 6 (later) | |
| `actor_command_script` | `…, cookie_running?, heartbeat?, pc_reached?` | udt 6 (later) | |
| `device` | `…, position, position_stamp` | udt 6 (later) | |
| `interactable` | `…, used, used_stamp` | udt 6 (later) | |
| `capturable` | `…, sense` (`{locked, lockStamp, owner, progressTenths?, resetStamp}`) | udt 6 (later) | |
| `faction` | `…, faction` | udt 6 (later) | |
| `safe_zone` | `…, inside` | udt 6 (later) | |
| `interaction` | `names[]` (hashes) | interaction incidents | |
| `loot` | `materials[]` (hashes) | gather incidents | |
| `hard_wipe` | none | the membership hard-wipe handshake completed | the running phase resets (`resetCurrentPhase`) |
| `snapshot` | none | debugging | answers `snapshot` with BullShcript's `debugSnapshot()` |
| `end` | none | the instance closes | answers `bye`; the process exits |

**Differs from §4.3:**

- `eval` (debug evaluation) is not implemented; use `snapshot`.
- `hard_wipe` takes no `reset` field. Sending it is the reset.
- The `report*` messages marked "later" are wired to BullShcript's report functions, but nothing
  on our side decodes their sense yet.

### `hello`

| Field | Default | Meaning |
|---|---|---|
| `proto` | 1 | must be 1 |
| `build` | `"56578"` | must be 56578 |
| `activity` | required | the `@activity` name (`cine_reveal`, `act_cosmo_m0_highway`, …) |
| `scripts_dir` | `D1_SCRIPTS_DIR` | the 56578 script folder (Fukugen Daihon `v56578_17_04_12_1251_evil`) |
| `tiger_dir` / `bullshcript_dir` | `D1_TIGER_DIR` / `D1_BULLSHCRIPT_DIR`, then `../third-party/Blam-Network_tiger` and `../../third-party/Blam-Network_tiger` beside the repository | the built BullShcript checkout |
| `bundles` or `bundles_path` | `D1_BUNDLES_DIR/<activity>.json` if present, else stand-ins | the private content export: BullShcript's `ActivityBundleTag[]` (`{bundle, tag, rows:[{name?, nameHash, typeId, typeIndex, cells?, memberNames?, childScript?, …}]}`) |
| `extra_bundles` | `[]` | more bundles to load as stand-ins, by name or hash, or `"guess"` (see below) |
| `incidents_path` | `ps4/fixtures/56578/incidents-56578.tsv` if present | the incident catalogue |
| `phase_index` | 0 | start at a later phase (resume, debugging) |
| `tick_hz` | 10 | script ticks per second |
| `seed` | 1 | the scripts' random seed |
| `log` | none | `"lua"` sends every Lua note as `log` |

**The content export.** `tools/export-bundles.mjs` writes one file per world into a private
`D1_BUNDLES_DIR` outside the repository: `{world, bundles: ActivityBundleTag[], slices}`. Either
that document or a bare `ActivityBundleTag[]` is accepted. Its rows carry the game's own type
ids and indices, the squad definitions' cells and member names, objective task lists, sequence
durations and each row's import identifier. Two things come from the scripts on top: an Encounter
row's archetype (`childScript`, by the row's name), and an alias for a restored member whose name
is a prefix of exactly one content row of its type in its own bundle (`thorium_dome` →
`thorium_dome_a`), said in `loaded.notes`. Neither changes an index.

**A world no script starts.** When no `@activity` names the activity but its content export is
there (`bundles`, `bundles_path` or `D1_BUNDLES_DIR/<activity>.json`), the sidecar answers with a
start-only stub, as the upstream stubs are: `stub: true`, no `phases`, an empty `start`, and a
`notes` line saying so. The content bundles load and their own scripts, if any, run; with none,
nothing moves the lifetime and the host decides. Without content it still refuses (`no script
starts activity …`): a stub on stand-in rows would have nothing to serve. This is how the bridge's
table can list every bound PvE activity, scripted or not (`tools/ps4_setup/d1-scripts-table.py`).

`§4.3`'s `fukugen_dir`, `compiled_dir` and `tick_hz:30` / `script_tick_s` are replaced by
`scripts_dir` and `tick_hz`. The folder compiles in memory in under half a second, so nothing
derived from the scripts is written to disk.

**Stand-in bundles.** Without `bundles`, the sidecar builds rows from the scripts' own member
declarations, named the way the activity bundles name them: `<phase>.<member>`, then
`<phase>.<member>.<archetype member>`. It builds them for the binder (`story_common`,
`portal_common` or `tower_common`, chosen by folder), the activity root, and every other bundle in
the root's own folder. Their `typeIndex` values are **ours, not the game's**, so the host must
never put a stand-in row on the wire. `loaded.stand_in` says which kind of bundle is in use. Area
encounters (`enc_*`) are separate bundles, and only content lists which ones an activity loads.
`extra_bundles: "guess"` loads those whose stem ends in the activity's mission tag (`m0`, `m01`,
`portal1`, …), and a guess is labelled as one wherever it is used.

## Sidecar → host

| `type` | Fields | Host action |
|---|---|---|
| `loaded` | `activity`, `proto`, `stand_in`, `stub` (no mission scripts), `drives_lifetime`, `compile_errors[]`, `phases[]` (`{index, name, name_hash, slice, slice_hash}`), `start` (`{start_slice, start_phase, spawn_transition, region_spawn_transitions, time_of_day}`), `bundles[]` (`{bundle, tag, path, rows}`), `script_tags[]`, `notes[]`, `extra_bundles[]`, `incidents`, `tick_hz` | check it; refuse a stand-in session on the wire; the start data can replace hand-written world-table values |
| `ready` | `unresolved[]`, `host_side[]`, `drives_lifetime`, `script_tags[]`, `stubs[]` | log; refuse if `unresolved` is non-empty and the host is strict. `host_side` lists the incident listeners, which BullShcript keeps host-side by design (no bundle has a row for one) |
| `rows` | `changes[]`: BullShcript's `ItemChange` exactly (`{item:{bundle,typeId,typeIndex}, kind, state, initial?}`), plus `wire` on lifetime changes | update the row table, encode the A/B images, and send the dirty entries in the next udt 5 frame |
| `rows_cleared` | `items[]` | those rows back to fresh/default (hard wipe) |
| `phase` | `index`, `name` (the phase row), `name_hash`, `decl` (the mission class), `slice`, `slice_hash` | the lifetime's mission fields (index, phase hash, bubble, resolved from content by `slice_hash`); the GAS state for that region; teleport if the slice changed |
| `activity_flag` | `hash`, `active` | the player rows' flags |
| `event` | `bundle`, `event_id`, `state` (`started`, `stopped` or `refused`), `reason?` | the answer to `event_start` / `event_stop`; `refused` when the activity loads no such bundle |
| `activity_complete` | `id` (965), `name: "activity_complete"`, `name_hash` (`0x42660690`) | send host incident 965 (udt 18): the host sends the completion envelope (965 → 3235 → 2937 → 2009 → 614) with its `0x80800595` body, decoded in #318 (`d1-activity-host/src/completion.rs`) |
| `ended` | none; sent once | stop ticking; BAP completion through d1-responder |
| `hard_wipe_armed` | `armed` | whether a hard wipe may start |
| `all_dead` | `dead` | with `armed`, start the membership hard-wipe handshake |
| `tick_done` | `now`, `ticks`, `time` (script seconds), `threads?` | the barrier for one tick |
| `log` | `text`, `stub?`, `missing?` | tracing. `stub` names a 56578 stub that ran; `missing` names a native nothing answered. Both are always sent; other Lua notes only with `log: "lua"` |
| `error` | `text`, `fatal?` | a fatal error means the session did not start |
| `snapshot` | `snapshot` | |
| `bye` | none | |

`ready` comes in reply to `start`, not to `hello`, because unresolved bindings are only known once
`start` has bound the rows. **Differs from §4.3** for that reason. Section 4.3 has no `loaded`
message; this protocol adds it as the reply to `hello`.

### The lifetime

A lifetime change keeps BullShcript's state, `{state, joinFlag, timeOfDay?}`, and adds
`wire: {phase}`. `phase` is the decoded value our lifetime record carries, the `@phase` of a
sensor list. The mapping is Codex's (#304):

| Runtime `state` | Meaning | `wire.phase` |
|---:|---|---:|
| 0 | pregame | 1 |
| 1 | countdown (the +1 shift only; never seen on 56578) | 2 |
| 2 | active | 3 |
| 3 | cinematic hold | 4 |
| 5 | results (`end_game`'s first write) | 6 |
| 6 | ended | 8 (a cinematic) or 7 (gameplay) |

The ended value depends on the activity's kind, not on the runtime: #318 recorded 6 → 8 for a
cinematic and 6 → 7 for gameplay (36 gameplay activities in the live captures, the 7 about 30 s
after the 6). The host's lifetime encoder (`row_encoders.rs`) picks it from the activity table's
`kind=`; the sidecar's `wire.phase` for 6 is the cinematic's 8.

Any other state gets `wire.phase: null`, and the host should refuse it. `ended` follows the
change to 6. **Differs from §4.3** (an addition): §4.3 left the 56578 numbering to the host.

### Incidents

The catalogue is Codex's `ps4/fixtures/56578/incidents-56578.tsv` (#306; columns `index`,
`name_hash`, `name`, …). Without it the sidecar still knows four 56578 ids, which #304 resolved from
the owned registry: 891 `cinematic_finished`, 965 `activity_complete`, 1726 `cinematic_skip` and 2681
`cinematic_started`. An `incident` the catalogue cannot resolve answers `error` and changes nothing.

## A session, end to end

```text
→ {"type":"hello","proto":1,"build":"56578","activity":"cine_reveal"}
← {"type":"loaded","stand_in":true,"phases":[{"index":0,"name":"cine_106_trv_phase",…},…],…}
→ {"type":"start"}
← {"type":"ready","unresolved":[],"stubs":["import_only_clear_slice_set_assignment"],…}
← {"type":"phase","index":0,"name":"cine_106_trv_phase","slice":"plaza",…}
→ {"type":"player_spawned","key":"1"}
→ {"type":"tick","now":1}
← {"type":"rows","changes":[{"kind":"lifetime","state":{"state":0,…},"wire":{"phase":1},…},
                            {"kind":"cinematic","state":{"playing":{"startedAt":1}},…},
                            {"kind":"lifetime","state":{"state":3,…},"wire":{"phase":4},…}]}
← {"type":"tick_done","now":1,"ticks":1,"time":0.1}
→ {"type":"cinematic_finished","bundle":…,"type_index":…}
→ {"type":"tick","now":2}
…
← {"type":"activity_complete","id":965,"name_hash":1113982608,…}
…
← {"type":"rows","changes":[…{"kind":"lifetime",…,"wire":{"phase":6}}]}
← {"type":"rows","changes":[…{"kind":"lifetime",…,"wire":{"phase":8}}]}
← {"type":"ended"}
→ {"type":"end"}
← {"type":"bye"}
```
