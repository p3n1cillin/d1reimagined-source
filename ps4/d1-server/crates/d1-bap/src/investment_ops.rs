// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! ⭐ Stream P: the wire side of the investment requests the responder had no answer for:
//! **406** (item state flags: the lock), **801/802** (talent node activation, node step change),
//! **804** (infusion), **904** (vendor interaction: packages, turn-ins), **1801/1802/1803** (Record
//! Book rewards and unsealing). The responder's `investment` module decides what each one does;
//! this module only reads the requests and writes what goes back.
//!
//! **Requests.** Every layout is the 56578 descriptor's (`ps4/fixtures/56578/rtti/fields.tsv`), read
//! the way the client's codec writes it: MSB-first, fields in descriptor order, signed fields biased
//! (`kind 3/4/5`: 8/16/32 bits, bias `0x80`/`0x8000`/`0x8000_0000`), `kind 6` a raw u64, `kind 2` one
//! bit. The item selector `I` (`0x80801983`) is `{bool, u64, i16}` (81 bits).
//!
//! | Service | Request | Bits |
//! |---|---|---|
//! | 406 | `0x80801A03`: `I @0; flags:i32 @0x18` | 113 |
//! | 801 | `0x8080171F`: `item:u64 @0, node:i8 @8` | 72 |
//! | 802 | `0x8080174D`: `item:u64 @0, node:i8 @8` | 72 |
//! | 804 | `0x80801A1F`: `target:u64 @0, node:i8 @8, source:u64 @0x10` | 136 |
//! | 904 | `0x808019EE`: `vendor:i8 @0, a:i16 @2, b:i16 @4, sale:i32 @8` | 72 |
//! | 1801 | `0x80801041`: `reward:i16 @0` | 16 |
//! | 1802 | `0x80801043`: `book:i8 @0` | 8 |
//! | 1803 | `0x80801046`: `entry:i16 @0, row:i8 @2, table:i8 @3` | 32 |
//!
//! **Replies.** Each is the shared result `R` (`0x80801796`: `status:i8/4-bit` bias 1,
//! `value:i32/32-bit` bias `0x8000_0000`, 36 bits, padded to 5 bytes). The live Tower capture
//! (notes/d1-live-captures-patrol-tower-2026-11-06.md) shows every exchange as **request, then a
//! family-4 push, then the reply**, with `R.value` = the family-4 version after that push (402's
//! replies carry 0). [`result_body`] writes it; [`change_delta`] writes the push.

/// MSB-first bit reader over a request body (after the 2-byte service id).
struct Bits<'a> {
    data: &'a [u8],
    at: usize,
}

impl Bits<'_> {
    fn take(&mut self, width: usize) -> Option<u64> {
        let mut v = 0u64;
        for _ in 0..width {
            let byte = *self.data.get(self.at / 8)?;
            v = (v << 1) | u64::from((byte >> (7 - self.at % 8)) & 1);
            self.at += 1;
        }
        Some(v)
    }

    /// A biased signed field (`kind 3/4/5`): the wire value minus `1 << (width - 1)`.
    fn biased(&mut self, width: usize) -> Option<i64> {
        let v = self.take(width)? ^ (1 << (width - 1));
        Some(((v << (64 - width)) as i64) >> (64 - width))
    }
}

/// MSB-first bit writer, the reader's inverse (for [`result_body`] and the tests).
#[derive(Default)]
pub struct BitWriter {
    bits: Vec<bool>,
}

impl BitWriter {
    pub fn put(&mut self, width: usize, value: u64) -> &mut Self {
        for k in (0..width).rev() {
            self.bits.push(value >> k & 1 == 1);
        }
        self
    }
    pub fn biased(&mut self, width: usize, value: i64) -> &mut Self {
        let mask = if width == 64 { u64::MAX } else { (1u64 << width) - 1 };
        self.put(width, (value as u64 ^ (1 << (width - 1))) & mask)
    }
    pub fn len(&self) -> usize {
        self.bits.len()
    }
    pub fn is_empty(&self) -> bool {
        self.bits.is_empty()
    }
    /// The bits, zero-padded to a byte boundary.
    pub fn bytes(&self) -> Vec<u8> {
        self.bits.chunks(8).map(|c| c.iter().enumerate().fold(0u8, |b, (i, &bit)| b | (u8::from(bit) << (7 - i)))).collect()
    }
}

/// The item selector `I` (`0x80801983`): `{f0:bool @0, instance:u64 @8, index:i16 @0x10}`. The
/// client's builders set `f0` = "the item has an instance" and `index` = its content index for a
/// stack (-1 when instanced is not guaranteed: 402 dismantles carry the content index too).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemSelector {
    pub instanced: bool,
    pub instance: u64,
    pub index: i16,
}

fn selector(b: &mut Bits) -> Option<ItemSelector> {
    Some(ItemSelector { instanced: b.take(1)? != 0, instance: b.take(64)?, index: b.biased(16)? as i16 })
}

/// A decoded request of one of the services this module covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// 406: the item's new state flags, as the client computed them (bit 0 lock, bit 1 open).
    ItemFlags { item: ItemSelector, flags: i32 },
    /// 801: activate `node` of `item`'s talent grid.
    ActivateNode { item: u64, node: i8 },
    /// 802: change `node`'s step (select it among the steps already open).
    ChangeNodeStep { item: u64, node: i8 },
    /// 804: infuse `source` into `target` through `target`'s infusion `node`.
    Infuse { target: u64, node: i8, source: u64 },
    /// 904: vendor interaction. `sale` is the vendor's sale index (-1 for none); `a` and `b` are
    /// the item request's `+0x70`/`+0x72` (their table is open).
    VendorInteract { vendor: i8, a: i16, b: i16, sale: i32 },
    /// 1801: record reward redemption, by the client's reward ordinal.
    RedeemRecordReward { reward: i16 },
    /// 1802: unseal a Record Book, by the client's book ordinal.
    UnsealBook { book: i8 },
    /// 1803: item request action 14 on a category-5 item: `{entry, row, table}` of the content
    /// table at manager slot `+0x490` (stride `0x60`).
    ClaimTableReward { entry: i16, row: i8, table: i8 },
}

/// The services [`decode`] reads.
pub const SERVICES: [u16; 8] = [406, 801, 802, 804, 904, 1801, 1802, 1803];

/// Each service's request bit count, from the descriptor (the oracle tests confirm them).
pub fn request_bits(service: u16) -> Option<usize> {
    Some(match service {
        406 => 81 + 32,
        801 | 802 => 72,
        804 => 136,
        904 => 72,
        1801 => 16,
        1802 => 8,
        1803 => 32,
        _ => return None,
    })
}

/// Decode a request body (the bitstream after the 2-byte service id).
pub fn decode(service: u16, body: &[u8]) -> Option<Request> {
    let mut b = Bits { data: body, at: 0 };
    Some(match service {
        406 => Request::ItemFlags { item: selector(&mut b)?, flags: b.biased(32)? as i32 },
        801 => Request::ActivateNode { item: b.take(64)?, node: b.biased(8)? as i8 },
        802 => Request::ChangeNodeStep { item: b.take(64)?, node: b.biased(8)? as i8 },
        804 => Request::Infuse { target: b.take(64)?, node: b.biased(8)? as i8, source: b.take(64)? },
        904 => Request::VendorInteract {
            vendor: b.biased(8)? as i8,
            a: b.biased(16)? as i16,
            b: b.biased(16)? as i16,
            sale: b.biased(32)? as i32,
        },
        1801 => Request::RedeemRecordReward { reward: b.biased(16)? as i16 },
        1802 => Request::UnsealBook { book: b.biased(8)? as i8 },
        1803 => Request::ClaimTableReward { entry: b.biased(16)? as i16, row: b.biased(8)? as i8, table: b.biased(8)? as i8 },
        _ => return None,
    })
}

/// 402 `inventory_perform_item_action` (`0x80801767`: `I @0; action:i32 @0x18`, 113 bits, the
/// same layout as 406): the item and the action index its definition lists. The live Tower
/// capture's dismantles and bounty turn-ins both sent action 1.
pub fn decode_item_action(body: &[u8]) -> Option<(ItemSelector, i32)> {
    let mut b = Bits { data: body, at: 0 };
    Some((selector(&mut b)?, b.biased(32)? as i32))
}

/// Encode a request the way the client does (tests, and the harness's synthetic requests).
pub fn encode(r: &Request) -> (u16, Vec<u8>) {
    let mut w = BitWriter::default();
    let service = match *r {
        Request::ItemFlags { item, flags } => {
            w.put(1, u64::from(item.instanced)).put(64, item.instance).biased(16, i64::from(item.index)).biased(32, i64::from(flags));
            406
        }
        Request::ActivateNode { item, node } => {
            w.put(64, item).biased(8, i64::from(node));
            801
        }
        Request::ChangeNodeStep { item, node } => {
            w.put(64, item).biased(8, i64::from(node));
            802
        }
        Request::Infuse { target, node, source } => {
            w.put(64, target).biased(8, i64::from(node)).put(64, source);
            804
        }
        Request::VendorInteract { vendor, a, b, sale } => {
            w.biased(8, i64::from(vendor)).biased(16, i64::from(a)).biased(16, i64::from(b)).biased(32, i64::from(sale));
            904
        }
        Request::RedeemRecordReward { reward } => {
            w.biased(16, i64::from(reward));
            1801
        }
        Request::UnsealBook { book } => {
            w.biased(8, i64::from(book));
            1802
        }
        Request::ClaimTableReward { entry, row, table } => {
            w.biased(16, i64::from(entry)).biased(8, i64::from(row)).biased(8, i64::from(table));
            1803
        }
    };
    (service, w.bytes())
}

/// The reply tag of each service (all are `R` alone).
pub fn reply_tag(service: u16) -> Option<u32> {
    Some(match service {
        406 => 0x8080_1A40,
        801 => 0x8080_1720,
        802 => 0x8080_183D,
        804 => 0x8080_19EA,
        904 => 0x8080_19B8,
        1801 => 0x8080_1042,
        1802 => 0x8080_1044,
        1803 => 0x8080_1047,
        _ => return None,
    })
}

/// The request tag of each service.
pub fn request_tag(service: u16) -> Option<u32> {
    Some(match service {
        406 => 0x8080_1A03,
        801 => 0x8080_171F,
        802 => 0x8080_174D,
        804 => 0x8080_1A1F,
        904 => 0x8080_19EE,
        1801 => 0x8080_1041,
        1802 => 0x8080_1043,
        1803 => 0x8080_1046,
        _ => return None,
    })
}

/// The shared result `R` as a reply body: `{status, value}`, 36 bits, zero-padded to 5 bytes.
/// `result_body(0, 0)` is the familiar `18 00 00 00 00`.
pub fn result_body(status: i8, value: i32) -> Vec<u8> {
    let mut w = BitWriter::default();
    w.put(4, (i64::from(status) + 1) as u64 & 0xF).biased(32, i64::from(value));
    w.bytes()
}

/// Read a reply body back (tests and logs).
pub fn parse_result(body: &[u8]) -> Option<(i8, i32)> {
    let mut b = Bits { data: body, at: 0 };
    let status = b.take(4)? as i8 - 1;
    Some((status, b.biased(32)? as i32))
}

/// One object of a family-4 change, in the order the push carries it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// The account object (family 4 / type 0), client-encoded.
    Account(Vec<u8>),
    /// The character's profile (family 4 / type 1), client-encoded, keyed by the profile SOID.
    Profile { soid: u64, payload: Vec<u8> },
    /// An item element (family 4 / type 3, `0x80801732`), client-encoded: new or changed.
    Item { soid: u64, payload: Vec<u8> },
    /// An item instance that is gone (an infusion's source, a turned-in item): an item object of
    /// length 0, as the live pushes delete a consumed engram or a dismantled item.
    Deleted(u64),
}

/// The family-4 DELTA carrying `changes` in order, at `version` (the client's last accepted + 1).
pub fn change_delta(account_soid: u64, version: u32, changes: &[Change]) -> Vec<u8> {
    let mut b = crate::queuez_family_header_versioned(
        crate::QUEUEZ_FAMILY_ACCOUNT,
        account_soid,
        changes.len() as u32,
        version,
        crate::QUEUEZ_FLAG_DELTA,
    );
    for c in changes {
        let (ty, key, payload): (u32, u64, &[u8]) = match c {
            Change::Account(p) => (crate::account_instance_type(), account_soid, p),
            Change::Profile { soid, payload } => (crate::profile_instance_type(), *soid, payload),
            Change::Item { soid, payload } => (crate::INVENTORY_INSTANCE_TYPE, *soid, payload),
            Change::Deleted(soid) => (crate::INVENTORY_INSTANCE_TYPE, *soid, &[]),
        };
        b.extend_from_slice(&ty.to_be_bytes());
        b.extend_from_slice(&key.to_be_bytes());
        b.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        b.extend_from_slice(payload);
    }
    b
}

/// The live order of one exchange: the push at `last_accepted + 1` (none when nothing changed),
/// then the reply whose value is the family-4 version the client holds after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exchange {
    pub push: Option<(u32, Vec<u8>)>,
    pub reply: Vec<u8>,
}

pub fn exchange(account_soid: u64, last_accepted: u32, status: i8, changes: &[Change]) -> Exchange {
    if changes.is_empty() {
        return Exchange { push: None, reply: result_body(status, last_accepted as i32) };
    }
    let version = last_accepted.wrapping_add(1);
    Exchange { push: Some((version, change_delta(account_soid, version, changes))), reply: result_body(status, version as i32) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_success_result_is_the_familiar_five_bytes() {
        assert_eq!(result_body(0, 0), vec![0x18, 0, 0, 0, 0]);
        assert_eq!(result_body(0, 0), crate::investment_504_response_body());
        // The 901 at 76.7 s of the live Tower capture: push version 590, reply value 590.
        assert_eq!(parse_result(&result_body(0, 590)), Some((0, 590)));
        assert_eq!(parse_result(&result_body(-1, -17)), Some((-1, -17)));
        assert_eq!(result_body(0, 590).len(), 5, "36 bits");
    }

    /// The live 801 `{u64 item, i8 node 0}` and 904 `{13, 5, 0, 114}` shapes, synthetic values.
    #[test]
    fn every_request_round_trips_at_its_descriptor_width() {
        let reqs = [
            Request::ItemFlags { item: ItemSelector { instanced: true, instance: 0xD100_0000_0000_0401, index: 3598 }, flags: 1 },
            Request::ActivateNode { item: 0x0102_0304_0506_0708, node: 0 },
            Request::ChangeNodeStep { item: 0x0102_0304_0506_0708, node: 14 },
            Request::Infuse { target: 0x401, node: 9, source: 0x402 },
            Request::VendorInteract { vendor: 13, a: 5, b: 0, sale: 114 },
            Request::VendorInteract { vendor: 22, a: 20, b: 0, sale: -1 },
            Request::RedeemRecordReward { reward: 7 },
            Request::UnsealBook { book: 3 },
            Request::ClaimTableReward { entry: 2, row: 1, table: 4 },
        ];
        for r in reqs {
            let (service, body) = encode(&r);
            assert_eq!(body.len(), request_bits(service).unwrap().div_ceil(8), "{r:?}");
            assert_eq!(decode(service, &body), Some(r));
            assert!(reply_tag(service).is_some() && request_tag(service).is_some());
        }
        let (_, flags) = encode(&Request::ItemFlags { item: ItemSelector { instanced: true, instance: 0x501, index: 1234 }, flags: 1 });
        assert_eq!(decode_item_action(&flags), Some((ItemSelector { instanced: true, instance: 0x501, index: 1234 }, 1)), "402 shares 406's layout");
        assert_eq!(decode(801, &[0; 4]), None, "a short body is refused");
        assert_eq!(decode(901, &[0; 16]), None);
    }

    /// The 801 request's bits by hand: the u64, then node 0 biased to 0x80.
    #[test]
    fn the_node_is_biased() {
        let (_, b) = encode(&Request::ActivateNode { item: 1, node: 0 });
        assert_eq!(b, vec![0, 0, 0, 0, 0, 0, 0, 1, 0x80]);
    }

    #[test]
    fn the_change_delta_keeps_the_order_and_deletes_with_length_zero() {
        let d = change_delta(0x1, 7, &[
            Change::Deleted(0x402),
            Change::Item { soid: 0x401, payload: vec![0xAB; 38] },
            Change::Account(vec![0xCD; 24]),
            Change::Profile { soid: 0x201, payload: vec![0xEF; 40] },
        ]);
        assert_eq!(crate::queuez_span_version(&d), Some(7));
        assert_eq!(crate::queuez_span_flags(&d), Some(crate::QUEUEZ_FLAG_DELTA));
        let objs = crate::queuez_span_objects(&d).unwrap();
        let shape: Vec<_> = objs.iter().map(|o| (o.instance_type, o.key, o.payload.1 - o.payload.0)).collect();
        assert_eq!(shape, vec![
            (crate::INVENTORY_INSTANCE_TYPE, 0x402, 0),
            (crate::INVENTORY_INSTANCE_TYPE, 0x401, 38),
            (crate::account_instance_type(), 0x1, 24),
            (crate::profile_instance_type(), 0x201, 40),
        ]);
    }

    #[test]
    fn the_reply_names_the_version_of_the_push_before_it() {
        let e = exchange(0x1, 589, 0, &[Change::Account(vec![1; 8])]);
        let (v, push) = e.push.unwrap();
        assert_eq!(v, 590);
        assert_eq!(crate::queuez_span_version(&push), Some(590));
        assert_eq!(parse_result(&e.reply), Some((0, 590)));
        // Nothing changed: no push, and the reply names the version the client already holds.
        let e = exchange(0x1, 589, 0, &[]);
        assert!(e.push.is_none());
        assert_eq!(parse_result(&e.reply), Some((0, 589)));
    }
}
