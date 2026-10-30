// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! ⭐ The **udt 5 `sensor_auth_update`** payload - the wire message that writes the activity
//! lifetime phase (`record+0x1AC`) the player-spawn gate `FUN_00414730` reads.
//!
//! # Why this is not the generic AH codec
//!
//! Every other AH → client message this project sends is a single schematized struct: the client
//! decodes it with `FUN_002A4090` and the server re-runs that codec's inverse `FUN_002A4140`
//! ([`crate::activity_host`]). **udt 5 is not.** Its handler `FUN_0029D3F0` forwards the raw
//! `(payload, len)` into a simulation-queue element (type `0x13`), and the element's applier
//! `FUN_002A1210` decodes the payload with a *bespoke* replicated walker over schema
//! `0x80800CF7`, then dispatches a bound setter reference on the named globals record. There is no
//! single-descriptor round-trip for the whole frame, and - because the client only ever *receives*
//! udt 5 - there is no client-side encoder to borrow. So the frame is assembled here, bit by bit,
//! and only its two schematized sub-fields are run through the client's own object codec.
//!
//! # The frame `[C]` (472 bits = 59 bytes, one continuous big-endian MSB-first bitstream)
//!
//! Re-derived on `runs/snapshots/rung36-biped-chain.snap` (`d1-replay controls` 20/20, exit 0) and
//! static disassembly (`d1-eboot controls` exit 0); adversarially audited (workflow `wf_f7f6ddbd`
//! plus a dedicated Fable pass). Bit offsets are inclusive-exclusive from bit 0:
//!
//! | bits | field | value | source |
//! |---|---|---|---|
//! | 0..8 | `gen` | must equal `world+0x1D` at receive | gate `cmp cl,[world+0x1D]/jne` @0x2A1458 |
//! | 8..9 | `has_sensor_block` | **0** - skip the 784-byte authority table | `FUN_002A1210` |
//! | 9..41 | `stamp` | 0 | `FUN_002A1210` |
//! | 41..42 | `init` | **1** | `FUN_002A1210` |
//! | 42..43 | `has_entry` | **1** - one named-globals entry follows | entry loop |
//! | 43..98 | name-ref | schema `0x808004B1`, **55 bits** | [`Self::name_ref_bits`] |
//! | 98..130 | `body_bit_length` | **`2 + value_bits`** (= 341 here) | ⭐ audit correction: it counts `fresh`+`presence`+value, NOT just the value |
//! | 130..131 | `fresh` | **1** - default-init the replica before overlay | `fresh=0` redirects to a discarded scratch = silent no-op |
//! | 131..132 | `presence` | **1** | the replicated wrapper's leading presence bit |
//! | 132..471 | value | schema `0x808008A3`, **339 bits** | [`Self::value_bits`] |
//! | 471..472 | terminator | **0** - `has_entry=0` ends the loop and runs the setter post-pass | entry loop |
//!
//! The name-ref selects the globals record: `{u32 fnv_hash, i8 @+4, i16 sub @+6}` where the hash
//! picks a namespace *group* and the record index is `group_base + sub`. For the pvp_moonbase
//! lifetime record that is group 0 (`hash 0xC2DACAF7` in-world), sub **3**.
//!
//! ⛔ **The hash and sub are PER-LOAD and MUST NOT be hardcoded** - the same record's group-0 hash
//! is `0x4786C0E0` in orbit and `0xC2DACAF7` in-world while the content tag stays `0x80A5EA2B`.
//! The server derives them at run time (see the doc on the crate that drives this) and passes them
//! in; this module only assembles what it is given.
//!
//! # The value `[C]`
//!
//! Schema `0x808008A3` is a 1284-byte (`0x504`) struct whose first field is the **4-bit,
//! bias-1** phase enum (`kind 3`): wire raw `phase + 1`, so phase `-1`/NONE = raw 0, phase 3 = raw
//! 4, phase 6 = raw 7. The decoded 1284-byte image is exactly what the setter `FUN_0011DE30`
//! `memcpy`s to `record+0x1AC` (`mov edx,0x504` @0x11DEEB, `call` @0x11DEF0). The spawn gate then
//! accepts `{3,6}` (`mov eax,0x48; bt eax,edi` @0x4147E5) and denies the rest (`mov eax,0x3B7`
//! @0x4147DB).
//!
//! ⭐ **Executed proof of the setter, on `rung36-biped-chain.snap`:** planting a `{0x808008A3,
//! ptr}` box and this crate's value image, `d1-replay replay --func 0x11de30` reaches the store
//! `memcpy(rdi=record+0x1AC, rsi=image, rdx=0x504)` (probe @0x11DEF0), and the getter
//! `FUN_0010C500` reports `al=1 state=3` on a planted-3 record vs `al=0 NONE` on the default.
//!
//! # What is served vs still `[U]`
//!
//! The frame is byte-exact and its two schematized fields round-trip through the client's own
//! codec. What is **not** yet demonstrated is that our slot is in the world's post-pass sweep
//! container, i.e. that `FUN_002CE2A0`'s dispatch actually fires on a fresh, server-delivered
//! element rather than only on content that was resident at load. That is the promotion gate and
//! it is settled **live** (`d1-live watch --rw write` on `record+0x1AC`, identity payload as the
//! null control) - so this ships as `synthetic-minimal`, behind a knob, off by default.

use crate::error::{OracleError, Result};
use crate::object::ObjectOracle;

/// Schema tag of the 4-bit-phase value struct the setter `FUN_0011DE30` copies to `record+0x1AC`.
pub const TAG_SENSOR_VALUE: u32 = 0x8080_08A3;
/// Schema tag of the `{hash, i8, i16}` globals name reference.
pub const TAG_NAME_REF: u32 = 0x8080_04B1;
/// The sensor AUTHORITY table, decoded by `0x2A1C40` into `world+0xA88` when `has_sensor_block`
/// is set (56578: the schema pointer at `0x191A310`, filled at load).
pub const TAG_AUTHORITY: u32 = 0x8080_0CF7;
/// Types a sensor client's state may carry as a kind-`0x22` typed value (a registry handle the
/// schema never reaches statically, so the oracle must be told). `0x8080279A`: the prop command,
/// three `(i32, i32, f32)` channels, in a prop state's command block (`0x808007D9` / `0x80800387`
/// `+0x20` count, `+0x30` type, `+0x40` value). Channels 0-2 at `(1, 1, 1.0)` on The Breach's
/// control and light props open the barrier to the starter rifle (01.24 JOIN210).
/// `0x80801732`: the family-4/type-3 inventory element, carried as a post-game reward display
/// entry in the kind-15 scoreboard's player row (`0x808008D0`). Without it the client's codec
/// faults on the unresolved type (a negative `memset`); with it a one-reward scoreboard image
/// round-trips byte-identical (2,388 bits).
pub const TYPED_VALUE_TYPES: &[u32] = &[0x8080_279A, 0x8080_1732];
/// Entries in the authority table: 65 blocks of 12 bytes after a `u32`.
pub const AUTHORITY_BLOCKS: usize = 65;

/// The setter's `memcpy` width, and thus the value image size. `mov edx,0x504` @0x11DEEB.
pub const VALUE_IMAGE_LEN: usize = 0x504;

/// FNV-1 basis = the hash of the empty string = "unset". The record's default name and its
/// per-entry default hashes are all this value.
pub const FNV1_BASIS: u32 = 0x811C_9DC5;
/// The record's "none" sentinel for a `u32` reference field.
const NONE_U32: u32 = 0xFFFF_FFFF;

/// One `sensor_auth_update` that assigns the activity lifetime phase to a named globals record.
///
/// `gen`, `name_hash` and `name_sub` are all run-time inputs, never constants - see the module
/// doc. `phase` is the signed enum the spawn gate reads (`-1` NONE, `3`, `6`, …); `tail` optionally
/// sets the two `FUN_0010CB60` spawn-precondition fields (`record+0x674`, `record+0x678`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SensorPhaseUpdate {
    /// Replication generation; must equal the client's `world+0x1D` at receive, or the applier
    /// silently skips the frame (`jne` @0x2A145C). A wrong value here is the project's signature
    /// failure mode, so it is a required input with no default.
    pub gen: u8,
    /// FNV-1 hash selecting the namespace group. Per-load; derive at run time.
    pub name_hash: u32,
    /// The name struct's `+4` byte. `0x10` in-world; must not be `-1` (`FUN_001076C0`).
    pub name_i8: u8,
    /// The record index within the group. `3` for the pvp_moonbase lifetime record.
    pub name_sub: i16,
    /// The lifetime phase to write. `3` or `6` pass the spawn gate; `-1` is NONE.
    pub phase: i8,
    /// `Some((id, hash))` also sets `record+0x674`/`record+0x678` (value fields `[8]`/`[9]`), the
    /// second spawn precondition `FUN_0010CB60` checks. `None` leaves them at their defaults.
    pub tail: Option<(u32, u32)>,
    /// The world's OTHER pending sensor entity clients, each sent as a fresh, value-less entry.
    /// Empty reproduces the audited single-entry frame. See [`SensorClientRef`].
    pub clients: Vec<SensorClientRef>,
    /// ⭐ The authority table (`has_sensor_block = 1`). `None` keeps the audited frame.
    pub authority: Option<SensorAuthority>,
}

/// ⭐ The sensor authority table: which machine simulates each of the 65 authority blocks.
///
/// 56578 `0x2A1C40` reads `has_sensor_block`; when set, a 65-bit changed mask (block 0 first),
/// then this struct (tag `0x80800CF7`, 784 bytes: `u32`, then 65 × `{peer {u8 @+1}, u16 seq @+8,
/// bool applied @+0xA}`) through the client's own codec, copied to `world+0xA88`. `0x2A1AB0`
/// applies each changed entry: a new `seq` opens a transfer (`0x240A60`), `applied` completes it
/// (`0x240AC0`, the game interface's authority hook). With the table all zero no machine simulates
/// mission objects: the objective handler `0x40A8D0` then never computes "this objective is mine".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SensorAuthority {
    /// Bit `i` set = entry `i` changed (65 bits used).
    pub changed: u128,
    /// The 784-byte struct image.
    pub image: Vec<u8>,
}

impl SensorAuthority {
    /// Every block to `machine` (the 6-bit machine id the client calls local, e.g. 14), with
    /// sequence `seq`, applied.
    pub fn all_to(machine: u8, seq: u16) -> Self {
        Self::all_to_with(machine, seq, true)
    }

    /// [`Self::all_to`] with the `applied` flag chosen. The applier (`0x2A1AB0`) starts a transfer
    /// on a new `seq` (block bit set in `world+0xA70` and `+0xA7C`) and ends it when `applied` is
    /// set (both bits cleared), so a table sent already applied leaves no block held. A prop's
    /// "may I create it" test (`0x108360` → `0x2A1A80`) wants its block's `+0xA70` bit set and
    /// `+0xA7C` clear: the transfer started and acknowledged by the client, not yet ended.
    pub fn all_to_with(machine: u8, seq: u16, applied: bool) -> Self {
        let mut image = vec![0u8; 4 + 12 * AUTHORITY_BLOCKS];
        for i in 0..AUTHORITY_BLOCKS {
            let e = 4 + 12 * i;
            image[e + 1] = machine;
            image[e + 8..e + 10].copy_from_slice(&seq.to_le_bytes());
            image[e + 0xA] = u8::from(applied);
        }
        Self { changed: (1u128 << AUTHORITY_BLOCKS) - 1, image }
    }
}

/// One pending sensor entity client the frame must deliver for the walker terminator to accept it.
///
/// ⭐ Build 56578, read live 2026-10-13: the walker `0x2D0CD0` clears a client's pending byte
/// (`+0x15`) only when the frame carries an entry for it, and the terminator `0x2D1600` rejects the
/// whole frame while any client the local machine must receive (owner `+0x60` == the local id or
/// `> 0x7F`, i.e. unassigned) is still pending. pvp_moonbase has 97 such clients in pool
/// "sensor entity clients 0" (tag `0xF42`); the audited frame delivered one. Per entry, after the
/// name ref and `body_bit_length`: if the client's A type (`+0xC`) is set, `fresh` + the replicated
/// value (a presence bit, then the object only when it is set); if its B type (`+8`) is set, a
/// `has_b` bit. `fresh=1, presence=0, has_b=0` default-initialises the client and clears pending.
/// Names are per-load like the phase record's, so the list comes from the live client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SensorClientRef {
    pub name_hash: u32,
    pub name_i8: u8,
    pub name_sub: i16,
    /// The client's A type (`+0xC`) is set, so the entry carries `fresh` + presence.
    pub has_a: bool,
    /// The client's B type (`+8`) is set, so the entry ends with a `has_b = 0` bit.
    pub has_b: bool,
    /// `Some((A type tag, value image))` sends the A value instead of leaving the default:
    /// `fresh = 1, presence = 1`, then the object encoded by the client's own codec. Build 56578's
    /// type-12 client bundles (`0xC2DACAF7` subs 7-18, A `0x808022A5`) need one: the player is
    /// linked to the bundle whose value `+0x40` equals its identity, and the spawn gate reads the
    /// bundle's `+9`.
    pub a_value: Option<(u32, Vec<u8>)>,
    /// `Some((B type tag, value image))` sends the B value: `has_b = 1`, a flag bit the walker
    /// stores at the client's `+0x16` (0, as live B-carrying clients hold), `fresh = 1`,
    /// `presence = 1`, the object, then the `u32` stored at `+0x10` (0) - walker `0x2D0F63..0x2D1012`. The type-12
    /// bundle's spawn-gate record is its B value (`obj+0x200` = 0x290 live), not A.
    pub b_value: Option<(u32, Vec<u8>)>,
}

impl SensorClientRef {
    /// Parse one `hash i8 sub has_a has_b [a_tag a_image_hex [b_tag b_image_hex]]` line
    /// (`0xC2DACAF7 0x8 0 1 1`); `#` starts a comment.
    pub fn parse_line(line: &str) -> Option<Self> {
        let line = line.split('#').next()?.trim();
        if line.is_empty() {
            return None;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        let num = |s: &str| -> Option<i64> {
            s.strip_prefix("0x").map_or_else(|| s.parse().ok(), |h| i64::from_str_radix(h, 16).ok())
        };
        Some(Self {
            name_hash: num(f.first()?)? as u32,
            name_i8: num(f.get(1)?)? as u8,
            name_sub: num(f.get(2)?)? as i16,
            has_a: num(f.get(3)?)? != 0,
            has_b: num(f.get(4)?)? != 0,
            a_value: match (f.get(5), f.get(6)) {
                (Some(t), Some(h)) => Some((num(t)? as u32, hex_bytes(h)?)),
                _ => None,
            },
            b_value: match (f.get(7), f.get(8)) {
                (Some(t), Some(h)) => Some((num(t)? as u32, hex_bytes(h)?)),
                _ => None,
            },
        })
    }

    fn name_ref_image(&self) -> Vec<u8> {
        let mut b = vec![0u8; 8];
        put_u32(&mut b, 0, self.name_hash);
        b[4] = self.name_i8;
        b[6..8].copy_from_slice(&self.name_sub.to_le_bytes());
        b
    }
}

impl SensorPhaseUpdate {
    /// The 1284-byte value image the setter copies. Everything is the record's schema default
    /// except the phase byte and, optionally, the two tail fields - which is exactly the image
    /// that reproduces the audited value block through the client's own codec.
    pub fn value_image(&self) -> Vec<u8> {
        let mut b = vec![0u8; VALUE_IMAGE_LEN];
        b[0] = self.phase as u8;
        b[1] = 0xFF;
        b[2] = 0x01;
        put_u32(&mut b, 0x04, NONE_U32);
        put_u32(&mut b, 0x08, FNV1_BASIS);
        put_u32(&mut b, 0x0C, NONE_U32);
        let (tail_id, tail_hash) = self.tail.unwrap_or((NONE_U32, FNV1_BASIS));
        put_u32(&mut b, 0x4C8, tail_id);
        put_u32(&mut b, 0x4CC, tail_hash);
        put_u32(&mut b, 0x4D8, NONE_U32);
        put_u32(&mut b, 0x4DC, FNV1_BASIS);
        put_u32(&mut b, 0x4E0, 0xFFFF_00FF);
        put_u32(&mut b, 0x4E4, FNV1_BASIS);
        b
    }

    /// The 8-byte name-reference image: `{u32 hash, i8 @+4, i16 @+6}`.
    pub fn name_ref_image(&self) -> Vec<u8> {
        let mut b = vec![0u8; 8];
        put_u32(&mut b, 0, self.name_hash);
        b[4] = self.name_i8;
        b[6..8].copy_from_slice(&self.name_sub.to_le_bytes());
        b
    }

    /// Encode the value struct through the client's own object codec, returning `(bit_count,
    /// payload)`. The payload is MSB-first with the final byte zero-padded.
    pub fn value_bits(&self) -> Result<(u32, Vec<u8>)> {
        let e = ObjectOracle::new(TAG_SENSOR_VALUE)?.encode(&self.value_image(), 0x1_0000)?;
        Ok((e.bits, e.payload))
    }

    /// Encode the name reference through the client's own object codec.
    pub fn name_ref_bits(&self) -> Result<(u32, Vec<u8>)> {
        let e = ObjectOracle::new(TAG_NAME_REF)?.encode(&self.name_ref_image(), 0x1_0000)?;
        Ok((e.bits, e.payload))
    }

    /// ⭐ Assemble the full 59-byte `sensor_auth_update` payload.
    ///
    /// The two schematized sub-fields are encoded by the client's own codec; everything else is the
    /// fixed framing read out of `FUN_002A1210`. The result is byte-exact against the audited
    /// payloads (see [`controls`]).
    pub fn encode(&self) -> Result<Vec<u8>> {
        let (name_bits, name_pay) = self.name_ref_bits()?;
        let (value_bits, value_pay) = self.value_bits()?;
        let mut w = BitWriter::default();
        w.put(u64::from(self.gen), 8);
        match &self.authority {
            None => w.put(0, 1), // has_sensor_block = 0
            Some(a) => {
                w.put(1, 1); // has_sensor_block
                for i in 0..AUTHORITY_BLOCKS {
                    w.put(((a.changed >> i) & 1) as u64, 1);
                }
                // `0x2A1C40` decodes it with the REPLICATED decoder `0x2C9AE0` (presence bit, then
                // the object), not the plain one the walker's A values use behind their own bits.
                let e = ObjectOracle::new(TAG_AUTHORITY)?.encode_replicated(&a.image, 0x1_0000)?;
                w.raw(&e.payload, e.bits as usize);
            }
        }
        w.put(0, 32); // stamp
        w.put(1, 1); // init
        w.put(1, 1); // has_entry
        w.raw(&name_pay, name_bits as usize);
        w.put(u64::from(2 + value_bits), 32); // body_bit_length = fresh + presence + value
        w.put(1, 1); // fresh
        w.put(1, 1); // presence
        w.raw(&value_pay, value_bits as usize);
        if !self.clients.is_empty() {
            let mut name = ObjectOracle::new(TAG_NAME_REF)?;
            for c in &self.clients {
                let e = name.encode(&c.name_ref_image(), 0x1_0000)?;
                let value = match (&c.a_value, c.has_a) {
                    (Some((tag, image)), true) => {
                        Some(ObjectOracle::with_types(*tag, TYPED_VALUE_TYPES)?.encode(image, 0x1_0000)?)
                    }
                    _ => None,
                };
                let value_bits = value.as_ref().map_or(0, |v| v.bits);
                let b_val = match (&c.b_value, c.has_b) {
                    (Some((tag, image)), true) => {
                        Some(ObjectOracle::with_types(*tag, TYPED_VALUE_TYPES)?.encode(image, 0x1_0000)?)
                    }
                    _ => None,
                };
                // B part: has_b, then (if set) flag + fresh + presence + object + u32.
                let b_bits = u32::from(c.has_b) + b_val.as_ref().map_or(0, |v| 3 + v.bits + 32);
                let body = 2 * u32::from(c.has_a) + value_bits + b_bits;
                w.put(1, 1); // has_entry
                w.raw(&e.payload, e.bits as usize);
                w.put(u64::from(body), 32);
                if c.has_a {
                    w.put(1, 1); // fresh
                    match &value {
                        Some(v) => {
                            w.put(1, 1); // presence
                            w.raw(&v.payload, v.bits as usize);
                        }
                        None => w.put(0, 1), // presence = 0: keep the default fresh installed
                    }
                }
                if c.has_b {
                    match &b_val {
                        Some(v) => {
                            w.put(1, 1); // has_b
                            w.put(0, 1); // flag -> client +0x16
                            w.put(1, 1); // fresh
                            w.put(1, 1); // presence
                            w.raw(&v.payload, v.bits as usize);
                            w.put(0, 32); // -> client +0x10
                        }
                        None => w.put(0, 1), // has_b = 0
                    }
                }
            }
        }
        w.put(0, 1); // terminator (has_entry = 0)
        if !self.clients.is_empty() || self.authority.is_some() {
            // Zero-pad to a byte: the walker stops at the terminator and never reads past it. The
            // audited single-entry frame is exactly 472 bits and stays unpadded (and checked). A
            // list with only `@authority`/`@phase` and no rows still carries the authority table,
            // whose width is not a byte multiple (run sense49: 5,966 bits, refused unpadded).
            w.pad_to_byte();
        }
        w.finish().map_err(|n| {
            OracleError::Shape(format!(
                "sensor frame is {n} bits, not a byte multiple - a field width is wrong"
            ))
        })
    }
}

fn hex_bytes(h: &str) -> Option<Vec<u8>> {
    if h.len() % 2 != 0 {
        return None;
    }
    (0..h.len()).step_by(2).map(|i| u8::from_str_radix(h.get(i..i + 2)?, 16).ok()).collect()
}

fn put_u32(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

/// A big-endian, MSB-first bit accumulator. The whole `sensor_auth_update` frame is one such
/// stream with no per-field byte alignment, so fields must be concatenated at the bit level.
#[derive(Default)]
struct BitWriter {
    bits: Vec<u8>,
}

impl BitWriter {
    /// Append the low `n` bits of `val`, most-significant first.
    fn put(&mut self, val: u64, n: u32) {
        for k in (0..n).rev() {
            self.bits.push(((val >> k) & 1) as u8);
        }
    }

    /// Append the first `nbits` bits of a byte payload, MSB-first within each byte.
    fn raw(&mut self, payload: &[u8], nbits: usize) {
        for i in 0..nbits {
            self.bits.push((payload[i >> 3] >> (7 - (i & 7))) & 1);
        }
    }

    fn pad_to_byte(&mut self) {
        while self.bits.len() % 8 != 0 {
            self.put(0, 1);
        }
    }

    /// Pack the accumulated bits into bytes, or return the bit count if it is not a byte multiple.
    fn finish(self) -> std::result::Result<Vec<u8>, usize> {
        if self.bits.len() % 8 != 0 {
            return Err(self.bits.len());
        }
        let mut out = vec![0u8; self.bits.len() / 8];
        for (i, bit) in self.bits.iter().enumerate() {
            if *bit != 0 {
                out[i >> 3] |= 1 << (7 - (i & 7));
            }
        }
        Ok(out)
    }
}

// ── controls ─────────────────────────────────────────────────────────────────────────────────────

/// The pinned in-world name for the pvp_moonbase lifetime record, used ONLY by the controls to
/// reproduce the audited payloads. Never used to build a live frame - the live hash is derived.
const AUDIT_NAME_HASH: u32 = 0xC2DA_CAF7;
const AUDIT_NAME_I8: u8 = 0x10;
const AUDIT_NAME_SUB: i16 = 3;

/// The audited phase-3 payload (workflow `wf_f7f6ddbd`, re-derived on `rung36-biped-chain.snap`).
const AUDIT_P3: &str = "0000000000785b595ee46000c00000557417fffffff811c9dc57fffffff020000000000811c9dc5000000003fffffffc08e4ee2807fff811c9dc50";
/// The audited phase-3 + tail-fix payload (`record+0x674 = 0`, `record+0x678 = 0xC2DACAF7`).
const AUDIT_TF: &str = "0000000000785b595ee46000c00000557417fffffff811c9dc57fffffff020000000001c2dacaf7000000003fffffffc08e4ee2807fff811c9dc50";

#[cfg(test)]
mod typed_value_tests {
    /// ⭐ The Breach activation: a created prop (`0x808007D9`) carrying one prop command with
    /// channels 0-2 at (1, 1, 1.0) round-trips through the client's codec once its type is in the
    /// registry.
    #[test]
    fn a_prop_command_round_trips() {
        if !crate::test_inputs::selected_eboot() {
            return;
        }
        let mut img = vec![0u8; 240];
        img[0..4].copy_from_slice(&1u32.to_le_bytes());
        img[8] = 1;
        img[0xC..0x10].copy_from_slice(&0x811C_9DC5u32.to_le_bytes());
        img[0x10] = 0xFF;
        img[0x12..0x14].copy_from_slice(&(-1i16).to_le_bytes());
        img[0x20] = 1;
        img[0x30..0x34].copy_from_slice(&0x8080_279Au32.to_le_bytes());
        for ch in 0..3 {
            let at = 0x40 + 12 * ch;
            img[at..at + 4].copy_from_slice(&1i32.to_le_bytes());
            img[at + 4..at + 8].copy_from_slice(&1i32.to_le_bytes());
            img[at + 8..at + 12].copy_from_slice(&1.0f32.to_le_bytes());
        }
        let mut o = crate::object::ObjectOracle::with_types(0x8080_07D9, super::TYPED_VALUE_TYPES).unwrap();
        let e = o.encode(&img, 0x1_0000).unwrap();
        assert_eq!(o.decode(&e.payload).unwrap().object, img);
    }
}

#[cfg(test)]
mod authority_tests {
    #[test]
    fn all_to_fills_every_block_and_marks_them_changed() {
        let a = super::SensorAuthority::all_to(14, 1);
        assert_eq!(a.image.len(), 784);
        assert_eq!(a.changed.count_ones(), 65);
        for i in 0..65 {
            let e = 4 + 12 * i;
            assert_eq!((a.image[e + 1], a.image[e + 8], a.image[e + 0xA]), (14, 1, 1));
        }
    }
}

fn audit(phase: i8, tail: Option<(u32, u32)>) -> SensorPhaseUpdate {
    SensorPhaseUpdate {
        gen: 0,
        name_hash: AUDIT_NAME_HASH,
        name_i8: AUDIT_NAME_I8,
        name_sub: AUDIT_NAME_SUB,
        phase,
        tail,
        clients: Vec::new(),
        authority: None,
    }
}

/// One control's outcome, matching the shape [`crate::activity_host`] controls use.
pub struct Control {
    pub name: &'static str,
    pub passed: bool,
    pub detail: String,
}

/// ⭐ The mandatory falsifiable controls the Fable audit required before these bytes may be pinned.
///
/// Each runs the client's own codec (so a broken eboot fails loudly) and every positive carries an
/// inversion. Returns the outcomes; the caller decides the exit code.
pub fn run() -> Vec<Control> {
    let mut c = Vec::new();
    let mut push = |name: &'static str, passed: bool, detail: String| {
        c.push(Control { name, passed, detail });
    };

    // 1. Byte-exact reproduction of the audited phase-3 and tail-fix payloads, and the single-nibble
    //    phase relationship, all assembled from the client's own codec output.
    match (audit(3, None).encode(), audit(6, None).encode(), audit(-1, None).encode()) {
        (Ok(p3), Ok(p6), Ok(id)) => {
            let exp_p3 = hex(AUDIT_P3);
            push(
                "phase-3 payload is byte-exact against the audited frame",
                p3 == exp_p3,
                format!("{} bytes, matches={}", p3.len(), p3 == exp_p3),
            );
            // phase is the value's first nibble = the LOW nibble of byte 16 (bits 132..136); the
            // HIGH nibble is the constant {bit128, bit129, fresh, presence} = 0x7.
            let diff: Vec<usize> = p3.iter().zip(&p6).enumerate().filter(|(_, (a, b))| a != b).map(|(i, _)| i).collect();
            push(
                "phase 3 vs 6 differ only in the value's first nibble (byte 16 low nibble)",
                diff == vec![16] && (p3[16] & 0x0F) == 4 && (p6[16] & 0x0F) == 7 && (p3[16] >> 4) == 7,
                format!("diff bytes={diff:?}, p3[16]={:#04x} p6[16]={:#04x}", p3[16], p6[16]),
            );
            let diff_id: Vec<usize> = p3.iter().zip(&id).enumerate().filter(|(_, (a, b))| a != b).map(|(i, _)| i).collect();
            push(
                "phase 3 vs -1 (identity/null control) differ only in byte 16",
                diff_id == vec![16] && (id[16] & 0x0F) == 0,
                format!("diff bytes={diff_id:?}, id[16]={:#04x}", id[16]),
            );
        }
        (a, b, d) => push(
            "phase-3 payload is byte-exact against the audited frame",
            false,
            format!("encode failed: {a:?} / {b:?} / {d:?}"),
        ),
    }

    // 2. The tail-fix payload sets fields [8]/[9] and is otherwise the phase-3 payload.
    match (audit(3, None).encode(), audit(3, Some((0, AUDIT_NAME_HASH))).encode()) {
        (Ok(p3), Ok(tf)) => {
            push(
                "tail-fix payload is byte-exact against the audited frame",
                tf == hex(AUDIT_TF),
                format!("{} bytes, matches={}", tf.len(), tf == hex(AUDIT_TF)),
            );
            let diff: Vec<usize> = p3.iter().zip(&tf).enumerate().filter(|(_, (a, b))| a != b).map(|(i, _)| i).collect();
            push(
                "tail-fix touches only the value tail, not the phase or name",
                !diff.is_empty() && diff.iter().all(|&i| i >= 34) && p3[16] == tf[16],
                format!("diff bytes={diff:?}"),
            );
        }
        (a, b) => push("tail-fix payload is byte-exact against the audited frame", false, format!("{a:?} / {b:?}")),
    }

    // 3. Body-length rule: bits 98..130 must read 2 + value_bits (341), and 339 must FAIL.
    match audit(3, None).value_bits() {
        Ok((vbits, _)) => {
            let p3 = audit(3, None).encode().unwrap_or_default();
            let field = read_bits(&p3, 98, 32);
            push(
                "body_bit_length = 2 + value_bits (fresh+presence+value), not value_bits alone",
                field == u64::from(2 + vbits) && field != u64::from(vbits),
                format!("field={field}, value_bits={vbits}, 2+value_bits={}", 2 + vbits),
            );
        }
        Err(e) => push("body_bit_length = 2 + value_bits", false, format!("value encode failed: {e}")),
    }

    // 4. Name-ref extraction: bits 43..75 are the FNV hash, and it is what the codec emitted.
    match audit(3, None).name_ref_bits() {
        Ok((nbits, npay)) => {
            let p3 = audit(3, None).encode().unwrap_or_default();
            let hash = read_bits(&p3, 43, 32) as u32;
            let i8w = read_bits(&p3, 75, 7) as u32;
            let subw = read_bits(&p3, 82, 16) as u32;
            push(
                "name-ref hash at bits 43..75 equals the input and the codec's own 55-bit output",
                nbits == 55 && hash == AUDIT_NAME_HASH && read_bits(&npay, 0, 32) as u32 == AUDIT_NAME_HASH,
                format!("hash={hash:#010x} i8_wire={i8w:#x} sub_wire={subw:#x} nbits={nbits}"),
            );
            push(
                "name-ref biases: i8 wire = i8+1, i16 wire = sub+0x8000",
                i8w == u32::from(AUDIT_NAME_I8) + 1 && subw == (AUDIT_NAME_SUB as u32).wrapping_add(0x8000) & 0xFFFF,
                format!("i8_wire={i8w:#x} (want {:#x}), sub_wire={subw:#x} (want {:#x})", AUDIT_NAME_I8 as u32 + 1, (AUDIT_NAME_SUB as u32 + 0x8000) & 0xFFFF),
            );
        }
        Err(e) => push("name-ref hash at bits 43..75 equals the input", false, format!("name encode failed: {e}")),
    }

    // 5. gen is carried verbatim at bits 0..8 - it is a required input, never a constant.
    match audit(3, None).encode() {
        Ok(g0) => {
            let g5 = SensorPhaseUpdate { gen: 5, ..audit(3, None) }.encode().unwrap_or_default();
            push(
                "gen is carried at bits 0..8 and nowhere else",
                read_bits(&g0, 0, 8) == 0 && read_bits(&g5, 0, 8) == 5
                    && g0.iter().skip(1).eq(g5.iter().skip(1)),
                format!("gen0={} gen5={}", read_bits(&g0, 0, 8), read_bits(&g5, 0, 8)),
            );
        }
        Err(e) => push("gen is carried at bits 0..8 and nowhere else", false, format!("{e}")),
    }

    // 6. Total frame is 472 bits = 59 bytes, terminator (bit 471) = 0, fresh/presence = 1.
    match audit(3, None).encode() {
        Ok(p) => push(
            "frame is 59 bytes with terminator 0 and fresh=presence=1",
            p.len() == 59 && read_bits(&p, 471, 1) == 0 && read_bits(&p, 130, 1) == 1 && read_bits(&p, 131, 1) == 1,
            format!("{} bytes, term={} fresh={} presence={}", p.len(), read_bits(&p, 471, 1), read_bits(&p, 130, 1), read_bits(&p, 131, 1)),
        ),
        Err(e) => push("frame is 59 bytes with terminator 0", false, format!("{e}")),
    }

    // 7. The phase byte is a 4-bit bias-1 enum: raw = phase + 1, in the value's first nibble
    //    (byte 16 low nibble = bits 132..136).
    match (audit(-1, None).encode(), audit(3, None).encode(), audit(6, None).encode()) {
        (Ok(m1), Ok(p3), Ok(p6)) => push(
            "phase enum is 4-bit bias-1: raw(-1)=0, raw(3)=4, raw(6)=7",
            (m1[16] & 0x0F) == 0 && (p3[16] & 0x0F) == 4 && (p6[16] & 0x0F) == 7,
            format!("raw(-1)={} raw(3)={} raw(6)={}", m1[16] & 0x0F, p3[16] & 0x0F, p6[16] & 0x0F),
        ),
        _ => push("phase enum is 4-bit bias-1", false, "encode failed".into()),
    }

    // 8. An authority table with no client rows (a list of only `@authority` and `@phase`) still
    //    encodes: the frame pads to a byte instead of being refused (run sense49, 5,966 bits).
    let mut bare = audit(7, None);
    bare.authority = Some(SensorAuthority::all_to(13, 2));
    match bare.encode() {
        Ok(b) => push("authority-only phase-7 frame pads to whole bytes", true, format!("{} bytes", b.len())),
        Err(e) => push("authority-only phase-7 frame pads to whole bytes", false, format!("{e}")),
    }

    c
}

/// Read `n` bits from `bytes` starting at absolute bit `off`, MSB-first, as an integer.
fn read_bits(bytes: &[u8], off: usize, n: usize) -> u64 {
    let mut v = 0u64;
    for i in 0..n {
        let bit = (bytes[(off + i) >> 3] >> (7 - ((off + i) & 7))) & 1;
        v = (v << 1) | u64::from(bit);
    }
    v
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
}

#[cfg(test)]
mod tests {
    /// The controls are also a test, so `cargo test` fails on the same condition
    /// `d1-oracle sensor controls` exits 2 on. Two instruments, one answer.
    ///
    /// ⚠ Runs the client's own object codec under Unicorn, so it needs the pinned eboot and is not
    /// cheap - but it is the only thing that proves the assembled frame is byte-exact.
    #[test]
    fn every_sensor_control_passes() {
        if !crate::test_inputs::selected_eboot() {
            return;
        }
        let failed: Vec<&str> =
            super::run().into_iter().filter(|c| !c.passed).map(|c| c.name).collect();
        assert!(failed.is_empty(), "failed sensor controls: {failed:?}");
    }

    /// A frame with extra sensor clients must still build (byte-padded) and must carry the audited
    /// phase entry unchanged as its prefix: only the terminator bit moves.
    #[test]
    fn extra_clients_extend_the_audited_frame() {
        if !crate::test_inputs::selected_eboot() {
            return;
        }
        let one = super::audit(3, None).encode().expect("audited frame");
        let lines = "0xC2DACAF7 0x8 0 1 1
# comment
0x39774EE1 0x4 3 0 1
";
        let clients: Vec<_> = lines.lines().filter_map(super::SensorClientRef::parse_line).collect();
        assert_eq!(clients.len(), 2);
        assert!(clients[0].has_a && clients[0].has_b && !clients[1].has_a);
        let many = super::SensorPhaseUpdate { clients, ..super::audit(3, None) }.encode().expect("multi");
        assert!(many.len() > one.len());
        // Bits 0..471 (everything before the single frame's terminator) are identical.
        for bit in 0..471 {
            assert_eq!(super::read_bits(&one, bit, 1), super::read_bits(&many, bit, 1), "bit {bit}");
        }
        assert_eq!(super::read_bits(&many, 471, 1), 1, "the next has_entry is set");
    }

    /// A client line with an A value parses, encodes through the client's codec, and grows the
    /// entry by exactly the value's bits (presence then object) over the value-less form.
    #[test]
    fn a_valued_client_carries_its_object() {
        if !crate::test_inputs::selected_eboot() {
            return;
        }
        let mut img = vec![0u8; 0x78];
        img[0..4].copy_from_slice(&[0xFF; 4]);
        img[8] = 1;
        img[9] = 1;
        img[0x40..0x48].copy_from_slice(&0xD100_0000_0000_0201u64.to_le_bytes());
        let hex: String = img.iter().map(|b| format!("{b:02x}")).collect();
        let bare = super::SensorClientRef::parse_line("0xC2DACAF7 0xc 7 1 1").unwrap();
        let valued =
            super::SensorClientRef::parse_line(&format!("0xC2DACAF7 0xc 7 1 1 0x808022A5 {hex}"))
                .unwrap();
        assert_eq!(valued.a_value.as_ref().map(|v| (v.0, v.1.len())), Some((0x8080_22A5, 0x78)));
        let f = |c| super::SensorPhaseUpdate { clients: vec![c], ..super::audit(3, None) }.encode();
        let (a, b) = (f(bare).expect("bare"), f(valued).expect("valued"));
        assert!(b.len() > a.len(), "the valued entry must be longer ({} vs {})", b.len(), a.len());
    }
}
