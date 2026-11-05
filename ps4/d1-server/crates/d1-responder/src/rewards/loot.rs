// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! End-of-activity rewards: the one entry point activities call.
//!
//! ```text
//! rewards::grant_activity_rewards(&mut store, character, activity_hash, &outcome) -> Result<RewardGrant>
//! ```
//!
//! For streams J (strikes, raids, Crucible) and A2 (the host script bridge): call it once per
//! fireteam member when the activity ends (`activity_complete`, incident 965 in 56578's content),
//! with that member's character id, the activity's manifest hash (the hash the client launched, as
//! the activity host logs it) and an [`ActivityOutcome`]. Each member is rolled separately
//! (`loot.per_player`). Check [`super::enabled`] first; with `D1_REWARDS` unset, do not call it.
//! The grant changes the store and returns what changed; pushing it to the client (profile and
//! account deltas) is the caller's, through the existing push paths.
//!
//! What it grants:
//! - the activity's own reward list from the manifest (`loot.activity_rewards`): Legendary Marks,
//!   Strange Coins and the other stackables at the values the list states, through the wallet and
//!   weekly caps; Experience as character XP (to the store's character-level progression) and to
//!   the equipped grids; the rest (reputation and display-only lines) returned in
//!   [`RewardGrant::for_progression`] for stream C;
//! - a `(Weekly Bonus Remaining: N)` marker in the list limits those rewards to N a week
//!   (`loot.weekly_bonus`);
//! - with [`ActivityOutcome::community_page`], the community guide's Legendary Mark grants and
//!   limits for that activity instead of the list's marks (`loot.mark_limits`);
//! - with [`ActivityOutcome::roll_engrams`], end-of-activity engrams at PLACEHOLDER chances
//!   (`loot.drop_chance`). Their Light is rolled when they are decrypted
//!   ([`super::engrams::decrypt`]).

use anyhow::{bail, Result};
use d1_loadout::model::Slot;

use super::currencies::{self, Granted, Source, LEGENDARY_MARKS};
use super::engrams::engram_for;
use super::item_instance::ItemInstance;
use super::placeholders as ph;
use super::store::{CharacterId, InstanceId, Owner, Period, RewardStore};
use super::tables::tables;
use super::talent_grid::share_xp;
use super::{seed_from_env, Rng};

/// The reward display item "Experience" (its value is XP).
pub const EXPERIENCE: u32 = 3_006_793_116;
/// The `character_level` progression.
pub const CHARACTER_LEVEL: u32 = 1_716_568_313;

/// How the activity ended for one player.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActivityOutcome {
    /// Completed (the activity's completion, not a wipe or a quit).
    pub completed: bool,
    /// The cryptarch-guide page for this activity (e.g. `siva-crisis-heroic-strike-playlist`), when
    /// the caller knows it. Selects the community Legendary Mark grants and limits.
    pub community_page: Option<String>,
    /// Roll end-of-activity engrams at the placeholder chances.
    pub roll_engrams: bool,
    /// A fixed seed for this grant; otherwise `D1_REWARDS_SEED` or the time.
    pub seed: Option<u64>,
}

/// What a grant changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RewardGrant {
    pub activity: u32,
    pub character: CharacterId,
    pub seed: u64,
    pub currencies: Vec<Granted>,
    pub items: Vec<InstanceId>,
    /// `(progression hash, amount)` added to the store.
    pub progressions: Vec<(u32, u32)>,
    /// `(display item, value)` lines this module does not apply (reputation, markers): for stream C.
    pub for_progression: Vec<(u32, u32)>,
    /// Grid items that gained XP: `(instance, levels gained)`.
    pub item_xp: Vec<(InstanceId, u32)>,
    /// Weekly bonus left after this grant, when the activity has a weekly bonus marker.
    pub weekly_bonus_left: Option<u32>,
    pub rules: Vec<&'static str>,
}

fn note(rules: &mut Vec<&'static str>, r: &'static str) {
    if !rules.contains(&r) {
        rules.push(r);
    }
}

/// Grant one player's rewards for an activity. See the module documentation.
pub fn grant_activity_rewards(store: &mut dyn RewardStore, character: CharacterId, activity_hash: u32, outcome: &ActivityOutcome) -> Result<RewardGrant> {
    let t = tables();
    let Some(activity) = t.activities.get(&activity_hash) else { bail!("activity {activity_hash} is not in the manifest table") };
    let seed = outcome.seed.unwrap_or_else(|| seed_from_env(store.now()));
    let mut rng = Rng::new(seed ^ u64::from(activity_hash) ^ character.rotate_left(17));
    let mut g = RewardGrant { activity: activity_hash, character, seed, ..RewardGrant::default() };
    if !outcome.completed {
        return Ok(g);
    }
    note(&mut g.rules, "loot.per_player");
    note(&mut g.rules, "loot.activity_rewards");

    let account = Owner::Account(store.account_of(character));
    let bonus_key = format!("activity.{activity_hash}.bonus");
    let bonus_limit = activity.rewards.iter().find_map(|(h, _)| t.weekly_bonus.get(h).copied());
    let bonus_open = match bonus_limit {
        Some(n) => {
            note(&mut g.rules, "loot.weekly_bonus");
            let used = store.counter(account, &bonus_key, Period::Weekly);
            if used < n {
                store.add_counter(account, &bonus_key, Period::Weekly, 1);
            }
            g.weekly_bonus_left = Some(n.saturating_sub(used + 1).min(n));
            used < n
        }
        None => true,
    };

    let community: Vec<_> = outcome
        .community_page
        .as_deref()
        .map(|p| t.mark_limits.iter().filter(|m| m.activity == p).collect())
        .unwrap_or_default();

    for (display, value) in &activity.rewards {
        if t.weekly_bonus.contains_key(display) {
            continue;
        }
        if *display == EXPERIENCE {
            if *value > 0 {
                store.add_progression(character, CHARACTER_LEVEL, *value);
                g.progressions.push((CHARACTER_LEVEL, *value));
                note(&mut g.rules, "grid.xp_share");
                for (id, levels, _) in share_xp(store, character, *value)? {
                    g.item_xp.push((id, levels));
                }
            }
            continue;
        }
        match t.reward_display.get(display) {
            Some(item) if *value > 0 => {
                note(&mut g.rules, "loot.display_map");
                if *item == LEGENDARY_MARKS && !community.is_empty() {
                    continue;
                }
                if !bonus_open {
                    continue;
                }
                let r = currencies::grant(store, character, *item, *value, &Source::Activity);
                for x in &r.rules {
                    note(&mut g.rules, x);
                }
                g.currencies.push(r);
            }
            _ => g.for_progression.push((*display, *value)),
        }
    }

    for m in community {
        note(&mut g.rules, "loot.mark_limits");
        let owner = if m.account { account } else { Owner::Character(character) };
        let (period, key) = match m.period.as_str() {
            "daily" => (Period::Daily, format!("marks.{}.{}.daily", m.activity, m.marks)),
            "weekly" => (Period::Weekly, format!("marks.{}.{}.weekly", m.activity, m.marks)),
            _ => (Period::Forever, String::new()),
        };
        if let Some(limit) = m.limit {
            if store.counter(owner, &key, period) >= limit {
                continue;
            }
            store.add_counter(owner, &key, period, 1);
        }
        let r = currencies::grant(store, character, LEGENDARY_MARKS, m.marks, &Source::Activity);
        for x in &r.rules {
            note(&mut g.rules, x);
        }
        g.currencies.push(r);
    }

    if outcome.roll_engrams {
        note(&mut g.rules, "loot.drop_chance");
        let slots = [Slot::Primary, Slot::Special, Slot::Heavy, Slot::Head, Slot::Arms, Slot::Chest, Slot::Legs, Slot::ClassItem];
        for (tier, chance) in ph::ACTIVITY_ENGRAM_PER_MILLE {
            if !rng.chance(*chance) {
                continue;
            }
            let slot = *rng.pick(&slots).expect("slots");
            let Some(engram) = engram_for(*tier, slot.bucket_hash()) else { continue };
            let id = store.new_instance_id();
            store.put_item(character, ItemInstance::roll(id, engram, 0, &mut rng)?);
            g.items.push(id);
        }
    }
    Ok(g)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewards::store::MemoryStore;

    const ACCOUNT: u64 = 0xD100_0000_0000_0001;
    const CHARACTER: u64 = 0xD100_0000_0000_0101;
    /// A Guardian Rises (the manifest's row for the story mission: reward list "XP Bonus" only).
    const AGR: u32 = 1_846_390_409;

    fn store() -> MemoryStore {
        let mut s = MemoryStore::with_character(ACCOUNT, CHARACTER, 0, 40);
        s.now = 1_477_990_800;
        s
    }

    fn activity_with(display: u32) -> u32 {
        let mut v: Vec<_> = tables().activities.values().filter(|a| a.rewards.iter().any(|(h, v)| *h == display && *v > 0)).map(|a| a.hash).collect();
        v.sort();
        v[0]
    }

    #[test]
    fn an_unfinished_activity_grants_nothing() {
        let mut s = store();
        let g = grant_activity_rewards(&mut s, CHARACTER, AGR, &ActivityOutcome::default()).unwrap();
        assert!(g.currencies.is_empty() && g.items.is_empty() && g.progressions.is_empty());
        assert!(grant_activity_rewards(&mut s, CHARACTER, 1, &ActivityOutcome::default()).is_err());
    }

    #[test]
    fn the_manifest_reward_list_is_granted() {
        let mut s = store();
        let a = activity_with(3_510_203_897); // a "Legendary Marks" line
        let want: u32 = tables().activities[&a].rewards.iter().filter(|(h, _)| *h == 3_510_203_897).map(|(_, v)| v).sum();
        let done = ActivityOutcome { completed: true, seed: Some(1), ..Default::default() };
        let g = grant_activity_rewards(&mut s, CHARACTER, a, &done).unwrap();
        assert_eq!(s.quantity(Owner::Account(ACCOUNT), LEGENDARY_MARKS), want.min(ph::MARKS_WEEKLY_CAP));
        assert!(g.rules.contains(&"loot.display_map"));
    }

    #[test]
    fn experience_feeds_the_character_and_the_equipped_grids() {
        let mut s = store();
        let a = activity_with(EXPERIENCE);
        let xp: u32 = tables().activities[&a].rewards.iter().filter(|(h, _)| *h == EXPERIENCE).map(|(_, v)| v).sum();
        let rifle = tables().items.values().filter(|d| d.talent_grid != 0 && d.instanced && Slot::from_bucket_hash(d.bucket) == Some(Slot::Primary)).map(|d| d.hash).min().unwrap();
        let id = s.new_instance_id();
        s.put_item(CHARACTER, ItemInstance::roll(id, rifle, 300, &mut Rng::new(2)).unwrap());
        s.equip(CHARACTER, id);
        let g = grant_activity_rewards(&mut s, CHARACTER, a, &ActivityOutcome { completed: true, seed: Some(2), ..Default::default() }).unwrap();
        assert_eq!(s.progression(CHARACTER, CHARACTER_LEVEL), xp);
        assert_eq!(s.item(id).unwrap().grid.as_ref().unwrap().xp, xp * ph::XP_SHARE_PER_MILLE / 1000);
        assert_eq!(g.item_xp.len(), 1);
    }

    #[test]
    fn a_weekly_bonus_runs_out() {
        let t = tables();
        let (a, n) = t
            .activities
            .values()
            .filter_map(|a| {
                let n = a.rewards.iter().find_map(|(h, _)| t.weekly_bonus.get(h).copied())?;
                a.rewards.iter().any(|(h, v)| *v > 0 && t.reward_display.contains_key(h)).then_some((a.hash, n))
            })
            .min()
            .unwrap();
        let mut s = store();
        let done = ActivityOutcome { completed: true, seed: Some(3), ..Default::default() };
        let mut granted_runs = 0;
        for _ in 0..n + 2 {
            let g = grant_activity_rewards(&mut s, CHARACTER, a, &done).unwrap();
            if g.currencies.iter().any(|c| c.requested > 0) {
                granted_runs += 1;
            }
        }
        assert_eq!(granted_runs, n, "activity {a}: bonus limit {n}");
        s.now += 7 * 86_400;
        let g = grant_activity_rewards(&mut s, CHARACTER, a, &done).unwrap();
        assert!(g.currencies.iter().any(|c| c.requested > 0), "a new week reopens the bonus");
    }

    #[test]
    fn community_mark_limits_apply_per_period() {
        let mut s = store();
        let a = activity_with(3_510_203_897);
        let page = "siva-crisis-heroic-strike-playlist";
        let row = tables().mark_limits.iter().find(|m| m.activity == page).unwrap();
        let done = ActivityOutcome { completed: true, community_page: Some(page.into()), seed: Some(4), ..Default::default() };
        let mut marks = 0;
        for _ in 0..row.limit.unwrap() + 3 {
            let g = grant_activity_rewards(&mut s, CHARACTER, a, &done).unwrap();
            marks += g.currencies.iter().filter(|c| c.item == LEGENDARY_MARKS).map(|c| c.granted).sum::<u32>();
        }
        assert_eq!(marks, row.marks * row.limit.unwrap());
    }

    #[test]
    fn engram_drops_are_seeded_and_per_player() {
        let a = activity_with(3_510_203_897);
        let roll = |seed: u64, character: u64| {
            let mut s = MemoryStore::with_character(ACCOUNT, character, 1, 40);
            let o = ActivityOutcome { completed: true, roll_engrams: true, seed: Some(seed), ..Default::default() };
            let g = grant_activity_rewards(&mut s, character, a, &o).unwrap();
            g.items.iter().map(|id| s.item(*id).unwrap().item_hash).collect::<Vec<_>>()
        };
        assert_eq!(roll(9, CHARACTER), roll(9, CHARACTER), "same seed, same drops");
        let mut differs = false;
        let mut any = 0;
        for seed in 0..64 {
            let a = roll(seed, CHARACTER);
            any += a.len();
            differs |= a != roll(seed, CHARACTER + 1);
            for h in a {
                assert!(tables().items[&h].is_engram());
            }
        }
        assert!(any > 0 && differs, "drops never varied between players");
    }
}
