// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! `Decoder::decode_at`: an object body that starts inside a bitstream, as an investment
//! message's arguments start after its 16-bit id. Offline; skips without the owned eboot.
mod common;

use common::inputs;
use d1_rsat::bits::BitWriter;
use d1_rsat::{Decoder, Encoder};

/// `vendor_buy_item` (901)'s request: vendor i8 @0, sale i16 @2, optional u64 @8.
const BUY_REQUEST: u32 = 0x8080_1777;

#[test]
fn an_object_after_a_message_id_decodes_from_its_bit_offset() {
    let Some(s) = inputs() else { return };
    let size = s.get(BUY_REQUEST).unwrap().struct_size as usize;
    let mut image = vec![0u8; size];
    image[0] = 7; // vendor
    image[2..4].copy_from_slice(&123i16.to_le_bytes()); // sale
    let enc = Encoder::new(s).encode(BUY_REQUEST, &image).unwrap();

    // u16 id, the arguments, then a has_payload bit: an investment request's shape.
    let mut w = BitWriter::new();
    w.write(901, 16);
    w.append(&enc.payload, enc.bits);
    w.write_bit(false);
    let (_, wire) = w.finish();

    let d = Decoder::new(s).decode_at(BUY_REQUEST, &wire, 16).unwrap();
    assert_eq!(d.bits, 16 + enc.bits, "the end position is absolute");
    assert_eq!(d.object, image);
    // From bit 0 the same bytes decode as something else (the id's bits are read as fields).
    let wrong = Decoder::new(s).decode_at(BUY_REQUEST, &wire, 0);
    assert!(wrong.map_or(true, |d| d.object != image));
}

#[test]
fn a_partial_walk_keeps_what_it_read_and_a_fill_shows_what_it_wrote() {
    let Some(s) = inputs() else { return };
    let size = s.get(BUY_REQUEST).unwrap().struct_size as usize;
    let mut image = vec![0u8; size];
    image[0] = 7;
    image[2..4].copy_from_slice(&123i16.to_le_bytes());
    image[8..16].copy_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    let enc = Encoder::new(s).encode(BUY_REQUEST, &image).unwrap();
    let dec = Decoder::new(s);

    // Cut the body inside its last field: the walk fails, but the vendor and sale were read.
    let cut = &enc.payload[..4];
    assert!(dec.decode_at(BUY_REQUEST, cut, 0).is_err());
    let (d, err) = dec.decode_at_partial(BUY_REQUEST, cut, 0, 0).unwrap();
    assert!(err.is_some(), "the cut body is reported as failed");
    assert_eq!((d.object[0], &d.object[2..4]), (7, &123i16.to_le_bytes()[..]));
    assert!(d.bits <= 32, "the reader stops inside the cut body");

    // A whole body decoded over 0x00 and over 0xFF agrees exactly where the walk wrote.
    let (a, none) = dec.decode_at_partial(BUY_REQUEST, &enc.payload, 0, 0x00).unwrap();
    let b = dec.decode_at_over(BUY_REQUEST, &enc.payload, 0, 0xFF).unwrap();
    assert!(none.is_none());
    assert_eq!(a.object, image);
    assert_eq!((a.object[0], &a.object[2..4]), (b.object[0], &b.object[2..4]));
    assert_eq!(a.object[8..16], b.object[8..16]);
    assert_ne!(a.object[1], b.object[1], "padding byte 1 is never written");
}
