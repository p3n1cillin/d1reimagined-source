// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! udt 5 **`sensor_auth_update`** frames, natively: the bespoke framing `d1_oracle::sensor` writes
//! and reads, with every embedded object (name reference, sensor value, authority table, each
//! entry's A and B values) encoded or decoded by this crate instead of the client's codec.
//!
//! The framing is not a schema; it is `0x2A1210`'s reader, as `d1_oracle::sensor` documents it:
//!
//! 1. `gen` u8; `has_sensor_block` bit; when set, 65 changed bits (block 0 first) and the
//!    authority object `0x80800CF7` through the **replicated** coder (presence bit, then the body);
//! 2. `stamp` u32; `init` bit;
//! 3. per entry: `has_entry = 1`, the name reference `0x808004B1`, `body_bit_length` u32, the body;
//!    `has_entry = 0` ends the list;
//! 4. the body: when the client has an A type, `fresh` and `presence`, then the A object when
//!    present; when it has a B type, `has_b`, and when set: a flag bit, `fresh`, `presence`, the B
//!    object and a trailing u32.
//!
//! This module only re-states that framing; it does not choose its values. Which entries a frame
//! carries, their names, team values and the authority table remain the caller's (the host's
//! sensor lists). `tests/equivalence.rs` requires the frames to be byte-identical to
//! `d1_oracle::sensor::SensorPhaseUpdate::encode` and the decodes to equal `FrameDecoder`'s.

use crate::bits::{read_bits, BitReader, BitWriter};
use crate::decode::Decoder;
use crate::encode::Encoder;
use crate::error::{Result, RsatError};
use crate::schema::Schemas;

/// The `{u32 hash, i8, i16}` globals name reference.
pub const TAG_NAME_REF: u32 = 0x8080_04B1;
/// The lifetime record's value (the 4-bit phase struct).
pub const TAG_SENSOR_VALUE: u32 = 0x8080_08A3;
/// The authority table: a `u32`, then 65 blocks of 12 bytes.
pub const TAG_AUTHORITY: u32 = 0x8080_0CF7;
/// Blocks in the authority table, and bits in its changed mask.
pub const AUTHORITY_BLOCKS: usize = 65;
/// Where the stamp sits in a frame without the authority table: after `gen` and the flag.
pub const STAMP_BIT_NO_AUTHORITY: u64 = 9;

/// A name reference: the 8-byte `0x808004B1` image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameRef {
    pub hash: u32,
    pub i8: u8,
    pub sub: i16,
}

impl NameRef {
    pub fn image(&self) -> [u8; 8] {
        let mut b = [0u8; 8];
        b[0..4].copy_from_slice(&self.hash.to_le_bytes());
        b[4] = self.i8;
        b[6..8].copy_from_slice(&self.sub.to_le_bytes());
        b
    }

    pub fn from_image(o: &[u8]) -> Self {
        Self {
            hash: u32::from_le_bytes([o[0], o[1], o[2], o[3]]),
            i8: o[4],
            sub: i16::from_le_bytes([o[6], o[7]]),
        }
    }
}

/// One entry to send: the client it names, which parts that client has, and the values to install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: NameRef,
    /// The client has an A type: the body carries `fresh` and `presence`.
    pub has_a: bool,
    /// `Some((tag, image))`: `presence = 1` and the A object. `None`: `presence = 0`, the default.
    pub a_value: Option<(u32, Vec<u8>)>,
    /// The client has a B type: the body carries `has_b`.
    pub has_b: bool,
    /// `Some((tag, image))`: `has_b = 1`, flag 0, `fresh = 1`, `presence = 1`, the B object, u32 0.
    pub b_value: Option<(u32, Vec<u8>)>,
}

/// One whole frame to assemble.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub gen: u8,
    /// `(changed mask, 784-byte image)` when the frame carries the authority table.
    pub authority: Option<(u128, Vec<u8>)>,
    pub stamp: u32,
    pub init: bool,
    pub entries: Vec<Entry>,
    /// Zero-pad to a byte after the terminator. Without it the frame must already be a byte
    /// multiple (the audited single-entry frame is exactly 472 bits).
    pub pad: bool,
}

/// An assembled frame and where its stamp is, so a host can restamp it without decoding it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    pub bytes: Vec<u8>,
    pub stamp_bit: u64,
    /// Bits up to and including the terminator.
    pub bits: u64,
}

impl Frame {
    /// Assemble the frame, every object through the native encoder.
    pub fn encode(&self, s: &Schemas) -> Result<Built> {
        let enc = Encoder::new(s);
        let mut w = BitWriter::new();
        w.write(u64::from(self.gen), 8);
        match &self.authority {
            None => w.write_bit(false),
            Some((changed, image)) => {
                w.write_bit(true);
                for i in 0..AUTHORITY_BLOCKS {
                    w.write_bit((changed >> i) & 1 == 1);
                }
                let e = enc.encode_replicated(TAG_AUTHORITY, image)?;
                w.append(&e.payload, e.bits);
            }
        }
        let stamp_bit = w.bit_len();
        w.write(u64::from(self.stamp), 32);
        w.write_bit(self.init);
        for entry in &self.entries {
            let name = enc.encode(TAG_NAME_REF, &entry.name.image())?;
            let a = match (&entry.a_value, entry.has_a) {
                (Some((tag, image)), true) => Some(enc.encode(*tag, image)?),
                _ => None,
            };
            let b = match (&entry.b_value, entry.has_b) {
                (Some((tag, image)), true) => Some(enc.encode(*tag, image)?),
                _ => None,
            };
            let a_bits = a.as_ref().map_or(0, |v| v.bits);
            let b_bits = u64::from(entry.has_b) + b.as_ref().map_or(0, |v| 3 + v.bits + 32);
            let body = 2 * u64::from(entry.has_a) + a_bits + b_bits;
            let body = u32::try_from(body)
                .map_err(|_| RsatError::Frame(format!("an entry body of {body} bits")))?;
            w.write_bit(true); // has_entry
            w.append(&name.payload, name.bits);
            w.write(u64::from(body), 32);
            if entry.has_a {
                w.write_bit(true); // fresh
                match &a {
                    Some(v) => {
                        w.write_bit(true); // presence
                        w.append(&v.payload, v.bits);
                    }
                    None => w.write_bit(false),
                }
            }
            if entry.has_b {
                match &b {
                    Some(v) => {
                        w.write_bit(true); // has_b
                        w.write_bit(false); // flag -> client +0x16
                        w.write_bit(true); // fresh
                        w.write_bit(true); // presence
                        w.append(&v.payload, v.bits);
                        w.write(0, 32); // -> client +0x10
                    }
                    None => w.write_bit(false),
                }
            }
        }
        w.write_bit(false); // terminator
        let bits = w.bit_len();
        if !self.pad && bits % 8 != 0 {
            return Err(RsatError::Frame(format!(
                "sensor frame is {bits} bits, not a byte multiple, and padding was not asked for"
            )));
        }
        let (_, bytes) = w.finish();
        Ok(Built { bytes, stamp_bit, bits })
    }
}

/// One decoded entry: its name, and its body as sent (MSB-first, zero-padded to a byte).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedEntry {
    pub name: NameRef,
    pub body_bits: u32,
    pub body: Vec<u8>,
}

/// A decoded frame, as `d1_oracle::sensor::FrameDecoder::decode` returns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedFrame {
    pub gen: u8,
    pub authority: Option<(u128, Vec<u8>)>,
    pub stamp_bit: u64,
    pub stamp: u32,
    pub init: bool,
    pub entries: Vec<DecodedEntry>,
    /// Bits read up to and including the terminator.
    pub bits: u64,
}

fn frame_err(what: String) -> RsatError {
    RsatError::Frame(what)
}

/// Bits `[off, off + n)` as a new zero-padded buffer (at least one byte, as the oracle's).
fn slice_bits(bytes: &[u8], off: u64, n: u64) -> Vec<u8> {
    let mut out = vec![0u8; (n.div_ceil(8) as usize).max(1)];
    for i in 0..n {
        if read_bits(bytes, off + i, 1) == 1 {
            out[(i >> 3) as usize] |= 0x80 >> (i & 7);
        }
    }
    out
}

/// ⭐ Decode a frame's framing natively: the name references and the authority table through the
/// native decoder, entry bodies kept as bits (each value's type is the receiving client's, not the
/// frame's; see [`decode_body`]).
pub fn decode_frame(s: &Schemas, payload: &[u8]) -> Result<DecodedFrame> {
    let dec = Decoder::new(s);
    let total = payload.len() as u64 * 8;
    let need = |pos: u64, n: u64| {
        if pos + n > total {
            Err(frame_err(format!("sensor frame ends at bit {total}, needs {}", pos + n)))
        } else {
            Ok(())
        }
    };
    need(0, 9)?;
    let gen = read_bits(payload, 0, 8) as u8;
    let mut pos = 9;
    let authority = if read_bits(payload, 8, 1) == 1 {
        need(pos, AUTHORITY_BLOCKS as u64)?;
        let mut changed = 0u128;
        for i in 0..AUTHORITY_BLOCKS as u64 {
            changed |= u128::from(read_bits(payload, pos + i, 1) as u8) << i;
        }
        pos += AUTHORITY_BLOCKS as u64;
        let d = dec.decode_replicated(TAG_AUTHORITY, &slice_bits(payload, pos, total - pos), &[])?;
        pos += d.bits;
        Some((changed, d.object))
    } else {
        None
    };
    need(pos, 34)?;
    let stamp_bit = pos;
    let stamp = read_bits(payload, pos, 32) as u32;
    pos += 32;
    let init = read_bits(payload, pos, 1) == 1;
    pos += 1;
    let mut entries = Vec::new();
    loop {
        need(pos, 1)?;
        let has_entry = read_bits(payload, pos, 1) == 1;
        pos += 1;
        if !has_entry {
            break;
        }
        let d = dec.decode(TAG_NAME_REF, &slice_bits(payload, pos, total - pos))?;
        pos += d.bits;
        need(pos, 32)?;
        let body_bits = read_bits(payload, pos, 32) as u32;
        pos += 32;
        need(pos, u64::from(body_bits))?;
        entries.push(DecodedEntry {
            name: NameRef::from_image(&d.object),
            body_bits,
            body: slice_bits(payload, pos, u64::from(body_bits)),
        });
        pos += u64::from(body_bits);
    }
    Ok(DecodedFrame { gen, authority, stamp_bit, stamp, init, entries, bits: pos })
}

/// The stamp's bit offset in a frame: 9, or after the authority table, whose width only a decode
/// of it gives. Native, so a connection loop may call it.
pub fn stamp_bit(s: &Schemas, payload: &[u8]) -> Result<u64> {
    if payload.len() < 2 {
        return Err(frame_err("sensor frame shorter than its header".into()));
    }
    if read_bits(payload, 8, 1) == 0 {
        return Ok(STAMP_BIT_NO_AUTHORITY);
    }
    let pos = 9 + AUTHORITY_BLOCKS as u64;
    let total = payload.len() as u64 * 8;
    if pos > total {
        return Err(frame_err("sensor frame ends inside its authority mask".into()));
    }
    let d = Decoder::new(s).decode_replicated(TAG_AUTHORITY, &slice_bits(payload, pos, total - pos), &[])?;
    Ok(pos + d.bits)
}

/// One entry body, decoded for a client whose A and B types are known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Body {
    /// `(fresh, A image when presence was set)`, when the client has an A type.
    pub a: Option<(bool, Option<Vec<u8>>)>,
    /// When the client has a B type and `has_b` was set: `(flag, fresh, B image when present,
    /// trailing u32)`.
    pub b: Option<(bool, bool, Option<Vec<u8>>, u32)>,
    /// Bits consumed; must equal the entry's `body_bit_length`.
    pub bits: u64,
}

/// ⭐ Decode one entry body for a client of known A/B types. `a_seed`/`b_seed` are the replica's
/// current images (empty: zero). A `fresh = 0` update overlays the replica, so decoding a delta
/// needs the receiving client's prior state: the seed is a parameter, never assumed zero.
pub fn decode_body(
    s: &Schemas,
    body: &[u8],
    body_bits: u32,
    a: Option<(u32, &[u8])>,
    b: Option<(u32, &[u8])>,
) -> Result<Body> {
    let dec = Decoder::new(s);
    let mut r = BitReader::new(body);
    let bit = |r: &mut BitReader<'_>| r.read_bit().ok_or_else(|| frame_err("entry body ends early".into()));
    let object = |r: &mut BitReader<'_>, tag: u32, seed: &[u8]| -> Result<Vec<u8>> {
        let size = s.get(tag)?.struct_size as usize;
        let mut img = if seed.is_empty() { vec![0u8; size] } else { seed.to_vec() };
        dec.decode_from(r, tag, &mut img)?;
        Ok(img)
    };
    let a_out = match a {
        Some((tag, seed)) => {
            let fresh = bit(&mut r)?;
            let present = bit(&mut r)?;
            let img = if present { Some(object(&mut r, tag, seed)?) } else { None };
            Some((fresh, img))
        }
        None => None,
    };
    let b_out = match b {
        Some((tag, seed)) => {
            if bit(&mut r)? {
                let flag = bit(&mut r)?;
                let fresh = bit(&mut r)?;
                let present = bit(&mut r)?;
                let img = if present { Some(object(&mut r, tag, seed)?) } else { None };
                let tail = r.read(32).ok_or_else(|| frame_err("entry body ends early".into()))? as u32;
                Some((flag, fresh, img, tail))
            } else {
                None
            }
        }
        None => None,
    };
    if r.bit_pos() != u64::from(body_bits) {
        return Err(frame_err(format!(
            "entry body decoded {} bits, its body_bit_length says {body_bits}",
            r.bit_pos()
        )));
    }
    Ok(Body { a: a_out, b: b_out, bits: r.bit_pos() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_ref_image_round_trips() {
        let n = NameRef { hash: 0xC2DA_CAF7, i8: 0x10, sub: -2 };
        assert_eq!(NameRef::from_image(&n.image()), n);
    }

    /// The idle heartbeat needs no object at all: gen, no table, stamp, init 0, terminator.
    #[test]
    fn an_empty_frame_is_the_43_bit_heartbeat() {
        if !crate::test_inputs::selected_eboot() {
            return;
        }
        let Ok(s) = crate::schemas() else { return };
        let f = Frame { gen: 3, authority: None, stamp: 45, init: false, entries: vec![], pad: true };
        let b = f.encode(s).unwrap();
        assert_eq!((b.bits, b.stamp_bit, b.bytes.len()), (43, 9, 6));
        let d = decode_frame(s, &b.bytes).unwrap();
        assert_eq!((d.gen, d.stamp, d.init, d.entries.len(), d.bits), (3, 45, false, 0, 43));
        assert!(Frame { pad: false, ..f }.encode(s).is_err(), "43 bits unpadded must refuse");
    }
}
