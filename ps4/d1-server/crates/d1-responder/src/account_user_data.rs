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
//! old field [5] unchanged. The result is decoded again and compared with the original before it is
//! served; on any mismatch the original account goes out. Needs the owned eboot (`D1_EBOOT`) for the
//! schemas, as the native codec does.
//!
//! ⚠ Every number below is a **build 56578** value (other builds differ: the 01.00 schema gives
//! `0x80801716` 1,072 bytes, 01.13 and 01.24 give 1,088). [`check_schemas`] compares them with the
//! loaded schemas and refuses (one warning) when the eboot in use disagrees.

use d1_rsat::bits::{BitReader, BitWriter};
use d1_rsat::{Decoder, Encoder, Schemas};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;
use tracing::{debug, warn};

/// The 701 request: `ps4/fixtures/56578/rtti/services.tsv` row 701 (request tag `0x808016dd`,
/// 1,088 bytes, `f0@0x0:0x80801716`).
pub const REQUEST_TAG: u32 = 0x8080_16dd;
/// The user-data struct: `ps4/fixtures/56578/rtti/schemas.tsv` (`0x80801716`, size 1088). Its field 1
/// (+1040, fields.tsv) is `0x808017d6`, whose field 1 (+16) is the 21-byte settings struct
/// `0x808017f0`: image offset +1056, which is where the tests put their recognisable pattern
/// (+1056..+1076). The server never interprets those bytes.
pub const IMAGE_TAG: u32 = 0x8080_1716;
/// `IMAGE_TAG`'s size in 56578 (schemas.tsv). A checked expectation: see [`check_schemas`].
pub const IMAGE_LEN: usize = 1088;
/// The account object: `ps4/fixtures/56578/rtti/fields.tsv` (`0x808018c7`).
pub const ACCOUNT_TAG: u32 = 0x8080_18c7;
/// The account field that is the image: fields.tsv `0x808018c7` field 5 (offset 128, kind 257,
/// child `0x80801716`).
pub const ACCOUNT_FIELD: usize = 5;
/// Field [5]'s offset in the account image (same row).
pub const ACCOUNT_FIELD_OFFSET: usize = 128;

/// Latest data that could not be persisted, per account (`players::current`, 0 = the knobs'
/// player). This lock also orders persistence and reads, so a failed update cannot land after
/// a later successful one.
static MEMORY: Mutex<BTreeMap<u64, Vec<u8>>> = Mutex::new(BTreeMap::new());

fn memory_key() -> u64 {
    crate::players::current().map(|i| i.account).unwrap_or(0)
}

/// Warn once per distinct cause, then only debug-log it (701 comes every 30 s).
fn warn_once(cause: &str, msg: String) {
    static SEEN: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
    if SEEN.lock().map(|mut s| s.insert(cause.to_string())).unwrap_or(true) {
        warn!("{msg}");
    } else {
        debug!("{msg}");
    }
}

#[cfg(test)]
thread_local! {
    /// A test's override of the knob (the process environment is shared between tests).
    static FORCE: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// The knob: unset, empty or `0` is off.
pub fn enabled() -> bool {
    #[cfg(test)]
    if let Some(v) = FORCE.with(std::cell::Cell::get) {
        return v;
    }
    std::env::var("D1_ACCOUNT_USER_DATA").map(|v| !matches!(v.trim(), "" | "0")).unwrap_or(false)
}

/// The schemas this module's constants were read for (build 56578): the image size, the account's
/// field [5] type and offset, and field 0 of the request.
fn check_schemas() -> anyhow::Result<&'static Schemas> {
    let s = d1_rsat::schemas()?;
    let image = s.get(IMAGE_TAG)?;
    anyhow::ensure!(image.struct_size as usize == IMAGE_LEN, "{IMAGE_TAG:#010x} is {} bytes in this build, not the {IMAGE_LEN} of 56578", image.struct_size);
    let f = s.get(ACCOUNT_TAG)?.fields.get(ACCOUNT_FIELD).ok_or_else(|| anyhow::anyhow!("the account has no field [{ACCOUNT_FIELD}]"))?;
    anyhow::ensure!(f.child == IMAGE_TAG && f.offset as usize == ACCOUNT_FIELD_OFFSET, "account field [{ACCOUNT_FIELD}] is not {IMAGE_TAG:#010x} at +{ACCOUNT_FIELD_OFFSET} in this build");
    anyhow::ensure!(f.kind == d1_rsat::schema::KIND_COMPOSITE && f.optional == 1, "account field [{ACCOUNT_FIELD}] is not a composite with one presence bit");
    let r = s.get(REQUEST_TAG)?.fields.first().ok_or_else(|| anyhow::anyhow!("the 701 request has no field 0"))?;
    anyhow::ensure!(r.child == IMAGE_TAG && r.offset == 0, "701 request field 0 is not {IMAGE_TAG:#010x} at +0 in this build");
    Ok(s)
}

/// The image in a 701 body (`u16 message id || args`): field 0 of the request, exactly [`IMAGE_LEN`] bytes.
pub fn extract(body: &[u8]) -> anyhow::Result<Vec<u8>> {
    let s = check_schemas()?;
    let d = Decoder::new(s).decode_at(REQUEST_TAG, body, 16)?;
    let image = d.object.get(..IMAGE_LEN).ok_or_else(|| anyhow::anyhow!("the decoded request is {} bytes", d.object.len()))?;
    Ok(image.to_vec())
}

/// Investment 701 arrived: keep its account user data (knob on). A body that does not decode stores
/// nothing (warned once per cause).
pub fn note_701(body: &[u8]) {
    if !enabled() {
        return;
    }
    match extract(body) {
        Ok(image) => keep(&image),
        Err(e) => warn_once(&format!("701 {e}"), format!("      ⛔ D1_ACCOUNT_USER_DATA: a 701 body ({} B) is not kept: {e:#}", body.len())),
    }
}

fn keep(image: &[u8]) {
    keep_with(image, crate::player_store::set_account_user_data);
}

fn keep_with(image: &[u8], persist: impl FnOnce(&[u8]) -> bool) {
    let mut pending = MEMORY.lock().unwrap_or_else(|e| e.into_inner());
    let key = memory_key();
    if persist(image) {
        pending.remove(&key);
    } else {
        pending.insert(key, image.to_vec());
    }
}

/// Pending data is newer than the store's previous value after a failed save.
pub fn stored() -> Option<Vec<u8>> {
    stored_with(crate::player_store::account_user_data)
}

fn stored_with(read_store: impl FnOnce() -> Option<Vec<u8>>) -> Option<Vec<u8>> {
    let pending = MEMORY.lock().unwrap_or_else(|e| e.into_inner());
    pending.get(&memory_key()).cloned().or_else(read_store)
}

/// The account is being built by a path that cannot carry field [5] (no account payload to splice
/// into): say so once per path when the knob is on and an image is kept. The path's account is
/// served as before.
pub fn note_unserved(path: &str) {
    if enabled() && stored().is_some() {
        warn_once(path, format!("      ⚠ D1_ACCOUNT_USER_DATA: kept user data exists but {path} builds the account without field [{ACCOUNT_FIELD}]; it is not served on that path"));
    }
}

/// The account bytes to put on the wire: `account` with the kept image as field [5] (knob on and an
/// image kept), else `account` unchanged. Any failure, or a splice that does not check out, serves
/// the original (warned once per cause).
pub fn serve(account: Vec<u8>) -> Vec<u8> {
    if !enabled() {
        return account;
    }
    let Some(image) = stored() else { return account };
    match splice_checked(&account, &image) {
        Ok(a) => {
            warn_once("served", format!("      ⭐ D1_ACCOUNT_USER_DATA: the account carries the kept user data ({} B image)", image.len()));
            a
        }
        Err(e) => {
            warn_once(&format!("serve {e}"), format!("      ⛔ D1_ACCOUNT_USER_DATA: the account is served unchanged: {e:#}"));
            account
        }
    }
}

/// [`splice`], then decode both: the result must decode, field [5] must equal the image, and every
/// other byte of the decoded object must match the original's.
pub fn splice_checked(account: &[u8], image: &[u8]) -> anyhow::Result<Vec<u8>> {
    let spliced = splice(account, image)?;
    let before = d1_rsat::decode(ACCOUNT_TAG, account).map_err(|e| anyhow::anyhow!("the account does not decode: {e}"))?.object;
    let after = d1_rsat::decode(ACCOUNT_TAG, &spliced).map_err(|e| anyhow::anyhow!("the spliced account does not decode: {e}"))?.object;
    let field = ACCOUNT_FIELD_OFFSET..ACCOUNT_FIELD_OFFSET + IMAGE_LEN;
    anyhow::ensure!(before.len() == after.len() && after.get(field.clone()) == Some(image), "the spliced account's field [{ACCOUNT_FIELD}] is not the image");
    anyhow::ensure!(before[..field.start] == after[..field.start] && before[field.end..] == after[field.end..], "the splice changed another field");
    Ok(spliced)
}

/// `account` with field [5] set to `image`, every other bit as it was.
pub fn splice(account: &[u8], image: &[u8]) -> anyhow::Result<Vec<u8>> {
    anyhow::ensure!(image.len() == IMAGE_LEN, "the user data is {} bytes, not {IMAGE_LEN}", image.len());
    let s = check_schemas()?;
    let dec = Decoder::new(s);
    let d = s.get(ACCOUNT_TAG)?;
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

    /// Serialises the tests that use the knob and the memory slot, and puts both back on drop.
    struct Knob(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

    impl Knob {
        fn new(on: bool) -> Self {
            static LOCK: Mutex<()> = Mutex::new(());
            let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
            FORCE.with(|f| f.set(Some(on)));
            MEMORY.lock().unwrap().clear();
            Knob(g)
        }
    }

    impl Drop for Knob {
        fn drop(&mut self) {
            FORCE.with(|f| f.set(None));
            if let Ok(mut m) = MEMORY.lock() {
                m.clear();
            }
        }
    }

    /// The tests that need the owned eboot's schemas skip without it.
    fn schemas() -> bool {
        let path = d1_eboot::image::default_eboot();
        if std::env::var_os("D1_EBOOT").is_none() && !std::path::Path::new(&path).exists() {
            eprintln!("SKIP: the selected executable is absent; set D1_EBOOT to an owned image");
            return false;
        }
        check_schemas().expect("the supplied or selected executable must provide compatible schemas");
        true
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

    fn inventory_bits() -> (Vec<u8>, usize) {
        let inv = crate::vendor::account_inventory(&Default::default(), &Default::default());
        let e = d1_rsat::encode(0x8080_0fe6, &inv).unwrap();
        (e.payload, e.bits as usize)
    }

    /// An account like the served ones: the character list, profile SOID and an inventory (field [6]).
    fn account() -> Vec<u8> {
        let (inv, bits) = inventory_bits();
        d1_bap::account_rsat_with_fields(0xD100_0000_0000_0001, &[0xD100_0000_0000_0101], 0xD100_0000_0000_0201, &[(6, &inv, bits)])
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

    fn field5(account: &[u8]) -> Vec<u8> {
        d1_rsat::decode(ACCOUNT_TAG, account).unwrap().object[ACCOUNT_FIELD_OFFSET..ACCOUNT_FIELD_OFFSET + IMAGE_LEN].to_vec()
    }

    /// The decoded account with field [5] zeroed: "every other field".
    fn others(account: &[u8]) -> Vec<u8> {
        let mut o = d1_rsat::decode(ACCOUNT_TAG, account).unwrap().object;
        o[ACCOUNT_FIELD_OFFSET..ACCOUNT_FIELD_OFFSET + IMAGE_LEN].fill(0);
        o
    }

    #[test]
    fn a_failed_save_serves_the_latest_data_until_a_later_save_succeeds() {
        if !schemas() {
            return;
        }
        let _k = Knob::new(true);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("d1-user-data-recovery-{}-{stamp}", std::process::id()));
        let mut store = crate::player_store::Store::open(&dir, 1, 2, 3, &Default::default()).unwrap();
        let a = image(0x10);
        let b = image(0x20);
        let c = image(0x30);
        let account = account();

        keep_with(&a, |im| crate::player_store::set_account_user_data_in(&mut store, im));
        let kept = stored_with(|| store.record.account.account_user_data.clone()).unwrap();
        assert_eq!(field5(&splice_checked(&account, &kept).unwrap()), a);

        // A directory at the atomic-write file forces a real save error on every platform.
        let blocked = crate::player_store::sidecar(store.path().unwrap(), "tmp");
        std::fs::create_dir(&blocked).unwrap();
        keep_with(&b, |im| crate::player_store::set_account_user_data_in(&mut store, im));
        assert_eq!(store.record.account.account_user_data, Some(a.clone()), "the failed save rolled back");
        assert_eq!(stored_with(|| store.record.account.account_user_data.clone()), Some(b.clone()));
        assert_eq!(field5(&serve(account.clone())), b, "pending data overrides the store's older image");

        std::fs::remove_dir(&blocked).unwrap();
        keep_with(&c, |im| crate::player_store::set_account_user_data_in(&mut store, im));
        assert!(!MEMORY.lock().unwrap().contains_key(&memory_key()), "successful persistence clears pending data");
        let kept = stored_with(|| store.record.account.account_user_data.clone()).unwrap();
        assert_eq!(field5(&splice_checked(&account, &kept).unwrap()), c);
        drop(store);
        let reopened = crate::player_store::Store::open(&dir, 1, 2, 3, &Default::default()).unwrap();
        assert_eq!(reopened.record.account.account_user_data, Some(c));
        drop(reopened);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn persistence_and_fallback_reads_share_the_pending_update_lock() {
        let _k = Knob::new(true);
        keep_with(&[1], |_| {
            assert!(MEMORY.try_lock().is_err(), "a concurrent update must wait until persistence finishes");
            false
        });
        assert_eq!(stored_with(|| panic!("pending data must take precedence")), Some(vec![1]));
        keep_with(&[2], |_| {
            assert!(MEMORY.try_lock().is_err());
            true
        });
        assert_eq!(stored_with(|| {
            assert!(MEMORY.try_lock().is_err(), "a concurrent update must wait until the read finishes");
            Some(vec![2])
        }), Some(vec![2]));
    }

    #[test]
    fn the_constants_match_the_loaded_schemas() {
        if !schemas() {
            return;
        }
        check_schemas().unwrap();
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
        let spliced = splice_checked(&a, &im).unwrap();
        assert_ne!(spliced, a);
        assert_eq!(field5(&spliced), im, "field [5] is the image");
        assert!(field5(&a).iter().all(|b| *b == 0), "it was absent");
        assert_eq!(others(&spliced), others(&a), "every other field is unchanged");
        // A second splice replaces the first.
        let again = splice_checked(&spliced, &image(0x10)).unwrap();
        assert_eq!(field5(&again), image(0x10));
        assert_eq!(others(&again), others(&a));
        assert!(splice(&a, &im[..10]).is_err(), "only a whole image is served");
    }

    #[test]
    fn an_account_that_already_has_field_5_is_edited_in_place() {
        if !schemas() {
            return;
        }
        // Field [5] present, with field [6] (the inventory) after it.
        let (inv, bits) = inventory_bits();
        let first = image(0x21);
        let with5 = splice(&d1_bap::account_rsat_with_fields(0xD100_0000_0000_0001, &[0xD100_0000_0000_0101], 0xD100_0000_0000_0201, &[(6, &inv, bits)]), &first).unwrap();
        assert_eq!(field5(&with5), first);
        let next = image(0x33);
        let out = splice_checked(&with5, &next).unwrap();
        assert_eq!(field5(&out), next);
        assert_eq!(others(&out), others(&with5), "the fields after [5] (the inventory) are as they were");
        assert_eq!(others(&out), others(&account()));
    }

    #[test]
    fn a_truncated_account_is_served_unchanged() {
        if !schemas() {
            return;
        }
        let _k = Knob::new(true);
        keep(&image(0x20));
        let a = account();
        for cut in [0, 1, 5, a.len() / 2, a.len() - 1] {
            let t = a[..cut].to_vec();
            assert_eq!(serve(t.clone()), t, "a {cut}-byte account");
        }
    }

    #[test]
    fn the_knob_gates_keeping_and_serving_and_a_second_701_replaces_the_first() {
        if !schemas() {
            return;
        }
        let a = account();
        {
            let _k = Knob::new(false);
            note_701(&body(&image(0x20)));
            assert_eq!(stored(), None, "off: nothing kept");
            assert_eq!(serve(a.clone()), a, "off: the account is as before");
        }
        let _k = Knob::new(true);
        assert_eq!(serve(a.clone()), a, "on, nothing kept yet: unchanged");
        note_701(&body(&image(0x20)));
        assert_eq!(stored(), Some(image(0x20)));
        note_701(&[0x02, 0xbd, 0xff]);
        assert_eq!(stored(), Some(image(0x20)), "a garbled body stores nothing");
        note_701(&body(&image(0x30)));
        assert_eq!(stored(), Some(image(0x30)), "the second 701 replaces the first");
        let served = serve(a.clone());
        assert_eq!(field5(&served), image(0x30));
        assert_eq!(others(&served), others(&a));
    }
}
