// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! ⭐ The native object encoder: build 56578's `0x2C9A80` (object encode), reimplemented.
//!
//! # What the client does, read from the 56578 image
//!
//! `0x2C9A80` is `call 0x2C8EB0; mov al,1; ret`. `0x2C8EB0(tag, object, bitstream, 1)`:
//!
//! 1. resolves the tag to its descriptor `d` through the registry;
//! 2. sizes a **change mask** of `[d+0x14] + 1` bits (anchor `+0x1C`, plus one) and sets every bit
//!    (`0x2C7690`: `0xFF` fill, then the last word masked to exactly that many bits);
//! 3. calls the walker `0x101C220` with the frame `{mask, 0, 1, object, bitstream}`: the walk's
//!    mask-bit base starts at **1**.
//!
//! The walker (an explicit-stack loop, here a recursion) visits a descriptor's fields in order, or,
//! for an array type (`capacity > 0`), its elements:
//!
//! * a field whose presence byte (`marker+9`) is set writes one bit first: whether mask bit
//!   `base + mask_bit` is set. Clear, and the field is skipped. With the full mask that is
//!   "present unless the bit index runs past the mask", which happens inside large arrays.
//! * a composite (`kind 1`) descends: the child's count is its capacity (array) or field count,
//!   replaced by the `i32` at `object + count_offset` when the field's count source is 1. The child
//!   object is `object + offset`; its mask-bit base is `base + mask_bit + presence width`. For an
//!   array element, the offsets are `stride * index` and the count is read relative to the
//!   **array's** object, not the element's.
//! * a leaf calls `table[kind]` (`0x1912C70`) with `(object + offset, mask, bit, params, bs)`.
//!
//! Every count and offset is the walker's `i32`/`i64` arithmetic. A count of zero or less encodes
//! nothing. The native walk refuses ([`RsatError::Runtime`]) where the client would read outside
//! the object it was handed, rather than invent the bytes it would have read.

use crate::bits::BitWriter;
use crate::error::{Result, RsatError};
use crate::schema::{Descriptor, Field, Schemas, KIND_COMPOSITE, KIND_FLOAT, KIND_TYPED_REF};

/// One encoded object body: what `ObjectOracle::encode` returns, natively.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encoded {
    pub tag: u32,
    pub bits: u64,
    pub payload: Vec<u8>,
}

/// Nesting limit. The walker's own stack holds 32 frames (`rsp+0x80`, 0x30 each, in a 0x6A0
/// frame); a cycle in a well-formed image is impossible, so this only bounds a corrupt one.
const MAX_DEPTH: usize = 32;

pub struct Encoder<'a> {
    schemas: &'a Schemas,
}

impl<'a> Encoder<'a> {
    pub fn new(schemas: &'a Schemas) -> Self {
        Self { schemas }
    }

    /// Encode one object body, as `0x2C9A80` does. `image` seeds the struct and the rest is zero,
    /// as the oracle's `make_object`; an image longer than the struct is refused.
    pub fn encode(&self, tag: u32, image: &[u8]) -> Result<Encoded> {
        let mut w = BitWriter::new();
        self.encode_into(&mut w, tag, image)?;
        let (bits, payload) = w.finish();
        Ok(Encoded { tag, bits, payload })
    }

    /// ⭐ A **replicated** full-state body: one presence bit (`1`), then the ordinary body. This is
    /// `0x2C9D20` with an all-ones mask, which is what the oracle's `encode_replicated` composes
    /// (`write_one_bit(1)` + the object encoder) and what the client's `0x2C9AE0` decodes.
    pub fn encode_replicated(&self, tag: u32, image: &[u8]) -> Result<Encoded> {
        let mut w = BitWriter::new();
        w.write_bit(true);
        self.encode_into(&mut w, tag, image)?;
        let (bits, payload) = w.finish();
        Ok(Encoded { tag, bits, payload })
    }

    /// Encode a replicated delta with the caller's change mask, as 56578 0x2C9D20.
    /// Words use the client's low-bit-first mask indexing; bit 0 is whole-object presence,
    /// and the walk starts at bit 1. Nonoptional fields are always written when present.
    /// This does not infer dirty fields from an image or from a previous replica.
    pub fn encode_replicated_masked(
        &self,
        tag: u32,
        image: &[u8],
        mask: &[u32],
    ) -> Result<Encoded> {
        let d = self.schemas.get(tag)?;
        let mask_n = d
            .mask_bits
            .checked_add(1)
            .ok_or_else(|| RsatError::Runtime {
                tag,
                what: "change-mask size overflows".into(),
            })?;
        let words = mask_n.div_ceil(32) as usize;
        if mask.len() != words || (mask_n % 32 != 0 && mask[words - 1] >> (mask_n % 32) != 0) {
            return Err(RsatError::Runtime {
                tag,
                what: format!(
                    "expected {mask_n} change-mask bits ({words} words), with zero padding"
                ),
            });
        }
        if image.len() > d.struct_size as usize {
            return Err(RsatError::BadImage {
                tag,
                len: image.len(),
                struct_size: d.struct_size as usize,
            });
        }
        self.schemas.check(tag)?;
        let mut w = BitWriter::new();
        let present = mask[0] & 1 != 0;
        w.write_bit(present);
        if present {
            self.encode_masked_into(&mut w, tag, image, Some(mask))?;
        }
        let (bits, payload) = w.finish();
        Ok(Encoded { tag, bits, payload })
    }

    /// Append one object body to an existing stream (a nested `0x2C9A80` call).
    pub fn encode_into(&self, w: &mut BitWriter, tag: u32, image: &[u8]) -> Result<()> {
        self.encode_masked_into(w, tag, image, None)
    }

    fn encode_masked_into(
        &self,
        w: &mut BitWriter,
        tag: u32,
        image: &[u8],
        mask: Option<&[u32]>,
    ) -> Result<()> {
        let d = self.schemas.get(tag)?;
        let size = d.struct_size as usize;
        if image.len() > size {
            return Err(RsatError::BadImage { tag, len: image.len(), struct_size: size });
        }
        self.schemas.check(tag)?;
        let mut obj = image.to_vec();
        obj.resize(size, 0);
        let mask_n = d.mask_bits.wrapping_add(1);
        let count = top_count(d);
        let mut walk = Walk {
            s: self,
            obj: &obj,
            mask_n,
            mask,
            w,
            top: tag,
        };
        walk.descriptor(d, 0, count, 1, 0)
    }
}

fn top_count(d: &Descriptor) -> i64 {
    if d.is_array() {
        i64::from(d.capacity)
    } else {
        i64::from(d.field_count)
    }
}

struct Walk<'e, 'a, 'w, 'm> {
    s: &'e Encoder<'a>,
    obj: &'e [u8],
    mask_n: u32,
    mask: Option<&'m [u32]>,
    w: &'w mut BitWriter,
    top: u32,
}

impl Walk<'_, '_, '_, '_> {
    fn runtime(&self, what: String) -> RsatError {
        RsatError::Runtime { tag: self.top, what }
    }

    fn bytes(&self, at: i64, n: usize) -> Result<&[u8]> {
        usize::try_from(at)
            .ok()
            .and_then(|a| self.obj.get(a..a.checked_add(n)?))
            .ok_or_else(|| {
                self.runtime(format!(
                    "a read of {n} bytes at +{at:#x} runs outside the {:#x}-byte object",
                    self.obj.len()
                ))
            })
    }

    fn i32_at(&self, at: i64) -> Result<i32> {
        Ok(i32::from_le_bytes(self.bytes(at, 4)?.try_into().unwrap()))
    }

    /// The change mask's test, as the walker makes it: word `(bit >> 5) & 0xFFFF` must be inside
    /// the mask, and the full mask has exactly `mask_n` low bits set.
    fn mask_bit(&self, bit: u32) -> bool {
        let word = (bit >> 5) & 0xFFFF;
        let words = self.mask_n.wrapping_add(31) >> 5;
        word < words
            && u64::from(word) * 32 + u64::from(bit & 31) < u64::from(self.mask_n)
            && self
                .mask
                .is_none_or(|mask| mask[word as usize] & (1 << (bit & 31)) != 0)
    }

    /// One presence bit, if the field has one. Returns whether the field is present.
    fn presence(&mut self, f: &Field, bit: u32) -> bool {
        if f.optional == 0 {
            return true;
        }
        let present = self.mask_bit(bit);
        self.w.write_bit(present);
        present
    }

    fn descriptor(&mut self, d: &Descriptor, obj: i64, count: i64, base: u32, depth: usize) -> Result<()> {
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
                let i32i = i as i32;
                let off = obj + i64::from(e.offset.wrapping_mul(i32i));
                let bitoff = e.mask_bit.wrapping_mul(i as u32);
                if !self.presence(e, base.wrapping_add(bitoff)) {
                    continue;
                }
                if e.kind == KIND_COMPOSITE {
                    // The count source is read relative to the ARRAY's object (`r8 = [rsp+0x28]`).
                    let child_base = base.wrapping_add(bitoff).wrapping_add(u32::from(e.optional));
                    self.composite(e, obj, off, child_base, depth)?;
                } else {
                    self.leaf(e, off, base.wrapping_add(bitoff))?;
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
            let bit = base.wrapping_add(f.mask_bit);
            if !self.presence(f, bit) {
                continue;
            }
            let at = obj + i64::from(f.offset);
            if f.kind == KIND_COMPOSITE {
                let child_base = bit.wrapping_add(u32::from(f.optional));
                self.composite(f, obj, at, child_base, depth)?;
            } else {
                self.leaf(f, at, bit)?;
            }
        }
        Ok(())
    }

    /// Descend into a composite field or element. `parent` is where the count source is read.
    fn composite(&mut self, f: &Field, parent: i64, at: i64, base: u32, depth: usize) -> Result<()> {
        let child = self.s.schemas.get(f.child)?;
        let count = if f.count_source() == 1 {
            i64::from(self.i32_at(parent + i64::from(f.count_offset()))?)
        } else {
            top_count(child)
        };
        self.descriptor(child, at, count, base, depth + 1)
    }

    fn leaf(&mut self, f: &Field, at: i64, _bit: u32) -> Result<()> {
        let bias = f.bias();
        let width = f.width();
        match f.kind {
            2 => {
                let b = self.bytes(at, 1)?[0];
                self.w.write_bit_byte(b);
            }
            3 => {
                let v = i32::from(self.bytes(at, 1)?[0] as i8).wrapping_add(bias as i32) as u32;
                self.w.write(u64::from(v), width);
            }
            4 => {
                let v = i16::from_le_bytes(self.bytes(at, 2)?.try_into().unwrap());
                let v = i32::from(v).wrapping_add(bias as i32) as u32;
                self.w.write(u64::from(v), width);
            }
            5 | 9 => {
                let v = u32::from_le_bytes(self.bytes(at, 4)?.try_into().unwrap()).wrapping_add(bias);
                self.w.write(u64::from(v), width);
            }
            6 | 10 => {
                let v = i64::from_le_bytes(self.bytes(at, 8)?.try_into().unwrap());
                let v = v.wrapping_add(i64::from(bias as i32)) as u64;
                self.w.write(v, width);
            }
            7 => {
                let v = u32::from(self.bytes(at, 1)?[0]).wrapping_add(bias);
                self.w.write(u64::from(v), width);
            }
            8 => {
                let v = u16::from_le_bytes(self.bytes(at, 2)?.try_into().unwrap());
                let v = u32::from(v).wrapping_add(bias);
                self.w.write(u64::from(v), width);
            }
            KIND_FLOAT => {
                let v = u32::from_le_bytes(self.bytes(at, 4)?.try_into().unwrap());
                let q = f.params[2];
                if (1..=31).contains(&q) {
                    let v = crate::quant::encode(
                        f32::from_bits(v),
                        f32::from_bits(f.params[0]),
                        f32::from_bits(f.params[1]),
                        q,
                    );
                    self.w.write(u64::from(v), q);
                } else {
                    self.w.write(u64::from(v), 32);
                }
            }
            // 0x2CA6B0 with world bounds disabled, as in the existing server/oracle.
            // Configured-world compression has a different selector/quantizer grammar.
            0x0D => {
                for lane in 0..3 {
                    let v = u32::from_le_bytes(self.bytes(at + lane * 4, 4)?.try_into().unwrap());
                    self.w.write(v as u64, 32);
                }
            }
            0x15..=0x17 => {
                // `0x1028EE0`: one bit `v != -1`, then the 32 bits of `v` when set.
                let v = u32::from_le_bytes(self.bytes(at, 4)?.try_into().unwrap());
                self.w.write_bit(v != u32::MAX);
                if v != u32::MAX {
                    self.w.write(u64::from(v), 32);
                }
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

    /// Kind `0x29`, `0x102A650`: encode `{u32 handle}` as an object of the cell's schema, then,
    /// unless the handle is `-1`, the bytes after it as an object of type `handle`. Both are
    /// complete `0x2C9A80` calls, each with its own full mask.
    fn typed_ref(&mut self, at: i64, payload_offset: i64) -> Result<()> {
        let handle_bytes = self.bytes(at, 4)?.to_vec();
        let handle = u32::from_le_bytes(handle_bytes.clone().try_into().unwrap());
        let (_, handle_tag) = self.s.schemas.cells.typed_ref_handle;
        let hd = self.s.schemas.get(handle_tag)?;
        if hd.struct_size > 4 {
            // `0x102A650` passes a 4-byte local; a wider handle schema would read its stack.
            return Err(self.runtime(format!(
                "typed-reference handle schema {handle_tag:#010x} is {} bytes, wider than the \
                 4-byte local the client passes",
                hd.struct_size
            )));
        }
        self.s.encode_into(&mut *self.w, handle_tag, &handle_bytes)?;
        if handle == u32::MAX {
            return Ok(());
        }
        let td = self.s.schemas.get(handle).map_err(|_| {
            self.runtime(format!(
                "typed reference names {handle:#010x}, which has no descriptor"
            ))
        })?;
        let payload = self
            .bytes(at + payload_offset, td.struct_size as usize)?
            .to_vec();
        self.s.encode_into(&mut *self.w, handle, &payload)
    }

    fn object_at(&mut self, tag: u32, at: i64) -> Result<()> {
        let size = self.s.schemas.get(tag)?.struct_size as usize;
        let image = self.bytes(at, size)?.to_vec();
        self.s.encode_into(self.w, tag, &image)
    }

    /// 0x102A500: selectors at +0/+4, primary at +0x10, secondary at +8.
    fn atom(&mut self, at: i64) -> Result<()> {
        let primary = self.bytes(at, 1)?[0];
        let secondary = self.bytes(at + 4, 1)?[0];
        let pt = self.s.schemas.atom_tag(primary, false, self.top)?;
        let st = self.s.schemas.atom_tag(secondary, true, self.top)?;
        for (offset, cell) in [0, 4].into_iter().zip(self.s.schemas.cells.atom_selectors) {
            self.object_at(cell.1, at + offset)?;
        }
        self.object_at(pt, at + 16)?;
        self.object_at(st, at + 8)
    }
}
