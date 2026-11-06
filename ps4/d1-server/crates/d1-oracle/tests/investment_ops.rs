// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Stream P: the client's own 56578 codec against `d1_bap::investment_ops`. Every request the
//! client encodes decodes to the same fields, and every reply this server writes is byte-for-byte
//! what the client's encoder writes for the same `R`. Synthetic values only; no capture bytes.
//! Skips without `D1_EBOOT`.
use std::path::Path;

use d1_bap::investment_ops::{self as ops, ItemSelector, Request};
use d1_eboot::{descriptors, image::Image};
use d1_oracle::object::ObjectOracle;

fn image() -> Option<Image> {
    let Ok(path) = std::env::var("D1_EBOOT") else {
        eprintln!("SKIP: set D1_EBOOT to the owned 56578 executable");
        return None;
    };
    if !Path::new(&path).is_file() {
        eprintln!("SKIP: D1_EBOOT input is absent");
        return None;
    }
    let image = Image::open(path).expect("read D1_EBOOT");
    assert_eq!(image.sha256(), "672f03411c0503dfbcda804cbb9fb27cdd9a8d73a8bcb71bf0c4f688073b0833", "build 56578 only");
    Some(image)
}

fn put(object: &mut [u8], offset: usize, bytes: &[u8]) {
    object[offset..offset + bytes.len()].copy_from_slice(bytes);
}

/// The C object the client's builder fills for `r` (the descriptor's offsets).
fn object_for(r: &Request, size: usize) -> Vec<u8> {
    let mut o = vec![0u8; size];
    match *r {
        Request::ItemFlags { item, flags } => {
            o[0] = u8::from(item.instanced);
            put(&mut o, 8, &item.instance.to_le_bytes());
            put(&mut o, 0x10, &item.index.to_le_bytes());
            put(&mut o, 0x18, &flags.to_le_bytes());
        }
        Request::ActivateNode { item, node } | Request::ChangeNodeStep { item, node } => {
            put(&mut o, 0, &item.to_le_bytes());
            o[8] = node as u8;
        }
        Request::Infuse { target, node, source } => {
            put(&mut o, 0, &target.to_le_bytes());
            o[8] = node as u8;
            put(&mut o, 0x10, &source.to_le_bytes());
        }
        Request::VendorInteract { vendor, a, b, sale } => {
            o[0] = vendor as u8;
            put(&mut o, 2, &a.to_le_bytes());
            put(&mut o, 4, &b.to_le_bytes());
            put(&mut o, 8, &sale.to_le_bytes());
        }
        Request::RedeemRecordReward { reward } => put(&mut o, 0, &reward.to_le_bytes()),
        Request::UnsealBook { book } => o[0] = book as u8,
        Request::ClaimTableReward { entry, row, table } => {
            put(&mut o, 0, &entry.to_le_bytes());
            o[2] = row as u8;
            o[3] = table as u8;
        }
    }
    o
}

#[test]
fn the_clients_encoder_writes_what_the_server_decodes() {
    let Some(image) = image() else { return };
    let index = descriptors::index(&image).unwrap();
    let requests = [
        // 406: lock (bit 0 set), unlock, and the open bit 1, on an instanced item and on a stack.
        Request::ItemFlags { item: ItemSelector { instanced: true, instance: 0xD100_0000_0000_0401, index: -1 }, flags: 1 },
        Request::ItemFlags { item: ItemSelector { instanced: true, instance: 0xD100_0000_0000_0401, index: 3598 }, flags: 0 },
        Request::ItemFlags { item: ItemSelector { instanced: false, instance: 0, index: 714 }, flags: 3 },
        // 801 node 0 and 802 node 14, the live shapes; a negative node as a sign control.
        Request::ActivateNode { item: 0x0102_0304_0506_0708, node: 0 },
        Request::ActivateNode { item: 0x0102_0304_0506_0708, node: -3 },
        Request::ChangeNodeStep { item: 0xD100_0000_0000_0401, node: 14 },
        Request::Infuse { target: 0xD100_0000_0000_0401, node: 9, source: 0xD100_0000_0000_0402 },
        // 904: the live package, rank-up, Gunsmith and Shipwright shapes.
        Request::VendorInteract { vendor: 13, a: 5, b: 0, sale: 114 },
        Request::VendorInteract { vendor: 11, a: 19, b: 0, sale: 211 },
        Request::VendorInteract { vendor: 25, a: 2, b: 0, sale: 242 },
        Request::VendorInteract { vendor: 22, a: 20, b: 0, sale: -1 },
        Request::RedeemRecordReward { reward: 7 },
        Request::RedeemRecordReward { reward: -1 },
        Request::UnsealBook { book: 3 },
        Request::ClaimTableReward { entry: 2, row: 1, table: 4 },
    ];
    for r in requests {
        let (service, ours) = ops::encode(&r);
        let tag = ops::request_tag(service).unwrap();
        let size = index.get(tag).expect("request descriptor").struct_size as usize;
        let object = object_for(&r, size);
        let mut oracle = ObjectOracle::new(tag).expect("positive controls");
        let encoded = oracle.encode(&object, 0x10000).unwrap_or_else(|e| panic!("{r:?}: {e}"));
        assert_eq!(encoded.bits as usize, ops::request_bits(service).unwrap(), "{r:?}: bits");
        assert_eq!(encoded.payload[..ours.len()], ours[..], "{r:?}: the client's bytes");
        assert_eq!(ops::decode(service, &encoded.payload), Some(r), "{r:?}: decoded from the client's bytes");
    }
}

/// 402 (`0x80801767`, the bounty turn-in's and the dismantle's request): action 1, as live.
#[test]
fn the_402_item_action_shares_406s_layout() {
    let Some(image) = image() else { return };
    let index = descriptors::index(&image).unwrap();
    let size = index.get(0x8080_1767).expect("402 request").struct_size as usize;
    for (item, action) in [
        (ItemSelector { instanced: true, instance: 0xD100_0000_0000_0501, index: 1234 }, 1),
        (ItemSelector { instanced: false, instance: 0, index: 714 }, 3),
    ] {
        let object = object_for(&Request::ItemFlags { item, flags: action }, size);
        let encoded = ObjectOracle::new(0x8080_1767).unwrap().encode(&object, 0x10000).unwrap();
        assert_eq!(encoded.bits, 113);
        assert_eq!(ops::decode_item_action(&encoded.payload), Some((item, action)));
    }
    // 402's reply carries 0 live: the five familiar bytes.
    let reply = vec![0u8; index.get(0x8080_1953).unwrap().struct_size as usize];
    let encoded = ObjectOracle::new(0x8080_1953).unwrap().encode(&reply, 0x10000).unwrap();
    assert_eq!(encoded.payload[..5], ops::result_body(0, 0)[..]);
}

#[test]
fn every_reply_is_the_clients_own_bytes_for_the_same_result() {
    let Some(image) = image() else { return };
    let index = descriptors::index(&image).unwrap();
    for service in ops::SERVICES {
        let tag = ops::reply_tag(service).unwrap();
        let size = index.get(tag).expect("reply descriptor").struct_size as usize;
        // Status 0 with the version the live 901 at 76.7 s named (590), a zero version, and an
        // error status with a negative value as the changed-value control.
        for (status, value) in [(0i8, 590i32), (0, 0), (1, -17), (0, i32::MAX)] {
            let mut object = vec![0u8; size];
            object[0] = status as u8;
            put(&mut object, 4, &value.to_le_bytes());
            let mut oracle = ObjectOracle::new(tag).expect("positive controls");
            let encoded = oracle.encode(&object, 0x10000).unwrap_or_else(|e| panic!("{service}: {e}"));
            assert_eq!(encoded.bits, 36, "{service}");
            let ours = ops::result_body(status, value);
            assert_eq!(encoded.payload[..ours.len()], ours[..], "{service} R{{{status}, {value}}}");
            assert_eq!(ops::parse_result(&encoded.payload), Some((status, value)));
        }
    }
}
