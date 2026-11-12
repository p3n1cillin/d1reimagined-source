// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The persistent player store: what a Guardian owns and has done, kept across activities and
//! responder restarts, as Bungie's world server kept character data (in memory, persisted, with
//! an immediate save on important events; GDC 2021, "Exploring Services Architecture at Bungie").
//!
//! **Off unless `D1_STORE=1`.** Without it nothing here runs and every object is read from the
//! fixture knobs at each sign-on, as before. With it, the store lives in `D1_STORE_DIR` (outside the
//! repository; default [`default_dir`]):
//!
//! * `identities.json`: player name -> the account, character and profile SOIDs ([`Identity`]),
//!   allocated once and never reused, so a player keeps one identity across sessions (and stream H's
//!   multiplayer sessions name the same one);
//! * `account-<soid:016x>.json`: one account and its characters (schema [`SCHEMA_VERSION`],
//!   documented in notes/ps4-56578-player-store-2026-11-05.md):
//!
//! * the served objects, byte for byte: the profile (`0x80801743` payload), the account, the
//!   character, the family-4/3 item elements and the roster cards;
//! * the decoded profile the server edits (purchases, equips, dismantles, completions), with a
//!   summary read from it: XP and level, equipped items, inventory slots, the current activity,
//!   completion flags;
//! * the account's inventory slots (currencies), the unlock flags served on 503/205, the vendor
//!   state (next item instance, refund records, the last reset seen per vendor);
//! * activity completions, checkpoints, reputation progress, and open `attributes` maps on the
//!   account, each character and each item for later services (the D1-era attribute rows).
//!
//! **Seeding**: the first sign-on with no record reads exactly the files the knobs name
//! (`D1_PROFILE_FILE`, `D1_ACCOUNT_FILE`, `D1_CHARACTER_FILE`, `D1_INVENTORY`, `D1_ROSTER_DIR`,
//! `D1_UNLOCK_FLAGS`, `D1_VENDOR_ACCOUNT_SLOTS`) through the same readers the serving code uses, so
//! an empty store serves what the fixtures serve. From then on the store is the source.
//!
//! **Saving** is Bungie's world-server pattern: the record is held in memory, important events save
//! at once ([`PlayerStore::save_now`]: vendor purchases (901), equips (403), dismantles (402), loot
//! grants, delivered profile and account pushes, completions, unlock-flag changes, checkpoints, the
//! control commands of `D1_STORE_CONTROL`), and everything else marks the record dirty
//! ([`PlayerStore::mark_dirty`]) for the periodic save, every `D1_STORE_SAVE_SECS` (300, the five
//! minutes of the GDC talk). A save writes `<file>.tmp`, flushes it to disk, keeps the old file as
//! `.prev` and renames the new one over it, so a crash leaves either the old or the new record,
//! never half of one; an unreadable record (a disk fault) falls back to `.prev` and is kept aside.
//!
//! **The trait** other streams use is [`PlayerStore`] (vendors and inventory, rewards, Light,
//! Grimoire, checkpoints, identity); [`with_store`] lends the responder's store through it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::progression::{self, layout, Curves, RefundPolicies, Rewards, VendorResets, XpGrant};

/// Bump on any change a version-1 reader would misread, and add the step to [`migrate`].
pub const SCHEMA_VERSION: u32 = 1;
const HISTORY: usize = 200;

// ── the record ──────────────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Record {
    pub schema: u32,
    #[serde(with = "hex64")]
    pub account_soid: u64,
    /// Incremented by every save.
    pub revision: u64,
    /// The knobs and values the record was seeded from.
    #[serde(default)]
    pub seeded_from: BTreeMap<String, String>,
    #[serde(default)]
    pub account: Account,
    /// Keyed by character SOID (`0x…`).
    #[serde(default)]
    pub characters: BTreeMap<String, Character>,
    /// The newest changes, oldest first (at most 200).
    #[serde(default)]
    pub history: Vec<Change>,
    /// The player name `identities.json` gives this account under (empty for records made before it).
    #[serde(default)]
    pub player: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Account {
    /// The account object as served (`D1_ACCOUNT_FILE` form; pushes replace it).
    #[serde(default, with = "b64::opt")]
    pub payload: Option<Vec<u8>>,
    /// Account inventory slots: slot -> (content index, quantity). Glimmer, Silver, Marks.
    #[serde(default)]
    pub slots: BTreeMap<u16, (u16, u32)>,
    /// The item instance an account slot holds (the vault: entry `+8`), slot -> instance SOID.
    /// Stackables have none. Kept beside `slots` so a record written before it reads the same.
    #[serde(default)]
    pub slot_instances: BTreeMap<u16, u64>,
    /// The unlock-flag indices served on 503 and 205 (None: the responder's default set).
    #[serde(default)]
    pub unlock_flags: Option<Vec<u16>>,
    /// The next item instance a purchase mints.
    #[serde(default, with = "hex64::opt")]
    pub next_item_soid: Option<u64>,
    #[serde(default)]
    pub refunds: Vec<Refund>,
    /// Vendor hash -> the latest reset seen (epoch minutes; progression::VendorSchedule).
    #[serde(default)]
    pub vendor_resets: BTreeMap<u32, i64>,
    /// Grimoire cards acquired: card id -> the score it gives (the account-wide Grimoire score is the sum).
    #[serde(default)]
    pub grimoire: BTreeMap<u32, u32>,
    #[serde(default)]
    pub attributes: BTreeMap<String, Value>,
    /// `D1_ACCOUNT_USER_DATA`: the client's account user data as it last saved it with investment
    /// message 701 (the 1,088-byte image of `0x80801716`, account field [5]), served back at the next
    /// sign-in. Opaque: the server reads none of it.
    #[serde(default, with = "b64::opt")]
    pub account_user_data: Option<Vec<u8>>,
    /// When `account_user_data` last changed (unix seconds).
    #[serde(default)]
    pub account_user_data_at: u64,
}

/// A vendor refund record (account field [20]; vendor.rs builds the object from these).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Refund {
    pub vendor: u8,
    pub sale: u16,
    pub item: u16,
    #[serde(with = "hex64")]
    pub instance: u64,
    pub costs: Vec<(u16, u32)>,
    pub bought: u64,
    pub expires: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Character {
    #[serde(with = "hex64")]
    pub profile_soid: u64,
    /// The profile payload the next sign-on serves.
    #[serde(default, with = "b64::opt")]
    pub profile: Option<Vec<u8>>,
    /// Where `profile` came from: `seed:<knob>`, `vendor`, `push`, `loot`, `control`.
    #[serde(default)]
    pub profile_source: String,
    /// `profile` decoded (29,560 bytes), when known: what edits start from.
    #[serde(default, with = "b64::opt")]
    pub profile_object: Option<Vec<u8>>,
    /// The family-3 character payload (`D1_CHARACTER_FILE`).
    #[serde(default, with = "b64::opt")]
    pub character: Option<Vec<u8>>,
    /// Family-4/3 item elements, in serving order.
    #[serde(default)]
    pub items: Vec<Item>,
    /// Roster cards (`f<family>t<type>-<key>.bin`), sorted by name.
    #[serde(default)]
    pub roster: Vec<Card>,
    #[serde(default)]
    pub summary: Option<Summary>,
    /// The activity in progress and the XP when it started.
    #[serde(default)]
    pub current: Option<Current>,
    #[serde(default)]
    pub completions: BTreeMap<u16, Completion>,
    /// The revision each activity last started at (field [6] named it, not completed): a
    /// completion at or after it is this run's (run fg2j: one run's completion is never applied
    /// twice).
    #[serde(default)]
    pub started: BTreeMap<u16, u64>,
    #[serde(default)]
    pub checkpoints: BTreeMap<u16, Checkpoint>,
    /// Weekly loot lockouts: activity index -> encounter id -> the reset week its loot was last paid
    /// (`checkpoints::reset_week` in the activity host). An encounter is locked while that week is
    /// the current one.
    #[serde(default)]
    pub lockouts: BTreeMap<u16, BTreeMap<String, u64>>,
    /// Reputation totals by progression name, including legacy store-only grants.
    #[serde(default)]
    pub progress: BTreeMap<String, u64>,
    /// Grants awaiting a decoded profile. These are deltas, so a seeded profile's existing
    /// reputation is preserved when its codec finishes decoding. Absent in older records.
    #[serde(default)]
    pub reputation_pending: BTreeMap<String, u64>,
    /// The character's Light as the rewards stream last computed it (the client derives the shown
    /// value from the equipped gear; this is the server's copy for matchmaking and gates).
    #[serde(default)]
    pub light: Option<u32>,
    /// Bounties held, by item hash.
    #[serde(default)]
    pub bounties: BTreeMap<u32, Bounty>,
    /// `profile_object` was edited after `profile` was encoded: the payload must be re-encoded
    /// before it is served (the responder does it at sign-on and on [`push_profile`]).
    #[serde(default)]
    pub profile_stale: bool,
    /// ⭐ Stream O2: the activity the story arc hands the character to next (`handoff`): the
    /// cinematic 165's is the Tower, 0. The completion that leads there is in the profile.
    #[serde(default)]
    pub next_activity: Option<HandOff>,
    #[serde(default)]
    pub attributes: BTreeMap<String, Value>,
}

/// A hand-off the activity host reported: the next activity, and the one that led to it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct HandOff {
    pub activity: u16,
    #[serde(default)]
    pub hash: Option<u32>,
    /// The completed activity's index, when its hash is in the activity table.
    #[serde(default)]
    pub from: Option<u16>,
    pub revision: u64,
}

/// A bounty a character holds: its objectives' progress, in the definition's order.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct Bounty {
    pub item_hash: u32,
    #[serde(default)]
    pub objectives: Vec<u32>,
    #[serde(default)]
    pub completed: bool,
    /// Unix seconds.
    #[serde(default)]
    pub acquired: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Item {
    #[serde(with = "hex64")]
    pub soid: u64,
    #[serde(with = "b64")]
    pub element: Vec<u8>,
    #[serde(default)]
    pub attributes: BTreeMap<String, Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Card {
    pub name: String,
    #[serde(with = "b64")]
    pub payload: Vec<u8>,
}

/// Read from the profile object; never written back.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Summary {
    /// character_level + character_prestige progress.
    pub xp: u64,
    pub level: u32,
    pub prestige: u32,
    /// Runtime index -> progress.
    pub progressions: BTreeMap<u16, u32>,
    /// Equipped-array index -> instance.
    pub equipped: BTreeMap<u8, String>,
    /// (slot, content index, instance, quantity) of every occupied profile slot.
    pub inventory: Vec<(u16, u16, String, u32)>,
    pub activity: Option<(u16, bool)>,
    pub completion_subs: BTreeMap<u16, u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Current {
    pub activity: u16,
    pub xp_at_start: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct Completion {
    pub count: u32,
    pub xp_after: u64,
    pub revision: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct Checkpoint {
    pub step: String,
    pub revision: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub revision: u64,
    /// Unix seconds.
    pub at: u64,
    pub what: String,
}

/// Read a record of any known schema and bring it to [`SCHEMA_VERSION`]. A record newer than this
/// build, or with no schema, is refused (and left untouched on disk).
pub fn migrate(v: Value) -> anyhow::Result<Record> {
    let schema = v.get("schema").and_then(Value::as_u64)
        .ok_or_else(|| anyhow::anyhow!("the record has no schema version"))?;
    match schema as u32 {
        SCHEMA_VERSION => Ok(serde_json::from_value(v)?),
        s if s > SCHEMA_VERSION => anyhow::bail!("schema {s} is newer than this responder's {SCHEMA_VERSION}"),
        s => anyhow::bail!("schema {s} is unknown (this responder reads {SCHEMA_VERSION})"),
    }
}

fn soid_key(soid: u64) -> String {
    format!("{soid:#018x}")
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// ── crash-safe files ────────────────────────────────────────────────────────────────────────

/// Write `bytes` to `path` so that a crash at any point leaves the old file or the new one: write
/// `<path>.tmp` and flush it to the disk, copy the old file to `<path>.prev`, rename the new one
/// over it (an atomic replace on NTFS and POSIX file systems).
pub fn atomic_write(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    use std::io::Write;
    let tmp = sidecar(path, "tmp");
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    if path.exists() {
        let _ = std::fs::copy(path, sidecar(path, "prev"));
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// `<path>.<ext>` (`account-….json.tmp`).
pub fn sidecar(path: &Path, ext: &str) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".");
    s.push(ext);
    PathBuf::from(s)
}

/// Read a JSON file written by [`atomic_write`]. A file that does not parse (a disk fault, or a
/// hand edit gone wrong) is moved aside to `<path>.corrupt-<unix>` and `<path>.prev` is read
/// instead; the second value says so. A leftover `.tmp` (a crash before the rename) is ignored.
pub fn read_json_recovering(path: &Path) -> anyhow::Result<(Value, Option<String>)> {
    let bytes = std::fs::read(path)?;
    match serde_json::from_slice::<Value>(&bytes) {
        Ok(v) => Ok((v, None)),
        Err(e) => {
            let prev = sidecar(path, "prev");
            let v: Value = serde_json::from_slice(&std::fs::read(&prev)
                .map_err(|p| anyhow::anyhow!("{}: {e}, and no {}: {p}", path.display(), prev.display()))?)
                .map_err(|p| anyhow::anyhow!("{}: {e}, and {} too: {p}", path.display(), prev.display()))?;
            let aside = sidecar(path, &format!("corrupt-{}", unix_now()));
            std::fs::rename(path, &aside)?;
            Ok((v, Some(format!("{} did not parse ({e}); moved to {} and read {}", path.display(), aside.display(), prev.display()))))
        }
    }
}

// ── identity ────────────────────────────────────────────────────────────────────────────────

/// A player's SOIDs. The first player's are the responder's knobs (`D1_ACCOUNT_SOID`,
/// `D1_CHARACTER_SOID`, `D1_PROFILE_SOID`), so turning the store on changes no served id; later
/// players get [`Identity::allocated`] ones.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    #[serde(with = "hex64")]
    pub account: u64,
    #[serde(with = "hex64")]
    pub character: u64,
    #[serde(with = "hex64")]
    pub profile: u64,
}

impl Identity {
    /// The responder's own knobs (and their defaults).
    pub fn from_env() -> Self {
        Identity {
            account: crate::env_soid_raw("D1_ACCOUNT_SOID", 0xD100_0000_0000_0001),
            character: crate::env_soid_raw("D1_CHARACTER_SOID", 0xD100_0000_0000_0101),
            profile: crate::env_soid_raw("D1_PROFILE_SOID", 0xD100_0000_0000_0201),
        }
    }

    /// Whether `soid`, the `+0x18` of a client's 40-byte member identity (or a host's key), names
    /// this player: its character SOID, or its **profile** SOID. Run fg2g (6 November): the
    /// client's `join_request` identity carried `0xD100_0000_0000_0201`, the profile SOID of the
    /// player whose character is `...101`, so the grant and the hand-off for the responder's own
    /// player were refused as a stranger's. The account is matched separately, as before.
    pub fn named_by(&self, soid: u64) -> bool {
        soid == self.character || soid == self.profile
    }

    /// Player `n`'s (n >= 1) identity: `0xD101_0000_nnnn_n001` / `...101` / `...201`, outside the
    /// `0xD100_...` range the knobs, items and purchases use.
    pub fn allocated(n: u32) -> Self {
        let base = 0xD101_0000_0000_0000u64 | (u64::from(n) << 12);
        Identity { account: base | 0x001, character: base | 0x101, profile: base | 0x201 }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct Identities {
    pub schema: u32,
    /// How many identities have been allocated (the n-th gets [`Identity::allocated`]`(n)`).
    pub allocated: u32,
    pub players: BTreeMap<String, Identity>,
}

/// `identities.json` in `dir`: the player's identity, allocated (and saved) on first sight. The
/// first player ever seen gets `first`; ids are never reused.
pub fn identity_for(dir: &Path, player: &str, first: Identity) -> anyhow::Result<Identity> {
    anyhow::ensure!(!player.is_empty(), "an empty player name");
    std::fs::create_dir_all(dir)?;
    let path = dir.join("identities.json");
    let mut ids: Identities = if path.exists() {
        let (v, recovered) = read_json_recovering(&path)?;
        if let Some(r) = recovered {
            tracing::warn!("      ⛔ player store: {r}");
        }
        serde_json::from_value(v)?
    } else {
        Identities { schema: 1, ..Default::default() }
    };
    if let Some(id) = ids.players.get(player) {
        return Ok(*id);
    }
    let id = if ids.allocated == 0 { first } else { Identity::allocated(ids.allocated) };
    anyhow::ensure!(!ids.players.values().any(|o| o.account == id.account), "identity {:#x} is already taken", id.account);
    ids.allocated += 1;
    ids.players.insert(player.to_string(), id);
    atomic_write(&path, &serde_json::to_vec_pretty(&ids)?)?;
    Ok(id)
}

// ── seeding: the readers the serving code uses ──────────────────────────────────────────────

/// The fixture knobs a record is seeded from.
#[derive(Debug, Clone, Default)]
pub struct Sources {
    pub profile_file: Option<String>,
    pub account_file: Option<String>,
    pub character_file: Option<String>,
    pub inventory: Option<String>,
    pub item_soid_base: u64,
    pub roster_dir: Option<String>,
    pub unlock_flags: Option<String>,
    pub vendor_account_slots: Option<String>,
}

impl Sources {
    pub fn from_env() -> Self {
        let var = |k: &str| std::env::var(k).ok();
        Sources {
            profile_file: var("D1_PROFILE_FILE"),
            account_file: var("D1_ACCOUNT_FILE"),
            character_file: var("D1_CHARACTER_FILE"),
            inventory: var("D1_INVENTORY"),
            item_soid_base: crate::env_soid("D1_ITEM_SOID_BASE", 0xD100_0000_0000_0301),
            roster_dir: var("D1_ROSTER_DIR").filter(|d| !d.is_empty()),
            unlock_flags: var("D1_UNLOCK_FLAGS").filter(|v| !v.is_empty()),
            vendor_account_slots: var("D1_VENDOR_ACCOUNT_SLOTS"),
        }
    }
}

/// `D1_INVENTORY` (a directory of `*.bin`, sorted, or one file): `(key, element, file name)`, keyed
/// `base + position` (a file that can't be read keeps its position).
pub fn read_inventory(spec: &str, base: u64) -> Vec<(u64, Vec<u8>, String)> {
    let path = Path::new(spec);
    let files: Vec<PathBuf> = if path.is_dir() {
        let mut v: Vec<PathBuf> = std::fs::read_dir(path).map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().map(|e| e == "bin").unwrap_or(false)).collect()).unwrap_or_default();
        v.sort();
        v
    } else if !spec.is_empty() {
        vec![path.to_path_buf()]
    } else {
        Vec::new()
    };
    files.iter().enumerate().filter_map(|(i, f)| match std::fs::read(f) {
        Ok(raw) => Some((base + i as u64, raw, f.file_name().unwrap_or_default().to_string_lossy().into_owned())),
        Err(e) => {
            tracing::warn!("      ⛔ D1_INVENTORY {}: {e}", f.display());
            None
        }
    }).collect()
}

/// `D1_ROSTER_DIR`: every `*.bin`, sorted by name (filename order is send order).
pub fn read_roster_dir(dir: &str) -> Option<Vec<(String, Vec<u8>)>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir).map_err(|e| tracing::warn!("      ⛔ D1_ROSTER_DIR {dir}: {e}")).ok()?
        .flatten().map(|e| e.path()).filter(|p| p.extension().map(|x| x == "bin").unwrap_or(false)).collect();
    files.sort();
    Some(files.iter().filter_map(|f| match std::fs::read(f) {
        Ok(b) => Some((f.file_name()?.to_string_lossy().into_owned(), b)),
        Err(e) => {
            tracing::warn!("      ⛔ D1_ROSTER_DIR {}: {e}", f.display());
            None
        }
    }).collect())
}

/// `D1_UNLOCK_FLAGS`, parsed as the 503/205 builder parses it.
pub fn parse_unlock_flags(spec: &str) -> Option<Vec<u16>> {
    spec.split(',').map(|s| s.trim().parse::<u16>().ok()).collect()
}

/// `D1_VENDOR_ACCOUNT_SLOTS` (`slot:content:quantity,…`), parsed as vendor.rs parses it.
pub fn parse_account_slots(spec: &str) -> BTreeMap<u16, (u16, u32)> {
    let mut out = BTreeMap::new();
    for s in spec.split(',').filter(|s| !s.is_empty()) {
        let p: Vec<u32> = s.split(':').filter_map(|x| x.parse().ok()).collect();
        if p.len() == 3 {
            out.insert(p[0] as u16, (p[1] as u16, p[2]));
        }
    }
    out
}

fn seed_character(src: &Sources, profile_soid: u64, seeded: &mut BTreeMap<String, String>) -> Character {
    let read = |k: &str, p: &Option<String>, seeded: &mut BTreeMap<String, String>| {
        let p = p.as_ref()?;
        seeded.insert(k.into(), p.clone());
        std::fs::read(p).ok()
    };
    let profile = read("D1_PROFILE_FILE", &src.profile_file, seeded);
    let character = read("D1_CHARACTER_FILE", &src.character_file, seeded);
    let items = src.inventory.as_ref().map(|spec| {
        seeded.insert("D1_INVENTORY".into(), spec.clone());
        read_inventory(spec, src.item_soid_base).into_iter()
            .map(|(soid, element, _)| Item { soid, element, attributes: BTreeMap::new() }).collect()
    }).unwrap_or_default();
    let roster = src.roster_dir.as_ref().and_then(|d| {
        seeded.insert("D1_ROSTER_DIR".into(), d.clone());
        read_roster_dir(d)
    }).unwrap_or_default().into_iter().map(|(name, payload)| Card { name, payload }).collect();
    Character {
        profile_soid,
        profile_source: if profile.is_some() { "seed:D1_PROFILE_FILE".into() } else { "seed:none".into() },
        profile,
        character,
        items,
        roster,
        ..Default::default()
    }
}

/// A new record from the fixture knobs: what the store serves first is what they serve.
pub fn seed(src: &Sources, account: u64, character: u64, profile_soid: u64) -> Record {
    let mut seeded = BTreeMap::new();
    let chr = seed_character(src, profile_soid, &mut seeded);
    let mut acct = Account::default();
    if let Some(p) = &src.account_file {
        seeded.insert("D1_ACCOUNT_FILE".into(), p.clone());
        acct.payload = std::fs::read(p).ok();
    }
    if let Some(spec) = &src.unlock_flags {
        seeded.insert("D1_UNLOCK_FLAGS".into(), spec.clone());
        acct.unlock_flags = parse_unlock_flags(spec);
    }
    if let Some(spec) = &src.vendor_account_slots {
        seeded.insert("D1_VENDOR_ACCOUNT_SLOTS".into(), spec.clone());
        acct.slots = parse_account_slots(spec);
    }
    Record {
        schema: SCHEMA_VERSION,
        account_soid: account,
        revision: 0,
        seeded_from: seeded,
        account: acct,
        characters: BTreeMap::from([(soid_key(character), chr)]),
        history: Vec::new(),
        player: String::new(),
    }
}

// ── the store ───────────────────────────────────────────────────────────────────────────────

pub struct Store {
    /// None: in memory only ([`Store::in_memory`], for tests and for streams without a disk).
    path: Option<PathBuf>,
    pub record: Record,
    character: u64,
    pub curves: Curves,
    pub rewards: Rewards,
    pub refund_policies: RefundPolicies,
    /// Changes since the last save that wait for the periodic one.
    dirty: Vec<String>,
}

/// What a vendor change (901, 403, 402) leaves: vendor.rs's state after it, encoded.
pub struct VendorCommit {
    pub profile_payload: Vec<u8>,
    pub profile_object: Vec<u8>,
    pub account_payload: Vec<u8>,
    pub slots: BTreeMap<u16, (u16, u32)>,
    /// The vault's instances (slot -> instance), written with `slots`.
    pub slot_instances: BTreeMap<u16, u64>,
    pub next_item_soid: u64,
    pub refunds: Vec<Refund>,
    pub added: Option<(u64, Vec<u8>)>,
    pub removed: Option<u64>,
    pub what: String,
}

impl Store {
    pub fn file_for(dir: &Path, account: u64) -> PathBuf {
        dir.join(format!("account-{account:016x}.json"))
    }

    /// A store with no file: saves only count revisions. What the other streams' tests can use.
    #[allow(dead_code)] // an entry point for streams I, M, J and H; not called by the responder yet
    pub fn in_memory(id: Identity, src: &Sources) -> Store {
        Store { path: None, record: seed(src, id.account, id.character, id.profile), character: id.character,
                curves: Curves::load(), rewards: Rewards::load(), refund_policies: RefundPolicies::load(), dirty: Vec::new() }
    }

    /// [`Store::open`] for a player named in `identities.json` (allocated on first sight).
    pub fn open_player(dir: &Path, player: &str, first: Identity, src: &Sources) -> anyhow::Result<Store> {
        let id = identity_for(dir, player, first)?;
        let mut s = Store::open(dir, id.account, id.character, id.profile, src)?;
        if s.record.player != player {
            s.record.player = player.to_string();
            s.save(&format!("player {player}"))?;
        }
        Ok(s)
    }

    /// Load the account's record, or seed it from `src` (and save) when there is none. A record
    /// without this character gets the character seeded beside the others.
    pub fn open(dir: &Path, account: u64, character: u64, profile_soid: u64, src: &Sources) -> anyhow::Result<Store> {
        std::fs::create_dir_all(dir)?;
        let path = Self::file_for(dir, account);
        let mut seeded = false;
        let record = if path.exists() {
            let (v, recovered) = read_json_recovering(&path)?;
            if let Some(r) = &recovered {
                tracing::warn!("      ⛔⛔ player store: {r}");
            }
            let r = migrate(v).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
            anyhow::ensure!(r.account_soid == account, "{} holds account {:#x}, not {account:#x}", path.display(), r.account_soid);
            r
        } else {
            seeded = true;
            seed(src, account, character, profile_soid)
        };
        let mut s = Store { path: Some(path), record, character, curves: Curves::load(), rewards: Rewards::load(),
                            refund_policies: RefundPolicies::load(), dirty: Vec::new() };
        if !s.record.characters.contains_key(&soid_key(character)) {
            let mut extra = BTreeMap::new();
            let chr = seed_character(src, profile_soid, &mut extra);
            s.record.characters.insert(soid_key(character), chr);
            s.save(&format!("character {character:#x} seeded from the fixtures"))?;
        } else if seeded {
            s.save("seeded from the fixtures")?;
        }
        Ok(s)
    }

    /// The record's file (None for an in-memory store).
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn chr(&self) -> &Character {
        &self.record.characters[&soid_key(self.character)]
    }

    pub fn chr_mut(&mut self) -> &mut Character {
        self.record.characters.get_mut(&soid_key(self.character)).expect("the character is seeded at open")
    }

    /// Persist now (with whatever was waiting for the periodic save): [`atomic_write`].
    pub fn save(&mut self, what: &str) -> anyhow::Result<()> {
        self.record.revision += 1;
        let what = match self.dirty.len() {
            0 => what.to_string(),
            n => format!("{what} (+{n} pending: {})", self.dirty.join("; ")),
        };
        self.record.history.push(Change { revision: self.record.revision, at: unix_now(), what: what.clone() });
        let excess = self.record.history.len().saturating_sub(HISTORY);
        self.record.history.drain(..excess);
        if let Some(path) = &self.path {
            atomic_write(path, &serde_json::to_vec_pretty(&self.record)?)?;
            tracing::warn!("      💾 player store r{}: {what}", self.record.revision);
        }
        self.dirty.clear();
        Ok(())
    }

    /// A change the periodic save picks up.
    pub fn touch(&mut self, what: &str) {
        self.dirty.push(what.to_string());
    }

    #[allow(dead_code)] // tests, and streams checking before a save
    pub fn is_dirty(&self) -> bool {
        !self.dirty.is_empty()
    }

    /// The periodic save: saves only when something is waiting.
    pub fn flush(&mut self) -> anyhow::Result<bool> {
        if self.dirty.is_empty() {
            return Ok(false);
        }
        self.save("periodic save")?;
        Ok(true)
    }

    /// A profile payload the client now has (or will on the next echo). A new payload makes the
    /// decoded object unknown until [`Store::adopt_object`] supplies it. Returns whether it changed.
    pub fn set_profile(&mut self, payload: &[u8], source: &str) -> bool {
        let c = self.chr_mut();
        if c.profile.as_deref() == Some(payload) {
            return false;
        }
        c.profile = Some(payload.to_vec());
        c.profile_source = source.to_string();
        c.profile_object = None;
        c.profile_stale = false;
        true
    }

    pub fn put_item(&mut self, soid: u64, element: &[u8]) {
        let items = &mut self.chr_mut().items;
        match items.iter_mut().find(|i| i.soid == soid) {
            Some(i) => i.element = element.to_vec(),
            None => items.push(Item { soid, element: element.to_vec(), attributes: BTreeMap::new() }),
        }
    }

    pub fn remove_item(&mut self, soid: u64) {
        self.chr_mut().items.retain(|i| i.soid != soid);
    }

    /// Replace roster cards by name (a loot grant's peer cards), keeping the order sorted.
    pub fn put_cards(&mut self, cards: Vec<(String, Vec<u8>)>) {
        let roster = &mut self.chr_mut().roster;
        for (name, payload) in cards {
            match roster.iter_mut().find(|c| c.name == name) {
                Some(c) => c.payload = payload,
                None => roster.push(Card { name, payload }),
            }
        }
        roster.sort_by(|a, b| a.name.cmp(&b.name));
    }

    pub fn commit_vendor(&mut self, c: VendorCommit) -> anyhow::Result<Vec<String>> {
        self.set_profile(&c.profile_payload, "vendor");
        let events = self.adopt_object(&c.profile_payload, c.profile_object)?;
        if let Some((soid, element)) = &c.added {
            self.put_item(*soid, element);
        }
        if let Some(soid) = c.removed {
            self.remove_item(soid);
        }
        let a = &mut self.record.account;
        a.payload = Some(c.account_payload);
        a.slots = c.slots;
        // Only the slots that still hold something keep an instance.
        a.slot_instances = c.slot_instances.into_iter().filter(|(slot, _)| a.slots.contains_key(slot)).collect();
        a.next_item_soid = Some(c.next_item_soid);
        a.refunds = c.refunds;
        self.save(&c.what)?;
        Ok(events)
    }

    /// Unlock flags: add and clear (the container holds 50). Without a served list (the
    /// responder's default set) there is nothing to change.
    pub fn change_unlock_flags(&mut self, add: &[u16], clear: &[u16]) -> anyhow::Result<String> {
        if add.is_empty() && clear.is_empty() {
            return Ok(String::new());
        }
        let list = self.record.account.unlock_flags.as_mut()
            .ok_or_else(|| anyhow::anyhow!("no served unlock-flag list (D1_UNLOCK_FLAGS) to change"))?;
        let before = list.clone();
        list.retain(|f| !clear.contains(f));
        for f in add {
            if !list.contains(f) {
                list.push(*f);
            }
        }
        if list.len() > 50 {
            *list = before;
            anyhow::bail!("more than 50 unlock flags; unchanged");
        }
        Ok(format!("unlock flags +{add:?} -{clear:?}"))
    }

    /// The decoded image of `payload`: becomes the edit object if `payload` is still the served
    /// profile, refreshes the summary, and reports what the change means: an activity started, or
    /// completed (its reward's unlock flags and reputation then apply). The store's reputation is
    /// then folded into the object ([`Store::sync_reputation`]); if that changed it, the object is
    /// stale and the caller pushes it.
    pub fn adopt_object(&mut self, payload: &[u8], object: Vec<u8>) -> anyhow::Result<Vec<String>> {
        anyhow::ensure!(object.len() == layout::OBJECT_BYTES, "a {}-byte profile object", object.len());
        if self.chr().profile.as_deref() != Some(payload) {
            return Ok(vec!["a decoded profile that is no longer the served one: ignored".into()]);
        }
        let new = summarize(&object, &self.curves);
        let revision = self.record.revision + 1;
        let old = self.chr().summary.clone();
        // The new object first, so a completion's reputation lands in it rather than in the
        // object it replaces.
        let c = self.chr_mut();
        c.profile_object = Some(object);
        c.summary = Some(new.clone());
        let mut events = Vec::new();
        let was = old.as_ref().and_then(|o| o.activity);
        if let Some((activity, completed)) = new.activity {
            if !completed && was != Some((activity, false)) {
                self.chr_mut().current = Some(Current { activity, xp_at_start: new.xp });
                self.chr_mut().started.insert(activity, revision);
                events.push(format!("activity {activity} started at {} XP", new.xp));
            }
            if completed && was != Some((activity, true)) {
                self.chr_mut().current = None;
                events.extend(self.record_completion(activity, new.xp, new.level, revision));
            }
        }
        match self.sync_reputation() {
            Ok(true) => events.push("the store's reputation was folded into the profile (push due)".into()),
            Ok(false) => {}
            Err(e) => events.push(format!("⛔ reputation not folded into the profile: {e}")),
        }
        Ok(events)
    }

    /// One completion of `activity` in the record: its count, its reward row's unlock flags and
    /// reputation. Called when field [6] shows it completed, and for a completion aside
    /// ([`Command::CompleteAside`]), which leaves field [6] alone.
    fn record_completion(&mut self, activity: u16, xp: u64, level: u32, revision: u64) -> Vec<String> {
        let mut events = Vec::new();
        let entry = self.chr_mut().completions.entry(activity).or_default();
        entry.count += 1;
        entry.xp_after = xp;
        entry.revision = revision;
        events.push(format!("activity {activity} completed ({xp} XP, level {level})"));
        if let Some(r) = self.rewards.0.get(&activity).cloned() {
            match self.change_unlock_flags(&r.unlock_add, &r.unlock_clear) {
                Ok(w) if !w.is_empty() => events.push(w),
                Ok(_) => {}
                Err(e) => events.push(format!("⛔ {e}")),
            }
            for (name, amount) in &r.reputation {
                events.push(self.add_reputation(name, *amount));
            }
            if r.status != "sourced" {
                events.push(format!("reward row {activity} is a {}: {}", r.status, r.source));
            }
        }
        events
    }

    /// Whether `activity` was completed in its current run: a completion recorded at or after the
    /// revision it last started at (any completion, when it never started in this record). Run
    /// fg2j: the one-slot field [6] check let the grant complete 165 a second time, with its XP,
    /// once the stand-in's completion of 164 had replaced 165's record.
    pub fn completed_this_run(&self, activity: u16) -> bool {
        let c = self.chr();
        match (c.completions.get(&activity), c.started.get(&activity)) {
            (Some(done), Some(start)) => done.revision >= *start,
            (Some(_), None) => true,
            (None, _) => false,
        }
    }

    /// Fold the store's reputation into the decoded profile through each curve's exported runtime
    /// index (#322's `progression-index.tsv`). The store's total is authoritative: an object that
    /// lacks it (an older payload delivered late, a vendor's profile) gets it back. Pending grants,
    /// made while no object was decoded, are deltas on top of what the object already holds, so a
    /// seeded profile's own reputation is kept. Works on a copy, so a full progression table or an
    /// unrepresentable total changes nothing. Returns whether the object changed (it is then
    /// stale). Without a decoded object nothing happens and pending grants keep waiting.
    pub fn sync_reputation(&mut self) -> anyhow::Result<bool> {
        let Some(mut object) = self.chr().profile_object.clone() else { return Ok(false) };
        if self.chr().progress.is_empty() {
            return Ok(false);
        }
        let entries = progression::progressions(&object);
        let mut applied = Vec::new();
        for (name, saved) in &self.chr().progress {
            let Some(index) = self.curves.by_name(name).and_then(|c| c.runtime_index) else { continue };
            let pending = self.chr().reputation_pending.get(name).copied().unwrap_or(0);
            let native = entries.get(&index).map(|p| u64::from(p.0));
            let total = saved.saturating_sub(pending).max(native.unwrap_or(0)).checked_add(pending)
                .and_then(|t| u32::try_from(t).ok())
                .ok_or_else(|| anyhow::anyhow!("{name}: reputation overflow"))?;
            if native != Some(u64::from(total)) {
                progression::set_progress(&mut object, index, total)?;
            }
            applied.push((name.clone(), total));
        }
        let changed = self.chr().profile_object.as_ref() != Some(&object);
        let summary = summarize(&object, &self.curves);
        let c = self.chr_mut();
        c.profile_object = Some(object);
        c.summary = Some(summary);
        c.profile_stale |= changed;
        for (name, total) in applied {
            c.reputation_pending.remove(&name);
            c.progress.insert(name, u64::from(total));
        }
        Ok(changed)
    }

    /// Reputation into the profile through the curve's exported runtime index; the object is then
    /// stale and [`push_profile`] delivers it. Without a decoded object the grant waits as a
    /// pending delta until one is adopted. A curve without a runtime index stays in the store
    /// only, and says so.
    pub fn add_reputation(&mut self, name: &str, amount: u32) -> String {
        let Some(curve) = self.curves.by_name(name).cloned() else {
            return format!("⛔ no progression curve named {name}");
        };
        if let Err(e) = self.sync_reputation() {
            return format!("⛔ {e}; reputation unchanged");
        }
        let native = curve.runtime_index.and_then(|index| self.chr().profile_object.as_ref()
            .and_then(|o| progression::progressions(o).get(&index).map(|p| u64::from(p.0))))
            .unwrap_or(0);
        let existing = native.max(self.chr().progress.get(name).copied().unwrap_or(0));
        let Some(total) = existing.checked_add(u64::from(amount)).filter(|n| *n <= u64::from(u32::MAX)) else {
            return format!("⛔ {name}: reputation overflow; unchanged");
        };
        let suffix = match (curve.runtime_index, self.chr().profile_object.clone()) {
            (Some(index), Some(mut object)) => {
                if let Err(e) = progression::set_progress(&mut object, index, total as u32) {
                    return format!("⛔ {e}; reputation unchanged");
                }
                let summary = summarize(&object, &self.curves);
                let c = self.chr_mut();
                c.profile_object = Some(object);
                c.summary = Some(summary);
                c.profile_stale = true;
                format!("; profile progression {index} (push due)")
            }
            (Some(index), None) => {
                *self.chr_mut().reputation_pending.entry(name.to_string()).or_default() += u64::from(amount);
                format!("; progression {index} waits for the decoded profile")
            }
            (None, _) => "; kept in the store only (no runtime index)".to_string(),
        };
        self.chr_mut().progress.insert(name.to_string(), total);
        let message = format!("{name} +{amount} = {total} (rank {} -> {}){suffix}", curve.level(existing), curve.level(total));
        self.touch(&message);
        message
    }

    /// Note every vendor reset since the last look (epoch minutes `now`).
    pub fn roll_vendor_resets(&mut self, schedules: &VendorResets, now: i64) -> Vec<String> {
        let mut out = Vec::new();
        for v in &schedules.0 {
            let last = v.last_reset(now);
            match self.record.account.vendor_resets.insert(v.vendor_hash, last) {
                Some(prev) if prev < last => out.push(format!("{} ({}) reset since r{}; next at epoch minute {}",
                                                               v.name, v.vendor_hash, self.record.revision, v.next_reset(now))),
                _ => {}
            }
        }
        out
    }

}

// ── the trait the other streams plug into ───────────────────────────────────────────────────

/// The first item instance a purchase mints when the store has none recorded (vendor.rs's too).
pub const FIRST_ITEM_SOID: u64 = 0xD100_0000_0000_0601;

/// Glimmer's 56578 content index (the account's slot 308 holds it).
pub const GLIMMER_CONTENT: u16 = 869;
/// ⭐ D1's Glimmer cap: 25,000 (the manifest's `maxStackSize`, `rewards::currencies::cap`; the
/// live Tower capture's wallet stopped there, notes/d1-live-captures-patrol-tower-2026-11-06.md).
/// Glimmer beyond it is lost, as in the game.
pub const GLIMMER_CAP: u32 = 25_000;

/// `after` for `content` once its cap holds: a gain stops at the cap (a wallet already above it,
/// from an older record, is not taken down), a spend is never changed.
pub fn capped(content: u16, have: u32, after: i64) -> i64 {
    if content == GLIMMER_CONTENT && after > i64::from(have) {
        after.min(i64::from(GLIMMER_CAP.max(have)))
    } else {
        after
    }
}

/// The persistent player store as the other streams see it: one account and the character signed
/// on. [`Store`] implements it on disk ([`Store::open`]) or in memory ([`Store::in_memory`]);
/// [`with_store`] lends the responder's. Mutators change the in-memory record only: an important
/// event then calls [`PlayerStore::save_now`], anything else [`PlayerStore::mark_dirty`] (the
/// periodic save). Values the client reads from the profile object (XP, completions) are edited
/// there and flagged for re-encoding ([`Character::profile_stale`]).
///
/// Streams: I (vendors, inventory, bounties), M (rewards, Light, Grimoire), J (checkpoints),
/// H (identity). notes/ps4-56578-player-store-2026-11-05.md has the table.
#[allow(dead_code)] // an entry point for streams I, M, J and H; not called by the responder yet
pub trait PlayerStore {
    // ── identity (H) ──
    fn identity(&self) -> Identity;
    /// The player name the identity is filed under (empty for an unnamed store).
    fn player(&self) -> &str;

    // ── persistence ──
    /// Save now (an important event); whatever waited for the periodic save goes with it.
    fn save_now(&mut self, what: &str) -> anyhow::Result<()>;
    /// Note a change for the periodic save.
    fn mark_dirty(&mut self, what: &str);

    // ── inventory and vendors (I) ──
    fn items(&self) -> &[Item];
    /// Add an item instance, or replace its element.
    fn put_item(&mut self, soid: u64, element: &[u8]);
    /// Whether it was there.
    fn remove_item(&mut self, soid: u64) -> bool;
    /// A fresh item instance SOID (never handed out before on this account).
    fn mint_item_soid(&mut self) -> u64;
    /// The quantity of a currency or material held in the account's slots (by content index).
    fn currency(&self, content: u16) -> u32;
    /// Add `delta` (negative: spend). Refuses to go below zero, or to add a content no slot holds.
    fn adjust_currency(&mut self, content: u16, delta: i64) -> anyhow::Result<u32>;
    fn refunds(&self) -> &[Refund];
    fn push_refund(&mut self, r: Refund);
    /// Take back the refund record for `instance` if it hasn't expired at `now` (the 901's clock).
    fn take_refund(&mut self, instance: u64, now: u64) -> Option<Refund>;
    /// How long a purchase from `vendor_hash` can be returned, in seconds (None: never). From the
    /// manifest: Eververse and Silver Dust, 15 minutes.
    fn refund_window(&self, vendor_hash: u32, item_hash: Option<u32>) -> Option<u64>;
    fn bounties(&self) -> &BTreeMap<u32, Bounty>;
    fn put_bounty(&mut self, b: Bounty);
    fn remove_bounty(&mut self, item_hash: u32) -> Option<Bounty>;
    /// Record the resets since the last look (epoch minutes); returns those that happened.
    fn roll_vendor_resets(&mut self, schedules: &VendorResets, now: i64) -> Vec<String>;

    // ── rewards, Light, Grimoire (M) ──
    /// character_level + character_prestige progress (None until the profile is decoded).
    fn xp(&self) -> Option<u64>;
    /// XP through the curves into the profile object (level 40 = 346,000, then prestige).
    fn add_xp(&mut self, amount: u32) -> anyhow::Result<XpGrant>;
    /// Reputation (rank progress) by progression name; returns what changed.
    fn add_reputation(&mut self, progression: &str, amount: u32) -> String;
    /// `(progress, rank)` for a progression the store keeps.
    fn reputation(&self, progression: &str) -> Option<(u64, u32)>;
    fn light(&self) -> Option<u32>;
    fn set_light(&mut self, light: Option<u32>);
    /// The account's Grimoire score (the sum of its cards' scores).
    fn grimoire_score(&self) -> u32;
    /// Whether the card is new.
    fn add_grimoire_card(&mut self, card: u32, score: u32) -> bool;
    fn completions(&self) -> &BTreeMap<u16, Completion>;
    fn unlock_flags(&self) -> Option<&[u16]>;
    fn change_unlock_flags(&mut self, add: &[u16], clear: &[u16]) -> anyhow::Result<String>;

    // ── checkpoints (J) ──
    fn checkpoint(&self, activity: u16) -> Option<&Checkpoint>;
    fn set_checkpoint(&mut self, activity: u16, step: &str);
    fn clear_checkpoint(&mut self, activity: u16) -> bool;
    /// The reset week `encounter`'s loot was last paid in `activity` (None: never).
    fn lockout_week(&self, activity: u16, encounter: &str) -> Option<u64>;
    /// Record that `encounter`'s loot was paid in reset week `week`.
    fn set_lockout(&mut self, activity: u16, encounter: &str, week: u64);

    // ── open attributes (any stream): the character's `attributes` map ──
    fn character_attribute(&self, key: &str) -> Option<&Value>;
    fn set_character_attribute(&mut self, key: &str, value: Value);

    // ── stream O2: delivery into the client's buckets ──
    /// Add `delta` (negative: take) of the stack item `content` in client bucket `bucket` (an
    /// ordinal of `vendor::BUCKETS`): an account bucket's slots (`account.slots`), or a character
    /// bucket's profile slots (the decoded profile object, which is then stale). The slot already
    /// holding `content` first, else the bucket's first free slot. Returns the new quantity.
    fn deliver_stack(&mut self, content: u16, bucket: u8, delta: i64) -> anyhow::Result<u32>;
    /// Put a new instance `soid` of `content` into a free slot of `bucket` and keep its element.
    /// Returns the slot.
    fn deliver_instance(&mut self, content: u16, bucket: u8, soid: u64, element: &[u8]) -> anyhow::Result<u16>;
}

impl PlayerStore for Store {
    fn identity(&self) -> Identity {
        Identity { account: self.record.account_soid, character: self.character, profile: self.chr().profile_soid }
    }

    fn player(&self) -> &str {
        &self.record.player
    }

    fn save_now(&mut self, what: &str) -> anyhow::Result<()> {
        self.save(what)
    }

    fn mark_dirty(&mut self, what: &str) {
        self.touch(what)
    }

    fn items(&self) -> &[Item] {
        &self.chr().items
    }

    fn put_item(&mut self, soid: u64, element: &[u8]) {
        Store::put_item(self, soid, element)
    }

    fn remove_item(&mut self, soid: u64) -> bool {
        let before = self.chr().items.len();
        Store::remove_item(self, soid);
        self.chr().items.len() != before
    }

    fn mint_item_soid(&mut self) -> u64 {
        let next = self.record.account.next_item_soid.unwrap_or(FIRST_ITEM_SOID);
        self.record.account.next_item_soid = Some(next + 1);
        next
    }

    fn currency(&self, content: u16) -> u32 {
        // The account's slots, plus the character's profile stacks (materials, Vanguard and
        // Crucible Marks, Silver Dust: character buckets) when the profile object is decoded.
        let account: u32 = self.record.account.slots.values().filter(|(c, _)| *c == content).map(|(_, q)| *q).sum();
        let profile: u32 = self.chr().profile_object.as_deref().map_or(0, |p| {
            (0..PROFILE_SLOT_COUNT).filter(|s| profile_slot_fits(p, *s)).map(|s| crate::vendor::profile_entry(p, s))
                .filter(|(c, _)| *c == content).map(|(_, q)| q).sum()
        });
        account.saturating_add(profile)
    }

    fn adjust_currency(&mut self, content: u16, delta: i64) -> anyhow::Result<u32> {
        let slot = self.record.account.slots.iter_mut().find(|(_, (c, _))| *c == content)
            .ok_or_else(|| anyhow::anyhow!("no account slot holds content {content}"))?;
        let after = capped(content, slot.1 .1, i64::from(slot.1 .1) + delta);
        anyhow::ensure!((0..=i64::from(u32::MAX)).contains(&after), "content {content}: {} {delta:+} is out of range", slot.1 .1);
        slot.1 .1 = after as u32;
        Ok(after as u32)
    }

    fn refunds(&self) -> &[Refund] {
        &self.record.account.refunds
    }

    fn push_refund(&mut self, r: Refund) {
        self.record.account.refunds.push(r)
    }

    fn take_refund(&mut self, instance: u64, now: u64) -> Option<Refund> {
        let refunds = &mut self.record.account.refunds;
        let i = refunds.iter().position(|r| r.instance == instance && now < r.expires)?;
        Some(refunds.remove(i))
    }

    fn refund_window(&self, vendor_hash: u32, item_hash: Option<u32>) -> Option<u64> {
        self.refund_policies.window_secs(vendor_hash, item_hash)
    }

    fn bounties(&self) -> &BTreeMap<u32, Bounty> {
        &self.chr().bounties
    }

    fn put_bounty(&mut self, b: Bounty) {
        self.chr_mut().bounties.insert(b.item_hash, b);
    }

    fn remove_bounty(&mut self, item_hash: u32) -> Option<Bounty> {
        self.chr_mut().bounties.remove(&item_hash)
    }

    fn roll_vendor_resets(&mut self, schedules: &VendorResets, now: i64) -> Vec<String> {
        Store::roll_vendor_resets(self, schedules, now)
    }

    fn xp(&self) -> Option<u64> {
        self.chr().profile_object.as_ref().map(|o| progression::total_xp(o, &self.curves))
    }

    fn add_xp(&mut self, amount: u32) -> anyhow::Result<XpGrant> {
        let curves = self.curves.clone();
        // On a copy: a refused prestige entry (a full table) must not leave the level part
        // applied, or a caller's retry (bridge_progressions re-queues) would pay it twice.
        let mut obj = self.chr().profile_object.clone().ok_or_else(|| anyhow::anyhow!("the profile is not decoded yet"))?;
        let g = progression::add_xp(&mut obj, &curves, amount)?;
        let summary = summarize(&obj, &curves);
        let c = self.chr_mut();
        c.profile_object = Some(obj);
        c.profile_stale = true;
        c.summary = Some(summary);
        Ok(g)
    }

    fn add_reputation(&mut self, progression: &str, amount: u32) -> String {
        Store::add_reputation(self, progression, amount)
    }

    fn reputation(&self, progression: &str) -> Option<(u64, u32)> {
        let p = *self.chr().progress.get(progression)?;
        Some((p, self.curves.by_name(progression)?.level(p)))
    }

    fn light(&self) -> Option<u32> {
        self.chr().light
    }

    fn set_light(&mut self, light: Option<u32>) {
        self.chr_mut().light = light;
    }

    fn grimoire_score(&self) -> u32 {
        self.record.account.grimoire.values().sum()
    }

    fn add_grimoire_card(&mut self, card: u32, score: u32) -> bool {
        self.record.account.grimoire.insert(card, score).is_none()
    }

    fn completions(&self) -> &BTreeMap<u16, Completion> {
        &self.chr().completions
    }

    fn unlock_flags(&self) -> Option<&[u16]> {
        self.record.account.unlock_flags.as_deref()
    }

    fn change_unlock_flags(&mut self, add: &[u16], clear: &[u16]) -> anyhow::Result<String> {
        Store::change_unlock_flags(self, add, clear)
    }

    fn checkpoint(&self, activity: u16) -> Option<&Checkpoint> {
        self.chr().checkpoints.get(&activity)
    }

    fn set_checkpoint(&mut self, activity: u16, step: &str) {
        let revision = self.record.revision + 1;
        self.chr_mut().checkpoints.insert(activity, Checkpoint { step: step.to_string(), revision });
    }

    fn clear_checkpoint(&mut self, activity: u16) -> bool {
        self.chr_mut().checkpoints.remove(&activity).is_some()
    }

    fn lockout_week(&self, activity: u16, encounter: &str) -> Option<u64> {
        self.chr().lockouts.get(&activity)?.get(encounter).copied()
    }

    fn set_lockout(&mut self, activity: u16, encounter: &str, week: u64) {
        self.chr_mut().lockouts.entry(activity).or_default().insert(encounter.to_string(), week);
    }

    fn character_attribute(&self, key: &str) -> Option<&Value> {
        self.chr().attributes.get(key)
    }

    fn set_character_attribute(&mut self, key: &str, value: Value) {
        self.chr_mut().attributes.insert(key.to_string(), value);
    }

    fn deliver_stack(&mut self, content: u16, bucket: u8, delta: i64) -> anyhow::Result<u32> {
        let (first, n, scope) = crate::vendor::bucket(bucket).ok_or_else(|| anyhow::anyhow!("unknown client bucket {bucket:#x}"))?;
        if scope == 1 {
            let slots = &mut self.record.account.slots;
            let slot = (first..first + n).find(|s| slots.get(s).is_some_and(|e| e.0 == content))
                .or_else(|| (delta > 0).then(|| (first..first + n).find(|s| !slots.contains_key(s))).flatten())
                .ok_or_else(|| anyhow::anyhow!("account bucket {bucket:#x}: no slot holds content {content} and none is free"))?;
            let have = slots.get(&slot).map_or(0, |e| e.1);
            let after = capped(content, have, i64::from(have) + delta);
            anyhow::ensure!((0..=i64::from(u32::MAX)).contains(&after), "content {content}: {have} {delta:+} is out of range");
            slots.insert(slot, (content, after as u32));
            return Ok(after as u32);
        }
        let c = self.chr_mut();
        let obj = c.profile_object.as_mut().ok_or_else(|| anyhow::anyhow!("character bucket {bucket:#x}: no decoded profile to put content {content} in"))?;
        let slot = (first..first + n).filter(|s| profile_slot_fits(obj, *s)).find(|s| crate::vendor::profile_entry(obj, *s).0 == content)
            .or_else(|| (delta > 0).then(|| (first..first + n).filter(|s| profile_slot_fits(obj, *s)).find(|s| crate::vendor::profile_entry(obj, *s).0 == 0xFFFF)).flatten())
            .ok_or_else(|| anyhow::anyhow!("character bucket {bucket:#x}: no slot holds content {content} and none is free"))?;
        let have = crate::vendor::profile_entry(obj, slot).1;
        let after = i64::from(if crate::vendor::profile_entry(obj, slot).0 == content { have } else { 0 }) + delta;
        anyhow::ensure!((0..=i64::from(u32::MAX)).contains(&after), "content {content}: {have} {delta:+} is out of range");
        let e = crate::vendor::PROFILE_SLOTS + 32 * slot as usize;
        if after == 0 {
            // An emptied stack frees its slot, as a purchase's price does (vendor::apply).
            obj[e..e + 2].copy_from_slice(&[0xFF, 0xFF]);
        } else {
            obj[e..e + 2].copy_from_slice(&content.to_le_bytes());
        }
        obj[e + 8..e + 16].fill(0);
        obj[e + 0x10..e + 0x14].copy_from_slice(&(after as u32).to_le_bytes());
        c.profile_stale = true;
        Ok(after as u32)
    }

    fn deliver_instance(&mut self, content: u16, bucket: u8, soid: u64, element: &[u8]) -> anyhow::Result<u16> {
        let (first, n, scope) = crate::vendor::bucket(bucket).ok_or_else(|| anyhow::anyhow!("unknown client bucket {bucket:#x}"))?;
        let slot = if scope == 1 {
            let a = &mut self.record.account;
            let slot = (first..first + n).find(|s| !a.slots.contains_key(s))
                .ok_or_else(|| anyhow::anyhow!("account bucket {bucket:#x} is full"))?;
            a.slots.insert(slot, (content, 1));
            a.slot_instances.insert(slot, soid);
            slot
        } else {
            let c = self.chr_mut();
            let obj = c.profile_object.as_mut().ok_or_else(|| anyhow::anyhow!("character bucket {bucket:#x}: no decoded profile to put {soid:#x} in"))?;
            let slot = (first..first + n).filter(|s| profile_slot_fits(obj, *s)).find(|s| crate::vendor::profile_entry(obj, *s).0 == 0xFFFF)
                .ok_or_else(|| anyhow::anyhow!("character bucket {bucket:#x} is full"))?;
            // The entry a purchase writes (vendor::apply): content, instance, quantity 1.
            let e = crate::vendor::PROFILE_SLOTS + 32 * slot as usize;
            obj[e..e + 2].copy_from_slice(&content.to_le_bytes());
            obj[e + 8..e + 16].copy_from_slice(&soid.to_le_bytes());
            obj[e + 0x10..e + 0x14].copy_from_slice(&1u32.to_le_bytes());
            // F49 (run agr31): a weapon that reaches an empty weapon slot is equipped, as a pickup is
            // in the game: the loot cache's special weapon goes straight to the special slot ("Push
            // Triangle to swap to your special weapon"). Delivered unequipped, it did not show.
            if (0x02..=0x04).contains(&bucket) {
                if let Some(index) = crate::vendor::equipped_index(bucket) {
                    let at = crate::progression::layout::EQUIPPED + 8 * index;
                    if obj.len() >= at + 8 && obj[at..at + 8] == [0u8; 8] {
                        obj[at..at + 8].copy_from_slice(&soid.to_le_bytes());
                        tracing::warn!("      💾 rewards: {soid:#x} equipped at index {index} (its bucket {bucket:#x} had none)");
                    }
                }
            }
            c.profile_stale = true;
            slot
        };
        Store::put_item(self, soid, element);
        Ok(slot)
    }
}

/// The profile's inventory slots (`vendor::BUCKETS`' character buckets end below 340).
const PROFILE_SLOT_COUNT: u16 = 340;

fn profile_slot_fits(obj: &[u8], slot: u16) -> bool {
    crate::vendor::PROFILE_SLOTS + 32 * slot as usize + 0x14 <= obj.len()
}

/// The store's view of a profile object.
pub fn summarize(obj: &[u8], curves: &Curves) -> Summary {
    let prog = progression::progressions(obj);
    let at = |name: &str| curves.by_name(name).and_then(|c| Some((c, prog.get(&c.runtime_index?)?.0)));
    let level = at(progression::CHARACTER_LEVEL);
    let prestige = at(progression::CHARACTER_PRESTIGE);
    let u64_at = |at: usize| u64::from_le_bytes(obj[at..at + 8].try_into().unwrap());
    Summary {
        xp: progression::total_xp(obj, curves),
        level: level.map(|(c, x)| c.level(x.into())).unwrap_or(1),
        prestige: prestige.map(|(c, x)| c.level(x.into())).unwrap_or(0),
        progressions: prog.iter().map(|(k, v)| (*k, v.0)).collect(),
        equipped: (0..layout::EQUIPPED_COUNT).filter_map(|i| {
            let s = u64_at(layout::EQUIPPED + 8 * i);
            (s != 0).then(|| (i as u8, soid_key(s)))
        }).collect(),
        inventory: (0..layout::ITEM_SLOT_COUNT).filter_map(|s| {
            let e = layout::ITEM_SLOTS + layout::ITEM_SLOT_STRIDE * s;
            let content = u16::from_le_bytes([obj[e], obj[e + 1]]);
            (content != 0xFFFF).then(|| (s as u16, content, soid_key(u64_at(e + 8)),
                                         u32::from_le_bytes(obj[e + 0x10..e + 0x14].try_into().unwrap())))
        }).collect(),
        activity: progression::activity_record(obj),
        completion_subs: progression::completion_subs(obj),
    }
}

// ── the responder's store ───────────────────────────────────────────────────────────────────

/// The responder's store settings (`D1_STORE*`); None when the store is off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub dir: PathBuf,
    pub player: String,
    pub save_every: std::time::Duration,
    pub control: Option<PathBuf>,
}

/// Where the store lives when `D1_STORE_DIR` is unset: `%LOCALAPPDATA%\d1reimagined\player-store`,
/// else `$HOME/.d1reimagined/player-store` (never the repository).
pub fn default_dir() -> PathBuf {
    match std::env::var_os("LOCALAPPDATA") {
        Some(d) => PathBuf::from(d).join("d1reimagined").join("player-store"),
        None => PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| ".".into())).join(".d1reimagined").join("player-store"),
    }
}

impl Config {
    /// `D1_STORE=1` turns it on; `D1_STORE_DIR`, `D1_STORE_PLAYER` (the identity's name, default
    /// `local`), `D1_STORE_SAVE_SECS` (300) and `D1_STORE_CONTROL` set it up.
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Option<Config> {
        if var("D1_STORE").as_deref().map(str::trim) != Some("1") {
            return None;
        }
        let secs = var("D1_STORE_SAVE_SECS").and_then(|v| v.trim().parse::<u64>().ok()).filter(|&n| n > 0).unwrap_or(300);
        Some(Config {
            dir: var("D1_STORE_DIR").filter(|d| !d.is_empty()).map(PathBuf::from).unwrap_or_else(default_dir),
            player: var("D1_STORE_PLAYER").filter(|p| !p.is_empty()).unwrap_or_else(|| "local".into()),
            save_every: std::time::Duration::from_secs(secs),
            control: var("D1_STORE_CONTROL").filter(|p| !p.is_empty()).map(PathBuf::from),
        })
    }

    pub fn from_env() -> Option<Config> {
        Self::from_vars(|k| std::env::var(k).ok())
    }
}

/// Whether the responder runs with the store (`D1_STORE=1`).
pub fn enabled() -> bool {
    Config::from_env().is_some()
}

fn global() -> Option<&'static Mutex<Store>> {
    static G: OnceLock<Option<Mutex<Store>>> = OnceLock::new();
    G.get_or_init(|| {
        let cfg = Config::from_env()?;
        match Store::open_player(&cfg.dir, &cfg.player, Identity::from_env(), &Sources::from_env()) {
            Ok(mut s) => {
                tracing::warn!("      💾 player store ON: {} (player {}, r{}, schema {}, saved every {:?})",
                               s.path().map(|p| p.display().to_string()).unwrap_or_default(), cfg.player,
                               s.record.revision, s.record.schema, cfg.save_every);
                // ⚠ LAB ONLY: D1_STORE_SEED_COMPLETED (run fg2j). Each listed activity the record
                // has no completion of is completed aside in the profile the client gets at
                // sign-on, so the client launches the next story activity itself (A Guardian Rises
                // done: the reveal, 165, launched for real rather than through a remap).
                for index in seed_completed_from(std::env::var("D1_STORE_SEED_COMPLETED").ok().as_deref()) {
                    if s.chr().completions.contains_key(&index) {
                        continue;
                    }
                    match seed_completion(&mut s, index) {
                        Ok(w) => tracing::warn!("      ⚠ D1_STORE_SEED_COMPLETED (lab only): {w}"),
                        Err(e) => tracing::warn!("      ⛔ D1_STORE_SEED_COMPLETED: activity {index} not completed ({e:#})"),
                    }
                }
                // Reputation granted while the profile was undecoded, or a record from before the
                // profile carried it, is folded in first; then a changed object is re-encoded.
                if let Err(e) = s.sync_reputation() {
                    tracing::warn!("      ⛔ player store: reputation not folded into the profile ({e:#})");
                }
                if s.chr().profile_stale {
                    match reencode(&mut s) {
                        Ok(()) => tracing::warn!("      💾 the stored profile was edited since it was encoded: re-encoded"),
                        Err(e) => tracing::warn!("      ⛔ player store: re-encoding the edited profile failed ({e:#}); serving the last encoding"),
                    }
                }
                let now = (unix_now() / 60) as i64;
                let resets = s.roll_vendor_resets(&VendorResets::load(), now);
                for r in &resets {
                    tracing::warn!("      💾 vendor reset: {r}");
                }
                if let Err(e) = s.save(&format!("signed on; {} vendor reset(s) since the last session", resets.len())) {
                    tracing::warn!("      ⛔ player store: {e}");
                }
                if let Some(path) = cfg.control.clone() {
                    start_control(path);
                }
                start_periodic_save(cfg.save_every);
                Some(Mutex::new(s))
            }
            Err(e) => {
                tracing::warn!("      ⛔⛔ player store {} NOT USED ({e:#}): serving the fixtures; the record is left as it is", cfg.dir.display());
                None
            }
        }
    }).as_ref()
}

/// `D1_STORE_SEED_COMPLETED=164[,…]`: the activity indices to seed completed (lab only).
pub fn seed_completed_from(value: Option<&str>) -> Vec<u16> {
    value.unwrap_or("").split(',').filter_map(|v| v.trim().parse().ok()).collect()
}

/// Complete `index` aside in `s`'s served profile (its subs and XP; field [6] left as seeded),
/// re-encoded and adopted, so the next sign-on serves it, and the record notes the completion.
fn seed_completion(s: &mut Store, index: u16) -> anyhow::Result<String> {
    let oracle = crate::vendor::oracle_path();
    let mut obj = match (s.chr().profile_object.clone(), s.chr().profile.clone()) {
        (Some(o), _) => o,
        (None, Some(p)) => crate::vendor::decode_object(&oracle, "0x80801743", &p, "store-seed-in")?,
        (None, None) => anyhow::bail!("no served profile to edit"),
    };
    let what = apply_seed_completion(s, &mut obj, index)?;
    let (encoded, _) = crate::vendor::encode(&oracle, "0x80801743", &obj, &crate::vendor::unique_name("store-seed-out"))?;
    s.set_profile(&encoded, "seed-completed");
    for e in s.adopt_object(&encoded, obj)? {
        tracing::warn!("      💾 {e}");
    }
    s.save(&format!("seed: {what}"))?;
    Ok(what)
}

/// The edit and the record of [`seed_completion`], without the codec: `obj` gains the completion,
/// the record notes it.
pub(crate) fn apply_seed_completion(s: &mut Store, obj: &mut [u8], index: u16) -> anyhow::Result<String> {
    let what = edit_profile(obj, &Command::CompleteAside(index), s)?;
    let summary = summarize(obj, &s.curves);
    let revision = s.record.revision + 1;
    for e in s.record_completion(index, summary.xp, summary.level, revision) {
        tracing::warn!("      💾 {e}");
    }
    Ok(format!("activity {index} seeded completed: {what}"))
}

// ── several players (stream O3) ─────────────────────────────────────────────────────────────

/// The other players' stores, by account: opened on first use from `identities.json`'s identity
/// (integration review, finding 4). The responder's own player is [`global`]'s.
fn other_stores() -> &'static Mutex<BTreeMap<u64, &'static Mutex<Store>>> {
    static O: OnceLock<Mutex<BTreeMap<u64, &'static Mutex<Store>>>> = OnceLock::new();
    O.get_or_init(Default::default)
}

fn others() -> Vec<&'static Mutex<Store>> {
    other_stores().lock().map(|m| m.values().copied().collect()).unwrap_or_default()
}

/// The player `identities.json` in `dir` files under exactly this identity (account, character
/// and profile), if any.
pub fn filed_player(dir: &Path, id: &Identity) -> Option<String> {
    let (v, _) = read_json_recovering(&dir.join("identities.json")).ok()?;
    let ids: Identities = serde_json::from_value(v).ok()?;
    ids.players.into_iter().find(|(_, i)| i == id).map(|(p, _)| p)
}

/// The player filed with this account and character (a grant's identity names both), if any.
pub fn filed_identity(dir: &Path, account: u64, character: u64) -> Option<Identity> {
    let (v, _) = read_json_recovering(&dir.join("identities.json")).ok()?;
    let ids: Identities = serde_json::from_value(v).ok()?;
    ids.players.into_values().find(|i| i.account == account && i.named_by(character))
}

/// The filed player whose character (or profile, [`Identity::named_by`]) is `character` (a grant
/// that names only a key), if exactly one.
pub fn filed_character(dir: &Path, character: u64) -> Option<Identity> {
    let (v, _) = read_json_recovering(&dir.join("identities.json")).ok()?;
    let ids: Identities = serde_json::from_value(v).ok()?;
    let mut found = ids.players.into_values().filter(|i| i.named_by(character));
    let first = found.next()?;
    found.next().is_none().then_some(first)
}

/// The store of the player with identity `id`: the responder's own ([`global`]) for its identity,
/// else that player's record in `dir`, opened (and seeded from the fixtures on first sight) once.
/// None when the store is off, or no player in `identities.json` has exactly this identity: a
/// record is only ever opened for a filed player, so a grant matched by identity can only pay a
/// real one.
pub fn store_of(id: &Identity) -> Option<&'static Mutex<Store>> {
    let primary = global()?;
    if primary.lock().ok().is_some_and(|s| s.identity() == *id) {
        return Some(primary);
    }
    let cfg = Config::from_env()?;
    open_other(&cfg.dir, id)
}

/// [`store_of`] for another player, in `dir`.
fn open_other(dir: &Path, id: &Identity) -> Option<&'static Mutex<Store>> {
    let mut map = other_stores().lock().ok()?;
    if let Some(m) = map.get(&id.account) {
        return m.lock().ok().is_some_and(|s| s.identity() == *id).then_some(*m);
    }
    let Some(player) = filed_player(dir, id) else {
        tracing::warn!("      ⛔ player store: no player in identities.json is {:#x}/{:#x}/{:#x}", id.account, id.character, id.profile);
        return None;
    };
    match Store::open(dir, id.account, id.character, id.profile, &Sources::from_env()) {
        Ok(mut s) => {
            if s.record.player != player {
                s.record.player = player.clone();
                let _ = s.save(&format!("player {player}"));
            }
            tracing::warn!("      💾 player store: '{player}' ({:#x}) opened beside the responder's own player (D1_MP=1)", id.account);
            let m: &'static Mutex<Store> = Box::leak(Box::new(Mutex::new(s)));
            map.insert(id.account, m);
            Some(m)
        }
        Err(e) => {
            tracing::warn!("      ⛔ player store: '{player}' NOT opened ({e:#})");
            None
        }
    }
}

/// The store the current work is for (`players::current`): the connection's or thread's player,
/// else the responder's own.
pub fn current_store() -> Option<&'static Mutex<Store>> {
    match crate::players::current() {
        Some(id) if crate::players::current_is_other() => store_of(&id),
        _ => global(),
    }
}

/// A player's own copy of a push file: the knob's path for the responder's own player (`None`),
/// else `<stem>-<account>.<ext>` beside it, which only that player's primary connection delivers.
pub fn player_file(base: &Path, account: Option<u64>) -> PathBuf {
    let Some(account) = account else { return base.to_path_buf() };
    let stem = base.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let name = match base.extension() {
        Some(e) => format!("{stem}-{account:016x}.{}", e.to_string_lossy()),
        None => format!("{stem}-{account:016x}"),
    };
    base.with_file_name(name)
}

fn other_account() -> Option<u64> {
    crate::players::current().filter(|_| crate::players::current_is_other()).map(|i| i.account)
}

/// `D1_PROFILE_PUSH_FILE` for the current player ([`player_file`]).
pub fn profile_push_file() -> Option<PathBuf> {
    std::env::var_os("D1_PROFILE_PUSH_FILE").filter(|p| !p.is_empty()).map(|p| player_file(Path::new(&p), other_account()))
}

/// `D1_ACCOUNT_PUSH_FILE` for the current player ([`player_file`]).
pub fn account_push_file() -> Option<PathBuf> {
    std::env::var_os("D1_ACCOUNT_PUSH_FILE").filter(|p| !p.is_empty()).map(|p| player_file(Path::new(&p), other_account()))
}

/// The world server's periodic save: every `every`, save if anything is waiting.
fn start_periodic_save(every: std::time::Duration) {
    std::thread::spawn(move || loop {
        std::thread::sleep(every);
        let Some(m) = global() else { return };
        for m in std::iter::once(m).chain(others()) {
            if let Ok(mut s) = m.lock() {
                if let Err(e) = s.flush() {
                    tracing::warn!("      ⛔ player store periodic save: {e:#}");
                }
            }
        }
    });
}

/// Encode the edited profile object into the served payload (the client's codec).
fn reencode(s: &mut Store) -> anyhow::Result<()> {
    s.sync_reputation()?;
    let obj = s.chr().profile_object.clone().ok_or_else(|| anyhow::anyhow!("no profile object"))?;
    let oracle = crate::vendor::oracle_path();
    let (encoded, _) = crate::vendor::encode(&oracle, "0x80801743", &obj, &crate::vendor::unique_name("store-reencode"))?;
    let c = s.chr_mut();
    c.profile = Some(encoded);
    c.profile_source = "store".into();
    c.profile_stale = false;
    Ok(())
}

/// Lend the responder's store to another stream: None when it is off (`D1_STORE` unset).
#[allow(dead_code)] // an entry point for streams I, M, J and H; not called by the responder yet
pub fn with_store<T>(f: impl FnOnce(&mut dyn PlayerStore) -> T) -> Option<T> {
    let m = current_store()?;
    let mut s = m.lock().ok()?;
    Some(f(&mut *s))
}

/// After a [`PlayerStore`] edit to the profile (XP or reputation): re-encode, save, and push
/// (`D1_PROFILE_PUSH_FILE`) as the control commands do. Nothing to do when it isn't stale.
pub fn push_profile(what: &str) -> anyhow::Result<()> {
    let m = current_store().ok_or_else(|| anyhow::anyhow!("the store is off"))?;
    let encoded = {
        let mut s = m.lock().map_err(|_| anyhow::anyhow!("poisoned"))?;
        let Some(encoded) = prepare_profile_push(&mut s, what)? else { return Ok(()) };
        encoded
    };
    write_push(&encoded)
}

/// Whether the store's profile must be pushed: its reputation folded in, and the object edited
/// since it was encoded. Needs no codec, so tests can drive it.
fn profile_push_due(s: &mut Store) -> anyhow::Result<bool> {
    s.sync_reputation()?;
    Ok(s.chr().profile_stale)
}

fn prepare_profile_push(s: &mut Store, what: &str) -> anyhow::Result<Option<Vec<u8>>> {
    if !profile_push_due(s)? {
        return Ok(None);
    }
    reencode(s)?;
    s.save(what)?;
    Ok(s.chr().profile.clone())
}

/// Write a profile push for the responder to deliver, one at a time.
fn write_push(encoded: &[u8]) -> anyhow::Result<()> {
    write_push_with_item(encoded, None)
}

/// Wait (up to 120 s) until the client took the last profile push (`D1_PROFILE_PUSH_FILE` gone).
pub fn wait_for_push_taken() {
    if let Some(push) = profile_push_file() {
        for _ in 0..120 {
            if !push.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
}

/// [`write_push`] with one new item: its encoded element and SOID beside the push
/// (`.item`, `.soid`), as a purchase writes them (`vendor::publish`).
fn write_push_with_item(encoded: &[u8], item: Option<(u64, &[u8])>) -> anyhow::Result<()> {
    if let Some(push) = profile_push_file() {
        // One push at a time: wait for the previous one to be delivered.
        wait_for_push_taken();
        if let Some((soid, element)) = item {
            std::fs::write(push.with_extension("item"), element)?;
            std::fs::write(push.with_extension("soid"), format!("{soid:#x}"))?;
        }
        std::fs::write(&push, encoded)?;
    }
    Ok(())
}

/// ⭐ Stream O2: push the store's account (its slots, vault instances and refund records) to the
/// client, `D1_ACCOUNT_PUSH_FILE`, encoded as a purchase's account push (`vendor::account_payload`).
/// This is the path for an account change when the vendor state is off (`D1_VENDOR_SALES` unset):
/// with it on, the vendor state publishes instead ([`crate::vendor::adopt_store_state`]).
pub fn push_account(what: &str) -> anyhow::Result<()> {
    let m = current_store().ok_or_else(|| anyhow::anyhow!("the store is off"))?;
    let (slots, instances, refunds) = {
        let s = m.lock().map_err(|_| anyhow::anyhow!("poisoned"))?;
        let a = &s.record.account;
        (a.slots.clone(), a.slot_instances.clone(), a.refunds.clone())
    };
    let payload = crate::vendor::account_payload(&crate::vendor::oracle_path(), &slots, &instances, &refunds)?;
    if let Some(p) = account_push_file() {
        std::fs::write(&p, &payload)?;
    } else {
        tracing::warn!("      ⚠ {what}: D1_ACCOUNT_PUSH_FILE is unset: the account is saved but not pushed");
    }
    let mut s = m.lock().map_err(|_| anyhow::anyhow!("poisoned"))?;
    s.record.account.payload = Some(payload);
    s.save(&format!("{what}: account pushed"))
}

/// `D1_ACCOUNT_USER_DATA`: keep the client's account user data (investment 701) in the current
/// player's record, replacing any earlier one, and save when it changed. True when the record holds
/// it; false when the store is off or the save failed (the record is put back, the failure logged
/// with the account id), and the caller keeps it in memory instead.
pub fn set_account_user_data(image: &[u8]) -> bool {
    let Some(m) = current_store() else { return false };
    let Ok(mut s) = m.lock() else { return false };
    set_account_user_data_in(&mut s, image)
}

pub(super) fn set_account_user_data_in(s: &mut Store, image: &[u8]) -> bool {
    if s.record.account.account_user_data.as_deref() == Some(image) {
        return true;
    }
    let old = (s.record.account.account_user_data.take(), s.record.account.account_user_data_at);
    s.record.account.account_user_data = Some(image.to_vec());
    s.record.account.account_user_data_at = unix_now();
    match s.save(&format!("account user data ({} B)", image.len())) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!("      ⛔ player store: account user data of {:#x} not saved: {e:#}", s.record.account_soid);
            (s.record.account.account_user_data, s.record.account.account_user_data_at) = old;
            false
        }
    }
}

/// The account user data the current player's record keeps (None: the store is off, or none yet).
pub fn account_user_data() -> Option<Vec<u8>> {
    view(|s| s.record.account.account_user_data.clone()).flatten()
}

/// Read from the store: None when it is off (the caller serves the knobs as before).
///
/// Stream O3: the current player's record (`players::current`; the responder's own otherwise).
pub fn view<T>(f: impl FnOnce(&Store) -> T) -> Option<T> {
    current_store().and_then(|m| m.lock().ok().map(|s| f(&s)))
}

fn with(what: &str, f: impl FnOnce(&mut Store) -> anyhow::Result<Vec<String>>) {
    let Some(m) = current_store() else { return };
    let Ok(mut s) = m.lock() else { return };
    match f(&mut s) {
        Ok(events) => events.iter().for_each(|e| tracing::warn!("      💾 {what}: {e}")),
        Err(e) => tracing::warn!("      ⛔ player store ({what}): {e:#}"),
    }
}

/// The served unlock-flag spec: the store's list, or `D1_UNLOCK_FLAGS`.
pub fn unlock_flag_spec() -> Option<String> {
    match view(|s| s.record.account.unlock_flags.clone()) {
        Some(list) => list.map(|l| l.iter().map(u16::to_string).collect::<Vec<_>>().join(",")).filter(|s| !s.is_empty()),
        None => std::env::var("D1_UNLOCK_FLAGS").ok().filter(|v| !v.is_empty()),
    }
}

/// Write a created Guardian's look (`d1_bap::character_lifecycle::CreateCharacterRequest::look_image`)
/// into a decoded peer view or profile object at `LOOK_AT`. Returns false (and leaves it) when the
/// object is too short. Everything past the look stays: the customization's f10 (the profile's
/// last-orbited destination) and f11 (the wear-helmet field).
pub fn write_look(object: &mut [u8], look: &[u8; d1_bap::character_lifecycle::LOOK_BYTES]) -> bool {
    use d1_bap::character_lifecycle::{LOOK_AT, LOOK_BYTES};
    match object.get_mut(LOOK_AT..LOOK_AT + LOOK_BYTES) {
        Some(at) => {
            at.copy_from_slice(look);
            true
        }
        None => false,
    }
}

/// The peer view (`0x808017FD`): the family-0 type-1 roster card.
const PEER_VIEW_TAG: &str = "0x808017FD";

/// The buckets a created Guardian's class kit replaces: helmet, gauntlets, chest, legs, class item
/// and Ghost. Weapons stay out (a new Guardian has none until the Khvostov), and so do the ship,
/// vehicle, emblem, emote and horn the content list also names (their policy is unverified).
const CLASS_KIT_BUCKETS: [u8; 6] = [0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0E];
/// The starting subclass per class, by content index (56578 item index; the archived manifest names
/// them): Titan Striker 710, Hunter Gunslinger 709, Warlock Voidwalker 713.
const STARTING_SUBCLASS: [u16; 3] = [710, 709, 713];
/// The second subclass a created Guardian holds (locked until level 15), per class: the live A
/// Guardian Rises capture's new Warlock was granted Sunsinger 712 beside Voidwalker 713 (bucket 0x06's
/// second slot). Titan Defender 711 and Hunter Bladedancer 708 by the same pairing (inferred; no
/// capture of their creation).
const SECOND_SUBCLASS: [u16; 3] = [711, 708, 712];

/// Put a second subclass into a decoded profile: a copy of bucket 0x06's first held slot (content
/// and instance replaced) in the bucket's first empty slot. Returns the instance it copied from, so
/// its item element can be copied too; None when the bucket has no held slot or no empty one.
pub fn add_second_subclass(profile: &mut [u8], content: u16, instance: u64) -> Option<u64> {
    let info = crate::store::bucket(SUBCLASS_BUCKET).filter(|i| i.scope == 0)?;
    let slot = |s: u16| crate::store::PROFILE_SLOTS + 32 * s as usize;
    let range = info.first..info.first + info.count;
    let held = range.clone().find(|&s| {
        profile.get(slot(s)..slot(s) + 16).is_some_and(|e| u16::from_le_bytes([e[0], e[1]]) != 0xFFFF && e[8..16] != [0; 8])
    })?;
    if range.clone().any(|s| profile.get(slot(s)..slot(s) + 2).is_some_and(|e| u16::from_le_bytes([e[0], e[1]]) == content)) {
        return None;
    }
    let empty = range.clone().find(|&s| profile.get(slot(s)..slot(s) + 2).is_some_and(|e| u16::from_le_bytes([e[0], e[1]]) == 0xFFFF))?;
    let src: [u8; 32] = profile[slot(held)..slot(held) + 32].try_into().ok()?;
    let from = u64::from_le_bytes(src[8..16].try_into().unwrap());
    let dst = &mut profile[slot(empty)..slot(empty) + 32];
    dst.copy_from_slice(&src);
    dst[0..2].copy_from_slice(&content.to_le_bytes());
    dst[8..16].copy_from_slice(&instance.to_le_bytes());
    Some(from)
}
const SUBCLASS_BUCKET: u8 = 0x06;
/// An item element's content index (`0x80801732` `+0x24`, as vendor.rs writes it).
const ITEM_CONTENT: usize = 0x24;

/// `(bucket, content)` a created Guardian of `class` starts with: `D1_STARTING_KIT`'s entries for
/// [`CLASS_KIT_BUCKETS`] and the class's starting subclass. Empty when the knob is unset. The
/// content's first default-gear list (`D1_STARTING_KIT=content`) is the classic starter gear:
/// Lightmail and the Mark of the Risen (Titan), Prototype 0.9 and the Makeshift Scarf (Hunter),
/// and the Generalist Shell.
pub fn class_kit(class: u8) -> Vec<(u8, u16)> {
    let kit = crate::character_create::kit_from_env();
    let Some(items) = kit.get(&class) else { return Vec::new() };
    let mut wants: Vec<(u8, u16)> = items.iter().copied().filter(|(b, _)| CLASS_KIT_BUCKETS.contains(b)).collect();
    if let Some(&sub) = STARTING_SUBCLASS.get(class as usize) {
        wants.push((SUBCLASS_BUCKET, sub));
    }
    wants
}

/// Put each `(bucket, content)` into the first occupied slot of that character bucket in a decoded
/// profile object, keeping its instance. Returns `(instance, content, bucket)` for each changed
/// instanced slot; an empty bucket is left empty (the served set has nothing equipped there).
pub fn apply_kit_to_profile(profile: &mut [u8], wants: &[(u8, u16)]) -> Vec<(u64, u16, u8)> {
    let mut changed = Vec::new();
    for &(b, content) in wants {
        let Some(info) = crate::store::bucket(b).filter(|i| i.scope == 0) else { continue };
        for slot in info.first..info.first + info.count {
            let e = crate::store::PROFILE_SLOTS + 32 * slot as usize;
            let Some(entry) = profile.get_mut(e..e + 16) else { break };
            if u16::from_le_bytes([entry[0], entry[1]]) == 0xFFFF {
                continue;
            }
            entry[0..2].copy_from_slice(&content.to_le_bytes());
            let instance = u64::from_le_bytes(entry[8..16].try_into().unwrap());
            if instance != 0 {
                changed.push((instance, content, b));
            }
            break;
        }
    }
    changed
}

/// ⭐ The profile flags a created Guardian starts with: build 56578's own creation record for its
/// class and race (investment globals `0x80A58001 +0xE8`, nine 176-byte records; exported by
/// tools/ps4_setup/d1-new-guardian-defaults.py, #338), the entries whose master-table storage is
/// profile field [11] (kind 3). All nine give subs 341, 673, 693, 706 and 826 = 2. Its kind-1
/// entries are account flags (D1_UNLOCK_FLAGS), not profile bytes. Empty when the record is absent.
pub fn creation_profile_flags(class: u8, race: u8) -> Vec<(u16, u8)> {
    static DOC: OnceLock<Value> = OnceLock::new();
    let doc = DOC.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../fixtures/56578/characters/new-guardian-defaults.json")).unwrap_or(Value::Null)
    });
    let Some(rows) = doc.get("data").and_then(Value::as_array) else { return Vec::new() };
    let Some(row) = rows.iter().find(|r| r["character_class"].as_u64() == Some(class as u64) && r["race"].as_u64() == Some(race as u64)) else {
        return Vec::new();
    };
    row["flags"].as_array().map(|f| f.iter()
        .filter(|g| g["kind"].as_u64() == Some(3))
        .filter_map(|g| Some((u16::try_from(g["sub"].as_u64()?).ok()?, u8::try_from(g["value"].as_u64()?).ok()?)))
        .collect()).unwrap_or_default()
}

/// `D1_CREATION_FLAGS=content`: a created Guardian's completion record is its creation record's
/// ([`creation_profile_flags`]) instead of the served profile's (a later save's 94 completions,
/// Restoration's among them). Unset: the served record stays.
pub fn creation_flags_from_env(class: u8, race: u8) -> Option<Vec<(u16, u8)>> {
    match std::env::var("D1_CREATION_FLAGS").ok().as_deref() {
        Some("content") => Some(creation_profile_flags(class, race)),
        _ => None,
    }
}

/// Replace a decoded profile's completion record (field [11], `layout::COMPLETIONS`) with exactly
/// `flags`. False (and nothing written) when the object is too short or a sub is out of range.
pub fn write_completion_record(profile: &mut [u8], flags: &[(u16, u8)]) -> bool {
    let Some(record) = profile.get_mut(layout::COMPLETIONS..layout::COMPLETIONS + layout::COMPLETION_BYTES) else { return false };
    if flags.iter().any(|&(sub, _)| sub as usize >= layout::COMPLETION_BYTES) {
        return false;
    }
    record.fill(0);
    for &(sub, value) in flags {
        record[sub as usize] = value;
    }
    true
}

/// Set a decoded item element's content index. False when the object is too short.
pub fn set_item_content(item: &mut [u8], content: u16) -> bool {
    match item.get_mut(ITEM_CONTENT..ITEM_CONTENT + 2) {
        Some(at) => {
            at.copy_from_slice(&content.to_le_bytes());
            true
        }
        None => false,
    }
}

/// ⭐ 501 `login_create_character`: give the store's character the look the player chose. Until
/// 7 November 2026 the served peer cards and profile kept their fixtures' look (an all-zero
/// customization), so a created Guardian came out as the fixture's, whatever was picked (run new2:
/// an Awoken female chosen). Each family-0 peer card and the profile are decoded with the client's
/// codec, the look written (`write_look`), and re-encoded; the profile is pushed. A card or profile
/// that does not decode or encode is left as it was, and the event says so.
///
/// With `D1_STARTING_KIT` set, the class's starting gear ([`class_kit`]) replaces the content of
/// the served set's equipped armour, Ghost and subclass (run new4: a Hunter was served the Titan
/// set's Oryx Mask and Striker), in the profile slots and those instances' item elements.
pub fn apply_created_look(look: [u8; d1_bap::character_lifecycle::LOOK_BYTES]) {
    let who = crate::players::current();
    // Off the 501 reply: the codec falls back to the oracle (seconds) when the native one refuses.
    std::thread::spawn(move || crate::players::run_as(who, || {
        let oracle = crate::vendor::oracle_path();
        let mut events = Vec::new();
        let cards = view(|s| s.chr().roster.iter().filter(|c| c.name.starts_with("f0t1-"))
            .map(|c| (c.name.clone(), c.payload.clone())).collect::<Vec<_>>()).unwrap_or_default();
        let mut new_cards = Vec::new();
        for (name, payload) in cards {
            let step = crate::vendor::decode_object(&oracle, PEER_VIEW_TAG, &payload, "created-look-card").and_then(|mut obj| {
                anyhow::ensure!(write_look(&mut obj, &look), "the decoded card is too short");
                Ok(crate::vendor::encode(&oracle, PEER_VIEW_TAG, &obj, &crate::vendor::unique_name("created-look-card"))?.0)
            });
            match step {
                Ok(encoded) => {
                    events.push(format!("created look written into {name}"));
                    new_cards.push((name, encoded));
                }
                Err(e) => events.push(format!("⛔ created look: {name} kept as it was ({e:#})")),
            }
        }
        let profile = view(|s| (s.chr().profile_object.clone(), s.chr().profile.clone())).unwrap_or((None, None));
        let object = match profile {
            (Some(obj), _) => Some(Ok(obj)),
            (None, Some(payload)) => Some(crate::vendor::decode_object(&oracle, "0x80801743", &payload, "created-look-profile")),
            (None, None) => None,
        };
        // The class's starting gear (D1_STARTING_KIT): its content into the slots the served set
        // already equips, and into those instances' item elements.
        let wants = class_kit(look[2]);
        // With a kit, the class's second subclass too (a new instance, minted now).
        let second = (!wants.is_empty()).then(|| SECOND_SUBCLASS.get(look[2] as usize).copied()).flatten()
            .and_then(|c| current_store().and_then(|m| m.lock().ok().map(|mut s| (c, s.mint_item_soid()))));
        let mut new_items = Vec::new();
        let mut second_from: Option<(u64, u64)> = None;
        let object = object.map(|o| o.map(|mut obj| {
            if !write_look(&mut obj, &look) {
                events.push("⛔ created look: the profile object is too short; look not written".into());
            } else {
                events.push("created look written into the profile".into());
            }
            let swapped = apply_kit_to_profile(&mut obj, &wants);
            let mut swapped = swapped;
            if let Some((content, instance)) = second {
                match add_second_subclass(&mut obj, content, instance) {
                    // Its element: the first subclass's, with this content (below, with the kit's).
                    Some(from) => {
                        events.push(format!("second subclass: content {content} as instance {instance:#x} (from {from:#x})"));
                        swapped.push((instance, content, SUBCLASS_BUCKET | 0x80));
                        second_from = Some((from, instance));
                    }
                    None => events.push(format!("second subclass {content}: no held subclass slot, or no empty one, or already held; not added")),
                }
            }
            // D1_CREATION_FLAGS=content: the creation record's flags, not the served save's.
            if let Some(flags) = creation_flags_from_env(look[2], look[0]) {
                if flags.is_empty() {
                    events.push(format!("⛔ creation flags: no creation record for class {} race {}; the served record stays", look[2], look[0]));
                } else if write_completion_record(&mut obj, &flags) {
                    events.push(format!("creation flags: the completion record is the creation record's {flags:?}"));
                } else {
                    events.push("⛔ creation flags: the profile object is too short; the served record stays".into());
                }
            }
            let items = view(|s| s.chr().items.iter().map(|i| (i.soid, i.element.clone())).collect::<BTreeMap<_, _>>()).unwrap_or_default();
            for (instance, content, bucket) in swapped {
                // The second subclass's element is a copy of the first subclass's.
                let source = match second_from {
                    Some((from, to)) if to == instance => from,
                    _ => instance,
                };
                let bucket = bucket & 0x7F;
                let Some(element) = items.get(&source) else {
                    events.push(format!("⛔ class kit: bucket {bucket:#x}'s instance {instance:#x} has no item element; the profile slot alone changed"));
                    continue;
                };
                let step = crate::vendor::decode_object(&oracle, "0x80801732", element, "class-kit-item").and_then(|mut item| {
                    anyhow::ensure!(set_item_content(&mut item, content), "the decoded item is too short");
                    // A copied element names its own instance at +0: the client refuses a QueueZ
                    // object whose header and payload ids differ (run cf5:
                    // error.investment_server_object_queuez_id_mismatch_in_header_and_rsat_payload).
                    if source != instance {
                        anyhow::ensure!(item.len() >= 8, "the decoded item is too short");
                        item[0..8].copy_from_slice(&instance.to_le_bytes());
                    }
                    Ok(crate::vendor::encode(&oracle, "0x80801732", &item, &crate::vendor::unique_name("class-kit-item"))?.0)
                });
                match step {
                    Ok(encoded) => {
                        events.push(format!("class kit: bucket {bucket:#x} -> content {content} (instance {instance:#x})"));
                        new_items.push((instance, encoded));
                    }
                    Err(e) => events.push(format!("⛔ class kit: item {instance:#x} kept ({e:#})")),
                }
            }
            obj
        }));
        let mut profile_edited = false;
        with("created look", |s| {
            s.put_cards(new_cards);
            for (soid, element) in &new_items {
                s.put_item(*soid, element);
            }
            match object {
                Some(Ok(obj)) => {
                    let c = s.chr_mut();
                    c.profile_object = Some(obj);
                    c.profile_stale = true;
                    profile_edited = true;
                }
                Some(Err(e)) => events.push(format!("⛔ created look: the profile did not decode ({e:#}); kept")),
                None => events.push("created look: no profile to edit".into()),
            }
            s.save("created look")?;
            Ok(std::mem::take(&mut events))
        });
        if profile_edited {
            if let Err(e) = push_profile("created look") {
                tracing::warn!("      ⛔ player store: the created look's profile push failed ({e:#})");
            }
        }
    }));
}

/// The roster cards: the store's, or `D1_ROSTER_DIR`'s.
pub fn roster_cards() -> Option<Vec<(String, Vec<u8>)>> {
    match view(|s| s.chr().roster.iter().map(|c| (c.name.clone(), c.payload.clone())).collect::<Vec<_>>()) {
        Some(cards) => Some(cards),
        None => read_roster_dir(&std::env::var("D1_ROSTER_DIR").ok().filter(|d| !d.is_empty())?),
    }
}

/// Decode `payload` with the client's codec off the caller's task, then adopt it.
fn decode_later(payload: Vec<u8>) {
    let who = crate::players::current();
    std::thread::spawn(move || crate::players::run_as(who, || {
        let oracle = crate::vendor::oracle_path();
        match crate::vendor::decode_object(&oracle, "0x80801743", &payload, "store-profile") {
            Ok(obj) => with("profile", |s| {
                let events = s.adopt_object(&payload, obj)?;
                if events.iter().any(|e| e.contains("completed") || e.contains("unlock")) {
                    s.save("profile decoded: a completion")?;
                } else {
                    s.touch("profile decoded");
                }
                Ok(events)
            }),
            Err(e) => {
                tracing::warn!("      ⛔ player store: the delivered profile did not decode ({e:#}); its summary waits");
                return;
            }
        }
        // The adopted object may lack the store's reputation (a completion's reward row, a grant
        // made while it was undecoded, an older payload): push the folded profile.
        if let Err(e) = push_profile("profile decoded: reputation folded in") {
            tracing::warn!("      ⛔ player store: the reputation push failed ({e:#})");
        }
    }));
}

/// A profile push the client received (`D1_PROFILE_PUSH_FILE`, with its optional item).
pub fn note_profile_delivered(payload: &[u8], item: Option<(u64, &[u8])>) {
    let mut decode = false;
    with("profile push", |s| {
        decode = s.set_profile(payload, "push");
        if let Some((soid, element)) = item {
            s.put_item(soid, element);
        }
        if decode || item.is_some() {
            s.save(&format!("profile push ({} B{})", payload.len(), item.map(|i| format!(", item {:#x}", i.0)).unwrap_or_default()))?;
        }
        Ok(Vec::new())
    });
    if decode {
        decode_later(payload.to_vec());
    }
}

/// An account push the client received (`D1_ACCOUNT_PUSH_FILE`).
pub fn note_account_delivered(payload: &[u8]) {
    with("account push", |s| {
        if s.record.account.payload.as_deref() != Some(payload) {
            s.record.account.payload = Some(payload.to_vec());
            s.save(&format!("account push ({} B)", payload.len()))?;
        }
        Ok(Vec::new())
    });
}

/// A loot grant the client received: the item, the profile after it, and the peer cards pushed
/// with it (`D1_LOOT_GRANT_ROSTER`'s type-1 cards).
pub fn note_loot_grant(soid: u64, element: &[u8], profile: Option<&[u8]>) {
    let mut decode = None;
    with("loot grant", |s| {
        s.put_item(soid, element);
        if let Some(p) = profile {
            if s.set_profile(p, "loot") {
                decode = Some(p.to_vec());
            }
        }
        if let Some(dir) = std::env::var("D1_LOOT_GRANT_ROSTER").ok().filter(|d| !d.is_empty()) {
            let cards: Vec<_> = read_roster_dir(&dir).unwrap_or_default().into_iter()
                .filter(|(n, _)| n.split_once('-').map(|(h, _)| h.ends_with("t1")).unwrap_or(false)).collect();
            s.put_cards(cards);
        }
        s.save(&format!("loot grant {soid:#x}"))?;
        Ok(Vec::new())
    });
    if let Some(p) = decode {
        decode_later(p);
    }
}

/// The pickup's change to a profile object, applied to the character's current one: every byte the
/// grant's profile (`after`) changes from the profile it was built against (`base`) is set in
/// `current`, unless the store has changed that byte itself, which it keeps. Returns the object and
/// how many bytes were applied and kept.
pub fn apply_pickup(current: &[u8], base: &[u8], after: &[u8]) -> (Vec<u8>, usize, usize) {
    let mut out = current.to_vec();
    let (mut applied, mut kept) = (0, 0);
    for i in 0..base.len().min(after.len()).min(current.len()) {
        if base[i] != after[i] {
            if current[i] == base[i] {
                out[i] = after[i];
                applied += 1;
            } else {
                kept += 1;
            }
        }
    }
    (out, applied, kept)
}

/// F43 (run agr20): with the store on, a loot grant's fixture profile replaced the character's
/// profile, from which the store had moved on (the started activity's field [6], pushed as the
/// family-4 delta before it), and the client crashed applying it. The pickup is instead the change
/// between the profile the grant's was built against (`D1_PROFILE_FILE`, the seed) and the grant's
/// profile (the pending record cleared, the item in its slot and bucket, the slot's completion),
/// applied to the store's current profile object and re-encoded. None when the store is off or an
/// object is unavailable: the grant then sends its fixture profile as before.
pub fn loot_grant_profile(grant: &[u8]) -> Option<Vec<u8>> {
    let m = current_store()?;
    let seed_path = std::env::var("D1_PROFILE_FILE").ok().filter(|p| !p.trim().is_empty())?;
    let seed = std::fs::read(&seed_path).ok()?;
    let oracle = crate::vendor::oracle_path();
    let decode = |payload: &[u8], name: &str| {
        crate::vendor::decode_object(&oracle, "0x80801743", payload, &crate::vendor::unique_name(name))
    };
    let (base, after) = match (decode(&seed, "grant-base"), decode(grant, "grant-after")) {
        (Ok(a), Ok(b)) if a.len() == b.len() => (a, b),
        (a, b) => {
            tracing::warn!("      ⛔ player store: the loot grant's profile pair did not decode alike ({:?}, {:?}); its fixture profile goes out",
                           a.err().map(|e| e.to_string()), b.err().map(|e| e.to_string()));
            return None;
        }
    };
    let mut s = m.lock().ok()?;
    let current = match s.chr().profile_object.clone() {
        Some(o) => o,
        None => decode(s.chr().profile.as_deref()?, "grant-current").ok()?,
    };
    if current.len() != base.len() {
        tracing::warn!("      ⛔ player store: the character's profile object is {} bytes, the grant's {}; its fixture profile goes out",
                       current.len(), base.len());
        return None;
    }
    let (obj, applied, kept) = apply_pickup(&current, &base, &after);
    s.chr_mut().profile_object = Some(obj);
    if let Err(e) = reencode(&mut s) {
        tracing::warn!("      ⛔ player store: the picked-up profile did not encode ({e:#}); its fixture profile goes out");
        return None;
    }
    s.chr_mut().profile_source = "loot".into();
    tracing::warn!("      💾 player store: the pickup applied to the character's profile ({applied} bytes; {kept} the store had changed kept) (F43)");
    s.chr().profile.clone()
}

/// Write a vendor change through (vendor.rs, after it wrote the pushes).
pub fn note_vendor(c: VendorCommit) {
    with("vendor", |s| s.commit_vendor(c));
}

/// What vendor.rs starts from. None when the store is off; `profile` None means the knob's object
/// (`D1_VENDOR_PROFILE_OBJ`), as before the store, while the served profile is still the seed.
pub struct VendorSeed {
    pub profile: Option<Vec<u8>>,
    pub slots: BTreeMap<u16, (u16, u32)>,
    pub slot_instances: BTreeMap<u16, u64>,
    pub next_item_soid: Option<u64>,
    pub refunds: Vec<Refund>,
}

pub fn vendor_seed() -> Option<VendorSeed> {
    let (object, payload, source, slots, slot_instances, next, refunds) = view(|s| {
        let c = s.chr();
        (c.profile_object.clone(), c.profile.clone(), c.profile_source.clone(), s.record.account.slots.clone(),
         s.record.account.slot_instances.clone(), s.record.account.next_item_soid, s.record.account.refunds.clone())
    })?;
    let profile = match (object, payload) {
        (Some(o), _) => Some(o),
        (None, _) if source.starts_with("seed") => None,
        (None, Some(p)) => match crate::vendor::decode_object(&crate::vendor::oracle_path(), "0x80801743", &p, "store-vendor-seed") {
            Ok(o) => Some(o),
            Err(e) => {
                tracing::warn!("      ⛔ player store: the stored profile did not decode for the vendor state ({e:#})");
                None
            }
        },
        (None, None) => None,
    };
    Some(VendorSeed { profile, slots, slot_instances, next_item_soid: next, refunds })
}

// ── control commands ────────────────────────────────────────────────────────────────────────

/// One `D1_STORE_CONTROL` command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// The character enters an activity (field [6]).
    Activity(u16),
    /// XP from the server (character_level, then prestige).
    Xp(u32),
    /// The activity completes: field [6], the rewards table's subs and XP.
    Complete(u16),
    /// The activity completes aside: its subs and XP, with field [6] left on the current activity
    /// (the lab's remap stand-in, `D1_STORE_SEED_COMPLETED`).
    CompleteAside(u16),
    /// No current activity: field [6] = `ffff` (the live reset after a cinematic's 8).
    ClearActivity,
    Checkpoint(u16, String),
    /// `checkpoint N -` or `clear-checkpoint N`: a full clear spends the checkpoint.
    ClearCheckpoint(u16),
    /// `lockout N ENCOUNTER week=W`: the encounter's loot was paid in reset week W.
    Lockout(u16, String, u64),
    Reputation(String, u32),
    Unlock { add: Vec<u16>, clear: Vec<u16> },
    /// `grant CHARACTER ACTIVITY_HASH OUTCOME ...` from the activity host (`rewards::remote`).
    Grant(crate::rewards::remote::GrantCommand),
    /// `handoff CHARACTER ACTIVITY_INDEX [from=HASH] [hash=HASH]` (stream O2, `rewards::remote`).
    HandOff(crate::rewards::remote::HandOffCommand),
    /// `completed CHARACTER ACTIVITY_HASH [aside]`: the activity host's scripts completed the
    /// activity (`activity_complete`); `complete` for the player the target names, now (run fg2h).
    /// `aside` (the lab's remap stand-in, run fg2i) keeps field [6] on the served activity.
    Completed(crate::rewards::remote::GrantTarget, u32, bool),
    /// `started CHARACTER ACTIVITY_HASH`: the activity host served the activity at the join; field
    /// [6] names it, completed 0, as the live host's profile push at the join did (run fg2j).
    Started(crate::rewards::remote::GrantTarget, u32),
    /// `ended CHARACTER ACTIVITY_HASH`: a cinematic-only activity ended (its 8); field [6] = `ffff`
    /// if it still names that activity, pushed with the reply to the client's next 104 (run fg2j).
    Ended(crate::rewards::remote::GrantTarget, u32),
}

pub fn parse_command(line: &str) -> anyhow::Result<Command> {
    let f: Vec<&str> = line.split_whitespace().collect();
    let num = |i: usize| -> anyhow::Result<u32> {
        f.get(i).ok_or_else(|| anyhow::anyhow!("{line:?}: missing argument {i}"))?.parse().map_err(|e| anyhow::anyhow!("{line:?}: {e}"))
    };
    Ok(match f.first().copied() {
        Some("activity") => Command::Activity(num(1)? as u16),
        Some("xp") => Command::Xp(num(1)?),
        Some("complete") => Command::Complete(num(1)? as u16),
        Some("checkpoint") if f.get(2..) == Some(&["-"][..]) => Command::ClearCheckpoint(num(1)? as u16),
        Some("checkpoint") => Command::Checkpoint(num(1)? as u16, f.get(2..).map(|r| r.join(" ")).unwrap_or_default()),
        Some("clear-checkpoint") => Command::ClearCheckpoint(num(1)? as u16),
        Some("lockout") => {
            let week = f.get(3).and_then(|w| w.strip_prefix("week=")).and_then(|w| w.parse().ok())
                .filter(|_| f.len() == 4)
                .ok_or_else(|| anyhow::anyhow!("{line:?}: lockout takes N ENCOUNTER week=W"))?;
            Command::Lockout(num(1)? as u16, f[2].to_string(), week)
        }
        Some("grant") => Command::Grant(crate::rewards::remote::parse(&f[1..]).map_err(|e| anyhow::anyhow!("{line:?}: {e:#}"))?),
        Some("handoff") => Command::HandOff(crate::rewards::remote::parse_handoff(&f[1..]).map_err(|e| anyhow::anyhow!("{line:?}: {e:#}"))?),
        Some(word @ ("completed" | "started" | "ended")) => {
            let (who, hash, aside) = match (word, f.as_slice()) {
                (_, [_, who, hash]) => (who, hash, false),
                ("completed", [_, who, hash, "aside"]) => (who, hash, true),
                _ => anyhow::bail!("{line:?}: {word} takes CHARACTER ACTIVITY_HASH{}", if word == "completed" { " [aside]" } else { "" }),
            };
            let target = crate::rewards::remote::parse_target(who).map_err(|e| anyhow::anyhow!("{line:?}: {e:#}"))?;
            let hash = hash.strip_prefix("0x").map_or_else(|| hash.parse::<u32>().ok(), |h| u32::from_str_radix(h, 16).ok())
                .ok_or_else(|| anyhow::anyhow!("{line:?}: {hash:?} is not a 32-bit hash"))?;
            match word {
                "completed" => Command::Completed(target, hash, aside),
                "started" => Command::Started(target, hash),
                _ => Command::Ended(target, hash),
            }
        }
        Some("reputation") => Command::Reputation(f.get(1).ok_or_else(|| anyhow::anyhow!("{line:?}: no progression"))?.to_string(), num(2)?),
        Some("unlock") => {
            let (mut add, mut clear) = (Vec::new(), Vec::new());
            for t in &f[1..] {
                match t.split_at(1) {
                    ("+", n) => add.push(n.parse()?),
                    ("-", n) => clear.push(n.parse()?),
                    _ => anyhow::bail!("{line:?}: unlock takes +N / -N"),
                }
            }
            Command::Unlock { add, clear }
        }
        _ => anyhow::bail!("{line:?}: not a store command (activity, xp, complete, checkpoint, clear-checkpoint, lockout, grant, handoff, completed, started, ended, reputation, unlock)"),
    })
}

/// Apply a profile-editing command to `obj` (no I/O).
pub fn edit_profile(obj: &mut [u8], cmd: &Command, store: &Store) -> anyhow::Result<String> {
    match cmd {
        Command::Activity(a) => progression::start_activity(obj, *a),
        Command::Xp(n) => {
            let g = progression::add_xp(obj, &store.curves, *n)?;
            Ok(format!("+{n} XP: level {} -> {}, prestige {} -> {}", g.level_before, g.level_after, g.prestige_before, g.prestige_after))
        }
        Command::Complete(a) => {
            let start = store.chr().current.filter(|c| c.activity == *a).map(|c| c.xp_at_start);
            let reward = store.rewards.0.get(a);
            if reward.is_none() {
                tracing::warn!("      💾 no rewards-table row for activity {a}: completed with no reward");
            }
            progression::complete_activity(obj, &store.curves, *a, reward, start).map(|r| r.0)
        }
        Command::CompleteAside(a) => progression::complete_aside(obj, &store.curves, *a, store.rewards.0.get(a)).map(|r| r.0),
        Command::ClearActivity => progression::clear_activity(obj),
        _ => anyhow::bail!("{cmd:?} does not edit the profile"),
    }
}

fn run_command(cmd: Command) -> anyhow::Result<String> {
    let m = current_store().ok_or_else(|| anyhow::anyhow!("the store is off"))?;
    match &cmd {
        Command::Checkpoint(a, step) => {
            let mut s = m.lock().map_err(|_| anyhow::anyhow!("poisoned"))?;
            s.set_checkpoint(*a, step);
            s.save(&format!("checkpoint {a} {step}"))?;
            return Ok(format!("checkpoint {a}: {step}"));
        }
        Command::ClearCheckpoint(a) => {
            let mut s = m.lock().map_err(|_| anyhow::anyhow!("poisoned"))?;
            let had = s.clear_checkpoint(*a);
            s.save(&format!("checkpoint {a} cleared"))?;
            return Ok(if had { format!("checkpoint {a} cleared") } else { format!("checkpoint {a}: none to clear") });
        }
        Command::Lockout(a, encounter, week) => {
            let mut s = m.lock().map_err(|_| anyhow::anyhow!("poisoned"))?;
            s.set_lockout(*a, encounter, *week);
            s.save(&format!("lockout {a} {encounter} week={week}"))?;
            return Ok(format!("lockout {a}: {encounter} paid in week {week}"));
        }
        Command::Grant(g) => return grant_for_player(g),
        Command::HandOff(h) => {
            let (m, who) = target_store(&h.target)?;
            return crate::players::run_as(who, || run_handoff(m, h));
        }
        Command::Completed(target, hash, aside) => {
            let (m, who) = target_store(target)?;
            return crate::players::run_as(who, || run_completed(m, *hash, *aside));
        }
        Command::Started(target, hash) => {
            let (m, who) = target_store(target)?;
            let index = activity_index_of(*hash).ok_or_else(|| anyhow::anyhow!("activity {hash:#010x} is not in the activity table"))?;
            return crate::players::run_as(who, || {
                let shown = m.lock().ok().and_then(|s| s.chr().summary.as_ref().and_then(|x| x.activity));
                if shown == Some((index, false)) {
                    return Ok(format!("activity {index} ({hash:#010x}): the profile already shows it started"));
                }
                run_command(Command::Activity(index)).map(|w| format!("activity {index} ({hash:#010x}) served at the join: {w}"))
            });
        }
        Command::Ended(target, hash) => {
            let (m, who) = target_store(target)?;
            let index = activity_index_of(*hash).ok_or_else(|| anyhow::anyhow!("activity {hash:#010x} is not in the activity table"))?;
            return crate::players::run_as(who, || {
                let shown = m.lock().ok().and_then(|s| s.chr().summary.as_ref().and_then(|x| x.activity));
                if shown.map(|s| s.0) != Some(index) {
                    return Ok(format!("activity {index} ended; field [6] names {shown:?}, not it: left"));
                }
                hold_push_for_104();
                run_command(Command::ClearActivity).map(|w| format!("activity {index} ({hash:#010x}) ended: {w}; pushed with the reply to the client's 104"))
            });
        }
        Command::Reputation(name, n) => {
            let (w, push) = {
                let mut s = m.lock().map_err(|_| anyhow::anyhow!("poisoned"))?;
                let w = s.add_reputation(name, *n);
                let push = prepare_profile_push(&mut s, &w)?;
                if push.is_none() {
                    s.save(&w)?;
                }
                (w, push)
            };
            // Into the profile and pushed now; without a decoded profile it waits (the next
            // decoded delivery folds it in and pushes it).
            if let Some(encoded) = push {
                write_push(&encoded)?;
            }
            return Ok(w);
        }
        Command::Unlock { add, clear } => {
            let mut s = m.lock().map_err(|_| anyhow::anyhow!("poisoned"))?;
            let w = s.change_unlock_flags(add, clear)?;
            s.save(&format!("{w} (served from the next sign-on)"))?;
            return Ok(w);
        }
        _ => {}
    }
    let oracle = crate::vendor::oracle_path();
    let (object, payload) = {
        let s = m.lock().map_err(|_| anyhow::anyhow!("poisoned"))?;
        (s.chr().profile_object.clone(), s.chr().profile.clone())
    };
    let mut obj = match (object, &payload) {
        (Some(o), _) => o,
        (None, Some(p)) => crate::vendor::decode_object(&oracle, "0x80801743", p, "store-control-in")?,
        (None, None) => anyhow::bail!("no served profile to edit"),
    };
    let what = {
        let s = m.lock().map_err(|_| anyhow::anyhow!("poisoned"))?;
        edit_profile(&mut obj, &cmd, &s)?
    };
    let (encoded, _) = crate::vendor::encode(&oracle, "0x80801743", &obj, &crate::vendor::unique_name("store-control-out"))?;
    let events = {
        let mut s = m.lock().map_err(|_| anyhow::anyhow!("poisoned"))?;
        s.set_profile(&encoded, "control");
        let events = s.adopt_object(&encoded, obj)?;
        s.save(&format!("{cmd:?}: {what}"))?;
        events
    };
    for e in &events {
        tracing::warn!("      💾 {e}");
    }
    write_push(&encoded)?;
    Ok(what)
}

/// A `grant` from the activity host (`rewards::remote`): stream M's grant on the live store, then
/// the pushes. Needs `D1_REWARDS=1`.
///
/// Stream O2: every item with a 56578 content index and a known bucket is delivered into the
/// client's slots (account buckets in the account's slots, character buckets in the profile
/// object), new instances with their element (`D1_VENDOR_ITEM_TEMPLATE`), and then pushed: through
/// the vendor state when it is on (it serves the account and profile), through the store's own
/// pushes otherwise ([`push_account`], the profile push), so the wallet reaches the client with
/// `D1_VENDOR_SALES` or `D1_SOCIAL` unset.
/// ⭐ Stream O3 (integration review, finding 4): which player's store a command from the activity
/// host is for, matched by the full identity it names.
///
/// * The responder's own player (account and character both its own): its store.
/// * Another player filed in `identities.json` under that account **and** character (an `id:`
///   target), or under that character alone (a `0x` key, when exactly one player has it): that
///   player's store, opened beside the first, with the player it runs as.
/// * Anyone else: with `D1_MP=1` refused (no player of this responder has that identity, so
///   nobody is paid by mistake); with it off, the responder's own player, as before (one player
///   per process: the open world keys characters by the identity's first eight bytes).
pub fn target_store(t: &crate::rewards::remote::GrantTarget) -> anyhow::Result<(&'static Mutex<Store>, Option<Identity>)> {
    let primary = global().ok_or_else(|| anyhow::anyhow!("the store is off"))?;
    let own = primary.lock().map_err(|_| anyhow::anyhow!("poisoned"))?.identity();
    let dir = Config::from_env().map(|c| c.dir);
    target_store_in(t, primary, own, dir.as_deref(), crate::mp::enabled(), &|id| match &dir {
        Some(d) => open_other(d, id),
        None => None,
    })
}

/// [`target_store`] without the environment: `own` is `primary`'s identity, `dir` the store's
/// folder, `open` opens another player's store.
pub(crate) fn target_store_in<'a>(
    t: &crate::rewards::remote::GrantTarget,
    primary: &'a Mutex<Store>,
    own: Identity,
    dir: Option<&Path>,
    strict: bool,
    open: &dyn Fn(&Identity) -> Option<&'a Mutex<Store>>,
) -> anyhow::Result<(&'a Mutex<Store>, Option<Identity>)> {
    use crate::rewards::remote::GrantTarget;
    let (account, character) = match t {
        GrantTarget::Identity(b) => {
            let m = d1_mp::identity::MemberIdentity::parse(b).ok_or_else(|| anyhow::anyhow!("the identity does not parse"))?;
            (Some(m.account.0), m.character.0)
        }
        GrantTarget::Key(k) => (None, *k),
    };
    if own.named_by(character) && account.is_none_or(|a| a == own.account) {
        return Ok((primary, None));
    }
    let other = dir.and_then(|d| match account {
        Some(a) => filed_identity(d, a, character),
        None => filed_character(d, character),
    });
    match other {
        Some(id) if id != own => match open(&id) {
            Some(m) => Ok((m, Some(id))),
            None => anyhow::bail!("player {:#x}'s record could not be opened", id.account),
        },
        _ if strict => anyhow::bail!(
            "no player of this responder is account {} character {character:#018x} (D1_MP=1: refused, so nobody else is paid)",
            account.map(|a| format!("{a:#018x}")).unwrap_or_else(|| "?".into())
        ),
        _ => Ok((primary, None)),
    }
}

/// A `grant` for the player its identity names ([`target_store`]), run as that player, so its
/// pushes go to that player's own push files and connection.
fn grant_for_player(g: &crate::rewards::remote::GrantCommand) -> anyhow::Result<String> {
    let (m, who) = target_store(&g.target)?;
    crate::players::run_as(who, || run_grant(m, g))
}

fn run_grant(m: &Mutex<Store>, g: &crate::rewards::remote::GrantCommand) -> anyhow::Result<String> {
    use crate::rewards::remote;
    anyhow::ensure!(crate::rewards::enabled(), "D1_REWARDS is off: the grant is not applied");
    let class = std::env::var("D1_CHARACTER_CLASS").ok().and_then(|v| v.trim().parse().ok()).unwrap_or(0);
    let template = std::env::var_os("D1_VENDOR_ITEM_TEMPLATE").and_then(|p| std::fs::read(p).ok());
    // The profile object the character buckets are edited in: the store's (the one source of
    // truth for the current profile, integration review finding 3; the vendor state starts each op
    // from it), else the vendor state's while the store has only its seed, else decoded.
    let vendor_on = crate::vendor::active();
    let (have, payload) = {
        let s = m.lock().map_err(|_| anyhow::anyhow!("poisoned"))?;
        (s.chr().profile_object.is_some(), s.chr().profile.clone())
    };
    let mut object = if !have && vendor_on { crate::vendor::current_profile() } else { None };
    if !have && object.is_none() {
        if let Some(p) = payload {
            match crate::vendor::decode_object(&crate::vendor::oracle_path(), "0x80801743", &p, &crate::vendor::unique_name("store-grant-in")) {
                Ok(o) => object = Some(o),
                Err(e) => tracing::warn!("      ⚠ rewards: the served profile did not decode ({e:#}); character-bucket items stay in the overlay"),
            }
        }
    }
    let (summary, wallet, profile, items) = {
        let mut s = m.lock().map_err(|_| anyhow::anyhow!("poisoned"))?;
        if let Some(o) = object {
            s.chr_mut().profile_object = Some(o);
        }
        let before = (s.record.account.slots.clone(), s.record.account.slot_instances.clone());
        let index = remote::ResponderIndex::from_slots(&before.0);
        let summary = remote::execute_with(&mut *s, g, &index, class, crate::mp::enabled(), template.as_deref())?;
        s.save(&format!("rewards: {}", summary.line))?;
        let after = (s.record.account.slots.clone(), s.record.account.slot_instances.clone());
        let wallet = (after != before).then_some(after);
        let profile = s.chr().profile_stale.then(|| s.chr().profile_object.clone()).flatten();
        let items: Vec<(u64, Vec<u8>)> = summary.delivered.iter()
            .filter_map(|id| s.chr().items.iter().find(|i| i.soid == *id).map(|i| (i.soid, i.element.clone())))
            .collect();
        (summary, wallet, profile, items)
    };
    for n in &summary.notes {
        tracing::warn!("      💾 rewards: {n}");
    }
    let what = format!("rewards: {}", summary.line);
    let changed = wallet.is_some() || profile.is_some() || !items.is_empty();
    if changed && vendor_on {
        // The vendor state serves the account and profile: it publishes the new ones (and writes
        // them back to the store through `note_vendor`).
        let had_wallet = wallet.is_some();
        let (slots, instances) = match wallet {
            Some(w) => w,
            None => view(|s| (s.record.account.slots.clone(), s.record.account.slot_instances.clone())).unwrap_or_default(),
        };
        if !crate::vendor::adopt_store_state(slots, instances, profile.clone(), items.clone(), &what) {
            push_store_state(m, had_wallet, profile.is_some(), &items, &what)?;
        }
    } else if changed {
        push_store_state(m, wallet.is_some(), profile.is_some(), &items, &what)?;
    }
    // ⭐ The ONE completion path (integration review, finding 6): a scripted activity's completed
    // grant also runs the store's completion (`complete N`: field [6], the rewards table's
    // completion subs such as 164's CONTINUE gate, its XP topped up since the activity started,
    // unlock flags, reputation). The manifest's XP lines stay out of the grant
    // (`Adapter::xp_from_manifest` is off), so XP is paid once, by the table.
    let completion = match completion_index(g) {
        // Run fg2h: recorded already at activity_complete (`completed`): not twice (run fg2j: per
        // activity, not by the one field [6] record).
        Some(index) if completed_this_run(m, index) => format!("; store completion of {index}: already recorded"),
        Some(index) => match run_command(Command::Complete(index)) {
            Ok(w) => format!("; store completion of {index}: {w}"),
            Err(e) => format!("; store completion of {index} NOT applied: {e:#}"),
        },
        None => String::new(),
    };
    Ok(format!(
        "{} for {:#018x}{}: {} currency line(s), {} item(s) ({} delivered), rules {:?}{completion}",
        summary.line,
        summary.character,
        if summary.retargeted { " (the grant named another key; D1_MP is off, one player per responder: applied to it)" } else { "" },
        summary.currencies.len(),
        summary.items,
        summary.delivered.len(),
        summary.rules
    ))
}

/// The activity index whose store completion a grant runs: a completed grant from a scripted
/// activity or the activity framework (`source=script`) whose hash is in the activity table. Open-
/// world grants (events, patrols, chests) are not activity completions.
pub fn completion_index(g: &crate::rewards::remote::GrantCommand) -> Option<u16> {
    use crate::rewards::remote::GrantOutcome;
    if g.outcome != GrantOutcome::Completed || g.source.as_deref() != Some("script") {
        return None;
    }
    crate::tables::tables().activities.values().find(|a| a.hash == g.activity_hash).map(|a| a.index)
}

/// The store's own pushes for a change the vendor state did not serve: the profile (re-encoded
/// from the edited object; each new item rides its own push) and the account.
fn push_store_state(m: &Mutex<Store>, wallet: bool, profile: bool, items: &[(u64, Vec<u8>)], what: &str) -> anyhow::Result<()> {
    if profile || !items.is_empty() {
        let encoded = {
            let mut s = m.lock().map_err(|_| anyhow::anyhow!("poisoned"))?;
            if s.chr().profile_stale {
                reencode(&mut s)?;
                s.save(what)?;
            }
            s.chr().profile.clone().unwrap_or_default()
        };
        let oracle = crate::vendor::oracle_path();
        if items.is_empty() {
            write_push(&encoded)?;
        }
        for (soid, element) in items {
            let (e, _) = crate::vendor::encode(&oracle, "0x80801732", element, &crate::vendor::unique_name("store-grant-item"))?;
            {
                let mut s = m.lock().map_err(|_| anyhow::anyhow!("poisoned"))?;
                s.put_item(*soid, &e);
            }
            write_push_with_item(&encoded, Some((*soid, &e)))?;
        }
        tracing::warn!("      ⭐ {what}: profile pushed by the player store ({} item(s))", items.len());
    }
    if wallet {
        push_account(what)?;
        tracing::warn!("      ⭐ {what}: account pushed by the player store (vendor state off)");
    }
    Ok(())
}

/// What a hand-off changes in the record (no I/O): the next activity, and whether the completed
/// activity's completion still has to be written into the profile's current-activity field
/// (field [6], `progression::layout::ACTIVITY`). `from` is the completed activity's index.
pub fn note_handoff(s: &mut Store, h: &crate::rewards::remote::HandOffCommand, strict: bool) -> anyhow::Result<(HandOff, Option<u16>)> {
    let (_, retargeted) = crate::rewards::remote::resolve_target(&h.target, &s.identity(), strict)?;
    let t = crate::tables::tables();
    if let (Some(hash), Some(row)) = (h.next_hash, t.activities.get(&h.next)) {
        anyhow::ensure!(row.hash == hash, "activity {} is {:#010x} in the activity table, the hand-off says {hash:#010x}", h.next, row.hash);
    }
    let from = h.from.and_then(|hash| t.activities.values().find(|a| a.hash == hash).map(|a| a.index));
    if let (Some(hash), None) = (h.from, from) {
        tracing::warn!("      ⚠ handoff: the completed activity {hash:#010x} is not in the activity table; only the next activity is recorded");
    }
    let rev = s.record.revision + 1;
    let record = HandOff { activity: h.next, hash: h.next_hash, from, revision: rev };
    s.chr_mut().next_activity = Some(record.clone());
    // The completion goes into field [6] unless the profile already shows it.
    let shown = s.chr().summary.as_ref().and_then(|x| x.activity);
    let complete = from.filter(|f| shown != Some((*f, true)));
    s.save(&format!("handoff: next activity {}{}{}", h.next, from.map(|f| format!(" after {f}")).unwrap_or_default(),
                    if retargeted { " (named another key; one player per responder)" } else { "" }))?;
    Ok((record, complete))
}

/// A `handoff` from the activity host (stream K's `activity_hand_off`; cine_reveal -> the Tower):
/// record the next activity, write the completed activity's completion into the profile the
/// client selects its next activity from (field [6], as the client moved from 164 to 165 once 164
/// was recorded complete there: notes/ps4-56578-continue-tower-2026-10-29.md), and push it.
fn run_handoff(m: &Mutex<Store>, h: &crate::rewards::remote::HandOffCommand) -> anyhow::Result<String> {
    let (record, complete) = {
        let mut s = m.lock().map_err(|_| anyhow::anyhow!("poisoned"))?;
        note_handoff(&mut s, h, crate::mp::enabled())?
    };
    let what = format!("next activity {} ({}){}", record.activity,
                       record.hash.map(|x| format!("{x:#010x}")).unwrap_or_else(|| "hash not given".into()),
                       record.from.map(|f| format!(", after {f}")).unwrap_or_default());
    match complete {
        // The same edit as `complete N`: field [6] completed, the reward row's subs, then the push.
        Some(from) => Ok(format!("{what}; {}", run_command(Command::Complete(from))?)),
        None => {
            push_profile("handoff")?;
            Ok(format!("{what}; the profile already shows the completion"))
        }
    }
}

/// The activity index a completed activity's hash names (the activity table).
pub fn activity_index_of(hash: u32) -> Option<u16> {
    crate::tables::tables().activities.values().find(|a| a.hash == hash).map(|a| a.index)
}

/// A `completed` from the activity host (run fg2h): the scripts' `activity_complete`, recorded at
/// once with the same edit as `complete N` (field [6] completed, the rewards table's subs, XP) and
/// pushed, so the client holds the completion before the activity ends. The grant at the results
/// then finds it done and does not complete it again.
fn run_completed(m: &Mutex<Store>, hash: u32, aside: bool) -> anyhow::Result<String> {
    let index = activity_index_of(hash).ok_or_else(|| anyhow::anyhow!("activity {hash:#010x} is not in the activity table"))?;
    if completed_this_run(m, index) {
        return Ok(format!("activity {index} ({hash:#010x}): already completed in this run"));
    }
    if aside {
        let w = run_command(Command::CompleteAside(index))?;
        // Field [6] did not move, so the record notes the completion itself.
        let events = {
            let mut s = m.lock().map_err(|_| anyhow::anyhow!("poisoned"))?;
            let (xp, level) = s.chr().summary.as_ref().map_or((0, 1), |x| (x.xp, x.level));
            let revision = s.record.revision + 1;
            let e = s.record_completion(index, xp, level, revision);
            s.save(&format!("activity {index} completed aside"))?;
            e
        };
        for e in &events {
            tracing::warn!("      💾 {e}");
        }
        return Ok(format!("activity {index} ({hash:#010x}) completed aside at activity_complete (lab stand-in): {w}"));
    }
    run_command(Command::Complete(index)).map(|w| format!("activity {index} ({hash:#010x}) completed at activity_complete: {w}"))
}

/// [`Store::completed_this_run`] on a locked store.
fn completed_this_run(m: &Mutex<Store>, index: u16) -> bool {
    m.lock().ok().is_some_and(|s| s.completed_this_run(index))
}

/// The next profile push goes with the reply to the client's 104 as well as on an echo (the live
/// host's `ffff` reset came in the push answering 104; `main.rs` reads this).
static PUSH_ON_104: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn hold_push_for_104() {
    PUSH_ON_104.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Whether a profile push is waiting for the client's 104 (taken: true once).
pub fn take_push_on_104() -> bool {
    PUSH_ON_104.swap(false, std::sync::atomic::Ordering::SeqCst)
}

/// The file the store writes beside its control file, for the activity host to read back at an
/// activity's launch: `<control>` with the extension `state.json` (`store-control.state.json`).
pub fn state_file_for(control: &Path) -> PathBuf {
    control.with_extension("state.json")
}

/// What the activity host reads back: the signed-on character, its checkpoints (the `step` the
/// host wrote, `checkpoints::parse_step`), its weekly loot lockouts and the last grants.
pub fn state_json(s: &Store) -> Value {
    let c = s.chr();
    serde_json::json!({
        "schema": SCHEMA_VERSION,
        "revision": s.record.revision,
        "player": s.record.player,
        "account": format!("{:#018x}", s.record.account_soid),
        "character": format!("{:#018x}", s.character),
        "checkpoints": c.checkpoints.iter().map(|(a, cp)| (a.to_string(), Value::String(cp.step.clone()))).collect::<serde_json::Map<_, _>>(),
        "lockouts": c.lockouts.iter().map(|(a, l)| (a.to_string(), serde_json::json!(l))).collect::<serde_json::Map<_, _>>(),
        "grants": c.attributes.get(crate::rewards::remote::ATTR_GRANTS).cloned().unwrap_or(Value::Array(Vec::new())),
        "next_activity": c.next_activity.as_ref().map(|h| serde_json::json!({"activity": h.activity, "hash": h.hash, "from": h.from})),
    })
}

fn write_state(control: &Path) {
    let Some(text) = view(|s| serde_json::to_string_pretty(&state_json(s))) else { return };
    let r = text.map_err(anyhow::Error::from).and_then(|t| atomic_write(&state_file_for(control), t.as_bytes()));
    if let Err(e) = r {
        tracing::warn!("      ⛔ player store state file: {e:#}");
    }
}

/// Take the control file's commands: rename it `.taken` first, then read that, so a line another
/// process appends meanwhile goes to a new file and waits for the next poll instead of being lost.
fn take_commands(path: &Path) -> Option<String> {
    let taken = path.with_extension("taken");
    std::fs::rename(path, &taken).ok()?;
    // An append that opened the file just before the rename finishes into `.taken`.
    std::thread::sleep(std::time::Duration::from_millis(50));
    std::fs::read_to_string(&taken).ok()
}

/// Poll the control file once a second: each line is a [`Command`]; the file is renamed `.taken`.
/// After each batch (and at the start) the store's state file is written for the activity host.
fn start_control(path: PathBuf) {
    // `global()` calls this from inside its own `OnceLock` initialiser, so nothing here may touch the
    // store on this thread: `write_state` reads it through `global()`, and that re-entrant
    // initialisation deadlocked the responder before it listened (run fg1b, 6 Nov 2026). The thread
    // writes the first state file instead; its `global()` waits until the initialiser has returned.
    std::thread::spawn(move || {
        write_state(&path);
        control_loop(path)
    });
}

fn control_loop(path: PathBuf) {
    loop {
        if let Some(text) = take_commands(&path) {
            for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
                match parse_command(line).and_then(run_command) {
                    Ok(w) => tracing::warn!("      💾 control {line:?}: {w}"),
                    Err(e) => tracing::warn!("      ⛔ control {line:?}: {e:#}"),
                }
            }
            write_state(&path);
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

// ── serde helpers ───────────────────────────────────────────────────────────────────────────

mod b64 {
    use base64::Engine;
    use serde::{Deserialize, Deserializer, Serializer};
    const E: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

    pub fn serialize<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&E.encode(v))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        E.decode(String::deserialize(d)?).map_err(serde::de::Error::custom)
    }

    pub mod opt {
        use super::E;
        use base64::Engine;
        use serde::{Deserialize, Deserializer, Serializer};

        pub fn serialize<S: Serializer>(v: &Option<Vec<u8>>, s: S) -> Result<S::Ok, S::Error> {
            match v {
                Some(b) => s.serialize_some(&E.encode(b)),
                None => s.serialize_none(),
            }
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<u8>>, D::Error> {
            Option::<String>::deserialize(d)?.map(|t| E.decode(t).map_err(serde::de::Error::custom)).transpose()
        }
    }
}

mod hex64 {
    use serde::{Deserialize, Deserializer, Serializer};

    fn parse<E: serde::de::Error>(t: &str) -> Result<u64, E> {
        u64::from_str_radix(t.trim_start_matches("0x"), 16).map_err(E::custom)
    }

    pub fn serialize<S: Serializer>(v: &u64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("{v:#018x}"))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
        parse(&String::deserialize(d)?)
    }

    pub mod opt {
        use serde::{Deserialize, Deserializer, Serializer};

        pub fn serialize<S: Serializer>(v: &Option<u64>, s: S) -> Result<S::Ok, S::Error> {
            match v {
                Some(x) => s.serialize_some(&format!("{x:#018x}")),
                None => s.serialize_none(),
            }
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
            Option::<String>::deserialize(d)?.map(|t| super::parse(&t)).transpose()
        }
    }
}

#[cfg(test)]
mod tests {
    /// A created look replaces identity and customization f0..f9 and nothing else: the object's
    /// key, the profile's last-orbited destination (f10, +0x28) and the wear-helmet field (+0x2C)
    /// stay.
    #[test]
    fn a_created_look_keeps_the_key_the_destination_and_the_helmet_field() {
        let mut obj = vec![0u8; 0x40];
        obj[..8].copy_from_slice(&0xD100_0000_0000_0201u64.to_le_bytes());
        obj[0x28..0x2C].copy_from_slice(&0x9320_15BBu32.to_le_bytes());
        obj[0x2C] = 1;
        let mut look = [0u8; d1_bap::character_lifecycle::LOOK_BYTES];
        look[..3].copy_from_slice(&[1, 1, 0]); // Awoken, female, Titan
        look[4..6].copy_from_slice(&551i16.to_le_bytes());
        assert!(super::write_look(&mut obj, &look));
        assert_eq!(&obj[8..11], &[1, 1, 0]);
        assert_eq!(i16::from_le_bytes([obj[0x0C], obj[0x0D]]), 551);
        assert_eq!(u64::from_le_bytes(obj[..8].try_into().unwrap()), 0xD100_0000_0000_0201, "the key");
        assert_eq!(u32::from_le_bytes(obj[0x28..0x2C].try_into().unwrap()), 0x9320_15BB, "the destination");
        assert_eq!(obj[0x2C], 1, "the helmet field");
        assert!(!super::write_look(&mut vec![0u8; 0x20], &look), "too short: left alone");
    }

    /// The class kit goes into each bucket's first occupied slot and keeps its instance; an empty
    /// bucket and an account bucket are left alone; the item element's content index follows.
    /// Every 56578 creation record gives the same five profile flags; a created Guardian's record
    /// becomes exactly those (the served save's other completions cleared).
    #[test]
    fn a_created_guardian_starts_with_its_creation_records_flags() {
        let want = vec![(341, 2), (673, 2), (693, 2), (706, 2), (826, 2)];
        for class in 0..3 {
            for race in 0..3 {
                let mut got = creation_profile_flags(class, race);
                got.sort();
                assert_eq!(got, want, "class {class} race {race}");
            }
        }
        assert!(creation_profile_flags(3, 0).is_empty());
        let mut obj = vec![0u8; layout::COMPLETIONS + layout::COMPLETION_BYTES + 4];
        obj[layout::COMPLETIONS + 42] = 2; // Restoration, from a later save
        obj[layout::COMPLETIONS + layout::COMPLETION_BYTES] = 9; // past the record: kept
        assert!(write_completion_record(&mut obj, &want));
        assert_eq!(progression::completion_subs(&obj).into_iter().collect::<Vec<_>>(), want);
        assert_eq!(obj[layout::COMPLETIONS + layout::COMPLETION_BYTES], 9);
        assert!(!write_completion_record(&mut obj, &[(2200, 2)]));
        assert!(!write_completion_record(&mut [0u8; 16], &want));
    }

    /// The second subclass is a copy of the held subclass slot in the bucket's next empty slot,
    /// with its own content and instance; once held, it is not added again.
    #[test]
    fn a_second_subclass_copies_the_held_slot_with_its_own_content_and_instance() {
        let info = crate::store::bucket(0x06).expect("the subclass bucket");
        let mut p = vec![0u8; crate::store::PROFILE_SLOTS + 32 * crate::store::PROFILE_SLOT_COUNT];
        for s in info.first..info.first + info.count {
            let e = crate::store::PROFILE_SLOTS + 32 * s as usize;
            p[e..e + 2].copy_from_slice(&0xFFFFu16.to_le_bytes());
        }
        let first = crate::store::PROFILE_SLOTS + 32 * info.first as usize;
        p[first..first + 2].copy_from_slice(&713u16.to_le_bytes());
        p[first + 8..first + 16].copy_from_slice(&0xD1_0301u64.to_le_bytes());
        p[first + 0x10] = 7;
        assert_eq!(add_second_subclass(&mut p, 712, 0xD1_0400), Some(0xD1_0301));
        let second = first + 32;
        assert_eq!(u16::from_le_bytes([p[second], p[second + 1]]), 712);
        assert_eq!(u64::from_le_bytes(p[second + 8..second + 16].try_into().unwrap()), 0xD1_0400);
        assert_eq!(p[second + 0x10], 7, "the rest of the entry is the held slot's");
        assert_eq!(add_second_subclass(&mut p, 712, 0xD1_0401), None, "already held");
    }

    #[test]
    fn a_class_kit_swaps_the_equipped_contents_and_keeps_the_instances() {
        let slot_of = |b: u8| crate::store::bucket(b).unwrap().first as usize;
        let mut profile = vec![0u8; crate::store::PROFILE_SLOTS + 32 * crate::store::PROFILE_SLOT_COUNT];
        for s in 0..crate::store::PROFILE_SLOT_COUNT {
            let e = crate::store::PROFILE_SLOTS + 32 * s;
            profile[e..e + 2].copy_from_slice(&0xFFFFu16.to_le_bytes());
        }
        let put = |p: &mut Vec<u8>, slot: usize, content: u16, instance: u64| {
            let e = crate::store::PROFILE_SLOTS + 32 * slot;
            p[e..e + 2].copy_from_slice(&content.to_le_bytes());
            p[e + 8..e + 16].copy_from_slice(&instance.to_le_bytes());
        };
        put(&mut profile, slot_of(0x06), 710, 0xD100_0000_0000_0301); // Striker
        put(&mut profile, slot_of(0x07), 6514, 0xD100_0000_0000_0303); // Oryx Mask
        let changed = super::apply_kit_to_profile(&mut profile, &[(0x07, 15), (0x06, 709), (0x08, 9)]);
        assert_eq!(changed, vec![(0xD100_0000_0000_0303, 15, 0x07), (0xD100_0000_0000_0301, 709, 0x06)], "gauntlets empty: skipped");
        let e = crate::store::PROFILE_SLOTS + 32 * slot_of(0x07);
        assert_eq!(u16::from_le_bytes([profile[e], profile[e + 1]]), 15, "Prototype 0.9c Helmet");
        assert_eq!(u64::from_le_bytes(profile[e + 8..e + 16].try_into().unwrap()), 0xD100_0000_0000_0303, "same instance");

        let mut item = vec![0u8; 0x40];
        assert!(super::set_item_content(&mut item, 709));
        assert_eq!(u16::from_le_bytes([item[0x24], item[0x25]]), 709);
        assert!(!super::set_item_content(&mut vec![0u8; 0x10], 709));
    }

    #[test]
    fn a_pickup_applies_the_grants_change_and_keeps_the_stores_own() {
        let base = [0xFFu8, 0xFF, 1, 0, 5, 0];
        let after = [0x0E, 0x0E, 0, 0, 5, 2];
        // The store changed byte 4 (field [6]) and byte 5 itself since the seed.
        let current = [0xFF, 0xFF, 1, 0, 9, 7];
        let (obj, applied, kept) = apply_pickup(&current, &base, &after);
        assert_eq!(obj, vec![0x0E, 0x0E, 0, 0, 9, 7]);
        assert_eq!((applied, kept), (3, 1), "byte 5 is the store's own and the grant's change there is not applied");
    }

    use super::*;

    const ACCT: u64 = 0xD100_0000_0000_0001;
    const CHR: u64 = 0xD100_0000_0000_0101;
    const PROF: u64 = 0xD100_0000_0000_0201;
    const LAB_FLAGS: &str = "98,308,419,440,834,2668,3391,5021,5998,6029,433,2539,2549,3558,3587,3520,3527,3531,3533,3534,3535,3536,3544,3568,3594,3595,305,2669,2670,3389";
    const LAB_SLOTS: &str = "308:869:25000,309:873:500,310:4548:200";

    fn fixtures() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/56578")
    }

    /// The knobs a lab run sets, at the repository's own fixtures.
    fn lab_sources() -> Sources {
        let f = fixtures();
        let p = |r: &str| Some(f.join(r).to_string_lossy().into_owned());
        Sources {
            profile_file: p("characters/profile-tutorial-t.bin"),
            account_file: p("vendor/account-currencies.bin"),
            character_file: None,
            inventory: p("characters/inventory-tutorial"),
            item_soid_base: 0xD100_0000_0000_0301,
            roster_dir: p("characters/rosters-f0-tutorial"),
            unlock_flags: Some(LAB_FLAGS.into()),
            vendor_account_slots: Some(LAB_SLOTS.into()),
        }
    }

    struct Dir(PathBuf);
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn tempdir(tag: &str) -> Dir {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let d = std::env::temp_dir().join(format!("d1-player-store-test-{}-{tag}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Dir(d)
    }

    fn open(dir: &Path, src: &Sources) -> Store {
        Store::open(dir, ACCT, CHR, PROF, src).unwrap()
    }

    /// The family-4 baseline the responder builds from these parts.
    fn family4(account: &[u8], profile: &[u8], items: &[(u64, Vec<u8>)]) -> Vec<u8> {
        let extras: Vec<(u32, u64, Vec<u8>)> = items.iter().map(|(k, e)| (d1_bap::INVENTORY_INSTANCE_TYPE, *k, e.clone())).collect();
        d1_bap::queuez_family4_baseline_body_raw_account_and_profile(ACCT, PROF, account, profile, None, &extras)
    }

    fn stored_items(s: &Store) -> Vec<(u64, Vec<u8>)> {
        s.chr().items.iter().map(|i| (i.soid, i.element.clone())).collect()
    }

    #[test]
    fn the_first_serve_is_the_fixtures_byte_for_byte() {
        let src = lab_sources();
        let d = tempdir("seed");
        let s = open(&d.0, &src);
        let f = fixtures();
        let profile = std::fs::read(f.join("characters/profile-tutorial-t.bin")).unwrap();
        let account = std::fs::read(f.join("vendor/account-currencies.bin")).unwrap();
        assert_eq!(s.chr().profile.as_deref(), Some(&profile[..]));
        assert_eq!(s.record.account.payload.as_deref(), Some(&account[..]));
        // The inventory, in the order and with the keys the responder gives the files.
        let mut files: Vec<_> = std::fs::read_dir(f.join("characters/inventory-tutorial")).unwrap()
            .map(|e| e.unwrap().path()).filter(|p| p.extension().unwrap() == "bin").collect();
        files.sort();
        let expect: Vec<(u64, Vec<u8>)> = files.iter().enumerate()
            .map(|(i, p)| (0xD100_0000_0000_0301 + i as u64, std::fs::read(p).unwrap())).collect();
        assert_eq!(expect.len(), 10);
        assert_eq!(stored_items(&s), expect);
        assert_eq!(family4(s.record.account.payload.as_ref().unwrap(), s.chr().profile.as_ref().unwrap(), &stored_items(&s)),
                   family4(&account, &profile, &expect), "the family-4 baseline is identical");
        let cards: Vec<_> = s.chr().roster.iter().map(|c| (c.name.clone(), c.payload.clone())).collect();
        assert_eq!(Some(cards), read_roster_dir(src.roster_dir.as_ref().unwrap()));
        assert_eq!(s.chr().roster.len(), 4);
        assert_eq!(s.record.account.unlock_flags.as_ref().unwrap().iter().map(u16::to_string).collect::<Vec<_>>().join(","), LAB_FLAGS);
        assert_eq!(s.record.account.slots, parse_account_slots(LAB_SLOTS));
        assert_eq!(s.chr().profile_source, "seed:D1_PROFILE_FILE");
        assert!(s.chr().profile_object.is_none(), "nothing decoded at seeding, so the vendor starts from its knob as before");
    }

    #[test]
    fn a_restart_serves_the_store_not_the_knobs() {
        let d = tempdir("restart");
        let mut s = open(&d.0, &lab_sources());
        s.set_profile(b"profile after the mission", "push");
        s.put_item(0xD100_0000_0000_03F1, b"arcadia");
        s.record.account.payload = Some(b"account with less glimmer".to_vec());
        s.save("test").unwrap();
        let before = s.record.clone();
        drop(s);
        // Restart with different knobs: the record wins.
        let s = open(&d.0, &Sources::default());
        assert_eq!(s.record, before);
        assert_eq!(s.chr().profile.as_deref(), Some(&b"profile after the mission"[..]));
        assert_eq!(s.chr().items.last().unwrap().element, b"arcadia");
        assert_eq!(s.chr().items.len(), 11);
        assert!(d.0.join(format!("account-{ACCT:016x}.json.prev")).exists());
    }

    #[test]
    fn a_new_character_on_an_existing_account_is_seeded() {
        let d = tempdir("chr");
        drop(open(&d.0, &lab_sources()));
        let s = Store::open(&d.0, ACCT, 0xD100_0000_0000_0102, PROF, &lab_sources()).unwrap();
        assert_eq!(s.record.characters.len(), 2);
        assert_eq!(s.chr().items.len(), 10);
    }

    #[test]
    fn schema_versions_are_checked() {
        let d = tempdir("schema");
        let mut s = open(&d.0, &Sources::default());
        s.save("x").unwrap();
        let path = s.path().unwrap().to_path_buf();
        drop(s);
        let mut v: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(v["schema"], SCHEMA_VERSION);
        assert!(migrate(v.clone()).is_ok());
        v["schema"] = (SCHEMA_VERSION + 1).into();
        let future = serde_json::to_vec(&v).unwrap();
        std::fs::write(&path, &future).unwrap();
        let e = Store::open(&d.0, ACCT, CHR, PROF, &lab_sources()).err().unwrap().to_string();
        assert!(e.contains("newer"), "{e}");
        assert_eq!(std::fs::read(&path).unwrap(), future, "a refused record is left untouched");
        v["schema"] = 0.into();
        assert!(migrate(v.clone()).is_err());
        v.as_object_mut().unwrap().remove("schema");
        assert!(migrate(v).is_err());
        // Another account's record is refused too.
        let other = tempdir("schema-acct");
        drop(open(&other.0, &Sources::default()));
        std::fs::rename(Store::file_for(&other.0, ACCT), Store::file_for(&other.0, 0xD1)).unwrap();
        assert!(Store::open(&other.0, 0xD1, CHR, PROF, &Sources::default()).is_err());
    }

    #[test]
    fn unknown_fields_and_attributes_survive() {
        let d = tempdir("attrs");
        let mut s = open(&d.0, &lab_sources());
        s.chr_mut().attributes.insert("grimoire.score".into(), 4200.into());
        s.chr_mut().items[0].attributes.insert("kills".into(), 12.into());
        s.record.account.attributes.insert("crucible.elo".into(), Value::from(1200));
        s.save("attributes").unwrap();
        drop(s);
        let s = open(&d.0, &Sources::default());
        assert_eq!(s.chr().attributes["grimoire.score"], 4200);
        assert_eq!(s.chr().items[0].attributes["kills"], 12);
        assert_eq!(s.record.account.attributes["crucible.elo"], 1200);
    }

    #[test]
    fn account_user_data_is_kept_replaced_and_reloaded() {
        let d = tempdir("user-data");
        let mut s = open(&d.0, &lab_sources());
        assert_eq!(s.record.account.account_user_data, None, "nothing until a 701 brings it");
        s.record.account.account_user_data = Some(vec![7; 1088]);
        s.record.account.account_user_data_at = 5;
        s.save("user data").unwrap();
        s.record.account.account_user_data = Some(vec![9; 1088]);
        s.save("user data again").unwrap();
        drop(s);
        let s = open(&d.0, &Sources::default());
        assert_eq!(s.record.account.account_user_data, Some(vec![9; 1088]), "the later one replaced the first");
        let text = std::fs::read_to_string(Store::file_for(&d.0, ACCT)).unwrap();
        assert!(text.contains("account_user_data"), "stored under its own key");
    }

    fn profile_object() -> Vec<u8> {
        let mut o = vec![0u8; layout::OBJECT_BYTES];
        for slot in 0..layout::ITEM_SLOT_COUNT {
            let e = layout::ITEM_SLOTS + layout::ITEM_SLOT_STRIDE * slot;
            o[e..e + 2].copy_from_slice(&[0xFF, 0xFF]);
        }
        o
    }

    /// The post-game: the mid-mission push starts the activity, the outro's push completes it, and
    /// a completion is recorded once however often the same profile is delivered.
    #[test]
    fn delivered_profiles_record_activity_completions() {
        let d = tempdir("postgame");
        let mut s = open(&d.0, &lab_sources());
        let mut active = profile_object();
        progression::start_activity(&mut active, 164).unwrap();
        s.set_profile(b"active", "push");
        let e = s.adopt_object(b"active", active.clone()).unwrap();
        assert_eq!(e, vec!["activity 164 started at 0 XP"]);
        assert_eq!(s.chr().current, Some(Current { activity: 164, xp_at_start: 0 }));
        let mut done = active.clone();
        let (_, g) = progression::complete_activity(&mut done, &s.curves, 164, s.rewards.0.get(&164), Some(0)).unwrap();
        assert_eq!(g.unwrap().level_xp, 900);
        s.set_profile(b"done", "push");
        let e = s.adopt_object(b"done", done.clone()).unwrap();
        assert!(e[0].starts_with("activity 164 completed (900 XP, level 2)"), "{e:?}");
        assert!(e.iter().any(|x| x.contains("stand-in")), "a stand-in reward is said so: {e:?}");
        assert_eq!(s.chr().completions[&164].count, 1);
        assert!(!s.set_profile(b"done", "push"), "the same payload again changes nothing");
        assert!(s.adopt_object(b"done", done).unwrap().is_empty());
        assert_eq!(s.chr().completions[&164].count, 1);
        let sum = s.chr().summary.clone().unwrap();
        assert_eq!((sum.xp, sum.level, sum.activity), (900, 2, Some((164, true))));
        assert_eq!(sum.completion_subs, BTreeMap::from([(40, 2), (673, 1)]));
        s.save("postgame").unwrap();
        drop(s);
        let mut s = open(&d.0, &Sources::default());
        assert_eq!(s.chr().completions[&164].count, 1);
        assert_eq!(s.chr().profile_object.as_ref().unwrap()[layout::ACTIVITY_COMPLETED], 1);
        assert!(s.adopt_object(b"stale", profile_object()).is_ok_and(|e| e[0].contains("no longer")));
    }

    #[test]
    fn control_commands_edit_the_profile_through_the_curves() {
        let d = tempdir("control");
        let s = open(&d.0, &lab_sources());
        let mut o = profile_object();
        for line in ["activity 164", "xp 900", "complete 164"] {
            edit_profile(&mut o, &parse_command(line).unwrap(), &s).unwrap();
        }
        let sum = summarize(&o, &s.curves);
        assert_eq!((sum.xp, sum.level), (1800, 2), "complete tops up from the activity's start: with no start recorded it adds the full 900");
        let mut o = profile_object();
        edit_profile(&mut o, &Command::Xp(400_000), &s).unwrap();
        let sum = summarize(&o, &s.curves);
        assert_eq!((sum.level, sum.prestige, sum.progressions[&117], sum.progressions[&118]), (40, 2, 346_000, 54_000));
        assert_eq!(parse_command("unlock +2838 -308").unwrap(), Command::Unlock { add: vec![2838], clear: vec![308] });
        assert_eq!(parse_command("checkpoint 164 the divide").unwrap(), Command::Checkpoint(164, "the divide".into()));
        assert!(parse_command("teleport 3").is_err());
        assert!(edit_profile(&mut o, &Command::Checkpoint(1, String::new()), &s).is_err());
    }

    #[test]
    fn unlock_flags_reputation_checkpoints_and_resets_persist() {
        let d = tempdir("unlocks");
        let mut s = open(&d.0, &lab_sources());
        s.change_unlock_flags(&[2838], &[308, 3391]).unwrap();
        let flags = s.record.account.unlock_flags.clone().unwrap();
        assert!(flags.contains(&2838) && !flags.contains(&308) && !flags.contains(&3391));
        assert!(s.change_unlock_flags(&(1000..1030).collect::<Vec<_>>(), &[]).is_err(), "50 at most");
        assert_eq!(s.record.account.unlock_flags.as_ref().unwrap(), &flags, "a refused change changes nothing");
        assert!(s.add_reputation("faction_fotc_vanguard", 1600).contains("rank 0 -> 1"));
        assert!(s.add_reputation("nonsense", 1).starts_with("⛔"));
        s.set_checkpoint(164, "dock 13");
        let resets = VendorResets::parse("vendor_hash\tname\ti\to\tp\n2668878854\tVanguard Quartermaster\t10080\t2340\t0\n");
        let tue = 1_474_372_800 / 60;
        assert!(s.roll_vendor_resets(&resets, tue).is_empty(), "the first look records, it doesn't report");
        assert!(s.roll_vendor_resets(&resets, tue + 60).is_empty(), "same week");
        assert_eq!(s.roll_vendor_resets(&resets, tue + 7 * 1440).len(), 1, "the next Tuesday");
        s.save("x").unwrap();
        drop(s);
        let s = open(&d.0, &Sources::default());
        assert_eq!(s.record.account.unlock_flags.as_ref().unwrap(), &flags);
        assert_eq!(s.chr().progress["faction_fotc_vanguard"], 1600);
        assert_eq!(s.chr().checkpoints[&164].step, "dock 13");
        assert_eq!(s.record.account.vendor_resets[&2668878854], (1_474_362_000 / 60) + 7 * 1440);
        let mut none = open(&tempdir("none").0, &Sources::default());
        assert!(none.change_unlock_flags(&[1], &[]).is_err(), "no served list, nothing to change");
    }

    fn progress_at(s: &Store, index: u16) -> Option<u32> {
        progression::progressions(s.chr().profile_object.as_ref()?).get(&index).map(|p| p.0)
    }

    /// Reputation reaches the profile object at the curve's exported runtime index (#322), and the
    /// push is due; a grant made while the profile is undecoded waits as a delta on top of the
    /// profile's own reputation; the store's total survives an older payload without being
    /// counted twice; a completion's reputation lands in the object that completed it.
    #[test]
    fn reputation_reaches_the_profile_push_through_the_exported_index() {
        let d = tempdir("reputation");
        let mut s = open(&d.0, &Sources::default());
        let vanguard = s.curves.by_name("faction_fotc_vanguard").and_then(|c| c.runtime_index).unwrap();
        let crucible = s.curves.by_name("faction_pvp").and_then(|c| c.runtime_index).unwrap();
        assert_eq!((vanguard, crucible), (133, 134), "progression-index.tsv's ordinals");

        // Undecoded: the grant waits, nothing to push.
        let w = s.add_reputation("faction_fotc_vanguard", 1600);
        assert!(w.contains("waits for the decoded profile"), "{w}");
        assert!(!profile_push_due(&mut s).unwrap());
        assert_eq!(s.chr().reputation_pending["faction_fotc_vanguard"], 1600);

        // The decoded profile already holds 5,000 of its own: the pending 1,600 goes on top.
        let mut o = profile_object();
        progression::set_progress(&mut o, vanguard, 5000).unwrap();
        s.set_profile(b"p1", "push");
        let e = s.adopt_object(b"p1", o).unwrap();
        assert!(e.iter().any(|x| x.contains("folded")), "{e:?}");
        assert_eq!(progress_at(&s, vanguard), Some(6600));
        assert!(s.chr().reputation_pending.is_empty());
        let rank = s.curves.by_name("faction_fotc_vanguard").unwrap().level(6600);
        assert_eq!(PlayerStore::reputation(&s, "faction_fotc_vanguard"), Some((6600, rank)));
        assert!(profile_push_due(&mut s).unwrap(), "the folded object is pushed");
        s.chr_mut().profile_stale = false; // as reencode leaves it

        // Decoded: straight into the object, the push due.
        let w = s.add_reputation("faction_fotc_vanguard", 400);
        assert!(w.contains("= 7000") && w.contains("progression 133 (push due)"), "{w}");
        assert_eq!(progress_at(&s, vanguard), Some(7000));
        assert!(profile_push_due(&mut s).unwrap());

        // The client echoes the pushed total: nothing more to push.
        let mut o = profile_object();
        progression::set_progress(&mut o, vanguard, 7000).unwrap();
        s.set_profile(b"p2", "push");
        assert!(s.adopt_object(b"p2", o).unwrap().is_empty());
        assert!(!profile_push_due(&mut s).unwrap());

        // An older payload delivered late (6,600): the store's 7,000 comes back, not 7,400.
        let mut o = profile_object();
        progression::set_progress(&mut o, vanguard, 6600).unwrap();
        s.set_profile(b"p3", "push");
        s.adopt_object(b"p3", o).unwrap();
        assert_eq!(progress_at(&s, vanguard), Some(7000));
        assert!(profile_push_due(&mut s).unwrap());

        // A completion's reward-row reputation lands in the completing object, once.
        s.rewards = Rewards::parse("activity\tname\tsubs\txp\treputation\tunlock_add\tunlock_clear\tstatus\tsource\n\
                                    164\tt\t-\t-\tfaction_pvp:100\t-\t-\tstand-in\ttest\n");
        assert_eq!(s.rewards.0[&164].reputation, vec![("faction_pvp".to_string(), 100)]);
        let mut done = profile_object();
        progression::set_progress(&mut done, vanguard, 7000).unwrap();
        progression::complete_activity(&mut done, &s.curves, 164, None, Some(0)).unwrap();
        s.set_profile(b"p4", "push");
        let e = s.adopt_object(b"p4", done.clone()).unwrap();
        assert!(e.iter().any(|x| x.starts_with("faction_pvp +100 = 100")), "{e:?}");
        assert_eq!((progress_at(&s, crucible), progress_at(&s, vanguard)), (Some(100), Some(7000)));
        assert!(profile_push_due(&mut s).unwrap());
        let again = s.adopt_object(b"p4", done).unwrap();
        assert!(again.iter().all(|x| !x.starts_with("faction_pvp")), "no second grant: {again:?}");
        assert_eq!(progress_at(&s, crucible), Some(100), "the store's total folded back, once");

        // Totals and pending deltas persist.
        s.chr_mut().profile_object = None;
        s.add_reputation("faction_pvp", 50);
        s.save("reputation").unwrap();
        drop(s);
        let s = open(&d.0, &Sources::default());
        assert_eq!((s.chr().progress["faction_pvp"], s.chr().reputation_pending["faction_pvp"]), (150, 50));
    }

    /// Stream O: `ps4/profiles/full-game.env`'s `[server]` knobs (#320) through the responder's
    /// parsers. The store, its command file and rewards resolve on, the state file is the one the
    /// activity host reads (`checkpoints::state_file_for`), and every `[server]` knob the responder
    /// reads is in its startup table (`=== d1-responder environment`), so the table shows it SET.
    /// The activity host's side is `d1-activity-host/tests/full_game_profile.rs`.
    #[test]
    fn the_full_game_profile_turns_the_store_seams_on() {
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../..");
        let profile = repo.join("ps4/profiles/full-game.env");
        if !profile.is_file() {
            eprintln!("SKIP: no {} (#320's profile)", profile.display());
            return;
        }
        fn read(path: &Path, server: &mut BTreeMap<String, String>) {
            let mut section = String::new();
            for line in std::fs::read_to_string(path).unwrap().lines().map(str::trim) {
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some(f) = line.strip_prefix("@include ") {
                    read(&path.parent().unwrap().join(f.trim()), server);
                } else if let Some(s) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                    section = s.to_string();
                } else if section == "server" {
                    let (k, v) = line.trim_start_matches('?').split_once('=').unwrap();
                    server.insert(k.to_string(), v.replace("${RUN}", "run").replace("${STAGING}", "staging"));
                }
            }
        }
        let mut server = BTreeMap::new();
        read(&profile, &mut server);
        let var = |k: &str| server.get(k).filter(|v| !v.is_empty()).cloned();
        let cfg = Config::from_vars(var).expect("D1_STORE=1 in full-game.env");
        let control = cfg.control.clone().expect("D1_STORE_CONTROL: the activity host's checkpoints and grants");
        assert_eq!(state_file_for(&control), Path::new("run/store-control.state.json"));
        assert_eq!(var("D1_REWARDS").as_deref(), Some("1"), "grants are applied (rewards::enabled)");
        // The responder's own knobs: every one it reads is in its startup table.
        let mut source = String::new();
        // The responder's own source and d1-mp's, whose knobs it reads through `Services::from_env`.
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut dirs = vec![src, PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../d1-mp/src")];
        while let Some(d) = dirs.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    dirs.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    source += &std::fs::read_to_string(&p).unwrap();
                }
            }
        }
        let table: Vec<&str> = crate::KNOBS.iter().chain(crate::rewards::KNOBS).map(|(k, _, _)| *k).collect();
        let unlisted: Vec<&String> = server
            .keys()
            .filter(|k| source.contains(&format!("\"{k}\"")) && !table.contains(&k.as_str()))
            .collect();
        assert!(unlisted.is_empty(), "the responder reads these full-game knobs but its startup table does not list them: {unlisted:?}");
    }

    /// Stream O: the commands the activity host writes (checkpoint clears, lockouts, grants).
    #[test]
    fn the_hosts_commands_parse() {
        assert_eq!(parse_command("checkpoint 15 -").unwrap(), Command::ClearCheckpoint(15));
        assert_eq!(parse_command("clear-checkpoint 15").unwrap(), Command::ClearCheckpoint(15));
        assert_eq!(parse_command("checkpoint 15 1 the-bridge week=2965").unwrap(), Command::Checkpoint(15, "1 the-bridge week=2965".into()));
        assert_eq!(parse_command("lockout 15 the-abyss week=2965").unwrap(), Command::Lockout(15, "the-abyss".into(), 2965));
        assert!(parse_command("lockout 15 the-abyss").is_err());
        assert!(parse_command("lockout 15 the-abyss week=x").is_err());
        let Command::Grant(g) = parse_command("grant 0xd100000000000101 0x0bbd0e26 completed source=script").unwrap() else { panic!() };
        assert_eq!(g.activity_hash, 0x0BBD_0E26);
        assert!(parse_command("grant 0x1 0x2 nope").is_err());
    }

    /// #309's gap: a vault item keeps its instance through a vendor commit, a save and a restart.
    #[test]
    fn vault_instances_and_lockouts_persist() {
        let d = tempdir("vault");
        let mut s = open(&d.0, &lab_sources());
        let mut slots = s.record.account.slots.clone();
        slots.insert(20, (1234, 1));
        s.commit_vendor(VendorCommit {
            profile_payload: b"vault".to_vec(),
            profile_object: profile_object(),
            account_payload: vec![1, 2, 3],
            slots,
            slot_instances: BTreeMap::from([(20, 0xD100_0000_0000_030B), (21, 0x999)]),
            next_item_soid: 0x700,
            refunds: Vec::new(),
            added: None,
            removed: None,
            what: "vault move".into(),
        })
        .unwrap();
        s.set_lockout(15, "the-abyss", 2965);
        s.save("x").unwrap();
        drop(s);
        let s = open(&d.0, &Sources::default());
        assert_eq!(s.record.account.slot_instances, BTreeMap::from([(20, 0xD100_0000_0000_030B)]), "slot 21 holds nothing, so no instance");
        assert_eq!(s.lockout_week(15, "the-abyss"), Some(2965));
        assert_eq!(s.lockout_week(15, "the-bridge"), None);
        // A record from before the field reads with no instances.
        let mut v = serde_json::to_value(&s.record).unwrap();
        v["account"].as_object_mut().unwrap().remove("slot_instances");
        v["characters"].as_object_mut().unwrap().values_mut().for_each(|c| { c.as_object_mut().unwrap().remove("lockouts"); });
        let old = migrate(v).unwrap();
        assert!(old.account.slot_instances.is_empty());
    }

    #[test]
    fn the_state_file_carries_checkpoints_lockouts_and_the_character() {
        let mut s = Store::in_memory(Identity::from_env(), &Sources::default());
        s.set_checkpoint(15, "1 the-bridge week=2965");
        s.set_lockout(15, "the-abyss", 2965);
        let v = state_json(&s);
        assert_eq!(v["character"], format!("{:#018x}", s.identity().character));
        assert_eq!(v["checkpoints"]["15"], "1 the-bridge week=2965");
        assert_eq!(v["lockouts"]["15"]["the-abyss"], 2965);
        assert_eq!(state_file_for(Path::new("run/store-control.txt")), Path::new("run/store-control.state.json"));
    }

    #[test]
    fn commands_are_taken_by_renaming_first() {
        let d = tempdir("take");
        let path = d.0.join("store-control.txt");
        assert!(take_commands(&path).is_none(), "no file, nothing to take");
        std::fs::write(&path, "checkpoint 15 -\n").unwrap();
        assert_eq!(take_commands(&path).as_deref(), Some("checkpoint 15 -\n"));
        assert!(!path.exists(), "the next writer starts a new file");
        std::fs::write(&path, "xp 1\n").unwrap();
        assert_eq!(take_commands(&path).as_deref(), Some("xp 1\n"), "a second batch replaces the old .taken");
    }

    #[test]
    fn a_loot_grant_keeps_the_item_and_its_cards() {
        let d = tempdir("loot");
        let mut s = open(&d.0, &lab_sources());
        s.put_item(0xD100_0000_0000_0401, b"khvostov");
        s.set_profile(b"profile with the rifle", "loot");
        let grant = fixtures().join("characters/roster-def1200-helm-saint14");
        let cards: Vec<_> = read_roster_dir(grant.to_str().unwrap()).unwrap().into_iter()
            .filter(|(n, _)| n.contains("t1-")).collect();
        let n = cards.len();
        assert!(n > 0);
        s.put_cards(cards.clone());
        s.save("loot").unwrap();
        drop(s);
        let s = open(&d.0, &Sources::default());
        assert_eq!(s.chr().items.iter().find(|i| i.soid == 0xD100_0000_0000_0401).unwrap().element, b"khvostov");
        for (name, payload) in cards {
            assert_eq!(s.chr().roster.iter().find(|c| c.name == name).unwrap().payload, payload);
        }
        let mut names: Vec<_> = s.chr().roster.iter().map(|c| c.name.clone()).collect();
        let sorted = { let mut v = names.clone(); v.sort(); v };
        assert_eq!(names, sorted);
        names.dedup();
        assert_eq!(names.len(), s.chr().roster.len(), "replaced by name, not duplicated");
    }

    // ── crash safety ──

    /// A crash after the `.tmp` is written but before the rename: the old record loads, whole, and
    /// the next save replaces the leftover.
    #[test]
    fn a_crash_before_the_rename_leaves_the_old_record() {
        let d = tempdir("crash-tmp");
        let mut s = open(&d.0, &lab_sources());
        s.save("before the crash").unwrap();
        let path = s.path().unwrap().to_path_buf();
        let on_disk = std::fs::read(&path).unwrap();
        let before = s.record.clone();
        // What a crash mid-save leaves: a half-written .tmp beside the old file.
        std::fs::write(sidecar(&path, "tmp"), &on_disk[..on_disk.len() / 2]).unwrap();
        drop(s);
        let mut s = open(&d.0, &Sources::default());
        assert_eq!(s.record, before, "the old record, untouched by the half-written one");
        s.save("after").unwrap();
        assert!(!sidecar(&path, "tmp").exists(), "the next save renames its own .tmp over the file");
        let v: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(v["revision"], before.revision + 1);
    }

    /// A record that doesn't parse (a torn write on a failing disk, a bad hand edit) falls back to
    /// the `.prev` copy, and the bad file is kept aside rather than overwritten.
    #[test]
    fn an_unreadable_record_falls_back_to_the_previous_one() {
        let d = tempdir("crash-prev");
        let mut s = open(&d.0, &lab_sources());
        s.record.account.slots.insert(308, (869, 1234));
        s.save("first").unwrap();
        s.record.account.slots.insert(308, (869, 999));
        s.save("second").unwrap();
        let path = s.path().unwrap().to_path_buf();
        drop(s);
        std::fs::write(&path, b"{\"schema\": 1, \"account_so").unwrap();
        let s = open(&d.0, &Sources::default());
        assert_eq!(s.record.account.slots[&308], (869, 1234), "the record before the last save");
        let aside: Vec<_> = std::fs::read_dir(&d.0).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".corrupt-")).collect();
        assert_eq!(aside.len(), 1, "the unreadable file is kept: {aside:?}");
        // With neither readable, the store refuses rather than seeding over them.
        std::fs::write(&path, b"x").unwrap();
        std::fs::write(sidecar(&path, "prev"), b"y").unwrap();
        assert!(Store::open(&d.0, ACCT, CHR, PROF, &lab_sources()).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"x");
    }

    #[test]
    fn atomic_write_replaces_and_keeps_the_previous() {
        let d = tempdir("atomic");
        let f = d.0.join("x.json");
        atomic_write(&f, b"1").unwrap();
        assert!(!sidecar(&f, "prev").exists());
        atomic_write(&f, b"2").unwrap();
        assert_eq!((std::fs::read(&f).unwrap(), std::fs::read(sidecar(&f, "prev")).unwrap()), (b"2".to_vec(), b"1".to_vec()));
        assert!(!sidecar(&f, "tmp").exists());
    }

    // ── saving ──

    #[test]
    fn small_changes_wait_for_the_periodic_save() {
        let d = tempdir("periodic");
        let mut s = open(&d.0, &lab_sources());
        let path = s.path().unwrap().to_path_buf();
        let saved = |p: &Path| -> u64 { serde_json::from_slice::<Value>(&std::fs::read(p).unwrap()).unwrap()["revision"].as_u64().unwrap() };
        let r0 = saved(&path);
        assert!(!s.flush().unwrap(), "nothing waiting, nothing written");
        s.set_light(Some(5));
        s.mark_dirty("light 5");
        assert!(s.is_dirty());
        assert_eq!(saved(&path), r0, "not yet on disk");
        assert!(s.flush().unwrap());
        assert_eq!(saved(&path), r0 + 1);
        assert!(!s.is_dirty());
        s.mark_dirty("bounty progress");
        s.save_now("purchase").unwrap();
        assert!(s.record.history.last().unwrap().what.contains("bounty progress"), "an important save carries what waited");
        assert!(!s.flush().unwrap());
    }

    #[test]
    fn the_store_is_off_unless_d1_store_is_1() {
        let vars = |pairs: &'static [(&'static str, &'static str)]| move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string());
        assert_eq!(Config::from_vars(vars(&[])), None);
        assert_eq!(Config::from_vars(vars(&[("D1_STORE", "0"), ("D1_STORE_DIR", "x")])), None);
        let c = Config::from_vars(vars(&[("D1_STORE", "1"), ("D1_STORE_DIR", "F:/store")])).unwrap();
        assert_eq!((c.dir, c.player.as_str(), c.save_every.as_secs(), c.control), (PathBuf::from("F:/store"), "local", 300, None));
        let c = Config::from_vars(vars(&[("D1_STORE", "1"), ("D1_STORE_SAVE_SECS", "30"), ("D1_STORE_PLAYER", "p2"),
                                         ("D1_STORE_CONTROL", "c.txt")])).unwrap();
        assert_eq!((c.player.as_str(), c.save_every.as_secs(), c.control), ("p2", 30, Some(PathBuf::from("c.txt"))));
        assert_eq!(c.dir, default_dir());
        assert!(!default_dir().starts_with(env!("CARGO_MANIFEST_DIR")), "never inside the repository");
    }

    // ── identity ──

    /// Stream O3: each player has its own push files beside the knob's, and a record is opened
    /// only for a player `identities.json` files under that exact identity.
    #[test]
    fn each_player_has_its_own_push_files_and_record() {
        let base = Path::new("run").join("profile-push.bin");
        assert_eq!(player_file(&base, None), base, "the responder's own player: the knob's file");
        assert_eq!(player_file(&base, Some(0xD101_0000_0000_1001)), Path::new("run").join("profile-push-d101000000001001.bin"));
        assert_eq!(player_file(&base, Some(1)).with_extension("item"), Path::new("run").join("profile-push-0000000000000001.item"));
        let d = tempdir("o3-filed");
        let first = Identity::from_env();
        identity_for(&d.0, "local", first).unwrap();
        let b = identity_for(&d.0, "friend", first).unwrap();
        assert_eq!(filed_player(&d.0, &b).as_deref(), Some("friend"));
        assert_eq!(filed_identity(&d.0, b.account, b.character), Some(b));
        assert_eq!(filed_identity(&d.0, first.account, b.character), None, "account and character must both match");
        assert_eq!(filed_character(&d.0, b.character), Some(b));
        assert_eq!(filed_player(&d.0, &Identity { profile: 1, ..b }), None, "the whole identity");
        let m = open_other(&d.0, &b).expect("a filed player's record opens");
        assert_eq!(m.lock().unwrap().record.player, "friend");
        assert!(std::ptr::eq(m, open_other(&d.0, &b).unwrap()), "opened once");
        assert!(open_other(&d.0, &Identity::allocated(77)).is_none(), "never for an unfiled identity");
    }

    #[test]
    fn identities_are_stable_and_never_reused() {
        let d = tempdir("ids");
        let first = Identity { account: ACCT, character: CHR, profile: PROF };
        let a = identity_for(&d.0, "local", first).unwrap();
        assert_eq!(a, first, "the first player keeps the knobs' ids");
        let b = identity_for(&d.0, "guardian2", first).unwrap();
        assert_eq!(b, Identity::allocated(1));
        assert_eq!((b.account, b.character, b.profile), (0xD101_0000_0000_1001, 0xD101_0000_0000_1101, 0xD101_0000_0000_1201));
        assert_eq!(identity_for(&d.0, "local", Identity::allocated(9)).unwrap(), a, "a known player keeps theirs");
        assert_eq!(identity_for(&d.0, "guardian2", first).unwrap(), b);
        assert_eq!(identity_for(&d.0, "guardian3", first).unwrap(), Identity::allocated(2));
        assert!(identity_for(&d.0, "", first).is_err());
        // Each identity opens its own record.
        let s = Store::open_player(&d.0, "guardian2", first, &Sources::default()).unwrap();
        assert_eq!((s.identity(), s.player()), (b, "guardian2"));
        assert!(Store::file_for(&d.0, b.account).exists());
        drop(s);
        let s = Store::open_player(&d.0, "guardian2", first, &Sources::default()).unwrap();
        assert_eq!(s.identity(), b);
    }

    // ── the trait ──

    /// What the other streams do through [`PlayerStore`], on the in-memory store.
    fn exercise(st: &mut dyn PlayerStore) {
        // I: inventory, currencies, refunds, bounties.
        let soid = st.mint_item_soid();
        assert_eq!(soid, FIRST_ITEM_SOID);
        assert_eq!(st.mint_item_soid(), FIRST_ITEM_SOID + 1);
        st.put_item(soid, b"gjallarhorn");
        assert!(st.items().iter().any(|i| i.soid == soid));
        assert!(st.remove_item(soid) && !st.remove_item(soid));
        assert_eq!(st.currency(869), 25_000);
        assert_eq!(st.adjust_currency(869, -570).unwrap(), 24_430);
        assert!(st.adjust_currency(869, -100_000).is_err(), "no debt");
        assert!(st.adjust_currency(1, 5).is_err(), "no slot holds it");
        assert_eq!(st.refund_window(3917130357, None), Some(900));
        assert_eq!(st.refund_window(1990950, None), None);
        st.push_refund(Refund { vendor: 12, sale: 0, item: 4604, instance: 0x77, costs: vec![(873, 1000)], bought: 100, expires: 1000 });
        assert!(st.take_refund(0x77, 1000).is_none(), "expired");
        assert_eq!(st.take_refund(0x77, 999).unwrap().costs, vec![(873, 1000)]);
        assert!(st.refunds().is_empty());
        st.put_bounty(Bounty { item_hash: 2874376956, objectives: vec![1], completed: false, acquired: 5 });
        assert_eq!(st.bounties()[&2874376956].objectives, vec![1]);
        assert!(st.remove_bounty(2874376956).is_some());
        // M: XP, reputation, Light, Grimoire, completions, unlocks.
        assert!(st.add_xp(10).is_err(), "no decoded profile yet");
        assert!(st.add_reputation("faction_pvp", 2000).contains("rank 0 -> 1"));
        assert_eq!(st.reputation("faction_pvp"), Some((2000, 1)));
        st.set_light(Some(400));
        assert_eq!(st.light(), Some(400));
        assert!(st.add_grimoire_card(700_100, 40) && st.add_grimoire_card(700_200, 10) && !st.add_grimoire_card(700_100, 40));
        assert_eq!(st.grimoire_score(), 50);
        assert!(st.completions().is_empty());
        st.change_unlock_flags(&[2838], &[]).unwrap();
        assert!(st.unlock_flags().unwrap().contains(&2838));
        // J: checkpoints.
        st.set_checkpoint(164, "the wall");
        assert_eq!(st.checkpoint(164).unwrap().step, "the wall");
        assert!(st.clear_checkpoint(164) && st.checkpoint(164).is_none());
        // H: identity.
        assert_eq!(st.identity(), Identity { account: ACCT, character: CHR, profile: PROF });
        st.mark_dirty("exercised");
        st.save_now("exercised").unwrap();
    }

    #[test]
    fn the_trait_works_in_memory_and_on_disk() {
        let id = Identity { account: ACCT, character: CHR, profile: PROF };
        let mut m = Store::in_memory(id, &lab_sources());
        exercise(&mut m);
        assert_eq!(m.path(), None);
        let d = tempdir("trait");
        let mut s = open(&d.0, &lab_sources());
        exercise(&mut s);
        drop(s);
        let s = open(&d.0, &Sources::default());
        assert_eq!((s.currency(869), s.grimoire_score(), s.light()), (24_430, 50, Some(400)), "what the streams wrote persists");
        assert_eq!(s.record.account.next_item_soid, Some(FIRST_ITEM_SOID + 2));
    }

    /// XP through the trait edits the decoded profile and marks it for re-encoding.
    #[test]
    fn xp_through_the_trait_follows_the_curve() {
        let id = Identity { account: ACCT, character: CHR, profile: PROF };
        let mut s = Store::in_memory(id, &Sources::default());
        s.chr_mut().profile_object = Some(profile_object());
        let g = s.add_xp(346_000).unwrap();
        assert_eq!((g.level_before, g.level_after, g.prestige_after), (1, 40, 0), "level 40 = 346,000 XP");
        let g = s.add_xp(25_000).unwrap();
        assert_eq!((g.level_xp, g.prestige_xp, g.prestige_after), (346_000, 25_000, 1));
        assert_eq!(s.xp(), Some(371_000));
        assert!(s.chr().profile_stale);
        assert_eq!(s.chr().summary.as_ref().unwrap().level, 40);
        assert!(s.set_profile(b"re-encoded", "store"));
        assert!(!s.chr().profile_stale, "a new payload is the profile again");
    }

    /// Integration review, finding 6: a scripted completion also runs the store's completion;
    /// an open-world grant, an incomplete run and an unknown hash do not.
    #[test]
    fn a_scripted_completed_grant_names_its_store_completion() {
        use crate::rewards::remote::{GrantCommand, GrantOutcome, GrantTarget};
        let g = |hash: u32, outcome, source: &str| GrantCommand {
            target: GrantTarget::Key(1), activity_hash: hash, outcome, page: None, engrams: false, seed: None, source: Some(source.into()), reputation: Vec::new(), item: None,
        };
        assert_eq!(completion_index(&g(0x6E0D_AE89, GrantOutcome::Completed, "script")), Some(164), "A Guardian Rises");
        assert_eq!(completion_index(&g(0x6E0D_AE89, GrantOutcome::Incomplete, "script")), None);
        assert_eq!(completion_index(&g(0x6E0D_AE89, GrantOutcome::Completed, "event:12:gold")), None);
        assert_eq!(completion_index(&g(0xDEAD_BEEF, GrantOutcome::Completed, "script")), None);
    }

    /// Run fg2j: a completion is this run's when it comes at or after the activity's start; the
    /// stand-in's completion aside never moves field [6]; the seed completes 164 aside, once.
    #[test]
    fn completions_are_per_activity_and_per_run() {
        let mut s = Store::in_memory(Identity::from_env(), &Sources::default());
        let mut obj = vec![0u8; progression::layout::OBJECT_BYTES];
        // The join: field [6] names 165, completed 0.
        edit_profile(&mut obj, &Command::Activity(165), &s).unwrap();
        s.chr_mut().profile = Some(vec![1]);
        s.adopt_object(&[1], obj.clone()).unwrap();
        s.save("step 1").unwrap();
        assert!(s.chr().started.contains_key(&165));
        assert!(!s.completed_this_run(165));
        // activity_complete: 165 completed.
        edit_profile(&mut obj, &Command::Complete(165), &s).unwrap();
        s.chr_mut().profile = Some(vec![2]);
        s.adopt_object(&[2], obj.clone()).unwrap();
        s.save("step 2").unwrap();
        assert!(s.completed_this_run(165));
        // The stand-in: 164 aside; field [6] stays 165 completed, so 165 stays this run's.
        let w = apply_seed_completion(&mut s, &mut obj, 164).unwrap();
        s.save("aside").unwrap();
        assert!(w.contains("aside"), "{w}");
        assert_eq!(progression::activity_record(&obj), Some((165, true)), "field [6] never moves to 164");
        assert!(s.completed_this_run(164) && s.completed_this_run(165), "both recorded, neither to repeat");
        assert_eq!(s.chr().completions[&165].count, 1);
        assert_eq!(s.chr().completions[&164].count, 1);
        // A later run of 165 starts again, and its completion is due again.
        edit_profile(&mut obj, &Command::Activity(165), &s).unwrap();
        s.chr_mut().profile = Some(vec![3]);
        s.adopt_object(&[3], obj.clone()).unwrap();
        s.save("step 3").unwrap();
        assert!(!s.completed_this_run(165), "a new run");
        // The end of a cinematic: field [6] = ffff, no current activity.
        edit_profile(&mut obj, &Command::ClearActivity, &s).unwrap();
        assert_eq!(progression::activity_record(&obj), None);
        assert_eq!(seed_completed_from(Some("164, 165,x")), vec![164, 165]);
        assert!(seed_completed_from(None).is_empty());
    }

    /// Run fg2h: the activity host's `completed TARGET HASH` line (the scripts' activity_complete)
    /// parses, names its player as the grant does, and maps the reveal's hash to 165.
    #[test]
    fn a_completed_line_names_its_player_and_activity() {
        const FG2G: &str = "e0d55e272f90000098ac2c43570fceb201000000000000d101020000000000d1f7bd728e56f51b90";
        let Command::Completed(target, hash, false) = parse_command(&format!("completed id:{FG2G} 0x6eaf0959")).unwrap() else { panic!() };
        assert_eq!(hash, 0x6EAF_0959);
        assert_eq!(target.character(), Some(0xD100_0000_0000_0201), "the profile SOID, matched by named_by");
        assert_eq!(activity_index_of(hash), Some(165));
        assert!(matches!(parse_command("completed 0xd100000000000101 0x6eaf0959").unwrap(), Command::Completed(_, 0x6EAF_0959, false)));
        assert!(matches!(parse_command("completed 0xd100000000000101 0x6e0dae89 aside").unwrap(), Command::Completed(_, 0x6E0D_AE89, true)));
        assert!(matches!(parse_command("started 0xd100000000000101 0x6eaf0959").unwrap(), Command::Started(_, 0x6EAF_0959)));
        assert!(matches!(parse_command("ended 0xd100000000000101 0x6eaf0959").unwrap(), Command::Ended(_, 0x6EAF_0959)));
        assert!(parse_command("started 0xd100000000000101 0x6eaf0959 aside").is_err(), "aside is for completed only");
        assert!(parse_command("completed 0x1").is_err());
        assert!(parse_command("completed 0x1 nothash").is_err());
        // The own player's store takes it (the identity's account and profile are its own).
        let own = Identity::from_env();
        let dir = std::env::temp_dir().join(format!("d1-completed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let primary = Mutex::new(Store::in_memory(own, &Sources::default()));
        let (m, who) = target_store_in(&target, &primary, own, Some(&dir), true, &|_| None).unwrap();
        assert!(std::ptr::eq(m, &primary) && who.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Stream O2, wire 8: the hand-off line records the next activity, names the completion to
    /// write into field [6], and is in the state file the activity host reads.
    #[test]
    fn a_handoff_records_the_next_activity_and_the_completion_to_write() {
        let t = crate::tables::tables();
        let (Some(reveal), Some(tower)) = (t.activities.get(&165), t.activities.get(&0)) else {
            panic!("the activity table has 165 and 0");
        };
        assert_eq!((reveal.hash, tower.hash), (0x6EAF_0959, 0x8AE8_DA69), "cine_reveal and the Tower (the story-missions note)");
        let mut s = Store::in_memory(Identity::from_env(), &Sources::default());
        let me = s.identity().character;
        let line = format!("handoff {me:#018x} 0 from=0x6eaf0959 hash=0x8ae8da69");
        let Command::HandOff(h) = parse_command(&line).unwrap() else { panic!("not a hand-off") };
        let (record, complete) = note_handoff(&mut s, &h, true).unwrap();
        assert_eq!((record.activity, record.from, complete), (0, Some(165), Some(165)));
        assert_eq!(s.chr().next_activity.as_ref().map(|n| n.activity), Some(0));
        assert_eq!(state_json(&s)["next_activity"]["activity"], 0);
        // A profile that already shows 165 completed is not edited again.
        s.chr_mut().summary = Some(Summary { activity: Some((165, true)), ..Summary::default() });
        assert_eq!(note_handoff(&mut s, &h, true).unwrap().1, None);
        // A wrong hash for the next activity, and another player's hand-off with MP on, are refused.
        let Command::HandOff(bad) = parse_command(&format!("handoff {me:#018x} 0 hash=0x1")).unwrap() else { panic!() };
        assert!(note_handoff(&mut s, &bad, true).is_err());
        let Command::HandOff(other) = parse_command("handoff 0x1234 0").unwrap() else { panic!() };
        assert!(note_handoff(&mut s, &other, true).is_err());
        assert!(note_handoff(&mut s, &other, false).is_ok(), "single player: applied to the signed-on character");
    }
}
