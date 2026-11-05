// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! # Public events on a bubble host (stream G)
//!
//! A public bubble's events are its own scripts, not the mission's: by Bungie's rules (GDC 2015) a
//! mission host may not create or change networked objects in a public bubble, and anything that
//! happens there for everyone is written into the bubble's script. In the live captures the event
//! bundles are sensor groups on the public instance's host: in the Cosmodrome `0x358EBAE1` (region
//! 48) and `0xD2C22DCF` (region 0, the Array, where The Dark Within's Defend the Warsat ran on the
//! instance the client joined at +746 s).
//!
//! What this module owns is the **structure**: which event runs when ([`EventSchedule`], one at a
//! time per bundle, with a quiet spell before each), the event's lifecycle ([`EventPhase`]), who
//! took part ([`Participation`]: a player is engaged by killing or damaging the event's enemies, as
//! the restored scripts' shared event globals bind it), and the reward call at the end
//! ([`crate::open_world::ActivityRewards`]). The event's own behaviour (its squads, objectives and
//! how it scores bronze, silver or gold) is its script, run by the mission-script bridge stream A2
//! is building, reached through [`BubbleScriptBridge`]. Until that bridge lands, [`RecordingBridge`]
//! stands in.
//!
//! ⚠ **Bungie's schedules were never published.** [`PLACEHOLDER_SCHEDULE`] is a labelled stand-in:
//! its windows are not the game's, and its event names are the kinds of public event D1 had, not
//! a list read from content.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::open_world::{ActivityOutcome, ActivityRewards, EventTier};

/// The kinds of public event Destiny 1 ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    /// Hold a point until a countdown ends (Warsat Down and its defence).
    Defend,
    /// Kill one powerful enemy before time runs out.
    Elimination,
    /// Kill waves of enemies against a clock.
    Skirmish,
    /// Destroy a Fallen Walker.
    Walker,
    /// A Taken invasion (The Taken King).
    TakenInvasion,
}

/// One event a bundle can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduledEvent {
    /// The event's index in its bundle's rotation (the script's variation).
    pub id: u32,
    pub kind: EventKind,
    /// The event's activity hash, for the reward call. 0 until read from content.
    pub activity_hash: u32,
    /// How long it runs before it fails, if the script does not end it first.
    pub time_limit: Duration,
}

/// One bundle's rotation: a quiet spell drawn from `quiet`, then one event picked from `events`.
#[derive(Debug, Clone, Copy)]
pub struct EventSchedule {
    pub bundle: u32,
    pub quiet_min: Duration,
    pub quiet_max: Duration,
    pub events: &'static [ScheduledEvent],
}

const fn ev(id: u32, kind: EventKind, limit_s: u64) -> ScheduledEvent {
    ScheduledEvent { id, kind, activity_hash: 0, time_limit: Duration::from_secs(limit_s) }
}

/// ⚠ **PLACEHOLDER SCHEDULE - NOT BUNGIE'S.** The schedules were server-side and never public. The
/// one live event start on the wire (The Dark Within, region 0) shows an event running on the
/// bubble's host, not when the next would have come. These windows (10-15 minutes quiet, 5 minutes
/// to finish) are stand-ins chosen to be plausible, and the rotations list the kinds of event D1
/// ran in those bubbles, not entries read from content.
pub const PLACEHOLDER_SCHEDULE: &[EventSchedule] = &[
    // Cosmodrome region 48 (around dome A).
    EventSchedule {
        bundle: 0x358E_BAE1,
        quiet_min: Duration::from_secs(600),
        quiet_max: Duration::from_secs(900),
        events: &[ev(0, EventKind::Defend, 300), ev(1, EventKind::Defend, 300)],
    },
    // Cosmodrome region 0 (the Array): where Defend the Warsat ran live.
    EventSchedule {
        bundle: 0xD2C2_2DCF,
        quiet_min: Duration::from_secs(600),
        quiet_max: Duration::from_secs(900),
        events: &[ev(0, EventKind::Defend, 300), ev(1, EventKind::Skirmish, 300)],
    },
];

/// The schedule for a bundle, if it has one.
pub fn schedule_for(bundle: u32) -> Option<&'static EventSchedule> {
    PLACEHOLDER_SCHEDULE.iter().find(|s| s.bundle == bundle)
}

/// ⭐ **The interface to the mission-script bridge (stream A2).** A bubble host asks the bridge to
/// run its bundles' scripts and to start or stop an event; the bridge reports back through
/// [`PublicEventRunner::on_script_finished`] and [`PublicEventRunner::on_engagement`]. Kept here as
/// a trait so this stream does not depend on the bridge's module, which may not exist yet.
pub trait BubbleScriptBridge {
    /// Load and start a public instance's bundles (its ambient layer, events, chests, nodes).
    fn start_bundles(&mut self, host: u64, bundles: &[u32]) -> Result<(), String>;
    /// Start one event of a bundle's rotation.
    fn start_event(&mut self, host: u64, bundle: u32, event: &ScheduledEvent) -> Result<(), String>;
    /// Stop an event (its time ran out, or the instance is going away).
    fn stop_event(&mut self, host: u64, bundle: u32, event_id: u32) -> Result<(), String>;
}

/// What a bridge was asked to do, recorded in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeCall {
    StartBundles { host: u64, bundles: Vec<u32> },
    StartEvent { host: u64, bundle: u32, event: u32 },
    StopEvent { host: u64, bundle: u32, event: u32 },
}

/// A bridge that records every call and runs nothing: the stand-in until stream A2's bridge exists.
#[derive(Debug, Default)]
pub struct RecordingBridge {
    pub calls: Vec<BridgeCall>,
}

impl BubbleScriptBridge for RecordingBridge {
    fn start_bundles(&mut self, host: u64, bundles: &[u32]) -> Result<(), String> {
        self.calls.push(BridgeCall::StartBundles { host, bundles: bundles.to_vec() });
        Ok(())
    }
    fn start_event(&mut self, host: u64, bundle: u32, event: &ScheduledEvent) -> Result<(), String> {
        self.calls.push(BridgeCall::StartEvent { host, bundle, event: event.id });
        Ok(())
    }
    fn stop_event(&mut self, host: u64, bundle: u32, event_id: u32) -> Result<(), String> {
        self.calls.push(BridgeCall::StopEvent { host, bundle, event: event_id });
        Ok(())
    }
}

/// Where a bundle's rotation is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPhase {
    /// Nothing running until then.
    Quiet { until: Instant },
    /// An event is running.
    Running { event: ScheduledEvent, since: Instant },
}

/// How an event ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResult {
    /// The script completed it, at this tier.
    Completed(EventTier),
    /// It failed or ran out of time.
    Failed,
}

/// Who took part in the running event: a character is engaged once it kills or damages the
/// event's enemies (the event's script reports each one).
#[derive(Debug, Default, Clone)]
pub struct Participation {
    engaged: BTreeMap<u64, u32>,
}

impl Participation {
    pub fn engage(&mut self, character: u64) {
        *self.engaged.entry(character).or_insert(0) += 1;
    }
    pub fn characters(&self) -> impl Iterator<Item = u64> + '_ {
        self.engaged.keys().copied()
    }
    pub fn is_engaged(&self, character: u64) -> bool {
        self.engaged.contains_key(&character)
    }
}

/// A notice for the host's log and for the instance's players.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventNotice {
    Started { bundle: u32, event: u32 },
    Ended { bundle: u32, event: u32, result: EventResult, rewarded: usize },
}

/// One bundle's events on one bubble host.
#[derive(Debug)]
pub struct PublicEventRunner {
    host: u64,
    schedule: &'static EventSchedule,
    phase: EventPhase,
    participation: Participation,
    rng: u64,
}

impl PublicEventRunner {
    /// Start the rotation: a quiet spell from `now`. The draw is seeded by the host id, so a run is
    /// reproducible.
    pub fn new(host: u64, schedule: &'static EventSchedule, now: Instant) -> Self {
        let mut r = Self {
            host,
            schedule,
            phase: EventPhase::Quiet { until: now },
            participation: Participation::default(),
            rng: host ^ 0x9E37_79B9_7F4A_7C15 ^ u64::from(schedule.bundle),
        };
        r.phase = EventPhase::Quiet { until: now + r.quiet() };
        r
    }

    pub fn phase(&self) -> EventPhase {
        self.phase
    }

    pub fn participation(&self) -> &Participation {
        &self.participation
    }

    fn next(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.rng.max(1);
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn quiet(&mut self) -> Duration {
        let (lo, hi) = (self.schedule.quiet_min.as_secs(), self.schedule.quiet_max.as_secs());
        Duration::from_secs(lo + self.next() % (hi.saturating_sub(lo) + 1))
    }

    /// Advance the clock: start the next event when the quiet spell ends, fail one whose time ran
    /// out. Returns what happened.
    pub fn tick(
        &mut self,
        now: Instant,
        bridge: &mut dyn BubbleScriptBridge,
        rewards: &mut dyn ActivityRewards,
    ) -> Vec<EventNotice> {
        match self.phase {
            EventPhase::Quiet { until } if now >= until && !self.schedule.events.is_empty() => {
                let pick = (self.next() % self.schedule.events.len() as u64) as usize;
                let event = self.schedule.events[pick];
                if let Err(e) = bridge.start_event(self.host, self.schedule.bundle, &event) {
                    tracing::warn!("public event {} of bundle {:#010x} not started: {e}", event.id, self.schedule.bundle);
                    self.phase = EventPhase::Quiet { until: now + self.quiet() };
                    return Vec::new();
                }
                self.participation = Participation::default();
                self.phase = EventPhase::Running { event, since: now };
                vec![EventNotice::Started { bundle: self.schedule.bundle, event: event.id }]
            }
            EventPhase::Running { event, since } if now.saturating_duration_since(since) >= event.time_limit => {
                let _ = bridge.stop_event(self.host, self.schedule.bundle, event.id);
                self.finish(EventResult::Failed, now, rewards)
            }
            _ => Vec::new(),
        }
    }

    /// The event's script reports a kill or damage by `character` on the event's enemies.
    pub fn on_engagement(&mut self, character: u64) {
        if matches!(self.phase, EventPhase::Running { .. }) {
            self.participation.engage(character);
        }
    }

    /// The event's script ended it.
    pub fn on_script_finished(
        &mut self,
        result: EventResult,
        now: Instant,
        rewards: &mut dyn ActivityRewards,
    ) -> Vec<EventNotice> {
        self.finish(result, now, rewards)
    }

    /// End the running event: report every engaged character's outcome, then go quiet again.
    fn finish(&mut self, result: EventResult, now: Instant, rewards: &mut dyn ActivityRewards) -> Vec<EventNotice> {
        let EventPhase::Running { event, .. } = self.phase else { return Vec::new() };
        let (completed, tier) = match result {
            EventResult::Completed(t) => (true, Some(t)),
            EventResult::Failed => (false, None),
        };
        let outcome = ActivityOutcome::PublicEvent { event_id: event.id, completed, tier };
        let mut rewarded = 0;
        for c in self.participation.characters().collect::<Vec<_>>() {
            match rewards.grant_activity_rewards(c, event.activity_hash, &outcome) {
                Ok(()) => rewarded += 1,
                Err(e) => tracing::warn!("public event {} reward for {c:#x} not granted: {e}", event.id),
            }
        }
        self.phase = EventPhase::Quiet { until: now + self.quiet() };
        vec![EventNotice::Ended { bundle: self.schedule.bundle, event: event.id, result, rewarded }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::open_world::RecordingRewards;

    const HOST: u64 = 0xD1A0_0000_0000_0002;

    #[test]
    fn the_placeholder_schedule_covers_the_captured_event_bundles() {
        assert!(schedule_for(0x358E_BAE1).is_some());
        assert!(schedule_for(0xD2C2_2DCF).is_some());
        assert!(schedule_for(0xEAAF_16E2).is_none());
        for s in PLACEHOLDER_SCHEDULE {
            assert!(s.quiet_min <= s.quiet_max && !s.events.is_empty());
        }
    }

    #[test]
    fn an_event_starts_after_the_quiet_spell_and_rewards_whoever_engaged() {
        let t0 = Instant::now();
        let s = schedule_for(0xD2C2_2DCF).unwrap();
        let mut run = PublicEventRunner::new(HOST, s, t0);
        let mut bridge = RecordingBridge::default();
        let mut rewards = RecordingRewards::default();
        let EventPhase::Quiet { until } = run.phase() else { panic!("starts quiet") };
        let wait = until - t0;
        assert!(wait >= s.quiet_min && wait <= s.quiet_max);
        assert!(run.tick(t0, &mut bridge, &mut rewards).is_empty());
        run.on_engagement(1); // nothing running: not counted
        let started = run.tick(until, &mut bridge, &mut rewards);
        let [EventNotice::Started { bundle, event }] = started.as_slice() else { panic!("{started:?}") };
        assert_eq!(*bundle, 0xD2C2_2DCF);
        assert_eq!(bridge.calls, vec![BridgeCall::StartEvent { host: HOST, bundle: 0xD2C2_2DCF, event: *event }]);
        run.on_engagement(1);
        run.on_engagement(2);
        run.on_engagement(1);
        let ended = run.on_script_finished(EventResult::Completed(EventTier::Gold), until, &mut rewards);
        assert!(matches!(ended.as_slice(), [EventNotice::Ended { rewarded: 2, .. }]));
        assert_eq!(rewards.grants.len(), 2);
        assert!(rewards.grants.iter().all(|(_, _, o)| matches!(
            o,
            ActivityOutcome::PublicEvent { completed: true, tier: Some(EventTier::Gold), .. }
        )));
        assert!(matches!(run.phase(), EventPhase::Quiet { .. }));
    }

    #[test]
    fn an_event_that_runs_out_of_time_fails_and_is_stopped() {
        let t0 = Instant::now();
        let s = schedule_for(0x358E_BAE1).unwrap();
        let mut run = PublicEventRunner::new(HOST, s, t0);
        let mut bridge = RecordingBridge::default();
        let mut rewards = RecordingRewards::default();
        let EventPhase::Quiet { until } = run.phase() else { panic!() };
        run.tick(until, &mut bridge, &mut rewards);
        run.on_engagement(5);
        let EventPhase::Running { event, .. } = run.phase() else { panic!("running") };
        let ended = run.tick(until + event.time_limit, &mut bridge, &mut rewards);
        assert!(matches!(ended.as_slice(), [EventNotice::Ended { result: EventResult::Failed, rewarded: 1, .. }]));
        assert!(matches!(bridge.calls.last(), Some(BridgeCall::StopEvent { .. })));
        assert_eq!(
            rewards.grants[0].2,
            ActivityOutcome::PublicEvent { event_id: event.id, completed: false, tier: None }
        );
    }

    #[test]
    fn the_draw_is_reproducible_per_host() {
        let t0 = Instant::now();
        let s = schedule_for(0x358E_BAE1).unwrap();
        let a = PublicEventRunner::new(HOST, s, t0).phase();
        let b = PublicEventRunner::new(HOST, s, t0).phase();
        assert_eq!(a, b);
    }
}
