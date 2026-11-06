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
