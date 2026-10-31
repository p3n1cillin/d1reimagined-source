# Per-draw cost: preparation for the command-processor handoff

2026-10-05, evening, Claude. Written before a second handoff on the command processor arrives,
at the operator's request: think it through first, with outside context. The question as the
operator put it: the same engine's Xbox One build runs under a Wine-based compatibility layer at
well beyond 60 frames a second, so the PS4 build under our runtime should be able to match it;
what are we missing? Reach tags as elsewhere: [C] [O] [R] [I] [U].

## 1. The comparison, stated carefully

- The Xbox One binary is the same engine seven months newer (build 56578 against our 52553) and
  talks to the console through `d3d11_x.dll`: the D3D11 resource model with usage flags, map and
  unmap, placement textures, ESRAM calls [C, `D:/dev/d1-xbox/XBOX-VS-PS4-BINARY-20260912.md`].
  Under the compatibility layer the game's D3D11 calls go through shim DLLs to DXVK, which turns
  them into Vulkan. That layer intercepts an API; it does not decode a GPU command stream.
- The PS4 binary builds PM4 command buffers itself and the CPU–GPU contract is raw memory plus
  fences. An emulator or runtime must reconstruct pipeline state from register writes at every
  draw and infer which bytes of guest memory are still valid [C, same note; R, fork `rendering.md`].
- The tester bundle's own briefs from 2026-09-10 recorded the Xbox layer "pinned at the title's
  30 fps cap" after the placement-texture work, and no figure above 30 [C, same note]. The
  operator reports it now runs far above 60; the record of that run (resolution, scene, machine)
  is not in this tree yet and is worth attaching to the handoff.

**What the Xbox layer's own records say** (a read-only reader went through
`~/dev/d1-xbox-moosh` on the box and the tester bundle's briefs, file by file, 2026-10-05
evening; second-hand here, each item with the path it gave) [R]:

- No uncapped in-world frame rate exists in those records. The author's uncapped figures
  (280 to 464 FPS at 900p/1080p, 142 FPS at 4K internal) are **orbit and menus** on a Ryzen 7
  9800X3D with an RTX 3080 (`src-main/reports/frame-budget/uncapped-1080p/REPORT.md`,
  `uncapped-4k/REPORT.md`, `STATE.md`). The author's best **in-world** number is 59.91 FPS at a
  60 FPS cap on that rig, with the simulation at 29.9 Hz and game clock 0.9986 s/s
  (`reports/native-input/animation-cadence.md`); the author writes that gameplay performance for
  later builds is unmeasured and the 60 Hz-simulation destination test "has not completed"
  (`reports/render-performance/README.md`, `reports/render-pose/simulation-60hz.md`).
- **On this seat** (the same Ryzen AI MAX+ 395 / Radeon 8060S, 1920x1080, title pacer and shim
  cap off) the Xbox layer logs **13 to 19 FPS in a patrol world** at about 8,800 to 9,500 draws a
  frame (`logs/xbox-act-p3/d1shim.log`, `xbox-act-p4`), and about 120 FPS in orbit and menus at
  28 to 138 draws a frame, which is most likely the display's vsync under FIFO, not renderer
  speed (`logs/main-run7-bap1/d1shim.log`).
- The launch is Wine 11.0/11.10 Staging with a custom DXVK 2.7.1 (`bin/play-xbox.sh`,
  `main/RUNTIME-SHA256SUMS`), FIFO with 3 to 4 swapchain images, 1080p. What made the author's
  build fast where it is fast: one VkImage per placement texture reused across re-creations
  (texture-creation CPU down 72.5%), write-watch sweeps instead of faults, uploads moved behind
  Present with a budget, a persistent texture cache, wrapper pooling, a 4.5 ms "render minimum"
  spin removed (`m0/d3d11_x.c`, `reports/frame-stall-reduction`, `wrapper-pooling`,
  `cpu-orbit-trace`). In the world on the box its counters show heavy texture create and upload
  churn (228,542 creates against 193,366 reuses at frame 5,640); whether that is its limiter there
  is not shown.

So the premise narrows: the Xbox layer is far beyond 60 in menus, about 60 capped in the world
on a discrete GPU, and well below our native client in the world on this very seat (13 to 19
against 37 to 46 virtual flips a second at a similar draw count). The two are not the same
problem, and the Xbox figure does not transfer as a target for the CPU side. What it does establish, if the scene and resolution match, is that **the GPU work of
this engine's frame is cheap on a PC GPU**: whatever our frame costs on the host GPU above that is
the renderer's own doing (barriers, pass breaks, copies, resolves), not the game's.

## 2. The budget

| | per draw |
|---|---:|
| 16.67 ms over 10,400 draws (stock field of view, Restoration spawn) | 1.60 µs |
| 16.67 ms over 13,300 draws (field of view 105) | 1.25 µs |
| bbport, command thread, measured [O] | 1.4 to 1.7 µs of CPU, drains included |
| bbport, recording thread, measured as CPU share [O] | about 1.3 µs (55 to 62% of a core at 45 flips/s and 10,400 draws) |
| bbport, Vulkan recorder, measured as CPU share [O] | about 1.3 µs (same) |
| the fork's single command-processor thread before its round 2 [R, `PLAN-20260912-cp-round2.md`] | 16.6 ms on-CPU per iteration at 11,446 draws: 1.45 µs |
| the fork's sampled record phases (run executor) [R, `RESEARCH-20260911` §10 boot 9] | pipe 0.29, sharps 0.12, slots 0.38, prewarm 0.08, index 0.26, tail 0.83: about 2.0 µs |
| the fork's design target for a repeated prepared draw [R, round-2 plan] | 0.35 µs |
| bbport on its own game (Bloodborne, 1,600 draws a frame), recording stage [C, `docs/parallel_gpu.md`] | about 2.8 µs a packet, "flat profile", cache misses over shared structures |

Two readings. First, bbport's three stages each sit near the whole budget on their own, so the
pipeline only helps while all three keep up; any one of them over 16 ms is the frame. Second, the
fork reached about the same per-draw cost on one thread that bbport spends on each of three;
bbport's split buys overlap, not less work. Measured CPU shares include spinning (drains and the
recorder's spin), so the real work per draw is somewhat less than the table says. Per-stage
profiles at the stock field of view (run 1 of the plan's 5.1) replace this table.

## 3. What the fast layers do

Standard guidance, with the sources, and whether bbport does it:

| Practice | Source | bbport today |
|---|---|---|
| One descriptor bind per draw, or none: dynamic offsets into large uniform buffers, frequency-based sets, bindless | zeux, "Writing an efficient Vulkan renderer"; AMD RDNA guide ("dynamic constant management … substantially reduce CPU overhead") | push descriptors per draw (`vkCmdPushDescriptorSetKHR` with the draw's writes) [C, `vk_graphics_pipeline.cpp`]; constants copied into a stream buffer per binding |
| Descriptor buffers (`VK_EXT_descriptor_buffer`) where descriptors change every draw | AMD Vulkan samples, "descriptor management" | not used; the seat exposes it [C] |
| Push constants for the few bytes that change per draw | AMD RDNA guide | used for the renderer's own push data |
| Avoid per-draw vertex stream binds | AMD RDNA guide | vertex buffers bound per draw (`EmitVertexBuffers`), memoised when prepared |
| Fewer, larger command buffers and submits (≥10 draws a command buffer; <10 submits a frame) | AMD RDNA guide; zeux | one recorder stream; submits per frame not yet counted here |
| Record on several threads | zeux | one recording thread; bbport's draw-preparation workers precompute pure parts and are off for Destiny (toggle 13) because they read guest memory at the wrong time |
| Deduplicate state: do not rebind an unchanged pipeline or re-open an unchanged pass | general | render-state memo (90% hits in Bloodborne), texture-set memo, access and binding memos [C, `parallel_gpu.md`]; hit rates for Destiny not measured |
| Let the GPU generate draws for runs of similar work (`VK_EXT_device_generated_commands`) | Khronos; RADV since Mesa 24.3 (Phoronix); the seat exposes it [C] | not used; the fork's "GPU as command processor" went this way and its own assessment says the naming did not move most of the work off the CPU [R, `ASSESSMENT-20260912`] |
| Whole guest memory as one GPU buffer with page-validity tracking (xenia's shared memory) | xenia `shared_memory.h` [R, via the fork's research note] | not used; bbport copies into host buffers and tracks writes by page protection; the fork imported guest memory (`VK_EXT_external_memory_host`) and then needed honest fences |

Two cautions from the fork's own record: the "GPU as command processor" kept pipeline selection,
descriptor capture, coherence decisions and recording on the CPU [R, assessment §Decision]; and
its Nsight traces showed the GPU starved by 400 to 600 wait-for-idle drains a frame from barriers
and pass ends, and by PCIe latency, not by shading [R, research §2]. The second does not apply to
a unified-memory APU; the first applies to any design.

## 4. What bbport does per draw today, and what is already paid once

From `vk_rasterizer.cpp` (`Draw`, `DrawRecord`), `vk_draw_pipe.h`, `vk_scheduler.h` and the
design notes [C]:

- **Command thread** (stage A): decode the packet, apply register writes and hash the changed
  blocks, select the pipeline (specialisation computed and compared per draw, sharps read),
  collect ring bindings, copy small read-only constants into its own ring (`ConstantRing`), write a
  draw packet (changed register blocks, user data, draw parameters) into the 16 MiB pipe.
  Profile at FOV 105 before the counter rule [O]: drains 30%, decode 8%, hand-over 7.5%,
  specialisation about 10%, register hashing 8%, ring bindings 5.6%, image descriptors 3.7%,
  `memmove` 3.6%, pipeline selection about 8%.
- **Recording thread** (stage B): apply the blocks to its register copy, `PrepareRenderState`,
  `BindResources` (texture cache lookups and `UpdateImage` per binding, buffer cache
  `ObtainBuffer` per binding, descriptor writes), `BeginRendering` (or the memo), resolve and emit
  vertex and index buffers, barriers, dynamic state, then queue the Vulkan commands as closures.
  Never profiled for Destiny.
- **Vulkan recorder**: runs the closures (`bindPipeline`, push descriptors, vertex and index
  binds, dynamic state, `drawIndexed`), in chunks. Never profiled for Destiny.
- **Already paid once, not per draw**: pipelines (cache with asynchronous compile), texture
  descriptions, render state while the pass continues, texture sets by sharp hash.

What is still per draw and could be per change: the pipeline bind (bbport binds the pipeline
in every draw's closure), the descriptor push (every binding rewritten each draw), vertex stream
binds, the constants copy, and on the command thread the specialisation compare and the
register-block hashing for every draw whether or not anything changed.

## 5. Candidate reductions, in the order I would test them

Each is a measured A/B behind a toggle, with the version check of the plan's section 7 on.

1. **Constants carried at decode.** The command thread already copies small constants into its
   ring; make that the only copy and the only read of constant memory, so no dump ever needs a
   drain and the recording thread binds without touching guest memory. Removes the remaining
   11 to 14% drain time and the ordering argument of patch 14 at once; prerequisite for turning the
   draw-preparation workers back on. Cost: the copy itself (~5 MB a frame at 13,000 draws).
2. **Per-change instead of per-draw on the command thread.** Skip specialisation and pipeline
   lookup when no register block or user-data word that feeds them changed since the previous
   draw (the dirty blocks are already tracked); count how often that is true for Destiny first.
3. **Fewer Vulkan commands per draw.** Bind the pipeline only when it changes; push only the
   descriptors that changed, or move to descriptor buffers with one offset per draw; one closure
   per draw instead of several. The recorder's cost is close to the driver's floor per command, so
   this is the only lever on that thread besides batching.
4. **Draw runs.** Consecutive draws with the same pipeline and bindings differing only in
   arguments become one `vkCmdDrawIndexedIndirect` with a count (the fork's "draw runs", about
   3.9 draws a run in its heavy scene [R]). Count the run lengths here first.
5. **Pass and barrier continuity**: measure pass ends, barriers and layout transitions a frame on
   this GPU with `BB_GPU_PROFILE=1` before touching them; the fork's lesson is that the GPU side
   can be the frame even when the CPU fits.
6. **Multi-draw runs with versioned preparation** (the fork's *active* fast path: host-built
   stage blocks, one `drawIndexedIndirect` per run of adjacent compatible draws, a shader ABI that
   selects per-draw data by draw index; its compute resolver and device-generated-command
   executor are retired and should stay so, per GPT's second handoff
   [RENDERER-FLOW-AND-COMMAND-PROCESSOR-20261005.md](RENDERER-FLOW-AND-COMMAND-PROCESSOR-20261005.md)
   §6, read 2026-10-05 [R]). Last, because it is a coherent unit (stage blocks, shader
   translation, bindless slots, range and event boundaries, retirement) rather than a file, and
   because its known hazards (the draw-index varying at location 31, attachment semantics in the
   range key, ordered same-frame fallback) come with it. Item 4 above is its first, smallest form.

Not on the list: more threads per draw. bbport measured that moving per-draw work to a helper
core costs about what it saves because the data is shared (`parallel_gpu.md`, texture helper);
the gains came from stages that own their data for long stretches.

## 5a. Memory: what the native runtime can do that the fork could not

The operator's point (2026-10-05 evening): the fork's unified-memory design imports and mirrors
large amounts of system memory and VRAM "just in case" instead of mapping exactly the
allocations the game makes. The fork's own research note confirms the shape: 132 chunks of
64 MiB, 8,448 MiB of guest backing imported up front, with a VRAM twin on top [R,
`RESEARCH-20260911` §5a]; two readers of its memory document report the game itself declaring
about 2,461 MiB of GPU heap in two large mappings plus about 1,008 MiB of CPU-cached heap [R, at
one remove].

Here the runtime implements `sceKernelAllocateDirectMemory` and the mappings itself
(`src/runtime_memory.c`), so it knows every allocation the game makes, its size, its memory type
(the PS4 kernel's WC_GARLIC and WB_ONION classes) and its lifetime, at the moment the game asks.
An in-place design can therefore import exactly the GARLIC ranges the game declares, when it
declares them, and nothing else; on this APU host-visible device memory is the same DRAM, so
"import" means the GPU reads the game's bytes where they are. That removes the copies and the
per-page write tracking for those ranges (bbport's constant copies are about 5 MB a frame and its
write faults 90 to 350 a second in the world [O], so the saving is mostly CPU work in
`ObtainBuffer` and the texture path, not bandwidth) [I].

The cost is the one the fork met at its stage 3: a GPU that reads guest memory in place needs the
game's fences written at real GPU completion, not at record time, or the game rewrites bytes the
GPU has not read yet (bbport dropped the same idea for Bloodborne for this reason,
`parallel_gpu.md`). With the game's own pre-submit wait on the previous frame's completion
(PLAN-TO-60-REV2 §2), honest fences make the frame at least "command building plus GPU execution
of the previous frame", which is the console's own pipeline and is fine if both fit in 16.7 ms.
What to measure before deciding: GPU execution time a frame on this seat (`BB_GPU_PROFILE=1`),
the exact allocation table the game makes under this runtime (sizes, types, when), and how much
of each stage's per-draw cost is copies and tracking. Design only after those three numbers.

## 6. What to measure first

With the baseline run of the plan's 5.1 (in progress as this is written):

- per-stage `perf` profiles at the stock field of view, moving;
- counts per frame: pipeline binds, descriptor writes, vertex binds, barriers, pass ends, layout
  transitions, submits, command buffers (the GPU profile labels give the last three);
- how often consecutive draws share a pipeline (run lengths) and how often the specialisation
  inputs are unchanged;
- GPU milliseconds a frame by label, with a profiler-off control.

### First numbers (2026-10-05 evening, stock field of view, moving; PLAN-TO-60-REV2 appendix A) [O]

- The Vulkan recorder is **59% spin**: its real work is about 0.5 µs a draw, almost all inside
  RADV (`radv_CmdSetVertexInputEXT` 7.2% of the thread, descriptor set update 2.4%, push
  descriptor init 1.6%). Section 2's "about 1.3 µs" for it was spin. The recorder is not a limit.
- Stage B is 11% spin and 67 to 74% busy; its largest single item is `memmove` (11.7%: the
  register blocks of each packet and the copies into stream buffers), then `BindTextures` 7.2,
  `Pipeline::BindResources` 5.6, `BeginRendering` 4.6, **mutexes 6%** (`pthread_mutex_lock` 3.7
  and unlock 2.3), `ResolveVertexBuffers` 3.1, `GetVertexInputs` 2.5, `MakeBeginSignature` 2.2.
- `amdgpu_vamgr_find_va` is 2.8% of stage B and 5.0% of the recorder: Vulkan memory is allocated
  on the hot path every frame. Who allocates (staging blocks, stream buffer growth, image or
  buffer creation) is the first thing to find; allocation belongs in a pool, not in a frame.
- The command thread spends 17.4% in the counter-rule drains (about 1,000 waited drains a frame:
  the rule keeps stage A within three groups of stage B, so the two run in lockstep), then
  decode 9.5, hand-over 7.7, specialisation 7.3 + 3.4, constant engine 4.9, ring bindings 4.8,
  register packets 4.6 + 3.9, sharps 4.3, pipeline lookup 3.8 + 2.1 + 1.8 + 1.7. Constants are
  already copied at decode for buffers up to 16 KiB that are not GPU-written
  (`CollectRingBindings`, `STREAM_THRESHOLD`), so item 1 of section 5 is mostly in place; what
  the drains still protect is stage B's own guest reads (vertex and index buffers, textures,
  buffers over 16 KiB or GPU-modified). An exact rule, "a dump waits only for the stage-B reads
  it overlaps" (stage A records the ranges it leaves to stage B with their pipe position, as it
  already does for pending writes), would replace both the counter rule and the reset rule with
  bookkeeping that needs no model of the console's counters, and would let stage A run a whole
  submission ahead. Candidate for the plan's 5.4, under the version check.
- At this field of view the frame rate is set by the tick handshake, not by per-draw cost
  (appendix A.4 of the plan); per-draw cost sets how much margin there is under the tick.

### Call-graph attribution (an Opus reader on the seat, 6 s per thread, dwarf unwinding, in the world, moving; 2026-10-05 13:00) [O, second-hand with paths]

- **Recording thread, `memmove` 11.25%**: 4.92% vertex uploads and 2.33% index uploads, both
  `CopySparseSerial` ← `MemoryManager::CopySparseMemory` ← `BufferCache::ObtainBuffer`
  (`buffer_cache.cpp:318`, the synchronous path: map, copy, commit) ← `ResolveVertexBuffers` /
  `ResolveIndexBuffer`; 2.78% the game's own `DMA_DATA` guest-to-guest copies run in order on
  this thread (`RunDmaData` → `CopyBuffer` → `memcpy`, both ends in the guest memfd mapping);
  0.98% `Scheduler::RecordData` copies (vertex-input bindings 0.59, descriptor data 0.39). No
  samples from the register-block copy.
- **Recording thread, mutexes 5.37%**: `Scheduler::pending_ops_mutex` 1.88% (`PopPendingOperations`
  on every draw, once the atomic early-out fails), `TextureCache::samplers_mutex` 1.79%
  (`GetSampler` ← `BindSamplers`), the texture cache's main mutex about 1.1% (`FindImage`),
  `recorder_mutex` 0.40%. No buffer-cache or memory-tracker mutex above 0.02%.
- **`amdgpu_vamgr_find_va`**: every sample is a new GPU buffer object
  (`radv_amdgpu_winsys_bo_create`). Recording thread 5.00%: `vkAllocateMemory` ←
  `BufferCache::EnsureResident` (`buffer_cache.cpp:465`, one `VkDeviceMemory` per residency step)
  ← `ObtainBuffer` ← `ResolveVertexBuffers` 4.33% / `ResolveIndexBuffer` 0.67%. Vulkan recorder
  9.97%: RADV growing its command stream (`radv_amdgpu_cs_grow` ← `radv_CmdDrawIndexed` 9.06%,
  `radv_CmdDraw` 0.86%); 210 `renderD128` mappings in 6 s on that thread. A long virtual-address
  hole list from the residency churn is the likely reason each search is slow [I]. **The
  recording thread also made about 960 read-only mappings of the guest memfd a second**; who,
  is not determined (`CopySparseSerial` itself does not map).
- **Command thread**: `DrainDrawPipeTo` 17.61%, all from the constant-engine coroutine resumed at
  `WaitOnCeCounter` (the counter rule's drains, removed by patch 16); then `ProcessGraphics`
  8.36, `PostDraw` 7.66, `StageSpecialization` constructor 7.62 (← `GetProgram`),
  `ProcessCeUpdate` 5.02, `CollectRingBindings` 4.89 + 1.49, `ApplyGraphicsRegisterPacket` 4.61,
  `GetSharp` 4.17, `HashRegisterPacket` 3.88, `GetProgram` 3.45, `operator==` 3.32.
- **Vulkan recorder**: 53% self time, 98% of it the idle spin; RADV entry points 42.8% in all:
  `radv_CmdDrawIndexed` 17.96 (9.06 of it the address search above), push descriptors 7.10,
  `radv_CmdSetVertexInputEXT` 7.06, `radv_CmdBindPipeline` 2.01, `radv_CmdDraw` 1.96,
  `radv_CmdBeginRendering` 1.94, bind vertex buffers 1.24, push constants 0.97.

### Result of patch group 16 (exact dump rule) and the design that replaces it

Built and tried 2026-10-05 evening (PLAN-TO-60-REV2 appendix A.9): with the counter rule replaced
by "a dump waits only for a tracked read the recording thread still has" (buffers over 16 KiB,
index data, DMA sources, indirect arguments), **no dump ever waited**, the picture flashed, the
command thread ran ahead until it blocked on GPU completion a third of the time, and the flip
rate fell. So the reads the counter rule protects are vertex streams or textures that the game
feeds through constant-engine dumps, and emulating the constant engine on the decode thread
makes the two stages run in near-lockstep by the game's own design (a wait within a few groups
of the draw engine). The rule stays off (toggle bit 60 set).

The replacement that keeps the exactness and frees the command thread: **copy at decode
whatever the dumps feed**. Constants up to 16 KiB already are (`CollectRingBindings`). Next:
stage A keeps the set of recently dumped granules with the position of the dump; when a draw's
vertex streams (from the fetch shader's attributes and the user data, as `GetVertexInputs` does
on stage B) or its index range overlap a recent dump and are small enough, stage A copies them
into its ring and the packet carries the ring offsets, so the recording thread binds the copy
and never reads dumped memory; then the dump needs no wait at all for those classes. Textures
fed by dumps, if any, remain the drained class. Cost: the fetch-shader walk on stage A
(estimated 0.1 to 0.2 µs a draw) and the copies (the dumped data is small by construction:
214 to 268 bytes a dump).

What this changes in section 5's order: the residency allocation (one device allocation per
step, on the draw path, and the address-space fragmentation it leaves for the driver) moves to
the top as the cheapest certain win; the per-draw lock in `PopPendingOperations` and the sampler
lock are small and mechanical; the vertex and index copies (7.3% of the recording thread) are
the in-place import's case (section 5a) and nothing smaller; the DMA copies are the game's own
work and stay.

## 7. Questions for the handoff

1. For the Xbox layer's figure: answered above from its own records (orbit and menus uncapped;
   60 capped in the world on an RTX 3080; 13 to 19 in the world on this seat). If the operator
   has seen an uncapped in-world run, its log is the missing piece.
2. Whether the fork's draw-memo hit rates for Destiny (how much of a frame repeats the previous
   one) were recorded; they size items 2 and 4 above without a new experiment.
3. Any measured per-command cost of RADV on this class of GPU for push descriptors against
   descriptor buffers; otherwise item 3 is measured here.

Sources used in section 3: [zeux, Writing an efficient Vulkan renderer](https://zeux.io/2020/02/27/writing-an-efficient-vulkan-renderer/);
[AMD RDNA Performance Guide](https://gpuopen.com/learn/rdna-performance-guide/);
[AMD Vulkan samples, descriptor management](https://gpuopen-librariesandsdks.github.io/Vulkan-Samples/samples/performance/descriptor_management/);
[Phoronix, RADV merges VK_EXT_device_generated_commands](https://www.phoronix.com/news/RADV-VK-EXT-DGC);
[WinDurango](https://github.com/WinDurango/WinDurango) for what the Xbox layer is; the fork's
`documents/perf/ASSESSMENT-20260912-command-processor.md`, `PLAN-20260912-cp-round2.md` and
`RESEARCH-20260911-where-the-frame-waits.md`; bbport's `docs/parallel_gpu.md`.
