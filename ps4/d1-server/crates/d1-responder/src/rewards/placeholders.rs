// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! PLACEHOLDERS. Every value here stands in for a rule Bungie's servers held and nobody has
//! published: drop chances, decrypt upgrades, dismantle amounts, the Light spread, costs. Each one
//! NEEDS EVIDENCE (a live capture, a recording with counts, or the original server rules) before it
//! can be called 1:1. They are kept in this one file so that replacing them touches nothing else,
//! and each names its rule in [`super::RULES`].

use d1_loadout::model::Tier;

/// `light.spread`: how far above the basis a Rare or Legendary drop may land.
pub const SPREAD_ABOVE_BASIS: u32 = 4;

/// `light.slow_zone`: the width of the band below a cap where drops climb more slowly, and the
/// chance (per mille) that a drop in it goes above the basis. Patch 2.4.0 described this for
/// Legendary engrams between 350 and 365 ("about half the time"); applying it below every cap is
/// the placeholder.
pub const SLOW_ZONE: u32 = 15;
pub const SLOW_ZONE_UP_PER_MILLE: u32 = 500;

/// `engram.upgrade`: chance (per mille) that an engram decrypts one tier higher.
pub fn engram_upgrade_per_mille(_tier: Tier) -> u32 {
    0
}

/// `engram.uncommon_convert.amount`: materials per converted Uncommon engram.
pub const UNCOMMON_CONVERT_AMOUNT: u32 = 1;

/// `loot.drop_chance`: per-mille chance of an engram of each tier at the end of a completed
/// activity, rolled independently.
pub const ACTIVITY_ENGRAM_PER_MILLE: &[(Tier, u32)] = &[(Tier::Rare, 400), (Tier::Legendary, 100), (Tier::Exotic, 10)];

/// `currency.marks_weekly_cap`: Legendary Marks a week from activities.
pub const MARKS_WEEKLY_CAP: u32 = 100;

/// `currency.dismantle.marks`: Legendary Marks for dismantling a Year 2/3 Legendary weapon, armour.
/// **Live, not a placeholder** any more: 3 for both, in the 5 November Tower capture's legendary
/// dismantles (`notes/d1-live-captures-patrol-tower-2026-11-06.md`, "Dismantle yields"); the same
/// amounts are `inventory_ops::PLACEHOLDER_YIELDS`' LIVE legendary rows.
pub const DISMANTLE_MARKS_WEAPON: u32 = 3;
pub const DISMANTLE_MARKS_ARMOR: u32 = 3;

/// `grid.xp_share`: per mille of the character's XP each equipped grid item gains.
pub const XP_SHARE_PER_MILLE: u32 = 1000;

/// `grid.node_cost`: what activating a weapon or armour talent node (801) costs, as `(item hash,
/// amount)`. ⚠ STAND-IN from ONE live 801 (the 5 November Tower capture, #328): **500 Glimmer and 10
/// Weapon Parts**, plus 10 of a material the capture does not name (not charged here). It replaced
/// "free", which that capture contradicts. Armour takes Armor Materials in place of Weapon Parts by
/// analogy. Subclass nodes cost nothing (D1: a subclass grid unlocks with XP alone). The real costs
/// are each node step's requirement list in the client's content (step `+0x98`, read by `0x285050`
/// and `0x28E9D0`, 16-byte entries checked by `0x28E590`); exporting it replaces this.
pub const NODE_GLIMMER_COST: u32 = 500;
pub const NODE_MATERIAL_COST: u32 = 10;

/// `records.book_points`: Record Book progression per completed record.
pub const BOOK_POINTS_PER_RECORD: u32 = 1000;

/// Every placeholder value, by rule id, for the tests and the note.
pub const ALL: &[(&str, &str)] = &[
    ("light.spread", "SPREAD_ABOVE_BASIS = 4"),
    ("light.slow_zone", "SLOW_ZONE = 15, SLOW_ZONE_UP_PER_MILLE = 500"),
    ("light.common_roll", "uniform between floor and cap"),
    ("light.infusion_cost", "no cost"),
    ("engram.upgrade", "0 per mille for every tier"),
    ("engram.pool.fallback", "every matching instanced item reaching the cap"),
    ("engram.uncommon_convert.amount", "UNCOMMON_CONVERT_AMOUNT = 1"),
    ("loot.drop_chance", "Rare 400, Legendary 100, Exotic 10 per mille"),
    ("currency.marks_weekly_cap", "MARKS_WEEKLY_CAP = 100"),
    ("grid.random_roll", "uniform over the node's steps"),
    ("grid.node_cost", "weapon/armour node: NODE_GLIMMER_COST = 500 Glimmer + NODE_MATERIAL_COST = 10 Weapon Parts / Armor Materials (one live 801); subclass free"),
    ("grid.xp_share", "XP_SHARE_PER_MILLE = 1000"),
    ("item.stat_roll", "uniform between the manifest minimum and maximum"),
    ("records.book_points", "BOOK_POINTS_PER_RECORD = 1000"),
];
