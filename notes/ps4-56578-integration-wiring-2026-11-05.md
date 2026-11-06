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
| 3a | | item hash ↔ 56578 content index | **pending** | Only Glimmer is mapped. Its content index is whatever the account's slot 308 holds, from the client's bucket table. Every other currency (Legendary Marks, Silver Dust, materials) stays in the kept overlay and does not reach the client. This needs the content-index table, as the store note says. |
| 3b | | the overlay's item instances and grids | **pending** | Engram drops (`engrams=1`) and new instances last one grant. They need an item-element writer with the client's indices (`item_instance::write_element`). |
| 3c | | a wallet change without the vendor state | **pending** | With `D1_VENDOR_SALES` unset, the new quantities stay in the store. The account object is not re-encoded from its slots, so no push is sent. |
| 4 | Checkpoint save | `checkpoints::ControlFileCheckpointStore` (#313) → `set_checkpoint` | wired (#313) | unchanged |
| 5 | Checkpoint clear | `checkpoint INDEX -` → `clear_checkpoint` | **wired** | The responder reads `checkpoint N -` (and `clear-checkpoint N`) as a clear. #313's store already sent it. |
| 6 | Weekly loot lockouts | `mark_looted` → the store | **wired** | A new record field, `characters[..].lockouts`: activity index → encounter → reset week. A new command, `lockout INDEX ENCOUNTER week=W`. `PlayerStore::{lockout_week, set_lockout}`. The host sends it on `mark_looted`. |
| 7 | Checkpoint read-back at launch | the store → the activity host | **wired** | After each batch of commands, and at sign-on, the store writes `<control>.state.json`: the character, its checkpoints, its lockouts and its last grants (`player_store::state_file_for`, `state_json`). `ControlFileCheckpointStore::load` and `is_looted` read it when the mirror has nothing. `ControlFileCheckpointStore::from_env` takes the character from it. |
| 7a | | a running `ActivityRun` using the store | **pending** | Nothing drives an `ActivityRun` on the wire yet (#313's note). The host resolves the store at startup and logs where checkpoints go. The run that calls it belongs to streams J and K. |
| 8 | The public host's lists | `public_host::FrameEncoder` → #319's `SensorEntryFrame` | **wired** | `public_host::entry_frame_encoder(codec, gen)` builds gen, no authority, `init = 1` and the region's entries. It goes through the host's `AhWireCodec::encode_sensor_entries`: `NativeCodec` (d1-rsat) under `D1_RSAT_NATIVE=1`, the client's own codec otherwise. `gen` is `D1_AH_SENSOR_GEN` (default 0). `main.rs` no longer passes `encoder_pending()`. No public list has been served to a client yet, and their values are the defaults. |
| 9 | The open world's script bridge | `public_events::BubbleScriptBridge` → `script_bridge::ScriptBridge` | **wired**, refuses until K's message | With `D1_AH_SCRIPTS=1` the open world's bridge is a `ScriptBridge` instead of `PendingBridge`. It still has no scripted run for a public bubble, so `start_bundles` refuses ("no scripted activity is running") and no event starts. Starting one needs stream K's single-public-event sidecar message. Protocol 1 has no per-event message, so `start_event` and `stop_event` are refused, as #319 states. |
| 10 | A public instance's checksum | `worlds.tsv` `public_checksum` (#314) → `global_activity_state+0x5C` | **wired** | `generated::read_public_checksums` reads it (with `D1_AH_WORLDS`), and `PublicHost::with_public_checksums` holds it. On a join that names a mission host, `state.rs` publishes the world's PUBLIC checksum instead of the mission's PRIVATE one, and logs both. This is the client's own index reading (#316). It is not measured on the wire. |
| 11 | Matchmaker ↔ public registry | `d1_mp::matchmaking::PublicInstances` (#312) over `public_registry` (#316) | **wired** | `impl PublicInstances for DestinationInstances`: G's five-line impl, now that d1-mp is a dependency of the activity host. |
| 12 | The public placement rule | H's `place_in_public` vs G's `PublicRegistry::place` | **one rule** | Emptiest first, ties to the oldest: G's rule, which G records as Bungie's stated design. `PublicRegistry::placement_order` is the rule, and `place` takes its first entry. The trait has a new provided method, `PublicInstances::placement_order`, which `place_in_public` follows. G's impl overrides it with the registry's own order, and its default is the same rule (`emptiest_first`). H's "fullest first" test now asserts the emptiest. The captures do not show which rule the live matchmaker used. |
| 13 | Public placement across processes | the responder's 42 bubble search → the registry | **pending** | The registry lives in the activity host and the 42 searches are answered in the responder (by `Directory`, from client advertisements). There is no feed of the host's instances to the responder, so `place_in_public` still has no runtime caller. |
| 14 | Identity | d1-mp's `MemberIdentity` in the activity host | **wired** for rewards | The forwarder sends the 40-byte identity. The responder reads its character SOID at `+0x18`. G's registry still keys characters by the identity's first 8 bytes. Rekeying it by the SOID needs evidence that the client fills `+0x18` with the character it plays. |
| 15 | Playlists | `encounter_framework::matchmaking_playlists` (#313) → `d1_mp::Matchmaker::add_playlist` | **pending** | The matchmaker runs in the responder, which does not load the activity framework's data. Wiring it needs that data in the responder process, or a feed from the host. |
| 16 | Vault instances | the vendor state's `account_instances` → the store (#309's gap) | **wired** | `VendorCommit.slot_instances` and the record's `account.slot_instances`, filtered to occupied slots. `vendor_seed` hands them back, so a vault item keeps its instance across a save and a restart. A record written before the field reads with none. |
| 17 | The command file itself | every writer → the responder's poller | **fixed** | The poller used to read the file and then rename it, so a line appended in between was lost. It now renames first and reads the `.taken` copy after 50 ms. A writer's next line starts a new file. |
| 18 | One profile, two servers | `full-game.env` → each server's knob parsers | **tested** | See [The profile test](#the-profile-test) below. |

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

- Rows 3a, 3b, 3c, 7a, 9, 13, 14 and 15 above.
- Nothing here has run. The first full-game run should show these log lines:
  - `⭐ rewards: grants go to the player store through ...` and
    `⭐ rewards: forwarded to the store: grant ...` in the activity host;
  - `💾 control "grant ...": ...` and `store-control.state.json` beside the command file, from the
    responder;
  - `public lists encoded as entry-only frames` and `⭐ public instance: publishing ...'s PUBLIC
    checksum`, on a public start.
