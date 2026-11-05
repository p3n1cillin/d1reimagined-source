// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The end-of-activity rewards interface the script bridge calls ([`crate::script_bridge`]).
//!
//! When a scripted activity's lifetime reaches ended, the bridge calls
//! [`ActivityRewards::grant_activity_rewards`] once for each character that joined it. **This crate
//! grants nothing**: what D1 gives at the end of an activity (XP, the post-game rewards, reputation)
//! comes from the game's own data and belongs to the service that owns the character (Stream M,
//! through d1-responder). [`NoRewards`] is the stub until that lands: it logs and grants nothing.
//!
//! The call is made on the activity host's connection loop, which must never wait (the client
//! drops a host that stays silent, `state::HOST_SILENCE_LIMIT_MS`). An implementation must return
//! at once and do its work elsewhere, for example by queueing it to another thread.

/// The character a reward is for, as the activity host knows it: the client's own 40-byte identity
/// from its `join_request` (`+0x10`, struct `0x80800C2D`), the same bytes the membership echoes.
/// The host never learns a character id; the implementer maps the identity to its character.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct CharacterRef {
    pub identity: Vec<u8>,
}

impl std::fmt::Debug for CharacterRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let head: String = self.identity.iter().take(8).map(|b| format!("{b:02x}")).collect();
        write!(f, "CharacterRef({head}…, {} bytes)", self.identity.len())
    }
}

/// How the activity ended for the characters in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityOutcome {
    /// The scripts called `set_activity_complete()` before the lifetime reached ended.
    Completed,
    /// The lifetime reached ended without the scripts marking the activity complete.
    EndedIncomplete,
    /// The scripts stopped (the sidecar exited or the client left) before the lifetime ended.
    Abandoned,
}

/// Grants what a finished activity gives. Implemented by the rewards service (Stream M).
pub trait ActivityRewards: Send + Sync {
    /// Called once per character and activity run, on the connection loop: return at once.
    /// `activity_hash` is the launched activity's hash (the activity table's key).
    fn grant_activity_rewards(
        &self,
        character: &CharacterRef,
        activity_hash: u32,
        outcome: ActivityOutcome,
    ) -> Result<(), String>;
}

/// The stub: logs each call and grants nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoRewards;

impl ActivityRewards for NoRewards {
    fn grant_activity_rewards(
        &self,
        character: &CharacterRef,
        activity_hash: u32,
        outcome: ActivityOutcome,
    ) -> Result<(), String> {
        tracing::info!(
            "rewards stub: activity {activity_hash:#010x} ended {outcome:?} for {character:?}; \
             nothing granted (no rewards service is connected)"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stub_grants_nothing_and_succeeds() {
        let c = CharacterRef { identity: vec![0xAB; 40] };
        assert!(NoRewards.grant_activity_rewards(&c, 0x1234, ActivityOutcome::Completed).is_ok());
        assert_eq!(format!("{c:?}"), "CharacterRef(abababababababab…, 40 bytes)");
    }
}
