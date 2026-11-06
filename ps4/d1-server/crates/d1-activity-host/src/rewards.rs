// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! ⭐ **The one rewards interface of the activity host.** Every reward the host reports goes
//! through [`ActivityRewards::grant_activity_rewards`]: a scripted activity's results (the script
//! bridge, [`crate::script_bridge`]) and the open world's public events, patrol missions, chests and
//! resource nodes (stream G, [`crate::open_world`], [`crate::public_events`]). There used to be two
//! traits, one per caller; this is both of them.
//!
//! **This crate grants nothing.** What D1 gives (XP, the post-game rewards, reputation, loot) comes
//! from the game's own data and belongs to the service that owns the character: stream M's
//! `rewards::grant_activity_rewards(&mut store, CharacterId, activity_hash, &ActivityOutcome)` in
//! d1-responder, another process (notes/ps4-56578-rewards-and-growth-2026-11-05.md).
//!
//! ⚠ **HOOK (stream O).** The runtime implementation is O's forwarder (`integrate/wiring`), which
//! writes grant commands the responder executes. Until it is plugged in, hosts use
//! [`LoggedRewards`], which logs and grants nothing. Plug the forwarder in where the host builds
//! its services: the script bridge's [`crate::script_bridge::ScriptConfig::rewards`] and the public
//! host's `OpenWorldServices::rewards`. Both take `Arc<Mutex<dyn ActivityRewards + Send>>`.
//!
//! **Who the reward is for.** The host never learns a character id: it knows the client's own
//! 40-byte identity from its `join_request` ([`CharacterRef`]). The u64 mapping is the identity's
//! first qword, little endian ([`CharacterRef::key`]), the same word the `activity_complete` body
//! carries as its player; stream G's public-instance code keys members by it. The forwarder maps
//! either to the responder's `CharacterId`.
//!
//! **When.** The script bridge grants when the lifetime reaches **6, the results**, not when it
//! ends: the post-game screens open at 7 (gameplay) and read the rewards the profile already holds,
//! and a cinematic-only activity's 8 is the client's own transition to what comes next (see
//! [`crate::script_bridge`], "Rewards at the results").
//!
//! The call is made on the activity host's connection loop, which must never wait (the client
//! drops a host that stays silent, `state::HOST_SILENCE_LIMIT_MS`). An implementation must return
//! at once and do its work elsewhere, for example by queueing it to another thread.

/// The character a reward is for, as the activity host knows it: the client's own 40-byte identity
/// from its `join_request` (`+0x10`, struct `0x80800C2D`), the same bytes the membership echoes.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct CharacterRef {
    pub identity: Vec<u8>,
}

impl CharacterRef {
    /// The u64 the host keys a character by: the identity's first qword, little endian.
    pub fn key(&self) -> u64 {
        let mut q = [0u8; 8];
        for (d, s) in q.iter_mut().zip(&self.identity) {
            *d = *s;
        }
        u64::from_le_bytes(q)
    }

    /// A character known only by its u64 key (stream G's members): the qword alone, which maps back
    /// to the same key.
    pub fn from_key(key: u64) -> Self {
        Self { identity: key.to_le_bytes().to_vec() }
    }
}

impl From<u64> for CharacterRef {
    fn from(key: u64) -> Self {
        Self::from_key(key)
    }
}

impl std::fmt::Debug for CharacterRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let head: String = self.identity.iter().take(8).map(|b| format!("{b:02x}")).collect();
        write!(f, "CharacterRef({head}…, {} bytes)", self.identity.len())
    }
}

/// A public event's completion tier, as Destiny 1 showed it (bronze, silver, gold). The tier is
/// reported by the event's script; the host never computes one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventTier {
    Bronze,
    Silver,
    Gold,
}

/// How an activity, or one piece of the open world, ended for one character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityOutcome {
    /// A scripted activity reached its results with the scripts' `set_activity_complete()`.
    Completed,
    /// A scripted activity reached its results (or its end) without being marked complete.
    EndedIncomplete,
    /// The scripts stopped (the sidecar exited or the client left) before the results.
    Abandoned,
    /// A public event the character took part in. `tier` is what the event's script reported.
    PublicEvent { event_id: u32, completed: bool, tier: Option<EventTier> },
    /// A patrol mission from a beacon.
    PatrolMission { beacon: u32 },
    /// A chest opened.
    Chest { bundle: u32, index: i16 },
    /// A resource node harvested.
    ResourceNode { bundle: u32, index: i16 },
}

impl ActivityOutcome {
    /// Whether it counts as a completion: the `completed` of stream M's `rewards::ActivityOutcome`
    /// (`d1-responder`), which the responder-side grant takes as
    /// `ActivityOutcome { completed, community_page: None, roll_engrams: false, seed: None }`.
    /// Chests, nodes and patrol missions are always completions; an event only when it was.
    pub fn completed(&self) -> bool {
        match self {
            Self::Completed => true,
            Self::EndedIncomplete | Self::Abandoned => false,
            Self::PublicEvent { completed, .. } => *completed,
            Self::PatrolMission { .. } | Self::Chest { .. } | Self::ResourceNode { .. } => true,
        }
    }
}

/// ⭐ Grants what a finished activity gives. Implemented by stream O's forwarder into stream M's
/// service (the HOOK above); [`LoggedRewards`] until then.
pub trait ActivityRewards {
    /// Called once per character and outcome, on the connection loop: return at once.
    /// `activity_hash` is the activity's manifest hash (the activity table's key), 0 when the
    /// caller has none.
    fn grant_activity_rewards(
        &mut self,
        character: &CharacterRef,
        activity_hash: u32,
        outcome: &ActivityOutcome,
    ) -> Result<(), String>;

    /// ⚠ HOOK (stream O): the story arc's hand-off. A completed activity whose table row names a
    /// next activity (`next=` in `D1_AH_SCRIPTS_ACTIVITIES`; cine_reveal → the Tower) reports it
    /// here, right after its grant, so the responder can record the completion and select the
    /// next activity before the client reaches the end (the client decides what to launch from the
    /// profile it is served; notes/ps4-56578-story-missions-2026-11-05.md). The default does
    /// nothing.
    fn activity_hand_off(
        &mut self,
        _character: &CharacterRef,
        _from_hash: u32,
        _next_index: u16,
        _next_hash: u32,
    ) -> Result<(), String> {
        Ok(())
    }
}

/// The rewards a host reports with until stream O's forwarder is plugged in: each grant is
/// logged, and nothing is given.
#[derive(Debug, Default, Clone, Copy)]
pub struct LoggedRewards;

/// The script bridge's old name for [`LoggedRewards`].
pub type NoRewards = LoggedRewards;

impl ActivityRewards for LoggedRewards {
    fn grant_activity_rewards(
        &mut self,
        character: &CharacterRef,
        activity_hash: u32,
        outcome: &ActivityOutcome,
    ) -> Result<(), String> {
        tracing::info!(
            "rewards stub: activity {activity_hash:#010x} {outcome:?} for {character:?} (key {:#x}); \
             nothing granted (no rewards service is connected)",
            character.key()
        );
        Ok(())
    }

    fn activity_hand_off(&mut self, character: &CharacterRef, from_hash: u32, next_index: u16, next_hash: u32) -> Result<(), String> {
        tracing::info!(
            "hand-off stub: activity {from_hash:#010x} completed for {character:?}; its next activity is \
             {next_index} ({next_hash:#010x}); nothing recorded (no rewards service is connected)"
        );
        Ok(())
    }
}

/// Records every grant and hand-off, for tests and for a host run with no progression service.
#[derive(Debug, Default)]
pub struct RecordingRewards {
    /// `(character key, activity hash, outcome)`.
    pub grants: Vec<(u64, u32, ActivityOutcome)>,
    /// The identities granted, in the same order.
    pub characters: Vec<CharacterRef>,
    /// `(character key, from hash, next index, next hash)`.
    pub hand_offs: Vec<(u64, u32, u16, u32)>,
}

impl ActivityRewards for RecordingRewards {
    fn grant_activity_rewards(
        &mut self,
        character: &CharacterRef,
        activity_hash: u32,
        outcome: &ActivityOutcome,
    ) -> Result<(), String> {
        self.grants.push((character.key(), activity_hash, *outcome));
        self.characters.push(character.clone());
        Ok(())
    }

    fn activity_hand_off(&mut self, character: &CharacterRef, from_hash: u32, next_index: u16, next_hash: u32) -> Result<(), String> {
        self.hand_offs.push((character.key(), from_hash, next_index, next_hash));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stub_grants_nothing_and_succeeds() {
        let c = CharacterRef { identity: vec![0xAB; 40] };
        assert!(LoggedRewards.grant_activity_rewards(&c, 0x1234, &ActivityOutcome::Completed).is_ok());
        assert!(LoggedRewards.activity_hand_off(&c, 0x1234, 0, 0x8AE8_DA69).is_ok());
        assert_eq!(format!("{c:?}"), "CharacterRef(abababababababab…, 40 bytes)");
    }

    #[test]
    fn the_u64_key_is_the_identitys_first_qword_and_maps_back() {
        let mut identity = vec![0u8; 40];
        identity[..8].copy_from_slice(&0x0123_4567_89AB_CDEFu64.to_le_bytes());
        identity[8] = 0x55;
        let c = CharacterRef { identity };
        assert_eq!(c.key(), 0x0123_4567_89AB_CDEF);
        assert_eq!(CharacterRef::from_key(c.key()).key(), c.key());
        assert_eq!(CharacterRef::from(7u64).key(), 7);
        // A short identity is zero-extended.
        assert_eq!(CharacterRef { identity: vec![1, 2] }.key(), 0x0201);
    }

    #[test]
    fn completion_is_the_responders_completed_flag() {
        assert!(ActivityOutcome::Completed.completed());
        assert!(!ActivityOutcome::EndedIncomplete.completed());
        assert!(!ActivityOutcome::Abandoned.completed());
        assert!(ActivityOutcome::Chest { bundle: 1, index: 2 }.completed());
        assert!(!ActivityOutcome::PublicEvent { event_id: 1, completed: false, tier: None }.completed());
    }

    #[test]
    fn the_recorder_keeps_both_callers_shapes() {
        let mut r = RecordingRewards::default();
        r.grant_activity_rewards(&CharacterRef::from_key(7), 0, &ActivityOutcome::PatrolMission { beacon: 3 }).unwrap();
        r.grant_activity_rewards(&CharacterRef { identity: vec![9; 40] }, 0x6EAF_0959, &ActivityOutcome::Completed).unwrap();
        r.activity_hand_off(&CharacterRef::from_key(7), 0x6EAF_0959, 0, 0x8AE8_DA69).unwrap();
        assert_eq!(r.grants[0], (7, 0, ActivityOutcome::PatrolMission { beacon: 3 }));
        assert_eq!(r.characters[1].identity.len(), 40);
        assert_eq!(r.hand_offs, vec![(7, 0x6EAF_0959, 0, 0x8AE8_DA69)]);
    }
}
