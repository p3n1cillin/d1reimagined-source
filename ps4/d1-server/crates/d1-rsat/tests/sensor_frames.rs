// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! ⭐ udt 5 natively: every sensor-list value in `ps4/fixtures/56578` through both codecs, and whole
//! `sensor_auth_update` frames assembled by [`d1_rsat::sensor::Frame`] against
//! `d1_oracle::sensor::SensorPhaseUpdate::encode` (the frames the host sends today), byte for byte,
//! with the native frame decoder against `d1_oracle::sensor::FrameDecoder`.
//!
//! Skips, saying why, without the owned 56578 executable (`D1_EBOOT`); counts are asserted.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::{hex, inputs, Gen};
use d1_oracle::sensor::{
    FrameDecoder, SensorAuthority, SensorClientRef, SensorPhaseUpdate, TYPED_VALUE_TYPES,
};
use d1_oracle::ObjectOracle;
use d1_rsat::schema::Schemas;
use d1_rsat::sensor::{self, Entry, Frame, NameRef};
use d1_rsat::{Decoder, Encoder};

fn fixtures() -> PathBuf {
    std::env::var_os("D1_FIXTURES")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/56578"))
}

/// Every sensor-list file and its parsed clients (world tables and step schedules parse to none).
fn sensor_lists() -> Vec<(String, Vec<SensorClientRef>, Option<SensorAuthority>)> {
    let mut out = Vec::new();
    for dir in ["mission", "postgame"] {
        let rd =
            std::fs::read_dir(fixtures().join(dir)).expect("fixture directory must be present");
        let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        files.sort();
        for p in files.into_iter().filter(|p| p.extension().is_some_and(|e| e == "txt")) {
            let text = std::fs::read_to_string(&p).unwrap();
            let clients: Vec<SensorClientRef> = text.lines().filter_map(SensorClientRef::parse_line).collect();
            let authority = text.lines().find_map(|l| {
                let l = l.split('#').next()?.trim();
                let v = l.strip_prefix("@authority")?.trim();
                let n: Vec<i64> = v
                    .split(',')
                    .map(|x| {
                        let x = x.trim();
                        x.strip_prefix("0x").map_or_else(|| x.parse().unwrap(), |h| i64::from_str_radix(h, 16).unwrap())
                    })
                    .collect();
                Some(SensorAuthority::all_to_with(
                    n[0] as u8,
                    n.get(1).copied().unwrap_or(1) as u16,
                    n.get(2).copied().unwrap_or(1) != 0,
                ))
            });
            // Include explicitly empty sensor lists as well; world tables and schedules
            // have no clients and do not use this filename prefix.
            let explicit_list=p.file_name().unwrap().to_string_lossy().starts_with("sensor-clients-");
            if !clients.is_empty() || authority.is_some() || explicit_list {
                out.push((format!("{dir}/{}", p.file_name().unwrap().to_string_lossy()), clients, authority));
            }
        }
    }
    out
}

/// The host's frame, natively: the lifetime record is the first entry, every client follows.
fn native_frame(p: &SensorPhaseUpdate) -> Frame {
    let mut entries = vec![Entry {
        name: NameRef { hash: p.name_hash, i8: p.name_i8, sub: p.name_sub },
        has_a: true,
        a_value: Some((sensor::TAG_SENSOR_VALUE, p.value_image())),
        has_b: false,
        b_value: None,
    }];
    entries.extend(p.clients.iter().map(|c| Entry {
        name: NameRef { hash: c.name_hash, i8: c.name_i8, sub: c.name_sub },
        has_a: c.has_a,
        a_value: c.a_value.clone(),
        has_b: c.has_b,
        b_value: c.b_value.clone(),
    }));
    Frame {
        gen: p.gen,
        authority: p.authority.as_ref().map(|a| (a.changed, a.image.clone())),
        stamp: 0,
        init: true,
        entries,
        pad: !p.clients.is_empty() || p.authority.is_some(),
    }
}

fn frame_is_native(s: &Schemas, p: &SensorPhaseUpdate) -> bool {
    p.clients
        .iter()
        .flat_map(|c| [c.a_value.as_ref().filter(|_| c.has_a), c.b_value.as_ref().filter(|_| c.has_b)])
        .flatten()
        .all(|(tag, _)| s.check(*tag).is_ok())
}

fn base(phase: i8) -> SensorPhaseUpdate {
    SensorPhaseUpdate {
        gen: 0,
        name_hash: 0x4786_C0E0,
        name_i8: 0x10,
        name_sub: 3,
        phase,
        tail: None,
        clients: Vec::new(),
        authority: None,
    }
}

/// Compare one whole frame: bytes, stamp bit, and both decoders' reading of it.
fn compare_frame(s: &Schemas, fd: &mut FrameDecoder, p: &SensorPhaseUpdate, case: &str) {
    let want = p.encode().unwrap_or_else(|e| panic!("oracle frame {case}: {e}"));
    let got = native_frame(p).encode(s).unwrap_or_else(|e| panic!("native frame {case}: {e}"));
    assert_eq!(hex(&got.bytes), hex(&want), "{case}: frame bytes");
    assert_eq!(Some(got.stamp_bit as usize), d1_oracle::sensor::known_stamp_bit(&want), "{case}: stamp bit");
    assert_eq!(sensor::stamp_bit(s, &want).unwrap(), got.stamp_bit, "{case}: native stamp_bit()");
    let o = fd.decode(&want).unwrap();
    let n = sensor::decode_frame(s, &want).unwrap();
    assert_eq!(
        (n.gen, n.stamp, n.init, n.bits, n.entries.len()),
        (o.gen, o.stamp, o.init, o.bits as u64, o.entries.len()),
        "{case}: decoded header"
    );
    assert_eq!(n.authority.as_ref().map(|a| (a.0, hex(&a.1))), o.authority.as_ref().map(|a| (a.changed, hex(&a.image))));
    for (ne, oe) in n.entries.iter().zip(&o.entries) {
        assert_eq!(
            (ne.name.hash, ne.name.i8, ne.name.sub, ne.body_bits, hex(&ne.body)),
            (oe.name_hash, oe.name_i8, oe.name_sub, oe.body_bits, hex(&oe.body)),
            "{case}: entry"
        );
    }
    // The lifetime entry's body decodes back to its value image.
    let b = sensor::decode_body(s, &n.entries[0].body, n.entries[0].body_bits, Some((sensor::TAG_SENSOR_VALUE, &[])), None)
        .unwrap();
    assert_eq!(b.a, Some((true, Some(p.value_image()))), "{case}: lifetime body");
}

#[test]
fn the_object_tags_and_sizes_match_the_oracles() {
    use d1_oracle::sensor as o;
    assert_eq!(
        (sensor::TAG_NAME_REF, sensor::TAG_SENSOR_VALUE, sensor::TAG_AUTHORITY, sensor::AUTHORITY_BLOCKS),
        (o::TAG_NAME_REF, o::TAG_SENSOR_VALUE, o::TAG_AUTHORITY, o::AUTHORITY_BLOCKS)
    );
    let Some(s) = inputs() else { return };
    assert_eq!(s.get(sensor::TAG_SENSOR_VALUE).unwrap().struct_size as usize, o::VALUE_IMAGE_LEN);
    assert_eq!(s.get(sensor::TAG_AUTHORITY).unwrap().struct_size as usize, 4 + 12 * o::AUTHORITY_BLOCKS);
}

/// ⭐ Every A and B value in every fixture sensor list, through both encoders and both decoders.
/// Values whose schema the native codec does not cover are counted by tag and reason.
#[test]
fn every_fixture_value_matches_the_client() {
    let Some(s) = inputs() else { return };
    let enc = Encoder::new(s);
    let dec = Decoder::new(s);
    let mut oracles: BTreeMap<u32, ObjectOracle> = BTreeMap::new();
    let (mut compared, mut lists) = (0usize, 0usize);
    let mut gated: BTreeMap<u32, (usize, String)> = BTreeMap::new();
    for (_, clients, _) in sensor_lists() {
        lists += 1;
        for c in &clients {
            for (tag, image) in [c.a_value.as_ref().filter(|_| c.has_a), c.b_value.as_ref().filter(|_| c.has_b)]
                .into_iter()
                .flatten()
            {
                if let Err(e) = s.check(*tag) {
                    assert!(e.is_unsupported(), "{tag:#010x}: {e}");
                    gated.entry(*tag).or_insert((0, e.to_string())).0 += 1;
                    continue;
                }
                let o = oracles
                    .entry(*tag)
                    .or_insert_with(|| ObjectOracle::with_types(*tag, TYPED_VALUE_TYPES).unwrap());
                let want = o.encode(image, 0x1_0000).unwrap();
                let got = enc.encode(*tag, image).unwrap();
                assert_eq!((got.bits, hex(&got.payload)), (u64::from(want.bits), hex(&want.payload)), "{tag:#010x} {}", hex(image));
                let od = o.decode(&want.payload).unwrap();
                let nd = dec.decode(*tag, &want.payload).unwrap();
                assert_eq!((nd.bits, hex(&nd.object)), (u64::from(od.bits), hex(&od.object)), "decode {tag:#010x}");
                compared += 1;
            }
        }
    }
    eprintln!("{compared} fixture values identical across {lists} lists; on the oracle by schema:");
    for (tag, (n, why)) in &gated {
        eprintln!("  {tag:#010x} x{n}: {why}");
    }
    assert!(
        lists >= 40,
        "only {lists} sensor lists found under {}",
        fixtures().display()
    );
    assert!(compared >= 782, "only {compared} values compared");
    assert!(
        gated.is_empty(),
        "fixture values still on the oracle: {gated:?}"
    );
}

/// Whole frames: the audited lifetime frame at every phase and with the tail, authority tables,
/// value-less A and B clients, valued A and B clients, and fixture lists whose values are native.
#[test]
fn whole_frames_match_the_hosts_frames() {
    let Some(s) = inputs() else { return };
    let mut fd = FrameDecoder::new().unwrap();
    let mut cases = 0;
    for phase in [-1i8, 3, 4, 6, 14] {
        compare_frame(s, &mut fd, &base(phase), &format!("audited phase {phase}"));
        cases += 1;
    }
    let mut p = base(3);
    p.tail = Some((0, 0xC2DA_CAF7));
    compare_frame(s, &mut fd, &p, "audited tail fix");
    p.authority = Some(SensorAuthority::all_to_with(14, 1, false));
    compare_frame(s, &mut fd, &p, "authority, unapplied");
    cases += 2;

    // Value-less clients (A only, A and B, B only), then valued ones of native types.
    let mut g = Gen::new(s, 0xF4A3);
    let mut p = base(6);
    p.gen = 2;
    p.authority = Some(SensorAuthority::all_to(3, 7));
    for (i, (a, b)) in [(true, false), (true, true), (false, true)].into_iter().enumerate() {
        p.clients.push(SensorClientRef {
            name_hash: 0x992B_B471,
            name_i8: 0x20,
            name_sub: i as i16,
            has_a: a,
            has_b: b,
            a_value: None,
            b_value: None,
        });
    }
    compare_frame(s, &mut fd, &p, "value-less clients");
    for (i, tag) in [0x8080_0463u32, 0x8080_071E, 0x8080_29A1].into_iter().enumerate() {
        p.clients.push(SensorClientRef {
            name_hash: 0x2FE7_DBE6,
            name_i8: 6,
            name_sub: 10 + i as i16,
            has_a: true,
            has_b: true,
            a_value: Some((tag, g.image(tag, false))),
            b_value: Some((0x8080_0463, g.image(0x8080_0463, false))),
        });
    }
    compare_frame(s, &mut fd, &p, "valued A and B clients");
    cases += 2;

    // Every fixture list must be fully native; do not filter away unsupported schemas.
    let mut native_lists: Vec<_> = sensor_lists()
        .into_iter()
        .filter_map(|(name, clients, authority)| {
            let mut p = base(6);
            p.clients = clients;
            p.authority = authority;
            assert!(
                frame_is_native(s, &p),
                "{name}: fixture is not fully native"
            );
            Some((name, p))
        })
        .collect();
    native_lists.sort_by_key(|(_, p)| {
        p.clients
            .iter()
            .filter(|c| c.a_value.is_some() || c.b_value.is_some())
            .count()
    });
    let mut fixture_frames = 0;
    let expected = native_lists.len();
    for (name, p) in native_lists {
        compare_frame(s, &mut fd, &p, &name);
        fixture_frames += 1;
        eprintln!("fixture frame {fixture_frames}/{expected}: {name}, identical");
    }
    eprintln!(
        "{} whole frames identical, {fixture_frames} of them fixture lists",
        cases + fixture_frames
    );
    assert_eq!(fixture_frames, expected);
    assert!(
        fixture_frames >= 65,
        "only {fixture_frames} fully native fixture lists"
    );
}

#[test]
fn explicitly_empty_fixture_lists_are_also_native() {
    let Some(s)=inputs() else {return};
    let mut fd=FrameDecoder::new().unwrap();
    let mut cases=0;
    for (name,clients,authority) in sensor_lists().into_iter().filter(|(_,c,_)| c.is_empty()) {
        let mut p=base(6);p.clients=clients;p.authority=authority;
        compare_frame(s,&mut fd,&p,&name);cases+=1;
    }
    assert!(cases>=1,"the empty cinematic sensor-list fixture was omitted");
    eprintln!("{cases} explicitly empty fixture lists byte-identical");
}

/// Offline wall time per complete udt 5 frame, including image cloning, schema routing and
/// packing. Oracle `SensorPhaseUpdate::encode` constructs its object harnesses on each call,
/// exactly as the host's oracle frame builder does. Both use warm filesystem/control state.
/// Run alone with `cargo test -p d1-rsat --release --test sensor_frames latency -- --ignored --nocapture`.
#[test]
#[ignore = "latency measurement; run explicitly in release mode, without competing tests"]
fn latency_per_fixture_udt5_frame() {
    use std::{hint::black_box, time::Instant};
    let Some(s) = inputs() else { return };
    assert!(!cfg!(debug_assertions), "measure with --release");
    let mut lists: Vec<_> = sensor_lists()
        .into_iter()
        .map(|(name, clients, authority)| {
            let mut p = base(6);
            p.clients = clients;
            p.authority = authority;
            let size = native_frame(&p).encode(s).unwrap().bytes.len();
            (size, name, p)
        })
        .collect();
    assert!(lists.len() >= 64);
    lists.sort_by_key(|p| p.0);
    let eligible = lists.len();
    let selected = [0, eligible / 2, eligible - 1];
    let mut native_times = Vec::new();
    let mut oracle_times = Vec::new();
    let mut mission = 0;
    for index in selected {
        let (_, name, p) = &lists[index];
        assert!(frame_is_native(s, &p), "{name}");
        let native = || {
            assert!(frame_is_native(s, black_box(&p)));
            let b = native_frame(black_box(&p)).encode(s).unwrap();
            // Same stamp-cache insertion the host does on either path.
            d1_oracle::sensor::remember_stamp_bit(&b.bytes, b.stamp_bit as usize);
            b.bytes
        };
        let control = p.encode().unwrap(); // control and cold filesystem work outside timed samples
        assert_eq!(native(), control, "{name}");
        let mut nt = Vec::new();
        let mut ot = Vec::new();
        for _ in 0..5 {
            let start = Instant::now();
            for _ in 0..100 {
                black_box(native());
            }
            nt.push(start.elapsed().as_secs_f64() * 1e6 / 100.0);
            let start = Instant::now();
            black_box(p.encode().unwrap());
            ot.push(start.elapsed().as_secs_f64() * 1e6);
        }
        nt.sort_by(f64::total_cmp);
        ot.sort_by(f64::total_cmp);
        eprintln!("BENCH {name}: {} bytes, native {:.3} us/frame, oracle {:.3} us/frame, {:.1}x (median of 5)",
            control.len(),nt[2],ot[2],ot[2]/nt[2]);
        native_times.push(nt[2]);
        oracle_times.push(ot[2]);
        mission += usize::from(name.starts_with("mission/"));
    }
    native_times.sort_by(f64::total_cmp);
    oracle_times.sort_by(f64::total_cmp);
    let n = native_times.len();
    let p95 = (n * 95).div_ceil(100) - 1;
    eprintln!("BENCH SUMMARY: {n} size-selected lists ({mission} mission) from {eligible} eligible, 500 native + 5 oracle calls/list; per-list medians distribution:");
    eprintln!(
        "native us/frame min {:.3} p50 {:.3} p95 {:.3} max {:.3}",
        native_times[0],
        native_times[n / 2],
        native_times[p95],
        native_times[n - 1]
    );
    eprintln!(
        "oracle us/frame min {:.3} p50 {:.3} p95 {:.3} max {:.3}",
        oracle_times[0],
        oracle_times[n / 2],
        oracle_times[p95],
        oracle_times[n - 1]
    );
}
