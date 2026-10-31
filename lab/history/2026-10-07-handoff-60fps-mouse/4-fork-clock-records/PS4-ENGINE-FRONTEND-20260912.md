# PS4 engine frontend: recovered binary contract

This is a static implementation specification against the actual 01.29 eboot, derived from the Xbox executable's D3D11 boundary. It is not a working renderer patch or a performance result. The accompanying JSON pins the image and entry bytes; `verify-ps4-renderer-contract.py` checks them without booting the game.

## Correspondence established

These are semantic counterparts, not a claim that every instruction or argument is identical across the two engine revisions.

| Operation | Xbox RVA | PS4 vaddr | Evidence in PS4 |
|---|---|---|---|
| Vertex stream binding | `0x823a20` | `0x7dcfa0` | Decodes the same 13-bit-index resource-handle scheme, obtains buffer address, constructs fetch descriptors from input-layout attributes. Caches handle and byte offset per stream. |
| Index binding | `0x823ae0` | `0x7dd1b0` | Resolves resource handle; caches handle at context `+0x1d68`, resource record at `+0x1d70`; emits index type, count and base. |
| Indexed draw | `0x825cd0` | `0x7df950` | Context flag, topology lookup, bound index resource, start-index address calculation, prepare-state call, indexed packet emission. |
| Nonindexed draw | `0x825d40` | `0x7dfb40` | Context flag, topology lookup, prepare-state call, automatic-index packet emission. |
| Finish/publish recording | `0x822e10` | `0x7dccd0` | Finalizes command spans; publishes ready flag atomically; releases the context's recording pointers. |
| Consume prepared work | fragment `0x822f48` | `0x7db3e0` | Tests the next queue entry's ready flag; consumes the entry in order and submits its recorded command buffers. |

Static direct xrefs: vertex bind 46 sites, index bind 20, indexed draw 9, nonindexed draw 40. These counts are call-site counts, not runtime coverage. The simple PS4 caller `0x7c3400` independently confirms the sequence: index bind at `0x7c342d`, vertex bind at `0x7c3449`, indexed draw at `0x7c3476` with index count 36.

## Engine context and ABI

PS4 uses the SysV register convention. Xbox disassembly uses the Microsoft x64 convention. Port argument meaning, not register numbers.

| PS4 engine-context field | Meaning established from instructions |
|---|---|
| `+0x26` byte | Draw gate used by both simple draw wrappers. |
| `+0x50` pointer | Active graphics command/state object. |
| `+0x58` pointer | Active submission entry during recording. |
| `+0x1d60` pointer | Vertex input-layout data consumed by vertex binding. |
| `+0x1d68` u32 | Cached index resource handle. |
| `+0x1d70` pointer | Resolved index resource record. |
| `+0x1d78 + stream*4` u32 | Cached vertex resource handle. |
| `+0x1df8 + stream*4` u32 | Cached vertex byte offset. |

The index record supplies base address at `+0x30`, index type at `+0x38`, and count at `+0x3c`. The indexed draw computes `base + (2*index_type + 2)*start_index` before emission. This gives 2/4-byte elements for type 0/1. It passes the draw count from R8D; ECX supplies start index. ESI selects topology. The role of EDX and remaining arguments is not established by this wrapper, which does not consume EDX.

The nonindexed wrapper takes engine context in RDI, topology enum in ESI, and count in EDX. It does not consume an explicit start-vertex argument. Do not assume its ABI exactly matches the Xbox wrapper.

Vertex binding takes context RDI, handle ESI, stream in low 16 bits of EDX, byte offset ECX, and a count override R8D. It short-circuits when cached handle and offset match. The layout provides stride at `layout + 4 + stream*4`; each attribute identifies its stream and element offset. Resource `+0x28` supplies the base address and `+0x30` supplies the default count. Successful binding writes both cache fields.

The leaf at `0x7dcef0` changes the layout pointer and invalidates all 32 handle/offset cache entries to `0xffffffff`. A host binding cache must observe that transition as well as map/residency changes.

## The existing state-preparation boundary

All 13 confirmed direct callers of `0xfc8530` lie in the engine graphics region. The simple draw wrappers call it with:

```
graphics = *(engine_context + 0x50)
RDI = graphics + 0xc0
RSI = graphics
RDX = graphics + 0x80
call 0xfc8530
emit draw
call/tailcall 0xfc8e50
```

The pre-draw helper checks dirty fields before emitting shader setup, including `sceGnmSetVsShader` through `0xfb9f10`. The post-draw leaf clears byte `[RDI+0xba8]` and tailcalls `0xfbb600`. Those side effects cannot simply be dropped.

The vertex-descriptor setter `0xfc5bd0` is particularly useful: it already receives `(state, stage, first_slot, count, descriptor_pointer)` in RDI/ESI/EDX/ECX/R8. It copies `16*count` bytes into the state backing and sets per-slot and per-group dirty bits. For its observed buffer-descriptor path:

```
backing = *(state + 8)
word_index = uint16_t(stage*0x642 + first_slot*4 + 0x4c0)
destination = backing + word_index*4
slot_dirty_byte = state + 0xcd + stage*4 + slot/8
slot_dirty_bit = 1 << (slot & 7)
```

The vertex binder calls this with `state = graphics+0xc0`, stage 2, attribute slot, count 1. This establishes a structured descriptor-update boundary and actual dirty metadata in the PS4 binary. It does not establish that all descriptor classes use this setter or that these masks remain dirty after pre-draw processing. Capture before the consuming/resetting operation, or maintain independent host generations from the setters.

Low-level emitters independently confirm draw semantics:

- `0xfb97b0`: SET_UCONFIG_REG packet, register offset `0x242`, topology value masked to 6 bits.
- `0xfb9830`: packet `0xc0012d00` (DRAW_INDEX_AUTO), supplied count, source value 2.
- `0xfb98f0`: packet `0xc0042700` (DRAW_INDEX_2), supplied count and index address.

## Recording ownership and ordered handoff

PS4 `0x7dce10` initializes an entry with ready flag `[entry+0x18]=0`, installs its pointer into the owner's 64-entry ring at `owner+0xc8+(producer&0x3f)*8`, then advances producer field `+0x2c8`. This publishes a reservation, not finished work.

PS4 `0x7dccd0` finishes the active buffers, atomically sets `[entry+0x18]=1`, and clears both engine pointers `+0x50/+0x58`. The consumer `0x7db3e0` checks that flag at `0x7db50b` before consuming the ring entry and advancing consumer field `+0x2cc`. The normal submission branch calls `sceGnmSubmitCommandBuffers` through `0xfbed10` at `0x7db59a`. Flip and label paths remain part of this consumer.

This is the PS4 counterpart to Xbox's ready-list protocol. Recording can preserve the engine's own context and entry ownership. Static inspection alone does not establish producer thread count or achievable scaling.

## Implementation target

Build the host frontend around the recovered recording entry, not around frame boundaries. Binding/setter hooks maintain host-owned state per guest recording context. Draw interception captures an immutable operation referencing that state version. Finish seals the entry's host stream; the existing ordered consumer makes it available to the renderer. Keep renderer work on its permitted threads; a guest hook must not call the current Rasterizer directly from an arbitrary guest worker.

The existing Vulkan backend remains responsible for shader translation, pipeline selection, resource residency, texture handling and GPU synchronization. The frontend supplies known bindings and draws instead of forcing the CP to reconstruct those covered operations from PM4.

For an incremental implementation, associate host operations with the exact command-buffer ranges and positions they replace. Preserve surrounding DMA, barriers, queries, compute and labels through the current parser. Execute a covered draw exactly once. A live engine-context pointer is not a sufficient queued record: that context is reused after finish.

Start with the nonindexed wrapper `0x7dfb40` and its complete state-preparation interval; add the indexed wrapper using the verified bound-index record. The necessary acceptance result is identical rendering for covered draws, exact operation ordering and a measured removal of their binding reconstruction/packet work. A callback that merely logs a draw or queues it while leaving the original draw executing is not the performance implementation.

## Remaining implementation work

The draw/buffer/list correspondence is established. Complete shader, texture, sampler, render-target and dynamic-state capture is still required before bypassing their reconstruction for a rendered pass. The existing pre-draw helper and structured setters are the next source of those fields. No claim of 60 fps, complete interception coverage, or completed emulator integration is made by this static contract.
