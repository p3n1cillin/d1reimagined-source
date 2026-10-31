# Frame-rate independence, phases 0 and 1: the implementation (2026-10-06)

Branch `frame-rate-phase1`, implementing FRAME-RATE-INDEPENDENCE-20261006.md sections 4.2
(Phase 0, the unit fixes), 4.3, 4.4, 4.6, 4.7, 4.9 and 4.11 (Phase 1, camera interpolation)
with the author's corrections of 2026-10-06 (camera hook site, director dt in catch-up groups,
alpha on continuation frames). Phases 2 to 4 are not here. A review found eleven items (six
distinct); section 10 says what was real, what changed and why. A second review (the design's
author, on `cf917a5b`, branch `frame-rate-phase1-fix`) found four more: D1_INTERP under the
fixed-clock override, the extract order, the rotation snap and the aim-lag note (section
10.1). Nothing has run on the seat yet: the seat test plan is section 11, the last section.

Reach tags: `[C]` read in `native/out/eboot.elf` with capstone or in the code named, `[O]`
observed in a run on this machine (WSL headless walk or the self-test), `[R]` read in a
document, `[I]` inferred, `[U]` unknown. Addresses are eboot virtual addresses (manifest
addresses are these + 0x400000).

## 1. What landed

| File | Change |
|---|---|
| `d1port/src/d1_frame_time.c` (new) | the switches, the sites, the trampoline emitter, the handler |
| `d1port/src/d1_frame_time.h` (new) | `d1_frame_time_install(image, size)`; `d1_frame_time_extract_order(...)`, the extract order counts since install, for the self-test |
| `d1port/src/probe.c` | one include, one call right after `install_engine_hooks(size)` on both hosts (after the merge with main's Windows port; the image is still writable there) |
| `native/build-headless.sh` | the new file beside `probe.c` on the runtime build line (core, discovery, windowed) and in `trap_sources`, Linux and Windows; `-lm` in the Linux `trap_libraries` (Windows already had it) |
| `d1port/tests/test_frame_time.c` (new) | the self-test: a fake image with the pinned bytes at every site, the real installer, trampolines and handler; build and run lines in its header |
| `native/guest/CUSA00219-frame-units.xml` (new) | the cloth constant replaced by the packet's accepted time, for the emulator's fixed-clock profile only (its header says why it must not be combined with the native clock); the loader does not use it and hooks over it if present |

Two switches, both off by default, independent of each other, read once at start:

- `D1_FRAME_UNITS=1`: every unit fix that applies to the play configuration (section 3: all
  four with the fixed-clock override off, cloth and ui with it on). A comma list selects some
  of them, for testing one at a time: `D1_FRAME_UNITS=director`, `cloth`, `fades`, `ui`, or
  for example `cloth,ui`, under the same rule. An unknown name prints a notice and selects
  nothing for that name.
- `D1_INTERP=1`: alpha, the director gated to ticks, camera interpolation, shader time
  (section 4). With the fixed-clock override on, alpha 1 and no director gating (3.0, 4.1).

The loader prints one line at start in every case, naming the configuration it read (section
3.0): `Frame time: D1_FRAME_UNITS off, D1_INTERP off (both default off; ...); fixed-clock
override byte 0x18c4aa4: override off: wall-time ticks` (or `override on: one tick per
frame`), or with a switch on `Frame time: fixed-clock override byte 0x18c4aa4: override off:
wall-time ticks; D1_FRAME_UNITS on ( director wall dt cloth frame share fades h UI ms carry ),
D1_INTERP on (...)` (only the fixes that will be installed are named; `on (none applicable)`
when every named fix was skipped). With the override on and director or fades selected, a
second line names them as skipped and says why; with the override on and `D1_INTERP=1`, the
switch line reads `D1_INTERP on (alpha 1 under the override: no camera blend, no shader-time
add, no director gating)` and a notice line says why (3.0). Then one line per site installed or left out,
and `Frame time: N of N applicable sites installed`, where the applicable sites are those of
the fixes that remain after the skip `[O]`.

Windows (after the merge with main's Windows port): the same code on both hosts. The handler
is declared `sysv_abi` (`runtime.h`'s `ABI`), since the trampolines call it as the guest's SysV
code would; the clock is `host_monotonic_ns`; the trampoline page comes from `runtime_low_map`
(the next host-owned piece of the placeholder-reserved guest space), else `VirtualAlloc` at 64
KiB hints below and above the image, as `install_engine_hooks`'s page does; each site write is
`VirtualProtect`ed and flushed; the page is sealed execute/read. Nothing is Linux-only, so no
notice is printed for a missing piece. Built under `-Werror` in CLANG64 `[O]`; not yet run on
Windows (section 9).

## 2. Mechanism

One 16 KiB code page is mapped within rel32 reach of the image (`image - 0x24000`, then
`image_end + 0x20000`, then two farther candidates; it does not collide with
`install_engine_hooks`'s page at `image - 0x10000`: both were present in the same walk `[O]`).
Each site gets a 1 KiB slot (15 sites, with the director's call wrap of section 4.2). Three
trampoline shapes, one handler:

- **Entry hook** (as `install_engine_hooks`): the pinned six-byte prologue `push rbp; mov
  rbp,rsp; push r15|r14` becomes `jmp rel32; nop`; the trampoline saves the registers, calls
  the handler, restores, runs the displaced prologue and jumps to entry+6. The director's
  variant tests the handler's return value first: 1 means restore and `ret` to the caller
  without running the function (used on zero-tick frames under `D1_INTERP`).
- **Mid-function hook**: one position-independent instruction (5, 7 or 8 bytes) becomes `jmp
  rel32` plus nops; the trampoline runs the displaced instruction first when it is kept (the UI
  loads `vmovss xmm0,[rbp-0x30]` as they are; the `lea r14,[rip+..]` re-emitted as `mov r14,
  imm64`; the cloth loads are not re-emitted, the handler sets XMM0), then the handler, then
  jumps to a chosen resume address.
- **Call-site wrap**: a `call rel32` becomes `jmp rel32`; the trampoline calls the handler
  (before), calls the original target, calls the handler (after), jumps back to site+5.

The saved frame is rbp, rdi, rsi, rdx, rcx, r8, r9, rax, r10, r11 and XMM0-15. The stack is
16-aligned for the handler call in every case (entry: RSP is 8 mod 16, padded; mid-function and
call sites: 0 mod 16, as each enclosing function's prologue leaves it `[C]`: `0x94b160`,
`0x7f0850` and `0x52be00` push five registers after rbp and subtract an amount that is 8 mod
16; `0x812d90` pushes four and subtracts 0x20; `0x7ea1f0` and `0x7eb200` push rbx and rax). The handler may rewrite XMM0 (a dt; the upper lanes are cleared, as a `vmovss` load
leaves them) or RAX (the UI result). The callee-saved registers are kept by the C handler;
rflags are not live at any of the sites `[C]`.

Pinned bytes are compared before any write; a site that differs is left out with a printed
notice. The two cloth sites also accept the frame-units manifest's bytes and hook over them.
Trampoline sizes as emitted: entry 379 bytes, entry with skip 561, mid-function 371 (cloth),
376 (UI) and 381 (first-person), wrap 742 `[O]`.

Cost: about seventeen handler calls a frame (section 6), each a register save, a short C
function with no I/O, and a restore. One report line every five seconds from the frame-start
path. No syscalls on the hot path other than `clock_gettime(CLOCK_MONOTONIC)` (vDSO; on
Windows the runtime's `host_monotonic_ns`) in the director and view handlers.

## 3. Phase 0: the unit fixes (`D1_FRAME_UNITS`)

### 3.0 Which fixes apply: the fixed-clock override byte, read at run time

The design's binding rule (FRAME-RATE-INDEPENDENCE-20261006.md section 5, "Two play
configurations"): `D1_FRAME_UNITS` must be correct under both play configurations and must
read the override byte rather than assume it.

- The byte: image offset `0x18c4aa4`, an eboot data byte, `01` as shipped. `0x5908cb` reads it
  (`lea rax,[0x18c4aa4]; cmp byte [rax],0`) and, while it is set, replaces the measured tick
  count with one tick of the fixed step per rendered frame `[C][R]`
  (`guest/CUSA00219-native-clock.xml` header). The clock manifest writes `00`.
- When it is read: in `d1_frame_time_install`, which runs after the guest patches are applied
  (`apply_patches`, then `install_engine_hooks`, then this), so the value is the one this run
  plays with `[C]`. Nothing in the loader writes it afterwards; whether the game itself ever
  writes it at run time on PS4 is `[U]` (the Xbox layer clears it at start, per the clock manifest's header `[R]`).
- **Override off** (the clock manifest, uncapped; the Linux play configuration): wall-time
  ticks, all four bugs are live, and `D1_FRAME_UNITS` installs what it names; `1` means all
  four.
- **Override on** (one tick per rendered frame; the Windows locked-60 configuration, 60 Hz
  manifest plus cadence on the stock vblank): the director's `h` and F10's `+0x08` are already
  the frame's time, so the director (3.1) and fades (3.3) fixes would be wrong. They are not
  installed even when named, and a notice names them as skipped and why. Cloth (3.2, a constant
  1/30 s) and the UI clock (3.4, truncated milliseconds) are wrong in both configurations and
  are installed when named; `1` means cloth and ui.
- A byte outside the image (never seen) is taken as on: the director and fades fixes are left
  out, the conservative side.
- The start line names the configuration it read (`override on: one tick per frame` or
  `override off: wall-time ticks`), and `N of N applicable sites installed` counts the sites of
  the fixes that remain after the skip. Applicable site counts, per configuration:

| `D1_FRAME_UNITS` (`D1_INTERP` off) | Override off | Override on |
|---|---|---|
| `1` | 10 | 6 (cloth, ui) |
| `director` | 3 | 0 (skipped) |
| `cloth` | 4 | 4 |
| `fades` | 5 | 0 (skipped) |
| `ui` | 4 | 4 |
| `1` with `D1_INTERP=1` | 15 | 10 (no director sites) |
| `D1_INTERP=1` alone | 8 | 6 (no director sites) |

  (Each count includes the two view-build wraps, installed whenever any fix or `D1_INTERP`
  is.)
- **`D1_INTERP` with the override on: alpha 1, no director gating** (second review, item 1).
  `0x5905c0` writes the kept fraction `G2+8` at `0x5908c4`, before the override test at
  `0x5908cb`; with the byte set, the override replaces only due (`ebx = 1`, `0x590965`) and
  accepted (`xmm1 = G+8`, `0x590960`) `[C]`. So `G2+8` is the discarded accumulator's phase
  against its own grid, not the phase of any tick the game ran. Blending by it at a locked 60
  would show pose(T-1+t) with t drifting on the 60.00 / 59.94 beat: latency drift, a
  near-freeze frame about every 17 s, jitter `[I]`. With the byte `01`, `D1_INTERP` therefore
  uses alpha 1 everywhere: the camera is served live (no snapshot, no blend) and the shader
  time gets no add. Its director gating (the director entry hook for `D1_INTERP` and the call
  wrap `0x4cba8c`) is not installed either, since its stall branch would feed the Phase 0
  wall-time dt, which must not apply with the byte `01`; the director keeps the engine's `h`.
  The start line says `alpha 1 under the override` and a notice line gives the reason. The
  other `D1_INTERP` sites (view wraps, first-person switch, getter wrap, shader-time wraps)
  still install and run as no-ops, so the mechanism can be checked under the override without
  a visible change.

### 3.1 Camera director: wall time since it last ran (`director`)

Override off only (3.0).

- Site: `0x8ede20`, entry hook, pinned `55 48 89 e5 41 57` `[C]`. Single caller `0x4cba8c`
  in `0x4cba50`, which loads h through `0x4cba6b call 0x590a30` and passes it in XMM0 `[C]`.
- Rule: dt = wall time since the director last ran, clamped to 0.5..100 ms; the first call
  keeps the engine's value. F11 runs only on frames with record +0x2b set (`0x74ba94`) `[C]`,
  so in a 2-tick catch-up group the director runs once and receives about two ticks of wall
  time, as the correction asks.
- Calls per frame: 1 per final frame.
- Expected effect: camera smoothing, transitions and the orbit camera's drift take the same
  wall time at 60 and at 160 FPS (today 2.7x fast at 160). Whether the director also scales
  look input by dt is `[U]` (design 8): the look-rate test in section 11 answers it.

### 3.2 Cloth: the frame's share of its group, not 1/30 s (`cloth`)

Both configurations (3.0). With the override on there are no catch-up groups, so the share is
the frame's whole record +0x08 `[I]`.

- Sites: `0x7f0905` and `0x7f090f`, `vmovss xmm0,[rip+disp]` loading `0x1652170` = 0.0333
  `[C]`, pinned `c5 fa 10 05 63 18 e6 00` and `c5 fa 10 05 59 18 e6 00`, mid-function hooks
  resuming at the store `0x7f0917` `[C]`. The engine's guard is untouched: `0x7f08e1 cmp
  byte [0x18c6eda],0` (01 in the file), `0x7f08ee vucomiss` of 0.001 against the packet's
  `+0x192e8`, and below 1 ms dt 0 with the pause byte `[rbp-0x15540]` `[C]`.
- Value: the published frame record's +0x08 for this frame, found by packet. The record's
  +0x08 is the frame's share of its catch-up group: on a measured frame `accepted - (n-1) x
  period` (`0x590def-0x590dff`), on a continuation frame one period (`0x590d61`), on a
  zero-tick frame the accepted time (`0x590e2b`); the shares of a group sum to its accepted
  time `[C]`. The packet's `+0x192e8` is not usable: X5 copies record +0x0c into it every
  outer frame (`0x7eaf67-0x7eaf7c`), and the continuation path copies the measured frame's
  accepted time (`0x590d92`), so both frames of a 2-tick group carry the whole group's time
  `[C]` (review finding 5).
- Plumbing: the view-build wrap (`0x7ea1fc` / `0x7eb209`, rdi = the frame packet `[C]`)
  stashes (packet, record +0x0c, record +0x08) in an eight-entry ring; the cloth handler looks
  up its packet (RDX, the same packet layout: cloth reads `+0x192e8`, `+0x187b5`, `+0x191e1`,
  X5 writes `+0x192e8`, `+0x1fc19` `[C][I]`) and uses the share only if the stashed accepted
  time equals the packet's, so a stash from another frame never matches. No match: the
  packet's accepted time, counted as `fallback`. The share is clamped to 0..100 ms.
- Calls per frame: 1 per cloth solve, plus the stash in the view wrap.
- Expected effect: cloth at wall speed at 30, 45, 60 and 160 FPS: the report's `cloth dt
  sum` reads 5.0 s per 5 s window when cloth is solved every frame (section 5).

### 3.3 F10 fades and effect ages: h per tick (`fades`)

Override off only (3.0).

- Sites: `0x7ccca0` (pinned `55 48 89 e5 41 56`), `0x7d6fb0` (`.. 41 56`), `0x8722c0`
  (`.. 41 57`), entry hooks `[C]`. Each has exactly one caller, F10 `0x74b770`, which runs
  them on tick frames only (gate `0x74b7ff` on rec+0x2a) with XMM0 = rec+0x08 `[C]`.
- Rule: XMM0 = h (from `*(0x26a3c00)+8`, falling back to 1/60 when the block is absent or the
  value is implausible).
- Calls per frame: 3 per tick frame (about 900 per 5 s).
- Expected effect: at 160 FPS fades and effect ages run at 1.0x instead of about 0.375x.

Not changed: `0x87fd30` (`0x74b914`) and `0x7c8c30` (`0x74b991`), which the design does not
list as bugs.

### 3.4 UI millisecond clock: carry the fraction (`ui`)

Both configurations (3.0).

- Function `0x94b160`, single caller F11 `0x74bb4f` with XMM0 = rec+0x04 (wall elapsed) and
  XMM1 = rec+0x0c (accepted) `[C]`. It stores XMM1 at `[rbp-0x30]` and XMM0 at `[rbp-0x34]`
  (`0x94b174`, `0x94b179`); the game-time path loads `[rbp-0x30]` at `0x94b1c2`, the other
  path loads `[rbp-0x34]` at `0x94b1cc`; both reach `0x94b1d1 vmulss` by 1000.0, `0x94b1d9
  vcvttss2si`, `0x94b1dd lock add [0x272b5c8],eax`; when paused the add is skipped
  (`0x94b1bd jne 0x94b1e4`) `[C]`.
- Sites: `0x94b1c2` and `0x94b1cc`, mid-function hooks, pinned `c5 fa 10 45 d0` and `c5 fa 10
  45 cc`, both resuming at `0x94b1dd`. The handler computes `v = dt x 1000 + fraction[path]`,
  `eax = trunc(v)`, `fraction[path] = v - eax`, and the engine's own `lock add` runs.
- Thread safety (corrected, review finding 4): `0x94b160` holds no lock here. The loop at
  `0x94b190-0x94b1a2` is a compare-exchange that copies the counter `0x272b5c8` (read at
  `0x94b17e`) into `0x272b5cc` and acquires nothing `[C]`. The two fraction statics are safe
  because `0x94b160` has one caller, F11 at `0x74bb4f`, which runs once per frame, and frames
  are serialized `[C][I]`. A second caller would need atomics.
- Calls per frame: 1 per final frame (none while paused).
- Expected effect: the UI clock runs exact instead of a few percent slow (16.683 ms frames
  truncate to 16 ms: 4% slow at a steady 60). Self-test: 600 frames of 16.6833 ms give 10009
  ms instead of 9600 `[O]`.

## 4. Phase 1: alpha and the camera (`D1_INTERP=1`)

### 4.1 Alpha source

Alpha is the engine's kept fractional tick `G2+8`, `G2 = *0x26a3c08`, read on the extract
path of the same frame:

- `0x5905c0` is called once per measured outer frame from `0x590dc9` and writes `G2+8` before
  the frame's graphs are instantiated `[C]`. The extract of frame N completes before frame
  N+1's `0x5905c0` (the outer loop builds the next frame after the frame's jobs, CADENCE 3.3)
  `[C][I]`. If `build conflicts` ever reads non-zero, that argument is wrong and a per-slot
  stash comes back.
- On continuation frames `0x5905c0` is not called (`0x590d2f cmp [r14+0x40],0`; `0x590d36`
  is the continuation path) `[C]`, so `G2+8` still holds the measured frame's value.
- Per frame, from the published record (`0x26ae348 + idx*0x38`, idx at +0x70): +0x2b set (last
  frame of a group, `0x590ecd`) -> alpha = clamp(`G2+8`, 0, 1); clear -> alpha = 1.
- With the fixed-clock override on, alpha is 1 on every frame and nothing is blended or added
  (3.0): `G2+8` there belongs to no tick.
- Which tick a build's alpha belongs to depends on the build coming after the director in the
  same outer frame; 4.3.1 measures that and handles the other order.

### 4.2 Director gated to ticks

- Tick counter: `G+0xc` (`G = *0x26a3c00`) is incremented once per call of `0x590a50`
  (`0x590a6d-0x590a72`), whose only caller is F1 at `0x74a97f`, gated on bit 1 of the byte
  `0x26ae454` (`0x74a96f`) `[C]`. It is also set from outside the tick by `0x5905b0`
  (`mov [G+0xc], edi`), reached by `jmp` from `0x6e60c2`, an engine interface entry `[C]`;
  when that runs is `[U]`.
- Rule: if the count has not advanced since the director last ran, the handler returns 1 and
  the trampoline returns to `0x4cba91` without running `0x8ede20`; otherwise dt = h x span,
  span = ticks advanced (2h once in a 2-tick group, the correction's rule).
- **Resync** (review finding 10): a count that moved backwards or by more than the catch-up
  allowance (2) was set from outside the tick, or the director did not run for a while: dt =
  h, the count is taken as the new base, and the camera snaps (no blend across it). Counted
  as `resyncs`.
- **Stall** (review finding 3): when the tick stops (the engine's paused test, `G+0` set and
  `G2+0` bit 1, as `0x5909e0` reads it `[C]`, or no tick for three tick periods, about 50 ms)
  the director runs every frame with the wall time since its last run, clamped to 0.5..100 ms
  (the Phase 0 rule), and the view build blends nothing until ticks resume; the first tick
  after a stall seeds the snapshots afresh. Counted as `stall runs` and `stall frames`. This
  replaces the earlier safety net (one forced run per 21 frames), which made a stalled camera
  move at a few hertz.
- Each run publishes the total of ticks the director has consumed and a snap counter (totals,
  so a view build that misses a run still sees the whole span), with the tick count `G+0xc`
  it ran at and the F0 frame serial (4.3.1), in one 64-bit word (total and tick count mod
  2^24, the serial byte, two flag bits) so a build reads a consistent set.
- **Published when the director returns** (second review, item 2). The entry hook runs before
  the body writes the pose; publishing there let a concurrent build pair the new tick total
  with the old pose. The director's only call, `0x4cba8c call 0x8ede20` in F11's `0x4cba50`
  (pinned `e8 8f 23 42 00`; RSP 0 mod 16 there: four pushes and `sub rsp,0x38` `[C]`), is
  wrapped; the after-handler publishes the run the entry hook prepared. If that site is left
  out, runs are published at the entry, with a notice. Not installed under the override.

### 4.3 Camera: snapshots by tick, the blend served to the extract build only

- Reviewer check, done: the packet's views come from the `0x812d90` build. `0x7e7900` has one
  caller, `0x812ed8` inside `0x812d90` `[C]`. `0x812d90` builds the main view `0x26d7fe0`
  through the callback `*0x26d8c20` (`0x812dd8`), the shadow views from it (`0x812e48`,
  `0x812e58`), the first-person view `0x26d83f0` through the same callback (`0x812e67`), then
  calls `0x7e7900` `[C]`. The callback pointers are set by `0x812b30` from `0x52bde0`:
  `*0x26d8c20 = 0x52be00`, `*0x26d8c28 = 0x52c360` `[C]`. The game-graph build `0x812d10`
  calls `*0x26d8c28` for `0x26d7fe0` and tail-jumps to `*0x26d8c20` (`0x52be00`) for the
  first-person view `0x26d83f0` (`0x812d6b-0x812d7a`) `[C]`, so `0x52be00` runs on both paths:
  anything that serves a blend must know which path it is on.
- **Snapshot rule** (review findings 1 and 2): at the extract build, if the director's
  consumed-tick total advanced since the last snapshot, previous = current and current = the
  live pose, even when the pose is unchanged, and span = the ticks between them. No advance:
  no shift (a change of the live pose without one is counted as `drift`). Keying on the
  director's runs rather than on `G+0xc` directly matters below 60: the count advances on the
  non-final frame of a group, where F11 and the director do not run.
- **Blend**: t = (span - 1 + alpha) / span, clamped to 0..1, so a 2-tick span shows tick
  T-1+alpha (the old rule showed T-2+2 alpha). Position lerps; forward and up lerp, then
  forward is normalised and up re-orthogonalised. fov is not blended (below).
- **Serving** (review finding 8): nothing writes the camera globals. The view-build wraps
  (`0x7ea1fc` / `0x7eb209`, pinned `e8 3f 94 02 00` / `e8 32 84 02 00`) compute the blend and
  set a thread-local flag; the first-person hook (`0x812e5d lea r14,[rip+0x1ec558c]`, pinned
  `4c 8d 35 8c 55 ec 01`, resume `0x812e64`) and the after-wrap clear it. The call `0x52be45
  call 0x8f2e80` inside `0x52be00` is wrapped (pinned `e8 36 70 3c 00`): before, the three
  output pointers (rdi position, rsi forward, rdx up, `0x8f2e93-0x8f2ec9` `[C]`) are kept;
  after, while the flag is set, the outputs are replaced by the blend, only if they equal the
  live pose read at the build's start (so the getter's own fallback values, `0x8f2ef6..`, and
  any write since are left alone, counted as `mismatched`). The game-graph build, the engine
  interface reader `0x6e639f` and `0x5b71aa` run on other threads or outside the flag's window
  and never see a blend `[C][I]`.
- **fov** is served live: `0x52be00` reads it through `0x8f2ac0` (`0x52be26`, XMM0) and
  `0x8f2b00` reads `+0x4ac` again itself (`0x8f2baf`) for the parameter block `[C]`; blending
  one without the other would give the view an inconsistent fov. A zoom transition therefore
  steps at tick rate; fixing it needs a hook inside `0x8f2b00` (open item).
- **Snaps** (design 4.7; the turn rule from the second review, item 3): the first pose; a
  resync; a position jump over 2.5 units or a fov change over 20 degrees (0.35 if the value
  reads as radians; the unit is `[U]`) per tick of span; a turn over 90 degrees a tick only
  together with a position step over 0.5 units a tick. A turn alone never snaps: the old 45
  degrees a tick (2700 degrees a second) is within a high-sensitivity flick and snapped
  mid-flick. 90 degrees a tick is 5400 degrees a second; with a step over 0.5 units it is a
  cut to a nearby viewpoint (a third-person camera, a respawn close by) that the 2.5-unit jump
  misses, while the camera on foot moves well under 0.5 units a tick `[I]` (the bound is a
  judgement, to be checked against `snaps` on the seat). At a span of 2 or more, 90 degrees a
  tick is 180 or more, which no turn exceeds. A turn near 180 degrees within one span lerps
  the forward through zero: the blend takes the newer axis there. After a snap the next tick
  is not blended (one-tick quarantine).
- One snapshot update at a time: a second build while one is updating is served live and
  counted as a `build conflict`.
- Calls per frame: view wrap 2, first-person 1, getter wrap 4 (main and first-person view).

### 4.3.1 Extract order (second review, item 2)

The blend assumes the extract-path view build (`0x812d90` through `0x7ea1f0` / `0x7eb200`)
runs after F11's director in the same outer frame, so the newest snapshot is the tick alpha
belongs to. The cross link X2~F11 suggests it; it was not established.

- **The serial.** F0 (`0x74a420`, the first game phase) increments the byte `*0x26ae448` at
  `0x74a44b`, at its start (`0x74a444 mov rax,[rip+0x1f63ffd]; inc byte [rax]`), unconditionally:
  the input poll after it is gated on +0x29, the increment is not `[C]`; the phases are
  instantiated every outer frame (design 1.1), so it counts outer frames, continuation frames
  included `[C][R]`. `0x74bd40` allocates the byte and sets it to 1; `0x749c10` and
  `0x749c30` read its low bit (a frame parity) `[C]`. The nine bytes at `0x74a444` are pinned;
  if they differ, the counter and its late rule are off, with a notice. The record flip
  (`0x590ede xor byte [0x26ae3b8],1`) carries only one bit and was not used.
- **The counter.** On each build of a final tick frame (record +0x2a, set by F0 at
  `0x74a73d` / `0x74a74b`, and +0x2b), compare the serial stamped with the director's last
  published run against the serial now: equal is **on time**; different with the tick count
  `G+0xc` ahead of that run's is **late** (the build came first: the run is still to come, or
  will not come); different with the count not ahead is **no tick** (nothing was pending: a
  tick flag the count did not record, the `0x74a96f` gate, or a build before F1's increment).
  Only final frames are classified, since F11 does not run on a non-final one.
- **The late rule** (the fallback, behind the same counter). A late build keys alpha to the
  newest snapshot's own tick: t = (span - 1 + alpha + lead) / span, lead = the ticks the count
  is ahead of that snapshot (clamped to 2). Any lead of 1 or more gives t >= 1: the build holds
  the newest pose instead of blending the older pair by this frame's alpha, which would step
  the view back (from T-1+a to T-2+a'). The next build shifts to the late run and blends as
  usual, so even a build that is always late interpolates at a consistent one-tick lag above
  60 FPS, off by at most this frame's alpha on tick frames. On time, lead is 0 and the rule is
  the ordinary blend; with late 0 it never engages. Counted as `held`.
- Expected: late 0, no tick 0. If the seat shows late mixed with on time, the late rule is
  already the treatment. If late is all, the ordering is the other way round: the view still
  never steps back, but tick frames show no blend (every frame at a steady 60), and taking
  the snapshot at the director's return would restore it (open item 14).

### 4.4 First-person weapon

Not interpolated in this phase. The first-person twin record (+0x70 of the `0x894550` extract
record) needs a per-slot node history keyed by accessor slot, the Phase 2 mechanism. The
first-person view is built on the live (tick) pose, so the gun and its own view agree; the
world behind it is up to one tick behind. See section 7.

### 4.5 Shader time

`0x82d7e0` (rdi = packet+0x18960) writes +0x0c, +0x10, +0x14 as (tick mod N) x h and +0x18 =
+0x0c unless a flag selects a frame-based value `[C]`. Sites: its calls at `0x7ea22a` (`e8 b1
35 04 00`) and `0x7eb237` (`e8 a4 25 04 00`), call-site wraps; the after handler adds alpha x h
to +0x0c, +0x10, +0x14, and to +0x18 when it equalled +0x0c. 2 calls per frame. No add with
the fixed-clock override on (3.0).

### 4.6 Per-frame systems

Particles take the packet's accepted time every frame (design 1.1); by the same mechanism as
cloth they may integrate a 2-tick group's time twice below 60 FPS `[I]` (open item; not
changed here). Cloth and UI are Phase 0 (3.2, 3.4). Audio's time base stays `[U]`.

## 5. Instruments

The report line, every five seconds, from the frame-start path:

```
Frame time [5.0 s]: frames N (due 0/1/2/3+: a/b/c/d; continuation e); director runs r (span 1/2/3+: x/y/z, ticks T) skips s stall runs q resyncs R dt mean X ms; fades F; ui U; cloth calls C dt sum S s fallback B; camera snapshots P snaps J blends L t mean A still W drift D stall frames Q served V mismatched M fp switches P2; shader time adds T2; build conflicts K; extract order on time O late Z (held H) no tick Y
```

Expected per 5 s window, in the world, moving, steady play (59.94 ticks a second, so about 300
ticks a window; "final frames" = frames - continuation, since a 2-tick group is one measured
non-final frame and one final continuation frame):

| Field | Installed under | Expected |
|---|---|---|
| frames | any switch | presents per window (Frame stats). Twice that means both extract jobs run each frame (open item 5) |
| due, continuation | any switch | at 60 FPS and above: due 1 about 300, due 2 and continuation 0. Below 60: continuation = due 2, and (due 1 - continuation) + 2 x due 2 = about 300 |
| director runs, dt mean | `director` alone | runs = final frames; dt mean = 5000 / runs ms (1000 / FPS at 60 and above) |
| director runs, span, ticks, skips | `D1_INTERP` | span ticks about 300; runs about 300 at 60 FPS and above (span 1), fewer below 60 (span 2 per group); runs + skips + stall runs = final frames; dt mean = 16.68 x ticks / runs ms |
| stall runs, stall frames, resyncs | `D1_INTERP` | 0 in play; non-zero only around pause, menus, loads, joins (note the time) |
| fades | `fades` | 3 x tick frames, about 900 |
| ui | `ui` | final frames (fewer while paused) |
| cloth calls, dt sum, fallback | `cloth` | calls = frames with cloth solved (0 with no cloth in view); dt sum = 5.0 s x calls / frames at any frame rate (less only below 30 FPS, where the game itself drops time); fallback 0 |
| camera snapshots | `D1_INTERP` | = director runs (each run seen by one build) |
| snaps | `D1_INTERP` | 0 while playing; one per teleport, respawn or cut |
| blends, still | `D1_INTERP` | blends + still = final frames with a fraction above 0. Turning: still about 0. Standing still: blends about 0 |
| t mean | `D1_INTERP` | about 0.5 at 60 FPS and above |
| drift | `D1_INTERP` | 0; non-zero means something besides the director moves the camera between ticks (design 8): a finding |
| served, mismatched | `D1_INTERP` | served = blends; mismatched 0 |
| fp switches, shader time adds | `D1_INTERP` | = frames |
| build conflicts | `D1_INTERP` | 0 |
| extract order on time | `D1_INTERP` | = director runs (one per final tick frame; stall runs are not tick runs); twice that if both extract jobs build each frame, as `frames` (open item 5) |
| extract order late, held | `D1_INTERP` | 0 (4.3.1). Non-zero: the build ran before the director in that frame; held = late builds the late rule served live. Note whether late is mixed with on time or all |
| extract order no tick | `D1_INTERP` | 0; non-zero: a final frame flagged as a tick (+0x2a) whose tick the count did not record (the `0x74a96f` gate), or a build before F1's increment |

With the fixed-clock override on and `D1_INTERP=1` (alpha 1, 3.0): director runs, skips and
stall runs come only from `D1_FRAME_UNITS` (none: the director is not hooked); camera
snapshots, snaps, blends, served, shader time adds and the extract order are 0; fp switches =
frames.

The view wrap (and so the histogram) is installed whenever any switch is on; with both off
nothing is installed and no line is printed. Flips per outer frame (design 4.9) is a
renderer-side instrument and is not here.

## 6. Hook budget

| Site | Kind | Calls per frame |
|---|---|---|
| director `0x8ede20` | entry, skip-capable | 1 (final frames) |
| director call `0x4cba8c` (`D1_INTERP`, override off) | wrap | 2 (final frames) |
| fades `0x7ccca0`, `0x7d6fb0`, `0x8722c0` | entry | 3 (tick frames) |
| UI `0x94b1c2` / `0x94b1cc` | mid-function | 1 (final frames) |
| cloth `0x7f0905` / `0x7f090f` | mid-function | 1 (per cloth solve) |
| view build `0x7ea1fc` / `0x7eb209` | wrap | 2 |
| first-person switch `0x812e5d` | mid-function | 1 |
| camera getter `0x52be45` | wrap | 4 (two views) |
| shader time `0x7ea22a` / `0x7eb237` | wrap | 2 |

About seventeen handler calls a frame against the 30-40k of the draw hooks (CADENCE 4): a few
microseconds a frame in all `[I]`, to be confirmed by the pacing line on the seat.

## 7. What is not done, and why the first-person view is built on the tick pose

- **First-person twin record history** (design 4.4): Phase 2's mechanism. Until it exists, a
  gun root extracted at tick k drawn through a view at k-1+alpha would be displaced on screen
  by the camera's inter-tick motion, a sawtooth that resets every tick. Building the
  first-person view on the live pose keeps the gun fixed on screen; the world behind it lags it
  by at most one tick. The seat test judges which is less visible (section 11, test C).
- fov interpolation (4.3).
- Non-final frames of a catch-up group show the director's last snapshot (alpha 1): below 60
  that frame carries no new camera information, as today (F11 does not run on it). Design 4.9
  option 2 (do not present those frames) is the follow-up.
- The 97-site `h` audit (design 4.2); camera cuts from the visibility system's flag; the
  flips-per-outer-frame counter; audio and network time bases `[U]`.

## 8. Compile and headless results (WSL Ubuntu, gcc 11.3.0, this machine)

After the review fixes:

- `gcc -std=c11 -O2 -g -Wall -Wextra -Werror -pthread -fsyntax-only -Id1port -Id1port/src
  -Inative/out` on `d1port/src/d1_frame_time.c` and `d1port/src/probe.c`: clean `[O]`.
- `bash native/build-headless.sh`: `out/d1-probe` built `[O]`.
- `D1_WALK_TIMEOUT=20 bash native/walk.sh` (builds `--discovery`): switches off, `exit=20` at
  `sceVideoOutOpen` (import index 301, caller `0x7e296c`), 199 log lines, line 2 `Frame time:
  D1_FRAME_UNITS off, D1_INTERP off (...)` `[O]`.
- `D1_FRAME_UNITS=1 D1_INTERP=1 D1_WALK_TIMEOUT=20 bash native/walk.sh`: `exit=20`, same stop,
  `Frame time: 14 of 14 applicable sites installed`, every site's pinned bytes present in the
  real image, trampoline page at `image - 0x24000` `[O]`.
- With `D1_STEP_WITNESS=1` as well: both pages present (`0x7ffff0600` for the witness,
  `0x7fffdc000` for these), `exit=20` `[O]`.
- `python3 native/guest_patches.py native/guest/CUSA00219-frame-units.xml --eboot
  native/out/eboot.elf --list` (main checkout's eboot): both writes listed with their pinned
  originals `[O]`.
- No hook site is reached before the stop (the game loop starts after `sceVideoOutOpen`), so
  the headless log carries no report line `[O][C]`.
- **Self-test** `d1port/tests/test_frame_time.c` (build and run lines in its header), 0
  failures under each of: both off, `D1_FRAME_UNITS=1`, `D1_INTERP=1`, both; `--alt` (cloth
  sites holding the manifest's bytes) with both on and both off; `D1_FRAME_UNITS=director`,
  `cloth`, `fades`, `ui`, `cloth,ui` (3, 4, 5, 4, 6 sites); an unknown name `[O]`. It checks:
  the director's first call, the zero-tick skip and 2h after two ticks under `D1_INTERP`, the
  clamped wall dt under `director`; the fades; the UI carry (10009 ms against the stock 9600);
  the cloth share of a 2-tick group summing to the accepted time, the fallback for an unknown
  packet, and the stock constant when off; the camera: first-pose snap, quarantine, a blend by
  0.25, a camera at rest after a move staying put on the next tick and between ticks (finding
  1), a 2-tick span at alpha 0.5 landing at T-1+alpha (finding 2), alpha 1 on a non-final
  frame, a teleport snap, a tick count set backwards (dt h, snap, finding 10), the getter's own
  fallback left alone, a paused stall (director on wall time, no blend, a fresh seed after,
  finding 3) and a stall by time; the first-person view always on the live pose and the camera
  globals never written (finding 8); the first-person hook leaving r14 = the view's address;
  the shader-time adds.

### 8.1 After the merge with main (`c8677649`) and the override rule (3.0)

WSL Ubuntu gcc, and MSYS2 CLANG64 clang on this PC:

- `gcc -std=c11 -O2 -g -Wall -Wextra -Werror -pthread -no-pie -fsyntax-only -Id1port
  -Id1port/src -Inative/out` on `probe.c`, `d1_frame_time.c` and `runtime_kernel.c`, plain and
  with `-DD1_CORE_RUNTIME -DD1_DISCOVERY`: clean `[O]`.
- Self-test, 0 failures in all 26 runs: 13 runs (both off, units, interp, both; `--alt` with
  units on and interp on or off; each of director, cloth, fades, ui, `cloth,ui`,
  `director,fades`; an unknown name) with the override byte `00`, and the same 13 with
  `--override-on` (byte `01`), where director and fades must behave as with the switch off.
  Site counts printed match section 3.0's table in every run; with `--override-on` the skip
  notice names exactly the skipped fixes, and `director` or `fades` alone gives `on (none
  applicable)` and `0 of 0` `[O]`.
- `bash native/build-headless.sh`: `out/d1-probe` built `[O]`.
- `native/walk.sh` (builds `--discovery`), every run `exit=20` at `sceVideoOutOpen` `[O]`:
  - no guest patches (the byte as shipped, `01`), switches off: line 2 reads `... fixed-clock
    override byte 0x18c4aa4: override on: one tick per frame`;
  - the same, `D1_FRAME_UNITS=1 D1_INTERP=1`: director and fades named as skipped, `11 of 11`;
  - with the clock manifest (`guest_patches.py guest/CUSA00219-native-clock.xml`, compiled
    against the main checkout's `native/out/eboot.elf`, read only; `--patches` to the probe;
    `Patches: 2 writes, 3 bytes applied`), `D1_FRAME_UNITS=1 D1_INTERP=1`: `override off:
    wall-time ticks`, `14 of 14`; `D1_FRAME_UNITS=director,fades`: `6 of 6`; switches off: the
    off line with `override off`.
- CLANG64: `bash native/build-headless.sh` (`out/d1-probe.exe`) and `bash
  native/build-headless.sh --discovery` (`out/d1-discovery-probe.exe`) under `-Werror`;
  `d1_frame_time.c` compiled alone with the Windows flags, plain and with the discovery
  defines: clean `[O]`. Not run on Windows: the operator was playing on this PC.

### 8.2 After the second review (section 10.1)

Branch `frame-rate-phase1-fix`; WSL Ubuntu gcc and MSYS2 CLANG64 clang on this PC. The walks
used copies of the main checkout's `native/out` inputs (`boot-linked.bin`, `content.bin`,
`destiny_import_names.inc`, `runtime_aliases.inc`, `eboot.elf`).

- `gcc -std=c11 -O2 -g -Wall -Wextra -Werror -pthread -no-pie -fsyntax-only -Id1port
  -Id1port/src -Inative/out` on `probe.c`, `d1_frame_time.c` and `runtime_kernel.c`, plain and
  with `-DD1_CORE_RUNTIME -DD1_DISCOVERY`: clean `[O]`.
- Self-test, 0 failures in all 30 runs: the 13 of 8.1 plus `--no-caller` with `D1_INTERP=1`
  and units on or off, with the byte `00`, and the same 15 with `--override-on` `[O]`. Site
  counts match 3.0's table (`D1_INTERP` alone 8 / 6, with units 15 / 10; `--no-caller` 7 of 8
  and 14 of 15, the call wrap left out with its notice). New checks:
  - item 1: with `--override-on` and `D1_INTERP=1`, the director site and its call wrap are
    not hooked, the main view is always the live pose, the shader time gets no add, and a
    stalled director keeps the engine's dt;
  - item 2: one on-time build, one late build (F0 and a tick, then the build, then the
    director) holding the newest pose (64.0, where the old rule gave 63.1, a step back from
    63.5), the zero-tick build after it blending the late run (64.6), on time again, and a tick
    flag with no counted tick: counts 2 / 1 / 1 through `d1_frame_time_extract_order`; 0 / 0 / 0
    without `D1_INTERP`'s gating. A mutation that drops the late rule fails the self-test
    (63.1) `[O]`;
  - item 3: a 120-degree and a 180-degree turn with a 0.25-unit step blended (the 45-degree
    rule snapped the first; the second serves the newer forward axis), a 120-degree turn with
    a 1-unit step snapped and quarantined. Mutations restoring the old turn rule or dropping
    the zero-forward guard fail the self-test `[O]`.
- `bash native/build-headless.sh`: `out/d1-probe` built `[O]`.
- `native/walk.sh` (builds `--discovery`), every run `exit=20` at `sceVideoOutOpen` (index 301)
  `[O]`:
  - no guest patches (byte `01`), switches off: `... override on: one tick per frame`;
  - the same, `D1_FRAME_UNITS=1 D1_INTERP=1`: `D1_INTERP on (alpha 1 under the override: no
    camera blend, no shader-time add, no director gating)`, the director and fades skip notice,
    the `D1_INTERP blending skipped` notice, no director site and no call wrap, `10 of 10`;
  - with the clock manifest (`Patches: 2 writes, 3 bytes applied`), `D1_FRAME_UNITS=1
    D1_INTERP=1`: `override off: wall-time ticks`, `camera director call 0x4cba8c (F11
    0x4cba50) -> trampoline` (742 bytes), no serial notice (the nine bytes at `0x74a444` hold
    in the real image), `15 of 15`; switches off: the off line with `override off`.
- CLANG64: `bash native/build-headless.sh` (`out/d1-probe.exe`) and `bash
  native/build-headless.sh --discovery` (`out/d1-discovery-probe.exe`) under `-Werror`;
  `d1_frame_time.c` alone with the Windows flags, plain and with the discovery defines: clean
  `[O]`. Not run on Windows.

## 9. Open items

1. Seat runs (section 11); nothing here has run on the seat.
2. **Integration with main: done** (merge commit on this branch, section 8.1). `probe.c` keeps
   main's Windows block whole; the install call follows `install_engine_hooks(size)` on both
   hosts.
3. First-person twin record interpolation (section 7); decide after test C.
4. fov interpolation needs a hook inside `0x8f2b00` (`0x8f2baf`) as well as `0x52be26`.
5. Whether `0x7ea1f0` and `0x7eb200` can run in the same frame (`frames` twice the presents,
   `build conflicts`).
6. The fov unit at `+0x4ac` (radians or degrees) for the snap threshold `[U]`.
7. Particles may double-integrate in catch-up groups below 60 (4.6) `[I]`: check `0x87c260`'s
   time source and, if confirmed, give it the cloth share.
8. When `0x5905b0` (the outside tick-count setter) runs `[U]`; `resyncs` shows it.
9. What bit 1 of `0x26ae454` (the gate on the tick increment, `0x74a96f`) means `[U]`.
10. The 97-site `h` audit (design 4.2); camera cuts from the visibility flag; audio and
    network time bases; flips per outer frame; design 4.9 option 2.
11. **`D1_INTERP` under the override: closed** (second review, item 1; 3.0). `G2+8` there is
    the discarded accumulator's phase `[C]`; `D1_INTERP` keeps alpha 1 and installs no director
    gating. Interpolation in the locked-60 configuration would need an alpha the game does not
    compute there (for example the phase of the presented frame against the 59.94 tick grid):
    not designed.
12. The Windows build of `d1_frame_time.c` has not run: a headless walk with the switches on
    (`d1-discovery-probe.exe`, expect `exit=20` and the same site lines), then the Windows
    seat tests in 11.2.
13. Whether the game ever writes `0x18c4aa4` after start on PS4 `[U]`: the configuration is
    read once at install.
14. The extract order (4.3.1) is measured, not known: read `extract order` on the seat. Late
    all (the build always before the director) would call for taking the snapshot at the
    director's return (the call wrap's after-handler) instead of at the build.
15. The 0.5-unit position step that lets a 90-degree turn snap (4.3) is a judgement `[I]`:
    check `snaps` against cuts and fast turns on the seat (a Sparrow at speed comes nearest).

## 10. Review and fixes (2026-10-06)

Eleven review items, six distinct (1 = 6, 2 = 7, 4 = 11). Each was checked against the code
and the eboot before changing anything.

| # | Item | Verdict | Change |
|---|---|---|---|
| 1, 6 | Snapshots keyed by a change of pose: a camera that stops keeps blending between its last two poses, a sawtooth at tick rate | Real `[C]`: the shift ran only on `memcmp(now, cur) != 0` and the blend on `prev != cur` | Snapshots keyed by the director's consumed-tick total (4.3). The reviewers proposed keying on `G+0xc` (`0x590a50-0x590a72`, verified `[C]`); refined because below 60 the count advances on the non-final frame where the director does not run, which would shift with an unchanged pose and record span 1 on the final frame. Self-test case added |
| 2, 7 | A 2-tick span blended with the one-tick alpha: T-2+2 alpha instead of T-1+alpha, and the snap thresholds per tick against a 2-tick span | Real `[C]` | t = (span - 1 + alpha) / span; thresholds scaled by span; span histogram in the report. Self-test case added. Residual (section 7): below 60 the non-final frame still shows the last snapshot, as A does, because F11 does not run on it |
| 3 | The safety net forces one director run per 21 frames when the tick stalls: the camera moves at a few hertz, slower than wall time, each forced pose blended with a frozen fraction | Real `[C]`; the tick increment's gate (`0x74a96f`, bit 1 of `0x26ae454`) verified `[C]`, its meaning `[U]` | Stall rule (4.2): the engine's paused test or three tick periods without a tick; then the director runs every frame on wall time and nothing is blended; ticks resume with a fresh seed. The code comment's "+0 paused byte" was also wrong: `G+0` is the active byte (`0x590450`), paused is `G2+0` bit 1 under it (`0x5909e0`) `[C]` |
| 4, 11 | The UI comment and record claim a spin lock at `0x94b190` | Real `[C]`: a compare-exchange copying `0x272b5c8` into `0x272b5cc`, no lock | Comment and 3.4 corrected: safety rests on the single serialized caller `0x74bb4f` |
| 5 | Cloth double-applies in catch-up groups: both frames read the group's full accepted time from the packet | Real `[C]` (`0x590d92`, `0x590e90-0x590e9a`, `0x7eaf67-0x7eaf7c`, `0x590def-0x590dff`, `0x590d61`) | Mid-function hooks giving each frame its record +0x08 share, stashed per packet by the view wrap (3.2); the manifest scoped to the fixed-clock profile with the reason in its header; `cloth dt sum` and `fallback` in the report. The record's former claim "same rate at the emulator's 30" is withdrawn |
| 8 | The blended pose was written into the shared camera globals for the whole extract build and written back unchecked | Real as a risk `[C]` (the write and the unconditional restore); a concrete concurrent reader during the window was not shown `[U]` | No global writes: blend served through the getter call `0x52be45` under a thread-local flag set by the extract wraps and cleared at `0x812e5d` and after the build (4.3); substituted only when the getter returned the live pose. Thread-local, not global, because `0x52be00` also runs on the game-graph path (`0x812d7a`) `[C]`. Deviation from the suggested fix: the fov getter `0x52be26` and `0x8f2b00` (`0x52be51`) are not wrapped, since `0x8f2b00` reads `+0x4ac` itself and a half-blended fov would be inconsistent; fov is served live (open item 4) |
| 9 | Instruments without expected values; a circular blends check; no look-rate test; no cloth number; the self-test not in the tree | Real `[R]` | Section 5 table of expected values per field; `still` (frames with a fraction but `prev == cur`) and `drift` replace the circular check; `cloth dt sum`; look-rate test in section 11; the self-test committed as `d1port/tests/test_frame_time.c` and extended |
| 10 | The tick count can be set from outside the tick (`0x5905b0` via `0x6e60c2`); an unsigned delta clamped to 10 gives a 10h director step | Real `[C]` | Resync (4.2): a delta below 0 or above 2 gives h, a new base and a camera snap, counted as `resyncs` |

Nothing was refuted outright. Two fixes differ from the reviewers' wording, with the reasons
above (items 1/6 and 8).

Also added while fixing: `D1_FRAME_UNITS` accepts a comma list (director, cloth, fades, ui) so
the four Phase 0 fixes can be tested one at a time on the seat.

### 10.1 Second review (the design's author, on `cf917a5b`)

Four items, checked against the eboot first-hand before changing anything (branch
`frame-rate-phase1-fix`).

| # | Item | Verdict | Change |
|---|---|---|---|
| 1 (medium) | `D1_INTERP` under the override byte `01` blends by a meaningless alpha: `0x5905c0` writes `G2+8` at `0x5908c4` before the override test at `0x5908cb`, and the override replaces only due and accepted, so `G2+8` is the discarded accumulator's phase. At a locked 60 the camera would show pose(T-1+t) with t drifting on the 60.00 / 59.94 beat, and shader time gets the same wrong add. Also (the first review's low finding) the director hook still installed there for `D1_INTERP` and its stall branch fed the Phase 0 wall-time dt | Real `[C]` (`0x5908c4`, `0x5908cb`, `0x590960` `xmm1 = G+8`, `0x590965` `ebx = 1`) | With the byte `01`: alpha 1 everywhere (no snapshot, no blend, the live pose served; no shader-time add); the director gating (the entry hook for `D1_INTERP` and the call wrap) not installed. A new site group `G_INTERP_DIR` carries it. The start line says so and a notice gives the reason (3.0). Self-test with `--override-on`: no director site, live pose, no add, engine dt under a stall |
| 2 (medium) | The order of the extract-path view build against F11's director within one outer frame is not established | A real gap `[C][U]`: the cross link X2~F11 suggests the order; nothing measured it | Extract order counter (4.3.1): F0's frame serial (`*0x26ae448`, incremented at `0x74a44b`, pinned) stamped with each published run and compared at each build of a final tick frame: on time, late, no tick, in the report line. The late rule (alpha keyed to the newest snapshot's own tick) behind the same counter: it engages only on late builds. Runs are now published when the director returns (a wrap of `0x4cba8c`), so a build never pairs a new total with the old pose. Self-test: 2 / 1 / 1 and the late build holding the newest pose |
| 3 (low) | The 45-degree-a-tick rotation snap fires mid-flick on high-sensitivity mice | Real `[I]`: 45 degrees a tick is 2700 degrees a second | A turn alone never snaps; over 90 degrees a tick it snaps only with a position step over 0.5 units a tick (a cut to a nearby viewpoint the 2.5-unit jump misses). The reviewer's wording, "90 degrees, and no rotation-only snap without a position jump", read with the 2.5-unit jump itself would leave the turn test dead (the jump snaps alone); the 0.5-unit step is the reading that keeps it a cut detector (open item 15). A turn near 180 degrees in one span takes the newer forward axis. Self-test cases added |
| 4 (low) | The seat test plan does not name the expected aim lag under `D1_INTERP` | Real `[R]` (design 4.10) | 11.5: up to one tick of rotation behind during fast turns, converging when the turn stops; Phase 4's late-latched view rotation is the remedy |

Nothing was refuted. Item 3's rule differs from the literal wording, for the reason in the
table.

## 11. Seat test plan (the operator's seat, on their word; never while they play)

### 11.1 Before the first run

1. The branch is integrated with main (open item 2, section 8.1); the self-test and the
   headless walks ran on the merge, and again with the second review's fixes (8.2).
2. Push the runtime files once. `box-walk.sh` sends only `d1port/` files that differ from the
   checkout's HEAD, so after a commit it sends none of these. From the integrated checkout,
   with the seat's address and key in the environment (never in a file):

   ```bash
   remote=dev/d1-native-20261005
   for f in d1port/src/d1_frame_time.c d1port/src/d1_frame_time.h d1port/src/probe.c; do
       tr -d '\r' < "$f" | ssh -i "$D1_BOX_KEY" -o BatchMode=yes "$D1_BOX" "cat > $remote/$f"
   done
   for f in d1port/src/d1_frame_time.c d1port/src/d1_frame_time.h d1port/src/probe.c; do tr -d '\r' < "$f" | sha256sum; done
   ssh -i "$D1_BOX_KEY" "$D1_BOX" "cd $remote && sha256sum d1port/src/d1_frame_time.c d1port/src/d1_frame_time.h d1port/src/probe.c"
   ```

   This assumes the seat's other `d1port/` files equal main's `[U]`; if not, push from a
   checkout where they do. `native/*.sh` (with `build-headless.sh`) is sent whole by every
   walk.

### 11.2 Invocations

Two play configurations (section 3.0), and the tests differ between them:

- **Override off** (the Linux seat): the launcher's 60 Hz manifest plus the cadence manifest
  and `guest/CUSA00219-native-clock.xml`, box-walk presentation defaults, hooks off. All tests
  below apply.
- **Override on** (Windows as first played: the 60 Hz manifest plus cadence on the stock
  vblank, locked 60): **B1 and B3 do not apply** (the director and fades fixes are skipped; a
  run with `D1_FRAME_UNITS=director` or `fades` must print the skip notice and `0 of 0`, and
  that is all it checks). B2 and B4 apply; `D1_FRAME_UNITS=1` there means cloth and ui. **D
  and C are mechanism checks only**: `D1_INTERP` keeps alpha 1 there (3.0), so nothing on
  screen may change against A (C shows B2 and B4's changes and nothing else); the switch
  line must say `alpha 1 under the override` with its notice.

The commands below are the Linux seat's. Every test adds only `D1_SEAT_ENV` words;
`D1_WALK_TIMEOUT` is the play time in seconds.

```bash
# D1_BOX and D1_BOX_KEY are exported in the shell beforehand (never written to a file).
base() { D1_ONLINE=1 D1_VPAD=1 D1_PAD=<pad> D1_WALK_TIMEOUT=900 \
  D1_GUEST_PATCHES=../apps/launcher/resources/CUSA00219-lab-manifest-T1-60hz-fov105.xml,guest/CUSA00219-native-cadence.xml,guest/CUSA00219-native-clock.xml \
  D1_SEAT_ENV="$1" bash native/box-walk.sh; }
# (The cadence manifest joined the play configuration on 2026-10-06: design section 5, "Two
# play configurations"; box-walk.sh's header lists the same three files.)

base "D1_STEP_WITNESS=1"                                  # A   baseline: witness and Frame stats only
base "D1_STEP_WITNESS=1 D1_FRAME_UNITS=director"          # B1  Phase 0, the director alone (override off only)
base "D1_STEP_WITNESS=1 D1_FRAME_UNITS=cloth"             # B2  cloth alone
base "D1_STEP_WITNESS=1 D1_FRAME_UNITS=fades"             # B3  fades alone (override off only)
base "D1_STEP_WITNESS=1 D1_FRAME_UNITS=ui"                # B4  UI clock alone
base "D1_STEP_WITNESS=1 D1_INTERP=1"                      # D   Phase 1 alone
base "D1_STEP_WITNESS=1 D1_FRAME_UNITS=1 D1_INTERP=1"     # C   both
# Rollback: test A's command (no D1_FRAME_UNITS, no D1_INTERP). The switches are read once at
# start and nothing persists; the seat needs no other change.
```

Order: A, then B1 to B4 one at a time, then D, then C. Stop at the first failure of the
universal checks below and report before going on. Each test covers three scenes: a light
view at 120+ FPS, a heavy view at 43-45 FPS (CADENCE 4b), and 30 s standing still.

### 11.3 What to read

In `native/out/windowed.log` on the seat (`grep -a -v -i authorization`; every run is also kept
under `native/out/runs/`):

1. Start: the `Frame time:` switch line naming the configuration (`override off: wall-time
   ticks` on the Linux seat; `override on: one tick per frame` under the Windows locked-60
   configuration; anything else means the patches did not apply as intended: stop) and
   exactly the selected fixes, then `N of N applicable sites installed`. Override off: A none
   (one "off" line), B1 3, B2 4, B3 5, B4 4, D 8, C 15, with no `serial increment` notice and
   no `call is not wrapped` notice. Override on: A none, B2 4, B4 4, D 6, C 10 with the skip
   notice naming director and fades and the `D1_INTERP blending skipped` notice (B1 and B3: the
   skip notice and `0 of 0`). A `left out` line means the seat's image differs at that site:
   stop and report.
2. Step witness, every window, identical to A in every test: Havok entries per second 59.9
   to 60.0, h 0.0167 (1/59.94), 1.000 simulated s per wall s. Anything else breaks the
   "simulation untouched" rule and ends the test.
3. Step gaps and Frame stats: the same distribution as A; the pacing median and p99 within
   0.5 ms of A.
4. The `Frame time [5.0 s]` line against section 5's table. Per test, the fields that must
   hold:
   - B1 (override off only): director runs = frames - continuation; dt mean = 5000 / runs ms.
   - B2: cloth dt sum about 5.0 s per window when cloth calls = frames, in the light view and
     in the 43-45 FPS view alike; fallback 0.
   - B3 (override off only): fades about 900.
   - B4: ui = frames - continuation (in the world, not paused).
   - D and C (override off): span ticks about 300; stall runs, resyncs, snaps, drift,
     mismatched, build conflicts 0 in steady play; served = blends; fp switches = shader time
     adds = frames; turning: still about 0; standing: blends about 0, nothing moves on screen;
     t mean about 0.5 at 120+ FPS; extract order on time = director runs, late 0, no tick 0
     (4.3.1: if late is non-zero, note whether it is mixed with on time or all, and whether it
     follows the heavy view; the run continues, the late rule holds the view).
   - D and C (override on): director runs 0, blends, served, shader time adds and the extract
     order 0; fp switches = frames.
5. frame_pairs_probe (`native/tools/frame_pairs_probe.py`, as in CADENCE 3.1), in D and C,
   turning at 120+ FPS: the world regions change on every pair (presents per second), no held
   pair followed by a doubled step; the gun region still about 60 changes a second (Phase 2).
   Standing still after a turn: no region changes (finding 1's sawtooth would show here).

### 11.4 Look-rate test (every test, light view, 120+ FPS)

Full right-stick deflection from a fixed facing, time three 360-degree turns (video or
stopwatch, take the mean); repeat with a fixed mouse sweep if the mouse is in use. Expected:
the same turn time in A, B1 (override off only), D and C within 5%. A difference in B1 means the director scales
look input by its dt (design 8); a difference in D or C also means look input is dropped or
scaled on skipped director frames. Either is a finding: note the times.

### 11.5 What the operator should see

- **B1** (override off only): camera transitions (aim-down-sights zoom, respawn swing, orbit drift) take the same
  wall time at 160 FPS as at 60; nothing else changes.
- **B2:** cloaks and banners calmer at 60 FPS (half today's speed) and at the same speed at 45
  and 160.
- **B3** (override off only): at 160 FPS fades and effect ages (loading fade, damage vignette) at their 60 FPS
  speed.
- **B4:** UI timers (respawn, match clock) agree with a stopwatch over 60 s.
- **D and C** (override off): at 120+ FPS, turning, the world is smooth with no doubled or
  missing steps, a fast mouse flick included (a turn alone never snaps, 4.3); standing still
  after a turn nothing moves; the gun stays fixed on screen (if it swims against the world,
  note how much: Phase 2); characters and effects still step at tick rate (Phase 2);
  teleports, respawns, third-person camera switches and cutscene cuts show no smear; zoom
  transitions step (fov is live, section 7). At 43-45 FPS, C should not judder more than A.
- **Aim lag under `D1_INTERP` (expected, not a failure).** The view shows the camera
  interpolated between the last two ticks, so during a fast turn the view's rotation is up
  to one tick behind the newest tick's (at 59.94 ticks a second, up to about 16.7 ms of
  rotation: at 1000 degrees a second, up to about 17 degrees), on top of today's latency. It
  converges when the turn stops: the view settles on the newest tick's facing within one tick
  and nothing moves after that. Design 4.10: free-run shows the world one tick behind; an
  exact lock adds nothing. Report whether it is noticeable in fast turns and in flick shots,
  against A at the same FPS. If it is, the remedy is Phase 4, the late-latched view rotation
  (the yaw and pitch from input since the last tick added at view build), not dropping
  interpolation; the first answers before Phase 4 are design 4.10's (an exact lock where the
  display allows, a late input sample, the anti-lag layers).
- **D and C** (override on): nothing on screen differs from A (C: only B2 and B4's changes);
  alpha is 1 there (3.0), so there is no aim lag from `D1_INTERP` either.
- **Rollback** if any of these fails: test A's command.

### 11.6 Seat results (2026-10-06 10:13-10:41Z, `d7bc1797`, instruments only)

The tree was `frame-rate-phase1` at `d7bc1797` (`da7b1e73` plus main), shipped to the seat
whole, with a fresh shader cache (a first boot, W, filled it and is not a test). Every test ran
the play configuration: the 60 Hz manifest, the cadence manifest and the clock manifest,
uncapped, 480 Hz immediate. Each added `D1_STEP_WITNESS=1` and parked the camera for the
frame-pair probe. Engine draw hooks were off. The route was `tools/seat-to-world.sh`, recalibrated
the same morning for uncapped menus (`e442d4c0`) [O]. An earlier run of these tests (09:59Z) is
void: the old route launched the Speaker cinematic at 340 menu FPS.

| test | where | FPS | witness (entries/s, sim s per wall s) | counters per 5 s window |
|---|---|---|---|---|
| A | Restoration, heavy view, 13k draws | 46-50 | 59.6-59.9, 0.993-0.999 | none (switches off); pacing median 20 ms, p99 29 ms |
| B1 `director` | Restoration | 51 | 59.8-60.0, 0.997-1.001 | director runs 239, dt mean 21.1 ms |
| B2 `cloth` | Restoration | 48 | 59.8, 0.996 | cloth calls 241-261 |
| B3 `fades` | Restoration | 48 | 59.9, 0.998 | fade calls 900-906 |
| B4 `ui` | off route (a 465-draw menu view) | 570 | 60.0, 1.000 | ui calls 2,827-2,844 |
| D `D1_INTERP` | Restoration, below the tick rate | 47-49 | 59.1-60.2, 0.986-1.003 | director runs 234-246 (span 1/2: 170-189 / 53-64, ticks 295-300); snapshots 234-243, snaps 1, blends 222-245, t mean 0.61-0.66, drift 0, stall 0, mismatched 0, build conflicts 0; extract order on time 234-243, late 0, held 0, no tick 0 |
| C all | off route (the 465-draw view) | 546-550 | 60.0, 1.000 | director runs 283-300 (ticks 300), skips about 2,450, blends about 2,750, t mean 0.50-0.52, snaps 0, drift 0, mismatched 0; extract order on time 283-300, late 0, no tick 0 |

No test faulted [O]. Reading:

- The simulation held its rate in every combination. The witness stayed within 1.5% of 60 Havok
  entries a second at 0.99-1.00 simulated seconds per wall second [O].
- Extract order (4.3.1): every final-tick build landed on time, both at 48 FPS (D) and at 550 FPS
  (C). The late rule and its hold never fired, so snapshotting at the director's return is not
  needed on this seat [O].
- Rotation snap (4.3): the 0.5-unit bound produced at most one snap per 5 s window, consistent
  with a camera cut, and none at 550 FPS [O].
- `ui` changes menu cursor travel at high frame rates (B4, C) [O]. With the UI millisecond carry
  on, the uncapped route's pushes no longer land on the Director targets. This is the expected
  effect of turning a per-frame unit into a per-time one [I]. A route for runs with `ui` on needs
  its own calibration, or the capped push lengths [I].
- D in Restoration ran below the tick rate, so it exercised the span-2 path more than the blend.
  The blend at high frame rates was exercised by C (t mean 0.5, uniform, as expected) but in a
  menu view [O]. A light-view in-world D run above 60 FPS is still to do [U].
- What the operator sees (11.5) is not recorded here: these were unattended runs [U].
