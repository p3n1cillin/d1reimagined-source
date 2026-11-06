# PS4 56578: a static integration review of #306–#323 (5–6 November 2026)

This is a read-only review of the merged work, done offline: no game, lab or emulator runs. It
looks for bugs between subsystems: knob and default conflicts, double grants, id-scheme
mismatches, wire-format disagreements between streams, one stream's assumptions against
another's evidence, and tests that can't fail.

**Line numbers** are at c99af915 (#306–#323 merged, #324 not yet). #324 (stream K) has since
landed and may have moved lines in the activity host. Paths are relative to
`ps4/d1-server/crates/` unless they start with `ps4/`, `tools/`, `tests/` or `docs/`.

**Severity:**
- **high:** wrong game state, a double grant or a broken run in a default or documented profile
  (`ps4/profiles/full-game.env`);
- **medium:** wrong under a documented opt-in combination, or latent until the next wiring step;
- **low:** latent, cosmetic, or a test weakness.

**Fixed in this branch** (only the clear, small fixes inside the files this branch already touches):
- **F1.** `social_hooks::bridge_progressions` used to hold the social state's lock while it ran.
  Its new profile push now runs on its own thread. The callers in `vendor.rs` hold the vendor
  state's lock, and the push waits up to 120 s for the vendor's own push to be delivered, which
  needs that lock.
- **F2.** `PlayerStore::add_xp` (`d1-responder/src/player_store.rs`) now works on a copy of the
  profile. A refused prestige entry (a full progression table) used to leave the level part applied
  without marking the profile stale, so the bridge's re-queue would have paid it twice.
- **F3.** A completion's reward-row reputation used to land in the object being replaced. The
  completing object now gets it, and the store's totals are folded into every adopted object. This
  was part of the reputation push; see notes/ps4-56578-player-store-2026-11-05.md.

Everything below was left for the fixing phase. Its status is the next section.

## Status after the fixing passes (O2 #327, O3 6 November)

Each finding by number. "#327" is stream O2's pass (merged); a commit hash is stream O3's
(branch `integrate/multiplayer-fixes`). Built and unit-tested only: nothing here has run in the
game.

| # | Status | Where, or why still open |
|---:|---|---|
| 1 | fixed (#327) | 165 is scripted as a cinematic with its regions; the profile test checks both. |
| 2 | fixed (#327) | The loopback responder starts without the store, MP and rewards knobs. |
| 3 | fixed (#327) | Vendor ops start from the store's state; `publish` waits for the last push. |
| 4 | **fixed (74efb865)** | The real fix, replacing #327's stopgap. Every BAP connection signs on as its own player with its full identity (`identities.json`'s scheme, `0xD101_0000_nnnn_n001/101/201`); the connection's task carries it (`players.rs`), so every id it is served is its own. The store holds several players' records keyed by identity; each player has its own push files and primary connection. Grants and hand-offs go to the store of the player whose account **and** character the identity names, and an identity nobody here has is refused with `D1_MP=1`. Tests: `two_players_are_each_paid_once_into_their_own_record`, `each_player_has_its_own_push_files_and_record`, `a_connections_task_and_a_thread_carry_its_player`. `[I]`: a connection is known from its first BAP 23; anything served before that is the knobs'. |
| 5 | fixed (#327) | `D1_SOCIAL_STOCK=0`. |
| 6 | fixed (#327) | One completion path. |
| 7 | fixed (#327) | Open-world grants carry the 40-byte identity. |
| 8 | **fixed (2c10bddf)** | One rule everywhere, emptiest first, oldest on a tie: the 42 directory (`rank_fill`), the strike playlist fill and `place_in_public`. PR #328's patrol and Tower captures (joined instances 30 s to 2.4 minutes old) are consistent with it; the rule itself is still not on the wire. |
| 9 | **fixed (c0d9af86)** | One script bridge per shared instance (`shared_scripts.rs`): one sidecar, every member's reports drive it, every output reaches every member, each member paid once. Test: `a_shared_instance_runs_one_script_for_both_members_and_pays_each_once`. |
| 10 | **fixed (c0d9af86)** | A public-instance connection drops its mission bridge at the join and runs no activity framework. Test: `tests/multiplayer_scenario.rs`. |
| 11 | **fixed (c0d9af86)** | The shared-instance join runs before the public role, so the space registers under the shared host. Test: `a_tower_join_registers_under_the_shared_instance_host`. |
| 12 | **fixed (74efb865)** | A refused social op puts the side store back as it was. The side stores are now per character, too. |
| 13 | fixed (#327) | Every item through the exported index. |
| 14 | fixed (#327) | With finding 3. |
| 15 | **fixed (74efb865)** | `D1_REFUND_WINDOW` is the window's length only; which purchases are refundable is the game's rule (with the store on, the manifest's policy per item), the same with `D1_SOCIAL` on and off. Defaults unchanged. |
| 16 | **not a bug (175f1c7a)** | The finding was wrong: the sidecar reads `D1_BUNDLES_DIR` from the environment it inherits (`sidecar.mjs`, `bundlesPathFor`). The launcher keeps requiring it (without it every row is a stand-in the bridge never sends) and now says so. |
| 17 | **fixed (175f1c7a)** | A hello that throws answers a fatal error. Test: `makes a hello that throws fatal, and a later failure not`. |
| 18 | **fixed (c0d9af86)** | The cache key names the encoder (native or oracle) and its revision; the format is v2, so pre-#321 frames are misses. |
| 19 | fixed (#327) | `activity_driver::RunDriver`. |
| 20 | open | The open world's bridge still has no scripted run for a bubble; it needs stream K's single-public-event message. |
| 21 | open | Chest, node and patrol grants still go out as `completed` (they are not store completions: `completion_index` takes only `source=script`). Needs their own outcome words in the grammar. |
| 22 | open | `min_players = team_size` still; the Crucible limits need a decision on the evidence (#326). |
| 23 | open | The registry's cap and TCP-drop leave are not enforced. Small, but it changes the open world's lifecycle: left for the public-instance stream. |
| 24 | open | The host still sends default `GrantDetails`. |
| 25 | fixed (#327) | |
| 26 | open | Account-owned overlay rows are still per character. |
| 27 | **fixed (5dd96de3)** | One dismantle table: `rewards::currencies::dismantle_yield` reads `inventory_ops::PLACEHOLDER_YIELDS` (PR #328's live rows), and the Marks amount is 3 for both. |
| 28 | open | Changing the host's parse to decimal-unless-`0x` would change how existing hex values without `0x` read; needs the profiles checked first. |
| 29 | open | A fixed seed still rolls the same engrams. |
| 30 | half fixed (5dd96de3) | `owner_of` no longer panics on an empty word. The control file's 50 ms window is still a race. |
| 31 | half fixed (175f1c7a) | The 7-or-8 end is documented. Still open: readiness on `players`, the 8-of-25 incident read, the ignored wipe fields. |
| 32 | half fixed (5dd96de3) | `D1_DISMANTLE_YIELDS` is in the knob table. Still open: the completeness test scans only `main.rs`; the sidecar command is split on whitespace; `bundles=` is cwd-relative. |
| 33 | open | |
| 34 | **fixed (175f1c7a)** | |
| 35-39 | open | Test weaknesses, not changed here (39's `every_hook_declines_when_mp_is_off` still never calls `init`, which is how it keeps the services absent). |

#327's open items:

- **The 43 answer** (2c10bddf, 74efb865, c0d9af86): a placed bubble search is answered with an
  advertisement from a member of the instance the registry chose (the host's snapshot names each
  instance's members by account), or with none when a new instance starts.
- **The bubble id's destination** (74efb865, c0d9af86): the snapshot also names each mission
  host's player and destination, and a search is placed among the searcher's own destination's
  instances.

PR #328's findings, taken into this pass:

- **Dismantle** (5dd96de3): `DISMANTLE_MARKS_ARMOR` is 3; `currency.dismantle.marks` and
  `currency.dismantle.other` are verified (the latter was wrong); the rewards path credits
  Glimmer and materials from the one table.
- **The Glimmer cap** (74efb865): 25,000, in the store and on the 402 dismantle.
- **Event tiers** (2c532378): a public event's completion goes to the instance's clients as its
  incident set (gold 1918, 3077, 732; silver 1918, 3076; bronze, never captured, the completion
  alone); `tier_of_completion` reads it back.
- **Placement** (c0d9af86): emptiest first kept; the evidence is in `public_registry.rs`'s doc.
- **Still open:** packages on 904 (straight into the inventory: +500 Glimmer and a gear item,
  Dead Orbit also 4 Motes), bounty turn-in on 402 action 1 (today a 402 on a bounty is not a hand-in: with `D1_SOCIAL`
  it is refused, a bounty having no dismantle action; it needs the hand-in and the equipped
  items' XP updates), and The Dark Within's four objective rows (its script reaches 1 of 3 phases, so a
  phase-to-row map alone would not finish it). Each needs its own wire work.

## High

1. **The reveal cinematic is scripted as gameplay, so it ends on lifetime 7, not 8.**
   - **Where:** `ps4/profiles/script-activities.txt:6`. The 165 `cine_reveal` line has no
     `kind=cinematic` (and no `regions=`).
   - **Why:** `ScriptedActivity::parse` defaults to gameplay (`d1-activity-host/src/script_bridge.rs:429`).
     For gameplay, `row_encoders.rs:209` sends 7 after 6.
   - **Scenario:** the full-game run sends 0→1→4→6→7, not the cinematic order 6→8 that #318
     recorded and that `docs/full-game-test.md:180` checks for. The 6→8 stamp pinning
     (`script_bridge.rs:1338`) never fires, and without `regions=` the phase never moves the slice.
   - **Fix:** one line, `kind=cinematic regions=0:25,1:26`. `tests/full_game_profile.rs:245` only
     checks that the table isn't empty, so it can't catch this.
2. **Two responders share one player store and one control file.**
   - **Where:** `tools/ps4_setup/Start-D1ContainedBoot.ps1:195`, then `:209`. The server
     environment, including `D1_STORE=1` and `D1_STORE_CONTROL` (`full-game.env:10,16`), is
     inherited by the second, loopback `d1-responder` (the emulator's WebAPI).
   - **Why:** both responders open the store at start-up (`print_knobs` → `unlock_flag_spec` →
     `global()`), and each starts its own control poller and periodic save. There is no lock file.
   - **Scenario:** about half the time the loopback responder takes the host's `grant` or
     `checkpoint` line. It saves the record, but the serving responder never sees the change,
     pushes nothing, and overwrites the record at its next save. Both write
     `store-control.state.json`.
   - **Fix:** small. Strip `D1_STORE`, `D1_STORE_CONTROL` and `D1_MP` from the loopback
     responder, or add a store lock.
3. **The vendor's profile copy overwrites the store's push, and XP is lost.**
   - **Where:** `d1-responder/src/vendor.rs:462` (`publish` always writes `D1_PROFILE_PUSH_FILE`);
     `:798-815`, `:831` (each op starts from the vendor state's own profile);
     `player_store.rs:704-706` (`commit_vendor` adopts that object).
   - **Scenario:** under full-game (store and social on), two bounty hand-ins arrive within one
     push round-trip.
     - Buy 1 publishes P1. The bridge adds XP to the store and pushes S1.
     - Buy 2 starts from P1, which lacks that XP. It overwrites the undelivered S1, and the store
       adopts it.
     - The XP is gone from both the client and the store. Reputation survives, because the store
       folds it back in; XP has no such fold.
   - **Same pattern:** `run_grant`, whose `adopt_account_slots` publishes straight after
     `push_profile`, and any control command followed by a vendor op.
   - **Double completion:** a vendor op that started from a pre-completion profile makes the store
     record the activity as started again. The completed profile then counts the completion a
     second time and re-applies its reward row (`player_store.rs:763-787`).
   - **Fix:** small to medium. With the store on, the vendor starts every op from the store's
     object, and `publish` waits for the push file as `write_push` does.
4. **Under `D1_MP=1`, every connection is served the same SOIDs, and grants are matched by
   character alone.**
   - **Where:** `d1-responder/src/main.rs:1531/1545/1552/1676/1683` and `vendor.rs:440-442` (one
     `env_soid` identity for all); `rewards/remote.rs:316-320` (the strict check compares only the
     identity's +0x18).
   - **Scenario:** a two-player shared instance. The host forwards one grant per member. Both pass
     the check and both pay player 1; player 2 gets nothing.
   - **Related:** a second store player's allocated identity (0xD101…) is never served, so all of
     that player's grants are refused.
   - **Fix:** per-connection identities (not small). As a stopgap, also check the account at +0x10
     and dedupe by (character, activity, run).
5. **`full-game.env` turns on `D1_SOCIAL_STOCK=1`, which the code says must stay off.**
   - **Where:** `ps4/profiles/full-game.env:28`, against `d1-responder/src/social_hooks.rs:84-89` and
     `vendors/rotation.rs:10-16`. The client lists the whole catalogue, and the rotation is a
     random stand-in.
   - **Scenario:** most Gunsmith orders and Bounty Tracker bounties the client shows are refused
     with NotAvailable.
   - **Fix:** one line.
6. **A scripted completion pays stream M's grant but never runs the store's completion.**
   - **Where:** `script_bridge.rs:1174` forwards `grant … completed` (`loot::grant_activity_rewards`:
     manifest XP and currencies). The store's completion, `Command::Complete`
     (`player_store.rs:1517`, `:1552`), sets the completion subs, the rewards table's XP, unlock
     flags and reputation. Nothing writes `complete N`.
   - **Scenario:** a full-game run of 164 or 165 never sets 164's subs 40/673 (the CONTINUE gate)
     or 165's sub 41.
   - **XP:** `rewards/player_store_adapter.rs:9-10,75` (`xp_from_manifest: false`) assumes the
     table gives the XP, so no completion XP is paid at all. Wiring `complete` beside `grant`
     without choosing one XP source would pay it twice.
   - **Fix:** moderate. Choose one source, and have the bridge or `remote::execute` run the
     completion.

## Medium

7. **Open-world grants are keyed by the machine id.**
   - **Where:** the key is set at `d1-activity-host/src/public_host.rs:91` and sent as
     `GrantTarget::Key` at `reward_forward.rs:219`. The responder reads it as a character SOID
     (`rewards/remote.rs:80`), and strict mode (`D1_MP=1`) refuses it at `:318`.
   - Two characters on one machine also share a key. This is latent: nothing grants from the open
     world yet. Fix: small, send the 40-byte identity.
8. **Two placement rules; the live one is fullest-first.**
   - **Where:** `PublicRegistry::placement_order` is emptiest-first (#323). The msg-42 search that
     actually runs uses `d1-mp/src/matchmaking.rs:116-130` (`rank_fill`) and `:301,307`, which are
     fullest-first.
   - **Fix:** small; align them, or say which rule is live.
9. **Shared instances don't share their scripts.**
   - **Where:** `d1-activity-host/src/main.rs:1139` builds a `ScriptBridge` and sidecar per
     connection.
   - **Scenario:** a fireteam in a scripted activity runs N copies of the script. One member's
     trigger advances only their copy, the udt 5 rows differ per client, and each connection sends
     its own completion and grant.
   - **Fix:** one bridge per instance group (not small).
10. **A public-instance connection runs the followed mission's script.**
    - **Where:** `d1-activity-host/src/state.rs:815`. Scripted frames win in `sensor_due`.
    - **Scenario:** once a mission with a public start (166) is scripted, the public connection
      starts a second sidecar, owns the bubble's udt 5, and sends a second `completed` grant.
    - **Fix:** small; no bridge for `PublicInstance`.
11. **A social-space join registers under the wrong host id.**
    - **Where:** `state.rs:1275` (`on_join` with `self.ah_id`) runs before `link.join` rewrites the
      id at `:1308`.
    - **Scenario:** shared and public hosts with distinct ids (all on in full-game). Player B
      joining A's Tower shows up as a second Tower instance with its own events and loot.
    - **Fix:** small.
12. **A refused social op discards the side store.**
    - **Where:** `d1-responder/src/vendor.rs:786` returns before `keep_side` (`:792`), after
      `social_hooks.rs:54` took the side store out.
    - **Scenario:** a refusal loses bounty progress, Armsday counters (allowing extra orders) and
      XP that `bridge_progressions` re-queued.
    - **Fix:** small; restore the side on error.
13. **#322's item index isn't used by grants, so Legendary Marks go to a hidden overlay.**
    - **Where:** `rewards/remote.rs:174-200` (`ResponderIndex` maps Glimmer only).
      `rewards/item_instance.rs:84-91` and `content::item_index` exist but aren't used here.
    - **Scenario:** Marks (content 4548, slot 310 in `lab-base.env`) go to `rewards.stacks`, so the
      client never sees them, and the 200 cap is checked against 0.
    - **Fix:** small; fall back to `content::item_index` and `content::progression_index`, then
      migrate the stacks.
14. **A grant and a vendor op can lose a wallet update.**
    - **Where:** `player_store.rs` `run_grant` (slot snapshot), `vendor.rs:509-516`,
      `player_store.rs:715`.
    - **Scenario:** a grant during an unpublished vendor op loses either the grant's Glimmer or the
      purchase's cost.
    - **Fix:** apply deltas under the vendor lock.
15. **`D1_REFUND_WINDOW` means different things with `D1_SOCIAL` on and off.**
    - **Where:** `vendor.rs:575-590` forces a refund record for every purchase.
      `vendors/purchase.rs:141-143` gives one only to sales whose rule is refundable. The KNOBS row
      says "every vendor".
16. **The launcher requires `D1_BUNDLES_DIR`, which nothing reads.**
    - **Where:** `tools/ps4_setup/d1-full-game.ps1:411-415`. The sidecar's bundles come only from
      the table's `bundles=` (`script_bridge.rs:380`).
    - A default run is NOT READY until an inert variable is set, and then still runs stand-in
      bundles.
17. **The sidecar reports hello failures as non-fatal.**
    - **Where:** `tools/script-host/src/sidecar.mjs:107`. The host only warns
      (`script_bridge.rs:1187-1192`), so `loaded` never comes and the run hangs silently.
      PROTOCOL.md says such an error is fatal. Fix: small.
18. **The sensor-cache key ignores the codec and the oracle's revision.**
    - **Where:** `d1-activity-host/src/codec.rs:767`.
    - **Scenario:** frames cached before #321's ScalarFloat correction are still served. With
      `D1_RSAT_NATIVE=1`, a cache hit skips the native encoder, and both profiles share
      `${STAGING}/sensor-cache`, so a native-versus-oracle comparison is confounded.
    - **Fix:** small; bump the key.

## Low

19. **The activity framework is loaded but drives nothing.** `d1-activity-host/src/main.rs:120-145`
    validates the library and drops it; no `ActivityRun` or `CrucibleMatch` runs outside tests. So
    with `D1_AH_ACTIVITY_FRAMEWORK` on, no checkpoint or lockout is written. The comment at `:122`
    says otherwise.
20. **The open world's bridge can never start a bundle.** It is a separate `ScriptBridge::new`
    (`main.rs:1039`) with no join, so `start_bundles` always refuses (`script_bridge.rs:1378`).
21. **Chest, node and patrol rewards would pay as activity completions.**
    `reward_forward.rs:209-219` sends them as `completed`. `public_host.rs:429` passes activity hash
    0, so a chest grant is refused.
22. **A Crucible match can form with one team empty.** `encounter_framework.rs:1108` sets
    `min_players = team_size`, and `d1-mp` only checks `teams_feasible`. Trials could form from one
    fireteam. `tests/activities.rs:250` asserts the 3, locking it in.
23. **The public registry never forgets players and never enforces its cap.**
    - `on_leave` runs only on udt 14 or a connectivity failure (`state.rs:1487,1499`), not on a TCP
      drop.
    - `attach` (`public_registry.rs:380`) ignores the cap of 6.
    - So occupancy drifts upward.
24. **The host never sends the reward page or the engram flag.** `reward_forward.rs:195,219` send
    default `GrantDetails`. Strike and raid Marks pages and end-of-activity engrams have no path to
    the responder.
25. **Item SOIDs can be reused.** `commit_vendor` (`player_store.rs:718`) writes back the vendor's
    cached `next_soid`, which can rewind past a reward's minted SOID (`vendor.rs:166,470`).
26. **Account-owned overlay rows are kept per character** (`rewards/remote.rs:271-338`), so a
    different character on the same account resets the weekly-bonus counters.
27. **Two dismantle placeholder tables disagree.** `inventory_ops.rs:167-177` gives 2 Marks;
    `rewards/placeholders.rs:37-38` and `currencies.rs:113-136` give 3 or 2. Only the first is
    wired.
28. **`D1_CHARACTER_SOID` is parsed two ways.** The responder (`main.rs:5444`) reads decimal unless
    the value starts with `0x`; the host (`checkpoints.rs:273-276`) always reads hex.
29. **A fixed `D1_REWARDS_SEED` (`full-game.env:34`) rolls the same engrams every completion.**
    `rewards/loot.rs:91-92` seeds by activity and character only.
30. **The control file has a race, and the reward overlay can panic.**
    - `take_commands` (`player_store.rs`) waits only 50 ms after the rename, so a late writer's line
      can be lost.
    - `owner_of` (`rewards/remote.rs:242`) panics on an empty string, under the store's mutex.
31. **Script-host protocol disagreements:**
    - `session.mjs:310` resets readiness on `players`, and the host never sends `ready`.
    - `types56578.mjs:32` and `PROTOCOL.md:167` say the run ends on 8 for every activity; #318
      recorded 7 for gameplay.
    - The host reads at most 8 incidents of a udt 18 envelope that holds 25 (`script_bridge.rs:855`).
    - `hard_wipe_armed.armed` and `all_dead.dead` are ignored (`script_bridge.rs:180-182`).
32. **Knob tables and paths:**
    - `D1_DISMANTLE_YIELDS` (`vendor.rs:683`) is missing from the responder's KNOBS table. The
      completeness test scans only `main.rs`.
    - `D1_AH_SCRIPTS_SIDECAR` is split on whitespace (`script_bridge.rs:504`).
    - `bundles=` resolves against the host's working directory.
33. **The baseline host now decodes udt 37 through Unicorn on the connection loop**
    (`d1-oracle/src/activity_host.rs:334`, #321).
34. **Stale documentation:**
    - `PROTOCOL.md:63` says `trigger_report`; the catalogue name is `player_trigger`.
    - `PROTOCOL.md:140` and `docs/full-game-test.md:156` say the 965 body is unknown; #318 decoded
      it.
    - `docs/full-game-test.md:156` names a knob, `D1_AH_SCRIPTS_COMPLETE_BODY`, that no code reads.
    - `docs/full-game-test.md:165` says every `[server]` knob reads SET, but the `D1_AH_*` knobs
      aren't in the responder's table.

### Tests that can't fail, or barely test

35. Profile and host tests:
    - `d1-activity-host/tests/full_game_profile.rs:136-138` treats a path whose parent folder is
      missing as outside the checkout, so a mistyped nested folder passes.
    - `:245` (the script bridge from the profile) asserts almost nothing; it misses finding 1.
36. Tests that skip whenever `D1_EBOOT` is unset, even though the default dump that the code
    itself loads is present:
    - `completion.rs:470`;
    - `d1-rsat/tests/common/mod.rs:12`;
    - `d1-oracle/tests/wire_catalog.rs:10`;
    - `content_tables.rs`'s `owned_56578_exports_match_every_committed_table`.

    They skip honestly, but they run less often than they could.
37. `d1-rsat/tests/sensor_frames.rs:69` tests a hand copy of the host's `native::sensor_frame_of`.
    The host's own test covers only a frame with no clients.
38. `tests/test_world_table.py:206` regenerates every worlds table but `activities.tsv`, which
    `generated.rs` reads. `tests/test_d1_worlds.mjs:14` hard-codes a local path, so it silently
    skips elsewhere.
39. Responder tests:
    - `d1-responder/src/rewards/remote.rs:469` asserts a tautology.
    - `mp.rs`'s `every_hook_declines_when_mp_is_off` never calls `init`.
    - `rewards/mod.rs`'s `off_by_default` asserts nothing when `D1_REWARDS` is set.
    - No grant test has a Marks slot, so finding 13 can't show up.

## What checked out

- The incident ids the host uses (891, 965 chain, 1726, 2014, 2079, 2681, 2899, 3403, 566) match
  `incidents-56578.tsv`, and the legacy joins 789→965 and 734→891 hold.
- FNV-1 agrees in Rust, Python and JavaScript.
- Activity index→hash agrees across `worlds/activities.tsv`, `characters/activity-table.tsv` and
  `activity-rule-programs.json`: 653 rows, no mismatch.
- `progression-curves.tsv`'s runtime indices match `progression-index.tsv` (26 of 26).
- The social `progressions.tsv` names and steps match the store's curves.
- The vendor reset schedules agree between the two tables (26 of 26). Eververse is 0xE97AAA75 in
  both, and Glimmer is content 869 in both.
- The membership offsets agree across `group.rs`, `messages.rs` and d1-oracle.
- The `grant` line grammar is the same on both sides.
- One `D1_RSAT_NATIVE` switch drives both codec paths.
- The bridge holds `Ended` until the last lifetime value is sent, so a grant never precedes the
  client's 7 or 8.
- These tests pass:
  - `tests/test_incidents_56578.py`, `test_rtti_catalog.py`, `test_srcp.py`, `test_world_table.py`
    and `test_full_game_profile.py`;
  - `scripts/test_d1_incident_catalog.py`;
  - `node --test` in `tools/script-host`.
