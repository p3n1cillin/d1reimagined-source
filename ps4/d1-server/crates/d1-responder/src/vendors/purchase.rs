// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Buying: investment 901 `vendor_buy_item`, request `0x80801777` = `{u8 vendor, i16 sale, u64
//! stamp}` (decoded, runs sense69 and sense88).
//!
//! The client names only the vendor and the sale; everything else is the sale table and the
//! manifest's rules. In D1 several things were purchases at a vendor, and go through here:
//! - ordinary purchases (Eva's emblems, the quartermasters' gear);
//! - picking up a bounty: a Bounty Tracker "Available Bounties" sale priced 0 Glimmer;
//! - decoding an engram: a Cryptarch "Decode Engram" sale whose price is the engram
//!   ([`super::cryptarch`]);
//! - Armsday: a "Foundry Orders" sale, and its later "Armsday Delivery" sale priced in the order
//!   ([`super::armsday`]).
//!
//! Checks, in order, each refusing with the game's own `VendorItemStatus`:
//! 1. the sale exists, and the vendor is not a package or preview (`inhibitBuying`): NotAvailable;
//! 2. the vendor offers it now ([`super::rotation`]; Xûr off his weekend): OutsidePurchaseWindow;
//! 3. the sale's `requiredLevel` (manifest) against the character's level: NoProgression;
//! 4. one at a time: a bounty already held (UniquenessViolation), a cosmetic already owned
//!    (AlreadyOwned; Eva's emblem said "Can only hold one at a time", run sense69);
//! 5. Armsday's order and delivery rules;
//! 6. every cost, from the cost item's own bucket (the client's price check `0x26D6E0`): NoFunds;
//! 7. room in the item's bucket: NoInventorySpace.
//!
//! A refundable sale (refundPolicy ≠ 0 in the manifest: only Eververse and the Silver Dust vendor)
//! leaves a receipt that expires after its `refundLimit` (15 minutes). Other purchases leave none:
//! D1 refunded only Tess Everis's store.

use super::{armsday, cryptarch, refuse, rotation, Clock, Kind, Market, Refusal, Status};
use crate::store::{self, CharacterId, Item, Owner, PlayerStore, Receipt};

/// Cosmetic buckets where an instanced item is held once (ships, vehicles, emblems, shaders,
/// emotes, horns). Observed for emblems (sense69); the rest follow the same collection rule (LIKELY).
const HOLD_ONE: &[u8] = &[0x0c, 0x0d, 0x19, 0x1b, 0x1c, 0x1e];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Purchase {
    pub item: Item,
    pub bucket: u8,
    pub slot: usize,
    pub receipt: Option<Receipt>,
    pub what: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuyOptions {
    /// Refuse sales the vendor does not offer now.
    pub check_stock: bool,
    /// The refund window in the client's stamp units. `None` = the sale's `refundLimit` minutes
    /// read as seconds (⚠ the stamp's unit is not known; it is compared with the receipt's expiry).
    pub refund_window: Option<u64>,
}

impl Default for BuyOptions {
    fn default() -> Self {
        BuyOptions { check_stock: true, refund_window: None }
    }
}

pub fn character<S: PlayerStore + ?Sized>(s: &S, chr: CharacterId) -> Result<store::Character, Refusal> {
    s.characters().into_iter().find(|c| c.id == chr)
        .ok_or_else(|| Refusal { status: Status::UnknownError, why: format!("no character {chr:#x}") })
}

pub fn buy<S: PlayerStore + Clone>(s: &mut S, m: &Market, chr: CharacterId, vendor: u8, sale: u16, clock: Clock, opts: BuyOptions) -> Result<Purchase, Refusal> {
    let Some(sold) = m.sales.get(&(vendor, sale)).cloned() else {
        return refuse(Status::NotAvailable, format!("vendor {vendor} has no sale {sale}"));
    };
    let c = character(s, chr)?;
    let v = m.vendor(vendor);
    if v.map(|v| v.inhibit_buying).unwrap_or(false) {
        return refuse(Status::NotAvailable, format!("{} is a package or preview, not a store", v.unwrap().name));
    }
    if opts.check_stock && v.is_some() && !rotation::offered(m.t, vendor, clock.minutes()).contains(&sale) {
        let st = if m.kind(vendor) == Kind::Xur { Status::OutsidePurchaseWindow } else { Status::NotAvailable };
        return refuse(st, format!("vendor {vendor} does not offer sale {sale} this period"));
    }
    let rule = m.t.rules.get(&(vendor, sale)).copied();
    if let Some(r) = rule {
        if r.required_level > c.level {
            return refuse(Status::NoProgression, format!("sale {vendor}.{sale} requires level {} (the character is {})", r.required_level, c.level));
        }
    }
    if m.t.decode_sales.contains_key(&(vendor, sale)) {
        return cryptarch::decode(s, m, chr, vendor, sale, &sold);
    }
    let title = m.t.category_title(vendor, sale).to_string();
    let mut work = s.clone();
    if m.kind(vendor) == Kind::Gunsmith {
        armsday::check(&work, m, chr, vendor, sale, &title, clock)?;
    }
    let row = m.t.items.get(&sold.item).copied();
    let owner = store::owner_of(sold.bucket, chr);
    let is_bounty = sold.bucket == store::BOUNTIES || row.map(|r| r.item_type == 4).unwrap_or(false);
    if (is_bounty || sold.bucket == store::QUESTS) && store::count(&work, owner, sold.bucket, sold.item) > 0 {
        return refuse(Status::UniquenessViolation, format!("{} is already held", sold.item));
    }
    if sold.instanced && HOLD_ONE.contains(&sold.bucket) && held_anywhere(&work, chr, sold.item) {
        return refuse(Status::AlreadyOwned, format!("{} can only be held one at a time", sold.item));
    }
    for &(ci, q, b) in &sold.costs {
        if q == 0 {
            continue;
        }
        let o = store::owner_of(b, chr);
        let have = store::count(&work, o, b, ci);
        if have < u64::from(q) {
            return refuse(Status::NoFunds, format!("{ci} (bucket {b:#x}): have {have}, price {q}"));
        }
        store::take(&mut work, o, b, ci, q).map_err(|e| Refusal { status: Status::NoFunds, why: e })?;
    }
    if sold.bucket == store::TEMPORARY {
        // A "Collect" sale: the price is a finished bounty or quest item, and the item is a
        // temporary reward pseudo-item (bucket 0x13 holds one slot). Nothing is stored: the rewards
        // of what was handed in (and of the pseudo-item itself) are granted (`bounties::grant`).
        let mut rewards = Vec::new();
        for &(ci, _, _) in &sold.costs {
            if let Some(b) = m.t.bounties.get(&ci) {
                if !crate::bounties::complete(&work, m.t, chr, ci) {
                    return refuse(Status::NoUnlock, format!("{ci}'s objectives are not complete"));
                }
                rewards.extend(b.rewards.iter().copied());
            }
        }
        if let Some(b) = m.t.bounties.get(&sold.item) {
            rewards.extend(b.rewards.iter().copied());
        }
        let granted = crate::bounties::grant(&mut work, m.t, chr, &rewards).map_err(|e| Refusal { status: Status::NoInventorySpace, why: e })?;
        *s = work;
        return Ok(Purchase { item: Item::stack(sold.item, 1), bucket: sold.bucket, slot: 0, receipt: None,
                             what: format!("vendor {vendor} sale {sale}: handed in {:?}; rewards {granted}", sold.costs) });
    }
    let item = if sold.instanced {
        Item::instanced(sold.item, work.mint_soid(), 0)
    } else {
        Item::stack(sold.item, 1)
    };
    let max = row.map(|r| r.max_stack).unwrap_or(0);
    let slot = store::give(&mut work, owner, sold.bucket, item.clone(), max)
        .map_err(|e| Refusal { status: Status::NoInventorySpace, why: e })?;
    let receipt = match rule {
        Some(r) if r.refund_policy != 0 => {
            let window = opts.refund_window.unwrap_or(u64::from(r.refund_limit) * 60);
            let rc = Receipt { vendor, sale, item: item.clone(), bucket: sold.bucket, costs: sold.costs.clone(),
                               bought: clock.stamp, expires: clock.stamp.saturating_add(window) };
            let mut all = work.receipts(chr);
            all.retain(|x| x.expires > clock.stamp);
            all.push(rc.clone());
            work.set_receipts(chr, all);
            Some(rc)
        }
        _ => None,
    };
    if m.kind(vendor) == Kind::Gunsmith {
        armsday::record(&mut work, m, chr, vendor, sale, &title, clock);
    }
    *s = work;
    Ok(Purchase {
        what: format!("vendor {vendor} sale {sale}: item {} -> bucket {:#x} slot {slot}; costs {:?}{}", sold.item, sold.bucket, sold.costs,
                      if receipt.is_some() { "; refundable" } else { "" }),
        item, bucket: sold.bucket, slot, receipt,
    })
}

fn held_anywhere<S: PlayerStore + ?Sized>(s: &S, chr: CharacterId, content: u16) -> bool {
    HOLD_ONE.iter().any(|b| store::count(s, Owner::Character(chr), *b, content) > 0)
        || store::count(s, Owner::Account, store::VAULT_GENERAL, content) > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::CHR;
    use crate::store::{count, Owner};
    use crate::vendors::testing::{market, rich, SATURDAY, WEDNESDAY};

    fn at(unix: i64) -> Clock {
        Clock { unix, stamp: 0xB11 }
    }

    /// Eva's emblems rotate (12 of 46 a day); these tests buy a known sale whatever the day's pick.
    const ANY_STOCK: BuyOptions = BuyOptions { check_stock: false, refund_window: None };

    #[test]
    fn evas_command_emblem_costs_570_glimmer_and_leaves_no_receipt() {
        let m = market();
        let mut s = rich();
        let p = buy(&mut s, &m, CHR, 6, 23, at(WEDNESDAY), ANY_STOCK).unwrap();
        assert_eq!(p.item.content, 840);
        assert_eq!(p.bucket, 0x19);
        assert_ne!(p.item.instance, 0);
        assert_eq!(count(&s, Owner::Account, 0x1f, 869), 24_430);
        assert!(p.receipt.is_none(), "Eva's sales are not refundable");
        assert!(s.receipts(CHR).is_empty());
        let again = buy(&mut s, &m, CHR, 6, 23, at(WEDNESDAY), ANY_STOCK).unwrap_err();
        assert_eq!(again.status, Status::AlreadyOwned, "Can only hold one at a time");
        assert_eq!(count(&s, Owner::Account, 0x1f, 869), 24_430, "a refused purchase changes nothing");
    }

    #[test]
    fn an_eververse_emote_leaves_a_15_minute_receipt() {
        let m = market();
        let mut s = rich();
        let p = buy(&mut s, &m, CHR, 12, 60, at(WEDNESDAY), BuyOptions { check_stock: false, refund_window: None }).unwrap();
        let r = p.receipt.unwrap();
        assert_eq!(r.expires, 0xB11 + 15 * 60);
        assert_eq!(s.receipts(CHR).len(), 1);
        assert_eq!(count(&s, Owner::Account, 0x22, 873), 1_500, "Air Guitar is 500 Silver");
    }

    #[test]
    fn no_funds_and_no_stock_are_refused_with_the_games_status() {
        let m = market();
        let mut poor = crate::store::tests::memory();
        assert_eq!(buy(&mut poor, &m, CHR, 6, 23, at(WEDNESDAY), ANY_STOCK).unwrap_err().status, Status::NoFunds);
        let mut s = rich();
        let xur_sale = *crate::tables::tables().category_sales(5, 5).first().unwrap();
        assert_eq!(buy(&mut s, &m, CHR, 5, xur_sale, at(WEDNESDAY), BuyOptions::default()).unwrap_err().status,
                   Status::OutsidePurchaseWindow, "Xûr is not in the Tower on a Wednesday");
        assert_eq!(buy(&mut s, &m, CHR, 6, 999, at(WEDNESDAY), BuyOptions::default()).unwrap_err().status, Status::NotAvailable);
        assert_eq!(buy(&mut s, &m, CHR, 52, 0, at(WEDNESDAY), BuyOptions::default()).unwrap_err().status,
                   Status::NotAvailable, "a package vendor is not a store");
    }

    #[test]
    fn xur_sells_his_weekend_stock_for_strange_coins() {
        let m = market();
        let mut s = rich();
        let t = crate::tables::tables();
        let offered = rotation::offered(t, 5, SATURDAY / 60);
        // Find an offered sale priced only in what this Guardian holds.
        let sale = offered.iter().copied().find(|k| {
            let sale = &m.sales[&(5, *k)];
            t.rules[&(5, *k)].required_level <= 40 && !sale.costs.is_empty()
                && sale.costs.iter().all(|c| [869, 872, 873, 4548].contains(&c.0) && c.1 <= 50)
        });
        if let Some(k) = sale {
            buy(&mut s, &m, CHR, 5, k, at(SATURDAY), BuyOptions::default()).unwrap();
        }
    }

    /// Eva offers 12 of her 46 emblems a day (category 0, quantity 12): a sale off today's list is
    /// refused when stock is checked.
    #[test]
    fn evas_emblems_rotate_daily() {
        let m = market();
        let t = crate::tables::tables();
        let today = rotation::offered(t, 6, WEDNESDAY / 60);
        let emblems = t.category_sales(6, 0);
        assert_eq!(emblems.iter().filter(|k| today.contains(k)).count(), 12);
        let off = *emblems.iter().find(|k| !today.contains(k)).unwrap();
        let mut s = rich();
        assert_eq!(buy(&mut s, &m, CHR, 6, off, at(WEDNESDAY), BuyOptions::default()).unwrap_err().status, Status::NotAvailable);
    }

    #[test]
    fn the_required_level_is_enforced() {
        let m = market();
        let mut s = rich();
        s.characters[0].level = 5;
        let xur_sale = *rotation::offered(crate::tables::tables(), 5, SATURDAY / 60).iter()
            .find(|k| crate::tables::tables().rules[&(5, **k)].required_level > 5).unwrap();
        assert_eq!(buy(&mut s, &m, CHR, 5, xur_sale, at(SATURDAY), BuyOptions::default()).unwrap_err().status, Status::NoProgression);
    }

    #[test]
    fn a_bounty_is_picked_up_once() {
        let m = market();
        let mut s = rich();
        let t = crate::tables::tables();
        let offered = rotation::offered(t, 7, WEDNESDAY / 60);
        let sale = *t.category_sales(7, 17).iter().find(|k| offered.contains(k)).unwrap();
        let p = buy(&mut s, &m, CHR, 7, sale, at(WEDNESDAY), BuyOptions::default()).unwrap();
        assert_eq!(p.bucket, crate::store::BOUNTIES);
        assert_eq!(buy(&mut s, &m, CHR, 7, sale, at(WEDNESDAY), BuyOptions::default()).unwrap_err().status, Status::UniquenessViolation);
    }
}
