// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Inventory: equip, unequip, transfer and the vault, dismantle with value back, infusion, and the
//! Postmaster's deliveries.
//!
//! | Service | Message | Wire |
//! |---|---|---|
//! | equip | 403 `inventory_equip_item` (`0x80801942`: u64 instance) | decoded, applied since run sense84 |
//! | unequip | 404 `inventory_unequip_item` (`0x8080189a`: u64 instance) | schema known, never sent by a run |
//! | transfer | 405 `inventory_transfer_item` (`0x808018ce`: u8, u8, u64, i16, i32) | schema known; which u8 is which is open |
//! | vault | 903 / 902 to the Vault vendor (`vendors::sell`) | schema known; the Vault's vendor row is open |
//! | dismantle | 402 `inventory_perform_item_action` (`0x80801767`: {bool, u64, i16} + i32) | decoded, applied since sense125 |
//! | infusion | not identified: the "Infuse" talent node (801–803 are the talent-grid services) | open |
//!
//! Rules are the D1 API's (`EquipFailureReason`, `TransferStatuses`) and the manifest's item flags.

use crate::store::{self, CharacterId, Item, Owner, PlayerStore};
use crate::tables::{ItemAction, Tables, TIER_EXOTIC};

/// `EquipFailureReason` / `TransferStatuses`, as refusals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    ItemUnequippable(String),
    ItemUniqueEquipRestricted(String),
    ItemNotOnCharacter(String),
    ItemIsEquipped(String),
    NotTransferrable(String),
    NoRoomInDestination(String),
    /// The served objects can't express it (yet), or the item can't take this action.
    Unsupported(String),
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

fn on_character<S: PlayerStore + ?Sized>(s: &S, chr: CharacterId, instance: u64) -> Result<(u8, usize, Item), Refused> {
    match store::find_instance(s, chr, instance) {
        Some((Owner::Character(c), b, i, it)) if c == chr => Ok((b, i, it)),
        Some(_) => Err(Refused::ItemNotOnCharacter(format!("{instance:#x} is in the vault"))),
        None => Err(Refused::ItemNotOnCharacter(format!("{instance:#x} is not held"))),
    }
}

fn is_weapon(b: u8) -> bool {
    (0x02..=0x04).contains(&b)
}
fn is_armor(b: u8) -> bool {
    (0x07..=0x0b).contains(&b)
}

/// 403: equip. One exotic weapon and one exotic armour piece at a time; class-locked gear for its
/// class only; the bucket's equipped index from the geared fixture (`vendor::equipped_index`).
pub fn equip<S: PlayerStore + Clone>(s: &mut S, t: &Tables, chr: CharacterId, instance: u64) -> Result<String, Refused> {
    let (b, slot, it) = on_character(s, chr, instance)?;
    let index = crate::vendor::equipped_index(b).ok_or_else(|| Refused::Unsupported(format!("bucket {b:#x} has no known equipped index")))?;
    let class = s.characters().into_iter().find(|c| c.id == chr).map(|c| c.class).unwrap_or(3);
    if let Some(r) = t.items.get(&it.content) {
        if !r.equippable() {
            return Err(Refused::ItemUnequippable(format!("{} is not equippable", it.content)));
        }
        if r.class != 3 && r.class != class {
            return Err(Refused::ItemUnequippable(format!("{} is for class {}", it.content, r.class)));
        }
        if r.tier == TIER_EXOTIC && (is_weapon(b) || is_armor(b)) {
            let eq = s.equipped(chr);
            for other in eq.iter().copied().filter(|x| *x != 0) {
                let Some((Owner::Character(_), ob, _, oi)) = store::find_instance(s, chr, other) else { continue };
                let same_kind = (is_weapon(ob) && is_weapon(b)) || (is_armor(ob) && is_armor(b));
                if same_kind && ob != b && t.items.get(&oi.content).map(|r| r.tier) == Some(TIER_EXOTIC) {
                    return Err(Refused::ItemUniqueEquipRestricted(format!("{other:#x} is the equipped exotic")));
                }
            }
        }
    }
    let mut work = s.clone();
    let old = work.equipped(chr)[index];
    work.set_equipped(chr, index, instance).map_err(|e| Refused::Unsupported(e.to_string()))?;
    *s = work;
    Ok(format!("{instance:#x} (bucket {b:#x}, slot {slot}) equipped at index {index}, replacing {old:#x}"))
}

/// 404: unequip. The weapon, armour, subclass and Ghost buckets always hold an equipped item, so
/// unequipping one of those equips the next item of its bucket (what the game did when the
/// equipped item left); with none left it is refused (ItemUnequippable). Elsewhere the entry is
/// cleared. ⚠ LIKELY: no run has sent a 404; which screen sends it is open.
pub fn unequip<S: PlayerStore + Clone>(s: &mut S, chr: CharacterId, instance: u64) -> Result<String, Refused> {
    let (b, _, _) = on_character(s, chr, instance)?;
    let eq = s.equipped(chr);
    let index = eq.iter().position(|x| *x == instance).ok_or_else(|| Refused::ItemUnequippable(format!("{instance:#x} is not equipped")))?;
    let must_hold = is_weapon(b) || is_armor(b) || b == 0x06 || b == 0x0e;
    let next = s.slots(Owner::Character(chr), b).into_iter().flatten().find(|i| i.instance != 0 && i.instance != instance).map(|i| i.instance);
    let mut work = s.clone();
    let what = match (must_hold, next) {
        (true, None) => return Err(Refused::ItemUnequippable(format!("bucket {b:#x} must keep an equipped item"))),
        (_, Some(n)) if must_hold => {
            work.set_equipped(chr, index, n).map_err(|e| Refused::Unsupported(e.to_string()))?;
            format!("{instance:#x} unequipped; {n:#x} takes index {index}")
        }
        _ => {
            work.set_equipped(chr, index, 0).map_err(|e| Refused::Unsupported(e.to_string()))?;
            format!("{instance:#x} unequipped (index {index} cleared)")
        }
    };
    *s = work;
    Ok(what)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    ToVault,
    FromVault,
}

/// 405 (and the Vault vendor's 903/902): move an item between a character and the vault. An item
/// for another character goes through the vault, as in D1.
pub fn transfer<S: PlayerStore + Clone>(s: &mut S, t: &Tables, chr: CharacterId, dir: Direction, instance: u64, content: u16, quantity: u32) -> Result<String, Refused> {
    let mut work = s.clone();
    let (from, to) = match dir {
        Direction::ToVault => {
            let hit = if instance != 0 {
                let (b, i, it) = on_character(&work, chr, instance)?;
                (Owner::Character(chr), b, i, it)
            } else {
                store::buckets(0).find_map(|bk| store::find_content(&work, Owner::Character(chr), bk.ordinal, content).map(|(i, it)| (Owner::Character(chr), bk.ordinal, i, it)))
                    .ok_or_else(|| Refused::ItemNotOnCharacter(format!("{content} is not held")))?
            };
            if hit.3.instance != 0 && work.equipped(chr).contains(&hit.3.instance) {
                return Err(Refused::ItemIsEquipped(format!("{:#x}", hit.3.instance)));
            }
            let vb = crate::vendors::sell::vault_bucket(hit.1).ok_or_else(|| Refused::NotTransferrable(format!("bucket {:#x} has no vault bucket", hit.1)))?;
            (hit, (Owner::Account, vb))
        }
        Direction::FromVault => {
            let hit = [store::VAULT_WEAPONS, store::VAULT_ARMOR, store::VAULT_GENERAL].iter().find_map(|&vb| {
                work.slots(Owner::Account, vb).into_iter().enumerate().find_map(|(i, x)| {
                    x.filter(|it| if instance != 0 { it.instance == instance } else { it.content == content }).map(|it| (Owner::Account, vb, i, it))
                })
            }).ok_or_else(|| Refused::ItemNotOnCharacter(format!("{instance:#x} / {content} is not in the vault")))?;
            let home = t.items.get(&hit.3.content).map(|r| r.bucket).ok_or_else(|| Refused::Unsupported(format!("{} has no bucket", hit.3.content)))?;
            (hit, (Owner::Character(chr), home))
        }
    };
    if t.items.get(&from.3.content).map(|r| !r.transferrable()).unwrap_or(false) {
        return Err(Refused::NotTransferrable(format!("{}", from.3.content)));
    }
    let it = from.3.clone();
    let q = if it.instance != 0 { 1 } else if quantity == 0 { it.quantity } else { quantity.min(it.quantity) };
    let moving = if it.instance != 0 { it.clone() } else { Item::stack(it.content, q) };
    let left = if it.instance != 0 || q == it.quantity { None } else { Some(Item { quantity: it.quantity - q, ..it.clone() }) };
    work.set_slot(from.0, from.1, from.2, left).map_err(|e| Refused::Unsupported(e.to_string()))?;
    let max = t.items.get(&it.content).map(|r| r.max_stack).unwrap_or(0);
    let slot = store::give(&mut work, to.0, to.1, moving, max).map_err(Refused::NoRoomInDestination)?;
    *s = work;
    Ok(format!("{} x{q} ({:#x}) {:?} bucket {:#x} slot {} -> {:?} bucket {:#x} slot {slot}", it.content, it.instance, from.0, from.1, from.2, to.0, to.1))
}

/// What a dismantle gives back. ⚠ PLACEHOLDER, NOT SOURCED: the amounts were the live servers' and
/// are in no data (the client's action block names a reward set per item, `data/social/items.tsv`
/// `reward_set`, and the set's rewards are empty in the client). The sets below are the ones the
/// dismantle note found by tier and bucket; the amounts are stand-ins so the flow can be exercised,
/// used only when `D1_DISMANTLE_PLACEHOLDER=1`. A sourced `D1_DISMANTLE_YIELDS` row always wins.
/// `(reward set, [(content, quantity, bucket)])`: Glimmer 869 (0x1f), Weapon Parts 1604 and Exotic
/// Shards 879 (Materials 0x05), Legendary Marks 4548 (0x23).
pub const PLACEHOLDER_YIELDS: &[(i32, &[(u16, u32, u8)])] = &[
    (21, &[(869, 25, 0x1f)]),                    // tier-1 armour
    (22, &[(869, 25, 0x1f)]),                    // tier-1 weapons
    (729, &[(869, 50, 0x1f)]),                   // rare armour
    (738, &[(1604, 1, 0x05)]),                   // rare weapons
    (730, &[(4548, 2, 0x23)]),                   // legendary armour
    (739, &[(4548, 2, 0x23), (1604, 1, 0x05)]),  // legendary weapons
    (728, &[(879, 1, 0x05)]),                    // exotic armour
    (737, &[(879, 1, 0x05)]),                    // exotic weapons
    (1117, &[(869, 25, 0x1f)]),                  // the Khvostov
];

pub fn placeholder_yields(reward_set: i32) -> &'static [(u16, u32, u8)] {
    PLACEHOLDER_YIELDS.iter().find(|r| r.0 == reward_set).map(|r| r.1).unwrap_or(&[])
}

/// 402 dismantle: the item leaves (an equipped item cannot be dismantled; an item without a "shard"
/// action block cannot either) and its yields are credited.
pub fn dismantle<S: PlayerStore + Clone>(s: &mut S, t: &Tables, chr: CharacterId, instance: u64, content: u16, quantity: u32, yields: &[(u16, u32, u8)]) -> Result<String, Refused> {
    if let Some(r) = t.items.get(&content) {
        if r.action != ItemAction::Shard {
            return Err(Refused::Unsupported(format!("{content} has no dismantle action ({:?})", r.action)));
        }
    }
    let mut work = s.clone();
    if instance != 0 {
        let (b, slot, it) = on_character(&work, chr, instance)?;
        if it.content != content {
            return Err(Refused::Unsupported(format!("{instance:#x} holds {}, the request names {content}", it.content)));
        }
        if work.equipped(chr).contains(&instance) {
            return Err(Refused::ItemIsEquipped(format!("{instance:#x}")));
        }
        work.set_slot(Owner::Character(chr), b, slot, None).map_err(|e| Refused::Unsupported(e.to_string()))?;
    } else {
        let b = t.items.get(&content).map(|r| r.bucket).ok_or_else(|| Refused::Unsupported(format!("{content} has no bucket")))?;
        store::take(&mut work, Owner::Character(chr), b, content, quantity.max(1)).map_err(Refused::ItemNotOnCharacter)?;
    }
    for &(ci, q, b) in yields {
        store::give(&mut work, store::owner_of(b, chr), b, Item::stack(ci, q), 0).map_err(Refused::NoRoomInDestination)?;
    }
    *s = work;
    Ok(format!("{instance:#x} ({content}) dismantled; yields {yields:?}"))
}

/// The Light an infusion gives the target. Rise of Iron's rule (since the April 2016 update): an item
/// infused with one of the same type gets all of the source's Light; otherwise 80% of the
/// difference (The Taken King), rounded down. Never lowers the target.
/// ⚠ LEGACY STAND-IN: the owned 56578 tier rates/floors are now exported in items/infusion.tsv.
/// This mutation has not been converted to native instance coordinates; rewards::light::infuse
/// implements the verified formula separately. Do not treat this 80% branch as verified content.
pub fn infused_light(target: u32, source: u32, same_type: bool) -> u32 {
    if source <= target {
        return target;
    }
    if same_type {
        source
    } else {
        target + (source - target) * 4 / 5
    }
}

/// Infuse `source` into `target`: the source is consumed, the target takes the new Light.
/// Both must be on the character, in the same bucket, with the target legendary or exotic.
/// ⚠ Costs (Glimmer and materials) are not in the manifest (the talent grid's infusion step has its
/// material requirements stripped) and the wire is not identified, so no cost is taken.
pub fn infuse<S: PlayerStore + Clone>(s: &mut S, t: &Tables, chr: CharacterId, target: u64, source: u64) -> Result<String, Refused> {
    let (tb, ts, ti) = on_character(s, chr, target)?;
    let (sb, ss, si) = on_character(s, chr, source)?;
    if tb != sb {
        return Err(Refused::Unsupported(format!("infusion needs the same slot (bucket {tb:#x} vs {sb:#x})")));
    }
    if s.equipped(chr).contains(&source) {
        return Err(Refused::ItemIsEquipped(format!("{source:#x}")));
    }
    let trow = t.items.get(&ti.content).copied();
    if trow.map(|r| r.tier < crate::tables::TIER_LEGENDARY).unwrap_or(true) {
        return Err(Refused::Unsupported(format!("{} is not legendary or exotic", ti.content)));
    }
    let same_type = t.items.get(&si.content).map(|r| r.item_type) == trow.map(|r| r.item_type);
    let light = infused_light(ti.light, si.light, same_type);
    let mut work = s.clone();
    work.set_light(Owner::Character(chr), tb, ts, light).map_err(|e| Refused::Unsupported(e.to_string()))?;
    work.set_slot(Owner::Character(chr), sb, ss, None).map_err(|e| Refused::Unsupported(e.to_string()))?;
    *s = work;
    Ok(format!("{source:#x} infused into {target:#x}: Light {} -> {light}", ti.light))
}

/// The Postmaster: an item that has no room (a reward, a package) goes to Lost Items.
pub fn deliver<S: PlayerStore + ?Sized>(s: &mut S, chr: CharacterId, item: Item) -> Result<usize, Refused> {
    store::give(s, Owner::Character(chr), store::LOST_ITEMS, item, 0).map_err(Refused::NoRoomInDestination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::{memory, wire, CHR};
    use crate::store::{count, give};

    fn t() -> &'static Tables {
        crate::tables::tables()
    }

    fn exotic_of(bucket: u8) -> u16 {
        let mut v: Vec<u16> = t().items.iter().filter(|(_, r)| r.bucket == bucket && r.tier == TIER_EXOTIC && r.equippable() && r.instanced() && r.class == 3).map(|(c, _)| *c).collect();
        v.sort();
        v[0]
    }

    #[test]
    fn equipping_swaps_and_refuses_a_second_exotic_weapon() {
        let mut s = memory();
        give(&mut s, Owner::Character(CHR), 0x02, Item::instanced(exotic_of(0x02), 0x10, 300), 1).unwrap();
        give(&mut s, Owner::Character(CHR), 0x03, Item::instanced(exotic_of(0x03), 0x11, 300), 1).unwrap();
        equip(&mut s, t(), CHR, 0x10).unwrap();
        assert_eq!(s.equipped(CHR)[6], 0x10);
        assert!(matches!(equip(&mut s, t(), CHR, 0x11), Err(Refused::ItemUniqueEquipRestricted(_))));
        assert!(matches!(equip(&mut s, t(), CHR, 0x99), Err(Refused::ItemNotOnCharacter(_))));
    }

    #[test]
    fn class_gear_is_for_its_class() {
        let mut s = memory();
        let hunter_helm = *t().items.iter().filter(|(_, r)| r.bucket == 0x07 && r.class == 1 && r.equippable()).map(|(c, _)| c).min().unwrap();
        give(&mut s, Owner::Character(CHR), 0x07, Item::instanced(hunter_helm, 0x20, 0), 1).unwrap();
        assert!(matches!(equip(&mut s, t(), CHR, 0x20), Err(Refused::ItemUnequippable(_))));
    }

    #[test]
    fn unequipping_a_weapon_equips_the_next_one() {
        let mut s = memory();
        give(&mut s, Owner::Character(CHR), 0x02, Item::instanced(3598, 0x401, 0), 1).unwrap();
        equip(&mut s, t(), CHR, 0x401).unwrap();
        assert!(matches!(unequip(&mut s, CHR, 0x401), Err(Refused::ItemUnequippable(_))), "the only rifle stays");
        give(&mut s, Owner::Character(CHR), 0x02, Item::instanced(3598, 0x30b, 0), 1).unwrap();
        unequip(&mut s, CHR, 0x401).unwrap();
        assert_eq!(s.equipped(CHR)[6], 0x30b);
    }

    #[test]
    fn a_transfer_to_the_vault_and_back_on_the_served_profile() {
        let mut s = wire();
        give(&mut s, Owner::Character(CHR), 0x02, Item::instanced(3598, 0x30b, 0), 1).unwrap();
        transfer(&mut s, t(), CHR, Direction::ToVault, 0x30b, 3598, 0).unwrap();
        // The vault's weapons are account slots 20.. (bucket 0x16): the entry and its instance.
        assert_eq!(s.account[&20], (3598, 0), "an instance's entry, quantity 0");
        assert_eq!(s.account_instances[&20], 0x30b);
        transfer(&mut s, t(), CHR, Direction::FromVault, 0x30b, 3598, 0).unwrap();
        assert!(s.account.get(&20).is_none());
        assert_eq!(store::find_instance(&s, CHR, 0x30b).map(|x| x.1), Some(0x02));
    }

    #[test]
    fn an_equipped_item_does_not_transfer() {
        let mut s = memory();
        give(&mut s, Owner::Character(CHR), 0x02, Item::instanced(3598, 0x401, 0), 1).unwrap();
        equip(&mut s, t(), CHR, 0x401).unwrap();
        assert!(matches!(transfer(&mut s, t(), CHR, Direction::ToVault, 0x401, 3598, 0), Err(Refused::ItemIsEquipped(_))));
    }

    #[test]
    fn a_dismantle_gives_the_placeholder_yields_of_its_reward_set() {
        let mut s = memory();
        give(&mut s, Owner::Character(CHR), 0x02, Item::instanced(3598, 0x30b, 0), 1).unwrap();
        let set = t().items[&3598].reward_set;
        assert_eq!(set, 1117, "the dismantle note's Khvostov set");
        dismantle(&mut s, t(), CHR, 0x30b, 3598, 0, placeholder_yields(set)).unwrap();
        assert_eq!(count(&s, Owner::Account, 0x1f, 869), 25);
        assert!(store::find_instance(&s, CHR, 0x30b).is_none());
        // Every placeholder row names a reward set the client data has.
        for (set, _) in PLACEHOLDER_YIELDS {
            assert!(t().dismantle_sets.contains_key(set), "reward set {set}");
        }
    }

    #[test]
    fn infusion_follows_the_rise_of_iron_rule() {
        assert_eq!(infused_light(350, 360, true), 360);
        assert_eq!(infused_light(350, 360, false), 358);
        assert_eq!(infused_light(360, 350, true), 360, "never lowers");
        let mut s = memory();
        let leg = *t().items.iter().filter(|(_, r)| r.bucket == 0x02 && r.tier == crate::tables::TIER_LEGENDARY && r.equippable() && r.instanced()).map(|(c, _)| c).min().unwrap();
        give(&mut s, Owner::Character(CHR), 0x02, Item::instanced(leg, 0x50, 350), 1).unwrap();
        give(&mut s, Owner::Character(CHR), 0x02, Item::instanced(leg, 0x51, 365), 1).unwrap();
        infuse(&mut s, t(), CHR, 0x50, 0x51).unwrap();
        assert_eq!(store::find_instance(&s, CHR, 0x50).unwrap().3.light, 365);
        assert!(store::find_instance(&s, CHR, 0x51).is_none(), "the source is consumed");
        assert!(matches!(infuse(&mut wire(), t(), CHR, 1, 2), Err(_)));
    }
}
