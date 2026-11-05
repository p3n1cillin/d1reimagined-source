// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Bounties and quests: pick up, progress, turn in.
//!
//! From the manifest (`data/social/bounties.tsv`): each bounty or quest step lists its objectives
//! (objective hash and completion value, e.g. "Quick Drops": 30 precision kills) and its rewards
//! (`values`: reward item and amount; most bounties give 2,500 Experience and 50 reputation).
//!
//! - **Pick up** is a 901 purchase of an "Available Bounties" sale (priced 0 Glimmer; decoded wire)
//!   into the Bounties bucket (16 slots, one of each), done by `vendors::purchase`.
//! - **Progress** comes from the activity host (kills, completions), through
//!   [`record_progress`]. Values stop at the completion value (the manifest's
//!   `allowValueChangeWhenCompleted` is false on bounties).
//! - **Turn in**: "Collect" sales (bucket 0x13 rewards priced in a finished quest item) are 901s
//!   too (`vendors::purchase`). Turning in an ordinary bounty is [`turn_in`]. ⚠ Its wire is not
//!   decoded: the client sends it from the bounty's own item action (402, or 904
//!   `vendor_interact`). To capture: hold a completed bounty in the Tower, turn it in at its vendor,
//!   and read the responder's `INVESTMENT message id` line and request bytes.
//! - A quest step turned in gives the questline's next step (`setItemHashes`) into Quests.
//!
//! Rewards: "Experience" and the "… Reputation" pseudo-items become progressions
//! ([`reward_progression`]). XP itself is stream C's (it owns levels); here it is added as a
//! progression keyed by the Experience item's hash, for stream C's store to apply.

use crate::store::{self, CharacterId, Item, Owner, PlayerStore};
use crate::tables::Tables;

/// The Experience reward item's hash, the key XP is added under.
pub const EXPERIENCE: u32 = 3006793116;

/// The progression a reward pseudo-item feeds (by the manifest names: LIKELY, a name match).
pub fn reward_progression(t: &Tables, content: u16) -> Option<u32> {
    let (hash, name) = t.reward_items.get(&content)?;
    Some(match name.as_str() {
        "Experience" => EXPERIENCE,
        "Vanguard Reputation" => 3233510749,  // faction_fotc_vanguard
        "Crucible Reputation" => 1357277120,  // faction_pvp
        "Cryptarch Reputation" => 529303302,  // faction_cryptarch
        _ => *hash,
    })
}

pub fn is_bounty(t: &Tables, content: u16) -> bool {
    t.bounties.contains_key(&content)
}

/// Are all of the item's objectives complete?
pub fn complete<S: PlayerStore + ?Sized>(s: &S, t: &Tables, chr: CharacterId, content: u16) -> bool {
    t.bounties.get(&content).map(|b| b.objectives.iter().all(|&(o, need)| s.objective(chr, o) >= need)).unwrap_or(false)
}

/// Add progress to an objective for every held bounty or quest step that has it. Returns the items
/// that became complete.
pub fn record_progress<S: PlayerStore + ?Sized>(s: &mut S, t: &Tables, chr: CharacterId, objective: u32, delta: u32) -> Vec<u16> {
    let held: Vec<u16> = [store::BOUNTIES, store::QUESTS].iter()
        .flat_map(|&b| s.slots(Owner::Character(chr), b).into_iter().flatten().map(|i| i.content)).collect();
    let need = held.iter().filter_map(|c| t.bounties.get(c)).flat_map(|b| b.objectives.iter())
        .filter(|(o, _)| *o == objective).map(|(_, n)| *n).max();
    let Some(need) = need else { return Vec::new() };
    let before: Vec<u16> = held.iter().copied().filter(|c| complete(s, t, chr, *c)).collect();
    let v = s.objective(chr, objective);
    s.set_objective(chr, objective, (v + delta).min(need));
    held.into_iter().filter(|c| !before.contains(c) && complete(s, t, chr, *c)).collect()
}

/// Grant rewards: progressions for pseudo-items, items into their bucket (or the Postmaster when
/// full). Returns a description.
pub fn grant<S: PlayerStore + ?Sized>(s: &mut S, t: &Tables, chr: CharacterId, rewards: &[(u16, u32)]) -> Result<String, String> {
    let mut out = Vec::new();
    for &(content, amount) in rewards {
        if let Some(p) = reward_progression(t, content) {
            s.add_progression(chr, p, amount);
            out.push(format!("{amount} progression {p}"));
            continue;
        }
        let Some(row) = t.items.get(&content).copied() else {
            out.push(format!("{amount} of {content} (not in the item table: skipped)"));
            continue;
        };
        let owner = store::owner_of(row.bucket, chr);
        for _ in 0..if row.instanced() { amount.max(1) } else { 1 } {
            let item = if row.instanced() { Item::instanced(content, s.mint_soid(), 0) } else { Item::stack(content, amount) };
            if store::give(s, owner, row.bucket, item.clone(), row.max_stack).is_err() {
                store::give(s, Owner::Character(chr), store::LOST_ITEMS, item, 0)?;
                out.push(format!("{content} to the Postmaster"));
            } else {
                out.push(format!("{content} x{}", if row.instanced() { 1 } else { amount }));
            }
        }
    }
    Ok(out.join(", "))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnedIn {
    pub content: u16,
    pub rewards: String,
    pub next: Option<u16>,
}

/// Turn in a completed bounty or quest step held by the character.
pub fn turn_in<S: PlayerStore + Clone>(s: &mut S, t: &Tables, chr: CharacterId, content: u16) -> Result<TurnedIn, String> {
    let b = t.bounties.get(&content).ok_or_else(|| format!("{content} is not a bounty or quest step"))?;
    let mut work = s.clone();
    let (bucket, slot) = [store::BOUNTIES, store::QUESTS].iter()
        .find_map(|&bk| store::find_content(&work, Owner::Character(chr), bk, content).map(|(i, _)| (bk, i)))
        .ok_or_else(|| format!("{content} is not held"))?;
    if !complete(&work, t, chr, content) {
        return Err(format!("{content}'s objectives are not complete"));
    }
    work.set_slot(Owner::Character(chr), bucket, slot, None).map_err(|e| e.to_string())?;
    for &(o, _) in &b.objectives {
        work.set_objective(chr, o, 0);
    }
    let rewards = grant(&mut work, t, chr, &b.rewards)?;
    if let Some(n) = b.next {
        let inst = work.mint_soid();
        store::give(&mut work, Owner::Character(chr), store::QUESTS, Item::instanced(n, inst, 0), 1)?;
    }
    *s = work;
    Ok(TurnedIn { content, rewards, next: b.next })
}

/// Abandon a bounty or quest (D1 let players drop them): the item goes, progress resets.
pub fn abandon<S: PlayerStore + Clone>(s: &mut S, t: &Tables, chr: CharacterId, content: u16) -> Result<(), String> {
    let mut work = s.clone();
    let (bucket, slot) = [store::BOUNTIES, store::QUESTS].iter()
        .find_map(|&bk| store::find_content(&work, Owner::Character(chr), bk, content).map(|(i, _)| (bk, i)))
        .ok_or_else(|| format!("{content} is not held"))?;
    work.set_slot(Owner::Character(chr), bucket, slot, None).map_err(|e| e.to_string())?;
    if let Some(b) = t.bounties.get(&content) {
        for &(o, _) in &b.objectives {
            work.set_objective(chr, o, 0);
        }
    }
    *s = work;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::{memory, CHR};

    /// "Quick Drops" (content 7616): 30 precision kills, 2,500 XP and 50 Vanguard reputation.
    #[test]
    fn quick_drops_progresses_and_turns_in_for_xp_and_vanguard_reputation() {
        let t = crate::tables::tables();
        let b = &t.bounties[&7616];
        assert_eq!(b.objectives, vec![(4167680434, 30)]);
        let mut s = memory();
        store::give(&mut s, Owner::Character(CHR), store::BOUNTIES, Item::instanced(7616, 0xB1, 0), 1).unwrap();
        assert!(turn_in(&mut s, t, CHR, 7616).is_err(), "not complete yet");
        assert!(record_progress(&mut s, t, CHR, 4167680434, 29).is_empty());
        assert_eq!(record_progress(&mut s, t, CHR, 4167680434, 5), vec![7616]);
        assert_eq!(s.objective(CHR, 4167680434), 30, "progress stops at the completion value");
        let done = turn_in(&mut s, t, CHR, 7616).unwrap();
        assert_eq!(done.next, None);
        assert_eq!(s.progression(CHR, EXPERIENCE), 2500);
        assert_eq!(s.progression(CHR, 3233510749), 50);
        assert!(store::find_content(&s, Owner::Character(CHR), store::BOUNTIES, 7616).is_none());
    }

    #[test]
    fn a_quest_step_gives_the_next_step() {
        let t = crate::tables::tables();
        let (&c, b) = t.bounties.iter().filter(|(_, b)| b.next.is_some() && !b.objectives.is_empty()).min_by_key(|(c, _)| **c).unwrap();
        let mut s = memory();
        store::give(&mut s, Owner::Character(CHR), store::QUESTS, Item::instanced(c, 0xC1, 0), 1).unwrap();
        for &(o, n) in &b.objectives {
            record_progress(&mut s, t, CHR, o, n);
        }
        let done = turn_in(&mut s, t, CHR, c).unwrap();
        let next = done.next.unwrap();
        assert!(store::find_content(&s, Owner::Character(CHR), store::QUESTS, next).is_some());
    }

    #[test]
    fn abandoning_resets_progress() {
        let t = crate::tables::tables();
        let mut s = memory();
        store::give(&mut s, Owner::Character(CHR), store::BOUNTIES, Item::instanced(7616, 0xB1, 0), 1).unwrap();
        record_progress(&mut s, t, CHR, 4167680434, 10);
        abandon(&mut s, t, CHR, 7616).unwrap();
        assert_eq!(s.objective(CHR, 4167680434), 0);
        assert!(abandon(&mut s, t, CHR, 7616).is_err());
    }
}
