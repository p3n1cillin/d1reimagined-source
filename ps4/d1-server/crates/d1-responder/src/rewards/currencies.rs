// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Materials and currencies: Glimmer, Legendary Marks and their weekly cap, Silver Dust, Motes of
//! Light, Strange Coins, planetary and upgrade materials; and what a dismantle gives back.
//!
//! The wallet caps are each item's `maxStackSize` in the manifest. The hashes below are the
//! stackable items themselves (the ones whose bucket is a currency or Materials bucket), not the
//! same-named display items an activity's reward list shows ([`super::tables::Tables::reward_display`]).

use anyhow::{bail, Result};
use d1_loadout::model::{Slot, Tier};

use super::placeholders as ph;
use super::store::{CharacterId, Owner, Period, RewardStore};
use super::tables::{tables, ItemDef};

pub const GLIMMER: u32 = 3_159_615_086;
pub const LEGENDARY_MARKS: u32 = 2_534_352_370;
pub const SILVER_DUST: u32 = 110_672_197;
pub const STRANGE_COIN: u32 = 1_738_186_005;
pub const MOTE_OF_LIGHT: u32 = 937_555_249;
pub const WEAPON_PARTS: u32 = 1_898_539_128;
pub const ARMOR_MATERIALS: u32 = 1_542_293_174;
pub const EXOTIC_SHARD: u32 = 452_597_397;
pub const ASCENDANT_SHARD: u32 = 258_181_985;
pub const ASCENDANT_ENERGY: u32 = 1_893_498_008;
pub const VANGUARD_MARKS: u32 = 1_415_355_173;
pub const CRUCIBLE_MARKS: u32 = 1_415_355_184;
/// Planetary materials: Spirit Bloom, Helium Filaments, Relic Iron, Spinmetal, Wormspore.
pub const PLANETARY: [u32; 5] = [2_254_123_540, 1_797_491_610, 3_242_866_270, 2_882_093_969, 3_164_836_592];

/// Buckets of account-wide wallets: Legendary Marks, Glimmer, Silver (the premium currency).
const ACCOUNT_BUCKETS: [u32; 3] = [2_689_798_304, 2_689_798_308, 2_689_798_311];

/// Who holds `item` for `character`, and the rule that says so.
pub fn owner(store: &dyn RewardStore, character: CharacterId, item: u32) -> (Owner, &'static str) {
    let account = Owner::Account(store.account_of(character));
    if item == GLIMMER {
        return (account, "currency.scope.glimmer");
    }
    let bucket = tables().items.get(&item).map_or(0, |d| d.bucket);
    if item == SILVER_DUST || ACCOUNT_BUCKETS.contains(&bucket) {
        (account, "currency.scope")
    } else {
        (Owner::Character(character), "currency.scope")
    }
}

/// `currency.caps`: the most one wallet or stack holds.
pub fn cap(item: u32) -> u32 {
    tables().items.get(&item).map_or(u32::MAX, |d| d.max_stack.max(1))
}

/// Where a grant comes from. Only activity grants count toward the Legendary Mark weekly cap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Activity,
    Dismantle,
    RecordBook,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Granted {
    pub item: u32,
    pub requested: u32,
    pub granted: u32,
    /// Over a cap (`currency.overflow`).
    pub lost: u32,
    pub rules: Vec<&'static str>,
}

/// Add `amount` of `item`, applying the wallet cap and, for Legendary Marks from activities, the
/// weekly cap. Whatever does not fit is lost.
pub fn grant(store: &mut dyn RewardStore, character: CharacterId, item: u32, amount: u32, source: &Source) -> Granted {
    let (owner, scope_rule) = owner(store, character, item);
    let mut rules = vec![scope_rule, "currency.caps"];
    let mut allowed = amount;
    if item == LEGENDARY_MARKS && *source == Source::Activity {
        rules.push("currency.marks_weekly_cap");
        let earned = store.counter(owner, "marks.weekly", Period::Weekly);
        allowed = allowed.min(ph::MARKS_WEEKLY_CAP.saturating_sub(earned));
    }
    let have = store.quantity(owner, item);
    let granted = allowed.min(cap(item).saturating_sub(have));
    if granted > 0 {
        store.set_quantity(owner, item, have + granted);
        if item == LEGENDARY_MARKS && *source == Source::Activity {
            store.add_counter(owner, "marks.weekly", Period::Weekly, granted);
        }
    }
    if granted < amount {
        rules.push("currency.overflow");
    }
    Granted { item, requested: amount, granted, lost: amount - granted, rules }
}

/// Take `amount` of `item`; refuses (and changes nothing) when there is not enough.
pub fn spend(store: &mut dyn RewardStore, character: CharacterId, item: u32, amount: u32) -> Result<()> {
    let (owner, _) = owner(store, character, item);
    let have = store.quantity(owner, item);
    if have < amount {
        bail!("not enough of item {item}: have {have}, need {amount}");
    }
    store.set_quantity(owner, item, have - amount);
    Ok(())
}

/// The dismantle reward set of a tier and kind, when the item's own is not known: the sets
/// `inventory_ops::PLACEHOLDER_YIELDS` names (its row comments give each set's tier).
fn dismantle_set(tier: Tier, weapon: bool) -> Option<i32> {
    Some(match (tier, weapon) {
        (Tier::Uncommon, false) => 727,
        (Tier::Uncommon, true) => 736,
        (Tier::Rare, false) => 729,
        (Tier::Rare, true) => 738,
        (Tier::Legendary, false) => 730,
        (Tier::Legendary, true) => 739,
        (Tier::Exotic, false) => 728,
        (Tier::Exotic, true) => 737,
        _ => return None,
    })
}

/// What dismantling an item gives back: `(item, quantity, rule)`.
///
/// Silver Dust engram items are recognised by the community engram pages (the patch notes name the
/// engrams, not their items). Everything else comes from **one table**, the 402 path's
/// (`inventory_ops::PLACEHOLDER_YIELDS`, by the item's 56578 reward set, else its tier's): the live
/// servers' Glimmer, Weapon Parts or Armor Materials and Legendary Marks (PR #328's Tower capture).
/// Year 1 Legendaries give the row without its Marks (Marks for dismantling came with Year 2,
/// patch 2.0.0). Basic and exotic rows are still the table's labelled placeholders.
pub fn dismantle_yield(item: &ItemDef) -> Vec<(u32, u32, &'static str)> {
    let pools = &tables().engram_pools;
    let in_box = |slug: &str| pools.iter().any(|p| p.slug == slug && p.items.contains(&item.hash));
    if in_box("icy-engram") {
        return vec![(SILVER_DUST, 5, "currency.dismantle.silver")];
    }
    if ["dusty-iron-engram", "triad-engram", "arena-engram", "exodus-engram", "warpath-engram", "unity-engram"]
        .iter()
        .any(|s| in_box(s))
    {
        return vec![(SILVER_DUST, 1, "currency.dismantle.silver")];
    }
    let Some(slot) = Slot::from_bucket_hash(item.bucket).filter(|s| s.is_weapon() || s.is_armor()) else { return Vec::new() };
    let content = crate::content::item_index(item.hash);
    let set = content
        .and_then(|ci| crate::tables::tables().items.get(&ci).map(|r| r.reward_set))
        .filter(|s| !crate::inventory_ops::placeholder_yields(*s).is_empty())
        .or_else(|| dismantle_set(item.tier, slot.is_weapon()));
    let Some(set) = set else { return Vec::new() };
    // Year 2 and 3 Legendaries reach the Year 3 cap; Year 1 Legendaries stop lower.
    let marks_era = item.tier != Tier::Legendary || item.primary_max >= super::light::LIGHT_CAP;
    let marks = if slot.is_weapon() { ph::DISMANTLE_MARKS_WEAPON } else { ph::DISMANTLE_MARKS_ARMOR };
    crate::inventory_ops::placeholder_yields(set)
        .iter()
        .filter_map(|&(ci, n, _)| {
            let hash = crate::content::item_hash(ci)?;
            if hash == LEGENDARY_MARKS {
                return marks_era.then_some((LEGENDARY_MARKS, marks, "currency.dismantle.marks"));
            }
            Some((hash, n, "currency.dismantle.other"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewards::store::MemoryStore;

    const ACCOUNT: u64 = 0xD100_0000_0000_0001;
    const CHARACTER: u64 = 0xD100_0000_0000_0101;

    fn store() -> MemoryStore {
        let mut s = MemoryStore::with_character(ACCOUNT, CHARACTER, 0, 40);
        s.now = 1_477_990_800; // a Tuesday reset
        s
    }

    #[test]
    fn wallet_caps_come_from_the_manifest() {
        assert_eq!(cap(GLIMMER), 25_000);
        assert_eq!(cap(LEGENDARY_MARKS), 200);
        assert_eq!(cap(SILVER_DUST), 250);
        for p in PLANETARY {
            assert_eq!(cap(p), 200, "{p}");
        }
    }

    #[test]
    fn glimmer_over_the_cap_is_lost() {
        let mut s = store();
        let g = grant(&mut s, CHARACTER, GLIMMER, 24_000, &Source::Other);
        assert_eq!((g.granted, g.lost), (24_000, 0));
        let g = grant(&mut s, CHARACTER, GLIMMER, 2_500, &Source::Other);
        assert_eq!((g.granted, g.lost), (1_000, 1_500));
        assert_eq!(s.quantity(Owner::Account(ACCOUNT), GLIMMER), 25_000);
        assert!(g.rules.contains(&"currency.overflow"));
    }

    #[test]
    fn marks_respect_the_wallet_and_the_weekly_cap() {
        let mut s = store();
        let account = Owner::Account(ACCOUNT);
        let mut total = 0;
        for _ in 0..12 {
            total += grant(&mut s, CHARACTER, LEGENDARY_MARKS, 15, &Source::Activity).granted;
        }
        assert_eq!(total, ph::MARKS_WEEKLY_CAP, "weekly cap");
        assert_eq!(grant(&mut s, CHARACTER, LEGENDARY_MARKS, 15, &Source::Activity).granted, 0);
        // Dismantling is not capped weekly, only by the wallet.
        let d = grant(&mut s, CHARACTER, LEGENDARY_MARKS, 150, &Source::Dismantle);
        assert_eq!(d.granted, 200 - ph::MARKS_WEEKLY_CAP);
        assert_eq!(s.quantity(account, LEGENDARY_MARKS), 200);
        // A new week: spend some, then earn again.
        s.now += 7 * 86_400;
        spend(&mut s, CHARACTER, LEGENDARY_MARKS, 120).unwrap();
        assert_eq!(grant(&mut s, CHARACTER, LEGENDARY_MARKS, 15, &Source::Activity).granted, 15);
        assert!(spend(&mut s, CHARACTER, LEGENDARY_MARKS, 1_000).is_err());
        assert_eq!(s.quantity(account, LEGENDARY_MARKS), 95);
    }

    #[test]
    fn marks_are_account_wide_and_materials_per_character() {
        let s = store();
        assert_eq!(owner(&s, CHARACTER, LEGENDARY_MARKS).0, Owner::Account(ACCOUNT));
        assert_eq!(owner(&s, CHARACTER, PLANETARY[0]).0, Owner::Character(CHARACTER));
        assert_eq!(owner(&s, CHARACTER, MOTE_OF_LIGHT).0, Owner::Character(CHARACTER));
    }

    #[test]
    fn dismantle_yields_follow_their_rules() {
        let t = tables();
        let icy = t.engram_pools.iter().find(|p| p.slug == "icy-engram").unwrap();
        let d = &t.items[&icy.items[0]];
        assert_eq!(dismantle_yield(d), vec![(SILVER_DUST, 5, "currency.dismantle.silver")]);
        let dusty = t.engram_pools.iter().find(|p| p.slug == "dusty-iron-engram").unwrap();
        assert_eq!(dismantle_yield(&t.items[&dusty.items[0]])[0].1, 1);
        // PR #328's live rows, through the one table: rare and lower gear gives materials (and
        // uncommon Glimmer), not nothing.
        let weapon = |d: &&ItemDef| Slot::from_bucket_hash(d.bucket).is_some_and(|s| s.is_weapon());
        let armour = |d: &&ItemDef| Slot::from_bucket_hash(d.bucket).is_some_and(|s| s.is_armor());
        let pick = |tier: Tier, w: bool| {
            t.items.values().find(|d| d.tier == tier && d.instanced && d.equippable && if w { weapon(d) } else { armour(d) }).unwrap()
        };
        let y = dismantle_yield(pick(Tier::Rare, true));
        assert!(y.contains(&(WEAPON_PARTS, 1, "currency.dismantle.other")), "{y:?}");
        let y = dismantle_yield(pick(Tier::Uncommon, false));
        assert!(y.iter().any(|l| l.0 == GLIMMER) && y.iter().any(|l| l.0 == ARMOR_MATERIALS), "{y:?}");
        let y2 = t.items.values().find(|d| d.tier == Tier::Legendary && d.equippable && armour(d) && d.primary_max >= crate::rewards::light::LIGHT_CAP).unwrap();
        let y = dismantle_yield(y2);
        assert!(y.contains(&(LEGENDARY_MARKS, 3, "currency.dismantle.marks")), "3 Marks for armour too: {y:?}");
        assert!(y.iter().any(|l| l.0 == ARMOR_MATERIALS), "{y:?}");
    }
}
