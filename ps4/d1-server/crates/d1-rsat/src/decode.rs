// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The native object **decoder**: build 56578's `0x2C9A90` (object decode) and `0x2C9AE0`
//! (replicated decode), reimplemented.
//!
//! # What the client does, read from the 56578 image
//!
//! `0x2C9A90` is `call 0x2C9110; mov al,1; ret`. `0x2C9110(tag, bitstream, object, object, 1)`
//! resolves the descriptor, sizes and clears a change mask (`[d+0x14] + 1` bits) that it **fills**
//! as it reads, and calls the decode walker `0x101EB30` with the same five-slot frame the encoder
//! uses. The walker is the encoder's `0x101C220` read the other way:
//!
//! * an optional field reads its presence bit (`0x73430`) and records it in the mask
//!   (`0x10265D0`); clear, and the field is left as it was;
//! * a composite descends with the same count rule: the child's capacity or field count, replaced
//!   by the `i32` at `parent + count_offset` when the count source is 1. In a decode that `i32` is
//!   the count field **just decoded** into the same image;
//! * a leaf calls `table[kind]`, the decode table at `0x19131B0` (`0x1913060 + 0x150`).
//!
//! The leaf decoders (all read from the image):
//!
//! | kind | handler | effect |
//! |---|---|---|
//! | 2 | `0x102BC00` | one bit, stored as a byte |
//! | 3, 7 | `0x102BC20` | `width` bits (`0x72C70`) minus the bias, low byte stored |
//! | 4, 8 | `0x102BC50` | the same, low two bytes |
//! | 5, 9 | `0x102BC80` | the same, four bytes |
//! | 6, 10 | `0x102BCB0` | `width` bits (`0x73170`, 64-bit) minus the sign-extended bias, eight bytes |
//! | 11 | `0x102BCE0` | unquantized (`params[2]` not in 1..=31): 32 raw bits |
//! | 0x15-0x17 | `0x102C1D0` | a bit; set: 32 bits, else `-1` |
//! | 0x29 | `0x102D490` | the handle through the type cell's schema into `+0`; unless `-1`, an object of that type into `+4` |
//!
//! `0x2C9AE0` reads one presence bit; set, it walks the same way (`0x2C8340`, mask base 1) over the
//! caller's image, so absent optional fields keep the seed. The decoder never clears the image it
//! is handed: an ordinary decode starts from zero (the oracle's zeroed allocation, the client's
//! own fresh object), a replicated one from the caller's seed.

use crate::bits::BitReader;
use crate::error::{Result, RsatError};
use crate::schema::{Descriptor, Field, Schemas, KIND_COMPOSITE, KIND_FLOAT, KIND_TYPED_REF};

/// One decoded object: what `ObjectOracle::decode` returns, natively.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    pub tag: u32,
    /// Bits consumed (`bs+0x24`).
    pub bits: u64,
    /// The whole struct image, `struct_size` bytes.
    pub object: Vec<u8>,
}

/// Nesting limit; see [`crate::encode`].
const MAX_DEPTH: usize = 32;

pub struct Decoder<'a> {
    schemas: &'a Schemas,
}

impl<'a> Decoder<'a> {
    pub fn new(schemas: &'a Schemas) -> Self {
        Self { schemas }
    }

    /// Decode one object body into a zero image, as `0x2C9A90` does into a fresh object.
    pub fn decode(&self, tag: u32, wire: &[u8]) -> Result<Decoded> {
        let d = self.schemas.get(tag)?;
        let mut obj = vec![0u8; d.struct_size as usize];
        let mut r = BitReader::new(wire);
        self.decode_from(&mut r, tag, &mut obj)?;
        Ok(Decoded { tag, bits: r.bit_pos(), object: obj })
    }

    /// Decode one object body that starts at bit `at` of `wire`, into a zero image. `bits` in the
    /// result is the reader's END position (absolute), so the caller can read what follows: an
    /// investment message is `u16 id || args || has_payload || ...` and its args start at bit 16.
    pub fn decode_at(&self, tag: u32, wire: &[u8], at: u64) -> Result<Decoded> {
        self.decode_at_over(tag, wire, at, 0)
    }

    /// [`Self::decode_at`] into an image whose every byte starts as `fill`. Bytes a walk leaves
    /// at `fill` were not written: decoding with two fills (0x00, 0xFF) and comparing tells which
    /// fields a sparse body (a QueueZ delta object) carries, whatever their values.
    pub fn decode_at_over(&self, tag: u32, wire: &[u8], at: u64, fill: u8) -> Result<Decoded> {
        let d = self.schemas.get(tag)?;
        let mut obj = vec![fill; d.struct_size as usize];
        let mut r = BitReader::at(wire, at);
        self.decode_from(&mut r, tag, &mut obj)?;
        Ok(Decoded { tag, bits: r.bit_pos(), object: obj })
    }

    /// [`Self::decode_at_over`] that keeps what the walk wrote when it fails: the image as far as
    /// the walk got, the reader's position, and the failure. For bodies of another build whose
    /// layout differs late in the struct (a 51539 capture's profile): the fields walked before the
    /// difference are read; nothing after it is. A caller must say which fields it relies on.
    pub fn decode_at_partial(
        &self,
        tag: u32,
        wire: &[u8],
        at: u64,
        fill: u8,
    ) -> Result<(Decoded, Option<RsatError>)> {
        let d = self.schemas.get(tag)?;
        let mut obj = vec![fill; d.struct_size as usize];
        let mut r = BitReader::at(wire, at);
        let err = self.decode_from(&mut r, tag, &mut obj).err();
        Ok((Decoded { tag, bits: r.bit_pos(), object: obj }, err))
    }

    /// ⭐ A **replicated** body (udts 11 and 21, the sensor authority table), as `0x2C9AE0`: one
    /// presence bit, then, when it is set, the ordinary walk over `seed`. An empty seed is the zero
    /// image ("the client has nothing yet"); any other seed must be exactly one struct.
    pub fn decode_replicated(&self, tag: u32, wire: &[u8], seed: &[u8]) -> Result<Decoded> {
        let d = self.schemas.get(tag)?;
        let size = d.struct_size as usize;
        let mut obj = if seed.is_empty() {
            vec![0u8; size]
        } else if seed.len() == size {
            seed.to_vec()
        } else {
            return Err(RsatError::BadImage { tag, len: seed.len(), struct_size: size });
        };
        let mut r = BitReader::new(wire);
        let present = r.read_bit().ok_or_else(|| RsatError::Wire {
            tag,
            what: "an empty body has no presence bit".into(),
        })?;
        if present {
            self.decode_from(&mut r, tag, &mut obj)?;
        }
        Ok(Decoded { tag, bits: r.bit_pos(), object: obj })
    }

    /// Decode one object body from `r` over `obj` (exactly the struct; a nested `0x2C9A90`).
    pub fn decode_from(&self, r: &mut BitReader<'_>, tag: u32, obj: &mut [u8]) -> Result<()> {
        let d = self.schemas.get(tag)?;
        if obj.len() != d.struct_size as usize {
            return Err(RsatError::BadImage { tag, len: obj.len(), struct_size: d.struct_size as usize });
        }
        self.schemas.check_decode(tag)?;
        let count = if d.is_array() { i64::from(d.capacity) } else { i64::from(d.field_count) };
        let mut walk = Walk { s: self, obj, r, top: tag };
        walk.descriptor(d, 0, count, 0)
    }
}

struct Walk<'e, 'a, 'o, 'r, 'b> {
    s: &'e Decoder<'a>,
    obj: &'o mut [u8],
    r: &'r mut BitReader<'b>,
    top: u32,
}

impl Walk<'_, '_, '_, '_, '_> {
    fn runtime(&self, what: String) -> RsatError {
        RsatError::Runtime { tag: self.top, what }
    }

    fn read(&mut self, width: u32) -> Result<u64> {
        let at = self.r.bit_pos();
        let len = self.r.bit_len();
        self.r.read(width).ok_or(RsatError::Wire {
            tag: self.top,
            what: format!("a {width}-bit read at bit {at} runs past the {len}-bit body"),
        })
    }

    fn range(&self, at: i64, n: usize) -> Result<std::ops::Range<usize>> {
        usize::try_from(at)
            .ok()
            .and_then(|a| Some(a..a.checked_add(n)?))
            .filter(|r| r.end <= self.obj.len())
            .ok_or_else(|| {
                self.runtime(format!(
                    "a {n}-byte access at +{at:#x} runs outside the {:#x}-byte object",
                    self.obj.len()
                ))
            })
    }

    fn put(&mut self, at: i64, bytes: &[u8]) -> Result<()> {
        let r = self.range(at, bytes.len())?;
        self.obj[r].copy_from_slice(bytes);
        Ok(())
    }

    fn i32_at(&self, at: i64) -> Result<i32> {
        let r = self.range(at, 4)?;
        Ok(i32::from_le_bytes(self.obj[r].try_into().unwrap()))
    }

    /// One presence bit, if the field has one. Returns whether the field is present.
    fn presence(&mut self, f: &Field) -> Result<bool> {
        if f.optional == 0 {
            return Ok(true);
        }
        Ok(self.read(1)? == 1)
    }

    fn descriptor(&mut self, d: &Descriptor, obj: i64, count: i64, depth: usize) -> Result<()> {
        if depth > MAX_DEPTH {
            return Err(self.runtime(format!("schema nesting past {MAX_DEPTH} levels at {:#010x}", d.tag)));
        }
        if d.is_array() {
            let e = d.element.as_ref().expect("array has an element");
            if count > i64::from(d.capacity) {
                return Err(self.runtime(format!(
                    "array {:#010x} count {count} exceeds its capacity {}",
                    d.tag, d.capacity
                )));
            }
            for i in 0..count.max(0) {
                let off = obj + i64::from(e.offset.wrapping_mul(i as i32));
                if !self.presence(e)? {
                    continue;
                }
                if e.kind == KIND_COMPOSITE {
                    // The count source is read relative to the ARRAY's object, as in the encoder.
                    self.composite(e, obj, off, depth)?;
                } else {
                    self.leaf(e, off)?;
                }
            }
            return Ok(());
        }
        if count > d.fields.len() as i64 {
            return Err(self.runtime(format!(
                "{:#010x} is walked for {count} fields and has {}",
                d.tag,
                d.fields.len()
            )));
        }
        for f in &d.fields[..count.max(0) as usize] {
            if !self.presence(f)? {
                continue;
            }
            let at = obj + i64::from(f.offset);
            if f.kind == KIND_COMPOSITE {
                self.composite(f, obj, at, depth)?;
            } else {
                self.leaf(f, at)?;
            }
        }
        Ok(())
    }

    fn composite(&mut self, f: &Field, parent: i64, at: i64, depth: usize) -> Result<()> {
        let child = self.s.schemas.get(f.child)?;
        let count = if f.count_source() == 1 {
            i64::from(self.i32_at(parent + i64::from(f.count_offset()))?)
        } else if child.is_array() {
            i64::from(child.capacity)
        } else {
            i64::from(child.field_count)
        };
        self.descriptor(child, at, count, depth + 1)
    }

    fn leaf(&mut self, f: &Field, at: i64) -> Result<()> {
        let bias = f.bias();
        let width = f.width();
        match f.kind {
            2 => {
                let b = self.read(1)? as u8;
                self.put(at, &[b])?;
            }
            3 | 7 => {
                let v = (self.read(width)? as u32).wrapping_sub(bias);
                self.put(at, &[v as u8])?;
            }
            4 | 8 => {
                let v = (self.read(width)? as u32).wrapping_sub(bias);
                self.put(at, &(v as u16).to_le_bytes())?;
            }
            5 | 9 => {
                let v = (self.read(width)? as u32).wrapping_sub(bias);
                self.put(at, &v.to_le_bytes())?;
            }
            6 | 10 => {
                let v = self.read(width)?.wrapping_sub(i64::from(bias as i32) as u64);
                self.put(at, &v.to_le_bytes())?;
            }
            KIND_FLOAT => {
                let q = f.params[2];
                let v = if (1..=31).contains(&q) {
                    let code = self.read(q)? as u32;
                    crate::quant::decode(
                        code,
                        f32::from_bits(f.params[0]),
                        f32::from_bits(f.params[1]),
                        q,
                        self.s.schemas.quant_half,
                    )
                    .to_bits()
                } else {
                    self.read(32)? as u32
                };
                self.put(at, &v.to_le_bytes())?;
            }
            0x0D => {
                for lane in 0..3 {
                    let v = self.read(32)? as u32;
                    self.put(at + lane as i64 * 4, &v.to_le_bytes())?;
                }
                self.put(at + 12, &self.s.schemas.position_w.to_le_bytes())?;
            }
            0x15..=0x17 => {
                let v = if self.read(1)? == 1 { self.read(32)? as u32 } else { u32::MAX };
                self.put(at, &v.to_le_bytes())?;
            }
            KIND_TYPED_REF => self.typed_ref(at, 4)?,
            0x22 => self.typed_ref(at, 16)?,
            0x23 => self.object_at(self.s.schemas.identity_tag(), at)?,
            0x26 => self.atom(at)?,
            k => {
                return Err(RsatError::Unsupported {
                    tag: self.top,
                    field: f.index,
                    what: format!("field kind {k:#04x}"),
                })
            }
        }
        Ok(())
    }

    /// Kind `0x29`, `0x102D490`: the handle object (the type cell's schema) into `+0`, then, unless
    /// the handle read back is `-1`, an object of type `handle` into `+4`. Both are complete
    /// `0x2C9A90` calls.
    fn typed_ref(&mut self, at: i64, payload_offset: i64) -> Result<()> {
        let (_, handle_tag) = self.s.schemas.cells.typed_ref_handle;
        let hd = self.s.schemas.get(handle_tag)?;
        let hsize = hd.struct_size as usize;
        if hsize > 4 {
            return Err(self.runtime(format!(
                "typed-reference handle schema {handle_tag:#010x} is {hsize} bytes, wider than the \
                 4-byte handle"
            )));
        }
        let hr = self.range(at, hsize)?;
        self.s.decode_from(self.r, handle_tag, &mut self.obj[hr])?;
        let handle = u32::from_le_bytes(self.obj[self.range(at, 4)?].try_into().unwrap());
        if handle == u32::MAX {
            return Ok(());
        }
        let td = self.s.schemas.get(handle).map_err(|_| {
            self.runtime(format!(
                "typed reference names {handle:#010x}, which has no descriptor"
            ))
        })?;
        let vr = self.range(at + payload_offset, td.struct_size as usize)?;
        self.s.decode_from(self.r, handle, &mut self.obj[vr])
    }

    fn object_at(&mut self, tag: u32, at: i64) -> Result<()> {
        let size = self.s.schemas.get(tag)?.struct_size as usize;
        let range = self.range(at, size)?;
        self.s.decode_from(self.r, tag, &mut self.obj[range])
    }

    /// 0x102D330 / 0x1C0950: decode selectors into zero locals, initialize the union,
    /// then decode each payload. The initializer preserves the selector padding bytes.
    fn atom(&mut self, at: i64) -> Result<()> {
        let mut selectors = [0u8; 2];
        for (i, cell) in self.s.schemas.cells.atom_selectors.iter().enumerate() {
            let mut image = vec![0; self.s.schemas.get(cell.1)?.struct_size as usize];
            self.s.decode_from(self.r, cell.1, &mut image)?;
            selectors[i] = image[0];
        }
        let pt = self.s.schemas.atom_tag(selectors[0], false, self.top)?;
        let st = self.s.schemas.atom_tag(selectors[1], true, self.top)?;
        self.put(at, &selectors[..1])?;
        self.put(at + 4, &selectors[1..])?;
        self.put(at + 8, &[0; 4])?;
        self.put(at + 16, &[0; 48])?;
        self.object_at(pt, at + 16)?;
        self.object_at(st, at + 8)
    }
}
