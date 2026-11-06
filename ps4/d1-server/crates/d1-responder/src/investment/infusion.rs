// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! 804: infuse `source` into `target` through the target's infusion `node`.
//!
//! The client sends it from `0xA43860` kind 3 after `0x26F260(source, target)` returned a mask of
//! 0 and the node check `0x284E10(target, node, ok << 3)` passed. The server repeats the mask
//! ([`refusal_mask`], the same bits):
//!
//! | Bit | Refused when |
//! |---|---|
//! | `0x1000` | an instance is missing |
//! | `0x800` | a definition is missing |
//! | `0x100` | the source is not in a bucket of the target's type (`0x26BE90`, definition `+0x88`) |
//! | `0x400` / `0x40` | the target has no infusion category / the two differ (definition `+0x30` block `+0x4C`): ⚠ **not exported**, so not checked here |
//! | `0x1` | the source's state flags have bit 0 (the lock, slot `+0x18`) |
//! | `0x80` | the source is equipped (`0x2696B0`, profile `+0x2B80`) |
//! | `0x200` | the gain is at most `0.0001` |
//!
//! **The gain** (`0x26F400`): each item's value is its level (`+0x18`) plus the quality curve at
//! its curve offset (`+0x20`) plus its talent quality (`0x286890`, the active nodes' contributions);
//! when the source's is higher, `gain = min(diff, max(diff × rate, curve(floor)))` with `rate` and
//! `floor` from the target tier's row (`items/infusion.tsv`, `content::infusion_gain`). ⚠ The
//! talent-quality term is not exported and is taken as 0 for both items.
//!
//! **The mutation** (never observed: no 804 is in any capture): the target's level rises by the
//! gain, rounded down (`infusion.level`, a labelled reading of "the value is level + curve"), and the
//! source is destroyed. The infusion node's own cost stays `light.infusion_cost` ("no cost").
//! The push is by analogy with a dismantle's and an 801's: the deleted source first, then the
//! profile (its slot freed), then the target's element.

use super::snapshot::{element as el, Place, Snapshot};
use super::talents::item_def;
use super::{Refusal, Status};

/// The manifest's item tier type hashes, by `rewards` tier (the rows of `items/infusion.tsv`).
fn tier_hash(t: d1_loadout::model::Tier) -> Option<u32> {
    use d1_loadout::model::Tier::*;
    Some(match t {
        Common => 3_340_296_461,
        Uncommon => 2_395_677_314,
        Rare => 2_127_292_149,
        Legendary => 4_008_398_120,
        Exotic => 2_759_499_571,
        _ => return None,
    })
}

/// The Attack quality curve (`STAT_DAMAGE`), the curve `content::infusion_gain` samples.
const ATTACK: u32 = 368_428_387;

/// An item's value in `0x26F400`'s units (talent quality taken as 0).
pub fn value(e: &[u8]) -> Option<f32> {
    let curve = crate::content::stat_curves().iter().find(|c| c.hash == ATTACK)?;
    let q = curve.quality(f32::from(el::i16_at(e, el::CURVE_OFFSET)))?;
    Some(el::i32_at(e, el::LEVEL) as f32 + q)
}

/// The gain the client computes, and the mask of `0x26F260`.
pub fn refusal_mask(s: &Snapshot, target: u64, source: u64) -> (u16, f32) {
    let (Some(te), Some(se)) = (s.elements.get(&target), s.elements.get(&source)) else { return (0x1000, 0.0) };
    let (Some(tc), Some(sc)) = (s.content_of(target), s.content_of(source)) else { return (0x1000, 0.0) };
    let (Some(td), Some(_sd)) = (item_def(tc), item_def(sc)) else { return (0x800, 0.0) };
    let tb = crate::content::bucket_of(tc);
    let sb = match s.locate(source) {
        Some(Place::Profile { bucket, .. }) => Some(bucket),
        _ => None,
    };
    if tb.is_none() || sb != tb {
        return (0x100, 0.0);
    }
    let mut mask = 0u16;
    if s.flags(source).unwrap_or(0) & 1 != 0 {
        mask |= 0x1;
    }
    if s.is_equipped(source) {
        mask |= 0x80;
    }
    let gain = match (value(te), value(se), tier_hash(td.tier)) {
        (Some(t), Some(f), Some(h)) if f > t => crate::content::infusion_gain(h, f - t).unwrap_or(0.0),
        _ => 0.0,
    };
    if gain <= 0.0001 {
        mask |= 0x200;
    }
    (mask, gain)
}

pub fn infuse(s: &mut Snapshot, target: u64, node: i8, source: u64) -> Result<String, Refusal> {
    if target == source {
        return Err(Refusal::new(Status::Invalid, "an item cannot be infused into itself"));
    }
    let (mask, gain) = refusal_mask(s, target, source);
    if mask != 0 {
        let status = if mask & 0x1800 != 0 { Status::NotFound } else { Status::NotAvailable };
        return Err(Refusal::new(status, format!("0x26F260 mask {mask:#x} (gain {gain})")));
    }
    let tc = s.content_of(target).unwrap();
    let grid = item_def(tc).and_then(|d| crate::rewards::tables::tables().grids.get(&d.talent_grid));
    if let Some(g) = grid {
        if node < 0 || !g.nodes.iter().any(|n| n.index == node as u16) {
            return Err(Refusal::new(Status::Invalid, format!("grid {} has no node {node}", g.hash)));
        }
    }
    let mut e = s.elements[&target].clone();
    let before = el::i32_at(&e, el::LEVEL);
    let after = before.saturating_add(gain.floor() as i32);
    el::set_i32(&mut e, el::LEVEL, after);
    s.remove_instance(source).map_err(|why| Refusal::new(Status::NotFound, why))?;
    s.set_element(target, e);
    Ok(format!("{source:#x} into {target:#x} (node {node}): level {before} -> {after} (gain {gain:.3}, infusion.level reading)"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::investment::snapshot::tests::{equip, snapshot, with_instance};
    use crate::investment::snapshot::Touch;
    use crate::rewards::tables::tables;
    use d1_loadout::model::Tier;

    /// Two Legendary weapons of one bucket, the target's grid with a node 0.
    fn pair() -> (u16, u16) {
        let t = tables();
        let mut defs: Vec<_> = t.items.values().filter(|d| d.instanced && d.tier == Tier::Legendary && d.bucket == 1_498_876_634).collect();
        defs.sort_by_key(|d| d.hash);
        let cs: Vec<u16> = defs.iter().filter_map(|d| crate::content::item_index(d.hash)).filter(|c| crate::content::bucket_of(*c) == Some(0x02)).take(2).collect();
        (cs[0], cs[1])
    }

    fn element(level: i32) -> Vec<u8> {
        let mut e = vec![0; el::BYTES];
        el::set_i32(&mut e, el::LEVEL, level);
        e
    }

    fn two(target_level: i32, source_level: i32) -> Snapshot {
        let (a, b) = pair();
        let mut s = snapshot();
        with_instance(&mut s, a, 0x401, element(target_level));
        with_instance(&mut s, b, 0x402, element(source_level));
        s
    }

    #[test]
    fn a_legendary_takes_the_whole_difference_and_the_source_is_gone() {
        let mut s = two(30, 40);
        let (mask, gain) = refusal_mask(&s, 0x401, 0x402);
        assert_eq!(mask, 0);
        assert!((gain - 10.0).abs() < 0.001, "Legendary rate 1: {gain}");
        infuse(&mut s, 0x401, 0, 0x402).unwrap();
        assert_eq!(el::i32_at(&s.elements[&0x401], el::LEVEL), 40);
        assert_eq!(s.locate(0x402), None);
        assert_eq!(s.log, vec![Touch::Deleted(0x402), Touch::Profile, Touch::Item(0x401)]);
    }

    #[test]
    fn the_clients_refusals_are_the_servers() {
        // Lower source: no gain.
        let mut s = two(40, 30);
        assert_eq!(refusal_mask(&s, 0x401, 0x402).0, 0x200);
        assert!(infuse(&mut s, 0x401, 0, 0x402).is_err());
        // Locked source.
        let mut s = two(30, 40);
        if let Some(Place::Profile { slot, .. }) = s.locate(0x402) {
            s.set_profile_flags(slot, 1);
        }
        assert_eq!(refusal_mask(&s, 0x401, 0x402).0, 0x1);
        // Equipped source.
        let mut s = two(30, 40);
        equip(&mut s, 6, 0x402);
        assert_eq!(refusal_mask(&s, 0x401, 0x402).0, 0x80);
        let before = s.clone();
        assert!(infuse(&mut s, 0x401, 0, 0x402).is_err());
        assert_eq!(s, before, "a refusal changes nothing");
        // Missing.
        assert_eq!(refusal_mask(&s, 0x401, 0x999).0, 0x1000);
    }

    #[test]
    fn a_rare_takes_half() {
        let (_, gain) = {
            let mut s = two(30, 40);
            // Rewrite the target as a Rare of the same bucket.
            let t = tables();
            let rare = t.items.values().filter(|d| d.instanced && d.tier == Tier::Rare && d.bucket == 1_498_876_634)
                .filter_map(|d| crate::content::item_index(d.hash)).find(|c| crate::content::bucket_of(*c) == Some(0x02)).unwrap();
            if let Some(Place::Profile { slot, .. }) = s.locate(0x401) {
                let (_, i, q, f) = s.profile_slot(slot);
                let _ = (i, q, f);
                let e = crate::vendor::PROFILE_SLOTS + 32 * slot as usize;
                s.profile[e..e + 2].copy_from_slice(&rare.to_le_bytes());
            }
            refusal_mask(&s, 0x401, 0x402)
        };
        assert!((gain - 5.0).abs() < 0.001, "Rare rate 0.5: {gain}");
    }
}
