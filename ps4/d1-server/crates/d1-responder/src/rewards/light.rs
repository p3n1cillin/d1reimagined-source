// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Light (2.x rules): an item's Light is its Attack or Defense, the character's Light comes from
//! the equipped items' Light, drops roll relative to the player's Light under the source's and the
//! engram tier's caps, and infusion raises an item to its fuel's Light.
//!
//! The client works out the Light it shows itself (`0x265EF0` sums the Light stat over the 20
//! equipped items, ps4/design/items-and-gear.md section 10). The server needs a character Light
//! only as the basis for drops, which is what [`highest_possible_light`] gives.

use d1_loadout::model::{Slot, Tier};

use super::placeholders as ph;
use super::store::{CharacterId, RewardStore};
use super::tables::{tables, ItemDef};
use super::Rng;

/// `light.cap`: the Light cap from 2.4.1 to the end of Destiny 1.
pub const LIGHT_CAP: u32 = 400;

/// The highest Light an engram of this tier decrypts to (`light.decode_cap.*`), with its rule.
pub fn decode_cap(tier: Tier) -> Option<(u32, &'static str)> {
    match tier {
        Tier::Rare => Some((365, "light.decode_cap.rare")),
        Tier::Legendary => Some((390, "light.decode_cap.legendary")),
        Tier::Exotic => Some((400, "light.decode_cap.exotic")),
        _ => None,
    }
}

/// `light.weights`: each slot's weight in the character's Light, out of [`WEIGHT_TOTAL`].
pub fn light_weight(slot: Slot) -> Option<u32> {
    match slot {
        Slot::Primary | Slot::Special | Slot::Heavy => Some(6),
        Slot::Head | Slot::Arms | Slot::Chest | Slot::Legs => Some(5),
        Slot::Ghost | Slot::Artifact | Slot::ClassItem => Some(4),
        _ => None,
    }
}

pub const WEIGHT_TOTAL: u32 = 50;

/// The character's Light from its equipped `(slot, Light)` pairs: the weighted mean, floored. An
/// empty slot counts as 0.
pub fn character_light(equipped: &[(Slot, u32)]) -> u32 {
    let sum: u64 = equipped.iter().filter_map(|(s, l)| light_weight(*s).map(|w| u64::from(w) * u64::from(*l))).sum();
    (sum / u64::from(WEIGHT_TOTAL)) as u32
}

/// One piece of gear a character could equip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GearPiece {
    pub slot: Slot,
    pub light: u32,
    pub exotic: bool,
}

/// `light.basis`: the highest character Light the pieces allow, best per slot with at most one
/// Exotic weapon and one Exotic armour piece equipped.
pub fn highest_possible_light(pieces: &[GearPiece]) -> u32 {
    let weapons = [Slot::Primary, Slot::Special, Slot::Heavy];
    let armour = [Slot::Head, Slot::Arms, Slot::Chest, Slot::Legs];
    let free = [Slot::Ghost, Slot::Artifact, Slot::ClassItem];
    let best = |slot: Slot, exotic: Option<bool>| -> Option<u32> {
        pieces.iter().filter(|p| p.slot == slot && exotic.map_or(true, |e| p.exotic == e)).map(|p| p.light).max()
    };
    // For a group with the one-Exotic rule: all non-Exotic, or one slot's Exotic with the rest non-Exotic.
    let group = |slots: &[Slot]| -> Vec<(Slot, u32)> {
        let plain: Vec<(Slot, u32)> = slots.iter().map(|s| (*s, best(*s, Some(false)).unwrap_or(0))).collect();
        let mut best_set = plain.clone();
        let score = |set: &[(Slot, u32)]| set.iter().map(|(s, l)| light_weight(*s).unwrap_or(0) * l).sum::<u32>();
        for (i, s) in slots.iter().enumerate() {
            if let Some(e) = best(*s, Some(true)) {
                let mut set = plain.clone();
                set[i].1 = set[i].1.max(e);
                if score(&set) > score(&best_set) {
                    best_set = set;
                }
            }
        }
        best_set
    };
    let mut set = group(&weapons);
    set.extend(group(&armour));
    set.extend(free.iter().map(|s| (*s, best(*s, None).unwrap_or(0))));
    character_light(&set)
}

/// The character's basis for drops, from everything it holds.
pub fn basis_for(store: &dyn RewardStore, character: CharacterId) -> u32 {
    let t = tables();
    let pieces: Vec<GearPiece> = store
        .items_of(character)
        .into_iter()
        .filter_map(|id| store.item(id))
        .filter_map(|inst| {
            let d = t.items.get(&inst.item_hash)?;
            let slot = Slot::from_bucket_hash(d.bucket)?;
            light_weight(slot)?;
            Some(GearPiece { slot, light: inst.primary, exotic: d.tier == Tier::Exotic })
        })
        .collect();
    highest_possible_light(&pieces)
}

/// A rolled Light and the rules that decided it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightRoll {
    pub value: u32,
    pub rules: Vec<&'static str>,
}

/// Where a drop comes from: its tier and the source's own Light floor and cap (an activity's
/// "rewards up to N"). `source_cap` 0 means no source cap beyond [`LIGHT_CAP`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DropSource {
    pub tier: Tier,
    pub floor: u32,
    pub source_cap: u32,
}

/// The Light of a new drop of `item` for a player whose basis is `basis`.
///
/// Bounds, whatever the roll: never above [`LIGHT_CAP`], the source cap, the tier's decrypt cap or
/// the item's own maximum; never below the item's own minimum; and for Rare and Legendary drops
/// never below `min(basis, cap)` (`light.at_or_above`).
pub fn roll_drop_light(rng: &mut Rng, basis: u32, source: DropSource, item: &ItemDef) -> LightRoll {
    let mut rules = vec!["light.cap", "light.item_bounds"];
    let mut cap = LIGHT_CAP;
    if source.source_cap > 0 {
        cap = cap.min(source.source_cap);
    }
    if let Some((c, r)) = decode_cap(source.tier) {
        cap = cap.min(c);
        rules.push(r);
    }
    if item.primary_max > 0 {
        cap = cap.min(item.primary_max);
    }
    let lo_item = item.primary_min;
    if cap <= lo_item {
        return LightRoll { value: lo_item, rules };
    }
    let value = match source.tier {
        Tier::Rare | Tier::Legendary | Tier::Exotic => {
            if source.tier != Tier::Exotic {
                rules.push("light.at_or_above");
            }
            rules.push("light.spread");
            let base = basis.min(cap).max(lo_item).max(source.floor.min(cap));
            if base >= cap {
                cap
            } else {
                let up = (base + ph::SPREAD_ABOVE_BASIS).min(cap);
                if base + ph::SLOW_ZONE >= cap {
                    rules.push("light.slow_zone");
                    if rng.chance(ph::SLOW_ZONE_UP_PER_MILLE) { rng.range(base + 1, up) } else { base }
                } else {
                    rng.range(base, up)
                }
            }
        }
        _ => {
            rules.push("light.common_roll");
            rng.range(lo_item.max(source.floor).min(cap), cap)
        }
    };
    LightRoll { value, rules }
}

/// `light.infusion`: the target's Light after consuming `fuel`. Never lowers the target, never
/// exceeds the target's own maximum (`target_max` 0: [`LIGHT_CAP`]).
pub fn infuse(target: u32, target_max: u32, fuel: u32) -> u32 {
    let max = if target_max == 0 { LIGHT_CAP } else { target_max.min(LIGHT_CAP) };
    target.max(fuel.min(max))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(tier: Tier, min: u32, max: u32) -> ItemDef {
        ItemDef {
            hash: 1,
            tier,
            bucket: Slot::Primary.bucket_hash(),
            item_type: 3,
            class_type: 3,
            instanced: true,
            equippable: true,
            talent_grid: 0,
            max_stack: 1,
            primary_stat: 368_428_387,
            primary_min: min,
            primary_max: max,
            rolled_stats: Vec::new(),
        }
    }

    #[test]
    fn light_roll_stays_within_its_bounds() {
        for tier in [Tier::Common, Tier::Uncommon, Tier::Rare, Tier::Legendary, Tier::Exotic] {
            for (min, max) in [(0, 400), (280, 400), (3, 400), (160, 335), (0, 0)] {
                let it = item(tier, min, max);
                for source_cap in [0, 340, 365, 385, 400] {
                    for basis in (0..=420).step_by(7) {
                        for seed in 0..12 {
                            let mut rng = Rng::new(seed * 1_000 + u64::from(basis));
                            let src = DropSource { tier, floor: 0, source_cap };
                            let r = roll_drop_light(&mut rng, basis, src, &it);
                            let mut cap = LIGHT_CAP;
                            if source_cap > 0 { cap = cap.min(source_cap); }
                            if let Some((c, _)) = decode_cap(tier) { cap = cap.min(c); }
                            if max > 0 { cap = cap.min(max); }
                            let cap = cap.max(min);
                            assert!(r.value <= cap, "{tier:?} {min}-{max} cap {source_cap} basis {basis}: {} > {cap}", r.value);
                            assert!(r.value >= min, "{tier:?}: {} < item min {min}", r.value);
                            if matches!(tier, Tier::Rare | Tier::Legendary) {
                                assert!(r.value >= basis.min(cap), "{tier:?} basis {basis}: {} below basis", r.value);
                                assert!(r.value <= basis.max(min).min(cap) + ph::SPREAD_ABOVE_BASIS);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn decrypt_caps_follow_the_patch_notes() {
        assert_eq!(decode_cap(Tier::Rare).unwrap().0, 365);
        assert_eq!(decode_cap(Tier::Exotic).unwrap().0, LIGHT_CAP);
        assert!(decode_cap(Tier::Legendary).unwrap().0 < LIGHT_CAP);
        assert!(decode_cap(Tier::Common).is_none());
    }

    #[test]
    fn the_cap_is_the_manifests_year_three_maximum() {
        let max = tables().items.values().filter(|d| d.tier == Tier::Legendary).map(|d| d.primary_max).max();
        assert_eq!(max, Some(LIGHT_CAP));
    }

    #[test]
    fn character_light_is_the_weighted_mean() {
        let all = |l: u32| -> Vec<(Slot, u32)> {
            Slot::ALL.iter().filter(|s| light_weight(**s).is_some()).map(|s| (*s, l)).collect()
        };
        assert_eq!(Slot::ALL.iter().filter_map(|s| light_weight(*s)).sum::<u32>(), WEIGHT_TOTAL);
        assert_eq!(character_light(&all(400)), 400);
        assert_eq!(character_light(&all(385)), 385);
        let mut mixed = all(390);
        mixed[0].1 = 400; // a primary 10 higher adds 10 * 6 / 50 = 1.2
        assert_eq!(character_light(&mixed), 391);
    }

    #[test]
    fn one_exotic_weapon_and_one_exotic_armour() {
        let p = |slot, light, exotic| GearPiece { slot, light, exotic };
        let mut pieces = vec![
            p(Slot::Primary, 380, false), p(Slot::Special, 380, false), p(Slot::Heavy, 380, false),
            p(Slot::Primary, 400, true), p(Slot::Heavy, 400, true),
        ];
        for s in [Slot::Head, Slot::Arms, Slot::Chest, Slot::Legs, Slot::Ghost, Slot::Artifact, Slot::ClassItem] {
            pieces.push(p(s, 380, false));
        }
        // Only one exotic weapon counts: 380 everywhere plus one weapon at 400 = 380 + 20*6/50.
        assert_eq!(highest_possible_light(&pieces), 382);
    }

    #[test]
    fn infusion_never_lowers_and_respects_the_item_maximum() {
        assert_eq!(infuse(300, 400, 385), 385);
        assert_eq!(infuse(390, 400, 385), 390);
        assert_eq!(infuse(300, 335, 400), 335);
        assert_eq!(infuse(3, 0, 420), LIGHT_CAP);
    }
}
