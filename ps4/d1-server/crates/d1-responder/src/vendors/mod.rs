// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Vendor services, 1:1 with Year 3 Destiny 1: buying (901), selling and the Vault and Postmaster
//! (903 / 902), refunds (905, Eververse only), stock rotation on the manifest's reset schedule, Xûr's
//! weekend, the Cryptarch, reputation and packages, and the Gunsmith's Armsday.
//!
//! Each service takes a [`crate::store::PlayerStore`] and the [`Market`] (the client's sale table
//! plus the compiled [`crate::tables`]), works on a clone and commits it only on success: a refused
//! request changes nothing. A refusal carries the game's own status (`VendorItemStatus`, from the
//! D1 API definitions) and a reason for the log.
//!
//! What is the game's data and what is a stand-in is said at each rule; the note
//! `notes/ps4-56578-social-and-vendors-2026-11-05.md` lists them together.

pub mod armsday;
pub mod cryptarch;
pub mod purchase;
pub mod refund;
pub mod reputation;
pub mod rotation;
pub mod schedule;
pub mod sell;

use std::collections::HashMap;

use crate::tables::{Tables, VendorRow};
use crate::vendor::Sale;

/// `VendorItemStatus` (DestinyDevs BungieNetPlatform `enums.json`, the D1 API). These are bit flags
/// in the API; a refusal carries one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    NoInventorySpace = 1,
    NoFunds = 2,
    NoProgression = 4,
    NoUnlock = 8,
    NoQuantity = 16,
    OutsidePurchaseWindow = 32,
    NotAvailable = 64,
    UniquenessViolation = 128,
    UnknownError = 256,
    AlreadySelling = 512,
    Unsellable = 1024,
    SellingInhibited = 2048,
    AlreadyOwned = 4096,
}

/// A refused vendor request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub status: Status,
    pub why: String,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.status, self.why)
    }
}

pub fn refuse<T>(status: Status, why: impl Into<String>) -> Result<T, Refusal> {
    Err(Refusal { status, why: why.into() })
}

/// What kind of vendor a client vendor row is, for the services that treat some specially.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The Vault is a vendor in the manifest (hash 892630493, "Take"), whose accepted buckets map each
    /// character bucket to a vault bucket.
    Vault,
    /// The Postmaster (hashes 2021251983 and 2762206170, "Collect"): accepts Messages, Special Orders
    /// and Lost Items.
    Postmaster,
    Xur,
    Eververse,
    Cryptarch,
    BountyTracker,
    Gunsmith,
    /// A vendor whose sales are a package's contents (inhibitBuying).
    Package,
    Other,
}

/// The sale table and the compiled tables, plus the two vendor rows the sale table cannot name.
///
/// ⚠ The Vault and the Postmaster sell nothing, so `vendor-sales.tsv` has no row for them and their
/// row index in the client's vendor table (tag `0x80A5FFCF`) is not known yet: one of the rows the
/// sale table skips (1, 4, 8, 18–20, 26, 36, 58, 96, 97). `D1_VAULT_VENDOR` / `D1_POSTMASTER_VENDOR`
/// name them until `d1-oracle vendor-sales` lists every row with its hash.
pub struct Market<'a> {
    pub t: &'a Tables,
    pub sales: &'a HashMap<(u8, u16), Sale>,
    pub vault_vendor: Option<u8>,
    pub postmaster_vendor: Option<u8>,
}

impl<'a> Market<'a> {
    pub fn new(t: &'a Tables, sales: &'a HashMap<(u8, u16), Sale>) -> Self {
        let env = |k: &str| std::env::var(k).ok().and_then(|v| v.parse().ok());
        Market { t, sales, vault_vendor: env("D1_VAULT_VENDOR"), postmaster_vendor: env("D1_POSTMASTER_VENDOR") }
    }

    pub fn vendor(&self, index: u8) -> Option<&'a VendorRow> {
        self.t.vendors.get(&index)
    }

    pub fn kind(&self, index: u8) -> Kind {
        if Some(index) == self.vault_vendor {
            return Kind::Vault;
        }
        if Some(index) == self.postmaster_vendor {
            return Kind::Postmaster;
        }
        let Some(v) = self.vendor(index) else { return Kind::Other };
        match v.hash {
            892630493 => Kind::Vault,
            2021251983 | 2762206170 => Kind::Postmaster,
            2796397637 => Kind::Xur,
            3917130357 | 3599585337 => Kind::Eververse,
            1527174714 => Kind::BountyTracker,
            570929315 => Kind::Gunsmith,
            _ if v.name == "Cryptarch" => Kind::Cryptarch,
            _ if v.inhibit_buying => Kind::Package,
            _ => Kind::Other,
        }
    }
}

/// When a request happens: the server's clock (Unix seconds, for resets) and the client's stamp
/// (the u64 a 901 or 905 carries, which refund expiry is compared with).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clock {
    pub unix: i64,
    pub stamp: u64,
}

impl Clock {
    pub fn minutes(&self) -> i64 {
        self.unix.div_euclid(60)
    }
}

/// splitmix64: the stand-in randomness for selections the original servers made (stock rotation,
/// engram outcomes, package contents). Seeded, so a test or a replay gives the same result.
pub fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// `q` distinct picks from `0..n`, seeded (partial Fisher-Yates).
pub fn pick(seed: u64, n: usize, q: usize) -> Vec<usize> {
    let mut v: Vec<usize> = (0..n).collect();
    let mut s = seed;
    for i in 0..q.min(n) {
        s = mix(s);
        let j = i + (s % (n - i) as u64) as usize;
        v.swap(i, j);
    }
    v.truncate(q.min(n));
    v
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use crate::store::tests::{memory, CHR};
    use crate::store::{give, Item, MemoryStore, Owner};

    pub fn market() -> Market<'static> {
        Market { t: crate::tables::tables(), sales: crate::tables::fixture_sales(), vault_vendor: Some(1), postmaster_vendor: Some(4) }
    }

    /// A Titan with Glimmer, Silver, Legendary Marks and some materials.
    pub fn rich() -> MemoryStore {
        let mut s = memory();
        give(&mut s, Owner::Account, 0x1f, Item::stack(869, 25_000), 0).unwrap();
        give(&mut s, Owner::Account, 0x22, Item::stack(873, 2_000), 0).unwrap();
        give(&mut s, Owner::Account, 0x23, Item::stack(4548, 200), 0).unwrap();
        give(&mut s, Owner::Character(CHR), 0x05, Item::stack(872, 50), 0).unwrap();
        s
    }

    /// Wednesday 4 November 2026 12:00 UTC: after the weekly reset and Armsday, before Xûr's Friday.
    pub const WEDNESDAY: i64 = 1_793_793_600;
    /// Saturday 7 November 2026 12:00 UTC: Xûr is in the Tower.
    pub const SATURDAY: i64 = WEDNESDAY + 3 * 86_400;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_are_distinct_and_seeded() {
        let a = pick(7, 20, 5);
        assert_eq!(a.len(), 5);
        let mut b = a.clone();
        b.sort();
        b.dedup();
        assert_eq!(b.len(), 5);
        assert_eq!(a, pick(7, 20, 5));
        assert_ne!(a, pick(8, 20, 5));
        assert_eq!(pick(1, 3, 9).len(), 3);
    }

    #[test]
    fn the_special_vendors_are_recognised() {
        let m = testing::market();
        assert_eq!(m.kind(5), Kind::Xur);
        assert_eq!(m.kind(12), Kind::Eververse);
        assert_eq!(m.kind(7), Kind::BountyTracker);
        assert_eq!(m.kind(25), Kind::Gunsmith);
        assert_eq!(m.kind(17), Kind::Cryptarch);
        assert_eq!(m.kind(52), Kind::Package);
        assert_eq!(m.kind(6), Kind::Other);
        assert_eq!(m.kind(1), Kind::Vault);
        assert_eq!(m.kind(4), Kind::Postmaster);
    }
}
