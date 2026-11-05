// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! ⭐ Native RSAT against the client's own codec: every comparison requires the **same payload
//! bytes and the same bit count** from `d1_rsat` and from `d1_oracle` (Unicorn running the 56578
//! encoder) on the same struct image.
//!
//! Inputs: the owned 56578 executable through `D1_EBOOT`. Without it every test prints `SKIP` and
//! returns. With it present, every case runs and the case counts are asserted, so a pass cannot
//! mean that nothing ran.
//!
//! Images are generated from the schema itself ([`Gen`]): leaves in range for their width and bias,
//! or raw random bytes (which exercises sign extension, bool bytes above 1 and overflowing values),
//! with every count source set to 0, 1, the capacity or a random count in between. Seeds are fixed,
//! so a failure names a reproducible case.

mod common;

use common::{ah_schemas, hex, inputs, put, Gen};
use d1_oracle::activity_host::{self, AhCodec};
use d1_oracle::ObjectOracle;
use d1_rsat::schema::{Descriptor, Field, Schemas, KIND_COMPOSITE, KIND_TYPED_REF};
use d1_rsat::{Encoder, RsatError};

/// The udts whose closure the native encoder does not cover, with the reason. Asserted exactly:
/// a new fallback, or a fallback that went away, must be looked at.
const EXPECTED_FALLBACK: &[(u32, &str)] = &[
    // `process_debug_command` and `send_client_heartbeat`: one dynamic-flags field each.
    (33, "field kind 0x22"),
    (37, "field kind 0x22"),
];

#[test]
fn the_build_cells_agree_with_the_oracles_measured_table() {
    let Some(s) = inputs() else { return };
    let (slot, tag) = s.cells.typed_ref_handle;
    let oracle = d1_oracle::codec::codec_for(&s.image_sha256).expect("oracle codec set");
    assert!(
        oracle.type_cells.iter().any(|&(sl, _, t, _)| sl == slot && t == tag),
        "kind 0x29's handle cell {slot:#x} -> {tag:#010x} is not in d1-oracle's measured type cells"
    );
}

#[test]
fn every_pinned_udt_is_classified_native_or_named_fallback() {
    let Some(s) = inputs() else { return };
    let mut fallbacks = Vec::new();
    let all = ah_schemas();
    assert!(all.len() >= 30, "only {} AH schemas derived", all.len());
    for (udt, tag, _) in &all {
        if let Err(e) = s.check(*tag) {
            assert!(e.is_unsupported(), "udt {udt}: {e}");
            fallbacks.push((*udt, e.to_string()));
        }
    }
    let got: Vec<u32> = fallbacks.iter().map(|f| f.0).collect();
    let want: Vec<u32> = EXPECTED_FALLBACK.iter().map(|f| f.0).collect();
    assert_eq!(got, want, "fallbacks: {fallbacks:?}");
    for ((_, why), (_, want)) in fallbacks.iter().zip(EXPECTED_FALLBACK) {
        assert!(why.contains(want), "{why}");
    }
}

fn compare_udt(codec: &mut AhCodec, enc: &Encoder, udt: u32, tag: u32, image: &[u8], case: &str) {
    let o = codec.encode(udt, image).unwrap_or_else(|e| panic!("oracle udt {udt} {case}: {e}"));
    let n = if activity_host::is_replicated(udt) {
        enc.encode_replicated(tag, image)
    } else {
        enc.encode(tag, image)
    }
    .unwrap_or_else(|e| panic!("native udt {udt} {case}: {e}"));
    assert_eq!(
        (n.bits, hex(&n.payload)),
        (u64::from(o.bits), hex(&o.payload)),
        "udt {udt} (tag {tag:#010x}) {case}: native differs from the client's encoder; image {}",
        hex(image)
    );
}

#[test]
fn every_native_udt_matches_the_oracle_on_zero_and_generated_images() {
    let Some(s) = inputs() else { return };
    let enc = Encoder::new(s);
    let mut codec = AhCodec::new();
    let mut cases = 0usize;
    let mut udts = 0usize;
    for (udt, tag, size) in ah_schemas() {
        if s.check(tag).is_err() {
            continue;
        }
        udts += 1;
        compare_udt(&mut codec, &enc, udt, tag, &vec![0u8; size], "zero image");
        cases += 1;
        for seed in 0..16u64 {
            let mut g = Gen::new(s, seed * 1000 + u64::from(udt));
            for raw in [false, true] {
                let img = g.image(tag, raw);
                compare_udt(&mut codec, &enc, udt, tag, &img, &format!("seed {seed} raw {raw}"));
                cases += 1;
            }
        }
    }
    eprintln!("native == oracle on {cases} images across {udts} udts");
    assert!(udts >= 28, "only {udts} native udts were compared");
    assert_eq!(cases, udts * 33);
}

/// The hot messages named in the plan, with more seeds each.
#[test]
fn the_hot_udts_match_on_many_generated_images() {
    let Some(s) = inputs() else { return };
    let enc = Encoder::new(s);
    let mut codec = AhCodec::new();
    let mut cases = 0;
    for udt in [0u32, 1, 2, 4, 11, 18] {
        let tag = activity_host::tag_for_udt(udt).unwrap();
        s.check(tag).unwrap_or_else(|e| panic!("hot udt {udt} must be native: {e}"));
        for seed in 0..64u64 {
            let mut g = Gen::new(s, 0xD1_0000 + seed * 64 + u64::from(udt));
            let img = g.image(tag, seed % 2 == 1);
            compare_udt(&mut codec, &enc, udt, tag, &img, &format!("hot seed {seed}"));
            cases += 1;
        }
    }
    assert_eq!(cases, 6 * 64);
}

/// One field at a time: each leaf of udt 1, 2 and 18 set to a non-zero in-range value on an
/// otherwise zero image, so a mismatch names the field.
#[test]
fn single_field_variations_match() {
    let Some(s) = inputs() else { return };
    let enc = Encoder::new(s);
    let mut codec = AhCodec::new();
    let (mut cases, mut past_capacity) = (0, 0);
    for udt in [1u32, 2, 18, 0, 4] {
        let tag = activity_host::tag_for_udt(udt).unwrap();
        let d = s.get(tag).unwrap();
        let mut leaves = Vec::new();
        collect_leaves(s, d, 0, &mut leaves, 0);
        // A udt whose every leaf sits in an array (udt 0's slot mask) has none to vary here; the
        // generated-image tests cover it.
        for (at, f) in leaves {
            let mut img = vec![0u8; d.struct_size as usize];
            let width = f.width().clamp(1, 64);
            let wire = if width >= 64 { u64::MAX } else { (1u64 << width) - 1 };
            let raw = wire.wrapping_sub(u64::from(f.bias()));
            let bytes: Vec<u8> = match f.kind {
                2 => vec![1],
                3 | 7 => vec![raw as u8],
                4 | 8 => (raw as u16).to_le_bytes().to_vec(),
                6 | 10 => wire.wrapping_sub(i64::from(f.bias() as i32) as u64).to_le_bytes().to_vec(),
                11 => 1.5f32.to_bits().to_le_bytes().to_vec(),
                _ => (raw as u32).to_le_bytes().to_vec(),
            };
            put(&mut img, at, &bytes);
            // A leaf may be an array's count: its slots' typed references must not name type 0.
            Gen::new(s, 0).null_refs(tag, &mut img);
            // A count leaf at the top of its width can exceed its array's capacity (the launch
            // slots' 6-bit count says 63 of 50): the client would walk past the array, the native
            // encoder refuses. That refusal is the negative `refusals_are_named...` checks.
            let native = if activity_host::is_replicated(udt) { enc.encode_replicated(tag, &img) } else { enc.encode(tag, &img) };
            if matches!(native, Err(RsatError::Runtime { .. })) {
                past_capacity += 1;
                continue;
            }
            compare_udt(&mut codec, &enc, udt, tag, &img, &format!("field at +{at:#x}"));
            cases += 1;
        }
    }
    eprintln!("{cases} single-field cases identical; {past_capacity} past a capacity, refused natively");
    assert!(cases > 40);
}

/// Every leaf reachable with all counts zero (the fields a zero image actually walks).
fn collect_leaves(s: &Schemas, d: &Descriptor, obj: i64, out: &mut Vec<(i64, Field)>, depth: usize) {
    if d.is_array() || depth > 16 {
        return;
    }
    for f in &d.fields {
        let at = obj + i64::from(f.offset);
        if f.kind == KIND_COMPOSITE {
            if f.count_source() != 1 {
                collect_leaves(s, s.get(f.child).unwrap(), at, out, depth + 1);
            }
        } else if f.kind != KIND_TYPED_REF && !(0x15..=0x17).contains(&f.kind) {
            out.push((at, *f));
        }
    }
}

// ── udt 5's sub-objects ────────────────────────────────────────────────────────────────────────

fn compare_object(o: &mut ObjectOracle, enc: &Encoder, tag: u32, image: &[u8], replicated: bool, case: &str) {
    let want = if replicated { o.encode_replicated(image, 0x1_0000) } else { o.encode(image, 0x1_0000) }
        .unwrap_or_else(|e| panic!("oracle {tag:#010x} {case}: {e}"));
    let got = if replicated { enc.encode_replicated(tag, image) } else { enc.encode(tag, image) }
        .unwrap_or_else(|e| panic!("native {tag:#010x} {case}: {e}"));
    assert_eq!(
        (got.bits, hex(&got.payload)),
        (u64::from(want.bits), hex(&want.payload)),
        "{tag:#010x} {case}: image {}",
        hex(image)
    );
}

#[test]
fn sensor_name_value_and_authority_objects_match() {
    use d1_oracle::sensor::{SensorAuthority, SensorPhaseUpdate, TAG_AUTHORITY, TAG_NAME_REF, TAG_SENSOR_VALUE};
    let Some(s) = inputs() else { return };
    let enc = Encoder::new(s);
    let mut name = ObjectOracle::new(TAG_NAME_REF).unwrap();
    let mut value = ObjectOracle::new(TAG_SENSOR_VALUE).unwrap();
    let mut auth = ObjectOracle::new(TAG_AUTHORITY).unwrap();
    let mut cases = 0;
    for phase in -1i8..=14 {
        for tail in [None, Some((7u32, 0x1234_5678u32))] {
            let p = SensorPhaseUpdate {
                gen: 1,
                name_hash: 0x4786_C0E0,
                name_i8: 0x10,
                name_sub: 3,
                phase,
                tail,
                clients: Vec::new(),
                authority: None,
            };
            compare_object(&mut value, &enc, TAG_SENSOR_VALUE, &p.value_image(), false, &format!("phase {phase}"));
            compare_object(&mut name, &enc, TAG_NAME_REF, &p.name_ref_image(), false, "name");
            cases += 2;
        }
    }
    for (machine, seq, applied) in [(14u8, 1u16, true), (3, 0xFFFF, false), (0, 0, true), (63, 7, false)] {
        let a = SensorAuthority::all_to_with(machine, seq, applied);
        compare_object(&mut auth, &enc, TAG_AUTHORITY, &a.image, true, &format!("authority {machine}"));
        cases += 1;
    }
    for seed in 0..24u64 {
        let mut g = Gen::new(s, 0x5E45_0000 + seed);
        let raw = seed % 2 == 1;
        compare_object(&mut value, &enc, TAG_SENSOR_VALUE, &g.image(TAG_SENSOR_VALUE, raw), false, "gen value");
        compare_object(&mut name, &enc, TAG_NAME_REF, &g.image(TAG_NAME_REF, raw), false, "gen name");
        compare_object(&mut auth, &enc, TAG_AUTHORITY, &g.image(TAG_AUTHORITY, raw), true, "gen authority");
        cases += 3;
    }
    assert_eq!(cases, 16 * 2 * 2 + 4 + 24 * 3);
}

/// Kind 0x29 with live launch slots: null references, and a present one naming the handle type
/// itself (`0x80800046`, which the oracle's registry holds as a type cell).
#[test]
fn typed_references_in_launch_slots_match() {
    use d1_oracle::sensor::TAG_SENSOR_VALUE;
    let Some(s) = inputs() else { return };
    let enc = Encoder::new(s);
    let mut value = ObjectOracle::new(TAG_SENSOR_VALUE).unwrap();
    let (_, handle_tag) = s.cells.typed_ref_handle;
    // 0x808008A3 +0x10 is 0x8080533D: {u32 count (6 bits) @+0, 50 x 24-byte slots @+4}; a slot
    // (0x80800D17) is {u32 @+0, typed reference @+4}.
    let slots = 0x10 + 0x4;
    let mut cases = 0;
    for count in [1u32, 2, 50] {
        for present in [false, true] {
            let mut img = vec![0u8; 0x504];
            img[0x10..0x14].copy_from_slice(&count.to_le_bytes());
            for i in 0..count as usize {
                let e = slots + 0x18 * i;
                img[e..e + 4].copy_from_slice(&(0x100 + i as u32).to_le_bytes());
                let h = if present && i % 2 == 0 { handle_tag } else { u32::MAX };
                img[e + 4..e + 8].copy_from_slice(&h.to_le_bytes());
                // The payload of a present reference: another nullable handle, alternately set.
                let inner = if i % 4 == 0 { 0xABCD_0000 + i as u32 } else { u32::MAX };
                img[e + 8..e + 12].copy_from_slice(&inner.to_le_bytes());
            }
            compare_object(&mut value, &enc, TAG_SENSOR_VALUE, &img, false, &format!("{count} slots, present {present}"));
            cases += 1;
        }
    }
    assert_eq!(cases, 6);
}

// ── negatives ──────────────────────────────────────────────────────────────────────────────────

#[test]
fn refusals_are_named_and_never_silent() {
    let Some(s) = inputs() else { return };
    let enc = Encoder::new(s);
    // Dynamic flags: static refusal.
    let t33 = activity_host::tag_for_udt(33).unwrap();
    assert!(matches!(enc.encode(t33, &[]), Err(RsatError::Unsupported { .. })));
    // A count past an array's capacity: the client would read past the array; refuse. The sensor
    // value's launch slots (`+0x10`: a 6-bit count, then 50 slots) can say 63.
    let mut img = vec![0u8; 0x504];
    for i in 0..50 {
        img[0x18 + 0x18 * i..0x1C + 0x18 * i].copy_from_slice(&u32::MAX.to_le_bytes());
    }
    img[0x10..0x14].copy_from_slice(&51u32.to_le_bytes());
    assert!(matches!(enc.encode(0x8080_08A3, &img), Err(RsatError::Runtime { .. })), "count 51");
    img[0x10..0x14].copy_from_slice(&50u32.to_le_bytes());
    assert!(enc.encode(0x8080_08A3, &img).is_ok(), "count 50 is the capacity");
    // A wrong-size image.
    let t11 = activity_host::tag_for_udt(11).unwrap();
    assert!(matches!(enc.encode(t11, &vec![0u8; 0x5039]), Err(RsatError::BadImage { .. })));
    // An unknown tag.
    assert!(matches!(enc.encode(0x8080_33F4, &[]), Err(RsatError::UnknownTag(_))));
    // A typed reference naming a type with no descriptor.
    let mut v = vec![0u8; 0x504];
    v[0x10] = 1;
    v[0x18..0x1C].copy_from_slice(&0x1234_5678u32.to_le_bytes());
    assert!(matches!(enc.encode(0x8080_08A3, &v), Err(RsatError::Runtime { .. })));
}

/// The decoy: a different descriptor with the same struct size must NOT encode a join_result
/// image to the same bytes, or "native matched" would be size matching.
#[test]
fn a_same_size_decoy_schema_encodes_differently() {
    let Some(s) = inputs() else { return };
    let enc = Encoder::new(s);
    let t3 = activity_host::tag_for_udt(3).unwrap();
    let decoy = activity_host::DECOY_TAG_SAME_STRUCT_SIZE;
    assert_eq!(s.get(t3).unwrap().struct_size, s.get(decoy).unwrap().struct_size);
    let mut g = Gen::new(s, 3);
    let img = g.image(t3, false);
    if s.check(decoy).is_ok() {
        assert_ne!(enc.encode(t3, &img).unwrap(), enc.encode(decoy, &img).unwrap());
    }
}
