// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! RSAT / investment **anchored schema descriptors**, read statically from the eboot.
//!
//! Port of `capture-rig/eboot-descriptors.py`. That script is now a thin wrapper around this
//! module (`design/format-description.md`: *no single format may be described twice*).
//!
//! # WHY THIS EXISTS
//!
//! `dump-registry.py` needs a running emulator at `investment_signin` and walks the package
//! registry. That was believed necessary because schema tags resolve through registry slot `0x400`,
//! which is "eboot-resident" (`historicaldocs/design/character-and-orbit.md` §15.1). **Eboot-resident means the
//! bytes are in the file.** Verified: the live descriptor for `0x80801796` at vaddr `0x139d1d8` is
//! byte-identical to the same vaddr in `dumps/patch-01.29-CUSA00219/eboot.bin`. So the whole schema
//! tree can be read offline.
//!
//! # FOUR PARSER DEFECTS, ALL OF WHICH PRODUCED CONFIDENT WRONG READINGS
//!
//! These comments are carried across verbatim from the Python. They are the reason this file
//! exists in one language only - a second copy of this layout is free to lose them again.
//!
//! 1. **The C-object offset is at `marker-8`, not `marker+0x20`.** Reading `+0x20` returns the
//!    *next* field's offset, and on the last field it lands in the neighbouring descriptor. This is
//!    what made an 8-byte struct appear to hold a field at `+0xc8`. (§24.10)
//! 2. **The header is NOT at fixed offsets.** `0x80801796` (size `0xc8`) has its field count at
//!    `+0x58`; `0x808045f0` (size `0xd0`) has it at `+0x60`. A fixed-offset read yields a *tag*
//!    where the count should be, which is what made valid descriptors look corrupt and get rejected
//!    as UNRESOLVED. ⇒ everything is parsed relative to the ANCHOR, never from the object start.
//! 3. **The field sentinel is NOT a constant u64.** `marker+0` is `u32 serialized_start`; only
//!    `marker+4` is the constant `0x3F000000`. An exact-u64 test passes only when
//!    `serialized_start` happens to be 0, and falsely reports "missing marker" for every other
//!    field - which made `0x80801941` look like it had variable stride. It does not; stride is an
//!    ordinary `0x28`.
//!    ⚠ This exact reading was ALREADY retracted in §13.4 and re-made in the Python. Check the
//!    retraction list before re-parsing anything here.
//! 4. Consequently the nested composites of `0x80801825` were never readable - the exact part of
//!    the schema the 503 argument grammar depends on.
//!
//! # THE ANCHOR
//!
//! Every descriptor carries the constant pair `0x80800050, 0x8080010a` immediately before its
//! field count. Parse relative to that, never from the object start:
//!
//! ```text
//! anchor+0x00  0x80800050          (constant)
//! anchor+0x04  0x8080010a          (constant)
//! anchor+0x08  u32 field count
//! anchor+0x10  u32 self tag        (a descriptor names itself - used to build the index)
//! anchor+0x14  u32 decoded C struct size
//! anchor+0x30  field records, stride 0x28
//!
//! field record, stride 0x28, located by its sentinel:
//!   marker+0x00  u32 serialized_start        (VARIES - 0,1,3,4,0x69,0xce,0xcf,0xd0 in 0x80801941)
//!   marker+0x04  u32 0x3F000000              (the constant; test THIS, never the full qword)
//!   marker-0x08  u32 offset within the decoded C struct   <-- NOT marker+0x20
//!   marker+0x08  u32 kind
//!   marker+0x0c  u32 child tag (0xffffffff = leaf)
//!   marker+0x14  u32 width in BITS
//! ```
//!
//! ⚠ **ATLAS §4.3 - `marker+0x14` is "width in BITS" only for LEAF fields.** `array-fields.py`
//! proves that on **composite** fields the same column is the byte offset into the parent C object
//! from which the element count is loaded. The Python asserts the bits reading unconditionally at
//! its line 46; [`Field::bits`] is therefore named `raw_bits_or_count_offset` in the typed API and
//! only rendered as "bits" when the field is a leaf.
//!
//! Cross-check applied by [`walk`]: a struct's last field offset plus that field's size must not
//! exceed the declared struct size. That check is what confirms the `marker-8` rule -
//! `0x80801825`'s last field sits at `+0x18` and `0x18 + 944 == 968`, exactly the struct size.

use std::collections::{BTreeMap, HashSet};

use serde::Serialize;

use crate::image::{Image, Result};

/// The constant pair that precedes every field count: `0x80800050, 0x8080010a`, little-endian.
pub const ANCHOR: [u8; 8] = [0x50, 0x00, 0x80, 0x80, 0x0A, 0x01, 0x80, 0x80];

/// ⚠ NOT a constant u64. The low word is `serialized_start` (the validated character table shows
/// 0, 1, 5, 6, 0x15, 0x2bc); ONLY the high word is the constant sentinel. An exact-u64 test passes
/// solely when `serialized_start == 0` and falsely reports "missing marker" for every other field.
/// This exact error is already retracted in §13.4 - it was re-made in the Python and corrected
/// 2026-08-07.
pub const SENTINEL: u32 = 0x3F00_0000;

pub const LEAF: u32 = 0xFFFF_FFFF;

/// Field-record stride. `0x80801941` looked like it had variable stride; it does not (defect 3).
pub const FIELD_STRIDE: u64 = 0x28;
pub const FIELDS_AT: u64 = 0x30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Field {
    pub index: usize,
    /// `marker` vaddr - the sentinel record this field was located by.
    pub marker: u64,
    /// `marker-0x08`. ⚠ NOT `marker+0x20` (defect 1).
    pub offset: u32,
    /// `marker+0x08`.
    pub kind: u32,
    /// `marker+0x0c`; [`LEAF`] means this field is a scalar.
    pub child_tag: u32,
    /// `marker+0x14`. **Bits only when [`Field::is_leaf`]** - on a composite this is the byte
    /// offset in the parent C object that the element count is loaded from (ATLAS §4.3).
    pub raw_bits_or_count_offset: u32,
    /// `marker+0x00`.
    pub serialized_start: u32,
}

impl Field {
    pub fn is_leaf(&self) -> bool {
        self.child_tag == LEAF
    }
    /// Width in bits - meaningful for leaves only.
    pub fn bits(&self) -> Option<u32> {
        self.is_leaf().then_some(self.raw_bits_or_count_offset)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Descriptor {
    pub tag: u32,
    pub anchor: u64,
    pub field_count: u32,
    /// `anchor+0x14` - size of the decoded C struct.
    pub struct_size: u32,
}

/// tag → descriptor, for every anchored descriptor in the image.
#[derive(Debug, Clone, Default)]
pub struct Index {
    by_tag: BTreeMap<u32, Descriptor>,
}

impl Index {
    pub fn get(&self, tag: u32) -> Option<&Descriptor> {
        self.by_tag.get(&tag)
    }
    pub fn len(&self) -> usize {
        self.by_tag.len()
    }
    pub fn is_empty(&self) -> bool {
        self.by_tag.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = (&u32, &Descriptor)> {
        self.by_tag.iter()
    }
    pub fn tags(&self) -> impl Iterator<Item = u32> + '_ {
        self.by_tag.keys().copied()
    }
}

/// Build the descriptor index.
///
/// ⚠ Unlike the Python, the anchor search runs **per file-backed LOAD segment** and converts each
/// hit through that segment's own bias (ATLAS §4.1). The Python searches the whole 29 MB file and
/// applies the text bias `0x4000` globally - correct only because every hit happens to land in
/// LOAD segment 0 in this build. `tests/controls.rs::index_matches_python_whole_file_scan` pins
/// that the two agree here, so the improvement is not a silent behaviour change.
pub fn index(image: &Image) -> Result<Index> {
    let mut by_tag: BTreeMap<u32, Descriptor> = BTreeMap::new();
    for anchor in image.find_all(&ANCHOR) {
        let (Ok(tag), Ok(count)) = (image.u32(anchor + 0x10), image.u32(anchor + 8)) else {
            continue;
        };
        // A descriptor names itself with a 0x8080xxxx tag and has a sane field count.
        if tag & 0xFFFF_0000 == 0x8080_0000 && count <= 64 {
            let struct_size = image.u32(anchor + 0x14).unwrap_or(0);
            by_tag.entry(tag).or_insert(Descriptor { tag, anchor, field_count: count, struct_size });
        }
    }
    Ok(Index { by_tag })
}

/// Read one descriptor's fields. `None` at position `i` means the sentinel was missing there -
/// the layout broke and the caller must stop walking this struct (the Python does exactly that).
pub fn fields(image: &Image, anchor: u64, count: u32) -> Vec<(u64, Option<Field>)> {
    (0..count as usize)
        .map(|i| {
            let mk = anchor + FIELDS_AT + i as u64 * FIELD_STRIDE;
            if image.u32(mk + 4).ok() != Some(SENTINEL) {
                return (mk, None);
            }
            let f = (|| {
                Some(Field {
                    index: i,
                    marker: mk,
                    offset: image.u32(mk.checked_sub(8)?).ok()?, // ⚠ marker-8, NOT marker+0x20
                    kind: image.u32(mk + 8).ok()?,
                    child_tag: image.u32(mk + 0x0C).ok()?,
                    raw_bits_or_count_offset: image.u32(mk + 0x14).ok()?,
                    serialized_start: image.u32(mk).ok()?,
                })
            })();
            (mk, f)
        })
        .collect()
}

/// One field slot as emitted to consumers: either a parsed [`Field`] or a missing sentinel.
///
/// `present == false` reproduces `eboot-descriptors.py`'s `(None, …, marker, None)` yield, which
/// its callers use as the signal to stop walking a struct.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct FieldSlot {
    pub index: usize,
    pub marker: u64,
    pub present: bool,
    pub offset: Option<u32>,
    pub kind: Option<u32>,
    pub child_tag: Option<u32>,
    /// ⚠ bits only when this is a leaf - see [`Field::raw_bits_or_count_offset`].
    pub raw_bits_or_count_offset: Option<u32>,
    pub serialized_start: Option<u32>,
    /// `marker+0x10`: leaf parameter 0, or composite count-source selector.
    pub raw_parameter0: Option<u32>,
    /// `marker+0x18`: leaf parameter 2, or composite count-source data.
    pub raw_parameter2: Option<u32>,
}

impl FieldSlot {
    fn new(image: &Image, index: usize, marker: u64, f: Option<Field>) -> Self {
        Self {
            index,
            marker,
            present: f.is_some(),
            offset: f.map(|f| f.offset),
            kind: f.map(|f| f.kind),
            child_tag: f.map(|f| f.child_tag),
            raw_bits_or_count_offset: f.map(|f| f.raw_bits_or_count_offset),
            serialized_start: f.map(|f| f.serialized_start),
            raw_parameter0: f.and_then(|_| image.u32(marker + 0x10).ok()),
            raw_parameter2: f.and_then(|_| image.u32(marker + 0x18).ok()),
        }
    }
}

/// A descriptor plus its raw field records - the whole-image dump the Python wrapper consumes in
/// ONE call, so that `fields()` never becomes 4,555 subprocess invocations.
#[derive(Debug, Clone, Serialize)]
pub struct DescriptorWithFields {
    #[serde(flatten)]
    pub descriptor: Descriptor,
    pub fields: Vec<FieldSlot>,
    /// Descriptor array header (`array-fields.py`): zero for ordinary structs.
    /// Positive values give the maximum/fixed number of inline elements.
    pub array_capacity: i32,
    /// Header +0x28/+0x2c/+0x30: element strides in the two C images and bit image.
    /// Meaningful only when `array_capacity > 0`; field offsets in arrays are strides.
    pub array_strides: [u32; 3],
}

pub fn dump_with_fields(image: &Image, idx: &Index) -> Vec<DescriptorWithFields> {
    idx.iter()
        .map(|(_, d)| DescriptorWithFields {
            descriptor: d.clone(),
            array_capacity: image.u32(d.anchor + 0x20).unwrap_or(0) as i32,
            array_strides: [0x28, 0x2c, 0x30].map(|o| image.u32(d.anchor + o).unwrap_or(0)),
            fields: fields(image, d.anchor, d.field_count)
                .into_iter()
                .enumerate()
                .map(|(i, (marker, f))| FieldSlot::new(image, i, marker, f))
                .collect(),
        })
        .collect()
}

/// One line of a schema-tree walk, in the order the Python prints them.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "line", rename_all = "snake_case")]
pub enum WalkLine {
    Missing { depth: usize, tag: u32 },
    Struct { depth: usize, tag: u32, struct_size: u32, field_count: u32, anchor: u64 },
    AlreadyExpanded { depth: usize },
    Leaf { depth: usize, index: usize, offset: u32, kind: u32, bits: u32, serialized_start: u32 },
    Composite { depth: usize, index: usize, offset: u32, kind: u32, child_tag: u32, serialized_start: u32 },
    NoMarker { depth: usize, index: usize, marker: u64 },
    Fit { depth: usize, offset: u32, span: u64, total: u64, struct_size: u32, ok: bool },
}

#[derive(Debug, Clone, Serialize)]
pub struct Problem {
    pub tag: u32,
    pub field: usize,
    pub why: String,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct Walk {
    pub lines: Vec<WalkLine>,
    pub problems: Vec<Problem>,
}

/// Walk a schema tree from `tag`, reproducing `eboot-descriptors.py`'s traversal exactly:
/// depth-limited to 6, cycle-guarded by a shared `seen` set, and stopping a struct at the first
/// field whose sentinel is missing.
pub fn walk(image: &Image, idx: &Index, tag: u32) -> Walk {
    let mut out = Walk::default();
    let mut seen = HashSet::new();
    walk_inner(image, idx, tag, 0, &mut seen, &mut out);
    out
}

fn walk_inner(
    image: &Image,
    idx: &Index,
    tag: u32,
    depth: usize,
    seen: &mut HashSet<u32>,
    out: &mut Walk,
) {
    let Some(d) = idx.get(tag) else {
        out.lines.push(WalkLine::Missing { depth, tag });
        return;
    };
    out.lines.push(WalkLine::Struct {
        depth,
        tag,
        struct_size: d.struct_size,
        field_count: d.field_count,
        anchor: d.anchor,
    });
    if seen.contains(&tag) {
        out.lines.push(WalkLine::AlreadyExpanded { depth });
        return;
    }
    seen.insert(tag);

    let fs = fields(image, d.anchor, d.field_count);
    for (i, (mk, f)) in fs.iter().enumerate() {
        let Some(f) = f else {
            out.lines.push(WalkLine::NoMarker { depth, index: i, marker: *mk });
            out.problems.push(Problem { tag, field: i, why: "missing marker".into() });
            return; // the Python stops this struct entirely
        };
        if f.is_leaf() {
            out.lines.push(WalkLine::Leaf {
                depth,
                index: i,
                offset: f.offset,
                kind: f.kind,
                bits: f.raw_bits_or_count_offset,
                serialized_start: f.serialized_start,
            });
        } else {
            out.lines.push(WalkLine::Composite {
                depth,
                index: i,
                offset: f.offset,
                kind: f.kind,
                child_tag: f.child_tag,
                serialized_start: f.serialized_start,
            });
            if depth < 6 {
                walk_inner(image, idx, f.child_tag, depth + 1, seen, out);
            }
        }
    }

    // Cross-check: the last field must fit inside the declared struct. This is what confirms the
    // `marker-8` rule (defect 1) - see the module docstring.
    if let Some(last) = fs.iter().filter_map(|(_, f)| f.as_ref()).last() {
        let span: u64 = if !last.is_leaf() {
            idx.get(last.child_tag).map(|c| c.struct_size as u64).unwrap_or_else(|| {
                (last.raw_bits_or_count_offset as u64 + 7) / 8
            })
        } else {
            (last.raw_bits_or_count_offset as u64 + 7) / 8
        };
        let total = last.offset as u64 + span;
        let ok = total <= d.struct_size as u64;
        out.lines.push(WalkLine::Fit {
            depth,
            offset: last.offset,
            span,
            total,
            struct_size: d.struct_size,
            ok,
        });
        if !ok {
            out.problems.push(Problem {
                tag,
                field: fs.iter().filter(|(_, f)| f.is_some()).count() - 1,
                why: format!("overrun {total} > {}", d.struct_size),
            });
        }
    }
}

/// Render a [`Walk`] byte-for-byte as `eboot-descriptors.py` prints it, so the two can be diffed.
pub fn render(walk: &Walk) -> String {
    let mut s = String::new();
    for line in &walk.lines {
        match *line {
            WalkLine::Missing { depth, tag } => {
                s.push_str(&format!("{}{:#010x}  <not present in eboot>\n", pad(depth), tag));
            }
            WalkLine::Struct { depth, tag, struct_size, field_count, anchor } => s.push_str(&format!(
                "{}{:#010x}  struct {} bytes, {} field(s)   [anchor {:#x}]\n",
                pad(depth),
                tag,
                struct_size,
                field_count,
                anchor
            )),
            WalkLine::AlreadyExpanded { depth } => {
                s.push_str(&format!("{}   (already expanded)\n", pad(depth)));
            }
            WalkLine::Leaf { depth, index, offset, kind, bits, serialized_start } => s.push_str(&format!(
                "{}   [{}] @+{:#05x}  kind {:#x}  LEAF {} bits   sstart {:#x}\n",
                pad(depth),
                index,
                offset,
                kind,
                bits,
                serialized_start
            )),
            WalkLine::Composite { depth, index, offset, kind, child_tag, serialized_start } => s
                .push_str(&format!(
                    "{}   [{}] @+{:#05x}  kind {:#x}  -> {:#010x}   sstart {:#x}\n",
                    pad(depth),
                    index,
                    offset,
                    kind,
                    child_tag,
                    serialized_start
                )),
            WalkLine::NoMarker { depth, index, marker } => s.push_str(&format!(
                "{}   [{}] <no marker at {:#x} - layout broke, stopping this struct>\n",
                pad(depth),
                index,
                marker
            )),
            WalkLine::Fit { depth, offset, span, total, struct_size, ok } => s.push_str(&format!(
                "{}   ↳ fit: last field @+{:#x} + {} = {} vs struct {}  {}\n",
                pad(depth),
                offset,
                span,
                total,
                struct_size,
                if ok { "OK" } else { "*** OVERRUN ***" }
            )),
        }
    }
    s
}

fn pad(depth: usize) -> String {
    "   ".repeat(depth)
}
