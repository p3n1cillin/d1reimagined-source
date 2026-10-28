// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Synthetic native-atom images; no captures or game content are embedded.
use d1_oracle::object::ObjectOracle;

#[test]
fn member_snap_and_move_round_trip_through_56578() {
    let Ok(image) = d1_eboot::image::Image::open_default() else {
        eprintln!("SKIP: owned executable unavailable");
        return;
    };
    if image.sha256() != "672f03411c0503dfbcda804cbb9fb27cdd9a8d73a8bcb71bf0c4f688073b0833" {
        eprintln!("SKIP: native atom tables are measured for 56578 only");
        return;
    }
    let mut body = vec![0u8; 2312];
    body[..4].copy_from_slice(&1u32.to_le_bytes());
    body[4] = 0xff;
    body[6] = 1;
    body[0x50..0x54].copy_from_slice(&0x811c9dc5u32.to_le_bytes());
    body[0x54] = 0xff;
    body[0x56..0x58].copy_from_slice(&u16::MAX.to_le_bytes());
    body[0x58..0x5c].copy_from_slice(&u32::MAX.to_le_bytes());
    body[0xb0..0xb4].copy_from_slice(&1u32.to_le_bytes());
    body[0xb8..0xbc].copy_from_slice(&2u32.to_le_bytes());
    for (i, (selector, marker)) in [(0u8, 0u32), (3, 1)].into_iter().enumerate() {
        let at = 0xc0 + 64 * i;
        body[at] = selector;
        body[at + 0x10..at + 0x14].copy_from_slice(&0x12345678u32.to_le_bytes());
        body[at + 0x14] = 0x38;
        body[at + 0x16..at + 0x18].copy_from_slice(&7u16.to_le_bytes());
        body[at + 0x18..at + 0x1c].copy_from_slice(&marker.to_le_bytes());
    }
    let mut o = ObjectOracle::new(0x80800868).expect("positive control and native tables");
    let (encoded, decoded) = o.round_trip(&body, 0x10000).expect("snap + move");
    assert_eq!(encoded.bits, 689);
    assert_eq!(decoded.bits, encoded.bits);
    let (replicated, decoded) = o
        .round_trip_replicated(&body, 0x10000)
        .expect("replicated member A");
    assert_eq!(replicated.bits, 690);
    assert_eq!(decoded.bits, replicated.bits);
    // A different marker must survive both codecs and change the wire image.
    body[0x118..0x11c].copy_from_slice(&2u32.to_le_bytes());
    let (other, _) = o.round_trip(&body, 0x10000).expect("changed marker");
    assert_ne!(other.payload, encoded.payload);
    // Exercise the same sensor-frame builder the host's `clients` control uses.
    // This catches missing variant setup beyond a standalone object round trip.
    let line = format!("0x12345678 2 7 1 1 0x80800868 {}",
        body.iter().map(|b| format!("{b:02x}")).collect::<String>());
    let client = d1_oracle::sensor::SensorClientRef::parse_line(&line).unwrap();
    let params = d1_oracle::sensor::SensorPhaseUpdate {
        gen: 0, name_hash: 0x12345678, name_i8: 16, name_sub: 3,
        phase: 3, tail: None, clients: vec![client], authority: None,
    };
    let with_program = params.encode().expect("host sensor frame with program");
    let without = d1_oracle::sensor::SensorPhaseUpdate { clients: vec![], ..params }
        .encode().expect("host sensor frame without program");
    assert!(with_program.len() > without.len() + 80);
}

#[test]
fn world_globals_period_reaches_the_native_clock_configuration() {
    let Ok(image) = d1_eboot::image::Image::open_default() else {
        eprintln!("SKIP: owned executable unavailable");
        return;
    };
    if image.sha256() != "672f03411c0503dfbcda804cbb9fb27cdd9a8d73a8bcb71bf0c4f688073b0833" {
        eprintln!("SKIP: world-globals addresses are measured for 56578 only");
        return;
    }
    assert_eq!(
        d1_oracle::activity_host::tag_for_udt(2).unwrap(),
        0x80800dc1
    );
    let mut body = [0u8; 8];
    body[0] = 1;
    body[4..].copy_from_slice(&(1000.0f32 / 30.0).to_le_bytes());
    let mut o = ObjectOracle::new(0x80800dc1).expect("positive control");
    let (encoded, decoded) = o.round_trip(&body, 0x10000).expect("world globals");
    assert_eq!(encoded.bits, 33);
    assert_eq!(decoded.bits, 33);
    let h = o.harness();
    let world = h.alloc(0x40).unwrap();
    let config = h.alloc(8).unwrap();
    h.write(config, &decoded.object).unwrap();
    h.call(0x2a2ca0, &[world, config])
        .expect("native configuration setter");
    assert_eq!(h.read(world + 0x14, 8).unwrap(), body);
    assert_eq!(h.read(world + 0x10, 1).unwrap(), [1]);
}
