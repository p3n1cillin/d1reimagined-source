// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! # The activity framework on the wire: a driver for `ActivityRun` (stream O2)
//!
//! [`crate::encounter_framework::ActivityRun`] (#313) takes events and answers with effects, but
//! nothing fed it on a live connection. [`RunDriver`] does, for one activity instance:
//!
//! * **Scripted activities** (the script bridge runs the restored scripts): the bridge's `phase`
//!   messages become [`Event::ScriptPhase`], `all_dead` [`Event::AllPlayersDead`] and
//!   `activity_complete` [`Event::ActivityCompleted`] ([`crate::script_bridge::ScriptBridge::take_framework_events`]).
//! * **Everything else**: the definitions written from the live captures
//!   (`sequences/*.json`, `CaptureSequence`) open their encounters on the client's own reports, so
//!   the host feeds them: udt 21's slice set ([`host_events::from_slice_set`]) and incident 3403's
//!   trigger ([`host_events::from_trigger_report`]).
//! * **Both**: the client's incidents 2079 (`player_died`) and 566 / 2014 (`player_spawned`), and
//!   the clock.
//!
//! What it does with the effects:
//!
//! * **Checkpoints save** through the run's [`CheckpointStore`] (the responder's store under
//!   `D1_STORE=1` and `D1_STORE_CONTROL`: `checkpoint` and `lockout` lines), which the run calls
//!   itself as encounters complete.
//! * **Wipes restart**: after [`Effect::Wipe`] the next `player_spawned` is the end of the wipe
//!   screen ([`Event::WipeFinished`]), and the run reopens the checkpoint's encounter. ⚠ 56578 has
//!   no hard-wipe handshake the host can see yet (the script bridge's note); the respawn is the
//!   client's own report that it reset, and is labelled as the stand-in signal.
//! * **The outcome reaches the grant**: when the run ends, every member is granted once through
//!   the host's one rewards trait (the forwarder: `grant` lines). For a scripted activity the
//!   script bridge already grants at the results (lifetime 6), so the driver does not grant twice.
//!
//! Characters are keyed as the checkpoint store keys them, by the character SOID the client's
//! 40-byte identity carries at `+0x18` ([`d1_mp::identity::MemberIdentity`]), and granted by the
//! full identity ([`CharacterRef`]).
//!
//! Off unless `D1_AH_ACTIVITY_FRAMEWORK` is set ([`Framework::install`] from `main.rs`): with it
//! unset no driver starts and nothing changes.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, OnceLock};

pub use crate::encounter_framework::host_events;
use crate::checkpoints::{CharacterId, CheckpointStore, UnixSeconds};
use crate::encounter_framework::{ActivityLibrary, ActivityRun, Effect, EndReason, Event, Modifiers, RunState};
use crate::rewards::{ActivityOutcome, ActivityRewards, CharacterRef};

/// Incidents the driver reads from the client (the 56578 catalogue's ids, as the script bridge
/// translates them).
pub const INCIDENT_TRIGGER: u32 = 3403;
pub const INCIDENT_PLAYER_DIED: u32 = 2079;
pub const INCIDENTS_PLAYER_SPAWNED: [u32; 2] = [566, 2014];

/// What every driver in the process shares: the library, the checkpoint store and the rewards.
pub struct Framework {
    pub lib: Arc<ActivityLibrary>,
    pub store: Arc<dyn CheckpointStore>,
    pub rewards: Option<Arc<Mutex<dyn ActivityRewards + Send>>>,
    /// The client's activity table (index -> hash), for an instance that knows only its index.
    pub hash_of_index: HashMap<u16, u32>,
}

static FRAMEWORK: OnceLock<Framework> = OnceLock::new();

impl Framework {
    /// Make `f` the process's framework (once; a second call is ignored and returns false).
    pub fn install(f: Framework) -> bool {
        FRAMEWORK.set(f).is_ok()
    }

    pub fn get() -> Option<&'static Framework> {
        FRAMEWORK.get()
    }

    /// The manifest hash an instance serves: the launched activity's when the host follows it,
    /// else the client table's hash for its index.
    pub fn activity_hash(&self, launched: Option<u32>, index: u16) -> Option<u32> {
        launched.or_else(|| self.hash_of_index.get(&index).copied())
    }
}

/// The client's activity table, index -> hash (`D1_AH_ACTIVITY_TABLE`, else this checkout's
/// `characters/activity-table.tsv`, the table the checkpoint store reads).
pub fn activity_table_from_env() -> HashMap<u16, u32> {
    let table = std::env::var("D1_AH_ACTIVITY_TABLE")
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../../fixtures/56578/characters/activity-table.tsv").to_string());
    let by_hash = crate::checkpoints::read_activity_table(&std::fs::read_to_string(&table).unwrap_or_default());
    let mut m = HashMap::new();
    for (h, i) in by_hash {
        m.entry(i).or_insert(h);
    }
    m
}

/// The character SOID an identity names (`+0x18`), else its first qword.
pub fn character_id(c: &CharacterRef) -> CharacterId {
    d1_mp::identity::MemberIdentity::parse(&c.identity).map(|m| m.character.0).unwrap_or_else(|| c.key())
}

fn unix_now() -> UnixSeconds {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// One activity instance's run.
pub struct RunDriver {
    run: ActivityRun,
    activity_hash: u32,
    players: BTreeMap<CharacterId, CharacterRef>,
    /// Grant the outcome here (off when the script bridge grants it).
    grants: bool,
    rewards: Option<Arc<Mutex<dyn ActivityRewards + Send>>>,
    granted: bool,
    last_tick: UnixSeconds,
}

impl std::fmt::Debug for RunDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunDriver").field("activity", &self.run.definition().id).field("state", self.run.state()).finish()
    }
}

impl RunDriver {
    /// Start a run of `activity_hash` for `players` (the first leads; a weekly activity loads its
    /// checkpoint), if a definition runs that hash. `scripted`: the script bridge runs the scripts
    /// and grants the outcome itself. The skulls come from the catalog row.
    pub fn start(
        lib: &ActivityLibrary,
        store: Arc<dyn CheckpointStore>,
        rewards: Option<Arc<Mutex<dyn ActivityRewards + Send>>>,
        activity_hash: u32,
        players: &[CharacterRef],
        scripted: bool,
        now: UnixSeconds,
    ) -> Option<(Self, Vec<Effect>)> {
        let def = Arc::clone(lib.by_hash(activity_hash)?);
        let modifiers = lib.catalog.get(activity_hash).map(Modifiers::of_catalog).unwrap_or_default();
        let ids: Vec<CharacterId> = players.iter().map(character_id).collect();
        let (run, fx) = ActivityRun::start(def, activity_hash, &ids, modifiers, store, now);
        let players = ids.into_iter().zip(players.iter().cloned()).collect();
        let d = Self { run, activity_hash, players, grants: !scripted, rewards, granted: false, last_tick: now };
        d.log(&fx);
        Some((d, fx))
    }

    /// [`Self::start`] on the installed [`Framework`].
    pub fn start_installed(activity_hash: u32, players: &[CharacterRef], scripted: bool) -> Option<(Self, Vec<Effect>)> {
        let f = Framework::get()?;
        Self::start(&f.lib, Arc::clone(&f.store), f.rewards.clone(), activity_hash, players, scripted, unix_now())
    }

    pub fn run(&self) -> &ActivityRun {
        &self.run
    }

    pub fn activity_hash(&self) -> u32 {
        self.activity_hash
    }

    /// Whether the run has ended (completed or returned to orbit).
    pub fn finished(&self) -> bool {
        matches!(self.run.state(), RunState::Completed | RunState::Ended(_))
    }

    /// A character joined the instance (a fireteam member arriving later).
    pub fn join(&mut self, c: &CharacterRef) -> Vec<Effect> {
        let id = character_id(c);
        if self.players.insert(id, c.clone()).is_some() {
            return Vec::new();
        }
        self.feed(Event::PlayerJoined(id))
    }

    /// Feed one event; returns the effects (also logged), and grants the outcome once the run has
    /// ended.
    pub fn feed(&mut self, ev: Event) -> Vec<Effect> {
        if let Event::Tick(t) = ev {
            self.last_tick = t;
        }
        let fx = self.run.handle(ev);
        self.log(&fx);
        if self.finished() {
            self.grant();
        }
        fx
    }

    /// The clock, at most once a second.
    pub fn tick(&mut self, now: UnixSeconds) -> Vec<Effect> {
        if now <= self.last_tick {
            return Vec::new();
        }
        self.feed(Event::Tick(now))
    }

    /// The client's incidents (udt 18 types) and, for 3403, its trigger report image. `scripted`
    /// runs take their triggers from the scripts' phases instead.
    pub fn on_incidents(&mut self, types: &[u32], image: &[u8], scripted: bool) -> Vec<Effect> {
        let mut fx = Vec::new();
        // One client per connection: the incident is the first player's (the joined client).
        let who = self.players.keys().next().copied();
        for &t in types {
            let ev = match t {
                INCIDENT_TRIGGER if !scripted => host_events::from_trigger_report(image),
                INCIDENT_PLAYER_DIED => who.map(Event::PlayerDied),
                t if INCIDENTS_PLAYER_SPAWNED.contains(&t) => {
                    if matches!(self.run.state(), RunState::Wiping { .. }) {
                        // ⚠ STAND-IN SIGNAL: the respawn after the wipe screen ends the wipe.
                        Some(Event::WipeFinished)
                    } else {
                        who.map(Event::PlayerRevived)
                    }
                }
                _ => None,
            };
            if let Some(ev) = ev {
                fx.extend(self.feed(ev));
            }
        }
        fx
    }

    /// The client reported slice set `slice_set` (udt 21 `+0xB8`). Scripted runs take their
    /// encounters from the scripts' phases.
    pub fn on_slice(&mut self, slice_set: u32, scripted: bool) -> Vec<Effect> {
        if scripted {
            return Vec::new();
        }
        match host_events::from_slice_set(slice_set) {
            Some(ev) => self.feed(ev),
            None => Vec::new(),
        }
    }

    /// What the run's end is for the grant.
    fn outcome(&self) -> Option<ActivityOutcome> {
        match self.run.state() {
            RunState::Completed => Some(ActivityOutcome::Completed),
            RunState::Ended(EndReason::Abandoned) => Some(ActivityOutcome::Abandoned),
            RunState::Ended(_) => Some(ActivityOutcome::EndedIncomplete),
            _ => None,
        }
    }

    fn grant(&mut self) {
        if self.granted || !self.grants {
            return;
        }
        self.granted = true;
        let (Some(outcome), Some(rewards)) = (self.outcome(), self.rewards.as_ref()) else {
            tracing::info!("activity framework: {} ended; no rewards service, nothing granted", self.run.definition().id);
            return;
        };
        let Ok(mut r) = rewards.lock() else {
            tracing::warn!("activity framework: the rewards service lock is poisoned");
            return;
        };
        // Every character that was in the run: those still present, and those who left.
        for c in self.players.values() {
            if let Err(e) = r.grant_activity_rewards(c, self.activity_hash, &outcome) {
                tracing::warn!("activity framework: rewards for {c:?}: {e}");
            }
        }
        tracing::info!("⭐ activity framework: {} {outcome:?}; granted for {} character(s)", self.run.definition().id, self.players.len());
    }

    fn log(&self, fx: &[Effect]) {
        for e in fx {
            match e {
                Effect::Rewards { .. } | Effect::RewardsLocked { .. } => tracing::info!("activity framework: {e:?}"),
                _ => tracing::info!("⭐ activity framework: {}: {e:?}", self.run.definition().id),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoints::{CheckpointKey, MemoryCheckpointStore};
    use crate::encounter_framework::{CheckpointPolicy, Trigger};
    use crate::rewards::RecordingRewards;

    fn dyn_rewards(r: &Arc<Mutex<RecordingRewards>>) -> Arc<Mutex<dyn ActivityRewards + Send>> {
        r.clone()
    }

    fn lib() -> ActivityLibrary {
        ActivityLibrary::load(&ActivityLibrary::default_dir()).expect("the committed activity library")
    }

    fn identity(character: u64) -> CharacterRef {
        let mut b = vec![0u8; 40];
        b[..8].copy_from_slice(&[9, 8, 7, 6, 5, 4, 3, 2]);
        b[0x10..0x18].copy_from_slice(&0xD100_0000_0000_0001u64.to_le_bytes());
        b[0x18..0x20].copy_from_slice(&character.to_le_bytes());
        CharacterRef { identity: b }
    }

    /// A definition whose encounters open on the client's own reports (a capture sequence).
    fn host_driven(lib: &ActivityLibrary) -> Option<Arc<crate::encounter_framework::ActivityDef>> {
        let mut v: Vec<_> = lib
            .activities
            .iter()
            .filter(|d| !d.activity_hashes.is_empty() && d.encounters.len() >= 2)
            .filter(|d| d.encounters.iter().skip(1).any(|e| matches!(e.begins_on, Trigger::SliceEntered(_) | Trigger::Trigger(_))))
            .cloned()
            .collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v.into_iter().next()
    }

    #[test]
    fn a_host_driven_run_follows_the_clients_reports_and_grants_once() {
        let lib = lib();
        let Some(def) = host_driven(&lib) else { panic!("the library has a capture-driven definition") };
        let hash = def.activity_hashes[0];
        let store = Arc::new(MemoryCheckpointStore::new());
        let rewards = Arc::new(Mutex::new(RecordingRewards::default()));
        let me = identity(0xD100_0000_0000_0101);
        let (mut d, fx) = RunDriver::start(&lib, store, Some(dyn_rewards(&rewards)), hash, std::slice::from_ref(&me), false, 1_000).unwrap();
        assert!(matches!(fx.first(), Some(Effect::EncounterBegan { index: 0, .. })), "{fx:?}");
        // Replay the live run's reports (its capture sequence), as the client would send them.
        let events = def.sequence.as_ref().and_then(|s| lib.sequences.get(s)).map(|s| s.replay_events()).unwrap_or_default();
        let mut opened = 0;
        for ev in events {
            let fx = match ev {
                Event::SliceEntered(s) => d.on_slice(s, false),
                ev @ Event::Trigger { .. } => d.feed(ev),
                _ => Vec::new(),
            };
            opened += fx.iter().filter(|e| matches!(e, Effect::EncounterBegan { .. })).count();
        }
        assert!(opened > 0, "the client's reports opened later encounters");
        assert!(d.on_slice(5, true).is_empty(), "a scripted run takes no slice reports");
        d.feed(Event::ActivityCompleted);
        assert!(d.finished());
        d.feed(Event::ActivityCompleted);
        let r = rewards.lock().unwrap();
        assert_eq!(r.grants.len(), 1, "granted once");
        assert_eq!(r.grants[0].1, hash);
        assert_eq!(r.grants[0].2, ActivityOutcome::Completed);
        assert_eq!(r.characters[0], me, "by the full identity");
    }

    #[test]
    fn a_weekly_run_saves_its_checkpoint_and_a_wipe_restarts_there() {
        let lib = lib();
        let mut v: Vec<_> = lib
            .activities
            .iter()
            .filter(|d| d.checkpoints == CheckpointPolicy::Weekly && !d.activity_hashes.is_empty())
            .filter(|d| d.encounters.iter().skip(1).any(|e| e.checkpoint && e.darkness))
            .cloned()
            .collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        let def = v.first().cloned().expect("a raid with a checkpointed darkness encounter");
        let hash = def.activity_hashes[0];
        let store = Arc::new(MemoryCheckpointStore::new());
        let rewards = Arc::new(Mutex::new(RecordingRewards::default()));
        let me = identity(0xD100_0000_0000_0101);
        let (mut d, _) = RunDriver::start(&lib, store.clone(), Some(dyn_rewards(&rewards)), hash, std::slice::from_ref(&me), true, 1_000).unwrap();
        let target = def.encounters.iter().enumerate().skip(1).find(|(_, e)| e.checkpoint && e.darkness).map(|(i, _)| i).unwrap();
        // Open the checkpointed encounter (a host override; the scripts would name its phase).
        let fx = d.feed(Event::BeginEncounter(def.encounters[target].id.clone()));
        assert!(fx.iter().any(|e| matches!(e, Effect::CheckpointReached { .. })), "{fx:?}");
        let key = CheckpointKey { character: 0xD100_0000_0000_0101, activity_hash: hash };
        assert_eq!(store.load(key).map(|c| c.encounter_index), Some(target), "saved for the character SOID at +0x18");
        // Everyone dies in the darkness zone: a wipe, then the respawn restarts the encounter.
        let fx = d.on_incidents(&[INCIDENT_PLAYER_DIED], &[], true);
        assert!(fx.iter().any(|e| matches!(e, Effect::Wipe { index, .. } if *index == target)), "{fx:?}");
        let fx = d.on_incidents(&[566], &[], true);
        assert!(fx.iter().any(|e| matches!(e, Effect::Respawned { index } if *index == target)), "{fx:?}");
        assert!(matches!(d.run().state(), RunState::Running { encounter, .. } if *encounter == target));
        // A scripted run's outcome is granted by the script bridge, not here.
        d.feed(Event::ActivityCompleted);
        assert!(d.finished());
        assert!(rewards.lock().unwrap().grants.is_empty());
        assert!(store.load(key).is_none(), "a full clear spends the checkpoint");
    }

    #[test]
    fn a_nightfall_wipe_ends_the_run_and_the_outcome_is_incomplete() {
        let lib = lib();
        let mut v: Vec<u32> = lib.catalog.activities.iter().filter(|c| c.category == "nightfall").map(|c| c.hash).filter(|h| lib.by_hash(*h).is_some()).collect();
        v.sort_unstable();
        let Some(&hash) = v.first() else { panic!("a Nightfall the library runs") };
        let rewards = Arc::new(Mutex::new(RecordingRewards::default()));
        let me = identity(0xD100_0000_0000_0101);
        let (mut d, _) = RunDriver::start(&lib, Arc::new(MemoryCheckpointStore::new()), Some(dyn_rewards(&rewards)), hash, std::slice::from_ref(&me), false, 1_000).unwrap();
        let fx = d.on_incidents(&[INCIDENT_PLAYER_DIED], &[], false);
        assert!(fx.iter().any(|e| matches!(e, Effect::ReturnedToOrbit(EndReason::NightfallWipe))), "{fx:?}");
        assert_eq!(rewards.lock().unwrap().grants[0].2, ActivityOutcome::EndedIncomplete);
    }

    #[test]
    fn the_activity_table_maps_indices_to_hashes() {
        let t = activity_table_from_env();
        assert_eq!(t.get(&0), Some(&0x8AE8_DA69), "the Tower");
        assert_eq!(t.get(&165), Some(&0x6EAF_0959), "cine_reveal");
    }

    #[test]
    fn an_unknown_activity_starts_no_run() {
        let lib = lib();
        assert!(RunDriver::start(&lib, Arc::new(MemoryCheckpointStore::new()), None, 0xDEAD_BEEF, &[identity(1)], false, 0).is_none());
    }
}
