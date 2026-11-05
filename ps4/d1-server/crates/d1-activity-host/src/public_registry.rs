// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! # The public-instance registry (stream G)
//!
//! One record per running public instance - a public bubble of a destination, or a social space -
//! keyed by where it is, with who is in it and who it is holding slots for. It decides, for a
//! player arriving at a public region, whether the player **starts** a new instance (the client's
//! join-request session type 3) or **joins** one (type 2), and which.
//!
//! ## The rules, and where each comes from
//!
//! Bungie's design (GDC 2015, `notes/d1-gdc2015-activity-hosts.md`):
//!
//! * **One bubble host per public instance.** Each instance here has its own activity-host id;
//!   the connection that presents that id is the instance's host.
//! * **Six players a bubble** ([`PUBLIC_BUBBLE_PLAYER_CAP`]). The live public instances each
//!   carried 16 player objects (`0xEAAF16E2` kind `0xC` 0-15), which is the object budget, not the
//!   matchmaking cap.
//! * **Fireteams always land in the same instance**, and each player in an instance **reserves
//!   slots for its absent fireteam members**, which is why live public bubbles averaged about three
//!   players. So an instance's occupancy is its members plus, for every fireteam with someone in
//!   it, the members of that fireteam who are not.
//! * **Fill the emptiest instance first** that has room for the whole fireteam; with none, start a
//!   new one (the standalone instance strangers can join later).
//!
//! The live captures (`notes/ps4-56578-public-instances-2026-11-05.md`):
//!
//! * An instance outlives a player's visit. The client stays connected to an instance it left
//!   until the mission ends, and an instance it joined had been running for minutes (first stamps
//!   of 3,356 and 17,142 at 10 Hz). Here an empty instance is kept for [`EMPTY_INSTANCE_LINGER`]
//!   (PLACEHOLDER) before it is retired.
//! * Leaving a region and coming back can start a new instance: Patrol the Dreadnaught's client
//!   started a second instance for region 8 (`..9720`) 60 s after it started the first.
//! * In Restoration the client created its own instance for region 56 although the search had
//!   found one. Which candidates the live matchmaker offered, and why that one was passed over,
//!   is not in the captures; the emptiest-first rule is Bungie's stated design.
//!
//! ## The bubble swap
//!
//! [`swap_stage`] gives the airlock timeline GDC 2015 describes, seconds before the swap: assets
//! pre-cache from 15 s, matchmaking for the next bubble's instance from 10 s, and at 5 s with
//! nothing found the player starts a standalone instance. Live, the client joined its new public
//! instance 7.5-13 s after the mission host marked the region public (Restoration 12.5 s, The Dark
//! Within 12.4 s, Patrol Venus 7.5 s, Patrol the Dreadnaught 12.4 s), and the mission host
//! advertised it 1.2-1.5 s after the join.
//!
//! The registry is plain data (no sockets, no clock of its own): the matchmaking answer (services,
//! stream H) and the activity-host connections (here) share one behind a mutex.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::open_world::{Destination, SocialSpace};

/// GDC 2015: "the standard bubble is 6 players".
pub const PUBLIC_BUBBLE_PLAYER_CAP: usize = 6;

/// ⚠ **PLACEHOLDER**: how long an instance with nobody in it is kept before it is retired. The live
/// instances outlived their players' visits; how long Bungie kept an empty one is not known.
pub const EMPTY_INSTANCE_LINGER: Duration = Duration::from_secs(300);

/// The airlock timeline, seconds before the bubble swap (GDC 2015, 10:16-13:21).
pub const SWAP_PRECACHE_S: f32 = 15.0;
pub const SWAP_MATCHMAKE_S: f32 = 10.0;
pub const SWAP_STANDALONE_S: f32 = 5.0;

/// Where a player is in the airlock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapStage {
    /// More than 15 s out: nothing yet.
    Idle,
    /// 15-10 s: streaming the next bubble's assets.
    Precache,
    /// 10-5 s: matchmaking for an instance of the next bubble.
    Matchmaking,
    /// Under 5 s with nothing found: start a standalone instance.
    Standalone,
}

/// The airlock stage `seconds_to_swap` before the swap.
pub fn swap_stage(seconds_to_swap: f32) -> SwapStage {
    if seconds_to_swap > SWAP_PRECACHE_S {
        SwapStage::Idle
    } else if seconds_to_swap > SWAP_MATCHMAKE_S {
        SwapStage::Precache
    } else if seconds_to_swap > SWAP_STANDALONE_S {
        SwapStage::Matchmaking
    } else {
        SwapStage::Standalone
    }
}

/// Where an instance is: a destination's public region, or a social space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum InstanceKey {
    /// A public region (slice-set id, `8 * bubble + n`) of a destination.
    Bubble { destination: Destination, region: u32 },
    Social(SocialSpace),
}

impl InstanceKey {
    /// The most players one instance takes.
    pub fn player_cap(self) -> usize {
        match self {
            Self::Bubble { .. } => PUBLIC_BUBBLE_PLAYER_CAP,
            Self::Social(_) => SocialSpace::PLACEHOLDER_PLAYER_CAP,
        }
    }
}

/// A player arriving: the character, its fireteam (0 = none) and the fireteam's size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arrival {
    pub character: u64,
    pub fireteam: u64,
    pub fireteam_size: u8,
}

impl Arrival {
    pub fn solo(character: u64) -> Self {
        Self { character, fireteam: 0, fireteam_size: 1 }
    }
}

/// How the player gets in, as the client's join request says it (`+0x38`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinKind {
    /// Session type 3: the client starts the instance.
    Starts,
    /// Session type 2: the client joins a running one.
    Joins,
}

impl JoinKind {
    pub fn session_type(self) -> u8 {
        match self {
            Self::Starts => 3,
            Self::Joins => 2,
        }
    }
}

/// The registry's answer for one arrival.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub host_id: u64,
    pub kind: JoinKind,
}

/// One running public instance.
#[derive(Debug, Clone)]
pub struct PublicInstance {
    pub host_id: u64,
    pub key: InstanceKey,
    pub started: Instant,
    /// Who is in it.
    members: Vec<Arrival>,
    /// When it last became empty.
    emptied: Option<Instant>,
    /// Whether the mission host has advertised it (the client reported it back, udt 21).
    pub advertised: bool,
}

impl PublicInstance {
    pub fn members(&self) -> impl Iterator<Item = &Arrival> {
        self.members.iter()
    }

    pub fn player_count(&self) -> usize {
        self.members.len()
    }

    /// Slots held for absent fireteam members: for every fireteam with someone here, the members
    /// of it who are not.
    pub fn reserved(&self) -> usize {
        let mut teams: BTreeMap<u64, (u8, usize)> = BTreeMap::new();
        for m in self.members.iter().filter(|m| m.fireteam != 0) {
            let e = teams.entry(m.fireteam).or_insert((m.fireteam_size, 0));
            e.0 = e.0.max(m.fireteam_size);
            e.1 += 1;
        }
        teams.values().map(|&(size, here)| (size as usize).saturating_sub(here)).sum()
    }

    /// Members plus reservations.
    pub fn occupancy(&self) -> usize {
        self.members.len() + self.reserved()
    }

    fn holds_fireteam(&self, fireteam: u64) -> bool {
        fireteam != 0 && self.members.iter().any(|m| m.fireteam == fireteam)
    }
}

/// Why a placement was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaceError {
    /// The fireteam is larger than an instance's cap.
    FireteamTooLarge { size: u8, cap: usize },
}

impl std::fmt::Display for PlaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FireteamTooLarge { size, cap } => {
                write!(f, "a fireteam of {size} does not fit an instance of {cap}")
            }
        }
    }
}

impl std::error::Error for PlaceError {}

/// Every running public instance.
#[derive(Debug)]
pub struct PublicRegistry {
    instances: BTreeMap<u64, PublicInstance>,
    next_id: u64,
}

impl PublicRegistry {
    /// Instance host ids are handed out from `first_id` (non-zero; 0 means "no activity host").
    pub fn new(first_id: u64) -> Self {
        Self { instances: BTreeMap::new(), next_id: first_id.max(1) }
    }

    pub fn get(&self, host_id: u64) -> Option<&PublicInstance> {
        self.instances.get(&host_id)
    }

    pub fn instances(&self) -> impl Iterator<Item = &PublicInstance> {
        self.instances.values()
    }

    pub fn instances_at(&self, key: InstanceKey) -> impl Iterator<Item = &PublicInstance> {
        self.instances.values().filter(move |i| i.key == key)
    }

    /// The instance a character is in, if any.
    pub fn instance_of(&self, character: u64) -> Option<u64> {
        self.instances
            .values()
            .find(|i| i.members.iter().any(|m| m.character == character))
            .map(|i| i.host_id)
    }

    /// ⭐ Place an arriving player at `key`. A player is in one instance at a time, so arriving
    /// anywhere leaves the previous one. In order:
    ///
    /// 1. already in an instance of `key`: stay;
    /// 2. a fireteam member is in one (its slot is reserved there): join it;
    /// 3. the emptiest instance of `key` with room for the whole fireteam (ties: the oldest): join;
    /// 4. otherwise start a new one.
    pub fn place(&mut self, key: InstanceKey, who: Arrival, now: Instant) -> Result<Placement, PlaceError> {
        let cap = key.player_cap();
        let size = who.fireteam_size.max(1);
        if size as usize > cap {
            return Err(PlaceError::FireteamTooLarge { size, cap });
        }
        if let Some(host) = self.instance_of(who.character) {
            if self.instances[&host].key == key {
                return Ok(Placement { host_id: host, kind: JoinKind::Joins });
            }
            self.leave(who.character, now);
        }
        let mate = self
            .instances
            .values()
            .find(|i| i.key == key && i.holds_fireteam(who.fireteam))
            .map(|i| i.host_id);
        let chosen = mate.or_else(|| {
            self.instances
                .values()
                .filter(|i| i.key == key && i.occupancy() + size as usize <= cap)
                .min_by_key(|i| (i.occupancy(), i.host_id))
                .map(|i| i.host_id)
        });
        match chosen {
            Some(host) => {
                let i = self.instances.get_mut(&host).expect("chosen from the map");
                i.members.push(who);
                i.emptied = None;
                Ok(Placement { host_id: host, kind: JoinKind::Joins })
            }
            None => {
                let host = self.next_id;
                self.next_id = self.next_id.wrapping_add(1).max(1);
                self.instances.insert(
                    host,
                    PublicInstance { host_id: host, key, started: now, members: vec![who], emptied: None, advertised: false },
                );
                Ok(Placement { host_id: host, kind: JoinKind::Starts })
            }
        }
    }

    /// An activity-host connection presenting `host_id` joined at `key`. A placement made by
    /// matchmaking already put the character there; a connection nobody placed (a single client
    /// with no matchmaker in front of it) registers its own instance, solo, so the registry still
    /// knows every instance that is running. Returns how the character got in.
    pub fn attach(&mut self, host_id: u64, key: InstanceKey, character: u64, now: Instant) -> JoinKind {
        if let Some(i) = self.instances.get_mut(&host_id) {
            // The instance's first member is the one who started it.
            let starter = i.members.first().map(|m| m.character) == Some(character);
            if !i.members.iter().any(|m| m.character == character) {
                i.members.push(Arrival::solo(character));
                i.emptied = None;
            }
            return if starter { JoinKind::Starts } else { JoinKind::Joins };
        }
        if self.instance_of(character).is_some() {
            self.leave(character, now);
        }
        self.instances.insert(
            host_id,
            PublicInstance { host_id, key, started: now, members: vec![Arrival::solo(character)], emptied: None, advertised: false },
        );
        if host_id >= self.next_id {
            self.next_id = host_id.wrapping_add(1).max(1);
        }
        JoinKind::Starts
    }

    /// The mission host advertised `host_id` (the client reported the instance in its udt 21).
    pub fn mark_advertised(&mut self, host_id: u64) -> bool {
        match self.instances.get_mut(&host_id) {
            Some(i) => {
                i.advertised = true;
                true
            }
            None => false,
        }
    }

    /// A character left its instance (bubble exit, peer leave, lost connection). Returns the
    /// instance it left. Its fireteam's reservation goes with it once no member of that fireteam is
    /// left there.
    pub fn leave(&mut self, character: u64, now: Instant) -> Option<u64> {
        let host = self.instance_of(character)?;
        let i = self.instances.get_mut(&host)?;
        i.members.retain(|m| m.character != character);
        if i.members.is_empty() {
            i.emptied = Some(now);
        }
        Some(host)
    }

    /// Retire instances that have been empty for `linger` or longer. Returns their host ids.
    pub fn retire_idle(&mut self, now: Instant, linger: Duration) -> Vec<u64> {
        let gone: Vec<u64> = self
            .instances
            .values()
            .filter(|i| i.emptied.is_some_and(|t| now.saturating_duration_since(t) >= linger))
            .map(|i| i.host_id)
            .collect();
        for h in &gone {
            self.instances.remove(h);
        }
        gone
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEPPES: InstanceKey = InstanceKey::Bubble { destination: Destination::Earth, region: 56 };
    const MOTHYARDS: InstanceKey = InstanceKey::Bubble { destination: Destination::Earth, region: 48 };
    const FIRST: u64 = 0xD1A0_0000_0000_0100;

    fn team(character: u64, fireteam: u64, size: u8) -> Arrival {
        Arrival { character, fireteam, fireteam_size: size }
    }

    #[test]
    fn the_first_player_starts_an_instance_and_the_next_joins_it() {
        let now = Instant::now();
        let mut r = PublicRegistry::new(FIRST);
        let a = r.place(STEPPES, Arrival::solo(1), now).unwrap();
        assert_eq!(a, Placement { host_id: FIRST, kind: JoinKind::Starts });
        assert_eq!(a.kind.session_type(), 3);
        let b = r.place(STEPPES, Arrival::solo(2), now).unwrap();
        assert_eq!(b, Placement { host_id: FIRST, kind: JoinKind::Joins });
        assert_eq!(b.kind.session_type(), 2);
        // Another region is another instance.
        let c = r.place(MOTHYARDS, Arrival::solo(3), now).unwrap();
        assert_eq!((c.host_id, c.kind), (FIRST + 1, JoinKind::Starts));
    }

    #[test]
    fn an_instance_takes_six_players_then_the_next_one_starts() {
        let now = Instant::now();
        let mut r = PublicRegistry::new(FIRST);
        for c in 1..=6 {
            assert_eq!(r.place(STEPPES, Arrival::solo(c), now).unwrap().host_id, FIRST);
        }
        assert_eq!(r.get(FIRST).unwrap().player_count(), PUBLIC_BUBBLE_PLAYER_CAP);
        let seventh = r.place(STEPPES, Arrival::solo(7), now).unwrap();
        assert_eq!((seventh.host_id, seventh.kind), (FIRST + 1, JoinKind::Starts));
    }

    #[test]
    fn the_emptiest_instance_fills_first() {
        let now = Instant::now();
        let mut r = PublicRegistry::new(FIRST);
        for c in 1..=6 {
            r.place(STEPPES, Arrival::solo(c), now).unwrap();
        }
        r.place(STEPPES, Arrival::solo(7), now).unwrap(); // starts FIRST + 1
        // Three leave the first: it has 3, the second 1. The next player goes to the second.
        for c in 1..=3 {
            r.leave(c, now);
        }
        assert_eq!(r.place(STEPPES, Arrival::solo(8), now).unwrap().host_id, FIRST + 1);
        // Both at 2-3 now: 3 vs 2, still the second; then they are level (3 and 3) and the
        // older (lower id) wins.
        assert_eq!(r.place(STEPPES, Arrival::solo(9), now).unwrap().host_id, FIRST + 1);
        assert_eq!(r.place(STEPPES, Arrival::solo(10), now).unwrap().host_id, FIRST);
    }

    #[test]
    fn a_player_reserves_slots_for_its_absent_fireteam_and_they_join_it() {
        let now = Instant::now();
        let mut r = PublicRegistry::new(FIRST);
        // One of a fireteam of three arrives: one member, two reserved.
        r.place(STEPPES, team(1, 0xF1, 3), now).unwrap();
        let i = r.get(FIRST).unwrap();
        assert_eq!((i.player_count(), i.reserved(), i.occupancy()), (1, 2, 3));
        // Three strangers fill it to six with the reservations held.
        for c in 10..=12 {
            assert_eq!(r.place(STEPPES, Arrival::solo(c), now).unwrap().host_id, FIRST);
        }
        assert_eq!(r.get(FIRST).unwrap().occupancy(), 6);
        // A fourth stranger cannot take a reserved slot.
        assert_eq!(r.place(STEPPES, Arrival::solo(13), now).unwrap().host_id, FIRST + 1);
        // The fireteam's other members land with their mate.
        assert_eq!(r.place(STEPPES, team(2, 0xF1, 3), now).unwrap().host_id, FIRST);
        assert_eq!(r.place(STEPPES, team(3, 0xF1, 3), now).unwrap().host_id, FIRST);
        let i = r.get(FIRST).unwrap();
        assert_eq!((i.player_count(), i.reserved()), (6, 0));
    }

    #[test]
    fn a_fireteam_that_does_not_fit_starts_its_own_instance() {
        let now = Instant::now();
        let mut r = PublicRegistry::new(FIRST);
        for c in 1..=4 {
            r.place(STEPPES, Arrival::solo(c), now).unwrap();
        }
        // A fireteam of three needs three slots; two are free.
        let p = r.place(STEPPES, team(9, 0xF2, 3), now).unwrap();
        assert_eq!((p.host_id, p.kind), (FIRST + 1, JoinKind::Starts));
        assert_eq!(
            r.place(STEPPES, team(20, 0xF3, 7), now),
            Err(PlaceError::FireteamTooLarge { size: 7, cap: 6 })
        );
    }

    #[test]
    fn leaving_drops_the_reservation_with_the_last_member_and_idle_instances_retire() {
        let t0 = Instant::now();
        let mut r = PublicRegistry::new(FIRST);
        r.place(STEPPES, team(1, 0xF1, 3), t0).unwrap();
        assert_eq!(r.leave(1, t0), Some(FIRST));
        let i = r.get(FIRST).unwrap();
        assert_eq!((i.player_count(), i.reserved()), (0, 0));
        assert!(r.retire_idle(t0 + Duration::from_secs(10), EMPTY_INSTANCE_LINGER).is_empty());
        assert_eq!(r.retire_idle(t0 + EMPTY_INSTANCE_LINGER, EMPTY_INSTANCE_LINGER), vec![FIRST]);
        assert!(r.get(FIRST).is_none());
        // Arriving somewhere else leaves the old instance.
        r.place(STEPPES, Arrival::solo(5), t0).unwrap();
        r.place(MOTHYARDS, Arrival::solo(5), t0).unwrap();
        assert_eq!(r.instances_at(STEPPES).next().unwrap().player_count(), 0);
        assert_eq!(r.instance_of(5).map(|h| r.get(h).unwrap().key), Some(MOTHYARDS));
    }

    #[test]
    fn a_connection_nobody_placed_registers_its_own_instance() {
        let t0 = Instant::now();
        let mut r = PublicRegistry::new(FIRST);
        let host = 0xD1A0_0000_0000_0002;
        assert_eq!(r.attach(host, STEPPES, 1, t0), JoinKind::Starts);
        assert_eq!(r.get(host).unwrap().player_count(), 1);
        // A second client joining the same host id is a join, and new ids skip past it.
        assert_eq!(r.attach(host, STEPPES, 2, t0 + Duration::from_secs(25)), JoinKind::Joins);
        assert!(r.mark_advertised(host));
        assert!(r.get(host).unwrap().advertised);
        assert!(r.place(MOTHYARDS, Arrival::solo(3), t0).unwrap().host_id > host);
    }

    #[test]
    fn the_airlock_stages_follow_bungies_timeline() {
        assert_eq!(swap_stage(20.0), SwapStage::Idle);
        assert_eq!(swap_stage(15.0), SwapStage::Precache);
        assert_eq!(swap_stage(12.0), SwapStage::Precache);
        assert_eq!(swap_stage(10.0), SwapStage::Matchmaking);
        assert_eq!(swap_stage(5.0), SwapStage::Standalone);
        assert_eq!(swap_stage(0.0), SwapStage::Standalone);
    }

    #[test]
    fn social_spaces_are_instances_too() {
        let now = Instant::now();
        let mut r = PublicRegistry::new(FIRST);
        let tower = InstanceKey::Social(SocialSpace::Tower);
        for c in 1..=SocialSpace::PLACEHOLDER_PLAYER_CAP as u64 {
            assert_eq!(r.place(tower, Arrival::solo(c), now).unwrap().host_id, FIRST);
        }
        assert_eq!(r.place(tower, Arrival::solo(99), now).unwrap().kind, JoinKind::Starts);
    }
}
