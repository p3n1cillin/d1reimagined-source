# PS4 56578: the social services between activities (5 November 2026)

Stream I: everything around the player between activities, built as server services in
`d1-responder`, with unit tests and no game runs. This note says what each service does, which of its
rules are the game's own data and which are stand-ins, and, for each one, whether the client's wire
format is decoded or what capture would decode it.

## Where it lives

| Module | What |
|---|---|
| `src/store.rs` | `PlayerStore`, the small trait every service changes state through; `MemoryStore` (tests) and `WireStore` (the served account entries and decoded profile that `vendor.rs` pushes) |
| `src/tables.rs` | the compiled tables in `data/social/*.tsv` |
| `src/vendors/` | buying, refunds, the Vault and Postmaster, stock rotation, Xûr, the Cryptarch, reputation, Armsday |
| `src/bounties.rs` | bounties and quests |
| `src/inventory_ops.rs` | equip, unequip, transfer, dismantle, infusion, Postmaster deliveries |
| `src/director.rs` | the unlock expression machine, activity rules, completions, launch checks |
| `src/character_create.rs` | creation, deletion, selection, the starting kit |
| `src/social_hooks.rs` | the wire hooks, all behind `D1_SOCIAL=1` |

Every service works on a clone of the store and commits it only on success, so a refused request
changes nothing. Refusals carry the D1 API's own statuses (`VendorItemStatus`, `TransferStatuses`,
`EquipFailureReason`, from the DestinyDevs BungieNetPlatform definitions).

**The player store (#307).** With `D1_SOCIAL=1` the services run on a `WireStore` of `vendor.rs`'s
purchase state. The result is published like a purchase, so with `D1_STORE=1` the player store keeps
the profile, the account and the refund records through `note_vendor`. XP and reputation from
rewards are handed to it with `add_xp` and `add_reputation` (`social_hooks::bridge_progressions`).
That is the whole adapter between the two traits. One gap is left for stream C: `VendorCommit.slots`
keeps content and quantity, so a vault item's instance (`State.account_instances`, written at entry
`+8`) is not persisted yet.

## The tables

`tools/ps4_setup/d1-social-tables.py` writes `ps4/d1-server/crates/d1-responder/data/social/`. Its
inputs are Bungie's final D1 manifest (DIM's `d1-manifest-base/strings-en.json.gz`, a local input
that is never committed), the client's vendor rows and activity indices (committed fixtures), and
the client's item, action and flag tables (generated from the owned packages). The tables hold
derived numbers only. Its controls (exit 2 on failure) check the joins: client vendor 6 is the
Guardian Outfitter, 12 Eververse and 5 Xûr; Glimmer is content 869; Restoration's completion is flag
row 132, kind 3, sub 42.

- **The client's vendor rows are the manifest's vendors, sale for sale.** Of the client's 124 vendor
  rows, 89 have a manifest vendor with the same hash and the same sale count, and every sale's item
  has the same content index. So the manifest's per-sale rules apply to the client's sale indices.
- **Buckets.** Of the 39 rows of the client's bucket table, 29 are named by their items' votes
  (`buckets.tsv`); the slot counts agree with the manifest's `itemCount`. The vault is 0x16 Weapons
  (108), 0x17 General (72) and 0x18 Armor (108). Bounties are 0x1a (16) and Quests 0x25 (32).
  0x10 and 0x14 are the Postmaster's Lost Items and Messages, but nothing says which is which yet.

## The services

| Feature | The game's data | Stand-ins (labelled in the code) | Wire |
|---|---|---|---|
| Buy | sale table; `requiredLevel`; costs from the cost item's bucket; one emblem at a time (sense69) | one-at-a-time extended to the other cosmetics (LIKELY) | **decoded**: 901 `{u8 vendor, i16 sale, u64 stamp}` |
| Stock rotation | each vendor's reset schedule; each category's `quantityAvailable` (Eva: 12 of 46 emblems a day) | *which* sales a refresh picks (seeded random) | **pending**: how the client learns the current stock (candidate 1401 `update_offers`). Until then the stock is enforced only with `D1_SOCIAL_STOCK=1` |
| Resets | `(unix minutes + offset) mod interval == 0`: Tuesday 09:00 UTC weekly, 09:00 daily, Wednesday Armsday, Friday Xûr | none | n/a |
| Xûr | his weekly stock, from Friday's reset | the 48-hour stay (Friday to Sunday) is the one constant | as Buy |
| Refunds | only Eververse and the Silver Dust vendor have refundable sales (policy 2, limit 15 minutes), as Bungie's FAQ said | the client stamp's unit (read as seconds) | **decoded**: 905 from the bitstream; the client has not sent one yet, because no screen lists the refund records (1 Nov) |
| Vault | **verified content**: client vendor row 26, hash 892630493; the manifest's accepted buckets | 902 read as "take back" (LIKELY, from Blam's name `vendor_buyback_item`) | **schema known** (903 `0x80801734`, 902 `0x8080180d`); `D1_VAULT_VENDOR` remains an override |
| Postmaster | **verified content**: client rows 20 and 36, hashes 2021251983 and 2762206170 | Lost Items = 0x10 | both rows recognized; `D1_POSTMASTER_VENDOR` remains an override |
| Sell (others) | D1 vendors bought nothing: `Unsellable` | none | as Vault |
| Transfer | `TransferStatuses`: equipped, non-transferrable, no room | the direction comes from where the item is, because the two u8s of 405 are not identified | **schema known** 405 `0x808018ce`; **pending**: a run that sends one |
| Unequip | weapons, armour, subclass and Ghost always keep an equipped item | the next item of the bucket takes its place (LIKELY) | **schema known** 404 (u64); no run has sent one |
| Equip | one exotic weapon and one exotic armour piece; class gear | none | **decoded** 403 (since sense84) |
| Dismantle | the item's own reward set (client action block) | **all amounts** (`inventory_ops::PLACEHOLDER_YIELDS`, only with `D1_DISMANTLE_PLACEHOLDER=1`; a sourced `D1_DISMANTLE_YIELDS` row wins) | **decoded** 402 (since sense125) |
| Infusion | **verified content**: seven tier-category rates/floor indices (definition `+0x8A`); the client formula is in the rewards note | `inventory_ops` still uses its labelled full/80% stand-in and no cost; it has not been converted to native instance coordinates | 804 is decoded statically (#318); server mutation and costs remain open |
| Cryptarch | decoding is a 901 of a "Decode Engram" sale priced in the engram (vendor 17, sale 52: one engram 4870 in Gauntlets) | the outcome pool (every item of that bucket, tier and class) and its Light | **decoded** (901) |
| Bounties | objectives and rewards from the manifest ("Quick Drops": 30 precision kills, 2,500 XP and 50 Vanguard reputation); picking up is a 901 of a 0-Glimmer sale; "Collect" sales are 901s priced in the finished item | reward items mapped to progressions by name | pick-up and collect **decoded** (901); turning in an ordinary bounty is **402 action 1** on the bounty (#328), served by stream P ([investment ops](ps4-56578-investment-ops-2026-11-06.md)) |
| Reputation | ranks from the progression's steps; packages are the "… Package" vendors | which gear sale a package gives (seeded stand-in) | a rank-up makes a package claimable; it is claimed with **904** at the vendor, straight into the inventory, not Lost Items (#328; stream P, [investment ops](ps4-56578-investment-ops-2026-11-06.md)) |
| Armsday | Foundry Orders (5 of 25, 2,500 Glimmer), deliveries priced in the order, Wednesday reset | one order a week ("Max Orders Placed" gives no number) | **decoded** (901) |
| Director | **verified content**: all 653 launch programs, 28 shared groups and each activity's direct launch/join/visibility expressions | unlock values are still read as zero by the server predictor; state coverage is separate from program coverage | compiled `activity-rules.tsv`; `D1_ACTIVITY_RULES` overrides rows |
| Character creation | **verified content lists**: three classes, 12 unconditional and six conditional preload items each; 501 identity fields | converting an asset-preload list into exactly the initial server grant is not verified; `D1_STARTING_KIT` stays empty by default | `starting-kit.tsv` is the first list in the existing knob format; both lists retained in `starting-kit-lists.json` |

**Update (6 November).** The live Tower capture settles the bounty turn-in (402, action 1), the
package flow (904 at the vendor, contents straight into the inventory), 405's bytes and the
dismantle amounts: [the live Tower note](d1-live-captures-patrol-tower-2026-11-06.md).

## What the wire evidence would take

1. **Vendor identities are closed by static export.** `vendor-index.tsv` preserves all 124 rows,
   including those without sales. `vendor-roles.tsv` identifies the Vault and both Postmasters;
   the service recognizes these rows without requiring overrides.
2. **903, 902 and 405 on the wire.** In the Tower, move a spare Khvostov to the vault and back from
   the character screen, then collect from the Postmaster. Read the `INVESTMENT message id` lines and
   the request bytes, and round-trip each request with `d1-oracle object --tag <tag>` to fix the
   field offsets that `social_hooks::parse` assumes.
3. **404.** Find the screen that unequips (if any). Its request is one u64.
4. **The bounty turn-in.** Complete a bounty and turn it in at Bounty Tracker Tower.
5. **Infusion.** Infuse one legendary into another, and capture whichever of 801–803 or 402 goes out.
6. **Vendor stock.** Watch the messages when a vendor screen opens (1401 `update_offers` is the
   candidate). The stock rule and the "Max Orders Placed" number come with it.

## Tests

The content pass uses `d1-oracle content-tables` (or
`tools/ps4_setup/d1-content-tables.py --oracle <d1-oracle>`). It reads the owned 56578 estate,
checks the executable identity, block classes, counts, pointer bounds and activity identities,
and writes only identifiers and numbers with source-entry SHA-256 digests. The private-input
test regenerates every table and compares it to the committed output; it skips with a reason
without `D1_EBOOT`, `D1_REPO` or packages. Consumer tests evaluate all launch programs and compare
the previously decoded Tower/intro verdicts. This is offline evidence; no game or lab run.

The default-gear preload method was found through the selected executable's RTTI at
`0x576150`, rather than applying the old `0x574FB0` address. Its virtual getter is `0x25D380`,
which resolves root `+0xE8` to tag `0x80A63A74`; the class lists are 40-byte rows at field `+0x18`.
The first list is always preloaded; the second is behind a client configuration condition.
The code loads assets for all three classes, which does not by itself prove a server grant.
The exported definitions also supply bucket and instancing metadata to the kit consumer:
the manifest-derived social table omits the Hunter starter definitions, and its row 5362
claims bucket 1/uninstanced where the owned horn definition says bucket 30/instanced.
Creation uses the verified definition metadata for these exported items. The rest of the
social item catalogue has not been re-audited by this pass.
Set `D1_STARTING_KIT=content` (or `<fixtures>/characters/starting-kit.tsv`) only to choose that exported first
list as a grant policy; that policy remains labelled until the create/grant path is established.

The services' unit tests use the compiled tables and the client's committed sale table: Eva's emblem,
an Eververse emote refunded inside 15 minutes and refused after, Xûr absent on a Wednesday and
selling on Saturday, an exotic engram decoding into an exotic Titan gauntlet, a bounty to turn-in with
its XP and reputation, a New Monarchy rank-up with its package, an Armsday order delivered the next
week, the Tower's unlock rule (serving flag 308 refuses it, as in sense64), Restoration's completion
byte, a rifle into the vault and back on the served profile bytes, and creation of three Guardians.
