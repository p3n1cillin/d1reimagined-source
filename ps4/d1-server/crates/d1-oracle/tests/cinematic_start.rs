// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Execute the 56578 cinematic setter with synthetic state and stubbed engine boundaries.
//! Reaching the start boundary is an offline result, not proof of playback or incident delivery.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use d1_eboot::{descriptors, image::Image};
use d1_oracle::harness::{Harness, Stub};
use unicorn_engine::RegisterX86;

const APPLY: u64 = 0x403140;
const LOCAL_PLAYER: u64 = 0x303BA0;
const START: u64 = 0x403770;
const VALUE: u64 = 0x1AC;

#[test]
fn cinematic_start_requires_a_new_counter_and_retries_only_on_another_change() {
    let Some(path) = std::env::var_os("D1_EBOOT") else {
        eprintln!("SKIP: set D1_EBOOT to the owned build 56578 executable");
        return;
    };
    if !Path::new(&path).is_file() {
        eprintln!("SKIP: D1_EBOOT does not name a present executable");
        return;
    }
    let image = Image::open(path).expect("read supplied executable");
    assert_eq!(
        image.sha256(),
        d1_oracle::weapon_stats::MEASURED_EBOOT_SHA256,
        "cinematic addresses were re-derived only for build 56578"
    );
    let schemas = descriptors::index(&image).unwrap();
    let schema = schemas.get(0x8080254D).expect("cinematic schema");
    assert_eq!(schema.struct_size, 0xD4);
    assert_eq!(
        descriptors::fields(&image, schema.anchor, schema.field_count)
            .into_iter()
            .map(|(_, f)| f.expect("valid field record").offset)
            .collect::<Vec<_>>(),
        [0, 4, 8, 9, 12, 20, 24, 156, 208]
    );

    let mut h = Harness::new(&image).unwrap();
    let record = h.alloc(0x290).unwrap();
    let incoming = h.alloc(0xD4).unwrap();
    let player = h.alloc(0x100).unwrap();
    h.write_u64(0x2689580 + 8, player).unwrap();
    h.write_u32(0x2689580 + 0x10, 0x100).unwrap();
    h.stub(LOCAL_PLAYER, Stub::Return(0));
    h.stub(START, Stub::Return(1));
    h.stub(0x301C80, Stub::Return(2)); // synthetic local team
    h.stub(0x402DE0, Stub::Return(0)); // stop is outside this experiment

    // Observe boundaries only; the existing Harness hook owns all stub execution.
    let visits = Rc::new(RefCell::new(Vec::new()));
    let observed = Rc::clone(&visits);
    h.uc()
        .add_code_hook(LOCAL_PLAYER, START, move |_, address, _| {
            if address == LOCAL_PLAYER || address == START {
                observed.borrow_mut().push(address);
            }
        })
        .unwrap();

    let mut value = vec![0u8; 0xD4];
    // The real name-ref predicate treats this as absent. No captured identity or game content.
    value[12..16].copy_from_slice(&0x811C9DC5u32.to_le_bytes());
    value[16] = 0xFF;
    value[18..20].copy_from_slice(&u16::MAX.to_le_bytes());
    value[20] = 0xFF; // no team filter

    let mut apply =
        |h: &mut Harness, counter: u32, playing: u8, local: u64, team: u8, count: u32| {
            visits.borrow_mut().clear();
            value[4..8].copy_from_slice(&counter.to_le_bytes());
            value[8] = playing;
            value[20] = team;
            value[24..28].copy_from_slice(&count.to_le_bytes());
            h.stub(LOCAL_PLAYER, Stub::Return(local));
            h.write(incoming, &value).unwrap();
            h.call(APPLY, &[record, incoming])
                .expect("execute real cinematic setter");
            assert_eq!(
                h.read(record + VALUE, 0xD4).unwrap(),
                value,
                "the setter stores even a request it cannot start"
            );
            visits.borrow().clone()
        };

    assert!(
        apply(&mut h, 1, 0, 0, 0xFF, 0).is_empty(),
        "idle counter change"
    );
    assert!(
        apply(&mut h, 1, 1, 0, 0xFF, 0).is_empty(),
        "flag alone is ignored"
    );
    assert_eq!(
        apply(&mut h, 2, 1, u32::MAX as u64, 0xFF, 0),
        [LOCAL_PLAYER],
        "no local player consumes the new counter without a start"
    );
    assert!(
        apply(&mut h, 2, 1, 0, 0xFF, 0).is_empty(),
        "player becoming valid does not retry the consumed counter"
    );
    assert_eq!(
        apply(&mut h, 3, 1, 0, 0xFF, 0),
        [LOCAL_PLAYER, START],
        "positive control: another counter change reaches start with timestamp zero"
    );
    assert_eq!(apply(&mut h, 4, 1, 0, 3, 0), [LOCAL_PLAYER], "wrong team");
    assert_eq!(
        apply(&mut h, 5, 1, 0, 2, 0),
        [LOCAL_PLAYER, START],
        "matching team"
    );
    assert_eq!(
        apply(&mut h, 6, 1, 0, 0xFF, 16),
        [LOCAL_PLAYER],
        "roster capacity gate"
    );
    assert_eq!(
        apply(&mut h, 7, 1, 0, 0xFF, 15),
        [LOCAL_PLAYER, START],
        "below capacity"
    );
    h.write(record + 0x280, &[1]).unwrap();
    assert!(
        apply(&mut h, 8, 1, 0, 0xFF, 0).is_empty(),
        "already playing"
    );
}

#[test]
fn cinematic_start_reports_success_only_after_the_engine_accepts_it() {
    let Some(path) = std::env::var_os("D1_EBOOT") else {
        eprintln!("SKIP: set D1_EBOOT to the owned build 56578 executable");
        return;
    };
    if !Path::new(&path).is_file() {
        eprintln!("SKIP: D1_EBOOT does not name a present executable");
        return;
    }
    let image = Image::open(path).expect("read supplied executable");
    assert_eq!(
        image.sha256(),
        d1_oracle::weapon_stats::MEASURED_EBOOT_SHA256,
        "cinematic addresses were re-derived only for build 56578"
    );
    let mut h = Harness::new(&image).unwrap();
    let record = h.alloc(0x290).unwrap();
    let globals = h.alloc(0x1B10).unwrap();
    let registry = h.alloc(8).unwrap();
    let table = h.alloc(0x40).unwrap();
    let definition = h.alloc(0x100).unwrap();
    h.write_u64(0x26AFEF0, globals).unwrap();
    h.write_u64(0x1A238A0, registry).unwrap();
    h.write_u64(registry, table).unwrap();
    h.write_u64(table + 0x10, definition).unwrap();
    h.write_u32(table + 0x30, 0x100).unwrap();
    h.write_u32(definition + 0x68, 0x12345678).unwrap(); // synthetic resource handle
    h.write_u32(record + VALUE + 0x9C, 2).unwrap();
    let cast = [0x123456789ABCDEF0u64, 0x23456789ABCDEF01];
    for (i, identity) in cast.into_iter().enumerate() {
        h.write_u64(record + VALUE + 0xA0 + 8 * i as u64, identity)
            .unwrap();
    }
    h.call(0x24DA20, &[])
        .expect("real incident-hash initializer");
    assert_eq!(h.read_u32(0x2651C90).unwrap(), 0x3C03F059);
    assert_eq!(h.read_u32(0x2651C94).unwrap(), 0xAECA3822);
    h.stub(0x403350, Stub::Return(0)); // observe report construction, not its transport
    let reports = Rc::new(RefCell::new(Vec::new()));
    let observed = Rc::clone(&reports);
    h.uc()
        .add_code_hook(0x403350, 0x403350, move |uc, _, _| {
            let argument = uc.reg_read(RegisterX86::RSI).unwrap();
            let mut bytes = [0u8; 4];
            uc.mem_read(argument, &mut bytes).unwrap();
            observed.borrow_mut().push(u32::from_le_bytes(bytes));
        })
        .unwrap();
    for (accepted, expected_hash) in [(0, 0x3C03F059), (1, 0xAECA3822)] {
        h.stub(0x4E3F70, Stub::Return(accepted));
        assert_eq!(h.call(START, &[record]).unwrap() as u8, accepted as u8);
        assert_eq!(h.read(record + 0x280, 1).unwrap(), [accepted as u8]);
        assert_eq!(h.read_u32(globals + 0x1ADC).unwrap(), 2);
        for (i, identity) in cast.into_iter().enumerate() {
            assert_eq!(
                h.read_u64(globals + 0x1AE0 + 8 * i as u64).unwrap(),
                identity,
                "cast starts at value +0xA0, without an extra eight-byte gap"
            );
        }
        assert_eq!(reports.borrow().last(), Some(&expected_hash));
    }
    assert_eq!(
        reports.borrow().len(),
        2,
        "positive and negative engine controls both report"
    );
}
