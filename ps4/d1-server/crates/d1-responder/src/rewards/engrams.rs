// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Engrams: what one decrypts into at the Cryptarch, and what happens when one is picked up.
//!
//! An engram item (`itemType` 8) carries the bucket of the slot it decrypts into
//! (`engram.bucket`) and its own tier. Decrypting gives an item of that bucket, for the
//! character's class (`engram.class`), at least the engram's tier (`engram.min_tier`, with an
//! upgrade chance that is a placeholder), from the slot's community pool (`engram.pool`) or the
//! labelled fallback, at a Light rolled by [`super::light::roll_drop_light`] under the tier's
//! decrypt cap.
//!
//! On the wire: decrypting is a Cryptarch interaction (the engram's action block says "decrypt";
//! `tools/ps4_setup/d1-item-actions.py` lists 182 such items). Which investment request carries it
//! in 56578 has not been seen yet (901 `vendor_buy_item` and 402 `inventory_perform_item_action`
//! are the candidates); the result reaches the client as a profile push like any other new item.

use anyhow::{bail, Result};
use d1_loadout::model::{Slot, Tier};

use super::currencies::{self, Source, ARMOR_MATERIALS, WEAPON_PARTS};
use super::item_instance::ItemInstance;
use super::light::{self, DropSource, LIGHT_CAP};
use super::placeholders as ph;
use super::store::{CharacterId, InstanceId, RewardStore};
use super::tables::tables;
use super::Rng;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecryptOutcome {
    pub engram: u32,
    pub item: ItemInstance,
    pub tier: Tier,
    pub rules: Vec<&'static str>,
}

fn next_tier(t: Tier) -> Tier {
    match t {
        Tier::Common => Tier::Uncommon,
        Tier::Uncommon => Tier::Rare,
        Tier::Rare => Tier::Legendary,
        Tier::Legendary | Tier::Exotic => Tier::Exotic,
        other => other,
    }
}

/// The items an engram of `tier` for `bucket` can give a character of `class_type`, and the rule
/// that chose the pool.
pub fn candidate_pool(tier: Tier, bucket: u32, class_type: u32) -> (Vec<u32>, &'static str) {
    let t = tables();
    let fits = |h: &u32| t.items.get(h).is_some_and(|d| d.tier == tier && d.bucket == bucket && d.instanced && d.fits_class(class_type));
    let mut community: Vec<u32> = t
        .engram_pools
        .iter()
        .filter(|p| p.tier == tier && p.bucket == bucket && !p.legacy)
        .flat_map(|p| p.items.iter().copied())
        .filter(fits)
        .collect();
    community.sort_unstable();
    community.dedup();
    if !community.is_empty() {
        return (community, "engram.pool");
    }
    let mut fallback: Vec<u32> = t
        .items
        .values()
        .filter(|d| d.tier == tier && d.bucket == bucket && d.instanced && d.equippable && d.fits_class(class_type) && !d.is_engram())
        .filter(|d| d.primary_stat == 0 || d.primary_max >= LIGHT_CAP)
        .map(|d| d.hash)
        .collect();
    fallback.sort_unstable();
    (fallback, "engram.pool.fallback")
}

/// Decrypt the engram instance `engram` the character holds: it is consumed and the new item put
/// in its place.
pub fn decrypt(store: &mut dyn RewardStore, character: CharacterId, engram: InstanceId, rng: &mut Rng) -> Result<DecryptOutcome> {
    let t = tables();
    let Some(inst) = store.item(engram) else { bail!("no item instance {engram:#x}") };
    let engram_hash = inst.item_hash;
    let Some(def) = t.items.get(&engram_hash) else { bail!("item {engram_hash} is not in the manifest table") };
    if !def.is_engram() {
        bail!("item {engram_hash} is not an engram");
    }
    let mut rules = vec!["engram.bucket", "engram.min_tier", "engram.upgrade"];
    let mut tier = def.tier;
    if rng.chance(ph::engram_upgrade_per_mille(tier)) {
        tier = next_tier(tier);
    }
    let class_type = store.class_type(character);
    if Slot::from_bucket_hash(def.bucket).is_some_and(|s| s.is_armor()) {
        rules.push("engram.class");
    }
    let (pool, pool_rule) = candidate_pool(tier, def.bucket, class_type);
    rules.push(pool_rule);
    let Some(item) = rng.pick(&pool).copied() else {
        bail!("no {tier:?} item for bucket {} and class {class_type} to decrypt into", def.bucket);
    };
    let basis = light::basis_for(store, character);
    rules.push("light.basis");
    let roll = light::roll_drop_light(rng, basis, DropSource { tier, floor: 0, source_cap: 0 }, &t.items[&item]);
    rules.extend(roll.rules.iter().copied());
    store.take_item(engram);
    let id = store.new_instance_id();
    let new = ItemInstance::roll(id, item, roll.value, rng)?;
    store.put_item(character, new.clone());
    Ok(DecryptOutcome { engram: engram_hash, item: new, tier, rules })
}

/// What happens to an engram when it is picked up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pickup {
    /// Into the inventory.
    Keep,
    /// `engram.uncommon_convert`: turned into materials.
    Converted { item: u32, quantity: u32 },
}

/// The pickup rule: at level 40, Uncommon (green) engrams become weapon or armour materials.
pub fn on_pickup(store: &mut dyn RewardStore, character: CharacterId, engram_hash: u32) -> Result<Pickup> {
    let Some(def) = tables().items.get(&engram_hash) else { bail!("item {engram_hash} is not in the manifest table") };
    if !def.is_engram() {
        bail!("item {engram_hash} is not an engram");
    }
    if def.tier != Tier::Uncommon || store.character_level(character) < 40 {
        return Ok(Pickup::Keep);
    }
    let material = if Slot::from_bucket_hash(def.bucket).is_some_and(|s| s.is_weapon()) { WEAPON_PARTS } else { ARMOR_MATERIALS };
    let g = currencies::grant(store, character, material, ph::UNCOMMON_CONVERT_AMOUNT, &Source::Other);
    Ok(Pickup::Converted { item: material, quantity: g.granted })
}

/// The engram item of `tier` for `bucket` that end-of-activity loot drops (the lowest hash of the
/// matching engrams: which of several same-tier engrams a source used is not known).
pub fn engram_for(tier: Tier, bucket: u32) -> Option<u32> {
    tables().items.values().filter(|d| d.is_engram() && d.tier == tier && d.bucket == bucket && d.instanced).map(|d| d.hash).min()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewards::store::MemoryStore;

    const CHARACTER: u64 = 0xD100_0000_0000_0101;

    fn store(class_type: u32, level: u32) -> MemoryStore {
        MemoryStore::with_character(1, CHARACTER, class_type, level)
    }

    fn give(s: &mut MemoryStore, item: u32, light: u32) -> InstanceId {
        let id = s.new_instance_id();
        let mut inst = ItemInstance::roll(id, item, light, &mut Rng::new(id)).unwrap_or(ItemInstance {
            id, item_hash: item, primary: 0, quantity: 1, grid: None, stats: Vec::new(),
        });
        inst.id = id;
        s.put_item(CHARACTER, inst);
        id
    }

    /// Decrypt tier rules: the right bucket and class, never below the engram's tier, within the
    /// tier's Light cap, and at or above the player's Light for Rare and Legendary engrams.
    #[test]
    fn decrypts_follow_the_tier_rules() {
        let t = tables();
        let mut engrams: Vec<_> = t
            .items
            .values()
            .filter(|d| d.is_engram() && d.instanced && matches!(d.tier, Tier::Rare | Tier::Legendary | Tier::Exotic))
            .filter(|d| Slot::from_bucket_hash(d.bucket).is_some_and(|s| s.is_weapon() || s.is_armor()))
            .collect();
        engrams.sort_by_key(|d| d.hash);
        let mut checked = 0;
        for class_type in 0..3 {
            for e in &engrams {
                let mut s = store(class_type, 40);
                // A full set of gear at 370 for the basis.
                for slot in [Slot::Primary, Slot::Special, Slot::Heavy, Slot::Head, Slot::Arms, Slot::Chest, Slot::Legs, Slot::ClassItem, Slot::Ghost, Slot::Artifact] {
                    let (pool, _) = candidate_pool(Tier::Legendary, slot.bucket_hash(), class_type);
                    if let Some(h) = pool.first() {
                        give(&mut s, *h, 370);
                    }
                }
                let basis = light::basis_for(&s, CHARACTER);
                let id = give(&mut s, e.hash, 0);
                let Ok(out) = decrypt(&mut s, CHARACTER, id, &mut Rng::new(u64::from(e.hash))) else { continue };
                let d = &t.items[&out.item.item_hash];
                assert_eq!(d.bucket, e.bucket, "engram {} decrypted into another bucket", e.hash);
                assert!(d.fits_class(class_type));
                assert!(out.tier >= e.tier && d.tier == out.tier);
                let (cap, _) = light::decode_cap(out.tier).unwrap();
                assert!(out.item.primary <= cap.max(d.primary_min), "{} over the {cap} cap", out.item.primary);
                if matches!(out.tier, Tier::Rare | Tier::Legendary) {
                    assert!(out.item.primary >= basis.min(cap).min(d.primary_max), "engram {}: {} below basis {basis}", e.hash, out.item.primary);
                }
                assert!(s.item(id).is_none(), "the engram is consumed");
                assert!(s.item(out.item.id).is_some());
                checked += 1;
            }
        }
        assert!(checked > 100, "{checked} decrypts checked");
    }

    #[test]
    fn legendary_pools_come_from_the_community_pages() {
        let (pool, rule) = candidate_pool(Tier::Legendary, Slot::Primary.bucket_hash(), 0);
        assert_eq!(rule, "engram.pool");
        assert!(!pool.is_empty());
        let (_, rule) = candidate_pool(Tier::Rare, Slot::Primary.bucket_hash(), 0);
        assert_eq!(rule, "engram.pool.fallback");
    }

    #[test]
    fn uncommon_engrams_become_materials_at_level_40() {
        let t = tables();
        let green = t.items.values().filter(|d| d.is_engram() && d.tier == Tier::Uncommon && Slot::from_bucket_hash(d.bucket).is_some_and(|s| s.is_weapon())).map(|d| d.hash).min().unwrap();
        let mut s = store(0, 39);
        assert_eq!(on_pickup(&mut s, CHARACTER, green).unwrap(), Pickup::Keep);
        let mut s = store(0, 40);
        assert_eq!(on_pickup(&mut s, CHARACTER, green).unwrap(), Pickup::Converted { item: WEAPON_PARTS, quantity: 1 });
        let blue = t.items.values().filter(|d| d.is_engram() && d.tier == Tier::Rare).map(|d| d.hash).min().unwrap();
        assert_eq!(on_pickup(&mut s, CHARACTER, blue).unwrap(), Pickup::Keep);
    }
}
