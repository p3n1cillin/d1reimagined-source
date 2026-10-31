### What
The responder applies vendor purchases itself. Set `D1_VENDOR_SALES` to the sale table (`d1-oracle vendor-sales write`, #120/#121), plus `D1_VENDOR_PROFILE_OBJ`, `D1_VENDOR_ITEM_TEMPLATE`, `D1_VENDOR_ACCOUNT_SLOTS` and `D1_ORACLE`. A 901 is then:
1. decoded with the client's codec (`{u8 vendor, i16 sale, u64}`);
2. looked up for the item, its bucket and instanced flag, and each cost with its bucket;
3. paid: each cost comes out of the stacks of its own bucket, on the account or the profile, which is the rule the client's price check (`0x26D6E0`) uses. A used-up stack is freed; nothing changes unless the whole purchase applies;
4. delivered: the item goes into a free slot of its bucket, as a new instance with its own SOID when instanced;
5. published: the account (only the intended fields plus the inventory, the new `d1_bap::account_rsat_with_inventory`) and the profile (with the item as `.item`/`.soid`) go to the existing push files, which the next echo delivers.

The work runs on its own thread, because the codec calls take seconds and must not stall the BAP conversation.

The note gains the full currency map (Glimmer, Silver and Legendary Marks in account slots 308/309/310; Vanguard and Crucible Marks in profile 278/279; Silver Dust; Strange Coins and materials in Materials) and a section on server-side purchases.

### Tested
- `cargo test -p d1-responder vendor`: 4 pass. They cover the table parse, refusing an unaffordable purchase, refusing an unknown sale, and a profile-held cost taken across stacks with the used-up stack freed.
- `cargo test -p d1-bap`: `account_with_inventory_only_adds_field_6` pins the account bit layout against `account_rsat`.
- An ignored test applies the Outfitter's sale 23 through the real codec (owned inputs). Its account payload is **byte-identical** to the one the client accepted in sense69.
- Live, sense72 (this branch's first two commits deployed): two purchases at Eva Levante, applied by the responder alone. Each was applied within about 3 seconds of ✕ and pushed as account and profile deltas (versions 6–9), with new instances `…0601` and `…0602`.

### Seen in the game
At Eva Levante, Command and then Crypto Shift were bought with nothing applied by hand. Glimmer went 25,000 → 24,430 → 23,860, the game showed its acquisition notices, and both emblems read "Can only hold one at a time" and grey out as owned. The header also shows the character-held currencies served in the profile (200 each).

AI-assisted: yes.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
