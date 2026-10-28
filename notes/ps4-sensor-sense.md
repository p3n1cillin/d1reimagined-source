# PS4 build 56578: client sensor sense updates

Measured on 28 October 2026. This is an offline observation tool; it does not
advance objectives, acknowledge state, alter squad members or change the served roster.

## Native derivation

The native execute probe at `0x299CB0` caught the udt-6 payload pointer and length,
with return address `0x2D05AC`. Its caller `0x2D0500` invokes writer `0x2D0640`.
The raw send is `0x299D10`; `0x299310` belongs to the older reference build.

The writer emits one continuous, MSB-first stream:

| Field | Width | Native source |
| --- | --- | --- |
| Initial flag | 1 bit | `!sender_state[0x62B]`; its broader meaning is still unpromoted |
| Generation | 8 bits | `sender_state+0x618` |
| Reserved | 1 bit | Literal zero |
| Entry present | 1 bit | One before each entry; zero terminates the list |
| Name reference | 55 bits | Object codec `0x2C9A80`, tag `0x808004B1` |
| Replicated B image | Variable | Encoder `0x2C9D20`; length comes from codec consumption |
| Revision | 32 bits | The entry's revision; distinct from the generation and name index |
| Padding | 0–7 bits | Zero, to the next byte boundary |

The entry fields repeat after the reserved bit. The native sensor-client record
has its name at `+0`, B schema tag at `+8`, A schema tag at `+0xC`, revision at
`+0x10`, owner at `+0x60` and dirty flag at `+0x67`. At writer `0x2D09D7`, `R14`
is the record, `R13` the new B image and `R12` the prior B image.

The existing `ObjectOracle::decode_replicated` runs the matching native decoder
`0x2C9AE0`. No replacement schema codec or third-party implementation was introduced.
Both name and body oracles retain the existing positive-control gate and image selection.

## Corrections to the earlier bit-diff reading

The first byte is **not** an eight-bit generation: the initial flag shifts it by
one bit. The apparent 32-bit prefixes following it are not namespace hashes at
that alignment. Decode the name reference instead.

Changes around bits 61–66 can be a change of name index, rather than a counter.
The common short packet in the saved Divide captures is a kind-1 squad B image.
The player-death clearing packet is kind 3, with a different B schema. It cannot
be promoted to an enemy-kill count merely because some groups of bits clear.
The squad B image includes floating-point and small state fields; their gameplay
meaning still needs a native consumer trace and an actual enemy-kill control.

## Use with private inputs

Build `d1-oracle` in a suitable local target directory, and select the owned 56578
image through `D1_BUILD=later-56578` and `D1_BUILDS`. No game image or capture is
provided by the repository.

First obtain native B tags with `d1-winguest.py sensor-clients --all --types`.
The observed mappings for this build include:

| Kind | B tag |
| --- | --- |
| `0x01` | `0x80800357` |
| `0x02` | `0x808005D9` |
| `0x03` | `0x80800830` |
| `0x04` | `0x80800387` |
| `0x0C` | `0x80802C30` |
| `0x18` | `0x8080048A` |
| `0x1C` | `0x808005C3` |
| `0x27` | `0x80802B18` |

These are explicit inputs, not inferred from packet bytes. Verify the mappings
for the target run and refuse a conflicting kind mapping. Use one decoder per
connection and preserve packet order.

```text
d1-oracle sensor decode-sense --input PRIVATE_CAPTURE.tsv --out NEW_PRIVATE_OUTPUT.jsonl --tsv --schema 1:0x80800357 --schema 2:0x808005D9 --schema 3:0x80800830 --schema 4:0x80800387 --schema 12:0x80802C30 --schema 24:0x8080048A --schema 28:0x808005C3 --schema 39:0x80802B18
```

TSV records are `timestamp<TAB>udt6<TAB>hex`; other event lines are ignored. Omit
`--tsv` for one hex payload per line. Output is JSONL containing the frame flags,
generation, names, tags, revisions, consumed body bits, decoded images and changed
offsets. Existing output files are refused. The first decode error stops the command,
and its partial output contains an error record.

Images are seeded from the prior **observation**, or an explicit zero image when
none exists. `reused_observation` describes that choice; it does not certify a
complete sender baseline. Generation changes discard cached images. Invalid packets
commit no overlays. A capture gap, retransmission or reordered packet can still make
the cached overlay incomplete or stale. Do not feed this output into mission logic
without proving baseline and revision handling at the protocol level.

## Validation and limits

- Four synthetic tests cover multiple entries, name and generation isolation,
  atomic rejection, missing schemas, reserved bits, trailing bytes/padding and every
  truncated byte prefix. They do not require a game image and make no native claim.
- The native codec decoded all 231 saved `sense30` packets and all 199 saved
  `sense31` packets, with exact entry boundaries and valid terminators/padding.
  Captures and decoded images remain private.
- A fresh native sender probe also confirmed the caller and payload framing.
- No verified enemy kill was present in those two captures. Kill-gated objective
  23, live-member interpretation and the drop2 Skiff departure remain open.
- There is no runtime change to `d1-activity-host`. This tool should help establish
  the missing evidence before progression is implemented.

## Later native checks (28 October, before 08:00 UK)

The same decoder accepted another 63, 89 and 232 captured packets from three
fresh runs: **814 packets in total**, each run decoded as its own ordered stream.
These totals describe codec/envelope validation, not complete capture baselines.

A native type-cell lookup for the squad B tag leads to slot `0x1974D90` and
builder `0x105230`. Its 160-byte B image starts at squad record `+0x1AC`.
Static tracing, with the relevant executable blocks independently matched to
native memory, gives these narrower field descriptions:

| B-image offset | Source |
| --- | --- |
| `+0x0C` | `0x105330`: iterate members through `0x17B380`, summing predicate `0x138190` |
| `+0x10` | Float copied from squad record `+0x270` |
| `+0x19` | State selected from record `+0x25C` flags: bit 2 → 3, else bit 3 → 2, else bit 1 → 1, else 0 |
| `+0x1A` | Record `+0x25C` bit 0 |

Predicate `0x138190` resolves the member's `+0x28/+0x30` state reference and
compares its `+0x274` word with the associated object-table entry's `+0x14C`.
It has permissive missing-reference paths. This is not the separately traced
`0x103F70` live-member loop (`entity+0x50 == -1`). Calling `+0x0C` a health or
enemy-kill count still requires a real member-death control. Execution of these
builder/predicate functions was not trapped in the later runs; the later checks
establish image identity and static data flow.

In the final combat trial, the bot fired at radar threats without acquiring a
nameplate, then the Guardian died. The observed first four squad words remained
unchanged across that trial. **No enemy kill is established.** Objective 23 and
the Skiff departure remain gated on the missing evidence.

## The member-death control (28 October, after 08:00 UK)

**`+0x0C` is the squad's live-member count.** Two controls:

- **Run sense35, a player kill (08:12:49).** A Vandal at the garage doorway, point-blank, was killed
  by rifle fire. The client reported incidents 734, 1825, 2389, 3056, 2462, 2512, 3246 and 3856 (a
  different set from the player death, 734/1825/2079/2767). In the same second, a read-only native
  B-image snapshot showed squad 17 (`0x9243DFFB/1/17`, distance field `+0x10` about 2.2) go from
  `[2,0,0,1]` to `[2,0,0,0]`. Its `+0x10` went to 2040.0, the value every squad shows when there is
  no player. The other eleven squads stayed at 1. A 95-byte udt 6 update followed 60 ms later; its
  bytes were lost because the 256-payload capture budget was already spent.
- **Run sense36, decoded by the host itself (09:14:50).** With `D1_AH_SENSE_SCHEMAS` set, the
  activity host decoded every Divide squad's count 0 → 1 on arrival (09:00:27). At 09:14:50.009 the
  client reported incidents 734 and 1435. The next udt 6 update (20 bytes, 46 ms later) carried
  squad 20's count, and the host logged `squad 0x9243dffb/0x1/20 live members 1 -> 0`. The player
  was walking, not fighting, so 1435 looks like a Fallen dying without the player (cause not yet
  identified).

`d1-activity-host` tracks these counts (opt-in) and requires an explicit cohort for a kill gate:
`!0x062dd95d:5,6,7,8,9,10,11,12|600`. Every listed kind-1 squad must have been
observed above zero and then **exactly zero**, in the same current epoch. The deadline logs a
**HOLD**, once; it never completes the step. The old `!GROUP|N` syntax is rejected rather than
silently accepting an incomplete cohort. A zero seed, negative count, missing member or an
unrelated group/kind cannot clear the gate.

Each connection owns its decoder and bounded input/output queues. A join, activity change or
subsequent current-slice change discards its old evidence and queues; a udt-6 generation change
also discards prior admission. Queued results carry the epoch and stale results are ignored.
The gate holds while any submitted delta remains undecoded. A refused/lost/oversized delta or
a dead worker holds this epoch permanently: restart the connection or enter the next world/slice
and obtain fresh positive observations before retrying. This deliberately prevents trusting a
baseline after a gap. In particular a repeat zero without a fresh positive after a reset is
insufficient. No listener timer cadence or world-clock behavior changes.

The decoder needs a B tag for every kind the world sends. For the Divide:

```text
D1_AH_SENSE_SCHEMAS=1:0x80800357,2:0x808005D9,3:0x80800830,4:0x80800387,0xC:0x80802C30,0x18:0x8080048A,0x1C:0x808005C3,0x27:0x80802B18
```

One Breach packet in sense36 (packet 24) was refused inside the client's codec
(`READ_UNMAPPED calling 0x2C9AE0`). That historical decoder continued after a refusal. The hardened gate now holds the affected epoch instead of trusting later overlays across the gap.

### What made the kill possible, and what still kills the Guardian

- **Aim.** The hip and ADS reticle sit at y 652 in the 1936x1119 window capture, 81 px below the
  frame centre. Earlier fight helpers aimed at the centre and fired over heads.
- **The gate sniper.** In sense36 the Guardian died roughly every 70 s on the route from the drop2
  respawn to the gate. The death screen names the killer: a House of Devils Vandal with a Wire
  Rifle, standing by the truck in the gate doorway. It fires before its nameplate is on screen.
- **A super/melee hang.** Pressing R1 point-blank, or L1+R1, puts the served Titan Striker into a
  third-person pose with arc on both fists that never completes. In sense36 the pose held from
  09:11:04 until the Guardian was killed at 09:11:37. A Striker should leap and slam (Fist of Havoc).
  The cause is open.
