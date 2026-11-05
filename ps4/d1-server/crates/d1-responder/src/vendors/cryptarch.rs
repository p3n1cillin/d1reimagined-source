// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The Cryptarch: decoding an engram.
//!
//! In the client's sale table a decode is a 901 purchase like any other. Master Rahool's "Decode
//! Engram" categories (three Cryptarch rows: Tower, Reef, Felwinter Peak) hold one sale per engram
//! kind, priced in the engram itself; sale 52 of vendor 17 is item 4690 for one engram 4870 in
//! bucket 8 (Gauntlets). The sold item is a stand-in the server replaced with the decoded gear.
//!
//! Rules (each a fact of the game, or labelled):
//! - The engram is the price: it leaves its bucket (an engram sits in the slot it decodes into).
//! - The result goes to the sold item's bucket, for the character's class (manifest classType, or
//!   any-class items), at the sold item's tier (manifest tierType: an Exotic Engram gives an exotic).
//! - ⚠ STAND-IN: the outcome pool. The live servers chose from loot tables that no data holds; here
//!   the candidates are every equippable, instanced item of that bucket, tier and class in the
//!   client's item table, picked at random with a seed from the engram's instance and the character.
//! - ⚠ STAND-IN: the result's Light. Rise of Iron decoded near the Guardian's best Light for the
//!   slot; here it is the best Light held in that bucket (0 where the store does not know Light).
//! - No room in the bucket: NoInventorySpace (the engram stays).

use super::purchase::Purchase;
use super::{refuse, Market, Refusal, Status};
use crate::store::{self, CharacterId, Item, Owner, PlayerStore};
use crate::tables::{ItemAction, Tables};
use crate::vendor::Sale;

/// Engram items (manifest itemType 8).
const ITEM_TYPE_ENGRAM: u8 = 8;

/// The candidates an engram of `tier` for `bucket` decodes into, for `class`.
pub fn pool(t: &Tables, bucket: u8, tier: u8, class: u8) -> Vec<u16> {
    let mut v: Vec<u16> = t.items.iter().filter(|(_, r)| {
        r.bucket == bucket && r.tier == tier && (r.class == class || r.class == 3) && r.equippable() && r.instanced()
            && r.item_type != ITEM_TYPE_ENGRAM && r.action != ItemAction::Decrypt
    }).map(|(c, _)| *c).collect();
    v.sort_unstable();
    v
}

pub fn decode<S: PlayerStore + Clone>(s: &mut S, m: &Market, chr: CharacterId, vendor: u8, sale: u16, sold: &Sale) -> Result<Purchase, Refusal> {
    let c = super::purchase::character(s, chr)?;
    let Some(&(engram, q, eb)) = sold.costs.first() else {
        return refuse(Status::NotAvailable, format!("decode sale {vendor}.{sale} has no engram price"));
    };
    let mut work = s.clone();
    let owner = store::owner_of(eb, chr);
    let Some((eslot, held)) = store::find_content(&work, owner, eb, engram) else {
        return refuse(Status::NoFunds, format!("no engram {engram} in bucket {eb:#x}"));
    };
    store::take(&mut work, owner, eb, engram, q.max(1)).map_err(|e| Refusal { status: Status::NoFunds, why: e })?;
    let row = m.t.items.get(&sold.item).copied();
    let tier = row.map(|r| r.tier).unwrap_or(0);
    let bucket = sold.bucket;
    let candidates = pool(m.t, bucket, tier, c.class);
    if candidates.is_empty() {
        return refuse(Status::UnknownError, format!("no tier-{tier} item for bucket {bucket:#x}, class {}", c.class));
    }
    let seed = held.instance ^ chr.rotate_left(17) ^ (u64::from(engram) << 40) ^ eslot as u64;
    let content = candidates[super::pick(seed, candidates.len(), 1)[0]];
    let light = work.slots(Owner::Character(chr), bucket).iter().flatten().map(|i| i.light).max().unwrap_or(0);
    let item = Item::instanced(content, work.mint_soid(), light);
    let slot = store::give(&mut work, Owner::Character(chr), bucket, item.clone(), 1)
        .map_err(|e| Refusal { status: Status::NoInventorySpace, why: e })?;
    *s = work;
    Ok(Purchase {
        what: format!("decoded engram {engram} at vendor {vendor} sale {sale}: {content} (tier {tier}, one of {} candidates) -> bucket {bucket:#x} slot {slot}",
                      candidates.len()),
        item, bucket, slot, receipt: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::CHR;
    use crate::store::{count, give};
    use crate::vendors::purchase::{buy, BuyOptions};
    use crate::vendors::testing::{market, rich, WEDNESDAY};
    use crate::vendors::Clock;

    #[test]
    fn an_exotic_engram_decodes_into_an_exotic_gauntlet_for_the_class() {
        let m = market();
        let mut s = rich();
        give(&mut s, Owner::Character(CHR), 0x08, Item::instanced(4870, 0xE1, 0), 1).unwrap();
        let p = buy(&mut s, &m, CHR, 17, 52, Clock { unix: WEDNESDAY, stamp: 1 }, BuyOptions::default()).unwrap();
        assert_eq!(count(&s, Owner::Character(CHR), 0x08, 4870), 0, "the engram is spent");
        let r = crate::tables::tables().items[&p.item.content];
        assert_eq!((r.bucket, r.tier), (0x08, 6));
        assert!(r.class == 0 || r.class == 3, "a Titan's or any class's");
        assert_ne!(p.item.content, 4690, "the sold stand-in is replaced");
    }

    #[test]
    fn decoding_without_the_engram_is_refused() {
        let m = market();
        let mut s = rich();
        let e = buy(&mut s, &m, CHR, 17, 52, Clock { unix: WEDNESDAY, stamp: 1 }, BuyOptions::default()).unwrap_err();
        assert_eq!(e.status, Status::NoFunds);
    }

    #[test]
    fn every_decode_sale_has_a_pool_for_some_class() {
        let t = crate::tables::tables();
        let m = market();
        let mut empty = 0;
        for (&(v, s), _) in &t.decode_sales {
            let sold = &m.sales[&(v, s)];
            let tier = t.items.get(&sold.item).map(|r| r.tier).unwrap_or(0);
            if (0..3).all(|c| pool(t, sold.bucket, tier, c).is_empty()) {
                empty += 1;
            }
        }
        // Some decode sales are materials or consumables (no gear pool); most must have one.
        assert!(empty * 4 < t.decode_sales.len(), "{empty} of {} decode sales have no pool", t.decode_sales.len());
    }
}
