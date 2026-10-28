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
