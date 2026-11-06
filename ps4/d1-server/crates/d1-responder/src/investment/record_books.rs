// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! 1801 (record reward redemption), 1802 (unseal a Record Book) and 1803 (item request action 14:
//! a table reward).
//!
//! The client sends 1801 from `cui::c_record_reward_redemption_request` slot 3 `0x9B0F40` (its
//! `+0x24` i16), 1802 from `cui::c_record_book_unseal_book_request_handler` slot 3 `0xA18100` (its
//! `+0x24` i8), and 1803 from the item request handler `0xA29D30` action 14 for a category-5 item:
//! `{entry +0x50, row +0x52, table +0x53}` of the content table at manager slot `+0x490`, after
//! `0x277340` checked the row's progression level (`+0x30`, row `+0x18`), its requirement list and
//! room for the reward item (it built the item from the entry's content index).
//!
//! **What is served:** each request is checked once per key and remembered in the character's
//! attributes (`investment.record_books.unsealed` / `.redeemed` / `.claimed`); a repeat is refused.
//! ⚠ **What is open:** the ordinals' tables (which book, which reward, which item an entry is) are
//! client content not exported here, and the Record Book state reaches the client only in pushed
//! objects whose fields are not decoded. So no item is granted and nothing is pushed: the reply
//! names the version the client already holds. `records.rewards` in `rewards::records` keeps the
//! manifest's book ranks and rewards for when the ordinals are mapped.

use serde_json::{json, Value};

use super::snapshot::Snapshot;
use super::{Refusal, Status};

const UNSEALED: &str = "investment.record_books.unsealed";
const REDEEMED: &str = "investment.record_books.redeemed";
const CLAIMED: &str = "investment.record_books.claimed";

fn once(s: &mut Snapshot, key: &str, item: Value, what: &str) -> Result<String, Refusal> {
    let mut list = s.attribute(key).and_then(Value::as_array).cloned().unwrap_or_default();
    if list.contains(&item) {
        return Err(Refusal::new(Status::NotAvailable, format!("{what} {item} was already done")));
    }
    list.push(item.clone());
    s.set_attribute(key, Value::Array(list));
    Ok(format!("{what} {item} remembered; its content mapping is open, so nothing is granted or pushed"))
}

pub fn unseal(s: &mut Snapshot, book: i8) -> Result<String, Refusal> {
    if book < 0 {
        return Err(Refusal::new(Status::Invalid, "no book"));
    }
    once(s, UNSEALED, json!(book), "book")
}

pub fn redeem(s: &mut Snapshot, reward: i16) -> Result<String, Refusal> {
    if reward < 0 {
        return Err(Refusal::new(Status::Invalid, "no reward"));
    }
    once(s, REDEEMED, json!(reward), "record reward")
}

pub fn claim(s: &mut Snapshot, entry: i16, row: i8, table: i8) -> Result<String, Refusal> {
    if entry < 0 || row < 0 || table < 0 {
        return Err(Refusal::new(Status::Invalid, format!("table {table} row {row} entry {entry}")));
    }
    once(s, CLAIMED, json!([table, row, entry]), "table reward")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::investment::snapshot::tests::snapshot;

    #[test]
    fn each_is_served_once_and_pushes_nothing() {
        let mut s = snapshot();
        unseal(&mut s, 3).unwrap();
        assert_eq!(unseal(&mut s, 3).unwrap_err().status, Status::NotAvailable);
        unseal(&mut s, 4).unwrap();
        redeem(&mut s, 7).unwrap();
        assert!(redeem(&mut s, 7).is_err());
        claim(&mut s, 2, 1, 4).unwrap();
        assert!(claim(&mut s, 2, 1, 4).is_err());
        claim(&mut s, 3, 1, 4).unwrap();
        assert!(claim(&mut s, -1, 1, 4).is_err() && unseal(&mut s, -1).is_err());
        assert!(s.log.is_empty(), "no object changes");
        assert_eq!(s.attribute(UNSEALED), Some(&json!([3, 4])));
    }
}
