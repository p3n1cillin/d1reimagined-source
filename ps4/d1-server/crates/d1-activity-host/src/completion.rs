// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The `activity_complete` host incident as the live hosts sent it: one udt 18 carrying a chain of
//! incidents and **one** metadata body (notes/ps4-56578-wire-catalog-2026-11-05.md, "The complete
//! encoding of incident 965").
//!
//! * **The envelope** (`0x8080097d`, 716 bytes): `u32 count @0`, up to 25 incident indices from
//!   `+4`, an empty raw-u32 list at `+0x68`, then the body: `u32 length @0xd4`, up to 500 bytes
//!   from `+0xd8`. The client types the body by the **first** incident's selector.
//! * **The chain**: every one of the 55 live completions sent 965, 3235, 2937, 2009 and 614 (all
//!   selector 27), in that order. Cinematic-only activities leave out 3235.
//! * **The body** (`0x80800595`, 128-byte struct, 725 bits on 56578, no presence bits): the
//!   activity instance and its host, the completing player, the activity's bundle hash, two
//!   activity-type hashes, an always-empty hash, the place hash, a bool, a float and seven counters.
//!
//! The hashes come from the activity's own manifest definition, given in the bridge's activity
//! table, never from a captured body. Two parts are **stand-ins**, said so wherever they are used:
//! the counters (live 51539 bodies carry four, `0, 0, 1, 1` for a completion and `0, 0, -1, -1`
//! for a cinematic-only one; which of 56578's seven they are is open, so they fill the first four)
//! and the session id at `+0x30` (0: which session the live host meant is open).

use crate::row_encoders::EndKind;

/// The body's schema.
pub const BODY_TAG: u32 = 0x8080_0595;
/// The body's struct size.
pub const BODY_SIZE: usize = 0x80;
/// The envelope's struct size (`0x8080097d`).
pub const ENVELOPE_SIZE: usize = 0x2CC;
/// The body's maximum length in the envelope.
pub const BODY_MAX: usize = 500;

/// `activity_complete` and its companions, in the live order (56578 indices, by the name-hash join).
pub const CHAIN_GAMEPLAY: [u32; 5] = [965, 3235, 2937, 2009, 614];
/// The same without 3235, as cinematic-only activities sent it.
pub const CHAIN_CINEMATIC: [u32; 4] = [965, 2937, 2009, 614];

/// The public-event chain the live host sent in one udt 18 (selector 14): `public_event_engaged`
/// 415, then `public_event_complete` 1918, `event_completion_reward3` 3077,
/// `public_event_complete_gold_tier` 732 and 2217. Its body type is open (the incident service's
/// override); the event's state rides its objective record (sensor kind 0x20). For stream G.
pub const PUBLIC_EVENT_ENGAGED: u32 = 415;
pub const PUBLIC_EVENT_COMPLETE_CHAIN: [u32; 4] = [1918, 3077, 732, 2217];

/// The incidents a completion sends.
pub fn chain(end: EndKind) -> &'static [u32] {
    match end {
        EndKind::Gameplay => &CHAIN_GAMEPLAY,
        EndKind::Cinematic => &CHAIN_CINEMATIC,
    }
}

/// The activity's manifest hashes the body carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActivityHashes {
    /// The activity (bundle) hash, `+0x44` (A Guardian Rises `0x6e0dae89`).
    pub activity: u32,
    /// The activity type, `+0x48` (Story `0x648999f4`, Strike `0xf502dd07`, Raid `0x79cbded5`).
    pub kind: u32,
    /// The second type hash, `+0x4c` (Story; `0x4d78882e` for cinematic-only activities).
    pub kind2: u32,
    /// The place or destination, `+0x54` (Mars `0xa776c5b6`).
    pub place: u32,
}

/// One completion body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompletionBody {
    /// `+0`: a per-instance word (consecutive for two fireteam members on the live host).
    pub instance_word: u32,
    /// `+0x10` and `+0x28`: the activity host id.
    pub host: u64,
    /// `+0x18`: the completing player's identity (the first qword of its 40-byte identity).
    pub player: u64,
    pub hashes: ActivityHashes,
    pub end: EndKind,
}

impl CompletionBody {
    /// The 128-byte struct image, laid out as the 56578 descriptor and the live bodies have it.
    pub fn image(&self) -> Vec<u8> {
        let mut b = vec![0u8; BODY_SIZE];
        let put = |b: &mut [u8], at: usize, v: &[u8]| b[at..at + v.len()].copy_from_slice(v);
        put(&mut b, 0, &self.instance_word.to_le_bytes());
        b[8] = 2; // id kind 2: an activity host
        put(&mut b, 0x10, &self.host.to_le_bytes());
        put(&mut b, 0x18, &self.player.to_le_bytes());
        put(&mut b, 0x28, &self.host.to_le_bytes());
        // +0x30 session id: stand-in 0. +0x38 false, +0x39 3, +0x3a 1: every live body.
        b[0x39] = 3;
        b[0x3a] = 1;
        let h = &self.hashes;
        for (i, v) in [h.activity, h.kind, h.kind2, 0x811C_9DC5, h.place].iter().enumerate() {
            put(&mut b, 0x44 + 4 * i, &v.to_le_bytes());
        }
        // +0x58 false, +0x5c 0.0. The counters: stand-ins from the four live 51539 ones.
        let c: [i32; 4] = match self.end {
            EndKind::Gameplay => [0, 0, 1, 1],
            EndKind::Cinematic => [0, 0, -1, -1],
        };
        for (i, v) in c.iter().enumerate() {
            put(&mut b, 0x60 + 4 * i, &v.to_le_bytes());
        }
        b
    }
}

/// The udt 18 struct image: the chain and one encoded body (`bits` long).
pub fn envelope(incidents: &[u32], body: &[u8], bits: u32) -> Result<Vec<u8>, String> {
    let len = (bits as usize).div_ceil(8);
    if incidents.is_empty() || incidents.len() > 25 {
        return Err(format!("{} incidents: an envelope carries 1 to 25", incidents.len()));
    }
    if len > BODY_MAX || len > body.len() {
        return Err(format!("a {bits}-bit body does not fit the envelope's {BODY_MAX} bytes"));
    }
    let mut img = vec![0u8; ENVELOPE_SIZE];
    img[0..4].copy_from_slice(&(incidents.len() as u32).to_le_bytes());
    for (i, t) in incidents.iter().enumerate() {
        img[4 + 4 * i..8 + 4 * i].copy_from_slice(&t.to_le_bytes());
    }
    img[0xD4..0xD8].copy_from_slice(&(len as u32).to_le_bytes());
    img[0xD8..0xD8 + len].copy_from_slice(&body[..len]);
    Ok(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(end: EndKind) -> CompletionBody {
        CompletionBody {
            instance_word: 0x1122_3344,
            host: 0x0102_0304_0506_0708,
            player: 0x0A0B_0C0D_0E0F_1011,
            hashes: ActivityHashes { activity: 0x6E0D_AE89, kind: 0x6489_99F4, kind2: 0x6489_99F4, place: 0xB224_17AE },
            end,
        }
    }

    #[test]
    fn the_body_has_the_documented_layout() {
        let i = body(EndKind::Gameplay).image();
        let u32_at = |at: usize| u32::from_le_bytes(i[at..at + 4].try_into().unwrap());
        assert_eq!((i.len(), u32_at(0), i[8]), (128, 0x1122_3344, 2));
        assert_eq!(&i[0x10..0x18], 0x0102_0304_0506_0708u64.to_le_bytes());
        assert_eq!(&i[0x28..0x30], &i[0x10..0x18], "the host twice");
        assert_eq!(&i[0x18..0x20], 0x0A0B_0C0D_0E0F_1011u64.to_le_bytes());
        assert_eq!((i[0x38], i[0x39], i[0x3a]), (0, 3, 1));
        assert_eq!([u32_at(0x44), u32_at(0x48), u32_at(0x4c), u32_at(0x50), u32_at(0x54)], [0x6E0D_AE89, 0x6489_99F4, 0x6489_99F4, 0x811C_9DC5, 0xB224_17AE]);
        assert_eq!([u32_at(0x60), u32_at(0x64), u32_at(0x68), u32_at(0x6c), u32_at(0x70)], [0, 0, 1, 1, 0]);
        let c = body(EndKind::Cinematic).image();
        assert_eq!(&c[0x68..0x70], [0xFF; 8], "-1, -1 for a cinematic-only activity");
    }

    #[test]
    fn the_envelope_carries_the_chain_and_one_body() {
        let img = envelope(chain(EndKind::Cinematic), &[0xAB; 91], 725).unwrap();
        assert_eq!(img.len(), ENVELOPE_SIZE);
        let u32_at = |at: usize| u32::from_le_bytes(img[at..at + 4].try_into().unwrap());
        assert_eq!([u32_at(0), u32_at(4), u32_at(8), u32_at(12), u32_at(16), u32_at(20)], [4, 965, 2937, 2009, 614, 0]);
        assert_eq!(u32_at(0x68), 0, "the nested list is empty");
        assert_eq!(u32_at(0xD4), 91, "725 bits");
        assert!(img[0xD8..0xD8 + 91].iter().all(|b| *b == 0xAB) && img[0xD8 + 91] == 0);
        assert_eq!(chain(EndKind::Gameplay), [965, 3235, 2937, 2009, 614]);
        assert!(envelope(&[], &[], 0).is_err());
        assert!(envelope(&[965], &[0; 600], 4800).is_err());
    }

    /// The body round-trips through the client's own codec in 725 bits (needs the eboot).
    #[test]
    fn the_body_is_725_bits_through_the_clients_codec() {
        if std::env::var("D1_EBOOT").map_or(true, |p| !std::path::Path::new(&p).is_file()) {
            eprintln!("SKIP: set D1_EBOOT to the owned 56578 executable");
            return;
        }
        let (bits, payload) = crate::codec::oracle_object(BODY_TAG, &body(EndKind::Gameplay).image()).expect("encode");
        assert_eq!(bits, 725);
        assert_eq!(payload.len(), 91);
        let img = envelope(chain(EndKind::Gameplay), &payload, bits).unwrap();
        assert_eq!(u32::from_le_bytes(img[0xD4..0xD8].try_into().unwrap()), 91);
    }
}
