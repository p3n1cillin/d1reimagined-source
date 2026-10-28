# Build 56578 character and level-derived item stats

The general appearance stat table is peer `+0x838`, select card `+0x870`, and
player record `+0x1460`. It uses the same sixteen packed `{i8 key, pad, u32 value}`
entries as each weapon table. Empty keys are `0xFF`; key zero is a real stat.

The existing tutorial fixture supplies Light 385 and no other general stats.
A fresh native spawn on 28 October 2026 confirmed that `0x9128EC` expands that
table through `0x28A2F0`. Its six configuration channels select keys 2/3/4/5/6/7
(Armor, Agility, Recovery, Intellect, Discipline, Strength). All six raw inputs
and output records were zero in that baseline. This establishes the missing
inputs, not their eventual effect on health, shields or damage resistance.

## Primary investment values need their item-level calculation

The owned armour definitions declare Defense (key 1) and Light (key 0) with raw
values zero. The rifle similarly declares Attack (key 11) with raw value zero.
These are curve requests, not necessarily zero contributions. On the measured
image, `0x26F7E0` walks the definition's `+0x50` base list and calls `0x281A20`.
Non-zero entries use their declared value; zero entries consult the selected
stat's authored level curve, instance level/curve offset, progression rank and
equipment weighting. The stat table comes from owned tag `0x80A5E244`.

`NativeItemStat` executes that wrapper and its curve routines under the existing
harness. Only content-interface lookup is substituted. The caller supplies an
actual decoded inventory instance. Active talent state or unimplemented
progression state is refused. It calculates base contributions only, without
claiming to include rolled-instance or talent contributions.

The measured Windows build of bundled Unicorn 2.1.5 mishandles three-operand
scalar arithmetic in these routines. A synthetic control with XMM0=2 and XMM1=10 gives 4 for
`vaddss xmm0,xmm1,xmm0`, whose result is 12. An opt-in interpreter handles the four
measured register arithmetic operations within the curve call ranges. It reads
both sources before writing, preserves the specified upper source lanes, zeros
the upper destination lanes, and refuses unfamiliar three-source memory forms
or non-finite/subnormal values. Ordinary codec harnesses and live image bytes are unchanged.

With that correction, owned-content controls for rifle Attack and gauntlet
Defense give 3/5/10/100/200 at instance levels 0/1/2/20/40. Absent keys and
out-of-range keys are distinguished; an active talent grid is refused. These are
offline execution controls, not proof of native combat behavior.

## Explicit private trials

```text
d1-oracle character derived-stat --input <private-peer.bin> --out <new-private-peer.bin> --inventory <private-inventory-dir> --table character --stat 1
d1-oracle character derived-stat --input <private-peer.bin> --out <new-private-peer.bin> --inventory <private-inventory-dir> --table primary --stat 11
```

The command resolves equipped SOIDs and content indices against staged inventory,
verifies each definition's equipment slot, derives declared base contributions,
and updates one packed entry. It preserves every unrelated decoded byte and
round-trips with the client's original codec. Existing output files are refused.
For a weapon, include the granted instance in the inventory directory as well as
the pre-pickup items. General Light is deliberately excluded: summing item Light
would use the wrong aggregation rule.

The present staged equipment is a level 40 geared seed, including an exotic chest
and event gear. Six equipped definitions each derive Defense 200; their sum is
1200. The level 40 rifle derives Attack 200. These are diagnostic candidates for
that seed. They do not establish the authored tutorial loadout, the correct
character Defense aggregation, or a survival improvement. The reference's level 2
/ Defense 59 at video 14:25 remains a separate fidelity target. Do not hardcode 59
or substitute a level 40 result for it.

A controlled fresh native spawn received Defense 1200 in the expanded general
table, with Light still 385 and the six body-stat inputs still zero. The debugger
detached cleanly. This validates delivery of that private candidate; it does not
establish resistance or survival behavior.

Native consumer traces, survival/damage comparisons, the 25/292 reference reserve,
subclass talent contributions and progression remain to be verified. Generated
fixtures, owned content, native traces and screenshots belong in private staging.
