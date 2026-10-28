// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The RSAT codec's game addresses, **per executable build**.
//!
//! The `pub const`s in [`crate::object`], [`crate::message`], [`crate::schema`],
//! [`crate::registry`] and [`crate::activity_host`] are the 01.29 reference values and keep their
//! derivation notes. The oracles do not call them directly: they call through the
//! [`CodecAddrs`] chosen by the image's SHA-256, and an image with no measured set is refused.
//! Reusing 01.29 numbers on another build would call into the middle of unrelated functions.
//!
//! # How the 56578 set was measured (static, 2026-10-13)
//!
//! Every value was re-found from structure, not by offsetting the 01.29 numbers. The 01.29 ELF is
//! not on disk, so each identification uses only the 56578 image and the 01.29 notes.
//!
//! * **Bitstream primitives:** these are at the **same** addresses as 01.29, and each body matches
//!   its documented behaviour. The ctor stores `buf`, `buf+len` and `len`. begin-write sets
//!   alignment at `+0x14` and mode 1. begin-read sets mode 3, end sets mode 5. The spill writes
//!   `+0x24/+0x28/+0x30`, and read-bit returns `bs[0x28] >> 63`.
//!   `WRITE_ONE_BIT` is `c_network_channel_simulation_gatekeeper` vtable slot 5 (RTTI, vtable
//!   `0x196B170`) = `0xAF1650`. Its body is byte-for-byte the 01.29 listing, including the tail
//!   call to the spill at `0x733A0`.
//! * **Object codec:** the only `call` that follows a begin-read in more than one caller and is
//!   not a bitstream primitive is `0x2C9A90`; the begin-write counterpart is `0x2C9A80`. Both are
//!   `call inner; mov al,1; ret` wrappers, as in 01.29, and they sit `+0xA90` from 01.29's.
//!   The replicated wrapper `0x2C9AE0` takes the tag in `ecx` and opens with the `tag_slot`
//!   arithmetic.
//! * **RSAT memset allowance:** the three documented bitset sites, each shifted by `+0xA90`, are
//!   the zeroing memset in the encode inner `0x2C8F82`, `bitset_set_all`'s `0xFF` fill at
//!   `0x2C76D5`, and the replicated wrapper's large-bitset arm returning to `0x2C9BD8`. Each has
//!   the same `cmp rdx,0x21` or `test dil,3` guard. The window ends at **`0x2C9E20`**, the
//!   prologue after the encode counterpart `0x2C9D20`, and not at a blind `+0xA90`. `0x2C9E20`
//!   memsets a 0x100-byte stack buffer that is not a bitset, and the allowance must not cover it.
//! * **Message codec:** the only callers of the object encoder and decoder in the message
//!   module are `0x261E90` and `0x262260`. The encoder takes
//!   `(handle, 1, out, capacity, out_len)`. It calls the manager getter `0x25CBD0` (a
//!   `lea; mov; ret` singleton read) and then `vfunc[0x4D0]`, and reads the definition's
//!   `+0x14`/`+0x18` tags. The decoder reads 16 bits and passes them to `0x2800E0`. That function
//!   walks the manager's definitions through `vfunc[0x4D0]` and returns `0xFFFF` on a miss: id → index.
//! * **Registry global:** the decoder has exactly three rip-references to `0x1A238A0`, which
//!   matches the three inlined walks in 01.29. The walk at `0x262559` is the construction
//!   contract in [`crate::registry`] instruction for instruction: `tag_slot × 0x40`, `+0x10`
//!   base, `+0x30` entry size, `+0x34` mask and `tag & 0x1FFF`.
//! * **Activity Host:** the decode and encode wrappers are the functions that bracket the object
//!   codec with ctor/begin/end, at `0x2A4AC0` and `0x2A4B70`. The encode wrapper still hands the
//!   bitstream `0x1DF00`. Its one caller is `activity_client_send_message`, `0x299D10`, which
//!   still tests bit 0 of the seventh argument for the raw arm.
//!   [`crate::activity_host::CALL_SITES_56578`] says how its rows were read.
//! * **AH receive dispatch table:** it is the only `.data` run with the 01.29 shape, `{type, pad,
//!   fnptr}` with type 4, 11, 10 and 7 at indices 3, 4, 5 and 7, and it is at `0x1910480`. All 17
//!   stored types are in 01.29's order. Its handlers, read through relocations, confirm the
//!   call-site rows independently. udt 0, 1 and 4 point at the decode functions whose descriptor
//!   globals those rows hold (`0x1018800`, `0x1018990`, `0x1018E30`). udt 11's handler
//!   `0x29D950` first calls `0x29D130`, the replicated-decode caller, and udt 7 is
//!   `sensor_message` at `0x29D7E0`.

/// One build's codec address set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodecAddrs {
    pub bs_ctor: u64,
    pub bs_begin_write: u64,
    pub bs_begin_read: u64,
    pub bs_flush: u64,
    pub bs_end: u64,
    pub write_bits_spill: u64,
    pub write_one_bit: u64,
    pub read_bit: u64,
    pub encode_obj: u64,
    pub decode_obj: u64,
    pub decode_obj_replicated: u64,
    /// `[lo, hi)`: where a direct `call memset` may clear a stack bitset. See [`crate::object`].
    pub codec_lo: u64,
    pub codec_hi: u64,
    pub msg_encoder: u64,
    pub msg_decoder: u64,
    pub get_manager: u64,
    pub id_to_index: u64,
    pub registry_global: u64,
    pub ah_decode_fn: u64,
    pub ah_encode_fn: u64,
    pub ah_send_message_fn: u64,
    pub ah_call_sites: &'static [(u32, u64, Option<usize>)],
    pub ah_raw_send: &'static [(u32, u64, &'static str)],
    /// The AH receive dispatch table, `{i32 type; i32 pad; fnptr}` × 17. See [`crate::activity_host`].
    pub ah_rx_dispatch: u64,
    /// Where the dynamic-flags decode faults in a static harness. `None` until it has been observed
    /// on this build: the refusal text then names the grow function only.
    pub dynamic_flags_fault_rip: Option<u64>,
    pub dynamic_flags_grow_fn: u64,
    /// Start-up type cells the field encoders (kind 0x23 `0x102A450`, the box encoders `0x10295D0`,
    /// `0x1029740`, `0x102A3xx`) read through pointer globals that the file leaves zero:
    /// `(pointer slot, cell, tag, descriptor object)`; the cell holds `{u32 tag, pad, u64 descriptor
    /// object}`. Read live on 56578. Without them every box field (e.g. the client bundle's player
    /// identity, kind `0x23`) encodes as tag 0 and faults. Empty until measured on a build.
    pub type_cells: &'static [(u64, u64, u32, u64)],
    /// Native lazy lookup tables: (table, initialized byte, source type-cell slots).
    /// Seed the initializer's result; never call unmapped C++ guard imports.
    pub type_cell_caches: &'static [(u64, u64, &'static [u64])],
}

pub const CODEC_0129: CodecAddrs = CodecAddrs {
    bs_ctor: crate::object::BS_CTOR,
    bs_begin_write: crate::object::BS_BEGIN_WRITE,
    bs_begin_read: crate::object::BS_BEGIN_READ,
    bs_flush: crate::object::BS_FLUSH,
    bs_end: crate::object::BS_END,
    write_bits_spill: crate::object::WRITE_BITS_SPILL,
    write_one_bit: crate::object::WRITE_ONE_BIT,
    read_bit: crate::object::READ_BIT,
    encode_obj: crate::object::ENCODE_OBJ,
    decode_obj: crate::object::DECODE_OBJ,
    decode_obj_replicated: crate::object::DECODE_OBJ_REPLICATED,
    codec_lo: crate::object::CODEC_LO,
    codec_hi: crate::object::CODEC_HI,
    msg_encoder: crate::message::ENCODER,
    msg_decoder: crate::message::DECODER,
    get_manager: crate::schema::GET_MANAGER,
    id_to_index: crate::schema::ID_TO_INDEX,
    registry_global: crate::registry::REGISTRY_GLOBAL,
    ah_decode_fn: crate::activity_host::DECODE_FN,
    ah_encode_fn: crate::activity_host::ENCODE_FN,
    ah_send_message_fn: crate::activity_host::SEND_MESSAGE_FN,
    ah_call_sites: crate::activity_host::CALL_SITES,
    ah_raw_send: crate::activity_host::RAW_SEND,
    ah_rx_dispatch: 0x18F_84A0,
    dynamic_flags_fault_rip: Some(crate::activity_host::DYNAMIC_FLAGS_FAULT_RIP),
    dynamic_flags_grow_fn: crate::activity_host::DYNAMIC_FLAGS_GROW_FN,
    type_cells: &[],
    type_cell_caches: &[],
};

/// Build 56578 (01.33). Measured as described in the module docs.
pub const CODEC_56578: CodecAddrs = CodecAddrs {
    bs_ctor: 0x7_2240,
    bs_begin_write: 0x7_23B0,
    bs_begin_read: 0x7_2400,
    bs_flush: 0x7_2560,
    bs_end: 0x7_2650,
    write_bits_spill: 0x7_33A0,
    write_one_bit: 0xAF_1650,
    read_bit: 0x7_3430,
    encode_obj: 0x2C_9A80,
    decode_obj: 0x2C_9A90,
    decode_obj_replicated: 0x2C_9AE0,
    codec_lo: 0x2C_6A90,
    codec_hi: 0x2C_9E20,
    msg_encoder: 0x26_1E90,
    msg_decoder: 0x26_2260,
    get_manager: 0x25_CBD0,
    id_to_index: 0x28_00E0,
    registry_global: 0x1A2_38A0,
    ah_decode_fn: 0x2A_4AC0,
    ah_encode_fn: 0x2A_4B70,
    ah_send_message_fn: 0x29_9D10,
    ah_call_sites: crate::activity_host::CALL_SITES_56578,
    ah_raw_send: crate::activity_host::RAW_SEND_56578,
    ah_rx_dispatch: 0x191_0480,
    dynamic_flags_fault_rip: None,
    dynamic_flags_grow_fn: 0x2C_7630,
    type_cells: &[
        (0x199_6050, 0x28D_BAD0, 0x8080_0046, 0x13D_2258),
        (0x197_4C00, 0x28B_F2D0, 0x8080_40F7, 0x133_BF70),
        (0x197_4BE0, 0x28B_F190, 0x8080_424E, 0x13D_9870),
        // Native atom payload variants: 1C0630 / 1C0740 read these cells after
        // decoding the primary and secondary selector. Measured read-only on
        // 56578; without them a nonempty member program resolves tag zero.
        (0x197_4BE8, 0x28B_F1D0, 0x8080_07B3, 0x13A_23F8),
        (0x197_4BF0, 0x28B_F1F0, 0x8080_0453, 0x134_7258),
        (0x197_4BF8, 0x28B_F210, 0x8080_046B, 0x12E_E140),
        (0x197_4C08, 0x28B_F310, 0x8080_051E, 0x135_9EF0),
        (0x197_4C10, 0x28B_F350, 0x8080_0335, 0x132_AB40),
        (0x197_4C18, 0x28B_F370, 0x8080_0547, 0x134_3CB8),
        (0x197_4C20, 0x28B_F390, 0x8080_047D, 0x130_8800),
        (0x197_4C28, 0x28B_F3B0, 0x8080_0716, 0x132_C7B0),
        (0x197_4C30, 0x28B_F410, 0x8080_040B, 0x131_6270),
        (0x197_4C38, 0x28B_F430, 0x8080_0842, 0x13D_A448),
        (0x197_4C40, 0x28B_F450, 0x8080_079F, 0x136_25D0),
        (0x197_4C48, 0x28B_F470, 0x8080_0661, 0x137_1B78),
        (0x197_4C50, 0x28B_F490, 0x8080_08D3, 0x133_FB38),
        // Kind 0x23 (`0x102A450`): type from `0x197E160` when the selector `0x25CC10` holds
        // (`[[0x26527E0]+0xC] != -1`, true live), else from `0x191A240`.
        (0x197_E160, 0x28C_37F0, 0x8080_1932, 0x131_8320),
        (0x191_A240, 0x269_8FA0, 0x8080_04AC, 0x131_78F0),
    ],
    // 1C0630 / 1C0740 copy the tags from these exact cells, then release
    // their function-local static guards. This is their complete table order.
    type_cell_caches: &[
        (
            0x25D_92C0,
            0x25D_92E8,
            &[
                0x197_4C18, 0x197_4C20, 0x197_4C28, 0x197_4C30, 0x197_4C38, 0x197_4C10, 0x197_4C40,
                0x197_4C08, 0x197_4C48, 0x197_4C50,
            ],
        ),
        (
            0x25D_92F0,
            0x25D_9300,
            &[0x197_4BE8, 0x197_4BF0, 0x197_4BF8],
        ),
    ],
};

/// The codec set for an image, by its SHA-256. `None` = no measured set: refuse to run.
pub fn codec_for(sha256: &str) -> Option<&'static CodecAddrs> {
    match sha256 {
        "7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954" => Some(&CODEC_0129),
        "672f03411c0503dfbcda804cbb9fb27cdd9a8d73a8bcb71bf0c4f688073b0833" => Some(&CODEC_56578),
        _ => None,
    }
}

/// The set for the selected build's image. Errors name the image when there is no set.
pub fn selected() -> crate::error::Result<&'static CodecAddrs> {
    let image = d1_eboot::image::Image::open_default()
        .map_err(|e| crate::error::OracleError::Eboot(e.to_string()))?;
    codec_for(&image.sha256()).ok_or_else(|| {
        crate::error::OracleError::Eboot(format!(
            "no codec address set for image sha256 {} - the codecs would be called at the wrong \
             addresses",
            image.sha256()
        ))
    })
}
