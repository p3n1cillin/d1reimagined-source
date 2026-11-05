// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! ⭐ **The fireteam service**: create, invite, accept, join, leave, kick, promote, with a leader,
//! a size cap, presence, and joining a fireteam member's activity in progress.
//!
//! ## The rules, and where each one comes from
//!
//! * **Everyone is always in a fireteam.** The client keeps a `fireteam` network session from
//!   sign-on (session index 0 of three: fireteam, posse, group; `ps4/design/peer-and-platform.md`
//!   §3.4), so a signed-on player is a fireteam of one, and leaving a fireteam puts the player back
//!   in a new one of their own.
//! * **The cap is the activity's.** An activity's content carries `minParty`/`maxParty`
//!   (`DestinyActivityDefinition`; `ps4/capture-rig/director-map.py` exports them): 3 for story,
//!   strikes and patrol, 6 for raids. A fireteam with no activity selected uses
//!   [`FireteamRules::default_cap`] (3). The fireteam may be larger than a newly selected
//!   activity allows; it is the **launch** that fails, with the client's own
//!   `error.fireteam_size_requirment_failed` [sic] ([`FireteamError::SizeRequirementFailed`]).
//! * **The leader** is whoever formed the fireteam, then whoever it promotes. If the leader leaves,
//!   leadership passes to the member who joined earliest (the client's own `delegate_leadership`
//!   is peer-side; this is the service's record of it).
//! * **Joining in progress.** Bungie's design: fireteam members always match into the same instance
//!   of a bubble, and a mission's activity host is one per fireteam (GDC 2015,
//!   notes/d1-gdc2015-activity-hosts.md). A player who joins a fireteam that is in an activity is
//!   told that activity's host ([`JoinOutcome::activity`]), and joins it by host id, as the live
//!   client did (activity-host request 0x10 naming the host, then a join of type 2;
//!   notes/d1-live-captures-2026-11-05.md).
//! * **Disconnects keep the slot** for [`FireteamRules::reconnect_grace_ms`], so a dropped player
//!   who signs back on returns to the same fireteam. `[I]` the length: no source gives D1's.
//!
//! Every mutation records an [`Event`]; the responder drains them to log and to push presence.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use crate::identity::{AccountSoid, CharacterSoid, FireteamId};

/// An activity's party bounds, from its content definition (`minParty`, `maxParty`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActivityParty {
    pub min_party: u8,
    pub max_party: u8,
}

/// What the fireteam is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActivityRef {
    /// The activity's content hash.
    pub activity_hash: u32,
    /// Its party bounds, if the content was read.
    pub party: Option<ActivityParty>,
    /// The mission activity host serving it, once launched.
    pub host: Option<u64>,
    /// Whether a fireteam member may join while it runs. Story, strikes, patrol and raids allow it;
    /// a Crucible match in progress does not take fireteam joins (its teams were formed by
    /// matchmaking). Set from the activity's mode.
    pub joinable_in_progress: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FireteamRules {
    /// The cap with no activity selected.
    pub default_cap: usize,
    /// The largest fireteam anything allows (raids: 6). Also the client's fireteam session size.
    pub max_cap: usize,
    /// How long a disconnected member keeps their slot.
    pub reconnect_grace_ms: u64,
}

impl Default for FireteamRules {
    fn default() -> Self {
        Self { default_cap: 3, max_cap: 6, reconnect_grace_ms: 120_000 }
    }
}

/// Who may join without an invite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JoinPolicy {
    /// Anyone may join (the service has no friends list, so "friends" reads as anyone).
    #[default]
    Open,
    /// Only an invite gets in.
    InviteOnly,
    /// Nobody new gets in, not even by invite.
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    Connected,
    /// Dropped at `since_ms`; the slot is kept for the grace period.
    Disconnected { since_ms: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub account: AccountSoid,
    pub character: Option<CharacterSoid>,
    /// Join order within the fireteam (smaller joined earlier).
    pub joined: u64,
    pub presence: Presence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fireteam {
    pub id: FireteamId,
    pub leader: AccountSoid,
    pub members: Vec<Member>,
    pub policy: JoinPolicy,
    pub activity: Option<ActivityRef>,
}

impl Fireteam {
    /// The size cap for joins: the selected activity's `maxParty`, else the default.
    pub fn cap(&self, rules: &FireteamRules) -> usize {
        match self.activity.and_then(|a| a.party) {
            Some(p) => usize::from(p.max_party).clamp(1, rules.max_cap),
            None => rules.default_cap,
        }
    }

    pub fn size(&self) -> usize {
        self.members.len()
    }

    pub fn contains(&self, account: AccountSoid) -> bool {
        self.members.iter().any(|m| m.account == account)
    }

    pub fn member(&self, account: AccountSoid) -> Option<&Member> {
        self.members.iter().find(|m| m.account == account)
    }

    pub fn accounts(&self) -> Vec<AccountSoid> {
        self.members.iter().map(|m| m.account).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InviteId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invite {
    pub id: InviteId,
    pub fireteam: FireteamId,
    pub from: AccountSoid,
    pub to: AccountSoid,
    pub sent_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaveReason {
    Left,
    Kicked,
    /// The disconnect grace ran out.
    TimedOut,
    /// Moved to another fireteam (accepted an invite, joined someone).
    Moved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InviteClosed {
    Accepted,
    Declined,
    /// The fireteam it was for is gone, or the inviter left it, or it closed.
    Withdrawn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Formed { fireteam: FireteamId, leader: AccountSoid },
    Joined { fireteam: FireteamId, account: AccountSoid },
    Left { fireteam: FireteamId, account: AccountSoid, reason: LeaveReason },
    LeaderChanged { fireteam: FireteamId, from: AccountSoid, to: AccountSoid },
    Disbanded { fireteam: FireteamId },
    Invited(Invite),
    InviteClosed { invite: InviteId, reason: InviteClosed },
    Disconnected { fireteam: FireteamId, account: AccountSoid },
    Reconnected { fireteam: FireteamId, account: AccountSoid },
    ActivityChanged { fireteam: FireteamId, activity: Option<ActivityRef> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FireteamError {
    /// The account has no fireteam: it never signed on.
    NotSignedOn(AccountSoid),
    NotAMember { account: AccountSoid, fireteam: FireteamId },
    NotLeader { account: AccountSoid, leader: AccountSoid },
    Full { fireteam: FireteamId, cap: usize },
    /// The fireteam's policy refuses this way in.
    Refused { fireteam: FireteamId, policy: JoinPolicy },
    NoSuchInvite(InviteId),
    /// The invite is for someone else.
    NotYours(InviteId),
    SelfTarget,
    AlreadyTogether,
    /// The fireteam is in an activity that takes no joins in progress.
    NotJoinable { activity_hash: u32 },
    /// The launch's size check: `error.fireteam_size_requirment_failed` in the client.
    SizeRequirementFailed { size: usize, min: u8, max: u8 },
}

impl fmt::Display for FireteamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSignedOn(a) => write!(f, "{a} is not signed on"),
            Self::NotAMember { account, fireteam } => write!(f, "{account} is not in {fireteam}"),
            Self::NotLeader { account, leader } => write!(f, "{account} is not the leader ({leader} is)"),
            Self::Full { fireteam, cap } => write!(f, "{fireteam} is full ({cap})"),
            Self::Refused { fireteam, policy } => write!(f, "{fireteam} is {policy:?}"),
            Self::NoSuchInvite(i) => write!(f, "no invite {}", i.0),
            Self::NotYours(i) => write!(f, "invite {} is for someone else", i.0),
            Self::SelfTarget => write!(f, "a player cannot target themself"),
            Self::AlreadyTogether => write!(f, "already in the same fireteam"),
            Self::NotJoinable { activity_hash } => {
                write!(f, "activity {activity_hash:#010x} takes no joins in progress")
            }
            Self::SizeRequirementFailed { size, min, max } => {
                write!(f, "fireteam_size_requirment_failed: {size} not in {min}..={max}")
            }
        }
    }
}

impl std::error::Error for FireteamError {}

/// Where a join put the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JoinOutcome {
    pub fireteam: FireteamId,
    /// ⭐ The activity the fireteam is in, with its host: the joiner's client joins that host.
    pub activity: Option<ActivityRef>,
}

#[derive(Debug, Clone, Default)]
pub struct FireteamService {
    rules: FireteamRules,
    next_id: u64,
    next_invite: u64,
    next_seq: u64,
    teams: BTreeMap<FireteamId, Fireteam>,
    of: HashMap<AccountSoid, FireteamId>,
    invites: BTreeMap<InviteId, Invite>,
    events: Vec<Event>,
}

/// The top bits of every fireteam id this service allocates, so one is never mistaken for an
/// account, character or activity-host id in a log.
pub const FIRETEAM_ID_BASE: u64 = 0xF17E_0000_0000_0000;

impl FireteamService {
    pub fn new(rules: FireteamRules) -> Self {
        Self { rules, next_id: 1, next_invite: 1, next_seq: 1, ..Self::default() }
    }

    pub fn rules(&self) -> &FireteamRules {
        &self.rules
    }

    /// Take the events recorded since the last call.
    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    pub fn get(&self, id: FireteamId) -> Option<&Fireteam> {
        self.teams.get(&id)
    }

    pub fn fireteam_of(&self, account: AccountSoid) -> Option<&Fireteam> {
        self.of.get(&account).and_then(|id| self.teams.get(id))
    }

    pub fn fireteams(&self) -> impl Iterator<Item = &Fireteam> {
        self.teams.values()
    }

    pub fn invites_for(&self, account: AccountSoid) -> Vec<&Invite> {
        self.invites.values().filter(|i| i.to == account).collect()
    }

    pub fn invite_by_id(&self, id: InviteId) -> Option<&Invite> {
        self.invites.get(&id)
    }

    fn seq(&mut self) -> u64 {
        let s = self.next_seq;
        self.next_seq += 1;
        s
    }

    fn form(&mut self, account: AccountSoid, character: Option<CharacterSoid>) -> FireteamId {
        let id = FireteamId(FIRETEAM_ID_BASE | self.next_id);
        self.next_id += 1;
        let joined = self.seq();
        self.teams.insert(
            id,
            Fireteam {
                id,
                leader: account,
                members: vec![Member { account, character, joined, presence: Presence::Connected }],
                policy: JoinPolicy::default(),
                activity: None,
            },
        );
        self.of.insert(account, id);
        self.events.push(Event::Formed { fireteam: id, leader: account });
        id
    }

    /// ⭐ A player signed on: their fireteam of one, or, if they dropped and are back within the
    /// grace, the fireteam they were in.
    pub fn sign_on(&mut self, account: AccountSoid, character: Option<CharacterSoid>, _now_ms: u64) -> FireteamId {
        if let Some(&id) = self.of.get(&account) {
            let team = self.teams.get_mut(&id).expect("index and teams agree");
            let m = team.members.iter_mut().find(|m| m.account == account).expect("indexed member");
            if character.is_some() {
                m.character = character;
            }
            if matches!(m.presence, Presence::Disconnected { .. }) {
                m.presence = Presence::Connected;
                self.events.push(Event::Reconnected { fireteam: id, account });
            }
            return id;
        }
        self.form(account, character)
    }

    /// The player chose a character (or changed it in orbit).
    pub fn set_character(&mut self, account: AccountSoid, character: CharacterSoid) -> Result<(), FireteamError> {
        let id = *self.of.get(&account).ok_or(FireteamError::NotSignedOn(account))?;
        let team = self.teams.get_mut(&id).expect("indexed");
        if let Some(m) = team.members.iter_mut().find(|m| m.account == account) {
            m.character = Some(character);
        }
        Ok(())
    }

    /// The player's connection dropped. They keep their slot for the grace period; a solo player's
    /// fireteam is simply kept until then too.
    pub fn sign_off(&mut self, account: AccountSoid, now_ms: u64) {
        let Some(&id) = self.of.get(&account) else { return };
        let team = self.teams.get_mut(&id).expect("indexed");
        if let Some(m) = team.members.iter_mut().find(|m| m.account == account) {
            if m.presence == Presence::Connected {
                m.presence = Presence::Disconnected { since_ms: now_ms };
                self.events.push(Event::Disconnected { fireteam: id, account });
            }
        }
        // Their pending invites lapse: an offline player cannot answer one.
        let gone: Vec<InviteId> = self.invites.values().filter(|i| i.to == account).map(|i| i.id).collect();
        for i in gone {
            self.close_invite(i, InviteClosed::Withdrawn);
        }
    }

    /// Expire disconnected members whose grace ran out.
    pub fn tick(&mut self, now_ms: u64) {
        let grace = self.rules.reconnect_grace_ms;
        let expired: Vec<AccountSoid> = self
            .teams
            .values()
            .flat_map(|t| t.members.iter())
            .filter(|m| matches!(m.presence, Presence::Disconnected { since_ms } if now_ms.saturating_sub(since_ms) >= grace))
            .map(|m| m.account)
            .collect();
        for account in expired {
            self.remove_member(account, LeaveReason::TimedOut);
            self.of.remove(&account);
        }
    }

    /// Take `account` out of its fireteam (fixing the leader, disbanding an empty one). The
    /// account is left with no fireteam; callers re-form or move it.
    fn remove_member(&mut self, account: AccountSoid, reason: LeaveReason) -> Option<FireteamId> {
        let id = self.of.remove(&account)?;
        let team = self.teams.get_mut(&id)?;
        team.members.retain(|m| m.account != account);
        self.events.push(Event::Left { fireteam: id, account, reason });
        if team.members.is_empty() {
            self.teams.remove(&id);
            self.events.push(Event::Disbanded { fireteam: id });
            let gone: Vec<InviteId> = self.invites.values().filter(|i| i.fireteam == id).map(|i| i.id).collect();
            for i in gone {
                self.close_invite(i, InviteClosed::Withdrawn);
            }
        } else if team.leader == account {
            let next = team
                .members
                .iter()
                .min_by_key(|m| (m.presence != Presence::Connected, m.joined))
                .map(|m| m.account)
                .expect("non-empty");
            team.leader = next;
            self.events.push(Event::LeaderChanged { fireteam: id, from: account, to: next });
        }
        // Invites the leaver sent for this fireteam are withdrawn with them.
        let gone: Vec<InviteId> =
            self.invites.values().filter(|i| i.fireteam == id && i.from == account).map(|i| i.id).collect();
        for i in gone {
            self.close_invite(i, InviteClosed::Withdrawn);
        }
        Some(id)
    }

    fn close_invite(&mut self, id: InviteId, reason: InviteClosed) {
        if self.invites.remove(&id).is_some() {
            self.events.push(Event::InviteClosed { invite: id, reason });
        }
    }

    fn team_of(&self, account: AccountSoid) -> Result<&Fireteam, FireteamError> {
        self.fireteam_of(account).ok_or(FireteamError::NotSignedOn(account))
    }

    fn check_room(&self, team: &Fireteam) -> Result<(), FireteamError> {
        let cap = team.cap(&self.rules);
        if team.size() >= cap {
            return Err(FireteamError::Full { fireteam: team.id, cap });
        }
        Ok(())
    }

    /// Move `account` into fireteam `to` (already checked), leaving its old one.
    fn move_into(&mut self, account: AccountSoid, to: FireteamId) -> JoinOutcome {
        let character = self.fireteam_of(account).and_then(|t| t.member(account)).and_then(|m| m.character);
        self.remove_member(account, LeaveReason::Moved);
        let joined = self.seq();
        let team = self.teams.get_mut(&to).expect("checked");
        team.members.push(Member { account, character, joined, presence: Presence::Connected });
        self.of.insert(account, to);
        self.events.push(Event::Joined { fireteam: to, account });
        let activity = team.activity;
        // Any other invites to this player close: they are in a fireteam now.
        let gone: Vec<InviteId> = self.invites.values().filter(|i| i.to == account).map(|i| i.id).collect();
        for i in gone {
            self.close_invite(i, InviteClosed::Withdrawn);
        }
        JoinOutcome { fireteam: to, activity }
    }

    /// `from` invites `to` into `from`'s fireteam. Any member may invite; a full or closed
    /// fireteam cannot. The invitee must be signed on.
    pub fn invite(&mut self, from: AccountSoid, to: AccountSoid, now_ms: u64) -> Result<InviteId, FireteamError> {
        if from == to {
            return Err(FireteamError::SelfTarget);
        }
        let team = self.team_of(from)?;
        if team.contains(to) {
            return Err(FireteamError::AlreadyTogether);
        }
        if team.policy == JoinPolicy::Closed {
            return Err(FireteamError::Refused { fireteam: team.id, policy: team.policy });
        }
        self.check_room(team)?;
        let invitee_online = self
            .fireteam_of(to)
            .and_then(|t| t.member(to))
            .is_some_and(|m| m.presence == Presence::Connected);
        if !invitee_online {
            return Err(FireteamError::NotSignedOn(to));
        }
        let fireteam = team.id;
        // One open invite per (fireteam, invitee): a repeat refreshes it.
        if let Some(existing) = self.invites.values().find(|i| i.fireteam == fireteam && i.to == to).map(|i| i.id) {
            return Ok(existing);
        }
        let id = InviteId(self.next_invite);
        self.next_invite += 1;
        let invite = Invite { id, fireteam, from, to, sent_ms: now_ms };
        self.invites.insert(id, invite.clone());
        self.events.push(Event::Invited(invite));
        Ok(id)
    }

    /// The invitee accepts: they leave their fireteam and join the inviter's. The cap is checked
    /// again here, because the fireteam may have filled since the invite went out.
    pub fn accept(&mut self, invite: InviteId, account: AccountSoid) -> Result<JoinOutcome, FireteamError> {
        let inv = self.invites.get(&invite).ok_or(FireteamError::NoSuchInvite(invite))?.clone();
        if inv.to != account {
            return Err(FireteamError::NotYours(invite));
        }
        let team = self.teams.get(&inv.fireteam).ok_or(FireteamError::NoSuchInvite(invite))?;
        if team.policy == JoinPolicy::Closed {
            return Err(FireteamError::Refused { fireteam: team.id, policy: team.policy });
        }
        if let Some(a) = team.activity.filter(|a| a.host.is_some() && !a.joinable_in_progress) {
            return Err(FireteamError::NotJoinable { activity_hash: a.activity_hash });
        }
        self.check_room(team)?;
        self.close_invite(invite, InviteClosed::Accepted);
        Ok(self.move_into(account, inv.fireteam))
    }

    pub fn decline(&mut self, invite: InviteId, account: AccountSoid) -> Result<(), FireteamError> {
        let inv = self.invites.get(&invite).ok_or(FireteamError::NoSuchInvite(invite))?;
        if inv.to != account {
            return Err(FireteamError::NotYours(invite));
        }
        self.close_invite(invite, InviteClosed::Declined);
        Ok(())
    }

    /// ⭐ `joiner` joins `member`'s fireteam without an invite ("join fireteam" on a player), which
    /// is how a player joins a fireteam member **in progress**: if that fireteam is in an
    /// activity, the outcome names its host.
    pub fn join_member(&mut self, joiner: AccountSoid, member: AccountSoid) -> Result<JoinOutcome, FireteamError> {
        if joiner == member {
            return Err(FireteamError::SelfTarget);
        }
        self.team_of(joiner)?;
        let team = self.team_of(member)?;
        if team.contains(joiner) {
            return Err(FireteamError::AlreadyTogether);
        }
        if team.policy != JoinPolicy::Open {
            return Err(FireteamError::Refused { fireteam: team.id, policy: team.policy });
        }
        if let Some(a) = team.activity.filter(|a| a.host.is_some() && !a.joinable_in_progress) {
            return Err(FireteamError::NotJoinable { activity_hash: a.activity_hash });
        }
        self.check_room(team)?;
        let to = team.id;
        Ok(self.move_into(joiner, to))
    }

    /// Leave the fireteam for a new one of the player's own.
    pub fn leave(&mut self, account: AccountSoid) -> Result<FireteamId, FireteamError> {
        let team = self.team_of(account)?;
        if team.size() == 1 {
            return Ok(team.id);
        }
        let character = team.member(account).and_then(|m| m.character);
        self.remove_member(account, LeaveReason::Left);
        Ok(self.form(account, character))
    }

    fn leader_check(&self, leader: AccountSoid) -> Result<&Fireteam, FireteamError> {
        let team = self.team_of(leader)?;
        if team.leader != leader {
            return Err(FireteamError::NotLeader { account: leader, leader: team.leader });
        }
        Ok(team)
    }

    pub fn kick(&mut self, leader: AccountSoid, target: AccountSoid) -> Result<FireteamId, FireteamError> {
        if leader == target {
            return Err(FireteamError::SelfTarget);
        }
        let team = self.leader_check(leader)?;
        if !team.contains(target) {
            return Err(FireteamError::NotAMember { account: target, fireteam: team.id });
        }
        let character = team.member(target).and_then(|m| m.character);
        self.remove_member(target, LeaveReason::Kicked);
        Ok(self.form(target, character))
    }

    pub fn promote(&mut self, leader: AccountSoid, target: AccountSoid) -> Result<(), FireteamError> {
        let team = self.leader_check(leader)?;
        if !team.contains(target) {
            return Err(FireteamError::NotAMember { account: target, fireteam: team.id });
        }
        let id = team.id;
        if leader != target {
            self.teams.get_mut(&id).expect("indexed").leader = target;
            self.events.push(Event::LeaderChanged { fireteam: id, from: leader, to: target });
        }
        Ok(())
    }

    pub fn set_policy(&mut self, leader: AccountSoid, policy: JoinPolicy) -> Result<(), FireteamError> {
        let id = self.leader_check(leader)?.id;
        self.teams.get_mut(&id).expect("indexed").policy = policy;
        Ok(())
    }

    /// The launch check: the fireteam's size against the activity's party bounds.
    pub fn check_launch(&self, fireteam: FireteamId, party: ActivityParty) -> Result<(), FireteamError> {
        let team = self.teams.get(&fireteam).ok_or(FireteamError::NotSignedOn(AccountSoid(0)))?;
        let size = team.size();
        if size < usize::from(party.min_party) || size > usize::from(party.max_party) {
            return Err(FireteamError::SizeRequirementFailed { size, min: party.min_party, max: party.max_party });
        }
        Ok(())
    }

    /// The leader selected (or launched) an activity. Selecting is always allowed; launching
    /// (`activity.host` set) is checked against the party bounds.
    pub fn set_activity(&mut self, leader: AccountSoid, activity: ActivityRef) -> Result<(), FireteamError> {
        let id = self.leader_check(leader)?.id;
        if let (Some(party), Some(_)) = (activity.party, activity.host) {
            self.check_launch(id, party)?;
        }
        self.teams.get_mut(&id).expect("indexed").activity = Some(activity);
        self.events.push(Event::ActivityChanged { fireteam: id, activity: Some(activity) });
        Ok(())
    }

    /// The fireteam is back in orbit.
    pub fn clear_activity(&mut self, fireteam: FireteamId) {
        if let Some(t) = self.teams.get_mut(&fireteam) {
            if t.activity.take().is_some() {
                self.events.push(Event::ActivityChanged { fireteam, activity: None });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: AccountSoid = AccountSoid(0xD101_0000_0000_A001);
    const B: AccountSoid = AccountSoid(0xD101_0000_0000_B001);
    const C: AccountSoid = AccountSoid(0xD101_0000_0000_C001);
    const D: AccountSoid = AccountSoid(0xD101_0000_0000_D001);

    fn signed_on(accounts: &[AccountSoid]) -> FireteamService {
        let mut s = FireteamService::new(FireteamRules::default());
        for &a in accounts {
            s.sign_on(a, Some(CharacterSoid(a.0 + 0x100)), 0);
        }
        s
    }

    const STRIKE: ActivityParty = ActivityParty { min_party: 1, max_party: 3 };
    const RAID: ActivityParty = ActivityParty { min_party: 1, max_party: 6 };

    fn activity(party: ActivityParty, host: Option<u64>) -> ActivityRef {
        ActivityRef { activity_hash: 0x1234_5678, party: Some(party), host, joinable_in_progress: true }
    }

    #[test]
    fn signing_on_is_a_fireteam_of_one_led_by_the_player() {
        let s = signed_on(&[A, B]);
        let a = s.fireteam_of(A).unwrap();
        assert_eq!((a.leader, a.size()), (A, 1));
        assert_ne!(a.id, s.fireteam_of(B).unwrap().id);
        assert_ne!(a.id.0, 0, "fireteam id 0 is the client's 'no fireteam'");
    }

    #[test]
    fn invite_accept_leave_lifecycle() {
        let mut s = signed_on(&[A, B]);
        let fa = s.fireteam_of(A).unwrap().id;
        let fb = s.fireteam_of(B).unwrap().id;
        let inv = s.invite(A, B, 10).unwrap();
        assert_eq!(s.invites_for(B).len(), 1);
        assert_eq!(s.accept(inv, A), Err(FireteamError::NotYours(inv)));
        let out = s.accept(inv, B).unwrap();
        assert_eq!(out.fireteam, fa);
        assert!(s.get(fb).is_none(), "B's solo fireteam disbanded");
        assert_eq!(s.fireteam_of(B).unwrap().accounts(), vec![A, B]);
        assert!(s.invites_for(B).is_empty());
        let events = s.drain_events();
        assert!(events.contains(&Event::Joined { fireteam: fa, account: B }));
        assert!(events.contains(&Event::Disbanded { fireteam: fb }));
        // B leaves: back to a fireteam of its own; A's is A alone again.
        let fb2 = s.leave(B).unwrap();
        assert_ne!(fb2, fa);
        assert_eq!(s.fireteam_of(A).unwrap().accounts(), vec![A]);
        assert_eq!(s.fireteam_of(B).unwrap().leader, B);
    }

    #[test]
    fn the_cap_is_three_and_six_for_a_raid() {
        let mut s = signed_on(&[A, B, C, D]);
        for x in [B, C] {
            let i = s.invite(A, x, 0).unwrap();
            s.accept(i, x).unwrap();
        }
        let fa = s.fireteam_of(A).unwrap().id;
        assert_eq!(s.invite(A, D, 0), Err(FireteamError::Full { fireteam: fa, cap: 3 }));
        // Selecting a raid lifts the cap to its maxParty.
        s.set_activity(A, activity(RAID, None)).unwrap();
        let i = s.invite(A, D, 0).unwrap();
        s.accept(i, D).unwrap();
        assert_eq!(s.fireteam_of(A).unwrap().size(), 4);
        // Four cannot launch a strike: the client's own size requirement.
        assert_eq!(
            s.set_activity(A, activity(STRIKE, Some(0xD1A0_0000_0000_0002))),
            Err(FireteamError::SizeRequirementFailed { size: 4, min: 1, max: 3 })
        );
        // Six is the hard ceiling whatever the content says.
        s.set_activity(A, activity(ActivityParty { min_party: 1, max_party: 12 }, None)).unwrap();
        assert_eq!(s.fireteam_of(A).unwrap().cap(s.rules()), 6);
    }

    #[test]
    fn an_invite_is_rechecked_when_accepted() {
        let mut s = signed_on(&[A, B, C, D]);
        let ib = s.invite(A, B, 0).unwrap();
        let ic = s.invite(A, C, 0).unwrap();
        let id = s.invite(A, D, 0).unwrap();
        s.accept(ib, B).unwrap();
        s.accept(ic, C).unwrap();
        let fa = s.fireteam_of(A).unwrap().id;
        assert_eq!(s.accept(id, D), Err(FireteamError::Full { fireteam: fa, cap: 3 }));
    }

    #[test]
    fn the_leader_passes_to_the_earliest_member_and_only_the_leader_kicks() {
        let mut s = signed_on(&[A, B, C]);
        for x in [B, C] {
            let i = s.invite(A, x, 0).unwrap();
            s.accept(i, x).unwrap();
        }
        assert_eq!(s.kick(B, C), Err(FireteamError::NotLeader { account: B, leader: A }));
        s.leave(A).unwrap();
        let t = s.fireteam_of(B).unwrap();
        assert_eq!((t.leader, t.accounts()), (B, vec![B, C]));
        s.kick(B, C).unwrap();
        assert_eq!(s.fireteam_of(C).unwrap().accounts(), vec![C]);
        s.promote(B, B).unwrap();
    }

    #[test]
    fn joining_a_member_in_progress_names_the_activity_host() {
        let mut s = signed_on(&[A, B]);
        let host = 0xD1A0_0000_0000_0003;
        s.set_activity(A, activity(STRIKE, Some(host))).unwrap();
        let out = s.join_member(B, A).unwrap();
        assert_eq!(out.activity.and_then(|a| a.host), Some(host));
        assert_eq!(s.fireteam_of(A).unwrap().accounts(), vec![A, B]);
    }

    #[test]
    fn a_crucible_match_in_progress_takes_no_joins_and_policies_hold() {
        let mut s = signed_on(&[A, B, C]);
        let pvp = ActivityRef { joinable_in_progress: false, ..activity(ActivityParty { min_party: 1, max_party: 6 }, Some(9)) };
        s.set_activity(A, pvp).unwrap();
        assert_eq!(s.join_member(B, A), Err(FireteamError::NotJoinable { activity_hash: 0x1234_5678 }));
        s.clear_activity(s.fireteam_of(A).unwrap().id);
        s.set_policy(A, JoinPolicy::InviteOnly).unwrap();
        assert!(matches!(s.join_member(B, A), Err(FireteamError::Refused { .. })));
        let i = s.invite(A, B, 0).unwrap();
        s.accept(i, B).unwrap();
        s.set_policy(A, JoinPolicy::Closed).unwrap();
        assert!(matches!(s.invite(A, C, 0), Err(FireteamError::Refused { .. })));
    }

    #[test]
    fn a_disconnect_keeps_the_slot_until_the_grace_runs_out() {
        let mut s = signed_on(&[A, B, C]);
        let i = s.invite(A, B, 0).unwrap();
        s.accept(i, B).unwrap();
        let fa = s.fireteam_of(A).unwrap().id;
        s.sign_off(B, 1_000);
        s.tick(1_000 + 60_000);
        assert!(s.fireteam_of(A).unwrap().contains(B), "kept within the grace");
        // B is back: same fireteam.
        assert_eq!(s.sign_on(B, None, 70_000), fa);
        assert!(s.drain_events().contains(&Event::Reconnected { fireteam: fa, account: B }));
        // This time B stays away past the grace and loses the slot.
        s.sign_off(B, 100_000);
        s.tick(100_000 + s.rules().reconnect_grace_ms);
        assert_eq!(s.fireteam_of(A).unwrap().accounts(), vec![A]);
        assert!(s.fireteam_of(B).is_none());
        // A later sign-on forms a new fireteam.
        assert_ne!(s.sign_on(B, None, 300_000), fa);
        // The leader dropping hands leadership to a connected member, not to a ghost.
        let i = s.invite(A, C, 0).unwrap();
        s.accept(i, C).unwrap();
        s.sign_off(A, 400_000);
        s.tick(400_000 + s.rules().reconnect_grace_ms);
        assert_eq!(s.fireteam_of(C).unwrap().leader, C);
    }

    #[test]
    fn invites_lapse_with_their_fireteam_and_offline_players_cannot_be_invited() {
        let mut s = signed_on(&[A, B]);
        let i = s.invite(A, B, 0).unwrap();
        assert_eq!(s.invite(A, B, 1), Ok(i), "a repeat invite is the same invite");
        s.sign_off(B, 5);
        assert!(s.invite_by_id(i).is_none(), "an offline invitee's invites lapse");
        assert_eq!(s.invite(A, B, 6), Err(FireteamError::NotSignedOn(B)));
        assert_eq!(s.invite(A, A, 6), Err(FireteamError::SelfTarget));
    }
}
