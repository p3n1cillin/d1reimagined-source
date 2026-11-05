// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Which sales a vendor offers now.
//!
//! The manifest lists each vendor's whole catalogue (Xûr: 654 sales) and, per category, how many of
//! its sales one refresh offers (`quantityAvailable`): Xûr's "Exotic Gear" categories offer 1 of
//! 12–24, the Bounty Tracker's "Available Bounties" 1–6, the Gunsmith's "Foundry Orders" 5 of 25.
//! The stock changes at the vendor's reset ([`super::schedule`]).
//!
//! ⚠ STAND-IN: *which* sales a refresh picked was the live servers' choice and is in no data. Here a
//! refresh picks `quantity` sales of the category at random, seeded by the vendor's hash, the
//! category and the period, so every player sees the same stock for the whole period, as in D1, and
//! it changes at each reset. Categories with quantity 0 (the Cryptarch's "Decode Engram", the
//! Bounty Tracker's "Collect Bounty"), or no more sales than the quantity, offer every sale.
//! Categories the manifest marks `hideFromRegularPurchase` (rewards handed out by other flows) are
//! offered too: the client decides what it lists. Xûr offers nothing outside his weekend.

use std::collections::{BTreeMap, BTreeSet};

use super::schedule::{period, xur_present, XUR_HASH};
use crate::tables::Tables;

/// The sales `vendor` offers at `now_minutes`.
pub fn offered(t: &Tables, vendor: u8, now_minutes: i64) -> BTreeSet<u16> {
    let Some(v) = t.vendors.get(&vendor) else { return BTreeSet::new() };
    if v.hash == XUR_HASH && !xur_present(t, now_minutes) {
        return BTreeSet::new();
    }
    let p = period(now_minutes, v.reset_interval, v.reset_offset);
    let mut by_category: BTreeMap<u16, Vec<u16>> = BTreeMap::new();
    for s in 0..v.sales {
        if let Some(r) = t.rules.get(&(vendor, s)) {
            by_category.entry(r.category).or_default().push(s);
        }
    }
    let mut out = BTreeSet::new();
    for (c, sales) in by_category {
        let q = t.categories.get(&(vendor, c)).map(|x| x.quantity as usize).unwrap_or(0);
        if q == 0 || sales.len() <= q || v.reset_interval == 0 {
            out.extend(sales);
            continue;
        }
        let seed = (u64::from(v.hash) << 32) ^ (u64::from(c) << 48) ^ (p as u64);
        out.extend(super::pick(seed, sales.len(), q).into_iter().map(|i| sales[i]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::tables;
    use crate::vendors::testing::{SATURDAY, WEDNESDAY};

    #[test]
    fn xur_sells_nothing_midweek_and_one_per_exotic_category_on_saturday() {
        let t = tables();
        assert!(offered(t, 5, WEDNESDAY / 60).is_empty());
        let sat = offered(t, 5, SATURDAY / 60);
        assert!(!sat.is_empty());
        // Category 5 ("Exotic Gear", quantity 1 of 12) contributes exactly one sale.
        let cat5 = t.category_sales(5, 5);
        assert_eq!(cat5.len(), 12);
        assert_eq!(cat5.iter().filter(|s| sat.contains(s)).count(), 1);
        // Same stock all weekend; a new pick the next week (seeded by the period).
        assert_eq!(sat, offered(t, 5, SATURDAY / 60 + 600));
        assert_ne!(sat, offered(t, 5, SATURDAY / 60 + 7 * 1440));
    }

    #[test]
    fn the_bounty_tracker_rotates_daily_and_its_collect_sales_are_always_open() {
        let t = tables();
        let today = offered(t, 7, WEDNESDAY / 60);
        let tomorrow = offered(t, 7, WEDNESDAY / 60 + 1440);
        assert_ne!(today, tomorrow);
        let available = t.category_sales(7, 17); // "Available Bounties", 3 of 19
        assert_eq!(available.iter().filter(|s| today.contains(s)).count(), 3);
        let collect = t.category_sales(7, 14); // "Collect Bounty", quantity 0
        assert!(collect.iter().all(|s| today.contains(s)));
    }

    #[test]
    fn a_vendor_that_never_resets_offers_its_whole_catalogue() {
        let t = tables();
        let v = t.vendors.values().find(|v| v.reset_interval == 0 && v.sales > 0 && !v.inhibit_buying).unwrap();
        assert_eq!(offered(t, v.index, WEDNESDAY / 60).len(), v.sales as usize);
    }
}
