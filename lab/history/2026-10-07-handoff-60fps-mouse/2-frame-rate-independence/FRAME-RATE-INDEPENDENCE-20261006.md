# Frame-rate independence: the design (2026-10-06)

The operator's goal: Havok and the game simulation run at a fixed 60 Hz; the client renders at
any rate above or below 60 without the game changing speed and without judder, smoother than
the d1port branch is today. This record is the research behind the design (ten read-only
agents: seven Sonnet readers on the web and the local records, three Opus disassembly digs on
the D1 PS4 eboot and the D2 PC executable), what it established, the design it leads to, the
phases with their exits, and the decisions that belong to the operator.

Reach tags: `[C]` read in code or disassembly (address given), `[O]` observed in a recorded run,
`[R]` read in a document or external source, `[I]` inferred, `[U]` unknown. Eboot addresses are
PS4 virtual addresses (guest-patch manifest addresses are these + 0x400000). The agents'
scripts, address maps and notes are kept under `artifacts/frame-rate-research-20261006/`
(git-ignored): `o1a/` (tick and frame consumers), `o1b/` (extract and render transforms), `o2/`
(D2 PC), `havok/NOTES.md`.

## 0. Verdict

Interpolation, not delta time. Keep authority (game logic, Havok `h = 1/60`, animation solve,
AI, networking) on the engine's own fixed tick, which the clock manifest already runs, and make
rendering a pure function of the last two ticks blended by a fraction. The engine has a clean
seam for it: its render extract runs on every rendered frame, including frames with no tick,
and copies object transforms into per-frame records keyed by a stable slot `[C]`. Nothing in
this engine family interpolates today. D1 PS4, D1 Xbox One and D2 PC all compute the leftover
tick fraction and throw it away `[C]`. Destiny 2 PC did not interpolate: it runs one variable,
smoothed tick per rendered frame, which is literally "delta time", and it carried
frame-rate-dependent gameplay bugs for years `[C][R]`. That route also varies Havok's step,
which breaks the operator's first requirement. Before any interpolation, four frame-rate bugs
in today's native configuration should be fixed. They are independent and cheap, and they
explain part of what feels wrong now (section 2, item 3).

## 1. What the research established

### 1.1 D1's clock, and what runs per tick versus per frame

The clock chain is in FRAME-CAPS-REMOVED-20261005.md section 5. New facts:

- **The fraction has no reader.** The kept fractional tick `G2+8` (`G2 = *0x26a3c08`) is read
  and written only inside the host-delta routine `0x5905c0` (`0x59088b`, `0x5908b7`,
  `0x5908c4`) and zeroed at init `0x5904b9` `[C]`. When the tick count is clamped (allowance 2),
  the fraction is left unchanged and the excess time is dropped (`0x5908b7`) `[C]`.
- **Frame record.** Two 0x38-byte slots at `0x26ae348 + idx*0x38`, index at `0x26ae3b8`,
  written by `0x590bf0` into slot `idx^1` and then flipped `[C]`. Fields used below:

  | Offset | Meaning |
  |---|---|
  | +0x00 | wall elapsed s |
  | +0x08 | "tick dt" |
  | +0x0c | accepted s |
  | +0x14 | due |
  | +0x2a | tick this frame |
  | +0x2b | last frame of a catch-up group |

  The spare bytes are +0x1c (4 bytes), +0x2d..0x2f and +0x34..0x37 `[C]`. The second dig names
  the same fields from base `0x26ae340`, so its offsets are 8 higher (+0x32 tick this frame,
  +0x1c due) `[C]`.
- **Units differ by mode.** With the override byte cleared, +0x08 on a tick frame is the
  frame's scaled real elapsed time, not `h`, and zero-tick frames carry the real elapsed time
  in +0x0c (`0x5908d8`, `0x590dd2-0x590de4`) `[C]`.
- **Fourteen game phases.** Built from the table at `0x192c2e0`; render extract graph X (8
  nodes, `0x19303e0`) and submit graph Y (7 nodes, `0x1930460`), all instantiated every outer
  frame by `0x592820` `[C]`.

| System | Rate today (native clock) | Time value | Site |
|---|---|---|---|
| Input poll (`scePadReadState`) | every measured frame | own wall delta | F0 `0x74a420` -> `0x927d60` `[C]` |
| Player input mapping | runs every frame, applies only on ticks (dt 0 on zero-tick frames with a local player) | due x h | `0x928250` -> `0x91d1f0` `[C]` |
| Game tick, AI, actions | tick | h | F1 `0x74a780`, F2, F3, F5 `[C]` |
| Animation solve, IK, sobject move | tick | h | F6 `0x74af30` -> `0x492960`, F9 `0x493e10` `[C]` |
| Havok | tick | h (`0x590a30`, 97 call sites) | F7 `0x74b480` -> `0xcace80` `[C]` |
| Camera director | every presented frame | **h, per frame** | F11 `0x4cba50`: `0x4cba6b call 0x590a30`, `0x4cba8c call 0x8ede20` `[C]`, checked first-hand |
| Camera update and view build | every presented frame, from per-tick object nodes | accepted | F1 tail `0x52d7c0` -> `0x812d10` -> `0x52c360`; extract `0x7ea1f0` -> `0x812d90` -> `0x52be00` `[C]` |
| Render extract and submit | every frame | accepted copied to packet +0x192e8 | X5 `0x804d20` -> `0x7eaf40`; Y `0x805260..` `[C]` |
| Particle simulation | every frame | accepted (real elapsed in native mode) | `0x87c260` `[C]` |
| Particle bookkeeping | tick | tick count | F10 `0x87fd30` `[C]` |
| Cloth | every frame | **constant 0.0333 s** | `0x7f0905`/`0x7f090f` load `0x1652170` `[C]`, checked first-hand |
| Fades, effect ages | tick | **+0x08 (frame elapsed in native mode)** | F10 -> `0x7d6fb0`, `0x8722c0` -> `0x86ddb0`, `0x7ccca0` `[C]` |
| Shader time | every frame, value stepped | (tick mod N) x h | `0x82d7e0` `[C]` |
| UI millisecond clock | every presented frame | `ms += trunc(dt*1000)` | `0x94b1d1-0x94b1dd` `[C]`, checked first-hand |
| Network update | every measured frame | none | `0xae1c70`, `0xae6940` `[C]` |

### 1.2 Nobody in the engine family interpolates; Destiny 2 PC uses delta time

- **D1 PS4.** The fraction has no reader (1.1). The render path keeps no previous/current
  pair of transforms: no motion-vector, velocity or temporal-AA surface exists in the surface
  table `0x16537f0`, and every extract record holds one transform set `[C]`. The only
  "previous" state is the visibility system's per-view record (`0x835c30`: previous view
  +0x10, projection +0x50, camera position +0x90, cut flag +0x470, jump threshold +0xa0), and
  "previous" there means the previous rendered frame, not the previous tick `[C]`.
- **D1 Xbox One.** The compatibility layer cleared the same override and logs the fraction
  passively. It records "no complete previous/current render snapshot pair or interpolation
  consumer for the fractional accumulator has been demonstrated yet" and names "render
  transforms and their prior/current state" as the next boundary `[R]`
  (`artifacts/renderer-research-20261005/xbox/uncapping-and-clocks.md`, `kx_simulation.h`).
- **D2 PC (build 86657, unpacked image).**
  - Its accumulator `0xb50f80` is D1's `0x5905c0` almost line for line, and its kept fraction
    `[obj+8]` again has no reader `[C]`.
  - It keeps the one-tick override (`0x1fb5fe0` = D1 `0x18c4aa4`) and adds a variable mode:
    `game_dt = smooth(min(elapsed, 1/max(10, min_fps)))`, where the smoother is a 120-entry
    mean or an EMA with k 0.8 (`0xb4f8a0`). One tick per rendered frame with that dt
    (`0xb5b84c..0xb5ba0e`) `[C]`.
  - The frame-time record has no alpha field `[C]`. The tick period follows the pacing
    settings: vsync, `framerate_cap` (default 200, range 30-10000) `[C]`.
  - The live dump in the image shows variable mode on, cap 178 and game dt about 33.4 ms
    `[C]`.
  - Each tick reports the activity time it covered to the host (`0x16e87b0`), which is how a
    variable-rate client stays consistent with a fixed-rate host `[C][I]`.
  - 31 gameplay sites still load 1/30 `[C]`. Public reports of fps-dependent damage (2021) and
    an fps-dependent encounter launch speed (2023) fit this design `[R]`.
  - The D2/D1 function map is `artifacts/frame-rate-research-20261006/o2/rva-map.tsv`.
- **"Time warp".** The string is not in either D2 binary or any local D2 record `[C][R]`.
  Public uses are the D2 Nightfall "Timewarp" modifiers and the "Time Skew Maxed" error
  (RUTABAGA, simulation falling behind wall time) `[R]`. The engine concept the operator
  remembers is the variable-dt tick above, which is what people call delta time `[I]`.

### 1.3 The render path: where transforms travel

- **Extract and feature renderers.**
  - The extract job is "extract: fixup_camera_and_views_and_begin_extract" (`0x7ea1f0`), with
    23 feature-renderer descriptors registered by `0x7fd3f0` / `0x803e10`
    (`+0x20` extract, `+0x50` prepare, `+0x70` submit) `[C]`.
  - The frame packet ring is `0x26caa30` (0x500000 bytes) `[C]`.
- **Skinned objects, gear and cloth.** All three go through one per-object extract,
  `0x894550`, run once per object per frame (atomic bit at packet +0x54) `[C]`.
  - It copies the node array (0x20 bytes per node: quaternion, then xyz plus uniform scale)
    and a root quaternion/translation into a 0x80-byte record `[C]`.
  - The record holds node count +0x18, nodes +0x20, root +0x30/+0x40, animation snapshot
    +0x50, and a first-person twin record +0x70 (mode 3) `[C]`.
  - It is filed in the packet at `+0x658 + accessor[+0x68]*8`, an 8192-entry table the size
    of the component-accessor pool, so the key is probably a stable slot `[C][I]`.
  - Prepare `0x8960f0` -> `0x8954e0` composes root x node and writes 3x4, 4x4 or
    quaternion-form palettes. Submit copies up to 216 float4 into command-buffer constants
    (`0x896450`, `0x81e270`, `0x7dd9b0`) `[C]`.
- **Rigid objects.**
  - The path is `0x109a3d0` -> `0x1095840` -> `0x2dd3c0` -> `0x2dd150(sobject handle)`: it
    reads the sobject's world quaternion +0xd0 and xyz/scale +0xe0, composes with the parent,
    and writes a 4x4 into a 0x90-byte instance record `[C]`.
  - `0x2dd150` has 13 gameplay callers; interpolate at the two render-side callers of
    `0x2dd3c0` (`0x1095840`, `0x10acff0`), never inside `0x2dd150` `[C]`.
- **Camera.**
  - The director `0x8ede20` writes the camera globals at `*0x2723060`: position +0x46c,
    forward +0x494, up +0x4a0, fov +0x4ac `[C]`.
  - The view callback `0x52be00` reads them through `0x8f2e80` / `0x8f2ac0` / `0x8f2b00` and
    builds a 0x410-byte render view (view +0x00, projection +0x40, position +0xc0) `[C]`.
  - `0x7e7900` fans that view out to the packet's views: main view `0x26d7fe0`, the
    first-person view `0x26d83f0` (type 9) and shadow views `0x26d69d0` `[C]`.
  - `0x82f3f0` stages the per-view constants (slot 2, 0x2a0 bytes) `[C]`.
- **Remote entities.** No network smoothing code with strings was found. The only lerping
  is an aim input converter (`c_input_value_converter_world_space_yaw_pitch_unbounded_lerping`,
  vfunc `0x4a9d10`) `[C]`.

### 1.4 Havok

- **Swept transforms.** Every Havok rigid body keeps a swept transform holding position and
  rotation at the start and end of the last step, `t0` and `t1`.
  `hkpRigidBody::approxTransformAt` blends them and is documented "for graphics" `[R]`
  (Havok 2013 headers via a public third-party copy; layout not checked against Bungie's
  2012.2.0 build).
- **Where the pair is not valid.** Bodies rewound by continuous collision carry
  `[t_impact, t1]`. Fixed and sleeping bodies store invDt 0. `warpTo` writes both ends, so
  teleports do not smear `[R]`.
- **Not our data path.** D1's render path reads the engine's own object nodes, not Havok
  bodies (1.3), so the swept pair is a fallback, not the plan `[I]`.
- **Do not change the step.** Havok's solver settings are fixed numbers tuned to the step
  (tau, damping, iterations), and its integrator clamps are per step. Its own variable
  timestepper warns of jitter and recommends fixed steps with an accumulated remainder `[R]`.
  Skyrim, Fallout 4, Dark Souls and the Bloodborne 60 FPS patch all broke physics or
  per-frame constants by varying `dt` `[R]`.
- **SDK.** Not found on the box (home partition, depth 9) or in the usual Windows folders.
  archive.org lists a "Havok 2012 SDK" whose listing includes
  `Havok_2012-2-0_Common_Manual.chi` (the D1 assert strings read `havok_2012_2_0_8`) `[R]`.
  Downloading it is the operator's call (licence); the design does not need it.

### 1.5 Industry and retrofit precedents

- **The canon** (Fiedler "Fix Your Timestep", Godot 4 physics interpolation, Unity, Bevy,
  Overwatch, Valorant). Fixed tick, snapshot before each tick, `alpha = accumulator / dt`,
  render `lerp(previous, current, alpha)`, which shows the world one tick late. Every
  interpolated entity needs a reset path; the camera and view model run at render rate; input
  is latched across 0- and 2-tick frames `[R]`.
- **Retrofits on code the authors did not write**, every one of them interpolation and none
  of them extrapolation `[R]`:
  - **Chimera (Halo CE PC).** The closest analogue: a closed binary, signature hooks, about
    1,000 lines.
    - It double-buffers each object's node transforms on every tick and computes alpha from
      the engine's own tick-progress value.
    - It writes blended values into the engine's live arrays before render and restores them
      after.
    - It snaps on distance thresholds per class, map load and checkpoint revert. It leaves
      first-person aim rotation unblended "for aiming feel".
  - **RT64 / Zelda64Recomp.** Interpolation at the renderer, matching matrices across frames
    by tags (about 8,000 lines of tagging in Majora's Mask) or by heuristics.
  - **Ship of Harkinian.** Records matrix operations per frame and replays them interpolated.
  - **sm64 ports.** Per-node history keyed by timestamp.
- **Failure classes all of them hit** `[R]`:
  - teleports under the snap threshold;
  - pool-slot reuse, needing a generation counter;
  - camera cuts;
  - billboards on camera snaps;
  - bones animated at a rate below the tick;
  - the "paper effect" of lerping final matrices;
  - motion vectors built from tick history instead of the interpolated transforms;
  - stragglers: UV scroll, cloth, lights, particles, HUD.
- **Halo MCC.** A 343 engineer described mixed units, "1/30 s" against "one tick", breaking
  weapons and scripts at 60 FPS, the same class as the D1 bugs in 2, item 3 `[R]`.

### 1.6 Pacing and display timing

- **CPU delta stutters.** A delta measured on the CPU at frame start stutters even at a
  constant frame rate (Ladavac, Croteam, "The Elusive Frame Timing") `[R]`. Alpha should come
  from a clock that advances by display refresh intervals.
- **Present timing.** `VK_EXT_present_timing` gives present and display timestamps and target
  times `[R]`.
  - Mesa 26.1+ (RADV and others) has it; the seat runs 25.3.6, so not there yet.
  - NVIDIA has it in beta drivers.
  - AMD Windows has `present_id2` / `present_wait2` and `fifo_latest_ready` but not
    present_timing.
  - Fallback: `present_wait` plus a software phase-locked loop on vblank times `[R][I]`.
- **The beat.** A 59.94 Hz tick on a 60.00 Hz display beats once about every 17 s: frames
  alternate 0 and 2 ticks `[I]`. Single-player engines lock the tick to the vblank grid
  `[R][I]`. In multiplayer that is not allowed. It makes ticks per wall second follow each
  player's display (60.00 against 59.94 is 0.1%, about 0.6 s of game time over ten minutes).
  The multiplayer answer is a shared tick rate from the wall clock, with interpolation hiding
  the beat. An exact lock is used only where the display can run at the shared rate (VRR, or a
  display mode equal to it) (4.8, correction 2026-10-06).
- **High-refresh fixed displays.** Steady presents at a divisor of the refresh beat
  free-running: 72 on a 144 Hz panel is smooth, a ragged 100 is not `[R]`.
- **VRR.** The application becomes the clock: present on its own steady grid, below the
  panel maximum and above its LFC floor `[R]`.

## 2. What you did not know

1. **Destiny 2 PC is the delta-time design, not interpolation.** It is one variable, smoothed
   tick per rendered frame, and the D1 hook point is identical, so it would be a small port.
   Its cost is Havok's step varying with frame rate and a long tail of per-frame and
   1/30-assuming code. D2 shipped with that tail, and D1 never had a PC pass to find it.
2. **No interpolation exists anywhere in this engine family.** The leftover fraction is
   computed and discarded in D1 PS4, D1 Xbox and D2 PC. Interpolation is new engine work, but
   the engine has the seam for it: extract runs every frame and copies transforms per object
   into keyed records.
3. **Today's native configuration has four frame-rate bugs of its own:**

   | Bug | Effect | Since |
   |---|---|---|
   | Camera director steps h every presented frame | its smoothing and transitions run about 2.7x fast at 160 FPS | the clock manifest |
   | Cloth steps 1/30 s every frame | 2x fast at 60, about 5x at 160 | under the 60 Hz manifest, also in the emulator's 60 FPS profile |
   | Fades and effect ages receive the frame's elapsed time, only on tick frames | about 0.375x speed at 160 FPS | the clock manifest |
   | UI millisecond clock truncates every frame | a few percent slow | always; the bug the Xbox layer fixed |

   None needs interpolation to fix.
4. **Why the gun steps while the camera looks smooth.**
   - The camera director runs every rendered frame (with a per-tick step), so the camera
     changes every frame.
   - The first-person gun (the extract's first-person twin record) and the characters change
     only on ticks.
   - The gun steps against a camera that moves every frame. That matches the operator's report
     in STALE-GEOMETRY-VERDICT-20261006.md and refines its candidate 1 `[I]`.
   - The design must put the camera and the gun on the same clock.
5. **Input is polled every frame but applied only on tick frames.** On tick frames it is
   scaled by due x h; on zero-tick frames the mapping receives dt 0 `[C]`. Presses on
   zero-tick frames must not be lost, and 2-tick frames must not double-apply. Whether the
   engine latches presses correctly is `[U]`.
6. **A CPU-measured frame delta stutters even at a constant frame rate.** The accumulator
   reads the CPU clock at the start of the outer frame. Smooth interpolation needs time
   measured at display, not at frame start.
7. **At 59.94 Hz ticks on a 60 Hz display, the 0/2-tick beat is permanent**, and in
   multiplayer the tick must not be locked to the display to remove it.
   - Every client must run the same `h` and the same ticks per wall second. Locking the tick
     to the vblank breaks the second for any display that is not exactly at the shared rate.
   - Interpolation makes the beat invisible. The blend factor drifts through 0..1 over about
     17 s, and the frame that carries the extra or missing tick is blended. The latency drifts
     between 0 and 1 tick.
   - Today the tick period (16,683 us) against `h = 1/60` runs the game at 0.999, as the
     original 29.97/1-30 pair did on PS4. A shared constant of 60.00 Hz (period 16,667 us)
     would make it exactly 1.000 and match PC monitors. That is a decision for the operator
     (section 7).
8. **Interpolation costs one tick of display latency** (16.7 ms behind the newest tick, about
   8 ms more on average than today). Lock mode on 60 Hz displays costs nothing. Where aim
   latency shows, the answer is a late-latched view rotation, not extrapolating the world.
9. **Havok already stores previous and current poses for every rigid body,** but D1's render
   path does not read Havok bodies. The transforms to interpolate are the engine's object
   nodes at extract.
10. **The game has no motion vectors, but the renderer does.** bbport's temporal upscaler
    (FSR 3, object-motion classifier tuned to Bloodborne) builds its own history. Once
    interpolation exists, its motion must come from the interpolated transforms, or moving
    objects ghost.
11. **Hook cost is real.** The per-draw engine hooks cost 5 ms a frame (CADENCE-20261006.md).
    Interpolation hooks are per object and per view, a few hundred a frame. They must do no
    I/O and no fault-safe syscalls on the hot path.
12. **Mixed time units are the main risk in any clock change.**
    - The record's +0x08 and +0x0c serve both per-tick and per-frame readers.
    - 97 call sites read `h`, and any of them reached from a per-frame path is a bug above
      60. The director is one such site already found.
    - This is the 343 lesson.
13. **Continuation frames below 60 are full frames.** When a frame owes 2 ticks, the engine runs
    the second as an extra outer frame that also extracts and submits. Whether that frame
    flips is `[U]`, and it decides how sub-60 play looks.
14. **The client's network update runs every measured frame,** so its rate follows the frame
    rate. D2 capped the equivalent at 40 Hz `[R]`. Server tolerance at 160+ is `[U]`.

## 3. Options

| Option | What | Havok at 60 | Judder | Latency | Risk | Verdict |
|---|---|---|---|---|---|---|
| A. Delta time, D2 style | one smoothed variable tick per frame | no, h varies | none from the clock | lowest | high: every per-tick assumption in D1 (97 h sites, 1/30 constants, Havok tuning, the server's tick rule) | rejected |
| B. Interpolation at extract, Chimera shape | fixed tick; snapshot per tick at the engine's extract; blend by alpha | yes | removed above and below 60 | +1 tick (0 in lock mode) | moderate: identity, snaps, stragglers | **chosen** |
| C. Interpolation at the GPU-command level, RT64 shape | match constants across frames in the renderer | yes | removed where matched | +1 tick | high: constants are anonymous bytes, no object identity, culling already ran | fallback for the camera only |
| D. Frame generation (FSR 3 / DLSS FG) | generate frames between rendered ones | yes | does not fix sim judder | +1 frame plus generation | artifacts, needs motion vectors | not a fix; optional layer later |
| E. 120 Hz authority | tick at 120 | no, h = 1/120 | removed at 120 only | lowest | very high: every rate-sensitive system | rejected |

The fork's roadmap reached the same fork in the road (`D:/dev/shadPS4/documents/perf/ROADMAP-20260912-60-120.md`
13.4-13.5) and its clock model already holds the pieces as a model:
`PresentationPolicy`, `fractional_alpha` and `SnapshotGenerations` in
`D:/dev/shadPS4/src/core/d1_clock_pipeline.h` `[R]`.

## 4. The design

### 4.1 Three clocks

1. **Authority tick.** 59.94 Hz today (period CFG+0x08 = 16,683 us, `h = 1/60`), driven by the
   engine's accumulator. Game logic, Havok, animation solve, AI and network state advance only
   here. Never varies.
2. **Display clock V.** The wall-clock time at which this frame will be shown, predicted from
   present timing (or the application's own grid under VRR). It drives the accumulator (4.8)
   and so alpha.
   - V must advance at the true wall rate: refresh intervals times the measured refresh
     period, never a nominal one. Then ticks per wall second stay at the shared constant on
     every client.
   - V decides the phase of a frame against the ticks, never the tick rate.
3. **View clock.** The camera, the first-person view model and cosmetic per-frame systems
   (particles, cloth, shader time, UI) read interpolated state and real frame time. They never
   write authority state.

### 4.2 The tick stays; the units get fixed (Phase 0)

| Fix | Change | Site |
|---|---|---|
| Camera director | run only on tick frames, keeping `h` | gate on +0x2a at `0x4cba50` |
| Cloth | pass the frame's real elapsed time, clamped, instead of 1/30 | `0x7f0905` / `0x7f090f` |
| F10 fades and ages | pass `h` instead of +0x08 when the native clock is on | `0x7ccca0`, `0x7d6fb0`, `0x8722c0` |
| UI clock | carry the fractional milliseconds | `0x94b160` |
| Audit | classify all 97 `h` call sites by the phase that reaches them; every per-frame reader of `h` is a bug candidate | static work from `o1a/getter-callers.txt` and the job tables |

The director fix is right once the camera is interpolated, because its outputs become per-tick
snapshots (4.4). Until then, giving it the frame's real dt keeps the camera smooth (an interim
choice to make by eye). The audit is needed before anyone claims a clean clock.

**Catch-up groups** (added 2026-10-06). F11, and with it the director, runs only on the final
frame of a catch-up group (+0x2b set) `[C]`. In a 2-tick group it runs once.
- **Interpolation on.** Gating the director "on tick frames with `h`" therefore loses a tick
  per group below 60. The correct dt is `h` x (ticks completed since the director last ran).
- **Phase 0 alone** (interpolation off). The correct dt is the wall time since the director
  last ran. That keeps today's per-frame camera but at the right speed.
- The same rule applies to any presented-frame-only consumer that is given a per-tick unit.

### 4.3 Snapshots and alpha

- **Alpha source, first version.** After `0x5905c0` returns in `0x590bf0`, alpha = `G2+8`
  (ticks, 0..1). `G2+8` is single-buffered, so stash it per frame. Two options: the record's
  spare +0x1c (written next to the slot flip at about `0x590e9a`), or a host-side table keyed
  by the published slot.
- **Carrying alpha to render.** Copy it into the render packet next to accepted (`0x7eaf7c`
  writes accepted to packet +0x192e8), so extract-time and prepare-time code reads the alpha of
  the frame it is building.
- **Snapshot rule.** For each keyed record, on a frame whose tick flag is set: previous =
  current, current = the freshly extracted values. Every frame: write `lerp(previous, current,
  alpha)` into the record that prepare reads.
  - Positions and scale lerp; quaternions nlerp with the sign flip. That is enough at one tick
    of motion, and it avoids the paper effect because nodes are stored as quaternion plus
    translation, not matrices.
- **Continuation frames.** `0x5905c0` is not called on a continuation frame (no time measured,
  `0x590d36`), so `G2+8` still holds the measured frame's value. Non-final frames of a group
  (+0x2b clear) render with alpha 1 (the newest tick) or are not presented (4.9). Only the
  final frame uses the stashed fraction.
- **To verify first** `[U]`: does frame f's extract see tick f's state or tick f-1's? The
  cross links (X0~F1, X1~F10, X2~F11, X4~F12, F13~X5) suggest after, but it decides which flag
  shifts the history. Test: log one node's value and the tick flag for 200 frames.

### 4.4 Camera and first-person weapon (Phase 1)

- **Snapshot.** Take the camera globals (`*0x2723060` +0x46c position, +0x494 forward, +0x4a0
  up, +0x4ac fov) on tick frames after the director runs. Serve interpolated values only to the
  view build on the extract path: `0x812d90` (X graph) -> `0x52be00`.
  - One site then feeds every view: main, first-person (type 9), shadows, culling and LOD.
- **Do not hook the getters** (correction 2026-10-06, checked first-hand). `0x8f2e80` is also
  called from the engine interface `c_spasaha_engine_interface` vfunc 16 (`0x6e6360`, call at
  `0x6e639f`) and from `0x5b7080` (call at `0x5b71aa`, a camera-facing transform). Those are
  non-render consumers, so an interpolated getter would leak into game-side code `[C]`.
  - The camera globals are touched only by the camera module (`0x8ed280-0x8f3780`).
  - The built views `0x26d7fe0` / `0x26d83f0` are referenced only by the render view module:
    `0x812b30`, `0x812d10`, `0x812d90` and the initializer `0x813440` `[C]`.
  - Leave the game-graph build (F1 tail -> `0x812d10`) uninterpolated. Interpolate the extract
    build (`0x812d90`), for example with a flag set around it, so the packet's views are
    interpolated and nothing on the game side reads a blended camera. Before relying on this,
    check that the packet views come from the `0x812d90` build `[U]`.
- **Cuts snap.** Reuse the visibility system's own jump rule (`0x835c30`: threshold +0xa0,
  cut flag +0x470), plus an explicit snap on camera type or target change.
- **The first-person twin record** (+0x70 of the skinned extract record) gets the same alpha
  and the same snapshot generation as the camera. They move together or the gun steps.
- **Shader time.** In `0x82d7e0`, add `alpha x h` to the wrapped times so animated materials
  move smoothly.
- **Exit.** Distinct view matrices per second = presents per second. The gun region's change
  pairs per second = presents per second with the frame-pair probe (`native/tools/frame_pairs_probe.py`).
  At 120+ FPS the operator sees no stepping in the gun.

### 4.5 Objects (Phase 2)

- **Skinned, gear and cloth.**
  - Hook `0x894550` inside its once-only branch, after the node and root copies. Key the
    history by accessor slot (+0x68) plus a generation: the node count, and the accessor's own
    generation if one exists, otherwise a spawn counter.
  - Blend the +0x20 nodes and the +0x30/+0x40 root.
  - Downstream (`0x8954e0`, all palette formats, all views, shadows) stays consistent without
    further hooks.
- **Rigid objects.** Blend the 4x4 at the render-side callers `0x1095840` and `0x10acff0`,
  keyed by sobject handle, decomposed to translation and quaternion before blending.
- **Stragglers, each a small item.**
  - dynamic_objects (descriptor type 1);
  - lights;
  - the particle extract's own transforms heap (`0x87b620`);
  - emitters attached to interpolated objects;
  - dynamic decals.
- **Culling and LOD** still see live state, which can cost one tick of edge popping. Accept it
  first; fix only if seen.
- **Exit.** For a set of tracked accessor slots, distinct pose samples per second = presents
  per second. No smear across respawn, teleport, cutscene or map load in the fleet lab
  (d1-fleet-lab bots: teleport, respawn and kill loops).

### 4.6 Per-frame systems

- **Particles** already simulate per frame with real elapsed time (native mode) `[C]`. They
  need only the interpolated emitter positions (Phase 2).
- **Cloth** gets real frame time (Phase 0).
- **UI** keeps its own per-frame clock with the fraction carried (Phase 0).
- **Audio** is device-clocked; its timed-node drain (F10 `0x8fcd10`) is per tick. Its time base
  is `[U]`; check that it is not reached per frame.

### 4.7 Discontinuities

- **Explicit snaps first:**
  - spawn and first appearance of a slot;
  - slot reuse (generation change);
  - node count change (LOD);
  - teleport and respawn;
  - camera cut;
  - map load and activity transition;
  - pause.
- **Heuristic backstop:** per-class distance thresholds per tick (Chimera uses 7.5 units for
  objects and 2.5 for bipeds as a starting shape) and a rotation-jump test (basis dot product).
- **After any snap,** interpolation stays off for one tick (quarantine). The fleet lab's
  teleport and kill loops are the test bed.

### 4.8 Pacing (Phase 3)

- **Feed the engine display time.** Replace `now_us` at the outer builder call `0x59395a`
  (`call 0x592820`, now_us in RDI). This is the technique the fork's wall-credit gate already
  proved at the same site. Feed V, the predicted display time of this frame, so the engine's
  own tick decisions and `G2+8` agree with when the frame is seen.
- **The multiplayer rule** (operator, 2026-10-06). Every client runs the same `h` and the
  same ticks per wall second, set by one shared constant that the session distributes. The
  display never sets the tick rate. The first version of this section had a lock mode that
  slaved the tick to the vblank (game speed = refresh x h). It is withdrawn: it would make a
  60.00 Hz player run 0.1% faster than a 59.94 Hz player.
- **Modes.** Chosen per output from the refresh rate and the present-timing feedback:
  - **Exact lock.** Only when the display runs at 1x, 2x or 4x the shared tick rate within
    measurement error.
    - This means a VRR panel driven at the tick rate, or a fixed mode equal to it: 59.94,
      119.88 or 239.76 Hz for a 59.94 constant; 60, 120 or 240 Hz for a 60.00 constant.
    - One tick per 1, 2 or 4 vblanks, with alpha = 1 at 1x and j/m at 2x and 4x.
    - Present with FIFO (or FIFO_LATEST_READY).
    - No speed change against other clients, and no added latency at 1x.
    - The launcher may prefer a matching display mode in fullscreen when the display offers
      one.
  - **Free-run interpolated.** Every other display, including 60.00 Hz against a 59.94
    constant.
    - V advances by whole refresh intervals at a steady divisor of the refresh, chosen from
      the p99 frame cost (step down after 3 misses in 30 frames, step up after 240 frames with
      slack).
    - Near-matching rates beat slowly: alpha drifts through 0..1 over about 17 s at
      60.00/59.94, and interpolation hides the frame that carries the extra or missing tick.
  - **VRR.** Detected by present timing (refresh interval `UINT64_MAX`) or Wayland refresh 0.
    Present on the application's own steady grid below the panel maximum. V = the grid time.
    The tick rate itself is the natural grid choice: exact lock, every frame a new tick.
  - **Lab switch only.** A "display lock" (tick slaved to vblank, speed = refresh x h) may
    exist for measurement in single-player. It is never on in a session with other players.
- **Sources of display time, in order:**
  1. `VK_EXT_present_timing` (Mesa 26.1+, NVIDIA beta).
  2. `present_wait` / `present_wait2` plus a software PLL on the vblank times.
  3. FIFO back-pressure timestamps plus the PLL.
- **Traps on record:** `D1_FPS_LIMIT=0` with the deadline on quantises presents to 20 Hz;
  `BB_VBLANK_HZ=0` selects the display refresh capped at 120 (FRAME-CAPS 3; memory). The pacing
  work replaces the 480 Hz virtual vblank and immediate presents with the modes above. That
  change lives in bbport's VideoOut and present thread.
- **Exit.**
  - Per frame, (V increment) minus (visible-time increment) is under 100 us RMS.
  - In exact-lock mode the ticks-per-frame histogram has no 0 or 2.
  - In every mode the step witness reads the shared constant: the same simulated seconds per
    wall second on every client, checked across two seats in the fleet lab.

### 4.9 Below 60

- **Catch-up.** The allowance of 2 ticks a frame stays; beyond it time is dropped (graceful
  slow-down, no spiral) `[C]`.
- **What to measure, Phase 0.**
  - A histogram of `due` per frame (a hook at `0x5905c0`'s return).
  - Flips per outer frame in a sub-60 view, which shows whether continuation frames present.
- **Then choose one:**
  1. Leave continuation frames as they are: mailbox drops the earlier one, and interpolation
     smooths the jump.
  2. Drop the present of non-final continuation frames: record +0x2b clear means more ticks
     are owed.
  3. Make continuation frames skip extract and submit. This saves the most and risks the most,
     because the frame packet ring expects every frame.

### 4.10 Latency

- **Free-run.** Interpolation shows the world one tick behind the newest tick.
- **Exact lock at 1x** adds nothing. Free-run on a near-matching display (60.00 against
  59.94) drifts between 0 and 1 tick, about 8 ms on average.
- **First answers,** before anything else:
  - exact lock where the display can match the shared rate (VRR, matching mode);
  - an input sample placed late in the frame;
  - the Mesa anti-lag layer, Anti-Lag 2 or Reflex where available, at the frame start.
- **Then, only if aim feels late at high refresh: Phase 4, late-latched view rotation.**
  - At view build, add the yaw and pitch from input received since the last tick on top of the
    interpolated camera. Recoil, aim assist and magnetism are applied by the tick and converge
    on the next tick.
  - This is what Chimera leaves unblended and what Carmack calls view bypass `[R]`. Measure the
    correction size before shipping it.

### 4.11 Hook budget

- **Per-frame hooks.** Camera snapshot and serve are a handful of calls. Object hooks run once
  per object on the game's job threads.
- **Budget.** Under 0.5 ms of added job-thread time per frame at the 10k-draw vista.
- **How to stay inside it.**
  - Use the loader's XMM-saving trampoline (the witness pattern).
  - Keep history in host memory sized for 8192 slots, with SIMD blends.
  - Do no logging on the hot path; counters only, printed in the existing report window.
- **Witness.** The draw-hook A/B (CADENCE) is the template: same build, hooks on and off,
  pacing histogram and step witness.

### 4.12 Temporal upscaler

- When interpolation is on, bbport's object-motion history must take the previous
  *interpolated* transform, not the previous tick's. The motion classifier was tuned to
  Bloodborne's skeleton constants (`motion_history.h:39-51`) and is unverified on Destiny.
- Until it is reviewed, run interpolation tests with the upscaler's object motion off.

## 5. Phases and exits

| Phase | Content | Exit | Size (rough) |
|---|---|---|---|
| 0 | `due` histogram and flips-per-outer counter; the four unit fixes; the 97-site `h` audit (static) | witness 1.000 at 45, 60 and 160 FPS; director, cloth, fade and UI speeds correct by video against the emulator at 60; audit table written | 200-400 lines (loader, manifest), one audit doc |
| 1 | alpha plumbing (4.3), camera and first-person interpolation (4.4), shader time | distinct view matrices and gun change pairs per second = presents; no gun stepping at 120+ by eye | 300-600 lines |
| 2 | skinned/gear/cloth at `0x894550`, rigid at `0x1095840`/`0x10acff0`, discontinuity rules, the stragglers | per-slot pose samples per second = presents; no smears in the fleet lab's teleport, respawn and cutscene loops | 800-1,500 lines |
| 3 | display clock V at `0x59395a` (wall rate, shared tick constant), present_wait PLL on the seat, present_timing where present, exact-lock/free/VRR modes, divisor selection | V-to-visible RMS < 100 us; no 0/2-tick frames in exact lock; identical step witness on two seats with different displays; operator verdict on 60, 120/144 and VRR displays | 500-900 lines (bbport VideoOut and present thread, loader) |
| 4 (optional) | late-latched view rotation, anti-lag integration | measured input-to-photon not worse than today's free-run at the same FPS | 200-400 lines |

The Windows port (WINDOWS-PORT-PLAN-20261006.md) carries all of this unchanged except the
Phase 3 extension selection (NVIDIA present_timing beta, AMD present_wait2).

**Coordination** (from the native client session, 2026-10-06):
- **Baseline for every phase.** The operator's play configuration is the launcher's 60 Hz
  manifest plus `guest/CUSA00219-native-clock.xml` (override off, deadline off), box-walk
  presentation defaults, and engine hooks off. The four unit bugs live in that configuration.
- **Validate instruments first.** Any instrument on the frame or job path (the `due`
  histogram, the interpolation counters) is checked against the "Step gaps" histogram line
  (`D1_STEP_WITNESS=1`) and `native/tools/frame_pairs_probe.py` before its numbers are
  believed. The draw hooks once cost 5 ms a frame unnoticed (CADENCE-20261006.md).
- **Where the code goes.** Clock work goes in `guest/` manifests or a separate loader file
  where possible. `probe.c`, `runtime_kernel.c`, `runtime_memory.c` and `d1port/gpu` are
  moving on the windows-lane-a/b/c and windows-integration branches. Phase 0 edits to
  `probe.c` start from windows-integration once it is merged to main.
- **The seat is the operator's.** Never run a cycle while they play.
- **Two play configurations** (correction from the native client session, 08:55 UTC
  2026-10-06; CADENCE-20261006.md section 4, FRAME-CAPS section 9):
  - **The cadence byte is not a no-op.** The cadence manifest's deadline byte at `0x5938eb` is
    required. On Windows under the stock 60 Hz vblank, the 60 Hz manifest without it gave
    locked 60 presents with the game at about half speed.
  - **The configurations now:**
    - Linux: 60 Hz manifest + cadence + clock manifest (override off, uncapped).
    - Windows, as first played: 60 Hz manifest + cadence on the stock vblank (override on,
      locked 60).
  - **Which unit bugs apply depends on the configuration.**
    - Override off: all four.
    - Override on at a locked 60: one tick per frame, so the director's `h` and F10's +0x08
      are correct there. Cloth is still 2x fast and the UI clock still truncates.
    - So `D1_FRAME_UNITS` must be correct under both. Read the override byte `0x18c4aa4` at
      run time rather than assuming it.
  - **Tick-period caveat.** This record takes the tick period (CFG+0x08) as 16,683 us under
    the 60 Hz constants, from the Linux witness. The Windows result reopens whether the
    deadline and the accumulator read the same field and what each holds per platform.
    - Read CFG+0x08 and CFG+0x28 live on both platforms before Phase 3 feeds display time at
      `0x59395a` `[U]`.
  - **Windows instrument.** The step witness now installs on Windows (opt-in,
    `D1_STEP_WITNESS=1`), so Phase 0 checks can run on this PC too.

## 6. Instruments

- **Step witness** (`D1_STEP_WITNESS=1`): Havok entries per second and simulated seconds per
  wall second. Must stay 59.94 and 1.000 in every phase.
- **`due` histogram per rendered frame** (new, Phase 0).
- **Motion samples per second.** Per tracked object and for the view: the count of distinct
  values per second, from the hooks' counters. This is the fork roadmap's criterion: "a scene
  with 120 presents but only 60 pose changes has not passed".
- **Frame-pair probe.** Content cadence per screen region (`native/tools/frame_pairs_probe.py`).
- **Present timing log.** V increment against visible increment, lateness, mode and divisor.

## 7. Decisions for the operator

**Decided 2026-10-06** (relayed by the native client session):
- The shared tick constant stays 59.94 (stock).
- Phase 0 and Phase 1 are being implemented now, without phases 2-4.
  - Code: a new `d1port/src/d1_frame_time.c`, on branch `frame-rate-phase1` in its own
    worktree.
  - Switches: `D1_FRAME_UNITS=1` and `D1_INTERP=1`, both default off.
  - Implementation record: `FRAME-RATE-PHASE1-IMPL-20261006.md` on that branch.
  - Seat tests only on the operator's word.

The list below is the original set of questions.

1. **Interpolation over D2-style delta time.** Recommended. Delta time is smaller to port but
   varies Havok's step and imports D2's bug class.
2. **The shared tick constant** (revised 2026-10-06 after the operator's multiplayer point:
   lock mode no longer sets the tick rate).
   - **59.94 Hz (today).** Game speed 0.999, as on PS4. Displays with 59.94/119.88 Hz modes
     and VRR panels get an exact lock.
   - **60.00 Hz.** Tick period 16,667 us. Game speed exactly 1.000. PC monitors at 60/120/240
     Hz and VRR panels get an exact lock.
   - Either way it is one value for every client in a session, distributed by the launcher or
     server, and a mismatched client is flagged.
   - Whether the server or the activity host assumes 59.94 anywhere is `[U]`. Check it before
     choosing 60.00.
3. **Camera latency.** Interpolated camera first (one tick behind in free-run), late-latched
   rotation only if aim feels late after Phase 3. Recommended.
4. **Phase 0 can land now.** The four unit fixes are independent of the rest. The cloth fix
   also matters to the emulator's 60 FPS profile.
5. **Havok SDK.** Not needed for this design. Downloading the archive.org copy is a licence
   question for the operator.

## 8. Open questions

- Does frame f's extract read tick f's state or tick f-1's? (4.3; one log test.)
- Do continuation frames flip? (4.9; one counter.)
- Is accessor slot +0x68 stable for an object's lifetime, and is there a generation field?
  (4.5)
- Which of the 97 `h` sites are reached per frame? (4.2)
- Is the camera's aim changed between ticks by anything other than the director? (2, item 5)
- Audio and network time bases at 160+ FPS (4.6, 2 item 14).
- Whether Bungie's Havok build keeps the 2013 swept-transform layout (only needed if Havok
  bodies become a data source).

## 9. Sources

Internal:
- FRAME-CAPS-REMOVED-20261005.md (sections 4, 5, 9)
- CADENCE-20261006.md
- STALE-GEOMETRY-VERDICT-20261006.md
- `artifacts/renderer-research-20261005/xbox/` (uncapping-and-clocks.md, kx_game_wait.h, kx_simulation.h)
- `D:/dev/shadPS4/documents/perf/ROADMAP-20260912-60-120.md`
- `D:/dev/shadPS4/documents/Debugging/Destiny1ClockPipeline.md`
- `D:/dev/shadPS4/src/core/d1_clock_pipeline.h`
- `D:/dev/d2-preservation/source/tiger-engine/status/runtime-port/D2_PS4_0159_NATIVE_ENGINE_LOOP.md`
- `artifacts/frame-rate-research-20261006/` (address maps and scripts)

External (each read by an agent; see the agents' notes for more):
- Fiedler, Fix Your Timestep: https://gafferongames.com/post/fix_your_timestep/
- Godot physics interpolation: https://docs.godotengine.org/en/stable/tutorials/physics/interpolation/physics_interpolation_introduction.html
- Godot transform interpolator: https://github.com/godotengine/godot/blob/master/core/math/transform_interpolator.cpp
- Chimera interpolation: https://github.com/SnowyMouse/chimera/tree/master/src/chimera/fix/interpolate
- RT64 frame matching: https://github.com/rt64/rt64/blob/main/src/hle/rt64_game_frame.cpp
- Zelda64Recomp tagging: https://github.com/Zelda64Recomp/Zelda64Recomp/tree/dev/patches
- Ship of Harkinian frame interpolation: https://github.com/HarbourMasters/Shipwright/blob/develop/soh/soh/frame_interpolation.cpp
- Tatarchuk, Destiny's Multithreaded Rendering Architecture (GDC 2015): https://advances.realtimerendering.com/destiny/gdc_2015/Tatarchuk_GDC_2015__Destiny_Renderer_web.pdf
- PC Gamer, Halo developers on unlocked frame rates: https://www.pcgamer.com/halos-developers-explain-what-can-go-wrong-with-unlocked-framerates-in-old-game-ports/
- Ladavac, The Elusive Frame Timing: https://medium.com/@alen.ladavac/the-elusive-frame-timing-168f899aec92
- VK_EXT_present_timing proposal: https://docs.vulkan.org/features/latest/features/proposals/VK_EXT_present_timing.html
- Mesa 26.1.0 release notes: https://docs.mesa3d.org/relnotes/26.1.0.html
- Havok 2013 headers (third-party copy; do not vendor): https://github.com/sigmaco/havok-2013
- Carmack, Latency Mitigation Strategies: https://danluu.com/latency-mitigation/
- Input in a fixed timestep: https://jakubtomsu.github.io/posts/input_in_fixed_timestep/
