// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Numeric 56578 exports shared by the store and rewards. No private inputs at runtime.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

struct Indices {
    by_index: BTreeMap<u16, u32>,
    by_hash: BTreeMap<u32, u16>,
    progressions: BTreeMap<u32, u16>,
}

fn indices() -> &'static Indices {
    static TABLE: OnceLock<Indices> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = Indices {
            by_index: BTreeMap::new(),
            by_hash: BTreeMap::new(),
            progressions: BTreeMap::new(),
        };
        for l in include_str!("../../../../fixtures/56578/items/item-content-index.tsv")
            .lines()
            .filter(|l| !l.starts_with('#') && !l.starts_with("item_hash") && !l.is_empty())
        {
            let f: Vec<_> = l.split('\t').collect();
            let h =
                u32::from_str_radix(f[0].trim_start_matches("0x"), 16).expect("exported item hash");
            let i = f[1].parse().expect("exported item index");
            assert!(t.by_index.insert(i, h).is_none());
            assert!(t.by_hash.insert(h, i).is_none());
        }
        for l in include_str!("../../../../fixtures/56578/progression/progression-index.tsv")
            .lines()
            .filter(|l| !l.starts_with('#') && !l.starts_with("index") && !l.is_empty())
        {
            let (i, h) = l.split_once('\t').expect("exported progression row");
            assert!(t
                .progressions
                .insert(h.parse().unwrap(), i.parse().unwrap())
                .is_none());
        }
        t
    })
}

pub fn item_hash(index: u16) -> Option<u32> {
    indices().by_index.get(&index).copied()
}
pub fn item_index(hash: u32) -> Option<u16> {
    indices().by_hash.get(&hash).copied()
}
pub fn progression_index(hash: u32) -> Option<u16> {
    indices().progressions.get(&hash).copied()
}

/// Where the client keeps items, by content index: the client bucket ordinal (`BUCKETS` in
/// `vendor.rs`).
struct Buckets {
    /// From the item definitions themselves (`def+0x88`), as `vendor-sales.tsv` and the starting
    /// kit export them: every sold item and every price item.
    by_content: BTreeMap<u16, u8>,
    /// Manifest `bucketHash` -> ordinal, derived from the rows above: each item with an ordinal
    /// gives one pair, and a manifest bucket is kept only when all its pairs agree.
    by_manifest: BTreeMap<u32, u8>,
}

fn buckets() -> &'static Buckets {
    static TABLE: OnceLock<Buckets> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut by_content = BTreeMap::new();
        let hex = |s: &str| u8::from_str_radix(s.trim_start_matches("0x"), 16).ok();
        for l in include_str!("../../../../fixtures/56578/vendor/vendor-sales.tsv").lines() {
            if l.starts_with('#') || l.starts_with("vendor") {
                continue;
            }
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() != 7 {
                continue;
            }
            if let (Ok(c), Some(b)) = (f[3].parse::<u16>(), hex(f[4])) {
                by_content.insert(c, b);
            }
            for cost in f[6].split(',').filter(|c| *c != "-") {
                let p: Vec<&str> = cost.split(':').collect();
                if let (Some(Ok(c)), Some(Some(b))) = (p.first().map(|c| c.parse::<u16>()), p.get(2).map(|b| hex(b))) {
                    by_content.insert(c, b);
                }
            }
        }
        for class in default_gear() {
            for i in class.items.iter().chain(&class.alternate_items) {
                by_content.entry(i.index).or_insert(i.bucket);
            }
        }
        let mut pairs: BTreeMap<u32, Option<u8>> = BTreeMap::new();
        for (&c, &b) in &by_content {
            let Some(def) = item_hash(c).and_then(|h| crate::rewards::tables::tables().items.get(&h)) else { continue };
            let e = pairs.entry(def.bucket).or_insert(Some(b));
            if *e != Some(b) {
                *e = None; // disagreeing rows: no derived ordinal for this manifest bucket
            }
        }
        let by_manifest = pairs.into_iter().filter_map(|(m, b)| Some((m, b?))).collect();
        Buckets { by_content, by_manifest }
    })
}

/// The client bucket ordinal content index `index` is kept in: its own definition's (where it is
/// sold or priced, or in the starting kit), else its manifest bucket's derived ordinal. `None`
/// when neither is known.
pub fn bucket_of(index: u16) -> Option<u8> {
    let b = buckets();
    if let Some(o) = b.by_content.get(&index) {
        return Some(*o);
    }
    let def = crate::rewards::tables::tables().items.get(&item_hash(index)?)?;
    b.by_manifest.get(&def.bucket).copied()
}

#[derive(Deserialize)]
pub struct DefaultGearItem {
    pub index: u16,
    pub bucket: u8,
    pub instanced: bool,
}

#[derive(Deserialize)]
pub struct DefaultGearClass {
    pub class: u8,
    pub items: Vec<DefaultGearItem>,
    pub alternate_items: Vec<DefaultGearItem>,
}

pub fn default_gear() -> &'static [DefaultGearClass] {
    static GEAR: OnceLock<Vec<DefaultGearClass>> = OnceLock::new();
    GEAR.get_or_init(|| {
        #[derive(Deserialize)]
        struct Table {
            data: Vec<DefaultGearClass>,
        }
        let t: Table = serde_json::from_str(include_str!(
            "../../../../fixtures/56578/characters/starting-kit-lists.json"
        ))
        .expect("exported default gear");
        t.data
    })
}

pub fn default_gear_item(index: u16, class: u8) -> Option<&'static DefaultGearItem> {
    let c = default_gear().iter().find(|c| c.class == class)?;
    c.items
        .iter()
        .chain(&c.alternate_items)
        .find(|i| i.index == index)
}

#[derive(Deserialize)]
pub struct StatCurve {
    pub index: u8,
    pub hash: u32,
    pub flags: u32,
    pub points: Vec<[i32; 2]>,
    pub quality_points: Vec<(i32, f32)>,
    pub slot_weights: Vec<[u16; 2]>,
}

pub fn stat_curves() -> &'static [StatCurve] {
    static CURVES: OnceLock<Vec<StatCurve>> = OnceLock::new();
    CURVES.get_or_init(|| {
        #[derive(Deserialize)]
        struct Table {
            data: Vec<StatCurve>,
        }
        let t: Table = serde_json::from_str(include_str!(
            "../../../../fixtures/56578/items/stat-curves.json"
        ))
        .expect("exported stat curves");
        for (i, curve) in t.data.iter().enumerate() {
            assert_eq!(usize::from(curve.index), i, "exported curve ordinals");
        }
        t.data
    })
}

/// The client's clamped piecewise-linear sampling, including duplicate x coordinates.
fn sample(points: &[(i32, f32)], x: f32) -> Option<f32> {
    if !x.is_finite() {
        return None;
    }
    let (first, last) = (points.first()?, points.last()?);
    if x <= first.0 as f32 {
        return Some(first.1);
    }
    if x >= last.0 as f32 {
        return Some(last.1);
    }
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if x >= a.0 as f32 && x < b.0 as f32 {
            let fraction = (x - a.0 as f32) / (b.0 - a.0) as f32;
            // Client 0x915E0: a + t*(b-a), using single precision.
            return Some(a.1 + fraction * (b.1 - a.1));
        }
    }
    None
}

impl StatCurve {
    pub fn quality(&self, x: f32) -> Option<f32> {
        sample(&self.quality_points, x)
    }
    /// The integer curve after quality has been added to the primary instance input. This is
    /// the primary, base-only part of 0x281A20, not a complete rolled/talented item calculation.
    pub fn primary(&self, input: i32, quality_index: i16) -> Option<u32> {
        if self.flags != 0 && !self.slot_weights.is_empty() {
            return None; // an equipment slot is required for a weighted stat
        }
        let quality = if self.quality_points.is_empty() {
            0.0
        } else {
            self.quality(f32::from(quality_index))?
        };
        let points: Vec<_> = self.points.iter().map(|p| (p[0], p[1] as f32)).collect();
        let v = sample(&points, input as f32 + quality)?;
        (v >= 0.0 && v <= u32::MAX as f32).then_some(v as u32)
    }
}

/// Infusion in the client's pre-display primary-stat coordinates, not displayed Attack/Defense.
/// Its floor is a sample of the Attack quality curve, not the literal table index.
pub fn infusion_gain(tier_hash: u32, diff: f32) -> Option<f32> {
    if !diff.is_finite() || diff < 0.0 {
        return None;
    }
    for l in include_str!("../../../../fixtures/56578/items/infusion.tsv")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("index") && !l.is_empty())
    {
        let f: Vec<_> = l.split('\t').collect();
        if f[1].parse::<u32>().ok()? != tier_hash {
            continue;
        }
        let rate: f32 = f[2].parse().ok()?;
        let floor_index: i16 = f[3].parse().ok()?;
        let attack = stat_curves().iter().find(|c| c.hash == 368_428_387)?;
        let floor = attack.quality(f32::from(floor_index))?;
        return Some(diff.min((diff * rate).max(floor)));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_exported_index_round_trips() {
        assert_eq!(indices().by_index.len(), 8609);
        assert_eq!(indices().progressions.len(), 252);
        for (&index, &hash) in &indices().by_index {
            assert_eq!(item_index(hash), Some(index));
            assert_eq!(item_hash(index), Some(hash));
        }
        assert_eq!(progression_index(1716568313), Some(117));
        assert_eq!(item_hash(u16::MAX), None);
    }
    #[test]
    fn every_currency_has_a_client_bucket() {
        use crate::rewards::currencies::*;
        assert_eq!(bucket_of(869), Some(0x1F), "Glimmer: account slot 308");
        assert_eq!(item_index(LEGENDARY_MARKS).and_then(bucket_of), Some(0x23), "Legendary Marks: account slot 310");
        assert_eq!(item_index(VANGUARD_MARKS).and_then(bucket_of), Some(0x20));
        assert_eq!(item_index(WEAPON_PARTS).and_then(bucket_of), Some(0x05), "Materials");
        for h in [GLIMMER, LEGENDARY_MARKS, SILVER_DUST, STRANGE_COIN, MOTE_OF_LIGHT, WEAPON_PARTS, ARMOR_MATERIALS, EXOTIC_SHARD,
                  ASCENDANT_SHARD, ASCENDANT_ENERGY, VANGUARD_MARKS, CRUCIBLE_MARKS].into_iter().chain(PLANETARY) {
            let b = item_index(h).and_then(bucket_of);
            assert!(b.and_then(crate::vendor::bucket).is_some(), "item {h}: bucket {b:?}");
        }
        // The derived manifest-bucket map never contradicts an item's own bucket.
        for (&c, &b) in &buckets().by_content {
            let Some(def) = item_hash(c).and_then(|h| crate::rewards::tables::tables().items.get(&h)) else { continue };
            assert_eq!(buckets().by_manifest.get(&def.bucket), Some(&b), "content {c}");
        }
        assert!(buckets().by_manifest.len() >= 20);
    }
    #[test]
    fn infusion_uses_the_curve_floor_and_tier_rate() {
        let tier = 2127292149;
        assert_eq!(infusion_gain(tier, 10.0), Some(5.0));
        assert!((infusion_gain(tier, 1.5).unwrap() - 1.0).abs() < 0.0001);
        assert_eq!(infusion_gain(tier, 0.5), Some(0.5));
        assert_eq!(infusion_gain(4008398120, 10.0), Some(10.0));
        assert_eq!(infusion_gain(0, 10.0), None);
        let attack = stat_curves().iter().find(|c| c.hash == 368428387).unwrap();
        assert_eq!(attack.primary(40, 0), Some(200));
        assert_eq!(attack.primary(40, 10), Some(210));
    }
}
