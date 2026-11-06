// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! 56578 handler controls and nonempty union/identity/position comparisons. Offline only.
mod common;

use common::{inputs, put, Gen};
use d1_eboot::{image::Image, reloc};
use d1_oracle::{harness::Stub, ObjectOracle};
use d1_rsat::schema::IdentityFormat;
use d1_rsat::{Decoder, Encoder, RsatError, Schemas};

fn compare(s: &Schemas, oracle: &mut ObjectOracle, tag: u32, image: &[u8]) {
    let n = Encoder::new(s).encode(tag, image).unwrap();
    let o = oracle.encode(image, 0x10000).unwrap();
    assert_eq!(
        (n.bits, &n.payload),
        (o.bits as u64, &o.payload),
        "encode {tag:#x}"
    );
    let nd = Decoder::new(s).decode(tag, &o.payload).unwrap();
    let od = oracle.decode(&o.payload).unwrap();
    assert_eq!(nd.bits, od.bits as u64, "decode bits {tag:#x}");
    if nd.object != od.object {
        let i = nd
            .object
            .iter()
            .zip(&od.object)
            .position(|(a, b)| a != b)
            .unwrap();
        panic!(
            "decode {tag:#x} differs at +{i:#x}: native {:02x}, oracle {:02x}",
            nd.object[i], od.object[i]
        );
    }
}

#[test]
fn handlers_and_all_type_cells_are_build_56578_evidence() {
    let Some(s) = inputs() else { return };
    let image = Image::open_default().unwrap();
    let relocs = reloc::all(&image).unwrap();
    for (kind, enc, dec) in [
        (0x0D, 0x1028780, 0x102C8D0),
        (0x22, 0x102A3D0, 0x102D220),
        (0x23, 0x102A450, 0x102D280),
        (0x26, 0x102A500, 0x102D330),
    ] {
        for (base, target) in [(0x1912C70, enc), (0x19131B0, dec)] {
            assert_eq!(
                relocs
                    .iter()
                    .find(|r| r.slot == base + kind * 8)
                    .unwrap()
                    .addend,
                target
            );
        }
    }
    let c = s.cells;
    let oracle = d1_oracle::codec::codec_for(&s.image_sha256).unwrap();
    for &(slot, tag) in std::iter::once(&c.typed_ref_handle)
        .chain(&c.identity)
        .chain(&c.atom_selectors)
        .chain(c.atom_primary)
        .chain(c.atom_secondary)
    {
        assert!(oracle
            .type_cells
            .iter()
            .any(|&(sl, _, t, _)| sl == slot && t == tag));
    }
    for ((_, _, slots), native) in oracle
        .type_cell_caches
        .iter()
        .zip([c.atom_primary, c.atom_secondary])
    {
        assert_eq!(*slots, native.iter().map(|c| c.0).collect::<Vec<_>>());
    }
}

#[test]
fn both_identity_arms_match_with_non_symmetric_bytes() {
    let Some(_) = inputs() else { return };
    let image = Image::open_default().unwrap();
    let mut s = Schemas::load(&image).unwrap();
    let tag = 0x80800646;
    let mut o = ObjectOracle::new(tag).unwrap();
    let mut cases = 0;
    for format in [IdentityFormat::Bytes, IdentityFormat::Platform] {
        s.identity_format = format;
        o.harness().stub(
            s.cells.identity_selector_fn,
            Stub::Return(u64::from(format == IdentityFormat::Platform)),
        );
        for seed in 0..16 {
            let mut img = Gen::new(&s, seed + 0x2300).image(tag, false);
            for (i, b) in img[..8].iter_mut().enumerate() {
                *b = (seed * 11 + i as u64 * 29) as u8;
            }
            compare(&s, &mut o, tag, &img);
            cases += 1;
        }
    }
    eprintln!("{cases} identity images identical, both selector arms");
    assert_eq!(cases, 32);
}

#[test]
fn typed_values_in_33_37_and_prop_state_are_objects_not_bitsets() {
    let Some(s) = inputs() else { return };
    let mut cases = 0;
    // These payloads have distinct sizes and include an empty schema and a quantized real32.
    let types = [
        s.cells.typed_ref_handle.1,
        0x8080279A,
        0x80800842,
        0x8080046B,
    ];
    for udt in [33, 37] {
        let tag = d1_oracle::activity_host::tag_for_udt(udt).unwrap();
        assert!(d1_oracle::activity_host::dynamic_flags_reason(tag)
            .unwrap()
            .is_none());
        let mut o = ObjectOracle::with_types(tag, &types).unwrap();
        for t in std::iter::once(u32::MAX).chain(types) {
            for seed in 0..8 {
                let mut image = vec![0xA7; s.get(tag).unwrap().struct_size as usize];
                put(&mut image, 0, &t.to_le_bytes());
                if t != u32::MAX {
                    put(&mut image, 16, &Gen::new(s, seed + 0x2200).image(t, false));
                }
                compare(s, &mut o, tag, &image);
                cases += 1;
            }
        }
    }
    eprintln!("{cases} typed-value AH images identical, null and four payload types");
    assert_eq!(cases, 80);
    let tag = 0x808007D9;
    let mut o = ObjectOracle::with_types(tag, &types).unwrap();
    for count in [0, 1, 3] {
        let mut image = Gen::new(s, 0x2200).image(tag, false);
        put(&mut image, 0x20, &(count as u32).to_le_bytes());
        for i in 0..count {
            let t = types[i as usize];
            let at = 0x30 + i * 64;
            put(&mut image, at, &t.to_le_bytes());
            put(
                &mut image,
                at + 16,
                &Gen::new(s, i as u64 + 0x22).image(t, false),
            );
        }
        compare(s, &mut o, tag, &image);
    }
    eprintln!("3 prop-state images identical, 0/1/3 live typed commands");
}

#[test]
fn every_atom_variant_and_quantized_secondary_matches() {
    let Some(s) = inputs() else { return };
    let tag = 0x80804242; // array of atoms; full mask includes all 32 elements
    let mut o = ObjectOracle::new(tag).unwrap();
    let mut cases = 0;
    for p in 0..s.cells.atom_primary.len() {
        for q in 0..s.cells.atom_secondary.len() {
            let mut image = vec![0x6B; s.get(tag).unwrap().struct_size as usize];
            for i in 0..32 {
                let at = (i * 64) as i64;
                put(&mut image, at, &[p as u8]);
                put(&mut image, at + 4, &[q as u8]);
                put(
                    &mut image,
                    at + 16,
                    &Gen::new(s, i + 0x2600).image(s.cells.atom_primary[p].1, false),
                );
                put(
                    &mut image,
                    at + 8,
                    &Gen::new(s, i + 0x2601).image(s.cells.atom_secondary[q].1, false),
                );
            }
            compare(s, &mut o, tag, &image);
            cases += 1;
        }
    }
    eprintln!("{cases} atom arrays identical, all primary/secondary combinations");
    assert_eq!(
        cases,
        s.cells.atom_primary.len() * s.cells.atom_secondary.len()
    );
    let mut bad = vec![0; s.get(tag).unwrap().struct_size as usize];
    bad[0] = 255;
    assert!(matches!(
        Encoder::new(s).encode(tag, &bad),
        Err(RsatError::Runtime { .. })
    ));
}

#[test]
fn positions_preserve_xyz_bits_and_set_the_fourth_lane() {
    let Some(s) = inputs() else { return };
    let tag = 0x8080293C;
    let mut o = ObjectOracle::new(tag).unwrap();
    let values: [u32; 8] = [
        0, 0x80000000, 1, 0x7f800000, 0xff800000, 0x7fc12345, 0x3f800000, 0xC1234567,
    ];
    for (i, v) in values.into_iter().enumerate() {
        let mut img = Gen::new(s, i as u64).image(tag, false);
        for lane in 0..4 {
            put(&mut img, 16 + lane * 4, &v.to_le_bytes());
        }
        compare(s, &mut o, tag, &img);
    }
    eprintln!(
        "{} position images identical, including IEEE edge cases",
        values.len()
    );
}

#[test]
fn every_atom_quantized_code_and_ieee_encode_edge_matches() {
    let Some(s) = inputs() else { return };
    let tag = s.cells.atom_secondary[2].1;
    let d = s.get(tag).unwrap();
    let width = d.fields[0].params[2];
    assert_eq!(
        width, 11,
        "the exhaustive control is bounded to this schema"
    );
    let mut o = ObjectOracle::new(tag).unwrap();
    let mut cases = 0;
    for code in 0..1u64 << width {
        let mut w = d1_rsat::bits::BitWriter::new();
        w.write(code, width);
        let (_, wire) = w.finish();
        let n = Decoder::new(s).decode(tag, &wire).unwrap();
        let od = o.decode(&wire).unwrap();
        assert_eq!(
            (n.bits, &n.object),
            (od.bits as u64, &od.object),
            "quantized code {code}"
        );
        compare(s, &mut o, tag, &n.object);
        cases += 1;
    }
    for v in [
        0u32, 0x80000000, 1, 0x7F800000, 0xFF800000, 0x7FC12345, 0x7F7FFFFF, 0xFF7FFFFF,
    ] {
        compare(s, &mut o, tag, &v.to_le_bytes());
        cases += 1;
    }
    eprintln!("{cases} quantized real32 cases identical, every wire code and IEEE encode edges");
    assert_eq!(cases, 2056);
}
