# Build 56578 weapon investment stats

The peer character supplies separate packed stat tables for the primary, special
and heavy weapons. The former fixtures leave them zero, so a correctly equipped
weapon can draw and fire while using minimum stat-group values.

The family-0/type-1 peer tables are at `+0x8B8`, `+0x938`, `+0x9B8`; the same shared
block appears in the character-select card at `+0x8F0`, `+0x970`, `+0x9F0`. Each
table holds sixteen eight-byte entries: signed-byte stat index and u32 investment
value at entry `+4`. Key `0xFF` means absent. Zero is a real key and trailing zero
entries can erase a preceding stat-0 value during expansion.

## Native result and limits

On 27 October 2026 a controlled natural rifle pickup changed only the primary
peer table's first pair, using the equipped definition's Magazine value. Owned
and native instruction bytes matched at all six trace controls. The hardware
trace observed the pair in the peer block at `0x54E14E`, as the primary input to
`0x910C60`, and as flat stat 20 immediately after `0x28A2F0` returned at `0x910DAC`.
Special and heavy remained zero. The HUD magazine changed from 12 to **25**;
reserve was **101**, so that one-stat trial did not fix the expected reserve.
The baseline HUD was 12/114. Raw traces, screenshots and generated fixtures remain
private. A codec round trip alone establishes shape, not native gameplay.

A second fresh natural pickup used the command's full base-stat list. The native
builder received Magazine 44, Impact 80 and Inventory 15, and the HUD showed
**25/134**. The debugger detached cleanly after four hardware hits. This proves
the full table reaches the builder, but the reserve still differs from the
reference's 285; its allocation needs a separate trace. Actual damage and kills
have not been measured by these stat-table tests.

## Generate a peer candidate from owned content

Build `d1-oracle` from `ps4/d1-server`, select the owned 56578 build, and generate
the selected build's item-index table through the existing preparation workflow.
Then:

```text
d1-oracle character weapon-stats --input <private-baseline-peer.bin> --out <new-private-peer.bin> --slot primary
```

The command decodes and round-trips the original peer with the client's codec,
resolves the equipped content index through the selected item table, verifies the
definition's item identity and equipment ordinal, and reads its base investment
list at **definition +0x50**. This differs from the optional instance-default
list at **+0x68**, which the inventory constructor copies to instance +0x88/+0x98.
It replaces only the selected peer table and sentinel-fills unused entries.
It then requires an identical codec round trip before creating a new output file.
Existing outputs, unsupported executable hashes, empty slots, duplicate or
out-of-range keys and lists longer than sixteen entries are refused.

`--slot special` and `--slot heavy` select the corresponding tables. For a
controlled single-stat trial, `--only-stat <index>` retains only that authored
entry; it never supplies a caller-invented value. The ordinary character builders
retain their prior defaults, so existing fixtures need explicit regeneration or
patching. Do not change a ready evidence package or a known baseline in place.

Copy the other roster objects unchanged into a new private directory and serve
that directory as `D1_LOOT_GRANT_ROSTER` for a fresh contained tutorial run. Keep
the item grant and profile constant. Require natural rifle trigger 35 and inspect
the actual HUD before promoting it. No generated peer, item definition or owned
media belongs in this repository.

Synthetic tests cover the two independent self-relative list fields, malformed
bounds, full key width, per-slot isolation, unused sentinels and key/size refusal.
These tests require no game content. Full weapon damage, armour, reserve, combat
completion and the ship objective still require native validation.
