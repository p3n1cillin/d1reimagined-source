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

## Still open

- The responder applying a purchase itself: map the 901 request to the vendor's sale item and price, update
  the stored account and profile, and push both.
- What the 901 request's u64 is.
- Selling (903) and the other vendors.
- Silver Dust and Legendary Marks in their own buckets (read their definitions' `+0x88`).
