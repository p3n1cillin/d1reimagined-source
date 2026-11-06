# Live captures: patrol, public events, the Tower and The Dark Within (6 November 2026)

The Discord member who shared the first captures recorded three more on 5 November 2026 (build 51539, the same
plugin as [the first 52](d1-live-captures-2026-11-05.md)):

| Capture | Length | What was done |
|---|---|---|
| The Dark Within | 5 min | the mission, with no public event |
| Patrol the Cosmodrome | 51 min | beacons (Assassination, Taken Blight, Survey twice, Assassination, Fetch, a Secret one with hidden steps), the Drill Site public event, a triple event (Warsat, Taken Corruption, Urzok the Hated at once), Taken champions |
| The Tower | 20 min | every vendor and kiosk, engram decrypts, purchases, the Speaker's exchange, a Dead Orbit donation and package, a Vanguard rank-up package, bounties turned in and picked up, dismantles, equips and upgrades, the Fan and the Jukebox |

**The captures stay private.** They hold the player's sign-on token and account ids, other
players' identities, session keys and addresses. Nothing from them is in this repository except
what is below: message ids, field layouts, timings, counts, quantities the game credited, and
content hashes. Times are seconds from the start of each capture unless a row says otherwise.

## How they were read

The services traffic (BAP 10/11 investment messages and the QueueZ 123 pushes) had never been
decoded from a capture. Three tools now do it (this PR):

- `d1-srcp.py investment CAPTURE`: every investment request and reply, decoded from bit 16 (after
  the service id) by the native codec, with its fields named from the RTTI export
  (`ps4/fixtures/56578/rtti/services.tsv`, `fields.tsv`), and the QueueZ pushes in between.
- `d1-srcp.py queuez CAPTURE`: each push's families, object types and lengths.
- `d1-srcp.py stacks CAPTURE`: the account and profile slots each push writes, with the quantity
  and its change.
- Underneath, `d1-rsat decode-batch` (lines `TAG HEX [AT [FILL]]`). A QueueZ object on this build
  is sparse: it carries only the fields that changed. Decoding it twice, over an image of 0x00 bytes
  and one of 0xFF, shows which bytes it wrote. A walk that fails part-way keeps what it read
  (`partial`).

**What decodes with the 56578 schemas:**

| Traffic | Result |
|---|---|
| Investment requests and replies | 541 of the 602 in the three captures decode exactly: the walk ends in the last byte, after a has-payload bit of 0. The other 61 are the periodic 702 requests: their walk ends 10,356 bits short of the 2,403-byte body, because 702's argument struct differs on 51539 |
| Account object, 51539 type checksum `0xB5DF2907` | the 56578 account `0x808018C7`. Bodies that also carry a later field stop past field [6], so its 326 inventory slots (`+0x4C8`) are still read |
| Profile object, `0xEB4B8439` | the 56578 profile `0x80801743` walks only part-way: slots 0 to 319 read sensibly, and everything after them differs on 51539 |
| Item object, `0x0EB6AC38` | does not decode (the item layout changed in Rise of Iron). Its length is still useful: **0 bytes is a deletion**, about 155 bytes a new item, 15 to 38 bytes a change (XP, a node) |
| Families 0 (`0x30D274D2`) and 2 (`0xE274E8AF`) | not read |

**Item numbers are not 56578's.** A capture's item content indices match 56578's for some items
(Glimmer 869, Heavy Ammo Synthesis 714, House Banners 738, the uncommon snipers 5039 and 5040, the
Kestrel Class EX ship 3489) and not for others (a Gunsmith weapon at 5005 where 56578 has 5030).
So no item is named below from its index alone. Material slots are named by the trades that moved
them (for example, the slot the Speaker's exchange takes two of for one Mote of Light).

**Vendor and sale numbers are 56578's.** All 180 purchases name a vendor row and a sale that match
56578's tables (`vendor-index.tsv`, `vendor-sales.tsv`) by what was bought and what it cost. Nine
prices were confirmed to the Glimmer or the item; three differ (below).

## Settled placeholders

| Subsystem | File or table | Placeholder | Live value | Evidence |
|---|---|---|---|---|
| Dismantle yields | `d1-responder/src/inventory_ops.rs` `PLACEHOLDER_YIELDS` (changed here) | rare armour 50 Glimmer; legendary 2 Marks (+1 Weapon Parts on weapons); uncommon not listed | uncommon armour **25 Glimmer + 1 Armor Materials**; uncommon weapons **26 to 31 Glimmer + 1 Weapon Parts**; rare armour **2 Armor Materials**; rare weapons **1 Weapon Parts**; legendary armour **3 Legendary Marks + 3 Armor Materials**; legendary weapons **3 Marks + 3 Weapon Parts + 88 Glimmer** | Tower, 12 dismantles (402), the push that answered each ("Dismantle yields") |
| Legendary Marks for dismantling | `d1-responder/src/rewards/placeholders.rs` `DISMANTLE_MARKS_*` (O2, **not changed**) | weapon 3, armour 2 | **3 for both** | Tower 142.3 s and 364.3 s: slot 310 +3 each |
| Rare and lower gear | `rewards/mod.rs` rule `currency.dismantle.other` (O2, **not changed**) | "gives nothing" | uncommon gives Glimmer and one material; rare gives materials; one free Gunsmith weapon gave nothing | as above |
| Patrol beacons | `d1-activity-host/src/open_world.rs` `PLACEHOLDER_BEACONS` (changed here) | three on Earth region 48 | six live: Assassination at 56, Taken Blight at 96, Survey at 56 and 96, Assassination and Fetch at 160 (goals still placeholders) | patrol: each beacon's interaction went to that region's instance ("Patrol beacons") |
| First public-event delay | `d1-activity-host/src/public_events.rs` `PLACEHOLDER_SCHEDULE` (changed here) | 600-900 s quiet | **485 s and 494 s** after the instance started. Later spells are unknown, and the same window stands in for them | patrol, the two events' instances ("Public events") |
| Event time limits | the same table | 300 s | not settled: the events finished in 121 s (gold) and 265 s (silver), and none failed | patrol |
| Region 56's event bundle | `open_world.rs` `NAMED_BUNDLES` (changed here) | `0xED3C5B0A` unnamed | the public-events bundle: Drill Site ran its objective there | patrol host `..dcbe` |
| The Tower's served list | `ps4/fixtures/56578/public/social-tower.txt` (regenerated here) | 120 records from 40 s of A Guardian Rises | **252 records over 1,187 s**: a fifth group, `0x8D3F1166` (64 records), and the hangar group `0x19D43443` grows from 11 records to 79 | Tower host `..dce0` |
| Earth region 96 | `ps4/fixtures/56578/public/public-earth-r96.txt` (new) | none | a fresh instance's 117 records, with groups `0xEB6A66ED`, `0x7C2845CF`, `0xC5394763` and two loot groups | patrol hosts `..dc97`, `..dd0e` |
| Social-space cap | `open_world.rs` `SocialSpace::PLACEHOLDER_PLAYER_CAP` | 16 | **not settled.** The live Tower host's membership held at most **5** peers, the client included (20 member slots, 30 player objects), falling to 2 | Tower, udt 11 occupancy mask `+0x502C` |
| Bounty turn-in | `notes/ps4-56578-social-and-vendors-2026-11-05.md` "402 or 904?" | open | **402** with action index 1 on the bounty item | Tower 1113-1117 s |
| Faction and rank-up packages | the same note: "the delivery to Lost Items" | open | **904 `vendor_interact` at the vendor**. The contents land straight in the character's inventory, not the Postmaster | Tower 344.8 s (Vanguard) and 1056.4 s (Dead Orbit) |
| 405's two bytes | wire catalogue: "owner/bucket meaning U" | open | to the Vault: byte 0 = **26 (the Vault's vendor row)**, byte 1 = **1**; a stack moves with instance 0, its content index and its quantity | Tower 187.1 s |

## The Tower: every exchange

**One shape for every service.** In every exchange, the client sends one BAP 10, the server
pushes QueueZ family 4 (the changed objects), and then sends the BAP 11 reply. In each reply the
shared result `R` is status 0, and its i32 value is the **family 4 version after the change**. For
example, the 901 at 76.7 s is answered by push version 590 and reply value 590. 402 is the
exception: its replies carry value 0. Pushes and reply arrive within 0.1-0.3 s of the request.

| Exchange | Request (service: fields) | The push before the reply (family 4 objects, bytes) | What changed |
|---|---|---|---|
| Decrypt an engram (7×) | 901 `{vendor 17 (Cryptarch), sale 68/73/78/87/91, u64 stamp}` | item deleted (0), new item (155), account (24), profile (4,388) | the engram leaves; the decrypted item takes a slot of its bucket. The account carries only field [8] |
| Dismantle (12×) | 402 `{I {flag 1, instance, i16 content}, action 1}` | item deleted (0), account (24-299), profile (230-285) | yields below |
| Buy (vendor) | 901 `{vendor, sale, stamp}` | account (212-532), profile, new item (155) when instanced | Glimmer down by the price (slot 308) |
| Kiosk (Emblem 40, Shader 31) | 901 `{40 or 31, sale}` | profile, new item | a free item; no currency moves |
| Speaker's exchange (137×) | 901 `{29, sale 2, 3 or 4}` (59, 36 and 42 times) | account and profile only | sale 3: 1 Ascendant Shard and 50 Glimmer for 1 Ascendant Energy; sale 4: 2 Shards for 1 Mote of Light; sale 2: 1 Energy and 50 Glimmer for 1 Shard. These are exactly 56578's sale costs |
| Dead Orbit donations (19×) | 901 `{13, sale 101/103/108}` (16, 1 and 2 times) | profile | 5 Motes of Light, 25 Armor Materials and 1 Heavy Ammo Synthesis a time (56578's costs) |
| Faction package (Dead Orbit) | **904** `{13, i16 5, i16 0, i32 114}` | account (258), profile (551), new item (155) | +500 Glimmer, a weapon in the character's inventory, +4 Motes of Light, +4 of material slot 6 |
| Vanguard rank-up package | **904** `{11 (Warlock Vanguard), 19, 0, 211}` | account (259), profile (549), new item (155) | +500 Glimmer, a legendary weapon in the inventory (dismantled at 364 s for legendary yields) |
| Bounty pick-up | 901 `{7 (Bounty Tracker), sale 100, stamp}`, price 0 | profile (260), new item (155) | the bounty takes a bounty slot |
| Bounty turn-in (3×) | **402** `{I, action 1}` on the bounty | the bounty deleted, **10 item updates of 15 bytes each** (XP to the 10 equipped items), profile (4,658-4,700) | +1 Mote of Light on one of the three. XP and reputation are in parts of the profile not read |
| Equip (8×) | 403 `{u64 instance}` | profile (388), a family 0 push | slots swap within the bucket |
| Vault transfer | 405 `{26, 1, 0, content 738, quantity 1}` | account (239), profile (230) | account slot 130 (Vault, general) holds the stack |
| Gunsmith | 904 `{25, 2, 0, 242}`, then a free 901 `{25, 374}` | item deleted and an item added; then a new weapon | the weapon-test exchange; the test weapon later dismantled for nothing |
| Shipwright | 904 `{22, 20, 0, -1}`, then 901 `{22, 38}` | profile (4,362), new item | a free item, then a ship for 2,250 Glimmer |
| Upgrade: activate a node | 801 `{u64 item, i8 node 0}` | account (532), profile (228), item (38) | **500 Glimmer, 10 Weapon Parts and 10 of material slot 5** |
| Upgrade: change a node's step | 802 `{u64 item, i8 14}` | profile (20), item (38) | nothing spent |
| The Fan, the Jukebox | **no services message** | none | client incidents only: one `interaction` (2805) at 929.9 s, and `cinematic_started` / `cinematic_finished` (2120 / 734) at 64.1, 95.5 and 585.9 s. The Tower's cinematic records (`0xF2D88BD0/6/0-1`) changed at those times |
| Opening a vendor | **no services message** | none | no 1401 or any other request opens a vendor screen. Stock is not asked for |

702 is sent every 20-60 s and on events, 24 times in the Tower. It is a 2,403-byte request
(`0x808016CF` on 56578, laid out differently on 51539) whose first fields are floats. Its reply
value is the family 4 version, and usually a small family 4 push answers it. Its purpose is still
open.

**Prices against 56578's table.** Confirmed: Heavy Ammo Synthesis 950 (Gunsmith 147), the
Speaker's 50 per exchange, the Warlock Vanguard's sale 197 at 750, the Vanguard Quartermaster's
sale 99 at 1,500 and sale 37 at 10 Legendary Marks, the Guardian Outfitter's sales 61 (2,250) and
13 (2,225), and the Shipwright's sale 38 at 2,250. Different: Outfitter sale 71 cost **2,250**
(56578: 7,500), the Speaker's sale 106 cost **820** (56578: 610), and Gunsmith sale 374 was **free**
(56578: 2,400). Either the price changed in Rise of Iron, or the sale's ordinal moved.

### Dismantle yields

Each is the change in the account (Glimmer slot 308, Legendary Marks slot 310) and in the
profile's material stack, in the push that answered the 402. The tier is read from the yields and
from where the item came from (the build's item numbers cannot name it). Legendary gear gave Marks;
uncommon gear came from loot drops or a 750-Glimmer class vendor sale; rare gear came from engram
decrypts.

| Time | Item (where from) | Glimmer | Legendary Marks | Material |
|---|---|---:|---:|---|
| 96.6 | decrypt, armour | 0 | 0 | Armor Materials (first sight of the stack) |
| 109.2 | decrypt, armour (rare) | 0 | 0 | +2 Armor Materials |
| 122.4 | decrypt, armour (legendary) | 0 | first sight (100) | +3 Armor Materials |
| 142.3 | decrypt, armour (legendary) | 0 | **+3** | +3 Armor Materials |
| 158.3 | decrypt, weapon | 0 | 0 | Weapon Parts (first sight) |
| 171.4 | decrypt, weapon (rare) | 0 | 0 | +1 Weapon Parts |
| 364.3 | Vanguard package, weapon (legendary) | **+88** | **+3** | +3 Weapon Parts |
| 1173.7 | Quartermaster purchase, weapon (uncommon) | +31 | 0 | +1 Weapon Parts |
| 1176.6 | Gunsmith test weapon | 0 | 0 | none |
| 1187.7 | patrol drop, sniper (uncommon) | +31 | 0 | +1 Weapon Parts |
| 1189.6 | patrol drop, sniper (uncommon) | +26 | 0 | +1 Weapon Parts |
| 1197.1 | class vendor purchase, gauntlets (uncommon) | +25 | 0 | +1 Armor Materials |

`PLACEHOLDER_YIELDS` now carries these per tier, its rows marked LIVE or PLACEHOLDER. No basic or
exotic gear was dismantled, so those rows stay stand-ins.

### The social space's membership

The Tower host (`..dce0`) was joined (session type 2) into an instance whose first stamp was
24,288, about 40 minutes old. Its membership (udt 11) began at update 260 with **5 peers**
(occupancy mask `0x1F`), then fell to 4, 3, 4 and 2. The player objects show the same: group
`0x40AEBF90` kind `0xC` has 30 records, of which only subs 4-8 were updated more than the idle
pair of frames. The Tower's matchmaking configuration (BAP 43 field 4 at 13.5 s) carries small
numbers: 15, 5 and 32 in its settings record; `{9, 3, 6}`, `{9, 3, 9}` and three `{6, 2, 6}` in
its rows. Their names are not known, so they settle nothing. **The cap stays a placeholder.**

## Patrol and public events

**The hosts.** The patrol's mission host (`..dc8b`, Patrol the Cosmodrome `0x09FA9D74`) stayed on
for the whole 51 minutes. It served Patrol the Cosmodrome's own group `0x0EC11855`, which has 245
records: 55 objective records (kind `0x20`), 34 of kind `0x23`, 65 of kind `0x4` and 40 triggers.
Its lifetime records `0x0EC11855/0x10/180` and `182` held 3 and 0, and the world's
`0x4786C0E0/0x10/3` stayed 0 throughout, as the wire catalogue found for patrol worlds. Nine public
instances joined beside it:

| Host | Joined at | Region | Started or joined (instance age) |
|---|---:|---:|---|
| `..dc5d` | 34 | 56 | started |
| `..dc97` | 133 | 96 | started |
| `..dcbe` | 559 | 56 | joined (142.6 s old) |
| `..dcdb` | 878 | 48 | joined (49 s old) |
| `..dd0e` | 1115 | 96 | started |
| `..dd15` | 1213 | 160 | started |
| `..dd42` | 1718 | 88 | started |
| `..dd61` | 2015 | 48 | started |
| `..dd75` | 2288 | 0 | joined (29.8 s old) |

Every region's instance served the same groups as that region's mission-time instances in the
earlier captures. A region is one place, whichever activity the client is in. Region 96 is new.

**Other players** shared instances (`..dcbe`, `..dcdb`, `..dd75` were joined), but no other peer
was engaged in an event (the capturer's account).

### Patrol beacons

**Taking a beacon.** The client sends incident `interaction` (51539 2805) to the public instance it
is in. About 2 s later the **patrol host** sends an incident naming the beacon. In the same tick, a
new objective record appears in group `0x0EC11855`: a delta of 2,545-2,600 bits (85 for two of
them). Each beacon has its own record. **Progress** is a 50-bit delta of that record per kill or
scan tick (the Surveys tick about ten times a second while scanning). **Completion** is one udt 18
from the patrol host with two incidents, **`bounty_complete` (51539 380, 56578 462)** and a second
one, then a 21-bit and a 117-bit delta of the record. A third incident often follows within a
second.

| # | Capturer's name | Region | Interaction | Accept incident (56578) | Objective record | Completed | Completion's second incident (56578) |
|---:|---|---:|---:|---|---:|---:|---|
| 1 | Assassination | 56 | 73.2 | 4 (4) | 61 | 425.0 | 1860 (2351) |
| 2 | Taken Blight | 96 | 474.8 | 3006 (3816) | 122 | 767.9 | 2670 (3409) |
| 3 | Survey | 56 | 1050.6 | 1268 (1564) | 85 | 1167.1 | 692 (842) |
| 4 | Survey | 96 | 1191.9 | 1267 (1563) | 82 | 1271.9 | 692 (842) |
| 5 | Assassination | 160 | 1378.7 | 2168 (2745) | 13 | 1442.5 | 2243 (2840) |
| 6 | Fetch | 160 | 1455.1 | 2 (2) | 64 | 1542.0 | 1860 (2351) |
| 7 | (Secret, see below) | 88 | 1755.0 | 1356 (1679) | 156 | 1878.4 | 1816 (2288), after 449 (543) and 824 (1011) |
| 8 | not named | 88 | 1927.2 | 2169 (2746) | 14 | 2059.9 | 2243 (2840) |
| 9 | not named | 48 | 2092.0 | 4 (4) | 63 | 2316.8 | 1860 (2351) |
| 10 | not named | 0 | 2355.8 | 2476 (3151) | 45 | 2404.1 | 57 (67) |

The wire has ten accepts; the capturer named seven. The two Surveys share their completion's
second incident (692), and the two Assassinations (#1, #9) share their accept incident (4). So
the accept incident most likely names the beacon's mission and the second completion incident its
reward kind [I]. None of the incidents' names are in the catalogue. The capturer listed Fetch, then
the Secret, then the triple event:

- **#7** is the only beacon with steps between its accept and its completion: two incidents
  (449, then 824 a second later), then the completion 1.5 s after that.
- **#10's completion started a hidden chain.** 18 s after it, the patrol host sent incident 2073 and
  opened a new objective record (131). The record closed (a 149-bit delta) as the next opened, each
  with a host incident: record 126 at 2734 s (incident 322), then record 128 at 2823 s (incident
  2565). The chain finished at 2837.2 s with `bounty_complete` and 706, then 26 at 2839.7 s, with
  no interaction from the client.

Either fits "a beacon with hidden steps". #10's chain is the multi-step one, and it ran into the
triple event.

**Rewards.** Glimmer reached the account's cap of **25,000** at beacon #2's completion: it was
24,570 at #1's, and rose 430 more, so #2's reward was at least 430. From then on no Glimmer change
could show. No beacon credited a material slot. XP and reputation are in parts of the profile that
are not read.

**Loot pickups** are 601 `{source kind 5, {u32 drop id, u64 the character, i32 counter}}`. The
reply echoes zeros and the family 4 version. The client then reports `inventory_item_added`
(2280). There were 21 in the patrol: most raised one stack (profile slot 209) by one, and some
brought gear.

### Public events

| | Drill Site | The triple (Warsat, Taken Corruption, Urzok the Hated) |
|---|---|---|
| Host, region | `..dcbe`, Earth 56 (joined, another player's) | `..dd75`, Earth 0, the Array (joined, another player's) |
| Bundle and objective record | `0xED3C5B0A` kind `0x20` sub 122 | `0xD2C22DCF` kind `0x20` sub 25: the bundle Defend the Warsat ran on in October |
| Record armed (a 21-bit delta) | 880.0 s: instance age **463 s** | 2731 s: instance age **472 s** |
| Event start (a 2,533 / 2,727-bit delta) | 911 s: age **494 s** | 2744 s: age **485 s** |
| `public_event_engaged` (51539 342, 56578 **415**) | 969.4 s: the player arrived; the record changed in the same second | 2766.4 s: likewise |
| Progress | deltas of 50 to 2,703 bits, 12 over 60 s | deltas of 19 to 267 bits |
| Completion udt 18 | 1032.0 s: `public_event_complete` (1554 → **1918**), `event_completion_reward3` (2414 → **3077**), `public_event_complete_gold_tier` (604 → **732**) | 3008.7 s: `public_event_complete` (**1918**), `event_completion_reward2` (2413 → **3076**); no tier incident |
| Tier | **gold**, 121 s from start | **silver**, 265 s from start; the player died 4 times (`player_died`, 1662) from 2911 s |
| Then | 21- and 398-bit deltas closing the record | the same (53, 2,479, 388 bits) |
| Reward seen | Motes of Light shown (228), first sight | +1 Mote of Light |

**The completion tiers.** Gold is reward3 plus the gold-tier incident; silver is reward2 with no
tier incident. October's Warsat in The Dark Within was gold, the same three plus 2217 (`0x919FF2F2`).
**Public events need no services request**: there is no investment message around either
completion.

**The Taken spawns run as their own cycle on the bubble's host,** not as a public event. Region 96
(`..dc97`), region 160 (`..dd15`) and region 0 (`..dd75`) each ran one cycle:

1. incident 258 (56578 **318**);
2. incident 973 (**1199**) every 30 s;
3. 1368 (**1695**) twice in the region-96 and 160 cycles, and 610 (**738**) once in region 0's;
4. 420 (**511**) at the end.

Each cycle lasted **177-178 s**:

| Region | Cycle | Started after |
|---:|---|---|
| 96 | 194.6-372.7 s | the instance start, 61 s |
| 160 | 1240.6-1418.8 s | the instance start, 27 s |
| 0 | 2757.2-2934.3 s | during the triple event |

Region 0's cycle is the triple's "Taken Corruption" and Urzok. The 1368 pair is most likely the
Taken champions' summons [I]. Like a public event, the cycle opened no objective record.

**What a triple spawn looks like:** the events bundle's one objective record runs the Warsat, while
the same host runs a Taken cycle beside it. That is one event per bundle, plus a host-wide Taken
cycle, not three events.

**Event timing.** Both events started about 8 minutes (485 s and 494 s) after their instances
started. The client never stayed for a second event, so the interval between events is not in the
capture. `PLACEHOLDER_SCHEDULE`'s quiet windows are now 480-500 s, labelled as the first spell only.

## The Dark Within, against the story-mission table

The live mission host (`..dc63`, activity `0xB9FCCAC2`, our index 168 `act_cosmo_m1_dome_a`,
start region 56) served groups that **match our world table record for record**:

| Group | Records |
|---|---:|
| `0x41D61614` | 60 |
| `0x4786C0E0` | 21 |
| `0x8BD80DBE` | 16 |
| `0xFF4A0C1A` | 16 |

Our table lists 31, 19 and 31 records in the shared groups `0x51F665BA`, `0x6071A5F0` and
`0xBDF3E29C`; the host served 1, 1 and 2 of them. Two public instances started beside
it. There was no public event and no join to region 0.

| Time from the host's join | What |
|---:|---|
| 0 | lifetime 0; incident 2426 (`activity_join`); the full lists |
| +23.6 s | the lists again; +24.5 s lifetime 1 |
| +32.9 s | lifetime 3 |
| +34.6 s, +44.6 s | objective `0xFF4A0C1A/0x20/0` changes (a 21-bit, then a 2,481-bit delta) |
| +89.6 s, +148.6 s | the lists again; at +148.5 s incident 2938 (`insertion_point`, 56578 3732) |
| +154.6 s | objective 0's last change |
| +159.6 to +172.6 s | objective 3: 2,481, 2,478, then 21 bits |
| +177.6 to +231.6 s | objective 6: 21, 2,481, then 2,481 bits; two 2938s at +231.3 s |
| +231.6 to +263.6 s | objective 9: 2,536, 21, then 2,536 bits; three 2938s at +264 s |
| **+266.0 s** | **lifetime 6** with the completion udt 18 (789, 2539, 2305, 1620, 509 → 965, 3235, 2937, 2009, 614) |
| **+296.3 s** | **lifetime 7: 30.3 s after the 6**, as in the other 36 gameplay activities |

So the live mission has **four objectives**, in records 0, 3, 6 and 9 of `0xFF4A0C1A`, stepped one
after another in about 4 minutes. In #324's table this world has **3 script phases, reaches 1 of
them and does not end**, with lifetime `1 → 3` on content. The live end (6, then 7 30 s later) is
the target. The script bridge's phases should map to these four objective rows. The mission group
`0x41D61614` has no objective records: its squads (kind 1) and kind 2 rows changed between +149 and
+264 s, and its ten triggers (kind `0x1D`) at +149 s.

## New wire facts

- **R's value is the family 4 version** after the transaction (901, 403, 405, 601, 702, 801, 802,
  904). For 402 it is 0.
- **901's optional u64** rises by about one a second across requests: a client time stamp.
  56578's note reads it as such.
- **402 action 1** is both dismantle (gear) and turn-in (bounty). The action index is per item
  definition.
- **904 `vendor_interact`** claims rank-up and faction packages, turns in the Gunsmith's test and
  takes a Shipwright freebie. Its fields are `{vendor, i16 selector, i16 0, i32 index or -1}`, and
  it puts the contents straight into the inventory.
- **A QueueZ family 4 object of length 0 is a deletion** (a consumed engram, a dismantled item, a
  turned-in bounty).
- **A bounty turn-in sends 10 item updates**, one to each equipped item with a grid, as the XP share.
- **The account's Glimmer cap of 25,000 holds on the server**: patrol rewards past it changed
  nothing.
- **Kiosks are 901s of a free sale.**
- **Beacons, events and the Taken cycles use no services message.** They are activity-host
  incidents and objective records. Only the loot they drop goes through 601.
- **51539 → 56578 incident pairs used here,** all from the name-hash join
  (`incidents-51539-to-56578.tsv`): 342 → 415, 380 → 462, 604 → 732, 1554 → 1918, 2413 → 3076,
  2414 → 3077, 2805 → 3569 (`interaction`), 2280 → 2899 (`inventory_item_added`).

## For stream O2 (not changed here)

These files are O2's on `integrate/wiring-2`:

- **`d1-responder/src/rewards/placeholders.rs`:** `DISMANTLE_MARKS_ARMOR` 2 → **3** (live). Weapons
  stay 3.
- **`d1-responder/src/rewards/mod.rs`:**
  - `currency.dismantle.marks` can be Verified, with "3 for any Legendary".
  - `currency.dismantle.other` is wrong: uncommon gives about 25-31 Glimmer and one material; rare
    gives 1 (weapon) or 2 (armour) materials and no Glimmer.
  - `grid.xp_share` is consistent with the 10 item updates of a turn-in.
- **`d1-responder/src/rewards/currencies.rs` `dismantle_yield`:** credit materials as well as Marks.
  The material is Weapon Parts for weapons and Armor Materials for armour: 1 or 2 by tier, and 3 for
  a Legendary. A Legendary weapon also gives 88 Glimmer. Uncommon gear gives Glimmer.
- **`d1-responder/src/player_store.rs`:** hold Glimmer at **25,000**: the live account did not move
  past it.
- **Packages:** deliver a rank-up or faction package on **904** at the vendor, straight into the
  inventory: +500 Glimmer, one gear item and some Motes of Light (4 in the Dead Orbit one). Do not
  route it through the Postmaster.
- **Bounty turn-in:** **402 action 1** on the bounty. Remove the item, and send one XP update to each
  equipped item.
- **`d1-activity-host/src/rewards.rs`, `completion.rs`:**
  - A public event's tier comes from its completion incident set: gold = 1918, 3077, 732; silver =
    1918, 3076.
  - A patrol mission completes with `bounty_complete` (462) plus a second incident, sent by the
    **patrol (mission) host**, not the bubble's.
- **`d1-activity-host/src/script_bridge.rs`, `rows.rs`:**
  - The Dark Within's four objective rows (`0xFF4A0C1A/0x20/0,3,6,9`) are its phases.
  - The live end is lifetime 6 at +266 s and 7 at +296 s.
  - Each objective record changes three times (one 21-bit delta and two of about 2,480 bits),
    one record after another.
- **`d1-activity-host/src/public_registry.rs`:** the live joined instances were 30 s to 2.4 minutes
  old when joined, and the social cap is unsettled (5 peers seen).
- **`d1-activity-host/src/main.rs`:** nothing.

## Still open

- **Event intervals** after the first, and the time limits: stay through two events in one instance.
- **The profile's later fields** (XP, reputation, the progression block) on 51539, and the item
  object. A 51539 executable's RSAT descriptors would decode them.
- **The incidents' names** for the beacon, step and Taken-cycle incidents above. Our names come
  from the name-hash join and the dictionary, and these hashes are not in it.
- **702's purpose**, 904's selector table, the Speaker's sale 106 and Outfitter sale 71 prices.
- **The social cap.** A capture in a busier Tower, or the names of the matchmaking configuration's
  numbers.

## Checks run

- `python -m unittest discover -s tests -p test_srcp.py`: 16 synthetic tests (7 new: the
  investment and QueueZ readers, the payload tail, field rendering, slot changes over two fills,
  partial profile walks).
- `cargo test --release -p d1-rsat --test decode_at`: 2 tests on the owned eboot (skipped without
  it). One decodes a 901 request from bit 16; the other keeps a partial walk's image and tells
  written bytes from the fill.
- `cargo test --release -p d1-activity-host --lib -- open_world public_events public_host` and
  `--test public_instances`; `cargo test --release -p d1-responder -- inventory_ops`.
- Private, not committed: the outputs of `investment`, `stacks`, `sessions`, `sensors`,
  `incidents` (both directions), `lifetime` and the udt 11 occupancy reads, in the private `work`
  folder.

No game, emulator or lab run was made.
