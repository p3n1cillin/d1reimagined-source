// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The compact tables under `tables/`, written by `tools/rewards_tables/gen_rewards_tables.py`
//! from Bungie's final D1 manifest (and, for the files whose header says COMMUNITY STAND-IN, from
//! nruble/cryptarch-guide). Each file's `#` header names its input and that input's SHA-256.
//!
//! The tables are compiled in and parsed once ([`tables`]). A malformed row is a generator bug, so
//! parsing panics with the file and line rather than skipping it silently.

use std::collections::HashMap;
use std::sync::OnceLock;

use d1_loadout::model::Tier;

/// One `DestinyInventoryItemDefinition` row (gear, engrams, materials and currencies only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemDef {
    pub hash: u32,
    pub tier: Tier,
    pub bucket: u32,
    pub item_type: u32,
    /// 0 Titan, 1 Hunter, 2 Warlock, 3 any.
    pub class_type: u32,
    pub instanced: bool,
    pub equippable: bool,
    /// 0 = none.
    pub talent_grid: u32,
    pub max_stack: u32,
    /// Attack (`STAT_DAMAGE`) or Defense (`STAT_DEFENSE`); 0 = none.
    pub primary_stat: u32,
    pub primary_min: u32,
    pub primary_max: u32,
    /// Armour stats an instance rolls: `(stat hash, minimum, maximum)`.
    pub rolled_stats: Vec<(u32, u32, u32)>,
}

impl ItemDef {
    pub fn is_engram(&self) -> bool {
        self.item_type == super::ITEM_TYPE_ENGRAM
    }

    pub fn fits_class(&self, class_type: u32) -> bool {
        self.class_type == 3 || self.class_type == class_type
    }
}

/// A `DestinyProgressionDefinition`: the cost of each step, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressionDef {
    pub hash: u32,
    pub identifier: String,
    pub scope: u32,
    pub repeat_last_step: bool,
    pub step_costs: Vec<u32>,
}

/// One talent-grid node, with the activation level of its steps (one level per node across the
/// whole manifest: 11,175 nodes, none with two).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeDef {
    pub index: u16,
    pub row: i32,
    pub column: i32,
    pub grid_level: u32,
    pub auto_unlocks: bool,
    pub is_random: bool,
    pub last_step_repeats: bool,
    pub random_repurchasable: bool,
    pub steps: u16,
    pub prerequisites: Vec<u16>,
    pub exclusive_with: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridDef {
    pub hash: u32,
    pub progression: u32,
    /// 0 = no cap beyond the progression.
    pub max_grid_level: u32,
    pub nodes: Vec<NodeDef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityDef {
    pub hash: u32,
    pub level: u32,
    pub power: u32,
    /// -1 on 144 rows.
    pub tier: i32,
    pub activity_type: u32,
    /// `(display item hash, value)` from the activity's reward list.
    pub rewards: Vec<(u32, u32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordBookDef {
    pub hash: u32,
    pub progression: u32,
    pub item: u32,
    pub record_count: u32,
    pub records: Vec<u32>,
    /// `(rank, item, quantity)`.
    pub rewards: Vec<(u32, u32, u32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectiveDef {
    pub hash: u32,
    pub completion_value: i64,
    pub counting_down: bool,
    pub allow_negative: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestStepDef {
    pub hash: u32,
    pub questline: u32,
    /// Index in the questline's `setItemHashes`; -1 when the questline does not list it.
    pub position: i32,
    pub objectives: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrimoireCardDef {
    pub id: u32,
    pub theme: u32,
    pub page: u32,
    pub points: u32,
    /// `(stat number, [(threshold, points)])`.
    pub stats: Vec<(u32, Vec<(u64, u32)>)>,
}

/// COMMUNITY STAND-IN: one engram page of nruble/cryptarch-guide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngramPool {
    pub slug: String,
    pub tier: Tier,
    pub bucket: u32,
    pub legacy: bool,
    pub items: Vec<u32>,
}

/// COMMUNITY STAND-IN: a Legendary Mark grant and its limit, by the guide's activity page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkLimit {
    pub activity: String,
    pub marks: u32,
    /// `daily`, `weekly` or empty.
    pub period: String,
    pub limit: Option<u32>,
    pub account: bool,
}

pub struct Tables {
    pub items: HashMap<u32, ItemDef>,
    pub progressions: HashMap<u32, ProgressionDef>,
    pub grids: HashMap<u32, GridDef>,
    pub activities: HashMap<u32, ActivityDef>,
    /// Activity reward display item -> the stackable item it stands for.
    pub reward_display: HashMap<u32, u32>,
    /// `(Weekly Bonus Remaining: N)` marker item -> N.
    pub weekly_bonus: HashMap<u32, u32>,
    pub record_books: HashMap<u32, RecordBookDef>,
    pub records: HashMap<u32, Vec<u32>>,
    pub objectives: HashMap<u32, ObjectiveDef>,
    pub quests: HashMap<u32, QuestStepDef>,
    pub grimoire: Vec<GrimoireCardDef>,
    /// The `cardBriefs totalPoints` sum the generator read from the manifest.
    pub grimoire_manifest_total: u32,
    pub engram_pools: Vec<EngramPool>,
    pub mark_limits: Vec<MarkLimit>,
}

const ITEMS: &str = include_str!("tables/items.tsv");
const PROGRESSIONS: &str = include_str!("tables/progressions.tsv");
const GRIDS: &str = include_str!("tables/talent_grids.tsv");
const ACTIVITIES: &str = include_str!("tables/activities.tsv");
const REWARD_DISPLAY: &str = include_str!("tables/reward_display.tsv");
const WEEKLY_BONUS: &str = include_str!("tables/weekly_bonus.tsv");
const RECORD_BOOKS: &str = include_str!("tables/record_books.tsv");
const RECORDS: &str = include_str!("tables/records.tsv");
const OBJECTIVES: &str = include_str!("tables/objectives.tsv");
const QUESTS: &str = include_str!("tables/quests.tsv");
const GRIMOIRE: &str = include_str!("tables/grimoire.tsv");
const ENGRAM_POOLS: &str = include_str!("tables/engram_pools.tsv");
const MARK_LIMITS: &str = include_str!("tables/mark_limits.tsv");

/// The parsed tables (parsed on first use).
pub fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(parse_all)
}

fn rows<'a>(name: &'static str, text: &'a str, cols: usize) -> impl Iterator<Item = (usize, Vec<&'a str>)> + 'a {
    text.lines().enumerate().filter(|(_, l)| !l.is_empty() && !l.starts_with('#')).map(move |(n, l)| {
        let f: Vec<&str> = l.split('\t').collect();
        assert_eq!(f.len(), cols, "{name}:{}: {} columns, expected {cols}", n + 1, f.len());
        (n + 1, f)
    })
}

fn num<T: std::str::FromStr>(name: &str, line: usize, s: &str) -> T {
    s.parse().unwrap_or_else(|_| panic!("{name}:{line}: not a number: {s:?}"))
}

fn list<T: std::str::FromStr>(name: &str, line: usize, s: &str, sep: char) -> Vec<T> {
    if s == "-" || s.is_empty() {
        return Vec::new();
    }
    s.split(sep).map(|x| num(name, line, x)).collect()
}

fn parse_all() -> Tables {
    let mut items = HashMap::new();
    for (n, f) in rows("items.tsv", ITEMS, 13) {
        let p = |i: usize| -> u32 { num("items.tsv", n, f[i]) };
        let d = ItemDef {
            hash: p(0),
            tier: Tier::from_tier_type(p(1)),
            bucket: p(2),
            item_type: p(3),
            class_type: p(4),
            instanced: p(5) != 0,
            equippable: p(6) != 0,
            talent_grid: p(7),
            max_stack: p(8),
            primary_stat: p(9),
            primary_min: p(10),
            primary_max: p(11),
            rolled_stats: if f[12] == "-" {
                Vec::new()
            } else {
                f[12].split(',')
                    .map(|r| {
                        let v: Vec<u32> = r.split(':').map(|x| num("items.tsv", n, x)).collect();
                        assert_eq!(v.len(), 3, "items.tsv:{n}: {r:?}");
                        (v[0], v[1], v[2])
                    })
                    .collect()
            },
        };
        items.insert(d.hash, d);
    }

    let mut progressions = HashMap::new();
    for (n, f) in rows("progressions.tsv", PROGRESSIONS, 5) {
        let d = ProgressionDef {
            hash: num("progressions.tsv", n, f[0]),
            identifier: f[1].to_string(),
            scope: num("progressions.tsv", n, f[2]),
            repeat_last_step: f[3] == "1",
            step_costs: list("progressions.tsv", n, f[4], ','),
        };
        progressions.insert(d.hash, d);
    }

    let mut grids = HashMap::new();
    for (n, f) in rows("talent_grids.tsv", GRIDS, 4) {
        let nodes = if f[3] == "-" {
            Vec::new()
        } else {
            f[3].split(';')
                .map(|node| {
                    let p: Vec<&str> = node.split(':').collect();
                    assert_eq!(p.len(), 8, "talent_grids.tsv:{n}: node {node:?}");
                    let flags: u32 = num("talent_grids.tsv", n, p[4]);
                    NodeDef {
                        index: num("talent_grids.tsv", n, p[0]),
                        row: num("talent_grids.tsv", n, p[1]),
                        column: num("talent_grids.tsv", n, p[2]),
                        grid_level: num("talent_grids.tsv", n, p[3]),
                        auto_unlocks: flags & 1 != 0,
                        is_random: flags & 2 != 0,
                        last_step_repeats: flags & 4 != 0,
                        random_repurchasable: flags & 8 != 0,
                        steps: num("talent_grids.tsv", n, p[5]),
                        prerequisites: list("talent_grids.tsv", n, p[6], '.'),
                        exclusive_with: list("talent_grids.tsv", n, p[7], '.'),
                    }
                })
                .collect()
        };
        let d = GridDef {
            hash: num("talent_grids.tsv", n, f[0]),
            progression: num("talent_grids.tsv", n, f[1]),
            max_grid_level: num("talent_grids.tsv", n, f[2]),
            nodes,
        };
        grids.insert(d.hash, d);
    }

    let mut activities = HashMap::new();
    for (n, f) in rows("activities.tsv", ACTIVITIES, 6) {
        let rewards = if f[5] == "-" {
            Vec::new()
        } else {
            f[5].split(',')
                .map(|r| {
                    let (h, v) = r.split_once(':').unwrap_or_else(|| panic!("activities.tsv:{n}: {r:?}"));
                    (num("activities.tsv", n, h), num("activities.tsv", n, v))
                })
                .collect()
        };
        let d = ActivityDef {
            hash: num("activities.tsv", n, f[0]),
            level: num("activities.tsv", n, f[1]),
            power: num("activities.tsv", n, f[2]),
            tier: num("activities.tsv", n, f[3]),
            activity_type: num("activities.tsv", n, f[4]),
            rewards,
        };
        activities.insert(d.hash, d);
    }

    let pairs = |name: &'static str, text: &'static str| -> HashMap<u32, u32> {
        rows(name, text, 2).map(|(n, f)| (num(name, n, f[0]), num(name, n, f[1]))).collect()
    };
    let reward_display = pairs("reward_display.tsv", REWARD_DISPLAY);
    let weekly_bonus = pairs("weekly_bonus.tsv", WEEKLY_BONUS);

    let mut record_books = HashMap::new();
    for (n, f) in rows("record_books.tsv", RECORD_BOOKS, 6) {
        let rewards = if f[5] == "-" {
            Vec::new()
        } else {
            f[5].split(',')
                .map(|r| {
                    let p: Vec<u32> = r.split(':').map(|x| num("record_books.tsv", n, x)).collect();
                    assert_eq!(p.len(), 3, "record_books.tsv:{n}: reward {r:?}");
                    (p[0], p[1], p[2])
                })
                .collect()
        };
        let d = RecordBookDef {
            hash: num("record_books.tsv", n, f[0]),
            progression: num("record_books.tsv", n, f[1]),
            item: num("record_books.tsv", n, f[2]),
            record_count: num("record_books.tsv", n, f[3]),
            records: list("record_books.tsv", n, f[4], ','),
            rewards,
        };
        record_books.insert(d.hash, d);
    }

    let records = rows("records.tsv", RECORDS, 2)
        .map(|(n, f)| (num("records.tsv", n, f[0]), list("records.tsv", n, f[1], ',')))
        .collect();

    let objectives = rows("objectives.tsv", OBJECTIVES, 4)
        .map(|(n, f)| {
            let d = ObjectiveDef {
                hash: num("objectives.tsv", n, f[0]),
                completion_value: num("objectives.tsv", n, f[1]),
                counting_down: f[2] == "1",
                allow_negative: f[3] == "1",
            };
            (d.hash, d)
        })
        .collect();

    let quests = rows("quests.tsv", QUESTS, 4)
        .map(|(n, f)| {
            let d = QuestStepDef {
                hash: num("quests.tsv", n, f[0]),
                questline: num("quests.tsv", n, f[1]),
                position: num("quests.tsv", n, f[2]),
                objectives: list("quests.tsv", n, f[3], ','),
            };
            (d.hash, d)
        })
        .collect();

    let mut grimoire = Vec::new();
    for (n, f) in rows("grimoire.tsv", GRIMOIRE, 5) {
        let stats = if f[4] == "-" {
            Vec::new()
        } else {
            f[4].split(';')
                .map(|s| {
                    let (number, ranks) = s.split_once('=').unwrap_or_else(|| panic!("grimoire.tsv:{n}: {s:?}"));
                    let ranks = if ranks == "-" {
                        Vec::new()
                    } else {
                        ranks.split(',')
                            .map(|r| {
                                let (t, p) = r.split_once(':').unwrap_or_else(|| panic!("grimoire.tsv:{n}: {r:?}"));
                                (num("grimoire.tsv", n, t), num("grimoire.tsv", n, p))
                            })
                            .collect()
                    };
                    (num("grimoire.tsv", n, number), ranks)
                })
                .collect()
        };
        grimoire.push(GrimoireCardDef {
            id: num("grimoire.tsv", n, f[0]),
            theme: num("grimoire.tsv", n, f[1]),
            page: num("grimoire.tsv", n, f[2]),
            points: num("grimoire.tsv", n, f[3]),
            stats,
        });
    }
    let grimoire_manifest_total = GRIMOIRE
        .lines()
        .find_map(|l| l.strip_prefix("# manifest cardBriefs totalPoints sum = "))
        .map(|v| num("grimoire.tsv", 0, v.trim()))
        .expect("grimoire.tsv: no manifest total header");

    let engram_pools = rows("engram_pools.tsv", ENGRAM_POOLS, 5)
        .map(|(n, f)| EngramPool {
            slug: f[0].to_string(),
            tier: Tier::from_tier_type(num("engram_pools.tsv", n, f[1])),
            bucket: num("engram_pools.tsv", n, f[2]),
            legacy: f[3] == "1",
            items: list("engram_pools.tsv", n, f[4], ','),
        })
        .collect();

    let mark_limits = rows("mark_limits.tsv", MARK_LIMITS, 5)
        .map(|(n, f)| MarkLimit {
            activity: f[0].to_string(),
            marks: num("mark_limits.tsv", n, f[1]),
            period: if f[2] == "-" { String::new() } else { f[2].to_string() },
            limit: if f[3] == "-" { None } else { Some(num("mark_limits.tsv", n, f[3])) },
            account: f[4] == "account",
        })
        .collect();

    Tables {
        items,
        progressions,
        grids,
        activities,
        reward_display,
        weekly_bonus,
        record_books,
        records,
        objectives,
        quests,
        grimoire,
        grimoire_manifest_total,
        engram_pools,
        mark_limits,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_table_parses_and_is_not_empty() {
        let t = tables();
        assert!(t.items.len() > 6000, "items {}", t.items.len());
        assert_eq!(t.progressions.len(), 252);
        assert!(t.grids.len() > 1000);
        assert_eq!(t.activities.len(), 653);
        assert_eq!(t.record_books.len(), 5);
        assert_eq!(t.records.len(), 168);
        assert_eq!(t.objectives.len(), 1388);
        assert_eq!(t.grimoire.len(), 834);
        assert!(!t.engram_pools.is_empty());
        assert!(!t.mark_limits.is_empty());
    }

    #[test]
    fn every_item_grid_is_in_the_grid_table() {
        let t = tables();
        for d in t.items.values().filter(|d| d.talent_grid != 0) {
            assert!(t.grids.contains_key(&d.talent_grid), "item {} grid {}", d.hash, d.talent_grid);
        }
    }

    #[test]
    fn node_references_stay_inside_their_grid() {
        for g in tables().grids.values() {
            let n = g.nodes.len() as u16;
            for node in &g.nodes {
                assert!(node.index < n, "grid {} node {}", g.hash, node.index);
                for p in node.prerequisites.iter().chain(&node.exclusive_with) {
                    assert!(*p < n, "grid {} node {} refers to {p}", g.hash, node.index);
                }
            }
        }
    }

    #[test]
    fn every_table_header_cites_its_source() {
        for (name, text) in [
            ("items", ITEMS), ("progressions", PROGRESSIONS), ("grids", GRIDS), ("activities", ACTIVITIES),
            ("reward_display", REWARD_DISPLAY), ("weekly_bonus", WEEKLY_BONUS), ("record_books", RECORD_BOOKS),
            ("records", RECORDS), ("objectives", OBJECTIVES), ("quests", QUESTS), ("grimoire", GRIMOIRE),
            ("engram_pools", ENGRAM_POOLS), ("mark_limits", MARK_LIMITS),
        ] {
            let first = text.lines().next().unwrap_or_default();
            assert!(first.starts_with("# ") && first.contains("sha256 "), "{name}: {first:?}");
            assert!(text.contains("gen_rewards_tables.py"), "{name}");
        }
        assert!(ENGRAM_POOLS.starts_with("# COMMUNITY STAND-IN"));
        assert!(MARK_LIMITS.starts_with("# COMMUNITY STAND-IN"));
    }
}
