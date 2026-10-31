# Destiny 1 clock, physics, presentation, and VRR pipeline

## Executive result

Destiny 1 cannot be made correct at 60 Hz by changing the three familiar timing constants or by
passing the display interval into Havok. The client builds one asynchronous frame graph whose
simulation, deferred completion, publication, rendering, input, audio, and external effects cross
the apparent frame boundary. The safe design is a fixed-step simulation clock inside a separately
paced presentation loop, with explicit ownership for every repeated job and once-only effect.

This branch contains static clock-model and observation foundations that can be implemented and
tested without booting the client. It does **not** contain a live D1 multi-tick scheduler, live
rate-control path, immutable-snapshot interpolation path, or live VRR integration:

- an exact rational wall-credit clock;
- distinct pinned `StockExact` and evidence-only `LabT1ThreeWrite60Control` profiles;
- a rational-authority candidate type that requires an explicit guarded tick-context `h` route;
- preflighted tick admission, infallible commit publication, and abort;
- explicit retain/drop debt and rate-epoch policies;
- exact per-frame conservation receipts;
- synchronized epoch-tagged snapshot-generation IDs, a deterministic single-authority event-ID
  allocator, and a separate bounded synchronized delivery ledger (not yet an immutable payload
  store or external-effect integration);
- exact rational interval and request pacers;
- rate-policy parsing for rational, uncapped, and VRR presentation;
- a Packet B/C/D **claim-completeness** model, including per-rate matching; it is deliberately not
  a runtime authorization gate;
- exact Packet B0 landing-byte and displaced-span guards, with relative-relocation sites classified
  and X3 proved as a static table-owned no-op rather than an unsafe runtime detour;
- a default-off, Linux-x86_64-only Packet B0 producer and trace-format decoder. A1 armed it and was
  structurally accepted, but A1 is rejected for non-perturbation and is not authorization. Fresh
  clock A2 on the return-identity carrier `07ac0350` then executed all 34 probes through an in-world
  stock scenario and ended with an accepted receipt: A2 is a strong B0-shaped structural candidate,
  and formal Packet B0 promotion is HOLD;
- a separately versioned KLC1 v2 raw Packet B1 producer with five copy-safe probes: locked drain
  snapshot, Havok wrapper/continuous-target/result, and render scalar store. It has strict packet
  selection, exact-source preflight, manifest-bound receipts, zero-drop/coverage rejection arms,
  and no B0 fallback; it is live observation machinery, not rate authority;
- a separately selected KLC1 v4/flags-`3` Packet B2 producer with 39 copy-safe B1/C0/deferred/F2
  probes, universal per-occurrence lifecycle validation, compiled-manifest custody, a fail-closed
  optimized callback gate, a sealed R8 offline carrier, and a separately sealed supplemental
  live-input carrier. A1/A2/A3 remain prelaunch or contextless negative evidence. A6 is the first
  sealed guest-executing B2 run: its producer stopped orderly with a valid bounded transport record,
  but its 19 absent paths and inspector contract defects make it non-candidate evidence. The failed
  A6 state is preserved by R4, and the next bounded same-ELF trace-disabled arm uses sealed R8
  context custody rather than the historical R3 carrier;
- deterministic **model** controls covering 30, 45, 60, 72, 90, 120, 144, `60000/1001`, and
  irregular presentation opportunities; they are not implemented live scheduler/rate modes.

`d1_clock_pipeline.cpp` is currently linked as model code only: no production caller owns its
`BeginPresentation`, `BeginTick`, `CommitTick`, or `EndPresentation` operations. Likewise,
`SnapshotGenerations` tracks synchronized generation IDs only; it is not immutable payload
storage and has no reader pins or retirement protocol. B2 is executable observation machinery,
not a scheduler, wrapper dispatcher, tick-context owner, or join implementation.

It does **not** claim that Destiny 1 currently runs correct 60 Hz physics, interpolated 144 Hz
motion, VRR, or any arbitrary 60/72/90/120/144 Hz scheduler mode. Those runtime claims remain
gated by unmodified-stock observations and an exact-stock refactor oracle. No public model object
can directly enable a live patch; an authorization-grade verified receipt type and verifier do not
exist yet. The B0 sidecar has a separate offline validator and grants no authority.

| Area | Present in this branch | Still required before a live rate change |
|---|---|---|
| Clock math | exact rational credit, debt, epochs, receipts, stress controls | bind it to one simulation-thread owner |
| Physics `h` | exact mathematical binary32 values and explicit route types | observe and implement a per-tick routed owner; do not rewrite the 97-reference global |
| Guest tick | begin/reserve/commit/abort state machine | extract C1 stock-equivalent boundaries and retirement fences |
| Presentation | pure rational/uncapped/VRR policy and pacer | two-worker VideoOut path, generation-tagged requests, fake-clock tests |
| Observation | raw-source custody, mapped pre-detour hash, exact landing guards, X3 table/no-op proof, default-off B0 producer, structural A1 result, durable terminal-`E8` lowering, executed fresh B0 A2 with an accepted receipt, B0 trace-schema smoke, offline sidecar validation, a distinct five-probe raw B1 producer/decoder with bounded stack sampling, frozen stock B1 A2 reaching `activity:in_world` with an accepted 49,452-record raw candidate, an authoritative offline-green and independently audited/sealed 39-row B2 wire-v4 observer, failed A6 live evidence, R4 custody, and the R8 same-ELF trace-disabled control | one valid-controller A7 non-perturbation run; if implicated, an enabled repeat and localized observer repair; then accepted stock B2 plus same-run F3/F6/F8 lifecycle extensions, per-outer phase proof, and non-perturbation closure |
| Authorization | offline claim checklist only | artifact-derived opaque verified receipts; no authority is granted by the B0 sidecar |

## The four clocks

The implementation and every future review should use four different names. Calling all four
values "FPS" recreates the original bug.

| Clock | Owner | Meaning | May drive Havok `h`? |
|---|---|---|---|
| Wall measurement | host monotonic clock | elapsed real time admitted after pause/load/clamp policy | No |
| Virtual vblank opportunity | VideoOut/guest contract | when the guest observes a display opportunity | No |
| Simulation authority | Destiny 1 scheduler | fixed rational number of authoritative ticks per second | Yes, as one fixed `h` per clock epoch |
| Host presentation | swapchain/panel policy | when a ready image is submitted or scanned out | No |

The desired configurations are combinations, not one number:

| Example | Simulation authority | Presentation policy | Expected behavior after all gates |
|---|---:|---:|---|
| Stock oracle | stock profile | stock | byte/field/order-equivalent baseline |
| 60 Hz physics on 60 Hz display | 60/1 | 60/1 | one tick and one presentation opportunity per interval |
| 60 Hz physics on 144 Hz display | 60/1 | 144/1 | zero or one tick per presentation, immutable-snapshot interpolation |
| 60 Hz physics on 59.94 Hz display | 60/1 | 60000/1001 | exact rational pacing; no rounded integer period |
| 60 Hz physics with VRR | 60/1 | VRR | request-driven host presentation; physics remains fixed |
| 90 Hz VR mode | separately validated 90/1 | 90/1 | shared gates plus a distinct 90/1 authority receipt |
| Uncapped host submission | fixed validated rate | uncapped | consume ready images without minting ticks or frames |

"Arbitrary" therefore means a validated rational policy. It does not mean an arbitrary floating
delta passed to the simulation. Deterministic model controls cover the named presentation rates
30, 45, 60, 72, 90, 120, 144, and `60000/1001`, plus irregular presentation opportunities. Other
valid rational policies remain model inputs, not individually controlled runtime claims;
simulation authority above stock remains a per-rate runtime-validation problem.

## Why the three-write 60 Hz patch is only a control arm

The existing T1 laboratory patch changes three independent initializer fields:

| Guest site | Stock literal / realized field | T1 literal / realized field | Meaning |
|---:|---:|---:|---|
| `0x5918b3` | `0x42057777` / `CFG+0x08 = 33366 us` | `0x41857777` / `16683 us` | source pump |
| `0x5918c7` | `30` / `CFG+0x38 = 30` | `60` / `60` | legacy integer rate |
| `0x5918d1` | `0x42055555` / `CFG+0x40 = 33333 us` | `0x41855555` / `16666 us` | nominal catch-up |

Those values are not one coherent 60 Hz clock: `16683 us` is approximately a 59.94 Hz pump,
`CFG+0x38` supplies a 1/60 fixed step through the current global route, and `16666 us` is another
rounded nominal value. `ClockProfile::LabT1ThreeWrite60Control` preserves that incoherence and is
never an authority candidate. The patch does not add:

- a rational wall-credit accumulator;
- zero/one/many tick scheduling;
- fresh per-tick job contexts;
- child-job retirement fences;
- immutable render generations;
- interpolation;
- once-only input, network, audio, UI, persistence, or event ownership;
- a policy for raw tick-count gates;
- separation of guest vblank from host presentation.

That patch is useful as a negative comparison because it exposes deferred catch-up and half-speed
behavior. It is not the implementation route.

## Exact clock contract

### Stock is not a normalized rational 30 profile

The stock client carries several non-coherent timing representations at once:

- source initializer `0x42057777`, realized as 33,366 microseconds;
- installed legacy integer rate: 30;
- nominal catch-up initializer `0x42055555`, realized as 33,333 microseconds;
- fixed physics step: float bits for `1/30`.

`ClockProfile::StockExact` preserves those fields independently. A rational 30/1 profile is a
different object. This distinction is required for Packet C1: the refactor must reproduce stock,
not silently normalize it into a cleaner clock.

The raw-to-realized contract is instruction-pinned. At `0x592e8f` the client loads binary32
`1000.0`; `0x592ed2`/`0x592ed7` multiply the source initializer with `vmulss` and convert it with
the signed-64-bit form of `vcvttss2si`, storing `CFG+0x08` at `0x592edc`. The nominal field follows
the same pipeline at `0x592ef4`/`0x592ef9`, storing `CFG+0x40` at `0x592efe`. The model therefore
applies the required default MXCSR ties-to-even rounding to the binary32 multiply and then truncates
toward zero. It rejects negative, zero, non-finite, overflowing, or raw/integer-mismatched claims.
The four stock/T1 pairs above are pinned controls for that exact pipeline; a live refactor must also
verify or establish the expected MXCSR mode.

A rational authority candidate keeps its mathematical period separate from both raw legacy
initializers and both realized integer fields. It is constructible only with an explicit
`ExplicitTickContext` `h` route, the same canonical rational rate carried by that route, and a
nonzero claimed landing-contract digest. Comparing the rate as well as its rounded float bits
prevents a nearby rational with the same binary32 value from borrowing authority. This still makes
it a candidate, not authorization. Any integer field retained in the legacy client requires an
internally truthful compatibility mapping; flooring 60 Hz to 16,666 microseconds or 144 Hz to
6,944 microseconds is not labeled exact, while an exactly matching field cannot be mislabeled as a
compatibility exception.

### Future rational scheduler model

For simulation rate `p/q`, one tick costs exactly:

```text
tick_duration_ns = 1,000,000,000 * q / p
```

The offline model stores wall credit as a reduced rational using checked 128-bit arithmetic. A
future live scheduler must apply the following rules; the list is not implemented guest scheduling:

1. Classify every presentation as active/accepted, inactive, or negative/rejected. Only the first
   may admit wall credit or spend debt retained from an earlier active frame.
2. Clamp a hitch explicitly and report every discarded nanosecond.
3. Compute due ticks from exact credit; do not round a nominal period.
4. Before guest work, reserve a checked post-tick credit state and preflight all counters.
5. Commit is then an infallible publication of the reserved state only after the guest tick and
   every required retirement fence succeed.
6. Abort discards the clock reservation without consuming credit only when no guest work began.
   It is an accounting rollback, not a rollback of guest writes, ingress consumption, queued jobs,
   publication, or effects. Once guest execution begins, a partial tick or pending required worker
   retirement must fail closed; it may not be converted into `AbortTick()`, committed, or carried
   through `EndPresentation()`.
7. At the outer boundary, retain whole-tick debt or, only for an active/accepted frame, drop and
   report it according to an explicit policy. Inactive and negative frames must retain old debt.
8. Derive interpolation alpha only from the fractional remainder.
9. Change rates only when no whole-tick debt exists. Preserve or discard only the exact fractional
   wall-duration remainder, preflight the candidate rate's due-tick and fractional-remainder
   calculations before publishing the new epoch, and report a discard as an exact
   numerator/denominator pair.
10. Emit and verify the exact identity `opening + admitted = committed + dropped + closing` for
    every frame. Also verify the canonical rate and tick duration, count-to-credit products, the
    `executed + dropped + retained = due` partition, `closing = retained * h + fractional`,
    `fractional < h`, and opening-to-final tick advance. Floating-point alpha is display-only
    telemetry, never accounting authority; its consistency with the exact fraction is checked.

The wall delta is never itself a physics delta.

### Host request pacing

`RationalRequestPacer` is an upper bound on consuming already-ready presentation requests. Its
first-ever request is admitted immediately. Later deadlines are absolute rational offsets from a
monotonic epoch and use ceiling division, so a fractional deadline is never admitted even a
fraction of a nanosecond early. A late wake advances to the first future slot and reports skipped
slots; it never generates a catch-up presentation burst. The queue owner must call
`OnQueueBecameEmpty` on the locked nonempty-to-empty transition. That increments the schedule
generation but retains the post-present not-before boundary. A new idle request rebases phase only
after that boundary has elapsed, preventing a post-idle pair of near-adjacent presents. An idle
rate change inherits the same boundary, so reconfiguration cannot erase a pending cooldown.
Calling the pacer does not create a guest frame, snapshot, authoritative tick, or physical scanout.

The current VideoOut path still needs the worker split described below before this pacer is wired
to a live presenter.

## VideoOut integration contract

The stock host path currently combines two responsibilities in one thread: Vulkan presentation
and guest vblank/status/event generation. A blocked acquire or present can therefore stall a
guest waiting for vblank. It also uses one rounded integer nanosecond period and historically read
`flip_rate` and `is_open` concurrently without synchronization.

This static branch deliberately leaves that live path untouched. Atomic scalar substitution alone
would not establish a coherent port lifetime, and rejecting a closed handle or negative rate would
change guest-visible behavior without a compatibility decision. The integration must address the
existing open/close, flip-rate, status-reset, event-vector, buffer/group, and queued-request races as
one reviewed ownership change. Do not present a scalar-only patch as VideoOut race safety.

The opt-in path should be completed as two workers while retaining the exact legacy loop when the
feature is disabled:

### Virtual-vblank worker

- owns the guest vblank timeline;
- uses `steady_clock` absolute deadlines and exact rational remainder;
- updates vblank status, signals registered events, and wakes guest waiters;
- never calls Vulkan presentation;
- wakes and exits on close or stop.

### Host-present worker

- owns Vulkan/ImGui presentation calls;
- wakes on a ready flip, close, policy change, or rational deadline;
- rational mode consumes at most one ready request per eligible slot;
- uncapped mode consumes ready requests immediately;
- VRR mode is request-driven and reports host capability/present mode honestly;
- never advances guest time or simulation credit.

Port open generation must travel with queued requests. Close increments the generation and wakes
both workers. A stale request may not present, decrement counters, or signal events in a newly
opened generation. No port, queue, or event lock may be held across a Vulkan call or event-queue
callback.

The future `SetFlipRate`/close pair, policy reconfiguration, queue-empty phase reset, and
request-generation capture must be driver-owned under one lock; the current live path does not yet
provide that ownership domain. A generation must be checked before preparing a request and again
after any GPU-thread handoff; a post-present status/event update must also refuse a stale
generation.

Mailbox, FIFO, and Immediate present modes do not by themselves prove physical VRR. Validation
must distinguish attempted present time, compositor acceptance, and actual panel scanout.

## Guest frame-graph evidence

The outer builder at `0x592820..0x592db0` constructs and submits one asynchronous graph. It is not
a synchronous function that can simply be called twice. The graph contains 14 F jobs, 8 X jobs,
and 7 Y jobs, with cross-table dependencies that allow rendering work to begin after intermediate
simulation work.

Important established boundaries include:

| Boundary | Address | Current contract |
|---|---:|---|
| Outer builder | `0x592820` | starts one graph construction |
| Post-publication/pre-graph seam | `0x592934` | clean observation seam, not yet a scheduler seam |
| Master submit complete | `0x592d86` | landing is after the submit call |
| Fixed-time publisher | `0x590a50..0x590a75` | loads the block value, stores timing fields, increments the tick, returns |
| Completed-publication caller seam | `0x74a984` | next callsite after `0x590a50` returns; pinned future-observer guard |
| F1 | `0x74a780` | builds per-tick state and performs one fixed-time publication |
| F10 | `0x74b770` | deferred drains, close, playback, release, block lifecycle end |
| Havok wrapper | `0xcace80` | RDI owns wrapper/world object; XMM0 is physics delta |
| X5 | `0x804d20` | render extraction path |
| Render extract | `0x7eaf40` | lower-bound snapshot consumer evidence |

The global fixed-rate getter has 97 references across 78 functions. It is not a Havok-only value.
A global replacement changes loading, outer-loop, and other consumers. Any future elevated-rate
implementation needs callsite classification and a routed physics `h`, not a universal getter
rewrite.

The continuous-simulation construction path and atlas pin its simulation `vfunc[4]` target to
`0xcb5330`; its guarded entry bytes are `554889e553504889fb8b433083f801770c`. At wrapper dispatch
`0xcaceb2`, RBX is the world, RDI is the simulation object, RAX is its vtable, and XMM0 is `h`;
`0xcaceb5` observes the returned EAX result. The indirect call instruction itself is not a safe
detour landing because changing its callee-visible return PC would repeat the A1 identity defect.
A live balanced observer should use the concrete target entry plus the wrapper result, while still
recording enough wrapper context to reject a different simulation mode rather than assuming every
world uses the continuous target.

## Packet B0 observer design

This branch contains a default-off Packet B0 producer for Linux x86_64 only. A1 armed 34 probes and
produced 306,492 records with zero dropped and zero unknown records; its atomic receipt was accepted
and the trace was accepted structurally. A1 is nevertheless rejected as non-perturbation evidence.
In the same run, the managed-session request received HTTP 201 and a full response body, followed by
a local managed-session result of `-1`; transport success and body completeness are not a proof of
the client-local session outcome. Neither result is runtime authorization or a rate-change claim.

The rejection is specific: a terminal displaced `E8` `call` was relocated as a call in the
trampoline, so the callee saw a trampoline return PC rather than the stock continuation. The durable
remedy is terminal-`E8` lowering that preserves the original continuation as the callee-visible,
same-thread, synchronous return PC before transferring to the resolved callee. This is the only
return-identity safety claim. It does not claim identity for asynchronous signals, exceptions,
unwinders, profilers, or arbitrary stack walks. The lowering is eligible only when SHSTK is
established inactive before guest threads start; otherwise arming fails closed. Installation remains
at the pre-thread seam. A1 cannot be repaired by reinterpretation: a newly frozen A2 after this
durable lowering was required, and carrier `07ac0350` supplies the lowering. Fresh clock A2 on that
carrier passed the exact route that failed A1: it reached orbit and `activity:in_world`, installed
and executed all 34 probes with zero dropped records and zero unknown sites, and published an
accepted receipt after an orderly producer stop. What A2 does NOT supply is non-perturbation
evidence, end-to-end disabled-path equivalence, or per-outer phase proof, so formal Packet B0
promotion remains HOLD and no rate change is authorized.

The rejected permanent-UD2 prototype remains rejected: synchronous clock, thread, memory, or file
syscalls at every hot SIGILL would perturb the schedule it is meant to measure.

When the explicit B0 environment gate is absent, the producer leaves guest text, the trampoline,
trace output, and producer threads untouched. When enabled, loader-owned raw-source SHA-256 custody
is enforced before arming, and the SHA-256 of the mapped image before any detour is recorded in the
trace metadata. Caller-supplied expected JSON is not custody. This is tamper-evident local
provenance, not remote attestation against a malicious local process.

### Implemented B0 constraints and remaining gate work

The implemented producer uses permanent rel32 detours staged before `main_thread.Run`, not a
restore/re-arm trap cycle. It is intentionally not a gate-grade producer yet. Its static contract
includes:

- all-fallible detour staging, including relocation and trampoline capacity, before the first
  source-byte write; a failed batch therefore leaves source bytes untouched;
- transactional use of the module's existing trampoline reservation and patch mutex;
- instruction-boundary decoding and relocation for every displaced span;
- durable terminal-`E8` lowering that preserves the stock continuation for the callee-visible
  synchronous return, with SHSTK-inactive/pre-thread-install eligibility enforced fail-closed;
- exact pre-patch bytes and overlap rejection before any source write;
- a fixed-size, preallocated, nonblocking multi-producer record ring;
- callbacks that preserve full YMM state and only capture registers and publish a ring slot - no
  file, clock, allocation, mutex, or memory-read syscall on guest workers;
- a drain worker that owns drain-stamped timestamps and file I/O; the drain-owned `sample_a` and
  `sample_b` fields are reserved and zero-filled in B0, so B0 performs no memory sampling;
- explicit ring-overflow and record-drop counts;
- lock-free producer admission that closes and waits for admitted callbacks before drain/close;
- an atomically published receipt sidecar bound to the final trace SHA-256 and byte size, labeled
  `orderly producer stop` rather than a clean client shutdown.

The offline sidecar-receipt validator and its positive and rejection controls are implemented.
They validate receipt schema; adjacent absolute or relative trace-path resolution; trace
size/SHA-256 binding; orderly producer-stop and I/O status; header/receipt budget agreement;
positive ring capacity; counter conservation; and complete successful-install records. The CMake
gate exercises a nested relative output path and rejects an altered receipt hash and missing
fixed-publication coverage. This is not a B0 stock-authority receipt: end-to-end disabled-path
equivalence, non-perturbation, and per-outer phase proof remain outstanding; B1 has since closed its
five-probe live-execution rung. B2 has closed its production/default-off build, format/producer,
optimized-callback, detour-ABI, exact-source preflight, and immutable base-plus-live-input carrier
gates. A1 and A2 stopped during prelaunch; A3 is contextless-client negative evidence. A6 then
became the first sealed guest-executing stock B2 run. It established bounded producer transport,
healthy presentation, successful server login, and an ordered media-check-to-bootload stall, but
not a B2 candidate or any rate authority. R4 preserves its changed current canonical state without
retroactively passing it, while the sealed R8 context supports the one-controller, same-ELF,
trace-disabled A7 control. The earlier B0 A1's accepted receipt
validates its recorded producer facts,
not non-perturbation, and does not authorize a rate change. The fresh A2 required after durable
terminal-`E8` lowering has since been produced on carrier `07ac0350` and its receipt was accepted;
that closes the execution and orderly-stop half only, and every item listed above stays outstanding.

After the budget closes, callbacks may perform only one cheap disabled check until teardown. They
may not continue full capture work for the process lifetime.

### Implemented raw Packet B1 packet

Packet B1 is a different wire protocol, not a flag reinterpretation of B0. KLC1 version 2 uses a
128-byte header, 336-byte records, exact profile flags `1`, and a successful-observer ticket at
record offset `+0x20`. That ticket is local ring-reservation order only; it is not a tick ID, time,
or cross-thread causality. The payload contains only raw facts: six words for the exact 48-byte
current-stack drain snapshot, the raw caller return word at `[RBP+8]`, the zero-extended raw incoming
mode at `[RBP-0x2074]`, one subject pointer, raw binary32 bits, and a raw u32 result. The caller word
is not normalized to a guest address or asserted caller identity, and the mode carries no enum
meaning beyond separately proven fixtures. The packet emits no clock epoch, present ID, tick,
ownership generation, rate, or publication claim.

The manifest binds five guarded, copy-safe sites with no relocated branch or call:

| Event | Guest address | Raw fact |
|---|---:|---|
| locked drain snapshot | `0x32655f` | fixed reads of `[RBP-0x2068,48]`, `[RBP-0x2074,4]`, and `[RBP+8,8]` while the stock drain lock is held |
| Havok wrapper begin | `0xcace80` | incoming wrapper/world pointer and XMM0 bits |
| continuous target begin | `0xcb5330` | incoming simulation pointer |
| Havok wrapper result | `0xcaceb5` | wrapper pointer and returned EAX |
| render scalar store | `0x7eaf7c` | exact destination `RBX+0x192e8` and XMM0 bits immediately before the stock store |

Its manifest digest is
`db8a03283edfd30ac581250e4eed57d078a6ef8833901e0530bf57a09a7259e5`; source preflight matches
all five spans against the pinned eboot. The callback performs no generic guest-memory read: the
drain landing is the sole bounded current-stack sampling recipe, and all other payloads come from
captured registers.

`D1_CLOCK_TRACE_PACKET` is strict: absent, empty, or exact lowercase `b0` selects B0; exact
lowercase `b1` selects B1; unknown values and case variants fail closed. B0 remains the default and
its wire and receipt schema are unchanged. An explicit B1 request now opens the v2 trace and stages
only its five-request batch under the same all-preflight-before-first-write transaction; any
identity, custody, span, planning, or batch failure refuses the complete packet. Its
`d1-clock-b1-producer-receipt-v1` validator requires exact manifest/wire fields, trace hash and
size, all five installs, orderly atomic stop, zero drops, zero unknown sites, no budget stop, and
counter conservation. Under that zero-drop receipt, observer tickets must be exactly contiguous
from zero. A supplied rejected receipt forces `raw_observation_candidate: false`, and an invalid
explicit packet selector invalidates an older adjacent sidecar before returning. Even an accepted
producer receipt leaves `b1_observation_receipt: false`. The B0 callback path performs no B1 payload
or ticket stores. The main emulator target receives `ENABLE_D1_CLOCK_TRACE`; explicit B1 never
falls back to B0.

### Static landing manifest

Instruction-aligned displaced-span guards and relocation classes have been pinned for:

- outer entry, post-publication, and post-submit;
- F0 through F13;
- X0 through X7;
- Y0 through Y6;
- the completed fixed-time publication caller seam;
- Havok entry and XMM0 `h`;
- render extraction.

The common F/X/Y entries begin with `push rbp; mov rbp,rsp`. Copy-safe prologues, RIP-relative
loads/LEAs, relative calls/jumps, and the X1 tail jump are classified separately. Terminal
displaced `E8` calls are not safe merely because their absolute targets relocate: their durable
lowering must make an ordinary same-thread callee return observe the stock continuation, not a
trampoline PC. That narrow condition says nothing about async signal or unwind identity. X3 is
one-byte `ret` and is statically pinned as entry 3 of the eight-entry X table. It is excluded from
the runtime-probe count; synthesizing a return or detouring a one-byte function would add risk while
observing no body. Preflight validates all eight 24-byte table descriptors at raw file offset
`0x1a9b558`, including size `8` and the expected function pointer for each entry. If temporal proof
of its handle is later required, observe the table dispatcher/dependency handle, not the `ret`. The
completed-publication landing is now the F1 caller seam at `0x74a984`, immediately after
`0x590a50` returns and before the `0x3305e0` call. The publisher's stores/increment occupy
`0x590a5f..0x590a74`; `0x590a55` is only a pre-publication RIP-relative LEA.

Current B0 records serialize a drain-stamped write timestamp, host-thread identity, raw site/probe
identity, diagnostic `outer_hint` and `publication_hint`, 16 captured GPR/rflags values plus a
zeroed reserved slot, and the low 16 bytes of captured YMM0. The stub preserves full YMM state for
guest return, but the trace does not serialize full YMM state. `sample_a` and `sample_b` are zero in
B0. The hint counters are diagnostic only; asynchronous work can outlive the latest outer, so they
are not `TickContext` values or per-outer phase proof.

### Trace-format smoke tool

`tools/d1_clock_trace_gate.py` has three operations:

```text
preflight <eboot> [--output preflight.json]
inspect <trace> [--preflight preflight.json] [--receipt <trace>.receipt.json] [--output b0.json]
self-test
```

The source preflight requires SHA-256
`7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954`. `inspect` exercises the
proposed binary schema and rejects malformed headers, nonzero header flags or reserved bytes, an
unexpected GPR-plus-XMM0 sample mask, record counts beyond the header budget, truncation, unwritten
slots, nonzero reserved record/sample fields, non-contiguous sequences, unknown probes, wrong
addresses, incomplete global coverage, missing outer ordering, fixed/Havok count mismatch, or
non-stock Havok `h`.

Preflight also reads every instruction-aligned displaced span at the pinned text-segment offset,
compares exact bytes, classifies copy-safe versus relocation-required sites, and hashes the
manifest itself. This closes a static byte-guard check only. The default-off producer additionally
records the mapped pre-detour hash after loader-owned raw-source custody succeeds, but neither fact
proves a live run, relocation correctness under execution, or non-perturbation. A1's structural
acceptance and accepted atomic receipt do not alter that limit; its terminal-`E8` return-PC defect
required fresh A2 evidence after durable lowering, and A2 on carrier `07ac0350` supplied it. A2
demonstrates a live run and execution-time relocation that does not break the client; it still does
not demonstrate non-perturbation, which is a separate measurement this preflight cannot make.

`inspect --receipt` validates the offline producer sidecar. Its exit status requires a
coverage-smoke candidate and, when supplied, an accepted sidecar; it still does not promote the
artifact to Packet B0. A valid sidecar is bound to the completed trace's resolved path, byte size,
and SHA-256; checks producer counters, budgets, and installs; and is written only after an orderly
producer stop. Its mapped-image digest is a recorded custody fact rather than independent proof of
the loaded object, and absence of a receipt means only that orderly producer stop was not proven.
Even with A1's accepted atomic receipt, the result retains `b0_stock_receipt: false` and
`zero_case_receipt: false`; its `outer_hint` ordering is diagnostic coverage, not per-outer phase
pairing or non-perturbation evidence.

## What the Havok 2012.2 SDK establishes

The era-matched SDK is useful as a semantic reference, not as proof of Destiny 1's wrapper or job
ownership.

It establishes that:

- `hkStepInfo` carries start time, end time, delta time, and inverse delta time as one coherent
  step contract;
- `hkpWorld::stepDeltaTime` advances one single-threaded step from one physics delta;
- `hkpWorld::stepMultithreaded` owns a queue, thread pool, and one physics delta;
- finer multithread control separates initialization, worker processing, and finish/cleanup;
- half-step and asynchronous interfaces split integrate, collide, time advance, and frame markers;
- callbacks, characters, vehicles, actions, constraints, and continuous simulation consume the
  same step information and therefore cannot be repaired by changing only one wrapper argument.

That supports the fixed-`h`, whole-lifecycle design. It does not tell us which interface Destiny 1
uses at `0xcace80`, whether the wrapper has visual sidecars, when its children retire, or how the F
graph owns the world. Packet B supplies those facts.

## Remaining evidence packets

### B1: mixed synchronous boundaries

The raw live packet above is implemented. B1 ownership still must close:

- F1 block allocation, exactly one publication per real tick, and prepared-but-not-published ZP;
- F10 deferred D0-D5 ordering, close, playback, release, and block reuse;
- zero-tick Z0 and ZP behavior;
- the exact publication/retirement seam;
- raw tick-counter locations used by F5, F10, gear, and other cadence gates.

Static closure has additionally identified the bounded F10 drain/retirement sites: locked snapshot
`0x32655f`, exact per-node retirement `0x2ff2ac`, release-call entry `0xbb3890`, common release
epilogue at `0xbb38c3`, publication close returned at `0x74b929`, and `0xb9d380` returned before
`0xb9d460` at caller landing `0x74b9e1`. Probe 269 samples the low byte at `block+4` before the
bit-0 test; probe 270 samples the same byte at the common epilogue. A paired `1 -> 0` bit proves
the release path returned through both queue-release calls and executed the stock clear, while a
`0 -> 0` pair records the already-clear path without claiming a release.
Every D0-D5 mode-0 drain re-snapshots until quiescent; gear work queued after D5 is intentionally
next-tick work. At the drain landing the observer samples only the locked fixed recipe: the 48-byte
snapshot, raw incoming mode, and raw caller return word. Elsewhere it captures only the declared
register-sized identities. It must not follow the snapshot's opaque base, infer reuse completion
from the earlier publication clear, or interpret `0x74b9e1` as later transport completion.

### B2: asynchronous ownership and effects

B2 must close:

- F2 descriptor, returned handles, indirect worker, and global overwrite lifetime;
- F3/F6/F8 child generations and joins;
- input poll/receive/decode ownership and edge assignment;
- network receive assignment and once-only send ownership;
- audio, UI, particles, cloth, persistence, and transport effect identities;
- immutable snapshot field schema beyond the known parity bit;
- all relevant fixed-rate getter callers.

The F2 child lifecycle is now statically resolved even though live occurrence evidence remains.
`network_send` is type `0x27e7e10` with work target `0xae64f0`; `network_receive` is type
`0x27e7e20` with work target `0xae6390`. The generic executor exposes raw handle
`{generation_u32,slot_u16,zero_u16}` at callback context `+0x48`; the top 16 bits must be zero and
the slot is `(raw_handle >> 32) & 0xffff`. The observer records the raw callback/type words at
slot `+0x60/+0x68` and callback return at `0xfaa4a`; it does not promote those raw words to durable
semantic identities. Retirement entry `0xfec11` uses the current generation, normal landing
`0xfecc0` observes the old generation after the increment, and state 7 selects requeue at
`0xfec63`. Free-list and requeue publication linearize at their preceding successful link CAS;
`0xfef18` and `0xfebb4` are the corresponding post-CAS success landings. The rejected wire-v3
recipes reread generation/list state there after another worker could already pop and reuse the
published slot. Wire v4 never reconstructs a handle at either landing. Normal publication captures
the current retirement RBP, pool, private saved slot, and successful CAS target, then inherits the
old handle from same-thread `0xfecc0`. Requeue publication captures the caller retirement RBP,
return PC, arithmetically derived pool, CAS target, and link-record index, then inherits the handle
from same-thread `0xfec63`. The requeue caller RBP is the predecessor's current retirement RBP by
the pinned call ABI; the requeue callee's current RBP is deliberately not used as that identity.
F10 can become runnable before an old slot is published free, so F10 is not a reuse fence.
Correlation never uses latest-handle globals, observer tickets, timestamps, or a
post-publication generation read as identity.

#### Packet B2 wire-v4 checkpoint

The source checkpoint implements one all-or-nothing 39-probe stock observer, not a scheduler
change. It keeps KLC1's 128-byte header and 336-byte record, uses wire version 4 and profile flags
`3`, preserves B1 v2 byte-for-byte, and uses a separate
`d1-clock-b2-producer-receipt-v2` sidecar. Its global observer-ticket stream records successful
reservation order only; a ticket is not time, a tick ID, or cross-thread causality. The canonical
manifest is `8d47a31f5fe3fa096937919da0d2911c177adc7b3d57b4e96a1bf9179623e2d5`.
Every real inspection requires the compiled C++ manifest export as well as the receipt, so a digest
literal cannot substitute for the compiled probe table. The decoder validates every observed F2
producer and worker attempt and rejects leaks, duplicates, mixed normal/state-7 outcomes, wrong
stable joins, a requeue publication whose captured helper return is not the exact stock
continuation `0xfec6e`, terminal-without-submit, and receive dependencies that do not name one
complete normal send occurrence. Exact stock disassembly proves both F2 creators reach the
allocator through `0xfb3e0`, which loads the one pool from global `0x25b85b0`; generation plus slot
is therefore a unique live-lifecycle key in this pinned image, not an outer-tick identity or a
cross-thread happens-before edge. A complete state-7 lifecycle remains quarantined and cannot
later satisfy normal or dependency evidence without a separately proved retry identity. Fixed header
identities require canonical zero padding. Source-level adversarial controls and the authoritative
R8 offline matrix pass. Evidence is
`build-clockint-gates/b2-gates-r8.lVeZoK`; its complete log SHA-256 is
`2c64b9c7d15e1dc6658afc968b7cd8f83198ce619366f2714671c401565c05a6`, and its carrier packet
SHA-256 is `e7a77617aaaba937a01c1ac028904a79a5cea2979f78b35b38ea399890cc344d`.
The matrix matched the exact 39-span preflight, built and audited the production callback, ran the
five selected tests, rejected a same-Clang configure with Python disabled for the exact required
package reason, proved feature-OFF symbol absence, and restored the normal trace-enabled cache.
The independent stock and final-ELF reviews pass. The exact R8 base carrier is sealed read-only at
`build-d1clock/freeze/a0d85c08-b2-r8`. Its 134-entry payload includes the clean HEAD archive,
all 18 dirty build-source files, R8 evidence, trace-enabled and feature-OFF ELFs, exact CMake/Ninja
and compiler metadata, compiled manifest and decoder, stock client, rate-clean patch, last-good
activity snapshot and sidecar, harness manifest/roster/scripts, starting emulator configuration,
and the three last-good server binaries. `FREEZE-MANIFEST.tsv` is SHA-256
`5dd2e447d4fe7f3731a82fb8e48ce9a92041f546be1df66944d0d61587bdba0c`;
`SHA256SUMS` is SHA-256
`44597cb305f5a976f40efaa5136db687d8fd1318d7bc619ad3d7f5c0e916854b`;
the external receipt digest is
`36314220f037120dfa8b697113a7f8b9c9aaf02547594e998b8321c0db40c02c`.
A first-hand post-publication checksum replay passed, the directory is non-writable, its atomic
stage and transfer inputs are gone, and the shared slot was released. This is an offline/build
carrier, not a complete live-input carrier. Its archived launch template supplied a 17-file
non-complete roster while `D1_COMPLETE_TITAN=1` requires the complete `abil-desc-stat` estate.
`CLOCK-B2-STOCK-20260826-A1` therefore stopped during prelaunch; no emulator, server, guest
instruction, trace, receipt, or live candidate resulted. The preserved abort record is
`runs/clock-captures/CLOCK-B2-STOCK-20260826-A1/PRELAUNCH-ABORT.tsv`, SHA-256
`69f4f4828e217f0aed94e35c1ca48e121486081df036785071285dcdc364883e`.

The first supplemental carrier, `build-d1clock/freeze/a0d85c08-b2-r8-live-inputs-r1`, correctly
bound the live inputs but wrote literal backslash-`t` bytes in its TSV receipts. The launcher's
real-tab parser rejected that receipt during `CLOCK-B2-STOCK-20260826-A2`, before capture-directory
creation or any emulator/server process. R1 remains immutable negative evidence.

The corrected r2 live inputs are sealed at
`build-d1clock/freeze/a0d85c08-b2-r8-live-inputs-r2`. Its 64 payload entries bind the exact complete
profile and character, 17 inventory elements, 21 roster elements, the state database and WAL, ten
runtime tables, the selected 8,516-file emulator user root, normalized last-good provenance, and
the launch/freezer sources. `LIVE-INPUT-MANIFEST.tsv` is SHA-256
`4d47c13690d718368186d82c33cb8c81cf5bc41ad3769348099fb7f52a6c22bc`; `SHA256SUMS` is SHA-256
`951aea184e687087c2c9bb40ad8a348f4f6ab63e1098873830a98c3eaf455b8b`; and
`FINAL-RECEIPT.tsv` is SHA-256
`9a1f2bf210cb432b51bb8f322e2f09f6dcdd90ee6c21a06d084f753a0d6b0545`. Independent checksum,
real-tab, parser, mode, process, stage, and shared-slot replay passed. No emulator or server was
launched while sealing either supplement. R2 is valid live-input custody, but its launcher is not
launch-ready: it relocates the emulator user root and passes a standalone frozen client without
the `sce_sys` and `sce_module` siblings required for identity and module loading. A3 therefore
armed zero probes and remains rejected negative evidence.

#### A6 live status, R4/R8 custody, and A7 routing

`CLOCK-B2-STOCK-20260827-A6` is a sealed failed run, not an unlaunched claim. Vulkan presentation
remained healthy at about 55.3 FPS and the server login succeeded. The emulator logged ordered
`bootflow:media_check` then `bootflow:bootload`, but never left bootload and opened none of
`ps4_client_bootstrap_unp3_0.pkg`, `ps4_ui_startup_unp3_0.pkg`, or
`ps4_ui_bootflow_unp3_0.pkg`. The black native Wayland window is therefore not a crash diagnosis.
Controller count was zero and is a recorded confound, but the package-open stage precedes operator
input, so controller absence is not the first observed blocker.

The exact A6 trace SHA-256 is
`ce6d202a6cfffefebd9ee25de934979e8bd728523a29ffae230da1df5d07db50`; its producer receipt,
inspection, final receipt, and result are respectively
`d5861efe1123e20411fd7a84a2617ec40c480faf826ae07612ee221048dacd9b`,
`a10dceeb302be21cb17d169f2a42e65636528c13c4fdb8fd30ad958a87178511`,
`4334bf813f86a4451bf331c5fb1e8d8502477cdb506618157e461b89397e7124`, and
`e1f905a64afffcef5ed8dbec51d753f0a38204b0d7ef4ca24f8a8b651f9c171c`; the outcome is 4. The
producer installed all 39 probes, emitted 100 contiguous records,
reported zero drops and rejections, and stopped orderly. Nineteen required paths were absent, so
this is not an accepted B2 observation candidate. The final probe-269 record is causal detour
evidence, not a conditional-coverage ambiguity. The old probe 270 selected
`0xbb38bf..0xbb38c7`; the already-clear `je 0xbb38c3` therefore entered byte 4 of the installed
`E9 disp32` patch instead of the stock epilogue. The full 39-row arm and a skip-269 arm both
stalled with zero probe-270 records. Skipping probe 270 alone reached `bootflow:start` with
169,024 contiguous records, and skipping 269+270 reached it in two runs. This retracts the earlier
noncausal interpretation and requires the common-epilogue replacement before another candidate.

The inspector has two confirmed contract defects, neither corruption proof: mapped PCs require
`-0x400000` normalization, and probe 289 at `0xfebb4` is a common helper reached from multiple
callers. Its ambiguous-producer and direct-PC errors are therefore false diagnoses. Preserve A6
and repair the inspector contract separately; do not reinterpret the valid bounded producer record
as a candidate.

R4 is the sealed exceptional current-state custody reconciliation of failed A6, never a retroactive
PASS. Its manifest, checksum list, and receipt SHA-256 values are respectively
`46310ba5bb0bd82d848674ff6ea496d1fe36346ad515d4266bac79fd5ed72dc6`,
`1a3d02bfc108b07524a5f780f26d09f3f34a9242356de544e048b442f50aad8e`, and
`a1095a5b0108abb7821bac6cc9fbbbe9c6986f383c0ada000379cfb4c1ad9c90`; it has 76 payloads.
The successor trace-disabled context is sealed at
`build-d1clock/freeze/a0d85c08-b2-r8-live-context-r8-trace-disabled`. Its manifest, checksum list,
and receipt SHA-256 values are respectively
`04ee4081ac28c50461d5c8c227b97dd5fa6fb358bd60abcbba133c4c697c433c`,
`c17e917cad58d0d1e915bdd68d13fa4272139b0700891085c360398393d7292f`, and
`25145f44151ad54b3917e6086be91067849eef82c8d24f6b7652c917e7950251`; it has 26 payloads.
It binds the same trace-capable ELF SHA-256
`177469d0979cbee270d42193bb6052f2742f0c0e47395ec2c37143daf80d2b5c`, Build ID
`819808cfbdbc422287b31d8140c577cee709fbf1`, with clock-trace variables absent.

The real no-pad launcher preflight has passed the complete custody chain and all 11 controls, then
correctly refused because its census did not contain exactly one physical controller. It cleaned its
stage and processes and did not launch. The next gate is recorded as
`CLOCK-B2-STOCK-20260827-A7` and uses the three R8 context hashes:

From the `shadPS4-d1clock` repository root:

```text
bash build-d1clock/live/frozen-stock-b2-live-r8.sh \
  CLOCK-B2-STOCK-20260827-A7 \
  c17e917cad58d0d1e915bdd68d13fa4272139b0700891085c360398393d7292f \
  04ee4081ac28c50461d5c8c227b97dd5fa6fb358bd60abcbba133c4c697c433c \
  25145f44151ad54b3917e6086be91067849eef82c8d24f6b7652c917e7950251
```

It requires exactly one stable controller, a 90-second owned-PGID deadline, actual package-open
matching, and controller custody; trace output, trace receipt, and inspection are intentionally
absent in this arm.

| A7 trace-disabled outcome | Smallest next action |
|---|---|
| progresses | instrumentation is implicated; repeat enabled and localize probes 269/270 and overbroad 289 |
| stalls in the same way | pursue the emulator/runtime axis; X11 is a second axis only |
| controller or custody check fails, or a trace artifact appears | invalidate the arm; correct the precondition without interpreting bootflow |
| guest or emulator exits unexpectedly | retain the failed evidence and diagnose that exit before claiming either routing branch |

Even the first row does not prove tick identity, cross-thread causality, simulation rate,
non-perturbation, C1 equivalence, interpolation safety, or live rate authority. It authorizes the
next observation/refactor rung, not arbitrary-rate execution.

#### Next same-run lifecycle packet

The next observer is not a decoder-side join with a prior B2 trace. Raw generation/slot values and
scheduling are run-local, so a separately selected wire version must repeat the generic worker
begin, callback return, retirement, generation invalidation, state-7, requeue, and free-list
success rows in the same trace as its new F3/F6/F8 identities. It needs its own profile flags,
manifest hash, receipt, strict selector, and all-or-nothing install gate.

The exact minimum is **29 probes**: 22 target-specific rows plus seven same-run generic lifecycle
rows. B2's worker-begin rows at `0xae1894` and `0xae6394` are send/receive-specific and cannot
observe arbitrary F3/F6/F8 callbacks. The successor therefore starts the generic lifecycle at
executor entry `0xfa980` (`RDX=raw_handle`) and reuses the independently guarded callback-return,
retirement, normal-invalidation, state-7-selection, requeue-publication, and free-list-publication
landings at `0xfaa4a`, `0xfec11`, `0xfecc0`, `0xfec63`, `0xfebb4`, and `0xfef18`.

The 22 target-specific rows are bounded exactly:

- one outer cadence row at terminal direct call `0x59395a` (`e8c1eeffff`); do not add the next
  stock call at `0x59395f`;
- seven F3 rows: entry/exit `0x74ac50`/`0x74acf6` and static submit sites `0x1fd025`,
  `0x1fd02d`, `0x1fd035`, `0x1f67b0`, and `0x1f67b8`;
- seven F6 rows: entry/exit `0x74af30`/`0x74b45e`, producer stores `0x488177`, `0x22a91d`,
  and `0x4cc666`, the 12-record enabled-path array at `0x74b34c` (`bf09000000`) from original
  `RSP+0x80..+0xd8`, and aggregate submit `0x492b34`;
- seven F8 rows: entry/exit `0x74b5a0`/`0x74b63f`, the tail-jump submissions at `0x488395`,
  `0x22a9e6`, and `0x4cc6a7`, and release pre/post `0x493d43`/`0x493d53`.

The five F3 addresses are static sites, not five dynamic handles. The first three execute inside a
fixed three-iteration loop and produce nine dynamic submissions when reached; the final two are
conditional. Acceptance must quantify and retire every observed occurrence and must not require
exactly five handles.

Entry rows bind `RDI` plus bounded dispatcher fields and require target identity and return PC
`0x23c7ca`. Submit rows carry `RDI=raw_handle`; store rows carry `RAX=raw_handle`; the F6 array is
a guaranteed-inline 96-byte copy of 12 `u64` records. Release pre captures `RBX=owner` and
`RSI=resource`; post inherits the matched resource rather than rereading mutable owner state.
Balanced release rows prove only that the unsupported indirect call returned. They do not identify
its target or prove allocator reuse. Outer rows are cadence markers, not tick authority or proof
that an asynchronous group belongs to one outer call.

`0x74b414` is fallback-only and is not a valid 12-record capture point: the enabled F6 path returns
from `0x334f50` at `0x74b34c` and jumps from `0x74b3b7` to `0x74b419`, bypassing `0x74b414`.
Likewise, `0x352413`, `0xbf1b1d`, and `0x493d50` are indirect calls that the current relocator
cannot lower; they must not enter a manifest or be called input writers/release-target evidence.
F5/F9/F10 begin-to-convergence envelopes are useful normal-path accounting only. No envelope or
top-level table completion is a complete-tick or nested-retirement receipt.

The 39 rows are composed as follows:

- five B1 rows inherited unchanged at `0x32655f`, `0xcace80`, `0xcb5330`, `0xcaceb5`, and
  `0x7eaf7c`;
- thirteen C0 rows at `0x74a780`, `0x74aa09`, `0xb9d1e0`, `0x590a5c`, `0xb9d270`, `0x74b770`,
  `0x74b929`, `0xb9d310`, `0xbb3890`, `0xbb38c3`, `0x74b9e1`, `0x74a984`, and `0x2ff2ac`;
- six deferred-return rows at `0x74ab0e`, `0x74ad6d`, `0x74adc2`, `0x74b7f1`, `0x74bbb4`, and
  `0x74bcbc`;
- fifteen F2 rows at `0xae6af2`, `0xae6b18`, `0xae6c22`, `0xae6c33`, `0xae1894`, `0xae6394`,
  `0xfec11`, `0xfecc0`, `0xfec63`, `0xfebb4`, `0xfef18`, `0xfaa4a`, `0xae6afb`, `0xae6b10`, and
  `0xae6c2b`.

The non-B1 displaced spans are pinned to these exact starting bytes:

```text
74a780 554889e54157    74aa09 f605443af60102  b9d1e0 554889e54157
590a5c 488b08c5fa114110  b9d270 554889e54156    74b770 554889e54157
74b929 8a056148f601      b9d310 554889e54156    bb3890 554889e553
bb38c3 4883c4085b        74b9e1 e87a1a4500      74a984 e8575cbeff
2ff2ac b8e8ffffff

74ab0e 498b07483b45d0    74ad6d 488d5dd04889df    74adc2 e819c6bdff -> 3273e0
74b7f1 4963442470        74bbb4 498b06483b45e0    74bcbc bf01000000

ae6af2 488903488b31      ae6b18 e8a34a61ff        ae6c22 488903488b31
ae6c33 e8884961ff        ae1894 e8d70d6fff        ae6394 4157415653
0fec11 4189f64889fb      0fecc0 0faef031d2        0fec63 c601024889df
0febb4 668b45da66448975cc 0fef18 668b45c266448945b4 0faa4a 49894548bf005c0000
ae6afb e8e04b61ff        ae6b10 e85b4b61ff        ae6c2b e8b04a61ff
```

These rows deliberately use the reviewed boundary meanings. `0x590a5c` is before the load and
first publication store; `0x74a984` is after fixed publication returned and before `0x3305e0`.
`0xbb38c3` is the common epilogue: on a probe-269 entry sample with bit 0 set, it is after both
queue-release calls and the bit-0 clear; with bit 0 already clear, the short branch lands directly
at the same site. The decoder pairs entry/epilogue records by observed thread and block and rejects
any epilogue sample retaining bit 0. `0x74b9e1` proves only that `0xb9d380` returned before
`0xb9d460`. The `0x74adc2` deferred landing is after the F4-set call
returned and before the displaced terminal cleanup call to `0x3273e0` (continuation `0x74adc7`).
The create sites `0xae6af2` and `0xae6c22` are before the
latest-handle stores, and `0xae6b18`/`0xae6c33` are before submit calls. The worker observations are
post-prologue so their frame recipes are coherent. `0xfaa4a` is callback-returned before result
store; `0xfec11` records current-generation retirement entry, `0xfecc0` reconstructs the old
generation after its increment, and `0xfec63` is state-7 requeue selected before state 2.
`0xfebb4`/`0xfef18` are success landings after their respective link CAS, not the linearization
instructions themselves and not broader head-publication claims. Their wire-v4 records contain
stable join fields only and never reread published allocator generation/list state. The
three dependency calls record child and dependency handles from RDI/RSI. The six deferred rows prove
only that the synchronous transition returned; no deferred bit/count state is claimed until a
bounded memory recipe is separately pinned.

The v4 record layout remains narrow: sequence at `+0`, drain timestamp at `+8`, raw Linux thread
pointer at `+16`, zero reserved word at `+24`, observer ticket at `+32`, guest address at `+40`,
probe/event identifiers and valid mask at `+48..+55`, seventeen 64-bit register slots at
`+56..+191` with slot 16 zero, raw XMM0 at `+192..+207`, and sixteen 64-bit event payload slots at
`+208..+335`. The raw thread pointer is an opaque same-live-session equality token only: it is not
an OS TID, guest identity, timestamp, or cross-session identity. The callback must obtain it without
a function call, syscall, or `pthread_self`.

F2's pre-publication common payload is raw handle, caller return PC, zero-extended slot, and raw
caller frame pointer. Event-local slots add the previous latest handle at the pre-store create
sites and the callback/type words at `0xfaa4a`. The two post-CAS publication rows intentionally
omit raw handle and use only the stable join fields specified above. Arithmetic or noncanonical recipe failures reject a sample before
reserving a ticket or consuming budget. These fixed reads are not a general safe mapping check:
the exact pinned landing and lifetime invariants provide their mapping validity, and a corrupted
but canonical pointer can still fault in this opt-in callback. The receipt gate requires exact
version, flags, record size, probe count, manifest and source custody; every install; SHSTK inactive
for terminal-call lowering; an orderly hash-bound stop; contiguous tickets equal to sequence;
valid masks and zero unused fields; and zero drops, unknowns, rejected samples, budget stop, or I/O
error. The compiled manifest export must exactly match the decoder's canonical rows. Receipt
integrity is separate from scenario sufficiency. A B2 observation candidate requires execution of
every non-state-7 row and universal assignment of every F2 record to one raw-handle occurrence:
each send producer segment is exactly `280 -> 292 -> 293 -> 281`; each receive producer segment is
exactly `282 -> 294 -> 283`; and every observed worker attempt is exactly begin -> callback return
-> retirement -> normal invalidation/free publication XOR state-7 selection/requeue publication on
one observed thread token. At least one send and receive must close normally, and every receive
dependency must name one complete normal send occurrence. State-7 rows 288/289 are optional; if
either appears, its attempt must be complete and its handle remains quarantined from normal or
dependency evidence because this wire has no proof-grade attempt/thread-incarnation identity.

Even an accepted B2 receipt is observation-only. It can establish exact executed landings and
program order among records carrying the same observed raw thread-pointer token in the exercised
stock scenario; the token alone does not prove thread lifetime or exclude TLS-address reuse. It
cannot establish simulation
rate, authoritative tick assignment, cross-thread causality, source/application tick identity,
non-perturbation, C1 equivalence, interpolation safety, or permission to change rate.

Server-authored state remains server-authored. Raising local simulation authority must not invent
network messages, acknowledgements, inventory changes, matchmaking state, or other external facts.
Inbound immutable data is assigned to one tick; outbound effects use stable IDs and commit once.

The stock timing publication is not an immutable snapshot channel. `0x590bf0` writes an inactive
`0x38`-byte slot, fences, and flips parity, but does not initialize every byte; F0 later mutates the
published slot at `+0x20` and `+0x2a`, and `0x749cb0` returns a raw pointer without a pin. X5 calls
`0x7eaf40`, whose only pinned payload lower bound is frame-record `+0x0c` copied to render-object
`+0x192e8`. A replacement must publish separately owned immutable generations and retain them
until every synchronous and asynchronous reader retires; parity is diagnostic only and is
ABA-prone after two publications.

## Refactor and authority gates

The proposed implementation order is intentionally asymmetric; none of these scheduler stages is
implemented live:

The exact outer control seam is now pinned: caller `0x593310` executes the five-byte
`call 0x592820` at `0x59395a`, passes the outer time input in `RDI=R14`, and resumes at
`0x59395f`. Internal landing `0x592934` is inside the builder's active frame and remains
observation-only. The generic table dispatcher at `0x23c7a0` passes a stable group binding through
`RDI` to each same-index wrapper, but its final-target counter proves only top-level table
completion. It does not prove nested allocator-slot retirement. The client therefore exposes a
usable future C0/C1 control seam and context key, not a callable complete-tick function.

1. **C0 shadow:** leave the direct stock `call 0x592820` at `0x59395a` and every shipped job at its
   original position. Observe the stock graph and evaluate scheduler decisions in shadow only;
   no wrapper controls execution. Extend the same-run lifecycle packet for F3/F6/F8
   submit/start/retire/reuse evidence while retaining the generic worker rows needed to correlate
   those handles.
2. **C1 exact stock:** add a default-off, group-keyed tick context and ABI-transparent same-index
   target preambles, then refactor one evidence-closed boundary at a time while remaining exactly
   stock-equivalent. An ordinary wrapper that calls the original target is forbidden until proved:
   its added frame changes the target-visible return PC from dispatcher continuation `0x23c7ca`.
   The unchanged-target form must preserve the callsite ABI and tail-jump after its preamble, or a
   per-call manifest must prove return-address, stack/unwind, live GPR/XMM, DF/MXCSR, and exception
   insensitivity. The existing observation detour is not a guest-call gateway. The first C1 route
   retains `0x592820`, the shipped 14-entry F group, and every dependency; it neither repeats the
   builder nor directly calls F targets. Its context is keyed by the generic dispatcher's `RDI`
   group binding, records wrapper begin/end and required child retirement, and fails closed as not
   committable until every required child retires. The preferred boundary order is F10, F6, F9,
   F5, F3/F8, F2, F1, then serial F4/F7. C1 executes only the observed stock `N=0/1` pattern with
   stock `33366` microseconds, integer rate 30, and interpolation disabled; it is not the multi-tick
   clock loop.
3. **D0 logical-tick equivalence:** from one authoritative checkpoint and one frozen
   input/network/RNG vector, compare C1 `[1,1,...]`, D `[N]`, and D `[0,N]` for N=2/3. At each
   logical tick require the same authoritative hash, next-tick queue digest, event-intent IDs and
   order, one completed fixed-`h` return, closed terminal ledger, released block, and immutable
   snapshot payload digest. Compare physical delivery separately by stable event ID and exclude
   presentation state from the authoritative hash.
4. **D1 rational scheduling:** only after C1, bind the exact clock to the validated live context
   owner, allow zero/one/many ticks behind a reversible flag, and add separately owned immutable
   snapshot payloads with publish-after-commit, reader pins, and retirement before interpolation
   consumes fractional credit.
5. **D2 presentation matrix:** fixed simulation under 30/45/60/72/90/120/144, 59.94, uncapped,
   and VRR presentation policies.
6. **D3 60 Hz authority:** validate raw counters, network cadence, replay, capacity, and every
   once-only effect at a 60/1 simulation rate.
7. **D4 higher authority:** validate 72/90/120/144 simulation authority separately where needed.

No higher step inherits authority merely because a lower step looked smooth.

The unresolved cross-domain gate is the commit cut. No current client landing simultaneously
proves the whole guest transition and Havok return, D0-D5/block/embedded-queue release, every
F2/F3/F6/F8 child beyond its true reuse boundary, all one-shot outputs still staged as tick-owned
intents, and a complete immutable render payload leased through asynchronous X/Y readers.
`0x590a50` advances too early; F10 can run before allocator free publication; `0xb9d310` clears only
block publication; `0xb9d380` closes only that block; and top-level group completion is not nested
child retirement. `TryCommit` must remain authority-lane-only and fail closed until a terminal
obligation ledger reaches zero, then expose event intents, release-publish the snapshot, and only
then commit the exact clock tick. Until that cut exists, T+1, external delivery, and presentation
cannot safely observe T.

## Per-tick ownership contract

Every repeated asynchronous job requires a persistent `TickContext*` in its descriptor. A global
"current tick" is invalid because jobs from tick `k` can remain live after the scheduler begins
tick `k+1`.

A future `TickContext` needs at least:

```text
clock_epoch
tick_id
fixed_h_bits
input_generation
network_generation
simulation_block
child_handles and retirement state
previous/current render generation
per-subsystem event ordinal counters
commit/abort state
```

Required fences include:

- terminal(`k`) before ingress(`k+1`) for state that is not generation-owned;
- child retirement before handle, block, or descriptor reuse;
- F1 allocation/publication paired with F10 close/release;
- one immutable render publication only after the authoritative tick commits;
- single-authority deterministic assignment of `{clock_epoch, tick, subsystem, ordinal}` in
  canonical tick-then-subsystem-then-ordinal order before asynchronous delivery, followed by one
  delivery-ledger commit per ID.

The minimum tick-terminal retirement ledger includes the Havok return, all D0-D5 drain returns,
block and embedded-queue release through `0x74b9e1`, normal F2 send/receive callback return plus
generation invalidation and free-list publication, complete F3 delayed-DAG retirement, F6's
12-entry child-group retirement, and F8 generation release. State-7 F2 requeue is quarantined until
its reuse semantics are observed. Top-level F completion or F10 dependency release alone is not a
reuse fence.

Never loop the whole F, X, or Y table. Their dependencies interleave simulation and rendering.

## Offline controls

The standalone controls compile without Vulkan, a client image, or emulator startup. They verify:

- rate reduction and invalid-rate refusal;
- pinned `vmulss`/`vcvttss2si` raw-to-realized stock fields versus the named T1 three-write control,
  including malformed and internally inconsistent mapping rejection;
- rejection of elevated profiles without an explicit guarded tick-context `h` route;
- deterministic correctly-rounded binary32 `h` for 30/60/72/90/120/144 and `60000/1001`;
- exact `60000/1001` cumulative pacing;
- fixed simulation independence from 30/45/60/72/90/120/144 presentation;
- fixed simulation independence from irregular VRR-like opportunities;
- request-pacer ceiling deadlines, no-burst behavior, idle phase reset, and generation changes;
- transactional abort without credit loss;
- infallible publication of a preflighted tick reservation;
- tick budget, retained debt, explicit debt drop, and structurally verified exact receipts;
- pause, negative wall delta, hitch clamp, and epoch policy;
- epoch reseed and monotonic same-epoch snapshot generations;
- deterministic authority-owned event ordinals plus bounded synchronized delivery deduplication;
- Packet B/C/D/elevated-rate claim completeness without runtime authorization;
- deterministic mixed-rate stress over 14,000 frame decisions.

These are model-level controls. They do not substitute for the stock trace or exact-client replay.

### Static verification handoff

The branch is safe to review without launching an emulator. The implementation owner should run
the isolated controls from the D1 clock worktree:

```text
cmake -S . -B /tmp/d1clock-b0-tests -G Ninja \
  -DCMAKE_BUILD_TYPE=RelWithDebInfo -DENABLE_TESTS=ON \
  -DENABLE_D1_CLOCK_TRACE=OFF
ninja -C /tmp/d1clock-b0-tests -j4 \
  shadps4_d1_clock_controls shadps4_d1_clock_trace_format_controls \
  shadps4_d1_clock_trace_b1_format_controls \
  shadps4_d1_clock_trace_producer_controls shadps4_d1_detour_stub_controls \
  shadps4_d1_detour_reloc_controls shadps4_d1_detour_plan_controls \
  shadps4_d1_sha256_controls
ctest --test-dir /tmp/d1clock-b0-tests --output-on-failure \
  -R '^shadps4_d1_'
/tmp/d1clock-b0-tests/tests/shadps4_d1_sha256_controls \
  <pinned-preservation-repo>/dumps/patch-01.29-CUSA00219/eboot.bin
python3 tools/d1_clock_trace_gate.py preflight \
  <pinned-preservation-repo>/dumps/patch-01.29-CUSA00219/eboot.bin \
  --output /tmp/d1clock-b0-preflight.json
```

The expected suite contains ten passing CTests: model, B0 and B1 format self-tests, full-YMM stub,
relocation, detour plan, B0 and B1 format round-trips, SHA-256, and producer-session/receipt gates. The
producer gate exercises accepted drop-run and threaded artifacts plus rejection of an altered
receipt hash and missing fixed-publication coverage. The owned-image SHA invocation adds the three
client-custody arms that the pathless CTest intentionally skips.

Fresh gate evidence completed in `build-clockint-gates`: all 10/10
`shadps4_d1_` CTests passed. Owned-image SHA controls passed all 13 checks for eboot digest
`7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954`. B1 self-test recomputes
and checks the manifest digest above; its producer route additionally exercises accepted receipt,
drop rejection, rejected-candidate suppression, exact ticket-contiguity rejection, stale-sidecar
invalidation, and missing-coverage rejection. Sanitized producer controls pass 92 checks. The actual
trace-enabled emulator links with the B1 batch. These results implement the observer but do not
implement the live scheduler or promote A1, A2, or B1.

### Completed B2 offline verification and reproducible handoff

Run this sequence only after the shared build/run owner posts an explicit release and a fresh
process/lock audit is clean. It does not launch shadPS4 and does not run Cargo:

```text
cmake -S . -B build-clockint-on -G Ninja \
  -DCMAKE_BUILD_TYPE=RelWithDebInfo -DENABLE_TESTS=OFF \
  -DENABLE_D1_CLOCK_TRACE=ON
ninja -C build-clockint-on -j4 \
  shadps4_d1_guest_lifecycle_b2_callback_disassembly

cmake -S . -B build-clockint-gates -G Ninja \
  -DCMAKE_BUILD_TYPE=RelWithDebInfo -DENABLE_TESTS=ON \
  -DENABLE_D1_CLOCK_TRACE=ON
ninja -C build-clockint-gates -j4 \
  shadps4_d1_guest_lifecycle_b2_format_controls \
  shadps4_d1_guest_lifecycle_b2_producer_controls \
  shadps4_d1_detour_stub_controls
ctest --test-dir build-clockint-gates --output-on-failure --no-tests=error \
  -R '^(shadps4_d1_guest_lifecycle_b2_(gate|roundtrip|producer|callback_gate_controls)|shadps4_d1_detour_stub_controls)$'

cmake -S . -B build-clockint-off -G Ninja \
  -DCMAKE_BUILD_TYPE=RelWithDebInfo -DENABLE_TESTS=OFF \
  -DENABLE_D1_CLOCK_TRACE=OFF
ninja -C build-clockint-off -j4 shadps4
nm -C build-clockint-off/shadps4 > build-clockint-off/d1-b2-off.symbols
! grep -F 'Core::D1GuestLifecycleB2' build-clockint-off/d1-b2-off.symbols
```

The project deliberately makes the production emulator and standalone tests mutually exclusive,
so no configuration may claim both targets. The first lane-released B2 gate exposed an invalid
test-side reference to `shadps4`; it failed configuration before either B2 control executable was
built. The production callback gate now belongs to the trace-enabled tests-off graph and depends
on the exact emulator ELF it inspects. Trace-enabled tests require the Python interpreter at
configure time. The tests-on graph separately owns the Python self-test,
production pack/strict-decode round-trip, real C++ producer/receipt gate, and ten-case adversarial
callback-parser control. The callback gate must inspect every recognized clone, require the Linux
`%fs:0` thread token, and reject calls, syscalls, indirect jumps, external branches, or a
stack-canary substitution. The feature-OFF
tests-off main target must still compile and its symbol inventory must contain no B2 producer or
installer. The selected verdict is exactly four passing B2 tests plus the executable detour-stub
control; zero selected tests is a refusal.

The first corrected-topology execution passed the original three tests-on B2 gates and the feature-OFF main
link plus negative B2-symbol audit. Its production callback gate then found one real external edge:
Clang had lowered the 640-byte captured-register copy to `memcpy@plt`. The same linked symbol also
contained a compiler-generated indirect jump table for the capture-recipe switch. The production
B2 translation unit now requires Clang, uses its constant-size inline-copy intrinsic for that one
snapshot, and is compiled with jump tables disabled. These are source-local lowering constraints;
the linked-ELF invariant still rejects every call, syscall, indirect jump, external branch, missing
`%fs:0` thread token, or unrecognized clone. Its AT&T operand parser ignores only the optional
demangled-symbol annotation when classifying a leading `*`; ten controls prove that pointer stars
in direct-branch annotations pass while register, RIP-relative, and absolute indirect branches,
external or unparsed direct branches, calls, and unknown clone shapes still reject.

Independent disassembly then found an ABI edge at the detour boundary: the forced inline copy
lowered to `rep movsq`, but guest DF=1 would violate the SysV callback-entry contract. The stub now
executes `pushfq; cld`, calls C++, and restores the original guest flags with `popfq`. Its exact
shape is 77 instructions. The executable inverted arm enters with guest DF=1 and proves that
captured `rflags` retains DF, the callback observes DF clear, and the stub restores guest DF before
the control clears it for its own C++ return.

The historical R7 positive matrix passed its then-defined checks. Evidence lives at
`build-clockint-gates/b2-gates-r7.uMy7Qo`; the complete log SHA-256 is
`321da0e3c6d0014c476ec97754ed90d13581055728c5062e20f9d4a827b273db`. It is not a freezable B2
result: the later hard-gate review found the post-publication allocator rereads and existential
lifecycle acceptance described above. R7's first inverted configure also stopped at the Clang
requirement rather than proving the required-Python failure. Its five selected tests, feature-OFF
symbol audit, and exact callback audit remain useful regression baselines only. The historical
trace-enabled binary is SHA-256
`b8ba944b26126b719bbfc63edcae3fd12df04060ea50a038a1e2870bb3091bdc`, ELF build ID
`4cee78ddb9064dedc3e209234c95bac64619b682`. Its sole unsuffixed callback symbol begins at
`0x9aefe0`, is `0x169e` bytes, has no clone, no call, no indirect or external branch, one required
`%fs:0` load, and no stack-canary path. All 226 jumps are direct and internal. The 640-byte
`rep movsq` completes before the release store that publishes the ready byte. The feature-OFF
binary is SHA-256 `bd48fa8f013070892210b2654ce74d08b43897b4a6dda408839b5bebd7e5463b`
and has no B2 producer or installer symbol. Neither historical binary is a wire-v4 candidate.

The authoritative wire-v4 R8 matrix is green at
`build-clockint-gates/b2-gates-r8.lVeZoK`. Its trace-enabled carrier is SHA-256
`177469d0979cbee270d42193bb6052f2742f0c0e47395ec2c37143daf80d2b5c`, ELF build ID
`819808cfbdbc422287b31d8140c577cee709fbf1`; its sole unsuffixed callback begins at
`0x9aefe0`, is `0x13db` bytes, and passed the production callback disassembly gate. The feature-OFF
carrier is SHA-256 `07808eafd6d7cf655074a014ec551a160d027854b52d22cffe88b5dff81e59b6`
and passed the B2 symbol-absence gate. The exact preflight JSON is SHA-256
`ee069e93af4e6f524484bb5d1fa693de01b64ee86a9919d9d4c11135054937d9`; the compiled C++
manifest artifact is SHA-256
`2f517eb6d22d206a7aab3112c5bfba158a727ca9650a4f70dfd0451f4fe866dc`. Five selected tests
passed, and the deliberately disabled Python configure failed with raw exit 1 for the exact
required-package diagnostic. The complete log and carrier-packet hashes are recorded above. The
independent stock-frame review proved the row-288/289 and row-287/290 joins and found the shared
requeue-helper return ambiguity; the decoder now requires `0xfec6e`, and its inverted wrong-caller
control passes. The independent final-ELF audit also passes over the exact carrier: one B2 callback
symbol, 1,072 instructions over `0x9aefe0..0x9b03bb`, zero calls, zero indirect or external branches,
one expected `%fs:0` thread-pointer load, no canary reference, and the fixed 640-byte inline copy
completed before the ready-byte release store. The success recipes read only the captured register
file and pinned private stack words; neither pool nor CAS/list memory is dereferenced after
publication.

The exact B2 offline/build carrier is frozen at `build-d1clock/freeze/a0d85c08-b2-r8`. Its
manifest, checksum list, receipt, and verification transcript hashes are respectively
`5dd2e447d4fe7f3731a82fb8e48ce9a92041f546be1df66944d0d61587bdba0c`,
`44597cb305f5a976f40efaa5136db687d8fd1318d7bc619ad3d7f5c0e916854b`,
`36314220f037120dfa8b697113a7f8b9c9aaf02547594e998b8321c0db40c02c`, and
`3f43850f593cff779d89a5460abdc5788fa9648fc69b56c7617c6b1df6166ccf`.
The current live-input custody is R4, the exceptional A6-reconciliation seal, not the historical
R2 input set. Its manifest, checksum-list, and receipt hashes are
`46310ba5bb0bd82d848674ff6ea496d1fe36346ad515d4266bac79fd5ed72dc6`,
`1a3d02bfc108b07524a5f780f26d09f3f34a9242356de544e048b442f50aad8e`, and
`a1095a5b0108abb7821bac6cc9fbbbe9c6986f383c0ada000379cfb4c1ad9c90`.
R8 trace-disabled context replaces the historical R3 launch context and is sealed at
`build-d1clock/freeze/a0d85c08-b2-r8-live-context-r8-trace-disabled`, with manifest,
checksum-list, and receipt hashes
`04ee4081ac28c50461d5c8c227b97dd5fa6fb358bd60abcbba133c4c697c433c`,
`c17e917cad58d0d1e915bdd68d13fa4272139b0700891085c360398393d7292f`, and
`25145f44151ad54b3917e6086be91067849eef82c8d24f6b7652c917e7950251`.
The next gate is one same-ELF trace-disabled A7 run after controller presence is recorded, using
the three external R8 context hashes. Preserve separate execution-integrity and bootflow outcomes:
a bounded deadline-owned close with custody intact can be execution PASS and bootflow STALLED.
Do not use the three-write 60 Hz patch or combine the next lifecycle packet with this B2 capture.

A successful `preflight` must match the whole-object digest, all 35 landing guards, the eight-entry
X table including the static X3 no-op, and the pinned relocation classification for every runtime
probe. Static classification plus the offline relocator controls are not proof that a detour
executed correctly in the client. The sidecar validator also does not prove mapped-object custody
independently, end-to-end disabled-path equivalence, or non-perturbation. None of these commands
starts shadPS4.

The trace-enabled build has a separate static disabled-path proof. Both `eboot.bin` loader paths
and `ArmBeforeGuestThreads` share `RequestedOutputPath()`: null or empty
`D1_CLOCK_TRACE_OUT` performs only the environment checks, so the loader skips raw-source hashing
and arming returns before loader identity reads, mapped-image hashing, session startup, or detour
installation. The arming call site is after relocation/memory setup and before `main_thread.Run`.
A feature-OFF build compiles all producer calls and objects out while retaining the legacy patch
symbols. This proves the intended static request gate and compile-out structure, not behavioral
equivalence to a legacy client execution.

## Completed B0/B1 A2 results and B2 handoff

A1's structural acceptance, zero dropped/unknown records, and accepted atomic receipt do not
overcome its terminal-`E8` return-PC defect. It is rejected for non-perturbation and cannot be
promoted, repurposed, or repaired by reinterpretation. Fresh A2 on the repaired frozen carrier has
already executed all 34 probes through an in-world stock scenario and ended with an accepted
receipt. It remains observation-only and stock-timing evidence.

The five-probe B1 observer is offline-proven, link-complete, and separately frozen. Its first live
attempt, `runs/CLOCK-B1-STOCK-20260826-125612`, passed exact preflight and armed all five probes but
hit the known stochastic Graphics Heartbeat startup fault at normalized guest `0x4faaf4`, reading
from `0x8`, before character select. The trace is a 128-byte header with zero records, no orderly
receipt exists, and the real inspector reports every probe missing plus
`raw_observation_candidate: false` and `b1_observation_receipt: false`. Treat it as a rejected A1
control attempt, not B1 evidence. Its launcher metadata records the default emulator source
directory rather than the explicit frozen executable, so custody rests on the executed
`SHADPS4_BIN` path and frozen SHA-256; the mismatched source-commit field must not be promoted. The
unchanged A2 retry, `runs/CLOCK-B1-STOCK-20260826-A2`, then cleared sign-on, character flow, orbit,
activity loading, `activity:physics_join`, and `activity:in_world`, and ended through a normal
operator close. The real inspector exited zero with an accepted producer receipt,
`raw_observation_candidate: true`, no missing probe, 49,452 records on five host-thread tokens,
28,954 locked drain snapshots, 11,843 render scalar stores, and balanced counts of 2,885 for each
Havok begin/continuous-target/result event. The 16,616,000-byte trace SHA-256 is
`64ca2f798aa6b12be05df195238ab46fbc4136c55871f17e414c06fd1a313b82`; its receipt SHA-256 is
`5190011b594073b23989bfeaf7a242d1e758176dd57bcf3ffdb87e8b39b78f22`, and the mapped-image
SHA-256 bound by that receipt is
`063aa916b7ca7b17e919683fa949f0f907584334ec5c064985e73f5030052555`.
`b1_observation_receipt` remains false by contract:
this is raw stock observation, not tick ownership, cross-thread causality, immutable-snapshot,
non-perturbation, C1-equivalence, or rate authority. The launcher's generic `account_ready` row is
false because the separate account trace was explicitly disabled; its `tracer armed 0 probes`
summary does not describe Packet B1. No independent visual or pause/resume checkpoint is claimed.

1. Preserve the passing offline sidecar gate and its acceptance/rejection controls; extend it only
   where a new intended claim is not covered by its current receipt checks.
2. Complete the trace-enabled build's end-to-end disabled-path equivalence test. Static source and
   disassembly prove the early return and absence of arming-side work when the environment variable
   is null or empty, but no client execution has established behavioral equivalence. Decide
   separately whether literal legacy-binary equivalence is required.
3. Preserve the independent decomp/disassembly, ABI, custody, receipt, and bounded return-identity
   reviews as part of the freeze packet. Do not describe the latter as signal or unwind safety.
4. Before any guest thread starts, establish that SHSTK is inactive; otherwise fail arming closed.
   Preserve the pre-thread installation evidence and the terminal-`E8` lowering proof that the
   callee-visible synchronous return PC is the stock continuation.
5. Prove execution-time detour lifecycle, mapped-object custody, drain/receipt failure handling,
   and teardown semantics before promoting any capture to evidence.
6. Freeze the exact emulator carrier, source diff or commit, client digest, probe-manifest digest,
   decoder digest, build options, active patch digest, and settings before the B1 candidate. Do not rebase
   immediately before capture.
7. Preserve the accepted frozen B1 A2 artifact as the live five-probe stock candidate. Any repeat
   still requires a clear emulator/build lane and must keep the three-write 60 Hz control and every
   elevated-rate patch disabled; never reinterpret a stochastic pre-character-select startup fault
   as observer evidence.
8. Exercise boot, orbit, one activity, pause/resume, loading, and a clean shutdown.
9. Preserve the raw trace, source-custody and mapped-image facts, atomic receipt sidecar, decoder
   output, emulator commit, patch manifest, settings, and log as one evidence packet.
10. Treat decoder output as coverage smoke until per-outer invariants and non-perturbation controls
    promote the candidate through review.
11. Review and preserve the accepted raw B1 artifact and passing B2 offline packet, then use the
    checksum-replayed supplemental live-input carrier to exercise the separately versioned 39-row
    B2 stock observer before C1. Never implement
    multi-tick execution on B0 or five-probe raw coverage alone.

## Review checklist

A live or authority-elevating change is not ready to merge if any answer below is unknown:

- Does disabled behavior meet the specified no-guest-mutation/no-trace-side-effect contract, and
  has any stronger literal legacy-binary equivalence requirement been separately proved?
- Is stock represented by `StockExact`, not normalized rational 30?
- Is wall time used only to mint exact credit?
- Is Havok passed one fixed `h` for the entire clock epoch?
- Is credit spent only after a complete guest tick commits?
- Can a tick context outlive every child job that references it?
- Are input edges and inbound packets assigned once?
- Are sends and other external effects identified and committed once?
- Is rendering reading immutable completed state?
- Is interpolation excluded from authoritative state?
- Are pause, load, hitch, debt, and epoch transitions explicit and logged?
- Is rational pacing absolute and catch-up-free?
- Does VRR remain a host presentation policy?
- Did the exact pinned source hash and every landing byte pass?
- For every terminal displaced `E8`, does durable lowering preserve the stock continuation as the
  callee-visible same-thread synchronous return PC, without claiming signal or unwind identity?
- Was SHSTK established inactive before pre-thread installation, with arming fail-closed otherwise?
- Is any 30/45/60/72/90/120/144 result labeled as an offline model control or future separately
  gated configuration rather than an implemented live scheduler mode?
- Did B0, B1, B2, C0, and C1 close in order before multi-tick execution?
- Was 60 Hz authority validated before any higher authority rate was claimed?

That is the bar for saying the physics engine is held properly while Destiny 1 presents above its
stock cadence.
