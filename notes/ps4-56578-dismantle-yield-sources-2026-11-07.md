# Dismantle yields: the missing source (7 November 2026)

Offline investigation only. No emulator/server was launched and no native gameplay claim is
made here. The responder already applies a sourced `D1_DISMANTLE_YIELDS` row and publishes the
account/profile changes through `vendor::on_item_action`, `apply_dismantle` and `publish`.
There is no new sourced production yield row in this change.

## The reward resolver, re-derived for 56578

The item action's `+0x40` u16 really is used to select a reward set. Its consumer was found in
the target executable by looking for the paired `+0x40` and `+0x42` reads, rather than applying
the earlier 360 offsets:

- `0x26EED3` reads the set; `0x26EEDE` reads the reward item. `0x26EEF4` calls `0x26E950`.
- `0x26EA0B` calls `0x2799E0`, which checks investment globals vfunc `+0x1F0`.
- That vtable slot resolves to `0x25EBB0`; `0x25EBCD` reads root `+0x3E8`.
- When present, `0x27AB00` selects the set through vfunc `+0x4B8` and walks its rewards.

In the owned 56578 root (`0x80A58001`, SHA-256
`b1fc4cc4ef0ab5c1338bd85df0a9f925c7c2e5e6eb2e24acced28b242c49cf02`), **the reference at
`+0x3E8` is `0xFFFFFFFF`: there is no reward table there**. This is different from the records
table at `+0x18`. The item blocks' reward-set indices alone do not supply amounts. An absent
table does not establish zero yields either.

`d1-item-actions.py --reward-table-status` reports this reference and the root digest without
scanning all items. It checks the executable against the repository's 56578 pin first. The
owned-input test also checks the root digest against the existing content export.

The old oracle's `oodle controls` command could not find its compiled-in 01.29 estate. That
control did not pass and is not cited as evidence. The extracted root matches the existing
56578 `content-tables` source digest; independent object/message controls pass. All addresses
above belong only to the pinned 56578 image.

## The live captures contain useful observations, not a ten-row yield table

The private corpus now has **55 captures**, including the newer Cosmodrome patrol and Tower
captures. The existing reader finds **46 service-402 requests: 40 instanced actions and six
stack actions**. A 402 is `inventory_perform_item_action`, so these are not all necessarily
dismantles: decrypting/using an item also needs classification from its own build's content.

In the Tower capture, nearby account slot-308 changes include **+31, +31, +26 and +25**, and
slot-310 changes include **+3**. Several profile stacks also change. These are candidates for
Glimmer/material/mark returns, not constants: some preceding quantities or content identities
are absent from QueueZ deltas, and other investment operations may overlap. The captures are
Xbox 360 build **51539**, so their content indices cannot be copied into 56578 tables. Item
level, investment state and the server's reward formula must be established before generalising.

`d1-srcp.py dismantles <capture.pcap> [...]` now emits private JSONL pairing each 402 with its
reply by sequence and showing stack writes between them. It reuses the existing SRCP reader,
schema definitions, native RSAT decoder and two-fill stack reader. It:

- distinguishes accepted and rejected/undecodable replies;
- preserves unknown content and missing preceding quantities as `null`, never as zero;
- reports overlapping requests, partial inventory decodes and non-instanced actions;
- omits instance/account identifiers, endpoints and sequence numbers from its result;
- does not generate a responder table or assert that adjacent deltas are rewards.

Captures and reports stay private. No live-derived binary fixtures or account records are added.

## Open

All production amounts remain unsourced: basic/Common/Rare/Legendary/Exotic weapons and armour,
the starter Khvostov's set 1117, and other material/Glimmer/mark/dust returns. Resolving a source
build's item identities, recording complete baselines and establishing how level/upgrades affect
amounts are required before filling any rule. Do not substitute a later observation for a formula.
The existing opt-in placeholder table remains labelled as such.

The evidence reporter and absent-table check are covered by synthetic tests and an input-gated
owned-root check. No Rust code or responder binary changed, so no build was required.
