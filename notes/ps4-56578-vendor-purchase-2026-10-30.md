# PS4 56578: currencies and the first vendor purchase (30 October 2026, evening)

Runs sense67–sense69 on the owned 56578 image, served from `main` with #118 (`D1_ACCOUNT_FILE`,
`D1_ACCOUNT_PUSH_FILE`, replies to investment 901/903). Private lists, payloads and media stay outside
the repository. This follows [the afternoon's note](ps4-56578-tower-social-vendors-2026-10-30.md), where the
Tower first loaded as the social space and Executor Hideo's screen opened.

## Result

At Eva Levante, the Guardian Outfitter, an emblem was bought twice:
- □ Approach opened the screen. With Glimmer served, the emblem's tooltip read "Glimmer 25000 / 570" with
  "✕ Acquire".
- ✕ sent investment **901**, which the responder answered with the result composite.
- The server then pushed the purchase's effects: Glimmer 25,000 → 24,430 in the account object, and the
  emblem as an item instance in the profile's Emblems bucket.
- The client showed the acquisition notice ("Command", then "Crypto Shift"), the tooltip changed to
  "Can only hold one at a time", and the tile greyed as owned.

The effects were applied by hand (account and profile pushes). The responder doesn't yet turn a 901 into
those changes itself.

## Where currencies live

The vendor's price check is `0x26D6E0(owner, content_index, bucket = 0xFF)`, called at `0xA23FA7` with the
cost's content index (cost record `+0xC`). The result is stored as the item's "have" count (`+0x168`).

1. With bucket `0xFF` the item's own bucket ordinal is used: item definition `+0x88` (the Khvostov reads
   2, Primary Weapons; Glimmer reads `0x1F`; the Command emblem `0x19`).
2. The ordinal is looked up in the 39-row bucket table (`investment_globals->vfunc[0x98]`, rows at table
   `+0x88`, 0x14 bytes: `{u8 ordinal; i32 first_slot @+4; i32 slot_count @+8; u8 scope @+0x10}`). Read live,
   it matches the 01.29 table: `0x1F` is account slot 308, `0x19` profile slots 210–219.
3. It sums the u32 at entry `+0x10` over the slots in that range whose u16 content index matches. Scope 1
   reads the account object (`account + 0x4C8 + 32 × slot`, 326 entries), scope 0 the profile
   (`profile + 0x38 + 32 × slot`, 340 entries).

So Glimmer (content index 869) is **account slot 308, quantity at +0x10**.

⚠ The account inventory (field `[6]`, `0x80800FE6`) is `{u32 serial; 326 × 32-byte entries @+8}`. The
descriptor's `+0x20` on the array field is the element stride, not an offset. Writing entries from `+0x28`
puts every entry one slot late. The vendor header (which reads the currency slots by position) then shows
the quantities under the wrong icons, and the price check finds nothing.

## The purchase request

Request `0x80801777` decodes, through the client's own codec, to `{u8 6, i16 23, u64 0xB11}` for the first
purchase and sale index 24 for the second. These look like vendor index, sale item index and a u64 still to
identify.

## Two traps found on the way

- **A full re-encode of the account overrides the client's defaults.** Decoding the hand-built account and
  re-encoding the whole object sends every field present with zero, and the client takes them: "HUD
  DISABLED" (HUD opacity 0). Encode only the fields you mean to send, splicing sub-objects encoded with
  `d1-oracle object --tag <sub-tag> --obj-file`.
- **The headless Guardian.** The served gear's helmet slot holds the Oryx Mask (content 6514), a Festival
  of the Lost mask. Its definition keeps its gear record at `+0x148` instead of a helmet's `+0x1F0`, and in
  third person nothing renders above the collar. A peer card with Helm of Saint-14 (content 1065, art 785)
  was served in sense69; the result is not yet confirmed on screen.

## Where each currency lives

From the sale table's cost buckets (`d1-oracle vendor-sales write`, item definition `+0x88`) and the bucket
table:

| Currency | Content | Bucket | Where |
|---|---|---|---|
| Glimmer | 869 | `0x1F` | account slot 308 |
| Silver | 873 | `0x22` | account slot 309 |
| Legendary Marks | 4548 | `0x23` | account slot 310 |
| Vanguard Marks | 870 | `0x20` | profile slot 278 |
| Crucible Marks | 871 | `0x21` | profile slot 279 |
| Silver Dust | 6189 | `0x1D` | profile slots 250–261 |
| Strange Coins, Exotic Shards, Motes of Light, Weapon Parts | 872, 879, 889, 1604 | `0x05` | profile Materials, slots 0–19 |

About a third of all 8,773 prices are paid in character-held items (profile buckets). The vendor header shows
these slots by position, which is why it showed the earlier test values under unrelated icons.

## Purchases applied by the server

The responder now applies a 901 itself when `D1_VENDOR_SALES` names the sale table:
- It decodes the request with the client's codec and looks up the item and costs.
- It deducts each cost from the stacks of the cost item's own bucket, on the account or the profile (the
  price-check rule), freeing a used-up stack.
- It puts the item in a free slot of its bucket: a new instance with its own SOID when the definition says
  instanced.
- It writes the new account (only the intended fields, plus the inventory) and profile to the existing push
  files, which the next echo delivers.

The account it builds for a purchase is byte-identical to the one the client accepted in sense69.

## The item-action dispatcher (31 October)

`0xA29D30` sends every vendor and inventory item action. It picks the service from the action type at its
object's `+0x74` (0–14, jump table `0xA2A214`):

| Action | Service |
|---|---|
| 0 | 901 `vendor_buy_item` (builder `0x27F530`: vendor `[+0x60]`, sale `[+0x54]`) |
| 1 | 903 `vendor_sell_item` (`0x552FC0` → builder `0x27F7D0`: instanced flag, instance, content index or -1, `[+0x6C]`, vendor `[+0x60]`) |
| 2 | 905 `vendor_refund_item` (`0x552F60` → builder `0x27F6F0`: instanced flag, instance, content index, and a u64 at `+0x18` from `0x265100(...)`: the same value 901 sends as its u64, most likely the client's current time) |
| 3, 4, 8, 9, 11, 12 | 402 |
| 5 / 6 / 7 | 403 `inventory_equip_item` / 404 `inventory_unequip_item` / 405 `inventory_transfer_item` |
| 10 | 904 `vendor_interact` |
| 13 | none |

Action 14 takes another path (`0xA07C20`). It is not yet known which screen sets action 1, so no run has
sent a 903 yet. The responder decodes and logs one if it comes (#144).

**The investment codec's bitstream.** 903's round trip shows how the client writes a request: each
field's bits in order, most significant first, with no padding between fields. The schema's signed
integers (and 903's "u8") are written biased, top bit flipped. `{1, 0x1122334455667788, 0x348},
0x55667, 6` encodes to `889119a22ab33bc441a44002ab33c300`, with bits 65, 81 and 113 set. 905's schema
tag isn't known, so the responder decodes its 145 bits directly with that rule and logs them
(`vendor::refund_request`). It answers with the result composite, the shape of every vendor response
read so far.

The UI sets the action through `0xA29C90` (it stores the vendor at `+0x60`, the action at `+0x74` and
`+0x6C`). Of its ten callers, only two pass a fixed action (8 and 11). The rest pass the action
through from their own callers. So the action comes from the UI content's item-action interface,
and the sell screen has to be found from that side.

## Executor Hideo in the social Tower (run sense85, 1 November)

- The social Tower (flags 308 and 3391 not served, `D1_AH_ACTIVITY_REMAP=165:0`) starts in Tower
  North. `lab/tower-route/seek.py -77.1,14.4 --no-nav` walks from there to Executor Hideo's dark
  corner by the crates. His "□ Approach" opens only on a **held** □ (about 1.5 s); a tap does
  nothing.
- The allegiance offer and the vendor screen read all five currencies: Glimmer, Legendary Marks,
  Silver, and the Vanguard and Crucible Marks served in the profile (100 each, through
  `d1-character-fixup.py --stack`).
- His stock: Material Exchange donations (for example 5 Motes of Light plus New Monarchy allegiance),
  and a rank 3 Unity Engram (5 Silver Dust plus allegiance and rank 3). Nothing is priced in marks,
  so the marks purchase needs the Vanguard or Crucible Quartermaster (sale-table vendors 16 and 24).
- Accepting allegiance is a purchase: the New Monarchy Faction Badge for 2,500 Glimmer, with "✕
  Acquire". Neither a tap nor a 2-second hold sent anything. No investment message reached the
  server, so the client refused the purchase itself. Why is open.
- Patches 99-40 (Research Assistant), at about (-116.9, 42.7) in Tower North, opens his screen on a
  held □ in the social Tower, with an empty stock. In the PvE Tower (through 165), approaching him
  opened nothing (30 October).

## Eva and refunds (run sense88, 1 November)

- In the social Tower, `lab/tower-route/seek.py -70.9,39.1 --no-nav` from the Tower North spawn ends
  beside Eva Levante. A held □ opens her screen.
- Buying the Command emblem (570 Glimmer) went through with nothing done by hand. The 901 arrived as
  `86801780000000000005a380` (vendor 6, sale 23). The responder spent the Glimmer and pushed the
  account and profile (`VENDOR PURCHASE APPLIED`). The client showed Glimmer 24,430, the acquisition
  notice and "Can only hold one at a time".
- No refund option appeared on the item or the screen. The client builds its refund UI from vendor
  refund receipts (`c_investment_vendor_refund_receipt`, up to 30 per character). The server doesn't
  serve any yet, so the client can't send a 905. Where the receipts live (an object family, or a
  profile field past the ones `d1-oracle profile layout` names) is the next question for refunds.

## Refund records: account field [20] (1 November)

The refund screen (`0xA205E0`) reads the records from the account object, not the profile. Field
`[20]` of the account (`0x8080174F`, 6,248 bytes) is a u32 followed by 30 records of 208 bytes
(`0x80801781`):

| Offset | Contents |
|---|---|
| `+0x00` | cost count (3 bits), then up to six costs `{u16 content, u64, u32 quantity}` from `+0x08` |
| `+0x98` | record type (2 bits); the screen only lists type 2 |
| `+0xA0` | the item bought: `{u16 content, u64 instance, u32 quantity}` |
| `+0xB8` | u16 (the responder writes the vendor index) |
| `+0xBC` | u32 (the responder writes the sale index) |
| `+0xC0` | u64 expiry, compared with the client's "now" |
| `+0xC8` | u64 (the responder writes the purchase stamp) |

The 901's u64 is the stamp the expiry is measured against, so the responder records each applied
purchase with expiry = stamp + `D1_REFUND_WINDOW` (default 1,800) and pushes field `[20]` with the
account. The meaning of `+0xB8`, `+0xBC` and `+0xC8` is a guess; the screen reads the costs, the
type, the item and the expiry.

Run sense91 (social Tower, 1 November):
- At Eva the Command emblem was bought again. The responder logged `refund record until 0x145d
  (bought at 0xd55)` and pushed an 8,563-byte account (field [20] beside the inventory). The client
  took it: Glimmer 24,430 and "Can only hold one at a time", as before.
- `lab/tower-route/find-bytes-readonly.py` found the record in the client's memory (two copies,
  searched by its expiry and stamp): 1 cost of 570 Glimmer (869), type 2, item 840 with its instance
  `0xD100000000000601`, 6 and 23 at `+0xB8`/`+0xBC`, expiry `0x145D`.
- Eva's screen still showed no refund list, also after closing and reopening it, and neither did the
  character screen opened from it.
- The screen's data function (`0xA205E0`, `c_investment_vendor_refund_component`) doesn't filter by
  vendor. It walks the 30 records from object `+0x5A58`, keeps those with type 2 whose expiry is
  ahead of "now", sorts them and copies them into its own list (entries from `+0xA0`, count at `+0x280`,
  at most 30). It runs only when
  `0x587880()` is 3 or less (it reads the active character index) and the three registry entry 4
  objects exist. So the record would be listed wherever the component is placed; Eva's screen doesn't
  place it. The next place to look is a gear vendor (Banshee-44, or a quartermaster).
- Where screens get the component: the factory `0x1130920` (allocates 0x1058 bytes, sets the
  vtable `0x195FE00 + 0x10`, an empty list) is registered at `0xA211B1`
  in a record at `0x2773A20`: type hash `0x7128FF278C066039`, a second record at `0x2773A58` with
  `0xDC317725EE33B80A` and `0x543200BC`, and no name string. With Eva's screen open, neither 64-bit
  hash was anywhere else in the client's memory, so screen content doesn't name the component by
  them. How screen content picks its components is the next thing to read.
- Banshee-44: from both the Tower North spawn and Executor Hideo, the Armsday waypoint is about
  22 m away but in directions that only agree if most of the distance is vertical, so his stall is
  on another level. Walking from Hideo toward (-73, -6) goes off the edge of the Tower.

## Still open

- The value behind the 901 and 905 u64: `0x265100` returns a pointer to `obj+0x3E88` (when `+0x3E80`
  is set) or `obj+8` of an object from the family-4/type-4 registry entry (`0x5753C0(4)`). The
  vendor refund screen (`0xA205E0`) compares it with each refund record's expiry (`+0x5B18`), so it
  is most likely the client's current time (`0xB11` in sense69).
- Selling (903), the refund itself (905, once the client offers it), and the other vendors.
- A purchase priced in Vanguard or Crucible Marks, at a quartermaster.
- Why the client won't send the faction badge purchase.
