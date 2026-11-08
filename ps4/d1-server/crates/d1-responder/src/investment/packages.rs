// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! 904 `vendor_interact`: claim a rank-up or faction package at its vendor, turn in at the
//! Gunsmith, and the vendors' other "Rewards" sales.
//!
//! **The request** (`0x808019EE`): `{vendor, a, b, sale}`. The item request handler (`0xA29D30`,
//! action 10) fills them from its `+0x60` (the vendor row), `+0x70`, `+0x72` and `+0x54` (the sale
//! index, as for a 901). The live Tower capture (#328) sent `{13, 5, 0, 114}` (Dead Orbit's
//! package), `{11, 19, 0, 211}` (the Warlock Vanguard's rank-up package), `{25, 2, 0, 242}` (the
//! Gunsmith's weapon-test turn-in) and `{22, 20, 0, -1}` (a Shipwright freebie). `a` and `b`'s
//! table is open; they are logged, not used.
//!
//! **The sale** is the vendor's own (`vendor-sales.tsv`): Dead Orbit's 114 and the Warlock
//! Vanguard's 211 are their faction's **Package** item (client bucket `0x13`, free; in the
//! manifest a "Rewards" category hidden from purchase, `tooltipStyle` package), the Gunsmith's 242
//! a "Turn in" reward (bucket `0x12`).
//!
//! **A package** opens straight into the inventory, never the Postmaster (#328):
//! - eligibility: the vendor's progression rank, as the player store keeps it
//!   (`PlayerStore::reputation`), less the packages of that progression already claimed
//!   (`investment.packages.<progression>`): one package a rank;
//! - contents ([`package_contents`]): **500 Glimmer** (both live packages; `package.glimmer`
//!   stand-in), **one gear item** from the package vendor's sales (the manifest's "Guaranteed
//!   Item"), for the character's class, seeded by character, progression and rank
//!   (`package.pick`, stand-in: the live servers rolled it), and for a faction package (not the
//!   Vanguard's or the Crucible's) **4 Motes of Light** (the Dead Orbit package, one capture;
//!   `package.motes` stand-in). The capture's "+4 of material slot 6" is not named, so not given.
//! - the push, as live: account (Glimmer), profile (Motes, the item's slot), the new item.
//!
//! **Any other Rewards sale** (the Gunsmith's turn-in) gives its item into its bucket. ⚠ Which
//! test weapon the turn-in consumes is open (the live push deleted one), so none is taken.
//! **`sale = -1`** (the Shipwright's freebie): which item it gives is open; refused.

use serde_json::json;

use super::snapshot::Snapshot;
use super::{Refusal, Status};
use crate::rewards::currencies::{GLIMMER, MOTE_OF_LIGHT};

/// Stand-ins, labelled (see the module note).
pub const PACKAGE_GLIMMER: u32 = 500;
pub const FACTION_PACKAGE_MOTES: u32 = 4;
/// The package token's client bucket: reward pseudo-items, granted and never kept.
const TEMPORARY: u8 = 0x13;
const GEAR_BUCKETS: [u8; 8] = [0x02, 0x03, 0x04, 0x07, 0x08, 0x09, 0x0a, 0x0b];

fn sales() -> &'static std::collections::HashMap<(u8, u16), crate::vendor::Sale> {
    static S: std::sync::OnceLock<std::collections::HashMap<(u8, u16), crate::vendor::Sale>> = std::sync::OnceLock::new();
    S.get_or_init(|| crate::vendor::parse_sales(include_str!("../../../../../fixtures/56578/vendor/vendor-sales.tsv")))
}

/// What a package gives: `(content, amount)` stacks and gear items (content indices).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Contents {
    pub stacks: Vec<(u16, u32)>,
    pub gear: Vec<u16>,
    pub rules: Vec<&'static str>,
}

fn class_fits(content: u16, class: u32) -> bool {
    super::talents::item_def(content).is_some_and(|d| d.class_type == 3 || d.class_type == class)
}

/// A package's contents for `vendor`'s progression at `rank`, for a character of `class`.
pub fn package_contents(vendor: u8, rank: u32, character: u64, class: u32) -> Contents {
    let t = crate::tables::tables();
    let mut c = Contents::default();
    let Some(v) = t.vendors.get(&vendor) else { return c };
    if let Some(g) = crate::content::item_index(GLIMMER) {
        c.stacks.push((g, PACKAGE_GLIMMER));
        c.rules.push("package.glimmer");
    }
    let package_vendors = crate::vendors::reputation::package_vendors(t, v.progression);
    let faction = package_vendors.iter().filter_map(|i| t.vendors.get(i)).all(|p| !p.name.contains("Vanguard") && !p.name.contains("Crucible"));
    if faction && !package_vendors.is_empty() {
        if let Some(m) = crate::content::item_index(MOTE_OF_LIGHT) {
            c.stacks.push((m, FACTION_PACKAGE_MOTES));
            c.rules.push("package.motes");
        }
    }
    let mut pool: Vec<u16> = package_vendors.iter().flat_map(|&pv| {
        let mut k: Vec<_> = sales().iter().filter(|((sv, _), s)| *sv == pv && s.instanced && GEAR_BUCKETS.contains(&s.bucket)).map(|((_, n), s)| (*n, s.item)).collect();
        k.sort();
        k.into_iter().map(|(_, i)| i)
    }).filter(|i| class_fits(*i, class)).collect();
    pool.dedup();
    if !pool.is_empty() {
        let seed = character ^ (u64::from(v.progression) << 20) ^ u64::from(rank);
        let k = crate::vendors::pick(seed, pool.len(), 1)[0];
        c.gear.push(pool[k]);
        c.rules.push("package.pick");
    }
    c
}

fn claimed_key(progression: u32) -> String {
    format!("investment.packages.{progression}")
}

pub fn interact(s: &mut Snapshot, vendor: i8, a: i16, b: i16, sale: i32, template: &[u8]) -> Result<String, Refusal> {
    let t = crate::tables::tables();
    let v = u8::try_from(vendor).ok().and_then(|v| t.vendors.get(&v))
        .ok_or_else(|| Refusal::new(Status::Invalid, format!("no vendor row {vendor}")))?;
    let Ok(sale) = u16::try_from(sale) else {
        return Err(Refusal::new(Status::Invalid, format!("vendor {} interaction {a}/{b} with no sale: its table is open", v.index)));
    };
    let row = sales().get(&(v.index, sale)).ok_or_else(|| Refusal::new(Status::Invalid, format!("vendor {} has no sale {sale}", v.index)))?;
    if !row.costs.is_empty() {
        return Err(Refusal::new(Status::Invalid, format!("vendor {} sale {sale} has a price: a purchase is a 901", v.index)));
    }
    if row.bucket == TEMPORARY && !row.instanced {
        // A package: one a rank of the vendor's progression.
        let name = t.progressions.get(&v.progression).map(|p| p.name.clone())
            .ok_or_else(|| Refusal::new(Status::Invalid, format!("vendor {} has no progression", v.index)))?;
        let rank = s.reputation.get(&name).map_or(0, |r| r.1);
        let claimed = s.attribute(&claimed_key(v.progression)).and_then(|x| x.as_u64()).unwrap_or(0) as u32;
        if rank <= claimed {
            return Err(Refusal::new(Status::NotAvailable, format!("{name}: rank {rank}, {claimed} package(s) claimed")));
        }
        let c = package_contents(v.index, claimed + 1, s.character, s.class_type);
        let mut given = Vec::new();
        for &(content, n) in &c.stacks {
            let cap = crate::content::item_hash(content).map_or(u32::MAX, crate::rewards::currencies::cap);
            let g = s.give(content, n, cap).map_err(|e| Refusal::new(Status::NoRoom, e))?;
            given.push(format!("{g} of {content}"));
        }
        for &content in &c.gear {
            let soid = s.give_instance(content, template.to_vec()).map_err(|e| Refusal::new(Status::NoRoom, e))?;
            given.push(format!("{content} as {soid:#x}"));
        }
        s.set_attribute(&claimed_key(v.progression), json!(claimed + 1));
        return Ok(format!("{name} package {} (rank {rank}, interaction {a}/{b}): {} [{}]", claimed + 1, given.join(", "), c.rules.join(", ")));
    }
    // Another Rewards sale: its item.
    let what = if row.instanced {
        format!("{:#x}", s.give_instance(row.item, template.to_vec()).map_err(|e| Refusal::new(Status::NoRoom, e))?)
    } else {
        let cap = crate::content::item_hash(row.item).map_or(u32::MAX, crate::rewards::currencies::cap);
        format!("{}", s.give(row.item, 1, cap).map_err(|e| Refusal::new(Status::NoRoom, e))?)
    };
    Ok(format!("vendor {} sale {sale} (interaction {a}/{b}): item {} ({what}); the turned-in item is open", v.index, row.item))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::investment::snapshot::tests::snapshot;
    use crate::investment::snapshot::Touch;

    fn dead_orbit() -> Snapshot {
        let mut s = snapshot();
        s.class_type = 2;
        let name = crate::tables::tables().progressions[&crate::tables::tables().vendors[&13].progression].name.clone();
        s.reputation.insert(name, (1_500, 1));
        s
    }

    #[test]
    fn the_dead_orbit_package_opens_into_the_inventory() {
        let mut s = dead_orbit();
        let what = interact(&mut s, 13, 5, 0, 114, &[0; 0xB8]).unwrap();
        assert_eq!(s.quantity(crate::content::item_index(GLIMMER).unwrap()), 500, "{what}");
        assert_eq!(s.quantity(crate::content::item_index(MOTE_OF_LIGHT).unwrap()), 4);
        let gear = *s.elements.keys().next().expect("a gear item");
        assert!(matches!(s.locate(gear), Some(super::super::snapshot::Place::Profile { .. })), "the inventory, not the Postmaster");
        assert_ne!(super::super::snapshot::profile_bucket(match s.locate(gear) { Some(super::super::snapshot::Place::Profile { slot, .. }) => slot, _ => 0 }), Some(0x10));
        assert_eq!(s.log, vec![Touch::Account, Touch::Profile, Touch::Item(gear)], "the live order");
        // One package a rank.
        assert_eq!(interact(&mut s, 13, 5, 0, 114, &[0; 0xB8]).unwrap_err().status, Status::NotAvailable);
    }

    #[test]
    fn the_vanguard_package_has_no_motes_and_a_rank_is_needed() {
        let mut s = snapshot();
        assert_eq!(interact(&mut s, 11, 19, 0, 211, &[0; 0xB8]).unwrap_err().status, Status::NotAvailable);
        let t = crate::tables::tables();
        s.reputation.insert(t.progressions[&t.vendors[&11].progression].name.clone(), (2_000, 1));
        interact(&mut s, 11, 19, 0, 211, &[0; 0xB8]).unwrap();
        assert_eq!(s.quantity(crate::content::item_index(MOTE_OF_LIGHT).unwrap()), 0);
        assert_eq!(s.quantity(crate::content::item_index(GLIMMER).unwrap()), 500);
    }

    #[test]
    fn a_turn_in_gives_its_sale_and_unknown_selectors_are_refused() {
        let mut s = snapshot();
        interact(&mut s, 25, 2, 0, 242, &[0; 0xB8]).unwrap();
        assert_eq!(s.quantity(4563), 1);
        assert_eq!(interact(&mut s, 22, 20, 0, -1, &[0; 0xB8]).unwrap_err().status, Status::Invalid);
        assert_eq!(interact(&mut s, 25, 0, 0, 374, &[0; 0xB8]).unwrap_err().status, Status::Invalid, "a priced sale is a 901");
    }
}
