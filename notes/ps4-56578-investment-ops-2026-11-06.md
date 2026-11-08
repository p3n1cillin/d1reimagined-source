# Investment requests served: locks, upgrades, infusion, packages, turn-ins, Record Books (6 November 2026)

Stream P. Build 56578. The responder now answers every investment request the client can send
that it had no answer for: **406** (the item lock), **801/802** (talent nodes), **804** (infusion),
**904** (packages, the Gunsmith's turn-in), **1801/1802/1803** (Record Books), and **402 action 1 on
a bounty** (the turn-in). Each one is decoded, checked, applied to the player's record, pushed to
the client as one family-4 change, and answered, in the order the live servers used. Two
placeholders the live Tower capture contradicts are corrected.

**No game, emulator or lab run was made.** Everything below is build and unit-test evidence, plus
static reads of the owned 56578 executable (Ghidra decompiles of the functions named) and the
private live captures (read with `d1-srcp.py queuez`; nothing from them is in this repository).

## How each exchange runs

The Tower capture ([#328](d1-live-captures-patrol-tower-2026-11-06.md)) shows every exchange as
**the request, one family-4 push of the changed objects, then the reply**, whose `R.value` is the
family-4 version after the push. 402's reply carries 0. The responder now does the same:

1. `d1_bap::investment_ops` decodes the request from its descriptor layout;
2. `investment::store_io::load` reads the connection's player from the player store (stream O3's
   per-player record: `players::current`, never the knobs when the connection has its own);
3. the handler works on a `Snapshot` (the decoded profile, the account's slots, the item elements
   it touches, the character's `investment.*` attributes) and logs each object it changes;
4. `store_io::commit` writes the change back through the store's trait and encodes the objects
   with the client's codec;
5. the connection writes the push as the next family-4 delta, then parks the reply
   (`R{0, version}`).

A refusal changes nothing, pushes nothing, and replies `R{1, the version the client holds}`.
Which status values the client tells apart is open (every live reply was 0), so `1` is a labelled
stand-in (`investment.refusal_status`). The client checks each request before sending it.

**Gates:** 402, 406 and 904 need `D1_SOCIAL=1`; 801, 802, 804 and 1801–1803 need `D1_REWARDS=1`.
Every one also needs the player store (`D1_STORE=1`) and a family-4 baseline the client has
accepted. Without them the responder answers exactly as before.

**The objects' order in a push** is the order the live servers used for the same kind of change
(read from the capture's QueueZ pushes):

| Exchange | Live push (#328) | Ours |
|---|---|---|
| 801 activate a node | account, profile, item (38 B) | the same |
| 802 change a step | profile (20 B), item (38 B) | item only (what the profile's 20 bytes are is open) |
| 904 package | account, profile, new item | the same |
| 402 bounty turn-in | bounty deleted, 10 item updates (15 B each), profile | deleted, each equipped item with a grid, profile |
| 804 infusion | none seen | deleted source, profile, target |
| 406 lock | none seen | profile |

A deleted item is an item object of length 0, as the live pushes delete a consumed engram.

## Each request

| Service | Request (decoded) | What the server does | Evidence and what stays open |
|---|---|---|---|
| **406** | `I {bool, u64, i16}, flags:i32` | writes the flags into the item's profile slot entry (`0x808016E0 +0x18`) | **Decoded**: `0xA29D30` sends that word with bit 0 set (action 8) or cleared (9), bit 1 set (11) or cleared (12); `0x26F260` refuses a source with bit 0 set. Bit 1's name is open. A vault item is refused: the store's account slots keep no flags word |
| **801** | `item:u64, node:i8` | the node's grid level, prerequisites and exclusive set (the manifest grid of the item's definition); charges the node cost; sets the node-state byte | Cost: `grid.node_cost` **corrected** (below). Node-state bytes written as `0x284E10`/`0x285050` read them (0 inactive; a fixed node counts its steps; a random node is 1 with its step in `+0x7E`): `item.node_state` stays a placeholder until a served item shows it |
| **802** | `item:u64, node:i8` | selects the node within its exclusive set, for free; the switched-off nodes are kept in `investment.grid.<instance>` | Live: free, item and profile changed. A node must have been active before |
| **804** | `target:u64, node:i8, source:u64` | the client's own refusal mask (`0x26F260`: missing, bucket, lock, equipped, no gain); the gain `min(diff, max(diff × rate, floor))`; the target's level rises by it (rounded down); the source is destroyed | The category bits (`0x40`/`0x400`, definition `+0x30` block `+0x4C`) and the talent-quality term (`0x286890`) are not exported, so not checked; the cost stays `light.infusion_cost` (none). Never seen live |
| **904** | `vendor:i8, a:i16, b:i16, sale:i32` | a package sale (the faction's Package item, bucket `0x13`, free): one a rank of the vendor's progression, contents straight into the inventory; any other free Rewards sale: its item | Contents: 500 Glimmer (both live packages), one gear item from the package vendor's sales for the character's class (seeded stand-in), and 4 Motes of Light for a faction package (Dead Orbit, one capture). The capture's "+4 of material slot 6" is not named. `a`/`b`'s table, which test weapon the Gunsmith's turn-in consumes, and the Shipwright's `sale -1` freebie (refused) are open |
| **402 action 1** on a bounty | `I, action:i32` (406's layout) | removes the bounty; Experience into the level and, at `grid.xp_share`, into each equipped item with a grid; reputation; items; a quest step's next step | Completion is checked when the store keeps the bounty's progress; otherwise the client's check stands (said in the log). Gear's 402 stays the dismantle |
| **1801** | `reward:i16` | remembered once (`investment.record_books.redeemed`) | the ordinal's reward is client content not exported: nothing granted or pushed |
| **1802** | `book:i8` | remembered once (`.unsealed`) | the book state's object fields are not decoded: nothing pushed |
| **1803** | `entry:i16, row:i8, table:i8` | remembered once (`.claimed`) | `0x277340` checks the row's progression level, its requirement list and room for the entry's item; the table (manager slot `+0x490`) is not exported, so nothing is granted |

## The two placeholders the capture contradicted

| Placeholder | Was | Now | Evidence |
|---|---|---|---|
| `grid.node_cost` (`rewards/placeholders.rs`) | free | a weapon or armour node: **500 Glimmer + 10 Weapon Parts** (armour: Armor Materials, by analogy); subclass nodes free. Still a labelled stand-in | one live 801 (#328) cost 500 Glimmer, 10 Weapon Parts and 10 of an unnamed material. The real costs are each node step's requirement list in the client's content (step `+0x98`, read by `0x285050` and `0x28E9D0`, 16-byte entries checked by `0x28E590`) |
| Rank-up packages (`vendors/reputation.rs`) | delivered to the Postmaster's Lost Items | a rank-up names the package; the player claims it at the vendor with 904, straight into the inventory | #328: 904 at the vendor, contents in the inventory |

## Checks

- `cargo test -p d1-bap --lib investment_ops`: every request round-trips at its descriptor width,
  the biases, `R` with a value (`result_body(0, 0)` is the familiar `18 00 00 00 00`), the change
  delta's order and length-0 deletion, and the reply naming the push's version.
- `cargo test -p d1-oracle --test investment_ops` (with `D1_EBOOT`): the client's own encoder
  writes each synthetic request (including the four live 904 shapes and 402) in the descriptor's
  bit count and bytes that `d1_bap::investment_ops` decodes to the same fields; each reply tag's
  encoding of `R{0, 590}`, `R{0, 0}`, `R{1, -17}` and `R{0, i32::MAX}` is byte-for-byte ours.
  Skips without `D1_EBOOT`.
- `cargo test -p d1-responder investment`, `vendors::reputation`, `rewards`: each handler on a
  snapshot (costs, refusals that change nothing, the live order of the logged objects), and the
  gates off by default.

## What would settle the rest

- **Node costs:** export each node step's requirement list from the owned content (the grid table
  at interface `+0x4E0`, step `+0x98`), or capture more 801s across item tiers.
- **Node-state bytes and 802's profile change:** a served item with one node toggled, read back.
- **Infusion:** a capture of an 804 and its push; the category field (definition `+0x30` block
  `+0x4C`) in the content export.
- **904's `a`/`b` and `sale -1`:** the vendor interaction table; a capture of the Shipwright's freebie.
- **Record Books:** the reward and book ordinals' tables (manager slot `+0x490`), and the Record
  Book fields of the pushed objects.
- **Refusal statuses:** a capture of a refused request.
