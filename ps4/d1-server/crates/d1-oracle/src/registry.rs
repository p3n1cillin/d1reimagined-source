// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The synthetic, **fail-closed** package registry the RSAT codecs walk.
//!
//! Port of `trace-investment-schema.py`'s `build_registry`, and the reasoning is the load-bearing
//! part:
//!
//! > Decoder `0x261FB0` inlines the registry walk THREE times - RIP-refs to the global `0x1A0B420`
//! > at `0x2622C7`, `0x2623FE`, `0x2624EF` - each with different register allocation, and its
//! > callees resolve more. **Per-site stubs are therefore the fragile choice: a site we have not
//! > found is a SILENT WRONG ANSWER until it happens to fault.** We build the real structure once
//! > and let the client's own code run.
//!
//! Construction contract:
//!
//! ```text
//! *(0x1A0B420) = holder ;  *holder = registry
//! registry + slot*0x40 :  +0x10 data_base   +0x30 entry_size(24)   +0x34 mask(-1)
//! entry E = data_base + index*24 ;  object = E - (mask & qword[E+8]) = E - loc
//!   occupied   -> loc = E - O   (resolves to O)
//!   UNOCCUPIED -> loc = E       (resolves to NULL, not a plausible address)   <-- FAIL CLOSED
//! ```
//!
//! Two details that are easy to get wrong and expensive to debug:
//!
//! 1. ⚠ **Never key on `tag & 0x1fff` alone.** Across the 4,555 descriptors that collapses onto
//!    3,873 indices with 656 collisions. [`tag_slot`] reproduces the client's own arithmetic.
//! 2. ⚠ **The descriptor index holds ANCHORS, not registry-object starts,** and the prefix varies
//!    (`0x50` for `0x80801825`/`0x80801796`, `0x58` for `0x808045f0`). [`object_start`] recovers it
//!    by the structural relation `O + 0x48 + qword[O+0x48] == anchor + 8`, which is unique for
//!    every tag tested. A fixed offset would silently mis-resolve.

use std::collections::{BTreeMap, BTreeSet};

use d1_eboot::descriptors::{self, Index, LEAF};
use d1_eboot::image::Image;

use crate::error::{OracleError, Result};
use crate::harness::Harness;

pub const REGISTRY_GLOBAL: u64 = 0x1A0_B420;
pub const ENTRY_SIZE: u64 = 24;

/// `(slot, index)` - the client's own arithmetic, read out of the walk at `0x2622D1`.
pub fn tag_slot(tag: u32) -> (u32, u32) {
    let eax = tag as i32;
    let esi = eax >> 13; // arithmetic
    let edx = ((esi as u32) >> 0x12) | 0x3FF;
    (((edx as i32) & esi) as u32, tag & 0x1FFF)
}

/// Registry-object start `O` for a descriptor anchor. Unique candidate, or `None`.
pub fn object_start(image: &Image, anchor: u64, span: u64) -> Option<u64> {
    let mut hits = Vec::new();
    for o in anchor.saturating_sub(span)..anchor {
        if let Ok(v) = image.u64(o + 0x48) {
            if o.wrapping_add(0x48).wrapping_add(v) == anchor + 8 {
                hits.push(o);
                if hits.len() > 1 {
                    return None;
                }
            }
        }
    }
    hits.first().copied()
}

/// Transitive closure of child tags from `root`.
///
/// **Minimal by construction** - a tag enters only because the schema reaches it, never because it
/// exists in the image. That is what makes an unoccupied slot meaningful.
pub fn closure(image: &Image, idx: &Index, root: u32) -> BTreeSet<u32> {
    let mut seen = BTreeSet::new();
    let mut todo = vec![root];
    while let Some(t) = todo.pop() {
        if seen.contains(&t) {
            continue;
        }
        let Some(d) = idx.get(t) else { continue };
        seen.insert(t);
        for (_, f) in descriptors::fields(image, d.anchor, d.field_count) {
            let Some(f) = f else { break }; // missing sentinel: the layout broke, stop this struct
            if f.child_tag != LEAF {
                todo.push(f.child_tag);
            }
        }
    }
    seen
}

/// Install a minimal fail-closed registry covering exactly the closure of `root`.
/// Returns the tags it covers.
pub fn build(
    h: &mut Harness,
    image: &Image,
    idx: &Index,
    root: u32,
    registry_global: u64,
    extra_roots: &[u32],
) -> Result<BTreeSet<u32>> {
    let mut tags = closure(image, idx, root);
    for &r in extra_roots {
        tags.extend(closure(image, idx, r));
    }
    if tags.is_empty() {
        return Err(OracleError::UnknownTag(root));
    }
    let mut slots: BTreeMap<u32, BTreeMap<u32, u64>> = BTreeMap::new();
    for &t in &tags {
        let (slot, index) = tag_slot(t);
        let anchor = idx.get(t).expect("in closure ⇒ in index").anchor;
        // Some small atom payloads contain a second, accidental relative-link
        // match inside the real descriptor. Use an independently measured
        // startup cell only when it names this tag and satisfies the same
        // anchor relation. Unknown/ambiguous descriptors still fail closed.
        let measured = crate::codec::codec_for(&image.sha256()).and_then(|c| {
            let candidates: BTreeSet<u64> =
                c.type_cells
                    .iter()
                    .filter(|&&(_, _, tag, _)| tag == t)
                    .map(|&(_, _, _, object)| object)
                    .filter(|&object| {
                        image.u64(object + 0x48).ok().is_some_and(|v| {
                            object.wrapping_add(0x48).wrapping_add(v) == anchor + 8
                        })
                    })
                    .collect();
            (candidates.len() == 1).then(|| *candidates.first().unwrap())
        });
        let o = object_start(image, anchor, 0x300)
            .or(measured)
            .ok_or_else(|| {
                OracleError::Eboot(format!(
                    "tag {t:#010x}: registry-object start not uniquely recoverable from anchor \
                 {anchor:#x}"
                ))
            })?;
        slots.entry(slot).or_default().insert(index, o);
    }

    let max_slot = *slots.keys().next_back().unwrap() as u64;
    let registry = h.alloc(0x40 * (max_slot + 1))?;
    for (&slot, entries) in &slots {
        let n = *entries.keys().next_back().unwrap() as u64 + 1;
        let data_base = h.alloc(n * ENTRY_SIZE)?;
        for i in 0..n {
            // fail closed: every entry resolves to NULL …
            let e = data_base + i * ENTRY_SIZE;
            h.write_u64(e + 8, e)?;
        }
        for (&index, &o) in entries {
            // … except the ones the schema actually reaches
            let e = data_base + u64::from(index) * ENTRY_SIZE;
            h.write_u64(e + 8, e.wrapping_sub(o))?;
        }
        let rec = registry + u64::from(slot) * 0x40;
        h.write_u64(rec + 0x10, data_base)?;
        h.write_i32(rec + 0x30, ENTRY_SIZE as i32)?;
        h.write_i32(rec + 0x34, -1)?;
    }

    let holder = h.alloc(8)?;
    h.write_u64(holder, registry)?;
    h.write_u64(registry_global, holder)?;
    Ok(tags)
}
