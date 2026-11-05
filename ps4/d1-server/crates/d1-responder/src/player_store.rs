// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The persistent player store: what a Guardian owns and has done, kept across activities and
//! responder restarts, as Bungie's world server kept character data (in memory, persisted, with
//! an immediate save on important events; GDC 2021, "Exploring Services Architecture at Bungie").
//!
//! **Off unless `D1_PLAYER_STORE=<dir>` is set** (outside the repository). Without it nothing here
//! runs and every object is read from the fixture knobs at each sign-on, as before.
//!
//! With it, `<dir>/account-<soid:016x>.json` holds one account and its characters (schema
//! [`SCHEMA_VERSION`], documented in notes/ps4-56578-player-store-2026-11-05.md):
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
//! **Writes** go through on every change, then the record is saved (write to `.tmp`, keep the old
//! file as `.prev`, rename): vendor purchases (901), equips (403), dismantles (402), loot grants,
//! every delivered profile and account push (the post-game rewards among them), unlock-flag
//! changes, and the control commands (`D1_PLAYER_STORE_CONTROL`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::progression::{self, layout, Curves, Rewards, VendorResets};

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
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Account {
    /// The account object as served (`D1_ACCOUNT_FILE` form; pushes replace it).
    #[serde(default, with = "b64::opt")]
    pub payload: Option<Vec<u8>>,
    /// Account inventory slots: slot -> (content index, quantity). Glimmer, Silver, Marks.
    #[serde(default)]
    pub slots: BTreeMap<u16, (u16, u32)>,
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
    #[serde(default)]
    pub attributes: BTreeMap<String, Value>,
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
    #[serde(default)]
    pub checkpoints: BTreeMap<u16, Checkpoint>,
    /// Progress of progressions the profile has no known entry for (reputation): name -> progress.
    #[serde(default)]
    pub progress: BTreeMap<String, u64>,
    #[serde(default)]
    pub attributes: BTreeMap<String, Value>,
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
    }
}

// ── the store ───────────────────────────────────────────────────────────────────────────────

pub struct Store {
    path: PathBuf,
    pub record: Record,
    character: u64,
    pub curves: Curves,
    pub rewards: Rewards,
}

/// What a vendor change (901, 403, 402) leaves: vendor.rs's state after it, encoded.
pub struct VendorCommit {
    pub profile_payload: Vec<u8>,
    pub profile_object: Vec<u8>,
    pub account_payload: Vec<u8>,
    pub slots: BTreeMap<u16, (u16, u32)>,
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

    /// Load the account's record, or seed it from `src` (and save) when there is none. A record
    /// without this character gets the character seeded beside the others.
    pub fn open(dir: &Path, account: u64, character: u64, profile_soid: u64, src: &Sources) -> anyhow::Result<Store> {
        std::fs::create_dir_all(dir)?;
        let path = Self::file_for(dir, account);
        let mut seeded = false;
        let record = if path.exists() {
            let r = migrate(serde_json::from_slice(&std::fs::read(&path)?)?)
                .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
            anyhow::ensure!(r.account_soid == account, "{} holds account {:#x}, not {account:#x}", path.display(), r.account_soid);
            r
        } else {
            seeded = true;
            seed(src, account, character, profile_soid)
        };
        let mut s = Store { path, record, character, curves: Curves::load(), rewards: Rewards::load() };
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

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn chr(&self) -> &Character {
        &self.record.characters[&soid_key(self.character)]
    }

    pub fn chr_mut(&mut self) -> &mut Character {
        self.record.characters.get_mut(&soid_key(self.character)).expect("the character is seeded at open")
    }

    /// Persist now: `.tmp`, then the old file to `.prev`, then rename.
    pub fn save(&mut self, what: &str) -> anyhow::Result<()> {
        self.record.revision += 1;
        self.record.history.push(Change { revision: self.record.revision, at: unix_now(), what: what.to_string() });
        let excess = self.record.history.len().saturating_sub(HISTORY);
        self.record.history.drain(..excess);
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&self.record)?)?;
        if self.path.exists() {
            let _ = std::fs::copy(&self.path, self.path.with_extension("json.prev"));
        }
        std::fs::rename(&tmp, &self.path)?;
        tracing::warn!("      💾 player store r{}: {what}", self.record.revision);
        Ok(())
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
    /// completed (its reward's unlock flags and reputation then apply).
    pub fn adopt_object(&mut self, payload: &[u8], object: Vec<u8>) -> anyhow::Result<Vec<String>> {
        anyhow::ensure!(object.len() == layout::OBJECT_BYTES, "a {}-byte profile object", object.len());
        if self.chr().profile.as_deref() != Some(payload) {
            return Ok(vec!["a decoded profile that is no longer the served one: ignored".into()]);
        }
        let new = summarize(&object, &self.curves);
        let revision = self.record.revision + 1;
        let old = self.chr().summary.clone();
        let mut events = Vec::new();
        let was = old.as_ref().and_then(|o| o.activity);
        if let Some((activity, completed)) = new.activity {
            if !completed && was != Some((activity, false)) {
                self.chr_mut().current = Some(Current { activity, xp_at_start: new.xp });
                events.push(format!("activity {activity} started at {} XP", new.xp));
            }
            if completed && was != Some((activity, true)) {
                let entry = self.chr_mut().completions.entry(activity).or_default();
                entry.count += 1;
                entry.xp_after = new.xp;
                entry.revision = revision;
                self.chr_mut().current = None;
                events.push(format!("activity {activity} completed ({} XP, level {})", new.xp, new.level));
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
            }
        }
        let c = self.chr_mut();
        c.profile_object = Some(object);
        c.summary = Some(new);
        Ok(events)
    }

    /// Reputation or rank progress for a progression the profile carries no known entry for: kept
    /// in the store, its rank from the curve.
    pub fn add_reputation(&mut self, name: &str, amount: u32) -> String {
        let Some(curve) = self.curves.by_name(name).cloned() else {
            return format!("⛔ no progression curve named {name}");
        };
        let p = self.chr_mut().progress.entry(name.to_string()).or_insert(0);
        let before = curve.level(*p);
        *p += u64::from(amount);
        let after = curve.level(*p);
        let mut s = format!("{name} +{amount} = {} (rank {before} -> {after})", *p);
        if curve.runtime_index.is_some() {
            // TODO(progression): write it into the profile's entry once a reputation's ordinal is read.
            s.push_str("; kept in the store only");
        }
        s
    }

    /// Note every vendor reset since the last look (epoch minutes `now`).
    pub fn roll_vendor_resets(&mut self, schedules: &VendorResets, now: i64) -> Vec<String> {
        let mut out = Vec::new();
        for v in &schedules.0 {
            let last = v.last_reset(now);
            match self.record.account.vendor_resets.insert(v.vendor_hash, last) {
                Some(prev) if prev < last => out.push(format!("{} ({}) reset since r{}", v.name, v.vendor_hash, self.record.revision)),
                _ => {}
            }
        }
        out
    }

    pub fn checkpoint(&mut self, activity: u16, step: &str) {
        let revision = self.record.revision + 1;
        self.chr_mut().checkpoints.insert(activity, Checkpoint { step: step.to_string(), revision });
    }
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

fn global() -> Option<&'static Mutex<Store>> {
    static G: OnceLock<Option<Mutex<Store>>> = OnceLock::new();
    G.get_or_init(|| {
        let dir = std::env::var("D1_PLAYER_STORE").ok().filter(|d| !d.is_empty())?;
        let account = crate::env_soid("D1_ACCOUNT_SOID", 0xD100_0000_0000_0001);
        let character = crate::env_soid("D1_CHARACTER_SOID", 0xD100_0000_0000_0101);
        let profile = crate::env_soid("D1_PROFILE_SOID", 0xD100_0000_0000_0201);
        match Store::open(Path::new(&dir), account, character, profile, &Sources::from_env()) {
            Ok(mut s) => {
                tracing::warn!("      💾 player store ON: {} (r{}, schema {})", s.path().display(), s.record.revision, s.record.schema);
                let now = (unix_now() / 60) as i64;
                let resets = s.roll_vendor_resets(&VendorResets::load(), now);
                for r in &resets {
                    tracing::warn!("      💾 vendor reset: {r}");
                }
                if let Err(e) = s.save(&format!("signed on; {} vendor reset(s) since the last session", resets.len())) {
                    tracing::warn!("      ⛔ player store: {e}");
                }
                if let Some(path) = std::env::var_os("D1_PLAYER_STORE_CONTROL") {
                    start_control(PathBuf::from(path));
                }
                Some(Mutex::new(s))
            }
            Err(e) => {
                tracing::warn!("      ⛔⛔ player store {dir} NOT USED ({e:#}): serving the fixtures; the record is left as it is");
                None
            }
        }
    }).as_ref()
}

/// Read from the store: None when it is off (the caller serves the knobs as before).
pub fn view<T>(f: impl FnOnce(&Store) -> T) -> Option<T> {
    global().and_then(|m| m.lock().ok().map(|s| f(&s)))
}

fn with(what: &str, f: impl FnOnce(&mut Store) -> anyhow::Result<Vec<String>>) {
    let Some(m) = global() else { return };
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

/// The roster cards: the store's, or `D1_ROSTER_DIR`'s.
pub fn roster_cards() -> Option<Vec<(String, Vec<u8>)>> {
    match view(|s| s.chr().roster.iter().map(|c| (c.name.clone(), c.payload.clone())).collect::<Vec<_>>()) {
        Some(cards) => Some(cards),
        None => read_roster_dir(&std::env::var("D1_ROSTER_DIR").ok().filter(|d| !d.is_empty())?),
    }
}

/// Decode `payload` with the client's codec off the caller's task, then adopt it.
fn decode_later(payload: Vec<u8>) {
    std::thread::spawn(move || {
        let oracle = crate::vendor::oracle_path();
        match crate::vendor::decode_object(&oracle, "0x80801743", &payload, "store-profile") {
            Ok(obj) => with("profile", |s| {
                let events = s.adopt_object(&payload, obj)?;
                s.save("profile decoded")?;
                Ok(events)
            }),
            Err(e) => tracing::warn!("      ⛔ player store: the delivered profile did not decode ({e:#}); its summary waits"),
        }
    });
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

/// Write a vendor change through (vendor.rs, after it wrote the pushes).
pub fn note_vendor(c: VendorCommit) {
    with("vendor", |s| s.commit_vendor(c));
}

/// What vendor.rs starts from. None when the store is off; `profile` None means the knob's object
/// (`D1_VENDOR_PROFILE_OBJ`), as before the store, while the served profile is still the seed.
pub struct VendorSeed {
    pub profile: Option<Vec<u8>>,
    pub slots: BTreeMap<u16, (u16, u32)>,
    pub next_item_soid: Option<u64>,
    pub refunds: Vec<Refund>,
}

pub fn vendor_seed() -> Option<VendorSeed> {
    let (object, payload, source, slots, next, refunds) = view(|s| {
        let c = s.chr();
        (c.profile_object.clone(), c.profile.clone(), c.profile_source.clone(), s.record.account.slots.clone(),
         s.record.account.next_item_soid, s.record.account.refunds.clone())
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
    Some(VendorSeed { profile, slots, next_item_soid: next, refunds })
}

// ── control commands ────────────────────────────────────────────────────────────────────────

/// One `D1_PLAYER_STORE_CONTROL` command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// The character enters an activity (field [6]).
    Activity(u16),
    /// XP from the server (character_level, then prestige).
    Xp(u32),
    /// The activity completes: field [6], the rewards table's subs and XP.
    Complete(u16),
    Checkpoint(u16, String),
    Reputation(String, u32),
    Unlock { add: Vec<u16>, clear: Vec<u16> },
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
        Some("checkpoint") => Command::Checkpoint(num(1)? as u16, f.get(2..).map(|r| r.join(" ")).unwrap_or_default()),
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
        _ => anyhow::bail!("{line:?}: not a store command (activity, xp, complete, checkpoint, reputation, unlock)"),
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
        _ => anyhow::bail!("{cmd:?} does not edit the profile"),
    }
}

fn run_command(cmd: Command) -> anyhow::Result<String> {
    let m = global().ok_or_else(|| anyhow::anyhow!("the store is off"))?;
    match &cmd {
        Command::Checkpoint(a, step) => {
            let mut s = m.lock().map_err(|_| anyhow::anyhow!("poisoned"))?;
            s.checkpoint(*a, step);
            s.save(&format!("checkpoint {a} {step}"))?;
            return Ok(format!("checkpoint {a}: {step}"));
        }
        Command::Reputation(name, n) => {
            let mut s = m.lock().map_err(|_| anyhow::anyhow!("poisoned"))?;
            let w = s.add_reputation(name, *n);
            s.save(&w)?;
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
    if let Some(push) = std::env::var_os("D1_PROFILE_PUSH_FILE").map(PathBuf::from) {
        // One push at a time: wait for the previous one to be delivered.
        for _ in 0..120 {
            if !push.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        std::fs::write(&push, &encoded)?;
    }
    Ok(what)
}

/// Poll the control file once a second: each line is a [`Command`]; the file is renamed `.taken`.
fn start_control(path: PathBuf) {
    std::thread::spawn(move || loop {
        if let Ok(text) = std::fs::read_to_string(&path) {
            let _ = std::fs::rename(&path, path.with_extension("taken"));
            for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
                match parse_command(line).and_then(run_command) {
                    Ok(w) => tracing::warn!("      💾 control {line:?}: {w}"),
                    Err(e) => tracing::warn!("      ⛔ control {line:?}: {e:#}"),
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    });
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
        let path = s.path().to_path_buf();
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
        s.checkpoint(164, "dock 13");
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
}
