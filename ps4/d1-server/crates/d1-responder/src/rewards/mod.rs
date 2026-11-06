// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Rewards and character growth (stream M): loot, engrams, Light, item instances, talent grids,
//! the Grimoire, Record Books and quests, and currencies.
//!
//! Every service works on a [`store::RewardStore`]; [`store::MemoryStore`] keeps it in memory until
//! stream C's persistent store implements the trait. Activities (streams J and A2) call one
//! function, [`grant_activity_rewards`]. Nothing here is wired into a live request path: the module
//! is off unless `D1_REWARDS=1` ([`enabled`]), and callers check that first.
//!
//! Every rule a service applies is listed in [`RULES`] with its source and status. Bungie's servers
//! decided drop rates, decrypt pools, dismantle amounts and Light weighting, and none of that is
//! public: those rules are [`Status::Placeholder`], kept in [`placeholders`], and say "needs
//! evidence". Rules from community collation are [`Status::Community`]. Only rules read from
//! Bungie's own data (the manifest tables under `tables/`), Bungie's patch notes or the client
//! are [`Status::Verified`]. `notes/ps4-56578-rewards-and-growth-2026-11-05.md` has the same table.

// The services are an interface for other streams; until they call it, most of it is unused here.
#![allow(dead_code)]

pub mod currencies;
pub mod engrams;
pub mod grimoire;
pub mod item_instance;
pub mod light;
pub mod loot;
pub mod placeholders;
pub mod player_store_adapter;
pub mod records;
pub mod remote;
pub mod store;
pub mod tables;
pub mod talent_grid;

#[allow(unused_imports)]
pub use loot::{grant_activity_rewards, ActivityOutcome, RewardGrant};

/// `DestinyInventoryItemDefinition.itemType` of an engram.
pub const ITEM_TYPE_ENGRAM: u32 = 8;

/// `D1_REWARDS=1` turns the services on. Default off.
pub fn enabled() -> bool {
    std::env::var("D1_REWARDS").is_ok_and(|v| v == "1")
}

/// `D1_REWARDS_SEED`: a fixed seed for every grant (tests, replays). Unset: seeded from the time.
pub fn seed_from_env(now: u64) -> u64 {
    std::env::var("D1_REWARDS_SEED")
        .ok()
        .and_then(|v| {
            let v = v.trim();
            v.strip_prefix("0x").map_or_else(|| v.parse().ok(), |h| u64::from_str_radix(h, 16).ok())
        })
        .unwrap_or(now ^ 0x9E37_79B9_7F4A_7C15)
}

/// The knobs this module reads, in the responder's `KNOBS` format.
pub const KNOBS: &[(&str, &str, &str)] = &[
    ("D1_REWARDS", "0", "1 = stream M's reward and growth services answer grant_activity_rewards (placeholders labelled; see notes/ps4-56578-rewards-and-growth-2026-11-05.md)"),
    ("D1_REWARDS_SEED", "<time>", "fixed seed for every reward roll (decimal or 0x hex), so a grant can be replayed"),
];

/// How a rule is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// From Bungie's own data or text: the manifest, the patch notes, or the client.
    Verified,
    /// From community collation (e.g. nruble/cryptarch-guide, DIM's D1 code). A stand-in.
    Community,
    /// Not public. An isolated, labelled stand-in that needs evidence.
    Placeholder,
}

/// One rule a service applies.
#[derive(Debug, Clone, Copy)]
pub struct Rule {
    pub id: &'static str,
    pub rule: &'static str,
    pub source: &'static str,
    pub status: Status,
}

const fn rule(id: &'static str, rule: &'static str, source: &'static str, status: Status) -> Rule {
    Rule { id, rule, source, status }
}

use Status::{Community, Placeholder, Verified};

/// Every rule, by id. Services name the rules they applied in their results.
pub const RULES: &[Rule] = &[
    // Light
    rule("light.cap", "Light cap 400 (2.4.1 and later)", "Patch notes 2.4.1 (drops and Exotic engrams up to 400); manifest: Year 3 Legendary primary stat maximum 400", Verified),
    rule("light.cap.2_4_0", "Light cap 385 in 2.4.0", "Patch notes 2.4.0 (Max Light raised to 385)", Verified),
    rule("light.item_bounds", "An instance's Attack/Defense lies within its item's own primary-stat minimum and maximum", "Manifest: DestinyInventoryItemDefinition.stats[primaryBaseStatHash]", Verified),
    rule("light.decode_cap.rare", "Rare engrams decrypt up to 365", "Patch notes 2.4.1", Verified),
    rule("light.decode_cap.legendary", "Legendary engrams decrypt up to 390 (2.4.1 gave 385)", "cryptarch-guide listPageData (legendary-engrams, final 2.6 state); Patch notes 2.4.1 say 385", Community),
    rule("light.decode_cap.exotic", "Exotic engrams decrypt up to 400", "Patch notes 2.4.1", Verified),
    rule("light.at_or_above", "Rare and Legendary drops try to land at or above the player's current Light", "Patch notes 2.4.0", Verified),
    rule("light.slow_zone", "In the last 15 Light below a cap, a drop goes higher only about half the time", "Patch notes 2.4.0 (Legendary engrams, 350-365); extended to later caps: placeholder, needs evidence", Placeholder),
    rule("light.spread", "A drop lands 0 to 4 above the basis", "Placeholder, needs evidence (not public)", Placeholder),
    rule("light.common_roll", "Common and Uncommon drops roll uniformly between the source floor and cap", "Placeholder, needs evidence (not public)", Placeholder),
    rule("light.basis", "The basis is the highest Light the character could equip (best per slot, at most one Exotic weapon and one Exotic armour)", "Community (Rise of Iron era player reports), needs evidence", Community),
    rule("light.weights", "Character Light = floor of the weighted mean: weapons 6, armour 5, Ghost/Artifact/class item 4 (of 50)", "Community (DIM's D1 Light calculation), needs evidence against the client's 0x265EF0 sum", Community),
    rule("light.infusion", "Infusing raises the target to the fuel's Light, up to the target's own maximum", "Community (Rise of Iron), needs evidence", Community),
    rule("light.infusion_cost", "Infusion costs nothing extra", "Placeholder, needs evidence (Glimmer and material costs not public)", Placeholder),
    // Engrams
    rule("engram.bucket", "An engram decrypts into the bucket it is listed under (its bucketTypeHash)", "Manifest: engram items (itemType 8) carry their slot's bucket", Verified),
    rule("engram.class", "Armour decrypts for the decrypting character's class", "Community", Community),
    rule("engram.min_tier", "An engram never decrypts below its own tier (no materials from Legendary engrams)", "Patch notes 2.0.0 (Legendary engrams no longer give materials); community for the general rule", Community),
    rule("engram.upgrade", "Chance to decrypt one tier higher: 0", "Placeholder, needs evidence (not public)", Placeholder),
    rule("engram.pool", "Decrypt pools per slot from the community engram pages", "cryptarch-guide rewardBoxData (legendary-/exotic-<slot>-engram)", Community),
    rule("engram.pool.fallback", "Without a community pool: every instanced item of that tier, bucket and class whose primary maximum reaches the Light cap", "Placeholder, needs evidence", Placeholder),
    rule("engram.uncommon_convert", "Level-40 characters turn Uncommon engrams into weapon or armour materials on pickup", "Patch notes 2.5.0", Verified),
    rule("engram.uncommon_convert.amount", "One material per converted engram", "Placeholder, needs evidence", Placeholder),
    // Loot
    rule("loot.activity_rewards", "Completion grants the activity's own reward list (Legendary Marks, Strange Coins, reputation, XP values)", "Manifest: DestinyActivityDefinition.rewards", Verified),
    rule("loot.display_map", "A reward display item stands for the stackable item of the same name", "Manifest: item names (reward_display.tsv)", Verified),
    rule("loot.weekly_bonus", "A '(Weekly Bonus Remaining: N)' marker limits the activity's bonus rewards to N a week, per account", "Manifest marker items; per-account scope from cryptarch-guide", Community),
    rule("loot.mark_limits", "Per-activity Legendary Mark grants and their daily/weekly limits", "cryptarch-guide activityPageData (mark_limits.tsv)", Community),
    rule("loot.drop_chance", "End-of-activity engram chances by tier", "Placeholder, needs evidence (not public)", Placeholder),
    rule("loot.per_player", "Each fireteam member's rewards are rolled separately", "Community (D1 personal loot); notes/loot-and-rewards.md design", Community),
    // Currencies
    rule("currency.caps", "Wallet caps are each item's maxStackSize (Glimmer 25,000, Legendary Marks 200, Silver Dust 250, planetary materials 200)", "Manifest: maxStackSize", Verified),
    rule("currency.overflow", "Amounts over a cap are lost", "Community", Community),
    rule("currency.marks_weekly_cap", "At most 100 Legendary Marks a week from activities (dismantling not counted)", "Placeholder: the Year 2 community figure, needs evidence for 2.6", Placeholder),
    rule("currency.scope.glimmer", "Glimmer is account-wide", "Client bucket table (vendor.rs BUCKETS: slot 308 scope 1)", Verified),
    rule("currency.scope", "Legendary Marks and Silver Dust are account-wide; materials and consumables are per character", "Community", Community),
    rule("currency.dismantle.silver", "Silver Dust engram items give 1 Silver Dust, Icy (Chroma) armour 5, Dusty Iron armour 1", "Patch notes 2.6.0 and 2.4.1.1", Verified),
    rule("currency.dismantle.marks", "Year 2 and 3 Legendary gear gives Legendary Marks", "Patch notes 2.0.0 (the source); the amount (3 a weapon, 2 armour) is a placeholder, needs evidence", Placeholder),
    rule("currency.dismantle.other", "Rare and lower gear gives nothing", "Placeholder, needs evidence", Placeholder),
    // Talent grids and growth
    rule("grid.level", "Grid level from item XP: step costs subtracted in order, the last repeating when repeatLastStep", "Manifest: DestinyProgressionDefinition steps; client 0x284630 (01.29 reading)", Verified),
    rule("grid.node_level", "A node needs grid level at least its steps' gridLevel", "Manifest: DestinyTalentGridDefinition activationRequirement", Verified),
    rule("grid.prerequisites", "A node needs its prerequisites active, and no node it is exclusive with", "Manifest fields; semantics community", Community),
    rule("grid.auto_unlock", "autoUnlocks nodes activate as soon as they are available", "Manifest field; semantics community", Community),
    rule("grid.random_roll", "Random nodes pick one of their steps uniformly when the instance is made", "Placeholder distribution, needs evidence", Placeholder),
    rule("grid.node_cost", "Activating a node costs nothing", "Placeholder, needs evidence (material requirement sets are not in DIM's manifest)", Placeholder),
    rule("grid.xp_share", "Every equipped item with a grid, and the subclass, gains the full XP the character earns", "Community (that they all gain); the split is a placeholder, needs evidence", Placeholder),
    rule("item.element_fields", "Element 0x80801732: SOID +0x00, level +0x18, content index +0x24, XP +0x38, progression ordinal +0x44, grid index +0x58, stat values +0x98", "Client reading (ps4/design/items-and-gear.md section 2)", Verified),
    rule("item.node_state", "Talent-grid node state (+0x5A, 36 bytes) and step selection (+0x7E)", "Layout not decoded; not written, needs evidence", Placeholder),
    rule("item.stat_roll", "Armour stats roll uniformly within the item's manifest minimum and maximum", "Range: manifest; distribution: placeholder, needs evidence", Placeholder),
    rule("item.level_for_light", "The element level that gives an Attack/Defense", "Needs the client's level curves (tag 0x80A5E244); resolver supplied by the caller, needs evidence", Placeholder),
    // Grimoire
    rule("grimoire.score", "Score = points of unlocked cards + points of each stat rank reached (all 834 cards: 5,610)", "Manifest: DestinyGrimoireCardDefinition; totals agree with the cardBriefs", Verified),
    rule("grimoire.triggers", "What unlocks a card or counts a stat", "Server-side rules, not public; callers unlock explicitly, needs evidence", Placeholder),
    // Records and quests
    rule("records.complete", "A record completes when every objective reaches its completionValue", "Manifest: DestinyRecordDefinition objectives, DestinyObjectiveDefinition", Verified),
    rule("records.book_points", "Each completed record adds 1000 to its book's progression", "Placeholder, needs evidence", Placeholder),
    rule("records.rewards", "Book rank rewards (rank, item, quantity), each claimed once", "Manifest: DestinyRecordBookDefinition rewards", Verified),
    rule("quests.advance", "A quest step completes when all its objectives do, and the next step in its questline replaces it", "Manifest: quest step objectives and questline setItemHashes order; semantics community", Community),
];

/// The rule with this id. Panics on an unknown id (a programming error).
pub fn rule_by_id(id: &str) -> &'static Rule {
    RULES.iter().find(|r| r.id == id).unwrap_or_else(|| panic!("unknown rewards rule {id:?}"))
}

/// A small seeded generator (SplitMix64). Every grant records its seed, so a test or a reviewer
/// can replay the exact rolls.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `lo..=hi` (`lo` when `hi < lo`).
    pub fn range(&mut self, lo: u32, hi: u32) -> u32 {
        if hi <= lo {
            return lo;
        }
        let span = u64::from(hi - lo) + 1;
        lo + (self.next_u64() % span) as u32
    }

    /// True with probability `per_mille / 1000`.
    pub fn chance(&mut self, per_mille: u32) -> bool {
        per_mille > 0 && (self.next_u64() % 1000) < u64::from(per_mille)
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            None
        } else {
            Some(&items[(self.next_u64() % items.len() as u64) as usize])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_ids_are_unique_and_placeholders_say_so() {
        let mut ids = std::collections::HashSet::new();
        for r in RULES {
            assert!(ids.insert(r.id), "duplicate rule {}", r.id);
            if r.status == Status::Placeholder {
                assert!(r.source.contains("needs evidence"), "{} is a placeholder without 'needs evidence'", r.id);
            }
            assert!(!r.source.is_empty() && !r.rule.is_empty(), "{}", r.id);
        }
    }

    #[test]
    fn every_placeholder_value_names_a_placeholder_rule() {
        for (id, _) in placeholders::ALL {
            assert_eq!(rule_by_id(id).status, Status::Placeholder, "{id}");
        }
    }

    #[test]
    fn the_note_lists_every_rule_with_its_status() {
        let note = include_str!("../../../../../../notes/ps4-56578-rewards-and-growth-2026-11-05.md");
        let mut rows = std::collections::HashMap::new();
        for line in note.lines().filter(|l| l.starts_with("| `")) {
            let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
            if let Some(id) = cells[0].strip_prefix('`').and_then(|c| c.strip_suffix('`')) {
                rows.insert(id.to_string(), cells.last().copied().unwrap_or_default().to_string());
            }
        }
        for r in RULES {
            let want = match r.status {
                Status::Verified => "verified",
                Status::Community => "community",
                Status::Placeholder => "placeholder",
            };
            assert_eq!(rows.get(r.id).map(String::as_str), Some(want), "the note's row for {}", r.id);
        }
        let count = |s: Status| RULES.iter().filter(|r| r.status == s).count();
        let summary = format!(
            "- {} verified;\n- {} community;\n- {} placeholder.",
            count(Status::Verified), count(Status::Community), count(Status::Placeholder)
        );
        assert!(note.replace('\r', "").contains(&summary), "the note's count should read:\n{summary}");
    }

    #[test]
    fn rng_is_reproducible_and_in_range() {
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        for _ in 0..1000 {
            let x = a.range(10, 20);
            assert_eq!(x, b.range(10, 20));
            assert!((10..=20).contains(&x));
        }
        assert!(!Rng::new(1).chance(0));
        assert!(Rng::new(1).chance(1000));
    }

    #[test]
    fn off_by_default() {
        if std::env::var_os("D1_REWARDS").is_none() {
            assert!(!enabled());
        }
    }
}
