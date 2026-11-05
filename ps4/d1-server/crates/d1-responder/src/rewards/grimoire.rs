// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The Grimoire: cards, stat cards and the score.
//!
//! The score is the points of every unlocked card plus the points of every stat rank reached
//! (`grimoire.score`); with all 834 cards and every rank it is the manifest's total, 5,610. What
//! unlocks a card or counts toward a stat was the server's business and is not public
//! (`grimoire.triggers`): callers say so explicitly with [`unlock_card`] and [`add_stat`].
//!
//! On the wire: the Grimoire is account-wide (it lives with the account, not a character), but no
//! 56578 structure that carries cards or the score has been decoded yet. Evidence needed: the
//! account object's fields read by the Grimoire screen, or a live capture of an account with
//! cards. Until then the state is server-side only.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, Result};

use super::store::{CharacterId, RewardStore};
use super::tables::{tables, GrimoireCardDef};

/// An account's Grimoire.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GrimoireState {
    pub cards: BTreeSet<u32>,
    /// `(card, stat number)` -> value.
    pub stats: BTreeMap<(u32, u32), u64>,
}

fn card(id: u32) -> Option<&'static GrimoireCardDef> {
    tables().grimoire.iter().find(|c| c.id == id)
}

/// Points one card gives in `state`: its own points if unlocked, plus every stat rank reached.
pub fn card_score(c: &GrimoireCardDef, state: &GrimoireState) -> u32 {
    let own = if state.cards.contains(&c.id) { c.points } else { 0 };
    let ranks: u32 = c
        .stats
        .iter()
        .map(|(n, ranks)| {
            let v = state.stats.get(&(c.id, *n)).copied().unwrap_or(0);
            ranks.iter().filter(|(t, _)| v >= *t).map(|(_, p)| p).sum::<u32>()
        })
        .sum();
    own + ranks
}

/// The Grimoire score of `state`.
pub fn score(state: &GrimoireState) -> u32 {
    tables().grimoire.iter().map(|c| card_score(c, state)).sum()
}

/// The highest score there is: every card and every rank.
pub fn max_score() -> u32 {
    tables().grimoire.iter().map(|c| c.points + c.stats.iter().flat_map(|(_, r)| r.iter().map(|(_, p)| p)).sum::<u32>()).sum()
}

/// Unlock a card for the character's account; returns the points it added.
pub fn unlock_card(store: &mut dyn RewardStore, character: CharacterId, id: u32) -> Result<u32> {
    let Some(c) = card(id) else { bail!("Grimoire card {id} is not in the manifest table") };
    let account = store.account_of(character);
    let mut s = store.grimoire(account);
    let before = card_score(c, &s);
    s.cards.insert(id);
    let after = card_score(c, &s);
    store.set_grimoire(account, s);
    Ok(after - before)
}

/// Count `delta` toward a stat card's stat; returns the points the ranks reached added.
pub fn add_stat(store: &mut dyn RewardStore, character: CharacterId, id: u32, stat: u32, delta: u64) -> Result<u32> {
    let Some(c) = card(id) else { bail!("Grimoire card {id} is not in the manifest table") };
    if !c.stats.iter().any(|(n, _)| *n == stat) {
        bail!("Grimoire card {id} has no stat {stat}");
    }
    let account = store.account_of(character);
    let mut s = store.grimoire(account);
    let before = card_score(c, &s);
    *s.stats.entry((id, stat)).or_default() += delta;
    let after = card_score(c, &s);
    store.set_grimoire(account, s);
    Ok(after - before)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewards::store::MemoryStore;

    #[test]
    fn grimoire_total_matches_the_manifest() {
        let t = tables();
        assert_eq!(max_score(), t.grimoire_manifest_total);
        assert_eq!(max_score(), 5_610);
        let mut all = GrimoireState::default();
        for c in &t.grimoire {
            all.cards.insert(c.id);
            for (n, ranks) in &c.stats {
                all.stats.insert((c.id, *n), ranks.iter().map(|(t, _)| *t).max().unwrap_or(0));
            }
        }
        assert_eq!(score(&all), max_score());
        assert_eq!(score(&GrimoireState::default()), 0);
        // Card points alone: 4,710 of the 5,610.
        let cards_only = GrimoireState { cards: all.cards.clone(), stats: BTreeMap::new() };
        assert_eq!(score(&cards_only), t.grimoire.iter().map(|c| c.points).sum::<u32>());
    }

    #[test]
    fn stat_ranks_add_points_at_their_thresholds() {
        let t = tables();
        let c = t.grimoire.iter().find(|c| c.stats.iter().any(|(_, r)| r.len() >= 3 && r[0].0 > 0)).unwrap();
        let (stat, ranks) = c.stats.iter().find(|(_, r)| r.len() >= 3 && r[0].0 > 0).unwrap();
        let mut s = MemoryStore::with_character(1, 2, 0, 40);
        assert_eq!(add_stat(&mut s, 2, c.id, *stat, ranks[0].0 - 1).unwrap(), 0);
        assert_eq!(add_stat(&mut s, 2, c.id, *stat, 1).unwrap(), ranks[0].1);
        let to_top = ranks.last().unwrap().0 - ranks[0].0;
        let rest: u32 = ranks[1..].iter().map(|(_, p)| p).sum();
        assert_eq!(add_stat(&mut s, 2, c.id, *stat, to_top).unwrap(), rest);
        assert_eq!(unlock_card(&mut s, 2, c.id).unwrap(), c.points);
        assert_eq!(unlock_card(&mut s, 2, c.id).unwrap(), 0, "unlocking twice adds nothing");
        assert!(add_stat(&mut s, 2, c.id, 99, 1).is_err());
        assert!(unlock_card(&mut s, 2, 1).is_err());
        assert_eq!(score(&s.grimoire(1)), card_score(c, &s.grimoire(1)));
    }
}
