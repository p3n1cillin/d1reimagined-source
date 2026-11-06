// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! ⭐ Stream I's wire hooks: the glue between the investment messages and the social services.
//! Everything here is behind `D1_SOCIAL=1` (default off: the lab's routes behave as before).
//!
//! | Message | With `D1_SOCIAL=1` |
//! |---|---|
//! | 901 buy | `vendors::purchase::buy` (rules, decodes, bounties, Armsday) instead of `vendor::apply` |
//! | 402 dismantle | as before; with `D1_DISMANTLE_PLACEHOLDER=1` too, the labelled placeholder yields |
//! | 403 equip | `inventory_ops::equip` (one exotic, class gear) |
//! | 404 unequip | `inventory_ops::unequip` |
//! | 405 transfer | `inventory_ops::transfer`, its direction from where the item is (the u8s are open) |
//! | 902 buyback | `vendors::sell::buyback` (the Vault) |
//! | 903 sell | `vendors::sell::sell` (the Vault, the Postmaster; others refuse) |
//! | 905 refund | `vendors::refund::refund` over the purchase state's refund records |
//!
//! The services run on a `store::WireStore` of `vendor.rs`'s purchase state (`vendor::with_wire_store`),
//! which is published like a purchase, so the player store (#307, `D1_STORE=1`) persists the profile,
//! the account and the refund records through `note_vendor`. What the served objects can't hold
//! (XP and reputation from rewards, objective progress) waits in a side [`MemoryStore`];
//! [`bridge_progressions`] hands XP and reputation to the player store (`add_xp`,
//! `add_reputation`). That is the whole adapter between the two traits.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use crate::store::{Character, MemoryStore, Receipt, WireStore};
use crate::vendor::{RefundRequest, Sale, SellRequest};
use crate::vendors::{purchase::BuyOptions, Clock, Market};

pub fn enabled() -> bool {
    std::env::var("D1_SOCIAL").is_ok_and(|v| v == "1")
}

fn env_num<T: std::str::FromStr>(k: &str, d: T) -> T {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

static SIDE: Mutex<Option<MemoryStore>> = Mutex::new(None);

/// The served character: `D1_CHARACTER_SOID`, class `D1_CHARACTER_CLASS` (default 0, the served
/// Titan) and level `D1_CHARACTER_LEVEL` (default 40, the geared Guardian's).
pub fn served_character() -> Character {
    Character {
        id: crate::env_soid("D1_CHARACTER_SOID", 0xD100_0000_0000_0101),
        race: 0, gender: 1,
        class: env_num("D1_CHARACTER_CLASS", 0),
        level: env_num("D1_CHARACTER_LEVEL", 40),
        customization: Vec::new(),
    }
}

pub fn wire_store(account: BTreeMap<u16, (u16, u32)>, account_instances: BTreeMap<u16, u64>, profile: Vec<u8>, next_soid: u64) -> WireStore {
    let side = SIDE.lock().ok().and_then(|mut g| g.take()).unwrap_or_else(MemoryStore::new);
    WireStore { character: served_character(), account, account_instances, profile, next_soid, side }
}

pub fn keep_side(side: MemoryStore) {
    if let Ok(mut g) = SIDE.lock() {
        *g = Some(side);
    }
}

/// The player store's refund record as a receipt (buckets from the sale table).
pub fn receipt_from(r: &crate::player_store::Refund, sales: &HashMap<(u8, u16), Sale>) -> Receipt {
    let sale = sales.get(&(r.vendor, r.sale));
    let bucket_of = |ci: u16| sale.and_then(|s| s.costs.iter().find(|c| c.0 == ci)).map(|c| c.2).unwrap_or(0x1f);
    Receipt {
        vendor: r.vendor, sale: r.sale,
        item: if r.instance != 0 { crate::store::Item::instanced(r.item, r.instance, 0) } else { crate::store::Item::stack(r.item, 1) },
        bucket: sale.map(|s| s.bucket).unwrap_or(0),
        costs: r.costs.iter().map(|&(c, q)| (c, q, bucket_of(c))).collect(),
        bought: r.bought, expires: r.expires,
    }
}

pub fn refund_from(r: &Receipt) -> crate::player_store::Refund {
    crate::player_store::Refund {
        vendor: r.vendor, sale: r.sale, item: r.item.content, instance: r.item.instance,
        costs: r.costs.iter().map(|&(c, q, _)| (c, q)).collect(), bought: r.bought, expires: r.expires,
    }
}

/// Stock is enforced only with `D1_SOCIAL_STOCK=1`: how the client learns a vendor's current stock
/// is not on the wire yet (the vendor screens list the whole catalogue today), so a server-side
/// rotation must not refuse what the client shows.
pub fn buy_options(refund_window: Option<u64>) -> BuyOptions {
    BuyOptions { check_stock: std::env::var("D1_SOCIAL_STOCK").is_ok_and(|v| v == "1"), refund_window }
}

pub fn market(sales: &HashMap<(u8, u16), Sale>) -> Market<'_> {
    Market::new(crate::tables::tables(), sales)
}

pub fn clock(stamp: u64) -> Clock {
    let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    Clock { unix, stamp }
}

/// The labelled placeholder dismantle yields, with `D1_SOCIAL=1` and `D1_DISMANTLE_PLACEHOLDER=1`.
pub fn placeholder_yields(content: u16) -> Option<Vec<(u16, u32, u8)>> {
    if !enabled() || !std::env::var("D1_DISMANTLE_PLACEHOLDER").is_ok_and(|v| v == "1") {
        return None;
    }
    let set = crate::tables::tables().items.get(&content)?.reward_set;
    let y = crate::inventory_ops::placeholder_yields(set);
    (!y.is_empty()).then(|| y.to_vec())
}

/// A decoded request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Unequip(u64),
    /// 405 `0x808018ce` {u8, u8, u64, i16, i32}: the two u8s are not identified.
    Transfer { a: u8, b: u8, instance: u64, content: i16, quantity: i32 },
    /// 902 `0x8080180d` {bool, u64, i16} + i16 (read as the vendor).
    Buyback { instanced: bool, instance: u64, content: i16, vendor: i16 },
    Sell(SellRequest),
    Refund(RefundRequest),
}

fn le_u64(o: &[u8], at: usize) -> u64 {
    o.get(at..at + 8).map(|b| u64::from_le_bytes(b.try_into().unwrap())).unwrap_or(0)
}
fn le_i16(o: &[u8], at: usize) -> i16 {
    o.get(at..at + 2).map(|b| i16::from_le_bytes([b[0], b[1]])).unwrap_or(0)
}

/// Read a decoded request object (`d1-oracle object --decode-file … --out-obj`). The offsets of 404
/// (one u64) and 903 are the codec's (round trips); those of 405 and 902 assume the codec's natural
/// alignment, as 903's turned out to have: ⚠ to confirm with `d1-oracle object --tag <tag>` round
/// trips once a run sends one.
pub fn parse(id: u16, obj: &[u8]) -> Option<Op> {
    Some(match id {
        404 => Op::Unequip(le_u64(obj, 0)),
        405 => Op::Transfer {
            a: *obj.first()?, b: *obj.get(1)?, instance: le_u64(obj, 8), content: le_i16(obj, 0x10),
            quantity: obj.get(0x14..0x18).map(|b| i32::from_le_bytes(b.try_into().unwrap())).unwrap_or(0),
        },
        902 => Op::Buyback { instanced: *obj.first()? != 0, instance: le_u64(obj, 8), content: le_i16(obj, 0x10), vendor: le_i16(obj, 0x18) },
        903 => Op::Sell(crate::vendor::sell_request(obj)?),
        _ => return None,
    })
}

/// Decode a request body: 905 from the bitstream, the others through the client's codec.
pub fn decode(id: u16, oracle: &str, req: &[u8]) -> anyhow::Result<Op> {
    if id == 905 {
        return crate::vendor::refund_request(req).map(Op::Refund).ok_or_else(|| anyhow::anyhow!("905: {} bytes is too short", req.len()));
    }
    let tag = match id {
        404 => "0x8080189a",
        405 => "0x808018ce",
        902 => "0x8080180d",
        903 => "0x80801734",
        _ => anyhow::bail!("{id} is not a social message"),
    };
    let obj = crate::vendor::decode_object(oracle, tag, req, &format!("social-{id}"))?;
    let op = parse(id, &obj).ok_or_else(|| anyhow::anyhow!("{id}: a short object ({} bytes)", obj.len()))?;
    tracing::warn!("      ⭐ SOCIAL ({id}) decoded: {op:?}");
    Ok(op)
}

/// Apply a decoded request to the served state.
pub fn apply(w: &mut WireStore, sales: &HashMap<(u8, u16), Sale>, op: Op) -> Result<String, String> {
    use crate::inventory_ops::{transfer, unequip, Direction};
    let t = crate::tables::tables();
    let m = market(sales);
    let chr = w.character.id;
    match op {
        Op::Unequip(i) => unequip(w, chr, i).map(|x| format!("unequip: {x}")).map_err(|e| e.to_string()),
        Op::Transfer { instance, content, quantity, a, b } => {
            // The direction from where the item is now: on the character -> the vault, else back.
            let on_character = matches!(crate::store::find_instance(w, chr, instance), Some((crate::store::Owner::Character(_), ..)));
            let dir = if instance == 0 || on_character { Direction::ToVault } else { Direction::FromVault };
            transfer(w, t, chr, dir, instance, content as u16, quantity.max(0) as u32)
                .map(|x| format!("transfer (u8s {a}, {b}): {x}")).map_err(|e| e.to_string())
        }
        Op::Buyback { instanced, instance, content, vendor } => {
            crate::vendors::sell::buyback(w, &m, chr, vendor as u8, instanced, instance, content, 0)
                .map(|x| format!("buyback: {x:?}")).map_err(|e| e.to_string())
        }
        Op::Sell(r) => crate::vendors::sell::sell(w, &m, chr, r.vendor, r.instanced, r.instance, r.content, r.quantity.max(0) as u32)
            .map(|x| format!("sell: {x:?}")).map_err(|e| e.to_string()),
        Op::Refund(r) => crate::vendors::refund::refund(w, chr, r.instance, r.content, r.stamp)
            .map(|x| format!("refund: {x:?}")).map_err(|e| e.to_string()),
    }
}

/// Hand the rewards' XP and reputation to the player store (`D1_STORE=1`); kept in the side store
/// otherwise. Then the profile push carries them to the client (`player_store::push_profile`).
/// XP the store can't take yet (its profile isn't decoded) goes back to the side store for the
/// next bridge rather than being dropped; reputation waits in the store itself.
pub fn bridge_progressions() {
    let pending: Vec<((u64, u32), u32)> = {
        let Ok(mut g) = SIDE.lock() else { return };
        let Some(side) = g.as_mut() else { return };
        if side.progressions.is_empty() || !crate::player_store::enabled() {
            return;
        }
        side.progressions.drain().collect()
    };
    let t = crate::tables::tables();
    let mut kept = Vec::new();
    for ((chr, hash), amount) in pending {
        let r = crate::player_store::with_store(|s| {
            if hash == crate::bounties::EXPERIENCE {
                s.add_xp(amount).map(|g| format!("{g:?}"))
            } else if let Some(p) = t.progressions.get(&hash) {
                Ok(s.add_reputation(&p.name, amount))
            } else {
                Ok(format!("⛔ progression {hash} has no curve"))
            }
        });
        if !matches!(r, Some(Ok(_))) {
            kept.push(((chr, hash), amount));
        }
        tracing::warn!("      ⭐ SOCIAL rewards -> player store: {amount} of {hash}: {r:?}");
    }
    if !kept.is_empty() {
        if let Ok(mut g) = SIDE.lock() {
            let side = g.get_or_insert_with(MemoryStore::new);
            for (key, amount) in kept {
                *side.progressions.entry(key).or_default() += amount;
            }
        }
    }
    // Off this thread: the callers hold the vendor state's lock, and the push waits (up to 120 s)
    // for the vendor's own push to be delivered, which needs that lock.
    std::thread::spawn(|| {
        if let Err(e) = crate::player_store::push_profile("social rewards: XP and reputation") {
            tracing::warn!("      ⛔ SOCIAL rewards: the profile push failed ({e:#})");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_parse_at_the_codec_offsets() {
        let mut o = vec![0u8; 0x20];
        o[0] = 1;
        o[1] = 2;
        o[8..16].copy_from_slice(&0xD100_0000_0000_030Bu64.to_le_bytes());
        o[0x10..0x12].copy_from_slice(&3598i16.to_le_bytes());
        o[0x14..0x18].copy_from_slice(&5i32.to_le_bytes());
        assert_eq!(parse(405, &o), Some(Op::Transfer { a: 1, b: 2, instance: 0xD100_0000_0000_030B, content: 3598, quantity: 5 }));
        o[0x18..0x1A].copy_from_slice(&7i16.to_le_bytes());
        assert_eq!(parse(902, &o), Some(Op::Buyback { instanced: true, instance: 0xD100_0000_0000_030B, content: 3598, vendor: 7 }));
        assert_eq!(parse(404, &0x30Bu64.to_le_bytes()), Some(Op::Unequip(0x30B)));
        assert_eq!(parse(501, &o), None);
    }

    #[test]
    fn refund_records_round_trip_as_receipts() {
        let sales = crate::tables::fixture_sales();
        let r = crate::player_store::Refund { vendor: 12, sale: 60, item: 4490, instance: 0x601, costs: vec![(873, 500)], bought: 10, expires: 910 };
        let rc = receipt_from(&r, sales);
        assert_eq!(rc.bucket, 0x1c);
        assert_eq!(rc.costs, vec![(873, 500, 0x22)], "Silver goes back to its bucket");
        assert_eq!(refund_from(&rc), r);
    }

    /// A transfer on the served state: the rifle goes to the vault and back by two 405s.
    #[test]
    fn a_405_moves_the_item_by_where_it_is() {
        let mut w = crate::store::tests::wire();
        w.character = served_character();
        let chr = w.character.id;
        crate::store::give(&mut w, crate::store::Owner::Character(chr), 0x02, crate::store::Item::instanced(3598, 0x30b, 0), 1).unwrap();
        let sales = crate::tables::fixture_sales();
        let op = Op::Transfer { a: 0, b: 0, instance: 0x30b, content: 3598, quantity: 0 };
        apply(&mut w, sales, op.clone()).unwrap();
        assert_eq!(w.account_instances.get(&20), Some(&0x30b));
        apply(&mut w, sales, op).unwrap();
        assert!(w.account_instances.is_empty());
    }
}
