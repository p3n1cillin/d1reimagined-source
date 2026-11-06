// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Grants from the activity host: the `grant` command of the store's control file.
//!
//! The activity host decides when an activity ends, but stream M's services run here, beside the
//! player store, in another process. The host writes one line per character into the store's
//! command file (`D1_STORE_CONTROL`, polled each second; `player_store::start_control`):
//!
//! ```text
//! grant CHARACTER ACTIVITY_HASH OUTCOME [page=PAGE] [engrams=1] [seed=SEED] [source=WORD]
//! ```
//!
//! * `CHARACTER`: `0x` and 16 hex digits, a character SOID (or the 8-byte key the host has), or
//!   `id:` and the 80 hex digits of the client's 40-byte identity (`join_request+0x10`,
//!   `d1_mp::identity::MemberIdentity`), whose character SOID is at `+0x18`;
//! * `ACTIVITY_HASH`: the manifest hash the client launched (`0x` hex or decimal);
//! * `OUTCOME`: `completed`, `incomplete` or `abandoned` (only `completed` grants anything, as
//!   [`super::loot::grant_activity_rewards`] rules);
//! * `page=`: the Cryptarch guide page ([`super::ActivityOutcome::community_page`]), `engrams=1`
//!   the placeholder engram roll, `seed=` a fixed seed, `source=` a free word for the log (for
//!   example `script`, `event:12:gold`, `chest`).
//!
//! The activity host's writer is `d1_activity_host::reward_forward` (same grammar, tested against
//! the same lines). [`execute`] runs one grant through [`super::player_store_adapter::Adapter`] on
//! the live store; `player_store` then saves, pushes the profile when XP changed it, and has the
//! vendor state publish the account when a wallet changed.
//!
//! **Which character.** The store holds one player per process. A grant naming another character
//! is refused when multiplayer is on (`D1_MP=1`: it is another player's), and applied to the
//! signed-on character otherwise, with the mismatch in the log: with one client, every grant is
//! its own (the open world keys characters by the identity's first 8 bytes, not the SOID).
//!
//! **What persists.** The adapter keeps what C's record has no field for in an overlay. Its period
//! counters (the weekly bonus, the Marks caps) and its stacks with no account slot (Legendary Marks
//! until their content index is read) are saved in the character's `attributes`
//! (`rewards.counters`, `rewards.stacks`) and restored before the next grant, so the weekly limits
//! hold across grants and restarts. Instances and their grids still last only one grant (open).

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};

use crate::player_store::PlayerStore;

use super::item_instance::ClientIndex;
use super::player_store_adapter::Adapter;
use super::store::{Owner, Period};
use super::{currencies, ActivityOutcome};

/// How the activity ended for the character, as the host reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantOutcome {
    Completed,
    Incomplete,
    Abandoned,
}

impl GrantOutcome {
    pub fn word(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Incomplete => "incomplete",
            Self::Abandoned => "abandoned",
        }
    }
}

/// Who a grant is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrantTarget {
    /// A character SOID, or the 8-byte key the host keyed the character by.
    Key(u64),
    /// The client's 40-byte identity.
    Identity(Vec<u8>),
}

impl GrantTarget {
    /// The character SOID it names: the key itself, or the identity's `+0x18`.
    pub fn character(&self) -> Option<u64> {
        match self {
            Self::Key(k) => Some(*k),
            Self::Identity(b) => d1_mp::identity::MemberIdentity::parse(b).map(|m| m.character.0),
        }
    }
}

/// One `grant` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantCommand {
    pub target: GrantTarget,
    pub activity_hash: u32,
    pub outcome: GrantOutcome,
    pub page: Option<String>,
    pub engrams: bool,
    pub seed: Option<u64>,
    pub source: Option<String>,
}

fn num64(s: &str) -> Result<u64> {
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(h) => u64::from_str_radix(h, 16),
        None => s.parse(),
    }
    .map_err(|e| anyhow!("{s:?}: {e}"))
}

fn hex_bytes(s: &str) -> Result<Vec<u8>> {
    if s.len() % 2 != 0 {
        bail!("odd hex length {}", s.len());
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| anyhow!("{s:?}: {e}"))).collect()
}

/// Parse the fields after the word `grant`.
pub fn parse(fields: &[&str]) -> Result<GrantCommand> {
    let [who, hash, outcome, rest @ ..] = fields else {
        bail!("grant takes CHARACTER ACTIVITY_HASH OUTCOME [page=..] [engrams=1] [seed=..] [source=..]");
    };
    let target = match who.strip_prefix("id:") {
        Some(h) => {
            let b = hex_bytes(h)?;
            if b.len() != d1_mp::identity::MemberIdentity::SIZE {
                bail!("an identity is {} bytes, got {}", d1_mp::identity::MemberIdentity::SIZE, b.len());
            }
            GrantTarget::Identity(b)
        }
        None => GrantTarget::Key(num64(who)?),
    };
    let activity_hash = u32::try_from(num64(hash)?).map_err(|_| anyhow!("activity hash {hash:?} is not 32-bit"))?;
    let outcome = match *outcome {
        "completed" => GrantOutcome::Completed,
        "incomplete" => GrantOutcome::Incomplete,
        "abandoned" => GrantOutcome::Abandoned,
        o => bail!("outcome {o:?} is not completed, incomplete or abandoned"),
    };
    let mut c = GrantCommand { target, activity_hash, outcome, page: None, engrams: false, seed: None, source: None };
    for t in rest {
        match t.split_once('=') {
            Some(("page", v)) if !v.is_empty() => c.page = Some(v.to_string()),
            Some(("engrams", v)) => c.engrams = v == "1",
            Some(("seed", v)) => c.seed = Some(num64(v)?),
            Some(("source", v)) => c.source = Some(v.to_string()),
            _ => bail!("{t:?}: not page=, engrams=, seed= or source="),
        }
    }
    Ok(c)
}

/// The canonical line for `c` (what the activity host writes).
pub fn format(c: &GrantCommand) -> String {
    let who = match &c.target {
        GrantTarget::Key(k) => format!("{k:#018x}"),
        GrantTarget::Identity(b) => format!("id:{}", b.iter().map(|x| format!("{x:02x}")).collect::<String>()),
    };
    let mut line = format!("grant {who} {:#010x} {}", c.activity_hash, c.outcome.word());
    if let Some(p) = &c.page {
        line += &format!(" page={p}");
    }
    if c.engrams {
        line += " engrams=1";
    }
    if let Some(s) = c.seed {
        line += &format!(" seed={s:#x}");
    }
    if let Some(s) = &c.source {
        line += &format!(" source={s}");
    }
    line
}

/// The client indices the responder knows: Glimmer's content index is the one the account's slot
/// 308 holds (the client's bucket table puts Glimmer at slot 308, account scope:
/// `currency.scope.glimmer`). Every other item has no 56578 content index yet, so it stays in the
/// adapter's overlay (and its `attributes` copy).
pub struct ResponderIndex {
    pub glimmer: Option<u16>,
}

/// The account slot that holds Glimmer (the client's bucket table, slot 308, scope 1).
pub const GLIMMER_SLOT: u16 = 308;

impl ResponderIndex {
    pub fn from_slots(slots: &std::collections::BTreeMap<u16, (u16, u32)>) -> Self {
        Self { glimmer: slots.get(&GLIMMER_SLOT).map(|(c, _)| *c) }
    }
}

impl ClientIndex for ResponderIndex {
    fn content_index(&self, item: u32) -> Option<u16> {
        if item == currencies::GLIMMER {
            self.glimmer
        } else {
            None
        }
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

/// What a grant did, for the log, the record's history and the store's state file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantSummary {
    pub line: String,
    pub character: u64,
    /// The grant named another character and was applied to the signed-on one.
    pub retargeted: bool,
    /// `(item hash, granted)` for every currency line.
    pub currencies: Vec<(u32, u32)>,
    pub items: usize,
    pub rules: Vec<&'static str>,
    pub notes: Vec<String>,
}

impl GrantSummary {
    pub fn json(&self) -> Value {
        json!({
            "line": self.line,
            "character": format!("{:#018x}", self.character),
            "retargeted": self.retargeted,
            "currencies": self.currencies.iter().map(|(h, n)| json!([h, n])).collect::<Vec<_>>(),
            "items": self.items,
            "rules": self.rules,
        })
    }
}

fn owner_word(o: Owner) -> String {
    match o {
        Owner::Account(a) => format!("a{a:x}"),
        Owner::Character(c) => format!("c{c:x}"),
    }
}

fn owner_of(w: &str) -> Option<Owner> {
    let (k, v) = w.split_at(1);
    let v = u64::from_str_radix(v, 16).ok()?;
    match k {
        "a" => Some(Owner::Account(v)),
        "c" => Some(Owner::Character(v)),
        _ => None,
    }
}

fn period_word(p: Period) -> &'static str {
    match p {
        Period::Daily => "daily",
        Period::Weekly => "weekly",
        Period::Forever => "forever",
    }
}

fn period_of(w: &str) -> Option<Period> {
    match w {
        "daily" => Some(Period::Daily),
        "weekly" => Some(Period::Weekly),
        "forever" => Some(Period::Forever),
        _ => None,
    }
}

/// The overlay's counters and slotless stacks, from the character's attributes.
fn restore_overlay(a: &mut Adapter<'_>, counters: Option<Value>, stacks: Option<Value>) {
    for row in counters.as_ref().and_then(Value::as_array).into_iter().flatten() {
        let r = row.as_array().map(Vec::as_slice).unwrap_or_default();
        if let [o, k, p, i, n] = r {
            if let (Some(o), Some(k), Some(p), Some(i), Some(n)) =
                (o.as_str().and_then(owner_of), k.as_str(), p.as_str().and_then(period_of), i.as_u64(), n.as_u64())
            {
                a.overlay.counters.insert((o, k.to_string(), p, i), n as u32);
            }
        }
    }
    for row in stacks.as_ref().and_then(Value::as_array).into_iter().flatten() {
        let r = row.as_array().map(Vec::as_slice).unwrap_or_default();
        if let [o, item, n] = r {
            if let (Some(o), Some(item), Some(n)) = (o.as_str().and_then(owner_of), item.as_u64(), n.as_u64()) {
                a.overlay.stacks.insert((o, item as u32), n as u32);
            }
        }
    }
}

fn overlay_json(a: &Adapter<'_>) -> (Value, Value) {
    let mut counters: Vec<_> = a.overlay.counters.iter().map(|((o, k, p, i), n)| (owner_word(*o), k.clone(), period_word(*p), *i, *n)).collect();
    counters.sort();
    let mut stacks: Vec<_> = a.overlay.stacks.iter().filter(|(_, n)| **n > 0).map(|((o, item), n)| (owner_word(*o), *item, *n)).collect();
    stacks.sort();
    (
        Value::Array(counters.into_iter().map(|(o, k, p, i, n)| json!([o, k, p, i, n])).collect()),
        Value::Array(stacks.into_iter().map(|(o, item, n)| json!([o, item, n])).collect()),
    )
}

/// The character attributes [`execute`] keeps.
pub const ATTR_COUNTERS: &str = "rewards.counters";
pub const ATTR_STACKS: &str = "rewards.stacks";
pub const ATTR_GRANTS: &str = "rewards.grants";
/// How many grant summaries the record keeps.
pub const GRANTS_KEPT: usize = 50;

/// Run one grant on `store` (the signed-on player): stream M's `grant_activity_rewards` through
/// the adapter, with `index` for the content indices and `class_type` the character's class.
/// `strict` (multiplayer on) refuses a grant for another character. The record is marked dirty;
/// the caller saves it.
pub fn execute(store: &mut dyn PlayerStore, cmd: &GrantCommand, index: &dyn ClientIndex, class_type: u32, strict: bool) -> Result<GrantSummary> {
    let own = store.identity().character;
    let named = cmd.target.character().ok_or_else(|| anyhow!("the identity does not parse"))?;
    let retargeted = named != own;
    if retargeted && strict {
        bail!("grant for character {named:#018x}, but this responder serves {own:#018x} (D1_MP=1: another player's)");
    }
    let outcome = ActivityOutcome {
        completed: cmd.outcome == GrantOutcome::Completed,
        community_page: cmd.page.clone(),
        roll_engrams: cmd.engrams,
        seed: cmd.seed,
    };
    let counters = store.character_attribute(ATTR_COUNTERS).cloned();
    let stacks = store.character_attribute(ATTR_STACKS).cloned();
    let line = format(cmd);
    let (g, notes, counters, stacks) = {
        let mut a = Adapter::new(store, index, class_type);
        restore_overlay(&mut a, counters, stacks);
        let g = super::grant_activity_rewards(&mut a, own, cmd.activity_hash, &outcome)?;
        a.finish(&format!("rewards: {line}"))?;
        let (c, s) = overlay_json(&a);
        (g, a.notes.clone(), c, s)
    };
    store.set_character_attribute(ATTR_COUNTERS, counters);
    store.set_character_attribute(ATTR_STACKS, stacks);
    let summary = GrantSummary {
        line,
        character: own,
        retargeted,
        currencies: g.currencies.iter().map(|c| (c.item, c.granted)).collect(),
        items: g.items.len(),
        rules: g.rules.clone(),
        notes,
    };
    let mut kept = store.character_attribute(ATTR_GRANTS).and_then(Value::as_array).cloned().unwrap_or_default();
    kept.push(summary.json());
    let drop = kept.len().saturating_sub(GRANTS_KEPT);
    kept.drain(..drop);
    store.set_character_attribute(ATTR_GRANTS, Value::Array(kept));
    store.mark_dirty(&summary.line);
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player_store::{Identity, Sources, Store};
    use crate::rewards::tables::tables;

    const IDENTITY: &str = "0102030405060708\
                            1112131415161718\
                            0100000000000000\
                            0101000000000000\
                            0000000000000000";

    #[test]
    fn grant_lines_round_trip_and_name_the_character() {
        let line = "grant 0xd100000000000101 0x0bbd0e26 completed page=siva-crisis-heroic-strike-playlist engrams=1 seed=0xd1f0 source=script";
        let f: Vec<&str> = line.split_whitespace().collect();
        let c = parse(&f[1..]).unwrap();
        assert_eq!(c.target, GrantTarget::Key(0xD100_0000_0000_0101));
        assert_eq!((c.activity_hash, c.outcome, c.engrams, c.seed), (0x0BBD_0E26, GrantOutcome::Completed, true, Some(0xD1F0)));
        assert_eq!(format(&c), line);
        let id = format!("id:{}", IDENTITY.replace(' ', ""));
        let c = parse(&[&id, "12345", "abandoned"]).unwrap();
        assert_eq!(c.target.character(), Some(0x0101), "the identity's character is at +0x18");
        assert_eq!(c.outcome, GrantOutcome::Abandoned);
        assert!(parse(&["0x1", "0x2"]).is_err());
        assert!(parse(&["0x1", "0x2", "won"]).is_err());
        assert!(parse(&["id:0102", "0x2", "completed"]).is_err(), "an identity is 40 bytes");
        assert!(parse(&["0x1", "0x1ffffffff", "completed"]).is_err());
    }

    fn lab_store() -> Store {
        let src = Sources { vendor_account_slots: Some("308:869:100,309:873:500".into()), ..Sources::default() };
        Store::in_memory(Identity::from_env(), &src)
    }

    /// The lowest-hash activity whose completion grants a currency, with that currency (so the
    /// test is the same on every run).
    fn currency_activity() -> Option<(u32, u32)> {
        use crate::rewards::store::MemoryStore;
        let mut hashes: Vec<u32> = tables().activities.keys().copied().collect();
        hashes.sort_unstable();
        hashes.into_iter().find_map(|h| {
            let mut m = MemoryStore::with_character(1, 2, 0, 40);
            let done = ActivityOutcome { completed: true, seed: Some(1), ..ActivityOutcome::default() };
            let g = crate::rewards::grant_activity_rewards(&mut m, 2, h, &done).ok()?;
            g.currencies.iter().find(|c| c.granted > 0).map(|c| (h, c.item))
        })
    }

    #[test]
    fn a_completed_grant_is_applied_and_kept_between_grants() {
        let mut s = lab_store();
        let me = s.identity().character;
        let (activity, item) = currency_activity().expect("the manifest table has activities that grant currencies");
        let index = ResponderIndex::from_slots(&s.record.account.slots);
        assert_eq!(index.glimmer, Some(869), "slot 308's content");
        let cmd = GrantCommand { target: GrantTarget::Key(me), activity_hash: activity, outcome: GrantOutcome::Completed, page: None, engrams: false, seed: Some(1), source: Some("test".into()) };
        let held = |s: &Store| -> u32 {
            if item == currencies::GLIMMER {
                return s.currency(869);
            }
            let stacks = s.character_attribute(ATTR_STACKS).and_then(Value::as_array).cloned().unwrap_or_default();
            stacks.iter().filter(|r| r[1].as_u64() == Some(u64::from(item))).filter_map(|r| r[2].as_u64()).sum::<u64>() as u32
        };
        let before = held(&s);
        let g = execute(&mut s, &cmd, &index, 0, true).unwrap();
        assert!(!g.retargeted);
        assert!(g.rules.contains(&"loot.activity_rewards"));
        let granted: u32 = g.currencies.iter().filter(|(h, _)| *h == item).map(|(_, n)| n).sum();
        assert!(granted > 0);
        assert_eq!(held(&s), before + granted, "in the wallet (Glimmer) or the kept overlay");
        // A second grant (a fresh adapter, as each control line is) starts from what was kept.
        let g2 = execute(&mut s, &cmd, &index, 0, true).unwrap();
        let granted2: u32 = g2.currencies.iter().filter(|(h, _)| *h == item).map(|(_, n)| n).sum();
        assert_eq!(held(&s), before + granted + granted2);
        assert_eq!(s.character_attribute(ATTR_GRANTS).and_then(Value::as_array).map(Vec::len), Some(2));
        assert!(s.is_dirty());
    }

    #[test]
    fn an_incomplete_grant_gives_nothing_and_another_players_is_refused_with_mp() {
        let mut s = lab_store();
        let index = ResponderIndex::from_slots(&s.record.account.slots);
        let activity = tables().activities.keys().next().copied().unwrap();
        let mut cmd = GrantCommand { target: GrantTarget::Key(0x1234), activity_hash: activity, outcome: GrantOutcome::Incomplete, page: None, engrams: false, seed: Some(1), source: None };
        assert!(execute(&mut s, &cmd, &index, 0, true).is_err(), "D1_MP=1: not this player's");
        let g = execute(&mut s, &cmd, &index, 0, false).unwrap();
        assert!(g.retargeted && g.currencies.is_empty() && g.rules.is_empty(), "single player: applied, and nothing for an incomplete run");
        cmd.activity_hash = 0xDEAD_BEEF;
        cmd.outcome = GrantOutcome::Completed;
        assert!(execute(&mut s, &cmd, &index, 0, false).is_err(), "not in the manifest table");
    }

    #[test]
    fn weekly_counters_survive_between_grants() {
        // The weekly-bonus activities: their counter must be read back from the attributes.
        let t = tables();
        let Some(activity) = t.activities.values().find(|a| a.rewards.iter().any(|(h, _)| t.weekly_bonus.contains_key(h))) else {
            eprintln!("SKIP: no weekly-bonus activity in the manifest table");
            return;
        };
        let mut s = lab_store();
        let me = s.identity().character;
        let index = ResponderIndex::from_slots(&s.record.account.slots);
        let cmd = GrantCommand { target: GrantTarget::Key(me), activity_hash: activity.hash, outcome: GrantOutcome::Completed, page: None, engrams: false, seed: Some(7), source: None };
        let limit = activity.rewards.iter().find_map(|(h, _)| t.weekly_bonus.get(h).copied()).unwrap();
        // Each grant here is a fresh adapter, as each control line is; the bonus still runs down.
        let mut lefts = Vec::new();
        for _ in 0..=limit {
            lefts.push(execute(&mut s, &cmd, &index, 0, true).unwrap().rules.contains(&"loot.weekly_bonus"));
        }
        assert!(lefts.iter().all(|x| *x));
        let counters = s.character_attribute(ATTR_COUNTERS).and_then(Value::as_array).cloned().unwrap();
        let used = counters.iter().find(|r| r[1].as_str().is_some_and(|k| k.ends_with(".bonus"))).and_then(|r| r[4].as_u64());
        assert_eq!(used, Some(u64::from(limit)), "the counter stops at the limit and is kept");
    }
}
