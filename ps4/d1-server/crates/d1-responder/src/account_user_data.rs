// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! `D1_ACCOUNT_USER_DATA`: keep what the client saves with investment message 701 and serve it back.
//!
//! The client sends message 701 about every 30 s. Its request (tag `0x808016dd`, 1,088 bytes) holds
//! one struct, `0x80801716`, at field 0: the client's account user data. The same type is field [5]
//! of the account object `0x808018c7` (offset 128, kind 257 = composite with a presence bit), so
//! serving it back is that field. Nothing here reads the image: it is stored and served opaque.
//!
//! The account is served as client-encoded bytes (`D1_ACCOUNT_FILE`, the player store, a push), and
//! a full decode and re-encode would send every other field present-and-zero (the HUD opacity
//! lesson in main.rs), so [`splice`] edits the bitstream: it walks fields 0..4 to find field [5]'s
//! presence bit, writes `1` and the natively encoded `0x80801716`, and copies everything after the
//! old field [5] unchanged. Needs the owned eboot (`D1_EBOOT`) for the schemas, as the native codec does.

use d1_rsat::bits::{BitReader, BitWriter};
use d1_rsat::{Decoder, Encoder};
use std::sync::Mutex;
use tracing::warn;

pub const REQUEST_TAG: u32 = 0x8080_16dd;
pub const IMAGE_TAG: u32 = 0x8080_1716;
pub const IMAGE_LEN: usize = 1088;
pub const ACCOUNT_TAG: u32 = 0x8080_18c7;
/// The account field the image is.
pub const ACCOUNT_FIELD: usize = 5;

/// Kept for the session when the player store is off.
static MEMORY: Mutex<Option<Vec<u8>>> = Mutex::new(None);

/// The knob: unset, empty or `0` is off.
pub fn enabled() -> bool {
    std::env::var("D1_ACCOUNT_USER_DATA").map(|v| !matches!(v.trim(), "" | "0")).unwrap_or(false)
}

/// The image in a 701 body (`u16 message id || args`): field 0 of the request, exactly [`IMAGE_LEN`] bytes.
pub fn extract(body: &[u8]) -> anyhow::Result<Vec<u8>> {
    let s = d1_rsat::schemas()?;
    let d = Decoder::new(s).decode_at(REQUEST_TAG, body, 16)?;
    let image = d.object.get(..IMAGE_LEN).ok_or_else(|| anyhow::anyhow!("the decoded request is {} bytes", d.object.len()))?;
    Ok(image.to_vec())
}

/// Investment 701 arrived: keep its account user data (knob on). A body that does not decode is
/// warned about once per call and stores nothing.
pub fn note_701(body: &[u8]) {
    if !enabled() {
        return;
    }
    match extract(body) {
        Ok(image) => keep(&image),
        Err(e) => warn!("      ⛔ D1_ACCOUNT_USER_DATA: 701 body ({} B) not kept: {e:#}", body.len()),
    }
}

fn keep(image: &[u8]) {
    if !crate::player_store::set_account_user_data(image) {
        if let Ok(mut m) = MEMORY.lock() {
            *m = Some(image.to_vec());
        }
    }
}

/// The kept image: the player store's, else the session's memory when the store is off.
pub fn stored() -> Option<Vec<u8>> {
    match crate::player_store::view(|_| ()) {
        Some(()) => crate::player_store::account_user_data(),
        None => MEMORY.lock().ok().and_then(|m| m.clone()),
    }
}

/// The account bytes to put on the wire: `account` with the kept image as field [5] (knob on and an
/// image kept), else `account` unchanged. A failure warns and serves it unchanged.
pub fn serve(account: Vec<u8>) -> Vec<u8> {
    if !enabled() {
        return account;
    }
    let Some(image) = stored() else { return account };
    match splice(&account, &image) {
        Ok(a) => {
            warn!("      ⭐ D1_ACCOUNT_USER_DATA: the account carries the kept user data ({} B image)", image.len());
            a
        }
        Err(e) => {
            warn!("      ⛔ D1_ACCOUNT_USER_DATA: the account is served unchanged: {e:#}");
            account
        }
    }
}

/// `account` with field [5] set to `image`, every other bit as it was.
pub fn splice(account: &[u8], image: &[u8]) -> anyhow::Result<Vec<u8>> {
    anyhow::ensure!(image.len() == IMAGE_LEN, "the user data is {} bytes, not {IMAGE_LEN}", image.len());
    let s = d1_rsat::schemas()?;
    let dec = Decoder::new(s);
    let d = s.get(ACCOUNT_TAG)?;
    anyhow::ensure!(d.fields.len() > ACCOUNT_FIELD && d.fields[ACCOUNT_FIELD].child == IMAGE_TAG, "account field [{ACCOUNT_FIELD}] is not {IMAGE_TAG:#010x}");
    let mut r = BitReader::new(account);
    // Walk one field (the leaves and sub-objects fields 0..=5 are); the stream is read, not kept.
    let skip = |r: &mut BitReader<'_>, i: usize| -> anyhow::Result<()> {
        let f = &d.fields[i];
        if f.optional != 0 && !r.read_bit().ok_or_else(|| anyhow::anyhow!("the account ends inside field [{i}]"))? {
            return Ok(());
        }
        if f.kind == d1_rsat::schema::KIND_COMPOSITE {
            anyhow::ensure!(f.count_source() == 0, "account field [{i}] has a counted child");
            let mut scratch = vec![0u8; s.get(f.child)?.struct_size as usize];
            dec.decode_from(r, f.child, &mut scratch)?;
        } else if matches!(f.kind, 5 | 6 | 9 | 10) {
            r.read(f.width()).ok_or_else(|| anyhow::anyhow!("the account ends inside field [{i}]"))?;
        } else {
            anyhow::bail!("account field [{i}] has kind {:#04x}", f.kind);
        }
        Ok(())
    };
    for i in 0..ACCOUNT_FIELD {
        skip(&mut r, i)?;
    }
    let at = r.bit_pos();
    skip(&mut r, ACCOUNT_FIELD)?; // the old field [5], present or not
    let after = r.bit_pos();
    let mut w = BitWriter::new();
    w.append(account, at);
    w.write_bit(true);
    Encoder::new(s).encode_into(&mut w, IMAGE_TAG, image)?;
    // The rest, including the old stream's zero padding (harmless: the decode stops after the last field).
    let mut tail = BitReader::at(account, after);
    while let Some(b) = tail.read_bit() {
        w.write_bit(b);
    }
    Ok(w.finish().1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tests that need the owned eboot's schemas skip without it.
    fn schemas() -> bool {
        match d1_rsat::schemas() {
            Ok(_) => true,
            Err(e) => {
                eprintln!("SKIP: no schemas ({e}); set D1_EBOOT to an owned image");
                false
            }
        }
    }

    /// A 1088-byte image with a recognisable pattern across the settings struct (+1056..+1076),
    /// made canonical (the fields are narrower than a byte: what the codec keeps is what it decodes).
    fn image(seed: u8) -> Vec<u8> {
        let mut im = vec![0u8; IMAGE_LEN];
        for (i, b) in im[1056..1076].iter_mut().enumerate() {
            *b = seed.wrapping_add(i as u8);
        }
        let e = d1_rsat::encode(IMAGE_TAG, &im).unwrap();
        let im = d1_rsat::decode(IMAGE_TAG, &e.payload).unwrap().object;
        assert!(im[1056..1076].iter().any(|b| *b != 0), "the pattern survives the codec");
        im
    }

    /// An account like the served ones: the character list, profile SOID and a refund-free inventory.
    fn account() -> Vec<u8> {
        let inv = crate::vendor::account_inventory(&Default::default(), &Default::default());
        let e = d1_rsat::encode(0x8080_0fe6, &inv).unwrap();
        d1_bap::account_rsat_with_fields(0xD100_0000_0000_0001, &[0xD100_0000_0000_0101], 0xD100_0000_0000_0201, &[(6, &e.payload, e.bits as usize)])
    }

    /// A 701 body: `u16 id || the request's args`.
    fn body(image: &[u8]) -> Vec<u8> {
        let mut req = vec![0u8; d1_rsat::schemas().unwrap().get(REQUEST_TAG).unwrap().struct_size as usize];
        req[..image.len()].copy_from_slice(image);
        let e = d1_rsat::encode(REQUEST_TAG, &req).unwrap();
        let mut w = BitWriter::new();
        w.write(701, 16);
        w.append(&e.payload, e.bits);
        w.finish().1
    }

    #[test]
    fn a_701_body_gives_its_image_back() {
        if !schemas() {
            return;
        }
        let im = image(0x40);
        assert_eq!(extract(&body(&im)).unwrap(), im);
    }

    #[test]
    fn a_short_or_garbled_body_gives_nothing() {
        if !schemas() {
            return;
        }
        assert!(extract(&[0x02, 0xbd]).is_err());
        assert!(extract(&[]).is_err());
        let b = body(&image(1));
        assert!(extract(&b[..b.len() / 2]).is_err());
    }

    #[test]
    fn the_spliced_account_carries_the_image_and_nothing_else_changes() {
        if !schemas() {
            return;
        }
        let a = account();
        let im = image(0x70);
        let spliced = splice(&a, &im).unwrap();
        assert_ne!(spliced, a);
        let before = d1_rsat::decode(ACCOUNT_TAG, &a).unwrap().object;
        let after = d1_rsat::decode(ACCOUNT_TAG, &spliced).unwrap().object;
        assert_eq!(&after[128..128 + IMAGE_LEN], &im[..], "field [5] is the image");
        assert!(before[128..128 + IMAGE_LEN].iter().all(|b| *b == 0), "it was absent");
        let (mut b2, mut a2) = (before, after);
        b2[128..128 + IMAGE_LEN].fill(0);
        a2[128..128 + IMAGE_LEN].fill(0);
        assert_eq!(a2, b2, "every other field is unchanged");
        // A second splice replaces the first, and an account that already had field [5] is edited too.
        let again = splice(&spliced, &image(0x10)).unwrap();
        let o = d1_rsat::decode(ACCOUNT_TAG, &again).unwrap().object;
        assert_eq!(&o[128..128 + IMAGE_LEN], &image(0x10)[..]);
        a2.copy_from_slice(&o);
        a2[128..128 + IMAGE_LEN].fill(0);
        assert_eq!(a2, b2);
        assert!(splice(&a, &im[..10]).is_err(), "only a whole image is served");
    }

    /// The knob and the memory path run in one test: both read the process environment.
    #[test]
    fn the_knob_gates_keeping_and_serving_and_a_second_701_replaces_the_first() {
        if !schemas() {
            return;
        }
        let a = account();
        std::env::remove_var("D1_ACCOUNT_USER_DATA");
        note_701(&body(&image(0x20)));
        assert_eq!(stored(), None, "off: nothing kept");
        assert_eq!(serve(a.clone()), a, "off: the account is as before");
        std::env::set_var("D1_ACCOUNT_USER_DATA", "0");
        assert_eq!(serve(a.clone()), a);
        std::env::set_var("D1_ACCOUNT_USER_DATA", "1");
        assert_eq!(serve(a.clone()), a, "on, nothing kept yet: unchanged");
        note_701(&body(&image(0x20)));
        assert_eq!(stored(), Some(image(0x20)));
        note_701(&[0x02, 0xbd, 0xff]);
        assert_eq!(stored(), Some(image(0x20)), "a garbled body stores nothing");
        note_701(&body(&image(0x30)));
        assert_eq!(stored(), Some(image(0x30)), "the second 701 replaces the first");
        let served = serve(a.clone());
        let o = d1_rsat::decode(ACCOUNT_TAG, &served).unwrap().object;
        assert_eq!(&o[128..128 + IMAGE_LEN], &image(0x30)[..]);
        std::env::remove_var("D1_ACCOUNT_USER_DATA");
        *MEMORY.lock().unwrap() = None;
    }
}
