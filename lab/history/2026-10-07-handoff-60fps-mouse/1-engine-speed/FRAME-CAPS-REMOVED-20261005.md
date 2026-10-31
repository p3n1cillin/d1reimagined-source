# The four caps: how the native client went from 43 presents a second to frame rate = work

2026-10-05, evening. Record of what was found and changed in the native Destiny 1 client
(bbport runtime + bbport's shadPS4-derived renderer) that took Restoration from 43–48 presents
a second moving and a flat 30.0 standing, with the game's speed tied to the frame rate, to
**frame rate = draws × per-draw cost with the simulation at exactly 60 ticks a second**:
156–166 FPS in a light view, 80–84 in a busy one, ~55 on the full open vista, all with
`1.000 simulated seconds per wall second`. None of it reduced the cost of a draw. All of it
removed things that stood between the renderer's work and the screen. Tags: `[O]` observed on
the seat, `[R]` read in code or a record, `[I]` inference, `[U]` unverified.

Read this before touching presentation, the clock, or the guest-patch set. The reproduction
is in section 6.

## 1. Where the frame went before tonight

The GPU command thread did ~1.7 µs of work per draw, 10,500 draws a frame in the Restoration
vista: 17–18 ms. That work is unchanged. What the player saw was 43–48 presents a second
moving (frames alternating 16.7 and 33.3 ms, median 33) and exactly 30.0 standing, and the
game ran slower whenever the frame rate dropped. Four independent mechanisms produced that,
each rounding the frame up or stretching it; each had to be found separately, and each alone
was enough to hide the next.

| # | Cap | Where | Effect on its own |
|---|---|---|---|
| 1 | The title's 30 Hz flip-rate request, honoured | bbport VideoOut | presents only on every other 60 Hz tick |
| 2 | The 60 Hz virtual vblank | bbport present thread | a 17–18 ms frame waited for the next tick: 33 ms |
| 3 | The game's outer-loop deadline | the eboot, `0x593310` | the next frame not started before 16,683 µs |
| 4 | The fixed-clock override | the eboot, `0x5905c0` | one simulation tick per rendered frame; game speed = frame rate ÷ tick rate |

## 2. Cap 1: the flip rate `[R][O]`

bbport's present thread takes a flip request only when `vblank_status.count % (flip_rate + 1)
== 0` (`gpu/shadps4/core/libraries/videoout/driver.cpp:746`), and `sceVideoOutSetFlipRate`
stored the title's request verbatim (`video_out.cpp:141-145`). Destiny asks for rate 1 (30 Hz)
early and 0 (60 Hz) later: the new log line shows it, `VideoOut: title requested flip rate 1
(30 Hz), stored 0` then `requested flip rate 0 (60 Hz), stored 0` `[O]`. While the request was
1, the client could not present more than 30 times a second whatever the frame cost; the
standing client sat at exactly 30.0 (150 flips in every 5 s window) `[O]`. The same line also
disabled bbport's immediate-flip path (`driver.cpp:789`: `if (!immediate_flips ||
main_port.flip_rate != 0) continue;`), which is why the earlier `BB_FPS_LIMIT=60` test showed
nothing.

The emulator fork had met this already: its play profile runs `SHADPS4_D1_FLIP_RATE_60_PROBE=1`,
whose `SelectStoredRate` rewrites a requested 1 to 0 (`D:/dev/shadPS4/src/core/d1_flip_rate_probe.cpp:45-59`).
The native client had no equivalent.

**Change:** `sceVideoOutSetFlipRate` stores 0 for a requested 1 unless `D1_FLIP_RATE=stock`,
and prints every request (`video_out.cpp`). Patch group: to be split out as its own group.

## 3. Cap 2: the 60 Hz virtual vblank `[R][O]`

With `flip_rate` 0 the present thread still flipped only on vblank ticks, at `BB_VBLANK_HZ`
= 60 by default. A frame that took 17–18 ms missed the 16.7 ms tick and waited for the next:
33 ms. Frames that happened to finish inside a tick presented at 16.7 ms, which is why the
average read 43–48 and the median interval was 33 ms `[O]`. bbport's own "uncapped" preset is
a 480 Hz virtual vblank with immediate presents (`gpu/shim/core/emulator_settings.h:32-52`);
the first test with today's binaries and no code change, `BB_VBLANK_HZ=480 BB_FPS_LIMIT=1000`,
went from 43–48 moving / 30.0 standing to **56–58 moving / 60.0 standing** `[O]`. The 60.0
standing was the next cap.

One trap, from GPT's review and confirmed in the code: `BB_VBLANK_HZ=0` selects 480 Hz but
`GetFrameLimit()` then defaults to the display refresh (`min(refresh, 120)`), and
`BB_FPS_LIMIT=0` disables the between-vblank branch entirely (`driver.cpp:730`). The setting
that works is the explicit pair `BB_VBLANK_HZ=480 BB_FPS_LIMIT=1000`.

**Change:** `native/box-walk.sh` launches with `BB_VBLANK_HZ=${D1_VBLANK_HZ:-480}
BB_FPS_LIMIT=${D1_FPS_LIMIT:-1000}` (comment there records the measurement). Numbers before
and after this change are not comparable.

## 4. Cap 3: the outer-loop deadline `[R][O]`

The game's outer loop (`0x593310`) computes `now_us` from the counter, takes the previous
frame's start from `TS+0x10`, adds the configured deadline (`0x5938eb: add rcx,[CFG+0x08]`,
33,366 µs stock; the fork's one-byte wake patch points it at `CFG+0x28` = 16,683 µs), and at
`0x59393a cmp r12, 13` / `0x59393e jg 0x593990` sleeps until the deadline before building the
next frame `[R]` (disassembly of `native/out/eboot.elf`; the bytes `49 83 fc 0d 7f 50`). That
is why a standing client, with nothing else in the way, pinned at exactly 60.0: 59.94 frames a
second is the deadline. The Xbox One compatibility layer's `D1_UNCAPPED` does one thing:
"removes only the deadline restriction in the frame scheduler" (`kx_game_wait.h`, saved under
`artifacts/renderer-research-20261005/xbox/`), keeping the job-graph completion checks and the
dependency/event waits that follow.

**Change:** `native/guest/CUSA00219-native-clock.xml`, entry "outer deadline off": the `jg`
becomes two nops (`Address="0x99393E" Value="9090" Original="7F50"`). The job checks at
`0x593940` onward still run, so the loop starts the next frame as soon as its jobs allow and
no sooner. Only correct together with cap 4's change: with the override on, more frames mean
a faster game.

## 5. Cap 4: the fixed-clock override `[R][O]`, the one nobody had

### 5.1 What the engine does with time

The chain is `0x59395a call 0x592820(now_us)` (the outer builder) → `0x59292f call 0x590bf0`
(the frame clock) → `0x590dc9 call 0x5905c0(elapsed_s, &accepted, &due, &native_dt,
&native_ticks)` (the host-delta routine); every link has exactly one caller `[R]`. Inside
`0x5905c0` the engine already owns a real elapsed-time accumulator:

- `ticks = trunc(scale · elapsed_s · (1e6 / CFG+0x08) + G2+8)` where `G2+8` is the kept
  fractional tick (`0x590873-0x590896`); the tick **period is CFG+0x08**, 33,366 µs stock
  (29.97 Hz), 16,683 µs under the "60 Hz" constants (59.94 Hz);
- an allowance of ticks a frame, `lrint(clamp(2·scale, 2, 10))` at normal speed, i.e. 2
  (`EBX` at `0x590760-0x5908c4`); excess wall time is discarded when clamped;
- `due = 0` is a legal result: the frame renders without a simulation tick (F1's non-tick path
  at `0x74a8e9`); the fork's A6 run had already exercised that path in the world without faults;
- `due = N > 1` runs one tick now and stores `N−1` as pending (`rec+0x40`); the outer loop then
  bypasses the deadline (`0x5934fc`) and builds the next frames immediately as continuation
  frames, one tick each, with no time measured (`0x590d36`). The engine never runs more than
  one tick per outer frame; catch-up is extra frames. `CFG+0x40` (33,333 µs, "nominal
  catch-up") is the duration credited to a continuation tick, not the allowance.

### 5.2 The override

Immediately after the accumulator, at `0x5908cb`:

```
5908c4 c4c17a11442408   vmovss [r12+8], xmm0     ; G2+8 = new fraction (always written)
5908cb 488d05d2413301   lea rax, [0x18c4aa4]     ; THE FLAG
5908d2 803800           cmp byte [rax], 0
5908d5 4489f3           mov ebx, r14d            ; due      = native ticks
5908d8 c5f828ca         vmovaps xmm1, xmm2       ; accepted = native accepted seconds
5908dc 0f8488000000     je 0x59096a              ; flag == 0 -> keep the native result
5908e2 ...              ; flag != 0 and normal speed and not suspended and elapsed > 0:
590960 c5fa104908       vmovss xmm1, [G+8]       ;   accepted = the fixed step
590965 bb01000000       mov ebx, 1               ;   due = 1: one tick per rendered frame
```

One initialised data byte, `0x18c4aa4`, default `01`, with exactly one reader in the whole
image and no writer `[R]` (a disp32 scan of the text segment found the single reference at
`0x5908cb`; no 4- or 8-byte absolute value of the address exists in the file). While it is set,
the engine throws its own answer away and substitutes "one fixed tick per frame": the game's
speed becomes frame rate ÷ tick rate. The step witness measured exactly that before the change:
`Step witness: 39.4 Havok entries/s, h 0.01667, 0.656 simulated s per wall s` at 39.4 presents
`[O]`, and the fork's A5 run (outer 36/s with `h` = 1/30 → fixed publications 36/s, the game 20%
fast) is the same mechanism from the other side.

This is the same structure as the Xbox One build, where the compatibility layer disables the
override with a startup data write and leaves the native accumulator in charge
(`kx_game_wait.h`: "At normal speed, this build replaces the measured delta with 1/30 second
and forces one game tick per rendered frame... Disable only that override; the preceding
native elapsed-time accumulator still emits fixed simulation ticks at the configured rate,
including frames with no simulation tick"). The Xbox signature (`movss [rsi+8],xmm7; cmp byte
[flag],0; ...; je`) is the same block compiled by a different compiler.

### 5.3 Why the emulator work never found it

The fork's clock research mapped the outer builder, the three constants, the deadline read,
the Havok wrapper and the fixed publisher, and measured that the fixed domain followed the
outer rate (A5). It then built an external wall-credit gate at the `now_us` call that hands
the engine an unchanged time to produce zero-tick frames, a re-implementation, outside the
engine, of what clearing the byte does inside it, and it could only hold ticks back, never
add them, so it capped frames at the display rate. The override byte and its reader
(`0x18c4aa4`, `0x5905c0`, `0x5908cb`) appear in no fork document and no project document before
tonight. The step witness (Havok wrapper `0xcace80`, `h` in XMM0, hooked by the loader) is what
made the question answerable in one run instead of by argument.

### 5.4 The change and what it does

`native/guest/CUSA00219-native-clock.xml`, entry "native elapsed-time ticks": `Address=
"0x1CC4AA4" Value="00" Original="01"` (manifest addresses are eboot vaddr + 0x400000; the
compiler checks `Original` and the file-backed segment). With it, under the 60 Hz constants:

- at any frame rate ≥ 60: 59.94 ticks a second, frames above that are zero-tick frames
  (render only), `Σh` = 0.999 simulated seconds per wall second;
- below 60: the allowance lets a late frame carry a second tick as an immediate continuation
  frame, which is a full frame; when the outer loop cannot keep up, time is dropped (a
  graceful slow-down, not a spiral);
- no interpolation: above the tick rate the world moves every one or two frames;
- `[I]` an integer-millisecond UI clock may drift at high rates (the Xbox layer needed a
  separate UI-clock fix); not observed yet.

Measured `[O]`: `Step witness: 59.9–60.1 Havok entries/s, h 0.01667, 0.998–1.002 simulated s
per wall s` in every window, in orbit at 280 presents a second and in Restoration at 156–166
(light view, 3.4–3.8k draws) and 80–84 (busy view, 6.5–6.8k draws), GPU command thread
1.74–1.90 µs a draw at 0% idle. Picture clean (`native/out/shots/clock-world.png`).

## 6. Reproduction

Walk with the 60 Hz set plus the clock manifest and the presentation defaults:

```bash
D1_BOX=<seat login> D1_BOX_KEY=<key path> D1_ONLINE=1 D1_VPAD=1 D1_PAD=18d1:9400 \
D1_GUEST_PATCHES=../apps/launcher/resources/CUSA00219-lab-manifest-T1-60hz-fov105.xml,guest/CUSA00219-native-cadence.xml,guest/CUSA00219-native-clock.xml \
D1_GUEST_SKIP="field of view" bash native/box-walk.sh
```

(`box-walk.sh` sets `BB_VBLANK_HZ=480 BB_FPS_LIMIT=1000`; the flip-rate lift is in the renderer.)

**Play mode versus measurement mode (added the same night, after playing it).** Uncapped
frames over a fixed 60 Hz simulation with no render interpolation is the known anti-pattern
(Gaffer's "Fix Your Timestep": frames between ticks show the previous world state with a
random phase; on a 60 Hz display it reads as missing and doubled steps, and it does not improve
at higher frame rates). This engine has the fixed-step accumulator (tonight) and **no render
interpolation**. Until that exists, the play configuration on any display is: the override byte
off (correct game speed) **and the game's own deadline on**, one tick per frame at 59.94,
paced: leave the "outer deadline off" entry out with `D1_GUEST_SKIP="deadline off"` (the exact
text: `"deadline"` alone also removes the 59.94 wake entry, whose name contains the word).
"Deadline off" is a measurement mode and the mode for high-refresh displays once interpolation
is built (the accumulator's fractional tick is the blend factor).

Two further facts from the same evening's play, recorded so nobody repeats the detour:

- **The display can lie.** With the box presenting a steady 60 (pacing median 16.67 ms, 0.3 ms
  jitter, bbport's corner counter at 60, witness 1.000) the seat's TV, a Vizio P65-F1 on
  HDMI, 1920×1080@60, showed what the operator read as 10–15 frames a second, "not every
  frame", with some elements smooth and others stepping; a Moonlight stream of the same desktop
  looked right. That is the TV's own motion processing outside its Game picture mode (motion
  interpolation and film-cadence detection), not the box. Before any frame-feel verdict: Game
  mode, low-latency on, motion processing off, or judge on a stream or a monitor.
- **Override off with the deadline on is suspect by construction** `[I]`: the outer period
  (16,683 µs) then equals the tick period (CFG+0x08), so the native accumulator sits on its
  rounding boundary every frame and ±0.3 ms of jitter can yield 0- or 2-tick frames instead
  of 1; the witness's 5 s average cannot see it. For paced play the emulator's configuration -
  **deadline on, override on** (exactly one fixed tick per frame), with tonight's presentation
  fixes is the safe choice; the override comes off only with free-running render and
  interpolation. A per-frame histogram of `due` (a hook at `0x5905c0`'s return) would settle it.
Read, from `native/out/windowed.log` on the seat (filter `grep -a -v -i authorization`):
`VideoOut: vblank 480 Hz, frame limit 1000 FPS`; `Patches: 13 writes`; `VideoOut: title
requested flip rate 1 (30 Hz), stored 0`; `Step witness: ...` (60 entries/s, 1.000); `Frame
stats: ... draws/frame`. To isolate one cap: `D1_GUEST_SKIP="deadline off"` or `"override"`,
`D1_VBLANK_HZ=60 D1_FPS_LIMIT=0`, `D1_SEAT_ENV="D1_FLIP_RATE=stock"`.

Caveats for measurement: the virtual-pad route (`tools/seat-to-world.sh`) was timed for 45–60
frames a second and failed to reach Restoration once at 280; drive in by hand or retime it.
`tools/seat-watch.sh` and `box-shot.sh` capture through the swap thread: not during a timing
window.

## 7. What this did not change, and what is next

The cost of a draw is unchanged: 1.7–1.9 µs on the GPU command thread (decode, register
apply, constant-engine bookkeeping, specialisation, resource walk), ~1.4 µs on the recording
thread (stage B, now 80–83% busy and the next limiter). Frame rate is now simply the draw
count of the view divided into that: the full Restoration vista at 10.5k draws stays ~55
moving. Everything that remains is per-draw work, in the order of PLAN-100FPS section 4a:
engine-fed resource inputs (step 3–4, round 3 in progress), the constant-engine drains
(17.6% of the command thread waiting on stage B), the recorder's copies and per-step
allocations, decode skip for covered draws, runs, memory in place. Each of those now shows on
screen directly instead of being rounded away by a tick.

## 8. Credits and sources

- The Xbox One compatibility layer (WinDurango-based; `kx_game_wait.h`, `kx_simulation.h`) for
  the override's shape and the "disable only the override" rule.
- The emulator fork's clock records (`documents/d1-outer-wake-5994.md`,
  `documents/Debugging/Destiny1ClockPipeline.md`) for the outer builder, the constants, the
  deadline read and the Havok sites; its `d1_flip_rate_probe` for cap 1.
- bbport for the uncapped preset and the two-stage renderer that makes the uncapped frame
  worth having.
- GPT's adversarial review of the state-model handoff for insisting the gate could not add
  ticks and pointing at the Xbox override as the thing to search for.
- The disassembly of the PS4 time control (`0x5905c0`, `0x590bf0`, `0x592820`, `0x593310`) was
  done tonight on `native/out/eboot.elf` with capstone; the scratch scripts live in the
  session's scratch directory, not in the repository, the addresses and bytes above are the
  record, and `python -c "import capstone"` plus the segment table in section 4 of
  RESEARCH-HANDOFF-STATE-MODEL-20261005.md are enough to re-derive them.

## 9. Correction, 2026-10-06: the engine draw hooks were on in every run above

Every frame-rate and feel verdict in this document, and every run of 2026-10-05, was taken with
the loader's engine draw hooks installed (patch group 18: the draw wrappers, the packet emitters,
the state preparation and the descriptor setter, three to four trampoline calls per draw on the
game's job threads). They were on by default. On 2026-10-06, an A/B with the same build and the
same manifest in the world: hooks on, 45.5 FPS at a 21.5 ms median frame with 179 of 228 ticks in
the 20-28 ms band (the game at three quarters speed, stepping); hooks off, 59.8 FPS at 16.5 ms
with the picture advancing every frame. The hooks cost the game about 5 ms a frame. They are
opt-in now (`D1_ENGINE_HOOKS=1`; the step witness alone with `D1_STEP_WITNESS=1`). The numbers
above (156-166 light, 80-84 busy, ~55 full vista, and the play-mode judder) must be re-measured
without them before they are quoted again; the uncapped ones can only go up. The full record of
the night that found this is CADENCE-20261006.md.

Play configuration after the correction (operator verdict 2026-10-06, "feels better now, even
when it goes under 60"): the launcher's 60 Hz manifest plus `guest/CUSA00219-native-cadence.xml`
(the deadline byte; not a no-op, see CADENCE section 4) plus `guest/CUSA00219-native-clock.xml`
(override off, deadline off), the box-walk presentation defaults, no hooks. On Windows the first
played configuration (2026-10-06 08:52 UTC, "feels right") is the 60 Hz manifest plus the cadence
manifest on the stock 60 Hz vblank: a locked 60 at the right speed. The "play mode =
override on + deadline on" paragraphs earlier in this document were written with the hooks
installed and are superseded; the capped configuration runs the game at frame rate divided by 60
in heavy views, the uncapped one keeps the game's speed exact and carries two ticks on a slow
frame.
