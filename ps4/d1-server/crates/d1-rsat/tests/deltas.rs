// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Caller-mask replicated deltas, against the client's mask-taking entry, not a composition.
mod common;
use common::{inputs, Gen};
use d1_oracle::ObjectOracle;
use d1_rsat::{Decoder, Encoder, RsatError};

fn full_mask(n: u32) -> Vec<u32> {
    let mut words = vec![u32::MAX; n.div_ceil(32) as usize];
    if n % 32 != 0 {
        *words.last_mut().unwrap() = (1 << (n % 32)) - 1;
    }
    words
}

#[test]
fn explicit_masks_encode_byte_identically_and_overlay_the_same_seed() {
    let Some(s) = inputs() else { return };
    let mut cases = 0;
    for tag in [
        d1_oracle::activity_host::tag_for_udt(11).unwrap(),
        d1_oracle::activity_host::tag_for_udt(21).unwrap(),
        d1_rsat::sensor::TAG_AUTHORITY,
    ] {
        let d = s.get(tag).unwrap();
        let n = d.mask_bits + 1;
        let full = full_mask(n);
        let mut o = ObjectOracle::new(tag).unwrap();
        let enc = Encoder::new(s);
        let dec = Decoder::new(s);
        let mut native_prior = Gen::new(s, 0x5eed).image(tag, false);
        let mut oracle_prior = native_prior.clone();
        let mut masks = vec![vec![0; full.len()], full.clone()];
        // Every one-hot mask, with root presence. Parent optional fields must also
        // be selected to reach a child; random masks below exercise those paths.
        for bit in 0..n {
            let mut m = vec![0; full.len()];
            m[0] = 1;
            m[(bit / 32) as usize] |= 1 << (bit % 32);
            masks.push(m);
        }
        for seed in 0..32 {
            let mut g = Gen::new(s, seed + 0xD317A);
            let mut m: Vec<u32> = full.iter().map(|v| g.next() as u32 & v).collect();
            m[0] |= 1;
            masks.push(m);
        }
        let mut changed = 0;
        for (i, mask) in masks.iter().enumerate() {
            let image = Gen::new(s, i as u64 + 0xD317).image(tag, false);
            let nw = enc.encode_replicated_masked(tag, &image, mask).unwrap();
            let ow = o.encode_replicated_masked(&image, mask, 0x10000).unwrap();
            assert_eq!(
                (nw.bits, &nw.payload),
                (ow.bits as u64, &ow.payload),
                "tag {tag:#x} mask {i}"
            );
            if *mask == full {
                let control = o.encode_replicated(&image, 0x10000).unwrap();
                assert_eq!(
                    (ow.bits, &ow.payload),
                    (control.bits, &control.payload),
                    "full-mask entry control"
                );
            }
            let nd = dec
                .decode_replicated(tag, &ow.payload, &native_prior)
                .unwrap();
            let od = o.decode_replicated(&ow.payload, &oracle_prior).unwrap();
            assert_eq!(
                (nd.bits, &nd.object),
                (od.bits as u64, &od.object),
                "overlay {tag:#x} mask {i}"
            );
            changed += usize::from(nd.object != native_prior);
            if mask.iter().all(|w| *w == 0) {
                assert_eq!(nw.bits, 1);
                assert_eq!(nd.object, native_prior);
            }
            native_prior = nd.object;
            oracle_prior = od.object;
            cases += 1;
        }
        assert!(changed > 1, "no nontrivial overlay for {tag:#x}");
        assert!(matches!(
            enc.encode_replicated_masked(tag, &[], &[]),
            Err(RsatError::Runtime { .. })
        ));
    }
    eprintln!("{cases} replicated deltas byte-identical; ordered nonzero-seed overlays identical");
    assert!(cases > 100);
}
