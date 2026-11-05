// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! ⭐ The responder's objects natively: the profile (`0x80801743`), the inventory item
//! (`0x80801732`), the character (`0x80801954`, the oracle's own positive control) and the roster
//! records, against `d1_oracle` on generated images; and every served profile and item body in
//! `ps4/fixtures/56578` decoded natively, compared with the client's decode, and re-encoded to the
//! very bytes the responder serves.
//!
//! Skips, saying why, without the owned 56578 executable (`D1_EBOOT`); counts are asserted.

mod common;

use std::path::{Path, PathBuf};

use common::{hex, inputs, Gen};
use d1_oracle::ObjectOracle;
use d1_rsat::{Decoder, Encoder};

const PROFILE: u32 = 0x8080_1743;
const ITEM: u32 = 0x8080_1732;

/// The responder's object schemas: profile, item, character, and the roster's contact, card, peer
/// and peer-contact records (`d1_oracle::{profile, inventory, character, roster}`).
const RESPONDER_TAGS: &[u32] =
    &[PROFILE, ITEM, 0x8080_1954, 0x8080_18B7, 0x8080_1810, 0x8080_17FD, 0x8080_1921];

fn fixtures() -> PathBuf {
    std::env::var_os("D1_FIXTURES")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/56578"))
}

fn files(dir: &Path, pred: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    v.retain(|p| p.file_name().is_some_and(|n| pred(&n.to_string_lossy())));
    v.sort();
    v
}

#[test]
fn the_responder_schemas_are_native() {
    let Some(s) = inputs() else { return };
    for &t in RESPONDER_TAGS {
        s.check(t).unwrap_or_else(|e| panic!("{t:#010x}: {e}"));
        s.check_decode(t).unwrap_or_else(|e| panic!("{t:#010x}: {e}"));
    }
    // Every descriptor the client's message codec also reaches from the character message.
    assert!(s.check(0x8080_1869).is_ok() && s.check(0x8080_1825).is_ok());
}

#[test]
fn responder_objects_match_on_generated_images() {
    let Some(s) = inputs() else { return };
    let enc = Encoder::new(s);
    let dec = Decoder::new(s);
    let (mut cases, mut past_capacity) = (0, 0);
    for &tag in RESPONDER_TAGS {
        let mut o = ObjectOracle::new(tag).unwrap();
        let size = s.get(tag).unwrap().struct_size as usize;
        let mut images = vec![vec![0u8; size]];
        for seed in 0..6u64 {
            images.push(Gen::new(s, 0x5E5_0000 + seed * 31 + u64::from(tag & 0xFFFF)).image(tag, seed % 2 == 1));
        }
        for (i, img) in images.iter().enumerate() {
            // Two arrays can read one count field (the profile's does, capacities 20 and more):
            // a count set for the larger walks the smaller past its end in the client, and the
            // native encoder refuses that image instead. Counted, never compared.
            let got = match enc.encode(tag, img) {
                Err(d1_rsat::RsatError::Runtime { .. }) => {
                    past_capacity += 1;
                    continue;
                }
                r => r.unwrap_or_else(|e| panic!("native {tag:#010x} image {i}: {e}")),
            };
            let want = o.encode(img, 0x10_0000).unwrap_or_else(|e| panic!("oracle {tag:#010x} image {i}: {e}"));
            assert_eq!((got.bits, hex(&got.payload)), (u64::from(want.bits), hex(&want.payload)), "{tag:#010x} image {i}");
            // The same holds on the way back: a generated count that its leaf cannot carry (a
            // width or bias) decodes to another count, which can pass a capacity.
            let nd = match dec.decode(tag, &want.payload) {
                Err(d1_rsat::RsatError::Runtime { .. }) => {
                    past_capacity += 1;
                    continue;
                }
                r => r.unwrap_or_else(|e| panic!("native decode {tag:#010x} image {i}: {e}")),
            };
            let od = o.decode(&want.payload).unwrap();
            assert_eq!(nd.bits, u64::from(od.bits), "{tag:#010x} image {i}: decode bits");
            assert!(nd.object == od.object, "{tag:#010x} image {i}: decoded images differ");
            cases += 1;
        }
    }
    eprintln!("{cases} responder images identical; {past_capacity} refused natively (a count past a capacity)");
    assert_eq!(cases + past_capacity, RESPONDER_TAGS.len() * 7);
    assert!(cases >= RESPONDER_TAGS.len() * 3, "{cases} compared");
}

/// ⭐ The bodies the responder actually serves: decode natively, equal the client's decode, and
/// re-encode natively to the served bytes exactly.
#[test]
fn served_profile_and_item_bodies_round_trip_natively() {
    let Some(s) = inputs() else { return };
    let enc = Encoder::new(s);
    let dec = Decoder::new(s);
    let f = fixtures();
    let mut bodies: Vec<(u32, PathBuf)> = Vec::new();
    for dir in [f.join("characters"), f.join("characters/grants"), f.join("postgame")] {
        bodies.extend(files(&dir, |n| n.starts_with("profile") && n.ends_with(".bin")).into_iter().map(|p| (PROFILE, p)));
        bodies.extend(files(&dir, |n| n.contains("profile") && n.ends_with(".draft.bin")).into_iter().map(|p| (PROFILE, p)));
    }
    for dir in [f.join("characters/inventory"), f.join("characters/inventory-tutorial")] {
        bodies.extend(files(&dir, |n| n.ends_with(".bin")).into_iter().map(|p| (ITEM, p)));
    }
    bodies.extend(files(&f.join("characters/grants"), |n| n.starts_with("khvostov")).into_iter().map(|p| (ITEM, p)));
    bodies.extend(files(&f.join("postgame"), |n| n.ends_with("inventory.draft.bin")).into_iter().map(|p| (ITEM, p)));
    bodies.sort();
    bodies.dedup();
    let mut profile = ObjectOracle::new(PROFILE).unwrap();
    let mut item = ObjectOracle::new(ITEM).unwrap();
    let (mut profiles, mut items) = (0, 0);
    for (tag, path) in &bodies {
        let body = std::fs::read(path).unwrap();
        let name = path.display();
        let nd = dec.decode(*tag, &body).unwrap_or_else(|e| panic!("{name}: {e}"));
        let o = if *tag == PROFILE { &mut profile } else { &mut item };
        let od = o.decode(&body).unwrap_or_else(|e| panic!("oracle {name}: {e}"));
        assert_eq!(nd.bits, u64::from(od.bits), "{name}: bits");
        assert!(nd.object == od.object, "{name}: decoded images differ");
        let back = enc.encode(*tag, &nd.object).unwrap();
        assert!(back.payload == body, "{name}: the native re-encode is not the served body");
        if *tag == PROFILE {
            profiles += 1;
        } else {
            items += 1;
        }
    }
    eprintln!("{profiles} served profiles and {items} served items round-trip natively");
    assert!(profiles >= 6 && items >= 20, "{profiles} profiles, {items} items");

    // The vendor's struct images (`.obj`): the profile the responder applies purchases to and
    // the item template.
    for (tag, file, o) in [(PROFILE, "vendor-profile.obj", &mut profile), (ITEM, "vendor-item-template.obj", &mut item)] {
        let img = std::fs::read(f.join("vendor").join(file)).unwrap();
        let want = o.encode(&img, 0x10_0000).unwrap();
        let got = enc.encode(tag, &img).unwrap();
        assert_eq!((got.bits, hex(&got.payload)), (u64::from(want.bits), hex(&want.payload)), "{file}");
    }
}
