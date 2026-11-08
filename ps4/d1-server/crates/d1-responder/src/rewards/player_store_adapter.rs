// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The reward services on stream C's persistent store (`player_store::PlayerStore`, #307).
//!
//! [`Adapter`] implements [`RewardStore`] over a borrowed `&mut dyn PlayerStore`. What C's store
//! keeps goes there:
//! - currencies and materials, through `currency` / `adjust_currency` by 56578 content index (the
//!   account's slots; an item with no slot, or no content index yet, stays in the overlay);
//! - character XP, through `add_xp` and C's curves, only with [`Adapter::xp_from_manifest`] (off by
//!   default: C's `activity-rewards.tsv` gives completion XP, and an activity must not get it twice);
//! - item instance SOIDs (`mint_item_soid`), and the item element when [`Adapter::element_template`]
//!   is set and [`write_element`] can place the content index;
//! - each Grimoire card's score (`add_grimoire_card`) and the character's Light (`set_light`, in
//!   [`Adapter::finish`]).
//!
//! What C's record has no field for yet (instances with their grids and rolls, period counters,
//! objectives, record-book claims) stays in [`Adapter::overlay`], a [`MemoryStore`].
//! `rewards::remote` saves all of it in the record's `attributes` after each grant and restores
//! it before the next (stream O2). With [`Adapter::deliver`] (the default) a stack or instance
//! whose content index and client bucket are known goes into that bucket's slots instead
//! (`PlayerStore::deliver_stack`, `deliver_instance`).
//!
//! In the responder:
//! ```text
//! player_store::with_store(|s| {
//!     let mut a = Adapter::new(s, &index, class_type);
//!     let g = rewards::grant_activity_rewards(&mut a, character, activity_hash, &outcome)?;
//!     a.finish("rewards: activity")?;
//!     Ok(g)
//! });
//! player_store::push_profile("rewards")?; // when XP changed the profile
//! ```

use std::collections::{BTreeSet, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::player_store::PlayerStore;

use super::grimoire::{card_score, GrimoireState};
use super::item_instance::{write_element, ClientIndex, ItemInstance};
use super::light;
use super::loot::CHARACTER_LEVEL;
use super::store::{AccountId, CharacterId, InstanceId, MemoryStore, Owner, Period, RewardStore};
use super::tables::tables;
use super::talent_grid::level_for_xp;

pub struct Adapter<'a> {
    pub store: &'a mut dyn PlayerStore,
    pub index: &'a dyn ClientIndex,
    pub class_type: u32,
    /// Give the manifest's "Experience" lines to C's `add_xp`. Off: they stay in the overlay.
    pub xp_from_manifest: bool,
    /// A decoded `0x80801732` element: new instances are written into C's items from it.
    pub element_template: Option<Vec<u8>>,
    pub overlay: MemoryStore,
    /// Items whose quantity lives in C's account slots.
    in_store: HashSet<u32>,
    /// What went to the overlay instead of C's store, and element fields left open.
    pub notes: Vec<String>,
    /// Stream O2: put stacks and new instances into their client buckets (`content::bucket_of`,
    /// `PlayerStore::deliver_stack` / `deliver_instance`), not only into a slot that already holds
    /// them. On by default; off is the first wiring pass's behaviour.
    pub deliver: bool,
    /// The instances delivered into a bucket by this adapter, in order: the caller pushes each.
    pub delivered: Vec<InstanceId>,
}

impl<'a> Adapter<'a> {
    pub fn new(store: &'a mut dyn PlayerStore, index: &'a dyn ClientIndex, class_type: u32) -> Adapter<'a> {
        let id = store.identity();
        let level = store.xp().and_then(|xp| {
            let p = tables().progressions.get(&CHARACTER_LEVEL)?;
            Some(level_for_xp(p, u32::try_from(xp).unwrap_or(u32::MAX), 0))
        });
        let mut overlay = MemoryStore::with_character(id.account, id.character, class_type, level.unwrap_or(0));
        overlay.now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let in_store = tables()
            .items
            .values()
            .filter(|d| d.max_stack > 1)
            .filter(|d| index.content_index(d.hash).is_some_and(|c| store.currency(c) > 0))
            .map(|d| d.hash)
            .collect();
        Adapter { store, index, class_type, xp_from_manifest: false, element_template: None, overlay, in_store, notes: Vec::new(), deliver: true, delivered: Vec::new() }
    }

    /// The client bucket `item` is delivered into, when it has a content index and one is known.
    pub fn bucket_for(&self, item: u32) -> Option<(u16, u8)> {
        let c = self.index.content_index(item)?;
        self.deliver.then(|| crate::content::bucket_of(c)).flatten().map(|b| (c, b))
    }

    /// Move stacks the overlay kept (an earlier grant's, restored from the record's attributes)
    /// into their client buckets, now that they have one. Returns how many moved.
    pub fn deliver_kept_stacks(&mut self) -> usize {
        let kept: Vec<((Owner, u32), u32)> = self.overlay.stacks.iter().filter(|(_, n)| **n > 0).map(|(k, n)| (*k, *n)).collect();
        let mut moved = 0;
        for ((owner, item), n) in kept {
            let Some((c, b)) = self.bucket_for(item) else { continue };
            match self.store.deliver_stack(c, b, i64::from(n)) {
                Ok(after) => {
                    self.overlay.stacks.remove(&(owner, item));
                    self.in_store.insert(item);
                    self.notes.push(format!("item {item} (content {c}): {n} kept from an earlier grant delivered to bucket {b:#x} (now {after})"));
                    moved += 1;
                }
                Err(e) => self.notes.push(format!("item {item} (content {c}): still kept in the overlay: {e:#}")),
            }
        }
        moved
    }

    /// After a grant: record the character's Light basis in C's store and mark the record dirty.
    pub fn finish(&mut self, what: &str) -> anyhow::Result<()> {
        let character = self.store.identity().character;
        let basis = light::basis_for(&*self, character);
        if basis > 0 {
            self.store.set_light(Some(basis));
        }
        self.store.mark_dirty(what);
        Ok(())
    }
}

impl RewardStore for Adapter<'_> {
    fn now(&self) -> u64 {
        self.overlay.now
    }
    fn account_of(&self, _character: CharacterId) -> AccountId {
        self.store.identity().account
    }
    fn class_type(&self, _character: CharacterId) -> u32 {
        self.class_type
    }
    fn character_level(&self, character: CharacterId) -> u32 {
        self.overlay.character_level(character)
    }

    fn quantity(&self, owner: Owner, item: u32) -> u32 {
        match self.index.content_index(item) {
            // Delivered items: what the client's buckets hold, plus what the overlay still keeps
            // (an amount whose delivery failed).
            Some(c) if self.bucket_for(item).is_some() => self.store.currency(c).saturating_add(self.overlay.quantity(owner, item)),
            Some(c) if self.in_store.contains(&item) => self.store.currency(c),
            _ => self.overlay.quantity(owner, item),
        }
    }
    fn set_quantity(&mut self, owner: Owner, item: u32, quantity: u32) {
        if let Some((c, b)) = self.bucket_for(item) {
            let kept = self.overlay.quantity(owner, item);
            let delta = i64::from(quantity) - i64::from(self.store.currency(c)) - i64::from(kept);
            if delta == 0 {
                return;
            }
            if kept == 0 {
                match self.store.deliver_stack(c, b, delta) {
                    Ok(_) => {
                        self.in_store.insert(item);
                        return;
                    }
                    Err(e) => self.notes.push(format!("item {item} (content {c}, bucket {b:#x}): {e:#}; kept in the overlay")),
                }
                self.overlay.set_quantity(owner, item, delta.max(0) as u32);
                return;
            }
            // The overlay keeps some (an earlier failed delivery): change that first, and take
            // any rest from the bucket.
            let left = i64::from(kept) + delta;
            self.overlay.set_quantity(owner, item, left.max(0) as u32);
            if left < 0 {
                if let Err(e) = self.store.deliver_stack(c, b, left) {
                    self.notes.push(format!("item {item} (content {c}): could not take {} from bucket {b:#x}: {e:#}", -left));
                }
            }
            return;
        }
        if let Some(c) = self.index.content_index(item) {
            let delta = i64::from(quantity) - i64::from(self.store.currency(c));
            if self.store.adjust_currency(c, delta).is_ok() {
                self.in_store.insert(item);
                return;
            }
            self.notes.push(format!("item {item} (content {c}): no account slot holds it; kept in the overlay"));
        } else {
            self.notes.push(format!("item {item}: no 56578 content index; kept in the overlay"));
        }
        self.overlay.set_quantity(owner, item, quantity);
    }

    fn new_instance_id(&mut self) -> InstanceId {
        self.store.mint_item_soid()
    }
    fn put_item(&mut self, character: CharacterId, item: ItemInstance) {
        if let Some(t) = &self.element_template {
            match write_element(t, &item, self.index) {
                Ok((element, open)) => {
                    match self.bucket_for(item.item_hash) {
                        Some((c, b)) => match self.store.deliver_instance(c, b, item.id, &element) {
                            Ok(slot) => {
                                self.delivered.push(item.id);
                                self.notes.push(format!("item {:#x} (content {c}) delivered to bucket {b:#x}, slot {slot}", item.id));
                            }
                            Err(e) => {
                                self.notes.push(format!("item {:#x} (content {c}): not placed in bucket {b:#x}: {e:#}; element kept", item.id));
                                self.store.put_item(item.id, &element);
                            }
                        },
                        None => self.store.put_item(item.id, &element),
                    }
                    if !open.is_empty() {
                        self.notes.push(format!("item {:#x}: left as the template has them: {}", item.id, open.join(", ")));
                    }
                }
                Err(e) => self.notes.push(format!("item {:#x}: not written to the store: {e}", item.id)),
            }
        }
        self.overlay.put_item(character, item);
    }
    fn item(&self, id: InstanceId) -> Option<&ItemInstance> {
        self.overlay.item(id)
    }
    fn item_mut(&mut self, id: InstanceId) -> Option<&mut ItemInstance> {
        self.overlay.item_mut(id)
    }
    fn take_item(&mut self, id: InstanceId) -> Option<ItemInstance> {
        self.store.remove_item(id);
        self.overlay.take_item(id)
    }
    fn items_of(&self, character: CharacterId) -> Vec<InstanceId> {
        self.overlay.items_of(character)
    }
    fn equipped(&self, character: CharacterId) -> Vec<InstanceId> {
        self.overlay.equipped(character)
    }

    fn counter(&self, owner: Owner, key: &str, period: Period) -> u32 {
        self.overlay.counter(owner, key, period)
    }
    fn add_counter(&mut self, owner: Owner, key: &str, period: Period, n: u32) {
        self.overlay.add_counter(owner, key, period, n)
    }

    fn progression(&self, character: CharacterId, progression: u32) -> u32 {
        match (progression, self.store.xp()) {
            (CHARACTER_LEVEL, Some(xp)) if self.xp_from_manifest => u32::try_from(xp).unwrap_or(u32::MAX),
            _ => self.overlay.progression(character, progression),
        }
    }
    fn add_progression(&mut self, character: CharacterId, progression: u32, amount: u32) {
        if progression == CHARACTER_LEVEL && self.xp_from_manifest {
            match self.store.add_xp(amount) {
                Ok(g) => {
                    self.notes.push(format!("XP +{amount} through the player store (level {} -> {}); push the profile", g.level_before, g.level_after));
                    return;
                }
                Err(e) => self.notes.push(format!("XP +{amount} kept in the overlay: {e}")),
            }
        }
        self.overlay.add_progression(character, progression, amount)
    }

    fn objective(&self, character: CharacterId, objective: u32) -> i64 {
        self.overlay.objective(character, objective)
    }
    fn set_objective(&mut self, character: CharacterId, objective: u32, value: i64) {
        self.overlay.set_objective(character, objective, value)
    }

    fn grimoire(&self, account: AccountId) -> GrimoireState {
        self.overlay.grimoire(account)
    }
    fn set_grimoire(&mut self, account: AccountId, state: GrimoireState) {
        for c in &tables().grimoire {
            let score = card_score(c, &state);
            if score > 0 {
                self.store.add_grimoire_card(c.id, score);
            }
        }
        self.overlay.set_grimoire(account, state)
    }

    fn claimed(&self, character: CharacterId) -> BTreeSet<(u32, u32)> {
        self.overlay.claimed(character)
    }
    fn set_claimed(&mut self, character: CharacterId, book: u32, rank: u32) {
        self.overlay.set_claimed(character, book, rank)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player_store::{Identity, Sources, Store};
    use crate::rewards::currencies::{self, Source, GLIMMER, LEGENDARY_MARKS};
    use crate::rewards::grimoire;

    /// Glimmer at content 869 (the responder's `D1_VENDOR_ACCOUNT_SLOTS` example, slot 308).
    struct Index;
    impl ClientIndex for Index {
        fn content_index(&self, item: u32) -> Option<u16> {
            (item == GLIMMER).then_some(869)
        }
        fn progression_ordinal(&self, _: u32) -> Option<u16> {
            None
        }
        fn grid_index(&self, _: u32) -> Option<u16> {
            None
        }
        fn level_for_primary(&self, _: u32, _: u32) -> Option<i32> {
            None
        }
        fn stat_position(&self, _: u32, _: u32) -> Option<usize> {
            None
        }
    }

    fn player_store() -> Store {
        let src = Sources { vendor_account_slots: Some("308:869:100".into()), ..Sources::default() };
        Store::in_memory(Identity::from_env(), &src)
    }

    #[test]
    fn currencies_go_to_the_player_stores_slots() {
        let mut s = player_store();
        let character = s.identity().character;
        {
            let mut a = Adapter::new(&mut s, &Index, 0);
            assert_eq!(a.quantity(Owner::Account(a.account_of(character)), GLIMMER), 100);
            let g = currencies::grant(&mut a, character, GLIMMER, 30_000, &Source::Other);
            assert_eq!(g.granted, 24_900, "the manifest cap applies on C's slots");
            // Legendary Marks have no content index here: the overlay keeps them.
            assert_eq!(currencies::grant(&mut a, character, LEGENDARY_MARKS, 10, &Source::Activity).granted, 10);
            assert!(a.notes.iter().any(|n| n.contains("no 56578 content index")));
            a.finish("test").unwrap();
        }
        assert_eq!(s.currency(869), 25_000);
    }

    #[test]
    fn grimoire_scores_and_instance_ids_come_from_the_player_store() {
        let mut s = player_store();
        let character = s.identity().character;
        let card = tables().grimoire.iter().find(|c| c.points > 0).unwrap();
        let id = {
            let mut a = Adapter::new(&mut s, &Index, 0);
            assert_eq!(grimoire::unlock_card(&mut a, character, card.id).unwrap(), card.points);
            a.new_instance_id()
        };
        assert_eq!(s.grimoire_score(), card.points);
        assert_ne!(s.mint_item_soid(), id, "ids are minted by the player store, once");
    }

    #[test]
    fn xp_stays_in_the_overlay_unless_asked() {
        let mut s = player_store();
        let character = s.identity().character;
        let mut a = Adapter::new(&mut s, &Index, 0);
        a.add_progression(character, CHARACTER_LEVEL, 2_500);
        assert_eq!(a.overlay.progression(character, CHARACTER_LEVEL), 2_500);
        a.xp_from_manifest = true;
        // This store has no decoded profile, so C's add_xp refuses and the overlay keeps it.
        a.add_progression(character, CHARACTER_LEVEL, 100);
        assert_eq!(a.overlay.progression(character, CHARACTER_LEVEL), 2_600);
        assert!(a.notes.iter().any(|n| n.contains("kept in the overlay")));
    }
}
