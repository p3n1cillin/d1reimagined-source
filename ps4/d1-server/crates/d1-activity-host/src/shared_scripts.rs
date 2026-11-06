// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! # One script bridge per shared instance (stream O3)
//!
//! Integration review, finding 9: with `D1_AH_SHARED_INSTANCES=1` several connections join one
//! instance (`crate::group`), but each built its own [`ScriptBridge`] and sidecar. A fireteam in a
//! scripted strike ran N copies of the mission script: one member's trigger advanced only their
//! copy, the udt 5 rows differed per client, and every connection sent its own completion and its
//! own grants.
//!
//! Now the instance has one bridge, and each connection holds a [`ScriptHandle`] to it:
//!
//! * **Every member's messages go to the one bridge.** Its join, slice sets, incidents and
//!   triggers drive the one script, as the live mission host ran one script for the fireteam.
//!   The bridge already keeps every member's identity and grants each of them once at the results
//!   (`ScriptBridge::reward`), so a fireteam of two is paid twice in all, once each.
//! * **Every output goes to every member.** Each udt 5 frame, each host incident (the completion
//!   envelope), each region request (the host teleport) and each activity-framework event is
//!   queued for every member and taken by that member's connection, so every client sees the same
//!   rows. A member who joins later gets the full list again: the bridge re-sends it on a
//!   second join of the same world (`rows.request_full`), to everyone.
//! * **Senses come from one member.** The squads' udt 6 senses describe one shared world, so only
//!   the earliest member still present feeds them (else each would be counted once per member).
//!   `[I]` which peer the live host trusted for them; the physics host is the likelier one.
//! * **The bridge outlives no one.** It goes when its last member's connection does.
//!
//! Off (shared instances unset), each connection's handle is its own bridge, the instance of one
//! member, and nothing changes.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Arc, Mutex, Weak};
use std::time::Instant;

use crate::encounter_framework::Event;
use crate::ClientMessage;
use crate::script_bridge::{ScriptBridge, SquadSense};

#[derive(Default)]
struct Outbox {
    frames: VecDeque<Vec<u8>>,
    incidents: VecDeque<Vec<u8>>,
    region: Option<u32>,
    events: Vec<Event>,
}

struct Shared {
    bridge: ScriptBridge,
    outboxes: BTreeMap<u64, Outbox>,
    next: u64,
}

impl Shared {
    /// Move the bridge's outputs (but frames, which [`ScriptHandle::sensor_due`] takes) into every
    /// member's outbox.
    fn collect(&mut self) {
        let region = self.bridge.take_region_request();
        let mut incidents = Vec::new();
        while let Some(i) = self.bridge.take_incident() {
            incidents.push(i);
        }
        let events = self.bridge.take_framework_events();
        if region.is_none() && incidents.is_empty() && events.is_empty() {
            return;
        }
        for o in self.outboxes.values_mut() {
            if region.is_some() {
                o.region = region;
            }
            o.incidents.extend(incidents.iter().cloned());
            o.events.extend(events.iter().cloned());
        }
    }
}

/// One connection's share of an instance's bridge.
pub struct ScriptHandle {
    shared: Arc<Mutex<Shared>>,
    member: u64,
}

impl std::fmt::Debug for ScriptHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScriptHandle").field("member", &self.member).field("members", &self.members()).finish()
    }
}

impl ScriptHandle {
    /// A bridge of its own: the instance has one member.
    pub fn solo(bridge: ScriptBridge) -> Self {
        let shared = Arc::new(Mutex::new(Shared { bridge, outboxes: BTreeMap::new(), next: 0 }));
        Self::attach(shared)
    }

    fn attach(shared: Arc<Mutex<Shared>>) -> Self {
        let member = {
            let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
            let m = s.next;
            s.next += 1;
            s.outboxes.insert(m, Outbox::default());
            m
        };
        Self { shared, member }
    }

    fn with<T>(&self, f: impl FnOnce(&mut Shared) -> T) -> T {
        let mut s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut s)
    }

    /// How many connections share this bridge.
    pub fn members(&self) -> usize {
        self.with(|s| s.outboxes.len())
    }

    /// Whether `other` is a handle to the same bridge.
    pub fn same_bridge(&self, other: &ScriptHandle) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared)
    }

    /// See [`ScriptBridge::on_client_message`]: every member's messages drive the one script.
    pub fn on_client_message(&self, msg: &ClientMessage, activity_index: u16, host_id: u64, now: Instant) {
        self.with(|s| {
            s.bridge.on_client_message(msg, activity_index, host_id, now);
            s.collect();
        });
    }

    pub fn scripted(&self) -> bool {
        self.with(|s| s.bridge.scripted())
    }

    /// See [`ScriptBridge::teleport_finished`]: any member's finished teleport ends the hold.
    pub fn teleport_finished(&self, region: u32, now: Instant) {
        self.with(|s| s.bridge.teleport_finished(region, now));
    }

    /// See [`ScriptBridge::client_sensed`]: any member's udt 6.
    pub fn client_sensed(&self, now: Instant) {
        self.with(|s| s.bridge.client_sensed(now));
    }

    /// See [`ScriptBridge::set_served_start`].
    pub fn set_served_start(&self, region: Option<u32>) {
        self.with(|s| s.bridge.set_served_start(region));
    }

    /// See [`ScriptBridge::sensor_due`]: a frame the bridge built goes to every member; this
    /// returns this member's next one.
    pub fn sensor_due(&self, now: Instant) -> Option<Vec<u8>> {
        let member = self.member;
        self.with(|s| {
            if let Some(frame) = s.bridge.sensor_due(now) {
                for o in s.outboxes.values_mut() {
                    o.frames.push_back(frame.clone());
                }
            }
            s.collect();
            s.outboxes.get_mut(&member)?.frames.pop_front()
        })
    }

    pub fn take_region_request(&self) -> Option<u32> {
        let member = self.member;
        self.with(|s| {
            s.collect();
            s.outboxes.get_mut(&member)?.region.take()
        })
    }

    pub fn take_incident(&self) -> Option<Vec<u8>> {
        let member = self.member;
        self.with(|s| {
            s.collect();
            s.outboxes.get_mut(&member)?.incidents.pop_front()
        })
    }

    pub fn take_framework_events(&self) -> Vec<Event> {
        let member = self.member;
        self.with(|s| {
            s.collect();
            s.outboxes.get_mut(&member).map(|o| std::mem::take(&mut o.events)).unwrap_or_default()
        })
    }

    /// See [`ScriptBridge::on_sense`]: only the earliest member still present reports.
    pub fn on_sense(&self, counts: &[crate::sense::SquadCount], squads: &[SquadSense]) {
        let member = self.member;
        self.with(|s| {
            if s.outboxes.keys().next() == Some(&member) {
                s.bridge.on_sense(counts, squads);
            }
        });
    }

    /// The bridge's own counters, for tests and logs.
    pub fn frames_sent(&self) -> u64 {
        self.with(|s| s.bridge.frames_sent)
    }
}

impl Drop for ScriptHandle {
    fn drop(&mut self) {
        let member = self.member;
        self.with(|s| {
            s.outboxes.remove(&member);
        });
    }
}

/// The instances' bridges, by the shared instance's host id. One per listener, beside
/// `crate::group::InstanceGroups`.
#[derive(Default)]
pub struct SharedScripts {
    instances: Mutex<HashMap<u64, Weak<Mutex<Shared>>>>,
}

impl std::fmt::Debug for SharedScripts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedScripts").field("instances", &self.len()).finish()
    }
}

impl SharedScripts {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// A handle to `host`'s bridge, made with `make` when the instance has none (its first member,
    /// or every earlier member has gone).
    pub fn join(&self, host: u64, make: impl FnOnce() -> ScriptBridge) -> ScriptHandle {
        let mut m = self.instances.lock().unwrap_or_else(|e| e.into_inner());
        m.retain(|_, w| w.strong_count() > 0);
        if let Some(shared) = m.get(&host).and_then(Weak::upgrade) {
            return ScriptHandle::attach(shared);
        }
        let h = ScriptHandle::solo(make());
        m.insert(host, Arc::downgrade(&h.shared));
        h
    }

    /// How many instances have a bridge now.
    pub fn len(&self) -> usize {
        let m = self.instances.lock().unwrap_or_else(|e| e.into_inner());
        m.values().filter(|w| w.strong_count() > 0).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
