// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Refunds: investment 905 `vendor_refund_item`.
//!
//! The request (decoded from the bitstream, `vendor::refund_request`) names the item `{instanced,
//! instance, content}` and the client's stamp. D1 refunded only Tess Everis's store (Eververse and
//! its Silver Dust kiosk), within about 15 minutes (Bungie's FAQ, October 2015). The final manifest
//! says the same: only those two vendors have sales with a refund policy, all with `refundLimit` 15.
//! [`super::purchase`] leaves a receipt for exactly those sales.
//!
//! A refund: the receipt must exist and not have expired (its expiry is ahead of the request's
//! stamp, the comparison the client's refund screen `0xA205E0` makes), the item must still be held;
//! the item is removed (DeletesItem and RevokesLicense both take it back), every cost is given back
//! to its bucket, and the receipt is dropped.

use super::{refuse, Refusal, Status};
use crate::store::{self, CharacterId, PlayerStore};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refunded {
    pub content: u16,
    pub costs: Vec<(u16, u32, u8)>,
}

pub fn refund<S: PlayerStore + Clone>(s: &mut S, chr: CharacterId, instance: u64, content: i16, stamp: u64) -> Result<Refunded, Refusal> {
    let mut work = s.clone();
    let mut receipts = work.receipts(chr);
    let Some(at) = receipts.iter().position(|r| {
        if instance != 0 { r.item.instance == instance } else { i32::from(r.item.content) == i32::from(content) }
    }) else {
        return refuse(Status::NotAvailable, format!("no refund receipt for {instance:#x} / {content}"));
    };
    let r = receipts.remove(at);
    if r.expires <= stamp {
        return refuse(Status::OutsidePurchaseWindow, format!("the refund window closed at {:#x} (now {stamp:#x})", r.expires));
    }
    let owner = store::owner_of(r.bucket, chr);
    if r.item.instance != 0 {
        let Some((o, b, slot, _)) = store::find_instance(&work, chr, r.item.instance) else {
            return refuse(Status::NotAvailable, format!("{:#x} is no longer held", r.item.instance));
        };
        if work.equipped(chr).contains(&r.item.instance) {
            return refuse(Status::NotAvailable, format!("{:#x} is equipped", r.item.instance));
        }
        work.set_slot(o, b, slot, None).map_err(|e| Refusal { status: Status::UnknownError, why: e.to_string() })?;
    } else {
        store::take(&mut work, owner, r.bucket, r.item.content, r.item.quantity.max(1))
            .map_err(|e| Refusal { status: Status::NotAvailable, why: e })?;
    }
    for &(ci, q, b) in &r.costs {
        if q > 0 {
            store::give(&mut work, store::owner_of(b, chr), b, store::Item::stack(ci, q), 0)
                .map_err(|e| Refusal { status: Status::NoInventorySpace, why: e })?;
        }
    }
    work.set_receipts(chr, receipts);
    *s = work;
    Ok(Refunded { content: r.item.content, costs: r.costs })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::CHR;
    use crate::store::{count, Owner};
    use crate::vendors::purchase::{buy, BuyOptions};
    use crate::vendors::testing::{market, rich, WEDNESDAY};
    use crate::vendors::Clock;

    const FREE: BuyOptions = BuyOptions { check_stock: false, refund_window: None };

    #[test]
    fn an_eververse_purchase_is_refunded_within_15_minutes() {
        let m = market();
        let mut s = rich();
        let p = buy(&mut s, &m, CHR, 12, 60, Clock { unix: WEDNESDAY, stamp: 1000 }, FREE).unwrap();
        assert_eq!(count(&s, Owner::Account, 0x22, 873), 1_500);
        let r = refund(&mut s, CHR, p.item.instance, -1, 1000 + 14 * 60).unwrap();
        assert_eq!(r.content, 4490);
        assert_eq!(count(&s, Owner::Account, 0x22, 873), 2_000, "the Silver comes back");
        assert!(store::find_instance(&s, CHR, p.item.instance).is_none(), "the emote is gone");
        assert!(s.receipts(CHR).is_empty());
        assert_eq!(refund(&mut s, CHR, p.item.instance, -1, 1000).unwrap_err().status, Status::NotAvailable, "only once");
    }

    #[test]
    fn a_late_refund_is_refused_and_changes_nothing() {
        let m = market();
        let mut s = rich();
        let p = buy(&mut s, &m, CHR, 12, 60, Clock { unix: WEDNESDAY, stamp: 1000 }, FREE).unwrap();
        let e = refund(&mut s, CHR, p.item.instance, -1, 1000 + 15 * 60).unwrap_err();
        assert_eq!(e.status, Status::OutsidePurchaseWindow);
        assert_eq!(count(&s, Owner::Account, 0x22, 873), 1_500);
        assert!(store::find_instance(&s, CHR, p.item.instance).is_some());
    }

    #[test]
    fn an_ordinary_vendors_purchase_cannot_be_refunded() {
        let m = market();
        let mut s = rich();
        let p = buy(&mut s, &m, CHR, 6, 23, Clock { unix: WEDNESDAY, stamp: 1000 }, FREE).unwrap();
        assert_eq!(refund(&mut s, CHR, p.item.instance, -1, 1001).unwrap_err().status, Status::NotAvailable);
    }
}
