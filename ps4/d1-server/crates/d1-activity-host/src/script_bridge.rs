// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! ⭐ The script bridge: Destiny's own mission scripts drive this activity host, through a sidecar
//! process running Blam Network's MIT BullShcript VM (`tools/script-host`, Stream A1).
//!
//! `D1_AH_SCRIPTS=1` turns it on; it is off by default, and an activity the bridge's table does not
//! name is served exactly as before (world table, mission steps). For a scripted activity the bridge
//! owns the udt 5 stream: the sidecar's row changes go through [`crate::row_encoders`] into the row
//! table ([`crate::rows`]), which sends the full list at the join and only changed rows afterwards.
//!
//! # Process model
//!
//! One sidecar per activity instance, spawned at the join of a scripted activity, speaking JSON
//! lines on stdin/stdout (stderr is logged). The host owns the clock: it sends `tick` once a
//! second with the activity time in 30 Hz ticks, and the sidecar answers with the changes that tick
//! made, then `tick_done`. Client events go in host order between ticks, so a run replays from the
//! host log. A sidecar that cannot start is retried twice; one that exits after `start` stops the
//! progression (no ticks, no events) while the rows already sent stay and heartbeats continue, so
//! the client is not dropped. Nothing is guessed in its place.
//!
//! # Messages (`proto` 1; F:/d1/third-party/trawl-2026-11-05/fukugen-gap-analysis.md §4.3)
//!
//! Every line is one JSON object with a `"type"`. Item references are BullShcript's
//! `{bundle, typeId, typeIndex}`; the bridge maps a bundle hash to the world's live group hash
//! (`map=` in the activity table, identity otherwise).
//!
//! Host → sidecar ([`HostMsg`]): `hello`, `start`, `tick {now}`, `players {players:[{key, slot}]}`,
//! `player_spawned {key}`, `player_died {key}`, `trigger {item}`, `cinematic_finished {item}`,
//! `cinematic_skipped {item, key}`, `incident {name_hash, index}` (every other client incident),
//! `squad_alive {item, count}`, `squad_state {item, state, flag}`, `slice {region}`, `stop`.
//!
//! Sidecar → host ([`SidecarMsg`]): `ready`, `rows {changes:[ItemChange]}` (BullShcript's type),
//! `rows_cleared {items}`, `phase {index, name, region}`, `activity_flag {hash, active}`,
//! `activity_complete`, `ended`, `hard_wipe_armed`, `all_dead`, `tick_done {now}`, `log`, `error`.
//!
//! # What each message does here
//!
//! * **Lifetime.** A `lifetime` row change sets the world's lifetime record (the one the world's
//!   list names) to [`lifetime_wire_state`]: pregame → 1, active → 3, cinematic hold → 4,
//!   results → 6, ended → 8. `phase` carries the mission-phase tuple only under
//!   `D1_AH_SCRIPTS_MISSION_PHASE=1` (its meaning on 56578 is unverified), and asks the host to
//!   move the client to the phase's region when it is not already there.
//! * **`activity_complete`** sends host incident **965** (`activity_complete`, from the incident
//!   catalogue) with the body in `D1_AH_SCRIPTS_COMPLETE_BODY`. The 56578 body is not yet known
//!   (notes/ps4-56578-fukugen-daihon-2026-11-05.md); without the knob the incident is not sent and
//!   the log says so, rather than sending an empty or captured body.
//! * **`ended`** calls [`ActivityRewards::grant_activity_rewards`] for each joined character.
//! * **Client incidents** (udt 18) are translated with the 56578 catalogue: 3403 → `trigger`
//!   (its row from the body), 891 / 1726 → `cinematic_finished` / `cinematic_skipped` (the row named
//!   in the body as 3403 names its trigger, else the one cinematic playing; never a guess between
//!   two), 566 / 2014 → `player_spawned`, 2079 → `player_died`, anything else → `incident`.
//! * **Client senses** (udt 6, `D1_AH_SENSE_SCHEMAS`): a squad's live-member count (B `+0x0C`,
//!   proven by the member-death control) → `squad_alive`; its state byte `+0x19` and bit `+0x1A`
//!   (traced statically from the squad record's flags; meaning open) → `squad_state`.

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::incident_catalog::{known, IncidentCatalog};
use crate::messages::ClientMessage;
use crate::rewards::{ActivityOutcome, ActivityRewards, CharacterRef, NoRewards};
use crate::row_encoders::{
    lifetime_image, lifetime_wire_state, CodecWorker, EncodeContext, Encoded, MappedRef, MissionPhase,
    OracleRowCodec, RowCodec, RowEncoders, TAG_LIFETIME,
};
use crate::rows::{FramePlan, Row, RowKey, RowTable};

pub const PROTO: u32 = 1;
/// The activity clock the sidecar counts in (the harness's: 30 Hz).
pub const TICK_HZ: u32 = 30;

// ── the protocol ─────────────────────────────────────────────────────────────────────────────────

/// BullShcript's `ItemRef`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ItemRef {
    pub bundle: u32,
    #[serde(rename = "typeId")]
    pub type_id: u8,
    #[serde(rename = "typeIndex")]
    pub type_index: i16,
}

/// BullShcript's `ItemChange`: `{item, kind, state, initial?}`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ItemChange {
    pub item: ItemRef,
    pub kind: String,
    #[serde(default)]
    pub state: Value,
    #[serde(default)]
    pub initial: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PlayerInfo {
    pub key: String,
    pub slot: u32,
}

/// Host → sidecar.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostMsg {
    Hello {
        proto: u32,
        build: String,
        activity: String,
        activity_index: u16,
        #[serde(skip_serializing_if = "Option::is_none")]
        bundles_path: Option<String>,
        tick_hz: u32,
        script_tick_s: f32,
    },
    Start,
    Tick { now: u64 },
    Players { players: Vec<PlayerInfo> },
    PlayerSpawned { key: String },
    PlayerDied { key: String },
    Trigger { item: ItemRef },
    CinematicFinished { item: ItemRef },
    CinematicSkipped { item: ItemRef, key: String },
    Incident { name_hash: u32, index: u32 },
    SquadAlive { item: ItemRef, count: i32 },
    SquadState { item: ItemRef, state: u8, flag: bool },
    Slice { region: u32 },
    Stop,
}

/// Sidecar → host.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SidecarMsg {
    Ready {
        #[serde(default)]
        unresolved: Vec<Value>,
        #[serde(default)]
        drives_lifetime: bool,
    },
    Rows {
        #[serde(alias = "rows")]
        changes: Vec<ItemChange>,
    },
    RowsCleared { items: Vec<ItemRef> },
    Phase {
        index: u32,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        region: Option<u32>,
    },
    ActivityFlag { hash: u32, active: bool },
    ActivityComplete {},
    Ended {},
    HardWipeArmed {},
    AllDead {},
    TickDone { now: u64 },
    Log { text: String },
    Error { text: String },
}

/// Parse one line from the sidecar. `Ok(None)` for a blank line.
pub fn parse_sidecar_line(line: &str) -> Result<Option<SidecarMsg>, String> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(None);
    }
    serde_json::from_str(line).map(Some).map_err(|e| format!("{e}: {line:.200}"))
}

/// One host message as its line (no newline).
pub fn host_line(msg: &HostMsg) -> String {
    serde_json::to_string(msg).expect("host messages always serialise")
}

// ── the link ─────────────────────────────────────────────────────────────────────────────────────

pub enum LinkEvent {
    Msg(SidecarMsg),
    Closed(String),
}

/// The bridge's side of a sidecar. [`ProcessLink`] in a host; a test double in the tests.
pub trait SidecarLink: Send {
    fn send(&mut self, msg: &HostMsg) -> Result<(), String>;
    fn poll(&mut self) -> Option<LinkEvent>;
}

/// A sidecar process. Its stdin and stdout are serviced by their own threads, so neither a slow
/// sidecar nor a full pipe ever blocks the connection loop. The child is killed on drop.
pub struct ProcessLink {
    child: std::process::Child,
    to: mpsc::Sender<String>,
    from: mpsc::Receiver<LinkEvent>,
}

impl ProcessLink {
    pub fn spawn(command: &[String], env: &[(String, String)]) -> Result<Self, String> {
        use std::io::{BufRead, Write};
        use std::process::{Command, Stdio};
        let (program, args) = command.split_first().ok_or("no sidecar command")?;
        let mut child = Command::new(program)
            .args(args)
            .envs(env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("{program}: {e}"))?;
        let mut stdin = child.stdin.take().ok_or("sidecar stdin")?;
        let stdout = child.stdout.take().ok_or("sidecar stdout")?;
        let stderr = child.stderr.take().ok_or("sidecar stderr")?;
        let (to, lines) = mpsc::channel::<String>();
        let (tx, from) = mpsc::channel();
        let spawn = |name: &str, f: Box<dyn FnOnce() + Send>| {
            std::thread::Builder::new().name(name.into()).spawn(f).map(|_| ()).map_err(|e| e.to_string())
        };
        spawn(
            "d1-ah-sidecar-in",
            Box::new(move || {
                for line in lines {
                    if stdin.write_all(line.as_bytes()).and_then(|_| stdin.write_all(b"\n")).and_then(|_| stdin.flush()).is_err() {
                        return;
                    }
                }
            }),
        )?;
        spawn(
            "d1-ah-sidecar-out",
            Box::new(move || {
                for line in std::io::BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    match parse_sidecar_line(&line) {
                        Ok(Some(m)) => {
                            if tx.send(LinkEvent::Msg(m)).is_err() {
                                return;
                            }
                        }
                        Ok(None) => {}
                        Err(e) => tracing::warn!("sidecar sent a line the bridge cannot read: {e}"),
                    }
                }
                let _ = tx.send(LinkEvent::Closed("the sidecar closed its output".into()));
            }),
        )?;
        spawn(
            "d1-ah-sidecar-err",
            Box::new(move || {
                for line in std::io::BufReader::new(stderr).lines().map_while(Result::ok) {
                    tracing::info!("sidecar: {line}");
                }
            }),
        )?;
        Ok(Self { child, to, from })
    }
}

impl SidecarLink for ProcessLink {
    fn send(&mut self, msg: &HostMsg) -> Result<(), String> {
        self.to.send(host_line(msg)).map_err(|_| "the sidecar input thread is gone".to_string())
    }

    fn poll(&mut self) -> Option<LinkEvent> {
        match self.from.try_recv() {
            Ok(e) => Some(e),
            Err(mpsc::TryRecvError::Empty) => match self.child.try_wait() {
                Ok(Some(status)) => Some(LinkEvent::Closed(format!("the sidecar exited ({status})"))),
                _ => None,
            },
            Err(mpsc::TryRecvError::Disconnected) => Some(LinkEvent::Closed("the sidecar output thread is gone".into())),
        }
    }
}

impl Drop for ProcessLink {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ── configuration ────────────────────────────────────────────────────────────────────────────────

/// One scripted activity (a line of `D1_AH_SCRIPTS_ACTIVITIES`).
#[derive(Clone, Debug)]
pub struct ScriptedActivity {
    pub index: u16,
    /// The `@activity` name the sidecar runs, e.g. `cine_reveal`.
    pub activity: String,
    /// The activity's hash (for the rewards call), `hash=`; 0 when not given.
    pub hash: u32,
    /// The group hash the world's list is in, as for the world table.
    pub group: u32,
    /// Every row of the world's list, with its served value.
    pub rows: Vec<(RowKey, Row)>,
    pub lifetime: RowKey,
    /// The list's `@phase` (decoded value), the lifetime's value until the scripts set one.
    pub lifetime_state: i8,
    pub authority: Option<d1_oracle::sensor::SensorAuthority>,
    pub bundles_path: Option<String>,
    /// Script bundle hash → live group hash.
    pub bundle_groups: HashMap<u32, u32>,
}

impl ScriptedActivity {
    /// Parse `INDEX ACTIVITY GROUP LIST [hash=H] [bundles=PATH] [map=B:G,...] [team=0]`, building
    /// the rows from LIST as the world table does (`sensor_list::params_for_list`).
    pub fn parse(
        line: &str,
        base: &d1_oracle::sensor::SensorPhaseUpdate,
        read: &dyn Fn(&str) -> Result<String, String>,
    ) -> Result<Self, String> {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 4 {
            return Err(format!("{line:?}: want `INDEX ACTIVITY GROUP LIST [hash=H] [bundles=PATH] [map=B:G,...] [team=0]`"));
        }
        let num = |s: &str| {
            s.strip_prefix("0x").map_or_else(|| s.parse::<u32>().ok(), |h| u32::from_str_radix(h, 16).ok())
        };
        let index: u16 = f[0].parse().map_err(|_| format!("{line:?}: bad index"))?;
        let group = num(f[2]).ok_or_else(|| format!("{line:?}: bad group"))?;
        let (mut hash, mut bundles_path, mut bundle_groups, mut team) = (0, None, HashMap::new(), -1);
        for t in &f[4..] {
            if let Some(v) = t.strip_prefix("hash=") {
                hash = num(v).ok_or_else(|| format!("{line:?}: bad hash"))?;
            } else if let Some(v) = t.strip_prefix("bundles=") {
                bundles_path = Some(v.to_string());
            } else if let Some(v) = t.strip_prefix("map=") {
                for pair in v.split(',') {
                    let (b, g) = pair.split_once(':').ok_or_else(|| format!("{line:?}: bad map {pair:?}"))?;
                    bundle_groups.insert(
                        num(b).ok_or_else(|| format!("{line:?}: bad bundle {b:?}"))?,
                        num(g).ok_or_else(|| format!("{line:?}: bad group {g:?}"))?,
                    );
                }
            } else if let Some(v) = t.strip_prefix("team=") {
                team = v.parse().map_err(|_| format!("{line:?}: bad team"))?;
            } else {
                return Err(format!("{line:?}: unknown field {t:?}"));
            }
        }
        let text = read(f[3])?;
        let params = crate::sensor_list::params_for_list(base, group, team, &text).map_err(|e| format!("{}: {e:#}", f[3]))?;
        Ok(Self {
            index,
            activity: f[1].to_string(),
            hash,
            group,
            rows: params.clients.iter().map(Row::from_client).collect(),
            lifetime: RowKey { group, kind: params.name_i8, index: params.name_sub },
            lifetime_state: params.phase,
            authority: params.authority.clone(),
            bundles_path,
            bundle_groups,
        })
    }

    /// The machine the authority table gives the blocks to (block 0's peer), else 14.
    pub fn machine(&self) -> u8 {
        self.authority.as_ref().and_then(|a| a.image.get(5).copied()).unwrap_or(14)
    }

    fn group_of(&self, bundle: u32) -> u32 {
        self.bundle_groups.get(&bundle).copied().unwrap_or(bundle)
    }

    fn bundle_of(&self, group: u32) -> u32 {
        self.bundle_groups.iter().find(|(_, g)| **g == group).map_or(group, |(b, _)| *b)
    }
}

/// Builds the row codec for a bridge (on the codec's own thread).
pub type CodecMaker = Arc<dyn Fn() -> CodecWorker + Send + Sync>;

/// Everything `D1_AH_SCRIPTS` configures. Built once at startup; shared by every instance.
pub struct ScriptConfig {
    pub sidecar: Vec<String>,
    pub activities: HashMap<u16, ScriptedActivity>,
    pub gen: u8,
    pub tail: Option<(u32, u32)>,
    pub incidents: IncidentCatalog,
    pub complete_body: Option<Vec<u8>>,
    pub mission_phase: bool,
    pub send_unlisted: bool,
    pub delta_init: bool,
    pub tick: Duration,
    pub scoreboard_columns: Vec<u32>,
    pub rewards: Arc<dyn ActivityRewards>,
    pub codec: CodecMaker,
}

impl std::fmt::Debug for ScriptConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut ids: Vec<_> = self.activities.keys().collect();
        ids.sort();
        f.debug_struct("ScriptConfig").field("sidecar", &self.sidecar).field("activities", &ids).finish_non_exhaustive()
    }
}

fn hex_bytes(s: &str) -> Option<Vec<u8>> {
    (s.len() % 2 == 0).then_some(())?;
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

impl ScriptConfig {
    /// The oracle codec on its own thread.
    pub fn oracle_codec() -> CodecMaker {
        Arc::new(|| CodecWorker::spawn(Box::new(|| Ok(Box::new(OracleRowCodec::default()) as Box<dyn RowCodec>))))
    }

    /// `D1_AH_SCRIPTS=1` and its knobs (see the module doc and the note). `Ok(None)` when off.
    pub fn from_env(base: &d1_oracle::sensor::SensorPhaseUpdate) -> Result<Option<Self>, String> {
        let on = |k: &str| std::env::var(k).is_ok_and(|v| v.trim() == "1");
        if !on("D1_AH_SCRIPTS") {
            return Ok(None);
        }
        let need = |k: &str| std::env::var(k).map_err(|_| format!("D1_AH_SCRIPTS=1 needs {k}"));
        let sidecar: Vec<String> = need("D1_AH_SCRIPTS_SIDECAR")?.split_whitespace().map(String::from).collect();
        let table = need("D1_AH_SCRIPTS_ACTIVITIES")?;
        let text = std::fs::read_to_string(&table).map_err(|e| format!("D1_AH_SCRIPTS_ACTIVITIES {table}: {e}"))?;
        let read = |entry: &str| {
            let path = crate::sensor_list::listed_path(&table, entry);
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
        };
        let mut activities = HashMap::new();
        for line in text.lines().map(|l| l.split('#').next().unwrap_or("").trim()).filter(|l| !l.is_empty()) {
            let a = ScriptedActivity::parse(line, base, &read).map_err(|e| format!("D1_AH_SCRIPTS_ACTIVITIES: {e}"))?;
            activities.insert(a.index, a);
        }
        let catalogue = std::env::var("D1_AH_SCRIPTS_INCIDENTS")
            .unwrap_or_else(|_| "ps4/fixtures/56578/incidents-56578.tsv".into());
        let incidents = IncidentCatalog::load(std::path::Path::new(&catalogue)).map_err(|e| {
            format!("D1_AH_SCRIPTS_INCIDENTS: {e} (the 56578 incident catalogue, PR #306)")
        })?;
        let complete_body = match std::env::var("D1_AH_SCRIPTS_COMPLETE_BODY") {
            Ok(h) => Some(hex_bytes(h.trim()).ok_or("D1_AH_SCRIPTS_COMPLETE_BODY is not hex")?),
            Err(_) => None,
        };
        let tick = match std::env::var("D1_AH_SCRIPTS_TICK_MS") {
            Ok(v) => Duration::from_millis(v.trim().parse().map_err(|_| "D1_AH_SCRIPTS_TICK_MS is not a number")?),
            Err(_) => Duration::from_secs(1),
        };
        let scoreboard_columns = match std::env::var("D1_AH_SCRIPTS_SCOREBOARD_COLUMNS") {
            Ok(path) => std::fs::read_to_string(&path)
                .map_err(|e| format!("D1_AH_SCRIPTS_SCOREBOARD_COLUMNS {path}: {e}"))?
                .lines()
                .map(|l| l.split('#').next().unwrap_or("").trim())
                .filter(|l| !l.is_empty())
                .map(|l| u32::from_str_radix(l.trim_start_matches("0x"), 16).map_err(|_| format!("scoreboard column {l:?}")))
                .collect::<Result<_, _>>()?,
            Err(_) => Vec::new(),
        };
        Ok(Some(Self {
            sidecar,
            activities,
            gen: base.gen,
            tail: base.tail,
            incidents,
            complete_body,
            mission_phase: on("D1_AH_SCRIPTS_MISSION_PHASE"),
            send_unlisted: on("D1_AH_ROWS_UNLISTED"),
            delta_init: on("D1_AH_ROWS_DELTA_INIT"),
            tick,
            scoreboard_columns,
            rewards: Arc::new(NoRewards),
            codec: Self::oracle_codec(),
        }))
    }
}

// ── the bridge ───────────────────────────────────────────────────────────────────────────────────

/// Starts a sidecar for an activity.
pub type Launcher = Box<dyn FnMut(&ScriptedActivity) -> Result<Box<dyn SidecarLink>, String> + Send>;

/// How many times a sidecar that did not start is tried, and how long apart.
const LAUNCH_ATTEMPTS: u32 = 3;
const LAUNCH_RETRY: Duration = Duration::from_secs(2);
/// A codec failure backs frames off this long (heartbeats continue).
const CODEC_BACKOFF: Duration = Duration::from_secs(3);

struct Run {
    activity: ScriptedActivity,
    link: Option<Box<dyn SidecarLink>>,
    t0: Instant,
    attempts: u32,
    retry_at: Option<Instant>,
    ready: bool,
    started: bool,
    halted: Option<String>,
    last_tick: Option<Instant>,
    awaiting_tick: Option<(u64, Instant)>,
    lifetime_state: i8,
    mission_phase: Option<MissionPhase>,
    complete: bool,
    ended: bool,
    rewarded: bool,
    current_slice: Option<u32>,
    unsupported: std::collections::HashSet<String>,
    /// The lifetime changed and no frame has carried it yet.
    lifetime_unsent: bool,
    /// Sidecar messages waiting for the pending lifetime value to go out (see [`Self::holds`]).
    held: VecDeque<SidecarMsg>,
}

impl Run {
    /// Whether `m` must wait until the pending lifetime value has gone out: each lifetime state the
    /// scripts set reaches the client in its own frame (results 6 before ended 8, as the live hosts
    /// sent them; the post-game reads the scoreboard while the phase is below 7), and `ended` comes
    /// after the last one.
    fn holds(&self, m: &SidecarMsg) -> bool {
        if !self.lifetime_unsent {
            return false;
        }
        match m {
            SidecarMsg::Ended {} => true,
            SidecarMsg::Rows { changes } => changes.iter().any(|c| {
                c.kind == "lifetime"
                    && c.state.get("state").and_then(Value::as_i64).and_then(lifetime_wire_state)
                        .is_some_and(|s| s != self.lifetime_state)
            }),
            _ => false,
        }
    }
}

/// One instance's bridge. See the module doc.
pub struct ScriptBridge {
    config: Arc<ScriptConfig>,
    launch: Launcher,
    rows: RowTable,
    encoders: RowEncoders,
    codec: Option<CodecWorker>,
    in_flight: Option<FramePlan>,
    codec_retry: Option<Instant>,
    run: Option<Run>,
    players: Vec<CharacterRef>,
    incidents_out: VecDeque<(u32, Vec<u8>)>,
    region_out: Option<u32>,
    pub frames_sent: u64,
}

impl std::fmt::Debug for ScriptBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScriptBridge")
            .field("activity", &self.run.as_ref().map(|r| r.activity.activity.as_str()))
            .field("frames_sent", &self.frames_sent)
            .finish_non_exhaustive()
    }
}

/// A player's key for the sidecar: the first eight bytes of its identity, in hex.
pub fn player_key(c: &CharacterRef) -> String {
    c.identity.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// A squad sense observation beyond the live count: B `+0x19` (state from the squad record's
/// `+0x25C` flags: bit 2 → 3, else bit 3 → 2, else bit 1 → 1, else 0) and `+0x1A` (its bit 0),
/// reported only when the decoder saw those bytes change. Static data flow, notes/ps4-sensor-sense.md;
/// what the states mean in play is not established.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SquadSense {
    pub group: u32,
    pub index: i16,
    pub state: u8,
    pub flag: bool,
}

/// The [`SquadSense`]s a decoded udt 6 frame changed.
pub fn squad_sense(frame: &d1_oracle::sensor_sense::SenseFrame) -> Vec<SquadSense> {
    frame
        .entries
        .iter()
        .filter(|e| e.tag == crate::sense::SQUAD_B_TAG && e.name.kind == 1 && e.name.index >= 0)
        .filter(|e| e.changed_offsets.iter().any(|o| (0x19..=0x1A).contains(o)))
        .filter_map(|e| {
            Some(SquadSense {
                group: e.name.group,
                index: e.name.index,
                state: *e.image.get(0x19)?,
                flag: e.image.get(0x1A)? & 1 == 1,
            })
        })
        .collect()
}

impl ScriptBridge {
    /// A bridge whose sidecars are `config.sidecar` processes.
    pub fn new(config: Arc<ScriptConfig>) -> Self {
        let command = config.sidecar.clone();
        let launch: Launcher = Box::new(move |a: &ScriptedActivity| {
            let mut env = vec![("D1_SCRIPT_ACTIVITY".to_string(), a.activity.clone())];
            if let Some(p) = &a.bundles_path {
                env.push(("D1_SCRIPT_BUNDLES".to_string(), p.clone()));
            }
            ProcessLink::spawn(&command, &env).map(|l| Box::new(l) as Box<dyn SidecarLink>)
        });
        Self::with_launcher(config, launch)
    }

    pub fn with_launcher(config: Arc<ScriptConfig>, launch: Launcher) -> Self {
        let mut rows = RowTable::new(config.gen);
        rows.set_send_unlisted(config.send_unlisted);
        rows.set_delta_init(config.delta_init);
        let mut encoders = RowEncoders::new();
        encoders.set_scoreboard_columns(config.scoreboard_columns.clone());
        Self {
            config,
            launch,
            rows,
            encoders,
            codec: None,
            in_flight: None,
            codec_retry: None,
            run: None,
            players: Vec::new(),
            incidents_out: VecDeque::new(),
            region_out: None,
            frames_sent: 0,
        }
    }

    /// Whether the activity now served is scripted (the bridge owns its udt 5 stream).
    pub fn scripted(&self) -> bool {
        self.run.is_some()
    }

    pub fn rows(&self) -> &RowTable {
        &self.rows
    }

    /// A host incident to send (`activity_complete`).
    pub fn take_incident(&mut self) -> Option<(u32, Vec<u8>)> {
        self.incidents_out.pop_front()
    }

    /// A region the scripts' phase wants the client in (the host teleport).
    pub fn take_region_request(&mut self) -> Option<u32> {
        self.region_out.take()
    }

    // ── inputs ────────────────────────────────────────────────────────────────────────────────

    /// Every client message, after the instance has handled it (`activity_index` is the activity it
    /// then serves).
    pub fn on_client_message(&mut self, msg: &ClientMessage, activity_index: u16, now: Instant) {
        match msg {
            ClientMessage::JoinRequest { struct_image } => {
                use d1_oracle::activity_host::replicate_membership::{IDENTITY_SIZE, JOIN_REQUEST_IDENTITY};
                if let Some(id) = struct_image.get(JOIN_REQUEST_IDENTITY..JOIN_REQUEST_IDENTITY + IDENTITY_SIZE) {
                    let c = CharacterRef { identity: id.to_vec() };
                    if !self.players.contains(&c) {
                        self.players.push(c);
                    }
                }
                self.join(activity_index, now);
            }
            ClientMessage::Other { user_defined_type: 18, struct_image } => self.on_incidents(struct_image),
            ClientMessage::ClientAuthoritativeDataUpdate { struct_image } => {
                let u = |o: usize| struct_image.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
                if let (Some(index), Some(hash)) = (u(0xB8), u(0xBC)) {
                    let region = index & 0x3FF;
                    if !matches!(hash, 0 | 0x811C_9DC5) && region != 0x3FF {
                        if let Some(run) = self.run.as_mut().filter(|r| r.current_slice != Some(region)) {
                            run.current_slice = Some(region);
                            self.send(HostMsg::Slice { region });
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// A decoded udt 6 update, in the instance's current epoch.
    pub fn on_sense(&mut self, counts: &[crate::sense::SquadCount], squads: &[SquadSense]) {
        let Some(run) = self.run.as_ref() else { return };
        let item = |group: u32, index: i16| ItemRef { bundle: run.activity.bundle_of(group), type_id: 1, type_index: index };
        let mut out: Vec<HostMsg> = counts
            .iter()
            .filter(|c| c.kind == 1 && c.live >= 0)
            .map(|c| HostMsg::SquadAlive { item: item(c.group, c.index), count: c.live })
            .collect();
        out.extend(squads.iter().map(|s| HostMsg::SquadState { item: item(s.group, s.index), state: s.state, flag: s.flag }));
        for m in out {
            self.send(m);
        }
    }

    fn join(&mut self, activity_index: u16, now: Instant) {
        let Some(activity) = self.config.activities.get(&activity_index).cloned() else {
            if let Some(old) = self.run.take() {
                tracing::info!("script bridge: activity {activity_index} is not scripted; {} stops", old.activity.activity);
                self.stop_run(old, now);
            }
            return;
        };
        if let Some(run) = self.run.as_ref() {
            if run.activity.index == activity_index && run.halted.is_none() && !run.ended {
                // The same world joined again: the client set its world up again, so every row
                // goes out once more, with its current value.
                tracing::info!("script bridge: {} joined again; the full list goes out again", activity.activity);
                self.rows.request_full();
                return;
            }
        }
        if let Some(old) = self.run.take() {
            self.stop_run(old, now);
        }
        let mut rows = activity.rows.clone();
        rows.push((activity.lifetime, self.lifetime_row(activity.lifetime_state, None)));
        self.rows.load_world(rows, activity.authority.clone());
        self.encoders.reset();
        self.in_flight = None;
        tracing::info!(
            "⭐ script bridge: activity {activity_index} is {} ({} rows); starting its sidecar",
            activity.activity,
            activity.rows.len() + 1
        );
        self.run = Some(Run {
            lifetime_state: activity.lifetime_state,
            activity,
            link: None,
            t0: now,
            attempts: 0,
            retry_at: Some(now),
            ready: false,
            started: false,
            halted: None,
            last_tick: None,
            awaiting_tick: None,
            mission_phase: None,
            complete: false,
            ended: false,
            rewarded: false,
            current_slice: None,
            unsupported: Default::default(),
            lifetime_unsent: false,
            held: VecDeque::new(),
        });
    }

    fn stop_run(&mut self, mut run: Run, _now: Instant) {
        if let Some(link) = run.link.as_mut() {
            let _ = link.send(&HostMsg::Stop);
        }
        if run.started && !run.ended {
            self.reward(&mut run, ActivityOutcome::Abandoned);
        }
    }

    fn on_incidents(&mut self, image: &[u8]) {
        if self.run.is_none() {
            return;
        }
        let Some(n) = image.get(0..4).map(|b| u32::from_le_bytes(b.try_into().unwrap())) else { return };
        for k in 0..n.min(8) as usize {
            let Some(t) = image.get(4 + 4 * k..8 + 4 * k).map(|b| u32::from_le_bytes(b.try_into().unwrap())) else { break };
            let msg = self.translate_incident(t, image);
            if let Some(m) = msg {
                self.send(m);
            }
        }
    }

    fn translate_incident(&self, t: u32, image: &[u8]) -> Option<HostMsg> {
        let run = self.run.as_ref()?;
        let item = |(group, kind, index): (u32, u8, u16)| ItemRef {
            bundle: run.activity.bundle_of(group),
            type_id: kind,
            type_index: index as i16,
        };
        let key = self.players.first().map(player_key).unwrap_or_default();
        let named = crate::state::trigger_report(image);
        Some(match t {
            _ if t == known::PLAYER_TRIGGER.index => match named {
                Some(n) => HostMsg::Trigger { item: item(n) },
                None => {
                    tracing::warn!("script bridge: incident 3403 names no trigger; not passed on");
                    return None;
                }
            },
            _ if t == known::CINEMATIC_FINISHED.index || t == known::CINEMATIC_SKIP.index => {
                let row = match named.filter(|n| n.1 == crate::row_encoders::KIND_CINEMATIC) {
                    Some(n) => item(n),
                    None => match self.encoders.playing_cinematics().as_slice() {
                        [only] => item((only.group, only.kind, only.index as u16)),
                        playing => {
                            tracing::warn!(
                                "script bridge: incident {t} names no cinematic and {} are playing; not passed on",
                                playing.len()
                            );
                            return None;
                        }
                    },
                };
                if t == known::CINEMATIC_FINISHED.index {
                    HostMsg::CinematicFinished { item: row }
                } else {
                    HostMsg::CinematicSkipped { item: row, key }
                }
            }
            _ if t == known::PLAYER_SPAWNED.index || t == known::PLAYER_SPAWNED_INITIAL.index => HostMsg::PlayerSpawned { key },
            _ if t == known::PLAYER_DIED.index => HostMsg::PlayerDied { key },
            _ => match self.config.incidents.name_hash(t) {
                Some(name_hash) => HostMsg::Incident { name_hash, index: t },
                None => {
                    tracing::warn!("script bridge: incident {t} is not in the 56578 catalogue; not passed on");
                    return None;
                }
            },
        })
    }

    // ── the sidecar ───────────────────────────────────────────────────────────────────────────

    fn send(&mut self, msg: HostMsg) {
        let Some(run) = self.run.as_mut() else { return };
        if run.halted.is_some() {
            return;
        }
        let Some(link) = run.link.as_mut() else { return };
        if let Err(e) = link.send(&msg) {
            Self::halt(run, format!("send failed: {e}"));
        }
    }

    fn halt(run: &mut Run, reason: String) {
        if run.halted.is_none() {
            tracing::error!(
                "** NAMED GATE ** script bridge: {} stopped ({reason}). Progression stops here; the rows \
                 already sent stay and heartbeats continue",
                run.activity.activity
            );
            run.halted = Some(reason);
            run.link = None;
        }
    }

    fn drive(&mut self, now: Instant) {
        let Some(run) = self.run.as_mut() else { return };
        if run.halted.is_some() {
            return;
        }
        // Launch (and retry a sidecar that did not start).
        if run.link.is_none() && run.retry_at.is_some_and(|t| now >= t) {
            run.attempts += 1;
            match (self.launch)(&run.activity) {
                Ok(mut link) => {
                    let hello = HostMsg::Hello {
                        proto: PROTO,
                        build: "56578".into(),
                        activity: run.activity.activity.clone(),
                        activity_index: run.activity.index,
                        bundles_path: run.activity.bundles_path.clone(),
                        tick_hz: TICK_HZ,
                        script_tick_s: self.config.tick.as_secs_f32(),
                    };
                    match link.send(&hello) {
                        Ok(()) => {
                            run.link = Some(link);
                            run.retry_at = None;
                        }
                        Err(e) => run.retry_at = Self::retry(run, now, format!("hello: {e}")),
                    }
                }
                Err(e) => run.retry_at = Self::retry(run, now, e),
            }
        }
        // Messages held for the lifetime, in order, once it has gone out.
        while let Some(run) = self.run.as_mut() {
            match run.held.pop_front() {
                Some(m) if run.holds(&m) => {
                    run.held.push_front(m);
                    break;
                }
                Some(m) => self.apply(m, now),
                None => break,
            }
        }
        // Events from the sidecar.
        loop {
            let Some(run) = self.run.as_mut() else { return };
            let Some(link) = run.link.as_mut() else { break };
            match link.poll() {
                None => break,
                Some(LinkEvent::Msg(m)) => {
                    let run = self.run.as_mut().expect("checked");
                    if !run.held.is_empty() || run.holds(&m) {
                        run.held.push_back(m);
                    } else {
                        self.apply(m, now);
                    }
                }
                Some(LinkEvent::Closed(reason)) => {
                    let run = self.run.as_mut().expect("checked");
                    run.link = None;
                    if !run.started && run.attempts < LAUNCH_ATTEMPTS {
                        run.ready = false;
                        run.retry_at = Self::retry(run, now, reason);
                    } else if run.ended {
                        tracing::info!("script bridge: {} sidecar finished", run.activity.activity);
                    } else {
                        Self::halt(run, reason);
                        let mut run = self.run.take().expect("checked");
                        if run.started {
                            self.reward(&mut run, ActivityOutcome::Abandoned);
                        }
                        self.run = Some(run);
                    }
                    break;
                }
            }
        }
        let Some(run) = self.run.as_mut() else { return };
        if run.halted.is_some() || run.link.is_none() {
            return;
        }
        // Start once the sidecar is ready and the world's full list has gone out.
        if !run.started && run.ready && self.rows.full_delivered() {
            run.started = true;
            run.last_tick = Some(now);
            tracing::info!("⭐ script bridge: {} scripts start", run.activity.activity);
            let players = self
                .players
                .iter()
                .enumerate()
                .map(|(slot, c)| PlayerInfo { key: player_key(c), slot: slot as u32 })
                .collect();
            self.send(HostMsg::Players { players });
            self.send(HostMsg::Start);
            return;
        }
        // The script clock.
        let run = self.run.as_mut().expect("checked");
        if run.started && !run.ended && run.last_tick.is_some_and(|t| now.saturating_duration_since(t) >= self.config.tick) {
            if let Some((n, at)) = run.awaiting_tick {
                if now.saturating_duration_since(at) < Duration::from_secs(10) {
                    return;
                }
                tracing::warn!("script bridge: no tick_done for tick {n} in 10 s; ticking on");
            }
            let ticks = (now.saturating_duration_since(run.t0).as_millis() * u128::from(TICK_HZ) / 1000) as u64;
            run.last_tick = Some(now);
            run.awaiting_tick = Some((ticks, now));
            self.send(HostMsg::Tick { now: ticks });
        }
    }

    fn retry(run: &mut Run, now: Instant, reason: String) -> Option<Instant> {
        if run.attempts >= LAUNCH_ATTEMPTS {
            Self::halt(run, format!("did not start after {} attempts: {reason}", run.attempts));
            None
        } else {
            tracing::warn!("script bridge: sidecar for {} did not start ({reason}); retrying", run.activity.activity);
            Some(now + LAUNCH_RETRY)
        }
    }

    fn lifetime_row(&self, state: i8, phase: Option<MissionPhase>) -> Row {
        Row {
            has_a: true,
            has_b: false,
            a: Some((TAG_LIFETIME, lifetime_image(state, self.config.tail, if self.config.mission_phase { phase } else { None }))),
            b: None,
        }
    }

    fn set_lifetime(&mut self) {
        let Some(run) = self.run.as_ref() else { return };
        let (key, row) = (run.activity.lifetime, self.lifetime_row(run.lifetime_state, run.mission_phase));
        self.rows.set(key, row);
    }

    fn apply(&mut self, msg: SidecarMsg, now: Instant) {
        let Some(run) = self.run.as_mut() else { return };
        match msg {
            SidecarMsg::Ready { unresolved, drives_lifetime } => {
                run.ready = true;
                tracing::info!(
                    "script bridge: {} sidecar ready ({} unresolved bindings{})",
                    run.activity.activity,
                    unresolved.len(),
                    if drives_lifetime { "" } else { "; the scripts do not drive the lifetime" }
                );
            }
            SidecarMsg::Rows { changes } => {
                for c in changes {
                    self.apply_change(c, now);
                }
            }
            SidecarMsg::RowsCleared { items } => {
                let groups: Vec<RowKey> = items
                    .iter()
                    .map(|i| RowKey { group: run.activity.group_of(i.bundle), kind: i.type_id, index: i.type_index })
                    .collect();
                for k in groups {
                    self.rows.reset(k);
                }
            }
            SidecarMsg::Phase { index, name, region } => {
                run.mission_phase = Some(MissionPhase {
                    index: index as i32,
                    name_hash: crate::incident_catalog::fnv1(name.as_deref().unwrap_or("")),
                    bubble: region.map_or(-1, |r| (r >> 3) as i32),
                });
                tracing::info!(
                    "⭐ script bridge: {} mission phase {index} {:?} (region {region:?})",
                    run.activity.activity,
                    name.as_deref().unwrap_or("")
                );
                if let Some(r) = region.filter(|r| run.current_slice != Some(*r)) {
                    self.region_out = Some(r);
                }
                self.set_lifetime();
            }
            SidecarMsg::ActivityFlag { hash, active } => {
                tracing::warn!("script bridge: activity flag {hash:#010x} = {active} has no 56578 encoding yet; not sent");
            }
            SidecarMsg::ActivityComplete {} => {
                run.complete = true;
                let k = known::ACTIVITY_COMPLETE;
                match &self.config.complete_body {
                    Some(body) => {
                        tracing::info!("⭐ script bridge: activity complete; host incident {} {}", k.index, k.name);
                        self.incidents_out.push_back((k.index, body.clone()));
                    }
                    None => tracing::warn!(
                        "** NAMED GATE ** script bridge: the scripts completed {}, but incident {} \
                         ({}) is not sent: its 56578 body is unknown (D1_AH_SCRIPTS_COMPLETE_BODY)",
                        run.activity.activity, k.index, k.name
                    ),
                }
            }
            SidecarMsg::Ended {} => {
                run.ended = true;
                let outcome = if run.complete { ActivityOutcome::Completed } else { ActivityOutcome::EndedIncomplete };
                tracing::info!("⭐ script bridge: {} ended ({outcome:?})", run.activity.activity);
                let mut run = self.run.take().expect("checked");
                self.reward(&mut run, outcome);
                self.run = Some(run);
            }
            SidecarMsg::HardWipeArmed {} | SidecarMsg::AllDead {} => {
                tracing::warn!("script bridge: the scripts asked for a hard wipe, which has no 56578 handshake yet");
            }
            SidecarMsg::TickDone { now: n } => {
                if run.awaiting_tick.is_some_and(|(t, _)| t == n) {
                    run.awaiting_tick = None;
                }
            }
            SidecarMsg::Log { text } => tracing::info!("sidecar: {text}"),
            SidecarMsg::Error { text } => tracing::warn!("sidecar error: {text}"),
        }
    }

    fn apply_change(&mut self, c: ItemChange, now: Instant) {
        let Some(run) = self.run.as_mut() else { return };
        if c.kind == "lifetime" {
            let state = c.state.get("state").and_then(Value::as_i64).unwrap_or(-1);
            match lifetime_wire_state(state) {
                Some(s) => {
                    if s != run.lifetime_state {
                        tracing::info!("⭐ script bridge: lifetime {} -> {s} (script state {state})", run.lifetime_state);
                    }
                    if s != run.lifetime_state {
                        run.lifetime_unsent = true;
                    }
                    run.lifetime_state = s;
                    self.set_lifetime();
                }
                None => tracing::warn!("script bridge: lifetime state {state} has no 56578 value; not sent"),
            }
            return;
        }
        let group = run.activity.group_of(c.item.bundle);
        let key = RowKey { group, kind: c.item.type_id, index: c.item.type_index };
        let groups = run.activity.bundle_groups.clone();
        let map = move |v: &Value| -> Option<MappedRef> {
            let r: ItemRef = serde_json::from_value(v.clone()).ok()?;
            let group = groups.get(&r.bundle).copied().unwrap_or(r.bundle);
            Some(MappedRef { group, kind: r.type_id, index: r.type_index })
        };
        let ctx = EncodeContext { stamp_tick: self.rows.tick(now), machine: run.activity.machine() };
        let base = self.rows.get(&key).cloned();
        match self.encoders.encode(key, &c.kind, &c.state, base.as_ref(), ctx, &map) {
            Encoded::Row(row) => {
                self.rows.set(key, row);
            }
            Encoded::Unsupported(why) => {
                let run = self.run.as_mut().expect("checked");
                if run.unsupported.insert(c.kind.clone()) {
                    tracing::warn!("script bridge: {} row {key} not sent: {why}", c.kind);
                }
            }
        }
    }

    fn reward(&mut self, run: &mut Run, outcome: ActivityOutcome) {
        if run.rewarded {
            return;
        }
        run.rewarded = true;
        for c in &self.players {
            if let Err(e) = self.config.rewards.grant_activity_rewards(c, run.activity.hash, outcome) {
                tracing::warn!("rewards for {c:?}: {e}");
            }
        }
    }

    // ── udt 5 ─────────────────────────────────────────────────────────────────────────────────

    /// The next udt 5 frame for a scripted activity, if one is due: the full list, a delta, or a
    /// heartbeat. Also runs the sidecar (events in, the clock). Call it on every poll.
    pub fn sensor_due(&mut self, now: Instant) -> Option<Vec<u8>> {
        self.run.as_ref()?;
        self.drive(now);
        if let Some(plan) = self.in_flight.take() {
            match self.codec.as_mut().and_then(CodecWorker::poll) {
                None => self.in_flight = Some(plan),
                Some(r) => {
                    if let Some(frame) = self.finish(plan, r, now) {
                        return Some(frame);
                    }
                }
            }
        }
        let backing_off = self.codec_retry.is_some_and(|t| now < t);
        if self.in_flight.is_none() && !backing_off {
            if let Some(plan) = self.rows.plan() {
                let codec = self.codec.get_or_insert_with(|| (self.config.codec)());
                codec.submit(&plan);
                match codec.poll() {
                    None => self.in_flight = Some(plan),
                    Some(r) => {
                        if let Some(frame) = self.finish(plan, r, now) {
                            return Some(frame);
                        }
                    }
                }
            }
        }
        self.rows.heartbeat_due(now)
    }

    fn finish(&mut self, plan: FramePlan, result: Result<crate::row_encoders::EncodedFrame, String>, now: Instant) -> Option<Vec<u8>> {
        let failed = |this: &mut Self, plan: FramePlan, e: String| {
            tracing::warn!("udt 5 {} not sent: {e}", if plan.full { "full list" } else { "delta" });
            this.rows.failed(plan);
            this.codec_retry = Some(now + CODEC_BACKOFF);
            None
        };
        match result {
            Ok(mut f) => match self.rows.stamp(&mut f.bytes, f.stamp_bit, plan.init, now) {
                Ok(()) => {
                    self.rows.sent(&plan, now);
                    if let Some(run) = self.run.as_mut() {
                        if plan.entries.iter().any(|(k, _)| *k == run.activity.lifetime) {
                            run.lifetime_unsent = false;
                        }
                    }
                    self.codec_retry = None;
                    self.frames_sent += 1;
                    tracing::info!(
                        "udt 5 {} ({} rows, stamp {})",
                        if plan.full { "full list, init" } else { "delta" },
                        plan.entries.len(),
                        self.rows.tick(now)
                    );
                    Some(f.bytes)
                }
                Err(e) => failed(self, plan, e),
            },
            Err(e) => failed(self, plan, e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::row_encoders::EncodedFrame;
    use std::sync::Mutex;

    // ── the protocol ──

    #[test]
    fn host_messages_are_the_documented_json_lines() {
        let item = ItemRef { bundle: 0x2FE7_DBE6, type_id: 6, type_index: 1 };
        assert_eq!(host_line(&HostMsg::Start), r#"{"type":"start"}"#);
        assert_eq!(host_line(&HostMsg::Tick { now: 30 }), r#"{"type":"tick","now":30}"#);
        assert_eq!(
            host_line(&HostMsg::CinematicFinished { item }),
            r#"{"type":"cinematic_finished","item":{"bundle":803724262,"typeId":6,"typeIndex":1}}"#
        );
        assert_eq!(
            host_line(&HostMsg::Incident { name_hash: 0xBD1F_B418, index: 2899 }),
            r#"{"type":"incident","name_hash":3172971544,"index":2899}"#
        );
        let hello = host_line(&HostMsg::Hello {
            proto: 1, build: "56578".into(), activity: "cine_reveal".into(), activity_index: 165,
            bundles_path: None, tick_hz: 30, script_tick_s: 1.0,
        });
        assert_eq!(hello, r#"{"type":"hello","proto":1,"build":"56578","activity":"cine_reveal","activity_index":165,"tick_hz":30,"script_tick_s":1.0}"#);
    }

    #[test]
    fn sidecar_lines_parse_bullshcript_item_changes() {
        let rows = parse_sidecar_line(
            r#"{"type":"rows","changes":[{"item":{"bundle":1,"typeId":6,"typeIndex":0},"kind":"cinematic","state":{"playing":{"startedAt":9}},"initial":true}]}"#,
        )
        .unwrap()
        .unwrap();
        let SidecarMsg::Rows { changes } = rows else { panic!() };
        assert_eq!(changes[0].item, ItemRef { bundle: 1, type_id: 6, type_index: 0 });
        assert!(changes[0].initial && changes[0].kind == "cinematic");
        assert_eq!(parse_sidecar_line(r#"{"type":"activity_complete"}"#).unwrap(), Some(SidecarMsg::ActivityComplete {}));
        assert_eq!(parse_sidecar_line(r#"{"type":"ended","extra":1}"#).unwrap(), Some(SidecarMsg::Ended {}));
        assert_eq!(
            parse_sidecar_line(r#"{"type":"phase","index":1,"name":"cine_107_twr_phase","region":26}"#).unwrap(),
            Some(SidecarMsg::Phase { index: 1, name: Some("cine_107_twr_phase".into()), region: Some(26) })
        );
        assert_eq!(parse_sidecar_line("  ").unwrap(), None);
        assert!(parse_sidecar_line(r#"{"type":"nonsense"}"#).is_err());
        assert!(parse_sidecar_line("not json").is_err());
    }

    // ── a fake sidecar running a synthetic two-cinematic activity ──

    const GROUP: u32 = 0x4786_C0E0;
    const CINE: u32 = 0x2FE7_DBE6;

    #[derive(Default)]
    struct Shared {
        sent: Vec<HostMsg>,
        frames: Vec<FramePlan>,
        rewards: Vec<(CharacterRef, u32, ActivityOutcome)>,
        launches: u32,
    }

    /// The synthetic script, in the shape of the reveal: pregame, cinematic 0 at the cinematic
    /// hold, phase 1 in region 26, complete, cinematic 1, results, ended.
    struct FakeSidecar {
        shared: Arc<Mutex<Shared>>,
        out: VecDeque<LinkEvent>,
        stage: u32,
        die_after_start: bool,
    }

    fn change(kind: &str, index: i16, type_id: u8, state: Value) -> ItemChange {
        ItemChange { item: ItemRef { bundle: CINE, type_id, type_index: index }, kind: kind.into(), state, initial: false }
    }

    fn lifetime(state: i64) -> ItemChange {
        ItemChange { item: ItemRef { bundle: 0x1234, type_id: 16, type_index: 0 }, kind: "lifetime".into(), state: serde_json::json!({"joinFlag": true, "state": state}), initial: false }
    }

    fn playing(index: i16, on: bool) -> ItemChange {
        let state = if on { serde_json::json!({"playing": {"startedAt": 99}}) } else { serde_json::json!({}) };
        change("cinematic", index, 6, state)
    }

    impl SidecarLink for FakeSidecar {
        fn send(&mut self, msg: &HostMsg) -> Result<(), String> {
            self.shared.lock().unwrap().sent.push(msg.clone());
            let mut reply = |m: SidecarMsg| self.out.push_back(LinkEvent::Msg(m));
            match msg {
                HostMsg::Hello { .. } => reply(SidecarMsg::Ready { unresolved: Vec::new(), drives_lifetime: true }),
                HostMsg::Start => {
                    if self.die_after_start {
                        self.out.push_back(LinkEvent::Closed("crashed".into()));
                        return Ok(());
                    }
                    reply(SidecarMsg::Rows { changes: vec![lifetime(0), playing(0, false), playing(1, false)] });
                    reply(SidecarMsg::Phase { index: 0, name: Some("cine_106_trv_phase".into()), region: Some(25) });
                }
                HostMsg::Tick { now } => {
                    if self.stage == 0 && *now >= 60 {
                        self.stage = 1;
                        reply(SidecarMsg::Rows { changes: vec![lifetime(3), playing(0, true)] });
                    }
                    reply(SidecarMsg::TickDone { now: *now });
                }
                HostMsg::CinematicFinished { item } if self.stage == 1 && item.type_index == 0 => {
                    self.stage = 2;
                    reply(SidecarMsg::Rows { changes: vec![playing(0, false)] });
                    reply(SidecarMsg::Phase { index: 1, name: Some("cine_107_twr_phase".into()), region: Some(26) });
                }
                HostMsg::Slice { region: 26 } if self.stage == 2 => {
                    self.stage = 3;
                    reply(SidecarMsg::ActivityComplete {});
                    reply(SidecarMsg::Rows { changes: vec![playing(1, true)] });
                }
                HostMsg::CinematicFinished { item } if self.stage == 3 && item.type_index == 1 => {
                    self.stage = 4;
                    reply(SidecarMsg::Rows { changes: vec![playing(1, false), lifetime(5)] });
                    reply(SidecarMsg::Rows { changes: vec![lifetime(6)] });
                    reply(SidecarMsg::Ended {});
                }
                _ => {}
            }
            Ok(())
        }

        fn poll(&mut self) -> Option<LinkEvent> {
            self.out.pop_front()
        }
    }

    /// Records each plan; the bytes are a heartbeat-shaped header (stamp at bit 9) plus a count.
    struct RecordingCodec(Arc<Mutex<Shared>>);
    impl RowCodec for RecordingCodec {
        fn name(&self) -> &'static str {
            "recording"
        }
        fn encode_frame(&mut self, plan: &FramePlan) -> Result<EncodedFrame, String> {
            self.0.lock().unwrap().frames.push(plan.clone());
            let mut bytes = d1_oracle::sensor::heartbeat_frame(plan.gen, 0);
            bytes.push(plan.entries.len() as u8);
            Ok(EncodedFrame { bytes, stamp_bit: 9 })
        }
    }

    struct RecordingRewards(Arc<Mutex<Shared>>);
    impl ActivityRewards for RecordingRewards {
        fn grant_activity_rewards(&self, c: &CharacterRef, hash: u32, outcome: ActivityOutcome) -> Result<(), String> {
            self.0.lock().unwrap().rewards.push((c.clone(), hash, outcome));
            Ok(())
        }
    }

    fn base() -> d1_oracle::sensor::SensorPhaseUpdate {
        d1_oracle::sensor::SensorPhaseUpdate {
            gen: 0, name_hash: GROUP, name_i8: 0x10, name_sub: 3, phase: 0, tail: None,
            clients: Vec::new(), authority: None,
        }
    }

    const LIST: &str = "@authority 13
@phase 0
0x2fe7dbe6 0x6 0 1 0
0x2fe7dbe6 0x6 1 1 0
0x4786c0e0 0xf 0 1 0
";

    fn bridge(shared: &Arc<Mutex<Shared>>, die_after_start: bool, complete_body: Option<Vec<u8>>) -> ScriptBridge {
        let activity = ScriptedActivity::parse(
            "165 cine_reveal 0x4786c0e0 list.txt hash=0x0BBD0E26",
            &base(),
            &|_| Ok(LIST.to_string()),
        )
        .unwrap();
        assert_eq!(activity.lifetime, RowKey { group: GROUP, kind: 0x10, index: 3 });
        assert_eq!(activity.machine(), 13);
        let s = shared.clone();
        let config = ScriptConfig {
            sidecar: vec!["fake".into()],
            activities: HashMap::from([(165, activity)]),
            gen: 0,
            tail: None,
            incidents: crate::incident_catalog::tests::synthetic(),
            complete_body,
            mission_phase: true,
            send_unlisted: false,
            delta_init: false,
            tick: Duration::from_secs(1),
            scoreboard_columns: Vec::new(),
            rewards: Arc::new(RecordingRewards(shared.clone())),
            codec: Arc::new(move || CodecWorker::inline(Box::new(RecordingCodec(s.clone())))),
        };
        let s = shared.clone();
        ScriptBridge::with_launcher(
            Arc::new(config),
            Box::new(move |_| {
                s.lock().unwrap().launches += 1;
                Ok(Box::new(FakeSidecar { shared: s.clone(), out: VecDeque::new(), stage: 0, die_after_start }) as Box<dyn SidecarLink>)
            }),
        )
    }

    fn join(identity: u8) -> ClientMessage {
        let mut struct_image = vec![0u8; 0x200];
        struct_image[0x10..0x38].fill(identity);
        ClientMessage::JoinRequest { struct_image }
    }

    /// A udt 18 struct image reporting `t`, naming a row at bit 303 as 3403 does.
    fn incident(t: u32, row: Option<(u32, u8, i16)>) -> ClientMessage {
        let mut body = vec![0u8; 64];
        if let Some((group, kind, index)) = row {
            let mut put = |at: usize, n: usize, v: u64| {
                for i in 0..n {
                    if (v >> (n - 1 - i)) & 1 == 1 {
                        body[(at + i) / 8] |= 0x80 >> ((at + i) % 8);
                    }
                }
            };
            put(303, 32, u64::from(group));
            put(335, 7, u64::from(kind) + 1);
            put(342, 16, (i32::from(index) + 0x8000) as u64);
        }
        ClientMessage::Other { user_defined_type: 18, struct_image: crate::state::incident_image(t, &body) }
    }

    fn slice(region: u32) -> ClientMessage {
        let mut struct_image = vec![0u8; 0x220];
        struct_image[0xB8..0xBC].copy_from_slice(&region.to_le_bytes());
        struct_image[0xBC..0xC0].copy_from_slice(&0x1234_5678u32.to_le_bytes());
        ClientMessage::ClientAuthoritativeDataUpdate { struct_image }
    }

    fn lifetime_of(plan: &FramePlan) -> Option<i8> {
        plan.entries.iter().find(|(k, _)| k.kind == 0x10).and_then(|(_, r)| r.a.as_ref()).map(|(_, i)| i[0] as i8)
    }

    fn cinematic_of(plan: &FramePlan, index: i16) -> Option<(u32, u32, u8)> {
        let (_, r) = plan.entries.iter().find(|(k, _)| k.kind == 6 && k.index == index)?;
        let i = &r.a.as_ref()?.1;
        Some((u32_at(i, 0), u32_at(i, 4), i[8]))
    }

    fn u32_at(b: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
    }

    /// Drive the bridge as the instance does, for `secs` seconds in 250 ms polls.
    fn run_for(b: &mut ScriptBridge, t: &mut Instant, secs: f32, frames: &mut Vec<Vec<u8>>) {
        let end = *t + Duration::from_secs_f32(secs);
        while *t < end {
            *t += Duration::from_millis(250);
            if let Some(f) = b.sensor_due(*t) {
                frames.push(f);
            }
        }
    }

    #[test]
    fn a_fake_sidecar_drives_a_whole_synthetic_activity() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge(&shared, false, Some(vec![0xAA, 0xBB]));
        let mut t = Instant::now();
        let mut wire = Vec::new();
        assert!(b.sensor_due(t).is_none(), "nothing before a join");
        b.on_client_message(&join(7), 99, t);
        assert!(!b.scripted(), "an activity the table does not name stays unscripted");
        b.on_client_message(&join(7), 165, t);
        assert!(b.scripted());

        // The join: the full list (every row, init, authority) goes out first; then start.
        run_for(&mut b, &mut t, 0.25, &mut wire);
        {
            let s = shared.lock().unwrap();
            let full = &s.frames[0];
            assert!(full.full && full.init && full.authority.is_some());
            assert_eq!(full.entries.len(), 4, "three listed rows and the lifetime");
            assert_eq!(lifetime_of(full), Some(0), "the list's @phase until the scripts set one");
            assert!(matches!(s.sent[0], HostMsg::Hello { activity_index: 165, .. }));
        }
        run_for(&mut b, &mut t, 0.5, &mut wire);
        {
            let s = shared.lock().unwrap();
            assert!(s.sent.iter().any(|m| matches!(m, HostMsg::Start)), "start after the full list");
            let pregame = s.frames.last().unwrap();
            assert!(!pregame.full && !pregame.init, "a delta");
            assert_eq!(lifetime_of(pregame), Some(1), "pregame -> 1");
            assert_eq!(pregame.entries.len(), 3, "the lifetime and the two cinematics it set");
        }
        assert_eq!(b.take_region_request(), Some(25), "phase 0 is in region 25, the client is not there yet");
        b.on_client_message(&slice(25), 165, t);

        // Ticks: at 2 s of activity time (tick 60) the first cinematic plays at the hold.
        run_for(&mut b, &mut t, 3.0, &mut wire);
        let play_stamp;
        {
            let s = shared.lock().unwrap();
            assert!(s.sent.iter().any(|m| matches!(m, HostMsg::Tick { now } if *now >= 60)));
            let play = s.frames.iter().find(|p| lifetime_of(p) == Some(4)).expect("the cinematic hold");
            let (start, counter, on) = cinematic_of(play, 0).expect("cinematic 0 in the same delta");
            assert_eq!((counter, on), (2, 1), "a new counter, playing");
            assert!(start > 0);
            play_stamp = start + 1;
        }
        assert!(b.rows().tick(t) >= play_stamp);

        // Idle: heartbeats every 3 s.
        let before = b.rows().heartbeats;
        run_for(&mut b, &mut t, 6.5, &mut wire);
        assert!(b.rows().heartbeats >= before + 2);
        assert!(wire.iter().any(|f| f.len() == 6), "heartbeats are the 6-byte live idle frame");

        // The client reports the first cinematic finished, naming its row.
        b.on_client_message(&incident(891, Some((CINE, 6, 0))), 165, t);
        run_for(&mut b, &mut t, 0.5, &mut wire);
        assert_eq!(b.take_region_request(), Some(26), "phase 1 wants region 26");
        b.on_client_message(&slice(26), 165, t);
        run_for(&mut b, &mut t, 0.5, &mut wire);
        assert_eq!(b.take_incident(), Some((965, vec![0xAA, 0xBB])), "activity_complete, with the configured body");
        assert_eq!(b.take_incident(), None);
        {
            let s = shared.lock().unwrap();
            let phase1 = s.frames.iter().rev().find(|p| lifetime_of(p).is_some()).unwrap();
            let image = &phase1.entries.iter().find(|(k, _)| k.kind == 0x10).unwrap().1.a.as_ref().unwrap().1;
            assert_eq!(u32_at(image, 4), 1, "mission phase index 1 (D1_AH_SCRIPTS_MISSION_PHASE)");
            assert_eq!(u32_at(image, 8), crate::incident_catalog::fnv1("cine_107_twr_phase"));
            assert_eq!(u32_at(image, 0xC), 3, "bubble 26 >> 3");
            let last = s.frames.last().unwrap();
            assert_eq!(cinematic_of(last, 1).map(|c| (c.1, c.2)), Some((2, 1)), "cinematic 1 plays");
        }

        // The second finish names no row: the one cinematic playing is it.
        b.on_client_message(&incident(891, None), 165, t);
        run_for(&mut b, &mut t, 1.0, &mut wire);
        {
            let s = shared.lock().unwrap();
            assert!(s.sent.iter().any(|m| matches!(m, HostMsg::CinematicFinished { item } if item.type_index == 1)));
            let states: Vec<i8> = s.frames.iter().filter_map(lifetime_of).collect();
            let mut seq = states.clone();
            seq.dedup();
            assert_eq!(seq, [0, 1, 4, 6, 8], "the live order: 0 -> 1 -> 4 -> 6 -> 8");
            assert_eq!(s.rewards.len(), 1, "rewards once, for the one character");
            assert_eq!((s.rewards[0].0.identity[0], s.rewards[0].1, s.rewards[0].2), (7, 0x0BBD_0E26, ActivityOutcome::Completed));
            let ticks = s.sent.iter().filter(|m| matches!(m, HostMsg::Tick { .. })).count();
            drop(s);
            run_for(&mut b, &mut t, 3.0, &mut wire);
            let after = shared.lock().unwrap().sent.iter().filter(|m| matches!(m, HostMsg::Tick { .. })).count();
            assert_eq!(after, ticks, "no ticks after the end");
        }
        // Every frame the bridge returned is stamped (the 10 Hz clock, nondecreasing).
        let stamps: Vec<u32> = wire.iter().map(|f| ((u64::from_be_bytes([0, 0, 0, f[1], f[2], f[3], f[4], f[5]]) >> 7) & 0xFFFF_FFFF) as u32).collect();
        assert!(stamps.windows(2).all(|w| w[0] <= w[1]), "{stamps:?}");
    }

    #[test]
    fn incidents_translate_through_the_catalogue() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge(&shared, false, None);
        let mut t = Instant::now();
        b.on_client_message(&join(1), 165, t);
        run_for(&mut b, &mut t, 1.0, &mut Vec::new());
        shared.lock().unwrap().sent.clear();
        b.on_client_message(&incident(3403, Some((0x992B_B471, 0x1D, 34))), 165, t);
        b.on_client_message(&incident(566, None), 165, t);
        b.on_client_message(&incident(2079, None), 165, t);
        b.on_client_message(&incident(2899, None), 165, t);
        b.on_client_message(&incident(1726, Some((CINE, 6, 1))), 165, t);
        b.on_client_message(&incident(891, None), 165, t);
        b.on_client_message(&incident(77777, None), 165, t);
        let s = shared.lock().unwrap();
        let key = "0101010101010101".to_string();
        assert_eq!(
            s.sent,
            vec![
                HostMsg::Trigger { item: ItemRef { bundle: 0x992B_B471, type_id: 0x1D, type_index: 34 } },
                HostMsg::PlayerSpawned { key: key.clone() },
                HostMsg::PlayerDied { key: key.clone() },
                HostMsg::Incident { name_hash: 0xBD1F_B418, index: 2899 },
                HostMsg::CinematicSkipped { item: ItemRef { bundle: CINE, type_id: 6, type_index: 1 }, key },
            ],
            "891 with no row and no cinematic playing, and an id outside the catalogue, are not passed on"
        );
    }

    #[test]
    fn activity_complete_without_a_known_body_is_not_sent() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge(&shared, false, None);
        let mut t = Instant::now();
        b.on_client_message(&join(1), 165, t);
        b.on_client_message(&slice(25), 165, t);
        run_for(&mut b, &mut t, 3.5, &mut Vec::new());
        b.on_client_message(&incident(891, Some((CINE, 6, 0))), 165, t);
        run_for(&mut b, &mut t, 0.5, &mut Vec::new());
        b.on_client_message(&slice(26), 165, t);
        run_for(&mut b, &mut t, 0.5, &mut Vec::new());
        assert_eq!(b.take_incident(), None);
    }

    #[test]
    fn a_sidecar_that_dies_after_start_halts_progression_but_not_the_heartbeat() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge(&shared, true, None);
        let mut t = Instant::now();
        b.on_client_message(&join(2), 165, t);
        let mut wire = Vec::new();
        run_for(&mut b, &mut t, 10.0, &mut wire);
        let s = shared.lock().unwrap();
        assert_eq!(s.launches, 1, "not restarted once the scripts had started");
        assert!(!s.sent.iter().any(|m| matches!(m, HostMsg::Tick { .. })), "no clock after the halt");
        assert_eq!(s.rewards.len(), 1);
        assert_eq!(s.rewards[0].2, ActivityOutcome::Abandoned);
        assert!(wire.iter().filter(|f| f.len() == 6).count() >= 2, "heartbeats continue");
    }

    #[test]
    fn a_sidecar_that_will_not_start_is_tried_three_times() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge(&shared, false, None);
        let s = shared.clone();
        b.launch = Box::new(move |_| {
            s.lock().unwrap().launches += 1;
            Err("node: not found".into())
        });
        let mut t = Instant::now();
        b.on_client_message(&join(3), 165, t);
        run_for(&mut b, &mut t, 10.0, &mut Vec::new());
        assert_eq!(shared.lock().unwrap().launches, 3);
        assert!(b.scripted(), "the world is still served from the table");
    }

    #[test]
    fn a_rejoin_resends_the_full_list_with_current_values() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge(&shared, false, None);
        let mut t = Instant::now();
        b.on_client_message(&join(4), 165, t);
        b.on_client_message(&slice(25), 165, t);
        run_for(&mut b, &mut t, 3.5, &mut Vec::new());
        b.on_client_message(&join(4), 165, t);
        run_for(&mut b, &mut t, 0.25, &mut Vec::new());
        let s = shared.lock().unwrap();
        let full = s.frames.last().unwrap();
        assert!(full.full && full.init);
        assert_eq!(lifetime_of(full), Some(4), "the current lifetime, not the list's");
        assert_eq!(s.launches, 1, "the same sidecar carries on");
        assert_eq!(s.rewards.len(), 0);
    }

    #[test]
    fn squad_senses_go_to_the_sidecar_by_bundle() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge(&shared, false, None);
        let mut t = Instant::now();
        b.on_client_message(&join(5), 165, t);
        run_for(&mut b, &mut t, 0.5, &mut Vec::new());
        shared.lock().unwrap().sent.clear();
        let counts = [crate::sense::SquadCount { group: 0x9243_DFFB, kind: 1, index: 17, live: 0 }];
        let squads = [SquadSense { group: 0x9243_DFFB, index: 17, state: 2, flag: true }];
        b.on_sense(&counts, &squads);
        let item = ItemRef { bundle: 0x9243_DFFB, type_id: 1, type_index: 17 };
        assert_eq!(
            shared.lock().unwrap().sent,
            vec![HostMsg::SquadAlive { item, count: 0 }, HostMsg::SquadState { item, state: 2, flag: true }]
        );
    }

    #[test]
    fn squad_sense_reads_only_changed_state_bytes() {
        use d1_oracle::sensor_sense::{SenseEntry, SenseFrame, SenseName};
        let entry = |kind, changed: Vec<usize>| {
            let mut image = vec![0; 160];
            image[0x19] = 3;
            image[0x1A] = 1;
            SenseEntry { name: SenseName { group: 9, kind, index: 2 }, tag: crate::sense::SQUAD_B_TAG,
                revision: 0, body_bits: 1, reused_observation: false, changed_offsets: changed, image }
        };
        let f = SenseFrame { initial_flag: false, generation: 1, padding_bits: 0,
            entries: vec![entry(1, vec![0x0C]), entry(1, vec![0x19]), entry(2, vec![0x1A])] };
        assert_eq!(squad_sense(&f), vec![SquadSense { group: 9, index: 2, state: 3, flag: true }]);
    }

    #[test]
    fn the_activity_table_line_parses_and_refuses_junk() {
        let read = |_: &str| Ok(LIST.to_string());
        let a = ScriptedActivity::parse("165 cine_reveal 0x4786c0e0 l.txt map=0x1:0x2,3:4 bundles=x.json team=0", &base(), &read).unwrap();
        assert_eq!((a.group_of(1), a.group_of(3), a.group_of(9)), (2, 4, 9));
        assert_eq!((a.bundle_of(2), a.bundle_of(9)), (1, 9));
        assert_eq!(a.rows.len(), 3);
        assert_eq!(a.bundles_path.as_deref(), Some("x.json"));
        assert!(ScriptedActivity::parse("165 cine_reveal 0x4786c0e0", &base(), &read).is_err());
        assert!(ScriptedActivity::parse("165 cine_reveal 0x4786c0e0 l.txt colour=red", &base(), &read).is_err());
        assert!(ScriptedActivity::parse("x cine_reveal 0x4786c0e0 l.txt", &base(), &read).is_err());
    }
}
