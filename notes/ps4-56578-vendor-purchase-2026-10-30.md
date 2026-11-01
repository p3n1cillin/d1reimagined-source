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
| 2 | 905 `vendor_refund_item` (`0x552F60` → builder `0x27F6F0`: instanced flag, instance, content index, and a u64 at `+0x18` from `0x265100(owner...)`: the same owner field 901 sends as its u64) |
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

## Still open

- What the owner field behind the 901 and 905 u64 is: `0x265100` returns `obj+0x3E88` (when `+0x3E80`
  is set) or `obj+8` of the object `0x5753C0(4)` hands out, the family-4/type-4 (unlock-state,
  `0x8080102E`) lookup ([signon and entitlement](../ps4/design/signon-and-entitlement.md)). It read
  `0xB11` in sense69, not the key we serve that object under (`0x7FFFFFFFFFFFFFFE`).
- Selling (903), refund receipts (so the client can offer a refund, 905), and the other vendors.
- A purchase priced in Vanguard or Crucible Marks, at a quartermaster.
- Why the client won't send the faction badge purchase.
