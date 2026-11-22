# PS3 orbit checkpoint (2026-09-25)

Current handoff for the local RPCS3 Destiny test (supersedes the 2026-09-24 character note's
"Next step"). With the loopback stand-ins the game now signs in, shows character select with
the stored Titan, and **loads into orbit** ("Set Destination"). Rendering is still
rainbow-distorted in places; that is a separate graphics issue.

## How to run

- RPCS3 copy `<PS3_ROOT>\RPCS3_TEST048_SETUP` (XInput pad = Player 1, async shaders).
  Launch it directly (not `Invoke-Rpcs3LocalTest.ps1`, which makes a keyboard-pad copy).
- Services: rpcn, `dns_sinkhole.py --destiny-hosts`, `stun_stub.py`, `signon_probe.py --mode
  auth3 --destiny-hosts --manifest-url http://content.local.test/manifest --manifest-packages
  ...\USRDIR\packages`, `lobby_probe.py --timeout 60`, and
  `bap_server.py --ws-signin --schema <PS3_ROOT>\SIGNON_LOCAL\investment_schema_character.json
  --store <PS3_ROOT>\SIGNON_LOCAL\characters.json`.
- The BAP stand-in can be restarted while the game shows an error; dismissing reconnects.
- Debugging: start `scripts/rpcs3_gdb_daemon.py` right after RPCS3 and never kill it while
  RPCS3 runs (the stub serves one connection per emulator session; a second one freezes the
  game). Use `python scripts/rpcs3_gdb_daemon.py send "hits N SECONDS ADDR,..."` or `send mem
  ADDR LEN`. Each breakpoint hit pauses the game for about a second.

## What the server now does (see test-log 049–062)

- 501 create: id = account id + 1; saved to `--store`; self delta publishes the character and
  makes it active (untested since the active-character fix: Finish may now proceed).
- 504 select: reply error_none, self delta (next version) with root +0x60 = character id.
- 0x17 acct id trans: echo request + u64 account id per name.
- 0x12 client config: protobuf `{1: 1, 2: 1}` (placeholder values).
- 104: result plus universe object `{0: 0x7fffffffffffffff, 8: 1}`.
- Families 0 (peer) and 2 (roster) are pushed as empty snapshots on subscription.

## Rules learned

- BAP replies must follow request order per queue (`0x7c3458`); any unanswered request
  blocks every later reply (failure 0x12 -> centipede).
- Bootflow errors are raised by the health loop `0x327d90` (conditions table `0x11ea038`):
  15 = BAP state (0xa0 centipede), 17 = investment sign-in (0xa1 chicken, reasons 0xa2+).

## Next

- Try Set Destination / the Tower and watch for new requests; level 0 and empty gear come
  from absent character progression fields (`0x80801954` +0x30, +0x68).
- Re-test character creation (Finish) now that 501 sets the active character.

## Update (comparisons 063–081)

- Run with `--universe-time s --self-flags "1;2@B0,C0-1023"`: past trial and purchase gates;
  selecting the Titan plays a story cutscene and starts an activity join.
- Current gate: the activity join's **Demonware bdMatchMaking createSession** request on the
  lobby connection (encrypted `AB 85` frame, format in test-log 080–081) is unanswered, so
  the join fails with **currant**. `lobby_probe.py` must decrypt post-auth frames and answer
  with an encrypted task reply (server keys: AES key 1, HMAC key 1; reply format still to be
  read from the client's bdRemoteTaskManager, receive path `0xa3a640` -> type byte ->
  `0xa3e8ac`). The ahp reply (16 bytes) likely identifies the activity host/session.
- Unattended loop: `Start-LocalStack.ps1 -Name X -BapArgs ...` then
  `Invoke-TitanLogin.ps1 -Name X`, then arm traces via the GDB daemon.
## Update (comparisons 082–093): activity join protocol

The activity join is a **private activity instance**: the client runs activity-host-side
logic partly itself, but negotiates through Bungie services. Steps now answered locally:
1. Demonware lobby bdMatchMaking createSession (encrypted AB 85 frames; `lobby_probe.py
   --reply-type 0x01 --session-result`).
2. `c->b_ahp` (0x10) on the main BAP: reply `u64 request id | IPv4 | u16 | port` = address of
   the **activity host proxy** (a second BAP server; `bap_server.py --ahp-reply ...`).
3. On the AHP connection: shello (field 2 = 2), `c->ah mgr req` (0x06, 3103 bytes) answered by
   `c->ah mgr rsp` (0x07, `02 | u64 host id | char[128]`; `--ah-mgr-reply`).
4. The client then sends `c->ah not` (0x08) to the host (u64 host id, type 3, 232 bytes) and
   waits for the host's answer: currently failure (0x18, 0x2a).
Next: the activity host messages (startup response, host identifier, host join/response,
entity indices). The error-name table `0x175eafc` lists every step. Test loop:
`%TEMP%\...\scratchpad\cycle2.ps1` pattern (stack + login + trace + AHP stand-in), about 8 min.
The TEST048 copy uses a keyboard pad now; the user's XInput config is saved as
`config\input_configs\global\Default.xinput.user.yml.bak`.
## Update (comparisons 094–107): activity host join answered
One command runs a full unattended join (about 10 minutes; `-Trace`/`-Dumps` optional):

    scripts\Invoke-JoinCycle.ps1 -Name TESTnnn -LobbyArgs @('--reply-type','0x01','--session-result') `
      -AhpReply 00000000000000007f01018000003233 -AhHost d1local-host `
      -AhArgs @('--ah-join','--ah-entities','8192:12288','--ah-state','--ah-map','orbit','--ah-world','125')

Host messages are typed structs (investment bit format); schema exported to
`SIGNON_LOCAL\activity_schema.json` (`rpcs3_tiger_types.py schema OUT 0x80800a58 ...`, the 64
handles in the tables at `0x18cffe0`/`0x18d0000`). The AHP stand-in now:
1. answers the host join request (c->ah not type 3, `0x80800e98`) with the **join response**
   (b->c not type 4, `0x80800a58`: request echoed, result 0, session id) — `--ah-join`;
2. grants **entity indices** (type 0, 384-word bitmap) — `--ah-entities`;
3. sends host flag type 11 and an **activity state** (type 1, `0x80800e76`: bool @0, i32 @0x10,
   map name char[40] at @0x60 @0x994, world id i16 at @0x60 @0x990) — `--ah-state --ah-map --ah-world`.
Failure progression: 0x2a (join response) -> 0x2b (entity indices) -> 0x2c (establish client) ->
0x36 "loading activity resources" (error **lime**) -> with `--ah-map orbit`: "World-transition
requires DLC." (world id 0) -> with `--ah-world 125` (the i16 the client sent in service 105):
the world transition completes, then (0x18, 0x36) "Transition completed, but we don't have an
active activity." (`0x334260`: the resource manager has no map loaded).

Why: the loader (`0x3363a0`) needs an activity index `*(0x30e0c8()+0x94)`; it is -1, so after
retries it falls back to loading orbit resources (state 0x1f). That index is set by
`0x30d488`, the receiver of a **world controller RPC** (descriptor table `0x1724c60`, pairs
sender/receiver, e.g. `0xd24e14`/`0x30d488`) that the host sends over the simulation stream.
Host type 38 (`0x25b9a8`) looks like that stream's carrier: a typed header (`*(0x179eb20)`)
then u32 bit length and up to 0x1f40 bytes of raw bit stream. Next: decode the simulation
stream framing (types 38/5/7) and the world controller RPC for the activity index, or find
the map name / activity index for world 125 so the activity itself loads.

## Update (comparisons 108–112): why the activity never loads
- The loader (`0x3362c0`) now starts loading the requested scenario (`--ah-map act_cosmo_m0_highway`
  reached `0x1f4a54(rm, "act_cosmo_m0_highway")`), but within ~2 s switches to state 3 (orbit
  fallback) without computing the activity index: `0x30c448` says abort. Its test `0x323ed0`
  aborts while the **group target** session record (group manager `*(0x18ea68c)`, record
  +0x7f8, valid bit +0xd0, world i16 at +0xd8) is pending and the **group current** record
  (+0x450, hash at +0x524) still equals the hash latched on entering orbit (`0x323e6c`, global
  `0x18ecdb8`). I.e. the fireteam's move from the current group session to the target group
  session (the Demonware lobby session from createSession) has not completed, so the client
  follows the group instead of loading the activity; the world transition then ends with
  "no active activity".
- Scenario names are in guest memory as registry keys (`act_cosmo_m0_highway:scenario_client`,
  `city_tower_default1:…`, `.map:cosmo…`), so valid names can be listed from a running game.
- Next: find what promotes the group target to current (lobby session join/state in
  `lobby_probe.py`: bdMatchMaking joinSession / session updates, or a BAP fireteam message),
  then the loader should compute the index, post event 0x36 and load the scenario.

## Update (comparisons 113–116): fireteam target blocks the activity load
Polling guest memory directly (no GDB; `rpcs3_guest_read.py`) during a join shows the group
manager `*(0x18ea68c)` with two groups (names table: fireteam_target / posse_target /
group_target; group g at mgr + g*0x11938, session buffer +0x10 + [+8]*0x8c98, lobby state
+0x7eb8, data +0x63b0). Group 0 = fireteam: current record at data+0x450 (vtable 0x12b7f58),
target record at data+0x7f8 (vtable 0x12c3ae0), each with valid +0xd0.
- When the director launches (about 6 s before the join), the fireteam **target** becomes valid
  (world i16 0x7e at +0xd8, 64-bit ids at +0xe0/+0xe8, +0xf0) and the **current** record is
  re-validated with hash +0xd4 = 0x811c9dc5 (FNV-1 of an empty string) - the same value latched
  on entering orbit (`0x323e6c`, global 0x18ecdb8). `0x323ed0` therefore reports "fireteam is
  travelling" and the activity loader aborts (`0x30c448` -> state 3).
- The world transition state object (static 0x18eba30, vtable 0x1725c40..: enter `0x333c04`,
  update `0x334004`) runs in phases: +0xa20 odd = copy the host scene (world 125,
  `act_cosmo_m0_highway`) - works; then `0x333dfc` bumps +0xa28 and phase 1 needs an active
  scenario in the resource manager, which the aborted loader never loaded -> "Transition
  completed, but we don't have an active activity."
- `--ah-world 126` (the fireteam target's world) fails the DLC check; 125 is right for the state.
- Host type 2 (`{bool, float}`, sets join flag 0x80) did not change this.
Open question: what makes the fireteam current record's hash non-empty (or clears the
target) once the target activity host is joined. The record is rebuilt locally by the
leader (`0x7bc690` -> `0x807d58` -> `0x805c28` diff -> `0x8045ac` copy); its source is built
in the session/peer code around `0x7bc600` (fields at +0x21b4..+0x21d8 of that object).

## Update (comparisons 117–125): destination travel, not an activity
- The director's launch after the story cutscene is a **destination travel**: `0x32f5e0`
  calls `0x41eec8(sel, {destination 0x7e = 126, flag}, activity 0)` on the
  investment_activity_selection_manager `*(0x1906960)` (+0 i16 destination, +2 u8, +4 u32
  activity) and enters state 0x18. With activity 0, `0x30c448` makes the activity loader skip
  the activity index/event 0x36 (state 3), so the destination is expected to load through the
  world change. (Changing the latched fireteam value at runtime did not help: experiment
  TEST118.) Event 0x36 is what sets the activity index and loads a scenario plus its three
  parts (`0x30d488` -> `0x1f4a54`, `0x1f4f00`/`0x1f4e60`, commit `0x1f5300`).
- World change = engine request slot 3 (`*(0x18a4328)`, flags +0x114 + 4*i: 1 posted, 2/3 in
  progress; handler `0x302728` -> `0x66be30`). With world 125 (service 105's value, which looks
  like the client's *current* world) the loaded scenario (`ambient_cosmo`, `cosmo_patrols_1`)
  becomes active and is dropped at once -> "no active activity"; orbit's own
  `ambient_bootstrap` stays active with the same flow. So the state's world must be the
  destination's: 126.
- World 126 needs the destination's unlocks: with `--self-flags 1;2@B0,C0-1023` it fails the DLC
  check (`0x4105c4`, unlock expression via `0xb67bc4`); with `--self-flags 2` (all true) it
  passes and the next gate is "World-transition requires permissions." (0x18, 0x17) from
  `0x40ddc4` -> `0xb4f494`: three destination checks `0xb51c64` (uses a float from `0xb5016c`,
  probably level), `0xb51894`, `0xb51698` (the last only for the fireteam leader).
- `scripts/rpcs3_activity_poll.py` polls these structures (read-only) beside a GDB trace.

## Update (comparisons 126–136): activity 126 permission solved
- The "world" in the fireteam target / activity selection is an **activity index**: 126 = the
  first story mission (its definition, found at run time, names `act_cosmo_m0_highway`).
  Service 105's 125 is something else (probably the current activity/world).
- "World-transition requires permissions." came from `0xb51698` (leader-only check): the
  activity's first unlock expression is `NOT flag[215]` (ops `01 00 00d7`, `02`; format: u32
  count, u32 offset, 4-byte ops from +offset+0x14 = u8 opcode, u8, i16 index; opcode 1 = flag
  == 2, 2 = NOT, 3..9 binary). The flag table there is the **universe** investment object
  (cache header type 0xf04cc38e, soid 0x7fffffffffffffff; table at 0x7009fe8a), defaults from
  content, flag 215 = 2. The universe struct's +0x3c list (0x8080185c = {opt i16 index,
  opt u2 tri-state}) overrides it: `bap_server.py --universe-flag-values 215=1` (sign-in and
  service 104) -> the check passes (TEST135).
- Remaining: the world change to activity 126 with scenario `act_cosmo_m0_highway` (or
  `ambient_cosmo`) loads it into the resource manager, which is emptied again at once ->
  "Transition completed, but we don't have an active activity." The activity loader still
  aborts earlier because the activity selection's +4 is 0 (`0x30c448`); +4 = 1 is set by
  `0x3291a4` (copy of the fireteam target) only when the game state `0xb26d88()` is 8 and the
  current world passes `0x305608`. Next: how the real flow reaches that state (or supplies
  +4) before the loader runs.
- Tooling: `Invoke-JoinCycle.ps1 -TraceAfter 'ahp req'` starts the trace at the join, so polled
  functions do not use up -Hits.

## Update (comparisons 137–148): into the activity world, then "lobster"
Full run (about 10 minutes):

    scripts\Invoke-JoinCycle.ps1 -Name TESTnnn -LobbyArgs @('--reply-type','0x01','--session-result') `
      -AhpReply 00000000000000007f01018000003233 -AhHost d1local-host `
      -ExtraBap @('--self-flags','2','--universe-flag-values','215=1') `
      -AhArgs @('--ah-join','--ah-entities','8192:12288','--ah-state','--ah-map','act_cosmo_m0_highway',
                '--ah-world','126','--ah-checksum','0x79130968','--ah-bubbles','22')

- Resource manager (`*(0x18b477c)`): names +0 / +0x148 (active buffers), +0x294 (requested);
  part slots +0x100 (active) / +0x394 (requested), kinds = suffix table 0x175ccb0 (4
  `:scenario_client`, 15 `:root`, 6..14 transition cinematics = index*3 + in/filler/out,
  index 0 spaceflight / 1 same_dest / 2 no_ship). Parts move to the active buffer only when the
  load completes with status 2 (`0x1f50fc`); an empty buffer loses its name.
- The world change (engine request slot 3, handler `0x302728`) loaded the scenario, then
  `0x147cd4` -> game `0x304628` failed and logged `load-failed` (unloading it again). Causes, in
  order: `0x258550` "activity checksum mismatch" (host state u32 @0x5c must equal the scenario
  definition's word at +8: 0x79130968, read at compare time), then `0x1c7608` "activity client
  bubble count mismatch" (host state i32 @0x10 = scenario bubble count, 22; i8[64] @0x14 bubble
  state indices, -1 = default).
- With both: the world load succeeds, the transition completes (status 0x21) and the client is
  in the activity world for ~1 s, sends activity-host messages 37, 20 (1536 zero bytes), 36
  (4 bytes), then 14/21 and leaves: "You have been returned to Orbit due to an error"
  (**lobster**, failure (0x18, 0xfe) from `0x327e44`, reason via `0x326fe4`/`0x3268d8`).
  Next: what the host must answer after arrival (types 20/36) / what `0x326fe4` checks.
- Why "lobster" (TEST149): once in the world, `0x327d90` checks 30 leave conditions every frame
  (mask at +8 of 0x18eb978). Condition 34's handler reads a state byte at `0x30e088()+0x89`
  (>= 6 = error) and maps it (table 0x11e9ec4) to the failure reason; 0xfe = state 12, set at
  `0x31c350` when the session peer table (`0x13e2a0()`, lookup `0x140820`, 0x200 peers) has no
  peer at the expected machine index (result 0 or 5). I.e. the activity host is not a known
  peer. The peer roster is what host type-5 simulation packets carry (`0x1c85e0`: u8 epoch,
  bool, 65-bit mask + delta of the 65-slot roster type 0x80800cf7 {opt char8 id, opt u16,
  bool}, u32, then the replicated object stream `0x1ce4a4`). Next: send type-5 packets with the
  host (and the player) in the roster, epoch = simulation +0x5010.
- Client messages after arrival: type 20 = `0x80800e22` (384-word bitmap, zero), type 36 =
  `0x80800bd0` (u32 0).
- Correction (TEST150/151): the "missing peer" is object table index 0. `0x13e2a0()` (global
  0x18a943c, table `*(+4)`: +0 scenario handle, +8.. 0x200 entries of 8 bytes, masks +0x1154/
  +0x1194) is filled by `0x140ce4` from the scenario's zone data for the **active zone set**
  (table +0x110c, -1 = none). +0x110c is set by `0x13ff9c` from engine request slot 2
  ("switch zone set", handler `0x30256c` -> `0x1403dc`), posted at `0x31ebf4` by the client's
  world-controller zone state (`0x30e128()`, records at +0x8 / +0xa8, 0xa0 bytes each) - i.e.
  the zone set must come from the host's world controller (replicated state / RPC events
  0x33..0x36), which the stand-in does not send yet. The type-5 roster only drives peers
  (`0x1c8280` -> session interface `0x123478` / `0x1234c8`).
- Zone set (TEST152-154): object table slot 0 = bubble 0's first object (bubbles use slots
  8*i; in orbit slot 0 holds orbit's `:scenario_client` handle and the active zone set becomes
  0x10). For the activity the table stays empty and +0x110c stays -1. The zone set is chosen
  by the world/zone load state machine `0x4b2d00` (state at +0x100: 2 = after world load ->
  `0x4b2b9c` default bubble states -> 3 = select zone: hash from global 0x190a220 / 0x190a224 or,
  mode 2, from character data via `0x293308`/`0x28e9f0` (+0x480) -> `0x1406f0` index ->
  `0x13f8ec` load or engine request slot 2 "switch zone set"). It is driven from the activity
  loader's normal path (`0x33612c`) and in-world states (`0x3298b8`, `0x32e638`, `0x32f840`);
  in our travel-only flow it never ran before the leave check fired right after status 0x21.
  So the remaining blocker is again the travel-only activity selection (+4 = 0, `0x30c448`).
- `scripts/rpcs3_activity_poll.py` now also shows the zone object `*(0x18ea618)` and the object
  table's active zone set and first slots.
- The leave check's state-12 test (`0x31bfe0`, world controller object r30 = `*(0x18ea570)`):
  for group session `0x315450(2)` (lobby state +0x7eb8) it reads record +0x90 (or +0x130 for
  the target group): i32 object index at +0, state byte at +0x11 (== 2 -> check), and requires
  that object to exist in the scenario object table (`0x140820`, index 0 here). The leader's
  activity selection is always travel-only (+4 = 0; only the fireteam-follow path `0x3291a4`
  writes 1), so travel is the normal path for a leader; the missing piece is the group's
  activity-host object that a real host would provide (via its simulation stream) before the
  world-controller record says it should exist.
- The in-activity state object 0x18eb978 uses vtable 0x12b8888 (+0x18 leave check `0x327d90`,
  +0x1c update `0x332dec`); the zone machine `0x4b2d00` is not called from it.
- Slice set (TEST156/157): `*(0x18ea604)` (0x410 bytes) gets a 0x150-byte copy of the zone object
  `*(0x18ea618)` at +0x88 on arrival (`0x31b5c8`). On arrival the zone object becomes byte0 = 1,
  byte1 = 2, target record +0xa8 = {i32 index 0, u32 hash 0, i32 0} (default {-1, 0x811c9dc5,
  -1}), +0x148 = 01000200; the leave check then looks up object 0 (slice set +0x130), which is
  absent -> lobster. The join response's trailing fields (`--ah-join-fields`) do not affect it.
  Open: what sets the zone object's target record to index 0 / hash 0 at arrival (probably a
  default when the host provided no zone set), and what the real host supplies instead.
- Where the zone record comes from: the world controller is authoritative on the client. When
  slice set +0x88..+0x1d8 differs from the zone object, `0x31b8b0`/`0x31bf00`/`0x31e6d8` send
  it as event 0x4a (`0x828688` -> poster `0x262bc4`, struct 0x80802e0e = {u8, u8, i8,
  0x80800d9f {optional record 0x80800d82 @8, optional record @0xa0, i8 @0x140, i8 @0x141,
  u8 @0x142}}); the receiver (RPC table 0x12c3ee0 entry 11, decoder `0xeb4f28`, receiver
  `0x8289f0` -> `0x31bca0`) copies it into the zone object. The target record itself is written
  by `0x31d358(slice, state, index, &hash)` (+0x89 state, +0x130 index, +0x134 hash); on arrival
  `0x31d778` calls it with state 2 and the index/hash from: slice +0x1e8 if set and present in
  the object table, else the zone selection global `*(0x1a7134c)` (+8 name hash, looked up with
  `0x1406f0`, `0xb25bec`), else the **host activity state @0x54 (u32 index) / @0x58 (i32
  hash)** (activity object +0x50b0, `0x1c9298`). Our state left both 0, hence record {0, 0}.
  The leave check's final test (`0x31c2f8`) looks the index up with `0x140820`: status 0 or 5 ->
  state 12 (lobster), 6 -> 13, 7 -> 14; index -1 skips it. The zone machine `0x31eaf8` also
  skips index -1, and for a present object (status 2/3) posts the slot-2 zone switch with the
  record's index and hash. `bap_server.py --ah-zone INDEX:HASH` sets the two fields.
- TEST158b (`--ah-zone -1:0x811c9dc5`): the arrival record is -1 (`0x31d358` r5 = -1), slice +0x89
  stays 2 and **lobster is gone**: the client stays in the activity world for ~72 s (slice
  +0x1d2 counts 2 -> 3 -> 4 at +10 s / +17 s), then leaves with **baboon** ("Removed from the game
  world"), service 505, back to orbit. (TEST158 itself stalled at character select: the client
  stopped sending BAP frames, unrelated.) Next: what baboon checks (probably host simulation
  traffic the stand-in never sends).
- Baboon (TEST159-161): the leave is condition 30 (`0x32798c`) with reason 0x67. Reasons index the
  failure name table at `0x175eafc` in 8-byte entries at (reason - 0x18); condition 30 maps the
  activity-host session status (`0x303c68(0, 0)` = `*(0x30e048() + i*0x140 + 0x8a0)`, >= 9 =
  failed) through `0x326358` (table 0x11ea1c4, status - 9): status 36 -> 0x67 =
  `error.private_activity_host_host_crashed`. (Statuses 9..38 are the
  `private_activity_host_*` errors: 26 timed out waiting for join response, 27 entity indices,
  28 establish, 31 lost session, 33 lost connection to BAP, 35 booted, 36 host crashed, ...)
  Status 36 is set at `0x308c48` while in status 6 (in activity): `0x25b210(host)` = now - the
  host messaging object's (host +0xba8) last receive time, compared with `0x121518()` (config
  +0x13f8 unless overridden by `*(0x175c554)`) + config +0x2d10, gated by `0x1c97b8(0x11e179c)`.
  The stand-in pushed host messages only at the join, so after the timeout (~70 s in activity)
  the host counts as crashed. `bap_server.py --ah-heartbeat SECONDS[:SUBTYPE]` pushes an empty
  host message (default type 11) every SECONDS after the join, piggybacked on the client's
  ~30/s type-15 messages.
- `rpcs3_gdb_daemon.py`: a dump spec `rN:0` prints register N itself (e.g. a loop index).
- TEST158 and TEST160 stalled (no network syscalls, F12 screenshots not written, CPU still busy):
  an emulator hang, unrelated to the arguments (the same arguments worked in 158b/159/161).
- TEST162 (`--ah-heartbeat 1`, type 11 every second): **no baboon** - no failure in a 300 s
  trace window (only the transition steps 0x1e..0x21 at the join), still in the activity after
  6+ minutes (client host messages at ~30/s). The screen is the black loading screen with the
  spinning loader: the client waits in the activity world, presumably for the host's
  simulation stream (type 5: roster, replicated objects - see above), which the stand-in
  does not send. F12 screenshots need the game window to have focus (Qt shortcut); the
  PrintWindow capture of the game window works while it is unfocused.

## Update (comparisons 162–163): the in-activity loading screen
- After arrival the game-flow state is base+0x1d50 (vtable 0x12b7ff8, update `0x3281d0`): an
  11-task list at +0x30 (0x18-byte entries {u64 dependency mask, desc}; runner `0xd264dc`:
  a task runs once its dependencies are done, returns 0 not started / 1 running / 2 done;
  masks +0x138 required, +0x140 started, +0x148 done). When done == required it posts
  transition step 0x22 (the black loading screen ends there). Tasks: 0 `0x32895c`, 1
  `0x328a38`, 2 `0x328260`, 3 `0x3282e0`, 4 `0x328310`, 5 `0x328ae0`, 6 `0x3283b8`, 7 `0x3284b8`,
  8 `0x328610`, 9 `0x3286a8`, 10 `0x3287e4`. TEST162 (zone record -1): done 0xfb, task 2 stuck.
- Task 2 = world-controller zone readiness (`0x31ba0c`): a current record (slice +0x90 / zone +8)
  exists, or the target record's (zone +0xa8) index has status 3/4 in the scenario object
  table (`0x140820`: entry -1 -> 5; +0x1154 bit -> 6; +0x1194 bit -> 7; +0x110c active -> 4;
  +0x1110 pending -> 2; +0x1114 bit loaded -> 3; else 1).
- The table (`*(0x18a9440)`: +0 scenario handle, +8 0x200 entries of 8 bytes, +0x100c list of
  the scenario's bubble zone handles (22 for act_cosmo_m0_highway), +0x110c active, +0x1110
  pending, +0x1114 loaded bitset) is indexed bubble*8 + sub-slot: `0x14a8dc(i)` = 8*i (the
  bubble's base), `0x14a8e8(i, s)` = 8*i + s + 1. The sim start (`0x1c7608`, loop at
  `0x1c7800`) registers entries (`0x13f570`) from the host activity state's bubble states
  (@0x14): -1 leaves the base entry absent (only bubbles 13 and 14 got sub-slot entries, 105
  and 113); a state >= 0 registers the base and its sub-states. Scenario definition: +0x14
  count / +0x18 offset of 0x90-byte bubble entries (+0x28 name hash).
- With the target record at a present entry (`--ah-zone 105:0x811c9dc5`, TEST163) the world
  controller loads that zone set itself (`0x31f6bc`: status 1 -> `0x13f8ec(table, 105)`), it
  reaches status 3 and **tasks 0-7 complete** (done 0xff). Task 9 waits for task 8; task 8
  (`0x328610`) finishes when a current record exists or once zone byte 2 bit 0x80 is set,
  which then sets world-controller state 2 so the zone machine (`0x31eaf8`) posts the zone-set
  switch. Bit 0x80 (slice +0x8a) is set by `0x31c750` (via `0x31d054`) only when slice +0x310
  (group-1 host record, 0xd8 bytes) matches: with an activity host session (`0x304b78(0,0)`
  != 0) the record comes from the host simulation object (`0x25a988(host)` = host +0xbb8,
  records at +0x2e50 / +0x2f28: i32 zone index == slice +0x138, per-peer byte at +0x38 +
  `0x25bc10(host)` == slice +0x1d0), copied by `0x31d06c`. The stand-in never fills it (both
  records are defaults), so the load stops there. Next: which host message carries these
  records (not types 1, 2, 4, 18, 27, 29, 38, 39, 42, 43 by struct size; likely the type-5
  simulation stream).
- `bap_server.py --ah-bubble-states DEFAULT[,BUBBLE=STATE...]` sets the activity state's
  per-bubble states (default -1).
- **Host type 11 is the world state** (TEST164): its handler `0x25715c` reads the body as one
  present bit + a delta of world state 0x808009ed (0x5038 bytes; `0x1ca478` -> `0x25d2dc`, the
  same encoding as other host messages) onto the host simulation's copy (host +0xbc0 =
  simulation +8), normalizes it (`0x25bbec`) and posts it to the world controller as RPC 6
  (session method slot 0x1a4 `0x2d424c` -> `0x8283ac`, event 0x1b; receiver `0x8288d0` ->
  `0x1c8944`). Layout: +0/+4 i32, +8 20 peer slots of 0x80800a39 (0x250: opt identity
  0x80800c2d = join request @0x10, opt 0x80800c56 {opt 0x80800b1f = join request @0x38, ...}),
  +0x2e48 opt 0x80800bd7 {40 group records 0x80800c6f (0xd8: u32 zone-set index @0, u2 @4,
  {i8, u2 state, u5 peer, bool} @8, i32 @0x20, {i8, i32, i8[20] per-peer @0x38, u8+i8[128]
  @0x4c} @0x28), ...}, opt 20-bit masks @0x5028/0x502c (occupied peers)/0x5030. The local
  peer slot is the one whose identity's first 8 bytes equal the simulation id (`0x1c9ab8`).
  `bap_server.py --ah-world-state ZONE[:PEERBYTE]` sends the client in slot 0 and group
  record 0 = {ZONE, state 1, peer 0, per-peer byte}. With `--ah-zone 105:0x811c9dc5
  --ah-world-state 105:1`: the world controller copied the record, set the zone-byte-2 flag,
  and **zone set 105 became active** (table +0x110c = 0x69, slice current record = 105);
  loading tasks 0-8 done, task 9 running, task 10 pending; still the loading screen.
- Loading task 9 (`0x3286a8`) also needs a resource of kind 0x10 (`0xb26e4c` -> `0x1483c4`) and
  the local player's object: `0x1226ec` (session method +0x88 `0x2d2e40`) = local user 0's
  player index `*(0x197fa80)+0xe4[4]` (-1 here), bound by `0x682fe0(user, player)` (player
  records 0x27b0 bytes at `*(0x197f9d4)+4`).
- **Host type 5 = simulation packets** (TEST165/166): event 0x13 -> `0x827d18` -> `0x1c85e0`:
  u8 epoch, roster-present bit, 65 changed-slot bits, roster delta (present bit + type
  0x80800cf7 {opt u32, 65 x {opt 8-byte peer id, opt u16, bool active}}), u32 host tick, then
  the replicated object stream (`0x1ce4a4`: per object a more-bit, typed object id, u32 length,
  state-present bit and a typed state; only read when the session's simulation is active and
  the epoch equals session +0x11). `bap_server.py --ah-roster SLOT[:GEN]` sends the client in a
  roster slot (resent with each heartbeat: a packet sent with the join is dropped because the
  host session does not exist yet). In TEST166 the peer joined (`0x123478`, then `0x1234c8`).
- Still no local player: the player manager (`0x66eb78` -> `0x66dd6c`, `0x66e808` only when
  `0x66c2d4()`; `0x81db98` -> `0x674054(machine id)`) binds the local user once its machine id
  (peer table `*(0x197f9dc)`: +4 mask, +8 20 x 8-byte ids, +0xa8 {1, own id}) is in the
  table; the table is empty (mask 0). `0x66c2d4()` is 0 because `*(*(0x197ede4))` = 0: the
  16-byte network-game id at setup +0x2730 (posted by the world change `0x302728` or
  `0x6884c4`/`0x68b9ac` through `0x4282d4` -> `0xd340fc` -> `0x66b730`) stays zero after a
  world change. Next: what fills the player manager's member list (`0x69c874`, 20 x 0xe0).
- Local player binding (TEST167/168): the world change (`0x302728`) builds the game setup with
  `0x66be30`: machine table mask 1 + own machine id (setup +0x20/+0x24) and own id at +0xc4;
  `0x6884c4` -> `0x66b958` loads it into the player manager's machine table (`0x674294`,
  `*(0x197f9dc)`) and binds the local user via `0x674054(own id)` (-> `0x682fe0`). One second
  later **world-controller RPC 16** (event 0xd, type 0x80802d76 = u32 mask + 20 machine ids,
  receiver `0x82cd68` -> `0x674294`) arrives with mask 0 and clears the table again. RPC 16 is
  posted by `0x82b998` from `0x81df58`, which copies the member list (+0x2c mask, +0x3c ids,
  valid flag +0xdc) of the current group entry of the group/session manager
  (`*(0x19b7224)`, index +0xd8640, 0x42350-byte entries from +0x11c50; the entry is the
  fireteam group object, helpers `0x80f2f8`/`0x80f304`, version counter `0x8129f0`). That
  member list is empty in the local flow, presumably because the BAP peer/roster families
  (subscriptions 0 and 2, pushed empty by the stand-in) carry the fireteam's machines. Next:
  what those families must contain for the local machine to be a member.
- The group entry that RPC 16 reads belongs to the manager `*(0x19b7224)` (entries keyed by a
  u64 group id at entry +0x10; all users in `0x82a264`-`0x82cbd4`, next to the RPC receivers of
  the table at 0x12c3ee0). Its RPC 0 (event 0x15, type 0x80802d3c, receiver `0x828714` ->
  `0x82af3c` -> `0x7e4e40`) is posted by session method 0x190 (`0x2d417c` -> `0x827ea8`) from
  **host message type 24** (`0xd0da9c`, type 0x80800d10 = {u3, 384-word bitmap}). The group
  objects of the fireteam manager (`*(0x18ea68c)`, sessions at +0x10 + group*0x11938) do contain
  the local member (TEST170: `0x802650` adds it at the join), but the RPC 16 entry stays
  empty. RPCS3's GDB stub rejects write watchpoints (Z2 -> empty reply; TEST171).
- Host machine list (TEST175/176): the host object keeps the activity's machines at +0x6204
  (count) / +0x6208 (10-byte records: 8-byte machine id, flag, flag); the client adds its own
  machine from the join participants (+0x540 count, 0x28-byte identities at +0x548). Host
  type 10 (`0xd0d6a8` -> `0x256eec`, raw bytes: u64 != 0 + 0x88 bytes to host +0x880, needs host
  flag 0x2 and not 0x4) rebuilds it. **Host type 43** (`0xd0e214`, type 0x80800f1a = 5-bit count
  + up to 20 machine ids) is a boot list: the per-host update (`0x30956c` -> `0x25afcc`) moves a
  listed machine to state 4 and the client shows **bass** ("Removed from the game world").
  The session listeners (`0x818794`, `0x66eaec`) never fire in this flow.
- The RPC 16 source entry (manager `*(0x19b7224)`, host mode +4 = 2, own machine id at +8,
  state +0x28 = 4) is entry 0 = the activity host connection (+8 host object); its member
  list (+0x28 count, +0x2c mask, +0x3c ids) stays empty.
- Correction on the binding: `0x674054(machine id)` binds local users only to **existing player
  records** (pool "players" `*(0x197f9d4)`: +4 records of 0x27b0 bytes, +0xc count; +0x628 machine
  slot, +0x62c user, +0x630) whose machine slot equals the local slot (+0xb4). The pool is
  empty (count 0) in the activity, so even the valid one-machine table at the world change binds
  nothing (`0x682fe0` never runs). Player records come from the player manager (`0x66dd6c`, run
  by the listener `0x66eaec` with flag bit 2), which is not invoked in this flow. The manager
  mode (+4) is `0x66c2c0()` = the first word of the setup's network id (0 after any world change
  -> host mode 2, RPC 16 posted by the client; 1 -> client mode).
- Where player records come from (static, 2026-09-26 02:15): the module notifier `0x68c4b0(flags)`
  (31 callbacks at 0x1764664, including the player manager `0x66eaec` and the session listener
  `0x818794`) is only called from `0x68b9ac`, which runs when a ".cor" file is loaded
  (`0x303150` -> `0x68bde0`) - not in the normal flow; TEST177 confirmed neither listener fires
  from boot to the activity. The normal-flow player creation found so far: the group message
  dispatcher `0x82cd90` (73 kinds by msg byte 0; registered at `0x82a280`) sends kinds 12/14
  to `0x8242a0`/`0x8243b0` -> `0x82087c` -> `0x672dec`/`0x672510`, which create players and bind
  them (`0x671560` -> `0x682fe0`). Next: who posts group messages of kind 12 (probably the
  peer/session layer when a machine joins the group) and whether the host can trigger it.
- Player creation traced further (TEST178/179, live traces 2026-09-26 10:30): all queued events
  pass first through the group dispatcher `0x82cd90` (registered in `*(0x18aed70)` by
  `0x82a264`; kind = event type), so event 0xc/0xe = player join. Event 0xc is posted by
  `0x8241a8` (bit-serialized record) for each record returned by `0x81eacc`, called from the
  RPC 16 sender `0x82b998` right after RPC 16. `0x81eacc` maps the activity host to its
  fireteam group session (`0x305c18(host)` -> `0x305ab8`: session B here, group object
  0x405dc548, lobby state 6) and, for lobby states 4-9, `0x81e404` builds join records: for each
  group player (mask +0x1cf4, 0xe0-byte records at +0x1cf8; player 0 = id `cadefb79e05fb7b0`,
  name "d1localtest") it emits a record only when the manager's **expected-player list**
  (obj = manager X + 0xd8650: mask +0x10, 0xe0-byte records from +0x18 with the player id at
  +0x18) holds that id with its +0x20 flag clear. The expected list is empty (mask 0), so no
  record, no event 0xc, no player, no binding. The player id also sits in the host's join
  participants (host +0x548 identity +8) and in world-state peer 0 (sent by the stand-in), but
  nothing copies it into the expected list. Next: find the writer of obj +0x10/+0x18 (obj
  methods around `0x81d000`-`0x820000`; not an OR/AND mask update) - likely fed by host data
  (world state peers or another world-controller RPC).
- **Player created and bound** (TEST180, 2026-09-26): the manager entry's member list is filled
  when the entry attaches to the activity host (`0x81c0c4(entry +0x1c, host)`, called from the
  manager reset `0x82a698` -> `0x82a5b0` -> `0x81aba4`): member mask = world state **+0x5028**
  (`0x1c9b98`), ids from the peers (`0x1c9a20`). The world-state delta now sets +0x5028 bit 0 as
  well as +0x502c: RPC 16 carries mask 1 + our machine id, the expected-player list fills, the
  group dispatcher gets event 0xc (`0x8241a8`) -> `0x8243a0` -> `0x820e0c` -> `0x672fdc` ->
  `0x671b64` -> `0x682fe0`: the player pool has 1 player and local user 0 is bound (the machine
  table keeps our machine). Loading tasks: still 0-8 done, task 9 running: its two remaining
  checks are the kind-0x10 lookup (`0xb26e4c` -> `0x1483c4(0x10, 0, &out)`) and the local player's
  object (`0x129db8` -> `0x129c8c` -> `0x14a12c`, +8 byte) - the player's in-world object is not
  created yet.
- Task 9's remaining checks (TEST181/182): (1) `0xb26e4c`: the game-mode object (world object of
  kind 0x10 in the per-world object registry: world index u16 `0x18a9680`, registry at
  `*(0x17d2d00)[idx]+0x30`, per-kind start/count at +0x16d10, ids at +0x15510; kind 0x10 has 1
  object, here at 0x70794f80, tag 0x80cd0ef4) must have its byte +0x110 set (not -1/0). That byte
  (`0xb26d88` returns it) is computed at object init (`0xb2d638`): +0x111 = `0xb30874(def)`,
  +0x110 = `0x18b88c(0xb308c8(def, tag))` = a per-world-object byte table (`*(0x18aae60)`, index
  from the object id) -> -1 here: the referenced object has no slot yet. (2) the local player's
  world object (`0x129c8c` in the entity pool 0x18a8ee0) does not exist. Both point at the
  player's in-world object not being spawned. Bubble states 0 for all bubbles (TEST181) and
  bubble 0's base zone set (TEST182: `--ah-zone 0 --ah-world-state 0:1`, zone set 0 active)
  change neither.
- **Simulation player** (TEST183, 2026-09-26): correction - the local player's `pla` object
  (pool 0x18a8ee0, 0x1a0 bytes; +0x18 machine id, +0x4c player handle, +0x50 simulation player
  entity) does exist; what task 9 lacks is its **simulation player** (+0x50 = -1). `0x14a12c` ->
  `0x14a05c` resolves +0x50 and the entity's +0x134/+0x138 to the entity's 0x50-byte block at
  +0x144 (type 0x808022a5); task 9 needs its bool +8, the spawn readiness check `0x35793c`
  ("can't spawn because no simulation player exists") its bool +9. The world holds 16
  simulation-player entities (kind 0xc, entity group 3 = key 0x4786c0e0, indices 5-20; group
  table at world+0x10 in 0x14-byte steps, key +0x10 / base slot +0xc, slot records
  world+0x510+slot*0x1c = {handle, offset, type}), all with owner machine id 0 at +0x15c. The
  binder `0x14a270(machine id)` (from the component handler `0x4ed7b4`, message table
  0x1249c60) finds the pla with that machine id and the kind-0xc entity whose owner id matches,
  then sets pla +0x50 (`0x128298`). The per-frame spawner (`0x35a3e0` -> `0x359ab0` for players
  whose record +0x644 unit is -1, event 0x4c = spawn, receiver `0x35acc8`) stops at `0x35793c`
  (traced: returns 0 every frame).
- Each entity's replication record (entity +0x104; e.g. {id 0x4786c0e0/0xc/5, +8 type
  0x80802c30, +0xc type 0x808022a5, +0x15 awaiting baseline, +0x3c bubble -1}) is updated by the
  type-5 object stream (`0x1ce4a4`): start bit, then per object 1 bit, id (type 0x808004b1 =
  {u32 group key, u8 kind (6 bits, +1), u16 index (+0x8000)}), u32 bit length, state A (reset
  bit, present bit, plain encode - `0x1ca478`), state B (present bit); 0 bit ends the list.
  Unknown ids are skipped by the length. Field kind 0x23 (machine id) is 64 raw bits
  (`0xd1d184`). `bap_server.py --ah-sim-player 0x4786c0e0:5:OWNER` replicates simulation player 5
  with that owner id and bools +8/+9 set.
- TEST184: the object is accepted (record +0x15 cleared, +0x42 set) and the state lands in the
  record's receive buffer (record +0x28/+0x2c = offset/world index 0x218), but it is not applied
  to the entity: the end-of-list pass (`0x1ce818`) first requires every replication record of
  the active bubbles to have had a baseline (`0x1ce1b8` checks +0x15), then applies received
  records (`0x1cfa1c` -> `0x1cf8f4` -> `0x25f6b8`). The client has 22 records, all bubble -1:
  0x992bb471/6/29, and group 0x4786c0e0 kinds 0xf/0, 0x22/1, 0x11/2, 0x10/3 (the game-mode
  object), 0x27/4 (with state B), 0xc/5-20. Also the owner id the binder compares is pla +0x18 =
  the player id 0x0a3d4a2c8fbea6ba, not the roster peer id. `--ah-baselines KEY/KIND/INDEX[/B],...`
  sends default baselines (reset bit, no fields) for the others.
- TEST185: with default baselines for all 22 objects the gate opens: the client applies the
  received states (`0x25f6b8` per record), the sim-player component handler runs the binder
  (`0x4ede14` -> `0x14a270`) and **pla +0x50 = simulation player 5**; the spawner then posts
  spawn event 0x4c for the player (`0x35acc8`, player +0xc bit 0) and continues in `0x358fc4`,
  which still fails `0x35793c`. Loading task 9 still waits on the game-mode check.
- The game-mode object (kind 0x10, group 0x4786c0e0 index 3) holds its replicated state (type
  0x808008a3, 0x9a0 bytes) at entity +0x110; `0xb26e4c`'s "slot byte" is that state's first
  field (4-bit u8). Task 9 needs it non-zero; `0x35793c` switches on it (table 0x11ed724,
  indexed by the value) and only 3 continues (0-2 and 4-8 refuse). The default baseline sets it
  to 0. `--ah-game-mode 0x4786c0e0:3:STATE` replicates it.
- **Loading screen dismissed** (TEST186): with the game-mode state 2 all 11 loading tasks
  complete (required/started/done = 0x7ff) and the loading screen goes away; the screen is black
  (only the "BETA BUILD" watermark) because the player has no unit yet. The spawn check
  `0x35793c` now fails its first test: the active zone set (object table +0x110c) must be a
  bubble's base entry (`0x14a934`: index != -1 and index & 7 == 0); 105 is bubble 13's second
  entry.
- Zone set for spawning (TEST187-189): `--ah-zone 104`/`0` alone end in **lobster** - base
  entries (8*i) exist only for bubbles whose host bubble state (activity state @0x14) is >= 0.
  With `--ah-bubble-states -1,13=0 --ah-zone 104:0x811c9dc5 --ah-world-state 104:1` zone set 104
  (bubble 13's base) becomes active, but loading bubble 13 instantiates its content: the client
  then has 121 replication records (groups 0x992bb471, 0x3c2f5ed5, 0x4786c0e0 and bubble 13's
  0x062dd95d; kinds 1/3/4 with state B), 99 awaiting a baseline, so nothing is applied and the
  simulation player stays unbound. `scripts/rpcs3_replicated_objects.py` dumps the record list
  from a running game in the `--ah-baselines` format; `--ah-baselines @FILE` reads it.
- Spawn processing (TEST190-191): with all 121 baselines, zone set 104 and game-mode state 3 the
  readiness check passes (`0x35793c` returns 1; `0x34cd48` returns 0) and the spawn event carries
  state 1 (player +0x5b0). The spawn processing `0x358688` then needs `0x3577b8`, the sim
  player's components `0x4ec7b4` (entity +0x1ec/+0x1f0 -> entity +0x18c, the replicated state's
  bool +0x48) and `0x4ec808` (`0x4ee5f4`), and player record +0x560 = 1 or 2 (it is 1); it sets
  byte +4 of the second component (a spawn request) and returns until that bool is set -
  presumably the host's grant. `--ah-sim-player ...:1` sets it (TEST192 set the real32 at +0x10
  by mistake: no change).
- Spawn body (TEST193): with the grant set the spawner enters the spawn body every frame
  (`0x358814`): spawn-point search over the 4 strategies (table 0x175f5ac) succeeds, then the
  player object creation `0x358210` fails at once: player record +0x1428 (appearance block whose
  first word is the player blueprint handle; "can't spawn because the player blueprint handle
  is invalid") is -1. It is set by `0x2bef68` from `0x2f5b28`/`0x2f6044(player, 0xff0-byte
  appearance)`, built by `0x670e40` from the player's character data at +0x8b0, which
  `0x673180` fills from the player manager's character cache (`0x299528`: 20 entries of 0xbb0
  bytes keyed by account soid). Our entry sits in state 1 (needs 5): the cache state machine
  `0x2998e0` looks up BAP family 0 (peer) kind 0 by the account soid (type 0x80801921, hash
  0x1d7bb8f6: {u64 account, u64 character, ...}) and then kind 1 by its character soid (type
  0x808017fd, hash 0xa0a1bc80: {u64 id, class, appearance, +0x30 0x80801855, ...}). The stand-in
  pushed family 0 empty. Object store pools: store + 0x2a3c0 + family*0x40 + kind*0x10 = {data
  offset from store+0x2298, count, stride, type}. `bap_server.py --peer-family` fills family 0
  with both objects from the stored character.
- **Guardian spawned in the mission** (TEST194, 2026-09-26 13:05): with `--peer-family` the
  character cache entry reaches state 6, `0x673180` fills player +0x8b0, the appearance/blueprint
  is set (player +0x1428 = 0x80a6a022), the spawner creates the player object (`0x358210` ->
  `0x3584ac` -> ...) and the player record gets its unit (+0x644 = 0x29fac5c7). The screen shows
  the mission world with the HUD (radar) behind an "Urgent message" dialog (servers going
  offline - presumably from the all-true account flag tables of `--self-flags 2`) and a
  "Level Up" toast; X dismisses the dialog. Rendering: the right half of the frame is
  overexposed/white, the left half dark (RPCS3 issue to look at separately).
- Rendering in the mission (TEST195-196): with the global RPCS3 config (Vulkan, all buffer
  read/write options off) the frame has fixed black rectangles (e.g. x 0-800, y 360-592) and the
  exposure drifts to saturated red/yellow within ~30 s (auto-exposure fed by the black areas);
  the emulator died ~12 min in with an RSX fatal (`simple_array<VkDescriptorImageInfo,8>::reserve`
  verification). A per-game config (`config/custom_configs/config_BLES01857.yml` in the private
  RPCS3 copy) with Handle RSX Memory Tiling + Write Color Buffers alone changed nothing; adding
  Read Color Buffers, Read Depth Buffer and Write Depth Buffer (TEST196) gives a correctly lit
  frame at first, but after ~1 min a black block and the overexposure return (surface aliasing?).
- TEST198 (2026-09-26): an experiment overriding `--self-flags` with
  `1;2@A0-1023,B0,C0-1023` reached the activity host but returned **banana** (required content)
  before the mission. This narrower flag set is not a valid graphics comparison with the
  successful `--self-flags 2` baseline. An RPCS3 `PrintWindow` capture appeared stuck on a
  shader-preload screen, but the emulator's F12 screenshot at 13:51:43 showed banana. Use F12
  captures for the actual frame; `PrintWindow` may return an old frame. Test log:
  `<PS3_ROOT>\SIGNON_LOCAL\logs\TEST198-bap.log`.
- RPCS3 upstream issue [#12802](https://github.com/RPCS3/rpcs3/issues/12802) (2022) reports
  a white square on the right in Destiny and says Force CPU Blit avoided an older RSX
  verification error but caused flicker/corruption. It is related evidence, not a confirmed fix
  for this build. The RPCS3 wiki did not yield a Destiny-specific configuration page.
- TEST199: with the TEST196 buffer settings and `Force CPU Blit: true`, RPCS3 crashed at the
  character screen, before service 504/select-character or the mission. Windows Application
  Error at 13:59:34 records exception `0xc0000005`, offset `0x3a56cb8`; the last attempted
  F12 screenshot is a zero-byte file. This run cannot determine whether the setting improves
  the mission image. `Force CPU Blit` was restored to false; the original per-game config is
  backed up as `config_BLES01857.before-test199.yml`. `scripts/Invoke-RpcsStep.ps1` now waits
  for a nonempty new screenshot and errors on timeout instead of returning a zero-byte file.
- TEST200 (2026-09-26): only `Allow Host GPU Labels: true` differed from the TEST196 game config,
  using the proven `--self-flags 2` baseline. This tests an RPCS3 developer's suggestion on
  [issue #17413](https://github.com/RPCS3/rpcs3/issues/17413) for a different game's flicker.
  The mission loaded. The initial frame (F12 14:09:44) was reasonably lit; after dismissing the
  urgent-message dialog the left side darkened and right side overexposed (14:10:08). The
  lighting briefly recovered (14:10:29) and stayed even while stationary (14:11:37), but after
  holding W for 800 ms it split again (14:12:11) and remained split at 14:13:01. A later live
  window capture showed the same split while a Level Up prompt appeared. Thus Host GPU Labels
  does not resolve the movement-triggered rendering fault. Screenshot files are under
  `<PS3_ROOT>\RPCS3_TEST048_SETUP\screenshots\BLES01857\`; the service logs are
  `<PS3_ROOT>\SIGNON_LOCAL\logs\TEST200-*.log`. The config was restored from
  `config_BLES01857.before-test200.yml` (`Allow Host GPU Labels: false`, `Force CPU Blit: false`).
  A still-running TEST200 process retains its launch-time setting until restarted.
- The next rendering diagnostic was to capture the bad frame with RPCS3's RSX capture facility
  and inspect the affected render target/texture history. Initially, the unattended game run
  exposed no main RPCS3 menu because a `Welcome to RPCS3` dialog was still open; TEST201
  completed that dialog and obtained the capture. Do not treat a `PrintWindow` screenshot as
  live evidence. Do not commit game content, private service data, or multi-gigabyte captures.
  There is no confirmed
  Destiny-specific RPCS3 setting fix in the checked upstream sources.
- TEST201 (2026-09-26): enabled only `Strict Rendering Mode: true` on top of the TEST196
  per-game settings. The mission loaded with the proven `--self-flags 2` account baseline, but
  the first F12 frame at 14:30:56 already had a bright purple horizontal band and large dark
  blocks behind the urgent-message dialog. Strict mode therefore does not fix the artifact.
  Restored the per-game config from `config_BLES01857.before-test201.yml` (strict false).
  RPCS3 `Utilities > Create RSX Capture` saved
  `<PS3_ROOT>\RPCS3_TEST048_SETUP\captures\BLES01857_20260926143340_capture.rrc.gz`
  (42,555,130 bytes); `RPCS3.log` confirms capture success. The menu was accessible after
  completing the RPCS3 welcome dialog. `Utilities > Open RSX Capture` asks to close the running
  game; inspection of the capture is the next step. TEST201 service logs are in
  `<PS3_ROOT>\SIGNON_LOCAL\logs\TEST201-*`.
- TEST202 (run launched as "TEST199" in parallel with the other agent's TEST199; its logs were
  renamed to `TEST202-*`, the other agent's TEST199 service logs were overwritten - its result
  is recorded above): full mission run with `--self-flags 2`, tracing the unlock-expression
  evaluator `0xb67984` and the account-flag copies (`0xb674c8`, `0xb67540`) from the join for
  300 s: **no hits**, although the guardian spawned and the urgent-message dialog appeared. So
  the urgent message is not raised by an unlock expression evaluated after the join; the
  content check ("banana") must also run before `ahp req`. The universe object (family 5 kind 3)
  has no scheduled entries (+0x90 count 0, +0xc4 = 0) and the stand-in never sends BAP
  "shutdown not" (0x14); the dialog's trigger is still unknown.
  `Invoke-JoinCycle.ps1` now also writes the full trace to `logs\<Name>-trace.log`.
- TEST203: OpenGL renderer with the TEST196 buffer options: RSX fatal 63 s after boot
  ("Memory allocation cannot fit image contents"), never signs in. Not usable; back to Vulkan.
- TEST204: TEST196 options + Accurate RSX reservation access + Disable Asynchronous Memory
  Manager: mission reached; the keyboard pad moves the guardian (W held 800 ms walks forward).
  Same fault as TEST200: right after moving the frame splits (black rectangles x 0-800 at
  y 0-145 and 360-592, rest overexposed); after ~60 s standing still it is correctly lit
  again. These options do not fix it (config restored from `before-test204`).
- TEST205: + Strict Texture Flushing: worse - a purple/white band over the lower half that did
  not change for a minute. Reverted.
- TEST206: + SPU XFloat Accuracy: Accurate: same fault. Right after moving the frame showed a
  clean vertical split at x = 800 (left normally exposed, right overexposed); a minute later the
  left part had black blocks at y 0-145 and 360-592 and a bright band between. The image is
  assembled from regions that are updated at different times; this fits SPU-side post
  processing writing into video memory in strips, some of which RSX does not pick up (or reads
  stale). Reverted.
- TEST207: + Accurate SPU DMA + Accurate Cache Line Stores: a short move (W 800 ms) and the frame
  a minute later were clean, but sustained movement (W 2.5 s, A, S, D) brought back the same
  fixed blocks (magenta x 0-800 y 0-145, black x 0-800 y 360-592, overexposed elsewhere); it
  recovers when standing still. At most a partial improvement.
- Summary of the RPCS3 settings search (TEST195-207): Read Color Buffers + Read Depth Buffer +
  Write Depth Buffer (with tiling and Write Color Buffers) are needed for a correctly lit frame
  at rest. Nothing tried removes the movement-triggered blocks: host GPU labels, strict
  rendering, strict texture flushing (worse), Force CPU Blit (crash), OpenGL (crash), RSX
  reservation accuracy, synchronous memory manager, accurate SPU xfloat, accurate SPU DMA /
  cache line stores. The block geometry never changes (always the left 800 px, rows 0-145 and
  360-592), so it is one specific surface/region, not general inaccuracy. Next: find which
  render target or SPU job covers that region (RSX capture from TEST201, or tracing the
  post-processing SPU job's output addresses), then report it upstream or work around it.
- Rendering checks are now scripted: `scripts/Invoke-MovementCheck.ps1` (dismiss, walk W/A/S/D,
  settle 45 s; one F12 frame per step) and `scripts/frame_blocks.py` (scores each frame: black /
  magenta fraction in the two fixed blocks and mean brightness left/right of x = 800; a normal
  frame of the dark interior averages 10-20, broken ones > 90 or > 35 % black in a block).
- The RPCS3 log shows RSX local address 0xCEA18000 reused every frame as both a 1024x312 and a
  512x312 texture ("Cached object ... does not match stored parameters", ~300 per 10 s each
  way). The game renders internally at 1024x624; 312 is half height, and the lower block starts
  at screen row 360 = internal row 312. The blocks in internal coordinates are x 0-640,
  y 0-~128 and 312-~512.
- TEST208: + Disable ZCull Occlusion Queries: worse - every frame after movement is broken and
  it does not recover after 45 s. Reverted.
- TEST209 (aliasing bias Prefer Color), TEST211 (Disable Vertex Cache; TEST210 with the same
  config hung the RSX thread at 3:31 before the mission) and TEST212 (Handle RSX Memory Tiling
  off with read/write buffers on): all still break after movement; none changes the pattern.
  The game runs at a steady 30 FPS (window title), so it is not a speed problem.
- In a broken frame the lens flare draws *inside* the black block: later passes are fine, only
  the lit scene in those regions is missing. The executable has SPU render jobs
  (`spu_deferred_lighting_job`, `spu_lighting_lights_array`, `spu_render_submit_lights_view_job`,
  `render_submit_lights_view_job`). The deferred-lighting job (state `*(0x1761280)`, dispatch
  `0x5afc34` -> `0x5afcd0`, per-light batches of up to 200 lights, jobs via `0xaf3bfc`) is not
  dispatched in the mission (state +0x9ac stays 1; no calls in 5 s). Next suspect: the
  SPU-built light command lists (lights view job) read by the RSX FIFO.
- TEST213: RSX FIFO Fetch Accuracy "PS3": same random mix of clean and broken frames. Reverted;
  the per-game config is back to the TEST196 settings.
- Upstream context: the reporter of RPCS3 issue #12802 (2022, real private servers) showed the
  same overexposed right side; their fork (`cohaereo/rpcs3-v4nguard`, last pushed 2022-09) only
  implements `sys_net_bnet_sendmsg` and comments out RSX verification asserts - no rendering
  fix. So this is a long-standing RPCS3 emulation fault, not something our stand-in causes.
  No per-game setting tried (TEST195-213) removes it. Remaining routes: (1) debug RPCS3 itself
  (build from source, log surface-cache decisions for the 1024x624 targets and the
  0xCEA18000 1024x312/512x312 alias), or (2) find the game pass that renders the lit scene
  into those regions from guest memory (Write Color Buffers is on, so surfaces are written back)
  and work around it in the game.
- **Root cause located (TEST213, live guest memory)**: the RPCS3 log names two 1024x624 textures
  in *main* memory, 0x445A3A80 and 0x44813A80 (adjacent, 1024*624*4 apart). Dumped from guest
  memory (big-endian A8R8G8B8; scratch helper `dumpimg.py`): 0x445A3A80 is the scene **depth**
  (complete), 0x44813A80 the G-buffer **normals** - but in guest memory only one 128-row band
  (e.g. rows 352-480, moving between samples) holds current data; the rest is black, the top
  rows magenta garbage. The SPUs (`spu_*lighting*` / lights-view jobs) read these main-memory
  copies, so lighting for regions with missing normals comes out black (or magenta, matching
  the magenta top-left block) and auto exposure then overexposes the rest. When the camera is
  still the partial write-backs of successive frames carry the same image, so it converges;
  movement exposes stale regions.
- RPCS3 source (VKGSRender.cpp, texture_cache.h): with Write Color Buffers, a render target's
  memory is locked for write-back over `m_surface_info` = the *current* framebuffer layout
  (width x height of the active surface setup). If the game fills the buffer in 128-row
  bands (separate surface setups or blits), only the latest band is protected and flushed on
  SPU access. Next test: Force CPU Blit (TEST214) in case the bands are RSX copies.
- TEST214: Force CPU Blit on the current baseline: RPCS3 grew to 24.7 GB and never finished
  signing in (the other agent's TEST199 crashed at character select with the same option). Not
  usable; reverted. The remaining fix is inside RPCS3: make the write-back/flush cover the whole
  main-memory G-buffer the SPUs read (or flush the full surface on SPU DMA access), which needs
  a source build of RPCS3.
- **RPCS3 source build** (2026-09-26): RPCS3 at our release commit b5a174e9 cloned to
  `<DEV_ROOT>\rpcs3` (submodules), Qt 6.11.2 msvc2022_64 from the Qt online repository (package
  archives downloaded directly: aqtinstall 3.3.0 does not know the 6.11 repository layout) to
  `<DEV_ROOT>\qt`, a minimal Vulkan SDK (`<DEV_ROOT>\vulkansdk`: Vulkan-Headers v1.4.341 + an import
  lib generated from `System32\vulkan-1.dll`), RPCS3's precompiled LLVM libs
  (`llvmlibs_mt.7z`, sha256 verified, extracted with 7-Zip's `7zr.exe`). `MSBuild rpcs3.sln
  /p:Configuration=Release` builds `bin\rpcs3.exe` in ~15 min (only the gtest test project
  fails). The private RPCS3 copy keeps the original as `rpcs3.orig.exe`.
- **Diagnosis with the instrumented build (TEST215)**: per frame the game uses the RSX blit
  engine (NV3089) to copy G-buffer data from VRAM into main memory for the SPUs: depth
  0xCF400000 -> 0x445A3A80 (1024x624), and **three different sources** (0xCE540000,
  0xCE1A8000, 0xCDE10000; pitch 5120) -> the same destination 0x44813A80; plus an NV0039
  copy 0xCE8E0000 -> 0x444EFA80 (1024 bytes x 624 lines). The SPU lighting jobs
  (`JQSPURSCellSpursKernel0-5`) read those buffers between the blits and fault into RPCS3's
  lazy write-back (thousands of access violations at 0x44813000-0x44813280 and 0x445A3A80+).
  RPCS3 does these blits on the GPU and keeps the result in its texture cache, writing it back
  only on access; with several blits to one address between SPU reads, the SPUs get stale or
  missing data -> the black/magenta blocks.
- Fix (patch `patches/rpcs3-destiny-main-memory-blit.patch`): in `nv3089::image_in`, a blit
  from VRAM (frame buffer DMA) into main memory (host buffer DMA) takes the existing CPU path,
  so the result is in guest memory immediately. Only these transfers change; the global
  Force CPU Blit (which bloats memory) stays off.
- TEST216 (blit fix deployed): the normals copy is now complete in guest memory right after the
  blit (rapid snapshots show the full normal map, then a partially written one, then the
  buffer overwritten) - but the frame still breaks after movement. So the blit write-back was
  a real defect but not the whole cause. New observations: the SPUs also *write* into
  0x44813A80 (write faults from `JQSPURSCellSpursKernel*`): between blits the first ~45 rows
  hold a compact table of 16-bit pairs (e.g. 0x014a/0x014b), most likely per-tile depth
  min/max that the RSX then samples for tiled light culling. Stale tile bounds would cull all
  lights in those tiles -> black blocks of tile-grid shape; the magenta garbage in dumps is
  this table read as colour. The 8-bit mask copy (NV0039 0xCE8E0000 -> 0x444EFA80, values
  3-10) has no block-shaped holes. Next diagnostic: log which textures the RSX samples from
  0x44000000-0x46000000 and whether RPCS3 serves them from its cache or re-uploads from memory.
- TEST217 (texture diagnostics): the RSX samples 0x44813A80 as a 1024x624 texture (fmt 0x9f)
  re-uploaded from memory on every use, and the depth copy 0x445A3A80 (fmt 0x9c/0x85) mostly
  from RPCS3's cache - legitimately: the CPU-path blit's writes fault and invalidate it. SPU and
  RSX write faults on these buffers are only a few per frame, no `semaphore_acquire` timeouts,
  and the fragment-program decode errors ("Bad src reg num", "Invalid Src type 3") happen only
  while the shader cache compiles at boot. The SPU-written table at the start of 0x44813A80 is
  linear (row groups 0-7, 21-29, 40-45), not a plain tile grid. Status: the blit write-back fix
  makes the SPUs' inputs correct in memory, but the movement-triggered blocks remain, so a second
  stale/ordering problem sits after the SPU stage (their output, or how the RSX consumes it).
  The patched `rpcs3.exe` stays deployed (original: `rpcs3.orig.exe`); the patch (fix +
  `D1DIAG` logging of blits, copies, faults and texture uploads in 0x44000000-0x46000000) is in
  `patches/rpcs3-destiny-main-memory-blit.patch`.
- TEST218: patched build + RSX FIFO Fetch Accuracy "PS3" + Accurate RSX reservation access:
  no change (blocks after movement, no recovery in 45 s). Config restored.
- Timeline trace (TEST219; marker-file gated `D1T` lines, `scripts/d1trace_timeline.py`):
  per frame the game does one blit of depth (0xCF400000 -> 0x445A3A80), one of normals from a
  *rotating* triple-buffered VRAM target (0xCDE10000 / 0xCE1A8000 / 0xCE540000 -> 0x44813A80)
  and the NV0039 byte extract 0xCE8E0000 -> 0x444EFA80 (1 of 4 bytes of a 1024-wide 32-bit
  surface: most likely the stencil byte of D24S8, values 3-10 = material ids). During gameplay
  the RSX samples only the depth copy (X32_FLOAT, then A8R8G8B8); the half-float samples of
  0x44813A80 seen in TEST217 all fall in the loading period. RSX semaphore acquires (labels
  0x603004x0) never wait. With tracing on, frames stretch to ~500 ms and the image was clean -
  consistent with a timing-dependent staleness. Current hypothesis: the copies read VRAM
  surfaces (stencil/normals) that RPCS3 has not written back yet, so the SPUs classify pixels
  with the previous frame's data - invisible when still, wrong after movement. TEST220 forces a
  write-back of the VRAM source (per-page `on_access_violation`) before every VRAM -> main copy.
- TEST220: forcing a write-back of the VRAM source (per-page `on_access_violation`) before every
  VRAM -> main copy: still broken - stale VRAM sources are not the cause. With the trace on,
  broken frames also occur, so it is not a pure timing artefact.
- TEST221 (FIFO call/jump tracing): the RSX command stream JUMPs into main-memory command
  segments at 0x4443xxxx-0x445xxxxx that the SPUs write (SPU-built RSX command lists -
  presumably the per-tile light draws of `spu_render_submit_lights_view_job`) and CALLs
  subroutines at 0x40Axxxxx-0x40Dxxxxx. The SPUs finish each segment well before the RSX jumps
  into it (only 16-byte link patches just before a target come later); the RSX never spins on a
  jump-to-self and no FIFO errors/recoveries are logged. Fragment programs are reloaded on every
  SET_SHADER_PROGRAM, so SPU-patched programs would be picked up. TEST222 tries Disable FIFO
  Reordering (untested so far).
- TEST222/223 (Disable FIFO Reordering): the first run hung the RSX before the mission; the
  retry reached it with the same blocks after movement. Reverted.
- Render graph of one gameplay frame (TEST224, `scripts/d1trace_timeline.py --graph`), internal
  resolution 1024x624:
  1. G-buffer: rtt0 0xCE8E0000 + rtt1 0xCE540000 (A8R8G8B8), zeta 0xCF400000.
  2. Copies to main memory (depth -> 0x445A3A80, normals -> 0x44813A80, stencil byte ->
     0x444EFA80). The depth copy holds valid linear floats (checked live).
  3. Small passes: 256x156 / 128x78 RGBA16F, zeta 0xCF330000 256x156 sampling the depth copy
     0x445A3A80; 32x32 rtt0-2 at 0xCF8B2800/4800/6800 (luminance/exposure chain).
  4. Shadow depth 0xCF680000 (1024x1024).
  5. Light accumulation 0xCEB60000 (1024x624 RGBA16F, pitch 8192) sampling normals 0xCE540000,
     depth 0xCF400000, the shadow map, 1x1 0xCF8BBF00 (exposure), 0xCF2E0000 256x156, the depth
     copy and 0xCE8E0000.
  6. Bloom/downsample chain (512x312 MRT at 0xCE702000/0xCEA18000/0xCE540000) and composite.
  Magenta/black blocks plus a whole-screen exposure change fit NaN/Inf values in part of the
  HDR light buffer feeding the 1x1 exposure average. Next: a one-frame dump of every render
  target (instrumented build, marker `<DEV_ROOT>/d1dump.on`) during movement, to find the first
  pass whose output has the blocks.
- **Blocks traced to surface aliasing (TEST225).** The build can dump every render/depth
  target of one frame when the RSX switches away from it (marker `<DEV_ROOT>/d1dump.on`, output
  `<PS3_ROOT>\d1dump\<id>\`, analysis `scripts/d1dump_scan.py`, driver
  `scripts/Invoke-MoveDump.ps1`). In a broken frame, G-buffer, depth, stencil and the small
  passes are clean. The first light pass into 0xCEB60000 (1024x624 RGBA16F, pitch 8192) leaves
  exactly x 0-639, y 312-539 at zero, and the top rows 0-126 dark. Those byte ranges are the
  HUD scratch targets the game renders at the end of every frame into the same memory:
  0xCEB60000 as 1280x127 ARGB8 and 0xCEDD0000 (= row 312 of the light buffer) as 1280x228
  ARGB8, both pitch 8192 (5120 bytes = 640 RGBA16F pixels per row). HUD elements (radar,
  toasts) appear on movement and fade when still, which explains the movement dependence. The
  final image takes those rectangles from the stale HUD data. The first dump (TEST225, dump 1)
  happened to be clean. Next (TEST226): trace draws, clears, inheritance (`old_contents`
  transfers) and background erases for this range to see why the light pass does not
  overwrite the inherited HUD data.
- TEST226/227 (draw, clear, inherit, blit traces for 0xCEB60000-0xCF040000):
  - End of frame N: the HUD target 0xCEDD0000 (1280x228) is bound and cleared, then two blits
    copy the top 127 rows of the frame (0xCE825400/0xCE826400, pitch 5120) into
    0xCEB60000/0xCEB61000 as ARGB8 at pitch 8192, and 0xCEB60000 is bound as a 1280x127 ARGB8
    HUD target. RPCS3 sends the blits into the FP16 light-buffer surface (prepare_transfer_target,
    inheriting 0xCEDD0000 first), then splits the light buffer into pieces 0xCEB61400 (384x624)
    and 0xCEC5E000 (640x497) when the 1280x127 target is bound.
  - Frame N+1: the first light-buffer draw is the sun (additive ONE/ONE; samples normals, depth,
    shadow map, exposure); the next pass samples the light buffer itself. RPCS3 rebuilds the
    surface from 0xCEDD0000, the two pieces and the 1280x127 target. Only 0xCEB61400 fails its
    memory check and is reloaded from guest memory; the rest keeps cached data (zeros from the
    HUD clear in the block, stale lighting elsewhere).
  - No RSX pass writes this range between the HUD and the first light draw, yet guest memory
    under the block holds real lighting values (mean 0.072 vs zeros in RPCS3's surface). The
    light buffer is filled outside the RSX - by the SPU lighting jobs, which read the G-buffer
    copies. The clean dump frame (TEST225) was clean because the forced write-backs made RPCS3
    reload the surface from guest memory.
  - Root cause: RPCS3's surface memory check (`render_target_descriptor::test`) samples only
    3 u64 values per surface (top-left, centre, bottom-right). Dark interiors are mostly zero
    before and after the SPU writes, so the change goes unnoticed and stale surface data wins.
    Movement triggers it because the HUD elements drive the surface splitting. TEST228 adds a
    6x5 grid of samples.
- TEST228: 33 memory samples per surface (3 + a 6x5 grid) fixed most frames but not all
  (blocks back for three frames in the second movement pass; a thin noise stripe at y ~185).
- **Fix (TEST229).** `rsx::g_local_mem_write_gen` (new `Emu/RSX/Common/local_mem_writes.h`) holds
  a write generation per 4KB page of RSX local memory (0xC0000000-0xCFFFFFFF).
  `spu_thread::do_dma_transfer` (via a scope guard, after the write) and the inline put path of
  `do_list_transfer` bump it for puts into video memory. `render_target_descriptor::sync_tag`
  snapshots the sum over the surface's pages, and `test()` fails when it changed. The existing
  memory-barrier logic then reloads the surface (or skips a stale inheritance source) from guest
  memory. Two full movement passes: 14/14 frames clean, with no blocks, overexposure or noise
  stripe, and lighting correct in dark interiors. `patches/rpcs3-destiny-fix.patch` holds the
  fix alone (plus the CPU path for VRAM -> main memory blits, TEST216).
  `patches/rpcs3-destiny-main-memory-blit.patch` holds the fix plus all diagnostics (D1T/D1DIAG
  tracing, one-frame dumps). The deployed `rpcs3.exe` is the diagnostic build, which is idle
  unless the marker files exist. Needed game config: Write/Read Color Buffers,
  Write/Read Depth Buffer, Handle RSX Memory Tiling.
- Open: whether the blit CPU path is still needed with the write tracking (it is harmless);
  PPU writes to unprotected video memory are not tracked; an upstream RPCS3 report/PR needs
  the user's go-ahead.

## Update (TEST230-235, 2026-09-27): the "Urgent message" dialog
- Text: "Destiny servers will go offline temporarily in the next few minutes. Pardon our dust.
  For more information, visit help.bungie.net" - a scheduled-maintenance warning, shown on
  entering the mission world (with the "BETA BUILD" watermark). Its UTF-16 text exists only in a
  UI string object (0x43a4d7c0 in TEST230); the localized source is not stored as plain text.
- BAP codecs: `0x41501c(type)` returns the codec from a runtime table at 0x175fba8 (filled at
  boot); vtable slot 3 decodes. Type 0x14 "shutdown not" = vtable 0x12b9650, decoder 0x416370
  (9 bytes: u8 + u64). The stand-ins never send it. Type 0x13 (client config rsp) decodes via
  0x415ec0 into {u64, u32}; field 1 = 0 does not remove the dialog (TEST231).
- Ruled out: client config values (231), services 701/702 (result-only replies), the
  heartbeat's empty type-11 host message (233), the account-message list below (not resolved in
  TEST235, so not the source).
- Account-message list: `0x34b8e8` (called from 0x2ff874) takes the self (family 4) investment
  object, checks its +1 byte, and `0x40acc0` walks a content list (singleton via `0x288a70`,
  cached at *(0x18d9528) + 4; entries 12 bytes {u32 op count, u32 ops offset, u32 hash} from
  +0x24) collecting the hashes whose unlock expression is true for the account. The cache was 0
  in TEST235 while the dialog was up, so that list never ran.
- The flag-table owner (`0xb4d730` returns it; 36 callers) has a byte at +1, tri-state flags
  from +2 (0x1000), and bitsets at +0x1004/+0x1804.
- The title configuration (SignOn field 4, descriptor 0x11c7677) has 58 submessages; only
  field 1 (content manifest) is sent, the rest are compiled defaults. Decoding
  the small submessages shows timeouts and bools, no named maintenance field yet.
- **GDB breakpoints do not work** with this RPCS3 setup: `ppu_breakpoint` refuses when the PPU
  decoder is LLVM (both the global and the per-game config use LLVM); the stub still answers
  "OK". All trace logs since TEST202 have zero hits for that reason. The interpreter would be far
  too slow for the game. Next options: add a PPU-side hook to the patched build (e.g. log calls
  to a given address from the LLVM function-call path), or continue statically.
- New tooling: `bap_server.py --client-config V1,V2`, `--ah-heartbeat SECONDS:-1` (no empty
  host message), `Invoke-MissionRun.ps1 -TraceAfter ''`.

## PPU call tracing without GDB (TEST236-239, 2026-09-27)
- The instrumented build now traces PPU function entries under the LLVM decoder. Each compiled
  function below 0x2000000 checks a byte table at the fixed host address 0x6d0000000000
  (`d1diag::g_ppu_mask`, one byte per 4-byte PPU address) and calls `ppu_d1hit` when it is set.
  The watch list is `<DEV_ROOT>/d1ppu.txt`, re-read about once a second from the RSX flip. One watch
  per line: `ADDR [MAXHITS] [rN:LEN ...]` (hex address, default 50 hits, memory dumps at
  registers). Each hit logs `D1P <us> pc lr r1 r3-r10 chain <saved return addresses> mN=<hex>`.
  A line `coverage` arms every function for a single logged first call (`D1C <us> pc lr`), and
  every reload logs `D1: D1P watch list reloaded at <us>`. The PPU object cache key is
  `v8-kusa-d1p2`, so the first boot after a translator change recompiles everything
  (several minutes). RPCS3 creates "functions" at many block boundaries, so first-call pcs are
  often block entries.
- `scripts/rpcs3_text_marker.py TEXT` polls guest memory for a UTF-16 text and appends a marker
  line to the watch file, which lines coverage up with the moment a UI text appears.
- Urgent-message findings: the universe-gated message list (`0x34b8e8` -> `0x40acc0`, content
  entries at 0x800dc540, hashes 0x5b06d63a ... 0x2207b0a3, flags 1858-1870) collects nothing.
  The dialog's text object sits at 0x43a4d7c0 in two runs (deterministic). The functions first
  called just before it appears cluster at 0x400708-0x401140 (state machine `0x401048` on the
  object at 0x18d9500, called from `0x286da4`) and at 0x551fac-0x553280 (UI; `0x5531bc(500)`
  from `0x286cec`). None of the 8680 candidates received the text object, so it is written by
  helpers first used earlier. Next time: watch 0x401048/0x40130c/0x5531bc and the UI cluster
  with dumps, or trace writes to 0x43a4d7c0 directly.

## Long-session memory growth in RPCS3 (TEST240-249, 2026-09-27)
- Symptom: in the mission, private commit grew ~14-15 GB/min and RPCS3 vanished 7-12 minutes
  in (commit limit 63 GB). Same on the official build (TEST241); the stock build's earlier RSX
  fatal (`simple_array<VkDescriptorImageInfo>::reserve`) was this too.
- Tools: `D1MEM` log line every 2 s (VKPresent.cpp: RPCS3's Vulkan pools, texture/surface
  cache, program and descriptor-set counters); a VirtualQueryEx diff of committed private
  regions (new 16 MB NT-heap segments, 32 MiB arenas).
- Cause 1: `vk::descriptor_set::init()` reserved 16448-entry image/buffer info pools
  (~0.9 MB) on first use of every descriptor set; Destiny has ~255k programs (pipelines) and
  cycles through them, initializing ~270 new sets per second. Fix: storage pools of 256 + 64
  entries, flushed at bind once 256 are used (same pointer-stability rule as before).
  ~14.5 -> ~1.5 GB/min (TEST246).
- Cause 2: every program's descriptor table keeps its own VkDescriptorPool (driver memory,
  ~90 KB each incl. storage) forever. Starting pools at 2 sets did not help much (TEST247: RSX
  stopped 20 min in with system RAM full). Fix: `vk::glsl::release_idle_descriptor_tables()`
  runs once per presented frame; every 64 frames it releases the set storage and descriptor pool
  of tables not committed for 256 frames (~8 s, far beyond GPU latency). The next `commit()`
  allocates a new set and rewrites all descriptors from the template. TEST249: private commit
  flat (12.9 GB) with ~35k tables released in the first minutes.
- The first sessions also compiled ~2700 new fragment programs per minute (TEST247) until the
  pipeline cache (persisted on disk) covered the mission; TEST248 added none. So the program
  churn was a cold cache, not a hashing problem.
- Both memory fixes are in `patches/rpcs3-destiny-fix.patch` and in the unfiled upstream draft
  (notes/rpcs3-upstream-draft.md).

## Session end (2026-09-27 ~09:05)
- The user's XInput pad config is restored in the private RPCS3 copy
  (`config/input_configs/global/Default.yml` = `Default.xinput.user.yml.bak`). The unattended
  scripts need the keyboard pad: `scripts/Set-PadConfig.ps1 -Mode keyboard` before a run,
  `-Mode xinput` afterwards.
- Deployed `rpcs3.exe` = the diagnostic build (rendering fix, memory fixes, D1T/D1DIAG/D1MEM
  logging, PPU call hook, `<DEV_ROOT>/d1noblitfix.on` switch); `rpcs3.orig.exe` = official.
  `<DEV_ROOT>/d1ppu.txt` holds no watches. RPCS3 and the loopback stand-ins are stopped.

## World-state group peer (2026-09-27, after TEST253)

- The next milestone is to drive the 99 bubble-13 object baselines and mission objectives.
  TEST253 already showed a movable Guardian in Dock 13, with no objective, waypoint or
  enemies. The world-state group record `0x80800c6f` is **not** obviously assigning
  simulation to a missing peer: its nested `0x80800e4a` at +8 has state at +1 and a
  5-bit peer at +2. The stand-in sets both state 1 and peer 0.
- `0x31d06c` writes the world controller's local slot at slice +0x230 from `0x25bc10`.
  That calls `0x1c9ab8` with the world-state identity table. `0x1c9ab8` scans the
  +0x502c occupied mask and compares each slot's 8-byte identity to the local machine
  identity. The stand-in occupies slot 0, so this returns 0 in a valid mission session.
  `0x31ca20`/`0x31cb80` compare the group record's peer byte at slice +0x31a with
  local slot +0x230 before following their local-group path. `0x31d06c` also uses
  this equality while processing changed group records. This supports peer 0 as the
  correct value for the current single-client world; it does **not** prove which
  component is responsible for spawning NPCs/objectives.
- New diagnostic: `scripts/rpcs3_world_peer.py` reads the live slice (zone at +0x310,
  local slot +0x230, group state +0x319, group peer +0x31a, per-peer byte +0x348).
  At setup, it returned zone -1 and local slot -1; it still needs an in-mission read.
  `bap_server.py --ah-world-state ZONE[:PEERBYTE[:GROUPPEER]]` can override the group
  peer for controlled comparisons, defaulting to 0. A real-schema encode/decode
  roundtrip confirmed `104:1:1` puts 1 in the nested peer field and both world-state
  masks remain 1.
- TEST254_PEER1 was interrupted before any mission comparison. A first-run RPCS3
  welcome prompt and NVIDIA Overlay crash dialog delayed startup; the screenshot
  helper timed out after sign-in. RPCS3 remained at Set Screen Bounds, and short UI
  simulated X presses did not advance the keyboard pad. The BAP connection stayed
  healthy (5 s echo responses); no activity-host join occurred. Do not use TEST254
  as evidence for owner-peer behavior. On a future run, confirm the dialogs are gone
  and the keyboard pad can advance setup, then compare the live `+0x230`/`+0x31a`
  values in the mission. This run has been stopped, its local stand-ins closed, and
  `Set-PadConfig.ps1 -Mode xinput` restored the user's controller mapping.
- `scripts/rpcs3_replicated_objects.py` now accepts `--json-out PATH` to export each
  object's group, kind, index, bubble, state A/B type handles and pending-baseline
  flag, plus `--schema-out PATH` to export the live type trees for those state handles.
  The existing stdout baseline-list format is unchanged. These options have been
  syntax-checked but require the next mission run for live verification. Suggested
  next capture: run the proven TEST253 configuration, then from a separate shell
  `python scripts/rpcs3_replicated_objects.py --json-out <path> --schema-out <path>`
  while in Dock 13. Group the 99 bubble-13 records by state type before constructing
  targeted replicated updates. No enemy or objective state has been synthesized yet.

## Replicated object state types in Dock 13 (TEST255)
| Kind | Group(s) | Count | State A (size, fields) | State B |
|------|----------|-------|------------------------|---------|
| 0x01 | 0x062dd95d (bubble 13) | 8 | 0x8080071e (96, 13) | 0x80800357 |
| 0x03 | 0x062dd95d | 1 | 0x808004fa (28, 2) | 0x80800830 |
| 0x04 | 0x062dd95d | 5 | 0x808007d9 (224, 4) | 0x80800387 |
| 0x05 | 0x3c2f5ed5 / 0x992bb471 | 46 / 16 | 0x808029a1 (44, 4: i32, u8, 28-byte struct, object id) | - |
| 0x06 | 0x992bb471 | 1 | 0x8080254d (212, 9) | - |
| 0x0c | 0x4786c0e0 | 16 | 0x808022a5 sim players | 0x80802c30 |
| 0x0f | 0x4786c0e0 | 1 | 0x808008d0 (13424, 6; a 12816-byte sub-struct) | - |
| 0x10 | 0x4786c0e0 | 1 | 0x808008a3 game mode | - |
| 0x11 | 0x4786c0e0 | 1 | 0x80800725 (36, 3) | - |
| 0x1d | all three bubble groups | 14 | 0x80800463 (8, 2) | - |
| 0x20 | 0x992bb471 | 8 | 0x80802a1b (720, 13: two object ids, ten 64-byte 0x8080293c records) | - |
| 0x21 | 0x062dd95d | 1 | 0x80800750 (400, 1) | - |
| 0x22 | 0x4786c0e0 | 1 | 0x808004a4 (32, 4) | - |
| 0x27 | 0x4786c0e0 | 1 | 0x80802aef (712, 2) | 0x80802b18 |

- Working hypothesis (unverified): kind 0x20 = squad/encounter state (up to ten member records),
  kind 0x05 = its members (62 objects for 8 squads), kind 0x0f = mission-wide script/objective
  state. The type handles are not immediates in the EBOOT (runtime type registry), so the
  consuming component handlers have to be found at run time: hook the record apply path
  (`0x1cfa1c` -> `0x1cf8f4` -> `0x25f6b8`) with the PPU hook and follow the per-kind handler
  calls, as was done for the sim-player binder (`0x4ede14` -> `0x14a270`).

## Replicated-state handlers per kind (TEST256-257)
The record apply path is `0x1cf8f4` -> `0x25f6b8(binding, state type, state)` -> `0x276154`,
which calls the component's apply handler through a member-function pointer (return address
0x276230). The handlers are found as the first-call coverage entries with that LR; each gets a
pointer to {state type, state data} in r4.

| Kind | State type | Handler | Neighbourhood |
|------|------------|---------|---------------|
| 0x01 | 0x8080071e | 0x1f8d20 | bubble objects |
| 0x03 | 0x808004fa | 0x1f6aa4 | bubble objects |
| 0x04 | 0x808007d9 | 0x2000e0 | bubble objects |
| 0x05 | 0x808029a1 | 0xb94414 | 0xb9xxxx |
| 0x06 | 0x8080254d | 0x4ed278 | player / sim-player code |
| 0x0c | 0x808022a5 | 0x4edd00 | sim player (binder 0x4ede14 next to it) |
| 0x0f | 0x808008d0 | 0xb2c778 | game-mode code |
| 0x10 | 0x808008a3 | 0xb2c8d4 | game mode |
| 0x11 | 0x80800725 | 0xb2d1e0 | game-mode code |
| 0x1d | 0x80800463 | 0xb2f99c | game-mode code |
| 0x20 | 0x80802a1b | 0x3561d8 | player spawner (0x35xxxx) |
| 0x21 | 0x80800750 | 0x200e38 | bubble objects |
| 0x22 | 0x808004a4 | 0xb26bac | game-mode code |
| 0x27 | 0x80802aef | 0x355dc0 | player spawner |

- Kind 0x0f's handler copies the whole state (0x3470 bytes) to entity +0x110 and sets a dirty
  byte at +0x3584. The state is four bounded lists: 16 x 800-byte entries keyed by a raw 64-bit
  id (player/machine: kind 0x23 field) - a per-player activity table, not objectives - plus
  8 x 56, 24 floats and 2 x {u32, raw64 id, u6}.
- Kind 0x20/0x27 are handled in the player spawner, so they are spawn-related (spawn groups),
  not enemy squads as first guessed.
- **Game mode +0x10 (0x8080533d) is a variable table**: u6 count + up to 50 entries of
  {u32 hash, 32-byte typed value (field kind 34)}. That fits server-driven mission sequencing
  (the activity host sets named variables; client scripts and UI react). The stand-in sends none.
  Next: find which variable hashes client code looks up (hook the table lookup) and try setting
  them via `--ah-object` / the game-mode state.
- New tooling: `bap_server.py --ah-object GROUP:KIND:INDEX:TYPE[:B]=JSON` replicates any object
  with an explicit state (fields by offset). The private activity schema now includes the 65
  live types exported in TEST255 (backup `activity_schema.before-test257.json`).

## Expected mission flow (A Guardian Rises) and where we spawn
Source: Destinypedia's "A Guardian Rises" walkthrough and transcript. Objectives in order:
Enter The Wall -> Find a weapon -> Grab the Rifle -> Fight through The Wall -> Fight through the
Fallen -> Find the ship -> Clear out the Fallen.
- Start: two cutscenes, then the Guardian wakes among cars outside the Wall; the first nav point
  marks a break in the wall. Inside, the Ghost lights a dark chasm, Shanks chase it, and a gate
  opens to the Khvostov 7G-02. Next come ambushes by Dregs and Vandals, a loot cache, a trip-mine
  corridor and a hall with 4 Vandals and 5 Dregs, then the Divide, where Fallen dropships arrive.
- **Dock 13 is the final area**: the hangar with the Arcadia jumpship and Rahn, Devil Captain
  (with Dregs, Vandals and Shanks). Clearing it triggers the end cutscene.
- The stand-in has been spawning the Guardian in bubble 13 (zone set 104) - the end of the
  mission - because that was the first bubble configuration that worked. TEST259 tries bubble 0
  (zone set 0) to reach the mission start.
- Script variables: the game-mode object's variable table is pushed into a global script-variable
  store (`0xb2c8d4` loop -> `0x1fed20` -> `0xd08b68`); client code reads it through getters
  (`0x1fed60` 47 callers, `0x1fee1c` 44, `0x1fedc4` 10, `0x1fed8c` 6).

## Mission script objects (TEST262)
Named script objects of act_cosmo_m0_highway, read from guest memory (strings at ~0x828dd400 in
that run), grouped by phase. Each objective has `_player_objective`, `_incoming_sequence`,
`_completion_sequence`, and mostly `_nav_point`, `_trigger_volume`, `_player_trigger`.
- `cosmo_m0_phase00_highway_intro`: `pf_104_res` and `pf_104_res_fp` (`_cinematic`: the Ghost
  resurrection cutscene, third and first person), `skeleton_static_1`.
- `cosmo_m0_phase00_highway`: `obj_m0_phase00_m1` (Enter the Wall), `pt_ab`/`pt_ab_1`,
  `tv_ab`/`tv_ab_1`, `tv_in_wall`, `pm_in_wall`, `breach`.
- `cosmo_m0_phase01_breach`: `obj_investigate_wall`, `obj_get_weapon`, `o_weapon`,
  `pf_104a_pwr` (cinematic + `_trigger[0]`), `sv_m0_gating`/`tg_m0_gating`,
  `tv_obj_investigate_wall`, `graveyard`, `globals`.
- `cosmo_m0_phase01b_breach`: `obj_goto_graveyard`, `graveyard`, `globals`.
- `cosmo_m0_phase02_graveyard`: `obj_m0_m04` (`_object`), `pt_obj_backup_adv`/
  `tv_obj_backup_adv`, `globals`.
- `cosmo_m0_phase03_hangar`: `obj_m0_goto_hangar`, `obj_m0_m04a`, `obj_m0_m04b`,
  `ap_obj_m0_m04b`, `tv_obj_m0_m04b`, `hangar`.
- `cosmo_m0_phase04_outro`: `pf_105_cto` (end cinematic).
Entity group keys are FNV-1 hashes of names: 0x992bb471 = "act_cosmo_m0_highway" (the other
groups did not match any string in guest memory).

## Slice sets and cinematics (static analysis, 2026-09-27)
From the decrypted EBOOT (`local\RESEARCH\ps3_decrypt\EBOOT.elf`; code addresses = file offset +
0x10000, TOC base 0x175a5e8; SNC forms addresses with `lis` + `addic`, not `addi`).
- The "zone sets" of the object table (bubble*8 + sub-slot) are the world controller's **slice
  sets** ("world controller slice set manager" `0x30dbf4` region, strings
  `activity:initial_slice_set_loading`, `transitioning:initial_slice_set`,
  `switching_orbit_slice_set`, activity-host debug `/ah_slice_set_names:`; the client prints
  `activity-slice-set-index` / `map-slice-set-index`).
- Spawn readiness `0x35793c` refuses, in order, per the message table at 0x11eda20..: FAH / GAH
  not instantiated, no slice set loaded, **in a cinematic slice set**, gear still loading, no
  simulation player. The cinematic test (`0x357a34`-`0x357b1c`): two slice-set handles from the
  interface `*(0x175cb1c)` vtable slots 0x170 / 0x178 (`0x1230bc`, `0x123138`), each resolved
  through the tag table; byte +0x10 of either definition == 0 -> no spawn. So +0x10 marks a
  gameplay slice set; a cinematic slice set clears it.
- World-controller cinematics: "world controller cinematics manager" referenced at `0x30dbf4`,
  "... cinematics nondeterministic globals" at `0x30c4d8`, "cinematics globals" at `0x3cc2c8`.
- Hypothesis (to test on the second PC, TEST345/350): the sub-slot entries registered at sim
  start even with every bubble state -1 - 105 (bubble 13) and 113 (bubble 14) - are the
  mission's cinematic slice sets: 105 = outro `pf_105_cto` in Dock 13, 113 = intro
  `pf_104_res` (Ghost revival), so **bubble 14 (base 112) = the highway start**. The real host
  would start the client in 113 and switch to 112 after the cinematic
  (`--ah-zone-schedule 112@90` mirrors that with a second world-state delta).
- **Objective sensor = replicated kind 3** (one per mission bubble: Dock 13 and The Breach have one
  each). The string "objective sensor client update state" is referenced at `0x1f6e98`, inside the
  kind-3 apply handler `0x1f6aa4`. State A `0x808004fa` (28 bytes) = `0x808040b9`: **24 u7 values,
  one per objective of the bubble**, plus a u31 at +24. State B `0x80800830` (632 bytes) = 24 x
  `0x8080086c` {u7, bool, `0x8080411b`} plus u31 and u32. The handler walks the definition's
  entries (count +0x4c, offset +0x50) and, when a received byte differs from the copy at object
  +0x124+i, stores it and runs the entry's follow-up. That fits the mission script's per-objective
  phases (`_incoming_sequence`, `_player_objective`, `_completion_sequence`). The stand-in's
  default baselines leave all 24 at 0 - no objective ever starts, hence no "ENTER THE WALL". Test:
  `--ah-object <bubble group>:3:<index>:0x808004fa={"0": {"0": N}}` for N = 1..4 in the start
  bubble once it is confirmed.
- Kind-3 apply detail (`0x1f6b64`-`0x1f6ca8`): per definition entry i (count +0x4c, 0x1c-byte
  entries at +0x50, each with a sub-count at +0x70) the received u7 is stored at object
  +0x124 + i*0x1a (a 26-byte record: state byte + per-entry flags), then `0xf3544` resolves the
  entry's definition and `0x89b37c(object + [+0x118] + i*0x31 + 0x128, 0, ...)` moves that
  entry's **script state machine** to the new state; definition flags at +0x38 (bits 0x78, 0x40)
  decide whether `0x10977c` activates it. So the 24 values are state-machine indices of
  data-defined sequences (objective incoming / player objective / completion), not fixed
  enums: the start bubble's objective 0 has to be stepped 1, 2, 3 empirically.
- World controller layout (constructor at `0x30db9c`-): +0x18 slice-set manager (3 slots of
  0x6350 bytes), +0x1c cinematics manager (0xac bytes, "world controller cinematics manager"),
  +0x24 a 0x90-byte subsystem (`0x4b53b4`).
- **Bubble map of act_cosmo_m0_highway (TEST260-342)**: 0 Skywatch (night), 1 no name (sky), 2 The
  Blast, 3 The Breach, 4 crashes RSX on load (fixed build untested there), 5 unnamed corridor, 6
  Mothyards, 13 Dock 13 (mission end, extra slice set 105), **14 highway start outside the Wall**
  (extra slice set 113), 7 The Steppes, 8 Lunar Complex, 9 Terrestrial Complex, 10 Jovian Complex, 11 Forgotten Shore, 12 fails to load (3/3), 15/16/18 unnamed, 17 Seraphim Vault, 19 Refinery, 20 Rocketyard, 21 The Devils' Lair. Mission start = zone
  set 112 (`--ah-bubble-states -1,14=0 --ah-zone 112:0x811c9dc5 --ah-world-state 112:1`).

## Intro cinematic and slice-set transitions (TEST350-366)
- Slot 113 (bubble 14's second slice set) holds only activity-group kind 4 index 0 (bubble 14)
  and kind 6 index 1 and 2: a spawn slot and two player-id lists (EBOOT string
  "cinematic_with_roster"). Kind 4 (`0x2000e0` -> `0x200014`) is a **spawn slot with a
  generation counter**: a higher u32 destroys the tracked entity (`0x1323e0`) so it is
  recreated. Setting the roster and stepping kind 4 (TEST360-363) left the screen black.
- Coverage diff 113 vs 112 (TEST365 vs TEST364, `scripts/d1coverage_diff.py`): 113 reaches
  128 functions 112 does not, 112 reaches 24,513 more (gameplay). In 113 the spawner refuses at
  `0x35799c` (active slice set not a bubble base), and the world-controller state `0x329660`
  takes its non-base branch (`0x3296a4`: countdown cleared) and posts event 0x23 via `0x3204a8`
  (`0x3296fc`), as 112 does after its 5 s countdown. Nothing there waits on our objects.
- Video path (not the Ghost intro): `0x3298xx` reads a u16 "video activity index" from
  `0x3174c8(0)+0x7f8` ("invalid video activity index!" / "failed to find activity video!"),
  starts the CRI/Bink player (`0xb7ba88`, `0xb7bcc8`; "bink\%s_%d.bik") and sets world
  controller state 4; world-controller activity states are `activity:initial_slice_set_loading`,
  `activity:physics_join`, `activity:in_world`, `activity:watch_video`; the client reports
  `finished-watching-video`. Transition kinds: `transitioning:initial_slice_set`,
  `normal_z_leg`, `debug_initiated`, `teleportation`. Realtime cinematics live in
  `ps3_cinematic_trv_*` / `ps3_cinematic_queen_*` packages.
- A second world-state delta naming another zone set is sent but not acted on (TEST350), so the
  host must command slice-set transitions some other way. TEST366 traces the type-11 handler
  `0x25715c`, the slice-set loader `0x13f8ec`, `0x31f6bc` and the reason/state poster
  `0x3204a8` across a 112 -> 24 (The Breach) switch.

## Static analysis of the uploaded EBOOT: how a slice-set switch starts (2026-09-27 evening)
From the decrypted EBOOT in the private `d1xenia-data` repo (`scripts/d1_eboot.py`), the TEST364-366
logs there and the exported schemas. Nothing here has run yet; the hooks in
`next-session-playbook.md` step 2 check it.
- **Engine requests.** `0x302100` registers 18 request slots through `0x11f80c` (table from
  `0x11f690()` = `*(0x18a4328)`: handler descriptors at +0x80 + 4*slot, describers at +0xc8,
  flags at +0x114, a 0x1804-byte parameter block per slot from +0x15c). Describers name them:
  slot 1 = "De-instantiate CURRENT slice-set." (`0x303330`), slot 2 = "Change slice-set
  (slice-set='%d', spawn='%s')." (`0x30335c`; handler `0x30256c`), slot 3 = "Change world
  (...)" (`0x3033a0`; handler `0x302728`). `0x11f91c(table, slot, params)` posts one (flag 0 ->
  1, parameters copied, flag 2); `0x11fa88` tests a slot for "posted".
- **Only two places post slot 2**: the world load machine `0x4b2d00` (`0x4b3330`, at load) and
  the world-controller zone machine `0x31eaf8` (`0x31ebf4`, in the world). The zone machine reads
  the zone state object (`0x30e128()`): state byte +0, target slice-set index +0xa8 and hash
  +0xac; when the target exists in the scenario object table (`0x140820` status 2/3) and slot 2
  is free, it posts {0xc, index, hash}. The zone state is filled from the world controller's
  slice record by event 0x4a / RPC entry 11 (see "Where the zone record comes from" above).
- **Who sets the slice record's target** (`0x31d358(slice, state, index, &hash)`): the
  world-controller update `0x31e658`, which runs only while `0x66c2d4()` == 0 (this machine is
  the authority over the slice set; string "authority-over-current-slice-set") and slice +0x81
  == 0, +0x88 != 0, state +0x89 < 6, +0x80 == 0. It calls, in order:
  - `0x31e300`: a **pending request** at slice +0x220 (i32 index), +0x224 (hash), +0x228 (u8):
    when the index is not -1, the target becomes {state 5, index, hash} and the request is
    cleared (+0x220 = -1, +0x224 = 0x811c9dc5).
  - `0x31dab0`: **follows the local player**. It resolves the local player's entity
    (`0x68324c(0)` -> `0x2d6544`), walks a list of volumes testing positions, takes the bubble the
    player is in, converts it with `0x14a8dc` (8 * bubble) and, when that base slice set is present
    in the scenario object table (`0x140820` status not 5), writes target {state 3, index}.
  - `0x31d778`: the arrival target (state 2) described earlier.
  So a mid-mission bubble change starts on the client, from the player's position (a z-leg), as
  in Bungie's description of bubble transitions (`public-research.md`). A host world-state delta
  naming another zone set is not part of this path, which fits TEST350 and TEST366.
- The pending request (+0x220) is written only by `0x31bcd0(index, &hash, u8)` (when
  `0x66c2d4()` == 0 and slice +0x88 != 0). Its one caller is `0x35bfec`, a small request state
  machine in the player spawner, reached through the spawner component's vtable (`0x12bbc18`,
  installed by `0x35be78`; slot 29 = `0x4e9470` -> `0x35bfec`). Every tick `0x1c922c(sim)` (from
  `0x1c5f08`, which walks the host sessions' simulations) calls vtable slot 28 with
  (sim, sim+0x5078, sim+0x18) and slot 29 with (sim, sim+0x5080, sim+0x20). Slot 29 compares the
  record at sim+0x5080 {u8 state @0, u8 @4, i32 index @8, u32 hash @0xc} with its copy at sim+0x20
  and, on a new request, raises the pending slice-set request: a **host-driven teleport /
  slice-set change** path (EBOOT transition kinds include `teleportation`). What fills sim+0x5080
  is still open: it lies between the world-state copy (sim+8, 0x5038 bytes) and the activity
  state (sim+0x50b0), and no host message handler touches it directly (checked two call levels
  deep for all 17 types).
- **Host message dispatch.** `0x258778` looks the subtype up in 17 {type, handler descriptor}
  pairs at `0x175cf5c`: 0 `0xd0d020`, 1 `0xd0d250`, 2 `0xd0d3f0`, 4 `0xd0d560`, 5 `0x25b920`, 7
  `0x257028`, 10 `0xd0d6a8`, 11 `0x2572e4` (a thunk to `0x25715c`), 18 `0xd0d6e8`, 23 `0xd0d88c`,
  24 `0xd0da9c`, 27 `0xd0dca4`, 29 `0xd0dde8`, 38 `0x25b9a8`, 39 `0xd0df2c`, 42 `0xd0e08c`, 43
  `0xd0e214`.
- **TEST366 caveat.** Its hook log (in the data repo) has exactly 10 hits each for `0x25715c` and
  `0x31f6bc`; six of the `0x25715c` hits fall within 1.6 ms at emu 5:15.95 (queued type-11
  messages handled in one burst). Two watches ending at exactly 10 hits looks like the watch's hit
  budget running out, so "type 11 no longer reaches the handler in the world" is not
  established. Re-check with a large budget (`25715c 5000 ...`).
- **Correction:** the string "objective sensor client update state" is referenced at
  `0x1f6e98` in `0x1f6e70` (called from `0x35b844`), the function after the kind-3 handler;
  `0x1f6aa4` ends at the tail branch `0x1f6e6c`.
- **Client -> host senders** (each sends through the host messaging object, host +0xba8, virtual
  +0x18, message number in r5; most encode a struct with a per-message descriptor global at
  0x179eb80-0x179ec10 through `0x25bd0c`): 3 join (`0x259c84`), 6 raw (`0x258678`, reached only
  through the interface table at 0x12b70b0), 8 (`0x259300`), 12 (`0x259904`), 13 (`0x259afc`), 14
  (`0x259da4`), 15 (`0x258138`, the ~30/s one), 17 (`0x259f10`), 18 (`0x25a21c`), 19 (`0x2587ec`),
  20 (`0x258934`), 21 (`0x257d28`), 22 (`0x257e18`), 25 (`0x25b274`), 26 (`0x25b46c`), 28
  (`0x258c08`), 30 (`0x258d2c`), 31 (`0x258f0c`), 32 (`0x25b63c`), 33 (`0x25a548`), 35
  (`0x25a094`), 36 (`0x25a73c`), 37 (`0x25a360`), 44 (`0x2590d8`), 45 (`0x2591ec`).
- **Type 21 = the client's own state, including its slice set.** `0x257c80` encodes one present
  bit + a delta (`0x1ca00c`) of the 0x220-byte struct 0x80800c56 at host +0x6498 against the last
  sent copy at +0x66b8. Its +0xb8 is 0x80800d9f, the world controller's slice-set record: current
  record @0, target record @0xa0 (0x80800d82: i32 slice-set index, u32 hash, i32), u3 state
  @0x142. TEST366 decoded: join -> state 5; 20:23:22 target 112 (state 4); 20:23:27 current 112,
  target -1, state 0. So the client reports every slice-set change to the host; the BAP stand-in
  now logs these as "client slice set: ..." (`client_slice_state`).
- **Type 6 = the client's upstream replicated object stream** (the "sense state" direction).
  `0x1ce098(sim, peer)` runs while the simulation's state (+0x604) is 3: `0x1cdb10` encodes the
  records the client authors into a 15 KB buffer (a flag bit from sim +0x613, the u8 epoch from
  +0x608, a 0 bit, then objects), the host session's messaging interface (vtable 0x12b70a0,
  slot +0x10 = `0x258678`) sends it as type 6, and each sent record's state becomes its new
  baseline (`0x25ffb8` / `0x25ff48`). In TEST366 all three type-6 messages carry one object,
  0x4786c0e0 / kind 0xc / index 5 = the local player's simulation player (object id at bit 11).
  The per-object header after the id does not match the host stream's u32 length, and neither
  state type decodes cleanly: hook `0x1cdb10` (or dump the buffer at `0x1ce134`) to pin it down.
  Trigger volumes and squads would report the same way once the client authors such records.
- Client -> host messages in TEST366 (`scripts/ah_log_summary.py`): 3 (join), 17 (8 bytes), 37
  (1087), 21 (164, 10, 16, 28 bytes: different bodies), 15 (1 byte, ~30/s), 20 (1536), 36 (4), 6
  (41 bytes, three times with a counter 1/2/3 at the end, from emu ~5:27) and 18 (54 bytes). After
  the zone-set switch at 20:25:46 the client sent nothing new. The client -> host type table
  (`*(0x179eba4)` = 0x18cffe0) is filled at run time, so the struct types need a run-time dump.

## PS3 packages read offline (2026-09-27 evening)
`scripts/d1_pkg.py` on the ten packages in the data repo: every header, table and block SHA-1
checks (791 blocks), so the big-endian v24 layout is right. Build string `live tool_lib test
pc_x64 41345.14.06.30.2046.live`, build time 2014-06-30/07-01.
- Compressed blocks (flag 0x100) start with 0xB7, the older Oodle framing of the PS4 D1 study:
  reading tag data needs a user-owned `oo2core_3_win64.dll`. Uncompressed blocks in the activity
  packages hold only Wwise streams (type 8:21), so no mission data is readable without it.
- **Charm's Rise of Iron class ids hold for this 2014 build**: the named tags are
  `map:cosmo:root` / `map:bootstrap:root` (class 0x8080052e = SActivity), `ambient_cosmo:
  scenario_client` / `ambient_bootstrap:scenario_client` (0x80800616, the activity info type),
  `ambient_cosmo` / `ambient_bootstrap` / `map:cosmo:unused` (0x808008ba).
- The shared manifests (packages 0x0430/0x0431) are stored uncompressed: 12,159 global-tag
  records {u32 size, u32 own tag, u32 child class, u32 child tag, dependency package-id list},
  14 classes. Among them 21 SActivity roots, 81 activity infos (0x80800616), 100 x 0x808008ba and
  136 bubble definitions (manifest class 0x80800580, as in the PS4 study); 8 bubble definitions
  sit in the Cosmodrome destination package 0x026d. Mapping the other package ids to names needs
  the disc's package file list.

## Cosmodrome map from the packages (Oodle available, 2026-09-27 night)
With the user-supplied `oo2core_3_win64.dll` (loaded on Linux by `scripts/oodle_pe_shim.c`), all 585
compressed blocks of the ten uploaded packages decode to exactly their used size.
`python scripts/d1_pkg.py bubbles 80cda002 <packages> --oodle <dll>` prints the result below.
- `map:cosmo:root` (0x80cda002, SActivity 0x8080052e) = {u32 size, u32 names tag 0x80cda003, u32
  tag 0x80cda004, array @0xc of **22** bubble definition tags}: the 22 bubbles the client checks
  for act_cosmo_m0_highway. Arrays on PS3: {u32 count, u32 offset from the count field} -> a
  0x14-byte header {0x80800184, count, element class, 0, 0} then the elements.
- The names tag (0x80800636) is an array of 82 location rows {u32, u32, u32 name hash, u8 kind +
  u16 row, i32 bubble index or -1, u32 flags}: 22 bubbles (kind 0), 26 kind-1 and 34 kind-2
  locations. **Names are FNV-1 hashes of short dev names**: bubble 0 `array` (Skywatch), 2
  `blast`, 3 `breach`, 8 `dome_a`, 9 `dome_c` (the Lunar / Terrestrial Complexes), 10
  `dome_hive`, 12 `graveyard`, 13 `hangar` (Dock 13), **14 `highway`** (confirms TEST342), 15
  `hive_shore`, 19 `refinery`, 20 `rocketyard`, 21 `ship`; a kind-2 row is `breach_sky`. Bubbles 6
  and 7 (Mothyards, Steppes) hash to names that differ only in the last character.
- Bubble definitions: 8 in package 0x26d (bubbles 0-7), the rest in 0x270 (8-12), 0x271 (13-16),
  0x272 (17-20), 0x274 (21). Each is {u32 size, array @4 of map-container tags}: bubble 0 has
  154, bubble 1 (sky) 6, 2 38, 3 9, 4 42, 5 8, 6 182, 7 172. So map containers are map chunks,
  **not** slice sets; the idea that slice set 8b + n is bubble b's container n
  (`public-research.md`) is wrong. The extra slice sets (105, 113) must come from elsewhere,
  probably the mission's activity data.
- Not in the uploaded packages: the mission's activity data (`act_cosmo_m0_highway` and its group
  keys appear in none of them; the Cosmodrome's 11 activity infos are in
  `ps3_cosmo_activities_activity_025c_0.pkg`) and the destination packages 0x270-0x274.

## act_cosmo_m0_highway from the disc (package 025c, 2026-09-27 night)
Named tags: `act_cosmo_m0_highway` (0x80cb8001, class 0x808008ba) and
`act_cosmo_m0_highway:scenario_client` (0x80cb800d, class 0x80800616); the package holds every
Cosmodrome story activity (m0 highway, m0_5 hangar, m1 dome_a, m2/m4 dome_c, m5 refinery, m6
rasputin, m7 command, portals, patrols).
- **scenario_client** = {u32 size, +4 0x992bb471 (activity group key = FNV-1 of the activity
  name), +8 0x79130968 (the activity checksum the client compares), ..., array @0x30 of 22
  per-bubble slice-set lists {bubble name hash, array of 0x3c-byte slice-set records}}. Every
  bubble has one slice set except `hangar` (13) and `highway` (14), which have a second one:
  **`cosmo_m0_phase04_outro` = 105 and `cosmo_m0_phase00_highway_intro` = 113** (both flagged
  1/1/1 at +0x10..+0x18). So slice set 8b + n is bubble b's n-th slice set in this list; each
  record names a slice-set tag (112 -> 0x80cb801d -> 0x80cb8ce3, 113 -> 0x80cb801e -> 0x80cb8d50)
  whose arrays list the replicated groups loaded with it. The tag ends with the mission's phases
  in order with a kind (0 cinematic, 2 gameplay): phase00_highway_intro (0), phase00_highway,
  phase01_breach, phase01b_breach, phase02_graveyard, phase03_hangar, phase04_outro (0).
- More bubble dev names: 6 `delta_n`, 7 `delta_s`, 11 `dry_sea`, 17 `rasputin`, 18
  `rasputin_vendor`.
- **Replicated groups** (class 0x8080076e; `d1_pkg.py groups`): {size, destination hash, group
  key, u32, array @0x10 of {u8 kind, 3 bytes, u32}, ...} then one dev name per object; **an
  object's replicated index is its position in the list** - all 118 records of TEST362 match.
  Full table: [`mission-objects-highway.md`](mission-objects-highway.md). The kinds, named:
  0x20 `_player_objective` (e.g. 0x992bb471/0x20/3 = obj_m0_phase00_m1, "Enter the Wall"), 0x5
  `_incoming_sequence` / `_completion_sequence`, 0x6 `_cinematic` (1 pf_104_res intro, 2
  pf_104_res_fp, 10 pf_104a_pwr, 29 pf_105_cto outro), 0x4 spawned props (skeleton_static_1,
  o_weapon; in bubble 14 the o_ghost_* lights and pigeon_1-22), 0x1 squads (sq_ghost), 0x1d
  `_player_trigger` / pt_* points, 0x1c pm_in_wall, 0x1e tg_m0_gating. Kinds 0x2c (nav points),
  0x37 (bubble links), 0x3a (trigger volumes), 0x3b (script objects), 0x3c (phases), 0x3d are
  listed but not replicated. The game-mode group 0x4786c0e0 (ambient package 0268): 0xf/0
  `scoreboard`, 0x22/1 `hard_wipe_globals`, 0x11/2 `lifetime_timer`, 0x10/3 `lifetime` (what
  `--ah-game-mode` sends), **0x27/4 `new_user_experience`**, 0xc/5-20 `players[0..15]`.
  Several earlier guesses were wrong (kind 0x20 is not a spawn group, 0xf not a per-player
  activity table); the handler labels are renamed.
- Each object has a definition tag (class 0x80800861) with typed components; the objective's
  (0x80cb911e) embeds a 0x80802a1b component (the kind-0x20 state type: u3 state @0, target
  object ids, ten 64-byte per-entry records) with a back-reference 992bb471/0x20/3. Its apply
  handler `0x3561d8` copies the state to component +0x110 and special-cases state 3.
- **Objective text from the disc** (`d1_pkg.py strings`: string index class 0x8080035a +
  language table 0x808008be, text bytes stored minus 0x1f). Each `_player_objective` definition
  carries its title and description hashes; the chain, with the activity-group kind-0x20 index:
  3 "Enter the Wall" / "Take refuge inside the ancient wall." (0x52ff1516 / 0xb1fb2ef7);
  7 "Find a Weapon"; 11 "Grab the Rifle"; 14 "Fight through the Wall"; 17 "Fight through the
  Fallen" ("Fight to the other side of the Divide..."); 20 and 23 "Find the Ship"; 26 "Clear out
  the Fallen" ("Eliminate the Fallen looting the ship."). This is the published objective order of
  A Guardian Rises, so the objects above are the mission script's objectives in order.

## Character gear: why the guardian renders see-through (static analysis, 2026-09-27)

- `0x670e40` -> `0x6706b0` build a player's appearance from its character data (player record
  +0x8b0, copied from the peer-family summary). Besides class/race/gender and the customization
  struct, `0x6706b0` reads the 20 equipment slots of progression struct `0x80801855` (+0x138:
  20 x `0x8080183c` {u64 instance id, i16 item index, i8, i16, six dyes}). The char data places
  that struct at +0x408, so the slots are at +0x540 (stride 0x28), the weapon perks at
  +0x914/0x934/0x954 and the weapon stats at +0x9f4/0xa74/0xaf4.
- Item index -> definition: `0x41436c` (-1 = none) -> `0x28a304` -> investment globals root
  +0xd8 item list (`0x288278`), 8-byte entries {u32, u32 definition tag}. The slot table (root
  +0xc8, `0x28a1dc`) gives the slot of each bucket: +7..+0xb armour, +0xc..+0xe weapons.
- The stand-in's summaries leave every slot empty, so no gear loads and the guardian stays the
  translucent gear placeholder. `bap_server.py --character-gear` / `--character-overlay` fill
  the slots (peer +0x30, select +0x68, self inventory +0x30 and equipped +0x1838). The item
  indexes need the investment globals packages. Details, the full gap list and the data wanted:
  [`feature-gaps.md`](feature-gaps.md).
- No Lua bytecode in any of the 79,148 decompressed Cosmodrome tags: the mission scripts are not
  on the disc and have to be rewritten for the stand-in host.
- Update (packages uploaded the same evening): the investment globals root is the named tag
  `investment_globals` `0x80a58000`; +0xd8 = 3,181-item list (definitions in package 012e),
  +0xb0 = the 14 equippable buckets (Subclass, Helmet, Gauntlets, Chest, Legs, Class Armor,
  Primary, Special, Heavy, Ships, Vehicle, Ghost, Emblems, Shaders; Bungie's public D1 API
  bucket hashes). The starter "wire armor" per class and `--character-gear starter:CLASS`:
  [`feature-gaps.md`](feature-gaps.md) section 1; next run TEST370.

## Mission object states decoded (static analysis, 2026-09-28)

The field-level meanings of the objective (`0x3561d8`), cinematic (`0x4ed278` / `0x4ece68` /
`0x4ecd08`), sequence (`0xb94414` / `0xb942a0`) and squad (`0x1f8d20`) states are in
[`mission-director.md`](mission-director.md). The main findings:
- An objective shows only for players in its +0x2c0 mask.
- A cinematic starts on a changed +0x4 counter with +0x8 set and the player in the cast list.
- A sequence plays only if its start time (+0x0) plus its duration is still ahead of the
  simulation clock. That clock (`0x1c94e0`) is the host tick of the type-5 packets, which the
  stand-in had always sent as 0; `--ah-sim-clock` fixes it.
- The client's type-6 upstream messages carry its own player object `0x4786c0e0/0xc/5`, with the
  object id at bit 11.
- TEST367: the host-pushed zone set 24 was ignored; the client stayed in 112 (test log).

