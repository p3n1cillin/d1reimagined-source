// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Record Books (Rise of Iron's and the later ones) and quests with their step objectives.
//!
//! A record completes when all its objectives reach their `completionValue`
//! (`records.complete`). A book's rank is its progression level; how many points a completed
//! record adds is a placeholder (`records.book_points`). A book's rank rewards come from the
//! manifest (`records.rewards`) and are claimed once. A quest step completes when all its
//! objectives do; the next step of its questline (the questline's `setItemHashes` order) follows
//! (`quests.advance`).
//!
//! On the wire: Record Books and quest steps are items (the Record Books and Quests buckets), and
//! objective progress has not been decoded in a 56578 structure yet. Evidence needed: a profile
//! with a quest in progress, and the record-book screen's reads. Until then progress is
//! server-side only.

use anyhow::{bail, Result};

use super::currencies::{self, Source};
use super::item_instance::ItemInstance;
use super::placeholders as ph;
use super::store::{CharacterId, RewardStore};
use super::tables::tables;
use super::talent_grid::level_for_xp;
use super::Rng;

/// Move an objective by `delta` (never below 0 unless the objective allows it); returns the value.
pub fn advance_objective(store: &mut dyn RewardStore, character: CharacterId, objective: u32, delta: i64) -> Result<i64> {
    let Some(def) = tables().objectives.get(&objective) else { bail!("objective {objective} is not in the manifest table") };
    let mut v = store.objective(character, objective).saturating_add(delta);
    if !def.allow_negative {
        v = v.max(0);
    }
    store.set_objective(character, objective, v);
    Ok(v)
}

pub fn objective_complete(store: &dyn RewardStore, character: CharacterId, objective: u32) -> bool {
    tables().objectives.get(&objective).is_some_and(|d| store.objective(character, objective) >= d.completion_value)
}

pub fn record_complete(store: &dyn RewardStore, character: CharacterId, record: u32) -> bool {
    tables()
        .records
        .get(&record)
        .is_some_and(|objs| !objs.is_empty() && objs.iter().all(|o| objective_complete(store, character, *o)))
}

/// `(completed records, progression points, rank)` of a book for this character.
pub fn book_progress(store: &dyn RewardStore, character: CharacterId, book: u32) -> Result<(u32, u32, u32)> {
    let t = tables();
    let Some(b) = t.record_books.get(&book) else { bail!("record book {book} is not in the manifest table") };
    let done = b.records.iter().filter(|r| record_complete(store, character, **r)).count() as u32;
    let points = done * ph::BOOK_POINTS_PER_RECORD;
    let rank = t.progressions.get(&b.progression).map_or(0, |p| level_for_xp(p, points, 0));
    Ok((done, points, rank))
}

/// Grant every rank reward the book has reached and not yet given. Instanced rewards become new
/// instances; stackable ones go through the wallet caps. Returns `(item, quantity)` given, and the
/// rewards the item table cannot place (granted to nobody, for the caller to handle).
pub fn claim_book_rewards(store: &mut dyn RewardStore, character: CharacterId, book: u32, rng: &mut Rng) -> Result<(Vec<(u32, u32)>, Vec<(u32, u32)>)> {
    let t = tables();
    let (_, _, rank) = book_progress(store, character, book)?;
    let claimed = store.claimed(character);
    let mut given = Vec::new();
    let mut other = Vec::new();
    for (r, item, qty) in &t.record_books[&book].rewards {
        if *r > rank || claimed.contains(&(book, *r)) {
            continue;
        }
        store.set_claimed(character, book, *r);
        match t.items.get(item) {
            Some(d) if d.instanced => {
                for _ in 0..*qty {
                    let id = store.new_instance_id();
                    store.put_item(character, ItemInstance::roll(id, *item, 0, rng)?);
                }
                given.push((*item, *qty));
            }
            Some(_) => {
                let g = currencies::grant(store, character, *item, *qty, &Source::RecordBook);
                given.push((*item, g.granted));
            }
            None => other.push((*item, *qty)),
        }
    }
    Ok((given, other))
}

pub fn quest_step_complete(store: &dyn RewardStore, character: CharacterId, step: u32) -> bool {
    tables()
        .quests
        .get(&step)
        .is_some_and(|q| q.objectives.iter().all(|o| objective_complete(store, character, *o)))
}

/// The step after `step` in its questline, if any.
pub fn next_quest_step(step: u32) -> Option<u32> {
    let t = tables();
    let q = t.quests.get(&step)?;
    if q.questline == 0 || q.position < 0 {
        return None;
    }
    t.quests.values().find(|n| n.questline == q.questline && n.position == q.position + 1).map(|n| n.hash)
}

/// If `step` is complete, the step that replaces it (`Ok(None)`: complete and the questline ends).
/// Errors when the step is not complete.
pub fn advance_quest(store: &dyn RewardStore, character: CharacterId, step: u32) -> Result<Option<u32>> {
    if !tables().quests.contains_key(&step) {
        bail!("quest step {step} is not in the manifest table");
    }
    if !quest_step_complete(store, character, step) {
        bail!("quest step {step} is not complete");
    }
    Ok(next_quest_step(step))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewards::store::MemoryStore;

    const ROI_BOOK: u32 = 243_968_262;

    #[test]
    fn the_rise_of_iron_book_ranks_with_its_records() {
        let t = tables();
        let b = &t.record_books[&ROI_BOOK];
        assert_eq!(b.records.len() as u32, b.record_count);
        let mut s = MemoryStore::with_character(1, 2, 0, 40);
        assert_eq!(book_progress(&s, 2, ROI_BOOK).unwrap(), (0, 0, 1));
        let mut last_rank = 1;
        for r in &b.records {
            for o in &t.records[r] {
                let need = t.objectives[o].completion_value;
                advance_objective(&mut s, 2, *o, need).unwrap();
            }
            assert!(record_complete(&s, 2, *r));
            let (_, _, rank) = book_progress(&s, 2, ROI_BOOK).unwrap();
            assert!(rank >= last_rank);
            last_rank = rank;
        }
        let (done, _, rank) = book_progress(&s, 2, ROI_BOOK).unwrap();
        assert_eq!(done, b.record_count);
        let top = b.rewards.iter().map(|r| r.0).max().unwrap();
        assert!(rank >= top, "rank {rank} below the top reward rank {top}");
        let (given, other) = claim_book_rewards(&mut s, 2, ROI_BOOK, &mut Rng::new(1)).unwrap();
        assert_eq!(given.len() + other.len(), b.rewards.len());
        let (again, again_other) = claim_book_rewards(&mut s, 2, ROI_BOOK, &mut Rng::new(1)).unwrap();
        assert!(again.is_empty() && again_other.is_empty(), "rewards are claimed once");
    }

    #[test]
    fn objectives_do_not_go_negative() {
        let mut s = MemoryStore::with_character(1, 2, 0, 40);
        let o = tables().objectives.values().find(|o| !o.allow_negative && o.completion_value > 1).unwrap();
        assert_eq!(advance_objective(&mut s, 2, o.hash, -5).unwrap(), 0);
        assert!(!objective_complete(&s, 2, o.hash));
        advance_objective(&mut s, 2, o.hash, o.completion_value).unwrap();
        assert!(objective_complete(&s, 2, o.hash));
    }

    #[test]
    fn quests_advance_through_their_questline() {
        let t = tables();
        let first = t.quests.values().filter(|q| q.position == 0 && q.questline != 0 && next_quest_step(q.hash).is_some()).map(|q| q.hash).min().unwrap();
        let mut s = MemoryStore::with_character(1, 2, 0, 40);
        assert!(advance_quest(&s, 2, first).is_err(), "not complete yet");
        let mut step = first;
        let mut seen = 1;
        loop {
            for o in &t.quests[&step].objectives {
                advance_objective(&mut s, 2, *o, t.objectives[o].completion_value).unwrap();
            }
            match advance_quest(&s, 2, step).unwrap() {
                Some(next) => {
                    assert_eq!(t.quests[&next].position, t.quests[&step].position + 1);
                    step = next;
                    seen += 1;
                }
                None => break,
            }
            assert!(seen < 100);
        }
        assert!(seen >= 2);
    }
}
