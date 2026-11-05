// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Selling (903 `vendor_sell_item`) and buying back (902 `vendor_buyback_item`): the Vault and the
//! Postmaster.
//!
//! D1's vendors did not buy gear (value back came from dismantling). In the manifest the **Vault**
//! (892630493, "Take") and the **Postmaster** (2021251983 / 2762206170, "Collect") are vendors with
//! `acceptedItems`: each accepted character bucket and the vault bucket it goes to. So giving an item
//! to the Vault is a sale to it, and the Postmaster's collect is one too. The 903 request
//! (`0x80801734`, decoded by a codec round trip) carries the item `{instanced, instance, content}`,
//! a quantity and the vendor; 902 (`0x8080180d`: the same item reference and an i16) is read as
//! taking an item back out of the Vault (LIKELY: Blam Network's dictionary names 902
//! `vendor_buyback_item`, and the vault is the one vendor that keeps what it is given).
//!
//! ⚠ No run has sent either yet (the 30 October note: no screen was found that sets action 1), and
//! the Vault's and Postmaster's row indices in the client's vendor table are open
//! ([`super::Market`]). Selling to any other vendor is refused: `Unsellable`.
//!
//! Rules (the D1 API's `TransferStatuses`): an equipped item cannot be moved (ItemIsEquipped), nor a
//! non-transferrable one (NotTransferrable), nor into a full bucket (NoRoomInDestination).

use super::{refuse, Kind, Market, Refusal, Status};
use crate::store::{self, CharacterId, Item, Owner, PlayerStore, VAULT_ARMOR, VAULT_GENERAL, VAULT_WEAPONS};

/// Where the Vault keeps an item of a character bucket: the manifest Vault's `acceptedItems`
/// (weapons to Weapons, armour to Armor, the rest listed to General), in client ordinals.
pub fn vault_bucket(ordinal: u8) -> Option<u8> {
    Some(match ordinal {
        0x02..=0x04 => VAULT_WEAPONS,
        0x07..=0x0b => VAULT_ARMOR,
        // Materials, Consumables, Ships, Vehicle, Ghost, Emblems, Shaders, Artifacts, Emotes,
        // Sparrow Horn, Ornaments.
        0x05 | 0x01 | 0x0c | 0x0d | 0x0e | 0x19 | 0x1b | 0x24 | 0x1c | 0x1e | 0x1d => VAULT_GENERAL,
        _ => return None,
    })
}

/// The Postmaster's buckets (Lost Items, Messages; Special Orders on the account).
pub const POSTMASTER_BUCKETS: &[u8] = &[0x10, 0x14, 0x15];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    pub content: u16,
    pub instance: u64,
    pub quantity: u32,
    pub from: (Owner, u8, usize),
    pub to: (Owner, u8, usize),
}

/// The item a request names, on the character (or in the account buckets for `account`).
fn locate<S: PlayerStore + ?Sized>(s: &S, chr: CharacterId, instanced: bool, instance: u64, content: i16, among: &[(Owner, u8)]) -> Option<(Owner, u8, usize, Item)> {
    if instanced && instance != 0 {
        let hit = store::find_instance(s, chr, instance)?;
        return among.iter().any(|(o, b)| *o == hit.0 && *b == hit.1).then_some(hit);
    }
    among.iter().find_map(|&(o, b)| store::find_content(s, o, b, content as u16).map(|(i, it)| (o, b, i, it)))
}

fn do_move<S: PlayerStore + ?Sized>(s: &mut S, m: &Market, from: (Owner, u8, usize, Item), to: (Owner, u8), quantity: u32) -> Result<Moved, Refusal> {
    let (fo, fb, fi, it) = from;
    let q = if it.instance != 0 { it.quantity.max(1) } else if quantity == 0 { it.quantity } else { quantity.min(it.quantity) };
    let moving = if it.instance != 0 { it.clone() } else { Item::stack(it.content, q) };
    let max = m.t.items.get(&it.content).map(|r| r.max_stack).unwrap_or(0);
    if it.instance != 0 || q == it.quantity {
        s.set_slot(fo, fb, fi, None).map_err(|e| Refusal { status: Status::UnknownError, why: e.to_string() })?;
    } else {
        let mut left = it.clone();
        left.quantity -= q;
        s.set_slot(fo, fb, fi, Some(left)).map_err(|e| Refusal { status: Status::UnknownError, why: e.to_string() })?;
    }
    let slot = store::give(s, to.0, to.1, moving, max).map_err(|e| Refusal { status: Status::NoInventorySpace, why: format!("NoRoomInDestination: {e}") })?;
    Ok(Moved { content: it.content, instance: it.instance, quantity: q, from: (fo, fb, fi), to: (to.0, to.1, slot) })
}

/// 903: give an item to a vendor.
pub fn sell<S: PlayerStore + Clone>(s: &mut S, m: &Market, chr: CharacterId, vendor: u8, instanced: bool, instance: u64, content: i16, quantity: u32) -> Result<Moved, Refusal> {
    let mut work = s.clone();
    let me = Owner::Character(chr);
    let moved = match m.kind(vendor) {
        Kind::Vault => {
            let among: Vec<(Owner, u8)> = store::buckets(0).filter(|b| vault_bucket(b.ordinal).is_some()).map(|b| (me, b.ordinal)).collect();
            let Some(hit) = locate(&work, chr, instanced, instance, content, &among) else {
                return refuse(Status::NotAvailable, format!("{instance:#x} / {content} is not in a bucket the Vault takes"));
            };
            if hit.3.instance != 0 && work.equipped(chr).contains(&hit.3.instance) {
                return refuse(Status::Unsellable, "ItemIsEquipped");
            }
            if m.t.items.get(&hit.3.content).map(|r| !r.transferrable()).unwrap_or(false) {
                return refuse(Status::Unsellable, "NotTransferrable");
            }
            let to = vault_bucket(hit.1).unwrap();
            do_move(&mut work, m, hit, (Owner::Account, to), quantity)?
        }
        Kind::Postmaster => {
            let among: Vec<(Owner, u8)> = POSTMASTER_BUCKETS.iter().map(|&b| (store::owner_of(b, chr), b)).collect();
            let Some(hit) = locate(&work, chr, instanced, instance, content, &among) else {
                return refuse(Status::NotAvailable, format!("the Postmaster holds no {instance:#x} / {content}"));
            };
            let Some(home) = m.t.items.get(&hit.3.content).map(|r| r.bucket) else {
                return refuse(Status::UnknownError, format!("{} has no bucket in the item table", hit.3.content));
            };
            do_move(&mut work, m, hit, (store::owner_of(home, chr), home), quantity)?
        }
        _ => {
            let name = m.vendor(vendor).map(|v| v.name.as_str()).unwrap_or("?");
            return refuse(Status::Unsellable, format!("{name} does not buy items (D1 gave value back by dismantling)"));
        }
    };
    *s = work;
    Ok(moved)
}

/// 902: take an item back from a vendor (the Vault).
pub fn buyback<S: PlayerStore + Clone>(s: &mut S, m: &Market, chr: CharacterId, vendor: u8, instanced: bool, instance: u64, content: i16, quantity: u32) -> Result<Moved, Refusal> {
    if m.kind(vendor) != Kind::Vault {
        return refuse(Status::NotAvailable, format!("vendor {vendor} keeps nothing to buy back"));
    }
    let mut work = s.clone();
    let among = [(Owner::Account, VAULT_WEAPONS), (Owner::Account, VAULT_ARMOR), (Owner::Account, VAULT_GENERAL)];
    let Some(hit) = locate(&work, chr, instanced, instance, content, &among) else {
        return refuse(Status::NotAvailable, format!("the Vault holds no {instance:#x} / {content}"));
    };
    let Some(home) = m.t.items.get(&hit.3.content).map(|r| r.bucket) else {
        return refuse(Status::UnknownError, format!("{} has no bucket in the item table", hit.3.content));
    };
    let moved = do_move(&mut work, m, hit, (store::owner_of(home, chr), home), quantity)?;
    *s = work;
    Ok(moved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::CHR;
    use crate::store::{count, give};
    use crate::vendors::testing::{market, rich};

    #[test]
    fn a_rifle_goes_into_the_vault_and_comes_back() {
        let m = market();
        let mut s = rich();
        give(&mut s, Owner::Character(CHR), 0x02, Item::instanced(3598, 0x30b, 0), 1).unwrap();
        let mv = sell(&mut s, &m, CHR, 1, true, 0x30b, -1, 0).unwrap();
        assert_eq!(mv.to.0, Owner::Account);
        assert_eq!(mv.to.1, VAULT_WEAPONS);
        assert!(store::find_content(&s, Owner::Character(CHR), 0x02, 3598).is_none());
        let back = buyback(&mut s, &m, CHR, 1, true, 0x30b, -1, 0).unwrap();
        assert_eq!((back.to.0, back.to.1), (Owner::Character(CHR), 0x02));
    }

    #[test]
    fn an_equipped_item_stays_and_other_vendors_do_not_buy() {
        let m = market();
        let mut s = rich();
        give(&mut s, Owner::Character(CHR), 0x02, Item::instanced(3598, 0x401, 0), 1).unwrap();
        s.set_equipped(CHR, 6, 0x401).unwrap();
        assert_eq!(sell(&mut s, &m, CHR, 1, true, 0x401, -1, 0).unwrap_err().why, "ItemIsEquipped");
        assert_eq!(sell(&mut s, &m, CHR, 6, true, 0x401, -1, 0).unwrap_err().status, Status::Unsellable);
    }

    #[test]
    fn part_of_a_stack_goes_to_the_vault() {
        let m = market();
        let mut s = rich();
        let mv = sell(&mut s, &m, CHR, 1, false, 0, 872, 20).unwrap();
        assert_eq!(mv.quantity, 20);
        assert_eq!(count(&s, Owner::Character(CHR), 0x05, 872), 30);
        assert_eq!(count(&s, Owner::Account, VAULT_GENERAL, 872), 20);
    }

    #[test]
    fn the_postmaster_hands_over_a_lost_item() {
        let m = market();
        let mut s = rich();
        give(&mut s, Owner::Character(CHR), store::LOST_ITEMS, Item::instanced(3598, 0x777, 0), 1).unwrap();
        let mv = sell(&mut s, &m, CHR, 4, true, 0x777, -1, 0).unwrap();
        assert_eq!(mv.to.1, 0x02);
        for i in 0..10 {
            give(&mut s, Owner::Character(CHR), 0x02, Item::instanced(3598, 0x900 + i, 0), 1).ok();
        }
        give(&mut s, Owner::Character(CHR), store::LOST_ITEMS, Item::instanced(3598, 0x778, 0), 1).unwrap();
        assert_eq!(sell(&mut s, &m, CHR, 4, true, 0x778, -1, 0).unwrap_err().status, Status::NoInventorySpace);
    }
}
