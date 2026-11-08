// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The schema model the native encoder walks, loaded once from the owned eboot.
//!
//! It is [`d1_eboot::descriptors`]' anchored descriptors, with the columns the client's walker reads
//! and that reader does not keep. The walker (56578 `0x101C220`, reached from the object encoder
//! `0x2C9A80` -> `0x2C8EB0`) addresses a descriptor as `d = anchor + 8`:
//!
//! | walker read | anchor-relative | meaning |
//! |---|---|---|
//! | `[d]` (i64) | `+0x08` | field count |
//! | `[d+0x14]` | `+0x1C` | the top schema's change-mask size, minus one (`0x2C8EB0` sizes the mask `+1`) |
//! | `[d+0x18]` (i32) | `+0x20` | `> 0`: an **array** type with this capacity |
//! | `[d+0x20/0x24/0x28]` | `+0x28/+0x2C/+0x30` | array: element byte stride, second stride, element mask-bit stride |
//! | `[d+0x30]` (u16) | `+0x38` | array: element kind word |
//! | `[d+0x34]` | `+0x3C` | array: element child tag |
//! | `[d+0x38..0x48]` | `+0x40..+0x50` | array: element parameter block |
//! | `[d + i*0x28 + 0x20]` (i32) | `marker-8` | field C offset |
//! | `[d + i*0x28 + 0x28]` | `marker+0` | field mask bit (`serialized_start`) |
//! | `[d + i*0x28 + 0x30]` (byte) | `marker+8` | kind: `1` composite, else the leaf-encoder index |
//! | `[d + i*0x28 + 0x31]` (byte) | `marker+9` | non-zero: one presence bit precedes the field |
//! | `[d + i*0x28 + 0x34]` | `marker+0xC` | child tag |
//! | `[d + i*0x28 + 0x38..0x48]` | `marker+0x10..0x20` | parameter block: a leaf's `{bias, width, ...}`, or a composite's `{count source (byte), count offset}` |
//!
//! The array header overlays field 0's record, so an array type's element is field 0 read whole.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::RwLock;

use d1_eboot::descriptors::{self, ANCHOR, FIELDS_AT, FIELD_STRIDE, LEAF, SENTINEL};
use d1_eboot::image::Image;

use crate::error::{Result, RsatError};

/// Field kinds the native encoder implements, with the 56578 leaf encoder each one is read from.
/// Everything else is [`RsatError::Unsupported`] and the caller falls back.
pub const SUPPORTED_KINDS: &[(u8, &str)] = &[
    (KIND_COMPOSITE, "composite / array (walker 0x101C220)"),
    (2, "bool, 1 bit from the source byte (0x1028550)"),
    (3, "i8 + bias, width bits (0x1028590)"),
    (4, "i16 + bias, width bits (0x10285D0)"),
    (5, "u32 + bias, width bits (0x1028610)"),
    (6, "i64 + bias, width bits (0x1028650 -> 0x73000)"),
    (7, "u8 + bias, width bits (0x1028670)"),
    (8, "u16 + bias, width bits (0x10286B0)"),
    (9, "u32 + bias, width bits (0x1028610, as kind 5)"),
    (10, "i64 + bias, width bits (0x1028650, as kind 6)"),
    (11, "f32, raw IEEE bits when unquantized (0x10286F0)"),
    (
        0x0D,
        "position xyz, raw world codec (0x1028780 / 0x102C8D0)",
    ),
    (
        0x15,
        "nullable u32 handle: bit (v != -1), then 32 bits (0x1028EE0)",
    ),
    (0x16, "nullable u32 handle, as 0x15 (0x1028EE0)"),
    (0x17, "nullable u32 handle, as 0x15 (0x1028EE0)"),
    (
        KIND_TYPED_REF,
        "typed reference: u32 handle through a type cell, then the handle's type (0x102A650)",
    ),
    (
        0x22,
        "typed value: handle, then object at +0x10 (0x102A3D0)",
    ),
    (0x23, "identity through the selected type cell (0x102A450)"),
    (0x26, "atom: two selectors and their payloads (0x102A500)"),
];

pub const KIND_COMPOSITE: u8 = 1;
pub const KIND_FLOAT: u8 = 11;
/// Kind `0x29`: `0x102A650` encodes the field's `u32` through the schema in a start-up type cell,
/// then, unless it is `-1`, the bytes after it as an object of the type that `u32` names.
pub const KIND_TYPED_REF: u8 = 0x29;

/// One field record (or an array's element record).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    pub index: usize,
    /// `marker-8`; in an array header, the element byte stride.
    pub offset: i32,
    /// `marker+0`; in an array header, the element mask-bit stride.
    pub mask_bit: u32,
    /// `marker+8`, low byte.
    pub kind: u8,
    /// `marker+9`: presence-bit width (0 or 1 in every 56578 descriptor).
    pub optional: u8,
    /// `marker+0xC`; [`LEAF`] for a leaf.
    pub child: u32,
    /// `marker+0x10..0x20`.
    pub params: [u32; 4],
}

impl Field {
    pub fn is_leaf(&self) -> bool {
        self.child == LEAF
    }
    /// A leaf's bias (`params[0]`, added before the write).
    pub fn bias(&self) -> u32 {
        self.params[0]
    }
    /// A leaf's width in bits (`params[1]`).
    pub fn width(&self) -> u32 {
        self.params[1]
    }
    /// A composite's count source: `1` = the element / field count is an `i32` in the parent object.
    pub fn count_source(&self) -> u8 {
        self.params[0] as u8
    }
    /// A composite's count offset in the parent object, when [`Self::count_source`] is 1.
    pub fn count_offset(&self) -> i32 {
        self.params[1] as i32
    }
}

/// One anchored descriptor, as the walker reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Descriptor {
    pub tag: u32,
    pub anchor: u64,
    pub struct_size: u32,
    /// `anchor+0x1C`. The top schema's mask holds `mask_bits + 1` bits.
    pub mask_bits: u32,
    /// `anchor+0x20`; `> 0` makes this an array type.
    pub capacity: i32,
    /// For an array: the element record (header overlaid on field 0). Its `offset` is the byte
    /// stride and its `mask_bit` the mask-bit stride.
    pub element: Option<Field>,
    /// The second stride `anchor+0x2C`, kept for completeness (the encode walker advances it but
    /// never reads it).
    pub stride2: i32,
    /// `anchor+0x08`, the declared field count.
    pub field_count: u32,
    pub fields: Vec<Field>,
}

impl Descriptor {
    pub fn is_array(&self) -> bool {
        self.capacity > 0
    }
}

/// Every anchored descriptor of one image, with the encoder's columns.
#[derive(Debug)]
pub struct Schemas {
    by_tag: BTreeMap<u32, Descriptor>,
    /// Memoised [`Self::check`] / [`Self::check_decode`] results, by `(tag, decode)`: every encode
    /// checks its schema, and a closure walk per message would be wasted work on the hot path.
    checked: RwLock<BTreeMap<(u32, bool), Result<()>>>,
    /// SHA-256 of the image the descriptors came from.
    pub image_sha256: String,
    /// Build-specific start-up type cells this image's encoders read (see [`crate::build`]).
    pub cells: &'static crate::build::BuildCells,
    /// Raw position decoding supplies this fourth SIMD lane from the owned image.
    pub position_w: u32,
    /// The client quantizer's midpoint multiplier, read at 56578 0x1610430.
    pub quant_half: f32,
    /// The identity arm selected by the embedding protocol. The offline oracle/server
    /// default is Bytes; the client can select Platform. There is no wire selector.
    pub identity_format: IdentityFormat,
}

impl Schemas {
    /// Load from an image. Refuses an image with no measured [`crate::build::BuildCells`], because
    /// typed references would then encode through an unknown type.
    pub fn load(image: &Image) -> Result<Self> {
        let sha = image.sha256();
        let cells = crate::build::cells_for(&sha).ok_or_else(|| RsatError::Build(format!(
            "no native RSAT build table for image sha256 {sha}; only measured builds are encoded"
        )))?;
        let index = descriptors::index(image).map_err(|e| RsatError::Image(e.to_string()))?;
        let mut by_tag = BTreeMap::new();
        for (&tag, d) in index.iter() {
            by_tag.insert(tag, read_descriptor(image, d)?);
        }
        let position_w = image
            .u32(cells.position_w)
            .map_err(|e| RsatError::Image(e.to_string()))?;
        let quant_half = f32::from_bits(
            image
                .u32(cells.quant_half)
                .map_err(|e| RsatError::Image(e.to_string()))?,
        );
        Ok(Self {
            by_tag,
            checked: RwLock::default(),
            image_sha256: sha,
            cells,
            position_w,
            quant_half,
            identity_format: IdentityFormat::Bytes,
        })
    }

    /// Load from the selected build's eboot (`$D1_EBOOT`, else the build registry's choice).
    pub fn load_default() -> Result<Self> {
        let image = Image::open_default().map_err(|e| RsatError::Image(e.to_string()))?;
        Self::load(&image)
    }

    pub fn get(&self, tag: u32) -> Result<&Descriptor> {
        self.by_tag.get(&tag).ok_or(RsatError::UnknownTag(tag))
    }

    pub fn len(&self) -> usize {
        self.by_tag.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_tag.is_empty()
    }

    /// Every tag reachable from `tag` through composite children and array elements.
    pub fn closure(&self, tag: u32) -> Result<BTreeSet<u32>> {
        let mut seen = BTreeSet::new();
        let mut todo = vec![tag];
        while let Some(t) = todo.pop() {
            if !seen.insert(t) {
                continue;
            }
            let d = self.get(t)?;
            for f in d.fields.iter().chain(d.element.iter()) {
                if f.kind == KIND_COMPOSITE {
                    todo.push(f.child);
                }
                match f.kind {
                    KIND_TYPED_REF | 0x22 => todo.push(self.cells.typed_ref_handle.1),
                    0x23 => todo.extend(self.cells.identity.iter().map(|c| c.1)),
                    0x26 => todo.extend(
                        self.cells
                            .atom_selectors
                            .iter()
                            .chain(self.cells.atom_primary)
                            .chain(self.cells.atom_secondary)
                            .map(|c| c.1),
                    ),
                    _ => {}
                }
            }
        }
        Ok(seen)
    }

    /// ⭐ The static capability check: every field reachable from `tag` is a kind the native
    /// encoder implements, with parameters it implements. `Ok` means only run-time conditions
    /// (counts out of range, a typed reference naming a type) can still refuse an encode.
    pub fn check(&self, tag: u32) -> Result<()> {
        self.memo(tag, false, || self.check_uncached(tag))
    }

    fn memo(&self, tag: u32, decode: bool, f: impl FnOnce() -> Result<()>) -> Result<()> {
        if let Some(r) = self.checked.read().unwrap_or_else(|e| e.into_inner()).get(&(tag, decode)) {
            return r.clone();
        }
        let r = f();
        self.checked.write().unwrap_or_else(|e| e.into_inner()).insert((tag, decode), r.clone());
        r
    }

    fn check_uncached(&self, tag: u32) -> Result<()> {
        for t in self.closure(tag)? {
            let d = self.get(t)?;
            if d.is_array() {
                let e = d.element.as_ref().expect("array has an element");
                check_field(t, e, true)?;
            } else {
                if d.fields.len() != d.field_count as usize {
                    return Err(RsatError::Unsupported {
                        tag: t,
                        field: d.fields.len(),
                        what: "a field record without its sentinel (the layout broke)".into(),
                    });
                }
                for f in &d.fields {
                    check_field(t, f, false)?;
                }
            }
        }
        Ok(())
    }
}

impl Schemas {
    /// [`Self::check`] for the **decoder**: the same constructs, less integer leaves of width 0.
    /// The encoder writes those as the client does (the value ORed into the accumulator); the
    /// client's reader (`0x72C70`) returns its cached word whole for them, which depends on the
    /// reader's word alignment and is not modelled.
    pub fn check_decode(&self, tag: u32) -> Result<()> {
        self.memo(tag, true, || self.check_decode_uncached(tag))
    }

    fn check_decode_uncached(&self, tag: u32) -> Result<()> {
        self.check(tag)?;
        for t in self.closure(tag)? {
            let d = self.get(t)?;
            for f in d.fields.iter().chain(d.element.iter()) {
                if (3..=10).contains(&f.kind) && f.width() == 0 {
                    return Err(RsatError::Unsupported {
                        tag: t,
                        field: f.index,
                        what: format!("decoding kind {:#x} with width 0", f.kind),
                    });
                }
            }
        }
        Ok(())
    }
}

fn check_field(tag: u32, f: &Field, element: bool) -> Result<()> {
    let unsupported = |what: String| Err(RsatError::Unsupported { tag, field: f.index, what });
    if f.optional > 1 {
        return unsupported(format!("a {}-bit presence flag", f.optional));
    }
    match f.kind {
        KIND_COMPOSITE => {
            if f.is_leaf() {
                return unsupported("a composite with no child".into());
            }
            let _ = element;
            Ok(())
        }
        2 => Ok(()),
        3..=10 => {
            if f.width() > 64 || (!matches!(f.kind, 6 | 10) && f.width() > 32) {
                return unsupported(format!("kind {:#x} with width {}", f.kind, f.width()));
            }
            Ok(())
        }
        KIND_FLOAT => {
            let q = f.params[2] as i32;
            if (1..=31).contains(&q) {
                let lo = f32::from_bits(f.params[0]);
                let hi = f32::from_bits(f.params[1]);
                if !lo.is_finite() || !hi.is_finite() || lo >= hi {
                    return unsupported(format!("quantized f32 bounds [{lo}, {hi}]"));
                }
            }
            Ok(())
        }
        KIND_TYPED_REF | 0x0D | 0x15..=0x17 | 0x22 | 0x23 | 0x26 => Ok(()),
        k => unsupported(format!("field kind {k:#04x}")),
    }
}

/// Kind 0x23 has two grammars; it carries no selector on the wire (0x102A450/0x102D280).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityFormat {
    Bytes,
    Platform,
}

impl Schemas {
    pub fn identity_tag(&self) -> u32 {
        self.cells.identity[usize::from(self.identity_format == IdentityFormat::Platform)].1
    }

    pub(crate) fn atom_tag(&self, selector: u8, secondary: bool, top: u32) -> Result<u32> {
        let table = if secondary {
            self.cells.atom_secondary
        } else {
            self.cells.atom_primary
        };
        table
            .get(selector as usize)
            .map(|c| c.1)
            .ok_or_else(|| RsatError::Runtime {
                tag: top,
                what: format!(
                    "atom {} selector {selector} outside 0..{}",
                    if secondary { "secondary" } else { "primary" },
                    table.len()
                ),
            })
    }
}

fn read_descriptor(image: &Image, d: &descriptors::Descriptor) -> Result<Descriptor> {
    let a = d.anchor;
    let u32_at = |va: u64| image.u32(va).map_err(|e| RsatError::Image(e.to_string()));
    if image.bytes(a, 8).map_err(|e| RsatError::Image(e.to_string()))? != ANCHOR {
        return Err(RsatError::Image(format!("{a:#x} is not a descriptor anchor")));
    }
    let mask_bits = u32_at(a + 0x1C)?;
    let capacity = u32_at(a + 0x20)? as i32;
    let stride2 = u32_at(a + 0x2C)? as i32;
    let read_field = |i: usize| -> Result<Option<Field>> {
        let mk = a + FIELDS_AT + i as u64 * FIELD_STRIDE;
        if u32_at(mk + 4)? != SENTINEL {
            return Ok(None);
        }
        let kind_word = u32_at(mk + 8)?;
        Ok(Some(Field {
            index: i,
            offset: u32_at(mk - 8)? as i32,
            mask_bit: u32_at(mk)?,
            kind: kind_word as u8,
            optional: (kind_word >> 8) as u8,
            child: u32_at(mk + 0xC)?,
            params: [u32_at(mk + 0x10)?, u32_at(mk + 0x14)?, u32_at(mk + 0x18)?, u32_at(mk + 0x1C)?],
        }))
    };
    let mut fields = Vec::with_capacity(d.field_count as usize);
    for i in 0..d.field_count as usize {
        match read_field(i)? {
            Some(f) => fields.push(f),
            // A broken layout: keep what parsed. `check` then refuses any walk that reaches the gap,
            // because the field count no longer matches.
            None => break,
        }
    }
    let element = if capacity > 0 {
        // The header overlays field 0: anchor+0x28 stride = marker-8, +0x30 bit stride = marker+0,
        // +0x38 kind word, +0x3C child, +0x40 params - exactly field 0's columns.
        let e = read_field(0)?.ok_or_else(|| {
            RsatError::Image(format!("array type {:#010x} has no element record", d.tag))
        })?;
        Some(e)
    } else {
        None
    };
    Ok(Descriptor {
        tag: d.tag,
        anchor: a,
        struct_size: d.struct_size,
        mask_bits,
        capacity,
        element,
        stride2,
        field_count: d.field_count,
        fields,
    })
}

/// A human-readable dump of `tag`'s closure with every column the encoder reads.
pub fn render(schemas: &Schemas, tag: u32) -> Result<String> {
    let mut s = String::new();
    for t in schemas.closure(tag)? {
        let d = schemas.get(t)?;
        s.push_str(&format!(
            "{:#010x} size {:#x} mask_bits {} {}anchor {:#x}\n",
            d.tag,
            d.struct_size,
            d.mask_bits,
            if d.is_array() { format!("ARRAY capacity {} ", d.capacity) } else { String::new() },
            d.anchor
        ));
        let rows: Vec<&Field> = match &d.element {
            Some(e) => vec![e],
            None => d.fields.iter().collect(),
        };
        for f in rows {
            s.push_str(&format!(
                "   [{}] {} {:#06x} kind {:#04x}{} {} params {:08x} {:08x} {:08x} {:08x}\n",
                f.index,
                if d.is_array() { "stride" } else { "off" },
                f.offset,
                f.kind,
                if f.optional != 0 { " OPT" } else { "" },
                if f.is_leaf() {
                    format!("leaf maskbit {:#x}", f.mask_bit)
                } else {
                    format!("-> {:#010x} maskbit {:#x}", f.child, f.mask_bit)
                },
                f.params[0],
                f.params[1],
                f.params[2],
                f.params[3]
            ));
        }
        if !d.is_array() && d.fields.len() != d.field_count as usize {
            s.push_str("   <layout broke: fewer field records than the count>\n");
        }
    }
    Ok(s)
}
