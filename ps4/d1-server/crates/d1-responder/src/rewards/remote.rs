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
//! **What reaches the client (stream O2).** Every item has its 56578 content index from the
//! exported item table (`content::item_index`) and a client bucket (`content::bucket_of`): stacks
//! go into the account's slots (account buckets: Glimmer, Legendary Marks) or the profile's
//! (character buckets: materials, Vanguard and Crucible Marks, Silver Dust), new instances into a
//! free slot of their bucket with an element from `D1_VENDOR_ITEM_TEMPLATE`. `player_store` then
//! pushes the account and the profile (through the vendor state when it is on, by itself when it
//! is off). A stack that cannot be placed (no decoded profile yet, a full bucket) is kept in the
//! overlay and delivered by a later grant.
//!
//! **What persists.** The adapter keeps what C's record has no field for in an overlay, and all
//! of it is saved in the character's `attributes` ([`OVERLAY_ATTRIBUTES`]: the period counters,
//! stacks not yet placed, item instances with their talent grids and rolls, progressions,
//! objectives, Record Book claims, the Grimoire's cards and stats) and restored before the next
//! grant, so nothing is lost when the adapter goes away, and the weekly limits hold across grants
//! and restarts.

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};

use crate::player_store::PlayerStore;

use super::item_instance::ClientIndex;
use super::player_store_adapter::Adapter;
use super::store::{Owner, Period, RewardStore};
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
    let target = parse_target(who)?;
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
    let mut line = format!("grant {} {:#010x} {}", target_word(&c.target), c.activity_hash, c.outcome.word());
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

/// ⭐ Stream O2: one `handoff` line, the story arc's next activity (stream K's
/// `ActivityRewards::activity_hand_off`; cine_reveal 165 -> the Tower 0):
///
/// ```text
/// handoff CHARACTER ACTIVITY_INDEX [from=HASH] [hash=HASH]
/// ```
///
/// * `CHARACTER` as in a `grant`;
/// * `ACTIVITY_INDEX`: the next activity's index in the client's activity table;
/// * `from=`: the manifest hash of the activity that completed (the store records its completion
///   in the profile's current-activity field, which the client selects its next activity from);
/// * `hash=`: the next activity's manifest hash, checked against the activity table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandOffCommand {
    pub target: GrantTarget,
    pub next: u16,
    pub from: Option<u32>,
    pub next_hash: Option<u32>,
}

/// Parse the fields after the word `handoff`.
pub fn parse_handoff(fields: &[&str]) -> Result<HandOffCommand> {
    let [who, next, rest @ ..] = fields else {
        bail!("handoff takes CHARACTER ACTIVITY_INDEX [from=HASH] [hash=HASH]");
    };
    let target = parse_target(who)?;
    let next = u16::try_from(num64(next)?).map_err(|_| anyhow!("activity index {next:?} is not 16-bit"))?;
    let mut c = HandOffCommand { target, next, from: None, next_hash: None };
    for t in rest {
        let hash = |v: &str| u32::try_from(num64(v)?).map_err(|_| anyhow!("{v:?} is not a 32-bit hash"));
        match t.split_once('=') {
            Some(("from", v)) => c.from = Some(hash(v)?),
            Some(("hash", v)) => c.next_hash = Some(hash(v)?),
            _ => bail!("{t:?}: not from= or hash="),
        }
    }
    Ok(c)
}

/// The canonical line for `c` (what the activity host writes).
pub fn format_handoff(c: &HandOffCommand) -> String {
    let mut line = format!("handoff {} {}", target_word(&c.target), c.next);
    if let Some(h) = c.from {
        line += &format!(" from={h:#010x}");
    }
    if let Some(h) = c.next_hash {
        line += &format!(" hash={h:#010x}");
    }
    line
}

fn parse_target(who: &str) -> Result<GrantTarget> {
    Ok(match who.strip_prefix("id:") {
        Some(h) => {
            let b = hex_bytes(h)?;
            if b.len() != d1_mp::identity::MemberIdentity::SIZE {
                bail!("an identity is {} bytes, got {}", d1_mp::identity::MemberIdentity::SIZE, b.len());
            }
            GrantTarget::Identity(b)
        }
        None => GrantTarget::Key(num64(who)?),
    })
}

fn target_word(t: &GrantTarget) -> String {
    match t {
        GrantTarget::Key(k) => format!("{k:#018x}"),
        GrantTarget::Identity(b) => format!("id:{}", b.iter().map(|x| format!("{x:02x}")).collect::<String>()),
    }
}

/// Which character a command applies to on this responder (one player per process): the
/// signed-on one; `strict` (multiplayer on) refuses another player's. Returns it and whether the
/// command named another key.
pub fn resolve_target(t: &GrantTarget, own: u64, strict: bool) -> Result<(u64, bool)> {
    let named = t.character().ok_or_else(|| anyhow!("the identity does not parse"))?;
    if named != own && strict {
        bail!("command for character {named:#018x}, but this responder serves {own:#018x} (D1_MP=1: another player's)");
    }
    Ok((own, named != own))
}

/// The client indices the responder knows: every item's 56578 content index from the exported
/// item table (#322, `content::item_index`, 8,609 items), and the progression ordinals from the
/// exported progression table. Glimmer's is the one the account's slot 308 holds when it holds
/// one (the client's bucket table puts Glimmer at slot 308, account scope); the export agrees
/// (869). Grid indices, curve levels and stat positions are still open (`ExportedIndex`).
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
            self.glimmer.or_else(|| crate::content::item_index(item))
        } else {
            crate::content::item_index(item)
        }
    }
    fn progression_ordinal(&self, progression: u32) -> Option<u16> {
        crate::content::progression_index(progression)
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
    /// New instances placed in a bucket (stream O2), in order: each is pushed with the profile.
    pub delivered: Vec<u64>,
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

/// The character attributes [`execute`] keeps the adapter's overlay in (stream O2: everything the
/// overlay holds, so nothing is lost when the adapter goes away).
pub const ATTR_COUNTERS: &str = "rewards.counters";
pub const ATTR_STACKS: &str = "rewards.stacks";
pub const ATTR_GRANTS: &str = "rewards.grants";
/// The overlay's item instances with their talent grids and rolls: `[{"item": .., "equipped": b}]`.
pub const ATTR_INSTANCES: &str = "rewards.instances";
/// Progressions the overlay keeps (all but XP through C's curves): `[[hash, value]]`.
pub const ATTR_PROGRESSIONS: &str = "rewards.progressions";
/// Objective values (bounties, quests, Record Book records): `[[hash, value]]`.
pub const ATTR_OBJECTIVES: &str = "rewards.objectives";
/// Record Book rewards claimed: `[[book, rank]]`.
pub const ATTR_CLAIMS: &str = "rewards.claims";
/// The Grimoire state (C's record keeps the scores; this keeps cards and stats):
/// `{"cards": [..], "stats": [[card, stat, value]]}`.
pub const ATTR_GRIMOIRE: &str = "rewards.grimoire";
/// Every overlay attribute, in the order [`overlay_attributes`] writes them.
pub const OVERLAY_ATTRIBUTES: [&str; 7] = [ATTR_COUNTERS, ATTR_STACKS, ATTR_INSTANCES, ATTR_PROGRESSIONS, ATTR_OBJECTIVES, ATTR_CLAIMS, ATTR_GRIMOIRE];
/// How many grant summaries the record keeps.
pub const GRANTS_KEPT: usize = 50;

fn rows(v: Option<&Value>) -> impl Iterator<Item = &[Value]> {
    v.and_then(Value::as_array).into_iter().flatten().map(|r| r.as_array().map(Vec::as_slice).unwrap_or_default())
}

/// The overlay as the record's attributes kept it (`kept[i]` is [`OVERLAY_ATTRIBUTES`]`[i]`).
fn restore_overlay(a: &mut Adapter<'_>, character: u64, kept: &[Option<Value>]) {
    let get = |k: &str| OVERLAY_ATTRIBUTES.iter().position(|x| *x == k).and_then(|i| kept.get(i)).and_then(Option::as_ref);
    for r in rows(get(ATTR_COUNTERS)) {
        if let [o, k, p, i, n] = r {
            if let (Some(o), Some(k), Some(p), Some(i), Some(n)) =
                (o.as_str().and_then(owner_of), k.as_str(), p.as_str().and_then(period_of), i.as_u64(), n.as_u64())
            {
                a.overlay.counters.insert((o, k.to_string(), p, i), n as u32);
            }
        }
    }
    for r in rows(get(ATTR_STACKS)) {
        if let [o, item, n] = r {
            if let (Some(o), Some(item), Some(n)) = (o.as_str().and_then(owner_of), item.as_u64(), n.as_u64()) {
                a.overlay.stacks.insert((o, item as u32), n as u32);
            }
        }
    }
    for row in get(ATTR_INSTANCES).and_then(Value::as_array).into_iter().flatten() {
        let Some(item) = row.get("item").cloned().and_then(|v| serde_json::from_value::<super::item_instance::ItemInstance>(v).ok()) else { continue };
        let id = item.id;
        a.overlay.put_item(character, item);
        if row.get("equipped").and_then(Value::as_bool) == Some(true) {
            a.overlay.equip(character, id);
        }
    }
    for r in rows(get(ATTR_PROGRESSIONS)) {
        if let [h, v] = r {
            if let (Some(h), Some(v)) = (h.as_u64(), v.as_u64()) {
                a.overlay.progressions.insert((character, h as u32), v as u32);
            }
        }
    }
    for r in rows(get(ATTR_OBJECTIVES)) {
        if let [h, v] = r {
            if let (Some(h), Some(v)) = (h.as_u64(), v.as_i64()) {
                a.overlay.objectives.insert((character, h as u32), v);
            }
        }
    }
    for r in rows(get(ATTR_CLAIMS)) {
        if let [b, rank] = r {
            if let (Some(b), Some(rank)) = (b.as_u64(), rank.as_u64()) {
                a.overlay.claimed.entry(character).or_default().insert((b as u32, rank as u32));
            }
        }
    }
    if let Some(g) = get(ATTR_GRIMOIRE) {
        let mut state = super::grimoire::GrimoireState::default();
        state.cards = g.get("cards").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_u64).map(|c| c as u32).collect();
        for r in rows(g.get("stats")) {
            if let [c, n, v] = r {
                if let (Some(c), Some(n), Some(v)) = (c.as_u64(), n.as_u64(), v.as_u64()) {
                    state.stats.insert((c as u32, n as u32), v);
                }
            }
        }
        let account = a.store.identity().account;
        a.overlay.grimoire.insert(account, state);
    }
}

/// The overlay as attributes: `(key, value)` for each of [`OVERLAY_ATTRIBUTES`].
fn overlay_attributes(a: &Adapter<'_>, character: u64) -> Vec<(&'static str, Value)> {
    let o = &a.overlay;
    let mut counters: Vec<_> = o.counters.iter().map(|((w, k, p, i), n)| (owner_word(*w), k.clone(), period_word(*p), *i, *n)).collect();
    counters.sort();
    let mut stacks: Vec<_> = o.stacks.iter().filter(|(_, n)| **n > 0).map(|((w, item), n)| (owner_word(*w), *item, *n)).collect();
    stacks.sort();
    let equipped: std::collections::BTreeSet<u64> = o.characters.get(&character).map(|c| c.equipped.clone()).unwrap_or_default();
    let instances: Vec<Value> = o.instances.values().map(|i| json!({ "item": i, "equipped": equipped.contains(&i.id) })).collect();
    let mut progressions: Vec<_> = o.progressions.iter().filter(|((c, _), _)| *c == character).map(|((_, h), v)| (*h, *v)).collect();
    progressions.sort();
    let mut objectives: Vec<_> = o.objectives.iter().filter(|((c, _), _)| *c == character).map(|((_, h), v)| (*h, *v)).collect();
    objectives.sort();
    let claims: Vec<_> = o.claimed.get(&character).into_iter().flatten().map(|(b, r)| json!([b, r])).collect();
    let account = a.store.identity().account;
    let grimoire = o.grimoire.get(&account).map(|g| {
        json!({
            "cards": g.cards.iter().collect::<Vec<_>>(),
            "stats": g.stats.iter().map(|((c, n), v)| json!([c, n, v])).collect::<Vec<_>>(),
        })
    });
    vec![
        (ATTR_COUNTERS, Value::Array(counters.into_iter().map(|(w, k, p, i, n)| json!([w, k, p, i, n])).collect())),
        (ATTR_STACKS, Value::Array(stacks.into_iter().map(|(w, item, n)| json!([w, item, n])).collect())),
        (ATTR_INSTANCES, Value::Array(instances)),
        (ATTR_PROGRESSIONS, Value::Array(progressions.into_iter().map(|(h, v)| json!([h, v])).collect())),
        (ATTR_OBJECTIVES, Value::Array(objectives.into_iter().map(|(h, v)| json!([h, v])).collect())),
        (ATTR_CLAIMS, Value::Array(claims)),
        (ATTR_GRIMOIRE, grimoire.unwrap_or(Value::Null)),
    ]
}

/// Run one grant on `store` (the signed-on player): stream M's `grant_activity_rewards` through
/// the adapter, with `index` for the content indices and `class_type` the character's class.
/// `strict` (multiplayer on) refuses a grant for another character. The record is marked dirty;
/// the caller saves it.
pub fn execute(store: &mut dyn PlayerStore, cmd: &GrantCommand, index: &dyn ClientIndex, class_type: u32, strict: bool) -> Result<GrantSummary> {
    execute_with(store, cmd, index, class_type, strict, None)
}

/// [`execute`] with an item element template (a decoded `0x80801732`, `D1_VENDOR_ITEM_TEMPLATE`):
/// new instances are written as elements and delivered into a slot of their bucket
/// ([`GrantSummary::delivered`]); without one they are kept in the overlay only.
pub fn execute_with(store: &mut dyn PlayerStore, cmd: &GrantCommand, index: &dyn ClientIndex, class_type: u32, strict: bool, template: Option<&[u8]>) -> Result<GrantSummary> {
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
    let kept: Vec<Option<Value>> = OVERLAY_ATTRIBUTES.iter().map(|k| store.character_attribute(k).cloned()).collect();
    let line = format(cmd);
    let (g, notes, attributes, delivered) = {
        let mut a = Adapter::new(store, index, class_type);
        a.element_template = template.map(<[u8]>::to_vec);
        restore_overlay(&mut a, own, &kept);
        a.deliver_kept_stacks();
        let g = super::grant_activity_rewards(&mut a, own, cmd.activity_hash, &outcome)?;
        a.finish(&format!("rewards: {line}"))?;
        (g, a.notes.clone(), overlay_attributes(&a, own), a.delivered.clone())
    };
    for (k, v) in attributes {
        store.set_character_attribute(k, v);
    }
    let summary = GrantSummary {
        line,
        character: own,
        retargeted,
        currencies: g.currencies.iter().map(|c| (c.item, c.granted)).collect(),
        items: g.items.len(),
        rules: g.rules.clone(),
        notes,
        delivered,
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
    fn handoff_lines_round_trip() {
        let line = "handoff 0xd100000000000101 0 from=0x6eaf0959 hash=0x8ae8da69";
        let f: Vec<&str> = line.split_whitespace().collect();
        let c = parse_handoff(&f[1..]).unwrap();
        assert_eq!(c, HandOffCommand { target: GrantTarget::Key(0xD100_0000_0000_0101), next: 0, from: Some(0x6EAF_0959), next_hash: Some(0x8AE8_DA69) });
        assert_eq!(format_handoff(&c), line);
        let id = format!("id:{}", IDENTITY.replace(' ', ""));
        assert_eq!(parse_handoff(&[&id, "166"]).unwrap().target.character(), Some(0x0101));
        assert!(parse_handoff(&["0x1"]).is_err());
        assert!(parse_handoff(&["0x1", "70000"]).is_err(), "an index is 16-bit");
        assert!(parse_handoff(&["0x1", "0", "to=3"]).is_err());
        assert_eq!(resolve_target(&GrantTarget::Key(5), 7, false).unwrap(), (7, true));
        assert!(resolve_target(&GrantTarget::Key(5), 7, true).is_err());
    }

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
            // Delivered into its client bucket (stream O2), or kept in the overlay.
            let delivered = crate::content::item_index(item).map_or(0, |c| s.currency(c));
            let stacks = s.character_attribute(ATTR_STACKS).and_then(Value::as_array).cloned().unwrap_or_default();
            delivered + stacks.iter().filter(|r| r[1].as_u64() == Some(u64::from(item))).filter_map(|r| r[2].as_u64()).sum::<u64>() as u32
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

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/56578/vendor").join(name)).unwrap()
    }

    /// The lab store with the lab's served profile decoded (`vendor-profile.obj`), so character
    /// buckets can be written.
    fn lab_store_with_profile() -> Store {
        let mut s = lab_store();
        s.chr_mut().profile_object = Some(fixture("vendor-profile.obj"));
        s
    }

    /// Stream O2, wire 1: every currency the rewards grant reaches a client bucket by its exported
    /// content index, none stays in the overlay: account buckets in the account's slots, character
    /// buckets in the profile object.
    #[test]
    fn every_currency_is_delivered_into_its_client_bucket() {
        use crate::rewards::currencies::*;
        let mut s = lab_store_with_profile();
        let me = s.identity().character;
        let index = ResponderIndex::from_slots(&s.record.account.slots);
        let all = [GLIMMER, LEGENDARY_MARKS, SILVER_DUST, STRANGE_COIN, MOTE_OF_LIGHT, WEAPON_PARTS, ARMOR_MATERIALS, EXOTIC_SHARD,
                   ASCENDANT_SHARD, ASCENDANT_ENERGY, VANGUARD_MARKS, CRUCIBLE_MARKS, PLANETARY[0], PLANETARY[1], PLANETARY[2], PLANETARY[3], PLANETARY[4]];
        let before: Vec<u32> = all.iter().map(|h| s.currency(crate::content::item_index(*h).unwrap())).collect();
        let mut granted = Vec::new();
        {
            let mut a = Adapter::new(&mut s, &index, 0);
            for h in all {
                granted.push(grant(&mut a, me, h, 3, &Source::Other).granted);
            }
            assert!(a.overlay.stacks.values().all(|n| *n == 0), "nothing kept in the overlay: {:?}", a.notes);
        }
        // The lab profile holds Silver Dust, Vanguard and Crucible Marks at their manifest caps,
        // so those grant nothing; every other line grants its 3.
        assert_eq!(granted.iter().filter(|g| **g == 3).count(), all.len() - 3, "{granted:?}");
        for ((h, b), g) in all.iter().zip(&before).zip(&granted) {
            let c = crate::content::item_index(*h).unwrap();
            assert_eq!(s.currency(c), b + g, "item {h} (content {c}) is in its bucket");
        }
        // Legendary Marks: account bucket 0x23, slot 310. Weapon Parts: character bucket 0x05
        // (the profile's slots 0-19), so the profile is now stale and is pushed.
        assert_eq!(s.record.account.slots.get(&310).map(|e| e.0), crate::content::item_index(LEGENDARY_MARKS));
        let parts = crate::content::item_index(WEAPON_PARTS).unwrap();
        let p = s.chr().profile_object.clone().unwrap();
        assert!((0..20).any(|slot| crate::vendor::profile_entry(&p, slot) == (parts, 3)));
        assert!(s.chr().profile_stale);
    }

    /// Stacks an earlier grant left in the overlay (no profile to put them in then) are delivered
    /// by the next grant.
    #[test]
    fn stacks_kept_by_an_earlier_grant_are_delivered_later() {
        let mut s = lab_store();
        let me = s.identity().character;
        let index = ResponderIndex::from_slots(&s.record.account.slots);
        let (activity, _) = currency_activity().unwrap();
        let parts = currencies::WEAPON_PARTS;
        let row = json!([[format!("c{me:x}"), parts, 7]]);
        s.set_character_attribute(ATTR_STACKS, row.clone());
        let cmd = GrantCommand { target: GrantTarget::Key(me), activity_hash: activity, outcome: GrantOutcome::Incomplete, page: None, engrams: false, seed: Some(1), source: None };
        execute(&mut s, &cmd, &index, 0, true).unwrap();
        assert_eq!(s.character_attribute(ATTR_STACKS), Some(&row), "no profile yet: still kept");
        s.chr_mut().profile_object = Some(fixture("vendor-profile.obj"));
        let g = execute(&mut s, &cmd, &index, 0, true).unwrap();
        assert_eq!(s.currency(crate::content::item_index(parts).unwrap()), 7);
        assert_eq!(s.character_attribute(ATTR_STACKS), Some(&json!([])));
        assert!(g.notes.iter().any(|n| n.contains("kept from an earlier grant delivered")), "{:?}", g.notes);
    }

    /// Stream O2, wire 2: the whole overlay (instances with grids, progressions, objectives,
    /// Record Book claims, the Grimoire, counters) survives the adapter, through the record.
    #[test]
    fn the_whole_overlay_survives_the_adapter() {
        use crate::rewards::store::RewardStore;
        let mut s = lab_store();
        let me = s.identity().character;
        let index = ResponderIndex::from_slots(&s.record.account.slots);
        let t = tables();
        let mut hashes: Vec<u32> = t.items.values().filter(|d| d.instanced && d.talent_grid != 0).map(|d| d.hash).collect();
        hashes.sort_unstable();
        let inst = crate::rewards::item_instance::ItemInstance::roll(0xD100_0000_0000_7001, hashes[0], 300, &mut crate::rewards::Rng::new(5)).unwrap();
        let card = t.grimoire.iter().find(|c| c.points > 0).unwrap().id;
        let saved = {
            let mut a = Adapter::new(&mut s, &index, 0);
            a.put_item(me, inst.clone());
            a.overlay.equip(me, inst.id);
            a.add_progression(me, 0x1234, 55);
            a.set_objective(me, 0xBEEF, -3);
            a.set_claimed(me, 7, 2);
            let mut g = crate::rewards::grimoire::GrimoireState::default();
            g.cards.insert(card);
            g.stats.insert((card, 1), 9);
            let account = a.store.identity().account;
            a.overlay.grimoire.insert(account, g.clone());
            a.add_counter(Owner::Account(1), "k.bonus", Period::Weekly, 2);
            (overlay_attributes(&a, me), g)
        };
        for (k, v) in &saved.0 {
            s.set_character_attribute(k, v.clone());
        }
        // The record goes through its JSON form, as a save and a restart do.
        let text = serde_json::to_string(&s.record).unwrap();
        s.record = serde_json::from_str(&text).unwrap();
        let kept: Vec<Option<Value>> = OVERLAY_ATTRIBUTES.iter().map(|k| s.character_attribute(k).cloned()).collect();
        let mut a = Adapter::new(&mut s, &index, 0);
        restore_overlay(&mut a, me, &kept);
        let account = a.store.identity().account;
        assert_eq!(a.item(inst.id), Some(&inst), "the instance with its grid and rolls");
        assert_eq!(a.equipped(me), vec![inst.id]);
        assert_eq!(a.progression(me, 0x1234), 55);
        assert_eq!(a.objective(me, 0xBEEF), -3);
        assert!(a.claimed(me).contains(&(7, 2)));
        assert_eq!(a.grimoire(account), saved.1);
        assert_eq!(a.counter(Owner::Account(1), "k.bonus", Period::Weekly), 2);
        assert_eq!(overlay_attributes(&a, me), saved.0, "restored and saved again: the same attributes");
    }

    /// New instances are written from the item template and placed in a slot of their bucket.
    #[test]
    fn a_new_instance_is_delivered_into_its_bucket_with_its_element() {
        use crate::rewards::store::RewardStore;
        let mut s = lab_store_with_profile();
        let me = s.identity().character;
        let index = ResponderIndex::from_slots(&s.record.account.slots);
        let t = tables();
        let character_bucket = |h: u32| {
            crate::content::item_index(h).and_then(crate::content::bucket_of).and_then(crate::vendor::bucket).is_some_and(|b| b.2 == 0)
        };
        let mut hashes: Vec<u32> = t.items.values().filter(|d| d.instanced && character_bucket(d.hash)).map(|d| d.hash).collect();
        hashes.sort_unstable();
        let inst = crate::rewards::item_instance::ItemInstance::roll(0xD100_0000_0000_7002, hashes[0], 300, &mut crate::rewards::Rng::new(5)).unwrap();
        let delivered = {
            let mut a = Adapter::new(&mut s, &index, 0);
            a.element_template = Some(fixture("vendor-item-template.obj"));
            a.put_item(me, inst.clone());
            a.delivered.clone()
        };
        assert_eq!(delivered, vec![inst.id]);
        let c = crate::content::item_index(inst.item_hash).unwrap();
        let p = s.chr().profile_object.clone().unwrap();
        let at = |x: u16| crate::vendor::PROFILE_SLOTS + 32 * x as usize;
        let slot = (0..340u16).find(|x| crate::vendor::profile_entry(&p, *x).0 == c && p[at(*x) + 8..at(*x) + 16] == inst.id.to_le_bytes()).expect("a profile slot holds it");
        let (first, n, _) = crate::vendor::bucket(crate::content::bucket_of(c).unwrap()).unwrap();
        assert!((first..first + n).contains(&slot));
        let e = &s.chr().items.iter().find(|i| i.soid == inst.id).unwrap().element;
        assert_eq!(u16::from_le_bytes([e[0x24], e[0x25]]), c, "the element names the content index");
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
