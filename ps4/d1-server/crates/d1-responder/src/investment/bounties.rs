// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! 402 `inventory_perform_item_action` with action 1 on a bounty: the turn-in.
//!
//! The live Tower capture (#328, 1113-1117 s) turned in three bounties this way: a 402
//! `{I {flag 1, instance, content}, action 1}` on the bounty, then one family-4 push carrying **the
//! bounty deleted, one 15-byte item update for each of the 10 equipped items** (their XP share) and
//! the profile, then the reply, whose value is **0** (402's replies carry 0, unlike the others).
//! Action 1 on gear is the dismantle; the action index is per item definition.
//!
//! Served here (behind `D1_SOCIAL=1`, the player store on):
//! - the item must be a held bounty or quest step (`tables::bounties`, stream I's manifest table);
//! - completion: when the player store keeps a record of it (`PlayerStore::bounties`), its
//!   objectives must be complete; ⚠ when it keeps none (progress was never reported to it), the
//!   client's own check stands, and the turn-in says so;
//! - rewards: "Experience" into the character's level (the profile, `progression::add_xp`) and, at
//!   `grid.xp_share`, into each equipped item that has a talent grid (element `+0x38`); the
//!   "… Reputation" pseudo-items into the store's reputation; real items into their buckets;
//! - a quest step gives the questline's next step.

use d1_bap::investment_ops::ItemSelector;

use super::snapshot::{element as el, Snapshot, Touch};
use super::{Refusal, Status};
use crate::bounties::{reward_progression, EXPERIENCE};

/// The action index a bounty's turn-in sends (#328).
pub const TURN_IN: i32 = 1;

pub fn is_bounty(content: u16) -> bool {
    crate::tables::tables().bounties.contains_key(&content)
}

/// The order of the live turn-in push: the deletion, the item updates, then the account and the profile.
fn live_order(log: &mut [Touch]) {
    log.sort_by_key(|t| match t {
        Touch::Deleted(_) => 0,
        Touch::Item(_) => 1,
        Touch::Account => 2,
        Touch::Profile => 3,
    });
}

pub fn turn_in(s: &mut Snapshot, item: ItemSelector, curves: Option<&crate::progression::Curves>, template: &[u8]) -> Result<String, Refusal> {
    let t = crate::tables::tables();
    let content = s.content_of(item.instance).ok_or_else(|| Refusal::new(Status::NotFound, format!("instance {:#x} is not held", item.instance)))?;
    let b = t.bounties.get(&content).ok_or_else(|| Refusal::new(Status::Invalid, format!("{content} is not a bounty or quest step")))?;
    let hash = crate::content::item_hash(content).unwrap_or(0);
    let mut notes = Vec::new();
    match s.bounties.get(&hash) {
        Some((progress, completed)) => {
            let done = *completed || b.objectives.iter().enumerate().all(|(i, &(_, need))| progress.get(i).copied().unwrap_or(0) >= need);
            if !done {
                return Err(Refusal::new(Status::NotAvailable, format!("bounty {content}: objectives {progress:?} of {:?}", b.objectives)));
            }
            s.bounties_done.push(hash);
        }
        None => notes.push("no progress record in the store: the client's completion check stands".to_string()),
    }
    s.remove_instance(item.instance).map_err(|e| Refusal::new(Status::NotFound, e))?;
    for &(reward, amount) in &b.rewards {
        match reward_progression(t, reward) {
            Some(EXPERIENCE) => {
                match curves {
                    Some(c) => match crate::progression::add_xp(&mut s.profile, c, amount) {
                        Ok(g) => {
                            s.touch(Touch::Profile);
                            notes.push(format!("+{amount} XP (level {} -> {})", g.level_before, g.level_after));
                        }
                        Err(e) => notes.push(format!("+{amount} XP not applied: {e}")),
                    },
                    None => notes.push(format!("+{amount} XP not applied: no curves")),
                }
                let share = (u64::from(amount) * u64::from(crate::rewards::placeholders::XP_SHARE_PER_MILLE) / 1000) as u32;
                for id in s.equipped() {
                    let Some(mut e) = s.elements.get(&id).cloned() else { continue };
                    if el::u16_at(&e, el::GRID) == 0xFFFF {
                        continue;
                    }
                    let xp = el::u32_at(&e, el::XP).saturating_add(share);
                    el::set_i32(&mut e, el::XP, xp as i32);
                    s.set_element(id, e);
                }
            }
            Some(p) if t.progressions.contains_key(&p) => {
                s.reputation_grants.push((t.progressions[&p].name.clone(), amount));
                notes.push(format!("+{amount} {}", t.progressions[&p].name));
            }
            Some(p) => notes.push(format!("progression {p}: no curve, skipped")),
            None => {
                let instanced = super::talents::item_def(reward).is_some_and(|d| d.instanced);
                let r = if instanced {
                    s.give_instance(reward, template.to_vec()).map(|soid| format!("{reward} as {soid:#x}"))
                } else {
                    let cap = crate::content::item_hash(reward).map_or(u32::MAX, crate::rewards::currencies::cap);
                    s.give(reward, amount, cap).map(|n| format!("{n} of {reward}"))
                };
                notes.push(r.unwrap_or_else(|e| format!("{reward}: {e}")));
            }
        }
    }
    if let Some(next) = b.next {
        match s.give_instance(next, template.to_vec()) {
            Ok(soid) => notes.push(format!("next step {next} as {soid:#x}")),
            Err(e) => notes.push(format!("next step {next}: {e}")),
        }
    }
    live_order(&mut s.log);
    Ok(format!("bounty {content} turned in: {}", notes.join("; ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::investment::snapshot::tests::{equip, snapshot, with_instance};

    fn bounty() -> (u16, u32) {
        let t = crate::tables::tables();
        let mut k: Vec<_> = t.bounties.iter().filter(|(c, b)| b.item_type == 4 && b.next.is_none() && crate::content::bucket_of(**c).is_some()
            && b.rewards.iter().any(|r| reward_progression(t, r.0) == Some(EXPERIENCE))).collect();
        k.sort_by_key(|(c, _)| **c);
        let (c, b) = k[0];
        (*c, b.rewards.iter().find(|r| reward_progression(t, r.0) == Some(EXPERIENCE)).unwrap().1)
    }

    #[test]
    fn a_turn_in_deletes_the_bounty_and_shares_its_xp_in_the_live_order() {
        let (content, xp) = bounty();
        let mut s = snapshot();
        with_instance(&mut s, content, 0x501, vec![0; el::BYTES]);
        let mut gridded = vec![0; el::BYTES];
        el::set_i32(&mut gridded, el::GRID, 7);
        with_instance(&mut s, 3598, 0x401, gridded);
        let mut plain = vec![0; el::BYTES];
        plain[el::GRID..el::GRID + 2].copy_from_slice(&0xFFFFu16.to_le_bytes());
        with_instance(&mut s, 3598, 0x402, plain);
        equip(&mut s, 6, 0x401);
        equip(&mut s, 7, 0x402);
        let what = turn_in(&mut s, ItemSelector { instanced: true, instance: 0x501, index: content as i16 }, None, &[0; el::BYTES]).unwrap();
        assert!(what.contains("client's completion check"), "{what}");
        assert_eq!(s.locate(0x501), None);
        assert_eq!(el::u32_at(&s.elements[&0x401], el::XP), xp, "the gridded item gains the share");
        assert_eq!(el::u32_at(&s.elements[&0x402], el::XP), 0, "an item with no grid does not");
        assert_eq!(s.log.first(), Some(&Touch::Deleted(0x501)));
        assert_eq!(s.log[1], Touch::Item(0x401));
        assert_eq!(s.log.last(), Some(&Touch::Profile));
    }

    #[test]
    fn an_incomplete_record_or_gear_is_refused() {
        let (content, _) = bounty();
        let mut s = snapshot();
        with_instance(&mut s, content, 0x501, vec![0; el::BYTES]);
        s.bounties.insert(crate::content::item_hash(content).unwrap(), (vec![0], false));
        let before = s.clone();
        let sel = ItemSelector { instanced: true, instance: 0x501, index: content as i16 };
        assert_eq!(turn_in(&mut s, sel, None, &[]).unwrap_err().status, Status::NotAvailable);
        assert_eq!(s, before);
        s.bounties.insert(crate::content::item_hash(content).unwrap(), (vec![], true));
        turn_in(&mut s, sel, None, &[]).unwrap();
        assert_eq!(s.bounties_done, vec![crate::content::item_hash(content).unwrap()]);
        assert!(!is_bounty(3598));
    }
}
