// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! ⭐⭐ The **Activity-Host payload codec** - `FUN_002A4090` (decode) / `FUN_002A4140` (encode).
//!
//! This module closes the gate `d1-activity-host`'s [`codec`](../../d1_activity_host/codec/index.html)
//! module named and quarantined: *"udt 3 (join_request) is schema-serialised on the wire; the codec
//! at FUN_002A4090 is NOT IMPLEMENTED."* It is implemented now, and it is implemented by **running
//! the client's own codec**, so nothing here is a reimplementation that can drift.
//!
//! # §1 The finding that made this cheap: `FUN_002A4090` **is** the object codec
//!
//! The decompile of `0x2A4090` is eleven lines, and every one of them is already in
//! [`crate::object`]:
//!
//! ```text
//! ulong FUN_002a4090(u32 tag, void *wire, u64 wire_len, void *out) {
//!     bs[0x14] = 1;                      // alignment 1        <- object.rs `bitstream()`
//!     FUN_00072240(bs, wire, wire_len);  // BS_CTOR
//!     FUN_00072400(bs);                  // BS_BEGIN_READ
//!     rc = FUN_002C9000(tag, bs, out, out, 1);   // ⭐ DECODE_OBJ, same five args, same order
//!     FUN_00072650(bs);                  // BS_END
//!     return rc & (bs[0x24] <= bs[0x10] * 8);    // <- object.rs `OracleError::ShortPayload`
//! }
//! ```
//!
//! and `0x2A4140` (the encoder half, reached from `activity_client_send_message` `0x299310`) is
//! `BS_CTOR(bs, buf, 0x1DF00); BS_BEGIN_WRITE(bs, 1); FUN_002C8FF0(tag, obj, bs, 1); BS_FLUSH(bs)`
//! - [`crate::object::ObjectOracle::encode`], argument for argument.
//!
//! ⇒ **The AH wire format is the QueueZ object format with a different descriptor tag.** The only
//! thing that was ever missing was *which tag*. `[C]`
//!
//! # §2 How the tags are recovered - five static steps, no guessing
//!
//! Each AH message names its schema through a **runtime-filled** global: `0x299310` is called as
//! `send(ac, target, udt, *DAT_0190xxxx, struct, struct_size, out)` and `FUN_01058020` as
//! `FUN_002A4090(*DAT_019022F8, wire, len, out)`. Those globals live in `.data` and hold a pointer
//! to a **BSS** record whose `+0x00` is the tag - so a naive static read gets zero. The tag is
//! recoverable anyway:
//!
//! 1. `DAT_0190xxxx` is itself an `R_X86_64_RELATIVE` slot → the **runtime record** in BSS
//!    (`0x19022F0 -> 0x2681CA0`).
//! 2. Exactly one *other* relocation slot points at that same record - its entry in a **sorted
//!    checksum table** (`0x1912CB0`), 16 bytes per entry: `{ptr, u32 checksum, u32 pad}`.
//! 3. Read the checksum at `table_slot + 8` (`0x6085E283`).
//! 4. Byte-scan the image for that `u32`. There are exactly two hits: the table entry itself, and
//!    `static_record + 8` - the *static* schema record (`0x136EF88`). This is
//!    [`crate::object`]'s own §"(family, type) → SCHEMA TAG map" method, reused.
//! 5. `static_record + 0x50` is the descriptor **anchor**, and `d1-eboot`'s index maps anchor → tag
//!    (`0x136EFD8 -> 0x80800E98`).
//!
//! ⚠ `static_record + 0x60` is the same word as `anchor + 0x10`, which is where
//! [`d1_eboot::descriptors::index`] reads the tag. Checking one against the other is **not** an
//! independent control and this module does not pretend it is. The two real controls are §3.
//!
//! # §3 The controls, and why they can fail
//!
//! * **26 size agreements.** The struct size in each derived descriptor must equal the size the
//!   client's *own call site* passes to `activity_client_send_message` - a number read out of a
//!   different place in the binary by a different person. `0x100` for udt 3, `0x1A8` for udt 4,
//!   `0xB1E0` for udt 37. One wrong tag is one wrong size.
//! * ⭐⭐ **Execution against real client bytes.** On 2026-08-08 the live client sent a **238-byte
//!   udt-3 `join_request`** to our Activity Host, and its *own* log line said what was in it
//!   ([`CAPTURED_JOIN_REQUEST`], [`CAPTURED_*`](CAPTURED_NONCE)). Decoding those bytes with the
//!   derived tag must reproduce every one of those values, and re-encoding the decoded struct must
//!   reproduce the **238 bytes byte for byte**. That control cannot pass by accident and it is not
//!   an algebraic identity - it is the client's encoder and our decoder meeting on bytes neither
//!   produced.
//! * **A negative that must keep failing**: the same payload under `0x80800FD5` - a *different*
//!   descriptor with the *same* `struct_size` of `0x100` - must NOT reproduce the nonce. Without it,
//!   "the size matched" would be doing all the work.
//!
//! # §4 What is still `[U]`
//!
//! * The **meaning** of most join_request fields. The oracle proves shape, never semantics.
//! * `udt 1 global_activity_state`: the descriptor says the struct is **`0x6B0` = 1712 bytes
//!   total**, while `d1-activity-host::messages` models it as `8 + 0x6B0` from `0xBAFB50`'s
//!   `memcpy(world+0x50B0, payload+8, 0x6B0)`. Those cannot both be the AH struct. The descriptor
//!   is authoritative *for the wire*; `0xBAFB50` is reading a simulation-queue element, which is a
//!   different object. Flagged rather than silently reconciled.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use d1_eboot::descriptors::{self, Index};
use d1_eboot::image::Image;
use d1_eboot::reloc;

use crate::error::{OracleError, Result};
use crate::object::{Decoded, Encoded, ObjectOracle};

/// `FUN_002A4090` - the AH receive path's decode wrapper. Wraps [`crate::object::DECODE_OBJ`].
pub const DECODE_FN: u64 = 0x2A_4090;
/// `FUN_002A4140` - the AH send path's encode wrapper. Wraps [`crate::object::ENCODE_OBJ`].
pub const ENCODE_FN: u64 = 0x2A_4140;
/// `activity_client_send_message`, `0x299310` - the call site every SEND tag was read from.
pub const SEND_MESSAGE_FN: u64 = 0x29_9310;
/// The scratch buffer `0x2A4140` hands the bitstream. Reused verbatim so our encode cannot run out
/// of room in a place the client's would not.
pub const ENCODE_CAPACITY: u64 = 0x1DF00;

/// `{descriptor global, +0x08} = descriptor object`; the tag is at `+0x00` of the same record.
const RECORD_ANCHOR_OFFSET: u64 = 0x50;
/// `record + 0x08` is the instance checksum - the key the sorted lookup table is sorted on.
const RECORD_CHECKSUM_OFFSET: u64 = 0x08;
/// `{ptr, u32 checksum, u32 pad}`.
const TABLE_ENTRY_CHECKSUM_OFFSET: u64 = 0x08;

/// ⭐ Every Activity-Host message whose descriptor global we have read out of a call site, with the
/// **struct size that call site passes**. `(user_defined_type, descriptor_global, struct_size)`.
///
/// The three `RECV` rows come from the receive handlers (`FUN_010579F0`, `FUN_01057B80`,
/// `FUN_01058020`); every other row is a `FUN_00299310` call site. The sizes are the control:
/// they are stated by the client at a place unrelated to the schema record.
///
/// ⚠ `struct_size` is `None` where the call site does not state one (the receive handlers state a
/// stack-buffer size, which is rounded up, not the struct size). Only udt 0 and 4 state exact ones.
pub const CALL_SITES: &[(u32, u64, Option<usize>)] = &[
    // ── receive handlers ─────────────────────────────────────────────────────────────────────
    (0, 0x0190_2310, Some(0x600)),  // entity_slots_allocated  FUN_010579F0 memset(buf, 0, 0x600)
    (1, 0x0190_2380, None),         // global_activity_state    FUN_01057B80
    (4, 0x0190_22F8, Some(0x1A8)),  // join_result              FUN_01058020 local_1e0[424]
    // ⭐ udt 11 `replicate_membership`. Its handler is `FUN_0029CF50 -> FUN_0029C730`, which states
    // the size TWICE and in two different ways - `local_528c = 0x5038` (the buffer size it hands
    // the codec) and `memcpy(image, ac+0xE70, 0x5038)` (the current state it seeds with) - so the
    // 0x5038 below is a size agreement from a place unrelated to the schema record, exactly like
    // the send-site rows.
    (11, 0x0190_2278, Some(0x5038)), // replicate_membership     FUN_0029C730 *DAT_01902278
    // ── send sites, `FUN_00299310(ac, target, udt, *global, struct, struct_size, out)` ────────
    (3, 0x0190_22F0, Some(0x100)),  // join_request             FUN_0029A170
    (12, 0x0190_22E0, Some(0x330)), // FUN_00299D60
    (13, 0x0190_22E8, Some(0x010)), // FUN_00299F90
    (17, 0x0190_2398, Some(0x008)), // c_activity_client::vfunc_5
    (18, 0x0190_22D0, Some(0x2CC)), // c_activity_client::vfunc_8
    (19, 0x0190_2308, Some(0x004)), // allocate_entity_indices  FUN_0029B240
    (20, 0x0190_2318, Some(0x600)), // free_entity_indices      FUN_0029B2E0
    (22, 0x0190_2268, Some(0x028)), // client_identity_update   FUN_0029CA40
    (25, 0x0190_2320, Some(0x608)), // FUN_0029B440
    (26, 0x0190_2340, Some(0x604)), // FUN_0029B530
    (28, 0x0190_2358, Some(0x004)), // FUN_0029CF60
    (30, 0x0190_2368, Some(0x608)), // FUN_0029CFE0
    (31, 0x0190_2370, Some(0x604)), // FUN_0029D0B0
    (32, 0x0190_23A0, Some(0x604)), // FUN_0029B5D0
    (33, 0x0190_23B8, Some(0x210)), // FUN_0029A910
    (35, 0x0190_23A8, Some(0x008)), // connectivity_failure     FUN_0029AD50
    (36, 0x0190_2390, Some(0x004)), // membership_acknowledgement FUN_0029AFF0
    (37, 0x0190_23B0, Some(0xB1E0)), // FUN_0029A9C0
    (41, 0x0190_2378, Some(0x004)), // FUN_0029D170
    (44, 0x0190_23D0, Some(0x284)), // FUN_0029D200
    (45, 0x0190_23D8, Some(0x170)), // FUN_0029D280
    (46, 0x0190_23C0, Some(0x00C)), // FUN_0029D350
    (47, 0x0190_23E0, Some(0x024)), // FUN_00298DC0 / FUN_00298770
    (48, 0x0190_23E8, Some(0x004)), // FUN_0029B1A0
    // ⭐ 2026-08-09, from the rungs-31→36 live session. Both were named gates in run
    // `20260809-054417-RUNG33-BYPASS` and both had a call site nobody had read.
    //
    // udt 14 `peer_leave_request`: `FUN_0029A610` calls
    // `FUN_00299310(ac, target, 0xe, *DAT_01902300, local_50, 0x14, out)`. The captured payload was
    // **20 bytes** and `0x14` is 20 - a size agreement from the wire as well as from the call site.
    (14, 0x0190_2300, Some(0x014)),
    // ⭐ udt 21 `client_authoritative_data_update`. It does **not** go through `FUN_00299310` at
    // all: `FUN_0029CC20` builds the bitstream itself and calls `messaging->vfunc[6]` (`0x594550`)
    // directly, so a scan restricted to `FUN_00299310` call sites could never find it. The
    // descriptor global is the third argument of its encoder,
    // `FUN_002C9290(ac+0x6748, ac+0x6968, *DAT_01902270, bs, ac+0x6B88, 1)`.
    //
    // The size is stated twice, in two unrelated places, and neither of them is the schema record:
    //   * `FUN_002987E0`'s success branch is `memcpy(ac+0x6748, ac+0x6968, 0x220)` - it copies the
    //     just-sent image over the snapshot.
    //   * the two struct pointers are `param_1 + 0xce9` and `param_1 + 0xd2d` on a `long*`, i.e.
    //     `ac+0x6748` and `ac+0x6968`, and `0x6968 - 0x6748 = 0x220`.
    (21, 0x0190_2270, Some(0x220)),
];

/// ⭐⭐ The Activity-Host messages the client sends with **no schema at all**.
///
/// `activity_client_send_message` (`FUN_00299310`) has two arms, chosen by its **seventh**
/// argument - a `byte*` whose bit 0 the function tests first:
///
/// ```c
/// bool FUN_00299310(ac, target, udt, u32 descriptor, void *obj, u32 size, byte *flags) {
///     if ((*flags & 1) == 0) {                              // ← the SCHEMA arm
///         FUN_002A4140(descriptor, obj, buf, &len);         //   encode through the descriptor
///         send(messaging, target, udt, buf, len, ac+0x6C6C);
///     } else {                                              // ← the RAW arm
///         send(messaging, target, udt, obj, size, ac+0x6C6C);   //   the struct image, verbatim
///     }
/// }
/// ```
///
/// The three call sites below pass `descriptor = 0xFFFFFFFF` **and** set the flag, so the bytes on
/// the wire are the struct image and there is no descriptor anywhere in the picture:
///
/// | udt | site | flag | descriptor | size |
/// |---:|---|---|---|---:|
/// | 6 `sensor_sense_update` | `c_activity_client::vfunc[4]` `0x2992B0` | `local_20[0] = 1` | `0xFFFFFFFF` | caller's |
/// | 8 `request_activity_host` | `FUN_002996D0` | `local_c58[0] = 1` | `0xFFFFFFFF` | `0x9D8` |
/// | **15 `activity_client_keepalive_request`** | `FUN_00298C40` | `local_170 = 1` | `0xFFFFFFFF` | **1** |
///
/// ⇒ ⛔ **udt 15's schema tag is not "unpinned", it does not exist.** Telling a reader to "read its
/// `FUN_00299310` call site and add a `CALL_SITES` row" is advice that cannot be followed: the call
/// site is read, and what it says is `0xFFFFFFFF`. The one byte it sends is
/// `&local_169` - a stack local `FUN_00298C40` never writes - so its *value* is uninitialised
/// stack and carries no information either.
pub const RAW_SEND: &[(u32, u64, &str)] = &[
    (6, 0x0029_92B0, "c_activity_client::vfunc[4] - sensor_sense_update"),
    (8, 0x0029_96D0, "FUN_002996D0 - request_activity_host, 0x9D8 bytes"),
    (15, 0x0029_8C40, "FUN_00298C40 - the keepalive, ONE uninitialised byte"),
];

/// ⭐ Build 56578's [`CALL_SITES`], read on 2026-10-13 from the 56578 image alone.
///
/// Each row was read mechanically at a call site, not by offsetting the 01.29 globals:
/// * the 28 callers of `activity_client_send_message` (`0x299D10`), where `edx` is the udt, the
///   descriptor comes from `lea rax,[rip+G]; mov rax,[rax]; mov ecx,[rax]`, and `r9d` is the size;
/// * the 12 callers of the AH decode wrapper (`0x2A4AC0`), where the descriptor is in `edi`;
/// * udt 11 from the replicated decode `0x2C9AE0` at `0x29D1D5`, with the descriptor in `ecx`;
/// * udt 21 from the replicated encode `0x2C9D20` at `0x29D6A4`, with the descriptor in `rdx`.
///
/// Every udt in the 01.29 table is present with the same role, and every global moved by the
/// same `+0x18090`. Every size the client states is unchanged **except udt 37**
/// `send_client_heartbeat`, which grew from `0xB1E0` to **`0xB750`**. udt 21's `0x220` is still
/// stated by the snapshot `memcpy(ac+0x6748, ac+0x6968, 0x220)` at `0x299420`. The sizes remain
/// the control, as in 01.29: [`derive`] reaches the schema record by an unrelated route, and
/// [`controls`] compares the two.
pub const CALL_SITES_56578: &[(u32, u64, Option<usize>)] = &[
    (0, 0x0191_A3A0, Some(0x600)),
    (1, 0x0191_A410, None),
    // world_globals_state: dispatch entry2 -> 1018D00, 8-byte body.
    // Queue 2409E0 -> BB2660 -> 2A1F40 -> 2A2CA0 copies it to world+14.
    (2, 0x0191_A418, Some(0x008)),
    (4, 0x0191_A388, Some(0x1A8)),
    (11, 0x0191_A308, Some(0x5038)),
    (3, 0x0191_A380, Some(0x100)),
    (12, 0x0191_A370, Some(0x330)),
    (13, 0x0191_A378, Some(0x010)),
    (17, 0x0191_A428, Some(0x008)),
    (18, 0x0191_A360, Some(0x2CC)),
    (19, 0x0191_A398, Some(0x004)),
    (20, 0x0191_A3A8, Some(0x600)),
    (22, 0x0191_A2F8, Some(0x028)),
    (25, 0x0191_A3B0, Some(0x608)),
    (26, 0x0191_A3D0, Some(0x604)),
    (28, 0x0191_A3E8, Some(0x004)),
    (30, 0x0191_A3F8, Some(0x608)),
    (31, 0x0191_A400, Some(0x604)),
    (32, 0x0191_A430, Some(0x604)),
    (33, 0x0191_A448, Some(0x210)),
    (35, 0x0191_A438, Some(0x008)),
    (36, 0x0191_A420, Some(0x004)),
    (37, 0x0191_A440, Some(0xB750)),
    (41, 0x0191_A408, Some(0x004)),
    (44, 0x0191_A460, Some(0x284)),
    (45, 0x0191_A468, Some(0x170)),
    (46, 0x0191_A450, Some(0x00C)),
    (47, 0x0191_A470, Some(0x024)),
    (48, 0x0191_A478, Some(0x004)),
    (14, 0x0191_A390, Some(0x014)),
    (21, 0x0191_A300, Some(0x220)),
];

/// Build 56578's [`RAW_SEND`]. The same three udts pass descriptor `0xFFFFFFFF` to `0x299D10`, with
/// the same sizes (udt 15 sends 1 byte, udt 8 sends `0x9D8`).
pub const RAW_SEND_56578: &[(u32, u64, &str)] = &[
    (6, 0x0029_9CB0, "c_activity_client::vfunc[4] - sensor_sense_update"),
    (8, 0x0029_A0D0, "FUN_0029A0D0 - request_activity_host, 0x9D8 bytes"),
    (15, 0x0029_9640, "FUN_00299640 - the keepalive, ONE uninitialised byte"),
];

/// The selected build's codec set. Panics with the reason when the image has none: every caller
/// below is meaningless without one.
fn addrs() -> &'static crate::codec::CodecAddrs {
    crate::codec::selected().unwrap_or_else(|e| panic!("{e}"))
}

/// The selected build's call-site rows ([`CALL_SITES`] on 01.29, [`CALL_SITES_56578`] on 56578).
pub fn call_sites() -> &'static [(u32, u64, Option<usize>)] {
    addrs().ah_call_sites
}

/// Is this udt sent with the raw arm of `activity_client_send_message`? See [`RAW_SEND`].
pub fn is_raw_send(user_defined_type: u32) -> Option<(u64, &'static str)> {
    addrs().ah_raw_send.iter().find(|(u, _, _)| *u == user_defined_type).map(|(_, a, w)| (*a, *w))
}

/// ⭐ Descriptor field kind **0x22** - a *dynamic flags* bitset, and the one field kind this oracle
/// cannot run.
///
/// Two Activity-Host messages consist of exactly one such field and nothing else - udt **33**
/// `process_debug_command` (`0x80800A3D`, `0x210`) and udt **37** `send_client_heartbeat`
/// (`0x80800D06`, `0xB1E0`). Decoding either faults, and the fault is **not** about the message:
///
/// ```text
/// 0x1065831  and eax, 0x3fff ; cmp eax, 0x3ffe ; je …     ← the bitset is not already external
/// 0x1065845  call 0x2C6BA0                                ← grow it
/// 0x106584A  mov  rax, [r14]                              ← the backing array pointer
/// 0x1065850  mov  edx, [rax+rcx*4]                        ⛔ READ_UNMAPPED
/// ```
///
/// `0x2C6BA0` allocates the backing array out of the game's **"dynamic flags" pool** -
/// `FUN_000B0550(&DAT_01ABCB80 + DAT_02666D18[n] * DAT_01ABCB90 + DAT_01ABCB80)`, whose own
/// diagnostic is `"out of dynamic flags '%s' pool"`. Those three globals are populated by the
/// game's allocator bring-up. A statically-mapped image has them zeroed, so the pool hands back a
/// pointer into nothing and the first index read faults.
///
/// ⇒ **This is a harness limitation with a named cause, not an unknown wire format.** It is also
/// inert: 33 and 37 are both client → AH with no client-side handler, so nothing an Activity Host
/// does depends on decoding them.
pub const FIELD_KIND_DYNAMIC_FLAGS: u32 = 0x22;

/// The instruction that faults, kept as a constant so the refusal message can name it and a reader
/// can go and look. `mov edx, [rax+rcx*4]` inside `FUN_010657C0`, one instruction after the grow.
pub const DYNAMIC_FLAGS_FAULT_RIP: u64 = 0x0106_5850;
/// `thunk_FUN_002C6840` - the grow that allocates out of the pool that does not exist here.
pub const DYNAMIC_FLAGS_GROW_FN: u64 = 0x002C_6BA0;

/// Whether a tag's top-level fields include a [`FIELD_KIND_DYNAMIC_FLAGS`] one, and if so why that
/// is fatal here. Memoised through [`dynamic_flags_tags`].
pub fn dynamic_flags_reason(tag: u32) -> Result<Option<String>> {
    if !dynamic_flags_tags()?.contains(&tag) {
        return Ok(None);
    }
    let a = addrs();
    let grow = a.dynamic_flags_grow_fn;
    let fault = match a.dynamic_flags_fault_rip {
        Some(rip) => format!("{rip:#x} faults READ_UNMAPPED"),
        None => "the first index read faults READ_UNMAPPED".to_string(),
    };
    Ok(Some(format!(
        "has a field of kind {FIELD_KIND_DYNAMIC_FLAGS:#04x} - a DYNAMIC FLAGS bitset. Its backing \
         array is allocated by {grow:#x} out of the game's \"dynamic flags\" pool \
         (01.29: FUN_000B0550 over DAT_01ABCB80/DAT_01ABCB90/DAT_02666D18), which only exists in a \
         live process; in a statically mapped image the pool returns a pointer into nothing and \
         {fault}. ⇒ a HARNESS limitation with a named \
         cause, not an unknown wire format - and inert, because every udt in this class is \
         client→AH with no client-side handler."
    )))
}

/// Every [`CALL_SITES`] tag whose descriptor carries a dynamic-flags field, derived from the
/// descriptor tree rather than listed. Run once per process.
pub fn dynamic_flags_tags() -> Result<&'static BTreeSet<u32>> {
    static T: OnceLock<std::result::Result<BTreeSet<u32>, String>> = OnceLock::new();
    match T.get_or_init(|| {
        let image = Image::open_default().map_err(|e| e.to_string())?;
        let index = descriptors::index(&image).map_err(|e| e.to_string())?;
        let schemas = schemas().as_ref().map_err(|e| e.clone())?;
        Ok(schemas
            .values()
            .filter(|s| {
                index.get(s.tag).is_some_and(|d| {
                    descriptors::fields(&image, d.anchor, d.field_count)
                        .into_iter()
                        .filter_map(|(_, f)| f)
                        .any(|f| f.kind == FIELD_KIND_DYNAMIC_FLAGS)
                })
            })
            .map(|s| s.tag)
            .collect())
    }) {
        Ok(s) => Ok(s),
        Err(e) => Err(OracleError::Eboot(e.clone())),
    }
}

/// Which udts are refused for the reason above. Derived, never listed.
pub fn dynamic_flags_udts() -> Result<Vec<u32>> {
    let tags = dynamic_flags_tags()?;
    let schemas = schemas().as_ref().map_err(|e| OracleError::Eboot(e.clone()))?;
    Ok(schemas.values().filter(|s| tags.contains(&s.tag)).map(|s| s.user_defined_type).collect())
}

/// One AH message's schema, and the whole chain that produced it - so a reader can re-walk every
/// step by hand with `d1-eboot reloc --slot` and `d1-eboot descriptors --tag`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AhSchema {
    pub user_defined_type: u32,
    /// `DAT_0190xxxx`, the `.data` slot the call site dereferences.
    pub descriptor_global: u64,
    /// Where that slot's relocation points: the BSS record filled at runtime.
    pub runtime_record: u64,
    /// The one other relocation slot aiming at the same record - its sorted-table entry.
    pub table_slot: u64,
    /// `table_slot + 8`, the instance checksum.
    pub checksum: u32,
    /// The static schema record found by scanning for that checksum.
    pub static_record: u64,
    /// `static_record + 0x50`, which is what `d1-eboot descriptors` calls the anchor.
    pub anchor: u64,
    /// ⭐ The schema tag. This is the thing that was missing.
    pub tag: u32,
    pub struct_size: usize,
    pub field_count: u32,
    /// The size the client's own call site passes, where it states one. The control.
    pub call_site_size: Option<usize>,
}

/// Derive one message's schema. See §2 for each step.
pub fn derive(
    image: &Image,
    index: &Index,
    user_defined_type: u32,
    descriptor_global: u64,
    call_site_size: Option<usize>,
) -> Result<AhSchema> {
    let eb = |e: d1_eboot::image::ImageError| OracleError::Eboot(e.to_string());

    let by_slot = reloc::by_slot(image).map_err(|e| OracleError::Eboot(e.to_string()))?;
    let by_addend = reloc::by_addend(image).map_err(|e| OracleError::Eboot(e.to_string()))?;

    let runtime_record = *by_slot.get(&descriptor_global).ok_or_else(|| {
        OracleError::Shape(format!(
            "{descriptor_global:#x} carries no R_X86_64_RELATIVE - it is not a descriptor global in \
             this image"
        ))
    })?;

    let mut others: Vec<u64> = by_addend
        .get(&runtime_record)
        .map(|v| v.iter().copied().filter(|s| *s != descriptor_global).collect())
        .unwrap_or_default();
    others.sort_unstable();
    if others.len() != 1 {
        return Err(OracleError::Shape(format!(
            "udt {user_defined_type}: record {runtime_record:#x} has {} sorted-table slots, expected \
             exactly 1 - the table shape assumed in §2 does not hold here",
            others.len()
        )));
    }
    let table_slot = others[0];
    let checksum = image.u32(table_slot + TABLE_ENTRY_CHECKSUM_OFFSET).map_err(eb)?;

    let hits = image.find_all(&checksum.to_le_bytes());
    let statics: Vec<u64> = hits
        .iter()
        .copied()
        .filter(|h| *h != table_slot + TABLE_ENTRY_CHECKSUM_OFFSET)
        .map(|h| h - RECORD_CHECKSUM_OFFSET)
        .collect();
    if statics.len() != 1 {
        return Err(OracleError::Shape(format!(
            "udt {user_defined_type}: checksum {checksum:#010x} occurs at {} places other than its \
             table entry; a unique static record is the premise of this derivation",
            statics.len()
        )));
    }
    let static_record = statics[0];
    let anchor = static_record + RECORD_ANCHOR_OFFSET;

    let d = index
        .iter()
        .map(|(_, d)| d)
        .find(|d| d.anchor == anchor)
        .ok_or_else(|| {
            OracleError::Shape(format!(
                "udt {user_defined_type}: {anchor:#x} is not an anchored descriptor - the record \
                 layout (anchor at +0x50) does not hold here"
            ))
        })?;

    Ok(AhSchema {
        user_defined_type,
        descriptor_global,
        runtime_record,
        table_slot,
        checksum,
        static_record,
        anchor,
        tag: d.tag,
        struct_size: d.struct_size as usize,
        field_count: d.field_count,
        call_site_size,
    })
}

/// Derive every row of [`CALL_SITES`].
pub fn derive_all(image: &Image, index: &Index) -> Result<Vec<AhSchema>> {
    let sites = crate::codec::codec_for(&image.sha256())
        .ok_or_else(|| {
            OracleError::Eboot(format!(
                "no Activity-Host call-site table for image sha256 {}",
                image.sha256()
            ))
        })?
        .ah_call_sites;
    sites
        .iter()
        .map(|(udt, g, size)| derive(image, index, *udt, *g, *size))
        .collect()
}

static SCHEMAS: OnceLock<std::result::Result<BTreeMap<u32, AhSchema>, String>> = OnceLock::new();

/// The derivation, run at most once per process. It is a pure function of one pinned image.
pub fn schemas() -> &'static std::result::Result<BTreeMap<u32, AhSchema>, String> {
    SCHEMAS.get_or_init(|| {
        let image = Image::open_default().map_err(|e| e.to_string())?;
        let index = descriptors::index(&image).map_err(|e| e.to_string())?;
        let all = derive_all(&image, &index).map_err(|e| e.to_string())?;
        Ok(all.into_iter().map(|s| (s.user_defined_type, s)).collect())
    })
}

/// The schema tag for a `user_defined_type`, or an error naming what is missing.
///
/// ⚠ Only the udts in [`CALL_SITES`] resolve. The other 22 of the 49 have no call site we have
/// read, and returning "no tag" for them is the point: **an unpinned udt must refuse, not
/// improvise.**
pub fn tag_for_udt(user_defined_type: u32) -> Result<u32> {
    match schemas() {
        Ok(m) => m.get(&user_defined_type).map(|s| s.tag).ok_or_else(|| {
            // ⭐ Three different reasons an id has no tag, and they demand three different actions.
            // Collapsing them into "UNPINNED - go read the call site" sent a session looking for a
            // descriptor that does not exist. See [`RAW_SEND`].
            if let Some((site, what)) = is_raw_send(user_defined_type) {
                OracleError::Shape(format!(
                    "udt {user_defined_type} is sent RAW: {what} calls FUN_00299310 at {site:#x} \
                     with descriptor 0xFFFFFFFF and the raw flag set, so the payload IS the struct \
                     image and THERE IS NO SCHEMA TAG TO PIN. Do not add a CALL_SITES row - read \
                     the bytes as they are."
                ))
            } else {
                OracleError::Shape(format!(
                    "udt {user_defined_type} has no descriptor global in CALL_SITES - its schema \
                     tag is UNPINNED. Read its `FUN_00299310` call site (or its receive handler, or \
                     - as udt 21 turned out to be - its direct `messaging->vfunc[6]` encoder) and \
                     add the row; do not guess a tag from a size match."
                ))
            }
        }),
        Err(e) => Err(OracleError::Eboot(e.clone())),
    }
}

/// The Activity-Host payload codec: one [`ObjectOracle`] per `user_defined_type`, built on demand.
///
/// ⚠ **Not `Send`.** It owns a Unicorn instance. A server that wants one must give it a thread -
/// see `d1_activity_host::codec::OracleSchemaCodec`.
#[derive(Default)]
pub struct AhCodec {
    oracles: BTreeMap<u32, ObjectOracle>,
    /// ⛔ Always `true` in a server. See [`AhCodec::set_rewind_enabled`].
    rewind_disabled: bool,
}

impl AhCodec {
    pub fn new() -> Self {
        Self::default()
    }

    /// ⛔⛔ **The leak control's inversion knob. Never call this in a server.** Applies to every
    /// oracle already built and to every one built afterwards. See
    /// [`SchemaHarness::set_rewind_enabled`](crate::schema::SchemaHarness::set_rewind_enabled).
    pub fn set_rewind_enabled(&mut self, on: bool) {
        self.rewind_disabled = !on;
        for o in self.oracles.values_mut() {
            o.set_rewind_enabled(on);
        }
    }

    fn oracle(&mut self, user_defined_type: u32) -> Result<&mut ObjectOracle> {
        if !self.oracles.contains_key(&user_defined_type) {
            let tag = tag_for_udt(user_defined_type)?;
            // ⭐ Refuse a dynamic-flags descriptor here, with its cause, rather than letting it
            // surface as `unicorn: READ_UNMAPPED calling 0x2c9000` - which reads like a codec bug
            // and is a missing runtime allocator. See [`FIELD_KIND_DYNAMIC_FLAGS`].
            if let Some(detail) = dynamic_flags_reason(tag)? {
                return Err(OracleError::Shape(format!(
                    "udt {user_defined_type} (tag {tag:#010x}) {detail}"
                )));
            }
            // `ObjectOracle::new` runs the pinned object control first and refuses otherwise.
            let mut o = ObjectOracle::new(tag)?;
            if self.rewind_disabled {
                o.set_rewind_enabled(false);
            }
            self.oracles.insert(user_defined_type, o);
        }
        Ok(self.oracles.get_mut(&user_defined_type).expect("just inserted"))
    }

    /// The struct size the client's decoder allocates for this message.
    pub fn struct_size(&mut self, user_defined_type: u32) -> Result<usize> {
        Ok(self.oracle(user_defined_type)?.struct_size())
    }

    /// Decode an AH wire payload exactly as `FUN_002A4090` does.
    ///
    /// ⚠ An under-length payload is [`OracleError::ShortPayload`], which is precisely the condition
    /// `0x2A4090`'s own return value ANDs in (`bs[0x24] <= bs[0x10] * 8`). Our error and the
    /// client's `false` are the same predicate.
    pub fn decode(&mut self, user_defined_type: u32, wire: &[u8]) -> Result<Decoded> {
        self.decode_seeded(user_defined_type, wire, &[])
    }

    /// Decode, honouring the **replicated** wrapper where the client uses one.
    ///
    /// `seed` is what `FUN_0029C730` calls the *current* image: a replicated body may carry only
    /// what changed, so decoding it against zeros yields "everything this message asserted, and
    /// zeros for everything it did not". That is honest for telemetry and wrong for authoritative
    /// state - which is why the seed is a parameter rather than an assumption.
    pub fn decode_seeded(
        &mut self,
        user_defined_type: u32,
        wire: &[u8],
        seed: &[u8],
    ) -> Result<Decoded> {
        if is_replicated(user_defined_type) {
            return self.oracle(user_defined_type)?.decode_replicated(wire, seed);
        }
        if !seed.is_empty() {
            return Err(OracleError::Shape(format!(
                "udt {user_defined_type} is not replicated - a seed image is meaningless for it, \
                 and accepting one silently would hide a caller's wrong assumption"
            )));
        }
        self.oracle(user_defined_type)?.decode(wire)
    }

    /// Encode a struct image exactly as `FUN_002A4140` does.
    ///
    /// The image must be **exactly** the schema's struct size. Truncation would be a silent
    /// half-message and padding would put invented bytes in fields the client reads.
    pub fn encode(&mut self, user_defined_type: u32, struct_image: &[u8]) -> Result<Encoded> {
        let o = self.oracle(user_defined_type)?;
        let want = o.struct_size();
        if struct_image.len() != want {
            return Err(OracleError::Shape(format!(
                "udt {user_defined_type}: struct image is {} bytes, the schema's struct is {want} \
                 ({want:#x}). Refusing to truncate or pad - either would put bytes the client reads \
                 into fields nobody wrote.",
                struct_image.len()
            )));
        }
        if is_replicated(user_defined_type) {
            return o.encode_replicated(struct_image, ENCODE_CAPACITY);
        }
        o.encode(struct_image, ENCODE_CAPACITY)
    }

    /// ⭐ Per-udt bump-arena accounting: `(udt, scaffolding, currently used, high water, guest
    /// allocations)`, all in bytes except the last.
    ///
    /// This exists because the failure it watches is **silent and time-delayed**. Run
    /// `20260809-120451-MOONBASE-FRESH` encoded `global_activity_state` 132 times over sixteen
    /// minutes and the 133rd died with `scratch heap exhausted`; the client then reported
    /// `error.private_activity_host_host_crashed`, which reads as *the client rejected us* rather
    /// than *our encoder ran out of memory*. A host that can print these numbers can see the
    /// difference before the run dies.
    pub fn arena_stats(&self) -> Vec<ArenaStats> {
        self.oracles
            .iter()
            .map(|(&user_defined_type, o)| ArenaStats {
                user_defined_type,
                scaffolding: o.scaffolding_bytes(),
                used: o.heap_used(),
                high_water: o.heap_high_water(),
                guest_allocs: o.guest_allocs(),
            })
            .collect()
    }

    /// The udts this codec has built an oracle for.
    pub fn warm_udts(&self) -> Vec<u32> {
        self.oracles.keys().copied().collect()
    }
}

/// One oracle's bump-arena accounting. See [`AhCodec::arena_stats`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArenaStats {
    pub user_defined_type: u32,
    /// Permanent scaffolding, allocated once at construction.
    pub scaffolding: u64,
    /// Committed right now.
    pub used: u64,
    /// ⭐ The furthest the arena has ever reached. **Flat ⇒ no leak.**
    pub high_water: u64,
    /// Allocations made by the guest's own code. Must be zero for the rewind to be sound.
    pub guest_allocs: u64,
}

impl ArenaStats {
    /// Bytes still available before [`crate::harness::HEAP_SIZE`] is exhausted.
    pub fn headroom(&self) -> u64 {
        crate::harness::HEAP_SIZE.saturating_sub(self.used)
    }

    /// One line for a run log.
    pub fn line(&self) -> String {
        format!(
            "udt {:>2}: scaffolding {:#x}  used {:#x}  high-water {:#x} of {:#x}  guest allocs {}",
            self.user_defined_type,
            self.scaffolding,
            self.used,
            self.high_water,
            crate::harness::HEAP_SIZE,
            self.guest_allocs
        )
    }
}

/// ⭐ Which Activity-Host messages carry the extra **leading presence bit**, i.e. go through
/// `FUN_002C9050`/`FUN_002C9290` rather than `FUN_002A4090`/`FUN_002A4140`.
///
/// A *whitelist by handler*, never a guess. Adding a udt here without reading its handler would
/// silently prepend a bit to a message the client codes without one - and a one-bit shift is not an
/// error anywhere, it is garbage that parses.
///
/// | udt | the read that puts it here |
/// |---:|---|
/// | 11 `replicate_membership` | `0x29C730` is the one AH→client handler in the 17-entry dispatch table that calls `FUN_002C9050`; the other three establish-client handlers (`0x1058020`, `0x10579F0`, `0x1057B80`) all call `FUN_002A4090` |
/// | 21 `client_authoritative_data_update` | `FUN_0029CC20`'s encoder is **`FUN_002C9290`** - the presence-bit writer - not `FUN_002A4140`, and it passes the client's own dirty mask `ac+0x6B88` as the mask argument |
pub fn is_replicated(user_defined_type: u32) -> bool {
    matches!(user_defined_type, 11 | 21)
}

// ── the captured vector: a real client's real join_request ───────────────────────────────────────

/// ⭐⭐ **238 bytes of `user_defined_type` 3 `join_request`, sent by the live client on 2026-08-08**
/// to our Activity Host, recovered from `runs/20260808-195418-F2-STATE3/responder.log`'s BAP
/// message-8 `SECURED` line (`msg_type: 8, sequence: 3`).
///
/// The message-8 frame is `{u16be type}{u32be seq}{u64be ah_id}{u32le 1}{u32le udt}{u32le len}{u8}`
/// then these 238 bytes. Nothing about the payload was reconstructed - it is the tail of the frame.
pub const CAPTURED_JOIN_REQUEST: &str = concat!(
    "4db6999fd100000000000a017e8289bc78000000925daffaa019d538d100000000000001d100000000000201536df06aca76",
    "5d1a9f4fd7bb7ec2e2c2e6cac2cadb7ec6c6d6d2cac37ec2c2c2c2c2c703bfcb8b0312c71fd787cb93a787bb02df16e2cae2",
    "e70b0edee2c2c202020202020202020202020202020202020202020202020202020202020202020202020202020202020202",
    "020202020202020202020202020202020202020202020202020202020202039bf607ea05cacbf30000000000000000800000",
    "000000000054db7c1ab29d9746b0000000380000001bf4144de3c000000017dc000000000000",
);

/// What the client's **own** log line said was in it:
///
/// ```text
/// networking:activity_client: … Sent join request: [AH %s][nonce %d]
///   identity:[account %lld][character %lld][machine %s][player %s]
/// [AH D1000000:00000A01][nonce 1303812511] identity:[account -3386706919782612991]
///   [character -3386706919782612479][machine xx:xx:xx:xx:xx:xx:0000]
///   [player xx:xx:xx:xx:xx:xx:D5:38]
/// … Properties [session Sun_08092026_115420_000001@orb@D1Guardian@7E8289BC7800] …
/// ```
///
/// The struct offsets those `%`s read are `FUN_0029A170`'s: `param_2[0]` nonce, `param_2+2` AH id,
/// `param_2+4` machine, `param_2+6` player, `param_2+8` account, `param_2+10` character,
/// `(char*)param_2 + 0x39` session - with `param_2` typed `undefined4 *`, so `+0x00, +0x08, +0x10,
/// +0x18, +0x20, +0x28, +0x39`.
pub const CAPTURED_NONCE: u32 = 1_303_812_511; // 0x4DB6999F
/// See [`CAPTURED_NONCE`].
pub const CAPTURED_AH_ID: u64 = 0xD100_0000_0000_0A01;
/// See [`CAPTURED_NONCE`]. `-3386706919782612991` as printed by `%lld`.
pub const CAPTURED_ACCOUNT: u64 = 0xD100_0000_0000_0001;
/// See [`CAPTURED_NONCE`]. `-3386706919782612479` as printed by `%lld`.
pub const CAPTURED_CHARACTER: u64 = 0xD100_0000_0000_0201;
/// `machine xx:xx:xx:xx:xx:xx:0000` - six bytes then two, little-endian in the struct.
pub const CAPTURED_MACHINE: [u8; 8] = [0x7E, 0x82, 0x89, 0xBC, 0x78, 0x00, 0x00, 0x00];
/// `player xx:xx:xx:xx:xx:xx:D5:38`.
pub const CAPTURED_PLAYER: [u8; 8] = [0x92, 0x5D, 0xAF, 0xFA, 0xA0, 0x19, 0xD5, 0x38];
/// The `Properties [session %s]` half of the same pair of log lines, at struct `+0x39`.
pub const CAPTURED_SESSION: &str = "Sun_08092026_115420_000001@orb@D1Guardian@7E8289BC7800";
/// Bits the client's decoder consumes from [`CAPTURED_JOIN_REQUEST`]. 238 bytes carry 1904, so the
/// payload is the encoder's own output rounded up to a byte, with **one** bit of padding.
pub const CAPTURED_BITS: u32 = 1903;

/// ⚠ **The negative control's tag.** `0x80800FD5` is a *different* descriptor with the *same*
/// `struct_size` of `0x100`. If the captured payload decoded to the right nonce under this too,
/// the size agreement in §3 would be doing no work at all.
pub const DECOY_TAG_SAME_STRUCT_SIZE: u32 = 0x8080_0FD5;

/// Struct offsets in `join_request` (tag `0x80800E98`), from `FUN_0029A170`'s format arguments and
/// confirmed field-for-field by the descriptor walk.
pub mod join_request {
    pub const NONCE: usize = 0x00;
    pub const AH_ID: usize = 0x08;
    pub const MACHINE: usize = 0x10;
    pub const PLAYER: usize = 0x18;
    pub const ACCOUNT: usize = 0x20;
    pub const CHARACTER: usize = 0x28;
    /// Field `[3]` is `0x80800B1F`, the first 184 bytes of a membership member's
    /// `0x80800C56` properties field.  A solo membership must echo this alongside the identity:
    /// `fireteam_from_player()` reads its fireteam identifier at [`FIRETEAM_ID`].
    pub const PROPERTIES: usize = 0x38;
    pub const PROPERTIES_SIZE: usize = 0xB8;
    /// `0x80800B1F` field `[5]`, a 64-bit fireteam identifier.
    pub const FIRETEAM_ID: usize = PROPERTIES + 0xA0;
    /// ⚠ Odd offset. `0x80800B1F`'s field `[1]` is at `+0x001` of a sub-struct based at `+0x38`.
    pub const SESSION: usize = 0x39;
    /// `0x80804056` - 128 bytes, NUL included.
    pub const SESSION_CAPACITY: usize = 128;
}

/// Struct offsets in `join_result` (tag `0x80800A58`). Field `[0]` is the **whole join_request
/// struct embedded at +0x00**, which is why the nonce and AH id are where they are.
pub mod join_result {
    /// The embedded `join_request`, `0x100` bytes, tag `0x80800E98`.
    pub const EMBEDDED_REQUEST: usize = 0x000;
    /// 3 bits. `0` = accepted.
    pub const RESULT: usize = 0x100;
    pub const SESSION_ID: usize = 0x101;
    /// ⚠ **128, not 139.** `d1-activity-host::messages` had `0x18C - 0x101 = 139`; the descriptor
    /// says the char array is `0x80804056` = 128 bytes, so `+0x181..+0x184` is padding and
    /// `+0x184`/`+0x188` are separate `u32` fields a long session id would corrupt.
    pub const SESSION_ID_CAPACITY: usize = 128;
    pub const FIELD_184: usize = 0x184;
    pub const FIELD_188: usize = 0x188;
    pub const ACTIVITY_HOST_CHANGED: usize = 0x18C;
    pub const SIMULATION_QUEUE: usize = 0x190;
    pub const REPLICATION_EPOCH: usize = 0x194;
    pub const GRACE_MS: usize = 0x196;
    pub const KEEPALIVE_MS: usize = 0x198;
}

/// Struct offsets in `global_activity_state` (udt 1, tag `0x80800E76`).
///
/// ⭐⭐ **The activity NAME lives here**, and rungs 31 and 33 need it: `FUN_01057B80` sets
/// `ac+0x130 |= 0x40` **only on a successful decode** of this message (verified in the decompile:
/// the `or` is inside `0x1057B80`'s body, on the `cVar3 != '\0'` branch), and that bit is what
/// rung 31 reads. So udt 1 is load-bearing three times over - establish-client, the activity name,
/// and the rung-33 world transition - and a stub that never sends it stalls in a way that looks
/// exactly like a codec failure.
pub mod global_activity_state {
    /// The outer tag's single field is the real struct, `0x80800B8F`.
    pub const INNER_TAG: u32 = 0x8080_0B8F;
    /// ⚠ `[R]` from another session's trace, **not verified at a consumer here**: the client
    /// requires this byte to be **non-zero**. It is field `[0]`, `kind 0x2` (bool), at `+0x000`.
    /// Two more bools follow at `+0x001` and `+0x002`, semantics `[U]`.
    pub const FLAG_0: usize = 0x000;
    /// ⭐⭐ `[C]` The **activity INDEX** - a `u16` at `+0x680`, and the field rung 33 gates on.
    ///
    /// `FUN_0078C770` is four instructions of substance:
    /// `world = FUN_00762C00(); return *(u16*)(FUN_002A1F50(world) + 0x680)`, and `FUN_002A1F50`
    /// is a one-line leaf, `return param_1 + 0x50B0` - i.e. **this blob**. Rung 33's
    /// `FUN_007A7420` passes that u16 straight to `FUN_0054F410(idx)` (the unlock gate) *and*
    /// to `activity_table[idx]` for the name it prints.
    ///
    /// ⇒ ⭐ Sending 0 here makes the client evaluate **`activity_table[0]`**, which is
    /// `HASH(0x8AE8DA69)` = 2,330,516,073 = **"Tower"** in the Bungie manifest - a social space
    /// whose unlock expression reads flags `{3389, 308, 419, 440}` and is unmet on a fresh
    /// account. That is the whole of `activity_failed_to_load_missing_dlc`: the client was
    /// correctly refusing an activity **we named**, not the one anyone selected.
    /// (`historicaldocs/design/rung33-unlock-gate.md` §8/§10 measured the consequence; this is the field.)
    ///
    /// **The structural control:** `0x80800CC5` field `[7] @+0x620` is a `kind 0x4` **16-bit
    /// leaf**, and `0x060 + 0x620 = 0x680` - the exact width and offset `FUN_0078C770` reads,
    /// derived from a table that has never seen that function. Its neighbour `[10] @+0x628` is
    /// [`NAME`]. **The index and the name are adjacent fields of one struct, and neither is
    /// derived from the other.**
    pub const ACTIVITY_INDEX: usize = 0x680;
    /// `[C]` derived: `0x80800B8F` field `[9]` at `+0x060` → `0x80800CC5`, whose field `[10]` at
    /// `+0x628` → `0x8080409F` - a 40-byte char array, the same shape as the session-id arrays.
    /// `0x060 + 0x628 = 0x688`.
    pub const NAME: usize = 0x688;
    /// `0x28`, NUL included. ⭐ `0x688 + 0x28 == 0x6B0` **exactly**, i.e. the name is the struct's
    /// last field and it tiles the struct to its end. That is the structural control on the layout.
    pub const NAME_CAPACITY: usize = 0x28;
}

/// ⭐⭐ Struct offsets in `replicate_membership` (udt **11**, tag `0x808009ED`, struct `0x5038`).
///
/// # The gate this closes, and the mistake it corrects
///
/// This module used to refuse udt 11, on the reading *"its RX handler `0x29CDE0` never calls
/// `FUN_002A4090` - it calls `FUN_002D2090` with a `0x2800` buffer and seed `0x811C9DC5`, a
/// different format."* ⛔ **That was the wrong handler.** The receive dispatch table `0x18F84A0`
/// is `{i32 type; i32 pad; fnptr}` and the client **linear-scans it comparing the stored type** -
/// it is *not* dense over the sorted AH→client id list, and reading index 7 as udt 11 is what went
/// wrong. The stored types, read straight out of `.data`:
///
/// ```text
/// idx 3 -> type  4 fn 0x1058020      idx 6 -> type  5 fn 0x29D3F0
/// idx 4 -> type 11 fn 0x29CF50  ⭐   idx 7 -> type  7 fn 0x29CDE0   (sensor_message)
/// idx 5 -> type 10 fn 0x1058150      idx 8 -> type 18 fn 0x1058180
/// ```
///
/// 11 sits **before** 10, so the list is not sorted and an index-to-udt mapping cannot be assumed.
/// `0x29CDE0` is `sensor_message`; the `FUN_002D2090`/`0x811C9DC5` finding is true **of udt 7**.
/// Independently corroborated by `activity-host-contract.md` §2, which matched `11
/// replicate_membership ↔ 0x29C730` from the handler's own diagnostic string, with no table
/// involved.
///
/// ⇒ udt 11 **is** schema-coded, by the same object codec, with one extra leading presence bit.
/// See [`crate::object::DECODE_OBJ_REPLICATED`].
///
/// # The layout, and where each offset is read
///
/// The descriptor (`d1-eboot descriptors --tag 0x808009ed`) gives seven fields; the two that matter
/// are cross-checked against `FUN_0029FE70`, the client's *"which membership slot am I?"* search,
/// which indexes the same bytes with hand-written arithmetic:
///
/// ```c
/// // FUN_0029FE70(long *p), p = ac+0xE68 - the membership OBJECT, whose image starts at +8
/// while ((*(uint *)((long)p + 0x5034) >> (i & 0x1f) & 1) == 0) ...   // occupancy mask
/// ... p[(long)(int)i * 0x4a + 2] != *p ...                           // member[i].machine_id
/// ```
///
/// `0x5034 - 8 = 0x502C` = field `[5]`; `0x4a * 8 = 0x250` = the member stride; and
/// `(i*0x4a + 2)*8 - 8 = i*0x250 + 8` = field `[2]`'s base plus `i` strides. Two derivations, one
/// answer, and the second one never saw the descriptor.
pub mod replicate_membership {
    /// The whole struct. `0x5038` = 20,536 bytes.
    pub const STRUCT_SIZE: usize = 0x5038;
    /// Field `[0]`, a `u32`. ⭐ **This is the `MEM-%d` the client prints** in its own
    /// `[AC … CON-%s EST-%s AH->%llx MEM-%d]` status line: `0x29C730` renders
    /// `FUN_00057E60(ac+0xE70)`, and `FUN_00057E60` is `return *param_1` - the image's first
    /// `u32`. `[U]` whether it is a count, a generation or an id; for one member, `1` is the
    /// answer under every reading.
    pub const MEMBER_COUNT: usize = 0x00;
    /// Field `[1]`, a `u32`. ⚠ `0x29C730` compares it against the **current** image's same field
    /// (`FUN_00078D50` = `return *(u32*)(param_1+4)`) and, **on a difference, zeroes the peer-table
    /// count at `ac+0x64B4`** - the table udt 10's apply built. A fresh client holds `0`, so
    /// sending `0` is the one value that provably disturbs nothing.
    pub const GENERATION: usize = 0x04;
    /// Field `[2]` → `0x808052FE`: the member array.
    pub const MEMBERS: usize = 0x08;
    /// `0x250` = 592 bytes per member (`0x80800A39`), from the descriptor's stride **and** from
    /// `FUN_0029FE70`'s `i * 0x4a` on a `long*`.
    pub const MEMBER_STRIDE: usize = 0x250;
    /// `11840 / 592`. The same 20 that caps `12 request_peer_reservation` and that `0xB70150`
    /// disbands above.
    pub const MEMBER_CAPACITY: usize = 20;
    /// Member field `[0]` → `0x80800C2D`, **40 bytes** - and `0x80800C2D` is udt 22
    /// `client_identity_update`, which is *also* the sub-struct embedded in `join_request` at
    /// `+0x10`. ⇒ a member's identity and the identity the client just sent us are the same 40
    /// bytes, so a solo membership is an **echo**, not a construction.
    pub const MEMBER_IDENTITY: usize = 0x00;
    /// `{u8 machine[8]; u8 player[8]; u64 account; u64 character; u64 fireteam}`.
    pub const IDENTITY_SIZE: usize = 0x28;
    /// Where that same 40 bytes live inside a `join_request` struct image.
    pub const JOIN_REQUEST_IDENTITY: usize = 0x10;
    /// Member field `[1]` is `0x80800C56` at member `+0x28`. Its first field is the exact
    /// `0x80800B1F` properties sub-structure embedded in the join request at `+0x38`.
    pub const MEMBER_PROPERTIES: usize = 0x28;
    pub const PROPERTIES_PREFIX_SIZE: usize = super::join_request::PROPERTIES_SIZE;
    pub const JOIN_REQUEST_PROPERTIES: usize = super::join_request::PROPERTIES;
    /// Membership-image offset read by `fireteam_from_player()` (`FUN_0075DB30`). It returns
    /// this qword for the member whose player identity matched; zero is an invalid fireteam and
    /// suppresses the player-add record in `FUN_00BA3D60`.
    pub const MEMBER_FIRETEAM_ID: usize = MEMBERS + MEMBER_PROPERTIES + 0xA0;
    pub const JOIN_REQUEST_FIRETEAM_ID: usize = super::join_request::FIRETEAM_ID;
    /// Field `[5]`, a 20-bit leaf: **one occupancy bit per member slot**. `FUN_0029FE70` walks
    /// `i = 0..=0x12` testing `mask >> i & 1`, and only a set bit makes it look at that slot when
    /// resolving the local member index.
    pub const OCCUPANCY_MASK: usize = 0x502C;
    /// Field `[4]`, also a 20-bit leaf: **one simulation-machine bit per member slot**.
    /// `FUN_00BA0B10` calls `FUN_002A4B90`, which returns this field, then for every set bit calls
    /// `FUN_002A4A80` and copies the member's first qword (machine id) into the simulation-machine
    /// cache. Leaving this zero preserves the Activity Client membership but makes
    /// `FUN_00BA3D60` see no machine ids and emit no player-add record.
    pub const SIMULATION_OCCUPANCY_MASK: usize = 0x5028;

    /// The struct image for a **solo fireteam: one member, the client itself**, built by echoing
    /// the identity and common properties prefix out of the `join_request` the client sent.
    ///
    /// Nothing here is invented: the 40 identity bytes and 184-byte `0x80800B1F` properties
    /// prefix are the client's own, and both occupancy bits select that same echoed member: one
    /// for `FUN_0029FE70`'s local-member lookup, one for `FUN_00BA0B10`'s simulation snapshot. The
    /// properties copy is load-bearing: its qword at prefix `+0xA0` is the fireteam identifier
    /// `FUN_0075DB30` returns, and `FUN_00BA3D60` emits no player-add record when that value is 0.
    ///
    /// Returns `None` if `join_request_image` is too short to contain both echoed structures.
    pub fn solo(join_request_image: &[u8]) -> Option<Vec<u8>> {
        let id = join_request_image
            .get(JOIN_REQUEST_IDENTITY..JOIN_REQUEST_IDENTITY + IDENTITY_SIZE)?;
        let properties = join_request_image.get(
            JOIN_REQUEST_PROPERTIES..JOIN_REQUEST_PROPERTIES + PROPERTIES_PREFIX_SIZE,
        )?;
        let mut b = vec![0u8; STRUCT_SIZE];
        b[MEMBER_COUNT..MEMBER_COUNT + 4].copy_from_slice(&1u32.to_le_bytes());
        // GENERATION stays 0 - see its doc comment; any other value resets ac+0x64B4.
        let slot = MEMBERS + MEMBER_IDENTITY;
        b[slot..slot + IDENTITY_SIZE].copy_from_slice(id);
        let properties_slot = MEMBERS + MEMBER_PROPERTIES;
        b[properties_slot..properties_slot + PROPERTIES_PREFIX_SIZE].copy_from_slice(properties);
        b[SIMULATION_OCCUPANCY_MASK..SIMULATION_OCCUPANCY_MASK + 4]
            .copy_from_slice(&1u32.to_le_bytes());
        b[OCCUPANCY_MASK..OCCUPANCY_MASK + 4].copy_from_slice(&1u32.to_le_bytes());
        Some(b)
    }
}

/// Re-derive [`replicate_membership`]'s member array from the descriptor tree instead of trusting
/// the constants. Returns `(members_offset, stride, capacity)`.
///
/// The array is expressed as an outer struct (`0x808052FE`) whose whole size is `capacity * stride`
/// and whose single field record states the stride. Deriving it here means a different eboot, or a
/// misread of the record layout, fails a control instead of silently shifting every member.
pub fn derive_membership_array(image: &Image, index: &Index) -> Result<(usize, usize, usize)> {
    let outer = index
        .get(tag_for_udt(11)?)
        .ok_or(OracleError::UnknownTag(0x8080_09ED))?;
    let f = descriptors::fields(image, outer.anchor, outer.field_count)
        .into_iter()
        .filter_map(|(_, f)| f)
        .find(|f| f.offset as usize == replicate_membership::MEMBERS)
        .ok_or_else(|| {
            OracleError::Shape(format!(
                "no field at +{:#x} of the membership struct",
                replicate_membership::MEMBERS
            ))
        })?;
    let array = index.get(f.child_tag).ok_or(OracleError::UnknownTag(f.child_tag))?;
    let elem = descriptors::fields(image, array.anchor, array.field_count)
        .into_iter()
        .filter_map(|(_, f)| f)
        .next()
        .ok_or_else(|| OracleError::Shape("the member array has no element record".into()))?;
    let stride = elem.offset as usize;
    if stride == 0 || array.struct_size as usize % stride != 0 {
        return Err(OracleError::Shape(format!(
            "member stride {stride:#x} does not divide the array's {:#x} - the array-as-struct \
             reading does not hold here",
            array.struct_size
        )));
    }
    Ok((replicate_membership::MEMBERS, stride, array.struct_size as usize / stride))
}

/// Re-derive [`global_activity_state::NAME`] and its capacity from the descriptor tree rather than
/// trusting the constants. Returns `(offset, capacity)`.
pub fn derive_activity_name_field(image: &Image, index: &Index) -> Result<(usize, usize)> {
    let outer = index
        .get(global_activity_state::INNER_TAG)
        .ok_or(OracleError::UnknownTag(global_activity_state::INNER_TAG))?;
    let last = descriptors::fields(image, outer.anchor, outer.field_count)
        .into_iter()
        .filter_map(|(_, f)| f)
        .max_by_key(|f| f.offset)
        .ok_or_else(|| OracleError::Shape("0x80800B8F has no readable fields".into()))?;
    let child = index
        .get(last.child_tag)
        .ok_or(OracleError::UnknownTag(last.child_tag))?;
    let inner_last = descriptors::fields(image, child.anchor, child.field_count)
        .into_iter()
        .filter_map(|(_, f)| f)
        .max_by_key(|f| f.offset)
        .ok_or_else(|| OracleError::Shape("the inner struct has no readable fields".into()))?;
    let name_tag = index
        .get(inner_last.child_tag)
        .ok_or(OracleError::UnknownTag(inner_last.child_tag))?;
    Ok((
        last.offset as usize + inner_last.offset as usize,
        name_tag.struct_size as usize,
    ))
}

// ── the leak control ─────────────────────────────────────────────────────────────────────────────

/// The five `user_defined_type`s an Activity Host drives on the establish path, and the direction
/// each is exercised in. `[3]` is inbound (the captured live `join_request`); the rest are outbound.
pub const HOT_PATH_UDTS: [u32; 5] = [3, 4, 1, 0, 11];

/// One arm's measurement. See [`LeakMeasurement::cycles`] for what a "cycle" is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeakMeasurement {
    /// Full passes over [`HOT_PATH_UDTS`] completed before the arm stopped.
    pub cycles: usize,
    /// `(udt, high water after cycle 1, high water at the end)`, bytes.
    pub high_water: Vec<(u32, u64, u64)>,
    /// Guest-side allocations across the whole arm. **Must be zero** for a rewind to be sound.
    pub guest_allocs: u64,
    /// ⭐⭐ **Did every cycle produce the SAME BYTES as the first?**
    ///
    /// This is the axis a leak fix can quietly break and a leak measurement can never see. Reusing
    /// arena memory means the next encode writes into the previous one's buffer, so a codec that
    /// reads any byte it did not write would emit *different wire bytes on the second call* - a
    /// silent wrong-payload bug strictly worse than the crash it replaced. It holds because
    /// [`crate::harness::Harness::alloc_aligned`] zeroes what it hands out, but "it holds" is a
    /// claim, so it is measured on every cycle.
    pub bytes_stable: bool,
    /// The cycle at which the bytes first differed, if they ever did.
    pub first_divergent_cycle: Option<usize>,
    /// Why it stopped early, if it did.
    pub stopped: Option<String>,
}

impl LeakMeasurement {
    /// Bytes the arena grew after the first cycle - the leak rate per cycle, summed over the udts.
    pub fn growth(&self) -> u64 {
        self.high_water.iter().map(|(_, first, last)| last.saturating_sub(*first)).sum()
    }

    pub fn is_flat(&self) -> bool {
        self.high_water.iter().all(|(_, first, last)| first == last)
    }
}

/// Run `cycles` full passes over [`HOT_PATH_UDTS`] and report what the bump arena did.
///
/// `rewind` is the axis: `true` is the shipping behaviour, `false` restores the pre-2026-08-09 leak
/// verbatim. Both arms are run by [`leak_control`] every time it runs.
pub fn measure_arena(cycles: usize, rewind: bool) -> Result<LeakMeasurement> {
    let mut codec = AhCodec::new();
    codec.set_rewind_enabled(rewind);

    let wire3 = crate::control::hex(CAPTURED_JOIN_REQUEST)?;
    // Outbound images, one per udt, at exactly the schema's struct size. Built once - the point of
    // the loop is the *encoder's* allocation behaviour, not the caller's.
    let mut images: Vec<(u32, Vec<u8>)> = Vec::new();
    for udt in HOT_PATH_UDTS.into_iter().filter(|u| *u != 3) {
        let n = codec.struct_size(udt)?;
        images.push((udt, vec![0u8; n]));
    }

    let mut first: BTreeMap<u32, u64> = BTreeMap::new();
    let mut reference: Option<Vec<Vec<u8>>> = None;
    let mut first_divergent_cycle = None;
    let mut done = 0usize;
    let mut stopped = None;
    for i in 0..cycles {
        let step = (|| -> Result<Vec<Vec<u8>>> {
            let mut out = vec![codec.decode(3, &wire3)?.object];
            for (udt, image) in &images {
                out.push(codec.encode(*udt, image)?.payload);
            }
            Ok(out)
        })();
        let out = match step {
            Ok(o) => o,
            Err(e) => {
                stopped = Some(e.to_string());
                break;
            }
        };
        match &reference {
            None => reference = Some(out),
            Some(r) if *r != out && first_divergent_cycle.is_none() => {
                first_divergent_cycle = Some(i);
            }
            Some(_) => {}
        }
        done = i + 1;
        if i == 0 {
            for s in codec.arena_stats() {
                first.insert(s.user_defined_type, s.high_water);
            }
        }
    }

    let stats = codec.arena_stats();
    Ok(LeakMeasurement {
        cycles: done,
        high_water: stats
            .iter()
            .map(|s| {
                (s.user_defined_type, first.get(&s.user_defined_type).copied().unwrap_or(0), s.high_water)
            })
            .collect(),
        guest_allocs: stats.iter().map(|s| s.guest_allocs).sum(),
        bytes_stable: first_divergent_cycle.is_none(),
        first_divergent_cycle,
        stopped,
    })
}

/// How many `global_activity_state` encodes a **leaking** arena survives, and what it dies of.
/// Returns `(successful encodes, the error the next one produced)`.
///
/// ⭐ This is the run-`20260809-120451-MOONBASE-FRESH` failure reproduced offline in under two
/// seconds. That run's `activity-host.log` has **131** udt-1 `WantsToPush` lines that produced a
/// push and a 132nd immediately followed by `** NAMED GATE ** push blocked: … scratch heap
/// exhausted` - see [`OBSERVED_UDT1_ENCODES_BEFORE_FAULT`].
pub fn measure_udt1_exhaustion(limit: usize) -> Result<(usize, String)> {
    let mut codec = AhCodec::new();
    codec.set_rewind_enabled(false);
    let image = vec![0u8; codec.struct_size(1)?];
    for i in 0..limit {
        if let Err(e) = codec.encode(1, &image) {
            return Ok((i, e.to_string()));
        }
    }
    Ok((limit, String::new()))
}

/// ⭐⭐ **THE LEAK CONTROL. The axis, in words: *this proves the encoder can run unbounded without
/// arena growth.***
///
/// # Why this control exists
///
/// On 2026-08-09, run `20260809-120451-MOONBASE-FRESH` sat in `activity:initial_slice_set_loading`
/// for sixteen minutes, published `global_activity_state` on every 5,125 ms heartbeat - and then
/// stopped. The 133rd encode returned `guest fault: scratch heap exhausted`, the host quietly
/// stopped publishing, and the client eventually declared
/// `error.private_activity_host_host_crashed`. **The observable pointed at the protocol and the
/// cause was our own allocator.**
///
/// 62 of the crate's 70 controls were green throughout, because **every one of them encodes each
/// message once.** A one-shot control cannot see a per-call leak; only a loop can. That is the
/// whole design brief for this function.
///
/// # The two arms, and why both run every time
///
/// * **Positive**: `cycles` full passes over [`HOT_PATH_UDTS`] with the rewind on. The arena's
///   high-water mark must be **identical** after cycle 1 and after cycle N, for every udt, and
///   `guest_allocs` must be zero - the precondition that makes the rewind sound rather than lucky.
/// * ⭐ **Inverted**: the *same loop* with the rewind off must **grow**, and a udt-1-only loop must
///   die with `scratch heap exhausted` within a few encodes of the 133 the live run reached. A
///   control that only ever passes is decoration; this one demonstrates its own failure mode on
///   every invocation.
///
/// `cycles` is the simulated session length: at the observed 5,125 ms heartbeat, 2,000 cycles is
/// **2.8 hours** of `global_activity_state` publishing.
pub fn leak_control(cycles: usize) -> Result<Vec<Control>> {
    let mut c = Vec::new();

    let fixed = measure_arena(cycles, true)?;
    let per_udt = fixed
        .high_water
        .iter()
        .map(|(u, f, l)| format!("udt {u}: {f:#x}→{l:#x}"))
        .collect::<Vec<_>>()
        .join("  ");

    c.push(check(
        "⭐⭐ the arena HIGH-WATER MARK IS FLAT across a multi-hour session's worth of encodes",
        fixed.cycles == cycles && fixed.is_flat() && fixed.stopped.is_none(),
        format!(
            "{} cycles of {:?} - {per_udt}{}",
            fixed.cycles,
            HOT_PATH_UDTS,
            fixed.stopped.as_deref().map(|s| format!("; STOPPED: {s}")).unwrap_or_default()
        ),
    ));

    c.push(check(
        "⭐ the guest's own code allocates NOTHING across a codec call - the rewind's precondition, \
         measured rather than assumed",
        fixed.guest_allocs == 0,
        format!(
            "{} allocations through the alloc_aligned import ({:#x}) over {} cycles; a non-zero \
             count means a guest pointer may outlive the call and the rewind would hand the same \
             bytes out twice",
            fixed.guest_allocs,
            crate::harness::IMP_ALLOC_ALIGNED,
            fixed.cycles
        ),
    ));

    c.push(check(
        "⭐⭐ every cycle emits the IDENTICAL bytes - reusing the arena did not let one encode read \
         another's leftovers",
        fixed.bytes_stable,
        format!(
            "{} cycles, {} udts each, byte-for-byte identical{}. This is the axis a leak fix breaks \
             silently: reused memory that the codec reads without writing would change the WIRE, \
             which is worse than the crash it replaced. It holds because `alloc_aligned` zeroes \
             what it hands out.",
            fixed.cycles,
            HOT_PATH_UDTS.len(),
            fixed
                .first_divergent_cycle
                .map(|i| format!(" - ** DIVERGED AT CYCLE {i} **"))
                .unwrap_or_default()
        ),
    ));

    // ── ⭐ THE INVERSION. Re-introduce the leak and require the control to go red. ─────────────
    let leaky = measure_arena(LEAK_ARM_CYCLES, false)?;
    c.push(check(
        "⭐⭐ INVERTED: with the rewind switched off the SAME loop grows, so a flat high-water mark \
         is a measurement and not a tautology",
        !leaky.is_flat() && leaky.growth() > 0,
        format!(
            "{} cycles leak {} bytes ({:#x}) - {}",
            leaky.cycles,
            leaky.growth(),
            leaky.growth(),
            leaky
                .high_water
                .iter()
                .map(|(u, f, l)| format!("udt {u}: {f:#x}→{l:#x}"))
                .collect::<Vec<_>>()
                .join("  ")
        ),
    ));

    // ── ⭐ THE INVERSION, TIED TO THE OBSERVED RUN ────────────────────────────────────────────
    let (n, why) = measure_udt1_exhaustion(LEAK_ARM_UDT1_LIMIT)?;
    c.push(check(
        "⭐⭐ INVERTED: a leaking udt-1 loop dies of `scratch heap exhausted` on EXACTLY the encode \
         run 20260809-120451-MOONBASE-FRESH died on",
        why.contains("scratch heap exhausted") && n == OBSERVED_UDT1_ENCODES_BEFORE_FAULT,
        format!(
            "{n} encodes succeeded and the next failed; the live run pushed \
             {OBSERVED_UDT1_ENCODES_BEFORE_FAULT} and had its next one blocked. {why}"
        ),
    ));

    // A cheap arithmetic cross-check on the mechanism, from numbers nobody chose: the per-encode
    // cost is dominated by the client's own 0x1DF00 scratch buffer.
    let per_encode = crate::harness::HEAP_SIZE / (n.max(1) as u64);
    c.push(check(
        "the per-encode cost IS the client's own 0x1DF00 encode buffer plus change - the arithmetic \
         that predicts the thirteen-minute lifetime",
        per_encode >= ENCODE_CAPACITY && per_encode < ENCODE_CAPACITY * 2,
        format!(
            "{per_encode:#x} arena bytes per udt-1 encode, of which {ENCODE_CAPACITY:#x} is the \
             buffer FUN_002A4140 hands the bitstream; {:#x} of arena / {per_encode:#x} = {n} \
             encodes, and the live run took {OBSERVED_SECONDS_TO_FAULT} s to make them",
            crate::harness::HEAP_SIZE,
        ),
    ));

    Ok(c)
}

/// Cycles [`controls`] runs the leak arm for. Enough that a per-call leak of *any* size shows up
/// (one leaked byte per cycle still moves the high-water mark), small enough that `controls` stays
/// a few seconds. The long simulation is `d1-oracle activity-host leak --cycles 2000`.
const LEAK_CYCLES_IN_CONTROLS: usize = 50;
/// Cycles the inverted arm runs. It only has to *grow*, so a handful is enough and keeps the
/// control cheap.
const LEAK_ARM_CYCLES: usize = 3;
/// Ceiling on the udt-1 exhaustion arm - comfortably past the observed count so a *failure to
/// exhaust* is reported rather than silently looping.
const LEAK_ARM_UDT1_LIMIT: usize = 400;
/// ⭐ `global_activity_state` encodes that **succeeded** in `runs/20260809-120451-MOONBASE-FRESH`
/// before the first `scratch heap exhausted`, counted from `activity-host.log`: 131 udt-1
/// `WantsToPush` lines produced a push, and the 132nd was followed immediately by
/// `** NAMED GATE ** push blocked`.
pub const OBSERVED_UDT1_ENCODES_BEFORE_FAULT: usize = 131;
/// Wall-clock seconds from the first udt-1 push (`20:06:55.275`) to the first fault
/// (`20:19:55.836`) - thirteen minutes, i.e. **the failure is a function of uptime, not of
/// anything about the activity.**
pub const OBSERVED_SECONDS_TO_FAULT: u64 = 780;

// ── controls ─────────────────────────────────────────────────────────────────────────────────────

pub use crate::profile::Control;

fn ok(name: &'static str, detail: impl Into<String>) -> Control {
    Control { name, pass: true, detail: detail.into() }
}
fn check(name: &'static str, pass: bool, detail: impl Into<String>) -> Control {
    Control { name, pass, detail: detail.into() }
}

/// Every control for this module. **Exit 2 on failure**, including the negative ones.
pub fn controls() -> Result<Vec<Control>> {
    let mut c = Vec::new();

    // ── §2, the derivation ────────────────────────────────────────────────────────────────────
    let image = Image::open_default().map_err(|e| OracleError::Eboot(e.to_string()))?;
    let index = descriptors::index(&image).map_err(|e| OracleError::Eboot(e.to_string()))?;
    let all = derive_all(&image, &index)?;

    c.push(check(
        "every CALL_SITES row derives a schema tag",
        all.len() == addrs().ah_call_sites.len(),
        format!("{} of {} rows", all.len(), addrs().ah_call_sites.len()),
    ));

    let mismatched: Vec<String> = all
        .iter()
        .filter_map(|s| {
            s.call_site_size.filter(|w| *w != s.struct_size).map(|w| {
                format!("udt {} tag {:#010x} struct {:#x} != call site {:#x}",
                        s.user_defined_type, s.tag, s.struct_size, w)
            })
        })
        .collect();
    let stated = all.iter().filter(|s| s.call_site_size.is_some()).count();
    c.push(check(
        "⭐ every derived struct_size equals the size the CLIENT'S OWN call site passes",
        mismatched.is_empty() && stated >= 25,
        if mismatched.is_empty() {
            format!("{stated} independent size agreements")
        } else {
            mismatched.join("; ")
        },
    ));

    let tags: Vec<u32> = all.iter().map(|s| s.tag).collect();
    let mut uniq = tags.clone();
    uniq.sort_unstable();
    uniq.dedup();
    c.push(check(
        "no two Activity-Host messages resolve to the same schema tag",
        uniq.len() == tags.len(),
        format!("{} distinct tags", uniq.len()),
    ));

    let jr = all.iter().find(|s| s.user_defined_type == 3).expect("udt 3 is a CALL_SITES row");
    let jres = all.iter().find(|s| s.user_defined_type == 4).expect("udt 4 is a CALL_SITES row");
    c.push(check(
        "udt 3 join_request = 0x80800E98 and udt 4 join_result = 0x80800A58",
        jr.tag == 0x8080_0E98 && jres.tag == 0x8080_0A58,
        format!("via checksums {:#010x} / {:#010x}", jr.checksum, jres.checksum),
    ));

    // join_result's field [0] is the whole join_request struct. This ties the two tags together
    // structurally, so a wrong udt-4 tag cannot survive a correct udt-3 tag.
    let embedded = descriptors::fields(&image, jres.anchor, jres.field_count)
        .first()
        .and_then(|(_, f)| *f)
        .map(|f| (f.offset, f.child_tag));
    c.push(check(
        "join_result's field [0] IS the join_request struct, at +0x00",
        embedded == Some((0, jr.tag)),
        format!("{embedded:?} vs expected (0, {:#010x})", jr.tag),
    ));

    // ── §3, execution against real client bytes ───────────────────────────────────────────────
    let wire = crate::control::hex(CAPTURED_JOIN_REQUEST)?;
    c.push(check(
        "the captured join_request is 238 bytes",
        wire.len() == 238,
        format!("{} bytes", wire.len()),
    ));

    let mut codec = AhCodec::new();
    let d = codec.decode(3, &wire)?;
    let s = &d.object;
    let q = |o: usize| u64::from_le_bytes(s[o..o + 8].try_into().unwrap());
    let session_end = s[join_request::SESSION..].iter().position(|b| *b == 0).unwrap_or(0);
    let session = String::from_utf8_lossy(
        &s[join_request::SESSION..join_request::SESSION + session_end],
    )
    .to_string();

    let fields_ok = u32::from_le_bytes(s[0..4].try_into().unwrap()) == CAPTURED_NONCE
        && q(join_request::AH_ID) == CAPTURED_AH_ID
        && s[join_request::MACHINE..join_request::MACHINE + 8] == CAPTURED_MACHINE
        && s[join_request::PLAYER..join_request::PLAYER + 8] == CAPTURED_PLAYER
        && q(join_request::ACCOUNT) == CAPTURED_ACCOUNT
        && q(join_request::CHARACTER) == CAPTURED_CHARACTER
        && session == CAPTURED_SESSION;
    c.push(check(
        "⭐⭐ the CLIENT'S OWN decoder reproduces every field the CLIENT'S OWN log printed",
        fields_ok && d.bits == CAPTURED_BITS,
        format!(
            "nonce {:#010x}  ah {:#018x}  account {:#018x}  character {:#018x}  session {session:?}  \
             {} bits of {}",
            u32::from_le_bytes(s[0..4].try_into().unwrap()),
            q(join_request::AH_ID),
            q(join_request::ACCOUNT),
            q(join_request::CHARACTER),
            d.bits,
            wire.len() * 8
        ),
    ));

    // ⭐ Both directions on bytes neither side of this project produced.
    let re = codec.encode(3, &d.object)?;
    c.push(check(
        "⭐⭐ re-encoding the decoded struct reproduces the captured 238 bytes BYTE FOR BYTE",
        re.payload == wire && re.bits == CAPTURED_BITS,
        format!("{} bytes / {} bits", re.payload.len(), re.bits),
    ));

    // ── the negatives, which must keep failing ────────────────────────────────────────────────
    let decoy = ObjectOracle::new(DECOY_TAG_SAME_STRUCT_SIZE)
        .and_then(|mut o| o.decode(&wire))
        .map(|dd| u32::from_le_bytes(dd.object[0..4].try_into().unwrap()));
    c.push(check(
        "NEGATIVE: a decoy tag with the SAME struct_size does not reproduce the nonce",
        decoy.as_ref().map(|n| *n != CAPTURED_NONCE).unwrap_or(true),
        match &decoy {
            Ok(n) => format!("{DECOY_TAG_SAME_STRUCT_SIZE:#010x} gave nonce {n:#010x}"),
            Err(e) => format!("{DECOY_TAG_SAME_STRUCT_SIZE:#010x} refused: {e}"),
        },
    ));

    c.push(check(
        "NEGATIVE: the schema wire is NOT the struct image - the raw-struct experiment stays \
         distinguishable",
        wire.len() != d.object.len() && wire[..] != d.object[..wire.len()],
        format!("wire {} bytes vs struct {} bytes", wire.len(), d.object.len()),
    ));

    // Udt 2 (world_globals_state) is pinned on 56578; udt 42 still has no row.
    let unpinned = tag_for_udt(42);
    c.push(check(
        "NEGATIVE: an udt with no call-site row REFUSES rather than improvising a tag",
        unpinned.is_err() && tag_for_udt(4).is_ok(),
        "udt 42 has no CALL_SITES row; udt 4 does - positive control on the same call",
    ));

    let short = codec.encode(3, &vec![0u8; 0x80]);
    c.push(check(
        "NEGATIVE: a struct image of the wrong size is refused, not padded",
        short.is_err() && codec.encode(3, &vec![0u8; 0x100]).is_ok(),
        "half-messages and invented padding are both silent on the wire",
    ));

    // ── join_result round trip ────────────────────────────────────────────────────────────────
    let mut jrb = vec![0u8; codec.struct_size(4)?];
    jrb[join_result::EMBEDDED_REQUEST..join_result::EMBEDDED_REQUEST + 0x100]
        .copy_from_slice(&d.object);
    jrb[join_result::RESULT] = 0;
    jrb[join_result::SESSION_ID..join_result::SESSION_ID + 5].copy_from_slice(b"d1ah1");
    jrb[join_result::REPLICATION_EPOCH] = 1;
    jrb[join_result::GRACE_MS..join_result::GRACE_MS + 2].copy_from_slice(&10_000u16.to_le_bytes());
    jrb[join_result::KEEPALIVE_MS..join_result::KEEPALIVE_MS + 2]
        .copy_from_slice(&2_000u16.to_le_bytes());
    let enc = codec.encode(4, &jrb)?;
    let back = codec.decode(4, &enc.payload)?;
    c.push(check(
        "join_result encodes and decodes back to the identical struct",
        back.object == jrb,
        format!("{} bytes / {} bits on the wire", enc.payload.len(), enc.bits),
    ));

    // ⭐ The cross-control that ties udt 4 to the captured bytes: join_result's first field IS the
    // join_request, so the first 1903 bits of a join_result carrying the captured request must be
    // the captured payload's bits. Compare on whole bytes plus the partial byte's high bits.
    let n = (CAPTURED_BITS / 8) as usize;
    let rem = (CAPTURED_BITS % 8) as u32;
    let mask = if rem == 0 { 0u8 } else { 0xFFu8 << (8 - rem) };
    let common = enc.payload.iter().zip(wire.iter()).take_while(|(a, b)| a == b).count();
    let prefix_ok = enc.payload.len() > n
        && enc.payload[..n] == wire[..n]
        && (enc.payload[n] & mask) == (wire[n] & mask);
    c.push(check(
        "⭐ the first 1903 bits of a join_result ARE the captured join_request's bits",
        prefix_ok,
        format!(
            "{n} whole bytes + {rem} bits, mask {mask:#04x}; join_result wire is {} bytes / {} bits \
             and agrees with the captured request for {common} bytes",
            enc.payload.len(),
            enc.bits
        ),
    ));

    // ── udt 1: the activity name, and the round trip that carries it ──────────────────────────
    let derived_name = derive_activity_name_field(&image, &index)?;
    let gas_size = codec.struct_size(1)?;
    c.push(check(
        "⭐ the activity NAME is a char[0x28] at +0x688, and it tiles global_activity_state to \
         its END",
        derived_name == (global_activity_state::NAME, global_activity_state::NAME_CAPACITY)
            && derived_name.0 + derived_name.1 == gas_size,
        format!(
            "derived +{:#05x} cap {:#x}; {:#x} + {:#x} = {gas_size:#x}",
            derived_name.0, derived_name.1, derived_name.0, derived_name.1
        ),
    ));

    let mut gas = vec![0u8; gas_size];
    gas[global_activity_state::FLAG_0] = 1;
    let name = b"act_cosmo_m0_highway";
    gas[global_activity_state::NAME..global_activity_state::NAME + name.len()]
        .copy_from_slice(name);
    let gas_enc = codec.encode(1, &gas)?;
    let gas_back = codec.decode(1, &gas_enc.payload)?;
    c.push(check(
        "global_activity_state round trips with a flag and an activity name intact",
        gas_back.object == gas,
        format!(
            "{} bytes / {} bits; name at +{:#05x} survives",
            gas_enc.payload.len(),
            gas_enc.bits,
            global_activity_state::NAME
        ),
    ));

    // ── udt 11 replicate_membership ───────────────────────────────────────────────────────────
    //
    // ⛔ These five INVERT the control that used to read *"NAMED GAP: udt 11 is NOT schema-coded
    // and this module refuses it"*. That control asserted a conclusion drawn from the WRONG
    // HANDLER - `0x29CDE0` is udt 7 `sensor_message`, not udt 11 - and the replacement has to
    // discriminate at least as hard, so each one below can fail on its own axis.

    let membership_size = codec.struct_size(11)?;
    c.push(check(
        "⭐ udt 11's tag is 0x808009ED and its struct is 0x5038 - the size FUN_0029C730 states",
        tag_for_udt(11)? == 0x8080_09ED
            && membership_size == replicate_membership::STRUCT_SIZE
            && membership_size == 0x5038,
        "derived through the same five steps as udt 3/4; `local_528c = 0x5038` and \
         `memcpy(image, ac+0xE70, 0x5038)` are the independent statements of the size",
    ));

    // The dispatch table is a linear scan over `{i32 type; i32 pad; fnptr}` and it is NOT sorted:
    // reading it as dense over the sorted AH→client list is what produced the retracted claim.
    let dispatch = (0..17)
        .map(|i| {
            let e = addrs().ah_rx_dispatch + i * 16;
            Ok((image.u32(e)?, image.u64(e + 8)?))
        })
        .collect::<std::result::Result<Vec<_>, d1_eboot::image::ImageError>>()
        .map_err(|e| OracleError::Eboot(e.to_string()))?;
    let types: Vec<u32> = dispatch.iter().map(|(t, _)| *t).collect();
    c.push(check(
        "⭐ the RX dispatch table stores its type ids and is NOT sorted - udt 11 is index 4, before \
         udt 10",
        types.get(4) == Some(&11)
            && types.get(5) == Some(&10)
            && types.get(7) == Some(&7)
            && !types.windows(2).all(|w| w[0] < w[1]),
        format!("types in table order: {types:?}"),
    ));

    let (m_off, m_stride, m_cap) = derive_membership_array(&image, &index)?;
    c.push(check(
        "⭐ the member array is derived, and FUN_0029FE70's own arithmetic agrees with it",
        (m_off, m_stride, m_cap)
            == (
                replicate_membership::MEMBERS,
                replicate_membership::MEMBER_STRIDE,
                replicate_membership::MEMBER_CAPACITY,
            )
            // FUN_0029FE70 indexes `p[i*0x4a + 2]` on a long*, with the image at object+8:
            // (i*0x4a + 2)*8 - 8 == i*0x250 + 8. Same base, same stride, different derivation.
            && 0x4a * 8 == m_stride
            && (2 * 8 - 8) == m_off
            // and the occupancy mask it tests, `*(u32*)(object + 0x5034)`, is image +0x502C.
            && 0x5034 - 8 == replicate_membership::OCCUPANCY_MASK,
        format!("members +{m_off:#x}, stride {m_stride:#x}, capacity {m_cap}"),
    ));

    // ⭐⭐ The decisive one: our bytes, the CLIENT'S OWN udt-11 decoder (FUN_002C9050), round trip.
    let solo = replicate_membership::solo(&d.object)
        .ok_or_else(|| OracleError::Shape("the captured join_request is too short".into()))?;
    let mem_enc = codec.encode(11, &solo)?;
    let mem_back = {
        let o = codec.oracle(11)?;
        o.decode_replicated(&mem_enc.payload, &[])?
    };
    let ident = &solo[replicate_membership::MEMBERS
        ..replicate_membership::MEMBERS + replicate_membership::IDENTITY_SIZE];
    let properties = &solo[replicate_membership::MEMBERS
        + replicate_membership::MEMBER_PROPERTIES
        ..replicate_membership::MEMBERS
            + replicate_membership::MEMBER_PROPERTIES
            + replicate_membership::PROPERTIES_PREFIX_SIZE];
    let fireteam_id = u64::from_le_bytes(
        solo[replicate_membership::MEMBER_FIRETEAM_ID
            ..replicate_membership::MEMBER_FIRETEAM_ID + 8]
            .try_into()
            .unwrap(),
    );
    c.push(check(
        "⭐⭐ a solo replicate_membership round trips through the CLIENT'S OWN FUN_002C9050",
        mem_back.object == solo
            && solo[replicate_membership::SIMULATION_OCCUPANCY_MASK] == 1
            && solo[replicate_membership::OCCUPANCY_MASK] == 1
            && ident[0x10..0x18] == CAPTURED_ACCOUNT.to_le_bytes()
            && ident[0x18..0x20] == CAPTURED_CHARACTER.to_le_bytes()
            && ident[0x00..0x08] == CAPTURED_MACHINE
            && ident[0x08..0x10] == CAPTURED_PLAYER
            && properties
                == &d.object[join_request::PROPERTIES
                    ..join_request::PROPERTIES + join_request::PROPERTIES_SIZE]
            && fireteam_id != 0
            && fireteam_id
                == u64::from_le_bytes(ident[0x20..0x28].try_into().unwrap()),
        format!(
            "{} bytes / {} bits; member[0]'s identity + properties are the captured client's own; \
             fireteam id {fireteam_id:#018x}, echoed not built",
            mem_enc.payload.len(),
            mem_enc.bits
        ),
    ));

    // NEGATIVE, and it tests the axis that actually worries us: is the presence bit real, or are we
    // prepending a bit the client does not read? Encode the SAME image with the plain encoder and
    // hand it to the same FUN_002C9050. One bit of misalignment must not survive.
    let plain = {
        let o = codec.oracle(11)?;
        o.encode(&solo, ENCODE_CAPACITY)?
    };
    let plain_back = {
        let o = codec.oracle(11)?;
        o.decode_replicated(&plain.payload, &[])
    };
    c.push(check(
        "NEGATIVE: the same image WITHOUT the leading presence bit does not survive FUN_002C9050",
        !plain_back.map(|d| d.object == solo).unwrap_or(false)
            && plain.payload != mem_enc.payload,
        "if this passed, the presence bit would be decoration and our wire would be one bit long",
    ));

    // NEGATIVE: a decoy tag of a DIFFERENT struct size must refuse, so the size agreement above is
    // not the only thing standing between us and a wrong tag.
    c.push(check(
        "NEGATIVE: udt 42 still refuses (no call site) while udt 11 now succeeds",
        tag_for_udt(42).is_err() && tag_for_udt(11).is_ok() && tag_for_udt(0).is_ok(),
        "closing udt 11 must not have opened the other 21 unpinned ids",
    ));

    c.push(ok(
        "the encoder's capacity is the client's own 0x1DF00 scratch",
        "FUN_002A4140 hands the bitstream 0x1DF00; ours cannot overflow where the client's would not",
    ));

    // ── 2026-08-09: the four named gates of run 20260809-054417-RUNG33-BYPASS ──────────────────

    // ⭐⭐ udt 15. The axis: does this module tell the truth about WHY there is no tag? The old
    // message said "UNPINNED - read its FUN_00299310 call site and add the row", which is advice
    // that cannot be followed, because the call site IS read and it says 0xFFFFFFFF.
    let raw15 = tag_for_udt(15);
    // Udt2 is measured on56578; retain an actually unpinned negative on both builds.
    let unpinned42 = tag_for_udt(42);
    let raw_msg = raw15.as_ref().err().map(|e| e.to_string()).unwrap_or_default();
    let unpinned_msg = unpinned42.as_ref().err().map(|e| e.to_string()).unwrap_or_default();
    c.push(check(
        "⭐⭐ udt 15's keepalive has NO SCHEMA TAG BY CONSTRUCTION, and says so differently from \
         a merely unpinned id",
        raw15.is_err()
            && raw_msg.contains("sent RAW")
            && raw_msg.contains("THERE IS NO SCHEMA TAG TO PIN")
            && unpinned42.is_err()
            && unpinned_msg.contains("UNPINNED")
            && !unpinned_msg.contains("sent RAW"),
        format!(
            "FUN_00298C40 passes descriptor 0xFFFFFFFF and sets the raw flag; udt 42 is the \
             discriminating negative - {} raw-send rows",
            addrs().ah_raw_send.len()
        ),
    ));

    // ⭐ udt 21 - the message whose encoder is NOT FUN_00299310, which is why no earlier scan found
    // it. Two independent statements of 0x220: the memcpy in FUN_002987E0 and the gap between the
    // two struct pointers FUN_0029CC20 passes.
    let s21 = codec.struct_size(21)?;
    let cad = vec![0u8; s21];
    let cad_enc = codec.encode(21, &cad)?;
    let cad_back = codec.decode_seeded(21, &cad_enc.payload, &[])?;
    c.push(check(
        "⭐ udt 21 client_authoritative_data_update derives a tag, its struct is 0x220, and it \
         round trips through the REPLICATED wrapper",
        s21 == 0x220 && is_replicated(21) && cad_back.object == cad,
        format!(
            "tag {:#010x}, struct {s21:#x}, {} bytes / {} bits on the wire; encoder FUN_002C9290 \
             (replicated) via messaging->vfunc[6], NOT FUN_00299310",
            tag_for_udt(21)?,
            cad_enc.payload.len(),
            cad_enc.bits
        ),
    ));

    // ⭐ udt 14. The size agreement is the ordinary one - `FUN_0029A610` passes `0x14` - and it is
    // the only claim made here. ⚠ The live payload was *also* 20 bytes, but that is the **wire**,
    // not the struct, and the two are different quantities (udt 3 is 256 vs 238); the coincidence
    // is suggestive and is deliberately not asserted.
    let s14 = codec.struct_size(14)?;
    c.push(check(
        "⭐ udt 14 peer_leave_request derives a tag and its struct is the 0x14 FUN_0029A610 passes",
        s14 == 0x14,
        format!("tag {:#010x}, struct {s14:#x} = {s14}", tag_for_udt(14)?),
    ));

    // ⭐⭐ The dynamic-flags refusal. The axis: a Unicorn fault must not be reported as if the
    // message were unknown. Derived, not listed - and the positive control is on the same call.
    let df = dynamic_flags_udts()?;
    let df_err = codec.struct_size(37).err().map(|e| e.to_string()).unwrap_or_default();
    c.push(check(
        "⭐ udts whose descriptor is a DYNAMIC FLAGS bitset are refused with their cause, and the \
         set is derived",
        df == vec![33, 37]
            && df_err.contains("DYNAMIC FLAGS")
            && df_err.contains("FUN_000B0550")
            && codec.struct_size(1).is_ok(),
        format!(
            "derived {df:?} from field kind {FIELD_KIND_DYNAMIC_FLAGS:#04x}; the fault is \
             {:?} after {:#x}; udt 1 is the positive control on the same call",
            addrs().dynamic_flags_fault_rip,
            addrs().dynamic_flags_grow_fn
        ),
    ));

    // ── ⭐⭐ 2026-08-09: THE LEAK. The class of bug the other 27 controls above cannot see, ─────
    // because every one of them encodes each message exactly once. Run at a modest cycle count
    // here so `controls` stays a few seconds; `d1-oracle activity-host leak --cycles 2000`
    // simulates a 2.8-hour session.
    c.extend(leak_control(LEAK_CYCLES_IN_CONTROLS)?);

    // NEGATIVE: seeding a NON-replicated udt must be refused rather than quietly ignored. A caller
    // that thinks udt 4 is a delta has a bug, and swallowing the seed hides it.
    let seeded_wrong = codec.decode_seeded(3, &wire, &vec![0u8; 0x100]);
    c.push(check(
        "NEGATIVE: a seed image handed to a non-replicated udt is refused, not ignored",
        seeded_wrong.is_err() && codec.decode_seeded(3, &wire, &[]).is_ok(),
        "udt 3 refuses a seed; the same call with no seed succeeds - the positive half",
    ));

    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_control_passes() {
        if !crate::test_inputs::selected_eboot() {
            return;
        }
        let failed: Vec<&str> =
            controls().unwrap().into_iter().filter(|c| !c.pass).map(|c| c.name).collect();
        assert!(failed.is_empty(), "failed controls: {failed:?}");
    }

    #[test]
    fn there_are_enough_controls_to_be_worth_running() {
        if !crate::test_inputs::selected_eboot() {
            return;
        }
        assert!(controls().unwrap().len() >= 12, "a controls suite this thin is decoration");
    }

    /// The two tags this whole session exists to establish, pinned as literals so a change in the
    /// derivation shows up as a test failure rather than a different answer.
    #[test]
    fn the_two_join_tags_are_pinned() {
        if !crate::test_inputs::selected_eboot() {
            return;
        }
        assert_eq!(tag_for_udt(3).unwrap(), 0x8080_0E98);
        assert_eq!(tag_for_udt(4).unwrap(), 0x8080_0A58);
        assert_eq!(tag_for_udt(0).unwrap(), 0x8080_0D64);
        assert_eq!(tag_for_udt(1).unwrap(), 0x8080_0E76);
        // ⭐ The fourth establish-client message, closed 2026-08-09.
        assert_eq!(tag_for_udt(11).unwrap(), 0x8080_09ED);
    }

    /// ⭐ The retraction, asserted rather than only written down: `0x29CDE0` is udt **7**
    /// `sensor_message`, and udt 11's handler is `0x29CF50`. The dispatch table stores its own type
    /// ids, so this needs no index-to-udt mapping at all - which is the mistake it guards against.
    #[test]
    fn the_rx_dispatch_table_is_not_dense_over_the_sorted_id_list() {
        if !crate::test_inputs::reference_eboot() {
            return;
        }
        let image = Image::open_reference().unwrap();
        let row = |i: u64| (image.u32(0x18F_84A0 + i * 16).unwrap(), image.u64(0x18F_84A0 + i * 16 + 8).unwrap());
        // The fnptr words are relocation slots and read 0 statically; only the types are inline.
        assert_eq!(row(4).0, 11, "index 4 is udt 11 replicate_membership");
        assert_eq!(row(5).0, 10, "…and udt 10 comes AFTER it - the list is not sorted");
        assert_eq!(row(7).0, 7, "index 7 is udt 7 sensor_message, the FUN_002D2090 one");
        let reloc = reloc::by_slot(&image).unwrap();
        assert_eq!(reloc.get(&(0x18F_84A0 + 4 * 16 + 8)), Some(&0x29_CF50));
        assert_eq!(reloc.get(&(0x18F_84A0 + 7 * 16 + 8)), Some(&0x29_CDE0));
    }

    /// Build 56578: the same 17 stored types in the same order, at `CODEC_56578.ah_rx_dispatch`, and
    /// the handlers agree with [`CALL_SITES_56578`] by an independent route. udt 0/1/4 point at
    /// the AH decode functions whose descriptor globals those rows hold, and udt 11's handler
    /// opens with a call to the replicated-decode caller. Skips when the 56578 image is absent.
    #[test]
    fn the_56578_rx_dispatch_table_agrees_with_its_call_site_rows() {
        let Some(b) = d1_build::builds().ok().and_then(|b| b.all.iter().find(|x| x.id == "later-56578"))
        else {
            return;
        };
        if !b.eboot_path().exists() {
            return;
        }
        let image = Image::open(b.eboot_path()).unwrap();
        let t = crate::codec::codec_for(&image.sha256()).unwrap().ah_rx_dispatch;
        assert_eq!(t, 0x191_0480);
        let types: Vec<u32> = (0..17).map(|i| image.u32(t + i * 16).unwrap()).collect();
        assert_eq!(types, [0, 1, 2, 4, 11, 10, 5, 7, 18, 23, 24, 27, 29, 38, 39, 42, 43]);
        let reloc = reloc::by_slot(&image).unwrap();
        let handler = |i: u64| *reloc.get(&(t + i * 16 + 8)).unwrap();
        assert_eq!(handler(0), 0x101_8800, "udt 0 -> decode with *0x191A3A0");
        assert_eq!(handler(1), 0x101_8990, "udt 1 -> decode with *0x191A410");
        assert_eq!(handler(2), 0x101_8D00, "udt 2 -> decode with *0x191A418");
        assert_eq!(handler(3), 0x101_8E30, "udt 4 -> decode with *0x191A388");
        assert_eq!(handler(4), 0x29_D950, "udt 11 replicate_membership");
        assert_eq!(handler(7), 0x29_D7E0, "udt 7 sensor_message");
        for (udt, g) in [(0u32, 0x191_A3A0u64), (1, 0x191_A410), (2, 0x191_A418), (4, 0x191_A388), (11, 0x191_A308)] {
            assert!(CALL_SITES_56578.iter().any(|(u, x, _)| *u == udt && *x == g), "udt {udt}");
        }
    }

    /// A solo membership is an **echo**: every non-zero byte in it either came from the client's
    /// own join_request or is a counter/bit the client's own code reads.
    #[test]
    fn a_solo_membership_echoes_identity_and_properties_without_inventing_fields() {
        let req = vec![0xAAu8; 0x100];
        let m = replicate_membership::solo(&req).unwrap();
        assert_eq!(m.len(), replicate_membership::STRUCT_SIZE);
        let id = replicate_membership::MEMBERS;
        assert_eq!(&m[id..id + 0x28], &req[0x10..0x38], "the identity is echoed verbatim");
        let properties = id + replicate_membership::MEMBER_PROPERTIES;
        assert_eq!(
            &m[properties..properties + replicate_membership::PROPERTIES_PREFIX_SIZE],
            &req[replicate_membership::JOIN_REQUEST_PROPERTIES
                ..replicate_membership::JOIN_REQUEST_PROPERTIES
                    + replicate_membership::PROPERTIES_PREFIX_SIZE],
            "the shared 0x80800B1F properties prefix is echoed verbatim"
        );
        assert_eq!(m[replicate_membership::GENERATION], 0, "0 is what leaves ac+0x64B4 alone");
        assert_eq!(
            m[replicate_membership::SIMULATION_OCCUPANCY_MASK],
            1,
            "slot 0 contributes its machine id to the simulation snapshot"
        );
        assert_eq!(m[replicate_membership::OCCUPANCY_MASK], 1, "slot 0 occupied, and only slot 0");
        // Everything else is zero: count the non-zero bytes and account for every one.
        let nz: Vec<usize> = (0..m.len()).filter(|i| m[*i] != 0).collect();
        let accounted: Vec<usize> = nz
            .iter()
            .copied()
            .filter(|i| {
                *i == replicate_membership::MEMBER_COUNT
                    || *i == replicate_membership::SIMULATION_OCCUPANCY_MASK
                    || *i == replicate_membership::OCCUPANCY_MASK
                    || (id..id + 0x28).contains(i)
                    || (properties..properties + replicate_membership::PROPERTIES_PREFIX_SIZE)
                        .contains(i)
            })
            .collect();
        assert_eq!(nz, accounted, "an unaccounted non-zero byte is an invented field");
    }
}
