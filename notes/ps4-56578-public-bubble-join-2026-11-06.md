# PS4 56578: what a client needs at a join (F32, 6 November 2026)

**Verified in the lab (runs fg7, fg7b):** with the join grant and the player record served (`fix/public-bubble-senses`), Restoration's client sensed to both hosts 2.1 s after its public instance's frame (live: 2.3 s), finished rung 34 (done `0x7ff`), reached `activity:in_world`, and with the roster marked ready (a stand-in) the scripts went active (lifetime 1 → 3) and the Guardian spawned in region 56 (incidents 566, 2014). The client then started its next public instance, region 48, as live.

The research below was done from captures and code. Sources:
- the private Restoration and A Guardian Rises captures (Xbox 360, build 51539), read with
  `d1-srcp.py` and `d1-oracle` (56578 codec);
- Ghidra headless decompiles and `llvm-objdump` of the 56578 eboot.

The captures stay private. This note keeps offsets, bit positions, small integers and group
hashes only: no ids, addresses or session data. `[V]` = read in code or on the wire. `[I]` = inferred.

## Summary

| Question | Answer |
|---|---|
| What is "peer 34"? | The second byte of the client's 8-byte **machine id**. The authority entry's first field is a `u8[8]` machine id, not a `u8` peer. The id is the one the client sends at `join_request+0x10` [V]. |
| What is the "local id" / "local machine" (14, 3, 12)? | The **bubble** (authority block) of the client's current slice set: `slice_set_index >> 3 & 0x3F`. It is not a machine id [V]. |
| What stops udt 6? | No authority block is held. Before in-world, the client sends senses only for a block the host granted (the authority table) [V]. |
| What stops task 9? | No player record carries the player's identity, so the player never links to a kind-0xC bundle. Task 9's leg A then finds no record [V]. |
| Why AGR and cine_reveal pass | Their hand lists serve `0x4786C0E0/0xC/5` with the identity at `A+0x40`. Restoration's PAH list is generated (no values), and our public list is entry-only [V]. |

## 1. Machine id, "peer 34" and the "local id"

**The authority entry** (`0x80800CF7` = `u32` + `0x8080540E`, an array of 65 × `0x80800B83`, 12 B each) [V, `rtti/fields.tsv`]:

| Entry offset | Field | Wire |
|---|---|---|
| `+0x0` | `0x8080405E`, **`u8[8]` machine id** (kind `0x101`) | presence bit + 64 bits |
| `+0x8` | `u16` seq (kind `0x108`) | presence bit + 16 bits |
| `+0xA` | `bool` applied | 1 bit |

A changed entry is 83 bits. An unchanged one is 3 bits.

**On the wire [V].** I searched both stamp-12 grants (the PAH at 9.9 s, the public instance at 33.7 s) for the 8 bytes at `join_request+0x10`, in join order. Each frame has exactly 23 hits, at bit 77 and then every 83 bits: one per granted block, 0–21 and 64. So every grant names the client's full machine id.

`d1-oracle sensor decode-batch` prints byte `+1` as `peer`, so "34" is the second byte of that id. `SensorAuthority::all_to_with` writes only that byte too.

**Where the host can get it [V]** (the same 8 bytes in each place):
- `join_request+0x10` (identity `+0x00`) and `join_request+0xE8` (properties `+0xB0`);
- the membership echo: member identity `+0x00` and `+0x5020`;
- udt 21 `+0xB0` and `+0x182`;
- udt 22 (`client_identity_update`, tag `0x80800C2D`, 40 B) `+0x00`.

The udt 22 at 36.6 s (public instance only) carried the client's fireteam id at `+0x20`, which its public join had left 0. The one at 395.5 s cleared it. On the 360 the id is hardware-derived. **It is personal data: never log or commit it.** Our lab client sends its own id there.

**The client never reads the machine id on the authority paths [V].**
- `0x2A1AB0` (apply) and `0x2A1440` (acknowledge) use only the block number, `seq` and `applied`.
- So do `0x2A1A80` (held test), `0x2A2200` (send gate), and `0x240A60`/`0x240AC0` with the hooks `0xBA2C20`/`0xBA2CC0`.
- `0x2A1C40` compares the current block's whole 12-byte entry (`0x685A0`) only to arm a 5 s timer (`world+0xDA8`).

That is why `D1_AH_SENSOR_AUTHORITY=3,1,0` and `12,2,0` worked with any "machine" value.

**The "local id" is the bubble [V].**
- `FUN_00345620(x)` is `FUN_0012D700([x+8]+0x2114)`, which is `v >> 3 & 0x3F` (or −1 when `v` is −1).
- The value at `[0x2693B60+8]+0x2114` is the current slice-set index (written by `FUN_00344F00`, per the brief). `d1-winguest.py`'s local id is this value.
- The client's own log calls it "map slice set index %d": `authority-over-current-slice-set`, in `0x2A1440`/`0x2A17D0`.
- A sensor client's owner `+0x60` is compared with it (terminator `0x2D1600`: pending clients whose owner equals it, or is above `0x7F`, must be delivered). So owner `+0x60` is a block too.

The earlier notes' machines 14, 3 and 12 are slice sets 112, 24 and 96 shifted right by 3. Those notes are `ps4-56578-blockers-2026-10-15`, `-10-26` and `continuation-2026-10-27`.

**The live grant pattern [V]** (all to the client's machine id, `applied = 0`):

| Host | At the join | On each bubble move |
|---|---|---|
| Restoration PAH, its public instance, AGR's PAH | blocks 0–21 and 64 (the domain: `+0xA84` bit 0, "authority-over-domain"), seq 1, about 0.8 s after the join, preceded about 0.3 s earlier by an empty table (flag 1, changed mask 0) | — |
| AGR's PAH | | First a frame releases every other block (machine 0, seq 0). The next re-grants the current block with seq + 1: block 14 seq 2, 3, 4; block 3 seq 2; block 12 seq 2. |
| Restoration, leaving the bubble at 395.7 s | | Both hosts do the same, re-granting block 7 seq 2. |
| Another player's instance (the client joins as the second peer) | only block 7, seq 3, later 5 | |

22 is `act_cosmo_m0_5_hangar`'s bubble count (`worlds.tsv`). cine_reveal (4 bubbles) got blocks 0–2 and 64, so the rule is not exactly "every bubble" [I].

## 2. The player records (group `0xEAAF16E2` / `0x4786C0E0`, kind `0xC`)

**The 56578 layout [V].** A is `0x808022A5`, B is `0x80802C30`. Both are 0x78 bytes with the same fields:

| Offset | Field |
|---|---|
| `+0x00` | `0x80800540`: `i32` (−1), `+0x04` hash (`0x811C9DC5`), bools at `+0x08`, `+0x09` and `+0x0A`, a 3-bit field at `+0x0B` and a 2-bit field at `+0x0C` |
| `+0x10` | `0x80802619`: a struct at `+0x10`, an `i32` at `+0x38` (the spawn team; −1 for story), bools at `+0x3C`–`+0x3E` |
| `+0x40` | `0x80800646`: the **identity** (kind `0x23`, 64 bits; the linker's key) at `+0x40`, `u8` at `+0x48`, a 6-bit field at `+0x49`, a struct at `+0x4C` (4-bit count + up to 8 hashes) |
| `+0x70` | `0x808023A5`: `bool` at `+0x70`, `u16` at `+0x72` |
| `+0x74` | `i32` |

`i32` fields are sent biased by `0x80000000`.

**The live entry, 520 bits [V]:**

| Bits | Content |
|---|---|
| 0–1 | `fresh`, `presence` |
| 2–242 | A (241 bits) |
| 243–246 | `has_b`, flag, `fresh`, `presence` |
| 247–487 | B (241 bits) |
| 488–519 | the `u32` that goes to the client record's `+0x10` |

A 244-bit entry is `fresh` + `presence` + A + `has_b = 0`.

**51539 versus 56578 [V by re-encoding].** Our 56578 codec encodes a default A in 273 bits. Removing its 32-bit `+0x38` team field leaves a 241-bit string equal to the live default A, except for the `u16` at `+0x72`: live is `0xBC00` in every record (`[I]` half-float −1.0), ours is 0.
- That 32-bit field is why `decode-values` refuses the live records ("read 753 bits").
- 51539 has no wire team field there [I].

**Which records carry what [V]:**
- **The public instance at stamp 25.** All 16 records carry A and B. Records 1–15 are identical defaults. Record 0 differs from them only in the identity: the 8 bytes at `join_request+0x28` (identity `+0x18`), at entry bits 112–175, byte-reversed. That is the same convention as our AGR lists' `+0x40`.
- **The PAH** (`0x4786C0E0/0xC/5..20`, stamps 14 and 267). Sub 5 (players[0]) has the identity and also `+0x74 = 0` (bits 211–242; default −1). Subs 6–20 are defaults.
- **Another player's instance.** The client's record was index 1, its peer slot, while index 0 was the host player's. So the record index is the member's peer slot.
- **The live A had `+0x08 = +0x09 = 0` at first.** Ours serve 1 and 1.
- **After the client's first udt 6.** Both hosts re-sent the player's A alone (public instance 35.9 s; PAH 36.0 s, stamp 273) with:
  - `+0x00 = 7`, the block (the bits match `0x80000007` against `0x7FFFFFFF`);
  - `+0x08 = 1`;
  - changes in `+0x0B`–`+0x10`.

  At about 51–52 s, `+0x09` went to 1 and the low bit of `+0x48` was set. `[I]` The host mirrors the B the client senses (client B: `+0x290` block, `+0x298`, `+0x299` spawn latch, written by `0x4098E0`) into A.

## 3. What each piece unblocks

**Senses (udt 6) need the authority grant [V]:**
- `0x29F720` (per tick, every world with `+0x1C`) calls `0x2A1440`. If the current block's pending bit `+0xA7C` or the domain bit `+0xA84` is set, and terminator `0x2D1600` passes, it acknowledges the block and sends a sense (`0x2D0500`).
- The slice-change path `0x6E8F60 → 0x7633C0 → 0x2A2200` sends only if the block is held (`+0xA70` set, `+0xA7C` clear).
- With no table, neither bit is ever set.
- The other sender, `0x29F460 → 0x2A1780` (unconditional), runs from `0xAE83F0` under `DAT_027BE850`. `[I]` That flag is the running simulation, i.e. in-world. That is how AGR and cine_reveal send udt 6 with no table: they reach in-world first.
- Live, the first udt 6 went to both hosts at 35.3 s. Both had granted (unapplied), and it came 0.1 s after the public full list let the terminator pass.
- Props also need it: `0x108360` creates an object only if its block (`def+0x48`, or the current block) is held.

**Task 9 needs the identity in a player record [V]:**
- Task 9 on 56578 is **`FUN_007965A0`** (its "Holding … `world_controller_spaceflight_hold_loop 0`" string).
- `activity-launch.md`'s `0x794D30`/`0x1192A0` are not function starts in this eboot. Ghidra puts `0x794D30` inside the session-parameter function `0x794CA0`, and `0x1192A0` is the tail of `0x119210`.
- It returns 2 (done) when all of these hold:
  - `FUN_0010C500(&phase)` and `phase != 0`: the lifetime, 1 in our run, passes;
  - leg A, **`FUN_001192B0(FUN_00303BA0())`** (the local player), is non-NULL and `[+8] != 0`;
  - flags `+0x1A8` bits 0, 2, 3, 5 and 6 are set;
  - it is not the case that `+0x1A8` bit 8 and `+0x1A1` bit 0 are both clear while `+0x199` bit 0 is set;
  - and `FUN_007660B0()` is false (otherwise it returns 1).
- `0x1192B0` takes the player entry's `+0x50` (the linked bundle) to the bundle's `+0x1F8`/`+0x200` record: B, `obj+0x290`, per the 13 October live read. With `+0x50 == −1` it returns NULL.
- The client fills B itself every tick in `0x4098E0`, and only for a linked bundle:
  - `B+0x00` = the block;
  - **`B+0x08` = 1 when the block is below `0x80` and `!0x9141F0()`**;
  - `B+0x09` latches when `A+0x38` equals the player's team.
- `+0x50` is set only by the linker `0x1195F0`. It runs at add-player and from the A setter `0x409710` when `A+0x40` changes, and links the type-12 object whose `A+0x40` equals the player key (13 October hardware watch).

The authority table is **not** an input to task 9 [V: the slice-set block, not authority]. `A+0x08`/`+0x09` are not either: the live A had them at 0.

**Why the private runs pass [V]:**
- `world-table-cine-live.txt` rows 164, 165 and 168 serve `0x4786C0E0/0xC/5` with A `+0x40` = the lab key. So the player links, task 9 passes and the client goes in-world, where `0x29F460` sends udt 6 with no table.
- Restoration (166) has no hand row. The generated list carries no values (`script_bridge` test: "the generated list carries no values"), and our public list is entry-only. No record has the identity, so the client holds.

## 4. Recipe

The starting client is peer slot 0. Timings are from the join.

**Public instance (`role_of_join` = public):**

| When | What |
|---|---|
| Join | udt 4, 0 and 11, as now (region slots `0xFFFFFFFF`) |
| about 0.5 s (live stamp 11) | udt 5 with `has_sensor_block = 1`, changed mask 0, no entries |
| about 0.7 s (stamp 12) | udt 5 with the authority table, no entries: changed = blocks `0..bubble_count−1` and 64 (Earth story worlds: `0x1_0000_0000_003F_FFFF`). Each changed entry is `+0..+7 = join_request[0x10..0x18]`, `+8 = 1`, `+0xA = 0`. For a joined instance (type 2), only the client's current block, with that block's next seq. |
| After the udt-21 region pick (live +2.2 s, stamp 25) | The full list. First `0xEAAF16E2/0xC/0..15` with A (`0x808022A5`) and B (`0x80802C30`) values; record [peer slot] A = default + `A[0x40..0x48] = reverse(join_request[0x28..0x30])`; the others default. Then the region's bundles as now. |
| After the client's first udt 6 (live +0.6 s), optional for task 9 | Re-send that record's A alone, mirroring the client's sensed B (`+0x00` block, `+0x08`, `+0x0B`–`+0x3E`), and later `+0x09` [I] |

The default A/B image (56578): `+0x00 = −1`, `+0x04 = 0x811C9DC5`, `+0x38 = −1`, `+0x72 = 0xBC00` (as live, `[I]`), `+0x74 = −1`, everything else 0.

**PAH:**
- The same empty table at about 0.5 s, then the same grant at about 0.8 s. Live also sent `0x4786C0E0/0xF/0` with the empty table.
- Serve `0x4786C0E0/0xC/5..20` with A and B, and sub 5 (players[0]) A = identity, with `+0x74 = 0` as live. **The generated-world path must build this from the joining client's identity.** Today only hand lists carry it, with a fixed lab key.

**Bubble moves** (both hosts): release every block except the current one (machine 0, seq 0), then re-grant the current block with seq + 1, beside the existing 23/24 exchange.

**Code to change:**
- `SensorAuthority::all_to_with` writes only byte `+1`, and to all 65 blocks. It needs the 8-byte machine id and a block set.
- `decode-batch`'s `peer` and `d1-srcp.py sensors`' label should say "machine id byte 1", or print whether the id equals the join's.

## Per-player records of a shared public instance (offline only)

Live (private captures of two-member public instances; the Xbox 51539 layout is a test target, the rule is what is used): the players group `0xEAAF16E2` kind `0xC` has 16 records, and record N is the member in membership peer slot N. A record names its player at `A+0x40`. Slots are the lowest free at join, freed on leave and reused by a later joiner. When a member joined, the others got an A-only delta for its record naming it; when one left, its record's A went back to the default (identity zero) the same second. The joiner's own full list carried every record's current value.

What the host does now (`public_host.rs`, `PublicRecords`):
- Each connection's record is its membership peer slot from `group::GroupLink::slot` (set by `state.rs` before the public join). With `D1_AH_SHARED_INSTANCES` off, or no group, the slot is 0, as before: a single client is served exactly as it was.
- The records are shared by every connection to one instance host id; each connection keeps its own cursor.
- A joiner's full list has its own key in its record and the other members' records as they stand (B stays the list's).
- The others get one A-only delta on their next poll; a leave sends one reset to the default image (identity zero) and frees the slot.
- #398's flag mirror writes `+0x4C` and `+0x50` into the record of the player whose mission host routed the flags, and the other members receive that record's change as a delta. A mission with several players in the instance writes each of their records.
- A slot of 16 or more has no record (logged once) but its member still gets the other records.

Not proven in game: that two clients really share one instance host id here (the second join must name the first's host, `D1_AH_SHARED_INSTANCES=1`), the live timing (2.1-3.1 s after the membership update; ours is the next poll), and B for other players' records (not invented).

## Open

- Whether the identity in only one host's record links the player. The linker walks the world content's type-12 table, and whether that table holds both hosts' groups is `[U]`. Serve both, as live did.
- `[I]` that `DAT_027BE850` is the in-world simulation flag.
- The meanings of `+0x72` (`0xBC00`), `+0x74` (0 on the PAH's player record only) and `+0x48` (set at about 52 s).
- Whether `FUN_00B5A680` (authority hook `0xBA2CC0`) uses any per-machine data.
