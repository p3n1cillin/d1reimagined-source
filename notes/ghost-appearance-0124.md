# Ghost equipment appearance in 01.24

Updated 8 October 2026. **JOIN201 shows the authored Generalist Shell in the
revival** (car window fly-in and turn to camera, reference 05:27–05:35) with
appearance row **0** and dye **15/2493**, now the 01.24 starter default. The
resurrection light then renders magenta and the emulator process exits. JOIN202 repeats it
through the committed default without the exit, shows the resurrection sphere and the
first-person Ghost, and **the Breach torch follows the camera**. The held Ghost in Nav Mode,
first-person brightness and the platform/power beat remain unaccepted.

## JOIN201: the world appearance comes from the art record

Every investment item has an art record: investment globals table `0x80a5f241`
(6,557 × 12-byte rows, indexed by item) gives its tag. This is checked against the
known Titan armour arrangements: items 198/200/202/204/206 reach records whose
`0x8080030a` arrays hold 188/189/190/191/192. In each record:

- `0x46ed4c` reads the world appearance at `+0xc + [+0xc] + 0x38` and resolves it
  through `0x2a66dc`. Ghost shells use rows **0–4** (unit `0x80a7d679` for row 0),
  Generalist is row 0. Vehicles use rows 41–53 (Sparrow S-10 is 41).
- The `0x808018ea` array holds default (channel, dye) pairs. Generalist has
  **(15, 2493)**, cosmetic name `0x172a9087`, i.e. `ghost_dyes\ghost_slot_0_starter_1`.
  The Titan helmet has channels 0–2 (549–551, `starter_guardian_armor`).
- Ghost shells have **no** `0x8080030a` arrangement array. 2706 belongs to another
  loaded record; the Arcadia jumpship's is 2321.

The investment record's `0x8080340e` value is 42 for every Ghost shell, 43 for
ships and 46 for vehicles. Rows 42, 43 and 46 all resolve the **same** generic
unit `0x80a6a008`. JOIN194–200's "oversized floating geometry" is a vehicle-like
craft flying the Ghost's authored path. Codex's cosmetic 15/1 was cosmetic index 1,
`portal_hunter_cloth_battleworn` (a Hunter cloth dye).

JOIN201 (committed server `b9654a9`, private gear file: Titan starter kit plus
`{"item": 862, "id": 267, "appearance": 0, "dyes": [[15, 2493]]}`) shows the
shell over the overturned truck, through the cars, in the car window turning to
camera, then flying to the Wall, where its shell opens. At native time 0:05:54 the
next shot draws magenta; the log ends right after RSX adds program vp 1809/fp 1830,
with no guest fault or Windows application error. Cleanup at 14:38:32 UTC is clean
(owned processes closed, title/watch restored, XInput, RPCN preserved).

The starter default now sends Generalist with appearance 0 and dye 15/2493;
an explicit dye list or appearance replaces it. Next: the renderer exit on the
resurrection effect, then the first-person conversation, held Ghost and torch.

## Earlier record (JOIN194–200)

## Fields and the server change

The final progression slot (`0x8080183c`, size `0x30`) separates:

| Field | Meaning in the traced path |
| --- | --- |
| `+0` | Equipped instance ID; Generalist remains `0x10b`. |
| `+8` | Investment item index; Generalist is 862. |
| `+0xa` | World appearance row, read by `0x71ec90` through `0x2a66dc` (Generalist: 0). |
| `+0xc` | Model arrangement index, independently converted by `0x46ed4c`. |
| `+0x18` | Six cosmetic channel/index pairs. Inventory buckets are a different mapping. |

Generalist's item record contains value 42 under `0x8080340e`. Tested as
`+0xa=42`, native lookup
resolves name `0x0b30b1ec` to unit resource `0x80a6a008`. The loaded unit library
contains that key and the executable's binary search finds it. The traced guest
lookup code matches the unmodified final executable. This does not establish
that the item's value belongs in this field or that the resolved unit/model is
the correct cinematic Ghost; oversized geometry makes that identification a
required next check.

Both 01.00 and 01.24 starter appearance defaults remain unchanged. Custom gear
accepts an explicit `appearance`, independently of `arrangement`; the following
is a diagnostic candidate, not an accepted Generalist default:

```json
{"11": {"item": 862, "id": 267, "appearance": 42, "dyes": []}}
```

Both optional indexes accept integers from -1 to 32767, including zero; boolean,
fractional, string and null values are rejected. Explicit values survive starter
kit overrides. Inventory records and equipped instance references keep their existing
layout. No default shell arrangement or cosmetic pair is added by this repair.

## Correction to the JOIN193 diagnosis

JOIN193 correctly established that both revival performances request the player's
Ghost cache through category 4. It did **not** establish that an empty-looking
descriptor or key zero meant the lookup returned nothing, nor did it rule out a
rendering or actor-binding fault.

JOIN195–196 distinguish the base unit from the optional cosmetic override:
`0x42fa68` uses cast `+0x1fc` as the base unit; cast `+0x200` is a separate override.
The latter can remain `0xffffffff` while the base unit resolves successfully.
Descriptor initialization also leaves its first twelve bytes unset. Nevertheless,
Ghost acquisition returns a valid cache handle (`0x3ef34000`, record zero) in
both third-person and first-person bindings. Zero is a valid record index.

The Ghost handle is at actor-cache `+0x31c`, from
`+0x1c + 0x2f0 + category*4`. Actor `+0x304` is the cosmetic resource, not that
handle. Read-only registry/record rechecks pass. These facts prove cache creation,
not a visible shell, complete unit construction, correct transforms or lighting.

## Native comparisons

All these comparisons retain the same C5 emulator and mission rules. Raw profiles,
package extracts, traces, screenshots and recordings remain private.

| Run | Private profile change | Result and limits |
| --- | --- | --- |
| JOIN194 | Appearance 42 | Dialog obscures much of revival; no visual acceptance. |
| JOIN195 | Appearance 42 and arrangement 2706 | Base unit resolves; inspected car/first-person output still lacks the expected shell and shows malformed geometry. |
| JOIN196 | Appearance 42 alone | Base unit and valid Ghost cache in both bindings. All nine third-person scenes and the first-person scene finish naturally. Ordinary walking reaches current24; Breach/Nav view remains black with no verified held Ghost. |
| JOIN197 | Appearance 42, cosmetic 14/1 | Channel is wrong: bucket 14 is not cosmetic channel 15. No matching cosmetic lookup. Both performances finish; later Nav input is followed by a guest access violation. |
| JOIN198 | Appearance 42, cosmetic 15/1 | Native lookup resolves `0x57d62749` to `0x80a6bfe2` and stores it in the Ghost cache descriptor. Oversized floating geometry remains; Message of the Day obscures much of first-person playback. Nav input is followed by the same access violation. |
| JOIN199 | Appearance 42, arrangement 2706, cosmetic 15/1 | Both resources resolve. An unobstructed later revival shot still has oversized floating geometry; first-person output becomes black/green with large geometry. Nav input again precedes the access violation. |
| JOIN200 | Temporary server default 42, no overlay or cosmetics | Normal service path resolves the same unit/cache. Sampled recording frames show oversized geometry; first-person is dark. Nav directly captures a null component pointer before the same access violation. The temporary starter default is withdrawn before commit. |

The native cached table and its returned pointer are rechecked in JOIN197;
`0x71ede8` reads Ghost cosmetic channel **15**. JOIN198's shared-return watch
independently captures both table pointers and the successful cosmetic lookup.
The authored Generalist cosmetic value 1 is consumed, but it does not restore the
expected visual result. Arrangement 2706 and cosmetic 15/1 remain private candidates.

JOIN196/197/198/199 performance observers collect 1,186/990/717/805 non-atomic
samples respectively, with zero reported errors or failed rechecks. Scene indices
advance through all nine third-person scenes, then the first-person scene. This
accepts natural sequence progression; dialog coverage and incorrect images prevent
full cinematic acceptance. Runs close their owned processes and restore title,
global configuration, watch configuration and XInput; RPCN remains preserved.
JOIN200 adds 712 samples with zero errors/failed rechecks, completes both natural
performances, and closes at 13:51:48 UTC. Eleven exact identities are independently
verified closed. Two selected original 30.28/30.34-second clips fully decode with
audio at 1280×720. Their contact-sheet/native-frame inspection is sampled visual
evidence, not a full audio/every-frame review or a gameplay FPS measurement.

## Nav failure and next checks

JOIN197–200 log `VM: Access violation reading location 0x1c` with compiled-block
PC `0x2ff2d8` after Nav input. This is a guest memory failure, not an Anteater code.
Static `0x2ff2d0` resolves a component datum and calls `0x1fb3c8`, which reads
`+0x1c` from its component pointer. JOIN200's filtered callback captures **r4=0**
at `0x1fb3c8`, LR `0x2ff330`, through caller chain `0x709eb0 → 0x70b1d0 →
0x40ed94`, 2.8 ms before the fatal log. A bounded component-pool read rechecks
the matching row and its zero resolved pointer; code still matches the ELF.
The original full datum and its ownership/lifetime are not yet captured. The JOIN199 entry budget
is used by earlier ordinary calls; their absent IDs do not identify the fault.

Next: verify the candidate unit/model identity and trace `0x955918` (called at
`0x709e98`) and its returned component datum. Correlate the cinematic unit and
its model/transform with the malformed draw, and obtain matching native graphics
evidence. Require the expected Ghost and hands throughout revival, held Ghost in
Nav Mode, the Breach torch, and intact character/world rendering together before
claiming the user's graphical and cinematic faults are fixed.

Six new equipment checks cover explicit fields, signed bounds, independent wire
bytes, final-schema delivery and unchanged defaults. Five fail against the prior
server; the compatibility check passes there too. The repaired server passes
**196 server tests**, **165 script tests**, the independent 100-bit equipment
fixture, actual final peer/select schema checks and mission-state preflight.
