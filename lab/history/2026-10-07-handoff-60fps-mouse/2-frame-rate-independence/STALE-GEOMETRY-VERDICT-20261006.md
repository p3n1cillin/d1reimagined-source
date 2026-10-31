# Stepped animated geometry: the verdict (2026-10-06)

Symptom (operator, Linux seat, Ryzen AI MAX+ 395 APU, RADV): in d1port the first-person gun,
characters and effects move in visible steps while the world and the camera are smooth. The
same at 60 FPS capped and at 300-350 FPS uncapped (step witness 60.0 Havok steps/s, 1.000
simulated s per wall s), on two monitors. Seat toggle mask 24576 (bit 13 DrawPreparation and
bit 14 DeferredStreamCopies off).

Inputs: STALE-GEOMETRY-BUFFERS-20261006.md, STALE-GEOMETRY-PIPE-20261006.md,
STALE-GEOMETRY-REFERENCE-20261006.md (same folder), plus FRAME-CAPS-REMOVED-20261005.md and a
first-hand check of the code they cite. Read-only work: nothing built or run. Reach tags: [C]
read in code, [O] observed in a log, [R] read in a document, [I] inferred, [U] unknown. Code
paths are under `d1port/gpu/shadps4/video_core/` unless they start with `d1port/` or `native/`.

## 0. Verdict in one paragraph

The leading explanation is on the game side, and the project already wrote it down. The clock
manifest that took the client to 300 FPS turns the fixed-clock override off and lets the
engine's own accumulator run 59.94 ticks a second "whatever the frame rate", and the record
says outright: "no interpolation: above the tick rate the world moves every one or two
frames" (FRAME-CAPS-REMOVED-20261005.md:168) [R]. It also names the visible result: frames
between ticks show the previous state with a random phase, which reads as missing and doubled
steps and "does not improve at higher frame rates" (same file :189-194) [R]. Override off with
the deadline on (a 60 FPS cap) is "suspect by construction": the outer period equals the tick
period, so jitter gives 0- or 2-tick frames (:210-216) [R]. That is the symptom exactly:
tick-driven content (gun animation, characters, effects) steps; the static world, seen
through a camera that evidently updates every rendered frame, is smooth; frame rate and
monitor make no difference [I]. The renderer hazards the three reports found are real in code
but are timing races. A race gives irregular stale or reversed poses, not a cadence that is
the same at 60 and 300 FPS [I]. The fork's double-speed record happened through a cross-frame
draw memo that d1port does not have (REFERENCE 1.3, 3.2) [C]. UMA: guest memory is NOT
imported as device memory in this build. Arenas are plain device-local allocations
(`buffer_cache/buffer_cache.cpp:81-84`), with no external-memory-host import anywhere in
video_core [C]. bbport dropped the import on purpose (`d1port/docs/parallel_gpu.md:191-197`)
[R]. So on the APU, GPU writes do not land in guest memory directly [C].

## 1. Ranked candidates

| # | mechanism | evidence | confidence |
| --- | --- | --- | --- |
| 1 | **Game side: free-running render over a fixed 60 Hz tick with no render interpolation.** Override byte off (`native/guest/CUSA00219-native-clock.xml:31-36`), so ticks come from wall time, not frames. Zero-tick frames are legal and render the old simulation state (FRAME-CAPS :104). Above 60 FPS, 4 of 5 frames carry no tick. At a 60 cap the frame and tick clocks are not locked, so 0/2-tick frames occur. The camera (at least rotation) updates every frame, so the static world is smooth while everything the tick advances steps. | FRAME-CAPS :104, :163-168, :189-199, :210-216 [R]; manifest [C]; the 1.000 witness at 300 FPS shows the override was off in that run [O operator, I]; whether the 60 FPS run had the override off is [U]; the camera updating per frame is [I] from the operator's report | high |
| 2 | **Renderer H1: constant-ring pending-write stamp one packet early.** `NotePendingGpuWrite` stamps a draw's or dispatch's own writes with `Head()` before `Begin`/`Commit` (`renderer_vulkan/vk_rasterizer.cpp:473`, `:561-563`, `:605`, `:626`, `:688`). `PendingWriteOverlaps` treats them as done once stage B finishes the previous packet (`:508`; `vk_draw_pipe.h:146-152`, `:223-226`). A small (<= 16 KiB) read-only binding posted in that window, on a range whose GPU bit is clear, is copied from guest memory on stage A (`vk_rasterizer.cpp:567-571`, `:539`). | mechanism [C], checked first-hand; Destiny producing animation data on the GPU into such buffers [U]; picture would be irregular old poses [I] | low-medium |
| 3 | **Temporal upscaler on animated draws.** The default is FSR 3 with jitter and object motion on (`d1port/gpu/shim/bbport_settings.h:49`, `:53`, `:55`). The object-motion classifier was tuned to Bloodborne's skeleton constants (`renderer_vulkan/motion_history.h:39-51`). Wrong or missing motion for skinned draws makes history reproject wrongly. | defaults [C]; that it gives steps rather than ghosting [U] | low |
| 4 | **Renderer H4: CE dump vs deferred upload copies.** `DrainDrawPipeTo` waits for stage B, not for copy threads that read guest memory later (DeferredUploads, `buffer_cache/buffer_cache.cpp:537-584`; `amdgpu/liverpool.cpp:433-457`). An earlier draw can upload a later dump's bytes (a future pose). | gap [C] per PIPE 5; reach [U]; irregular by nature [I] | low |
| 5 | **Copy and readback side: the stage-B thread-local BbCopy batch not drained before a stage-A submit (ParallelCopies), and Relaxed readbacks (CPU reads of GPU output never faulted).** | `d1port/gpu/shim/bbport_copy.cpp:236-295`, `region_manager.h:104-108`, `emulator_settings.h:29-31` [C] per BUFFERS 3; Destiny's simulation reading GPU output [U, unlikely] | very low |

Ruled out: the fork's read-after-read mechanism. It is an image-tracker rule whose harm went
through the fork's cross-frame draw memo (`DrawMemoValidate`). d1port keeps the stock image
barrier rule (`texture_cache/image.cpp:268-306`) and has no such memo (REFERENCE 3.1-3.2) [C].
The vertex-buffer write-after-read gap in REFERENCE 3.3 would tear, not step [I].

## 2. Tests, in order

How to read the probe in every test. Hold the camera still and keep the gun animating
continuously (idle sway while strafing, or a reload). Then, separately, turn the camera
without walking. For each test record:
(a) gun-region change pairs per second against the step witness (about 60/s);
(b) world-region change pairs per second against FPS;
(c) any A-B-A reversal in the gun region (a pose that returns to an older one).

The probe must compare consecutive presented frames. A request-driven dump that skips frames
cannot see a zero-tick frame [I].

- Candidate 1 predicts: (a) at most about 60/s at any FPS >= 60, (b) equal to FPS while
  turning, (c) never.
- Candidates 2 and 4 predict: (a) above the tick count, or (c) present.

### Test 1. The best split overall, needs a relaunch: exactly one tick per frame

Relaunch with the clock manifest's two entries left out, which gives the emulator
configuration (override on, deadline on, one fixed tick per rendered frame at 59.94):
`D1_GUEST_SKIP="D1 clock" bash native/box-walk.sh ...` with `D1_TOGGLES` left at 24576.
"D1 clock" matches both entry names (`native/guest/CUSA00219-native-clock.xml:31`, `:38`)
and nothing in the 60 Hz lab manifest. The match is a case-sensitive substring
(`native/guest_patches.py:97`) [C]. Do not use `D1_GUEST_SKIP="override"` alone: with the
deadline off, that ties game speed to the frame rate again (FRAME-CAPS :86-88) [R]. Judge in a
light view that holds 60 FPS: with the override on, a slow frame slows the game but still
carries one tick.

- Steps gone: candidate 1 confirmed. All renderer candidates are untouched by this change,
  same toggles and same renderer.
- Steps unchanged with exactly one tick per frame: candidate 1 is refuted for the 60 FPS case.
  Go to test 2.

### Test 2. The best live split (no relaunch): every renderer suspect off at once

Write **137455886592** to out/toggles.txt. That is 24576 plus:

- bit 37 DrawPipeline (137438953472), which also removes the constant ring and H4, because
  `PostDraw` runs only on the pipelined path (`renderer_vulkan/vk_rasterizer.cpp:418-420`,
  `:942`, `:1292`);
- bit 24 upscaler off (16777216, `renderer_vulkan/vk_temporal_upscaler.cpp:222-228`);
- bit 17 ParallelCopies (131072);
- bit 8 DeferredUploads (256).

Frame rate drops (the pipe was worth most of 300+), but candidate 1's look does not depend on
FPS above 60 [I]. Check that the FPS line stays above 60.

- Probe unchanged (gun at about 60 changes/s, no reversals) and the steps still visible:
  candidates 2-5 are refuted together. Back to the game side.
- Steps gone or the probe changes: bisect with tests 3-5. Restore 24576 between tests.

### Test 3. Upscaler alone: **16801792** (24576 + 2^24)

History restarts on the switch (`renderer_vulkan/vk_temporal_upscaler.cpp:223`). Look for
ghost trails turning into clean steps, or into smooth motion. Object motion alone (vertex
stores off, upscaler on) is **536895488** (24576 + 2^29,
`renderer_vulkan/vk_object_motion.cpp:141`).

### Test 4. Constant ring alone: **68719501312** (24576 + 2^36)

This is H1. Small read-only bindings go back to stage B's stream path, which honours the GPU
bit (`buffer_cache/buffer_cache.cpp:295-321`). Expect a small FPS change. Look for reversals
(c) disappearing. If they do, confirm with the witness counter in section 3b before any fix
lands.

### Test 5. Draw pipe alone: **137438978048** (24576 + 2^37)

Then H4: **576460752303448320** (24576 + 2^59 CeCounterDrain + 256 DeferredUploads), then
ParallelCopies alone: **155648** (24576 + 2^17).

### Environment-only checks (restart)

- `BB_READBACKS=2` (Precise) for the readback half of candidate 5. Witness: read_faults goes
  above zero in play.
- Confirm that the seat env does not set BB_HOT_PAGES=1, BB_VK_RECORD_THREAD=0 or BB_UFFD=1
  (BUFFERS 5.6) [U].

### By eye, no relaunch

Strafe without turning. If the world then steps too, camera translation is tick-driven and
only rotation is per frame. That is further support for candidate 1 [I].

## 3. Fixes

### 3a. Candidate 1 (top): game side, no renderer change

- **Now (configuration):** play with the override on and the deadline on, the configuration
  the record already calls safe for paced play (FRAME-CAPS :210-216) [R]. Files:
  `native/box-walk.sh` (a play default of `D1_GUEST_SKIP="D1 clock"`, or a separate play
  profile that leaves `native/guest/CUSA00219-native-clock.xml` out of `D1_GUEST_PATCHES`). In
  the manifest, mark both entries as measurement mode only.
  Risk: the frame rate is capped at 59.94, and any frame over 16.7 ms slows game time (cap 4's
  old behaviour). Restoration's busy and vista views (80-84 and about 55 FPS) slow the game
  wherever they drop under 60 [R].
- **Proper (engine work, large):** render interpolation inside the engine, blending the
  render-side transforms of the last two ticks by the accumulator's kept fraction `G2+8`
  (FRAME-CAPS :99-100, :199) [R]. Where the engine publishes object and bone transforms to its
  render layer is [U]. Finding it is a disassembly task on the guest, not a renderer task.
  Risk: high; it touches every animated object.
- **Witness first (code, small):** a loader hook at `0x5905c0`'s return that histograms `due`
  per rendered frame (FRAME-CAPS :216). Zero-tick and two-tick frames that line up with the
  probe's held and doubled gun frames settle the question without any by-eye call.

### 3b. Candidate 2 (top renderer candidate, only if tests 1-2 point at the renderer)

- File: `renderer_vulkan/vk_rasterizer.cpp`.
- Functions: `CollectRingBindings` (515-573), `PostDraw` (575-690) and `NotePendingGpuWrite`
  (468-488).
- Change: in `CollectRingBindings`, collect the written ranges (`desc.is_written`) into a
  per-call list instead of calling `NotePendingGpuWrite` at 562. Skip the ring copy for any
  read-only binding that overlaps a range in that list, so a packet that reads what it writes
  does not take the copy. In `PostDraw`, after `draw_pipe->Commit` at 688, note each collected
  range with `NotePendingGpuWrite`, so the stamp is the packet end. This matches what
  WriteData and DmaData already do (`amdgpu/liverpool.cpp:1220-1227`, `:1239-1244`).
- Do not use a raw `Head()+size` stamp: `Begin` can move `head` across a wrap
  (`vk_draw_pipe.h:63-71`).
- Witness before and after: in `BindBuffers` where a ring binding is used
  (`vk_rasterizer.cpp:2055-2064`), count ring bindings whose range `IsRegionGpuModified`
  reports true at stage B time. Any count above zero is a stale copy. Print it with
  BB_FRAME_STATS.
- Risk: low. A few more small bindings fall back to the stream path, and the change only
  narrows what stage A copies.

## 4. What refutes the renderer class

Any one of these sends the work back to the game side:

1. Test 1 (one tick per frame, same renderer, same toggles) removes the steps.
2. Test 2 (137455886592: pipe, ring, upscaler, parallel and deferred copies all off) leaves the
   steps and the probe read-out unchanged at a frame rate above 60. The same holds for a
   restart with `BB_DRAW_PIPE=0 BB_UPSCALER=off`.
3. The probe shows gun-region changes at or below the tick count (about 60/s) at 300 FPS,
   with world-region changes equal to FPS while turning, and never an A-B-A reversal. A stale
   renderer read produces extra changes or reversals. Tick-limited content cannot.
4. The `due` histogram's zero-tick and two-tick frames line up one-for-one with held and
   doubled gun frames.

Conversely, a reversal in the gun region, gun changes above the tick count, or a probe change
under tests 3-5 keeps the renderer in play. In that case candidate 2 is first in line.
