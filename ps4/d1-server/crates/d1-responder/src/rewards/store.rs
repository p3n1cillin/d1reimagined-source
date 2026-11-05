// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The state the reward services read and change, behind one small trait.
//!
//! [`RewardStore`] is all the reward and growth services ask of the player store. [`MemoryStore`]
//! keeps it in memory for tests and for the responder until stream C's persistent store
//! (`progression/persist`) implements the same trait. The services never encode anything; turning
//! a changed store into profile and account pushes is the store's (and the responder's) job, through
//! the existing `0x80801743` / `0x80801732` paths (see [`super::item_instance`]).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::grimoire::GrimoireState;
use super::item_instance::ItemInstance;

pub type AccountId = u64;
pub type CharacterId = u64;
pub type InstanceId = u64;

/// Who holds a stack: the account (shared by its characters) or one character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Owner {
    Account(AccountId),
    Character(CharacterId),
}

/// A reset period for a counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Period {
    Daily,
    Weekly,
    Forever,
}

/// What the reward services need from the player store.
pub trait RewardStore {
    /// Seconds since the Unix epoch, UTC. Counters reset by it ([`period_index`]).
    fn now(&self) -> u64;
    fn account_of(&self, character: CharacterId) -> AccountId;
    /// `classType`: 0 Titan, 1 Hunter, 2 Warlock.
    fn class_type(&self, character: CharacterId) -> u32;
    /// The character level (stream C owns its XP; this is read only here).
    fn character_level(&self, character: CharacterId) -> u32;

    fn quantity(&self, owner: Owner, item: u32) -> u32;
    fn set_quantity(&mut self, owner: Owner, item: u32, quantity: u32);

    /// A new, never-used item instance id.
    fn new_instance_id(&mut self) -> InstanceId;
    fn put_item(&mut self, character: CharacterId, item: ItemInstance);
    fn item(&self, id: InstanceId) -> Option<&ItemInstance>;
    fn item_mut(&mut self, id: InstanceId) -> Option<&mut ItemInstance>;
    fn take_item(&mut self, id: InstanceId) -> Option<ItemInstance>;
    /// Every instance the character holds (inventory and equipped, plus the account's vault).
    fn items_of(&self, character: CharacterId) -> Vec<InstanceId>;
    fn equipped(&self, character: CharacterId) -> Vec<InstanceId>;

    /// A counter that resets each `period` (the period index is part of the key).
    fn counter(&self, owner: Owner, key: &str, period: Period) -> u32;
    fn add_counter(&mut self, owner: Owner, key: &str, period: Period, n: u32);

    fn progression(&self, character: CharacterId, progression: u32) -> u32;
    fn add_progression(&mut self, character: CharacterId, progression: u32, amount: u32);

    fn objective(&self, character: CharacterId, objective: u32) -> i64;
    fn set_objective(&mut self, character: CharacterId, objective: u32, value: i64);

    fn grimoire(&self, account: AccountId) -> GrimoireState;
    fn set_grimoire(&mut self, account: AccountId, state: GrimoireState);

    /// Record-book rewards already claimed, as `(book, rank)`.
    fn claimed(&self, character: CharacterId) -> BTreeSet<(u32, u32)>;
    fn set_claimed(&mut self, character: CharacterId, book: u32, rank: u32);
}

/// Unix time of the first daily reset after the epoch: 1 January 1970 09:00 UTC.
const DAILY_EPOCH: u64 = 9 * 3600;
/// The first weekly reset after the epoch: Tuesday 6 January 1970 09:00 UTC (the epoch is a Thursday).
const WEEKLY_EPOCH: u64 = 5 * 86_400 + 9 * 3600;

/// The index of the reset period `now` falls in. Destiny 1's resets are 09:00 UTC daily and
/// Tuesday 09:00 UTC weekly (the vendor reset offsets in Bungie's manifest; research record,
/// 5 November 2026). Times before the first reset count as period 0.
pub fn period_index(period: Period, now: u64) -> u64 {
    match period {
        Period::Daily => now.saturating_sub(DAILY_EPOCH) / 86_400 + u64::from(now >= DAILY_EPOCH),
        Period::Weekly => now.saturating_sub(WEEKLY_EPOCH) / 604_800 + u64::from(now >= WEEKLY_EPOCH),
        Period::Forever => 0,
    }
}

#[derive(Debug, Clone, Default)]
pub struct MemoryCharacter {
    pub account: AccountId,
    pub class_type: u32,
    pub level: u32,
    pub items: BTreeSet<InstanceId>,
    pub equipped: BTreeSet<InstanceId>,
}

/// The in-memory [`RewardStore`].
#[derive(Debug, Clone, Default)]
pub struct MemoryStore {
    pub now: u64,
    pub characters: BTreeMap<CharacterId, MemoryCharacter>,
    pub stacks: HashMap<(Owner, u32), u32>,
    pub instances: BTreeMap<InstanceId, ItemInstance>,
    pub next_instance: InstanceId,
    pub counters: HashMap<(Owner, String, Period, u64), u32>,
    pub progressions: HashMap<(CharacterId, u32), u32>,
    pub objectives: HashMap<(CharacterId, u32), i64>,
    pub grimoire: HashMap<AccountId, GrimoireState>,
    pub claimed: HashMap<CharacterId, BTreeSet<(u32, u32)>>,
}

impl MemoryStore {
    /// A store with one character. Instance ids start at the responder's item SOID base
    /// (`D1_ITEM_SOID_BASE`'s default) plus `0x1000`, clear of the served fixtures.
    pub fn with_character(account: AccountId, character: CharacterId, class_type: u32, level: u32) -> MemoryStore {
        let mut s = MemoryStore { next_instance: 0xD100_0000_0000_1301, ..MemoryStore::default() };
        s.characters.insert(character, MemoryCharacter { account, class_type, level, ..Default::default() });
        s
    }

    pub fn equip(&mut self, character: CharacterId, id: InstanceId) {
        if let Some(c) = self.characters.get_mut(&character) {
            c.items.insert(id);
            c.equipped.insert(id);
        }
    }
}

impl RewardStore for MemoryStore {
    fn now(&self) -> u64 {
        self.now
    }
    fn account_of(&self, character: CharacterId) -> AccountId {
        self.characters.get(&character).map_or(0, |c| c.account)
    }
    fn class_type(&self, character: CharacterId) -> u32 {
        self.characters.get(&character).map_or(3, |c| c.class_type)
    }
    fn character_level(&self, character: CharacterId) -> u32 {
        self.characters.get(&character).map_or(0, |c| c.level)
    }
    fn quantity(&self, owner: Owner, item: u32) -> u32 {
        self.stacks.get(&(owner, item)).copied().unwrap_or(0)
    }
    fn set_quantity(&mut self, owner: Owner, item: u32, quantity: u32) {
        self.stacks.insert((owner, item), quantity);
    }
    fn new_instance_id(&mut self) -> InstanceId {
        let id = self.next_instance;
        self.next_instance += 1;
        id
    }
    fn put_item(&mut self, character: CharacterId, item: ItemInstance) {
        if let Some(c) = self.characters.get_mut(&character) {
            c.items.insert(item.id);
        }
        self.instances.insert(item.id, item);
    }
    fn item(&self, id: InstanceId) -> Option<&ItemInstance> {
        self.instances.get(&id)
    }
    fn item_mut(&mut self, id: InstanceId) -> Option<&mut ItemInstance> {
        self.instances.get_mut(&id)
    }
    fn take_item(&mut self, id: InstanceId) -> Option<ItemInstance> {
        for c in self.characters.values_mut() {
            c.items.remove(&id);
            c.equipped.remove(&id);
        }
        self.instances.remove(&id)
    }
    fn items_of(&self, character: CharacterId) -> Vec<InstanceId> {
        self.characters.get(&character).map(|c| c.items.iter().copied().collect()).unwrap_or_default()
    }
    fn equipped(&self, character: CharacterId) -> Vec<InstanceId> {
        self.characters.get(&character).map(|c| c.equipped.iter().copied().collect()).unwrap_or_default()
    }
    fn counter(&self, owner: Owner, key: &str, period: Period) -> u32 {
        let p = period_index(period, self.now);
        self.counters.get(&(owner, key.to_string(), period, p)).copied().unwrap_or(0)
    }
    fn add_counter(&mut self, owner: Owner, key: &str, period: Period, n: u32) {
        let p = period_index(period, self.now);
        *self.counters.entry((owner, key.to_string(), period, p)).or_default() += n;
    }
    fn progression(&self, character: CharacterId, progression: u32) -> u32 {
        self.progressions.get(&(character, progression)).copied().unwrap_or(0)
    }
    fn add_progression(&mut self, character: CharacterId, progression: u32, amount: u32) {
        let v = self.progressions.entry((character, progression)).or_default();
        *v = v.saturating_add(amount);
    }
    fn objective(&self, character: CharacterId, objective: u32) -> i64 {
        self.objectives.get(&(character, objective)).copied().unwrap_or(0)
    }
    fn set_objective(&mut self, character: CharacterId, objective: u32, value: i64) {
        self.objectives.insert((character, objective), value);
    }
    fn grimoire(&self, account: AccountId) -> GrimoireState {
        self.grimoire.get(&account).cloned().unwrap_or_default()
    }
    fn set_grimoire(&mut self, account: AccountId, state: GrimoireState) {
        self.grimoire.insert(account, state);
    }
    fn claimed(&self, character: CharacterId) -> BTreeSet<(u32, u32)> {
        self.claimed.get(&character).cloned().unwrap_or_default()
    }
    fn set_claimed(&mut self, character: CharacterId, book: u32, rank: u32) {
        self.claimed.entry(character).or_default().insert((book, rank));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekly_reset_is_tuesday_0900_utc() {
        // Tuesday 1 November 2016 09:00:00 UTC = 1477990800.
        let reset = 1_477_990_800;
        assert_eq!(period_index(Period::Weekly, reset - 1) + 1, period_index(Period::Weekly, reset));
        assert_eq!(period_index(Period::Weekly, reset), period_index(Period::Weekly, reset + 6 * 86_400 + 86_399));
        assert_eq!(period_index(Period::Daily, reset - 1) + 1, period_index(Period::Daily, reset));
        assert_eq!(period_index(Period::Daily, reset), period_index(Period::Daily, reset + 86_399));
    }

    #[test]
    fn counters_reset_with_their_period() {
        let mut s = MemoryStore::with_character(1, 2, 0, 40);
        s.now = 1_477_990_800;
        let o = Owner::Account(1);
        s.add_counter(o, "k", Period::Weekly, 3);
        assert_eq!(s.counter(o, "k", Period::Weekly), 3);
        s.now += 7 * 86_400;
        assert_eq!(s.counter(o, "k", Period::Weekly), 0);
    }
}
