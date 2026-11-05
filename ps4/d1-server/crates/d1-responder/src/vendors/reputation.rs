// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Reputation and rank-up packages (the factions, the Vanguard, the Crucible, the Cryptarch).
//!
//! Ranks come from the vendor's progression in the manifest (`steps[].progressTotal`; Dead Orbit,
//! Future War Cult, New Monarchy, the Vanguard and the Crucible: 1,500, 2,000, then 2,500 per rank,
//! with the last step repeating). The packages are vendors too: "New Monarchy Package", "Vanguard
//! Package" and so on (`inhibitBuying`), whose sales are a package's possible contents.
//!
//! ⚠ STAND-IN: which of a package's sales a rank-up gives (the live servers rolled it). Here one
//! sale of the package vendor matching the progression, seeded by the character and the rank.
//! ⚠ STAND-IN: where the package goes. D1 handed it over at the vendor; until that flow is on the
//! wire it is delivered to the Postmaster's Lost Items.
//! ⚠ The faction **allegiance** itself (accepting a faction's badge) is refused by the client before
//! any request is sent (run sense92); this module only counts reputation.

use crate::store::{self, CharacterId, Item, PlayerStore};
use crate::tables::Tables;

/// Rank for an amount of progress: how many steps it completes.
pub fn rank(steps: &[u32], repeat_last: bool, progress: u32) -> u32 {
    let mut left = progress;
    let mut r = 0u32;
    let mut i = 0usize;
    loop {
        let need = match steps.get(i) {
            Some(n) => *n,
            None if repeat_last && !steps.is_empty() => *steps.last().unwrap(),
            None => return r,
        };
        if need == 0 || left < need {
            return r;
        }
        left -= need;
        r += 1;
        i += 1;
        if r > 10_000 {
            return r;
        }
    }
}

/// The package vendors of a progression, by name: the vendor whose progression it is, plus " Package".
pub fn package_vendors(t: &Tables, progression: u32) -> Vec<u8> {
    let names: Vec<String> = t.vendors.values().filter(|v| v.progression == progression && !v.inhibit_buying)
        .map(|v| match v.name.as_str() {
            n if n.ends_with("Vanguard") || n.contains("Vanguard") => "Vanguard Package".to_string(),
            n if n.contains("Crucible") => "Crucible Package".to_string(),
            n => format!("{n} Package"),
        }).collect();
    let mut out: Vec<u8> = t.vendors.values().filter(|v| v.inhibit_buying && names.contains(&v.name)).map(|v| v.index).collect();
    out.sort_unstable();
    out.dedup();
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankUp {
    pub rank: u32,
    pub package: Option<(u8, u16, u16)>,
}

/// Add reputation; deliver a package for every rank gained.
pub fn add<S: PlayerStore + ?Sized>(s: &mut S, t: &Tables, sales: &std::collections::HashMap<(u8, u16), crate::vendor::Sale>,
                                   chr: CharacterId, progression: u32, amount: u32) -> Vec<RankUp> {
    let before = s.progression(chr, progression);
    s.add_progression(chr, progression, amount);
    let Some(p) = t.progressions.get(&progression) else { return Vec::new() };
    let (r0, r1) = (rank(&p.steps, p.repeat_last, before), rank(&p.steps, p.repeat_last, before + amount));
    let vendors = package_vendors(t, progression);
    (r0 + 1..=r1).map(|r| {
        let package = vendors.first().and_then(|&v| {
            let n = t.vendors[&v].sales as usize;
            if n == 0 {
                return None;
            }
            let k = super::pick(chr ^ (u64::from(progression) << 20) ^ u64::from(r), n, 1)[0] as u16;
            let sale = sales.get(&(v, k))?;
            let item = if sale.instanced { Item::instanced(sale.item, s.mint_soid(), 0) } else { Item::stack(sale.item, 1) };
            store::give(s, store::Owner::Character(chr), store::LOST_ITEMS, item, 0).ok()?;
            Some((v, k, sale.item))
        });
        RankUp { rank: r, package }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::{memory, CHR};

    #[test]
    fn faction_ranks_follow_the_manifest_steps() {
        let s = [1500, 2000, 2500];
        assert_eq!(rank(&s, true, 0), 0);
        assert_eq!(rank(&s, true, 1499), 0);
        assert_eq!(rank(&s, true, 1500), 1);
        assert_eq!(rank(&s, true, 3500), 2);
        assert_eq!(rank(&s, true, 6000), 3);
        assert_eq!(rank(&s, true, 8500), 4, "the last step repeats");
        assert_eq!(rank(&s, false, 8500), 3);
    }

    #[test]
    fn a_new_monarchy_rank_up_brings_a_package() {
        let t = crate::tables::tables();
        assert_eq!(package_vendors(t, 3871980777), vec![52, 53, 54, 55], "the New Monarchy Packages");
        assert!(!package_vendors(t, 3233510749).is_empty(), "the Vanguard Packages");
        let mut s = memory();
        let ups = add(&mut s, t, crate::tables::fixture_sales(), CHR, 3871980777, 1600);
        assert_eq!(ups.len(), 1);
        assert_eq!(ups[0].rank, 1);
        assert!(ups[0].package.is_some());
        assert_eq!(s.progression(CHR, 3871980777), 1600);
        assert!(add(&mut s, t, crate::tables::fixture_sales(), CHR, 3871980777, 100).is_empty(), "no rank in between");
    }
}
