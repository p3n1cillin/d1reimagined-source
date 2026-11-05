// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! ⭐ **Several clients in one activity instance**: shared membership, peer slots, entity-index
//! ranges, and physics-host election through the activity host.
//!
//! Until now every activity-host connection was its own instance with one member, the client
//! itself ([`crate::messages::ReplicateMembership::solo`]). Bungie's model is one mission host per
//! fireteam and one bubble host per public instance, each holding every player in it (GDC 2015,
//! notes/d1-gdc2015-activity-hosts.md). This module is that shared instance, keyed by activity
//! host id, behind `D1_AH_SHARED_INSTANCES=1` (off by default; [`GroupLink`] is the hook).
//!
//! ## Who is in which instance
//!
//! A client's `join_request` names the host it is joining at `+0x08`. A client starting an
//! instance names the host it was given (message 7); a client joining another player's instance
//! names that player's host, which it got by id (activity-host request 0x10 with the host id, a
//! join of type 2: the live Restoration capture). So **every connection whose join request names
//! the same host is in the same instance**, and the instance is the [`InstanceGroup`] under that id.
//!
//! ## What each member is told
//!
//! One `replicate_membership` image, the same for everyone (udt 11, `0x5038` bytes):
//!
//! * member slot `i` holds member `i`'s 40-byte identity and 184-byte properties prefix, each
//!   echoed from **that member's own** join request, as the solo membership echoes one;
//! * the occupancy masks (`+0x502C` and the simulation mask `+0x5028`) have bit `i` for each
//!   occupied slot: `FUN_0029FE70` finds the local member by walking the set bits, so each client
//!   finds itself wherever it sits;
//! * `+0x5020`, the acknowledged peer-session host, is the elected physics host's identity;
//! * `+0x00`, the update number, advances on every change ([`InstanceGroup::generation`]): the
//!   client re-reads the membership when it moves (56578 `0x778D60`).
//!
//! ## Entity indices
//!
//! Each live host granted a client 500 indices **by its peer slot**: slot 0 got 0-499, and the
//! second peer of another player's instance got 500-999 (notes/d1-live-captures-2026-11-05.md).
//! [`entity_range`] is that rule. `[I]` the 500: it held for every host in the captures, and the
//! client's own pool is 12,288 (`ENTITY_INDEX_COUNT`), room for the 20 member slots at 500 each
//! only up to slot 24, which the 20-slot cap never reaches.
//!
//! ## The physics host
//!
//! "The activity host arbitrates which console becomes physics host, and every migration goes
//! through it" (GDC 2015). The first member is elected. When the physics host leaves, the member
//! who joined earliest is elected and the change is a [`Migration`], which the instance answers
//! with a reconcile: the new host is sent the current authoritative state. A member may claim
//! the role ([`InstanceGroups::claim_physics_host`]); the claim is granted only when nobody holds
//! it, so the host does not flap between consoles.

use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::sync::{Arc, Mutex};

use crate::entity::{EntitySlotMask, ENTITY_INDEX_COUNT};
use crate::messages::ReplicateMembership as RM;

/// Member slots in a membership image (`11840 / 592`).
pub const MEMBER_CAPACITY: usize = 20;
/// Entity indices granted per peer slot, as every live host did.
pub const ENTITY_INDICES_PER_PEER: usize = 500;
/// Where a `join_request` names the activity host it joins.
pub const JOIN_REQUEST_AH_ID: usize = 0x08;

/// The entity indices of peer slot `slot`.
pub fn entity_range(slot: usize) -> Range<usize> {
    let start = (slot * ENTITY_INDICES_PER_PEER).min(ENTITY_INDEX_COUNT);
    start..(start + ENTITY_INDICES_PER_PEER).min(ENTITY_INDEX_COUNT)
}

/// [`entity_range`] as a mask.
pub fn entity_mask(slot: usize) -> EntitySlotMask {
    let mut m = EntitySlotMask::empty();
    for i in entity_range(slot) {
        m.set(i);
    }
    m
}

/// The host a join request names, if it names one.
pub fn requested_host(join_request_image: &[u8]) -> Option<u64> {
    let b = join_request_image.get(JOIN_REQUEST_AH_ID..JOIN_REQUEST_AH_ID + 8)?;
    Some(u64::from_le_bytes(b.try_into().ok()?)).filter(|&h| h != 0)
}

/// One member of a shared instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// The connection (the listener numbers them).
    pub conn: u64,
    pub slot: usize,
    pub identity: [u8; RM::IDENTITY_SIZE],
    pub properties: Vec<u8>,
    joined: u64,
}

impl Member {
    pub fn account(&self) -> u64 {
        u64::from_le_bytes(self.identity[0x10..0x18].try_into().unwrap())
    }

    pub fn character(&self) -> u64 {
        u64::from_le_bytes(self.identity[0x18..0x20].try_into().unwrap())
    }

    pub fn fireteam(&self) -> u64 {
        u64::from_le_bytes(self.properties[0xA0..0xA8].try_into().unwrap())
    }
}

/// A physics-host change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    pub from: Option<usize>,
    pub to: Option<usize>,
}

/// One shared instance.
#[derive(Debug, Clone, Default)]
pub struct InstanceGroup {
    members: BTreeMap<usize, Member>,
    physics_host: Option<usize>,
    generation: u64,
    next_join: u64,
}

impl InstanceGroup {
    pub fn members(&self) -> impl Iterator<Item = &Member> {
        self.members.values()
    }

    pub fn len(&self) -> usize {
        self.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    pub fn physics_host(&self) -> Option<usize> {
        self.physics_host
    }

    /// Advances on every membership or host change.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    fn elect_earliest(&mut self) -> Migration {
        let from = self.physics_host;
        self.physics_host = self.members.values().min_by_key(|m| m.joined).map(|m| m.slot);
        Migration { from, to: self.physics_host }
    }

    /// ⭐ Write this instance's members into a membership image built for one of them (the solo
    /// image: its own identity in slot 0). Everything outside the member array, the masks and the
    /// host acknowledgement is left as the caller built it.
    pub fn compose(&self, image: &mut [u8]) -> bool {
        if image.len() < RM::STRUCT_SIZE {
            return false;
        }
        let array = RM::MEMBERS..RM::MEMBERS + MEMBER_CAPACITY * RM::MEMBER_STRIDE;
        image[array].fill(0);
        let mut mask = 0u32;
        for m in self.members.values() {
            let base = RM::MEMBERS + m.slot * RM::MEMBER_STRIDE;
            image[base..base + RM::IDENTITY_SIZE].copy_from_slice(&m.identity);
            let p = base + RM::MEMBER_PROPERTIES;
            image[p..p + m.properties.len()].copy_from_slice(&m.properties);
            mask |= 1 << m.slot;
        }
        image[RM::SIMULATION_OCCUPANCY_MASK..RM::SIMULATION_OCCUPANCY_MASK + 4].copy_from_slice(&mask.to_le_bytes());
        image[RM::OCCUPANCY_MASK..RM::OCCUPANCY_MASK + 4].copy_from_slice(&mask.to_le_bytes());
        let ack = RM::ACKNOWLEDGED_SESSION_HOST;
        match self.physics_host.and_then(|s| self.members.get(&s)) {
            Some(h) => image[ack..ack + 8].copy_from_slice(&h.identity[..8]),
            None => image[ack..ack + 8].fill(0),
        }
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupError {
    /// The join request is too short to carry an identity and properties.
    ShortJoinRequest(usize),
    /// All 20 member slots are taken.
    Full { host: u64 },
}

impl std::fmt::Display for GroupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ShortJoinRequest(n) => write!(f, "a {n}-byte join request carries no identity and properties"),
            Self::Full { host } => write!(f, "activity host {host:#018x} has all {MEMBER_CAPACITY} member slots taken"),
        }
    }
}

impl std::error::Error for GroupError {}

/// What a join gave the member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Joined {
    pub slot: usize,
    pub physics_host: Option<usize>,
    pub generation: u64,
    /// Set when this join elected a physics host (the instance was empty).
    pub migration: Option<Migration>,
}

/// Every shared instance, by activity host id.
#[derive(Debug, Default)]
pub struct InstanceGroups {
    groups: HashMap<u64, InstanceGroup>,
}

pub type SharedGroups = Arc<Mutex<InstanceGroups>>;

impl InstanceGroups {
    pub fn shared() -> SharedGroups {
        Arc::new(Mutex::new(Self::default()))
    }

    pub fn get(&self, host: u64) -> Option<&InstanceGroup> {
        self.groups.get(&host)
    }

    /// `conn` joins `host` with its join request. A connection joining again (a re-join on the
    /// same connection) keeps its slot and refreshes its identity.
    pub fn join(&mut self, host: u64, conn: u64, join_request_image: &[u8]) -> Result<Joined, GroupError> {
        let id = join_request_image
            .get(RM::JOIN_REQUEST_IDENTITY..RM::JOIN_REQUEST_IDENTITY + RM::IDENTITY_SIZE)
            .ok_or(GroupError::ShortJoinRequest(join_request_image.len()))?;
        let props = join_request_image
            .get(RM::JOIN_REQUEST_PROPERTIES..RM::JOIN_REQUEST_PROPERTIES + RM::PROPERTIES_PREFIX_SIZE)
            .ok_or(GroupError::ShortJoinRequest(join_request_image.len()))?;
        let g = self.groups.entry(host).or_default();
        let slot = match g.members.values().find(|m| m.conn == conn).map(|m| m.slot) {
            Some(s) => s,
            None => (0..MEMBER_CAPACITY).find(|s| !g.members.contains_key(s)).ok_or(GroupError::Full { host })?,
        };
        let joined = match g.members.get(&slot) {
            Some(m) => m.joined,
            None => {
                g.next_join += 1;
                g.next_join
            }
        };
        g.members.insert(slot, Member { conn, slot, identity: id.try_into().unwrap(), properties: props.to_vec(), joined });
        let migration = if g.physics_host.is_none() { Some(g.elect_earliest()) } else { None };
        g.generation += 1;
        Ok(Joined { slot, physics_host: g.physics_host, generation: g.generation, migration })
    }

    /// `conn` left `host`. Returns the migration if it was the physics host. An emptied instance
    /// is forgotten (its scripts' state belongs to the instance, which reconciles a new arrival).
    pub fn leave(&mut self, host: u64, conn: u64) -> Option<Migration> {
        let g = self.groups.get_mut(&host)?;
        let slot = g.members.values().find(|m| m.conn == conn)?.slot;
        g.members.remove(&slot);
        g.generation += 1;
        let migration = (g.physics_host == Some(slot)).then(|| g.elect_earliest());
        if g.members.is_empty() {
            self.groups.remove(&host);
        }
        migration
    }

    /// A member claims the physics host role. Granted only when no member holds it.
    pub fn claim_physics_host(&mut self, host: u64, conn: u64) -> bool {
        let Some(g) = self.groups.get_mut(&host) else { return false };
        let Some(slot) = g.members.values().find(|m| m.conn == conn).map(|m| m.slot) else { return false };
        match g.physics_host {
            Some(s) if s == slot => true,
            Some(s) if g.members.contains_key(&s) => false,
            _ => {
                g.physics_host = Some(slot);
                g.generation += 1;
                true
            }
        }
    }

    /// The activity host moves the role (a migration it decided, e.g. the host's link degraded).
    pub fn elect(&mut self, host: u64, slot: usize) -> Option<Migration> {
        let g = self.groups.get_mut(&host)?;
        if !g.members.contains_key(&slot) || g.physics_host == Some(slot) {
            return None;
        }
        let from = g.physics_host.replace(slot);
        g.generation += 1;
        Some(Migration { from, to: Some(slot) })
    }

    pub fn compose(&self, host: u64, image: &mut [u8]) -> bool {
        self.groups.get(&host).is_some_and(|g| g.compose(image))
    }

    pub fn generation(&self, host: u64) -> u64 {
        self.groups.get(&host).map_or(0, |g| g.generation)
    }

    /// The peer list: `(slot, conn, account, is physics host)`.
    pub fn peers(&self, host: u64) -> Vec<(usize, u64, u64, bool)> {
        self.groups
            .get(&host)
            .map(|g| g.members.values().map(|m| (m.slot, m.conn, m.account(), g.physics_host == Some(m.slot))).collect())
            .unwrap_or_default()
    }
}

/// ⭐ **The hook**: one connection's link to the shared instances. Leaving happens on drop, so a
/// closed connection always leaves its instance.
#[derive(Debug)]
pub struct GroupLink {
    groups: SharedGroups,
    conn: u64,
    host: Option<u64>,
    slot: Option<usize>,
    seen_generation: u64,
}

impl GroupLink {
    pub fn new(groups: SharedGroups, conn: u64) -> Self {
        Self { groups, conn, host: None, slot: None, seen_generation: 0 }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, InstanceGroups> {
        self.groups.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Join the instance the request names (else `own_host`). Returns the host joined.
    pub fn join(&mut self, own_host: u64, join_request_image: &[u8]) -> Result<(u64, Joined), GroupError> {
        let host = requested_host(join_request_image).unwrap_or(own_host);
        if let Some(old) = self.host.filter(|h| *h != host) {
            self.lock().leave(old, self.conn);
        }
        let j = self.lock().join(host, self.conn, join_request_image)?;
        self.host = Some(host);
        self.slot = Some(j.slot);
        self.seen_generation = j.generation;
        Ok((host, j))
    }

    pub fn host(&self) -> Option<u64> {
        self.host
    }

    pub fn slot(&self) -> Option<usize> {
        self.slot
    }

    /// Rewrite a membership image with the whole instance.
    pub fn compose(&self, image: &mut [u8]) -> bool {
        self.host.is_some_and(|h| self.lock().compose(h, image))
    }

    /// Whether the instance changed since this connection last published its membership.
    pub fn take_changed(&mut self) -> bool {
        let Some(h) = self.host else { return false };
        let g = self.lock().generation(h);
        let changed = g != self.seen_generation;
        self.seen_generation = g;
        changed
    }

    pub fn is_physics_host(&self) -> bool {
        match (self.host, self.slot) {
            (Some(h), Some(s)) => self.lock().get(h).and_then(InstanceGroup::physics_host) == Some(s),
            _ => false,
        }
    }

    pub fn peers(&self) -> Vec<(usize, u64, u64, bool)> {
        self.host.map(|h| self.lock().peers(h)).unwrap_or_default()
    }
}

impl Drop for GroupLink {
    fn drop(&mut self) {
        if let Some(h) = self.host {
            let m = self.lock().leave(h, self.conn);
            tracing::info!("shared instance {h:#018x}: connection {} left{}", self.conn,
                m.map(|m| format!(", physics host {:?} -> {:?}", m.from, m.to)).unwrap_or_default());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic join request: identity `{machine, player, account, character, fireteam}` at
    /// `+0x10`, properties at `+0x38` with the fireteam id at `+0xA0`, the host at `+0x08`.
    fn join_request(host: u64, account: u64, fireteam: u64) -> Vec<u8> {
        let mut b = vec![0u8; 0x100];
        b[0..4].copy_from_slice(&(account as u32).to_le_bytes());
        b[8..16].copy_from_slice(&host.to_le_bytes());
        b[0x10..0x18].copy_from_slice(&(account ^ 0x5555).to_le_bytes()); // machine
        b[0x18..0x20].copy_from_slice(&(account ^ 0xAAAA).to_le_bytes()); // player
        b[0x20..0x28].copy_from_slice(&account.to_le_bytes());
        b[0x28..0x30].copy_from_slice(&(account + 0x100).to_le_bytes());
        b[0x30..0x38].copy_from_slice(&fireteam.to_le_bytes());
        b[0x38] = 3;
        b[0x38 + 0xA0..0x38 + 0xA8].copy_from_slice(&fireteam.to_le_bytes());
        b
    }

    const HOST: u64 = 0xD1A0_0000_0000_0001;

    fn solo_image(req: &[u8]) -> Vec<u8> {
        RM::solo(req).unwrap().image
    }

    fn member_account(image: &[u8], slot: usize) -> u64 {
        let o = RM::MEMBERS + slot * RM::MEMBER_STRIDE + 0x10;
        u64::from_le_bytes(image[o..o + 8].try_into().unwrap())
    }

    fn mask(image: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(image[at..at + 4].try_into().unwrap())
    }

    #[test]
    fn two_clients_in_one_instance_see_the_same_membership() {
        let groups = InstanceGroups::shared();
        let ra = join_request(HOST, 0xA, 0xF17E_0000_0000_0001);
        let rb = join_request(HOST, 0xB, 0xF17E_0000_0000_0001);
        let mut a = GroupLink::new(groups.clone(), 1);
        let mut b = GroupLink::new(groups.clone(), 2);
        let (ha, ja) = a.join(0x1111, &ra).unwrap();
        let (hb, jb) = b.join(0x2222, &rb).unwrap();
        assert_eq!((ha, hb), (HOST, HOST), "both named the same host, so they share it");
        assert_eq!((ja.slot, jb.slot), (0, 1));
        assert!(a.is_physics_host() && !b.is_physics_host());
        let mut ia = solo_image(&ra);
        let mut ib = solo_image(&rb);
        assert!(a.compose(&mut ia) && b.compose(&mut ib));
        assert_eq!(ia, ib, "one membership, the same bytes for every member");
        assert_eq!((member_account(&ia, 0), member_account(&ia, 1)), (0xA, 0xB));
        assert_eq!(mask(&ia, RM::OCCUPANCY_MASK), 0b11);
        assert_eq!(mask(&ia, RM::SIMULATION_OCCUPANCY_MASK), 0b11);
        assert_eq!(&ia[RM::ACKNOWLEDGED_SESSION_HOST..RM::ACKNOWLEDGED_SESSION_HOST + 8], &ra[0x10..0x18], "A's identity: the physics host");
        // Each member's own fireteam id travels in its own properties.
        let fo = RM::MEMBERS + RM::MEMBER_STRIDE + RM::MEMBER_PROPERTIES + 0xA0;
        assert_eq!(u64::from_le_bytes(ia[fo..fo + 8].try_into().unwrap()), 0xF17E_0000_0000_0001);
        // A saw B arrive.
        assert!(a.take_changed());
        assert!(!a.take_changed());
        assert_eq!(a.peers().len(), 2);
    }

    #[test]
    fn a_leaving_physics_host_migrates_to_the_earliest_member_and_the_slot_is_reused() {
        let groups = InstanceGroups::shared();
        let mut a = GroupLink::new(groups.clone(), 1);
        let mut b = GroupLink::new(groups.clone(), 2);
        let mut c = GroupLink::new(groups.clone(), 3);
        a.join(0, &join_request(HOST, 0xA, 1)).unwrap();
        b.join(0, &join_request(HOST, 0xB, 1)).unwrap();
        c.join(0, &join_request(HOST, 0xC, 2)).unwrap();
        let before = groups.lock().unwrap().generation(HOST);
        drop(a);
        assert_eq!(groups.lock().unwrap().get(HOST).unwrap().physics_host(), Some(1), "B joined next");
        assert!(groups.lock().unwrap().generation(HOST) > before);
        assert!(b.is_physics_host() && b.take_changed() && c.take_changed());
        // A reconnects: a new connection, the free slot 0, and no host change.
        let mut a2 = GroupLink::new(groups.clone(), 4);
        let (_, j) = a2.join(0, &join_request(HOST, 0xA, 1)).unwrap();
        assert_eq!((j.slot, j.physics_host, j.migration), (0, Some(1), None));
        let mut img = solo_image(&join_request(HOST, 0xA, 1));
        a2.compose(&mut img);
        assert_eq!(mask(&img, RM::OCCUPANCY_MASK), 0b111);
        assert_eq!(&img[RM::ACKNOWLEDGED_SESSION_HOST..RM::ACKNOWLEDGED_SESSION_HOST + 8], &join_request(HOST, 0xB, 1)[0x10..0x18]);
        drop((b, c, a2));
        assert!(groups.lock().unwrap().get(HOST).is_none(), "an empty instance is forgotten");
    }

    #[test]
    fn claims_are_granted_only_to_an_unheld_role_and_the_host_may_elect() {
        let mut g = InstanceGroups::default();
        g.join(HOST, 1, &join_request(HOST, 0xA, 1)).unwrap();
        g.join(HOST, 2, &join_request(HOST, 0xB, 1)).unwrap();
        assert!(!g.claim_physics_host(HOST, 2), "A holds it");
        assert!(g.claim_physics_host(HOST, 1));
        assert_eq!(g.elect(HOST, 1), Some(Migration { from: Some(0), to: Some(1) }));
        assert_eq!(g.elect(HOST, 1), None);
        assert_eq!(g.leave(HOST, 2), Some(Migration { from: Some(1), to: Some(0) }));
        assert!(!g.claim_physics_host(0x9, 1));
    }

    #[test]
    fn different_hosts_are_different_instances_and_capacity_is_twenty() {
        let mut g = InstanceGroups::default();
        g.join(HOST, 1, &join_request(HOST, 0xA, 1)).unwrap();
        g.join(HOST + 1, 2, &join_request(HOST + 1, 0xB, 2)).unwrap();
        assert_eq!(g.peers(HOST).len(), 1);
        assert_eq!(g.peers(HOST + 1)[0].0, 0, "slot 0 of its own instance");
        for c in 3..3 + 19 {
            g.join(HOST, c, &join_request(HOST, c, 1)).unwrap();
        }
        assert_eq!(g.join(HOST, 99, &join_request(HOST, 99, 1)), Err(GroupError::Full { host: HOST }));
        assert!(matches!(g.join(HOST, 100, &[0u8; 0x20]), Err(GroupError::ShortJoinRequest(0x20))));
        // A re-join on the same connection keeps its slot.
        let j = g.join(HOST, 1, &join_request(HOST, 0xA, 1)).unwrap();
        assert_eq!(j.slot, 0);
    }

    #[test]
    fn entity_ranges_follow_the_peer_slot() {
        assert_eq!(entity_range(0), 0..500);
        assert_eq!(entity_range(1), 500..1000);
        assert_eq!(entity_range(19), 9500..10000);
        assert_eq!(entity_mask(1).count(), 500);
        assert!(entity_mask(1).get(500) && !entity_mask(1).get(499));
        assert_eq!(requested_host(&join_request(HOST, 1, 1)), Some(HOST));
        assert_eq!(requested_host(&join_request(0, 1, 1)), None);
    }

    /// ⭐ The hook end to end: two activity instances (two connections) whose clients name the
    /// same host get one membership with both members, each its own entity range, and the first
    /// sees the second arrive and leave through its own membership re-publish.
    #[test]
    fn two_connections_through_activity_instances_share_membership() {
        use crate::messages::{AhMessage, ClientMessage};
        use crate::state::ActivityInstance;
        use std::time::{Duration, Instant};
        let groups = InstanceGroups::shared();
        let now = Instant::now();
        let mut a = ActivityInstance::new(HOST, "d1ah-a", now);
        let mut b = ActivityInstance::new(HOST + 7, "d1ah-b", now);
        a.set_group(GroupLink::new(groups.clone(), 0));
        b.set_group(GroupLink::new(groups.clone(), 1));
        let join = |i: &mut ActivityInstance, account: u64| {
            i.on_connect(now);
            i.on_client_message(&ClientMessage::JoinRequest { struct_image: join_request(HOST, account, 0xF17E_0000_0000_0001) }, now)
        };
        let membership = |pushes: &[crate::push::AhPush]| pushes.iter().find_map(|p| match &p.message {
            AhMessage::ReplicateMembership(m) => Some(m.image.clone()),
            _ => None,
        });
        let grant = |pushes: &[crate::push::AhPush]| pushes.iter().find_map(|p| match &p.message {
            AhMessage::EntitySlotsAllocated(m) => Some(m.clone()),
            _ => None,
        });
        let pa = join(&mut a, 0xA);
        let pb = join(&mut b, 0xB);
        assert_eq!(b.ah_id(), HOST, "B presents the host it joined");
        assert!(pb.iter().all(|p| p.recipient_id == HOST));
        assert_eq!(grant(&pa).unwrap().count(), 500);
        let gb = grant(&pb).unwrap();
        assert!(gb.get(500) && gb.get(999) && !gb.get(0) && !gb.get(1000), "B's range is its slot's");
        let mb = membership(&pb).unwrap();
        assert_eq!(mask(&mb, RM::OCCUPANCY_MASK), 0b11);
        assert_eq!((member_account(&mb, 0), member_account(&mb, 1)), (0xA, 0xB));
        // A's next membership poll re-publishes with B in it.
        let later = now + Duration::from_millis(10);
        let ma = match a.membership_due(later).expect("A re-publishes").message {
            AhMessage::ReplicateMembership(m) => m.image,
            _ => unreachable!(),
        };
        assert_eq!(&ma[RM::MEMBERS..RM::SIMULATION_OCCUPANCY_MASK + 8], &mb[RM::MEMBERS..RM::SIMULATION_OCCUPANCY_MASK + 8]);
        assert!(a.membership_due(later).is_none(), "nothing changed since");
        // B's connection closes: A is alone again and still the physics host.
        drop(b);
        let ma = match a.membership_due(later).expect("A re-publishes").message {
            AhMessage::ReplicateMembership(m) => m.image,
            _ => unreachable!(),
        };
        assert_eq!(mask(&ma, RM::OCCUPANCY_MASK), 0b01);
        assert!(a.group().unwrap().is_physics_host());
    }
}
