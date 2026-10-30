# PS4 56578: the Tower as its own social space, and the first vendor screen (30 October 2026)

Runs sense64–sense66 on the owned 56578 image, served from `main` (including #110). Private lists,
captures and media stay outside the repository. Times are UTC. This follows
[this morning's note](ps4-56578-tower-stay-vendors-2026-10-30.md), which traced vendor screens to the UI
context: the Tower ran as a stand-in for the next story mission, so the client loaded the PvE menus.

## Why the Tower's own activity was refused

sense64 published CONTINUE's activity 165 as index 0 (`D1_AH_ACTIVITY_REMAP=165:0`). Index 0 is
`0x8AE8DA69`, the Tower's own activity. The client left after 1.2 s with `activity_failed_to_load_missing_permissions`
(`0xedf37dcd`), from `0x7a8dd0` ("is lacking permissions -- world-transition not allowed").

The permission (`0x547ef0`) combines three predicates from `0x2648d0`. The deciding one, `0x267b50` →
`0x2679e0`, works like this:

- The activity row (content root `+0x48`, 16-byte rows, row `+8` relative) lists requirement ids at row
  `+8` (count) / `+0x10` (relative list). Index 0 has `6291abc8` and `c6cd9e47`; activities 164 and 165
  have `03fdbbf4` and `c6cd9e47`.
- Each id is binary-searched in content `0x80a5ffb0` (root `+0x28`; 28 entries of 40 bytes, key at `+0x20`).
  An entry holds two lists of expressions (`{u64 count, relative ops}`), and each must evaluate true.
- The evaluator `0x28e590` is a small stack machine over 8-byte ops `{u8 op, u32 operand at +4}`:
  op 1 pushes `flag[i] == 2` (`0x28cfe0`, 8,192 bytes), op 10 pushes `value[i]` (`0x28d160`, 2,048 ints
  at `+0x2000`), op 11 pushes a literal, op 2 is NOT, op 21 is negate, and ops 3–20 are binary (op 3 is
  OR). The flag array is indexed by **master flag-table row** (content `0x80a5ffcd`, 7,731 rows).

Decoded live:

| Activity | Rule |
|---|---|
| 0, the Tower | `NOT f[308]`, `NOT f[3391]`, `(f[419] OR f[440])`, `(f[2668] OR f[440])` |
| 164 (A Guardian Rises), 165 | `NOT f[309]`, `NOT f[3392]`, and the same shared pair |

A signature scan of the running client found the flag array, with 308 and 3391 at 2 (set). Both come from
the default `D1_UNLOCK_FLAGS` list, the 56578 translation of the 01.29 ten-flag set
(`d1_bap::UNLOCK_FLAGS_56578`), which was served on the reading that index 0 needs them set. On 56578,
op 2 makes it the other way round.

## Result: sense66

The same route as sense63, serving the default list without 308 and 3391, with
`D1_AH_ACTIVITY_REMAP=165:0`:

- A Guardian Rises ran normally, and the hangar cleared without help.
- At CONTINUE the host published index 0 (16:10:03). The client joined, acknowledged membership and
  **stayed**.
- Live reads: stage 9, substage 47, world type 2 (social). Vendor screen `0xc2ddef93` is registered in a
  loaded package (`0x80876000`) that the PvE context doesn't load.
- On screen: the social Tower's own objective ("Armsday: speak to Banshee-44"), and "□ Approach" prompts at
  vendors (Xûr, Executor Hideo).
- At Executor Hideo (New Monarchy), □ opened the faction allegiance offer and then the vendor screen:
  reputation, material exchange, a rank reward, the stock-refresh timer and item details. ○ closed it back
  to the Tower. Seen twice.

## Changes

- `d1-agr-run.ps1` and `d1-lab-served.ps1` no longer serve 308 or 3391.
- `d1_bap::UNLOCK_FLAGS_56578` keeps the faithful translation, documented with this rule.

## Still open

- Other vendors (Banshee-44, the Vanguard, the Cryptarch, Xûr), buying, and reputation.
- CONTINUE should lead to the scripted first Tower visit (165) with its own steps, and the social Tower
  after it. The remap is a diagnostic stand-in.
- What the other two predicates check (`0x2680a0`, the unlock status of `def+0x8c`; `0x267ca0`), and
  the blocked-activity list `0x28a620`. All three passed here.
- Lab: following the green waypoint in the Tower sticks in a dark stairwell. Vendors were reached by
  triangulating their HUD marker and walking with the wall-following seek.
