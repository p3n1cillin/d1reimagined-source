# PS4 56578: cinematic sensor start conditions (5 November 2026)

**Result: verified binary behavior, unresolved native failure.** A kind-6 value is not a
`{start, state, flag}` state machine. Its `+4` word is a change counter and `+8` is a
playing byte. The setter first stores the entire value, then attempts playback only if
the counter changed. A rejected attempt consumes that change. Fixing the flag, making
the player ready, or repeating the same counter does not retry it.

This investigation is offline. No shadPS4, server, route, live capture, input or
watchpoint operation was run. The prior cine-live8 observations are reported in
[the live-capture note](d1-live-captures-2026-11-05.md#serving-it-5-november), not newly
observed here. No claim about a new native run or emulator commit is made.
AI-assisted: yes, OpenAI Codex; review and any subsequent native validation remain
with the maintainer.

Labels below distinguish **verified binary**, **verified offline execution**, **verified
repository code**, **capture decode**, **prior observation**, and **inference/open**.
All executable addresses are **vaddrs in build 56578**, SHA-256
`672f03411c0503dfbcda804cbb9fb27cdd9a8d73a8bcb71bf0c4f688073b0833`.
No 01.29 address was applied to this image.

## The consumer and its gates

**Verified binary.** Descriptor `0x8080254D`, anchor `0x1302700`, declares a `0xD4`-byte
value with field offsets `0, 4, 8, 9, 0xC, 0x14, 0x18, 0x9C, 0xD0`. The cinematic
setter wrapper is `0x403520`; it obtains its typed value using the descriptor global
at `0x19B2B48` and calls `0x403140` at `0x403570`. Its bound method is relocated at
`0x19A3868`. The update method at `0x402EF0`, bound at `0x19A3850`, also calls the
same apply routine at `0x402F8C` when its deferred-update byte `record+0x281` is set.

**Verified binary.** Both wrapper and update first test `0x765C40`. A true result
suppresses their usual apply path. That predicate resolves the reference at
`(*0x26CA0A8)+0x80` and compares the result with `-1`; this is the world-controller
cinematics-manager path. It is not a test of the sensor value's timestamp.
Whether that reference was valid during cine-live8 is **open**.

**Verified binary and offline execution.** In `0x403140` the value lives at
`record+0x1AC`; `record+0x280` is the local playing/accepted byte:

| Gate / action | 56578 evidence |
| --- | --- |
| Save the old counter, copy all `0xD4` bytes, compare the new counter | `0x403162` through `0x403184` |
| Equal counter: return without start, stop or preload work | branch to `0x403321` at `0x403184` |
| New `value+8` equals local `record+0x280`: skip the start/stop transition | `0x40318A` through `0x40319A` |
| New playing byte is zero: stop, rather than start | `0x4031A0` / `0x4032A7` |
| Obtain a valid local player; `-1` rejects the start | `0x4031B4` through `0x4031C0`, calling `0x303BA0` |
| If `value+0xC` names an object, resolve it and test the player's object against it | `0x4031E8` through `0x40326D`, using `0x1076C0`, `0x109760`, `0x126CB0` |
| Signed byte at `value+0x14`: `-1` bypasses the filter; otherwise it must equal `0x301C80(player)` | `0x40326F` through `0x403290` |
| Signed count at `value+0x18` must be at most 15 | `0x403292` through `0x40329B` |
| Attempt the start | call `0x403770` at `0x4032A0` |

**Verified binary.** The local-player lookup is a real player-record dependency, not
just a cast-list membership check. `0x303BA0` calls game-interface wrapper `0x2400B0`
(vtable offset `0x138`), validates the returned slot against player pool `0x2689580`,
then builds a handle. The interface is installed at `0x6E7814`/`0x6E781B`; its
`0x19439A0+0x138` slot resolves to `0x6E7E20`. That method reads the local player
selection through `0x923790` and `0x9236B0`, ultimately from `(*0x273EB80)+0xC0`.
An in-world activity ladder state alone does not establish that this lookup succeeds.

**Verified binary.** The absent object reference is initialized by `0x1076E0`:
hash `0x811C9DC5`, byte `-1`, sub-index `-1`. The predicate at `0x1076C0` tests the
byte and sub-index; an all-zero reference is present, not absent. The object-present
path can call `0x126CB0` with a null resolved object. Keep the proper absent reference.

**Verified binary.** `value+9` requests preloading only after a counter change and
while `record+0x280` is false (`0x4032AF` through `0x40331C`, calling `0x4E4830`).
Preloading is not a prerequisite checked before the start call. There is no read of
`value+0` in this apply routine, and no comparison against the udt-5 frame stamp here.
This does **not** prove that frame timing is irrelevant elsewhere in replication.

**Verified offline execution.** The new
[cinematic-start tests](../ps4/d1-server/crates/d1-oracle/tests/cinematic_start.rs)
execute the owned client's instructions through the existing `d1-oracle::Harness`:

- A changed idle counter followed by playing=1 at the same counter does not start.
- An attempt without a local player stores its value but does not reach start.
- Making the player valid and resending that counter still does not reach start.
- Another counter change, playing=1, no object/filter, and timestamp **zero** reaches
  `0x403770`. This is the positive control for those negative cases.
- A wrong team or count 16 rejects; matching team or count 15 reaches start.
- An already-playing record skips another start.

The local-player/team queries and the start boundary are stubbed in this experiment.
This verifies the setter's decisions, not that the renderer or cinematic engine runs.

## From a start attempt to the started incident

**Verified binary and offline execution.** `0x403770` copies up to six cast identities
from `value+0xA0`, counted at `value+0x9C`, into cinematic globals
`(*0x26AFEF0)+0x1ADC/+0x1AE0`. These are eight-byte identities, not machine numbers;
there is no extra eight-byte gap before the first element.

It reads the cinematic resource identifier at **definition/component `+0x68`** and
calls `0x4E3F70(resource, true)` at `0x403849`. The returned boolean is stored in
`record+0x280` at `0x40384E`. A true return selects the hash at `0x2651C94`,
`0xAECA3822`; false selects `0x2651C90`, `0x3C03F059`. Initializer `0x24DA20`
writes these hashes at `0x24DB60`/`0x24DB6D`. Both branches call the incident builder
`0x403350` at `0x40387D`.

The second test executes the real initializer and start routine, with acceptance
stubbed at `0x4E3F70` and reporting observed at `0x403350`. It checks both outcomes,
the local playing byte, and the cast-copy offsets. No incident is sent anywhere.

**Verified binary.** `0x4E3F70` is another possible rejection point. It walks the
registered cinematic bindings in `(*0x26AFEF0)+0x1358`, stride `0x38`, count at
`+0x1350`. It asks each binding for its identifier through `0x354400`, compares that
identifier with the requested resource at `0x4E40BE`, and invokes the matching
binding's start method through `0x354440` at `0x4E41A6`. No matching binding returns
false; a matching binding can itself return false. Registration comes through
`0x4E3910`, called from `0x4E7EB0` and tail-called from `0x4F1793`.

**Open.** The identity and runtime readiness of cine_reveal's matching binding, and
the concrete callback selected by its loaded component, have not been recovered
from this task's static inputs. Delivering a sensor entry and clearing its pending
byte does not demonstrate that this binding exists or accepted the start.

**Prior observation / bounded static correspondence.** The reported wire type is
2681 on PS4 56578 and 2120 on 51539. The start hash `0xAECA3822` matches the
previously decoded incident catalog's cinematic entry 2120 in
[the incident investigation](incident-diagnostics.md). The 56578 sender here passes
the hash, not literal integer 2681. Wire numbers come from the loaded incident
catalog; this pass did not derive the full 56578 or 51539 catalog or disassemble a
51539 executable. Do not infer a universal numeric offset between builds.

**Inference.** Absence of 2681 alone cannot tell whether the wrapper suppressed the
apply, a setter precondition failed, the resource binding was absent, the binding
rejected playback, or reporting failed. The reporter `0x403350` also obtains a
local player before constructing its event. Pending=0 does not distinguish these.

## What the current files and retail capture actually say

**Verified repository code.** In the checked-in `cine_reveal-play-0.txt`, the
cinematic value has counter 2 and **playing=0**. It cannot request playback through
this setter. The first cinematic row in `agr-step1.txt` is also stopped; the next
row is the one with playing=1. Using the first row as evidence that counter 2 alone
starts a cinematic is incorrect. This is separate from the flag-1 attempts
reported for cine-live8; their failure is still **open**.

**Verified repository code.** The reveal file declares one cast element, but its
first element at `+0xA0` is zero; its synthetic nonzero identity is eight bytes
later. The start routine consumes the zero element. That should be corrected if a
cast is supplied, but cast membership is not a gate in `0x403140` and this does not
by itself explain the missing start incident.

**Verified repository code.** `SensorPhaseUpdate::encode` in
`d1-oracle/src/sensor.rs` emits `has_sensor_block=1` when `authority` is `Some`.
`D1_AH_SENSOR_AUTHORITY=3,1,0` is parsed into such a table by the host. The introductory
single-entry example that skips the table does not describe every configured frame.
`APPLIED=0` is an unfinished transfer, not an absent table. Whether the selected
authority blocks and transfer completion are correct for the failed run remains
**open**; the setter itself does not inspect that table.

**Capture decode, not a PS4 native observation.** Re-read the private
`2026-10-27_19-55-14.A-Guardian-Rises.pcap`, host suffix `a7f6`, with the existing
`d1-srcp.py` reader and `d1-oracle sensor decode-batch` / `decode-values`. Sensor
controls passed first. Only control-field summaries were retained; no trace,
decoded image, player identity or exported fixture was written to the repository.

| Capture time (s) | Record | Frame stamp | Value `+0` | Counter | Playing | Preload | Roster / cast counts |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1058.927 | `/6/0`, `/6/1` | 108 | 0 | 1 | 0 | 0 | 0 / 0 |
| 1061.504 | `/6/0` | 133 | 132 | 2 | 1 | 0 | 0 / 0 |
| 1094.620 | `/6/0` | 463 | 0 | 4 | 0 | 0 | 0 / 0 |
| 1099.226 | `/6/1` | 509 | 508 | 2 | 1 | 0 | 0 / 0 |
| 1145.509 | `/6/1` | 973 | 0 | 4 | 0 | 0 | 0 / 0 |

The filter byte decodes to `0xFF` in all these values. This pass produces start
132 (`0x84`) for the first play, **not `0xC7`**, and each play's start is one less
than its containing frame's stamp. These measurements supersede the earlier
description for this capture only; they are not constants for the server.

**Inference supported by binary and capture.** Neither a nonempty cast nor a
second cinematic record is a demonstrated start prerequisite for `/6/0`. The
retail host used empty cast/roster lists. PS4 record enumeration, rather than the
360's two-record content layout, must determine what the PS4 host addresses.

**Inference, plausible explanation for the reported sequence of attempts.** If the
first counter-2 / playing-0 command was applied, all subsequent counter-2 /
playing-1 commands were no-ops at the counter comparison, irrespective of removing
`/6/1`. That explains why changing only the flag during that loaded instance could
not start it. This pass did not read back the failed run's counter or establish
which edits reached it, so this remains a hypothesis about cine-live8, not a
newly verified native diagnosis.

## Verification and handoff

**Verified tooling limitation.** The shared `d1-eboot controls` command fails its
01.29 fixed-address controls on 56578 (the build registry correctly marks the
global address set as not re-derived). Its global PASS status was not claimed.
The relevant paths above were read from 56578 and checked with build-specific,
independently read controls. Final four-kind xref runs passed all of:

```text
--control call:0x12AA7C0:0x403177
--control jmp:0x3000B0:0x4F162C
--control riprel:0x1A1F100:0x403154
--control rela:0x402EF0:0x19A3850
```

**Verified offline execution.** From `ps4/d1-server`, with `D1_EBOOT` naming the owned
56578 executable and `LIBCLANG_PATH` naming the local LLVM bin directory:

```sh
cargo test -p d1-oracle --test cinematic_start --offline -- --nocapture
```

Both tests pass. They use the existing ELF reader, descriptors, relocation-aware
Unicorn harness and import stubs. They skip with an explanation when `D1_EBOOT` is
unset or absent; a present malformed or wrong-build input fails instead of skipping.
No new codec or captured binary fixture was introduced.

The no-input run was also executed: both tests printed their `D1_EBOOT` skip
reason. Its first build attempt omitted `LIBCLANG_PATH` and failed in bindgen;
restoring that environment variable made the build and skip check succeed.
`rustfmt --check` passed for the new test file.
The required conventions check, with title
`d1-oracle: verify 56578 cinematic start gates`, passed with zero errors and zero
notices; the staged whitespace check also passed.

**Done / open.** The 56578 sensor setter, its edge-consumption behavior, the local
player dependency, the cast-copy layout, the engine-acceptance/reporting boundary,
and the retail control fields are established offline. The actual failed gate in
cine-live8, binding-specific playback readiness, and the complete incident-catalog
mapping remain open. No activity-host runtime change is claimed as a fix, so no
experimental host behavior was enabled or installed in the lab.

**Concrete next server change / experiment (inferred, not yet a native fix):** build
the reveal play value with `playing=1` and a **fresh per-record counter**, after
local-player and resource readiness; keep the proper absent object reference,
filter `-1`, and empty roster/cast lists as in the capture. A deliberate retry must
advance the last served counter again; derive it from the host's per-record state,
not a fixed 2/3/4 or a captured stamp. Keep timestamp zero for the first controlled
comparison, since the setter positive control accepts it. Before adding another
host message or changing the world phase, distinguish the wrapper's suppression,
`0x303BA0 == -1`, and `0x4E3F70 == false` in an explicitly authorized future native
run. Such a host retry feature should be off by default and unit-tested; it has
not been implemented here because delivery is not proof of a safe playback retry.

## Addendum: what `0x4E3F70` checks (Ghidra, 5 November 2026, Claude)

A first use of `d1-ghidra.ps1 -Decompile` (#289) on `0x403140`, `0x403770` and `0x4E3F70`. This reading is described in our own words; the decompiled C stays in the lab staging folder.

**`0x403140` (the setter).**
- It copies the 0xD4-byte record to `+0x1AC` on the cinematic object, which puts the counter at `+0x1B0` and the playing byte at `+0x1B4`.
- It returns at once if the counter didn't change.
- Next it compares the new playing byte with the object's **applied** state byte at `+0x280`.
  - If they match, nothing is done.
  - A playing byte of 0 stops the cinematic.
  - A playing byte of 1 needs a local player (`0x303BA0 != -1`) and goes on into the start.

**`0x403770` (the start).**
- It copies up to six 8-byte entries from the object into a global list. The entries are at `+0x24C` and their count is at `+0x248`; they're probably the participants.
- It reads an id from the cinematic's bound object (field `+0x68`) and calls `0x4E3F70(id, 1)`.
- The result **becomes the applied state** at `+0x280`. A rejected start therefore leaves the applied state at 0 while the counter has already been consumed, which matches the behaviour Codex saw.
- It then reports through `0x403350`. One of two constants is sent: `0x02651C94` on success and `0x02651C90` on failure. These are probably the started and failed incidents. Their values are still to be read.

**`0x4E3F70` (binding acceptance).**
- It scans a global table of 0x38-byte entries; the count is at global `+0x1350` and the entries start at `+0x1320`.
- For each entry it derives an id from the entry's two handles and compares it with the cinematic's id.
- **No match means the start is refused.** On a match with the start flag, it activates the entry through `0x354440` and clears the entry's pending byte. Without the flag, it only marks the entry pending.

**What follows.** The "binding-specific playback readiness" left open above is concrete: the cinematic's binding must already be registered in that client table when the play value arrives.
- Something client-side registers it, probably when the cinematic's content loads for the bubble.
- That fits cine-live8: a play sent soon after in_world was rejected.
- It also fits the talk's model (`d1-gdc2015-activity-hosts.md`), where the host sends a play only once the console has the content.

**Next.**
- Find the writer of the global `+0x1350` count, so we know what event registers a binding.
- Have the host send play (with a fresh counter) only after that event, or after the client's matching sense report.
