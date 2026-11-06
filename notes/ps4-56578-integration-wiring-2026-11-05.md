# PS4 56578: wiring the subsystems to one another (5 November 2026)

Stream O of the full-game operation. About twelve subsystems merged today, each behind its own knob
and each unit-tested alone. Several were built but not connected, so in the full-game profile
([`ps4/profiles/full-game.env`](../ps4/profiles/full-game.env), #320) they would have done nothing.
This change connects them.

**Built and unit-tested only.** No game, emulator or lab run has exercised any of it. Every wire is
behind the knobs that already existed; with them unset, both servers behave exactly as before.

## The seams

Each row is one place where one subsystem hands something to another. "Wired" means the code path
exists and is tested; it does not mean it has been seen working in the game.

| # | Seam | From → to | Status | How, or why not |
|---|---|---|---|---|
| 1 | End-of-activity rewards, scripted activities | the script bridge's `rewards::ActivityRewards` (#319, activity host) → stream M's `rewards::grant_activity_rewards` (#311, responder) | **wired** | `reward_forward::GrantForwarder` implements the trait. It queues one `grant` line per character into the store's command file (below). It is on when `D1_REWARDS=1`, `D1_STORE=1` and `D1_STORE_CONTROL` are all set; otherwise the bridge keeps `NoRewards`. Stream K moves the call to lifetime 6 and unifies the traits; the forwarder stays the same. |
| 2 | Open-world rewards | stream G's `open_world::ActivityRewards` (#316) → the same grant | **wired** | The same forwarder. The open world names characters by the registry's 8-byte key, the first 8 bytes of the identity, which is not the character SOID. The responder applies such a grant to its signed-on character when `D1_MP` is off (one player per process). With `D1_MP=1` it refuses it. The event activity hashes are G's placeholders. A hash that is not in M's manifest table is refused and logged. |
| 3 | The grant on the store | the `grant` command → `rewards::player_store_adapter::Adapter` over `player_store::PlayerStore` | **wired** | `rewards::remote::execute`. It saves, pushes the profile when XP changed it (`push_profile`), and has the vendor state publish the account when a wallet changed (`vendor::adopt_account_slots`). The adapter's period counters (weekly bonus, Marks caps) and its stacks with no account slot persist in the character's `attributes` (`rewards.counters`, `rewards.stacks`). The last 50 grants are kept in `rewards.grants`. |
| 3a | | item hash ↔ 56578 content index | **wired** (O2) | Every item through #322's exported table (`content::item_index`, 8,609 items; `ResponderIndex`). `content::bucket_of` gives each content index its client bucket: the item definition's own `+0x88` where `vendor-sales.tsv` or the starting kit carries it, else its manifest bucket's ordinal, derived from those rows and kept only where every row agrees (25 manifest buckets, no disagreement). `PlayerStore::deliver_stack` puts an account bucket's stack in the account's slots (Glimmer 308, Legendary Marks 310) and a character bucket's in the decoded profile (materials 0-19, Vanguard Marks 278, Crucible Marks 279, Silver Dust 250-261), which is then stale and pushed. Stacks an earlier grant kept in the overlay are delivered by the next grant. Refunds: the social purchase path now takes the sold item's refund window (`refund_window_for_item`), per sale, not per vendor (Eververse refunds 67 of its 123 sales). |
| 3b | | the overlay's item instances and grids | **wired** (O2) | The whole overlay persists in the character's `attributes` (`remote::OVERLAY_ATTRIBUTES`: counters, unplaced stacks, instances with grids and rolls, progressions, objectives, Record Book claims, the Grimoire) and is restored before each grant. A new instance is written from `D1_VENDOR_ITEM_TEMPLATE` (`write_element`) and placed in a free slot of its bucket (`deliver_instance`); each one goes out as its own profile push with `.item`/`.soid`. The element's level, grid index, node states and stat positions are still the template's (`ClientIndex` has no answer yet). |
| 3c | | a wallet change without the vendor state | **wired** (O2) | With the vendor state on, `vendor::adopt_store_state` publishes the store's slots, profile object and items. With it off (`D1_VENDOR_SALES` or `D1_SOCIAL` unset), the store pushes by itself: the profile re-encoded from its object, and the account through `player_store::push_account` (`vendor::account_payload`, the encoder `publish` now shares). |
| 4 | Checkpoint save | `checkpoints::ControlFileCheckpointStore` (#313) → `set_checkpoint` | wired (#313) | unchanged |
| 5 | Checkpoint clear | `checkpoint INDEX -` → `clear_checkpoint` | **wired** | The responder reads `checkpoint N -` (and `clear-checkpoint N`) as a clear. #313's store already sent it. |
| 6 | Weekly loot lockouts | `mark_looted` → the store | **wired** | A new record field, `characters[..].lockouts`: activity index → encounter → reset week. A new command, `lockout INDEX ENCOUNTER week=W`. `PlayerStore::{lockout_week, set_lockout}`. The host sends it on `mark_looted`. |
| 7 | Checkpoint read-back at launch | the store → the activity host | **wired** | After each batch of commands, and at sign-on, the store writes `<control>.state.json`: the character, its checkpoints, its lockouts and its last grants (`player_store::state_file_for`, `state_json`). `ControlFileCheckpointStore::load` and `is_looted` read it when the mirror has nothing. `ControlFileCheckpointStore::from_env` takes the character from it. |
| 7a | | a running `ActivityRun` using the store | **wired** (O2) | `activity_driver::RunDriver`, started at the join when `D1_AH_ACTIVITY_FRAMEWORK` installed the framework and a definition runs the launched hash. Scripted activities: the bridge's `phase`, `all_dead` and `activity_complete` (`ScriptBridge::take_framework_events`). The rest: the client's slice sets and trigger reports, the capture sequences' own events. Incident 2079 is a death, 566/2014 a respawn, and a respawn after a wipe ends it (a labelled stand-in: no hard-wipe handshake is known). Checkpoints and lockouts go through the run's store, keyed by the identity's character SOID. The outcome is granted once per member by the full identity, except for scripted activities, which the bridge grants at 6. |
| 8 | The public host's lists | `public_host::FrameEncoder` → #319's `SensorEntryFrame` | **wired** | `public_host::entry_frame_encoder(codec, gen)` builds gen, no authority, `init = 1` and the region's entries. It goes through the host's `AhWireCodec::encode_sensor_entries`: `NativeCodec` (d1-rsat) under `D1_RSAT_NATIVE=1`, the client's own codec otherwise. `gen` is `D1_AH_SENSOR_GEN` (default 0). `main.rs` no longer passes `encoder_pending()`. No public list has been served to a client yet, and their values are the defaults. |
| 9 | The open world's script bridge | `public_events::BubbleScriptBridge` → `script_bridge::ScriptBridge` | **wired**, refuses until K's message | With `D1_AH_SCRIPTS=1` the open world's bridge is a `ScriptBridge` instead of `PendingBridge`. It still has no scripted run for a public bubble, so `start_bundles` refuses ("no scripted activity is running") and no event starts. Starting one needs stream K's single-public-event sidecar message. Protocol 1 has no per-event message, so `start_event` and `stop_event` are refused, as #319 states. |
| 10 | A public instance's checksum | `worlds.tsv` `public_checksum` (#314) → `global_activity_state+0x5C` | **wired** | `generated::read_public_checksums` reads it (with `D1_AH_WORLDS`), and `PublicHost::with_public_checksums` holds it. On a join that names a mission host, `state.rs` publishes the world's PUBLIC checksum instead of the mission's PRIVATE one, and logs both. This is the client's own index reading (#316). It is not measured on the wire. |
| 11 | Matchmaker ↔ public registry | `d1_mp::matchmaking::PublicInstances` (#312) over `public_registry` (#316) | **wired** | `impl PublicInstances for DestinationInstances`: G's five-line impl, now that d1-mp is a dependency of the activity host. |
| 12 | The public placement rule | H's `place_in_public` vs G's `PublicRegistry::place` | **one rule** | Emptiest first, ties to the oldest: G's rule, which G records as Bungie's stated design. `PublicRegistry::placement_order` is the rule, and `place` takes its first entry. The trait has a new provided method, `PublicInstances::placement_order`, which `place_in_public` follows. G's impl overrides it with the registry's own order, and its default is the same rule (`emptiest_first`). H's "fullest first" test now asserts the emptiest. The captures do not show which rule the live matchmaker used. |
| 13 | Public placement across processes | the responder's 42 bubble search → the registry | **wired** (O2), placement only | `D1_AH_PUBLIC_INSTANCES_FILE` (both servers): the host writes the registry's instances there on every arrival and leave (`public_registry::sync_with_responder`). On a slot-1 search the responder runs `place_in_public` over that snapshot (`public_feed::FilePublicInstances`, the registry's rule) and writes `reserve HOST SLOTS` to `<file>.reserve`, which the host applies before the next arrival, so the fireteam's slots are held. Open: the 43 answer still lists the clients' own advertisements; which advertisement is which host instance is not known from their bytes. The bubble id carries no destination, so a region id two destinations share matches both. |
| 14 | Identity | d1-mp's `MemberIdentity` in the activity host | **wired** (O2) | The registry keys members by the whole 40-byte identity (`Arrival::character: CharacterRef`, `public_host::identity_of_join`), so two characters of one console are two members. The open world still keys by the first 8 bytes internally, but its grants go out with the full identity (`CharacterRef::for_key` over the identities the host saw join). With `D1_MP=1` the responder also checks the identity's account (`+0x10`), a stopgap until each connection has its own SOIDs. |
| 15 | Playlists | `encounter_framework::matchmaking_playlists` (#313) → `d1_mp::Matchmaker::add_playlist` | **wired** (O2) | The host side writes them to `ps4/fixtures/56578/activities/matchmaking-playlists.tsv` (a test fails when it is stale; `D1_REGEN_PLAYLISTS=1`). With `D1_MP=1` and `D1_AH_ACTIVITY_FRAMEWORK`, `mp::init` adds every row. |
| 16 | Vault instances | the vendor state's `account_instances` → the store (#309's gap) | **wired** | `VendorCommit.slot_instances` and the record's `account.slot_instances`, filtered to occupied slots. `vendor_seed` hands them back, so a vault item keeps its instance across a save and a restart. A record written before the field reads with none. |
| 17 | The command file itself | every writer → the responder's poller | **fixed** | The poller used to read the file and then rename it, so a line appended in between was lost. It now renames first and reads the `.taken` copy after 50 ms. A writer's next line starts a new file. |
| 18 | One profile, two servers | `full-game.env` → each server's knob parsers | **tested** | See [The profile test](#the-profile-test) below. |
| 19 | The story hand-off | the bridge's `activity_hand_off` (stream K) → the store | **wired** (O2) | `handoff CHARACTER ACTIVITY_INDEX from=HASH hash=HASH`. The responder keeps the next activity (`characters[..].next_activity`, and the state file), writes the completed activity's completion into the profile's current-activity field [6] (`+0x2C20`/`+0x2C38`) and pushes it. That is the field the client selected 165 from once 164 was recorded complete there ([29 October](ps4-56578-continue-tower-2026-10-29.md)); the step-driven host never recorded 165, and ours went to orbit. Not seen in the game. |
| 20 | One completion path | a scripted `grant ... completed source=script` → the store's `complete N` | **wired** (O2) | The integration review's finding 6: the grant now also runs the store's completion (field [6], the rewards table's subs such as 164's CONTINUE gate, its XP topped up since the start, unlock flags, reputation). The manifest's XP lines stay out of the grant (`xp_from_manifest` off), so XP is paid once. Open-world grants are not completions. |
| 21 | mission_pact's thread failure | the sidecar on content | **not ours** | A 32-bit FNV-1 collision that BullShcript's member lookup accepts: `fnv1("amb_smpl_hive_center01.sq_hive_entrance_thrall01.place[0]")` = `fnv1("amb_smpl_fallen_command01._trigger_volume")` = `0xB2BD1A98`. `place[0]` binds that trigger volume (a real content row of an ambient bundle), so `place()` calls a table. The fallback `resolveByBundleNamespace` (`host/bound/bindings.ts`) matches rows by hash alone. A one-line upstream fix (skip a hash hit whose row has another name) removes it, and nothing else changes across the 63 activities (tested on a throwaway copy, not committed). Reported for Blam Network; no workaround here. |

## The function stream K calls

**`d1_activity_host::reward_forward::GrantForwarder::forward`**:

```rust
forwarder.forward(
    GrantTarget::Identity(character.identity.clone()), // the client's 40-byte identity
    activity_hash,                                      // the launched activity's manifest hash
    GrantOutcome::Completed,                            // or Incomplete / Abandoned
    GrantDetails { source: Some("script".into()), ..GrantDetails::default() },
)
```

It returns at once: the line goes to a writer thread, so a disk stall cannot hold the connection
loop. `GrantForwarder::from_env()` gives the forwarder when the knobs above are set, and `None`
otherwise. Both reward traits on `main` are implemented over it. A unified trait implements its
one method the same way and keeps `main.rs` handing it the forwarder (`ScriptConfig::rewards`,
`OpenWorldServices::rewards`). `GrantDetails` also carries `page` (the Cryptarch guide page),
`engrams` and `seed`.

**Update (stream K, [story missions](ps4-56578-story-missions-2026-11-05.md)):** the two traits are now one, `rewards::ActivityRewards`, and `GrantForwarder` implements it once over `forward`: a 40-byte `CharacterRef` goes out as the identity, the open world's `CharacterRef::from_key` as the key, with the same `source` words. The bridge calls it at lifetime 6. Its new `activity_hand_off` (cine_reveal → the Tower) is logged by the forwarder, because the control grammar has no line for it yet. The script bridge now sends `event_start` / `event_stop`, so row 9 no longer refuses a single event once a scripted run is going.

## The control lines

The responder's store command file (`D1_STORE_CONTROL`) now takes:

```text
grant CHARACTER ACTIVITY_HASH OUTCOME [page=PAGE] [engrams=1] [seed=SEED] [source=WORD]
checkpoint INDEX STEP          # #313's save (unchanged)
checkpoint INDEX -             # clear (also: clear-checkpoint INDEX)
lockout INDEX ENCOUNTER week=W
```

- `CHARACTER` is `id:` followed by the identity's 80 hex digits, or a `0x` key.
- `OUTCOME` is `completed`, `incomplete` or `abandoned`; only `completed` grants anything.
- A grant needs `D1_REWARDS=1` in the responder; without it, the grant is refused and logged.
- Both sides test the same lines: `rewards::remote` in the responder, `reward_forward` in the host.

## The profile test

`d1-activity-host/tests/full_game_profile.rs` reads `full-game.env` with its includes, the way the
launcher does. It substitutes this checkout for `${REPO}`, `${FIXTURES}` and `${PROFILES}`, and a
temporary folder for `${RUN}`, `${STAGING}` and `${BIN}`. Anything else is the lab machine's own,
the private inputs: the Blam Network checkout, Node, the content bundle export and the eboot. It
checks four things:

- **Every input from this checkout exists.** These are the tables, lists and fixtures the
  `[server]` knobs name.
- **The cross-process pairs agree:**
  - the responder's `D1_ACTIVITY_FOLLOW_FILE` is the host's `D1_AH_ACTIVITY_FOLLOW_FILE`;
  - `D1_STORE_CONTROL` is not `D1_AH_CONTROL_FILE`, since each server takes its own file;
  - the push files differ;
  - `D1_REWARDS` goes with `D1_STORE`;
  - `D1_AH_PUBLIC_HOST` goes with `D1_AH_DISTINCT_HOST_IDS`;
  - `D1_AH_SHARED_INSTANCES` goes with `D1_MP`;
  - `D1_AH_SCRIPTS` has its three inputs and `D1_AH_SENSOR_PHASE`, because the bridge is set up
    with the sensor frames and stays off without a phase.

  The grant forwarder resolves on from the profile's own variables, and its state file is the one
  the responder writes.
- **The tables load:**
  - the activity framework validates;
  - every activity in `D1_AH_WORLDS_ACTIVITIES` has a generated world;
  - the PUBLIC checksums read;
  - every public list leads with the players group and goes through the entry-only encoder.
- **The script bridge's configuration builds** from the profile, and the forwarder can be its
  rewards.

The responder's side is `player_store`'s `the_full_game_profile_turns_the_store_seams_on`. The
store and its command file resolve on, and the state file is named as the host expects. Every
`[server]` knob the responder reads appears in its startup table (`=== d1-responder environment`),
so the table shows each one as `SET`. It found one gap: the multiplayer knobs (`D1_MP`,
`D1_MM_CONFIG_FILE`, `D1_MP_ACCOUNTS`, `D1_MP_FIRETEAM_CAP`, `D1_MP_RECONNECT_GRACE_MS`) were read but
not in the table. They are now.

## Test hygiene

Ten `d1-activity-host` tests ran the client's codec and failed without `D1_EBOOT` (the multiplayer
note's ten):
- five in `codec`;
- two in `controls`;
- one each in `listener`, `messages` and `push`.

They now skip with `SKIP <test>: needs the owned eboot (set D1_EBOOT ...)`, through
`eboot_or_skip`, which asks `d1_oracle::codec::selected()` for the image. The crate's other eboot
tests skip the same way.

## Tests

| Suite | Result | New in this change |
|---|---|---|
| `cargo test -p d1-responder` | 160 passed, 1 ignored (the existing one) | `rewards::remote` (4: the grammar, a grant into the wallet and the kept overlay, refusals, weekly counters across grants); `player_store` (5: the new commands, vault instances and lockouts through a save, the state file, rename-then-read, the full-game profile) |
| `cargo test -p d1-activity-host`, with `D1_EBOOT` | 231 unit + 9 + 4 + 2 integration, all passed | `reward_forward` (3), `checkpoints` (1), `generated` (1), `public_registry` (1), `public_host` (2, one with the eboot), `tests/full_game_profile.rs` (4) |
| `cargo test -p d1-activity-host`, no `D1_EBOOT` | the same counts, all passed; the 11 eboot tests skip | |
| `cargo test -p d1-mp` | 30 unit + 7 scenario, all passed | the emptiest-first fill, the registry-owned order |
| `python -I -m pytest tests/test_full_game_profile.py` | 12 passed (307 subtests) | (unchanged; still green after the doc edits) |

## Open

- Row 9 above, and the open parts of rows 3b (element fields), 13 (the 43 answer) and 14 (per-connection SOIDs). Stream O2's pass is [below](#stream-o2-the-second-pass-6-november).
- Nothing here has run. The first full-game run should show these log lines:
  - `⭐ rewards: grants go to the player store through ...` and
    `⭐ rewards: forwarded to the store: grant ...` in the activity host;
  - `💾 control "grant ...": ...` and `store-control.state.json` beside the command file, from the
    responder;
  - `public lists encoded as entry-only frames` and `⭐ public instance: publishing ...'s PUBLIC
    checksum`, on a public start.

## Stream O2: the second pass (6 November)

The wires the first pass left pending (rows 3a-3c, 7a, 13-15) and the hand-off (19), on top of
stream K (#324) and the content exports (#322). Built and unit-tested only; nothing has run in the
game. New knob: `D1_AH_PUBLIC_INSTANCES_FILE` (full-game: `${RUN}/public-instances.json`; baseline:
unset). With every knob unset nothing changes.

From the integration review ([notes](ps4-56578-integration-review-2026-11-05.md)), fixed here:

- **1.** The stand-in `ps4/profiles/script-activities.txt` scripts 165 as a cinematic with its two
  phase regions (stream K's generated table already did); the profile test now checks both.
- **2.** The contained boot starts the loopback responder without `D1_STORE*`, `D1_MP`,
  `D1_REWARDS` or the public-instance file, so one process owns the store and its command file.
- **3, 14, 25.** With the store on, every vendor op starts from the store's profile, wallet, refund
  records and next item SOID (`vendor::sync_from_store`), and `publish` waits for the last profile
  push to be taken before writing its own.
- **4** (stopgap). With `D1_MP=1` a grant's identity must also name this responder's account.
- **5.** `full-game.env` sets `D1_SOCIAL_STOCK=0`.
- **6.** One completion path (row 20).
- **7, 13.** Row 14 and row 3a.
- **19.** Row 7a.

Left for the next pass: per-connection SOIDs under `D1_MP=1` (4), the fullest-first directory
ranking against the registry's emptiest-first rule (8), one script bridge per shared instance (9,
10), the social-space host id (11), the side store on a refused op (12), and the review's low items.

| Suite | Result |
|---|---|
| `cargo test -p d1-responder` | 177 passed, 1 ignored (13 new) |
| `cargo test -p d1-activity-host`, with `D1_EBOOT` | see the pull request |
| `cargo test -p d1-mp` | 31 unit + 7 scenario (1 new) |
| `python -I -m pytest tests/test_full_game_profile.py` | 12 passed |
