// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Synthetic request values against the owned 56578 codec; no capture bytes.
use std::path::Path;

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
    assert_eq!(
        image.sha256(),
        "672f03411c0503dfbcda804cbb9fb27cdd9a8d73a8bcb71bf0c4f688073b0833",
        "catalogue vectors are build 56578 only"
    );
    Some(image)
}

fn put(object: &mut [u8], offset: usize, bytes: &[u8]) {
    object[offset..offset + bytes.len()].copy_from_slice(bytes);
}

#[test]
fn vendor_inventory_and_talent_requests_decode_nonzero_values() {
    let Some(image) = image() else { return };
    let index = descriptors::index(&image).unwrap();
    // Ids are content-derived (notes/investment-services.md); these assertions test
    // the associated schema shapes, not an eboot-only discovery of the routing map.
    for (id, tag, bits) in [
        (902, 0x8080180d, 97),
        (903, 0x80801734, 121),
        (403, 0x80801942, 64),
        (404, 0x8080189a, 64),
        (405, 0x808018ce, 128),
        (801, 0x8080171f, 72),
        (802, 0x8080174d, 72),
    ] {
        let d = index.get(tag).expect("schema present");
        let mut object = vec![0u8; d.struct_size as usize];
        let soid = 0x0102_0304_0506_0708u64;
        match id {
            902 | 903 => {
                object[0] = 1;
                put(&mut object, 8, &soid.to_le_bytes());
                put(&mut object, 0x10, &7i16.to_le_bytes());
                if id == 902 {
                    put(&mut object, 0x18, &9i16.to_le_bytes());
                } else {
                    put(&mut object, 0x18, &3i32.to_le_bytes());
                    object[0x1c] = 2;
                }
            }
            405 => {
                object[0] = 1;
                put(&mut object, 8, &soid.to_le_bytes());
                put(&mut object, 0x10, &7i16.to_le_bytes());
                put(&mut object, 0x14, &3i32.to_le_bytes());
            }
            _ => {
                put(&mut object, 0, &soid.to_le_bytes());
                if id == 801 || id == 802 {
                    object[8] = 2;
                }
            }
        }
        let mut oracle = ObjectOracle::new(tag).expect("positive controls and schema registry");
        let (encoded, decoded) = oracle
            .round_trip(&object, 0x10000)
            .unwrap_or_else(|e| panic!("service {id}, tag {tag:#x}: {e}"));
        assert_eq!(encoded.bits, bits, "service {id}");
        assert_eq!(decoded.bits, bits, "service {id}");
        assert_eq!(
            decoded.object, object,
            "nonzero fields and padding: service {id}"
        );
        object[if id == 902 || id == 903 || id == 405 {
            8
        } else {
            0
        }] ^= 0x40;
        assert_ne!(
            oracle.encode(&object, 0x10000).unwrap().payload,
            encoded.payload,
            "inverted value control: service {id}"
        );
    }
}

#[test]
fn shared_result_and_vendor_replies_decode_an_error_as_well_as_success() {
    let Some(image) = image() else { return };
    let index = descriptors::index(&image).unwrap();
    for tag in [0x808016ef, 0x8080183a, 0x808018ab, 0x80801719, 0x8080194a] {
        let d = index.get(tag).expect("reply schema");
        let mut object = vec![0u8; d.struct_size as usize];
        let mut oracle = ObjectOracle::new(tag).expect("positive controls");
        let (success, _) = oracle.round_trip(&object, 0x10000).expect("success result");
        assert_eq!(success.bits, 36);
        object[0] = 2;
        put(&mut object, 4, &(-17i32).to_le_bytes());
        let (error, decoded) = oracle
            .round_trip(&object, 0x10000)
            .expect("nonzero error result");
        assert_eq!(decoded.object, object);
        assert_eq!(error.bits, 36);
        assert_ne!(success.payload, error.payload);
    }
}

#[test]
fn refund_interaction_and_offers_have_the_56578_shapes() {
    let Some(image) = image() else { return };
    let index = descriptors::index(&image).unwrap();
    // The empty 1401 request has ambiguous registry-object anchors in this harness.
    // Assert its real descriptor shape without pretending an oracle decode succeeded.
    let offers = index.get(0x80801703).expect("offers request descriptor");
    assert_eq!(offers.field_count, 0);
    assert_eq!(offers.struct_size, 1);
    for (tag, expected_bits) in [(0x80801e76, 147), (0x808019ee, 72)] {
        let mut object = vec![0u8; index.get(tag).unwrap().struct_size as usize];
        if tag == 0x80801e76 {
            object[0] = 1; // two-bit selector, bias 1
            put(&mut object, 8, &0x0102_0304_0506_0708u64.to_le_bytes());
            put(&mut object, 0x10, &7i16.to_le_bytes());
            put(&mut object, 0x18, &0x1122_3344_5566_7788u64.to_le_bytes());
        } else if tag == 0x808019ee {
            object[0] = 2;
            put(&mut object, 2, &7i16.to_le_bytes());
            put(&mut object, 4, &9i16.to_le_bytes());
            put(&mut object, 8, &3i32.to_le_bytes());
        }
        let mut oracle = ObjectOracle::new(tag).expect("positive controls");
        let (encoded, decoded) = oracle.round_trip(&object, 0x10000).expect("56578 request");
        assert_eq!(encoded.bits, expected_bits, "tag {tag:#x}");
        assert_eq!(decoded.bits, expected_bits);
        assert_eq!(decoded.object, object);
    }
    // Refund's reply is the same full result grammar, including nonzero errors.
    let mut reply = vec![0u8; index.get(0x808019bc).unwrap().struct_size as usize];
    reply[0] = 2;
    put(&mut reply, 4, &(-17i32).to_le_bytes());
    let mut oracle = ObjectOracle::new(0x808019bc).unwrap();
    let (encoded, decoded) = oracle.round_trip(&reply, 0x10000).unwrap();
    assert_eq!(encoded.bits, 36);
    assert_eq!(decoded.object, reply);
}

#[test]
fn second_item_talent_activation_and_record_book_requests_decode() {
    let Some(image) = image() else { return };
    let index = descriptors::index(&image).unwrap();
    for (tag, bits, reply) in [
        (0x80801a1f, 136, 0x808019ea), // 804: two item identities and a node selector
        (0x80801041, 16, 0x80801042),  // 1801: record reward redemption
        (0x80801043, 8, 0x80801044),   // 1802: unseal book
    ] {
        let mut object = vec![0u8; index.get(tag).unwrap().struct_size as usize];
        if tag == 0x80801a1f {
            put(&mut object, 0, &0x0102_0304_0506_0708u64.to_le_bytes());
            object[8] = 2;
            put(&mut object, 0x10, &0x1122_3344_5566_7788u64.to_le_bytes());
        } else {
            object[0] = 7;
        }
        let mut oracle = ObjectOracle::new(tag).expect("positive controls");
        let (encoded, decoded) = oracle.round_trip(&object, 0x10000).expect("request layout");
        assert_eq!(encoded.bits, bits);
        assert_eq!(decoded.bits, bits);
        assert_eq!(decoded.object, object);
        let mut result = vec![0u8; index.get(reply).unwrap().struct_size as usize];
        result[0] = 2;
        put(&mut result, 4, &(-17i32).to_le_bytes());
        let mut oracle = ObjectOracle::new(reply).unwrap();
        let (encoded, decoded) = oracle.round_trip(&result, 0x10000).expect("reply layout");
        assert_eq!(encoded.bits, 36);
        assert_eq!(decoded.object, result);
    }
}

/// Incident body selector 27 (`activity_complete` and its four companions) names
/// `0x80800595`; notes/ps4-56578-wire-catalog-2026-11-05.md has the binding chain.
/// Synthetic values only: a hash-like activity, a host and a player identity.
fn activity_complete_body(counters: &[i32; 7]) -> Vec<u8> {
    let mut body = vec![0u8; 0x80];
    put(&mut body, 0, &0x1122_3344u32.to_le_bytes()); // activity instance word
    body[8] = 2; // id kind, 3-bit bias 1
    put(&mut body, 0x10, &0x0102_0304_0506_0708u64.to_le_bytes()); // activity host id
    put(&mut body, 0x18, &0x0a0b_0c0d_0e0f_1011u64.to_le_bytes()); // participant identity
    put(&mut body, 0x28, &0x0102_0304_0506_0708u64.to_le_bytes());
    put(&mut body, 0x30, &0x2122_2324_2526_2728u64.to_le_bytes());
    body[0x39] = 3;
    body[0x3a] = 1;
    for (i, h) in [
        0x6e0d_ae89u32,
        0x6489_99f4,
        0x6489_99f4,
        0x811c_9dc5,
        0xb224_17ae,
    ]
    .iter()
    .enumerate()
    {
        put(&mut body, 0x44 + 4 * i, &h.to_le_bytes()); // activity, type, type, none, place
    }
    for (i, c) in counters.iter().enumerate() {
        put(&mut body, 0x60 + 4 * i, &c.to_le_bytes());
    }
    body
}

#[test]
fn activity_complete_metadata_is_725_bits_and_the_51539_length_is_short() {
    let Some(image) = image() else { return };
    let index = descriptors::index(&image).unwrap();
    let d = index
        .get(0x80800595)
        .expect("activity_complete metadata schema");
    assert_eq!((d.struct_size, d.field_count), (128, 18));
    let body = activity_complete_body(&[0, 0, 1, 1, 0, 0, 0]);
    let mut oracle = ObjectOracle::new(0x80800595).expect("positive controls");
    let (encoded, decoded) = oracle.round_trip(&body, 0x10000).expect("965 body");
    // No presence bits: 99 + 206 + 2 + 3 + 5x32 + 1 + 32 + 7x32.
    assert_eq!(encoded.bits, 725);
    assert_eq!(decoded.object[..0x7c], body[..0x7c]);
    // Changed-value control: a counter moves the payload, not its length.
    let (other, _) = oracle
        .round_trip(&activity_complete_body(&[0, 0, -1, -1, 0, 0, 0]), 0x10000)
        .unwrap();
    assert_eq!(other.bits, 725);
    assert_ne!(other.payload, encoded.payload);
    // The 51539 live bodies are 79 bytes (four counters). On 56578 that length is under-read.
    let short = &encoded.payload[..79];
    assert!(
        oracle.decode(short).is_err(),
        "a 79-byte body must not decode as 56578"
    );
}
