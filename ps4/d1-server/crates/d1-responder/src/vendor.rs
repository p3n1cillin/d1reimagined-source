// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Vendor purchases (investment 901) applied by the server.
//!
//! A purchase names only `{u8 vendor, i16 sale, u64}` (`0x80801777`). `D1_VENDOR_SALES`
//! (`d1-oracle vendor-sales write`) says what that sale is: the item, its bucket and instanced flag,
//! and each cost with its bucket. This module keeps the account's inventory slots and the profile,
//! deducts the cost from the cost item's own bucket slot (the rule the client's price check
//! `0x26D6E0` uses), puts the item in a free slot of its bucket (a new instance when instanced), and
//! writes the new account and profile to `D1_ACCOUNT_PUSH_FILE` / `D1_PROFILE_PUSH_FILE`, where the
//! existing echo-time pushes deliver them.
//!
//! Variables: `D1_VENDOR_SALES` (enables it), `D1_VENDOR_PROFILE_OBJ` (the served profile as a
//! decoded C object, `d1-oracle object --tag 0x80801743 --decode-file … --out-obj`),
//! `D1_VENDOR_ITEM_TEMPLATE` (a decoded `0x80801732` item element), `D1_VENDOR_ACCOUNT_SLOTS`
//! (`slot:content:quantity,…`, the slots `D1_ACCOUNT_FILE` serves), `D1_ORACLE`.
//!
//! Encoding goes through `d1-oracle` (the client's own codec), as `stage_loadout` does.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// The client's bucket table (`investment_globals->vfunc[0x98]`, read live): ordinal ->
/// (first slot, slot count, scope 0 = profile / 1 = account).
const BUCKETS: &[(u8, u16, u16, u8)] = &[
    (0x05, 0, 20, 0), (0x01, 20, 20, 0), (0x06, 40, 3, 0), (0x07, 43, 10, 0), (0x08, 53, 10, 0),
    (0x09, 63, 10, 0), (0x0a, 73, 10, 0), (0x0b, 83, 10, 0), (0x02, 93, 10, 0), (0x03, 103, 10, 0),
    (0x04, 113, 10, 0), (0x0c, 123, 10, 0), (0x0f, 133, 10, 0), (0x0d, 143, 10, 0), (0x0e, 153, 10, 0),
    (0x11, 163, 5, 0), (0x12, 168, 1, 0), (0x10, 169, 20, 0), (0x14, 189, 20, 0), (0x15, 0, 20, 1),
    (0x16, 20, 108, 1), (0x17, 128, 72, 1), (0x18, 200, 108, 1), (0x13, 209, 1, 0), (0x19, 210, 10, 0),
    (0x1b, 220, 10, 0), (0x1c, 230, 10, 0), (0x1e, 240, 10, 0), (0x1d, 250, 12, 0), (0x1a, 262, 16, 0),
    (0x1f, 308, 1, 1), (0x20, 278, 1, 0), (0x21, 279, 1, 0), (0x22, 309, 1, 1), (0x23, 310, 1, 1),
    (0x24, 280, 10, 0), (0x25, 290, 32, 0), (0x26, 311, 1, 1), (0x27, 312, 6, 1),
];

const PROFILE_SLOTS: usize = 0x38;
const ACCOUNT_ENTRIES: usize = 326;

/// One sale: item content index, its bucket and instanced flag, and `(content, quantity, bucket)` costs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sale {
    pub item: u16,
    pub bucket: u8,
    pub instanced: bool,
    pub costs: Vec<(u16, u32, u8)>,
}

/// Parse `vendor-sales.tsv` (the bucket-column format).
pub fn parse_sales(text: &str) -> HashMap<(u8, u16), Sale> {
    let mut out = HashMap::new();
    for line in text.lines() {
        if line.starts_with('#') || line.starts_with("vendor") {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 7 {
            continue;
        }
        let hex = |s: &str| u8::from_str_radix(s.trim_start_matches("0x"), 16).ok();
        let (Ok(v), Ok(s), Ok(item), Some(bucket)) = (f[0].parse::<u8>(), f[1].parse::<u16>(), f[3].parse(), hex(f[4])) else {
            continue;
        };
        let costs = if f[6] == "-" {
            Vec::new()
        } else {
            f[6].split(',').filter_map(|c| {
                let p: Vec<&str> = c.split(':').collect();
                Some((p.first()?.parse().ok()?, p.get(1)?.parse().ok()?, hex(p.get(2)?)?))
            }).collect()
        };
        out.insert((v, s), Sale { item, bucket, instanced: f[5] == "1", costs });
    }
    out
}

/// A profile slot's `(content index, stack quantity at +0x10)`.
fn profile_entry(profile: &[u8], slot: u16) -> (u16, u32) {
    let e = PROFILE_SLOTS + 32 * slot as usize;
    (u16::from_le_bytes([profile[e], profile[e + 1]]), u32::from_le_bytes(profile[e + 0x10..e + 0x14].try_into().unwrap()))
}

fn bucket(ordinal: u8) -> Option<(u16, u16, u8)> {
    BUCKETS.iter().find(|b| b.0 == ordinal).map(|b| (b.1, b.2, b.3))
}

struct State {
    sales: HashMap<(u8, u16), Sale>,
    account: BTreeMap<u16, (u16, u32)>,
    profile: Vec<u8>,
    template: Vec<u8>,
    next_soid: u64,
    oracle: String,
}

fn state() -> &'static Mutex<Option<State>> {
    static S: OnceLock<Mutex<Option<State>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(load().map_err(|e| tracing::warn!("      vendor purchases off: {e}")).ok()))
}

fn load() -> anyhow::Result<State> {
    let sales_path = std::env::var("D1_VENDOR_SALES")?;
    let sales = parse_sales(&std::fs::read_to_string(&sales_path)?);
    let profile = std::fs::read(std::env::var("D1_VENDOR_PROFILE_OBJ")?)?;
    let template = std::fs::read(std::env::var("D1_VENDOR_ITEM_TEMPLATE")?)?;
    let mut account = BTreeMap::new();
    for spec in std::env::var("D1_VENDOR_ACCOUNT_SLOTS").unwrap_or_default().split(',').filter(|s| !s.is_empty()) {
        let p: Vec<u32> = spec.split(':').filter_map(|x| x.parse().ok()).collect();
        if p.len() == 3 {
            account.insert(p[0] as u16, (p[1] as u16, p[2]));
        }
    }
    let oracle = std::env::var("D1_ORACLE").unwrap_or_else(|_| "d1-server/target/release/d1-oracle".into());
    tracing::warn!("      vendor purchases on: {} sales from {sales_path}, account {account:?}", sales.len());
    Ok(State { sales, account, profile, template, next_soid: 0xD100_0000_0000_0601, oracle })
}

/// Run `d1-oracle object --tag TAG --obj-file …`; returns the payload and its bit count.
fn encode(oracle: &str, tag: &str, obj: &[u8], name: &str) -> anyhow::Result<(Vec<u8>, usize)> {
    let dir = std::env::temp_dir().join("d1-vendor");
    std::fs::create_dir_all(&dir)?;
    let src = dir.join(format!("{name}.obj"));
    let out = dir.join(format!("{name}.bin"));
    std::fs::write(&src, obj)?;
    let _ = std::fs::remove_file(&out);
    let o = std::process::Command::new(oracle)
        .args(["object", "--tag", tag, "--obj-file"]).arg(&src).arg("--out").arg(&out).output()?;
    let text = String::from_utf8_lossy(&o.stdout);
    if !text.contains("IDENTICAL") {
        anyhow::bail!("{name}: the client's codec refused it: {}", text.lines().last().unwrap_or(""));
    }
    let bits = text.split("/ ").nth(1).and_then(|t| t.split(' ').next()).and_then(|n| n.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("{name}: no bit count in the oracle's output"))?;
    Ok((std::fs::read(&out)?, bits))
}

/// Decode the 901 request with the client's codec: (vendor, sale).
fn decode_request(oracle: &str, req: &[u8]) -> anyhow::Result<(u8, u16)> {
    let hex: String = req.iter().map(|b| format!("{b:02x}")).collect();
    let o = std::process::Command::new(oracle).args(["object", "--tag", "0x80801777", "--decode", &hex]).output()?;
    let text = String::from_utf8_lossy(&o.stdout);
    let w0 = text.lines().find_map(|l| {
        let l = l.trim();
        l.strip_prefix("+0x0000").map(|v| u64::from_str_radix(v.trim().trim_start_matches("0x"), 16))
    });
    let w0 = match w0 {
        Some(Ok(w)) => w,
        _ => 0, // all-zero object: vendor 0, sale 0
    };
    Ok(((w0 & 0xFF) as u8, ((w0 >> 16) & 0xFFFF) as u16))
}

/// A 903 `vendor_sell_item` request (`0x80801734`, 32 bytes): an item reference `{bool @0, u64 @8,
/// i16 @0x10}`, then `i32 @0x18` and `u8 @0x1C`. The offsets are the client codec's (a round trip of
/// every field is identical, 121 bits). The client's builder (`0x27F7D0`, called from the vendor
/// screen's action `0xA29D30` through `0x552FC0`) sets: the bool = the item has an instance, the u64 =
/// that instance, the i16 = the content index (-1 when instanced), the u8 = the vendor (the same
/// screen field 901's vendor comes from), and the i32 = the screen's `+0x6C`, likely the quantity. No
/// run has sent one yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SellRequest {
    pub instanced: bool,
    pub instance: u64,
    pub content: i16,
    pub quantity: i32,
    pub vendor: u8,
}

/// Read a decoded 903 object.
pub fn sell_request(obj: &[u8]) -> Option<SellRequest> {
    if obj.len() < 0x1D {
        return None;
    }
    Some(SellRequest {
        instanced: obj[0] != 0,
        instance: u64::from_le_bytes(obj[8..16].try_into().ok()?),
        content: i16::from_le_bytes([obj[0x10], obj[0x11]]),
        quantity: i32::from_le_bytes(obj[0x18..0x1C].try_into().ok()?),
        vendor: obj[0x1C],
    })
}

/// Decode a 903 request body with the client's codec and log its fields (no state change yet).
pub fn on_sell(req: Vec<u8>) {
    let oracle = std::env::var("D1_ORACLE").unwrap_or_else(|_| "d1-server/target/release/d1-oracle".into());
    std::thread::spawn(move || {
        let decoded = (|| -> anyhow::Result<SellRequest> {
            let dir = std::env::temp_dir().join("d1-vendor");
            std::fs::create_dir_all(&dir)?;
            let src = dir.join("sell-request.bin");
            let obj = dir.join("sell-request.obj");
            std::fs::write(&src, &req)?;
            let _ = std::fs::remove_file(&obj);
            std::process::Command::new(&oracle)
                .args(["object", "--tag", "0x80801734", "--decode-file"]).arg(&src)
                .arg("--out-obj").arg(&obj).output()?;
            sell_request(&std::fs::read(&obj)?).ok_or_else(|| anyhow::anyhow!("short object"))
        })();
        match decoded {
            Ok(s) => tracing::warn!("      ⭐ VENDOR SELL (903) decoded: {s:?} (not applied: the fields are unconfirmed)"),
            Err(e) => tracing::warn!("      ⛔ vendor sell (903) not decoded: {e}"),
        }
    });
}

/// The investment codec's bitstream, read most significant bit first. Integers the schema marks
/// signed (and the "u8" of 903) are written biased, with their top bit flipped. Worked out from 903's
/// round trip: `{1, 0x1122334455667788, 0x348}, 0x55667, 6` encodes to
/// `889119a22ab33bc441a44002ab33c300`, where bits 65, 81 and 113 (the top bits of the i16, i32 and u8)
/// are set.
struct Bits<'a> {
    data: &'a [u8],
    at: usize,
}

impl Bits<'_> {
    fn take(&mut self, width: usize) -> Option<u64> {
        let mut v = 0u64;
        for _ in 0..width {
            let byte = *self.data.get(self.at / 8)?;
            v = (v << 1) | u64::from((byte >> (7 - self.at % 8)) & 1);
            self.at += 1;
        }
        Some(v)
    }

    fn biased(&mut self, width: usize) -> Option<i64> {
        let v = self.take(width)? ^ (1 << (width - 1));
        Some(((v << (64 - width)) as i64) >> (64 - width))
    }
}

/// A 905 `vendor_refund_item` request. The client's builder (`0x27F6F0`, through `0x552F60` from
/// the vendor screen's action 2) writes the same item reference as 903 (`{bool @0, u64 @8, i16
/// @0x10}`) and a u64 at `+0x18` from `0x265100(owner...)`, the same owner field 901 sends as its u64
/// (`obj+0x3E88` when `+0x3E80` is set, else `obj+8`; `0xB11` in run sense69's purchase).
/// Decoded from the bitstream directly, since its schema tag isn't known yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefundRequest {
    pub instanced: bool,
    pub instance: u64,
    pub content: i16,
    pub owner: u64,
}

pub fn refund_request(req: &[u8]) -> Option<RefundRequest> {
    let mut b = Bits { data: req, at: 0 };
    Some(RefundRequest {
        instanced: b.take(1)? != 0,
        instance: b.take(64)?,
        content: b.biased(16)? as i16,
        owner: b.take(64)?,
    })
}

/// Log a 905 request's fields (no state change yet).
pub fn on_refund(req: &[u8]) {
    match refund_request(req) {
        Some(r) => tracing::warn!("      ⭐ VENDOR REFUND (905) decoded: {r:?} (not applied yet)"),
        None => tracing::warn!("      ⛔ vendor refund (905): {} bytes is too short to decode", req.len()),
    }
}

/// Apply one purchase to the state. Returns (description, new account slots changed, item sidecar).
fn apply(st: &mut State, vendor: u8, sale_index: u16) -> anyhow::Result<(String, Option<(u64, Vec<u8>)>)> {
    let sale = st.sales.get(&(vendor, sale_index)).cloned()
        .ok_or_else(|| anyhow::anyhow!("vendor {vendor} has no sale {sale_index} in D1_VENDOR_SALES"))?;
    // Costs: from the slots of the cost item's own bucket, where the client's price check
    // (0x26D6E0) sums them: the stack quantity at +0x10 of every slot holding that content index.
    let mut account = st.account.clone();
    let mut profile = st.profile.clone();
    for (ci, q, b) in &sale.costs {
        let (first, n, scope) = bucket(*b).ok_or_else(|| anyhow::anyhow!("unknown bucket {b:#x}"))?;
        let slots: Vec<u16> = (first..first + n).filter(|s| if scope == 1 {
            account.get(s).map(|e| e.0) == Some(*ci)
        } else {
            profile_entry(&profile, *s).0 == *ci
        }).collect();
        let quantity = |s: u16, a: &BTreeMap<u16, (u16, u32)>, p: &[u8]| if scope == 1 { a[&s].1 } else { profile_entry(p, s).1 };
        let have: u64 = slots.iter().map(|s| quantity(*s, &account, &profile) as u64).sum();
        anyhow::ensure!(have >= *q as u64, "{ci} (bucket {b:#x}): {have} < price {q}");
        let mut owed = *q;
        for s in slots {
            if owed == 0 {
                break;
            }
            let take = owed.min(quantity(s, &account, &profile));
            owed -= take;
            if scope == 1 {
                let e = account.get_mut(&s).unwrap();
                e.1 -= take;
            } else {
                let left = profile_entry(&profile, s).1 - take;
                let e = PROFILE_SLOTS + 32 * s as usize;
                if left == 0 {
                    profile[e..e + 2].copy_from_slice(&[0xFF, 0xFF]);
                    profile[e + 8..e + 16].fill(0);
                }
                profile[e + 0x10..e + 0x14].copy_from_slice(&left.to_le_bytes());
            }
        }
    }
    // The item: a free slot of its own bucket.
    let (first, n, scope) = bucket(sale.bucket).ok_or_else(|| anyhow::anyhow!("unknown bucket {:#x}", sale.bucket))?;
    let mut sidecar = None;
    let mut next_soid = st.next_soid;
    if scope == 0 {
        let slot = (first..first + n).find(|s| profile_entry(&profile, *s).0 == 0xFFFF)
            .ok_or_else(|| anyhow::anyhow!("bucket {:#x} is full", sale.bucket))?;
        let e = PROFILE_SLOTS + 32 * slot as usize;
        let mut soid = 0u64;
        if sale.instanced {
            soid = next_soid;
            next_soid += 1;
            let mut item = st.template.clone();
            item[0..8].copy_from_slice(&soid.to_le_bytes());
            item[0x24..0x26].copy_from_slice(&sale.item.to_le_bytes());
            sidecar = Some((soid, encode(&st.oracle, "0x80801732", &item, "item")?.0));
        }
        profile[e..e + 2].copy_from_slice(&sale.item.to_le_bytes());
        profile[e + 8..e + 16].copy_from_slice(&soid.to_le_bytes());
        profile[e + 0x10..e + 0x14].copy_from_slice(&1u32.to_le_bytes());
    } else {
        let slot = (first..first + n).find(|s| !account.contains_key(s) || account[s].0 == sale.item)
            .ok_or_else(|| anyhow::anyhow!("account bucket {:#x} is full", sale.bucket))?;
        let have = account.get(&slot).map(|e| e.1).unwrap_or(0);
        account.insert(slot, (sale.item, have + 1));
    }
    st.account = account;
    st.profile = profile;
    st.next_soid = next_soid;
    Ok((format!("vendor {vendor} sale {sale_index}: item {} -> bucket {:#x}; costs {:?}", sale.item, sale.bucket, sale.costs), sidecar))
}

/// Encode the account (only the intended fields, plus the inventory) and write both push files.
fn publish(st: &State, sidecar: Option<(u64, Vec<u8>)>) -> anyhow::Result<()> {
    let mut inv = vec![0u8; 10_440];
    for s in 0..ACCOUNT_ENTRIES {
        inv[8 + 32 * s..8 + 32 * s + 2].copy_from_slice(&[0xFF, 0xFF]);
    }
    for (slot, (ci, q)) in &st.account {
        let e = 8 + 32 * *slot as usize;
        inv[e..e + 2].copy_from_slice(&ci.to_le_bytes());
        inv[e + 0x10..e + 0x14].copy_from_slice(&q.to_le_bytes());
    }
    let (bits, n) = encode(&st.oracle, "0x80800fe6", &inv, "account-inventory")?;
    let acct = crate::env_soid("D1_ACCOUNT_SOID", 0xD100_0000_0000_0001);
    let chr = crate::env_soid("D1_CHARACTER_SOID", 0xD100_0000_0000_0101);
    let prof = crate::env_soid("D1_PROFILE_SOID", 0xD100_0000_0000_0201);
    let account = d1_bap::account_rsat_with_inventory(acct, &[chr], prof, &bits, n);
    let (profile, _) = encode(&st.oracle, "0x80801743", &st.profile, "profile")?;
    if let Some(p) = std::env::var_os("D1_ACCOUNT_PUSH_FILE").map(PathBuf::from) {
        std::fs::write(&p, &account)?;
    }
    if let Some(p) = std::env::var_os("D1_PROFILE_PUSH_FILE").map(PathBuf::from) {
        if let Some((soid, element)) = sidecar {
            std::fs::write(p.with_extension("item"), element)?;
            std::fs::write(p.with_extension("soid"), format!("{soid:#x}"))?;
        }
        std::fs::write(&p, &profile)?;
    }
    Ok(())
}

/// Keep the purchase state's profile equal to the profile the client last received: decode each
/// pushed profile payload (`D1_PROFILE_PUSH_FILE`, e.g. the post-mission completion) with the client's
/// codec and adopt it, so `D1_VENDOR_PROFILE_OBJ` only has to match the profile served at boot.
pub fn note_profile_pushed(payload: Vec<u8>) {
    if state().lock().map(|s| s.is_none()).unwrap_or(true) {
        return;
    }
    std::thread::spawn(move || {
        let Ok(mut guard) = state().lock() else { return };
        let Some(st) = guard.as_mut() else { return };
        let decoded = (|| -> anyhow::Result<Vec<u8>> {
            let dir = std::env::temp_dir().join("d1-vendor");
            std::fs::create_dir_all(&dir)?;
            let src = dir.join("pushed-profile.bin");
            let obj = dir.join("pushed-profile.obj");
            std::fs::write(&src, &payload)?;
            let _ = std::fs::remove_file(&obj);
            std::process::Command::new(&st.oracle)
                .args(["object", "--tag", "0x80801743", "--decode-file"]).arg(&src)
                .arg("--out-obj").arg(&obj).output()?;
            let o = std::fs::read(&obj)?;
            anyhow::ensure!(o.len() == st.profile.len(), "decoded {} bytes, expected {}", o.len(), st.profile.len());
            Ok(o)
        })();
        match decoded {
            Ok(o) => {
                st.profile = o;
                tracing::warn!("      vendor purchases: profile state follows the pushed profile");
            }
            Err(e) => tracing::warn!("      ⛔ vendor purchases: could not adopt the pushed profile: {e}"),
        }
    });
}

/// Handle a 901 request body off the conversation's task: the oracle calls take seconds.
pub fn on_buy(req: Vec<u8>) {
    if state().lock().map(|s| s.is_none()).unwrap_or(true) {
        return;
    }
    std::thread::spawn(move || {
        let mut guard = match state().lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let Some(st) = guard.as_mut() else { return };
        let result = (|| -> anyhow::Result<String> {
            let (vendor, sale) = decode_request(&st.oracle, &req)?;
            let (what, sidecar) = apply(st, vendor, sale)?;
            publish(st, sidecar)?;
            Ok(what)
        })();
        match result {
            Ok(what) => tracing::warn!("      ⭐⭐ VENDOR PURCHASE APPLIED: {what}; pushes written"),
            Err(e) => tracing::warn!("      ⛔ vendor purchase NOT applied: {e}"),
        }
    });
}

/// The profile's equipped array: 20 instance ids (u64) after the 340 inventory slots.
const EQUIPPED: usize = 0x2B78;

/// The equipped-array index of a bucket, read from the geared fixture (each equipped instance's
/// inventory slot gives its bucket): subclass, the five armour pieces, the three weapons, the vehicle
/// and the ghost. Other buckets are not known yet.
fn equipped_index(ordinal: u8) -> Option<usize> {
    Some(match ordinal {
        0x06 => 1,
        0x07..=0x0B => 2 + (ordinal - 0x07) as usize,
        0x02..=0x04 => 7 + (ordinal - 0x02) as usize,
        0x0D => 11,
        0x0E => 12,
        _ => return None,
    })
}

/// Apply a 403 `inventory_equip_item`: point the equipped entry of the item's bucket at it.
fn apply_equip(profile: &mut [u8], instance: u64) -> anyhow::Result<String> {
    let slot = (0..340u16).find(|s| {
        let e = PROFILE_SLOTS + 32 * *s as usize;
        u64::from_le_bytes(profile[e + 8..e + 16].try_into().unwrap()) == instance
    }).ok_or_else(|| anyhow::anyhow!("instance {instance:#x} is in no profile slot"))?;
    let ordinal = BUCKETS.iter().find(|b| b.3 == 0 && b.1 <= slot && slot < b.1 + b.2).map(|b| b.0)
        .ok_or_else(|| anyhow::anyhow!("slot {slot} is in no character bucket"))?;
    let index = equipped_index(ordinal).ok_or_else(|| anyhow::anyhow!("bucket {ordinal:#x} has no known equipped index"))?;
    let at = EQUIPPED + 8 * index;
    let old = u64::from_le_bytes(profile[at..at + 8].try_into().unwrap());
    profile[at..at + 8].copy_from_slice(&instance.to_le_bytes());
    Ok(format!("{instance:#x} (slot {slot}, bucket {ordinal:#x}) equipped at index {index}, replacing {old:#x}"))
}

/// Decode a 403 request (`0x80801942`, one u64) with the client's codec.
fn decode_instance(oracle: &str, req: &[u8]) -> anyhow::Result<u64> {
    let hex: String = req.iter().map(|b| format!("{b:02x}")).collect();
    let o = std::process::Command::new(oracle).args(["object", "--tag", "0x80801942", "--decode", &hex]).output()?;
    let text = String::from_utf8_lossy(&o.stdout);
    text.lines().find_map(|l| l.trim().strip_prefix("+0x0000").map(|v| u64::from_str_radix(v.trim().trim_start_matches("0x"), 16)))
        .ok_or_else(|| anyhow::anyhow!("no +0x0000 in the decoded 403 request"))?
        .map_err(Into::into)
}

/// Handle a 403 request body: equip the item in the purchase state's profile and push it.
pub fn on_equip(req: Vec<u8>) {
    if state().lock().map(|s| s.is_none()).unwrap_or(true) {
        return;
    }
    std::thread::spawn(move || {
        let Ok(mut guard) = state().lock() else { return };
        let Some(st) = guard.as_mut() else { return };
        let result = (|| -> anyhow::Result<String> {
            let instance = decode_instance(&st.oracle, &req)?;
            let mut profile = st.profile.clone();
            let what = apply_equip(&mut profile, instance)?;
            st.profile = profile;
            publish(st, None)?;
            Ok(what)
        })();
        match result {
            Ok(what) => tracing::warn!("      ⭐⭐ EQUIP APPLIED: {what}; profile push written"),
            Err(e) => tracing::warn!("      ⛔ equip NOT applied: {e}"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const TSV: &str = "# x\n# y\n# z\nvendor\tsale\tvendor_hash\tcontent_index\tbucket\tinstanced\tcosts\n\
                       6\t23\t0x080760b4\t840\t0x19\t1\t869:570:0x1f\n";

    fn st(glimmer: u32) -> State {
        let mut profile = vec![0u8; 29_560];
        for s in 0..340 {
            profile[PROFILE_SLOTS + 32 * s..PROFILE_SLOTS + 32 * s + 2].copy_from_slice(&[0xFF, 0xFF]);
        }
        let mut account = BTreeMap::new();
        account.insert(308, (869, glimmer));
        State { sales: parse_sales(TSV), account, profile, template: vec![0; 184], next_soid: 0x601, oracle: String::new() }
    }

    #[test]
    fn the_table_parses() {
        let s = parse_sales(TSV);
        assert_eq!(s[&(6, 23)], Sale { item: 840, bucket: 0x19, instanced: true, costs: vec![(869, 570, 0x1F)] });
    }

    #[test]
    fn a_purchase_without_enough_glimmer_changes_nothing() {
        let mut s = st(500);
        assert!(apply(&mut s, 6, 23).is_err());
        assert_eq!(s.account[&308], (869, 500));
    }

    /// With the owned inputs: apply the Outfitter's sale 23 through the real codec and write the
    /// pushes. `D1_TEST_PROFILE_OBJ`, `D1_TEST_ITEM_TEMPLATE`, `D1_ORACLE` (+ `D1_BUILDS`/`D1_BUILD`),
    /// `D1_ACCOUNT_PUSH_FILE`, `D1_PROFILE_PUSH_FILE`. Run with `--ignored`.
    #[test]
    #[ignore]
    fn a_purchase_through_the_real_codec() {
        let mut s = st(25_000);
        s.profile = std::fs::read(std::env::var("D1_TEST_PROFILE_OBJ").unwrap()).unwrap();
        s.template = std::fs::read(std::env::var("D1_TEST_ITEM_TEMPLATE").unwrap()).unwrap();
        s.oracle = std::env::var("D1_ORACLE").unwrap();
        let (what, sidecar) = apply(&mut s, 6, 23).unwrap();
        assert_eq!(s.account[&308], (869, 24_430), "{what}");
        assert_eq!(&s.profile[PROFILE_SLOTS + 32 * 210..PROFILE_SLOTS + 32 * 210 + 2], &840u16.to_le_bytes());
        assert!(sidecar.is_some());
        publish(&s, sidecar).unwrap();
    }

    /// A price paid in a character-held material comes out of the profile's slots of its bucket,
    /// across stacks, and an emptied stack is freed. (No instance, so no codec call.)
    #[test]
    fn a_profile_held_cost_is_taken_from_its_stacks() {
        let mut s = st(0);
        s.sales.insert((5, 0), Sale { item: 900, bucket: 0x05, instanced: false, costs: vec![(872, 30, 0x05)] });
        let put = |p: &mut Vec<u8>, slot: usize, ci: u16, q: u32| {
            let e = PROFILE_SLOTS + 32 * slot;
            p[e..e + 2].copy_from_slice(&ci.to_le_bytes());
            p[e + 0x10..e + 0x14].copy_from_slice(&q.to_le_bytes());
        };
        put(&mut s.profile, 0, 872, 20);
        put(&mut s.profile, 1, 872, 15);
        apply(&mut s, 5, 0).unwrap();
        // The first stack is used up and freed, so the item takes that first free Materials slot.
        assert_eq!(profile_entry(&s.profile, 0), (900, 1));
        assert_eq!(profile_entry(&s.profile, 1), (872, 5));
        let mut poor = st(0);
        poor.sales = s.sales.clone();
        put(&mut poor.profile, 0, 872, 29);
        assert!(apply(&mut poor, 5, 0).is_err());
        assert_eq!(profile_entry(&poor.profile, 0), (872, 29), "a refused purchase changes nothing");
    }

    /// The layout the client's codec round-trips (d1-oracle object --tag 0x80801734).
    #[test]
    fn a_sell_request_reads_at_the_codec_offsets() {
        let obj = hex("0100000000000000887766554433221148030000000000006756050006000000");
        assert_eq!(sell_request(&obj), Some(SellRequest {
            instanced: true, instance: 0x1122_3344_5566_7788, content: 0x348, quantity: 0x55667, vendor: 6,
        }));
        assert_eq!(sell_request(&obj[..0x1C]), None);
    }

    /// The bit reader reproduces 903's round trip through the client codec.
    #[test]
    fn the_bitstream_reads_903s_known_encoding() {
        let wire = hex("889119a22ab33bc441a44002ab33c300");
        let mut b = Bits { data: &wire, at: 0 };
        assert_eq!(b.take(1), Some(1));
        assert_eq!(b.take(64), Some(0x1122_3344_5566_7788));
        assert_eq!(b.biased(16), Some(0x348));
        assert_eq!(b.biased(32), Some(0x55667));
        assert_eq!(b.biased(8), Some(6));
        assert_eq!(b.take(1), Some(0), "121 bits, then padding");
    }

    /// A 905 built the same way: an instanced item and the owner value; a negative content index (-1)
    /// round-trips through the bias.
    #[test]
    fn a_refund_request_decodes() {
        let mut bits = String::new();
        bits += "1";
        bits += &format!("{:064b}", 0xD100_0000_0000_0601u64);
        bits += &format!("{:016b}", (-1i16 as u16) ^ 0x8000);
        bits += &format!("{:064b}", 0x1234_5678_9ABC_DEF0u64);
        while bits.len() % 8 != 0 {
            bits.push('0');
        }
        let bytes: Vec<u8> = (0..bits.len()).step_by(8).map(|i| u8::from_str_radix(&bits[i..i + 8], 2).unwrap()).collect();
        assert_eq!(refund_request(&bytes), Some(RefundRequest {
            instanced: true, instance: 0xD100_0000_0000_0601, content: -1, owner: 0x1234_5678_9ABC_DEF0,
        }));
        assert_eq!(refund_request(&bytes[..10]), None);
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    /// Equipping the second rifle (sense81's 403) moves the primary entry (index 7) to it.
    #[test]
    fn an_equip_points_the_bucket_entry_at_the_item() {
        let mut p = st(0).profile;
        let put = |p: &mut Vec<u8>, slot: usize, ci: u16, soid: u64| {
            let e = PROFILE_SLOTS + 32 * slot;
            p[e..e + 2].copy_from_slice(&ci.to_le_bytes());
            p[e + 8..e + 16].copy_from_slice(&soid.to_le_bytes());
        };
        put(&mut p, 93, 3598, 0xD100_0000_0000_0401);
        put(&mut p, 94, 3598, 0xD100_0000_0000_030B);
        p[EQUIPPED + 56..EQUIPPED + 64].copy_from_slice(&0xD100_0000_0000_0401u64.to_le_bytes());
        apply_equip(&mut p, 0xD100_0000_0000_030B).unwrap();
        assert_eq!(&p[EQUIPPED + 56..EQUIPPED + 64], &0xD100_0000_0000_030Bu64.to_le_bytes());
        assert!(apply_equip(&mut p, 0xD100_0000_0000_0999).is_err(), "an instance in no slot is refused");
        put(&mut p, 0, 872, 0xD100_0000_0000_0500);
        assert!(apply_equip(&mut p, 0xD100_0000_0000_0500).is_err(), "Materials have no equipped entry");
    }

    #[test]
    fn an_unknown_sale_is_refused() {
        let mut s = st(25_000);
        assert!(apply(&mut s, 6, 99).is_err());
    }
}
