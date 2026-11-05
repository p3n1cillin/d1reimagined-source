# PS4 56578: a native RSAT codec plan (5 November 2026)

Replace production execution of the client's codec with a native, schema-driven Rust codec,
starting with the activity host's outgoing hot messages. Keep `d1-oracle` as the differential
test oracle. aknumbers (Destiny 1 Preservation Project) advises, as supplied for this task:
“def important to not rely on oracle for production, actually solve RSAT, it's not that hard”.
The immediate engineering reason is the connection-loop latency recorded in
[the stall investigation](ps4-56578-lab-stalls-2026-11-05.md).

**Stage 2 status:** all fixture mission sensor lists can be built natively behind
`D1_RSAT_NATIVE=1`; caller-mask replicated encoding is also implemented. Results and limits
are offline, recorded in [§5](#5-native-codec-status-5-november-2026).

Sections 1–4 began as **source analysis and a proposed validation programme** against revision
`33b6589`, without running tools during the route run then in progress. Section 5 records the
subsequent static analysis, offline controls, differential tests and benchmark. Historical
observations retain their original scope and build. No game or lab run was performed for either
native stage; future native-run reports must name both build 56578 and the emulator commit.

## 1. What RSAT means here

RSAT is the client's bit-packed object serialization grammar driven by schema descriptors.
An AH `user_defined_type` chooses the schema and wrapper; its payload is neither the decoded C
struct nor an investment message with an extra message id. Reuse the existing BAP envelopes,
message definitions and schema reader. [ObjectOracle](../ps4/d1-server/crates/d1-oracle/src/object.rs)
distinguishes object and message codecs explicitly; the negative `RawStructCodec` experiment in
[host codec.rs](../ps4/d1-server/crates/d1-activity-host/src/codec.rs) records why raw struct bytes
cannot substitute for schema bytes.

### Descriptors and build identity

[d1-eboot descriptors.rs](../ps4/d1-server/crates/d1-eboot/src/descriptors.rs) is the authoritative
reader, with these anchor-relative fields:

| Location | Meaning |
|---|---|
| anchor `+0`, `+4` | marker pair `0x80800050`, `0x8080010a` |
| anchor `+8`, `+0x10`, `+0x14` | field count, self tag, decoded struct size |
| anchor `+0x30 + i * 0x28` | field marker `m`; `m+4` alone is sentinel `0x3f000000` |
| `m-8`, `m+0`, `m+8`, `m+0xc` | C offset, serialized start, kind word, child tag |
| `m+0x14` | leaf width/parameter; for composites, potentially the parent count offset |

`serialized_start` is schema/mask positioning metadata, not a universal absolute wire offset:
optional fields and array lengths change the wire position. Header positions before the anchor
are not universal. `child_tag == 0xffffffff` denotes a leaf. The reader documents four earlier
parser defects, and `walk` checks field fit; its bounded diagnostic walk is not a complete codec
coverage analysis.

For AH schemas, reuse `activity_host::{derive, derive_all, schemas, tag_for_udt}` in
[activity_host.rs](../ps4/d1-server/crates/d1-oracle/src/activity_host.rs): call-site descriptor
global -> relocated runtime record -> unique other relocation/table slot -> checksum -> unique
static schema record -> anchored descriptor. `controls()` checks every row derives, independently
stated call-site sizes agree, tags are distinct, and join-result field zero embeds join-request.
Its same-size decoy-tag negative prevents size matching from masquerading as identification.

Use the **56578** `CALL_SITES_56578` rows and image SHA-256 selection in
[codec.rs](../ps4/d1-server/crates/d1-oracle/src/codec.rs), not the 01.29 public address constants
in neighbouring modules. The 56578 object encode/decode entries are `0x2c9a80`/`0x2c9a90`, replicated
decode is `0x2c9ae0`, and AH wrappers are `0x2a4b70`/`0x2a4ac0`. Their source records structural
re-derivation. No cross-build address arithmetic is part of this proposal.

### Field kinds, arrays and bits

| Construct | Evidence and native requirement |
|---|---|
| Composite, low kind byte `1` | Recurse into the child schema. Preserve offsets and defaults; do not treat a child tag as a wire prefix. |
| Optional field, high kind byte `1` | One field-presence bit precedes the value. `kind & 0x100` means optional, **not repeated**. Absence, present zero and present null are separate cases. |
| Scalar leaves | Bool `2`; integer/enum leaves include `3`, `4`, `5`, `6`. Width and signed bias depend on the leaf handler and parameters, not just the Rust storage type. The sensor phase is kind `3`, four bits, bias 1; the activity index is a kind-`4` 16-bit leaf. The retained older codec names kinds `7`-`10` as integer variants and `11` as real32; those mappings and specialized reference leaves require 56578 controls before promotion. |
| Fixed and dynamic arrays | The child descriptor supplies capacity, element kind/tag and strides; a composite's count-source selects a fixed count or a count loaded from the parent image. Serialize the count field according to its own schema, then the active elements; unused capacity is not transmitted. |
| Replicated object | Extra leading presence bit, then the object grammar under a change mask. Full-state encode uses an all-ones mask. Partial decode overlays a prior image; it must preserve fields absent from the update. |
| Sensor `fresh` | Separate from presence. It controls replica initialization before an overlay. A fresh default with no value is meaningful. Do not generalize the audited lifetime entry's discard behaviour to every sensor delta. |
| Typed/boxed value | Resolve a nullable runtime type handle and dispatch the selected schema. Registry/type-cell state matters; null must still emit its required encoding. Unknown selected types must refuse. |
| Typed value, kind `0x22` on 56578 | The relocated handler pair writes/reads a handle through its type cell, then the named object at `+0x10`. This includes udts 33/37. The old dynamic-flags refusal belongs to the 01.29 classification, not this dispatch; see §5's controls. The walker's change-mask bitset is a separate construct. |

The array proof is [array-fields.py](../ps4/capture-rig/array-fields.py), especially `PROOF` groups
A-E and H: it separates the two kind bytes, parent count-source at `m+0x10`, count offset at
`m+0x14`, and the array header's capacity at anchor `+0x20`, strides at `+0x28/+0x2c/+0x30`, element
kind at `+0x38` and child at `+0x3c`. **Those instruction pins are 01.29 reference evidence**;
do not run that script unchanged as a 56578 control. Extend the existing Rust descriptor model
to retain the missing array/leaf parameters, and validate those reads on 56578 with array boundary
controls. `raw_bits_or_count_offset` already warns against interpreting composites as bit widths.
The host's launch-array geometry and membership-array controls provide additional independent
size/stride checks, but cannot prove every array codec branch.
The older [server/investment.py](../server/investment.py) is useful reference evidence for leaf
variants and nullable handles; it is not a second 56578 specification to copy into Rust.

Wire bits are **MSB first**, continuous across fields, with the final byte zero-padded. Decoded
PS4 struct images use little-endian scalar storage: do not memcpy those bytes onto the wire.
`ObjectOracle::{encode, decode}` report actual bit positions and reject overreads. The enforced
object positive control in [control.rs](../ps4/d1-server/crates/d1-oracle/src/control.rs) checks the
character's SOID, identity and exactly 97 consumed bits. `sensor::run()` checks the 55-bit name
reference's hash and biases, phase -1/3/6 mapping to raw 0/4/7, body lengths and terminator.

The independent [Blam crosscheck](rsat-blam-crosscheck.md), implemented by
[tools/rsat_crosscheck](../tools/rsat_crosscheck/README.md), records 26/28 checks matching across
23 project encodings: optional/nested fields, signed biases, bounded arrays, framing and MSB order.
It validates useful primitives, **not AH schema completeness or replicated deltas**. The note and
script identify unsigned-u32 decoding and explicit QueueZ version as the two reported differences;
the tool README instead names a narrow-leaf bias difference. Resolve that reporting inconsistency
when rerunning, and assert named outcomes rather than accepting “two differences” blindly. Narrow
enums require explicit bias 1 regardless of the library default.

### Replication and the udt-5 wrapper

`activity_host::is_replicated` selects **11 and 21**, by client handlers. In `ObjectOracle`,
`encode_replicated` composes presence=1 with the ordinary full-state object encoder;
`decode_replicated` accepts an exact-size seed. `activity_host::controls()` round-trips solo
membership and udt 21, and its negative sends plain membership bytes to the replicated decoder
and requires a mismatch. The wrapper's return code includes a buffer-size check, so success
alone is insufficient: compare decoded data and consumed bits.

[sensor.rs](../ps4/d1-server/crates/d1-oracle/src/sensor.rs) already assembles udt 5 natively around
oracle-encoded subobjects. Its grammar is:

1. generation u8; authority-present bit; if set, 65 changed bits in block-index order followed
   by the replicated `0x80800cf7` authority object;
2. stamp u32; init bit; repeated entry-present bit, name-reference object, u32 body bit length,
   body; a zero entry-present terminates;
3. within an A body, fresh and presence bits, then the value when present; B adds its own has-B,
   flag, fresh/presence, object and trailing u32 as `SensorPhaseUpdate::encode` specifies.

`body_bit_length` counts the **whole entry body**, including fresh/presence and any B framing.
Append each encoded subobject's valid bits, not its padded bytes. The audited single lifetime
entry is 472 bits/59 bytes with a 339-bit value and body length 341; these are control targets,
not general lengths. Authority and extra entries move the stamp/value positions. Generation,
name, state, team and authority come from input/content/client reports.

Existing controls: `sensor::run()` checks audited phase/tail variants and length inversions;
`decode_frame_reads_back_an_encoded_frame`, `extra_clients_extend_the_audited_frame`,
`a_valued_client_carries_its_object`, `the_heartbeat_and_stamp_match_the_live_host` and
`a_stamped_frame_decodes_with_its_stamp` check framing. `FrameDecoder` exposes entry bodies as
bits; it does **not** decode every value or reconstruct every sensor delta.

Typed values need special care: `sensor::TYPED_VALUE_TYPES`,
`typed_value_tests::a_prop_command_round_trips`, `ObjectOracle::with_types`, and
[SchemaHarness::new_with_types](../ps4/d1-server/crates/d1-oracle/src/schema.rs) demonstrate
resolving prop-command and reward types outside the static schema tree. The same sources call
these “kind 0x22 typed values”; AH code inherited a dynamic-flags label from 01.29.
**Stage 2 resolves the 56578 conflict:** the relocated encode/decode table entries are
`0x102A3D0`/`0x102D220`, the same typed-value grammar in both contexts. Null/present values,
four payload types in udts 33/37, and populated prop commands now have differential controls.
[01.24 nullable-field findings](ghost-light-0124.md) remain historical, not an address source.
Sensor entity kind numbers are a separate namespace. Kind `0x23` identity boxes and atom
selector tables likewise require the measured
56578 type cells in `codec.rs`; do not reduce them to a generic integer by size.

## 2. What actually runs on the connection loop

[listener.rs](../ps4/d1-server/crates/d1-activity-host/src/listener.rs) calls decode in
`on_client_notification`, and encode through `try_push` ->
[AhPush::to_bap_body](../ps4/d1-server/crates/d1-activity-host/src/push.rs). `on_tick` considers
udts 1, 5, 11 and queued 18. `OracleWorker::call` sends a job and blocks on `rrx.recv()`;
a dedicated Unicorn thread therefore still holds the synchronous caller. `control_oracle`
separates long sensor builds, but does not make ordinary calls asynchronous. Old comments calling
this “one join and a handful of grants” or “well under a millisecond” do not describe the current
mission workload.

Priority below ranks removal value using observed turn costs and code frequency, not a newly
measured per-function benchmark. Sizes are **decoded images**, never fixed wire lengths.

The available **turn-cost order** is incident 18 (3.3-4.5 s), then ticks pushing 1/5 (about
1.8-2.0 s); 2 and 11 have no isolated warm timing. **Cold preparation** is a separate order:
large sensor lists (tens of seconds), then the expensive membership oracle setup. The captured
**outgoing frequency order** is 5 > 11 > 1 = 2 > 18, subject to the cross-build limit below.
Do not multiply these different measurements into a fictitious CPU-cost total.

| Priority / udt | Direction and codec | Schema tag / image size | Frequency and cost evidence |
|---|---|---|---|
| 1: **1 global_activity_state** | encode, ordinary | `0x80800e76`, child `0x80800b8f`; `0x6b0` = 1,712 B | Idle host heartbeat every 2 s, plus join and state refresh. Re-encodes via `publish_activity_state`. Ticks pushing 1 or 5 measured about 1.8-2.0 s. |
| 2: **2 world_globals_state** | encode, ordinary | `0x80800dc1`, child `0x80800d68`; 8 B | When configured, join and each state refresh, paired with activity state. No isolated latency measurement; small, useful first equivalence target. |
| 3: **5 sensor_auth_update** | send prebuilt bytes; oracle encode of subobjects during builds | **No whole-frame tag/size**. Name `0x808004b1`: 8 B; lifetime `0x808008a3`: `0x504` = 1,284 B; authority `0x80800cf7`: `4 + 65*12` = 784 B. Other A/B tags and sizes follow each sensor list/schema. | Once, configured repeats, mission transitions; optional stamp-only idle frames every 3 s. Normal send clones/restamps cached bytes, not a whole-frame oracle call. Tick cost is not proof of udt-5 encoding cost. Existing build comments record roughly 31-40 s for substantial lists. |
| 4: **11 replicate_membership** | encode, replicated full state | `0x808009ed`; `0x5038` = 20,536 B | Join, state refresh, membership/teleport changes. Largest hot image; prewarm comments identify it as most expensive of the original five oracles. No isolated current timing. |
| 5: **18 incident** | decode inbound; encode queued host incidents | `0x8080097d`; `0x2cc` = 716 B | Event-driven, potentially bursts. Observed incident turns 3.3, 3.7, 4.5 s: strongest direct reason to prioritize decode after the first encoder work. Incoming turns can also encode replies. |

The size/tag sources are `CALL_SITES_56578`, AH controls, `GlobalActivityState`, sensor constants,
`state::publish_world_globals`, and [the 56578 clock finding](ps4-56578-pilots-clock-combat.md).
[state.rs](../ps4/d1-server/crates/d1-activity-host/src/state.rs) defines `HOST_HEARTBEAT_MS=2000`,
sensor repetition and the stamp heartbeat. The listener normally polls at a quarter of the host
heartbeat, 500 ms; **these messages are not all encoded on every poll**. A slow udt-15 turn can
be expensive because it triggers a host heartbeat, despite the inbound payload being raw.

For frequency context, [the private-capture analysis](d1-live-captures-2026-11-05.md) records an
approximately 11-minute **last-generation live-server AGR session**: outgoing 5/11/1/2/18 counts
569/65/53/53/20, incoming 6/15/18/21/37 counts 1,375/178/173/54/32. These are not PS4 host
benchmarks, production rates to hardcode, or proof of identical layouts: last-gen udt 1 faults
under the 56578 schema. The stall note separately reports 56578 whole-turn timings and contention;
it does not establish that RSAT caused every freeze or that native encoding alone fixes them.

Other active paths, after these five:

- Join: decode 3 (`0x80800e98`, `0x100` B), encode 4 (`0x80800a58`, `0x1a8` B) and 0
  (tag derived by `tag_for_udt(0)`, `0x600` B). Refresh 17 (8 B) elicits 0/1/optional 2/11.
- Repeated incoming telemetry: replicated 21 (`0x220` B) preserves
  `client_authoritative_image` per connection; 36 acknowledgement (4 B), 22 identity (40 B),
  and other schema-coded reports still enter the ordinary worker. Numeric tags for these and
  udt 0 should come from the 56578 derivation inventory, not older-build notes; that inventory
  was not executed here. Include them in the coverage report before implementation promotion.
- Optional slot handover: decode 25/26 (`0x608`/`0x604` B), encode 23/24
  (`0x608`/`0x604` B); tags derive from their 56578 call-site rows. Incoming 19 (4 B) can cause
  an entity grant 0; 20 is a `0x600`-byte release mask. These are conditional/event work.
- Raw 15 is one byte; raw 6 is a bespoke sense frame with schema-coded B subobjects, not a
  generic AH struct. [SenseWorker](../ps4/d1-server/crates/d1-activity-host/src/sense.rs) already
  submits without waiting. Raw 8 is intercepted by the activity-host manager, not normal AH
  traffic. No invented RSAT tags for these.
- 33/37 (`0x80800a3d`, `0x210` B / `0x80800d06`, **56578 `0xb750` B**) are deliberately not
  decoded. `dynamic_flags_tags()` currently inspects top-level fields, despite some comments
  saying “tree”: a future native coverage classifier must inspect all reachable fields and
  selected runtime types. The old `0xb1e0` heartbeat size belongs to the reference build.

## 3. Staged implementation

### Stage 0: make the supported subset explicit

Reuse `d1-eboot`'s image/descriptor/relocation readers and AH tag derivation. Extend that one
descriptor model to expose array headers, count sources, and required leaf parameters. Build
a complete, cycle-safe schema closure for each selected udt, including runtime selected types;
fail on missing markers, unresolved children, unsupported kinds and out-of-bounds geometry.
Record a build-pinned schema fingerprint and capability report per encode/decode direction.
Keep private descriptor exports outside the repository; commit code and concise findings only.

Create one shared Rust RSAT core at the existing codec boundary, usable by `d1-bap`, the host
and offline tools. Extract/reuse the existing MSB readers/writers in `d1-bap` and sensor framing
instead of adding parallel implementations. Keep `AhWireCodec`, `AhMessage`, `ClientMessage`,
`sensor_list::params_for_list` and the envelope/seal code as the shared callers. A validated
schema model can be loaded once from an owned input at preparation/startup; production must
execute no guest code. Separating schema packaging from serialization avoids an accidental
promise that removing Unicorn also removes every owned-content dependency.

### Stage 1: native encoder, oracle only for equivalence tests

Implement checked MSB writes, scalar rules, optional/default decisions, composites, fixed arrays
and parent-count arrays. Start with 2 and the supported 1 closure, then 11 full-state encode,
5's name/lifetime/authority and each fixture A/B value, and 18. The table ranks operational
value; using the 8-byte world configuration as the first small implementation control is sensible.
Add join/entity/handover encoders as their schema closures become fully covered.

Require **identical payload bytes, bit count and final padding against d1-oracle for every
eligible fixture and captured case**, plus generated boundary cases. No promotion based only
on a successful decode. Keep the existing zero-launch-slot guard for udt 1: an unexercised
runtime-width element is not implemented merely because count zero works. For sensors preserve
the existing framing while replacing each oracle subobject call with the shared native engine;
do not replace the whole frame with a guessed schema. Unsupported types remain explicit gates.

Integrate by per-udt/schema capability, with actionable unsupported errors and no synchronous
production fallback on a native failure. An offline differential/shadow run can compare both
codecs; running the oracle on every production message defeats the objective. Ultimately move
the Unicorn dependency behind an offline/test boundary, including sensor helpers that currently
live in `d1-oracle`.

### Stage 2: native decoder and seeded deltas

Prioritize 18 decode because it has measured costly turns, then ordinary 3/17/19/22/36 and
other reports, and seeded 21. Implement dirty-mask/optional absence and defaults explicitly;
retain per-connection images and clear them on session/world epoch changes. Add 11 decode for
equivalence tooling, then sensor A/B decoding and udt-6 B overlays as their controls permit.
Do not deduplicate stateful updates or decode every delta against zero. `fresh=0` sensor updates
need prior state and the walker rules: raw body extraction is not full decoding.

### Stage 3: unresolved cases and the interim scheduling fix

Kind 0x22 flags, ambiguous typed-value dispatch, kind 0x23/type-cell-selected boxes and atom
variants, dynamic-width kind 0x29 in populated launch slots, and unpinned udts are separate
gates. Resolve each from 56578 handler/schema/content evidence with positive and negative controls.
Do not import 01.24/01.29 offsets, assume IEEE float bit-copy for every float leaf, or invent
runtime widths. The current oracle cannot certify paths that its own harness refuses.

Meanwhile, keep remaining emulation **off the connection loop**:

- Use a bounded worker job/completion interface, nonblocking submission and polling, with
  per-connection ordering and epoch tokens. Extend the successful `SenseWorker` pattern;
  replacing `rrx.recv()` with an async wait inside the same serial driver is insufficient if
  it still prevents that driver from servicing heartbeats. A serialized connection owner
  must continue to service socket input, completions and a heartbeat timer while jobs run.
- Prepare and cache immutable RSAT bodies before they are needed. Reuse the sensor cache and
  separate control worker; cache misses build asynchronously. Key by executable/schema identity,
  codec/cache version and **all** semantic inputs, including seed/mask for delta results. The
  present sensor key uses build text and parameters; strengthen it with image/schema identity.
  Publish bytes and measured stamp offsets atomically so a hit never requires an oracle decode.
- Cache unchanged full activity/world state and rebuild on actual changes; prebuild finite
  mission sensor variants. Stamp/init changes use the existing bounded bit patch. Generation
  or world changes invalidate the appropriate cached state. Never reuse membership identity
  across connections or a delta result against another seed.
- Queue only application payload work. Keep BAP sequence/ack state, conversation mutations,
  sealing nonces and socket writes under one ordered owner; cache no sealed packets. Maintain
  a ready, semantically valid heartbeat while jobs are pending. Never send udt 16 as an invented
  keepalive reply. On overload, hold dependent mission progression with a diagnostic; never
  silently drop required deltas, apply stale completions or block all connections behind one job.

Udts 33/37 remain explicitly unparsed while no host behaviour depends on them; moving an
unsupported decode to a worker does not make it supported. The scheduling work protects liveness
during migration, but is not the final native-codec deliverable.

## 4. Equivalence tests and promotion gates

Run this programme **between route runs**, never as part of this documentation task.

1. **Pin inputs and run the instruments' controls first.** Require the selected 56578 image
   through `D1_EBOOT`, repository fixtures through `D1_REPO` (or `D1_FIXTURES`), and the existing
   oracle via `D1_ORACLE`. Record image hash, repository revision, corpus counts and seed strategy.
   Run existing object/AH/sensor controls and the relevant host controls; run the independent
   Blam primitive crosscheck with its known named discrepancies resolved. Missing private inputs
   skip with a printed reason. Present but wrong/unreadable inputs, failed controls, empty selected
   corpora and unexplained refusals fail. Do not silently fall back to a different build.
2. **Fixtures are inputs, not all wire bodies.** Discover all applicable entries under
   [ps4/fixtures/56578](../ps4/fixtures/56578/README.md). Parse mission/world/step/sensor lists
   with the existing readers, including relative references, authority/phase overrides, team
   variants, A/B values, Breach wave lists, cinematics and draft post-game scoreboards. Compare
   native/oracle subobjects and assembled frames for every fixture case. Profiles, inventories,
   rosters and vendor objects exercise shared RSAT rules through their existing readers/oracles;
   do not feed them to the AH codec as arbitrary udts. Inventory unsupported fixture types
   explicitly. Assert case counts and supported/unsupported coverage so “all passed” cannot mean
   nothing ran.
3. **Private captures stay private.** Read bodies through
   [d1-srcp.py](../tools/ps4_setup/d1-srcp.py), reusing `d1_srcp` parsing and
   `activity-host decode-batch` (`UDT HEX [STREAM]`). `decode_all` deduplicates stateless pairs;
   `decode_ordered` preserves 11/21 ordering and prior images. Stream keys must distinguish
   connection/session, host, direction and udt; reset on reconnect. Exit zero from decode-batch
   is not success for every row: check row count, every JSON `error`, declared size and bit count.
   Reconstruct images from `size` and sparse little-endian qwords without discarding trailing zero
   regions. Route udt 5 to **`sensor decode-batch`**, then typed entry values as supported; raw
   udt 6 uses the existing sense decoder with metadata-derived B tags. Never run udt 5 through
   the generic AH tag lookup. Do not decode captures during the active route run.
4. **Three comparisons per supported ordinary case.** For an image `I`, require native and
   oracle encoders to produce the same bytes and bits; decode native bytes through the oracle
   and oracle bytes through native, comparing the complete defined image and consumed bits.
   Use actual nonzero fixture values, defaults, absent fields and one-field-at-a-time variations.
   Compare protocol-defined fields, not unspecified padding/pointers; any normalization must be
   explicit and justified by the schema, never a mask hiding a mismatch. Wrong-size images refuse.
5. **Capture identity needs masks.** For ordinary captured bodies require native decode to
   equal the oracle decode, and native re-encode to equal oracle re-encode; require exact captured
   bytes where canonical representation is established. For replicated 11/21 or sensor deltas,
   retain the original field-presence/change mask and seed if reconstructing the original wire.
   Existing `AhCodec::encode` writes full state: `encode(decode(delta)) == delta` is generally
   false. Compare both full-state encoders on the recovered image, both decoders on the original
   ordered delta stream, and later both delta encoders under the **same** mask. A native diagnostic
   decode can retain an encoding trace; adding an oracle mask-control API is separate work.
   Until masks/noncanonical choices can be reproduced, captured-delta byte-identity remains an
   explicit unpassed gate, not a waived check. Last-gen udt-1 failures are labelled incompatible
   input, never evidence against or for the 56578 schema.
6. **Boundary and negative controls.** Exercise every reachable kind, 0/1/max/max+1 array
   counts, optional absent/present-zero/present-null, signed extremes and explicit narrow biases,
   all 64-bit spill alignments, final padding and non-byte-aligned sensor bodies. Cover zero and
   one-hot masks, presence=0 preserving a nonzero seed, fresh/default versus overlay, A-only,
   B-only and A+B, authority/no-authority, empty/multiple entries, stamp wrap and generation reset.
   Test truncated inputs, excessive body lengths, unknown tags/types, invalid widths, unresolved
   metadata and wrong direction. Keep the existing decoy-tag, plain-versus-replicated,
   wrong-size and launch-count negatives. Generate valid cases from schema constraints rather
   than random bytes that only test refusal. Float edge/quantization tests follow each actual
   handler's supported domain.
7. **Prove the scheduling property separately.** Inject a deliberately delayed codec worker
   and cache misses without real emulation: heartbeat and socket servicing continue, queues stay
   bounded, same-stream completions remain ordered, stale epochs are discarded and no nonce is
   reused. Check cache invalidation across image/schema/input changes. After equivalence passes,
   benchmark cold/warm native and oracle costs per udt and report tail latency and queue delay.
   These are offline results. Only a subsequently requested 56578 native run, with emulator commit
   and client/host timing evidence, can establish improved in-game liveness.

Promote an encoder or decoder only when its schema closure is covered and every applicable
fixture/capture comparison passes. Report unresolved cases individually. Completion means every
enabled production encode/decode path executes native code, including sensor subobjects; the
oracle remains available for tests and research, and unsupported protocol work remains a named gate.

## 5. Native codec status (5 November 2026)

**Stage 2 is implemented and tested offline; no game or lab run.** The
[`d1-rsat` crate](../ps4/d1-server/crates/d1-rsat/src/lib.rs) reads descriptors from the owned
56578 executable (`D1_EBOOT`, SHA-256 `672f0341…0833`). Production native code links no Unicorn
and executes no guest code. `D1_RSAT_NATIVE=1` remains opt-in; unsupported schemas route by
capability, while a failure on a covered schema is an error, never an oracle retry.

### New handlers and their evidence

The 56578 encoder table at `0x1912C70` and decoder table at `0x19131B0` contain relocations,
not file qwords. `tests/stage2.rs` asserts each handler pair against those relocations and checks
all type-cell rows and atom-table order against the existing oracle table. Tags in the type
cells also have anchored descriptors in the owned image. No object bytes or binary fixtures
were added to the repository.

| Kind | 56578 encode / decode | Native behaviour and scope |
|---|---|---|
| `0x23` | `0x102A450` / `0x102D280` | Identity box through the selected type cell: byte-array `0x808004AC` or platform integer `0x80801932`. Both directions and both selector arms are compared on nonsymmetric identities. `Schemas::identity_format` makes the out-of-band choice explicit; the default is the existing offline oracle/server's byte-array arm. There is no selector in the wire body. Covers the player bundle `0x808022A5`, `0x8080254D` and scoreboard `0x808008D0`. |
| `0x22` | `0x102A3D0` / `0x102D220` | Typed value: encode/decode the handle through cell `0x1996050` (`0x80800046`), then, unless null, the named schema at `+0x10`. The gap is not transmitted. This is the same handler in udts 33/37 and prop state `0x808007D9`; it is **not** a dynamic-flags field on 56578. The 01.29 refusal is retained only for that build. The walker's separate change-mask bitset still exists. |
| `0x26` | `0x102A500` / `0x102D330` | Atom selectors at `+0`/`+4` through their two schemas; primary payload at `+0x10`, secondary at `+8`. All ten primary and three secondary variants are checked. The decoder reproduces `0x1C0950`'s initialization and preserves its padding. This closes `0x80800868`. Invalid selectors refuse before indexing a table. |
| `0x0D` | `0x1028780` / `0x102C8D0` | Position in the existing server/oracle's **unbounded-world mode**: three raw f32 lanes, with the decoded fourth SIMD lane read from the executable. IEEE edge cases compare in both directions. This closes `0x80802A1B` for fixture frame construction. Runtime world-bounds compression is a different, still-unimplemented mode; no bounds are guessed from sample positions. |

The atom secondary type `0x8080046B` reaches a quantized kind-11 real32. Its endpoint-preserving
quantizer (`0x917C0`/`0x91850`, called by `0x726A0`/`0x72C20`) is now native too, preserving f32
operation order, integer conversion and clamping. The exhaustive control covers every one of
its 2,048 wire codes, plus IEEE encode edges.

That control exposed the Windows Unicorn 2.1.5 scalar-VEX aliasing defect already documented in
[`scalar_float.rs`](../ps4/d1-server/crates/d1-oracle/src/scalar_float.rs): a three-source
instruction can execute destructively. The existing instruction interpreter is reused in the
statically identified RSAT quantizer range only, with independent aliasing/IEEE controls. The
native codec follows the executable's instructions, not that emulator defect. No guest image
bytes are patched. Investment-curve users keep the interpreter's finite-only mode.

### Coverage before and after

| Structure | Stage 1 | Stage 2 |
|---|---|---|
| Pinned schema-coded AH udts | 31 native, 33/37 gated | **33 native, encode and decode**; none of the pinned schema-coded udts falls back |
| Sensor value schemas represented in fixture lists | Four native, six gated | **All ten native**, including all four kinds above |
| Fully native nonempty fixture lists | **1 / 64** | **64 / 64** |
| Empty / authority-only fixture lists | Omitted from the collector | **2 / 2**, explicitly checked |
| Total fixture sensor lists now collected | 64 | **66 / 66 native**: 65 mission and one postgame |
| Fixture A/B values | **390 native**, 392 on oracle | **782 / 782 native**, encode bytes and decode images identical |
| Raw AH 6, 8, 15 | Verbatim | Verbatim; schema-coded raw-6 subobjects remain separate work |
| Responder schemas | Native in library | Same; the responder still invokes the oracle executable |

The newly included lists are `mission/sensor-clients-cine_reveal-empty.txt` and
`mission/agr-postgame-phase7.draft.txt` (authority only). World tables and step schedules are not
sensor lists. Frame comparisons combine each fixture's clients and authority with a synthetic
lifetime record; they do not claim to execute its mission schedule.

### Byte equality (owned 56578 input present)

All comparisons require equal bit counts as well as the corresponding bytes; decode compares
**every struct byte**, including padding/default initialization, not selected offsets.

| Comparison | Stage 2 result |
|---|---|
| Every native AH udt, zero/null image plus 16 seeds × in-range/raw images | **1,089 images across 33 udts**, identical encoding (stage 1: 1,023 / 31) |
| Hot AH udts 0, 1, 2, 4, 11, 18 | **384** additional images, identical |
| Single-field variations | **49** identical; five counts above capacity safely refused |
| Decode the generated AH encodings | **421** identical bodies across 33 udts; eight raw-overflow bodies refused as before (stage 1: 395 / 31) |
| Mutated/truncated AH bodies | **330** cases: 195 identical, 133 short bodies refused by both, two unsafe counts refused before client overrun |
| Fixture A/B values | **782** identical encodings and decoded images across the 66 collected lists |
| Whole udt 5 frames | **75 cases**: 73 in the complete nonempty/synthetic sweep, plus the two newly collected empty/authority-only fixtures checked separately. Both frame decoders agree on entries, bit counts, authority and stamp position |
| Identity / typed values / atom arrays / raw positions | **32 / 83 / 30 / 8** additional images identical, both directions; the 30 arrays cover every atom selector combination |
| Atom quantized real32 | **2,056** cases: every 11-bit code plus eight IEEE encode edges, identical |
| Explicit-mask replicated deltas | **763** byte-identical encodings and identical ordered overlays over nonzero seeds |
| Responder generated objects | **48** identical encode/decode; one unsafe profile count refused |
| Served responder bodies | **Six profiles and 25 items**, decoded images identical and re-encodings reproduce served bytes; vendor object images still agree |

The normal whole-frame test now enumerates all 66 lists without filtering by native capability
or taking a small prefix. Any gated fixture fails the test. Differential tests print `SKIP`
without `D1_EBOOT`; a supplied existing executable that cannot load fails. Missing fixture
directories fail rather than silently producing an empty corpus.

### Replicated deltas

`Encoder::encode_replicated_masked` accepts explicit low-bit-first u32 mask words. Bit zero
controls whole-object presence; field traversal begins at bit one. Unselected optional fields
preserve the seed on decode; nonoptional fields still serialize when the root is present.
Nested typed objects start their own full masks, as the client does.

`ObjectOracle::encode_replicated_masked` independently calls the client's actual `0x2C9D20`
entry with an external change-mask object, avoiding the live mask allocator. Its full-mask
output is controlled against the pre-existing full-state composition before comparing deltas.
Tests cover zero, root-only, every one-hot bit, full and generated masks for udts 11/21 and the
sensor authority table, advancing native and oracle seeds through the same ordered stream.
Wrong mask lengths and dirty padding refuse. This API does not infer dirty fields or change
production's existing full-state default. Captured-delta identity remains unpassed.

### Latency micro-benchmark

The ignored release test `latency_per_fixture_udt5_frame` selects the smallest, median-size
and largest fixture frames
from the full corpus. It includes native routing, image cloning, packing and stamp-cache
insertion, and compares the current oracle `SensorPhaseUpdate::encode` path, including its
per-object harness construction. Startup schema loading, cold control/filesystem work, socket
queues and scheduling delay are outside the timed samples. Each case uses five batches of 100
native frames and five oracle frames; results are medians, not an in-game liveness claim.

Measured on Windows at BelowNormal priority, release build, with no competing test process:

| Fixture / frame size | Native, microseconds per frame | Existing oracle builder, milliseconds per frame |
|---|---:|---:|
| `mission/sensor-clients-cine_reveal-empty.txt`, 59 bytes | **1.365** | **874.295** |
| `mission/agr-wall-done.txt`, 2,074 bytes | **30.178** | **3,833.022** |
| `mission/agr-bw-107.txt`, 7,054 bytes | **112.739** | **39,209.590** |

All three frames were checked byte-identical before timing. The timed workload was 1,500
native builds and 15 oracle builds, selected from 66 eligible lists. The large ratios include
the oracle builder repeatedly loading/scaffolding its object harnesses; these are full frame
builder measurements, not a comparison of cached primitive leaf operations. The benchmark
passed in 270.62 seconds. It measures neither cold startup nor end-to-end network latency.

### Validation and remaining gates

- The full `d1-rsat --release` differential run passed. Subsequent focused runs passed the final
  source changes and added empty-list cases; the 64 nonempty whole-frame comparisons were not
  rerun after adding those two collector cases. The benchmark is opt-in and ignored normally.
- `d1-activity-host --release`: **197 tests passed**, including native trait integration and the
  corrected build-specific udt-37 control.
- `d1-oracle --release --lib`: **69 passed, one unrelated failure**: character controls require
  the absent private `item-content-index.tsv`. Cinematic-start and native-atom integration:
  **four passed**. The 01.29 Oodle probe skipped with its stated build/input reason. The older
  vector suite had **11 passes and three failures**: two 01.29 size assertions against 56578,
  and an unconditionally opened absent reference executable. These were not changed or counted
  as passing codec gates. All **five 56578 wire-catalog integration tests passed** separately.
- Workspace-wide `cargo test --workspace --release --no-run` fails on Windows because existing
  `d1-live` code requires Unix APIs (`waitpid`, `std::os::unix`, signals). No live tools ran.
- No comparison against private captures, no native game/lab run, no claim about connection-loop
  liveness. Those remain separate gates.
- Runtime world-bounds position compression, integer-width-zero decoding, raw-6 sensor worker
  subobjects, production mask generation and responder integration remain follow-up work.

AI-assisted: yes. No pull request was opened for this branch.
