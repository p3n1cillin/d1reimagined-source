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
pub(crate) const BUCKETS: &[(u8, u16, u16, u8)] = &[
    (0x05, 0, 20, 0), (0x01, 20, 20, 0), (0x06, 40, 3, 0), (0x07, 43, 10, 0), (0x08, 53, 10, 0),
    (0x09, 63, 10, 0), (0x0a, 73, 10, 0), (0x0b, 83, 10, 0), (0x02, 93, 10, 0), (0x03, 103, 10, 0),
    (0x04, 113, 10, 0), (0x0c, 123, 10, 0), (0x0f, 133, 10, 0), (0x0d, 143, 10, 0), (0x0e, 153, 10, 0),
    (0x11, 163, 5, 0), (0x12, 168, 1, 0), (0x10, 169, 20, 0), (0x14, 189, 20, 0), (0x15, 0, 20, 1),
    (0x16, 20, 108, 1), (0x17, 128, 72, 1), (0x18, 200, 108, 1), (0x13, 209, 1, 0), (0x19, 210, 10, 0),
    (0x1b, 220, 10, 0), (0x1c, 230, 10, 0), (0x1e, 240, 10, 0), (0x1d, 250, 12, 0), (0x1a, 262, 16, 0),
    (0x1f, 308, 1, 1), (0x20, 278, 1, 0), (0x21, 279, 1, 0), (0x22, 309, 1, 1), (0x23, 310, 1, 1),
    (0x24, 280, 10, 0), (0x25, 290, 32, 0), (0x26, 311, 1, 1), (0x27, 312, 6, 1),
];

pub(crate) const PROFILE_SLOTS: usize = 0x38;
const ACCOUNT_ENTRIES: usize = 326;

/// One sale: item content index, its bucket and instanced flag, and `(content, quantity, bucket)` costs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sale {
    /// The vendor's manifest hash (`DestinyVendorDefinition`), which the refund policies key on.
    pub vendor_hash: u32,
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
        let vendor_hash = u32::from_str_radix(f[2].trim_start_matches("0x"), 16).unwrap_or(0);
        out.insert((v, s), Sale { vendor_hash, item, bucket, instanced: f[5] == "1", costs });
    }
    out
}

/// A profile slot's `(content index, stack quantity at +0x10)`.
pub(crate) fn profile_entry(profile: &[u8], slot: u16) -> (u16, u32) {
    let e = PROFILE_SLOTS + 32 * slot as usize;
    (u16::from_le_bytes([profile[e], profile[e + 1]]), u32::from_le_bytes(profile[e + 0x10..e + 0x14].try_into().unwrap()))
}

pub(crate) fn bucket(ordinal: u8) -> Option<(u16, u16, u8)> {
    BUCKETS.iter().find(|b| b.0 == ordinal).map(|b| (b.1, b.2, b.3))
}

struct State {
    sales: HashMap<(u8, u16), Sale>,
    account: BTreeMap<u16, (u16, u32)>,
    profile: Vec<u8>,
    template: Vec<u8>,
    next_soid: u64,
    oracle: String,
    /// Refund records sent as account field [20] (newest last, at most [`REFUND_RECORDS`]).
    refunds: Vec<RefundRecord>,
    /// Stream I: the item instance of an account entry (the vault), written at entry `+8`. The
    /// player store keeps it (`VendorCommit.slot_instances`, the record's `slot_instances`).
    account_instances: BTreeMap<u16, u64>,
}

/// Account field [20] (`0x8080174F`, 6248 bytes): a u32, then 30 records of 208 bytes
/// (`0x80801781`). The vendor refund screen (`0xA205E0`) shows the records whose type (`+0x98`, 2 bits)
/// is 2 and whose expiry (`+0xC0`) is later than the client's time (the u64 a 901 or 905 request
/// carries). Each holds the costs to give back (`+0x00`: 3-bit count, up to six `{u16 content @0, u64
/// @8, u32 quantity @0x10}` from `+0x08`) and the item (`+0xA0`, same shape). `+0xB8` (u16), `+0xBC`
/// (u32) and `+0xC8` (u64) are filled with the vendor, the sale index and the purchase time as a guess.
const REFUND_RECORDS: usize = 30;
const REFUND_FIELD_BYTES: usize = 6248;
const REFUND_RECORD_BYTES: usize = 208;

/// The records persist with the player store (`D1_STORE=1`), so they are its type.
use crate::player_store::Refund as RefundRecord;

/// The 6248-byte `0x8080174F` object for `records` (oldest first).
fn refund_object(records: &[RefundRecord]) -> Vec<u8> {
    let mut o = vec![0u8; REFUND_FIELD_BYTES];
    let records = &records[records.len().saturating_sub(REFUND_RECORDS)..];
    o[0..4].copy_from_slice(&(records.len() as u32).to_le_bytes());
    for (i, r) in records.iter().enumerate() {
        let at = 8 + REFUND_RECORD_BYTES * i;
        let costs = &r.costs[..r.costs.len().min(6)];
        o[at] = costs.len() as u8;
        for (j, (content, quantity)) in costs.iter().enumerate() {
            let c = at + 8 + 24 * j;
            o[c..c + 2].copy_from_slice(&content.to_le_bytes());
            o[c + 0x10..c + 0x14].copy_from_slice(&quantity.to_le_bytes());
        }
        o[at + 0x98] = 2;
        o[at + 0xA0..at + 0xA2].copy_from_slice(&r.item.to_le_bytes());
        o[at + 0xA8..at + 0xB0].copy_from_slice(&r.instance.to_le_bytes());
        o[at + 0xB0..at + 0xB4].copy_from_slice(&1u32.to_le_bytes());
        o[at + 0xB8..at + 0xBA].copy_from_slice(&u16::from(r.vendor).to_le_bytes());
        o[at + 0xBC..at + 0xC0].copy_from_slice(&u32::from(r.sale).to_le_bytes());
        o[at + 0xC0..at + 0xC8].copy_from_slice(&r.expires.to_le_bytes());
        o[at + 0xC8..at + 0xD0].copy_from_slice(&r.bought.to_le_bytes());
    }
    o
}

fn state() -> &'static Mutex<Option<State>> {
    static S: OnceLock<Mutex<Option<State>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(load().map_err(|e| tracing::warn!("      vendor purchases off: {e}")).ok()))
}

/// The first item instance a purchase mints, when the player store has no later one.
const FIRST_PURCHASE_SOID: u64 = crate::player_store::FIRST_ITEM_SOID;

fn load() -> anyhow::Result<State> {
    let sales_path = std::env::var("D1_VENDOR_SALES")?;
    let sales = parse_sales(&std::fs::read_to_string(&sales_path)?);
    let template = std::fs::read(std::env::var("D1_VENDOR_ITEM_TEMPLATE")?)?;
    // With the player store on, start from what it kept (the profile and account as last served,
    // the next instance, the refund records). Its first boot gives the knobs' values, as without it.
    let seed = crate::player_store::vendor_seed();
    let profile = match seed.as_ref().and_then(|s| s.profile.clone()) {
        Some(p) => p,
        None => std::fs::read(std::env::var("D1_VENDOR_PROFILE_OBJ")?)?,
    };
    let account = match &seed {
        Some(s) => s.slots.clone(),
        None => crate::player_store::parse_account_slots(&std::env::var("D1_VENDOR_ACCOUNT_SLOTS").unwrap_or_default()),
    };
    let next_soid = seed.as_ref().and_then(|s| s.next_item_soid).unwrap_or(FIRST_PURCHASE_SOID);
    let account_instances = seed.as_ref().map(|s| s.slot_instances.clone()).unwrap_or_default();
    let refunds = seed.map(|s| s.refunds).unwrap_or_default();
    let oracle = oracle_path();
    tracing::warn!("      vendor purchases on: {} sales from {sales_path}, account {account:?}", sales.len());
    Ok(State { sales, account, profile, template, next_soid, oracle, refunds, account_instances })
}

/// `D1_ORACLE`, the client's codec.
pub(crate) fn oracle_path() -> String {
    std::env::var("D1_ORACLE").unwrap_or_else(|_| "d1-server/target/release/d1-oracle".into())
}

/// A scratch-file name no other caller uses at the same time.
pub(crate) fn unique_name(prefix: &str) -> String {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    format!("{prefix}-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst))
}

/// Decode a client-encoded payload into its C object (`d1-oracle object --decode-file ... --out-obj`).
pub(crate) fn decode_object(oracle: &str, tag: &str, payload: &[u8], name: &str) -> anyhow::Result<Vec<u8>> {
    let dir = std::env::temp_dir().join("d1-vendor");
    std::fs::create_dir_all(&dir)?;
    let name = unique_name(name);
    let src = dir.join(format!("{name}.bin"));
    let obj = dir.join(format!("{name}.obj"));
    std::fs::write(&src, payload)?;
    let _ = std::fs::remove_file(&obj);
    std::process::Command::new(oracle).args(["object", "--tag", tag, "--decode-file"]).arg(&src)
        .arg("--out-obj").arg(&obj).output()?;
    let o = std::fs::read(&obj).map_err(|e| anyhow::anyhow!("{tag}: the client's codec wrote no object: {e}"));
    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&obj);
    o
}

/// Run `d1-oracle object --tag TAG --obj-file …`; returns the payload and its bit count.
pub(crate) fn encode(oracle: &str, tag: &str, obj: &[u8], name: &str) -> anyhow::Result<(Vec<u8>, usize)> {
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

/// Decode the 901 request with the client's codec: (vendor, sale, the client's time stamp at +8).
fn decode_request(oracle: &str, req: &[u8]) -> anyhow::Result<(u8, u16, u64)> {
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
    let stamp = text.lines().find_map(|l| {
        l.trim().strip_prefix("+0x0008").and_then(|v| u64::from_str_radix(v.trim().trim_start_matches("0x"), 16).ok())
    }).unwrap_or(0);
    Ok(((w0 & 0xFF) as u8, ((w0 >> 16) & 0xFFFF) as u16, stamp))
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
/// @0x10}`) and a u64 at `+0x18` from `0x265100(...)`, the same value 901 sends as its u64 (`0xB11`
/// in run sense69's purchase). The vendor refund screen (`0xA205E0`) compares that value with each
/// refund record's expiry (`+0x5B18`), so it is most likely the client's current time.
/// Decoded from the bitstream directly, since its schema tag isn't known yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefundRequest {
    pub instanced: bool,
    pub instance: u64,
    pub content: i16,
    pub stamp: u64,
}

pub fn refund_request(req: &[u8]) -> Option<RefundRequest> {
    let mut b = Bits { data: req, at: 0 };
    Some(RefundRequest {
        instanced: b.take(1)? != 0,
        instance: b.take(64)?,
        content: b.biased(16)? as i16,
        stamp: b.take(64)?,
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

/// ⭐ Integration review findings 3, 14 and 25: with the player store on, the store is the one
/// source of truth for the current profile, wallet, refund records and next item SOID. Every vendor
/// op starts from it (a grant, an XP push or a completion may have changed it since this state's
/// last publish), so an op never publishes a profile that lacks them, and an instance SOID the
/// rewards minted is never minted again. The store's profile object is used when it is decoded.
fn sync_from_store(st: &mut State) {
    let Some((object, slots, instances, next, refunds)) = crate::player_store::view(|s| {
        let a = &s.record.account;
        (s.chr().profile_object.clone(), a.slots.clone(), a.slot_instances.clone(), a.next_item_soid, a.refunds.clone())
    }) else {
        return;
    };
    if let Some(o) = object.filter(|o| o.len() == st.profile.len()) {
        st.profile = o;
    }
    st.account = slots;
    st.account_instances = instances;
    if let Some(n) = next {
        st.next_soid = st.next_soid.max(n);
    }
    st.refunds = refunds;
}

/// Encode the account (only the intended fields, plus the inventory) and write both push files,
/// then write the change through to the player store (when on): `what` is the change, `removed` a
/// destroyed instance.
fn publish(st: &State, sidecar: Option<(u64, Vec<u8>)>, what: &str, removed: Option<u64>) -> anyhow::Result<()> {
    let account = account_payload(&st.oracle, &st.account, &st.account_instances, &st.refunds)?;
    let (profile, _) = encode(&st.oracle, "0x80801743", &st.profile, "profile")?;
    if let Some(p) = std::env::var_os("D1_ACCOUNT_PUSH_FILE").map(PathBuf::from) {
        std::fs::write(&p, &account)?;
    }
    if let Some(p) = std::env::var_os("D1_PROFILE_PUSH_FILE").map(PathBuf::from) {
        // One profile push at a time: a store push not yet delivered is not overwritten
        // (integration review, finding 3).
        crate::player_store::wait_for_push_taken();
        if let Some((soid, element)) = &sidecar {
            std::fs::write(p.with_extension("item"), element)?;
            std::fs::write(p.with_extension("soid"), format!("{soid:#x}"))?;
        }
        std::fs::write(&p, &profile)?;
    }
    crate::player_store::note_vendor(crate::player_store::VendorCommit {
        profile_payload: profile,
        profile_object: st.profile.clone(),
        account_payload: account,
        slots: st.account.clone(),
        slot_instances: st.account_instances.clone(),
        next_item_soid: st.next_soid,
        refunds: st.refunds.clone(),
        added: sidecar,
        removed,
        what: what.to_string(),
    });
    Ok(())
}

/// The account object's inventory (`0x80800FE6`, field [6]) for `slots` and their instances: every
/// entry empty (`0xFFFF`) but the slots that hold something.
pub(crate) fn account_inventory(slots: &BTreeMap<u16, (u16, u32)>, instances: &BTreeMap<u16, u64>) -> Vec<u8> {
    let mut inv = vec![0u8; 10_440];
    for s in 0..ACCOUNT_ENTRIES {
        inv[8 + 32 * s..8 + 32 * s + 2].copy_from_slice(&[0xFF, 0xFF]);
    }
    for (slot, (ci, q)) in slots {
        let e = 8 + 32 * *slot as usize;
        if e + 0x14 > inv.len() {
            continue;
        }
        inv[e..e + 2].copy_from_slice(&ci.to_le_bytes());
        inv[e + 0x10..e + 0x14].copy_from_slice(&q.to_le_bytes());
        if let Some(i) = instances.get(slot) {
            inv[e + 8..e + 16].copy_from_slice(&i.to_le_bytes());
        }
    }
    inv
}

/// ⭐ The account push for a wallet: the inventory (field [6]) and the refund records (field [20])
/// encoded with the client's codec, in the account RSAT a purchase pushes. The vendor state's
/// [`publish`] and the player store's own account push (`player_store::push_account`, stream O2:
/// the store on, the vendor state off) both build it here.
pub(crate) fn account_payload(oracle: &str, slots: &BTreeMap<u16, (u16, u32)>, instances: &BTreeMap<u16, u64>, refunds: &[RefundRecord]) -> anyhow::Result<Vec<u8>> {
    let (bits, n) = encode(oracle, "0x80800fe6", &account_inventory(slots, instances), &unique_name("account-inventory"))?;
    let acct = crate::env_soid("D1_ACCOUNT_SOID", 0xD100_0000_0000_0001);
    let chr = crate::env_soid("D1_CHARACTER_SOID", 0xD100_0000_0000_0101);
    let prof = crate::env_soid("D1_PROFILE_SOID", 0xD100_0000_0000_0201);
    let refunds = if refunds.is_empty() {
        None
    } else {
        Some(encode(oracle, "0x8080174f", &refund_object(refunds), &unique_name("account-refunds"))?)
    };
    let mut fields: Vec<(u32, &[u8], usize)> = vec![(6, &bits, n)];
    if let Some((rb, rn)) = &refunds {
        fields.push((20, rb, *rn));
    }
    Ok(d1_bap::account_rsat_with_fields(acct, &[chr], prof, &fields))
}

/// Whether the vendor state is on (`D1_VENDOR_SALES` and its inputs loaded).
pub fn active() -> bool {
    state().lock().map(|s| s.is_some()).unwrap_or(false)
}

/// The profile object the vendor state serves (None when it is off).
pub fn current_profile() -> Option<Vec<u8>> {
    state().lock().ok()?.as_ref().map(|s| s.profile.clone())
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
            let o = decode_object(&st.oracle, "0x80801743", &payload, "pushed-profile")?;
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

/// The player store's wallet changed outside a vendor request (a reward grant from the activity
/// host, `player_store::run_grant`): take its slots and vault instances, its edited profile object
/// (stacks and items a grant put in character buckets, stream O2) and the new instances' decoded
/// elements, and publish the account and profile the way a purchase does, so the client receives
/// them and the next purchase starts from them. Each instance goes out as its own push (a push
/// carries one item, `D1_PROFILE_PUSH_FILE`'s `.item` / `.soid`), the next once the client took the
/// last. Returns false when the vendor state is off (`D1_VENDOR_SALES` unset): the caller pushes
/// through the player store instead (`player_store::push_account`).
pub fn adopt_store_state(slots: BTreeMap<u16, (u16, u32)>, instances: BTreeMap<u16, u64>, profile: Option<Vec<u8>>, items: Vec<(u64, Vec<u8>)>, what: &str) -> bool {
    let Ok(mut guard) = state().lock() else { return false };
    let Some(st) = guard.as_mut() else {
        return false;
    };
    st.account = slots;
    st.account_instances = instances;
    if let Some(p) = profile.filter(|p| p.len() == st.profile.len()) {
        st.profile = p;
    }
    if let Some(max) = items.iter().map(|(soid, _)| *soid + 1).max() {
        st.next_soid = st.next_soid.max(max);
    }
    let mut sidecars: Vec<Option<(u64, Vec<u8>)>> = Vec::new();
    for (soid, element) in items {
        match encode(&st.oracle, "0x80801732", &element, &unique_name("grant-item")) {
            Ok((e, _)) => sidecars.push(Some((soid, e))),
            Err(e) => tracing::warn!("      ⛔ {what}: item {soid:#x} did not encode ({e:#}); it reaches the client with the next profile only"),
        }
    }
    if sidecars.is_empty() {
        sidecars.push(None);
    }
    for (i, sidecar) in sidecars.into_iter().enumerate() {
        if i > 0 {
            crate::player_store::wait_for_push_taken();
        }
        let soid = sidecar.as_ref().map(|s| s.0);
        match publish(st, sidecar, what, None) {
            Ok(()) => tracing::warn!("      ⭐ {what}: account and profile pushed{}", soid.map(|s| format!(" with item {s:#x}")).unwrap_or_default()),
            Err(e) => tracing::warn!("      ⛔ {what}: the push failed: {e:#}"),
        }
    }
    true
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
        sync_from_store(st);
        let result = (|| -> anyhow::Result<String> {
            let (vendor, sale, stamp) = decode_request(&st.oracle, &req)?;
            // ⭐ Stream I (D1_SOCIAL=1): the manifest's rules, decodes, bounties and Armsday.
            if crate::social_hooks::enabled() {
                return social_buy(st, vendor, sale, stamp);
            }
            let (what, sidecar) = apply(st, vendor, sale)?;
            // The refund record the client's refund screen needs (account field [20]).
            let s = &st.sales[&(vendor, sale)];
            let window = refund_window_for_item(s.vendor_hash, Some(s.item));
            let note = match window {
                Some(window) => {
                    st.refunds.push(RefundRecord {
                        vendor, sale, item: s.item, instance: sidecar.as_ref().map(|x| x.0).unwrap_or(0),
                        costs: s.costs.iter().map(|&(content, quantity, _)| (content, quantity)).collect(),
                        bought: stamp, expires: stamp.saturating_add(window),
                    });
                    format!("refund record until {:#x} (bought at {stamp:#x})", stamp.saturating_add(window))
                }
                None => format!("vendor {:#x} refunds nothing (refund-policies.tsv): no refund record", s.vendor_hash),
            };
            publish(st, sidecar, &format!("purchase: {what}"), None)?;
            Ok(format!("{what}; {note}"))
        })();
        match result {
            Ok(what) => tracing::warn!("      ⭐⭐ VENDOR PURCHASE APPLIED: {what}; pushes written"),
            Err(e) => tracing::warn!("      ⛔ vendor purchase NOT applied: {e}"),
        }
    });
}

/// How long a purchase from `vendor_hash` can be returned (seconds of the 901's clock), or None.
/// `D1_REFUND_WINDOW` (seconds) forces one for every vendor. Otherwise, with the player store on
/// (`D1_STORE=1`), D1's rule from the manifest: Eververse and its Silver Dust store only, 15 minutes
/// (progression::RefundPolicies). With it off, every purchase gets 1,800, as the lab has run since
/// #167, so the lab's behaviour is unchanged.
fn refund_window(vendor_hash: u32) -> Option<u64> {
    refund_window_for_item(vendor_hash, None)
}

fn refund_window_for_item(vendor_hash: u32, content: Option<u16>) -> Option<u64> {
    let forced = std::env::var("D1_REFUND_WINDOW").ok().and_then(|v| v.parse().ok());
    let store = crate::player_store::enabled().then(crate::progression::RefundPolicies::load);
    refund_window_for_item_with(vendor_hash, content, forced, store.as_ref())
}

/// [`refund_window_for_item`] without the environment.
fn refund_window_for_item_with(vendor_hash: u32, content: Option<u16>, forced: Option<u64>, policies: Option<&crate::progression::RefundPolicies>) -> Option<u64> {
    match (forced, policies, content) {
        (None, Some(p), Some(ci)) => p.window_secs(vendor_hash, Some(crate::content::item_hash(ci)?)),
        _ => refund_window_with(vendor_hash, forced, policies),
    }
}

/// The refund window of one sale: its vendor's policy, narrowed to the sold item when the store
/// is on (`refund_window_for_item`). An unknown sale has its vendor's (none) window.
fn sale_refund_window(sale: Option<&Sale>) -> Option<u64> {
    match sale {
        Some(s) => refund_window_for_item(s.vendor_hash, Some(s.item)),
        None => refund_window(0),
    }
}

/// [`refund_window`] without the environment: `policies` is Some when the store is on.
fn refund_window_with(vendor_hash: u32, forced: Option<u64>, policies: Option<&crate::progression::RefundPolicies>) -> Option<u64> {
    match (forced, policies) {
        (Some(w), _) => Some(w),
        (None, Some(p)) => p.window_secs(vendor_hash, None),
        (None, None) => Some(1800),
    }
}

/// The profile's equipped item SOIDs: 20 x u64 at +0x2B80 (`d1-oracle profile layout`).
const EQUIPPED: usize = 0x2B80;

/// The equipped-array index of a bucket, read from the geared fixture (each equipped instance's
/// inventory slot gives its bucket): subclass, the five armour pieces, the three weapons, the vehicle
/// and the ghost. Other buckets are not known yet.
pub(crate) fn equipped_index(ordinal: u8) -> Option<usize> {
    Some(match ordinal {
        0x06 => 0,
        0x07..=0x0B => 1 + (ordinal - 0x07) as usize,
        0x02..=0x04 => 6 + (ordinal - 0x02) as usize,
        0x0D => 10,
        0x0E => 11,
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
        sync_from_store(st);
        let result = (|| -> anyhow::Result<String> {
            let instance = decode_instance(&st.oracle, &req)?;
            // ⭐ Stream I (D1_SOCIAL=1): the game's equip rules (one exotic, class gear).
            if crate::social_hooks::enabled() {
                let what = with_wire_store(st, |w, _| { let chr = w.character.id; crate::inventory_ops::equip(w, crate::tables::tables(), chr, instance).map_err(|e| e.to_string()) })?;
                publish(st, None, &format!("equip: {what}"), None)?;
                return Ok(what);
            }
            let mut profile = st.profile.clone();
            let what = apply_equip(&mut profile, instance)?;
            st.profile = profile;
            publish(st, None, &format!("equip: {what}"), None)?;
            Ok(what)
        })();
        match result {
            Ok(what) => tracing::warn!("      ⭐⭐ EQUIP APPLIED: {what}; profile push written"),
            Err(e) => tracing::warn!("      ⛔ equip NOT applied: {e}"),
        }
    });
}

/// Decode a 402 `inventory_perform_item_action` request (`0x80801767`: {bool, u64 instance, i16
/// content index} + i32) with the client's codec: `(instanced, instance, content, quantity)`.
/// Holding Square on an unequipped Khvostov in the character screen (sense118, 2 November) sent
/// `e880000000000185c7074000000000` = {1, 0xd10000000000030b, 3598} + 0.
fn decode_item_action(oracle: &str, req: &[u8]) -> anyhow::Result<(bool, u64, u16, u32)> {
    let hex: String = req.iter().map(|b| format!("{b:02x}")).collect();
    let o = std::process::Command::new(oracle).args(["object", "--tag", "0x80801767", "--decode", &hex]).output()?;
    let text = String::from_utf8_lossy(&o.stdout);
    let field = |at: &str| text.lines().find_map(|l| l.trim().strip_prefix(at).map(|v| u64::from_str_radix(v.trim().trim_start_matches("0x"), 16)));
    let first = field("+0x0000").ok_or_else(|| anyhow::anyhow!("no +0x0000 in the decoded 402 request"))??;
    let instance = field("+0x0008").ok_or_else(|| anyhow::anyhow!("no +0x0008 in the decoded 402 request"))??;
    let third = field("+0x0010").ok_or_else(|| anyhow::anyhow!("no +0x0010 in the decoded 402 request"))??;
    Ok((first & 1 == 1, instance, (third & 0xFFFF) as u16, (third >> 32) as u32))
}

/// What dismantling one item gives back: `content_index -> [(content, quantity, bucket)]`, from
/// `D1_DISMANTLE_YIELDS` (`content_index content:quantity:bucket,...`, `#` comments). The yields
/// were the original servers' rules, not client data, so each row must cite its source; an item
/// with no row gives nothing back (logged), never an invented amount.
fn dismantle_yields() -> HashMap<u16, Vec<(u16, u32, u8)>> {
    let Some(path) = std::env::var_os("D1_DISMANTLE_YIELDS") else { return HashMap::new() };
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    parse_yields(&text)
}

fn parse_yields(text: &str) -> HashMap<u16, Vec<(u16, u32, u8)>> {
    let hex = |s: &str| u8::from_str_radix(s.trim_start_matches("0x"), 16).ok();
    text.lines().map(|l| l.split('#').next().unwrap_or("").trim()).filter(|l| !l.is_empty()).filter_map(|l| {
        let (ci, rest) = l.split_once(char::is_whitespace)?;
        let ys = rest.split(',').filter_map(|y| {
            let p: Vec<&str> = y.trim().split(':').collect();
            Some((p.first()?.parse().ok()?, p.get(1)?.parse().ok()?, hex(p.get(2)?)?))
        }).collect();
        Some((ci.trim().parse().ok()?, ys))
    }).collect()
}

/// Apply a dismantle (a 402 for one instanced item): empty its profile slot and credit its yields.
/// An equipped item can't be dismantled (the game offers no Dismantle on it).
fn apply_dismantle(st: &mut State, instance: u64, content: u16, yields: &[(u16, u32, u8)]) -> anyhow::Result<String> {
    anyhow::ensure!(instance != 0, "a 402 without an instance (a stack) is not handled yet");
    let mut profile = st.profile.clone();
    let mut account = st.account.clone();
    let slot = (0..340u16).find(|s| {
        let e = PROFILE_SLOTS + 32 * *s as usize;
        u64::from_le_bytes(profile[e + 8..e + 16].try_into().unwrap()) == instance
    }).ok_or_else(|| anyhow::anyhow!("instance {instance:#x} is in no profile slot"))?;
    let held = profile_entry(&profile, slot).0;
    anyhow::ensure!(held == content, "slot {slot} holds content {held}, the request names {content}");
    let equipped = (0..20).any(|i| u64::from_le_bytes(profile[EQUIPPED + 8 * i..EQUIPPED + 8 * i + 8].try_into().unwrap()) == instance);
    anyhow::ensure!(!equipped, "{instance:#x} is equipped");
    let e = PROFILE_SLOTS + 32 * slot as usize;
    profile[e..e + 2].copy_from_slice(&[0xFF, 0xFF]);
    profile[e + 8..e + 16].fill(0);
    profile[e + 0x10..e + 0x14].fill(0);
    for (ci, q, b) in yields {
        let (first, n, scope) = bucket(*b).ok_or_else(|| anyhow::anyhow!("unknown bucket {b:#x}"))?;
        if scope == 1 {
            let at = (first..first + n).find(|s| account.get(s).map(|x| x.0) == Some(*ci))
                .or_else(|| (first..first + n).find(|s| !account.contains_key(s)))
                .ok_or_else(|| anyhow::anyhow!("account bucket {b:#x} is full"))?;
            let have = account.get(&at).map(|x| x.1).unwrap_or(0);
            account.insert(at, (*ci, have + q));
        } else {
            let at = (first..first + n).find(|s| profile_entry(&profile, *s).0 == *ci)
                .or_else(|| (first..first + n).find(|s| profile_entry(&profile, *s).0 == 0xFFFF))
                .ok_or_else(|| anyhow::anyhow!("bucket {b:#x} is full"))?;
            let have = profile_entry(&profile, at).1;
            let x = PROFILE_SLOTS + 32 * at as usize;
            profile[x..x + 2].copy_from_slice(&ci.to_le_bytes());
            profile[x + 0x10..x + 0x14].copy_from_slice(&(have + q).to_le_bytes());
        }
    }
    st.profile = profile;
    st.account = account;
    Ok(format!("{instance:#x} (content {content}, slot {slot}) dismantled; yields {yields:?}"))
}

/// Handle a 402 request body: dismantle the item in the purchase state's profile and push it.
pub fn on_item_action(req: Vec<u8>) {
    if state().lock().map(|s| s.is_none()).unwrap_or(true) {
        return;
    }
    std::thread::spawn(move || {
        let Ok(mut guard) = state().lock() else { return };
        let Some(st) = guard.as_mut() else { return };
        sync_from_store(st);
        let result = (|| -> anyhow::Result<String> {
            let (instanced, instance, content, quantity) = decode_item_action(&st.oracle, &req)?;
            anyhow::ensure!(instanced && quantity == 0, "only an instanced item's dismantle is handled (instanced {instanced}, quantity {quantity})");
            let table = dismantle_yields();
            let yields = table.get(&content).cloned()
                .or_else(|| crate::social_hooks::placeholder_yields(content)) // Stream I, labelled stand-ins
                .unwrap_or_default();
            let mut what = apply_dismantle(st, instance, content, &yields)?;
            if !table.contains_key(&content) && !yields.is_empty() {
                what.push_str(" (PLACEHOLDER yields, D1_DISMANTLE_PLACEHOLDER=1: not sourced)");
            } else if !table.contains_key(&content) {
                what.push_str(" (no D1_DISMANTLE_YIELDS row for this item: nothing given back)");
            }
            publish(st, None, &format!("dismantle: {what}"), Some(instance))?;
            Ok(what)
        })();
        match result {
            Ok(what) => tracing::warn!("      ⭐⭐ DISMANTLE APPLIED: {what}; pushes written"),
            Err(e) => tracing::warn!("      ⛔ dismantle NOT applied: {e}"),
        }
    });
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
//  ⭐ Stream I (2026-11-05): the social services on the purchase state. The services
//  (`crate::vendors`, `crate::inventory_ops`, ...) act on a `store::WireStore` built from this
//  state; the result is written back and published like a purchase, so the player store (#307)
//  persists it through `note_vendor`. Only with `D1_SOCIAL=1`.
// ═══════════════════════════════════════════════════════════════════════════════════════════════

/// Run `f` on the state as a `WireStore` (with the receipts as the store's), and adopt the result
/// only when `f` succeeds.
fn with_wire_store<T>(st: &mut State, f: impl FnOnce(&mut crate::store::WireStore, &HashMap<(u8, u16), Sale>) -> Result<T, String>) -> anyhow::Result<T> {
    let mut w = crate::social_hooks::wire_store(st.account.clone(), st.account_instances.clone(), st.profile.clone(), st.next_soid);
    let chr = w.character.id;
    let receipts = st.refunds.iter().map(|r| crate::social_hooks::receipt_from(r, &st.sales)).collect();
    crate::store::PlayerStore::set_receipts(&mut w, chr, receipts);
    let out = f(&mut w, &st.sales).map_err(|e| anyhow::anyhow!(e))?;
    st.refunds = crate::store::PlayerStore::receipts(&w, chr).iter().map(crate::social_hooks::refund_from).collect();
    st.account = w.account.clone();
    st.account_instances = w.account_instances.clone();
    st.profile = w.profile.clone();
    st.next_soid = w.next_soid;
    crate::social_hooks::keep_side(w.side);
    Ok(out)
}

/// A 901 through `vendors::purchase::buy`: the sold item's element (an instance) is built from the
/// item template as `apply` does.
fn social_buy(st: &mut State, vendor: u8, sale: u16, stamp: u64) -> anyhow::Result<String> {
    // The refund window of this sale's item (stream O2: per sale, as the manifest's refund
    // policy lists items, not per vendor; `refund_window_for_item`).
    let opts = crate::social_hooks::buy_options(sale_refund_window(st.sales.get(&(vendor, sale))));
    let p = with_wire_store(st, |w, sales| {
        let m = crate::social_hooks::market(sales);
        let chr = w.character.id;
        crate::vendors::purchase::buy(w, &m, chr, vendor, sale, crate::social_hooks::clock(stamp), opts).map_err(|e| e.to_string())
    })?;
    let sidecar = if p.item.instance != 0 {
        let mut item = st.template.clone();
        item[0..8].copy_from_slice(&p.item.instance.to_le_bytes());
        item[0x24..0x26].copy_from_slice(&p.item.content.to_le_bytes());
        Some((p.item.instance, encode(&st.oracle, "0x80801732", &item, "item")?.0))
    } else {
        None
    };
    publish(st, sidecar, &format!("purchase: {}", p.what), None)?;
    crate::social_hooks::bridge_progressions();
    Ok(p.what)
}

/// The 404/405/902/903/905 services: decode, apply, publish.
pub(crate) fn on_social(id: u16, req: Vec<u8>) {
    if state().lock().map(|s| s.is_none()).unwrap_or(true) {
        return;
    }
    std::thread::spawn(move || {
        let Ok(mut guard) = state().lock() else { return };
        let Some(st) = guard.as_mut() else { return };
        sync_from_store(st);
        let oracle = st.oracle.clone();
        let result = (|| -> anyhow::Result<String> {
            let op = crate::social_hooks::decode(id, &oracle, &req)?;
            let what = with_wire_store(st, |w, sales| crate::social_hooks::apply(w, sales, op))?;
            publish(st, None, &what, None)?;
            crate::social_hooks::bridge_progressions();
            Ok(what)
        })();
        match result {
            Ok(what) => tracing::warn!("      ⭐⭐ SOCIAL ({id}) APPLIED: {what}; pushes written"),
            Err(e) => tracing::warn!("      ⛔ social ({id}) NOT applied: {e}"),
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
        State { sales: parse_sales(TSV), account, profile, template: vec![0; 184], next_soid: 0x601, oracle: String::new(), refunds: Vec::new(), account_instances: BTreeMap::new() }
    }

    fn with_rifle(s: &mut State, slot: usize, instance: u64, equipped: bool) {
        let e = PROFILE_SLOTS + 32 * slot;
        s.profile[e..e + 2].copy_from_slice(&3598u16.to_le_bytes());
        s.profile[e + 8..e + 16].copy_from_slice(&instance.to_le_bytes());
        if equipped {
            s.profile[EQUIPPED + 8 * 6..EQUIPPED + 8 * 7].copy_from_slice(&instance.to_le_bytes());
        }
    }

    #[test]
    fn a_dismantle_empties_the_slot_and_credits_its_yields() {
        let mut s = st(100);
        with_rifle(&mut s, 94, 0xd100_0000_0000_030b, false);
        apply_dismantle(&mut s, 0xd100_0000_0000_030b, 3598, &[(869, 5, 0x1f)]).unwrap();
        assert_eq!(profile_entry(&s.profile, 94).0, 0xFFFF);
        assert_eq!(s.account[&308], (869, 105));
    }

    #[test]
    fn an_equipped_item_is_not_dismantled() {
        let mut s = st(100);
        with_rifle(&mut s, 93, 0xd100_0000_0000_0401, true);
        assert!(apply_dismantle(&mut s, 0xd100_0000_0000_0401, 3598, &[]).is_err());
        assert_eq!(profile_entry(&s.profile, 93).0, 3598);
    }

    #[test]
    fn the_yields_table_parses() {
        let y = parse_yields("# source: ...
3598	869:5:0x1f, 1043:1:0x24
");
        assert_eq!(y[&3598], vec![(869, 5, 0x1f), (1043, 1, 0x24)]);
    }

    #[test]
    fn the_table_parses() {
        let s = parse_sales(TSV);
        assert_eq!(s[&(6, 23)], Sale { vendor_hash: 0x080760b4, item: 840, bucket: 0x19, instanced: true, costs: vec![(869, 570, 0x1F)] });
    }

    /// With the store on, D1's rule: Tess's purchases are refundable for 15 minutes, Eva's are not.
    /// With it off, every purchase keeps the lab's 1,800.
    #[test]
    fn refund_records_follow_the_manifest_with_the_store() {
        let p = crate::progression::RefundPolicies::load();
        let (eververse, eva) = (0xe97a_aa75, 0x0807_60b4);
        assert_eq!(refund_window_with(eververse, None, Some(&p)), Some(900));
        assert_eq!(refund_window_with(eva, None, Some(&p)), None);
        assert_eq!(refund_window_with(eva, None, None), Some(1800), "store off: as before");
        assert_eq!(refund_window_with(eva, Some(60), Some(&p)), Some(60), "D1_REFUND_WINDOW forces one");
        let real = parse_sales(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../../fixtures/56578/vendor/vendor-sales.tsv")).unwrap());
        assert_eq!(real[&(12, 0)].vendor_hash, eververse, "client vendor 12 is Eververse");
        assert_eq!(real[&(6, 23)].vendor_hash, eva);
    }

    /// Stream O2: a sale's refund window is its item's, not its vendor's. Eververse refunds 67 of
    /// its 123 manifest sales; the others get no refund record.
    #[test]
    fn refunds_are_per_sale_not_per_vendor() {
        let p = crate::progression::RefundPolicies::load();
        let real = parse_sales(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../../fixtures/56578/vendor/vendor-sales.tsv")).unwrap());
        let eververse: Vec<&Sale> = real.values().filter(|s| s.vendor_hash == 0xe97a_aa75).collect();
        let windows: Vec<Option<u64>> = eververse.iter().map(|s| refund_window_for_item_with(s.vendor_hash, Some(s.item), None, Some(&p))).collect();
        assert!(windows.iter().any(|w| *w == Some(900)), "some Eververse sales refund for 15 minutes");
        assert!(windows.iter().any(Option::is_none), "and some do not, though the vendor does");
        assert_eq!(refund_window_with(0xe97a_aa75, None, Some(&p)), Some(900), "the per-vendor answer");
        assert_eq!(refund_window_for_item_with(0xe97a_aa75, Some(0), Some(60), Some(&p)), Some(60), "D1_REFUND_WINDOW still forces one");
    }

    /// The account inventory the store's own account push encodes (stream O2, vendor state off).
    #[test]
    fn the_account_inventory_holds_the_slots_and_their_instances() {
        let slots = BTreeMap::from([(308, (869, 25_000)), (310, (4548, 7)), (20, (1234, 1))]);
        let instances = BTreeMap::from([(20, 0xD100_0000_0000_7777u64)]);
        let inv = account_inventory(&slots, &instances);
        assert_eq!(inv.len(), 10_440);
        let at = |s: usize| 8 + 32 * s;
        assert_eq!(&inv[at(0)..at(0) + 2], &[0xFF, 0xFF], "an empty entry");
        assert_eq!(u16::from_le_bytes([inv[at(308)], inv[at(308) + 1]]), 869);
        assert_eq!(u32::from_le_bytes(inv[at(308) + 0x10..at(308) + 0x14].try_into().unwrap()), 25_000);
        assert_eq!(u32::from_le_bytes(inv[at(310) + 0x10..at(310) + 0x14].try_into().unwrap()), 7);
        assert_eq!(u64::from_le_bytes(inv[at(20) + 8..at(20) + 16].try_into().unwrap()), 0xD100_0000_0000_7777);
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
        publish(&s, sidecar, "test purchase", None).unwrap();
    }

    /// A price paid in a character-held material comes out of the profile's slots of its bucket,
    /// across stacks, and an emptied stack is freed. (No instance, so no codec call.)
    #[test]
    fn a_profile_held_cost_is_taken_from_its_stacks() {
        let mut s = st(0);
        s.sales.insert((5, 0), Sale { vendor_hash: 0, item: 900, bucket: 0x05, instanced: false, costs: vec![(872, 30, 0x05)] });
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

    /// A 905 built the same way: an instanced item and the time stamp; a negative content index (-1)
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
            instanced: true, instance: 0xD100_0000_0000_0601, content: -1, stamp: 0x1234_5678_9ABC_DEF0,
        }));
        assert_eq!(refund_request(&bytes[..10]), None);
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    /// Equipping the second rifle (sense81's 403) moves the primary entry (index 6) to it.
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
        p[EQUIPPED + 48..EQUIPPED + 56].copy_from_slice(&0xD100_0000_0000_0401u64.to_le_bytes());
        apply_equip(&mut p, 0xD100_0000_0000_030B).unwrap();
        assert_eq!(&p[EQUIPPED + 48..EQUIPPED + 56], &0xD100_0000_0000_030Bu64.to_le_bytes());
        assert!(apply_equip(&mut p, 0xD100_0000_0000_0999).is_err(), "an instance in no slot is refused");
        put(&mut p, 0, 872, 0xD100_0000_0000_0500);
        assert!(apply_equip(&mut p, 0xD100_0000_0000_0500).is_err(), "Materials have no equipped entry");
    }

    /// A refund record lands where the client's refund screen reads it: type 2 at +0x98, the item at
    /// +0xA0, the expiry at +0xC0, the costs from +0x08, with the count in front.
    #[test]
    fn a_refund_record_has_the_screens_layout() {
        let r = RefundRecord { vendor: 6, sale: 23, item: 840, instance: 0xD100_0000_0000_0601,
                               costs: vec![(869, 570)], bought: 0xB11, expires: 0xB11 + 1800 };
        let o = refund_object(&[r.clone()]);
        assert_eq!(o.len(), REFUND_FIELD_BYTES);
        assert_eq!(&o[0..4], &1u32.to_le_bytes());
        let at = 8;
        assert_eq!(o[at], 1, "one cost");
        assert_eq!(&o[at + 8..at + 10], &869u16.to_le_bytes());
        assert_eq!(&o[at + 0x18..at + 0x1C], &570u32.to_le_bytes());
        assert_eq!(o[at + 0x98], 2);
        assert_eq!(&o[at + 0xA0..at + 0xA2], &840u16.to_le_bytes());
        assert_eq!(&o[at + 0xA8..at + 0xB0], &0xD100_0000_0000_0601u64.to_le_bytes());
        assert_eq!(&o[at + 0xC0..at + 0xC8], &(0xB11u64 + 1800).to_le_bytes());
        let many: Vec<_> = (0..35).map(|i| RefundRecord { sale: i, ..r.clone() }).collect();
        let o = refund_object(&many);
        assert_eq!(&o[0..4], &30u32.to_le_bytes(), "only the newest 30");
        assert_eq!(&o[8 + 0xBC..8 + 0xC0], &5u32.to_le_bytes(), "the oldest kept is the sixth");
    }

    #[test]
    fn an_unknown_sale_is_refused() {
        let mut s = st(25_000);
        assert!(apply(&mut s, 6, 99).is_err());
    }
}
