// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Item instances: what the server knows about one item it made (its Light, rolled perks, armour
//! stats, XP and grid), and how much of that it can write into the client's own item element.
//!
//! The client's element is `0x80801732` (184 bytes; ps4/design/items-and-gear.md section 2). The
//! fields a server fills, with their status:
//!
//! | Offset | Field | Written from |
//! |---|---|---|
//! | `+0x00` u64 | instance SOID | [`ItemInstance::id`] |
//! | `+0x18` i32 | item level | [`ClientIndex::level_for_primary`] (the client's level curves; open) |
//! | `+0x24` u16 | content index | [`ClientIndex::content_index`] (56578's item table) |
//! | `+0x38` u32 | progression points (XP) | the grid's XP |
//! | `+0x44` u16 | progression ordinal, `0xFFFF` none | [`ClientIndex::progression_ordinal`] |
//! | `+0x58` u16 | talent grid index, `0xFFFF` none | [`ClientIndex::grid_index`] |
//! | `+0x5A` 36 B | node states | NOT WRITTEN: layout not decoded (`item.node_state`) |
//! | `+0x7E` 8 B | step selection | NOT WRITTEN (`item.node_state`) |
//! | `+0x98` 8 x u32 | stat values, by the definition's stat-list position | [`ClientIndex::stat_position`] |
//!
//! The indices are 56578 table positions, not manifest hashes, so they come from the caller (the
//! responder builds them from `capture-rig/item-content-index.tsv` and the content tables). Until
//! a resolver answers, the field is left as the template has it and named in the result.
//!
//! Evidence needed to finish it: the node-state byte layout (a served instance with one node
//! toggled, read back from the client), the per-item level inverse (the global curves from tag
//! `0x80A5E244` are now in `items/stat-curves.json`, but offsets/rank/talents still matter), and
//! the order of a definition's stat list at `def+0x68`.

use anyhow::{bail, Result};

use super::store::InstanceId;
use super::talent_grid::GridState;
use super::tables::tables;
use super::Rng;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ItemInstance {
    pub id: InstanceId,
    pub item_hash: u32,
    /// Attack or Defense: the item's Light. 0 for items without a primary stat.
    pub primary: u32,
    pub quantity: u32,
    pub grid: Option<GridState>,
    /// Rolled armour stats: `(stat hash, value)`.
    pub stats: Vec<(u32, u32)>,
}

impl ItemInstance {
    /// Make a new instance of `item_hash` at `light`: the Light clamped to the item's own range,
    /// random perks rolled (`grid.random_roll`), armour stats rolled within their manifest range
    /// (`item.stat_roll`).
    pub fn roll(id: InstanceId, item_hash: u32, light: u32, rng: &mut Rng) -> Result<ItemInstance> {
        let Some(def) = tables().items.get(&item_hash) else { bail!("item {item_hash} is not in the manifest table") };
        if !def.instanced {
            bail!("item {item_hash} is not instanced");
        }
        let primary = if def.primary_stat == 0 {
            0
        } else if def.primary_max > 0 {
            light.clamp(def.primary_min, def.primary_max)
        } else {
            light.max(def.primary_min)
        };
        let grid = if def.talent_grid != 0 { Some(GridState::new(def.talent_grid, rng)?) } else { None };
        let stats = def.rolled_stats.iter().map(|(h, lo, hi)| (*h, rng.range(*lo, *hi))).collect();
        Ok(ItemInstance { id, item_hash, primary, quantity: 1, grid, stats })
    }
}

/// 56578 table positions the element needs. Every method may answer `None` (not known yet).
pub trait ClientIndex {
    fn content_index(&self, item_hash: u32) -> Option<u16>;
    fn progression_ordinal(&self, progression_hash: u32) -> Option<u16>;
    fn grid_index(&self, grid_hash: u32) -> Option<u16>;
    /// The element level (`+0x18`) whose curve gives `primary` for this item.
    fn level_for_primary(&self, item_hash: u32, primary: u32) -> Option<i32>;
    /// The position of `stat_hash` in the item definition's stat list (`def+0x68`), 0..8.
    fn stat_position(&self, item_hash: u32, stat_hash: u32) -> Option<usize>;
}

/// The committed 56578 item and progression indices. Item-specific curve offsets, talent-grid
/// indices and rolled-stat positions still need their own resolver; missing answers remain open.
pub struct ExportedIndex;
impl ClientIndex for ExportedIndex {
    fn content_index(&self, hash:u32) -> Option<u16> { crate::content::item_index(hash) }
    fn progression_ordinal(&self, hash:u32) -> Option<u16> { crate::content::progression_index(hash) }
    fn grid_index(&self, _:u32) -> Option<u16> { None }
    fn level_for_primary(&self, _:u32, _:u32) -> Option<i32> { None }
    fn stat_position(&self, _:u32, _:u32) -> Option<usize> { None }
}

pub const ELEMENT_BYTES: usize = 0xB8;
const NONE16: u16 = 0xFFFF;

/// Write `inst` into a copy of `template` (a decoded `0x80801732` element). Returns the element
/// and the fields left as the template had them because the resolver could not answer.
/// The content index is required: without it the element would name the wrong item, silently.
pub fn write_element(template: &[u8], inst: &ItemInstance, index: &dyn ClientIndex) -> Result<(Vec<u8>, Vec<&'static str>)> {
    if template.len() != ELEMENT_BYTES {
        bail!("item element template is {} bytes, expected {ELEMENT_BYTES}", template.len());
    }
    let Some(content) = index.content_index(inst.item_hash) else {
        bail!("no 56578 content index for item {}", inst.item_hash);
    };
    let mut e = template.to_vec();
    let mut open = Vec::new();
    e[0x00..0x08].copy_from_slice(&inst.id.to_le_bytes());
    e[0x24..0x26].copy_from_slice(&content.to_le_bytes());
    match index.level_for_primary(inst.item_hash, inst.primary) {
        Some(level) => e[0x18..0x1C].copy_from_slice(&level.to_le_bytes()),
        None if inst.primary > 0 => open.push("level (+0x18)"),
        None => {}
    }
    let (xp, ordinal, grid) = match &inst.grid {
        None => (0, Some(NONE16), Some(NONE16)),
        Some(g) => {
            let gd = tables().grids.get(&g.grid);
            let ord = gd.and_then(|gd| index.progression_ordinal(gd.progression));
            (g.xp, ord, index.grid_index(g.grid))
        }
    };
    e[0x38..0x3C].copy_from_slice(&xp.to_le_bytes());
    match ordinal {
        Some(o) => e[0x44..0x46].copy_from_slice(&o.to_le_bytes()),
        None => open.push("progression ordinal (+0x44)"),
    }
    match grid {
        Some(gi) => e[0x58..0x5A].copy_from_slice(&gi.to_le_bytes()),
        None => open.push("talent grid index (+0x58)"),
    }
    if inst.grid.is_some() {
        open.push("node states (+0x5A, +0x7E)");
    }
    for (stat, value) in &inst.stats {
        match index.stat_position(inst.item_hash, *stat).filter(|p| *p < 8) {
            Some(p) => e[0x98 + 4 * p..0x9C + 4 * p].copy_from_slice(&value.to_le_bytes()),
            None => {
                if !open.contains(&"stat values (+0x98)") {
                    open.push("stat values (+0x98)");
                }
            }
        }
    }
    Ok((e, open))
}

#[cfg(test)]
mod tests {
    use super::*;
    use d1_loadout::model::Tier;

    struct Known;
    impl ClientIndex for Known {
        fn content_index(&self, _: u32) -> Option<u16> {
            Some(3598)
        }
        fn progression_ordinal(&self, _: u32) -> Option<u16> {
            Some(12)
        }
        fn grid_index(&self, _: u32) -> Option<u16> {
            Some(345)
        }
        fn level_for_primary(&self, _: u32, _: u32) -> Option<i32> {
            Some(40)
        }
        fn stat_position(&self, _: u32, stat: u32) -> Option<usize> {
            Some((stat % 3) as usize)
        }
    }
    struct Unknown;
    impl ClientIndex for Unknown {
        fn content_index(&self, _: u32) -> Option<u16> {
            Some(1)
        }
        fn progression_ordinal(&self, _: u32) -> Option<u16> {
            None
        }
        fn grid_index(&self, _: u32) -> Option<u16> {
            None
        }
        fn level_for_primary(&self, _: u32, _: u32) -> Option<i32> {
            None
        }
        fn stat_position(&self, _: u32, _: u32) -> Option<usize> {
            None
        }
    }

    fn legendary_armour() -> u32 {
        let mut v: Vec<_> = tables()
            .items
            .values()
            .filter(|d| d.tier == Tier::Legendary && d.instanced && d.talent_grid != 0 && !d.rolled_stats.is_empty() && d.primary_max == 400)
            .map(|d| d.hash)
            .collect();
        v.sort();
        v[0]
    }

    #[test]
    fn a_rolled_instance_stays_inside_its_definition() {
        let hash = legendary_armour();
        let def = &tables().items[&hash];
        for seed in 0..50 {
            let i = ItemInstance::roll(7, hash, 420, &mut Rng::new(seed)).unwrap();
            assert_eq!(i.primary, def.primary_max);
            for ((h, v), (dh, lo, hi)) in i.stats.iter().zip(&def.rolled_stats) {
                assert_eq!(h, dh);
                assert!((*lo..=*hi).contains(v));
            }
            assert_eq!(i.grid.as_ref().unwrap().grid, def.talent_grid);
        }
        assert_eq!(ItemInstance::roll(7, hash, 0, &mut Rng::new(1)).unwrap().primary, def.primary_min);
        assert!(ItemInstance::roll(7, crate::rewards::currencies::GLIMMER, 0, &mut Rng::new(1)).is_err());
    }

    #[test]
    fn element_fields_land_at_their_offsets() {
        let inst = ItemInstance::roll(0xD100_0000_0000_1301, legendary_armour(), 385, &mut Rng::new(3)).unwrap();
        let template = vec![0xAAu8; ELEMENT_BYTES];
        let (e, open) = write_element(&template, &inst, &Known).unwrap();
        assert_eq!(u64::from_le_bytes(e[0..8].try_into().unwrap()), inst.id);
        assert_eq!(i32::from_le_bytes(e[0x18..0x1C].try_into().unwrap()), 40);
        assert_eq!(u16::from_le_bytes([e[0x24], e[0x25]]), 3598);
        assert_eq!(u32::from_le_bytes(e[0x38..0x3C].try_into().unwrap()), inst.grid.as_ref().unwrap().xp);
        assert_eq!(u16::from_le_bytes([e[0x44], e[0x45]]), 12);
        assert_eq!(u16::from_le_bytes([e[0x58], e[0x59]]), 345);
        assert!(e[0x5A..0x86].iter().all(|b| *b == 0xAA), "node state must stay as the template has it");
        assert_eq!(open, vec!["node states (+0x5A, +0x7E)"]);

        let (e, open) = write_element(&template, &inst, &Unknown).unwrap();
        assert_eq!(&e[0x18..0x1C], &[0xAA; 4]);
        assert!(open.contains(&"level (+0x18)") && open.contains(&"talent grid index (+0x58)"));
        assert!(write_element(&template[1..], &inst, &Known).is_err());
    }
}
