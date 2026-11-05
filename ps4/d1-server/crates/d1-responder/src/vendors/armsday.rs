// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The Gunsmith (Banshee-44) and Armsday.
//!
//! From the manifest: the Gunsmith resets on Wednesday 09:00 UTC (10080 / 900, Armsday). His
//! "Foundry Orders" category offers 5 of 25 weapon orders a week, each 2,500 Glimmer, with the
//! failure strings "Already Tested" and "Max Orders Placed". His "Armsday Delivery" categories sell
//! the finished weapon for the order item (sale 421: Arminius-D for one Häkke Arminius-D order).
//!
//! Rules:
//! - An order is a purchase (the order item goes to the weapon's bucket, as the client's sale table
//!   says), and is counted for the Gunsmith's current week.
//! - A delivery is a purchase priced in the order; it opens only after the next Armsday reset (an
//!   order placed this week is delivered next Armsday): OutsidePurchaseWindow before then.
//! - ⚠ STAND-IN: [`MAX_ORDERS_PER_WEEK`]. "Max Orders Placed" says there was a limit; its number is
//!   not in the data. One order per week until a source gives it.

use super::schedule::period;
use super::{refuse, Clock, Market, Refusal, Status};
use crate::store::{CharacterId, PlayerStore};

pub const MAX_ORDERS_PER_WEEK: u64 = 1;
pub const FOUNDRY_ORDERS: &str = "Foundry Orders";
pub const ARMSDAY_DELIVERY: &str = "Armsday Delivery";

fn week(m: &Market, vendor: u8, clock: Clock) -> i64 {
    m.vendor(vendor).map(|v| period(clock.minutes(), v.reset_interval, v.reset_offset)).unwrap_or(0)
}

/// Refuse what Armsday's rules refuse.
pub fn check<S: PlayerStore + ?Sized>(s: &S, m: &Market, chr: CharacterId, vendor: u8, sale: u16, title: &str, clock: Clock) -> Result<(), Refusal> {
    let w = week(m, vendor, clock);
    if title == FOUNDRY_ORDERS {
        let key = format!("armsday:orders:{w}");
        if s.counter(chr, &key) >= MAX_ORDERS_PER_WEEK {
            return refuse(Status::NoQuantity, "Max Orders Placed");
        }
    } else if title == ARMSDAY_DELIVERY {
        let order = m.sales.get(&(vendor, sale)).and_then(|x| x.costs.first()).map(|c| c.0).unwrap_or(0);
        let placed = s.counter(chr, &format!("armsday:order:{order}"));
        if placed == 0 {
            return refuse(Status::NoFunds, format!("no order {order} was placed"));
        }
        if (placed as i64) - 1 >= w {
            return refuse(Status::OutsidePurchaseWindow, format!("order {order} arrives next Armsday"));
        }
    }
    Ok(())
}

/// Record an order (after it was paid for).
pub fn record<S: PlayerStore + ?Sized>(s: &mut S, m: &Market, chr: CharacterId, vendor: u8, sale: u16, title: &str, clock: Clock) {
    if title != FOUNDRY_ORDERS {
        return;
    }
    let w = week(m, vendor, clock);
    let key = format!("armsday:orders:{w}");
    let n = s.counter(chr, &key);
    s.set_counter(chr, &key, n + 1);
    if let Some(item) = m.sales.get(&(vendor, sale)).map(|x| x.item) {
        // The week it was placed, + 1 so that 0 means "none".
        s.set_counter(chr, &format!("armsday:order:{item}"), (w + 1) as u64);
    }
}

#[cfg(test)]
mod tests {
    use crate::store::tests::CHR;
    use crate::store::{count, Owner};
    use crate::vendors::purchase::{buy, BuyOptions};
    use crate::vendors::testing::{market, rich, WEDNESDAY};
    use crate::vendors::{Clock, Status};

    #[test]
    fn an_order_is_delivered_the_next_armsday() {
        let m = market();
        let mut s = rich();
        let now = Clock { unix: WEDNESDAY, stamp: 1 };
        let free = BuyOptions { check_stock: false, refund_window: None };
        buy(&mut s, &m, CHR, 25, 396, now, free).unwrap();
        assert_eq!(count(&s, Owner::Character(CHR), 0x02, 4630), 1, "the order");
        assert_eq!(buy(&mut s, &m, CHR, 25, 397, now, free).unwrap_err().why, "Max Orders Placed");
        assert_eq!(buy(&mut s, &m, CHR, 25, 421, now, free).unwrap_err().status, Status::OutsidePurchaseWindow);
        let next = Clock { unix: WEDNESDAY + 7 * 86_400, stamp: 2 };
        let p = buy(&mut s, &m, CHR, 25, 421, next, free).unwrap();
        assert_eq!(p.item.content, 4964, "Arminius-D");
        assert_eq!(count(&s, Owner::Character(CHR), 0x02, 4630), 0, "the order is handed in");
    }
}
