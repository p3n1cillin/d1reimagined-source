// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! Build 56578's client incident catalogue: incident index ↔ FNV-1 name hash, both ways.
//!
//! The tables are Codex's content export (PR #306, `ps4/fixtures/56578/incidents-56578.tsv` and
//! `incidents-51539-to-56578.tsv`; method in `notes/ps4-56578-incident-catalog-2026-11-05.md`):
//! 3,902 records read from the owned packages, 213 with verified names, and the 01.24/51539 table
//! joined to them by hash. This module reads them at run time; it embeds none of them.
//!
//! BullShcript's scripts name incidents (`bind_incident_name_hash("inventory_item_added")`), and its
//! sidecar reports incidents by name hash, so the script bridge translates every incident it passes
//! either way through here. The few identities the bridge itself acts on are [`known`] constants,
//! checked against the loaded table ([`IncidentCatalog::verify_known`]): a table that disagrees is
//! refused rather than half-trusted.

use std::collections::HashMap;

/// FNV-1 (32-bit), the hash the catalogue's names use: multiply, then XOR each byte.
pub fn fnv1(s: &str) -> u32 {
    s.bytes().fold(0x811C_9DC5u32, |h, b| h.wrapping_mul(0x0100_0193) ^ u32::from(b))
}

/// One incident identity the bridge acts on: its 56578 index, its name hash and its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Known {
    pub index: u32,
    pub hash: u32,
    pub name: &'static str,
}

/// The identities the bridge uses, each confirmed in the catalogue's notes: the three cinematic
/// and trigger ids by matched live-capture pairs, `activity_complete` by its 01.24 twin 789.
pub mod known {
    use super::Known;
    pub const CINEMATIC_FINISHED: Known = Known { index: 891, hash: 0x3C03_F059, name: "cinematic_finished" };
    pub const CINEMATIC_STARTED: Known = Known { index: 2681, hash: 0xAECA_3822, name: "cinematic_started" };
    pub const CINEMATIC_SKIP: Known = Known { index: 1726, hash: 0x7352_DAFE, name: "cinematic_skip" };
    pub const PLAYER_TRIGGER: Known = Known { index: 3403, hash: 0xDD6D_986F, name: "player_trigger" };
    pub const ACTIVITY_COMPLETE: Known = Known { index: 965, hash: 0x4266_0690, name: "activity_complete" };
    pub const PLAYER_SPAWNED: Known = Known { index: 566, hash: 0x2766_028D, name: "player_spawned" };
    pub const PLAYER_SPAWNED_INITIAL: Known = Known { index: 2014, hash: 0x84D5_0D24, name: "player_spawned_initial" };
    pub const PLAYER_DIED: Known = Known { index: 2079, hash: 0x88CD_EB67, name: "player_died" };
    pub const INVENTORY_ITEM_ADDED: Known = Known { index: 2899, hash: 0xBD1F_B418, name: "inventory_item_added" };
    /// F49: the client opened A Guardian Rises' loot cache (live capture, 684.5 s: `gather_material`
    /// 1851 with this one; the host's answer was the special weapon's push).
    pub const TUTORIAL_WEAPON_SPECIAL: Known = Known { index: 1131, hash: 0x4BE0_1A52, name: "tutorial_weapon_special" };

    pub const ALL: [Known; 10] = [
        CINEMATIC_FINISHED,
        CINEMATIC_STARTED,
        CINEMATIC_SKIP,
        PLAYER_TRIGGER,
        ACTIVITY_COMPLETE,
        PLAYER_SPAWNED,
        PLAYER_SPAWNED_INITIAL,
        PLAYER_DIED,
        INVENTORY_ITEM_ADDED,
        TUTORIAL_WEAPON_SPECIAL,
    ];
}

/// One catalogue record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IncidentRecord {
    pub index: u32,
    pub name_hash: u32,
    /// Blank in the table for the 3,689 records with no verified name.
    pub name: Option<String>,
    pub payload_selector: u8,
}

/// The 56578 table, with the optional 51539 → 56578 index map.
#[derive(Clone, Debug, Default)]
pub struct IncidentCatalog {
    records: Vec<IncidentRecord>,
    by_hash: HashMap<u32, u32>,
    legacy: HashMap<u32, u32>,
}

fn hex_u32(s: &str) -> Option<u32> {
    u32::from_str_radix(s.strip_prefix("0x")?, 16).ok()
}

impl IncidentCatalog {
    /// Parse `incidents-56578.tsv`: a header, then `index name_hash name payload_selector aux` rows
    /// with contiguous indices from 0 and distinct hashes (the exporter's own invariants).
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        let header = lines.next().ok_or("empty incident catalogue")?;
        if !header.starts_with("index\tname_hash\tname\tpayload_selector") {
            return Err(format!("unexpected incident catalogue header {header:?}"));
        }
        let mut c = Self::default();
        for (n, line) in lines.enumerate().filter(|(_, l)| !l.trim().is_empty()) {
            let f: Vec<&str> = line.split('\t').collect();
            let bad = || format!("incident catalogue row {}: {line:?}", n + 2);
            if f.len() < 4 {
                return Err(bad());
            }
            let index: u32 = f[0].parse().map_err(|_| bad())?;
            let name_hash = hex_u32(f[1]).ok_or_else(bad)?;
            let payload_selector: u8 = f[3].parse().map_err(|_| bad())?;
            if index as usize != c.records.len() {
                return Err(format!("incident catalogue row {}: index {index} is not contiguous", n + 2));
            }
            if c.by_hash.insert(name_hash, index).is_some() {
                return Err(format!("incident catalogue: duplicate hash {name_hash:#010x} at index {index}"));
            }
            let name = (!f[2].is_empty()).then(|| f[2].to_string());
            if let Some(name) = &name {
                if fnv1(name) != name_hash {
                    return Err(format!("incident catalogue: {name:?} does not hash to {name_hash:#010x}"));
                }
            }
            c.records.push(IncidentRecord { index, name_hash, name, payload_selector });
        }
        Ok(c)
    }

    /// Add `incidents-51539-to-56578.tsv` (`index_51539 index_56578 name_hash name`; a blank target
    /// means absent). Each row's hash must be the 56578 record's.
    pub fn add_legacy(&mut self, text: &str) -> Result<(), String> {
        let mut lines = text.lines();
        let header = lines.next().ok_or("empty legacy incident map")?;
        if !header.starts_with("index_51539\tindex_56578\tname_hash") {
            return Err(format!("unexpected legacy incident map header {header:?}"));
        }
        for line in lines.filter(|l| !l.trim().is_empty()) {
            let f: Vec<&str> = line.split('\t').collect();
            let bad = || format!("legacy incident map row {line:?}");
            let old: u32 = f.first().and_then(|s| s.parse().ok()).ok_or_else(bad)?;
            let Some(new) = f.get(1).filter(|s| !s.is_empty()) else { continue };
            let new: u32 = new.parse().map_err(|_| bad())?;
            let hash = f.get(2).and_then(|s| hex_u32(s)).ok_or_else(bad)?;
            if self.name_hash(new) != Some(hash) {
                return Err(format!("legacy incident map: 56578 index {new} is not {hash:#010x}"));
            }
            self.legacy.insert(old, new);
        }
        Ok(())
    }

    /// Load the table at `path`, and the legacy map beside it when present.
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut c = Self::parse(&text)?;
        let legacy = path.with_file_name("incidents-51539-to-56578.tsv");
        if let Ok(text) = std::fs::read_to_string(&legacy) {
            c.add_legacy(&text)?;
        }
        c.verify_known()?;
        Ok(c)
    }

    /// Refuse a table that disagrees with an identity the bridge acts on.
    pub fn verify_known(&self) -> Result<(), String> {
        for k in known::ALL {
            if self.name_hash(k.index) != Some(k.hash) || fnv1(k.name) != k.hash {
                return Err(format!(
                    "incident catalogue disagrees with {} = {} ({:#010x})",
                    k.name, k.index, k.hash
                ));
            }
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn name_hash(&self, index: u32) -> Option<u32> {
        self.records.get(index as usize).map(|r| r.name_hash)
    }

    pub fn index_of(&self, name_hash: u32) -> Option<u32> {
        self.by_hash.get(&name_hash).copied()
    }

    pub fn index_of_name(&self, name: &str) -> Option<u32> {
        self.index_of(fnv1(name))
    }

    pub fn name(&self, index: u32) -> Option<&str> {
        self.records.get(index as usize)?.name.as_deref()
    }

    /// The 56578 index of a 51539 / 01.24 index, by the hash join.
    pub fn from_legacy(&self, index_51539: u32) -> Option<u32> {
        self.legacy.get(&index_51539).copied()
    }

    /// A short label for logs: the name if known, else the hash.
    pub fn label(&self, index: u32) -> String {
        match (self.name(index), self.name_hash(index)) {
            (Some(n), _) => format!("{index} {n}"),
            (None, Some(h)) => format!("{index} {h:#010x}"),
            (None, None) => format!("{index} (not in the catalogue)"),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A synthetic table: the real rows for the identities the bridge uses, at their real indices,
    /// with unnamed filler between them (the filler hashes are not the catalogue's).
    pub(crate) fn synthetic() -> IncidentCatalog {
        let mut text = String::from("index\tname_hash\tname\tpayload_selector\taux\n");
        let max = known::ALL.iter().map(|k| k.index).max().unwrap();
        for i in 0..=max {
            match known::ALL.iter().find(|k| k.index == i) {
                Some(k) => text.push_str(&format!("{i}\t{:#010x}\t{}\t17\t65535,65535,65535,65535\n", k.hash, k.name)),
                None => text.push_str(&format!("{i}\t{:#010x}\t\t0\t65535,65535,65535,65535\n", 0x1000_0000u32 + i)),
            }
        }
        IncidentCatalog::parse(&text).expect("synthetic catalogue")
    }

    #[test]
    fn fnv1_matches_the_catalogue_names() {
        for k in known::ALL {
            assert_eq!(fnv1(k.name), k.hash, "{}", k.name);
        }
        assert_eq!(fnv1(""), 0x811C_9DC5);
    }

    #[test]
    fn the_table_translates_both_ways_and_refuses_bad_rows() {
        let c = synthetic();
        c.verify_known().unwrap();
        assert_eq!(c.index_of(0x4266_0690), Some(965));
        assert_eq!(c.index_of_name("cinematic_finished"), Some(891));
        assert_eq!(c.name(1726), Some("cinematic_skip"));
        assert_eq!(c.label(3), "3 0x10000003");
        let head = "index\tname_hash\tname\tpayload_selector\taux\n";
        assert!(IncidentCatalog::parse(&format!("{head}1\t0x00000001\t\t0\tx\n")).is_err(), "gap");
        assert!(IncidentCatalog::parse(&format!("{head}0\t0x00000001\t\t0\tx\n1\t0x00000001\t\t0\tx\n")).is_err(), "duplicate");
        assert!(IncidentCatalog::parse(&format!("{head}0\t0x00000001\tkill\t0\tx\n")).is_err(), "name/hash disagree");
        let mut wrong = c.clone();
        wrong.records[965].name_hash = 1;
        assert!(wrong.verify_known().is_err());
    }

    #[test]
    fn the_legacy_map_joins_by_hash() {
        let mut c = synthetic();
        let map = "index_51539\tindex_56578\tname_hash\tname\n789\t965\t0x42660690\tactivity_complete\n2114\t\t0xae820ae1\t\n";
        c.add_legacy(map).unwrap();
        assert_eq!(c.from_legacy(789), Some(965));
        assert_eq!(c.from_legacy(2114), None, "a blank target is absent, never index 0");
        assert!(c.add_legacy("index_51539\tindex_56578\tname_hash\tname\n1\t965\t0x00000001\t\n").is_err());
    }

    /// The real table, when it is present (it is on Codex's branch until PR #306 merges; or point
    /// `D1_AH_SCRIPTS_INCIDENTS` at a copy).
    #[test]
    fn the_committed_catalogue_has_every_known_identity() {
        let path = std::env::var("D1_AH_SCRIPTS_INCIDENTS").map(std::path::PathBuf::from).unwrap_or_else(|_| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/56578/incidents-56578.tsv")
        });
        if !path.is_file() {
            eprintln!("SKIP: no incident catalogue at {}", path.display());
            return;
        }
        let c = IncidentCatalog::load(&path).expect("catalogue");
        assert_eq!(c.len(), 3902);
        assert_eq!(c.from_legacy(789), Some(965));
        assert_eq!(c.from_legacy(734), Some(891));
    }
}
