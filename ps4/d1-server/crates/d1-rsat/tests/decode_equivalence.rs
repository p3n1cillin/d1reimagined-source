// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! ⭐ The native **decoder** against the client's own: every comparison requires the same decoded
//! struct image (every byte, not a sample of fields) and the same bit count from `d1_rsat` and from
//! `d1_oracle` (Unicorn running 56578's `0x2C9A90` / `0x2C9AE0`) on the same body.
//!
//! Bodies are (a) the client's own encodings of schema-generated images, and (b) those bodies with
//! bits flipped, which reach what full-state encodings never do: absent optional fields, other
//! counts, values out of a field's usual range and bodies that end early. Replicated udts decode
//! over a non-zero seed, so "an absent field keeps the seed" is compared, not assumed.
//!
//! Skips, saying why, without the owned 56578 executable (`D1_EBOOT`); counts are asserted.

mod common;

use common::{ah_schemas, hex, inputs, Gen};
use d1_oracle::activity_host::{self, AhCodec};
use d1_oracle::ObjectOracle;
use d1_rsat::{Decoder, Encoder, RsatError};

/// The udts whose closure the native decoder does not cover. Asserted exactly.
const EXPECTED_DECODE_FALLBACK: &[(u32, &str)] = &[];

#[test]
fn every_pinned_udt_is_classified_for_decode() {
    let Some(s) = inputs() else { return };
    let mut fallbacks = Vec::new();
    for (udt, tag, _) in ah_schemas() {
        if let Err(e) = s.check_decode(tag) {
            assert!(e.is_unsupported(), "udt {udt}: {e}");
            fallbacks.push((udt, e.to_string()));
        }
    }
    let got: Vec<u32> = fallbacks.iter().map(|f| f.0).collect();
    let want: Vec<u32> = EXPECTED_DECODE_FALLBACK.iter().map(|f| f.0).collect();
    assert_eq!(got, want, "decode fallbacks: {fallbacks:?}");
    for ((_, why), (_, want)) in fallbacks.iter().zip(EXPECTED_DECODE_FALLBACK) {
        assert!(why.contains(want), "{why}");
    }
}

/// The outcome of one body through both decoders, compared.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    /// Both decoded it to the same image and bit count.
    Same,
    /// Both refused it (the body ends early, or the client's return check fails).
    BothRefused,
    /// The native walk refused a construct the client would have read outside its object for
    /// (a count past a capacity, a typed reference naming no type): not sent to the oracle.
    NativeRuntimeRefusal,
}

fn compare_decode(codec: &mut AhCodec, dec: &Decoder, udt: u32, tag: u32, wire: &[u8], seed: &[u8], case: &str) -> Outcome {
    let replicated = activity_host::is_replicated(udt);
    let n = if replicated { dec.decode_replicated(tag, wire, seed) } else { dec.decode(tag, wire) };
    match n {
        Err(RsatError::Runtime { .. }) => return Outcome::NativeRuntimeRefusal,
        Err(RsatError::Wire { .. }) => {
            assert!(
                codec.decode_seeded(udt, wire, seed).is_err(),
                "udt {udt} {case}: the native decoder says the body is short, the client's decoded it; body {}",
                hex(wire)
            );
            return Outcome::BothRefused;
        }
        Err(e) => panic!("native udt {udt} {case}: {e}"),
        Ok(n) => {
            let o = codec
                .decode_seeded(udt, wire, seed)
                .unwrap_or_else(|e| panic!("oracle udt {udt} {case}: {e}; native decoded it; body {}", hex(wire)));
            assert_eq!(n.bits, u64::from(o.bits), "udt {udt} {case}: bits; body {}", hex(wire));
            if n.object != o.object {
                let first = (0..n.object.len()).find(|&i| n.object[i] != o.object[i]).unwrap();
                panic!(
                    "udt {udt} (tag {tag:#010x}) {case}: images differ first at +{first:#x} \
                     (native {:02x}, client {:02x}); body {}",
                    n.object[first],
                    o.object[first],
                    hex(wire)
                );
            }
            Outcome::Same
        }
    }
}

#[test]
fn every_native_udt_decodes_like_the_client() {
    let Some(s) = inputs() else { return };
    let enc = Encoder::new(s);
    let dec = Decoder::new(s);
    let mut codec = AhCodec::new();
    let (mut same, mut refused, mut udts) = (0usize, 0usize, 0usize);
    for (udt, tag, size) in ah_schemas() {
        if s.check_decode(tag).is_err() {
            continue;
        }
        udts += 1;
        let mut zero = vec![0; size];
        Gen::new(s, 0).null_refs(tag, &mut zero);
        let mut images = vec![zero];
        for seed in 0..12u64 {
            let mut g = Gen::new(s, 0xDEC0_0000 + seed * 1000 + u64::from(udt));
            images.push(g.image(tag, seed % 2 == 1));
        }
        for (i, img) in images.iter().enumerate() {
            let wire = if activity_host::is_replicated(udt) { enc.encode_replicated(tag, img) } else { enc.encode(tag, img) }
                .unwrap_or_else(|e| panic!("native encode udt {udt}: {e}"));
            let o = compare_decode(&mut codec, &dec, udt, tag, &wire.payload, &[], &format!("image {i}"));
            // Images 0 and the even seeds hold in-range values: their bodies must decode. The raw
            // (odd) ones may overflow a width, as the client's encoder lets them, and a body whose
            // polluted bits read back as other counts can run short (both must refuse it) or past
            // a capacity (the native walk refuses before the client would overrun its array).
            let raw = i > 0 && (i - 1) % 2 == 1;
            if raw {
                if o != Outcome::Same {
                    refused += 1;
                    continue;
                }
            } else {
                assert_eq!(o, Outcome::Same, "udt {udt} image {i}");
            }
            same += 1;
        }
    }
    eprintln!(
        "native decode == client decode on {same} bodies across {udts} udts; {refused} raw-image          bodies refused (short in both, or a count past a capacity)"
    );
    assert!(udts >= 28, "only {udts} udts decoded natively");
    assert_eq!(same + refused, udts * 13);
    assert!(same >= udts * 7, "{same} identical decodes");
}

/// Bodies with bits flipped: the decoders must agree on what each one decodes to, or both refuse.
/// Replicated udts decode over a generated seed, so absent fields must keep it.
#[test]
fn mutated_bodies_decode_alike_or_refuse_alike() {
    let Some(s) = inputs() else { return };
    let enc = Encoder::new(s);
    let dec = Decoder::new(s);
    let mut codec = AhCodec::new();
    let mut tally = std::collections::BTreeMap::<String, usize>::new();
    for (udt, tag, _) in ah_schemas() {
        if s.check_decode(tag).is_err() {
            continue;
        }
        for seed in 0..10u64 {
            let mut g = Gen::new(s, 0xF11B_0000 + seed * 977 + u64::from(udt));
            let img = g.image(tag, false);
            let replicated = activity_host::is_replicated(udt);
            let mut wire = if replicated { enc.encode_replicated(tag, &img) } else { enc.encode(tag, &img) }
                .unwrap()
                .payload;
            let flips = 1 + g.below(6);
            for _ in 0..flips {
                let bit = g.below(wire.len() as u64 * 8);
                wire[(bit / 8) as usize] ^= 0x80 >> (bit % 8);
            }
            if seed % 3 == 0 && wire.len() > 1 {
                wire.truncate(wire.len() - 1 - g.below(wire.len() as u64 / 2) as usize);
            }
            let prior = if replicated { Gen::new(s, seed ^ 0x5EED).image(tag, false) } else { Vec::new() };
            let o = compare_decode(&mut codec, &dec, udt, tag, &wire, &prior, &format!("mutation {seed}"));
            *tally.entry(format!("{o:?}")).or_default() += 1;
        }
    }
    eprintln!("mutated bodies: {tally:?}");
    assert!(tally.get("Same").copied().unwrap_or(0) >= 100, "{tally:?}");
}

/// A replicated body with presence 0 leaves the seed exactly as it was, in both decoders.
#[test]
fn an_absent_replicated_body_keeps_the_seed() {
    let Some(s) = inputs() else { return };
    let dec = Decoder::new(s);
    let mut codec = AhCodec::new();
    for udt in [11u32, 21] {
        let tag = activity_host::tag_for_udt(udt).unwrap();
        let seed = Gen::new(s, u64::from(udt)).image(tag, false);
        assert_eq!(compare_decode(&mut codec, &dec, udt, tag, &[0x00], &seed, "presence 0"), Outcome::Same);
        let n = dec.decode_replicated(tag, &[0x00], &seed).unwrap();
        assert_eq!((n.bits, n.object == seed), (1, true));
    }
}

/// udt 5's objects: the name reference, the lifetime value (with live launch slots and typed
/// references) and the replicated authority table.
#[test]
fn sensor_objects_decode_like_the_client() {
    use d1_oracle::sensor::{TAG_AUTHORITY, TAG_NAME_REF, TAG_SENSOR_VALUE};
    let Some(s) = inputs() else { return };
    let enc = Encoder::new(s);
    let dec = Decoder::new(s);
    let mut cases = 0;
    for (tag, replicated) in [(TAG_NAME_REF, false), (TAG_SENSOR_VALUE, false), (TAG_AUTHORITY, true)] {
        let mut o = ObjectOracle::new(tag).unwrap();
        for seed in 0..16u64 {
            let mut g = Gen::new(s, 0x5E50_0000 + seed);
            let img = g.image(tag, seed % 2 == 1);
            let wire = if replicated { enc.encode_replicated(tag, &img) } else { enc.encode(tag, &img) }.unwrap();
            let n = if replicated { dec.decode_replicated(tag, &wire.payload, &[]) } else { dec.decode(tag, &wire.payload) }
                .unwrap();
            let w = if replicated { o.decode_replicated(&wire.payload, &[]) } else { o.decode(&wire.payload) }.unwrap();
            assert_eq!((n.bits, hex(&n.object)), (u64::from(w.bits), hex(&w.object)), "{tag:#010x} seed {seed}");
            cases += 1;
        }
    }
    // Present typed references in the value's launch slots (handle type 0x80800046).
    let (_, handle_tag) = s.cells.typed_ref_handle;
    let mut o = ObjectOracle::new(TAG_SENSOR_VALUE).unwrap();
    for count in [1u32, 7, 50] {
        let mut img = vec![0u8; 0x504];
        img[0x10..0x14].copy_from_slice(&count.to_le_bytes());
        for i in 0..count as usize {
            let e = 0x14 + 0x18 * i;
            img[e..e + 4].copy_from_slice(&(0x200 + i as u32).to_le_bytes());
            let h = if i % 2 == 0 { handle_tag } else { u32::MAX };
            img[e + 4..e + 8].copy_from_slice(&h.to_le_bytes());
            let inner = if i % 4 == 0 { 0x1234_0000 + i as u32 } else { u32::MAX };
            img[e + 8..e + 12].copy_from_slice(&inner.to_le_bytes());
        }
        let wire = enc.encode(TAG_SENSOR_VALUE, &img).unwrap();
        let n = dec.decode(TAG_SENSOR_VALUE, &wire.payload).unwrap();
        let w = o.decode(&wire.payload).unwrap();
        assert_eq!((n.bits, hex(&n.object)), (u64::from(w.bits), hex(&w.object)), "{count} slots");
        cases += 1;
    }
    assert_eq!(cases, 3 * 16 + 3);
}

/// Malformed and wrong-shaped input refuses with a named error.
#[test]
fn decode_refusals_are_named() {
    let Some(s) = inputs() else { return };
    let dec = Decoder::new(s);
    let t2 = activity_host::tag_for_udt(2).unwrap();
    assert!(matches!(dec.decode(t2, &[]), Err(RsatError::Wire { .. })), "an empty body");
    let t11 = activity_host::tag_for_udt(11).unwrap();
    assert!(matches!(
        dec.decode_replicated(t11, &[], &[]),
        Err(RsatError::Wire { .. })
    ));
    assert!(matches!(
        dec.decode_replicated(t11, &[0x80], &[0u8; 8]),
        Err(RsatError::BadImage { .. })
    ));
    let t33 = activity_host::tag_for_udt(33).unwrap();
    assert!(matches!(dec.decode(t33, &[]), Err(RsatError::Wire { .. })));
    assert!(matches!(
        dec.decode(0x8080_33F4, &[0]),
        Err(RsatError::UnknownTag(_))
    ));
    // The value's launch-slot count is a 6-bit field: 63 decodes, and exceeds the 50 slots.
    let mut w = d1_rsat::bits::BitWriter::new();
    for (v, n) in [(1u64, 4u32), (0, 3), (0, 1), (0, 32), (0, 32), (0, 32), (63, 6)] {
        w.write(v, n);
    }
    let (_, body) = w.finish();
    let mut body = body;
    body.resize(64, 0);
    assert!(matches!(dec.decode(0x8080_08A3, &body), Err(RsatError::Runtime { .. })), "count 63 > 50");
}
