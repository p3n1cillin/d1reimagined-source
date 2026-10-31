# Codex to Claude: completion chain accepted; phase 7 postgame gate; progression names; squad draft

28 October 2026, 23:49 UK. The user explicitly requested this hand-back to the
Claude desktop Code session "PS4 build 56578 retarget". I read your 20:30 handoff
from the top, including all appended results and the 23:30 order of work.

You retain all lab and game input. I have sent no game input, called no native
game functions, used no debugger, written no game memory, published no control,
edited no active schedule, stopped no lab process, or built/replaced the host.
All bounded native probes have exited. Private drafts below are UNSERVED.

## 1. Completion DID execute the activity chain; the live lifetime is still 3

This is stronger than merely confirming the responder sent a packet.
At approximately 23:47, sense48 still had shadPS4 PID 2444, eboot base 0x51c0000.
Refresh these on every later read; addresses here are evidence, not future pins.

Read-only results:

- Native family/type 4, local index 1 profile, key D100000000000201:
  profile address 0x37f909140; +0x2c20 = 164, +0x2c38 = 1.
- Its progression count at +0x3660 is 5. Inline entries at +0x3664 are
  117,118,124,249,250, with progress/daily/weekly all zero.
- Selection record [base+0x26bc068] = 0x205008f30:
  bytes start `0100a500ff00ffff`; type byte 1, activity u16 165.
  **The completion handler has already selected the chain activity 165.**
- Current content's kind-16 lifetime component is 0x380262120.
  Its signed phase byte at +0x1ac remains **3**, not 7.
- The in-world handler (vtable static 0x1945ad0) is at 0x788e420.
  Its postgame flag words +0x68/+0x70/+0x78 are all zero, so their bit-0
  guards do not suppress the initial postgame entry.

### Exact postgame branch

The relevant state is **activity_in_world**, not setup_orbit_outro:

1. Vtable 0x1945ad0, update slot +0x38 = 0x797240.
2. It calls 0x7976b0 at 0x79732f.
3. 0x7976b0 calls 0x75e4c0. That calls lifetime getter 0x10c430 and returns
   `phase == 7` (cmp eax,7 at 0x75e4c9).
4. If true and handler bit 0 at +0x78/+0x70/+0x68 is clear, it reaches
   string **"Enter post game"** (0x165aec9), sets handler+0x68 bit 0,
   calls 0x774f50(0), obtains data via 0x3db440 and 0x771940,
   then invokes 0x76f0f0 and 0x770ba0.

Thus **phase 7 is an exact necessary gate for the actual postgame path**.
This is static plus live guard evidence, NOT a claim that rewards/orbit have
already appeared. We still need your visual/native test. Later return-to-orbit
UI behavior and any additional data it requests remain to be observed.

The setup_orbit_outro state (state 0x20, update 0x7ae540) is entered by the
prologue_intro_loading path at 0x7ae965. Its name alone was a misleading lead
for this activity ending; do not force that state.

### Important distinction: two definitions returned by 0x5498e0

0x5498e0 returns **investment definition in RAX** and **client definition in
RDX**. Do not read chain+0x98 from the client definition.

- Client definition for 164: 0x380b2d6a0, +0x18 = 811c9dc5; the early
  completion guard passes.
- Investment definition for 164: 0x380a0be88, hash 6e0dae89 (A Guardian Rises).
- Its +0x98 relative pointer = 0x2c510, resolving to 0x380a38430.
- Chain expression count = 0: 0x28eba0 returns false, so no unlock-expression
  rejection is taken. Chain record +0x10 type = 1, +0x12 activity = 165.
- Activity 165 hash = 6eaf0959. The archived naming database also calls it
  A Guardian Rises; do not assume it is an arbitrary orbit index. Your current
  world table has no 165 row, so watch the subsequent launch/selection behavior.
- The live selection record above proves the 164 -> 165 path already ran.

I initially inspected the client definition's zero +0x98 and briefly inferred
"no chain" in progress commentary. That was incorrect; the two-definition
distinction and the live selected-165 read above supersede it.

### Concrete phase-7 draft and source PR

PR **#73**: https://github.com/p3n1cillin/d1reimagined-source/pull/73
Commit 9fff49bd0e88c3cb6c83a702efc6e34f5cd5d8e8,
branch codex/postgame-lifetime-phase. Pushed before opening PR. Public source
checkout F:\d1\public-work\d1reimagined-sense is clean.

The existing `clients GROUP PATH` builder always inherits launch `phase`.
`sensor_list::params_for_list` filters OUT a matching lifetime row, even if
the row contains a valid A image. Simply writing phase 7 into a lifetime row
does not work with that parser.

PR73 adds an explicit list directive **`@phase 7`**. It overrides the separate
generated lifetime entry, works for live controls/world tables/mission steps,
rejects duplicates and values outside the four-bit bias-1 range -1..14, and
preserves existing behavior when omitted. All seven sensor-list tests passed
in a standalone rustc test executable linked against existing dependency rlibs.
No host executable or host library was rebuilt. `git diff --check` passed.

Private unpublished list:
`F:\d1\d1preservation-main\cfw-staging\agr-postgame-phase7.draft.txt`

It contains `@authority 13,2,0` (current outro authority, not launch 3) and
`@phase 7`. There were zero pending clients when sampled; `--all --types`
showed the 22 current delivered clients. Reconcile fresh generation/pending
clients before use. In the PR73-capable host, the control is:

`clients 4786c0e0 F:\d1\d1preservation-main\cfw-staging\agr-postgame-phase7.draft.txt`

This is an instruction for YOUR later test, not a publication from me.
I have NOT deployed the PR. Follow your stop-before-host-build rule.
After sending, read lifetime+0x1ac first: require 7, then inspect handler+0x68
and actual screen/requests. This separates delivery failure from downstream UI.
The sense45 phase tests did not establish that phase 7 landed; their launch
client list differs from the outro's. Sense48's current phase is demonstrably 3.

For a transport-capable later test, the owned oracle generated this standalone
59-byte udt-5 payload, gen0, group4786c0e0, kind16/sub3, phase7, no authority
block or other clients:
`000000000068f0d81c046000c00000557817fffffff811c9dc57fffffff020000000000811c9dc5000000003fffffffc08e4ee2807fff811c9dc50`
This standalone frame uses default lifetime fields. The list/PR route retains
the builder's tail settings. No raw-udt control command currently exists in
the inspected host; do not send that payload as an incident.

## 2. Progressions 117/118/124/249/250 are mapped

Owned live definition array: investment interface vtable +0x1b8 -> 0x25e6c0;
cache at base+0x26527f8+0x180 -> table; definition array header at table+8,
252 records, self-relative array, stride 56. Safe per-index getter is
0x25b3f0 (vtable+0x470). The five IDs in globals+0x166..0x16e match your reads.

| Native id | Definition hash | Name | Owned step data |
|---|---|---|---|
|117|6650c0f9|character_level|40 steps; initial costs 0,900,4100,5000,...|
|118|79002d5e|character_prestige|one repeating 25,000-XP step|
|124|f4c31a9b|death_penalty|one zero-cost step, unlock flag2206|
|249|793ad10e|weekly_pve|100 one-unit steps; last grants flag2643|
|250|793ad11b|weekly_pvp|100 one-unit steps; last grants flag2644|

Names come from the ALREADY LOCAL archived manifest at
`F:\d1\d1preservation-main\manifest\data\world_en.sqlite`, opened read-only.
Every name's FNV-1 independently matches the owned live definition hash, and
the archived step costs match the owned table. No network/download was used.
The archive is a newer naming corpus; owned 56578 definitions remain authoritative.

Values: prestige, death_penalty and both weekly-mark tracks can stay at zero
for this new tutorial character; daily/weekly buckets stay zero. 117 is
**cumulative raw XP**, not the displayed level. 0 is level1; 900 is the
level2 threshold; 5000 is the level3 threshold. Do NOT write 2 to mean level2.
If reproducing a reference character that has reached level2, 900 is a useful
explicit synthetic minimum, not an authenticated end-of-mission XP total.
We have no XP grant ledger for these synthetic kills/completion, so an exact
earned XP total cannot be reconstructed from the existing zero entries.
The five-zero profile is accepted and already chains; XP is not the missing
phase-7 gate. Do not bundle an XP experiment into the first phase test.

## 3. Squad assignment: diagnostic row set ready; authentic mapping still missing

Private UNSERVED draft:
`F:\d1\d1preservation-main\cfw-staging\agr-dock13-assign-1112.draft.txt`

It preserves all other rows from agr-dock13-obj26-m52.txt, changes ONLY the A
images for squads11/12 and encounter3/4, and labels each inference. It is a
**diagnostic assignment experiment**, not the recovered original mission script.
Do not publish it in current outro105; it is for Dock104 with fresh client and
authority checks. I have not confirmed movement or resolved-task acceptance.

Native branch now settled:

- A+0 (8-byte object ref) -> squad+0x3e0, A+0x68 -> +0x3e8,
  A+0x6c -> +0x3ec via 0x1046b0.
- 0x1047d0 resolves A+0 and registers the squad with the objective's per-task
  list via 0x1c2750, indexed by **A+0x6c**.
- 0x171610 calculates assignment costs; 0x173820 walks each top-level task's
  registered squads and chooses a subtask according to costs/capacities.
- 0x105c50 is the setter called by that allocator: it stores the resolved
  firing-area-set handle at squad+0x274 and **subtask index** at +0x3f0.
  Thus +0x3f0 is NOT the top-level task ordinal.

Owned obj_m0_hangar_fallen (80cb86ac; definition base0x748) has nine 40-byte
top-level task records. Their subtask firing-area-set refs (kind42) are:

0:[44], 1:[45], 2:[46,19,20], 3:[22], 4:[21], 5:[40], 6:[41], 7:[23], 8:[43].
Top-level task2's name hash is FNV-1("gating"). This is a real task/firing-area
structure, not evidence of nine chronological waves. The ten object-reference
slots in the examined metadata are absent, so this does not supply the original
squad11/12 -> top-level task membership or its release timing.

Encounter A (808004fa,28bytes) is also NOT a current-wave selector:
- First24 bytes are per-task revision/reset counters. At 0x1c2330 the client
  compares A[i] to runtime+0x1e0+i*26; a change copies the byte and clears that
  task's subflags/runtime state (49-byte task runtime record).
- U32 A+0x18 is copied to runtime+0x450; a change clears runtime+0x454.
- There is no special native "1 enables encounter" test in this setter.

The draft deliberately assigns BOTH squads11/12 to encounter062dd95d/3/4,
top-level task0, whose only authored subtask points to firing-area-set44,
capacity4. A+0 reference bytes are 5dd92d0603000400. A+0x68=0 is retained from
baseline (role unresolved), A+0x6c=0 is an inferred test selection. Desired
counts and generation are preserved. Encounter A[0]=1 is a fresh reset counter;
all other revision bytes and U32+0x18 are zero. These starting counters are
test choices, not authored original values.

Expected diagnostic: +0x3e0 resolves encounter, +0x3ec=0, allocator makes
+0x3f0=0 and +0x274 resolves firing-area-set44. Check that before judging
movement. The three changed images encode and round-trip IDENTICAL through
the owned codec: 124-byte squad A x2 and 28-byte encounter A x1.

Still missing for authenticity: original server's objective/task assignments
and timing for all eight squads, the role/value of A+0x68, and whether reference
late Dregs correspond to11/12. This does NOT authenticate cohort5-10 or prove
the reference skips11/12. The task0 experiment isolates the native assignment
mechanism without pretending that those unknowns are settled.

## Evidence and reproducibility

All new raw evidence/scripts are PRIVATE under:
`lab/history\2026-10-28-completion-codex`

- probe-completion.py: bounded read-only profile/lifetime/selection/definition
  traversal. Validates native code bytes at 56b260,56dc00,10c430,75e4c0 against
  the SHA-pinned owned ELF. No debugger or native calls.
- completion-*.json, probe-last.txt: timestamped snapshots. Some earlier files
  predate the corrected inline progression parser and two-definition chain
  distinction; use the NEWEST snapshot / this handoff.
- native-completion-paths.txt: owned disassembly for reproducing the gates.
- progression-names.json: five names/hash/steps from the local naming corpus.
- build-assignment-draft.py, assignment-codec-checks.json: preserves baseline
  fields and validates all three changed images through the owned codec.
- sensor-list-tests.exe: standalone seven-test harness, not a host build.

The profile path is 56b260 -> manager [base+26bacc0], then 56dc00(type4,
localindex1,key). 1009850 returns constant localindex1. Table entry geometry is
manager+type*96+localindex*16+4fd90 -> offset,count,stride; data starts at
manager+40a0+offset. Progression entries are INLINE at profile+3664, not a
self-relative array. Do not use localindex0's account-key record as character.

Recommended immediate test: PR73-capable phase7 update after completion/outro,
verify native phase7 and postgame latch, then observe screen and chain165 launch.
Leave XP and the Dock assignment experiment separate. No public raw evidence.

END CODEX HAND-BACK 2026-10-28 23:49 UK
