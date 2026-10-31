# Destiny 1 native client: 60+ FPS and real mouse, handoff (2026-10-07)

Target: CUSA00219 patch 01.29. All addresses are eboot virtual addresses unless a manifest says
otherwise (manifests use vaddr + 0x400000). Evidence tags in the records: `[C]` code or
disassembly read, `[O]` observed live, `[R]` read in a record, `[I]` inferred, `[U]` unknown.

Every byte change in here is a client modification. Findings that depend on one are not
evidence of how the shipped game behaves.

There are two halves to "60 and above", and they are separate problems:

1. **Letting the engine run faster at all** (part 1). Four caps stood between the renderer and
   the screen, and one of them tied game speed to frame rate. With them removed, frame rate =
   draws x per-draw cost, and the simulation stays at 59.94 ticks a second at any frame rate.
2. **Making frames above the tick rate worth having** (part 2). The engine has no render
   interpolation, so above 60 the world, gun and characters step at tick rate. The design is
   fixed 60 Hz authority plus interpolation at the engine's render extract, not D2-style
   delta time.

## Reading order

1. `1-engine-speed/FRAME-CAPS-REMOVED-20261005.md`: the four caps, with bytes and measurements.
   Read section 9 first: the frame-rate numbers in sections 1-7 were taken with debug draw
   hooks that cost 5 ms a frame. They must be re-measured.
2. `1-engine-speed/CADENCE-20261006.md`: the night that found the hook cost, the play
   configurations, and why the cadence byte is required.
3. `2-frame-rate-independence/FRAME-RATE-INDEPENDENCE-20261006.md`: the interpolation design,
   four clock bugs in today's configuration, phases 0-4.
4. `2-frame-rate-independence/FRAME-RATE-PHASE1-IMPL-20261006.md`: phases 0 and 1 as built
   (`source/d1_frame_time.c`).
5. `1-engine-speed/PLAN-100FPS-20261005.md`: where the frame goes now (per-draw cost) and the
   plan to 100+. Section 7.1 records what was tried and closed.
6. `5-mouse/`: see the mouse section below.

## Part 1: what made the engine able to go faster

| # | Cap | Site | Fix |
|---|---|---|---|
| 0 | Engine time control at 30 Hz | constants at `0x5918b3/c7/d1` | `3-guest-manifests/CUSA00219-60hz-time-control-EXCERPT.xml`: tick period 16,683 us (59.94 Hz), rate 60, catch-up 16,666 us |
| 1 | Title asks VideoOut for a 30 Hz flip rate | renderer VideoOut | store 0 for a requested 1 (`D1_FLIP_RATE=stock` restores it) |
| 2 | 60 Hz virtual vblank: a 17 ms frame waits to 33 ms | renderer present thread | `BB_VBLANK_HZ=480 BB_FPS_LIMIT=1000` (the explicit pair; 0 values do something else) |
| 3 | Outer-loop deadline: no frame before 16,683 us | `0x59393e jg` | `CUSA00219-native-clock.xml` "outer deadline off" (two nops) |
| 4 | Fixed-clock override: one tick per rendered frame, game speed = FPS / 60 | data byte `0x18c4aa4`, read at `0x5908cb` in `0x5905c0` | `CUSA00219-native-clock.xml` "override off" (byte to 0) |

Cap 4 is the key finding. The engine already has a real elapsed-time accumulator in
`0x5905c0`. It allows zero-tick frames, and extra ticks run as continuation frames. One
initialised byte with no writer throws that result away. The Xbox One compatibility layer
clears the same flag at start. The fork's clock research (part 4) never found it and built an
external wall-credit gate instead. That gate could only hold ticks back, so it capped frames
at the display rate.

Also required: the cadence byte `0x5938eb` (`CUSA00219-native-cadence.xml`). It points the
outer deadline read at CFG+0x28 (16,683 us). On Windows under the stock 60 Hz vblank,
leaving it out gave a locked 60 presents with the game at about half speed.

**Play configurations as of 2026-10-06:**
- Linux, uncapped: 60 Hz constants + cadence + clock manifest, 480 Hz virtual vblank, immediate
  presents, engine hooks off.
- Windows, locked 60: 60 Hz constants + cadence on the stock 60 Hz vblank. With the override
  on here, only the cloth and UI-clock bugs of part 2 apply.

**Where the frame goes now:** the renderer's GPU command thread, about 1.5-1.9 us a draw at
10,000+ draws a frame. Without hooks, the 10.7k-draw vista runs about 60 FPS, a 13.4k-draw
view runs 43-45, and light views ran 150+ (that last figure was taken with hooks, so
re-measure it). `PLAN-100FPS` is the plan, and its section
7.1 records two dead ends:
- engine-fed resource records at the draw-emit boundary: correct, but not a win;
- bbport's draw-preparation workers: 0.6% hit rate.

`DRAW-COST-PREP`, `SPEC-STEPS-6-10` and `RENDERER-FLOW-AND-COMMAND-PROCESSOR` are the per-draw
work. `PLAN-TO-60-REV2` holds the earlier measurements it stands on.

**Instruments:**
- **Step witness** (`D1_STEP_WITNESS=1`, in `d1port/src/probe.c`): Havok entries a second and
  simulated seconds per wall second. These must read 59.94 and 1.000.
- **Step-gap histogram:** printed by the same witness.
- **Frame-pair probe:** `source/frame_pairs_probe.py`.
- **Draw hooks** (`D1_ENGINE_HOOKS=1`): leave these off unless you are measuring them. They
  cost 5 ms a frame.

## Part 2: frame-rate independence (render above the tick rate)

- **Phase 0** fixes four unit bugs:
  - the camera director steps `h` every frame (`0x4cba6b`);
  - cloth uses a constant 1/30 s per frame (`0x7f0905`/`0x7f090f`);
  - F10 fades and ages get elapsed time only on tick frames;
  - the UI clock truncates milliseconds (`0x94b1d1`).
- **Phase 1** interpolates the camera, the first-person weapon and shader time, using the
  kept tick fraction (G2+8 in `0x5905c0`). Nothing in this engine family reads that fraction
  today.
- **Built** in `source/d1_frame_time.c` (on main), behind `D1_FRAME_UNITS=1` and `D1_INTERP=1`,
  both off by default. The implementation record has the seat test plan.
- **Open:** phases 2-4:
  - 2: objects at the extract (`0x894550`; rigid at `0x1095840`/`0x10acff0`);
  - 3: display-clock pacing at `0x59395a`;
  - 4: late-latched aim.
- **The tick stays at 59.94 for every client** (multiplayer needs identical `h` and ticks per
  wall second). Do not propose variable `h` without re-reading section 3 of the design.

`CUSA00219-frame-units.xml` is the cloth fix as bytes, but only for the override-on
configuration. Its header explains why not to combine it with the clock manifest.

## Part 3: manifests and loader

`3-guest-manifests/guest_patches.py` applies the XML manifests. It checks each `Original`
before writing. Address = eboot vaddr + 0x400000.

## Part 4: the fork's clock records (background)

`Destiny1ClockPipeline.md` and `d1-outer-wake-5994.md` from the shadPS4 fork map the outer
builder, the three constants, the deadline read and the Havok sites. Their conclusions are
superseded on two points:
- **The 60 Hz constants are "only a control arm".** That was true with the override on. With
  the override off, the engine's own accumulator gives correct speed (part 1).
- **A wall-credit gate is needed.** The override byte replaces it.

The four-clocks vocabulary and the per-rate tables are still useful.

`PS4-ENGINE-FRONTEND-20260912.md` is the static map of the engine's render layer, with Xbox
D3D11 counterparts. `ROADMAP-20260912-60-120.md` is the emulator-side route to 60/120.

## Part 5: real mouse

The native recomp has no mouse path yet. The working implementation is in the shadPS4 fork
(`5-mouse/source/d1_native_mouse.cpp`, with its headers), and it is what a native port should
reproduce.

It drives the engine's own keyboard-and-mouse layer, which survived on PS4 with the Xbox
layout. Only the platform producer that fills it was missing:

| Site | What the fork does there |
|---|---|
| `0x924CC5` (KBM builder entry, under the native input lock) | writes the full mouse sample and 104 keys into the engine's state at `0x1ABC3E0` and sets the mouse-enable byte `0x1ABBC53` while captured |
| `0x91AB3C` / `0x91B7A7` | "hybrid look" for player 0: pad actions preserved, stick look live while the mouse is idle, pitch corrected; the look record is restored after |
| `0x91BB3E` | aim-assist bypass: zeroes the two assist strengths before the game computes its assist flag (the Xbox adapter's equivalent gate) |
| `0x92684F` | mouse buttons merged after pad selection: LMB fire, RMB ADS, M4 reload, M5 crouch/slide |
| `0x91FE67` | no content pitch target and no auto-centering decay under capture (`vertical-recenter-research.md`) |
| `0x9202CF` | no soft pitch-limit spring on frames without vertical motion |
| `0x91AA17` | mouse look no longer grows the recoil offset (the "vertical spring" was recoil recovery) |
| `0x984060` | menu cursor; the native getter feeds it, so this detour only acts in cursor-only mode |

On Windows the input source is raw input (`WM_INPUT`): hardware counts, no pointer
ballistics. Buttons and wheel are forwarded into the normal bindings. Look and fire/ADS were
verified live in Earth patrol. The older route, which added deltas to the look-rate fields
`+0x2CC/+0x2D4` at `0x91AB3C`, survives only as the cursor-only fallback.

`d1-native-mouse-OUTDATED.md` describes that older route and is kept only for history.
`menu-cursor-mouse-handoff.md` is the address map of the KBM layer: mouse state layout, key
table, getter `0xad670`, builder, cursor.

**How the mouse interacts with part 2:**
- Input is polled every frame but applied only on tick frames, so above 60 FPS mouse look
  lands at tick rate.
- Phase 4 (late-latched view rotation, design section 4.10) is the fix.
- Whether the camera director scales look input by its dt is open. A fixed mouse sweep timed
  at 60 and at 160 FPS answers it (phase 1 record, section 11).

## Not in this zip

- The research evidence folders (`artifacts/frame-rate-research-20261006/`,
  `artifacts/renderer-research-20261005/`). They are git-ignored and contain third-party files.
- The Xbox compatibility layer's `kx_game_wait.h`, cited for the override's shape.
- The rest of the renderer (`d1port/gpu`) and the loader (`d1port/src/probe.c`). They are in
  the repository.
