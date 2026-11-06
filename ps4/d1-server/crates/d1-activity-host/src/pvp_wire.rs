// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! # Stream R: a Crucible match on each client's udt 5
//!
//! [`crate::pvp_driver`] keeps one match per instance. Each client of the instance has its own
//! activity-host connection and its own udt 5 stream, so each connection has a [`CrucibleConn`]:
//! it feeds the shared driver what its client reports, and sends its client what the driver
//! publishes, in order, through the same row table and wire codec the script bridge uses
//! ([`crate::rows::RowTable`], [`crate::row_encoders::CodecWorker`]):
//!
//! * **the world's full list first** (every row of the map's list, `init = 1`), with the match's
//!   current lifetime and scoreboard in it;
//! * then **one frame per publish**: each lifetime value in its own frame (1, 2, 3, 6, 7, 8), the
//!   final scoreboard in the frame before 6 (the results data `0xAB9B80` reads the UI store at 6,
//!   and the copy into it stops at 7);
//! * scoreboard deltas between lifetimes, collapsed to the latest while a frame is out;
//! * heartbeats after 3 s idle.
//!
//! The lifetime row is the map list's own (its group, kind 16, the list's sub), and the
//! scoreboard row is the list's kind-15 row; a list without one sends no scoreboard and says so.
//! `score_to_win` rides in the lifetime value's named-value list when
//! `D1_AH_CRUCIBLE_SCORE_TO_WIN` names the int type's tag ([`crate::scoreboard_rows`]).
//!
//! **Configuration** (all off by default; [`CrucibleConfig::from_env`]):
//!
//! | Knob | Meaning |
//! |---|---|
//! | `D1_AH_CRUCIBLE=1` | Turns it on (and shared instances, for the physics-host election) |
//! | `D1_AH_CRUCIBLE_ACTIVITIES` | The Crucible activity table: the script bridge's line grammar (`INDEX ACTIVITY GROUP LIST [world=W] [start=N] ...`) plus `playlist=HASH`. LIST may be `@generated` with `D1_AH_WORLDS` |
//! | `D1_AH_CRUCIBLE_PLAYLIST` | The playlist for a launch no formed match and no table line names |
//! | `D1_AH_CRUCIBLE_MATCHES` | A file of `crucible-match HOST PLAYLIST MAP PLAYERS [FIRETEAMS]` lines (stream H's `MatchFormed`, across the process boundary), polled |
//! | `D1_AH_CRUCIBLE_COLUMNS` | The scoreboard's column table ([`crate::scoreboard_rows::Columns`]) |
//! | `D1_AH_CRUCIBLE_SCORE_TO_WIN` | The int typed value's tag: send `score_to_win` (needs `D1_RSAT_NATIVE=1`; the client's codec has no int typed value registered) |
//! | `D1_AH_CRUCIBLE_COUNTDOWN_LIFETIME=0`, `_COUNTDOWN_MS`, `_REMATCH_MS`, `_TRIALS_TICKET` | [`crate::pvp_driver::DriverConfig`] |

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::checkpoints::CharacterId;
use crate::messages::ClientMessage;
use crate::pvp_driver::{Member, Publish, SharedRegistry};
use crate::row_encoders::{lifetime_image, CodecDone, CodecJob, CodecWorker, KIND_SCOREBOARD, TAG_LIFETIME};
use crate::rows::{FramePlan, Row, RowKey, RowTable};
use crate::script_bridge::{CodecMaker, ScriptedActivity};
use crate::scoreboard_rows::{self, Columns};

/// A frame that would not encode is planned again after this.
const CODEC_BACKOFF: Duration = Duration::from_secs(3);

/// One Crucible map in the activity table.
#[derive(Clone, Debug)]
pub struct CrucibleActivity {
    /// The map's world list, parsed as the script bridge parses its table.
    pub world: ScriptedActivity,
    /// The playlist a launch of this line plays when no formed match names one.
    pub playlist: Option<u32>,
}

impl CrucibleActivity {
    /// The kind-15 row of the map's list.
    pub fn scoreboard_key(&self) -> Option<RowKey> {
        self.world.rows.iter().map(|(k, _)| *k).find(|k| k.kind == KIND_SCOREBOARD)
    }

    /// Parse a table line: the script bridge's grammar plus `playlist=HASH`.
    pub fn parse(
        line: &str,
        base: &d1_oracle::sensor::SensorPhaseUpdate,
        read: &dyn Fn(&str) -> Result<String, String>,
        worlds: Option<&crate::generated::GeneratedWorlds>,
    ) -> Result<Self, String> {
        let mut playlist = None;
        let mut rest = Vec::new();
        for t in line.split_whitespace() {
            match t.strip_prefix("playlist=") {
                Some(v) => {
                    let n = v.strip_prefix("0x").map_or_else(|| v.parse::<u32>().ok(), |h| u32::from_str_radix(h, 16).ok());
                    playlist = Some(n.ok_or_else(|| format!("{line:?}: bad playlist"))?);
                }
                None => rest.push(t),
            }
        }
        let world = ScriptedActivity::parse_with(&rest.join(" "), base, read, worlds)?;
        Ok(Self { world, playlist })
    }
}

/// Everything `D1_AH_CRUCIBLE` configures; shared by every connection.
pub struct CrucibleConfig {
    pub registry: SharedRegistry,
    pub activities: HashMap<u16, CrucibleActivity>,
    /// The client's activity table (index → hash) for a map with no table line.
    pub hash_of_index: HashMap<u16, u32>,
    pub columns: Columns,
    pub codec: CodecMaker,
    /// Incident index → name hash (the 56578 catalogue).
    pub catalog: Arc<crate::incident_catalog::IncidentCatalog>,
    /// The lifetime image's tail, as the launch parameters give it.
    pub tail: Option<(u32, u32)>,
    pub gen: u8,
    /// `D1_AH_CRUCIBLE_PLAYLIST`.
    pub playlist: Option<u32>,
    /// `D1_AH_CRUCIBLE_MATCHES` and how far it has been read.
    pub matches_file: Option<(PathBuf, Mutex<u64>)>,
    /// The driver's clock starts here.
    pub epoch: Instant,
}

impl std::fmt::Debug for CrucibleConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut idx: Vec<_> = self.activities.keys().collect();
        idx.sort();
        f.debug_struct("CrucibleConfig").field("activities", &idx).field("playlist", &self.playlist).finish()
    }
}

impl CrucibleConfig {
    /// `D1_AH_CRUCIBLE=1`: the configuration, from the knobs above. `None` when off. `data` is the
    /// framework's `crucible.json`; `rewards` the host's one rewards trait.
    pub fn from_env(
        base: &d1_oracle::sensor::SensorPhaseUpdate,
        data: Arc<crate::pvp::CrucibleData>,
        rewards: Option<Arc<Mutex<dyn crate::rewards::ActivityRewards + Send>>>,
    ) -> Result<Option<Self>, String> {
        if !crate::pvp_driver::enabled() {
            return Ok(None);
        }
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        let mut activities = HashMap::new();
        if let Some(table) = var("D1_AH_CRUCIBLE_ACTIVITIES") {
            let table = table.trim().to_string();
            let text = std::fs::read_to_string(&table).map_err(|e| format!("D1_AH_CRUCIBLE_ACTIVITIES {table}: {e}"))?;
            let read = |entry: &str| {
                let path = crate::sensor_list::listed_path(&table, entry);
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
            };
            // `@generated` lists come from the generated world tables (stream E), as the bridge's.
            let worlds_dir = var("D1_AH_WORLDS").unwrap_or_else(|| "ps4/fixtures/56578/worlds".into());
            let worlds = crate::generated::GeneratedWorlds::load(std::path::Path::new(worlds_dir.trim())).ok();
            for line in text.lines() {
                let line = line.split('#').next().unwrap_or("").trim();
                if line.is_empty() {
                    continue;
                }
                let a = CrucibleActivity::parse(line, base, &read, worlds.as_ref())?;
                activities.insert(a.world.index, a);
            }
        }
        let num = |v: String| {
            let v = v.trim().to_string();
            v.strip_prefix("0x").map_or_else(|| v.parse::<u32>().ok(), |h| u32::from_str_radix(h, 16).ok())
        };
        let config = crate::pvp_driver::DriverConfig::from_vars(var);
        if config.score_to_win_tag.is_some() && !crate::native::enabled() {
            tracing::warn!(
                "⚠ D1_AH_CRUCIBLE_SCORE_TO_WIN without D1_RSAT_NATIVE=1: the client's codec has no int typed value registered, so a lifetime with score_to_win would not encode"
            );
        }
        let catalog_path = var("D1_AH_SCRIPTS_INCIDENTS")
            .unwrap_or_else(|| concat!(env!("CARGO_MANIFEST_DIR"), "/../../../fixtures/56578/incidents-56578.tsv").to_string());
        let catalog = crate::incident_catalog::IncidentCatalog::load(std::path::Path::new(catalog_path.trim()))?;
        let shared = crate::pvp_driver::Shared::new(data, config, rewards);
        Ok(Some(Self {
            registry: crate::pvp_driver::MatchRegistry::shared(shared),
            activities,
            hash_of_index: crate::activity_driver::activity_table_from_env(),
            columns: Columns::from_env()?,
            codec: crate::script_bridge::ScriptConfig::host_codec(),
            catalog: Arc::new(catalog),
            tail: base.tail,
            gen: base.gen,
            playlist: var("D1_AH_CRUCIBLE_PLAYLIST").and_then(num),
            matches_file: var("D1_AH_CRUCIBLE_MATCHES").map(|p| (PathBuf::from(p.trim()), Mutex::new(0))),
            epoch: Instant::now(),
        }))
    }

    fn ms(&self, now: Instant) -> u64 {
        u64::try_from(now.saturating_duration_since(self.epoch).as_millis()).unwrap_or(u64::MAX)
    }

    /// The Crucible map an activity index is, if it is one.
    fn activity(&self, index: u16) -> Option<(u32, Option<&CrucibleActivity>)> {
        let a = self.activities.get(&index);
        let hash = a.map(|a| a.world.hash).filter(|h| *h != 0).or_else(|| self.hash_of_index.get(&index).copied())?;
        let reg = self.registry.lock().ok()?;
        let is_map = reg.config().data.maps.iter().any(|m| m.hash == hash);
        (a.is_some() || is_map).then_some((hash, a))
    }
}

/// ⭐ One connection's part in a Crucible match.
pub struct CrucibleConn {
    config: Arc<CrucibleConfig>,
    host: Option<u64>,
    me: Option<CharacterId>,
    activity: Option<CrucibleActivity>,
    scoreboard: Option<RowKey>,
    rows: RowTable,
    codec: Option<CodecWorker>,
    in_flight: Option<FramePlan>,
    retry_at: Option<Instant>,
    /// Which match (the driver's serial) and how far into its publishes this connection is.
    cursor: Option<(u64, usize)>,
    queue: VecDeque<Vec<Publish>>,
    identities: HashMap<String, [u8; 8]>,
    warned_no_board: bool,
    pub frames_sent: u64,
}

impl std::fmt::Debug for CrucibleConn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CrucibleConn").field("host", &self.host).field("me", &self.me).finish()
    }
}

impl CrucibleConn {
    pub fn new(config: Arc<CrucibleConfig>) -> Self {
        let gen = config.gen;
        Self {
            config,
            host: None,
            me: None,
            activity: None,
            scoreboard: None,
            rows: RowTable::new(gen),
            codec: None,
            in_flight: None,
            retry_at: None,
            cursor: None,
            queue: VecDeque::new(),
            identities: HashMap::new(),
            warned_no_board: false,
            frames_sent: 0,
        }
    }

    /// Whether this connection serves a Crucible match (its udt 5 is this module's).
    pub fn active(&self) -> bool {
        self.host.is_some() && self.activity.is_some()
    }

    pub fn host(&self) -> Option<u64> {
        self.host
    }

    pub fn rows(&self) -> &RowTable {
        &self.rows
    }

    /// Every client message, after the instance answered it. `host` is the instance's host id
    /// (the shared instance's, under [`crate::group`]), `slot` this member's slot and
    /// `physics_host` whether this member is the elected physics host.
    pub fn on_client_message(&mut self, msg: &ClientMessage, host: u64, activity_index: u16, slot: Option<usize>, physics_host: bool, now: Instant) {
        let ms = self.config.ms(now);
        match msg {
            ClientMessage::JoinRequest { struct_image } => {
                use d1_oracle::activity_host::replicate_membership::{IDENTITY_SIZE, JOIN_REQUEST_IDENTITY, JOIN_REQUEST_PROPERTIES};
                let Some(id) = struct_image.get(JOIN_REQUEST_IDENTITY..JOIN_REQUEST_IDENTITY + IDENTITY_SIZE) else { return };
                let Some((map, line)) = self.config.activity(activity_index) else {
                    if self.host.take().is_some() {
                        tracing::info!("crucible: activity {activity_index} is not a Crucible map; this connection leaves its match");
                        self.leave(ms);
                    }
                    return;
                };
                let line = line.cloned();
                let fireteam = struct_image
                    .get(JOIN_REQUEST_PROPERTIES + 0xA0..JOIN_REQUEST_PROPERTIES + 0xA8)
                    .map_or(0, |b| u64::from_le_bytes(b.try_into().unwrap()));
                let character = crate::rewards::CharacterRef { identity: id.to_vec() };
                crate::rewards::remember_identity(&character);
                let member = Member::new(character, slot, fireteam);
                let me = member.id();
                if self.host.is_some_and(|h| h != host) {
                    self.leave(ms);
                }
                let playlist = line.as_ref().and_then(|l| l.playlist).or(self.config.playlist);
                self.poll_matches_file();
                let joined = self.config.registry.lock().map_err(|_| "the match registry is poisoned".to_string()).and_then(|mut r| {
                    r.join(host, map, playlist, member, ms)?;
                    if physics_host {
                        if let Some(d) = r.get_mut(host) {
                            d.set_physics_host(Some(me));
                        }
                    }
                    Ok(())
                });
                if let Err(e) = joined {
                    tracing::warn!("⚠ crucible: host {host:#018x} activity {activity_index}: no match ({e}); served as before");
                    self.host = None;
                    return;
                }
                self.host = Some(host);
                self.me = Some(me);
                match line {
                    Some(a) => {
                        self.scoreboard = a.scoreboard_key();
                        if self.scoreboard.is_none() && !self.warned_no_board {
                            self.warned_no_board = true;
                            tracing::warn!("crucible: activity {activity_index}'s list names no scoreboard (kind 15) row; the scoreboard is not sent");
                        }
                        let mut rows = a.world.rows.clone();
                        rows.retain(|(k, _)| *k != a.world.lifetime);
                        rows.push((a.world.lifetime, self.lifetime_row(a.world.lifetime_state, None)));
                        self.rows.load_world(rows, a.world.authority.clone());
                        self.activity = Some(a);
                    }
                    None => {
                        tracing::warn!("crucible: activity {activity_index} is a Crucible map with no line in D1_AH_CRUCIBLE_ACTIVITIES (no world list): the match runs, nothing is sent");
                        self.activity = None;
                    }
                }
                self.in_flight = None;
                self.cursor = None;
                self.queue.clear();
            }
            ClientMessage::Other { user_defined_type: 18, struct_image } => {
                let (Some(host), Some(me)) = (self.host, self.me) else { return };
                let n = struct_image.get(0..4).map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()));
                let names: Vec<u32> = (0..n.min(25) as usize)
                    .filter_map(|k| struct_image.get(4 + 4 * k..8 + 4 * k).map(|b| u32::from_le_bytes(b.try_into().unwrap())))
                    .filter_map(|t| self.config.catalog.name_hash(t))
                    .collect();
                if let Ok(mut r) = self.config.registry.lock() {
                    r.report(host, me, &names, ms);
                }
            }
            _ => {}
        }
        if let (Some(host), Some(me), Ok(mut r)) = (self.host, self.me, self.config.registry.lock()) {
            if let Some(d) = r.get_mut(host) {
                if physics_host {
                    d.set_physics_host(Some(me));
                } else if d.physics_host() == Some(me) {
                    d.set_physics_host(None);
                }
            }
        }
    }

    fn poll_matches_file(&self) {
        let Some((path, offset)) = &self.config.matches_file else { return };
        let (Ok(mut off), Ok(mut r)) = (offset.lock(), self.config.registry.lock()) else { return };
        *off = r.poll_formed_file(path, *off);
    }

    fn leave(&mut self, ms: u64) {
        if let (Some(host), Some(me)) = (self.host, self.me) {
            if let Ok(mut r) = self.config.registry.lock() {
                r.leave(host, me, ms);
            }
        }
        self.activity = None;
    }

    fn lifetime_row(&self, state: i8, score_to_win: Option<(u32, i32)>) -> Row {
        let mut image = lifetime_image(state, self.config.tail, None);
        if let Some((tag, v)) = score_to_win {
            if let Err(e) = scoreboard_rows::lifetime_with_options(&mut image, &[scoreboard_rows::named_int(scoreboard_rows::SCORE_TO_WIN, tag, v)]) {
                tracing::warn!("crucible: score_to_win not written: {e}");
            }
        }
        Row { has_a: true, has_b: false, a: Some((TAG_LIFETIME, image)), b: None }
    }

    /// Take the driver's new publishes into the queue: one group per lifetime, scoreboards
    /// collapsed to the latest in a run.
    fn pull(&mut self, now: Instant) {
        let (Some(host), true) = (self.host, self.activity.is_some()) else { return };
        self.poll_matches_file();
        let ms = self.config.ms(now);
        let Ok(mut r) = self.config.registry.lock() else { return };
        r.tick(host, ms);
        let Some(d) = r.get(host) else { return };
        for m in d.members() {
            if let Some(id) = scoreboard_rows::native_identity(&m.character.identity) {
                self.identities.insert(m.id().to_string(), id);
            }
        }
        let serial = d.serial();
        let new: Vec<Publish> = match self.cursor {
            Some((s, at)) if s == serial => d.published()[at.min(d.published().len())..].to_vec(),
            // A new match, or this connection's first look: the current state, as one group.
            _ => {
                self.queue.clear();
                self.queue.push_back(d.current());
                self.cursor = Some((serial, d.published().len()));
                Vec::new()
            }
        };
        if let Some((_, at)) = self.cursor.as_mut() {
            *at += new.len();
        }
        drop(r);
        for p in new {
            match p {
                Publish::Scoreboard(_) => match self.queue.back_mut() {
                    Some(g) if g.iter().all(|x| matches!(x, Publish::Scoreboard(_))) => *g = vec![p],
                    _ => self.queue.push_back(vec![p]),
                },
                Publish::Lifetime { .. } => self.queue.push_back(vec![p]),
            }
        }
    }

    /// Put one group's rows in the table.
    fn apply(&mut self, group: Vec<Publish>) {
        let Some(a) = self.activity.as_ref() else { return };
        let lifetime = a.world.lifetime;
        for p in group {
            match p {
                Publish::Lifetime { state, score_to_win } => {
                    let row = self.lifetime_row(state, score_to_win);
                    self.rows.set(lifetime, row);
                }
                Publish::Scoreboard(s) => {
                    let Some(key) = self.scoreboard else { continue };
                    let ids = &self.identities;
                    let image = scoreboard_rows::image(&s, &self.config.columns, &|k| ids.get(k).copied());
                    let has_b = self.rows.get(&key).is_some_and(|r| r.has_b);
                    let b = self.rows.get(&key).and_then(|r| r.b.clone());
                    self.rows.set(key, Row { has_a: true, has_b, a: Some((scoreboard_rows::TAG, image)), b });
                }
            }
        }
    }

    /// The next udt 5 frame for a Crucible connection, if one is due: the full list, a delta, or
    /// a heartbeat. Call it on every poll.
    pub fn sensor_due(&mut self, now: Instant) -> Option<Vec<u8>> {
        if !self.active() {
            return None;
        }
        self.pull(now);
        let mut frame = None;
        while let Some(done) = self.codec.as_mut().and_then(CodecWorker::poll) {
            match done {
                Ok(CodecDone::Frame(r)) => {
                    if let Some(plan) = self.in_flight.take() {
                        frame = frame.or(self.finish(plan, r, now));
                    }
                }
                Ok(CodecDone::Object(_)) => {}
                Err(e) => {
                    if let Some(plan) = self.in_flight.take() {
                        self.finish(plan, Err(e), now);
                    }
                    self.codec = None;
                    break;
                }
            }
        }
        if frame.is_some() {
            return frame;
        }
        let backing_off = self.retry_at.is_some_and(|t| now < t);
        if self.in_flight.is_none() && !backing_off {
            // Before the world's full list: everything queued is the current state. After it: one
            // group per frame, so each lifetime value lands on its own.
            if !self.rows.full_delivered() {
                while let Some(g) = self.queue.pop_front() {
                    self.apply(g);
                }
            } else if self.rows.dirty_len() == 0 {
                if let Some(g) = self.queue.pop_front() {
                    self.apply(g);
                }
            }
            if let Some(plan) = self.rows.plan() {
                let codec = self.codec.get_or_insert_with(|| (self.config.codec)());
                codec.submit(CodecJob::Frame(plan.clone()));
                self.in_flight = Some(plan);
                if let Some(Ok(CodecDone::Frame(r))) = self.codec.as_mut().and_then(CodecWorker::poll) {
                    let plan = self.in_flight.take().expect("just set");
                    if let Some(f) = self.finish(plan, r, now) {
                        return Some(f);
                    }
                }
            }
        }
        self.rows.heartbeat_due(now)
    }

    fn finish(&mut self, plan: FramePlan, result: Result<crate::row_encoders::EncodedFrame, String>, now: Instant) -> Option<Vec<u8>> {
        let fail = |this: &mut Self, plan: FramePlan, e: String| {
            tracing::warn!("crucible udt 5 {} not sent: {e}", if plan.full { "full list" } else { "delta" });
            this.rows.failed(plan);
            this.retry_at = Some(now + CODEC_BACKOFF);
            None
        };
        match result {
            Ok(mut f) => {
                let stamp = self.rows.tick(now);
                match self.rows.stamp_at(&mut f.bytes, f.stamp_bit, plan.init, stamp) {
                    Ok(()) => {
                        self.rows.sent(&plan, now);
                        self.retry_at = None;
                        self.frames_sent += 1;
                        Some(f.bytes)
                    }
                    Err(e) => fail(self, plan, e),
                }
            }
            Err(e) => fail(self, plan, e),
        }
    }
}

impl Drop for CrucibleConn {
    fn drop(&mut self) {
        let ms = self.config.ms(Instant::now());
        self.leave(ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pvp_driver::tests::{data, identity, playlist_of};
    use crate::pvp_driver::{incidents, DriverConfig, MatchRegistry, Shared};
    use crate::rewards::{ActivityOutcome, ActivityRewards, CharacterRef, RecordingRewards};
    use crate::row_encoders::{EncodedFrame, KIND_LIFETIME};

    const GROUP: u32 = 0x5A5A_0001;
    const LIST: &str = "@authority 13
@phase 0
0x5a5a0001 0xf 0 1 0
0x5a5a0001 0x6 0 1 0
";

    fn base() -> d1_oracle::sensor::SensorPhaseUpdate {
        d1_oracle::sensor::SensorPhaseUpdate { gen: 0, name_hash: GROUP, name_i8: KIND_LIFETIME, name_sub: 3, phase: 0, tail: None, clients: Vec::new(), authority: None }
    }

    #[derive(Default)]
    struct Wire {
        /// Each frame's plan, per connection.
        frames: Vec<(usize, FramePlan)>,
    }

    fn recording(wire: Arc<Mutex<Wire>>, conn: usize) -> CodecMaker {
        Arc::new(move || {
            let wire = wire.clone();
            CodecWorker::inline(Box::new(move |job| match job {
                CodecJob::Frame(plan) => {
                    wire.lock().unwrap().frames.push((conn, plan.clone()));
                    let mut bytes = d1_oracle::sensor::heartbeat_frame(plan.gen, 0);
                    bytes.push(plan.entries.len() as u8);
                    CodecDone::Frame(Ok(EncodedFrame { bytes, stamp_bit: 9 }))
                }
                CodecJob::Object { .. } => CodecDone::Object(Err("no objects".into())),
            }))
        })
    }

    /// A map with its world line, a playlist of `mode`, and a recording codec per connection.
    struct Bench {
        registry: SharedRegistry,
        rewards: Arc<Mutex<RecordingRewards>>,
        wire: Arc<Mutex<Wire>>,
        map: u32,
        epoch: Instant,
        line: CrucibleActivity,
    }

    impl Bench {
        fn new(mode: &str, flavour: Option<&str>, config: DriverConfig) -> Self {
            let d = data();
            let mut maps: Vec<u32> = d.maps.iter().map(|m| m.hash).collect();
            maps.sort_unstable();
            let map = maps[0];
            let playlist = playlist_of(&d, mode, flavour);
            let rewards = Arc::new(Mutex::new(RecordingRewards::default()));
            let r: Arc<Mutex<dyn ActivityRewards + Send>> = rewards.clone();
            let registry = MatchRegistry::shared(Shared::new(d, config, Some(r)));
            let line = CrucibleActivity::parse(&format!("470 crucible_map {GROUP:#x} list.txt hash={map:#x} playlist={playlist:#x}"), &base(), &|_| Ok(LIST.to_string()), None).unwrap();
            Self { registry, rewards, wire: Arc::new(Mutex::new(Wire::default())), map, epoch: Instant::now(), line }
        }

        fn conn(&self, n: usize) -> CrucibleConn {
            let catalog = crate::incident_catalog::IncidentCatalog::load(
                &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/56578/incidents-56578.tsv"),
            )
            .unwrap();
            let config = CrucibleConfig {
                registry: self.registry.clone(),
                activities: [(470u16, self.line.clone())].into_iter().collect(),
                hash_of_index: HashMap::new(),
                columns: Columns::default(),
                codec: recording(self.wire.clone(), n),
                catalog: Arc::new(catalog),
                tail: None,
                gen: 0,
                playlist: None,
                matches_file: None,
                epoch: self.epoch,
            };
            CrucibleConn::new(Arc::new(config))
        }

        fn at(&self, ms: u64) -> Instant {
            self.epoch + Duration::from_millis(ms)
        }
    }

    fn join_request(c: &CharacterRef, fireteam: u64) -> ClientMessage {
        use d1_oracle::activity_host::replicate_membership::{IDENTITY_SIZE, JOIN_REQUEST_IDENTITY, JOIN_REQUEST_PROPERTIES};
        let mut image = vec![0u8; JOIN_REQUEST_PROPERTIES + 0x100];
        image[JOIN_REQUEST_IDENTITY..JOIN_REQUEST_IDENTITY + IDENTITY_SIZE].copy_from_slice(&c.identity);
        image[JOIN_REQUEST_PROPERTIES + 0xA0..JOIN_REQUEST_PROPERTIES + 0xA8].copy_from_slice(&fireteam.to_le_bytes());
        ClientMessage::JoinRequest { struct_image: image }
    }

    /// A udt 18 naming these incidents by their 56578 index.
    fn incident(indices: &[u32]) -> ClientMessage {
        let mut image = vec![0u8; crate::completion::ENVELOPE_SIZE];
        image[0..4].copy_from_slice(&(indices.len() as u32).to_le_bytes());
        for (k, i) in indices.iter().enumerate() {
            image[4 + 4 * k..8 + 4 * k].copy_from_slice(&i.to_le_bytes());
        }
        ClientMessage::Other { user_defined_type: 18, struct_image: image }
    }

    const KILL_CHAIN: [u32; 2] = [734, 1825];
    const DIED: [u32; 1] = [2079];
    const HOST: u64 = 0xD1A0_0000_0000_0042;

    /// Poll every connection from `from` to `to` ms in 100 ms steps.
    fn run(b: &Bench, conns: &mut [CrucibleConn], from: u64, to: u64) {
        let mut t = from;
        while t <= to {
            for c in conns.iter_mut() {
                c.sensor_due(b.at(t));
            }
            t += 100;
        }
    }

    fn lifetime_of(plan: &FramePlan, key: RowKey) -> Option<i8> {
        plan.entries.iter().find(|(k, _)| *k == key).and_then(|(_, r)| r.a.as_ref()).map(|(_, i)| i[0] as i8)
    }

    /// The lifetimes conn `n`'s frames carried, in order (repeats in a full list dropped).
    fn lifetimes(w: &Wire, n: usize, key: RowKey) -> Vec<i8> {
        let mut v: Vec<i8> = Vec::new();
        for (c, p) in &w.frames {
            if *c == n {
                if let Some(l) = lifetime_of(p, key) {
                    if v.last() != Some(&l) {
                        v.push(l);
                    }
                }
            }
        }
        v
    }

    fn board_of(plan: &FramePlan, key: RowKey) -> Option<Vec<u8>> {
        plan.entries.iter().find(|(k, _)| *k == key).and_then(|(_, r)| r.a.as_ref()).map(|(_, i)| i.clone())
    }

    fn f32_at(b: &[u8], at: usize) -> f32 {
        f32::from_le_bytes(b[at..at + 4].try_into().unwrap())
    }

    fn i32_at(b: &[u8], at: usize) -> i32 {
        i32::from_le_bytes(b[at..at + 4].try_into().unwrap())
    }

    /// ⭐ A whole synthetic Control match through four fake clients: join, countdown, kills and
    /// deaths reported by the consoles, zone captures, the score limit, results, post-game,
    /// rematch; every frame's lifetime in order; the final board before 6; one grant each.
    #[test]
    fn a_whole_control_match_through_fake_clients() {
        let b = Bench::new("control", None, DriverConfig::default());
        let lifetime = b.line.world.lifetime;
        let board = b.line.scoreboard_key().expect("the list's kind-15 row");
        let mut conns: Vec<CrucibleConn> = (0..4).map(|n| b.conn(n)).collect();
        let ids: Vec<CharacterRef> = (1..=4).map(|n| identity(n, if n <= 2 { 11 } else { 22 })).collect();
        // The formed match: four players, two fireteams.
        b.registry.lock().unwrap().formed(crate::pvp_driver::MatchSetup {
            host: HOST,
            map: b.map,
            playlist: b.line.playlist.unwrap(),
            expected_players: Some(4),
            fireteams: vec![11, 22],
        });
        for (n, c) in conns.iter_mut().enumerate() {
            c.on_client_message(&join_request(&ids[n], if n < 2 { 11 } else { 22 }), HOST, 470, Some(n), n == 0, b.at(0));
            assert!(c.active());
        }
        {
            let r = b.registry.lock().unwrap();
            let d = r.get(HOST).unwrap();
            assert_eq!(d.physics_host(), Some(crate::pvp_driver::tests::id(1)), "the first member is the physics host");
            let team = |n: u64| d.member(crate::pvp_driver::tests::id(n)).unwrap().team;
            assert_eq!(team(1), team(2), "a fireteam stays together");
            assert_eq!(team(3), team(4));
            assert_ne!(team(1), team(3));
            assert_eq!(d.lifetime(), 2, "four joined: the countdown");
        }
        run(&b, &mut conns, 0, 9_900);
        run(&b, &mut conns, 10_000, 10_500);
        assert_eq!(b.registry.lock().unwrap().get(HOST).unwrap().lifetime(), 3, "play");
        // Players 1 and 2 (team A) kill 3 and 4 over and over; 3 captures a zone once.
        let mut t = 11_000;
        let mut ended_at = None;
        for k in 0..200u64 {
            let (killer, victim) = if k % 2 == 0 { (0, 2) } else { (1, 3) };
            conns[killer].on_client_message(&incident(&KILL_CHAIN), HOST, 470, Some(killer), killer == 0, b.at(t));
            conns[victim].on_client_message(&incident(&DIED), HOST, 470, Some(victim), false, b.at(t + 200));
            if k == 3 {
                conns[2].on_client_message(&incident(&[774, 1338]), HOST, 470, Some(2), false, b.at(t + 300));
            }
            run(&b, &mut conns, t, t + 900);
            t += 1_000;
            if b.registry.lock().unwrap().get(HOST).unwrap().lifetime() >= 6 {
                ended_at = Some(t);
                break;
            }
        }
        let ended_at = ended_at.expect("the score limit ends the match");
        {
            let r = b.registry.lock().unwrap();
            let d = r.get(HOST).unwrap();
            let res = d.result().unwrap();
            let winner = d.member(crate::pvp_driver::tests::id(1)).unwrap().team;
            assert_eq!(res.winning_team, winner);
            let limit = d.mode().score_limit.unwrap();
            assert!(res.team_scores[&winner.unwrap()] >= limit);
            let loser = d.member(crate::pvp_driver::tests::id(3)).unwrap().team.unwrap();
            assert_eq!(res.team_scores[&loser], 200.0, "one zone (stand-in 200 points)");
            assert_eq!(d.grants().len(), 4);
        }
        run(&b, &mut conns, ended_at, ended_at + 61_000);
        assert_eq!(b.registry.lock().unwrap().get(HOST).unwrap().lifetime(), 8);
        let w = b.wire.lock().unwrap();
        for n in 0..4 {
            assert_eq!(lifetimes(&w, n, lifetime), vec![2, 3, 6, 7, 8], "conn {n}: the full list carries 2, then one frame each");
            let mine: Vec<&FramePlan> = w.frames.iter().filter(|(c, _)| *c == n).map(|(_, p)| p).collect();
            assert!(mine[0].full && mine[0].init, "the full list first");
            assert_eq!(mine[0].entries.len(), 3, "every row of the map's list");
            for p in &mine[1..] {
                assert!(!p.full);
                assert!(p.entries.len() == 1, "one row per delta: {:?}", p.entries.iter().map(|e| e.0).collect::<Vec<_>>());
            }
            // The frame before 6 is the final board, with the winner's score.
            let six = mine.iter().position(|p| lifetime_of(p, lifetime) == Some(6)).unwrap();
            let last_board = mine[..six].iter().rev().find_map(|p| board_of(p, board)).expect("a board before 6");
            assert_eq!(i32_at(&last_board, scoreboard_rows::PLAYER_COUNT), 4, "every player has a row");
            assert_eq!(i32_at(&last_board, scoreboard_rows::TEAM_COUNT), 2);
            let team_scores: Vec<f32> = (0..2).map(|t| f32_at(&last_board, scoreboard_rows::TEAM_ROWS + scoreboard_rows::TEAM_STRIDE * t + scoreboard_rows::TEAM_STATS)).collect();
            assert!(team_scores.iter().any(|s| *s >= 15000.0), "{team_scores:?}");
            assert!(mine[six..].iter().all(|p| board_of(p, board).is_none()), "no board after the results");
        }
        drop(w);
        let r = b.rewards.lock().unwrap();
        assert_eq!(r.grants.len(), 4, "once per player");
        let won = r.grants.iter().filter(|g| matches!(g.2, ActivityOutcome::Crucible(o) if o.won)).count();
        assert_eq!(won, 2);
        assert!(r.grants.iter().all(|g| g.1 == b.line.playlist.unwrap()), "granted for the playlist");
        assert!(r.grants.iter().all(|g| matches!(g.2, ActivityOutcome::Crucible(o) if o.faction == crate::pvp_driver::Faction::Crucible && o.reputation > 0 && o.trials.is_none())));
    }

    /// ⭐ A whole synthetic Elimination match: three a side, no respawns, rounds from the deaths
    /// alone, revives, first to the round count.
    #[test]
    fn a_whole_elimination_match_through_fake_clients() {
        let b = Bench::new("elimination", None, DriverConfig { countdown_ms: 1_000, round_break_ms: 2_000, ..DriverConfig::default() });
        let lifetime = b.line.world.lifetime;
        let mut conns: Vec<CrucibleConn> = (0..6).map(|n| b.conn(n)).collect();
        let ids: Vec<CharacterRef> = (1..=6).map(|n| identity(n, if n <= 3 { 11 } else { 22 })).collect();
        b.registry.lock().unwrap().formed(crate::pvp_driver::MatchSetup { host: HOST, map: b.map, playlist: b.line.playlist.unwrap(), expected_players: Some(6), fireteams: vec![11, 22] });
        for (n, c) in conns.iter_mut().enumerate() {
            c.on_client_message(&join_request(&ids[n], if n < 3 { 11 } else { 22 }), HOST, 470, Some(n), n == 0, b.at(0));
        }
        run(&b, &mut conns, 0, 1_500);
        let wins_needed = b.registry.lock().unwrap().get(HOST).unwrap().mode().rounds.as_ref().unwrap().wins_needed;
        let mut t = 2_000;
        for round in 0..wins_needed {
            // Team A wipes team B; in round 0 one of B is revived first and dies again.
            for v in 3..6 {
                let k = v - 3;
                conns[k].on_client_message(&incident(&KILL_CHAIN), HOST, 470, Some(k), k == 0, b.at(t));
                conns[v].on_client_message(&incident(&DIED), HOST, 470, Some(v), false, b.at(t + 100));
                run(&b, &mut conns, t, t + 500);
                t += 600;
                if round == 0 && v == 3 {
                    conns[3].on_client_message(&incident(&[3615]), HOST, 470, Some(3), false, b.at(t));
                    conns[0].on_client_message(&incident(&KILL_CHAIN), HOST, 470, Some(0), true, b.at(t + 100));
                    conns[3].on_client_message(&incident(&DIED), HOST, 470, Some(3), false, b.at(t + 200));
                    run(&b, &mut conns, t, t + 500);
                    t += 600;
                }
            }
            let r = b.registry.lock().unwrap();
            let d = r.get(HOST).unwrap();
            let a = d.member(crate::pvp_driver::tests::id(1)).unwrap().team.unwrap();
            assert_eq!(d.round_wins(a), round + 1, "round {round}");
            drop(r);
            run(&b, &mut conns, t, t + 2_500);
            t += 2_600;
        }
        let r = b.registry.lock().unwrap();
        let d = r.get(HOST).unwrap();
        let res = d.result().expect("the round count ends it");
        let a = d.member(crate::pvp_driver::tests::id(1)).unwrap().team;
        assert_eq!(res.winning_team, a);
        let kills = |n: u64| d.result().unwrap().standings.iter().find(|s| s.player == crate::pvp_driver::tests::id(n)).unwrap().kills;
        assert_eq!(kills(1), f64::from(wins_needed) + 1.0, "player 1 killed in each round, twice in the first");
        assert_eq!(d.lifetime(), 6);
        drop(r);
        let w = b.wire.lock().unwrap();
        assert_eq!(lifetimes(&w, 5, lifetime), vec![2, 3, 6], "rounds stay at 3");
        assert_eq!(b.rewards.lock().unwrap().grants.len(), 6);
    }

    /// Iron Banner: its faction's reputation, level advantages, and the grant line's `rep=`.
    #[test]
    fn iron_banner_grants_its_faction() {
        let b = Bench::new("control", Some("iron_banner"), DriverConfig { countdown_ms: 0, ..DriverConfig::default() });
        let mut conns: Vec<CrucibleConn> = (0..2).map(|n| b.conn(n)).collect();
        for (n, c) in conns.iter_mut().enumerate() {
            c.on_client_message(&join_request(&identity(n as u64 + 1, 11 + n as u64), 11 + n as u64), HOST, 470, Some(n), n == 0, b.at(0));
        }
        run(&b, &mut conns, 0, 500);
        conns[0].on_client_message(&incident(&KILL_CHAIN), HOST, 470, Some(0), true, b.at(600));
        conns[1].on_client_message(&incident(&DIED), HOST, 470, Some(1), false, b.at(700));
        drop(conns.pop());
        run(&b, &mut conns, 800, 1_000);
        let r = b.rewards.lock().unwrap();
        assert_eq!(r.grants.len(), 1, "the forfeit's winner; the leaver gets nothing");
        let ActivityOutcome::Crucible(o) = r.grants[0].2 else { panic!("{:?}", r.grants[0]) };
        assert!(o.won && o.level_advantages);
        assert_eq!(o.faction, crate::pvp_driver::Faction::IronBanner);
        let rules = &data().rewards;
        assert_eq!(i64::from(o.reputation), rules.reputation_completed["Iron Banner"] + rules.reputation_win_bonus["Iron Banner"]);
        assert_eq!(
            crate::reward_forward::crucible_reputation(&o),
            vec![("faction_event_iron_banner".to_string(), o.reputation)]
        );
    }

    /// Play one Trials match on `host`: whoever `win` says takes every round by kills.
    fn trials_match(b: &Bench, host: u64, win: bool) {
        let mut conns: Vec<CrucibleConn> = (0..2).map(|n| b.conn(n)).collect();
        for (n, c) in conns.iter_mut().enumerate() {
            c.on_client_message(&join_request(&identity(n as u64 + 1, 11 + n as u64), 11 + n as u64), host, 470, Some(n), n == 0, b.at(0));
        }
        run(b, &mut conns, 0, 300);
        let (k, v) = if win { (0, 1) } else { (1, 0) };
        let mut t = 400;
        while b.registry.lock().unwrap().get(host).unwrap().lifetime() < 6 {
            conns[k].on_client_message(&incident(&KILL_CHAIN), host, 470, Some(k), k == 0, b.at(t));
            conns[v].on_client_message(&incident(&DIED), host, 470, Some(v), v == 0, b.at(t + 100));
            run(b, &mut conns, t, t + 6_000);
            t += 6_100;
            assert!(t < 120_000, "the match ends");
        }
    }

    /// A Trials card: Boon counts the first win twice, Mercy forgives the first loss, a flawless
    /// card reaches the Lighthouse at nine wins with the three rewards, and a finished card
    /// starts afresh.
    #[test]
    fn a_trials_card_follows_its_matches() {
        let b = Bench::new("trials", Some("trials"), DriverConfig { countdown_ms: 0, round_break_ms: 1_000, ..DriverConfig::default() });
        let me = crate::pvp_driver::tests::id(1);
        b.registry.lock().unwrap().config().trials.lock().unwrap().set_boons(me, vec![crate::pvp::Boon::Boon, crate::pvp::Boon::Mercy]);
        let mut cards = Vec::new();
        for (k, win) in [true, false, true, true, true, true, true, true, true].into_iter().enumerate() {
            trials_match(&b, HOST + k as u64, win);
            let r = b.rewards.lock().unwrap();
            let o = r.grants.iter().rev().find(|g| g.0 == identity(1, 11).key()).map(|g| g.2).unwrap();
            let ActivityOutcome::Crucible(o) = o else { panic!("{o:?}") };
            assert_eq!(o.won, win, "match {k}");
            cards.push((o, o.trials.expect("a Trials outcome carries the card")));
        }
        assert_eq!((cards[0].1.wins, cards[0].1.wins_added), (2, 2), "Boon: the first win counts twice");
        assert_eq!((cards[1].1.losses, cards[1].1.losses_added), (0, 0), "Mercy: the first loss is forgiven");
        let (last_o, last) = cards[8];
        assert_eq!(last.wins, 9);
        assert_eq!(last.state, crate::pvp_driver::CardState::Complete { flawless: true }, "the Lighthouse");
        assert_eq!(last.rewards_reached, 3, "5, 7 and 9 wins");
        assert!(last_o.level_advantages);
        assert!(crate::reward_forward::crucible_source(&last_o).ends_with(":trials=9-0:flawless"));
        assert!(crate::reward_forward::crucible_reputation(&last_o).contains(&("r1_s4_tickets.pvp.trials_of_osiris.wins".to_string(), 1)));
        // The opponent (no boons) lost eight of nine: its card ended at three losses, twice.
        let ended = b.rewards.lock().unwrap().grants.iter().filter(|g| g.0 == identity(2, 12).key())
            .filter(|g| matches!(g.2, ActivityOutcome::Crucible(o) if o.trials.is_some_and(|t| t.state == crate::pvp_driver::CardState::Ended)))
            .count();
        assert_eq!(ended, 2);
        // The next match starts a fresh card (with the boons again).
        trials_match(&b, HOST + 100, false);
        let r = b.rewards.lock().unwrap();
        let ActivityOutcome::Crucible(o) = r.grants.iter().rev().find(|g| g.0 == identity(1, 11).key()).unwrap().2 else { unreachable!() };
        let fresh = o.trials.unwrap();
        assert_eq!((fresh.wins, fresh.losses, fresh.state), (0, 0, crate::pvp_driver::CardState::Continues), "a fresh card, its Mercy spent on this loss");
    }

    /// The server never simulates combat: nothing it sends is a combat message, only udt 5 frames
    /// of the lifetime and scoreboard rows; the physics host stays a console, and moves when it
    /// leaves.
    #[test]
    fn the_server_only_relays_and_the_physics_host_is_a_console() {
        let b = Bench::new("control", None, DriverConfig { countdown_ms: 0, ..DriverConfig::default() });
        let lifetime = b.line.world.lifetime;
        let board = b.line.scoreboard_key().unwrap();
        let mut conns: Vec<CrucibleConn> = (0..3).map(|n| b.conn(n)).collect();
        for (n, c) in conns.iter_mut().enumerate() {
            c.on_client_message(&join_request(&identity(n as u64 + 1, 11 + n as u64), 11 + n as u64), HOST, 470, Some(n), n == 0, b.at(0));
        }
        run(&b, &mut conns, 0, 500);
        conns[0].on_client_message(&incident(&KILL_CHAIN), HOST, 470, Some(0), true, b.at(600));
        run(&b, &mut conns, 600, 2_000);
        let w = b.wire.lock().unwrap();
        for (_, p) in &w.frames {
            for (k, _) in &p.entries {
                assert!(p.full || *k == lifetime || *k == board, "a delta carries only the match's rows: {k}");
            }
        }
        drop(w);
        // The physics host leaves: the group elects another console (group.rs), which the
        // connection reports on its next message.
        drop(conns.remove(0));
        conns[0].on_client_message(&incident(&[0x1234]), HOST, 470, Some(1), true, b.at(2_100));
        assert_eq!(b.registry.lock().unwrap().get(HOST).unwrap().physics_host(), Some(crate::pvp_driver::tests::id(2)));
    }

    #[test]
    fn a_table_line_takes_its_playlist() {
        let a = CrucibleActivity::parse(&format!("470 m {GROUP:#x} l.txt playlist=0x1234 hash=0x55"), &base(), &|_| Ok(LIST.to_string()), None).unwrap();
        assert_eq!(a.playlist, Some(0x1234));
        assert_eq!(a.world.hash, 0x55);
        assert_eq!(a.scoreboard_key(), Some(RowKey { group: GROUP, kind: KIND_SCOREBOARD, index: 0 }));
        assert!(CrucibleActivity::parse(&format!("470 m {GROUP:#x} l.txt playlist=zz"), &base(), &|_| Ok(LIST.to_string()), None).is_err());
    }

    #[test]
    fn score_to_win_rides_the_lifetime_when_its_tag_is_given() {
        let b = Bench::new("control", None, DriverConfig { score_to_win_tag: Some(0x8080_0007), ..DriverConfig::default() });
        let lifetime = b.line.world.lifetime;
        let mut c = b.conn(0);
        c.on_client_message(&join_request(&identity(1, 11), 11), HOST, 470, Some(0), true, b.at(0));
        c.sensor_due(b.at(0));
        let w = b.wire.lock().unwrap();
        let (_, first) = &w.frames[0];
        let image = first.entries.iter().find(|(k, _)| *k == lifetime).and_then(|(_, r)| r.a.clone()).unwrap().1;
        let limit = b.registry.lock().unwrap().get(HOST).unwrap().mode().score_limit.unwrap();
        assert_eq!(scoreboard_rows::read_option(&image, scoreboard_rows::SCORE_TO_WIN, 0x8080_0007), Some(limit as i32));
        assert_eq!(image[0], 1, "pregame");
    }

    #[test]
    fn a_map_without_a_playlist_starts_no_match() {
        let b = Bench::new("control", None, DriverConfig::default());
        let mut line = b.line.clone();
        line.playlist = None;
        let mut c = b.conn(0);
        Arc::get_mut(&mut c.config).unwrap().activities.insert(470, line);
        c.on_client_message(&join_request(&identity(1, 11), 11), HOST, 470, Some(0), true, b.at(0));
        assert!(!c.active(), "no formed match, no playlist: served as before");
        assert!(c.sensor_due(b.at(0)).is_none());
    }

    #[test]
    fn a_kill_incident_reaches_the_driver_through_the_catalogue() {
        let b = Bench::new("control", None, DriverConfig { countdown_ms: 0, ..DriverConfig::default() });
        let mut conns: Vec<CrucibleConn> = (0..2).map(|n| b.conn(n)).collect();
        for (n, c) in conns.iter_mut().enumerate() {
            c.on_client_message(&join_request(&identity(n as u64 + 1, 11 + n as u64), 11 + n as u64), HOST, 470, Some(n), n == 0, b.at(0));
        }
        run(&b, &mut conns, 0, 200);
        // 1825 is `kill` in the catalogue: the console that killed reports it.
        assert_eq!(conns[0].config.catalog.name_hash(1825), Some(incidents::KILL));
        conns[0].on_client_message(&incident(&KILL_CHAIN), HOST, 470, Some(0), true, b.at(300));
        conns[1].on_client_message(&incident(&DIED), HOST, 470, Some(1), false, b.at(400));
        let r = b.registry.lock().unwrap();
        let d = r.get(HOST).unwrap();
        let s = d.scoreboard();
        let p1 = s.players.iter().find(|p| p.character_key == crate::pvp_driver::tests::id(1).to_string()).unwrap();
        assert!(p1.stats.iter().any(|x| x.name_hash == crate::pvp::fnv1("kills") && x.value == 1.0));
    }
}
