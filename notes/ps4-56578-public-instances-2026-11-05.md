# PS4 56578: public instances and the bubble host (5 November 2026)

Stream G: public spaces. That covers Restoration's public start (activity 166), patrol zones, public events and the social spaces. This note reads every public instance in the private live captures (Xbox 360, build 51539) host by host. It then describes what `d1-activity-host` now builds from them. **Nothing here has run in the lab or the emulator.** It is built and unit-tested only, and it is off by default (`D1_AH_PUBLIC_HOST=1`).

The captures stay private. Only derived facts are kept here and in `ps4/fixtures/56578/public`: message types, timings, record names and counts, and group hashes. No tokens, account ids, names, addresses or values. `tools/ps4_setup/d1-public-fixtures.py` regenerates the fixtures from the captures.

## Naming: the PAH is the mission host

In our notes "PAH" names the host the client starts first. That host names the mission in its join incident, receives every slice set, and alone serves region records. It is the mission host in the GDC 2015 talk's terms. Bungie's *bubble activity host* is the other kind of connection: one per public instance, beside the mission host. The GDC note's "What this means" section maps the PAH to the bubble host, but the captures disagree (a pointer has been added there).

## How the client tells the two apart

The client says which kind of host it wants in its join request (udt 3). Below are all 20 joins of five captures, decoded with the 56578 codec:

| join request | mission host | public instance | social space |
|---|---|---|---|
| `+0x38` session type | 3 | 3 if the client starts it, 2 if it joins one | Tower 2 (joined), Reef 3 (started) |
| `+0xC8` | 0 | the mission host's id | 0 |
| `+0xD8` fireteam id | set | 0 | set |

So a join that names a mission host at `+0xC8` is a public instance. This is the rule `public_host::role_of_join` applies.

## What a public instance serves

**No region records.** Every live public instance's membership had region id `0xFFFFFFFF` in all 40 slots, from its first membership on. Our mission host's "no record" (`D1_AH_SLICE_TOKEN=0`) is all zeros. Those are different bytes, so a public instance clears the slots to `0xFFFFFFFF` (`clear_region_records`). Its membership update numbers start at 2, like a mission host's.

**Its own sensor list.** Each list leads with group **`0xEAAF16E2`**, which holds 16 records of kind `0xC` (player objects). That is true of every public instance on every destination. Our mission worlds' `0x4786C0E0` is never served on a public instance, and **no list has a lifetime-phase record** (kind `0x10`). The region's bundles follow. Restored scripts exist for some of them, and the group hash is the script's `@activity_bundle`. Their headers name an ambient layer (small encounters, patrol beacons, music), a public-events bundle, chests, resource nodes and, on Venus, Taken invasions.

| Destination | Public instances read | Regions (bundles after the player group) |
|---|---|---|
| Earth | 15 (6 missions) | 0 (5), 48 (4), 56 (4), 88 (4), 160 (6) |
| Moon | 11 (7) | 8 (4), 16 (4), 48 (4) |
| Venus | 10 (5) | 32 (6), 72 (5), 80 (5), 120 (5), 160 (4), 224 (3) |
| Mars | 15 (5) | 8 (5), 48 (5), 112 (6), 144 (5), 192 (6), 208 (4) |
| Dreadnaught | 2 (Patrol the Dreadnaught) | 8 (5) |
| Plaguelands | none (Rise of Iron never came to the 360) | |

**A region is one place on its destination, whichever mission is there.** Every pair of freshly started instances of one region served the same groups:
- Earth region 56 in Restoration, The Dark Within, The Warmind, The Last Array and Siege of the Warmind;
- Moon region 16 in The Sword of Crota, Shrine of Oryx, Chamber of Night and The Summoning Pits;
- Mars region 8 in four missions.

So the fixtures are keyed by (destination, region), not by mission. `bubbles.tsv` has every group with its record counts and the captures it was seen in.

**When the list comes.** On a freshly started instance the host sent its authority table at about 0.5 s (stamp 11-12). The full list followed 0.8-3.7 s after the join (stamps 14-38). The region is not in the join. The client names it in its first udt 21 on that connection, in the slice-set records (TARGET `+0x158`, CURRENT `+0xB8`), 0.7 s after the join in Restoration. Our host picks the region's list from that report.

**Lists grow.** A fresh instance's first list is not all it serves. Later frames name new records: events' squads, loot. Each fixture keeps those records as `# later +N s` comments, which are not served.

**Incidents.** Most public instances sent no udt 18. The client sent its incidents to the public instance while it was in that bubble: 39 to Restoration's region-56 instance, with none to the mission host until it left the bubble at 395 s.

## Timings

- **Region marked public → the client joins its instance:**
  - Restoration: 12.5 s;
  - The Dark Within: 12.4 s;
  - Patrol Venus: 7.5 s;
  - Patrol the Dreadnaught: 12.4 s.
- **Join → the mission host advertises it:** 1.2-1.5 s in all five cases.
- **GDC 2015's airlock:** pre-cache at 15 s before the swap, matchmake at 10 s, and start standalone at 5 s (`public_registry::swap_stage`).
- **Instances outlive a visit.** After leaving a bubble, the client stays connected to its instance until the mission ends (udt 14 to both hosts at 552 s in Restoration).
- **Joined instances had been running a while.** The client's join found instances 12.4 s to 28.6 minutes old (first stamps 124 to 17,142 at 10 Hz).
- **Returning can start a new instance.** In Patrol the Dreadnaught, the client went back to region 8 60 s later and started a second instance (`..9720`).

## The bubble swap and the slot handover

This is Restoration's exit from its region-56 instance (times in seconds):

| Time | What |
|---|---|
| 395.47 | the client sends udt 32 `abdicate_authority` and udt 17 to **both** hosts, and two udt 25 `abandon_entity_slots` to the public instance |
| 395.73 | the public instance answers with two udt 23, each echoing a set |
| 395.80 | the client sends two udt 26 |
| 396.05 | the public instance sends two udt 24 |
| 395.86 | the mission host sends an incident; from 396.1 the client reports its incidents to the mission host |

Here the host answered each 25 on its own. A Guardian Rises' host once merged two (`public_host::merge_slot_sets`). The existing slot-handover answer (`state.rs`) is switched on for every public instance.

The Dark Within's region-48 instance also ran a different exchange, at 457-460 s:
- an authority reset: udt 42 `advance_replication_epoch`, then 27, 28, 29, 30 and 31 (reset, query and the per-bubble answer);
- then 23, 26 and 24;
- earlier, udt 12 `request_peer_reservation` (354 s, 423 s) and udt 13 release (458 s). These are the fireteam slot reservations GDC 2015 describes, made by the client.

**None of this is served yet.**

## Public events

The one event start on the wire is Defend the Warsat in The Dark Within:
- the client, in private region 64, joined region 0's instance (the Array) at 746 s;
- the join was type 2: another player's instance, 12 s old;
- its event bundle is `0xD2C22DCF`, which the restored scripts name as the Array's public events.

The events run on the bubble's host, as GDC 2015 says. When the next event would come is not in any capture: Bungie's schedules were server-side.

## Patrol and the social spaces

**A patrol is a mission host with the same public instances.** Patrol Venus's mission host (`..69db`) serves its own patrol group (`0x7CF2807B`: 60 objective records and 59 of kind `0x23`, `[I]` the patrol missions). Its region-72 instance served exactly the groups Eye of a Gate Lord's region-72 instance did. Patrol the Dreadnaught is the same, with group `0x78708B21` (47 and 22).

**The social spaces are hosts of their own.** A Tower or Reef join is a mission-type join (`+0xC8 = 0`, with the fireteam). The Tower was joined (type 2) into an instance about 98 minutes old. The Reef's Vestian Outpost was started (type 3). Both serve player group `0x40AEBF90` with 30 player objects, then their cast: the Tower its plaza, hangar and two cinematic records; the Reef `0xE525F723`, which the restored scripts name as its people. Our host already serves the Tower from `world-table-tower.txt`. What stream G adds is the shared instance: a join into a social space's world is registered under that space in the registry, so a second player can be placed into it.

## What was built (`d1-activity-host`)

| Module | What |
|---|---|
| `public_registry.rs` | Instances keyed by (destination, region) or social space. Each player is in one instance. Fireteams land with their members, and present members reserve slots for absent ones. The emptiest instance with room for the whole fireteam fills first (ties go to the oldest), otherwise a new instance starts. The cap is 6 per bubble (GDC 2015). Placements are type 3 (starts) or 2 (joins). An empty instance retires after a PLACEHOLDER 300 s. `attach` registers an instance no matchmaker placed. |
| `public_host.rs` | The join-request role, the region pick from udt 21, the region lists (`load_public_table`), cleared region records and slot-set merging. `FrameEncoder` builds each region's frame once at setup. |
| `public_events.rs` | `BubbleScriptBridge`, the interface to stream A2's script bridge (`RecordingBridge` stands in until it lands). One event at a time per bundle, after a quiet spell. Participation comes from engagement reports, and the end goes through the reward call. The event schedule is a labelled `PLACEHOLDER_SCHEDULE`. |
| `open_world.rs` | Destinations and social spaces, with the regions marked public on each. Bundle roles come from the restored scripts' headers, or `[I]` from the loot shape. Also patrol beacons and missions (`PatrolBoard`, PLACEHOLDER beacons), chests and resource nodes opened once per character (`LootField`), and `ActivityRewards::grant_activity_rewards(character, activity_hash, outcome)` for stream M. |
| `state.rs` hooks (marked `stream G`) | A join that names a mission host serves no region records, no mission steps and no frame until the region is named. It also turns on the slot handover. The udt 21 report installs the region's frame. Peer leave and connectivity failure free the registry slot. |
| `public_host.rs`, open world | Once an instance's place is known, its open world starts once and is shared by every connection to that instance. The bridge starts the region's bundles, and each bundle with a schedule rotates its events on the host's tick. The region's chests and resource nodes (the kind-`0x4` records of the bundles the scripts name) open once per character after a PLACEHOLDER 60 s. Patrol beacons are per region. The bridge reports engagement and the event's end, and loot and patrol go through `ActivityRewards`. A social-space join registers the space's shared instance. |
| `main.rs` | `D1_AH_PUBLIC_HOST=1` loads `D1_AH_PUBLIC_TABLE` (default `ps4/fixtures/56578/public/public-table.txt`) and shares one registry and one set of instance worlds between all connections. It needs `D1_AH_DISTINCT_HOST_IDS=1`. Until streams A2 and M land, the bridge is `PendingBridge`, which refuses (so no event starts), and rewards go to `LoggedRewards`, which logs them and gives nothing. |

Tests: the registry's fill, cap, reservation, leave and retire rules; the join roles; the cleared records; all 23 fixtures loading with the live groups; the region pick; the event lifecycle; patrol and loot. `tests/public_instances.rs` runs two fake clients, each with its own mission host, into one region-56 instance:
- the mission host's advertisement echo;
- the public list served on both connections;
- the slot handover;
- a leave freeing the slot.

A second test runs the open world of one region-48 instance through two connections:
- the four bundles start once;
- the events bundle starts one event although both connections tick;
- both engaged players are rewarded at gold;
- each chest opens once per character;
- a join into the Tower registers its shared instance.

## Not there yet

- **The frames.** A public list has no lifetime record, so it needs an entry-only `sensor_auth_update`. That is stream A2's `d1_oracle::sensor::SensorEntryFrame`, not merged yet. Until it is, `encoder_pending()` names every list unservable, and the host serves the public instance no list. Plugging it in is one closure in `main.rs`.
- **The public instance's activity state.** `global_activity_state` for a public instance:
  - by the client's own index it should carry the world's PUBLIC checksum (`scenario_record+0x10`), since the activity client is `[AC PUBLIC …]`;
  - `worlds.rs` (stream E) holds only PRIVATE checksums, so for now the host publishes the mission's;
  - udt 1 of build 51539 does not decode with the 56578 codec, so the live values are unread.
- **Values.** The lists carry names only, and every value is the default. Two kinds in the Dreadnaught, Earth 160, Mars 144 and Moon 16 lists (`0x16` and `0x24`, 11 records) are in no native list yet, so they are commented out.
- **Matchmaking.** The search answer (services message 0x2a) should come from the registry. A second client needs the services host to hand out a running instance's id (0x10/0x11) and the connection to present it. That is stream H (`d1-responder`). Today a single client's connection registers its own instance.
- **The authority-reset exchange** (udt 27-31, 42) and the peer reservations (12, 13).
- **Event schedules, patrol beacons and loot timings are placeholders.** The real schedules were never public. The beacons' goals and the events' activity hashes come from content once read. Event scoring (bronze, silver, gold) belongs to each event's script.
- **Lost sectors are not a Destiny 1 feature** (they came with Destiny 2). Their open-world equivalents here are the chests and patrol missions.
- **Social spaces.** The Tower and the Reef fixtures are reference lists, and the Tower is still served by `world-table-tower.txt`. Felwinter Peak's world name and lists are unknown: no capture has it. So is the social-space player cap: `PLACEHOLDER_PLAYER_CAP` is 16, while each space carried 30 player objects.
