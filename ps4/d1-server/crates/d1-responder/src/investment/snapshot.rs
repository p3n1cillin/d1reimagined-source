// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The state one investment exchange works on: the decoded profile (`0x80801743`), the account's
//! slots, the item elements it touches (`0x80801732`, decoded) and the character's `investment.*`
//! attributes. A handler edits a [`Snapshot`] and logs each object it changed, in order; the
//! responder then commits it to the player store and pushes those objects in that order
//! (`store_io`). A refused request leaves the snapshot unused, so it changes nothing.
//!
//! Addresses are the client's: a profile slot entry is `0x808016E0` (32 B at `0x38 + 32 * slot`:
//! content index `+0`, instance `+8`, quantity `+0x10`, **state flags `+0x18`**), the equipped
//! instances are 20 × u64 at `+0x2B80`, and an item's bucket is its slot range
//! (`vendor::BUCKETS`). `+0x18` is the flags word: the item request handler `0xA29D30` reads it
//! back into the 406 request (actions 8/9 set or clear bit 0, 11/12 bit 1), and the infusion check
//! `0x26F260` refuses a source whose bit 0 is set.

use std::collections::BTreeMap;

use serde_json::Value;

pub const EMPTY: u16 = 0xFFFF;
const SLOT_BYTES: usize = 32;
/// Profile slots the character buckets use (`vendor::BUCKETS`' scope-0 rows end below 340).
pub const PROFILE_SLOTS: u16 = 340;
/// The equipped instances, 20 × u64 (`d1-oracle profile layout`).
pub const EQUIPPED: usize = 0x2B80;
pub const EQUIPPED_COUNT: usize = 20;

/// What changed, in the order the push carries it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Touch {
    Account,
    Profile,
    Item(u64),
    Deleted(u64),
}

/// Where an instance is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// A character bucket: the profile's slot.
    Profile { slot: u16, bucket: u8 },
    /// An account bucket (the vault): the account's slot.
    Account { slot: u16, bucket: u8 },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub account_soid: u64,
    pub character: u64,
    pub profile_soid: u64,
    /// 0 Titan, 1 Hunter, 2 Warlock.
    pub class_type: u32,
    pub profile: Vec<u8>,
    pub account: BTreeMap<u16, (u16, u32)>,
    pub account_instances: BTreeMap<u16, u64>,
    /// Decoded item elements by instance.
    pub elements: BTreeMap<u64, Vec<u8>>,
    /// The character's `investment.*` attributes (kept by the player store).
    pub attributes: BTreeMap<String, Value>,
    /// Instance SOIDs reserved from the store for this exchange (new items take them in order).
    pub minted: Vec<u64>,
    /// Reputation `(progress, rank)` by progression name, as the store keeps it (904 reads it).
    pub reputation: BTreeMap<String, (u64, u32)>,
    /// Bounty records the store keeps, by item hash: `(objective progress, completed)`.
    pub bounties: BTreeMap<u32, (Vec<u32>, bool)>,
    /// What the commit hands the store through its trait: reputation by progression name, and the
    /// bounty records turned in.
    pub reputation_grants: Vec<(String, u32)>,
    pub bounties_done: Vec<u32>,
    pub log: Vec<Touch>,
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    b.get(at..at + 2).map_or(EMPTY, |x| u16::from_le_bytes([x[0], x[1]]))
}
fn u32_at(b: &[u8], at: usize) -> u32 {
    b.get(at..at + 4).map_or(0, |x| u32::from_le_bytes(x.try_into().unwrap()))
}
fn u64_at(b: &[u8], at: usize) -> u64 {
    b.get(at..at + 8).map_or(0, |x| u64::from_le_bytes(x.try_into().unwrap()))
}

/// The scope-0 (character) bucket a profile slot belongs to.
pub fn profile_bucket(slot: u16) -> Option<u8> {
    crate::vendor::BUCKETS.iter().find(|b| b.3 == 0 && b.1 <= slot && slot < b.1 + b.2).map(|b| b.0)
}
pub fn account_bucket(slot: u16) -> Option<u8> {
    crate::vendor::BUCKETS.iter().find(|b| b.3 == 1 && b.1 <= slot && slot < b.1 + b.2).map(|b| b.0)
}

impl Snapshot {
    pub fn touch(&mut self, t: Touch) {
        if !self.log.contains(&t) {
            self.log.push(t);
        }
    }

    fn entry(slot: u16) -> usize {
        crate::vendor::PROFILE_SLOTS + SLOT_BYTES * slot as usize
    }

    fn slot_fits(&self, slot: u16) -> bool {
        Self::entry(slot) + SLOT_BYTES <= self.profile.len()
    }

    /// `(content, instance, quantity, flags)` of a profile slot.
    pub fn profile_slot(&self, slot: u16) -> (u16, u64, u32, u32) {
        let e = Self::entry(slot);
        (u16_at(&self.profile, e), u64_at(&self.profile, e + 8), u32_at(&self.profile, e + 0x10), u32_at(&self.profile, e + 0x18))
    }

    fn write_profile_slot(&mut self, slot: u16, content: u16, instance: u64, quantity: u32, flags: u32) {
        let e = Self::entry(slot);
        self.profile[e..e + 2].copy_from_slice(&content.to_le_bytes());
        self.profile[e + 8..e + 16].copy_from_slice(&instance.to_le_bytes());
        self.profile[e + 0x10..e + 0x14].copy_from_slice(&quantity.to_le_bytes());
        self.profile[e + 0x18..e + 0x1C].copy_from_slice(&flags.to_le_bytes());
        self.touch(Touch::Profile);
    }

    /// Where `instance` is: a profile slot first, then the account's.
    pub fn locate(&self, instance: u64) -> Option<Place> {
        if instance == 0 {
            return None;
        }
        if let Some(slot) = (0..PROFILE_SLOTS).filter(|s| self.slot_fits(*s)).find(|s| self.profile_slot(*s).1 == instance) {
            return Some(Place::Profile { slot, bucket: profile_bucket(slot)? });
        }
        let (&slot, _) = self.account_instances.iter().find(|(_, i)| **i == instance)?;
        Some(Place::Account { slot, bucket: account_bucket(slot)? })
    }

    /// The content index of the item an instance is.
    pub fn content_of(&self, instance: u64) -> Option<u16> {
        match self.locate(instance)? {
            Place::Profile { slot, .. } => Some(self.profile_slot(slot).0),
            Place::Account { slot, .. } => self.account.get(&slot).map(|e| e.0),
        }
    }

    pub fn equipped(&self) -> Vec<u64> {
        (0..EQUIPPED_COUNT).map(|i| u64_at(&self.profile, EQUIPPED + 8 * i)).filter(|i| *i != 0).collect()
    }

    pub fn is_equipped(&self, instance: u64) -> bool {
        self.equipped().contains(&instance)
    }

    /// The slot's state flags (bit 0: locked).
    pub fn flags(&self, instance: u64) -> Option<u32> {
        match self.locate(instance)? {
            Place::Profile { slot, .. } => Some(self.profile_slot(slot).3),
            // The account's slots keep no flags word in the player store (`Account.slots`).
            Place::Account { .. } => Some(0),
        }
    }

    pub fn set_profile_flags(&mut self, slot: u16, flags: u32) {
        let (c, i, q, _) = self.profile_slot(slot);
        self.write_profile_slot(slot, c, i, q, flags);
    }

    /// How much of a stackable content the character holds: the account's slots and the profile's.
    pub fn quantity(&self, content: u16) -> u32 {
        let account: u32 = self.account.values().filter(|e| e.0 == content).map(|e| e.1).sum();
        let profile: u32 = (0..PROFILE_SLOTS).filter(|s| self.slot_fits(*s)).map(|s| self.profile_slot(s))
            .filter(|e| e.0 == content && e.1 == 0).map(|e| e.2).sum();
        account.saturating_add(profile)
    }

    /// Take `n` of a stack, from the account's slots first, then the profile's.
    pub fn take(&mut self, content: u16, n: u32) -> Result<(), String> {
        let have = self.quantity(content);
        if have < n {
            return Err(format!("content {content}: {have} < {n}"));
        }
        let mut owed = n;
        let slots: Vec<u16> = self.account.iter().filter(|(_, e)| e.0 == content).map(|(s, _)| *s).collect();
        for s in slots {
            let e = self.account.get_mut(&s).unwrap();
            let t = owed.min(e.1);
            e.1 -= t;
            owed -= t;
            if t > 0 {
                self.touch(Touch::Account);
            }
        }
        let fits: Vec<u16> = (0..PROFILE_SLOTS).filter(|s| self.slot_fits(*s)).collect();
        for s in fits {
            if owed == 0 {
                break;
            }
            let (c, i, q, f) = self.profile_slot(s);
            if c != content || i != 0 {
                continue;
            }
            let t = owed.min(q);
            owed -= t;
            if q - t == 0 {
                self.write_profile_slot(s, EMPTY, 0, 0, 0);
            } else {
                self.write_profile_slot(s, c, 0, q - t, f);
            }
        }
        Ok(())
    }

    /// Give `n` of a stack into its client bucket (`content::bucket_of`): the slot already holding
    /// it, else the bucket's first free slot. `cap` is the stack's limit (amounts over it are lost).
    /// Returns what was given.
    pub fn give(&mut self, content: u16, n: u32, cap: u32) -> Result<u32, String> {
        let bucket = crate::content::bucket_of(content).ok_or_else(|| format!("content {content} has no client bucket"))?;
        let (first, count, scope) = crate::vendor::bucket(bucket).ok_or_else(|| format!("unknown bucket {bucket:#x}"))?;
        if scope == 1 {
            let slot = (first..first + count).find(|s| self.account.get(s).is_some_and(|e| e.0 == content))
                .or_else(|| (first..first + count).find(|s| !self.account.contains_key(s)))
                .ok_or_else(|| format!("account bucket {bucket:#x} is full"))?;
            let have = self.account.get(&slot).map_or(0, |e| e.1);
            let after = have.saturating_add(n).min(cap.max(have));
            self.account.insert(slot, (content, after));
            self.touch(Touch::Account);
            return Ok(after - have);
        }
        let range: Vec<u16> = (first..first + count).filter(|s| self.slot_fits(*s)).collect();
        let slot = range.iter().copied().find(|s| { let e = self.profile_slot(*s); e.0 == content && e.1 == 0 })
            .or_else(|| range.iter().copied().find(|s| self.profile_slot(*s).0 == EMPTY))
            .ok_or_else(|| format!("character bucket {bucket:#x} is full"))?;
        let (c, _, have, f) = self.profile_slot(slot);
        let have = if c == content { have } else { 0 };
        let after = have.saturating_add(n).min(cap.max(have));
        self.write_profile_slot(slot, content, 0, after, if c == content { f } else { 0 });
        Ok(after - have)
    }

    /// A new instance of `content` with `element` (its SOID is written at `+0`): the next reserved
    /// SOID, a free slot of its bucket. Returns the SOID.
    pub fn give_instance(&mut self, content: u16, mut element: Vec<u8>) -> Result<u64, String> {
        let bucket = crate::content::bucket_of(content).ok_or_else(|| format!("content {content} has no client bucket"))?;
        let (first, count, scope) = crate::vendor::bucket(bucket).ok_or_else(|| format!("unknown bucket {bucket:#x}"))?;
        if self.minted.is_empty() {
            return Err("no instance SOID was reserved".into());
        }
        let soid = self.minted[0];
        if scope == 1 {
            let slot = (first..first + count).find(|s| !self.account.contains_key(s)).ok_or_else(|| format!("account bucket {bucket:#x} is full"))?;
            self.account.insert(slot, (content, 1));
            self.account_instances.insert(slot, soid);
            self.touch(Touch::Account);
        } else {
            let slot = (first..first + count).filter(|s| self.slot_fits(*s)).find(|s| self.profile_slot(*s).0 == EMPTY)
                .ok_or_else(|| format!("character bucket {bucket:#x} is full"))?;
            self.write_profile_slot(slot, content, soid, 1, 0);
        }
        self.minted.remove(0);
        if element.len() >= element::CONTENT + 2 {
            element[0..8].copy_from_slice(&soid.to_le_bytes());
            element[element::CONTENT..element::CONTENT + 2].copy_from_slice(&content.to_le_bytes());
        }
        self.elements.insert(soid, element);
        self.touch(Touch::Item(soid));
        Ok(soid)
    }

    /// Destroy an instance: its slot freed, its element gone (pushed as a length-0 item object).
    pub fn remove_instance(&mut self, instance: u64) -> Result<(), String> {
        match self.locate(instance).ok_or_else(|| format!("instance {instance:#x} is in no slot"))? {
            Place::Profile { slot, .. } => self.write_profile_slot(slot, EMPTY, 0, 0, 0),
            Place::Account { slot, .. } => {
                self.account.remove(&slot);
                self.account_instances.remove(&slot);
                self.touch(Touch::Account);
            }
        }
        for i in 0..EQUIPPED_COUNT {
            if u64_at(&self.profile, EQUIPPED + 8 * i) == instance {
                self.profile[EQUIPPED + 8 * i..EQUIPPED + 8 * i + 8].fill(0);
            }
        }
        self.elements.remove(&instance);
        self.log.retain(|t| *t != Touch::Item(instance));
        // A deletion goes first, as the live pushes put a consumed engram or a dismantled item.
        self.log.insert(0, Touch::Deleted(instance));
        Ok(())
    }

    pub fn set_element(&mut self, instance: u64, element: Vec<u8>) {
        self.elements.insert(instance, element);
        self.touch(Touch::Item(instance));
    }

    pub fn attribute(&self, key: &str) -> Option<&Value> {
        self.attributes.get(key)
    }

    pub fn set_attribute(&mut self, key: &str, value: Value) {
        self.attributes.insert(key.to_string(), value);
    }
}

/// The client's item element (`0x80801732`, decoded) fields the services read and write.
pub mod element {
    pub const LEVEL: usize = 0x18;
    pub const CURVE_OFFSET: usize = 0x20;
    pub const CONTENT: usize = 0x24;
    pub const XP: usize = 0x38;
    pub const GRID: usize = 0x58;
    /// 36 node-state bytes, one per node.
    pub const NODE_STATES: usize = 0x5A;
    pub const NODE_STATE_COUNT: usize = 36;
    /// 8 step bytes, read as `byte % steps` by a random node (`0x285050`).
    pub const STEPS: usize = 0x7E;
    pub const STEP_COUNT: usize = 8;
    pub const BYTES: usize = 0xB8;

    pub fn i32_at(e: &[u8], at: usize) -> i32 {
        e.get(at..at + 4).map_or(0, |x| i32::from_le_bytes(x.try_into().unwrap()))
    }
    pub fn i16_at(e: &[u8], at: usize) -> i16 {
        e.get(at..at + 2).map_or(-1, |x| i16::from_le_bytes([x[0], x[1]]))
    }
    pub fn u16_at(e: &[u8], at: usize) -> u16 {
        i16_at(e, at) as u16
    }
    pub fn u32_at(e: &[u8], at: usize) -> u32 {
        i32_at(e, at) as u32
    }
    pub fn set_i32(e: &mut [u8], at: usize, v: i32) {
        e[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub const CHR: u64 = 0xD100_0000_0000_0101;

    /// An empty profile object of the served size, every slot empty.
    pub fn snapshot() -> Snapshot {
        let mut profile = vec![0u8; 29_560];
        for s in 0..PROFILE_SLOTS as usize {
            let e = crate::vendor::PROFILE_SLOTS + 32 * s;
            profile[e..e + 2].copy_from_slice(&[0xFF, 0xFF]);
        }
        Snapshot {
            account_soid: 0xD100_0000_0000_0001,
            character: CHR,
            profile_soid: 0xD100_0000_0000_0201,
            profile,
            minted: vec![0xD100_0000_0000_0901, 0xD100_0000_0000_0902],
            ..Snapshot::default()
        }
    }

    /// `content` as an instance in the first free slot of its bucket, with `element`.
    pub fn with_instance(s: &mut Snapshot, content: u16, soid: u64, element: Vec<u8>) {
        s.minted.insert(0, soid);
        s.give_instance(content, element).unwrap();
        s.log.clear();
    }

    pub fn equip(s: &mut Snapshot, index: usize, soid: u64) {
        s.profile[EQUIPPED + 8 * index..EQUIPPED + 8 * index + 8].copy_from_slice(&soid.to_le_bytes());
    }

    #[test]
    fn stacks_and_instances_go_to_their_buckets_and_come_back_out() {
        let mut s = snapshot();
        // Glimmer (869) is an account stack (slot 308); Weapon Parts a character material.
        let parts = crate::content::item_index(crate::rewards::currencies::WEAPON_PARTS).unwrap();
        assert_eq!(s.give(869, 30_000, 25_000).unwrap(), 25_000, "the cap holds");
        assert_eq!(s.give(parts, 12, 200).unwrap(), 12);
        assert_eq!(s.log, vec![Touch::Account, Touch::Profile]);
        s.take(869, 500).unwrap();
        s.take(parts, 10).unwrap();
        assert_eq!((s.quantity(869), s.quantity(parts)), (24_500, 2));
        assert!(s.take(parts, 3).is_err());
        let rifle = 3598;
        with_instance(&mut s, rifle, 0x401, vec![0; element::BYTES]);
        assert!(matches!(s.locate(0x401), Some(Place::Profile { bucket: 0x02, .. })));
        assert_eq!(element::u16_at(&s.elements[&0x401], element::CONTENT), rifle);
        equip(&mut s, 6, 0x401);
        s.remove_instance(0x401).unwrap();
        assert_eq!(s.locate(0x401), None);
        assert!(s.equipped().is_empty(), "a destroyed item is unequipped");
        assert_eq!(s.log.first(), Some(&Touch::Deleted(0x401)), "a deletion leads the push");
    }
}
