// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The player store the social services act on (stream I: vendors, inventory, bounties, the
//! Director, character creation).
//!
//! Every state change the services make goes through [`PlayerStore`], a deliberately small trait,
//! so the services plug into the persistent store stream C is building (`progression/persist`)
//! without change. Two implementations live here:
//!
//! - [`MemoryStore`]: everything in memory, for tests and for shadowing a run.
//! - [`WireStore`]: the served objects themselves, the account inventory entries and the decoded
//!   profile (`0x80801743`) that `vendor.rs` already pushes. A change made through it is a change to
//!   the bytes the client receives. What those objects cannot hold (reputation, objective progress,
//!   refund receipts, counters) is kept beside them in a [`MemoryStore`].
//!
//! Inventory is addressed the way the client addresses it: by **bucket ordinal** (item definition
//! `+0x88`) and the slot within the bucket's range of the client's bucket table
//! (`vendor::BUCKETS`, read live from `investment_globals->vfunc[0x98]`). Scope 1 buckets belong to
//! the account (the vault, Glimmer, Silver, Legendary Marks), scope 0 buckets to a character.
//!
//! Services work on a clone and commit it whole (`*store = work`), so a refused request changes
//! nothing; stores are therefore `Clone`.

use std::collections::{BTreeMap, HashMap};

pub type Content = u16;
pub type Instance = u64;
pub type CharacterId = u64;

/// The empty content index of a slot.
pub const EMPTY: Content = 0xFFFF;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Owner {
    Account,
    Character(CharacterId),
}

/// One slot's contents: an instanced item (`instance != 0`, quantity 0 or 1) or a stack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub content: Content,
    pub instance: Instance,
    pub quantity: u32,
    /// Primary stat (Light / Attack / Defense). 0 = not known to this store. The served profile
    /// does not hold it: it lives in the item element (`0x80801732`), whose stat field is not
    /// decoded yet.
    pub light: u32,
}

impl Item {
    pub fn stack(content: Content, quantity: u32) -> Self {
        Item { content, instance: 0, quantity, light: 0 }
    }
    pub fn instanced(content: Content, instance: Instance, light: u32) -> Self {
        Item { content, instance, quantity: 1, light }
    }
    /// The quantity this slot counts for in a price check (an instanced item counts once).
    pub fn count(&self) -> u64 {
        if self.instance != 0 { u64::from(self.quantity.max(1)) } else { u64::from(self.quantity) }
    }
}

/// A Guardian: identity (wire order {race, gender, class}, `d1_bap::Identity`) and level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Character {
    pub id: CharacterId,
    /// 0 Human, 1 Awoken, 2 Exo (DestinyRace).
    pub race: u8,
    /// 0 Male, 1 Female (DestinyGender).
    pub gender: u8,
    /// 0 Titan, 1 Hunter, 2 Warlock (DestinyClass).
    pub class: u8,
    pub level: u32,
    /// The 501 request's customization block, kept as sent (`0x8080181A`).
    pub customization: Vec<u8>,
}

/// A refund receipt: what account field [20] (`0x8080174F`) shows the client, plus where the item
/// went. Only refundable sales get one (`vendors::refund`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Receipt {
    pub vendor: u8,
    pub sale: u16,
    pub item: Item,
    pub bucket: u8,
    /// `(content, quantity, bucket)` given back on a refund.
    pub costs: Vec<(Content, u32, u8)>,
    /// The client's stamp sent with the purchase (901's u64).
    pub bought: u64,
    pub expires: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    UnknownBucket(u8),
    SlotOutOfRange { bucket: u8, slot: usize },
    UnknownCharacter(CharacterId),
    /// The served objects cannot hold this (yet). Names what is missing.
    NotRepresentable(&'static str),
    Full(u8),
    Duplicate(CharacterId),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::UnknownBucket(b) => write!(f, "unknown bucket {b:#x}"),
            StoreError::SlotOutOfRange { bucket, slot } => write!(f, "slot {slot} is outside bucket {bucket:#x}"),
            StoreError::UnknownCharacter(c) => write!(f, "unknown character {c:#x}"),
            StoreError::NotRepresentable(w) => write!(f, "the served objects cannot hold {w} yet"),
            StoreError::Full(b) => write!(f, "bucket {b:#x} is full"),
            StoreError::Duplicate(c) => write!(f, "character {c:#x} exists"),
        }
    }
}

impl std::error::Error for StoreError {}

/// The state changes stream I needs, and nothing else. ⭐ The seam to stream C's persistent store.
pub trait PlayerStore {
    fn characters(&self) -> Vec<Character>;
    fn insert_character(&mut self, c: Character) -> Result<(), StoreError>;
    fn remove_character(&mut self, id: CharacterId) -> Result<(), StoreError>;
    /// A new server object id (item instances, characters).
    fn mint_soid(&mut self) -> u64;

    /// The bucket's slots, `None` = empty. Length = the bucket's capacity.
    fn slots(&self, owner: Owner, bucket: u8) -> Vec<Option<Item>>;
    fn set_slot(&mut self, owner: Owner, bucket: u8, slot: usize, item: Option<Item>) -> Result<(), StoreError>;

    /// The 20 equipped instances (profile `+0x2B80`); 0 = none.
    fn equipped(&self, chr: CharacterId) -> Vec<Instance>;
    fn set_equipped(&mut self, chr: CharacterId, index: usize, instance: Instance) -> Result<(), StoreError>;
    /// Change an item's primary stat (infusion).
    fn set_light(&mut self, owner: Owner, bucket: u8, slot: usize, light: u32) -> Result<(), StoreError>;

    /// Reputation and other progressions, by progression hash. (XP and ranks are stream C's.)
    fn progression(&self, chr: CharacterId, hash: u32) -> u32;
    fn add_progression(&mut self, chr: CharacterId, hash: u32, amount: u32);

    /// The profile's completion record (field [11]): one byte per sub; 2 = completed,
    /// 1 = in progress (`d1-profile-completions.py`).
    fn completion(&self, chr: CharacterId, sub: u16) -> u8;
    fn set_completion(&mut self, chr: CharacterId, sub: u16, value: u8);
    /// Unlock flags served as pairs (503/205 `0x80801941`), by master flag-table row.
    fn flag(&self, chr: CharacterId, row: u16) -> u8;
    fn set_flag(&mut self, chr: CharacterId, row: u16, value: u8);

    /// Objective progress (bounties, quests), by objective hash.
    fn objective(&self, chr: CharacterId, objective: u32) -> u32;
    fn set_objective(&mut self, chr: CharacterId, objective: u32, value: u32);

    fn receipts(&self, chr: CharacterId) -> Vec<Receipt>;
    fn set_receipts(&mut self, chr: CharacterId, receipts: Vec<Receipt>);

    /// Small named counters (Armsday orders this week, purchases this period).
    fn counter(&self, chr: CharacterId, key: &str) -> u64;
    fn set_counter(&mut self, chr: CharacterId, key: &str, value: u64);
}

/// One row of the client's bucket table, with the manifest bucket it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bucket {
    pub ordinal: u8,
    pub first: u16,
    pub count: u16,
    /// 0 = character (profile), 1 = account.
    pub scope: u8,
    pub name: &'static str,
}

/// Names by ordinal. Sources: `data/social/buckets.tsv` (each ordinal's items vote for a manifest
/// bucket) and the slot counts, which match the manifest's `itemCount`. ✓ = the vote and the count
/// agree; `~` = by elimination (no items vote, the count fits); `?` = still open.
const BUCKET_NAMES: &[(u8, &str)] = &[
    (0x01, "Consumables"), (0x02, "Primary Weapons"), (0x03, "Special Weapons"), (0x04, "Heavy Weapons"),
    (0x05, "Materials"), (0x06, "Subclass"), (0x07, "Helmet"), (0x08, "Gauntlets"), (0x09, "Chest Armor"),
    (0x0a, "Leg Armor"), (0x0b, "Class Armor"), (0x0c, "Ships"), (0x0d, "Vehicle"), (0x0e, "Ghost"),
    (0x0f, "Mission"), (0x10, "~Lost Items or Messages (Postmaster)"), (0x11, "?5-slot (manifest 1626737477)"),
    (0x12, "Hidden"), (0x13, "Temporary"), (0x14, "~Messages or Lost Items (Postmaster)"),
    (0x15, "~Special Orders (account)"), (0x16, "~Vault: Weapons"), (0x17, "~Vault: General"),
    (0x18, "~Vault: Armor"), (0x19, "Emblems"), (0x1a, "Bounties"), (0x1b, "Shaders"), (0x1c, "Emotes"),
    (0x1d, "Ornaments"), (0x1e, "~Sparrow Horn"), (0x1f, "Glimmer"), (0x20, "Vanguard Marks"),
    (0x21, "Crucible Marks"), (0x22, "Silver"), (0x23, "Legendary Marks"), (0x24, "Artifacts"),
    (0x25, "~Quests"), (0x26, "~Record Books"), (0x27, "~Legacy Record Books"),
];

pub fn bucket(ordinal: u8) -> Option<Bucket> {
    crate::vendor::BUCKETS.iter().find(|b| b.0 == ordinal).map(|&(o, first, count, scope)| Bucket {
        ordinal: o, first, count, scope,
        name: BUCKET_NAMES.iter().find(|n| n.0 == o).map(|n| n.1).unwrap_or("?"),
    })
}

/// Every bucket of one scope.
pub fn buckets(scope: u8) -> impl Iterator<Item = Bucket> {
    crate::vendor::BUCKETS.iter().filter(move |b| b.3 == scope).filter_map(|b| bucket(b.0))
}

/// The owner of a bucket for a character: the account for scope 1, else the character.
pub fn owner_of(ordinal: u8, chr: CharacterId) -> Owner {
    match bucket(ordinal) {
        Some(b) if b.scope == 1 => Owner::Account,
        _ => Owner::Character(chr),
    }
}

/// The vault buckets (account, scope 1) by their slot counts: 108 weapons, 72 general, 108 armour.
pub const VAULT_WEAPONS: u8 = 0x16;
pub const VAULT_GENERAL: u8 = 0x17;
pub const VAULT_ARMOR: u8 = 0x18;
/// The Postmaster's Lost Items. ⚠ 0x10 and 0x14 are both 20-slot character buckets with no item
/// voting for either; which is Lost Items and which Messages is open (see the note).
pub const LOST_ITEMS: u8 = 0x10;
pub const BOUNTIES: u8 = 0x1a;
/// One slot of reward pseudo-items ("Experience", "Of Iron Made"): granted, never kept.
pub const TEMPORARY: u8 = 0x13;
pub const QUESTS: u8 = 0x25;

// ── helpers over any store ─────────────────────────────────────────────────────────────────

/// What the client's price check `0x26D6E0` computes: the sum of the matching slots' quantities.
pub fn count<S: PlayerStore + ?Sized>(s: &S, owner: Owner, bucket: u8, content: Content) -> u64 {
    s.slots(owner, bucket).iter().flatten().filter(|i| i.content == content).map(Item::count).sum()
}

/// Take `quantity` of a stacked `content` from a bucket, across stacks, freeing used-up stacks.
pub fn take<S: PlayerStore + ?Sized>(s: &mut S, owner: Owner, bucket: u8, content: Content, quantity: u32) -> Result<(), String> {
    let have = count(s, owner, bucket, content);
    if have < u64::from(quantity) {
        return Err(format!("{content} (bucket {bucket:#x}): {have} < {quantity}"));
    }
    let mut owed = quantity;
    for (i, slot) in s.slots(owner, bucket).into_iter().enumerate() {
        if owed == 0 {
            break;
        }
        let Some(mut it) = slot else { continue };
        if it.content != content {
            continue;
        }
        let q = if it.instance != 0 { 1 } else { it.quantity };
        let t = owed.min(q);
        owed -= t;
        let left = q - t;
        let new = if left == 0 && (it.instance != 0 || !is_fixed_slot(bucket)) {
            None
        } else {
            it.quantity = left;
            Some(it)
        };
        s.set_slot(owner, bucket, i, new).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// One-slot currency buckets keep their slot (with quantity 0) when spent to nothing, as the
/// served account does for Glimmer.
fn is_fixed_slot(ordinal: u8) -> bool {
    bucket(ordinal).map(|b| b.count == 1).unwrap_or(false)
}

/// Put an item in a bucket: merge a stack into the same content below `max_stack` (0 = no limit),
/// else the first free slot. Returns the slot.
pub fn give<S: PlayerStore + ?Sized>(s: &mut S, owner: Owner, bucket: u8, item: Item, max_stack: u32) -> Result<usize, String> {
    let slots = s.slots(owner, bucket);
    if slots.is_empty() {
        return Err(format!("unknown bucket {bucket:#x}"));
    }
    if item.instance == 0 {
        for (i, slot) in slots.iter().enumerate() {
            if let Some(it) = slot {
                if it.content == item.content && it.instance == 0 && (max_stack == 0 || it.quantity + item.quantity <= max_stack) {
                    let mut m = it.clone();
                    m.quantity += item.quantity;
                    s.set_slot(owner, bucket, i, Some(m)).map_err(|e| e.to_string())?;
                    return Ok(i);
                }
            }
        }
    }
    let free = slots.iter().position(|x| x.is_none()).ok_or_else(|| format!("bucket {bucket:#x} is full"))?;
    s.set_slot(owner, bucket, free, Some(item)).map_err(|e| e.to_string())?;
    Ok(free)
}

/// Where an instance is: (owner, bucket, slot, item). Searches the character's buckets and the account's.
pub fn find_instance<S: PlayerStore + ?Sized>(s: &S, chr: CharacterId, instance: Instance) -> Option<(Owner, u8, usize, Item)> {
    if instance == 0 {
        return None;
    }
    for (owner, scope) in [(Owner::Character(chr), 0u8), (Owner::Account, 1u8)] {
        for b in buckets(scope) {
            for (i, it) in s.slots(owner, b.ordinal).into_iter().enumerate() {
                if let Some(it) = it {
                    if it.instance == instance {
                        return Some((owner, b.ordinal, i, it));
                    }
                }
            }
        }
    }
    None
}

/// The first slot of a bucket holding `content` (for stacks named by content index).
pub fn find_content<S: PlayerStore + ?Sized>(s: &S, owner: Owner, bucket: u8, content: Content) -> Option<(usize, Item)> {
    s.slots(owner, bucket).into_iter().enumerate().find_map(|(i, x)| x.filter(|it| it.content == content).map(|it| (i, it)))
}

// ── MemoryStore ────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default)]
pub struct MemoryStore {
    pub characters: Vec<Character>,
    pub inventory: HashMap<(Owner, u8), Vec<Option<Item>>>,
    pub equipped: HashMap<CharacterId, Vec<Instance>>,
    pub progressions: HashMap<(CharacterId, u32), u32>,
    pub completions: HashMap<(CharacterId, u16), u8>,
    pub flags: HashMap<(CharacterId, u16), u8>,
    pub objectives: HashMap<(CharacterId, u32), u32>,
    pub receipts: HashMap<CharacterId, Vec<Receipt>>,
    pub counters: HashMap<(CharacterId, String), u64>,
    pub next_soid: u64,
}

impl MemoryStore {
    pub fn new() -> Self {
        MemoryStore { next_soid: 0xD100_0000_0000_0A01, ..Default::default() }
    }
}

impl PlayerStore for MemoryStore {
    fn characters(&self) -> Vec<Character> {
        self.characters.clone()
    }
    fn insert_character(&mut self, c: Character) -> Result<(), StoreError> {
        if self.characters.iter().any(|x| x.id == c.id) {
            return Err(StoreError::Duplicate(c.id));
        }
        self.characters.push(c);
        Ok(())
    }
    fn remove_character(&mut self, id: CharacterId) -> Result<(), StoreError> {
        let n = self.characters.len();
        self.characters.retain(|c| c.id != id);
        if self.characters.len() == n {
            return Err(StoreError::UnknownCharacter(id));
        }
        self.inventory.retain(|k, _| k.0 != Owner::Character(id));
        self.equipped.remove(&id);
        Ok(())
    }
    fn mint_soid(&mut self) -> u64 {
        let s = self.next_soid;
        self.next_soid += 1;
        s
    }
    fn slots(&self, owner: Owner, b: u8) -> Vec<Option<Item>> {
        let Some(info) = bucket(b) else { return Vec::new() };
        self.inventory.get(&(owner, b)).cloned().unwrap_or_else(|| vec![None; info.count as usize])
    }
    fn set_slot(&mut self, owner: Owner, b: u8, slot: usize, item: Option<Item>) -> Result<(), StoreError> {
        let info = bucket(b).ok_or(StoreError::UnknownBucket(b))?;
        let v = self.inventory.entry((owner, b)).or_insert_with(|| vec![None; info.count as usize]);
        let s = v.get_mut(slot).ok_or(StoreError::SlotOutOfRange { bucket: b, slot })?;
        *s = item;
        Ok(())
    }
    fn equipped(&self, chr: CharacterId) -> Vec<Instance> {
        self.equipped.get(&chr).cloned().unwrap_or_else(|| vec![0; 20])
    }
    fn set_equipped(&mut self, chr: CharacterId, index: usize, instance: Instance) -> Result<(), StoreError> {
        let v = self.equipped.entry(chr).or_insert_with(|| vec![0; 20]);
        *v.get_mut(index).ok_or(StoreError::NotRepresentable("an equipped index past 20"))? = instance;
        Ok(())
    }
    fn set_light(&mut self, owner: Owner, b: u8, slot: usize, light: u32) -> Result<(), StoreError> {
        let mut s = self.slots(owner, b);
        let it = s.get_mut(slot).and_then(|x| x.as_mut()).ok_or(StoreError::SlotOutOfRange { bucket: b, slot })?;
        it.light = light;
        let it = it.clone();
        self.set_slot(owner, b, slot, Some(it))
    }
    fn progression(&self, chr: CharacterId, hash: u32) -> u32 {
        self.progressions.get(&(chr, hash)).copied().unwrap_or(0)
    }
    fn add_progression(&mut self, chr: CharacterId, hash: u32, amount: u32) {
        *self.progressions.entry((chr, hash)).or_default() += amount;
    }
    fn completion(&self, chr: CharacterId, sub: u16) -> u8 {
        self.completions.get(&(chr, sub)).copied().unwrap_or(0)
    }
    fn set_completion(&mut self, chr: CharacterId, sub: u16, value: u8) {
        self.completions.insert((chr, sub), value);
    }
    fn flag(&self, chr: CharacterId, row: u16) -> u8 {
        self.flags.get(&(chr, row)).copied().unwrap_or(0)
    }
    fn set_flag(&mut self, chr: CharacterId, row: u16, value: u8) {
        self.flags.insert((chr, row), value);
    }
    fn objective(&self, chr: CharacterId, objective: u32) -> u32 {
        self.objectives.get(&(chr, objective)).copied().unwrap_or(0)
    }
    fn set_objective(&mut self, chr: CharacterId, objective: u32, value: u32) {
        self.objectives.insert((chr, objective), value);
    }
    fn receipts(&self, chr: CharacterId) -> Vec<Receipt> {
        self.receipts.get(&chr).cloned().unwrap_or_default()
    }
    fn set_receipts(&mut self, chr: CharacterId, receipts: Vec<Receipt>) {
        self.receipts.insert(chr, receipts);
    }
    fn counter(&self, chr: CharacterId, key: &str) -> u64 {
        self.counters.get(&(chr, key.to_string())).copied().unwrap_or(0)
    }
    fn set_counter(&mut self, chr: CharacterId, key: &str, value: u64) {
        self.counters.insert((chr, key.to_string()), value);
    }
}

// ── WireStore: the served account entries and decoded profile ────────────────────────────

/// The profile's inventory slots (340 × 32 bytes from `+0x38`).
pub const PROFILE_SLOTS: usize = 0x38;
pub const PROFILE_SLOT_COUNT: usize = 340;
/// The 20 equipped SOIDs (`d1-oracle profile layout`).
pub const PROFILE_EQUIPPED: usize = 0x2B80;
/// The completion record on 56578: one byte per sub from `+0x53C0`, 2200 bytes
/// (`tools/ps4_setup/d1-profile-completions.py`).
pub const PROFILE_COMPLETIONS: usize = 0x53C0;
pub const PROFILE_COMPLETION_COUNT: usize = 2200;

/// The served state as `vendor.rs` keeps it: account entries `slot -> (content, quantity)` and their
/// instances, and the decoded profile object of the one served character.
#[derive(Clone, Debug)]
pub struct WireStore {
    pub character: Character,
    pub account: BTreeMap<u16, (u16, u32)>,
    pub account_instances: BTreeMap<u16, u64>,
    pub profile: Vec<u8>,
    pub next_soid: u64,
    /// What the served objects cannot hold yet.
    pub side: MemoryStore,
}

impl WireStore {
    fn profile_ok(&self) -> bool {
        self.profile.len() >= PROFILE_COMPLETIONS + PROFILE_COMPLETION_COUNT
    }
    fn is_me(&self, owner: Owner) -> bool {
        owner == Owner::Character(self.character.id)
    }
}

impl PlayerStore for WireStore {
    fn characters(&self) -> Vec<Character> {
        vec![self.character.clone()]
    }
    fn insert_character(&mut self, _: Character) -> Result<(), StoreError> {
        Err(StoreError::NotRepresentable("a second character (the served profile is one character)"))
    }
    fn remove_character(&mut self, _: CharacterId) -> Result<(), StoreError> {
        Err(StoreError::NotRepresentable("deleting the served character"))
    }
    fn mint_soid(&mut self) -> u64 {
        let s = self.next_soid;
        self.next_soid += 1;
        s
    }
    fn slots(&self, owner: Owner, b: u8) -> Vec<Option<Item>> {
        let Some(info) = bucket(b) else { return Vec::new() };
        let range = info.first..info.first + info.count;
        if info.scope == 1 && owner == Owner::Account {
            range.map(|s| {
                self.account.get(&s).filter(|e| e.0 != EMPTY).map(|&(c, q)| {
                    let instance = self.account_instances.get(&s).copied().unwrap_or(0);
                    Item { content: c, instance, quantity: if instance != 0 { 1 } else { q }, light: 0 }
                })
            }).collect()
        } else if info.scope == 0 && self.is_me(owner) && self.profile.len() >= PROFILE_SLOTS + 32 * PROFILE_SLOT_COUNT {
            range.map(|s| {
                let e = PROFILE_SLOTS + 32 * s as usize;
                let c = u16::from_le_bytes([self.profile[e], self.profile[e + 1]]);
                (c != EMPTY).then(|| {
                    let instance = u64::from_le_bytes(self.profile[e + 8..e + 16].try_into().unwrap());
                    let q = u32::from_le_bytes(self.profile[e + 0x10..e + 0x14].try_into().unwrap());
                    // An instance is served with quantity 0; it counts as one.
                    Item { content: c, instance, quantity: if instance != 0 { 1 } else { q }, light: 0 }
                })
            }).collect()
        } else {
            vec![None; info.count as usize]
        }
    }
    fn set_slot(&mut self, owner: Owner, b: u8, slot: usize, item: Option<Item>) -> Result<(), StoreError> {
        let info = bucket(b).ok_or(StoreError::UnknownBucket(b))?;
        if slot >= info.count as usize {
            return Err(StoreError::SlotOutOfRange { bucket: b, slot });
        }
        let s = info.first + slot as u16;
        if info.scope == 1 {
            if owner != Owner::Account {
                return Err(StoreError::NotRepresentable("an account bucket owned by a character"));
            }
            match item {
                Some(it) => {
                    // As in the profile, an instance's entry carries quantity 0.
                    self.account.insert(s, (it.content, if it.instance != 0 { 0 } else { it.quantity }));
                    if it.instance != 0 {
                        self.account_instances.insert(s, it.instance);
                    } else {
                        self.account_instances.remove(&s);
                    }
                }
                None => {
                    self.account.remove(&s);
                    self.account_instances.remove(&s);
                }
            }
            return Ok(());
        }
        if !self.is_me(owner) {
            return Err(StoreError::UnknownCharacter(match owner { Owner::Character(c) => c, Owner::Account => 0 }));
        }
        let e = PROFILE_SLOTS + 32 * s as usize;
        if self.profile.len() < e + 32 {
            return Err(StoreError::NotRepresentable("a profile this short"));
        }
        match item {
            Some(it) => {
                self.profile[e..e + 2].copy_from_slice(&it.content.to_le_bytes());
                self.profile[e + 8..e + 16].copy_from_slice(&it.instance.to_le_bytes());
                // An instanced item is served with quantity 0 (the equip notes); a stack with its count.
                let q = if it.instance != 0 { 0 } else { it.quantity };
                self.profile[e + 0x10..e + 0x14].copy_from_slice(&q.to_le_bytes());
            }
            None => {
                self.profile[e..e + 2].copy_from_slice(&EMPTY.to_le_bytes());
                self.profile[e + 8..e + 16].fill(0);
                self.profile[e + 0x10..e + 0x14].fill(0);
            }
        }
        Ok(())
    }
    fn equipped(&self, chr: CharacterId) -> Vec<Instance> {
        if chr != self.character.id || self.profile.len() < PROFILE_EQUIPPED + 160 {
            return vec![0; 20];
        }
        (0..20).map(|i| u64::from_le_bytes(self.profile[PROFILE_EQUIPPED + 8 * i..PROFILE_EQUIPPED + 8 * i + 8].try_into().unwrap())).collect()
    }
    fn set_equipped(&mut self, chr: CharacterId, index: usize, instance: Instance) -> Result<(), StoreError> {
        if chr != self.character.id {
            return Err(StoreError::UnknownCharacter(chr));
        }
        if index >= 20 || self.profile.len() < PROFILE_EQUIPPED + 160 {
            return Err(StoreError::NotRepresentable("that equipped index"));
        }
        let at = PROFILE_EQUIPPED + 8 * index;
        self.profile[at..at + 8].copy_from_slice(&instance.to_le_bytes());
        Ok(())
    }
    fn set_light(&mut self, _: Owner, _: u8, _: usize, _: u32) -> Result<(), StoreError> {
        Err(StoreError::NotRepresentable("an item's Light (the item element's stat field is not decoded)"))
    }
    fn progression(&self, chr: CharacterId, hash: u32) -> u32 {
        self.side.progression(chr, hash)
    }
    fn add_progression(&mut self, chr: CharacterId, hash: u32, amount: u32) {
        self.side.add_progression(chr, hash, amount)
    }
    fn completion(&self, chr: CharacterId, sub: u16) -> u8 {
        if chr == self.character.id && self.profile_ok() && (sub as usize) < PROFILE_COMPLETION_COUNT {
            self.profile[PROFILE_COMPLETIONS + sub as usize]
        } else {
            self.side.completion(chr, sub)
        }
    }
    fn set_completion(&mut self, chr: CharacterId, sub: u16, value: u8) {
        if chr == self.character.id && self.profile_ok() && (sub as usize) < PROFILE_COMPLETION_COUNT {
            self.profile[PROFILE_COMPLETIONS + sub as usize] = value;
        } else {
            self.side.set_completion(chr, sub, value)
        }
    }
    fn flag(&self, chr: CharacterId, row: u16) -> u8 {
        self.side.flag(chr, row)
    }
    fn set_flag(&mut self, chr: CharacterId, row: u16, value: u8) {
        self.side.set_flag(chr, row, value)
    }
    fn objective(&self, chr: CharacterId, objective: u32) -> u32 {
        self.side.objective(chr, objective)
    }
    fn set_objective(&mut self, chr: CharacterId, objective: u32, value: u32) {
        self.side.set_objective(chr, objective, value)
    }
    fn receipts(&self, chr: CharacterId) -> Vec<Receipt> {
        self.side.receipts(chr)
    }
    fn set_receipts(&mut self, chr: CharacterId, receipts: Vec<Receipt>) {
        self.side.set_receipts(chr, receipts)
    }
    fn counter(&self, chr: CharacterId, key: &str) -> u64 {
        self.side.counter(chr, key)
    }
    fn set_counter(&mut self, chr: CharacterId, key: &str, value: u64) {
        self.side.set_counter(chr, key, value)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub const CHR: CharacterId = 0xD100_0000_0000_0101;

    pub fn titan() -> Character {
        Character { id: CHR, race: 0, gender: 1, class: 0, level: 40, customization: Vec::new() }
    }

    pub fn memory() -> MemoryStore {
        let mut s = MemoryStore::new();
        s.insert_character(titan()).unwrap();
        s
    }

    pub fn empty_profile() -> Vec<u8> {
        let mut p = vec![0u8; 29_560];
        for s in 0..PROFILE_SLOT_COUNT {
            p[PROFILE_SLOTS + 32 * s..PROFILE_SLOTS + 32 * s + 2].copy_from_slice(&EMPTY.to_le_bytes());
        }
        p
    }

    pub fn wire() -> WireStore {
        WireStore { character: titan(), account: BTreeMap::new(), account_instances: BTreeMap::new(),
                    profile: empty_profile(), next_soid: 0xD100_0000_0000_0A01, side: MemoryStore::new() }
    }

    #[test]
    fn the_bucket_table_names_the_known_rows() {
        assert_eq!(bucket(0x1f).map(|b| (b.first, b.count, b.scope, b.name)), Some((308, 1, 1, "Glimmer")));
        assert_eq!(bucket(0x02).map(|b| (b.first, b.count, b.name)), Some((93, 10, "Primary Weapons")));
        assert_eq!(bucket(VAULT_WEAPONS).map(|b| (b.count, b.scope)), Some((108, 1)));
        assert_eq!(bucket(VAULT_GENERAL).map(|b| b.count), Some(72));
        assert_eq!(bucket(VAULT_ARMOR).map(|b| b.count), Some(108));
        assert_eq!(bucket(BOUNTIES).map(|b| b.count), Some(16), "the manifest's Bounties hold 16");
        assert_eq!(bucket(QUESTS).map(|b| b.count), Some(32), "the manifest's Quests hold 32");
        assert!(bucket(0x40).is_none());
        assert_eq!(crate::vendor::BUCKETS.len(), 39);
        assert!(crate::vendor::BUCKETS.iter().all(|b| BUCKET_NAMES.iter().any(|n| n.0 == b.0)), "every row is named");
    }

    /// The same sequence on both stores gives the same view: the wire store's bytes behave as a store.
    #[test]
    fn memory_and_wire_stores_agree() {
        fn run<S: PlayerStore>(s: &mut S) -> (u64, u64, Vec<Option<Item>>) {
            give(s, Owner::Account, 0x1f, Item::stack(869, 1000), 0).unwrap();
            take(s, Owner::Account, 0x1f, 869, 400).unwrap();
            assert!(take(s, Owner::Account, 0x1f, 869, 601).is_err());
            give(s, Owner::Character(CHR), 0x05, Item::stack(872, 20), 200).unwrap();
            give(s, Owner::Character(CHR), 0x05, Item::stack(872, 5), 200).unwrap();
            give(s, Owner::Character(CHR), 0x02, Item::instanced(3598, 0x30b, 0), 1).unwrap();
            (count(s, Owner::Account, 0x1f, 869), count(s, Owner::Character(CHR), 0x05, 872), s.slots(Owner::Character(CHR), 0x02))
        }
        let (mut m, mut w) = (memory(), wire());
        let a = run(&mut m);
        let b = run(&mut w);
        assert_eq!(a.0, 600);
        assert_eq!(a.1, 25, "the second stack merged into the first");
        assert_eq!(a, b);
        // The wire store wrote the profile bytes where the client reads them.
        let e = PROFILE_SLOTS + 32 * 93;
        assert_eq!(&w.profile[e..e + 2], &3598u16.to_le_bytes());
        assert_eq!(&w.profile[e + 0x10..e + 0x14], &0u32.to_le_bytes(), "an instance is served with quantity 0");
        assert_eq!(w.account[&308], (869, 600));
        assert_eq!(find_instance(&w, CHR, 0x30b).map(|x| (x.1, x.2)), Some((0x02, 0)));
    }

    #[test]
    fn a_spent_currency_keeps_its_slot_and_a_spent_stack_is_freed() {
        let mut s = memory();
        give(&mut s, Owner::Account, 0x1f, Item::stack(869, 10), 0).unwrap();
        take(&mut s, Owner::Account, 0x1f, 869, 10).unwrap();
        assert_eq!(s.slots(Owner::Account, 0x1f)[0], Some(Item::stack(869, 0)));
        give(&mut s, Owner::Character(CHR), 0x05, Item::stack(872, 3), 0).unwrap();
        take(&mut s, Owner::Character(CHR), 0x05, 872, 3).unwrap();
        assert_eq!(s.slots(Owner::Character(CHR), 0x05)[0], None);
    }

    #[test]
    fn the_wire_completion_record_is_the_profile_bytes() {
        let mut w = wire();
        w.set_completion(CHR, 42, 2);
        assert_eq!(w.profile[PROFILE_COMPLETIONS + 42], 2);
        assert_eq!(w.completion(CHR, 42), 2);
        assert!(w.set_light(Owner::Character(CHR), 0x02, 0, 300).is_err(), "Light is not in the served objects");
    }
}
