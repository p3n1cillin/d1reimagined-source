// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The **object** codec - `0x2C9000` (decode) / `0x2C8FF0` (encode).
//!
//! Port of `capture-rig/oodle/queuez-object-oracle.py`.
//!
//! # WHICH ORACLE, AND WHY THE WRONG ONE IS SILENT
//!
//! [`crate::message`] drives the *message* codec `0x261BE0`/`0x261FB0`, which wraps the object in a
//! `u16` message id and a trailing span-presence bit. A **QueueZ server object** payload has
//! NEITHER: the QueueZ handler hands the raw `rsat[rsat_length]` bytes straight to the object
//! codec. Read out of `0x56B5F0` (the object-install path reached from `push:queuez` → `0x5655B0`):
//!
//! ```text
//! 0x56B646  mov dword [bs+0x14], 1                        ; alignment 1
//! 0x56B656  call 0x72240(bs, payload, rsat_length)        ; bitstream ctor
//! 0x56B812  call 0x72400(bs)                              ; begin read (mode 3)
//! 0x56B833  call 0x2C9000(edi = SCHEMA HANDLE, rsi = bs, rdx = obj, rcx = obj, r8d = 1)
//! 0x56B83B  call 0x72650(bs)                              ; end (mode 5)
//! ```
//!
//! ⚠ **The encoder's argument order is NOT the decoder's.** `0x2C8FF0(edi = tag, rsi = object,
//! rdx = bitstream, ecx = 1)` - read at `0x261CD5..0x261CE7`. Picking the wrong oracle, or
//! transposing these, produces a *plausible wrong answer* rather than an error.
//!
//! # THE NARROWED memset GUARD
//!
//! [`crate::harness`] refuses any `memset` overlapping the guest stack, because an unpopulated
//! scaffolding pointer that lands there wipes a return address and then presents as "dispatched to
//! a null import". Right in spirit, too wide in practice: the RSAT codec's dynamic-field **bitset**
//! is an ordinary frame local, memset twice per encode. Both sites are the bitset and nothing else,
//! verified by disassembly:
//!
//! ```text
//! 0x2C84F2  xor esi,esi; call 0x129D240   in 0x2C8420, rdi = [rbp-0x520] = &bitset,
//!                                          rdx = ((span+1+0x1f)>>5)*4
//! 0x2C6C45  mov esi,0xff; call 0x129D240  in 0x2C6C00 = bitset_set_all(bitset), rdi = [rbx]
//! ```
//!
//! So a stack-overlapping `memset` is permitted **only** when the return address is a confirmed
//! direct-call site inside the RSAT codec module. Every other destination still faults.
//!
//! # THE (family, type) → SCHEMA TAG MAP, and how it was recovered  `[C]`
//!
//! `0x56C550` writes `layout_entry+0x18 = *(u32*)(*(void**)schema_slot)`; the slot contents are
//! runtime, so the tag is **not** statically readable there. It is recoverable anyway, because
//! `0x56B6BC` resolves that same handle to a registry object and compares `object+0x08` with the
//! wire `instance_type` - so the tag is the one whose registry object carries the live-measured
//! checksum. Byte-scanning the eboot for each checksum finds exactly one record each:
//!
//! ```text
//! checksum      record       descriptor self-tag   size     family/type
//! 0xA2AC2C74    0x139ADD0    0x80801951            0x00A0   3/0  ROSTER
//! 0x3431F1BA    0x1350750    0x80801954            0x0A80   3/1  character
//! 0x8127F15A    0x1304800    0x80801743            0x7078   4/1  PROFILE
//! 0x64AE75BF    0x12FA3F0    0x80801803            0x7338   4/0  account
//! ```
//!
//! `record+0x50` is the descriptor **anchor** (`0x80800050 0x8080010a`), `record+0x58` its field
//! count and `record+0x60` its self tag - so [`d1_eboot::descriptors`]'s index already contains all
//! of them. ⚠ `record+0x10` (`0x808017DC` for family 3, `0x80801714` for family 4/1) is **not** the
//! object's tag.

use std::collections::BTreeSet;

use crate::control;
use crate::error::{OracleError, Result};
use crate::harness::{self, Harness};
use crate::schema::SchemaHarness;

pub const BS_CTOR: u64 = 0x7_2240; // (this, buf, byte_len)
pub const BS_BEGIN_WRITE: u64 = 0x7_23B0; // (this, alignment)
pub const BS_BEGIN_READ: u64 = 0x7_2400; // (this)
pub const BS_FLUSH: u64 = 0x7_2560; // (this, out_param)  ⚠ takes an out-param in rsi
pub const BS_END: u64 = 0x7_2650; // (this)
pub const ENCODE_OBJ: u64 = 0x2C_8FF0; // (tag, object, bitstream, 1)
pub const DECODE_OBJ: u64 = 0x2C_9000; // (tag, bitstream, object, object, 1)
/// `FUN_000733A0(bitstream, value, nbits)` - the bitstream's **spill** path, not a general write.
///
/// ⚠⚠ It is only correct when `nbits >= 64 - bs[0x30]`: it computes `uVar4 = nbits - room` and
/// stores that as the new bit count, so calling it with one bit on a fresh stream sets
/// `bs[0x30] = (u32)(1 - 64)` and flushes eight bytes. Every caller in the image guards it with an
/// explicit `if (room < nbits)`. **Measured, not assumed** - wiring it up unguarded produced a udt
/// 11 wire whose presence bit read back as 0 and whose decode consumed exactly 1 bit.
pub const WRITE_BITS_SPILL: u64 = 0x7_33A0;
/// ⭐ `c_network_channel_simulation_gatekeeper::vfunc[5]` (`0xAEF740`) - the out-of-line
/// **write-one-bit**, `(src, _, bitstream)`, taking the bit from `*(u8*)(src + 8)`.
///
/// It is the same three-instruction accumulator update the codec inlines everywhere, with the same
/// `room < 1` guard around [`WRITE_BITS_SPILL`]. Verified a pure leaf by disassembly - its whole
/// body is `movzx esi,[rdi+8]` then reads/writes of `bs+0x24`, `bs+0x28`, `bs+0x30`, or the tail
/// call, and nothing else:
///
/// ```text
/// 0xAEF744  movzx esi, byte [rdi+8]        0xAEF758  mov  rax, [rdx+0x28]
/// 0xAEF748  mov   eax, [rdx+0x30]          0xAEF75C  add  rax, rax
/// 0xAEF74B  cmp   eax, 0x3F                0xAEF75F  or   rax, rsi
/// 0xAEF74E  ja    0xAEF768                 0xAEF762  mov  [rdx+0x28], rax
/// 0xAEF750  inc   eax                      0xAEF768  mov  rdi, rdx
/// 0xAEF752  mov   [rdx+0x30], eax          0xAEF76B  mov  edx, 1
/// 0xAEF755  inc   dword [rdx+0x24]         0xAEF770  call 0x733A0
/// ```
///
/// We borrow it as a primitive rather than reimplementing the update. ⚠ Its *name* is a vtable
/// slot on an unrelated class; the identity that matters is the body above, not the class.
pub const WRITE_ONE_BIT: u64 = 0xAE_F740;
/// `FUN_00073430(bitstream)` - **read one bit**, MSB-first, advancing `+0x24`. `hot,leaf`;
/// `uVar4 = bs[0x28]; bs[0x28] = uVar4*2; bs[0x30]++; bs[0x24]++; return uVar4 >> 0x3F`.
pub const READ_BIT: u64 = 0x7_3430;
/// ⭐ `FUN_002C9050(bitstream, object, &size, tag, 1)` - the **replicated** decode wrapper, and
/// literally the function `FUN_0029C730` runs on an inbound udt 11 `replicate_membership`.
///
/// It differs from [`DECODE_OBJ`] in exactly two ways, both read in the decompile:
/// 1. it consumes **one leading presence bit** ([`READ_BIT`]) and only decodes if it is set;
/// 2. it returns `descriptor_struct_size <= *size` - a **size check, not a decode result** - and
///    writes the struct size back through `size`.
///
/// The body it decodes is the ordinary object grammar: `FUN_002C78B0` and `FUN_002C8680` both call
/// the same inner decoder `FUN_0105DD20` with the same five-slot frame.
pub const DECODE_OBJ_REPLICATED: u64 = 0x2C_9050;
/// Frame slack in `0x56B5F0` is `0xD8`; rounded up.
pub const BS_SIZE: u64 = 0x100;
/// `bitstream+0x24` is the running **bit** position. Pinned by the control: decoding the 13-byte
/// character vector leaves `+0x24 = 97`. (`+0x20` is 64-bit prefetch accounting, not a position.)
pub const BS_BITPOS: u64 = 0x24;

pub const CODEC_LO: u64 = 0x2C_6000;
/// ⚠ **Widened from `0x2C9100` to `0x2C9400` on 2026-08-09**, when [`DECODE_OBJ_REPLICATED`] was
/// wired up and faulted with `memset would clobber the STACK … return=0x2C9148`.
///
/// `0x2C9148` is inside `FUN_002C9050` (body `0x2C9050..0x2C9272`) and it is the **same** dynamic
/// -field bitset as the two sites already allowed: the `else { memset(local_148, 0) }` arm taken
/// when the bitset needs more than `0x20` bytes. `FUN_002C9290` (`..0x2C9377`) is its encode
/// counterpart. The window now ends past both bodies and before the next function.
///
/// ⚠ This is a *guard* being relaxed, so state what still holds: the allowance is still only for
/// **direct `call memset` sites found by a byte scan inside this window**, not for the window
/// itself - an unpopulated pointer reaching `memset` from anywhere else still faults.
pub const CODEC_HI: u64 = 0x2C_9400;

/// One decoded server object, with everything a caller needs to assert **shape**.
///
/// ⚠ The oracle is a decoder, not a semantics engine. It proves that N bits decode into an object
/// with values at the offsets you expect. It cannot tell you whether the value is *meaningful* to
/// the game - keep `[C]` on the encoding and `[U]` on the meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    pub tag: u32,
    /// Bits the client actually consumed, read from `bitstream+0x24`.
    pub bits: u32,
    /// The whole decoded C struct, `struct_size` bytes.
    pub object: Vec<u8>,
}

impl Decoded {
    pub fn qword(&self, offset: usize) -> u64 {
        self.object
            .get(offset..offset + 8)
            .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
            .unwrap_or(0)
    }

    /// Every non-zero qword, by C offset. **This is the check people skip** - a stray non-zero
    /// qword means a bit-grammar error that would otherwise reach the wire.
    pub fn nonzero_qwords(&self) -> Vec<(usize, u64)> {
        (0..self.object.len().saturating_sub(7))
            .step_by(8)
            .map(|o| (o, self.qword(o)))
            .filter(|(_, v)| *v != 0)
            .collect()
    }

    pub fn nonzero_offsets(&self) -> Vec<usize> {
        self.nonzero_qwords().into_iter().map(|(o, _)| o).collect()
    }

    /// Fail unless the set of non-zero qword offsets is **exactly** `expected`.
    pub fn require_nonzero_qwords(&self, expected: &[usize]) -> Result<()> {
        let got: BTreeSet<usize> = self.nonzero_offsets().into_iter().collect();
        let want: BTreeSet<usize> = expected.iter().copied().collect();
        if got == want {
            return Ok(());
        }
        let fmt = |s: &BTreeSet<usize>| {
            s.iter().map(|o| format!("+{o:#05x}")).collect::<Vec<_>>().join(" ")
        };
        Err(OracleError::Shape(format!(
            "tag {:#010x}: non-zero qwords are [{}], expected [{}] - a stray non-zero qword is a \
             bit-grammar bug that would otherwise reach the wire",
            self.tag,
            fmt(&got),
            fmt(&want)
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encoded {
    pub tag: u32,
    pub bits: u32,
    pub payload: Vec<u8>,
}

pub struct ObjectOracle {
    s: SchemaHarness,
}

impl ObjectOracle {
    /// Build an oracle for `tag`. **The positive control runs first** (memoised across the
    /// process): if it has not passed, this returns [`OracleError::ControlFailed`] and no decode
    /// can be reported. That is the skill's first non-negotiable, enforced in code.
    pub fn new(tag: u32) -> Result<Self> {
        control::require_object_control()?;
        Self::new_unchecked(tag)
    }

    /// [`Self::new`] with typed-value types resolvable (see [`SchemaHarness::new_with_types`]).
    pub fn with_types(tag: u32, value_types: &[u32]) -> Result<Self> {
        control::require_object_control()?;
        Self::new_unchecked_with(tag, value_types)
    }

    /// Construct without running the control. **Only** the control itself may call this.
    pub(crate) fn new_unchecked(tag: u32) -> Result<Self> {
        Self::new_unchecked_with(tag, &[])
    }

    fn new_unchecked_with(tag: u32, value_types: &[u32]) -> Result<Self> {
        let mut s = SchemaHarness::new_with_types(tag, 1, 0, value_types)?;
        let memset = harness::addrs_for(&s.image.sha256())
            .ok_or_else(|| OracleError::Eboot("no harness address set for this image".into()))?
            .memset;
        let (lo, hi) = (s.c.codec_lo, s.c.codec_hi);
        let sites = harness::direct_call_sites(&s.image, memset, lo, hi)?;
        if sites.is_empty() {
            return Err(OracleError::Eboot(format!(
                "no direct `call {memset:#x}` sites in {lo:#x}..{hi:#x} - the byte scan is broken, \
                 and without a positive control an empty result is not evidence"
            )));
        }
        s.h.allow_stack_memset_from(sites);
        Ok(Self { s })
    }

    pub fn tag(&self) -> u32 {
        self.s.tag
    }
    pub fn struct_size(&self) -> usize {
        self.s.struct_size
    }
    pub fn harness(&mut self) -> &mut Harness {
        &mut self.s.h
    }

    // ── arena observability: the numbers a leak control watches ──────────────────────────────────

    /// Bytes of permanent scaffolding - the registry, the fake manager, the definition record.
    /// Everything above this is one call's scratch and is released by the next call.
    pub fn scaffolding_bytes(&self) -> u64 {
        self.s.transient_mark() - harness::HEAP
    }

    /// Bytes currently committed in the bump arena.
    pub fn heap_used(&self) -> u64 {
        self.s.h.heap_used()
    }

    /// ⭐ The furthest the arena has ever reached. **Flat across iterations ⇒ no leak.** Rising ⇒
    /// the arena is being consumed and the process has a wall-clock deadline it does not know about.
    pub fn heap_high_water(&self) -> u64 {
        self.s.h.heap_high_water()
    }

    /// Allocations the **guest's own code** has made. The rewind's safety precondition is that this
    /// stays zero across a codec call; the leak control asserts it rather than assuming it.
    pub fn guest_allocs(&self) -> u64 {
        self.s.h.guest_allocs()
    }

    /// ⛔ **The leak control's inversion knob** - see
    /// [`SchemaHarness::set_rewind_enabled`](crate::schema::SchemaHarness::set_rewind_enabled).
    /// Never call this in a server.
    pub fn set_rewind_enabled(&mut self, on: bool) {
        self.s.set_rewind_enabled(on);
    }

    fn bitstream(&mut self, buf: u64, len: u64) -> Result<u64> {
        let bs = self.s.h.alloc(BS_SIZE)?;
        self.s.h.write_u32(bs + 0x14, 1)?; // alignment 1, as 0x56B646
        self.s.h.call(self.s.c.bs_ctor, &[bs, buf, len])?;
        Ok(bs)
    }

    /// Decode a QueueZ object payload exactly as `0x56B5F0` does.
    ///
    /// ⚠ Returns [`OracleError::ShortPayload`] when the client read more bits than the payload
    /// carries. Such a decode still "succeeds" - that is precisely the trap.
    pub fn decode(&mut self, payload: &[u8]) -> Result<Decoded> {
        self.s.begin_call()?;
        let size = self.s.struct_size.max(16) as u64;
        let obj = self.s.h.alloc(size)?;
        let buf = self.s.h.alloc(payload.len().max(16) as u64)?;
        self.s.h.write(buf, payload)?;
        let bs = self.bitstream(buf, payload.len() as u64)?;
        self.s.h.call(self.s.c.bs_begin_read, &[bs])?;
        let rc = (self.s.h.call(self.s.c.decode_obj, &[u64::from(self.s.tag), bs, obj, obj, 1])? & 0xFF) as u8;
        let bits = self.s.h.read_u32(bs + BS_BITPOS)?;
        self.s.h.call(self.s.c.bs_end, &[bs])?;
        if rc != 1 {
            return Err(OracleError::CodecReturned { func: self.s.c.decode_obj, rc });
        }
        let available = payload.len() * 8;
        if bits as usize > available {
            return Err(OracleError::ShortPayload { bits_read: bits, bits_available: available });
        }
        Ok(Decoded { tag: self.s.tag, bits, object: self.s.h.read(obj, self.s.struct_size)? })
    }

    /// Encode one server-object body. `object` seeds the decoded C struct; the rest is zero.
    pub fn encode(&mut self, object: &[u8], capacity: u64) -> Result<Encoded> {
        self.s.begin_call()?;
        let obj = self.s.make_object(object)?;
        let buf = self.s.h.alloc(capacity)?;
        let bs = self.bitstream(buf, capacity)?;
        self.s.h.call(self.s.c.bs_begin_write, &[bs, 1])?;
        let rc = (self.s.h.call(self.s.c.encode_obj, &[u64::from(self.s.tag), obj, bs, 1])? & 0xFF) as u8;
        if rc != 1 {
            return Err(OracleError::CodecReturned { func: self.s.c.encode_obj, rc });
        }
        let bits = self.s.h.read_u32(bs + BS_BITPOS)?;
        // ⚠ 0x72560 takes an OUT param in rsi (`mov [rsi], ecx` at 0x7263E). Omitting it leaves a
        // stale rsi and the flush stores through it - which surfaces as WRITE_UNMAPPED from a
        // function that looks argument-less.
        let scratch = self.s.h.alloc(8)?;
        self.s.h.call(self.s.c.bs_flush, &[bs, scratch])?;
        let nbytes = ((bits as usize) + 7) / 8;
        Ok(Encoded { tag: self.s.tag, bits, payload: self.s.h.read(buf, nbytes)? })
    }

    /// ⭐ Encode a **replicated** body - `{1 presence bit}{the ordinary object body}`.
    ///
    /// This is what `FUN_002C9290` writes, composed out of the client's own primitives rather than
    /// reimplemented:
    ///
    /// ```text
    /// FUN_002C9290(_, obj, tag, bs, mask, flags):
    ///     bit = mask[0] & 1
    ///     write it   (inline accumulator update, or FUN_000733A0(bs, bit, 1) once 64 bits are in)
    ///     if bit:  FUN_002C7510(tag, mask, 0, obj, bs, flags)
    /// ```
    ///
    /// and `FUN_002C7510` is [`ENCODE_OBJ`]'s inner `FUN_002C8420` with the caller's mask instead
    /// of an all-ones one: the two build the **identical five-slot frame**
    /// `{mask, depth, depth+1, obj, bs}` and call the same `FUN_0105B410`. `FUN_002C8420`'s only
    /// extra act is `FUN_002C6C00(&mask)` = *set every bit*. ⇒ with "everything changed", which is
    /// what a full-state replication means, `write_bit(1) + ENCODE_OBJ` **is** `FUN_002C9290`.
    ///
    /// ⭐ And that composition is not a construction of ours - it is shipped. `FUN_00628810` is a
    /// client encoder that does exactly it: `write_one_bit(bs, this[0xC])`, then
    /// `if (this[0xC]) { …; FUN_002C8420(tag, this+8, bs, 1); }`.
    ///
    /// ⚠ Nothing here hand-packs a bit. The presence bit is written by the client's own
    /// [`WRITE_ONE_BIT`], so the alignment and the MSB-first order are the client's, not ours - and
    /// [`Self::round_trip_replicated`] proves it by handing the result to the client's own
    /// [`DECODE_OBJ_REPLICATED`].
    pub fn encode_replicated(&mut self, object: &[u8], capacity: u64) -> Result<Encoded> {
        self.s.begin_call()?;
        let obj = self.s.make_object(object)?;
        let buf = self.s.h.alloc(capacity)?;
        let bs = self.bitstream(buf, capacity)?;
        self.s.h.call(self.s.c.bs_begin_write, &[bs, 1])?;
        // The presence bit: "field 0 changed". A full-state replication says yes.
        // `WRITE_ONE_BIT` takes it from `src + 8`.
        let src = self.s.h.alloc(16)?;
        self.s.h.write(src, &[0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0])?;
        self.s.h.call(self.s.c.write_one_bit, &[src, 0, bs])?;
        let rc = (self.s.h.call(self.s.c.encode_obj, &[u64::from(self.s.tag), obj, bs, 1])? & 0xFF) as u8;
        if rc != 1 {
            return Err(OracleError::CodecReturned { func: self.s.c.encode_obj, rc });
        }
        let bits = self.s.h.read_u32(bs + BS_BITPOS)?;
        let scratch = self.s.h.alloc(8)?;
        self.s.h.call(self.s.c.bs_flush, &[bs, scratch])?;
        let nbytes = (bits as usize).div_ceil(8);
        Ok(Encoded { tag: self.s.tag, bits, payload: self.s.h.read(buf, nbytes)? })
    }

    /// Run the client's replicated encoder with an explicit change mask (56578 0x2C9D20).
    /// Unlike `encode_replicated`, this calls the mask-taking entry directly. The supplied
    /// low-bit-first words are represented as an external dynamic-flags object, avoiding
    /// the live allocator. Bit 0 is the root presence bit, the field walk starts at bit 1.
    pub fn encode_replicated_masked(
        &mut self,
        object: &[u8],
        mask: &[u32],
        capacity: u64,
    ) -> Result<Encoded> {
        let d = self
            .s
            .index
            .get(self.s.tag)
            .ok_or_else(|| OracleError::Shape("missing descriptor".into()))?;
        let n = self
            .s
            .image
            .u32(d.anchor + 0x1C)
            .map_err(|e| OracleError::Eboot(e.to_string()))?
            + 1;
        if n > 0x1FFF
            || mask.len() != n.div_ceil(32) as usize
            || (n % 32 != 0 && mask[mask.len() - 1] >> (n % 32) != 0)
        {
            return Err(OracleError::Shape(format!(
                "expected {n} mask bits with zero padding (external mask capacity 8191)"
            )));
        }
        self.s.begin_call()?;
        let obj = self.s.make_object(object)?;
        let words = self.s.h.alloc(mask.len() as u64 * 4)?;
        for (i, word) in mask.iter().enumerate() {
            self.s.h.write_u32(words + i as u64 * 4, *word)?;
        }
        let flags = self.s.h.alloc(16)?;
        self.s.h.write_u64(flags, words)?;
        // 0x2C9D20 and 0x101C220: signed low 14 bits -2 = external words,
        // signed bits 14..27 = bit count. The upper four bits are allocator bookkeeping.
        self.s.h.write_u32(flags + 8, (n << 14) | 0x3FFE)?;
        let buf = self.s.h.alloc(capacity)?;
        let bs = self.bitstream(buf, capacity)?;
        self.s.h.call(self.s.c.bs_begin_write, &[bs, 1])?;
        let entry = self.s.c.encode_obj_replicated;
        let rc = (self
            .s
            .h
            .call(entry, &[0, obj, u64::from(self.s.tag), bs, flags, 1])?
            & 255) as u8;
        if rc != 1 {
            return Err(OracleError::CodecReturned { func: entry, rc });
        }
        let bits = self.s.h.read_u32(bs + BS_BITPOS)?;
        let scratch = self.s.h.alloc(8)?;
        self.s.h.call(self.s.c.bs_flush, &[bs, scratch])?;
        Ok(Encoded {
            tag: self.s.tag,
            bits,
            payload: self.s.h.read(buf, (bits as usize).div_ceil(8))?,
        })
    }

    /// ⭐ Decode a **replicated** body by running the client's own [`DECODE_OBJ_REPLICATED`],
    /// argument for argument as `FUN_0029C730` calls it.
    ///
    /// `FUN_0029C730` seeds the output image with the client's **current** membership
    /// (`memcpy(image, ac+0xE70, 0x5038)`) before decoding, because a replicated body may carry
    /// only what changed. `seed` reproduces that; pass zeros for "the client has nothing yet",
    /// which is the state at join.
    ///
    /// ⚠ The return value of `0x2C9050` is `struct_size <= *size`, **not** decode success - so it
    /// is reported but never treated as one. The real check is whether the image came back the way
    /// it went in ([`Self::round_trip_replicated`]).
    pub fn decode_replicated(&mut self, payload: &[u8], seed: &[u8]) -> Result<Decoded> {
        self.s.begin_call()?;
        let struct_size = self.s.struct_size;
        if !seed.is_empty() && seed.len() != struct_size {
            return Err(OracleError::Shape(format!(
                "tag {:#010x}: seed image is {} bytes, the struct is {struct_size} ({struct_size:#x}) \
                 - FUN_0029C730 seeds with exactly one struct or nothing",
                self.s.tag,
                seed.len()
            )));
        }
        let obj = self.s.h.alloc(struct_size.max(16) as u64)?;
        if !seed.is_empty() {
            self.s.h.write(obj, seed)?;
        }
        // `local_528c = 0x5038` - the caller states the buffer size and 0x2C9050 writes it back.
        let size_ptr = self.s.h.alloc(8)?;
        self.s.h.write_u32(size_ptr, struct_size as u32)?;
        let buf = self.s.h.alloc(payload.len().max(16) as u64)?;
        self.s.h.write(buf, payload)?;
        let bs = self.bitstream(buf, payload.len() as u64)?;
        self.s.h.call(self.s.c.bs_begin_read, &[bs])?;
        let rc = (self
            .s
            .h
            .call(self.s.c.decode_obj_replicated, &[bs, obj, size_ptr, u64::from(self.s.tag), 1])?
            & 0xFF) as u8;
        let bits = self.s.h.read_u32(bs + BS_BITPOS)?;
        self.s.h.call(self.s.c.bs_end, &[bs])?;
        if rc != 1 {
            return Err(OracleError::CodecReturned { func: self.s.c.decode_obj_replicated, rc });
        }
        let available = payload.len() * 8;
        if bits as usize > available {
            return Err(OracleError::ShortPayload { bits_read: bits, bits_available: available });
        }
        Ok(Decoded { tag: self.s.tag, bits, object: self.s.h.read(obj, struct_size)? })
    }

    /// [`Self::encode_replicated`] then [`Self::decode_replicated`], requiring the C object back
    /// unchanged. The decode half is the client's own udt-11 decoder, so a passing round trip is
    /// the client accepting our bytes - not our encoder agreeing with itself.
    pub fn round_trip_replicated(
        &mut self,
        object: &[u8],
        capacity: u64,
    ) -> Result<(Encoded, Decoded)> {
        let enc = self.encode_replicated(object, capacity)?;
        let dec = self.decode_replicated(&enc.payload, &[])?;
        let n = object.len().min(self.s.struct_size);
        if dec.object[..n] != object[..n] {
            let first = (0..n).find(|&i| dec.object[i] != object[i]).unwrap_or(0);
            return Err(OracleError::Shape(format!(
                "replicated round trip differs at +{first:#05x}: sent {:02x}, got {:02x}",
                object[first], dec.object[first]
            )));
        }
        Ok((enc, dec))
    }

    /// Encode, then decode the result back and require the C object to be identical. This is the
    /// round trip `queuez-object-oracle.py` performs by default.
    pub fn round_trip(&mut self, object: &[u8], capacity: u64) -> Result<(Encoded, Decoded)> {
        let enc = self.encode(object, capacity)?;
        let dec = self.decode(&enc.payload)?;
        let n = object.len().min(self.s.struct_size);
        if dec.object[..n] != object[..n] {
            let first = (0..n).find(|&i| dec.object[i] != object[i]).unwrap_or(0);
            return Err(OracleError::Shape(format!(
                "round trip differs at +{first:#05x}: sent {:02x}, got {:02x}",
                object[first], dec.object[first]
            )));
        }
        Ok((enc, dec))
    }
}

/// Pack an MSB-first bit string (`"1010…"`) into bytes, zero-padded to a byte.
///
/// The bit grammar the client reads: for each field in order, a presence bit; if present, the value
/// inline, MSB-first, packed MSB-first.
pub fn pack_bits(bits: &str) -> Vec<u8> {
    let mut s: String = bits.chars().filter(|c| *c == '0' || *c == '1').collect();
    while s.len() % 8 != 0 {
        s.push('0');
    }
    s.as_bytes()
        .chunks(8)
        .map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | u8::from(b == b'1')))
        .collect()
}
