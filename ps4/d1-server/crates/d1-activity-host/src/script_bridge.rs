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
//! lines on stdin/stdout (stderr is logged). The contract is A1's `tools/script-host/PROTOCOL.md`
//! (protocol 1): `hello` → `loaded`; `start` (once the world's full list is out and the start
//! cushion after it, `D1_AH_SCRIPTS_START_DELAY`, has passed) → `ready`; then
//! one `tick` per 10 Hz udt 5 stamp step, carrying the stamp, each answered with its changes and
//! `tick_done`. Client events go in host order between ticks, so a run replays from the host log.
//! A sidecar that cannot start is retried twice; one that exits after `start` (or refuses the
//! session) stops the progression while the rows already sent stay and heartbeats continue, so
//! the client is not dropped. Nothing is guessed in its place.
//!
//! Item references from the sidecar are BullShcript's `{bundle, typeId, typeIndex}`; host messages
//! name a row by `bundle` and `type_index`. The bridge maps a bundle hash to the world's live group
//! hash (`map=` in the activity table, identity otherwise). **Stand-in rows** (`loaded.stand_in`,
//! no content export: their indices are not the game's) never go on the wire; only the lifetime,
//! which the bridge keys itself, does.

//! # The start (runs fg2 and fg3b, 6 November)
//!
//! The first full list is the world's own: a scripted activity that `D1_AH_WORLD_TABLE` names
//! starts from that row's list, every record with its value, the bridge's own rows merged in
//! ([`ScriptedActivity::merge_world_list`]), and its lifetime starts at the list's `@phase`, else
//! 0, the live first value; never the launch phase `D1_AH_SENSOR_PHASE` (lab-base's 3 only arms
//! the sensor path). The scripts start [`DEFAULT_START_DELAY_S`] seconds later (a stand-in: the
//! client reports nothing when its initial slice set has loaded), and client events before then
//! wait for the start. A phase in the slice set the client loads at the join asks for no teleport.
//! The note: notes/ps4-56578-script-bridge-2026-11-05.md, "The first integrated run".

//! # What each message does here
//!
//! * **Lifetime.** A `lifetime` row change sets the world's lifetime record (the one the world's
//!   list names) to [`lifetime_wire_state`]: pregame → 1, active → 3, cinematic hold → 4,
//!   results → 6, and ended → 7 for a gameplay activity or 8 for a cinematic-only one
//!   (`kind=cinematic`; notes/ps4-56578-wire-catalog-2026-11-05.md). The sidecar's `wire.phase` is
//!   checked against it. `phase` carries the mission-phase tuple only under
//!   `D1_AH_SCRIPTS_MISSION_PHASE=1` (its meaning on 56578 is unverified), and asks the host to move
//!   the client to the phase's region (`regions=` in the activity table, since protocol 1 names the
//!   slice, not its id) when it is not already there.
//! * **`activity_complete`** sends one udt 18: 965, 3235, 2937, 2009 and 614 (3235 left out for a
//!   cinematic-only activity) with one `0x80800595` body built from the activity's manifest hashes
//!   ([`crate::completion`]); without the hashes it is not sent, and the log says so.
//! * **Rewards at the results.** When the lifetime reaches **6** the bridge calls
//!   [`ActivityRewards::grant_activity_rewards`] once for each joined character, `Completed` if the
//!   scripts completed the activity. Not at the end: a gameplay activity's 7 opens the post-game
//!   screens, which show what the profile holds by then, and a cinematic-only activity's 8 is the
//!   client's own transition to what comes next. A run that ends without passing 6 is granted at
//!   `ended` (`EndedIncomplete`), and one that stops early is `Abandoned`.
//! * **The arc.** A completed activity whose table row says `next=INDEX:HASH` reports the hand-off
//!   right after its grant ([`ActivityRewards::activity_hand_off`]): cine_reveal's next activity is
//!   the Tower. The client picks what it launches from the profile it is served, so the responder
//!   records it (stream O's forwarder).
//! * **Slices.** With the generated world tables (`D1_AH_WORLDS`) and their slice states, a phase's
//!   region comes from content (the scenario state named by the phase, else the bubble the phase's
//!   slice names), and each slice set the client enters makes its bundles active (`active_bundles`).
//! * **Public events.** Stream G's `start_event` / `stop_event` send `event_start` / `event_stop`:
//!   the event's bundle joins or leaves the active set.
//! * **Client incidents** (udt 18) are translated with the 56578 catalogue: 3403 → `trigger`
//!   (its row from the body), 891 / 1726 → `cinematic_finished` / `cinematic_skipped` (the row named
//!   in the body as 3403 names its trigger, else the one cinematic playing; never a guess between
//!   two), 566 / 2014 → `player_spawned`, 2079 → `player_died`, anything else → `incident {id,
//!   name_hash}`.
//! * **Client senses** (udt 6, `D1_AH_SENSE_SCHEMAS`): a squad's live-member count (B `+0x0C`,
//!   proven by the member-death control) → `squad_alive`. Its state byte `+0x19` and bit `+0x1A`
//!   (traced statically from the squad record's flags; meaning open) are decoded and logged only:
//!   protocol 1 has no message for them.

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::incident_catalog::{known, IncidentCatalog};
use crate::messages::ClientMessage;
use crate::rewards::{ActivityOutcome, ActivityRewards, CharacterRef, LoggedRewards};
use crate::completion::{ActivityHashes, CompletionBody};
use crate::row_encoders::{
    lifetime_image, lifetime_wire_state, CodecDone, CodecJob, CodecWorker, EncodeContext, Encoded, EndKind,
    MappedRef, MissionPhase, RowEncoders, TAG_LIFETIME,
};
use crate::rows::{FramePlan, Row, RowKey, RowTable};

pub const PROTO: u32 = 1;
/// Script ticks per second: one per udt 5 stamp step (PROTOCOL.md "Clock and ordering").
pub const TICK_HZ: u32 = 10;
/// The lifetime value every live mission starts at, at the join and at each world load (52 live
/// captures; notes/ps4-56578-wire-catalog-2026-11-05.md).
pub const LIVE_FIRST_LIFETIME: i8 = 0;
/// `D1_AH_SCRIPTS_START_DELAY`'s default, in seconds: the scripts start this long after the first
/// full list went out. ⚠ A STAND-IN cushion, not a retail rule: the client sends nothing on the
/// wire when its initial slice set has loaded (runs fg2 and fg3b, 6 November: between the first
/// full list and the first cinematic there are only keepalives and the 60 s `high_water` beacon),
/// and the working step schedule (`cine-reveal-steps.txt`) waited 60 s before pregame.
pub const DEFAULT_START_DELAY_S: f64 = 60.0;
/// `D1_AH_SCRIPTS_ARRIVAL_CUSHION`'s default, in seconds: how long the script clock stays held after
/// the client is taken to have arrived in the slice set a phase moved it to. ⚠ A STAND-IN, like the
/// start cushion (run fg2e, 6 November: a play 4 s after the move to 26 never started; the step
/// schedule's played 900 s after it).
pub const DEFAULT_ARRIVAL_CUSHION_S: f64 = 20.0;
/// A move with no arrival seen (no udt 21 report, no teleport finished) is taken as arrived after
/// this long, so the scripts never stop for good.
pub const ARRIVAL_TIMEOUT: Duration = Duration::from_secs(120);
/// `D1_AH_SCRIPTS_LIST_RETRY`'s default, in seconds: a full list the client has not answered with
/// a udt 6 (`sensor_sense_update`) this long after it went out goes out again. ⚠ A stand-in
/// (run fg2f, 6 November: a list 0.3 s after the join, before the client's state refresh, was
/// never answered and the reveal never started; fg2e's was answered 11 s after, fg3b's 0.2 s).
pub const DEFAULT_LIST_RETRY_S: f64 = 10.0;
/// How many times an unanswered first full list goes out again before the start cushion counts
/// anyway.
pub const LIST_RETRIES: u32 = 5;

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

/// BullShcript's `ItemChange`: `{item, kind, state, initial?}`. The sidecar adds `wire: {phase}`
/// to lifetime changes (PROTOCOL.md "The lifetime").
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ItemChange {
    pub item: ItemRef,
    pub kind: String,
    #[serde(default)]
    pub state: Value,
    #[serde(default)]
    pub initial: bool,
    #[serde(default)]
    pub wire: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PlayerInfo {
    pub key: String,
    pub slot: u32,
    /// The scripts' `all_players_are_ready()`. Without it the sidecar waits for a
    /// `player_spawned`, but the client spawns only once the lifetime is active, which the
    /// scripts set only after that wait: Restoration sat at pregame in-world, black (run fg7).
    /// ⚠ A stand-in: ready once the client answered a full list with its senses (udt 6). Live the
    /// host copied the client's own player-bundle state (B, whose `+8` it writes each tick) into
    /// the bundle's A about 0.6 s after its first senses; that copy is not decoded here yet.
    pub ready: bool,
}

/// Host → sidecar (tools/script-host/PROTOCOL.md). A row is named by `bundle` and `type_index`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostMsg {
    Hello {
        proto: u32,
        build: String,
        activity: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        bundles_path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        incidents_path: Option<String>,
        tick_hz: u32,
        /// `D1_AH_SCRIPTS_START_PHASE` (lab): start at this phase ordinal; omitted by default.
        #[serde(skip_serializing_if = "Option::is_none")]
        phase_index: Option<u32>,
    },
    Start,
    Tick { now: u64 },
    Players { players: Vec<PlayerInfo> },
    PlayerSpawned { key: String },
    PlayerDied { key: String },
    Trigger { bundle: u32, type_index: i16 },
    CinematicFinished { bundle: u32, type_index: i16 },
    CinematicSkipped { bundle: u32, type_index: i16, key: String },
    Incident { id: u32, name_hash: u32 },
    SquadAlive { bundle: u32, type_index: i16, count: i32 },
    /// The bundles whose threads run (a public bubble's, from stream G; a slice set's, as the
    /// client enters it).
    ActiveBundles { bundles: Vec<u32> },
    /// One public event of a bundle starts: the bundle joins the active set.
    EventStart { bundle: u32, event_id: u32 },
    /// One public event stops: the bundle leaves the active set.
    EventStop { bundle: u32, event_id: u32 },
    End,
}

/// Sidecar → host (tools/script-host/PROTOCOL.md). Fields the bridge does not use are ignored.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SidecarMsg {
    /// The reply to `hello`.
    Loaded {
        /// Rows built from the scripts' own declarations: their indices are not the game's, so
        /// their rows never go on the wire.
        #[serde(default)]
        stand_in: bool,
        #[serde(default)]
        stub: bool,
        #[serde(default)]
        drives_lifetime: bool,
        #[serde(default)]
        compile_errors: Vec<Value>,
    },
    /// The reply to `start`.
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
        name_hash: Option<Value>,
        #[serde(default)]
        slice_hash: Option<Value>,
        /// Not in protocol 1 (the phase names its slice by name and hash); accepted if sent.
        #[serde(default)]
        region: Option<u32>,
    },
    ActivityFlag { hash: u32, active: bool },
    /// The answer to `event_start` / `event_stop`: `started`, `stopped` or `refused`.
    Event {
        bundle: u32,
        #[serde(default)]
        event_id: Option<u32>,
        state: String,
        #[serde(default)]
        reason: Option<String>,
    },
    ActivityComplete {},
    Ended {},
    HardWipeArmed {},
    AllDead {},
    TickDone { now: u64 },
    Log { text: String },
    Error {
        text: String,
        #[serde(default)]
        fatal: bool,
    },
    Done {},
    Snapshot {},
    Bye {},
}

/// A hash the sidecar may send as a number or a `"0x…"` string.
fn hash_value(v: &Value) -> Option<u32> {
    match v {
        Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        Value::String(s) => u32::from_str_radix(s.trim_start_matches("0x"), 16).ok(),
        _ => None,
    }
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
    /// The list's `@phase` (decoded value), else the launch phase (`D1_AH_SENSOR_PHASE`), as the
    /// world table builds it. Stream R's Crucible worlds start at it; the script bridge starts at
    /// [`Self::initial_lifetime`].
    pub lifetime_state: i8,
    /// The `@phase` the list names itself, if it does.
    pub listed_phase: Option<i8>,
    /// The spawn team written into the player bundle's A value (`team=`, -1 by default).
    pub team: i32,
    pub authority: Option<d1_oracle::sensor::SensorAuthority>,
    pub bundles_path: Option<String>,
    /// Script bundle hash → live group hash.
    pub bundle_groups: HashMap<u32, u32>,
    /// Mission phase index → the region (slice-set id, `bubble << 3 | state`) it runs in. The
    /// sidecar names a phase's slice by name and hash; until content resolves those, the table says.
    pub regions: HashMap<u32, u32>,
    /// How the activity ends (`kind=cinematic`; gameplay otherwise): the lifetime's last value and
    /// the completion chain differ.
    pub end: EndKind,
    /// The activity's manifest hashes for the `activity_complete` body (`type=`, `type2=`,
    /// `place=`, with `hash=`). Without them the completion is not sent.
    pub hashes: Option<ActivityHashes>,
    /// The world (`world=`, else the activity name), for the generated tables.
    pub world: String,
    /// The start slice set (`start=`), when the table gives one.
    pub start: Option<u32>,
    /// The next activity of the story arc (`next=INDEX:HASH`): reported at completion.
    pub next: Option<(u16, u32)>,
    /// Slice set -> the bundle groups it loads (from the generated world tables).
    pub slice_groups: HashMap<u32, Vec<u32>>,
    /// Slice set -> its sensor list's rows (from the generated world tables): the list the client
    /// registers there, sent again in full when it moves to that set. Shared by every row of the
    /// table on the same world, group and team ([`SliceRowsCache`]).
    pub slice_rows: Arc<HashMap<u32, Vec<(RowKey, Row)>>>,
    /// Mission phase name hash -> its slice set, and slice (region) hash -> the bubble's first
    /// private slice set (from the generated world tables and their slice states).
    pub phase_slices: HashMap<u32, u32>,
    pub bubble_slices: HashMap<u32, u32>,
    /// Served values for records the start's list does not name, from the lists the step
    /// schedule serves on entering a slice set (`D1_AH_SCRIPTS_SLICE_LISTS`): the base image a
    /// script change builds on, and the value a slice set's full list carries until the scripts
    /// set one. Run fg2d: /6/1 from the encoder's default (no cast) never started; the step
    /// schedule's move-26 value (a one-element cast) did.
    pub values: HashMap<RowKey, Row>,
    /// `D1_AH_SCRIPTS_PUBLIC_START_LIST`: set for a story mission that starts in a public slice set
    /// ([`Self::use_public_start_list`]).
    pub mission_list: Option<MissionHostList>,
}

/// What the live mission host served for a story mission that starts in a public slice set: the
/// world's global rows and the private bubbles' shared rows, not the public start's whole list
/// (its public-bubble rows belong to the public hosts). Three live Restoration / Dark Within
/// captures (51539), decoded against the 56578 tables.
#[derive(Clone, Debug)]
pub struct MissionHostList {
    /// The public slice sets of the world (no relist for them).
    pub public_slices: Vec<u32>,
    /// Private slice set -> the list the client holds there (its own list and the shared rows).
    pub relists: HashMap<u32, Vec<(RowKey, Row)>>,
    /// The public start slice set and the full relist the client gets when it first reports being
    /// there: the start's globals and the shared rows (the live host's first udt 6 followed it).
    pub start_relist: (u32, Vec<(RowKey, Row)>),
}

/// Every slice set's rows of one world, shared by the table's rows on that world (stream Q: the
/// table lists every bound PvE activity, and a world's heroic, nightfall and playlist copies would
/// otherwise each hold their own copy). Keyed by world, group, team and the lifetime name the base
/// gives (`params_for_list` reads those).
pub type SliceRowsCache = HashMap<(String, u32, i32, u8, i16), Arc<HashMap<u32, Vec<(RowKey, Row)>>>>;

impl ScriptedActivity {
    /// Parse `INDEX ACTIVITY GROUP LIST [hash=H] [kind=cinematic] [type=H type2=H place=H]
    /// [bundles=PATH] [map=B:G,...] [regions=P:R,...] [team=0]`, building
    /// the rows from LIST as the world table does (`sensor_list::params_for_list`).
    pub fn parse(
        line: &str,
        base: &d1_oracle::sensor::SensorPhaseUpdate,
        read: &dyn Fn(&str) -> Result<String, String>,
    ) -> Result<Self, String> {
        Self::parse_with(line, base, read, None)
    }

    /// [`Self::parse`] with the generated world tables: LIST may be `@generated` (the world's
    /// sensor list for `start=`, as `D1_AH_WORLDS` builds it), and the slice tables are filled.
    pub fn parse_with(
        line: &str,
        base: &d1_oracle::sensor::SensorPhaseUpdate,
        read: &dyn Fn(&str) -> Result<String, String>,
        worlds: Option<&crate::generated::GeneratedWorlds>,
    ) -> Result<Self, String> {
        Self::parse_cached(line, base, read, worlds, &mut SliceRowsCache::new())
    }

    /// [`Self::parse_with`], sharing each world's slice lists through `cache` (the whole table).
    pub fn parse_cached(
        line: &str,
        base: &d1_oracle::sensor::SensorPhaseUpdate,
        read: &dyn Fn(&str) -> Result<String, String>,
        worlds: Option<&crate::generated::GeneratedWorlds>,
        cache: &mut SliceRowsCache,
    ) -> Result<Self, String> {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 4 {
            return Err(format!("{line:?}: want `INDEX ACTIVITY GROUP LIST [hash=H] [kind=cinematic] [type=H type2=H place=H] [bundles=PATH] [map=B:G,...] [regions=P:R,...] [team=0] [world=W] [start=N] [next=I:H]`"));
        }
        let num = |s: &str| {
            s.strip_prefix("0x").map_or_else(|| s.parse::<u32>().ok(), |h| u32::from_str_radix(h, 16).ok())
        };
        let index: u16 = f[0].parse().map_err(|_| format!("{line:?}: bad index"))?;
        let group = num(f[2]).ok_or_else(|| format!("{line:?}: bad group"))?;
        let (mut hash, mut bundles_path, mut team) = (0, None, -1);
        let (mut bundle_groups, mut regions) = (HashMap::new(), HashMap::new());
        let (mut end, mut kind, mut kind2, mut place) = (EndKind::Gameplay, None, None, None);
        let (mut world, mut start, mut next) = (f[1].to_string(), None, None);
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
            } else if let Some(v) = t.strip_prefix("regions=") {
                for pair in v.split(',') {
                    let (p, r) = pair.split_once(':').ok_or_else(|| format!("{line:?}: bad regions {pair:?}"))?;
                    regions.insert(
                        num(p).ok_or_else(|| format!("{line:?}: bad phase {p:?}"))?,
                        num(r).ok_or_else(|| format!("{line:?}: bad region {r:?}"))?,
                    );
                }
            } else if let Some(v) = t.strip_prefix("kind=") {
                end = match v {
                    "cinematic" => EndKind::Cinematic,
                    "gameplay" => EndKind::Gameplay,
                    _ => return Err(format!("{line:?}: kind is cinematic or gameplay")),
                };
            } else if let Some(v) = t.strip_prefix("type2=") {
                kind2 = Some(num(v).ok_or_else(|| format!("{line:?}: bad type2"))?);
            } else if let Some(v) = t.strip_prefix("type=") {
                kind = Some(num(v).ok_or_else(|| format!("{line:?}: bad type"))?);
            } else if let Some(v) = t.strip_prefix("place=") {
                place = Some(num(v).ok_or_else(|| format!("{line:?}: bad place"))?);
            } else if let Some(v) = t.strip_prefix("team=") {
                team = v.parse().map_err(|_| format!("{line:?}: bad team"))?;
            } else if let Some(v) = t.strip_prefix("world=") {
                world = v.to_string();
            } else if let Some(v) = t.strip_prefix("start=") {
                start = Some(num(v).ok_or_else(|| format!("{line:?}: bad start"))?);
            } else if let Some(v) = t.strip_prefix("next=") {
                let (i, h) = v.split_once(':').ok_or_else(|| format!("{line:?}: next is INDEX:HASH"))?;
                let i = u16::try_from(num(i).ok_or_else(|| format!("{line:?}: bad next index"))?)
                    .map_err(|_| format!("{line:?}: bad next index"))?;
                next = Some((i, num(h).ok_or_else(|| format!("{line:?}: bad next hash"))?));
            } else {
                return Err(format!("{line:?}: unknown field {t:?}"));
            }
        }
        let text = if f[3] == "@generated" {
            let w = worlds.ok_or_else(|| format!("{line:?}: @generated needs the generated world tables (D1_AH_WORLDS)"))?;
            let s = start.ok_or_else(|| format!("{line:?}: @generated needs start="))?;
            if !w.slices.contains_key(&(world.clone(), s)) {
                return Err(format!("{line:?}: the world tables have no slice set {s} for {world}"));
            }
            w.sensor_list(&world, s)
        } else {
            read(f[3])?
        };
        let (mut slice_groups, mut phase_slices, mut bubble_slices) = (HashMap::new(), HashMap::new(), HashMap::new());
        let mut slice_rows = Arc::new(HashMap::new());
        if let Some(w) = worlds {
            slice_rows = cache
                .entry((world.clone(), group, team, base.name_i8, base.name_sub))
                .or_insert_with(|| {
                    let mut rows = HashMap::new();
                    for s in w.slice_sets(&world) {
                        if let Ok(p) = crate::sensor_list::params_for_list(base, group, team, &w.sensor_list(&world, s)) {
                            rows.insert(s, p.clients.iter().map(Row::from_client).collect::<Vec<_>>());
                        }
                    }
                    Arc::new(rows)
                })
                .clone();
            for s in w.slice_sets(&world) {
                slice_groups.insert(s, w.slice_groups(&world, s));
                let Some(set) = w.slices.get(&(world.clone(), s)) else { continue };
                if !set.valid {
                    continue;
                }
                if let Some(h) = set.state_hash {
                    phase_slices.entry(h).or_insert(s);
                }
                if let (Some(h), false) = (set.region_hash, set.public) {
                    bubble_slices.entry(h).or_insert(s);
                }
            }
        }
        let params = crate::sensor_list::params_for_list(base, group, team, &text).map_err(|e| format!("{}: {e:#}", f[3]))?;
        Ok(Self {
            index,
            activity: f[1].to_string(),
            hash,
            group,
            rows: params.clients.iter().map(Row::from_client).collect(),
            lifetime: RowKey { group, kind: params.name_i8, index: params.name_sub },
            lifetime_state: params.phase,
            listed_phase: crate::sensor_list::listed_phase(&text),
            team,
            authority: params.authority.clone(),
            bundles_path,
            bundle_groups,
            regions,
            end,
            hashes: match (kind, kind2, place) {
                (Some(kind), Some(kind2), Some(place)) if hash != 0 => {
                    Some(ActivityHashes { activity: hash, kind, kind2, place })
                }
                (None, None, None) => None,
                _ => return Err(format!("{line:?}: the completion body needs hash=, type=, type2= and place= together")),
            },
            world,
            start,
            next,
            slice_groups,
            slice_rows,
            phase_slices,
            bubble_slices,
            values: HashMap::new(),
            mission_list: None,
        })
    }

    /// `D1_AH_SCRIPTS_PUBLIC_START_LIST`: when the activity's start slice set is public, the join
    /// list becomes [`GeneratedWorlds::mission_host_clients`] (the activity's `private_start`
    /// slice set's globals and the private bubbles' shared rows), and the client's move into a private slice set a phase names
    /// relists [`GeneratedWorlds::mission_host_relist_clients`]; any other move
    /// relists nothing. Served values already on a row are kept. Returns the join list's row count
    /// (the lifetime record included), or `None` when the start is not public (nothing changes).
    pub fn use_public_start_list(
        &mut self,
        w: &crate::generated::GeneratedWorlds,
        base: &d1_oracle::sensor::SensorPhaseUpdate,
    ) -> Result<Option<usize>, String> {
        let Some(start) = self.start else { return Ok(None) };
        let Some(set) = w.slices.get(&(self.world.clone(), start)).filter(|s| s.public) else { return Ok(None) };
        let _ = set;
        let Some(private_start) = w.activities.get(&self.index).and_then(|a| a.private_start) else {
            return Err("has no private start slice set".into());
        };
        let rows_of = |this: &Self, clients: &[(u32, crate::generated::Client)], title: &str| -> Result<Vec<(RowKey, Row)>, String> {
            let text = crate::generated::GeneratedWorlds::clients_text(title, clients);
            let p = crate::sensor_list::params_for_list(base, this.group, this.team, &text).map_err(|e| format!("{e:#}"))?;
            Ok(p.clients
                .iter()
                .map(Row::from_client)
                .map(|(k, mut r)| {
                    // A value the activity already serves for the record (the hand world table, the step schedule) stays.
                    if let Some((_, old)) = this.rows.iter().find(|(m, _)| *m == k) {
                        r.a = r.a.or_else(|| old.a.clone());
                        r.b = r.b.or_else(|| old.b.clone());
                    } else if let Some(v) = this.served_value(&k) {
                        r = v;
                    }
                    (k, r)
                })
                .collect())
        };
        let clients = w.mission_host_clients(&self.world, private_start).map_err(|e| format!("{e:#}"))?;
        let lifetime = clients.iter().any(|(g, c)| *g == self.lifetime.group && c.kind == self.lifetime.kind && c.index == self.lifetime.index);
        if !lifetime {
            return Err(format!("{}: the mission host's list lacks the lifetime record {}", self.activity, self.lifetime));
        }
        let rows = rows_of(self, &clients, &format!("{} mission host list (D1_AH_SCRIPTS_PUBLIC_START_LIST)", self.world))?;
        let n = rows.len() + 1;
        let mut relists = HashMap::new();
        for id in w.slice_sets(&self.world) {
            if w.slices.get(&(self.world.clone(), id)).is_some_and(|s| !s.public) {
                let c = w.mission_host_relist_clients(&self.world, id);
                relists.insert(id, rows_of(self, &c, &format!("{} slice set {id}: mission host relist", self.world))?);
            }
        }
        let c = w.mission_host_start_clients(&self.world, start);
        let start_relist = (start, rows_of(self, &c, &format!("{} slice set {start}: mission host start relist", self.world))?);
        self.mission_list = Some(MissionHostList { public_slices: w.public_slice_sets(&self.world), relists, start_relist });
        self.rows = rows;
        Ok(Some(n))
    }

    /// The lifetime value the bridge's first full list carries: the list's own `@phase`, else 0,
    /// the live hosts' first value in every mission at join and world load (live order 0 → 1 → 4
    /// → 6 → 8 for the reveal; notes/ps4-56578-wire-catalog-2026-11-05.md). Never the launch
    /// phase `D1_AH_SENSOR_PHASE`, which only arms the sensor path (lab-base serves 3): run fg2
    /// served 3 and then the scripts' 1, a step backwards, and the client stayed loading.
    pub fn initial_lifetime(&self) -> i8 {
        self.listed_phase.unwrap_or(LIVE_FIRST_LIFETIME)
    }

    /// Serve the world's own list (`D1_AH_WORLD_TABLE`'s row for this activity, the list the
    /// world table and mission step 0 serve) in the first full list: every record of it, with its
    /// values, and the bridge's own rows merged in (a row the world list does not name is kept;
    /// a row both name takes the world list's value, or the bridge's where the world list has
    /// none). The world list's `@phase` and `@authority` win when it names them; its start slice
    /// set stands in for a missing `start=`. Returns `(world rows, rows only the bridge names,
    /// rows with a value)`.
    pub fn merge_world_list(
        &mut self,
        base: &d1_oracle::sensor::SensorPhaseUpdate,
        group: u32,
        text: &str,
        start: Option<u32>,
    ) -> Result<(usize, usize, usize), String> {
        if group != self.group {
            return Err(format!("the world table's group {group:#010x} is not the bridge's {:#010x}", self.group));
        }
        let p = crate::sensor_list::params_for_list(base, group, self.team, text).map_err(|e| format!("{e:#}"))?;
        let lifetime = RowKey { group, kind: p.name_i8, index: p.name_sub };
        if lifetime != self.lifetime {
            return Err(format!("the world table's lifetime record {lifetime} is not the bridge's {}", self.lifetime));
        }
        let mut merged: Vec<(RowKey, Row)> = p.clients.iter().map(Row::from_client).collect();
        let world = merged.len();
        let mut only_ours = 0;
        for (k, ours) in &self.rows {
            match merged.iter_mut().find(|(m, _)| m == k) {
                Some((_, theirs)) => {
                    if theirs.a.is_none() {
                        theirs.a = ours.a.clone();
                    }
                    if theirs.b.is_none() {
                        theirs.b = ours.b.clone();
                    }
                }
                None => {
                    merged.push((*k, ours.clone()));
                    only_ours += 1;
                }
            }
        }
        let valued = merged.iter().filter(|(_, r)| r.a.is_some() || r.b.is_some()).count();
        self.rows = merged;
        if let Some(phase) = crate::sensor_list::listed_phase(text) {
            self.listed_phase = Some(phase);
        }
        if crate::sensor_list::lists_authority(text) {
            self.authority = p.authority;
        }
        if self.start.is_none() {
            self.start = start;
        }
        Ok((world, only_ours, valued))
    }

    /// The list the client holds in slice set `region`: the world's list (every record of the
    /// start's list, with the bridge's merged in), then the records the slice set's own generated
    /// list adds. Run fg2c (6 November): slice set 26 adds the Tower cinematic /6/1, which the
    /// start's list does not name; the step schedule that plays serves `cine_reveal-move-26.txt`,
    /// these 22 records, at the move. `None` when the generated tables have no list for `region`.
    pub fn slice_list(&self, region: u32) -> Option<Vec<(RowKey, Row)>> {
        if let Some(m) = &self.mission_list {
            // The mission host's own lists: a public slice set's rows belong to the public hosts.
            let mut rows = m.relists.get(&region)?.clone();
            live_trigger_defaults(&mut rows);
            return Some(rows);
        }
        let own = self.slice_rows.get(&region)?;
        let mut rows = self.rows.clone();
        for (k, r) in own {
            if *k != self.lifetime && !rows.iter().any(|(m, _)| m == k) {
                rows.push((*k, self.served_value(k).unwrap_or_else(|| r.clone())));
            }
        }
        live_trigger_defaults(&mut rows);
        Some(rows)
    }

    /// A record's served value from the step schedule's slice lists, if one names it with a value.
    pub fn served_value(&self, key: &RowKey) -> Option<Row> {
        self.values.get(key).filter(|r| r.a.is_some() || r.b.is_some()).cloned()
    }

    /// Take the served values of the records `text` (a step list served on entering a slice set)
    /// names beyond the start's list. The first list naming a record wins. Returns how many.
    pub fn add_slice_values(
        &mut self,
        base: &d1_oracle::sensor::SensorPhaseUpdate,
        group: u32,
        text: &str,
    ) -> Result<usize, String> {
        if group != self.group {
            return Err(format!("group {group:#010x} is not the bridge's {:#010x}", self.group));
        }
        let p = crate::sensor_list::params_for_list(base, group, self.team, text).map_err(|e| format!("{e:#}"))?;
        let mut n = 0;
        for (k, r) in p.clients.iter().map(Row::from_client) {
            if (r.a.is_none() && r.b.is_none()) || k == self.lifetime || self.rows.iter().any(|(m, _)| *m == k) {
                continue;
            }
            if let std::collections::hash_map::Entry::Vacant(e) = self.values.entry(k) {
                e.insert(r);
                n += 1;
            }
        }
        Ok(n)
    }

    /// The machine the authority table gives the blocks to (block 0's peer), else 14.
    pub fn machine(&self) -> u8 {
        self.authority.as_ref().and_then(|a| a.image.get(5).copied()).unwrap_or(14)
    }

    /// The region (slice set) a phase runs in: the table's `regions=` first, else content's
    /// (the state named by the phase, else the bubble its slice names).
    pub fn region_of_phase(&self, index: u32, name_hash: u32, slice_hash: Option<u32>) -> Option<u32> {
        self.regions
            .get(&index)
            .copied()
            .or_else(|| self.phase_slices.get(&name_hash).copied())
            .or_else(|| slice_hash.and_then(|h| self.bubble_slices.get(&h).copied()))
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
    /// The catalogue's path, passed to the sidecar in `hello`.
    pub incidents_path: Option<String>,
    pub mission_phase: bool,
    pub send_unlisted: bool,
    pub delta_init: bool,
    pub scoreboard_columns: Vec<u32>,
    /// HOOK (stream O): the rewards service. [`LoggedRewards`] until O's forwarder is plugged
    /// in (crate::rewards).
    pub rewards: Arc<Mutex<dyn ActivityRewards + Send>>,
    pub codec: CodecMaker,
    /// Send `active_bundles` as the client enters each slice set (default on when the activity
    /// has slice tables; `D1_AH_SCRIPTS_ACTIVE_BUNDLES=0` turns it off).
    pub active_bundles: bool,
    /// `D1_AH_SCRIPTS_START_DELAY` (seconds, default [`DEFAULT_START_DELAY_S`]): how long after
    /// the first full list the scripts start (`players`, `start`, and so their first phase, region
    /// move and lifetime change). ⚠ A stand-in cushion: no client "loaded" signal is known.
    pub start_delay: Duration,
    /// `D1_AH_SCRIPTS_ARRIVAL_CUSHION` (seconds, default [`DEFAULT_ARRIVAL_CUSHION_S`]): after a
    /// phase moves the client, the script clock is held until the client arrives (its udt 21
    /// report, else the host's teleport request finished) and this long after. ⚠ A stand-in.
    pub arrival_cushion: Duration,
    /// `D1_AH_SCRIPTS_LIST_RETRY` (seconds, default [`DEFAULT_LIST_RETRY_S`]; `0` = off): until the
    /// client answers the first full list with a udt 6, that list goes out again this often (up to
    /// [`LIST_RETRIES`] times), and the start cushion counts from the answer. `None`: the cushion
    /// counts from the first full list, and nothing is re-sent. ⚠ A stand-in.
    pub list_retry: Option<Duration>,
    /// `D1_AH_SCRIPTS_WORLD_LOOT` (F42): world-loot drops the host announces when the scripts
    /// create their row (the starter rifle's `o_weapon`).
    pub world_loot: Vec<WorldLoot>,
    /// ⭐ F52: props that carry a device command, and the player trigger that arms them
    /// (`D1_AH_SCRIPTS_DEVICE_RULES`).
    pub device_rules: Vec<DeviceRule>,
}

/// A world-loot drop the host announces with incident 1851 (`gather_material`, body `0x80800CB6`)
/// when the scripts create its row: in 56578 the loot exists only once the host announces it, and
/// the client claims it with service 601 (notes/ps4-56578-blockers-2026-10-26.md).
/// ⭐ F52: a prop the live host served with a device command (the tripwires of A Guardian Rises'
/// Breach), and the player trigger whose report arms it, in order. A table line:
/// `activity group/kind/index trigger_group/kind/index order` (the arming goes out `order` x 0.2 s
/// after the report, as the live host's did, one tripwire every 0.2 s).
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceRule {
    pub activity: u16,
    pub object: RowKey,
    pub trigger: RowKey,
    pub order: u32,
}

impl DeviceRule {
    pub fn parse_table(text: &str) -> Result<Vec<Self>, String> {
        let key = |t: &str| -> Option<RowKey> {
            let mut p = t.split('/');
            let hex = |v: &str| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok();
            let group = hex(p.next()?)?;
            let kind = hex(p.next()?)? as u8;
            let index: i16 = p.next()?.parse().ok()?;
            Some(RowKey { group, kind, index })
        };
        text.lines()
            .map(|l| l.split('#').next().unwrap_or("").trim())
            .filter(|l| !l.is_empty())
            .map(|l| {
                let f: Vec<&str> = l.split_whitespace().collect();
                let bad = || format!("device rule {l:?}: want activity group/kind/index trigger_group/kind/index order");
                if f.len() != 4 {
                    return Err(bad());
                }
                Ok(DeviceRule {
                    activity: f[0].parse().map_err(|_| bad())?,
                    object: key(f[1]).ok_or_else(bad)?,
                    trigger: key(f[2]).ok_or_else(bad)?,
                    order: f[3].parse().map_err(|_| bad())?,
                })
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorldLoot {
    pub activity: u16,
    pub row: RowKey,
    /// The nonce `{key, id}` the claim names (the character's pending loot record).
    pub key: u32,
    pub id: u64,
    pub position: [f32; 3],
    /// The world field: the bubble the loot spawns in (the client spawns it only there).
    pub world: u32,
}

/// The objective row a squad value names at `+0` (`group u32, kind u8, 0, index i16`), when it is
/// a kind-3 row.
pub fn squad_objective(row: &Row) -> Option<RowKey> {
    let (tag, image) = row.a.as_ref()?;
    if *tag != crate::row_encoders::TAG_SQUAD || image.len() < 8 || image[4] != crate::row_encoders::KIND_OBJECTIVE {
        return None;
    }
    let group = u32::from_le_bytes(image[0..4].try_into().ok()?);
    let index = i16::from_le_bytes(image[6..8].try_into().ok()?);
    (group != crate::row_encoders::NONE_HASH && index >= 0).then_some(RowKey { group, kind: crate::row_encoders::KIND_OBJECTIVE, index })
}

/// The world-loot drop incident (`gather_material`) and its body's tag.
pub const WORLD_LOOT_CHAIN: &[u32] = &[1851];
pub const WORLD_LOOT_TAG: u32 = 0x8080_0CB6;

impl WorldLoot {
    /// `activity group/kind/index key id x y z world` lines (`#` comments), as
    /// `D1_AH_SCRIPTS_WORLD_LOOT` names.
    pub fn parse_table(text: &str) -> Result<Vec<Self>, String> {
        fn num(v: &str) -> Option<u64> {
            match v.strip_prefix("0x") {
                Some(h) => u64::from_str_radix(h, 16).ok(),
                None => v.parse().ok(),
            }
        }
        text.lines()
            .map(|l| l.split('#').next().unwrap_or("").trim())
            .filter(|l| !l.is_empty())
            .map(|l| {
                let f: Vec<&str> = l.split_whitespace().collect();
                let bad = || format!("world loot line {l:?}: want activity group/kind/index key id x y z world");
                if f.len() != 8 {
                    return Err(bad());
                }
                let r: Vec<&str> = f[1].split('/').collect();
                if r.len() != 3 {
                    return Err(bad());
                }
                let row = RowKey {
                    group: num(r[0]).ok_or_else(bad)? as u32,
                    kind: num(r[1]).ok_or_else(bad)? as u8,
                    index: r[2].parse().map_err(|_| bad())?,
                };
                let f32_at = |i: usize| f[i].parse::<f32>().map_err(|_| bad());
                Ok(Self {
                    activity: f[0].parse().map_err(|_| bad())?,
                    row,
                    key: num(f[2]).ok_or_else(bad)? as u32,
                    id: num(f[3]).ok_or_else(bad)?,
                    position: [f32_at(4)?, f32_at(5)?, f32_at(6)?],
                    world: num(f[7]).ok_or_else(bad)? as u32,
                })
            })
            .collect()
    }

    /// The `0x80800CB6` image: nonce `{key @0, flag @8, id @0x10}`, the recipient (an empty handle
    /// at `+0x18`, the id at `+0x40`), the position at `+0x60` (w 1.0) and the world at `+0x70`
    /// (`lab/history/lab-scripts-2026-10-29/lootbody.py`, which the step schedule's drop used).
    pub fn image(&self, recipient: u64) -> Vec<u8> {
        let mut b = vec![0u8; 128];
        b[0..4].copy_from_slice(&self.key.to_le_bytes());
        b[8] = 1;
        b[0x10..0x18].copy_from_slice(&self.id.to_le_bytes());
        b[0x18..0x20].fill(0xFF);
        b[0x40..0x48].copy_from_slice(&recipient.to_le_bytes());
        for (i, v) in self.position.iter().chain([1.0f32].iter()).enumerate() {
            b[0x60 + 4 * i..0x64 + 4 * i].copy_from_slice(&v.to_le_bytes());
        }
        b[0x70..0x74].copy_from_slice(&self.world.to_le_bytes());
        b
    }
}

/// `D1_AH_SCRIPTS_LIST_RETRY`, in seconds; `0` turns the retry and the udt 6 gate off.
pub fn list_retry_from(value: Option<&str>) -> Result<Option<Duration>, String> {
    seconds_knob("D1_AH_SCRIPTS_LIST_RETRY", value, DEFAULT_LIST_RETRY_S).map(|d| Some(d).filter(|d| !d.is_zero()))
}

/// `D1_AH_SCRIPTS_ARRIVAL_CUSHION`, in seconds.
pub fn arrival_cushion_from(value: Option<&str>) -> Result<Duration, String> {
    seconds_knob("D1_AH_SCRIPTS_ARRIVAL_CUSHION", value, DEFAULT_ARRIVAL_CUSHION_S)
}

fn seconds_knob(name: &str, value: Option<&str>, default: f64) -> Result<Duration, String> {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(Duration::from_secs_f64(default)),
        Some(v) => v
            .parse::<f64>()
            .ok()
            .filter(|s| s.is_finite() && *s >= 0.0)
            .map(Duration::from_secs_f64)
            .ok_or_else(|| format!("{name}={v:?}: want seconds, 0 or more")),
    }
}

/// The image the live mission host served for a player trigger no script had set: enabled, arming
/// 0. In A Guardian Rises' capture every trigger in the gameplay list went out so
/// (`0x992BB471/0x1D/30`, `/31`, `/32`, `/37`, `/42` at its first gameplay frame), and the client
/// fired `/30` on entering its volume; the host then sent it with a new arming (`0x0707`).
pub const LIVE_TRIGGER_IMAGE: [u8; 8] = [1, 0, 0, 0, 0, 0, 0, 0];

/// Give each player trigger (kind 0x1D) that a list names with an A value but serves none the
/// live host's image ([`LIVE_TRIGGER_IMAGE`]). Without it the record goes out at the default,
/// which the client holds disabled, so it never fires: run agr2 walked into the Wall and phase 1's
/// `pt_first_obj` (`0x992BB471/0x1D/30`) never reported (F36). The scripts only arm a trigger they
/// `enable()`/`reset()`; one they only poll was armed by the host's list. Returns how many.
pub fn live_trigger_defaults(rows: &mut [(RowKey, Row)]) -> usize {
    use crate::row_encoders as re;
    let mut n = 0;
    for (k, r) in rows.iter_mut() {
        if k.kind == re::KIND_PLAYER_TRIGGER && r.has_a && r.a.is_none() {
            r.a = Some((re::TAG_PLAYER_TRIGGER, LIVE_TRIGGER_IMAGE.to_vec()));
            n += 1;
        }
    }
    n
}

/// `D1_AH_DUMP_FRAMES=<file>` (a diagnostic): each udt 5 frame the bridge sends is appended as a
/// JSON line, `{stamp, full, frame, entries: [[row, tag, image hex]...]}`, so a run's frames can be
/// read back with the client's own codec (`tools/ps4_setup/d1-frames.py`). Unset: nothing. The
/// frames carry the player's machine id (the join grant): write them outside the repository.
fn dump_path() -> Option<&'static std::path::PathBuf> {
    static PATH: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();
    PATH.get_or_init(|| std::env::var_os("D1_AH_DUMP_FRAMES").filter(|v| !v.is_empty()).map(Into::into)).as_ref()
}

/// The host's own udt 5 frames (mission steps, the lab list) for `D1_AH_DUMP_FRAMES`: `{source:
/// "host", frame}` lines, beside the bridge's.
pub fn dump_host_frame(bytes: &[u8]) {
    let Some(path) = dump_path() else { return };
    let hex: String = bytes.iter().map(|x| format!("{x:02x}")).collect();
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{}", serde_json::json!({"source": "host", "frame": hex}));
    }
}

fn dump_frame(stamp: u32, plan: &FramePlan, bytes: &[u8]) {
    let Some(path) = dump_path() else { return };
    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    let entries: Vec<serde_json::Value> = plan
        .entries
        .iter()
        .map(|(k, r)| match &r.a {
            Some((tag, image)) => serde_json::json!([k.to_string(), format!("{tag:#010x}"), hex(image)]),
            None => serde_json::json!([k.to_string(), null, null]),
        })
        .collect();
    let line = serde_json::json!({"stamp": stamp, "full": plan.full, "authority": plan.authority.is_some(), "frame": hex(bytes), "entries": entries});
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{line}");
    }
}

/// `D1_AH_SCRIPTS_START_DELAY`, in seconds (`0` = start as soon as the full list is out).
pub fn start_delay_from(value: Option<&str>) -> Result<Duration, String> {
    seconds_knob("D1_AH_SCRIPTS_START_DELAY", value, DEFAULT_START_DELAY_S)
}

/// `D1_AH_SCRIPTS_PUBLIC_START_LIST`: give every activity that starts in a public slice set the
/// mission host's list. An activity that cannot (no private start, no lifetime record, a list that
/// does not parse) logs a warning and keeps today's list; the config still loads. Returns
/// `(activities given the mission host list, activities that fell back)`.
pub fn apply_public_start_list(
    activities: &mut HashMap<u16, ScriptedActivity>,
    w: &crate::generated::GeneratedWorlds,
    base: &d1_oracle::sensor::SensorPhaseUpdate,
) -> (usize, usize) {
    let mut indexes: Vec<u16> = activities.keys().copied().collect();
    indexes.sort_unstable();
    let (mut given, mut fell_back) = (0, 0);
    for i in indexes {
        let Some(a) = activities.get_mut(&i) else { continue };
        match a.use_public_start_list(w, base) {
            Ok(Some(n)) => {
                given += 1;
                tracing::info!("script bridge: activity {i} ({}) joins with the mission host list, {n} rows", a.activity);
            }
            Ok(None) => {}
            Err(e) => {
                fell_back += 1;
                tracing::warn!(
                    "script bridge: activity {i} ({}): {e}; today's list stands (D1_AH_SCRIPTS_PUBLIC_START_LIST)",
                    a.activity
                );
            }
        }
    }
    (given, fell_back)
}

/// The step schedule's lists as the bridge's served values (`D1_AH_SCRIPTS_SLICE_LISTS`, a
/// `D1_AH_MISSION_STEPS` file: `index when group list [region] [incident=...]`). Of each scripted
/// activity's steps, the first (the start's list) and every step that names a region (the list
/// served on entering that slice set) give the values of the records the start's list lacks
/// ([`ScriptedActivity::add_slice_values`]); the other steps are states, not slice lists.
pub fn apply_slice_lists(
    activities: &mut HashMap<u16, ScriptedActivity>,
    steps: &str,
    base: &d1_oracle::sensor::SensorPhaseUpdate,
    read: &dyn Fn(&str) -> Result<String, String>,
) -> Vec<String> {
    let num = |s: &str| s.strip_prefix("0x").map_or_else(|| s.parse::<u32>().ok(), |h| u32::from_str_radix(h, 16).ok());
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for line in steps.lines().map(|l| l.split('#').next().unwrap_or("").trim()).filter(|l| !l.is_empty()) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let Some(a) = f.first().and_then(|i| i.parse::<u16>().ok()).and_then(|i| activities.get_mut(&i)) else { continue };
        if f.len() < 4 {
            continue;
        }
        let region = f.get(4).filter(|t| !t.starts_with("incident=")).and_then(|t| num(t));
        let first = seen.insert(a.index);
        if !first && region.is_none() {
            continue;
        }
        let Some(group) = num(f[2]) else {
            out.push(format!("⚠ activity {}: step list line {line:?}: bad group", a.index));
            continue;
        };
        let where_ = region.map_or_else(|| "the start".to_string(), |r| format!("slice set {r}"));
        out.push(match read(f[3]).and_then(|text| a.add_slice_values(base, group, &text)) {
            Ok(0) => continue,
            Ok(n) => format!(
                "⭐ script bridge: activity {} ({}): {n} served values from {} ({where_})",
                a.index, a.activity, f[3]
            ),
            Err(e) => format!("⚠ activity {}: step list {} not used: {e}", a.index, f[3]),
        });
    }
    out
}

/// Serve each scripted activity that `D1_AH_WORLD_TABLE` (`TABLE`, its text) names with the world
/// table's own list ([`ScriptedActivity::merge_world_list`]), as the instance serves that world
/// without the bridge. `read` opens a list named in the table. Returns one line per activity.
pub fn apply_world_table(
    activities: &mut HashMap<u16, ScriptedActivity>,
    table: &str,
    base: &d1_oracle::sensor::SensorPhaseUpdate,
    read: &dyn Fn(&str) -> Result<String, String>,
) -> Vec<String> {
    let num = |s: &str| s.strip_prefix("0x").map_or_else(|| s.parse::<u32>().ok(), |h| u32::from_str_radix(h, 16).ok());
    let mut out = Vec::new();
    for line in table.lines().map(|l| l.split('#').next().unwrap_or("").trim()).filter(|l| !l.is_empty()) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let Some(a) = f.first().and_then(|i| i.parse::<u16>().ok()).and_then(|i| activities.get_mut(&i)) else { continue };
        if f.len() != 4 && f.len() != 5 {
            out.push(format!("⚠ activity {}: world table line {line:?} not used (want `index world group list [start]`)", a.index));
            continue;
        }
        if f[1] != a.world {
            out.push(format!("⚠ activity {}: the world table serves {:?}, the bridge {:?}; its own list stands", a.index, f[1], a.world));
            continue;
        }
        let Some(group) = num(f[2]) else {
            out.push(format!("⚠ activity {}: world table group {:?} unreadable", a.index, f[2]));
            continue;
        };
        let start = match f.get(4) {
            None => None,
            Some(s) => match num(s) {
                Some(r) => Some(r),
                None => {
                    out.push(format!("⚠ activity {}: world table start {s:?} unreadable", a.index));
                    continue;
                }
            },
        };
        let result = read(f[3]).and_then(|text| a.merge_world_list(base, group, &text, start));
        out.push(match result {
            Ok((world, ours, valued)) => format!(
                "⭐ script bridge: activity {} ({}) serves the world table's list {}: {world} rows ({valued} with values) \
                 and {ours} only the bridge's list names, lifetime starting at {}",
                a.index, a.activity, f[3], a.initial_lifetime()
            ),
            Err(e) => format!("⚠ activity {}: the world table's list {} not used: {e}; the bridge's own list stands", a.index, f[3]),
        });
    }
    out
}

impl std::fmt::Debug for ScriptConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut ids: Vec<_> = self.activities.keys().collect();
        ids.sort();
        f.debug_struct("ScriptConfig").field("sidecar", &self.sidecar).field("activities", &ids).finish_non_exhaustive()
    }
}

impl ScriptConfig {
    /// The oracle codec on its own thread.
    /// The host's wire codec, as the listener chooses it: stream B's native one under
    /// `D1_RSAT_NATIVE=1` (the client's own for schemas it does not cover), else the client's own.
    pub fn host_codec() -> CodecMaker {
        Arc::new(|| {
            let codec: Arc<dyn crate::codec::AhWireCodec> = if crate::native::enabled() {
                Arc::new(crate::native::NativeCodec)
            } else {
                Arc::new(crate::codec::SchemaCodec)
            };
            CodecWorker::spawn(codec)
        })
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
        // The generated world tables (stream E) and their slice states: `@generated` lists, the
        // phase regions and the slice -> bundle table.
        let worlds_dir = std::env::var("D1_AH_WORLDS").unwrap_or_else(|_| "ps4/fixtures/56578/worlds".into());
        let worlds = match crate::generated::GeneratedWorlds::load(std::path::Path::new(&worlds_dir)) {
            Ok(mut w) => {
                let states = std::env::var("D1_AH_SCRIPTS_SLICE_STATES")
                    .unwrap_or_else(|_| "ps4/fixtures/56578/scripts/slice-states.tsv".into());
                match std::fs::read_to_string(&states) {
                    Ok(t) => {
                        let n = w.load_slice_states(&t).map_err(|e| format!("{states}: {e:#}"))?;
                        tracing::info!("script bridge: {n} slice states from {states}");
                    }
                    Err(e) => tracing::warn!("script bridge: no slice states ({states}: {e}); phases take their regions from the table only"),
                }
                Some(w)
            }
            Err(e) => {
                tracing::warn!("script bridge: no generated world tables ({worlds_dir}: {e:#}); `@generated` lists are refused");
                None
            }
        };
        let bundles_dir = std::env::var("D1_BUNDLES_DIR").ok();
        let mut activities = HashMap::new();
        let mut cache = SliceRowsCache::new();
        for line in text.lines().map(|l| l.split('#').next().unwrap_or("").trim()).filter(|l| !l.is_empty()) {
            let mut a = ScriptedActivity::parse_cached(line, base, &read, worlds.as_ref(), &mut cache)
                .map_err(|e| format!("D1_AH_SCRIPTS_ACTIVITIES: {e}"))?;
            if a.bundles_path.is_none() {
                // The private content export (tools/script-host/tools/export-bundles.mjs).
                a.bundles_path = bundles_dir
                    .as_ref()
                    .map(|d| std::path::Path::new(d).join(format!("{}.json", a.world)))
                    .filter(|p| p.exists())
                    .map(|p| p.display().to_string());
            }
            activities.insert(a.index, a);
        }
        // The hand world table (`D1_AH_WORLD_TABLE`, rows that win over the generated worlds): a
        // scripted activity it names starts from the world's own list, as the world table serves
        // it without the bridge (run fg3b, 6 November: the live list, @phase 0, plays).
        if let Some(path) = std::env::var("D1_AH_WORLD_TABLE").ok().filter(|p| !p.trim().is_empty()) {
            let table = std::fs::read_to_string(&path).map_err(|e| format!("D1_AH_WORLD_TABLE {path}: {e}"))?;
            let read = |entry: &str| {
                let p = crate::sensor_list::listed_path(&path, entry);
                std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))
            };
            for line in apply_world_table(&mut activities, &table, base, &read) {
                if line.starts_with('⚠') {
                    tracing::warn!("script bridge: {line}");
                } else {
                    tracing::info!("{line}");
                }
            }
        }
        // The step schedule's slice lists (`D1_AH_SCRIPTS_SLICE_LISTS`, else `D1_AH_MISSION_STEPS`):
        // the served values of the records a slice set adds (run fg2d: the Tower cinematic /6/1).
        if let Some(path) = ["D1_AH_SCRIPTS_SLICE_LISTS", "D1_AH_MISSION_STEPS"]
            .iter()
            .find_map(|k| std::env::var(k).ok().filter(|p| !p.trim().is_empty()))
        {
            let steps = std::fs::read_to_string(&path).map_err(|e| format!("D1_AH_SCRIPTS_SLICE_LISTS {path}: {e}"))?;
            let read = |entry: &str| {
                let p = crate::sensor_list::listed_path(&path, entry);
                std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))
            };
            for line in apply_slice_lists(&mut activities, &steps, base, &read) {
                if line.starts_with('⚠') {
                    tracing::warn!("script bridge: {line}");
                } else {
                    tracing::info!("{line}");
                }
            }
        }
        // `D1_AH_SCRIPTS_PUBLIC_START_LIST=1`: a story mission that starts in a public slice set is
        // served the live mission host's list (after the values above are in, which it keeps).
        if std::env::var("D1_AH_SCRIPTS_PUBLIC_START_LIST").is_ok_and(|v| v.trim() == "1") {
            if let Some(w) = worlds.as_ref() {
                apply_public_start_list(&mut activities, w, base);
            } else {
                tracing::warn!("script bridge: D1_AH_SCRIPTS_PUBLIC_START_LIST needs the generated world tables; ignored");
            }
        }
        let world_loot = match std::env::var("D1_AH_SCRIPTS_WORLD_LOOT").ok().filter(|p| !p.trim().is_empty()) {
            Some(path) => {
                let text = std::fs::read_to_string(&path).map_err(|e| format!("D1_AH_SCRIPTS_WORLD_LOOT {path}: {e}"))?;
                let drops = WorldLoot::parse_table(&text)?;
                tracing::info!("script bridge: {} world-loot drops from {path} (D1_AH_SCRIPTS_WORLD_LOOT)", drops.len());
                drops
            }
            None => Vec::new(),
        };
        let device_rules = match std::env::var("D1_AH_SCRIPTS_DEVICE_RULES").ok().filter(|p| !p.trim().is_empty()) {
            Some(path) => {
                let text = std::fs::read_to_string(&path).map_err(|e| format!("D1_AH_SCRIPTS_DEVICE_RULES {path}: {e}"))?;
                let rules = DeviceRule::parse_table(&text)?;
                tracing::info!("script bridge: {} device rules from {path} (D1_AH_SCRIPTS_DEVICE_RULES)", rules.len());
                rules
            }
            None => Vec::new(),
        };
        let start_delay = start_delay_from(std::env::var("D1_AH_SCRIPTS_START_DELAY").ok().as_deref())?;
        let arrival_cushion = arrival_cushion_from(std::env::var("D1_AH_SCRIPTS_ARRIVAL_CUSHION").ok().as_deref())?;
        let list_retry = list_retry_from(std::env::var("D1_AH_SCRIPTS_LIST_RETRY").ok().as_deref())?;
        tracing::info!(
            "script bridge: the scripts start {:.0} s after the first full list (D1_AH_SCRIPTS_START_DELAY; \
             ⚠ a stand-in cushion: the client sends no 'loaded' signal on the wire)",
            start_delay.as_secs_f64()
        );
        let catalogue = std::env::var("D1_AH_SCRIPTS_INCIDENTS")
            .unwrap_or_else(|_| "ps4/fixtures/56578/incidents-56578.tsv".into());
        let incidents = IncidentCatalog::load(std::path::Path::new(&catalogue)).map_err(|e| {
            format!("D1_AH_SCRIPTS_INCIDENTS: {e} (the 56578 incident catalogue, PR #306)")
        })?;
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
            incidents_path: Some(catalogue),
            mission_phase: on("D1_AH_SCRIPTS_MISSION_PHASE"),
            send_unlisted: on("D1_AH_ROWS_UNLISTED"),
            delta_init: on("D1_AH_ROWS_DELTA_INIT"),
            scoreboard_columns,
            rewards: Arc::new(Mutex::new(LoggedRewards)),
            codec: Self::host_codec(),
            active_bundles: !std::env::var("D1_AH_SCRIPTS_ACTIVE_BUNDLES").is_ok_and(|v| v.trim() == "0"),
            start_delay,
            arrival_cushion,
            list_retry,
            world_loot,
            device_rules,
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
/// Ticks the sidecar may be behind (20 s at 10 Hz) before the clock waits for it.
const MAX_TICKS_OUT: u64 = 200;

struct Run {
    activity: ScriptedActivity,
    link: Option<Box<dyn SidecarLink>>,
    attempts: u32,
    retry_at: Option<Instant>,
    /// The sidecar answered `hello` with `loaded`.
    loaded: bool,
    /// Its rows are stand-ins (no content export): they never go on the wire.
    stand_in: bool,
    started: bool,
    halted: Option<String>,
    /// The last 10 Hz stamp sent as a `tick`, and how many ticks have no `tick_done` yet.
    last_tick: Option<u32>,
    ticks_out: u64,
    lifetime_state: i8,
    mission_phase: Option<MissionPhase>,
    complete: bool,
    ended: bool,
    rewarded: bool,
    current_slice: Option<u32>,
    /// F42: world-loot rows already announced in this run.
    loot_announced: std::collections::BTreeSet<RowKey>,
    /// F49: the loot cache's special weapon was granted in this run.
    special_weapon_granted: bool,
    /// `D1_AH_SCRIPTS_SCOREBOARD`: kills and deaths the client reported this run (precision kills
    /// are not counted: no client incident for them is known).
    score_kills: u32,
    score_deaths: u32,
    /// The reports were not attributable to one player (logged once).
    score_unattributed_logged: bool,
    /// The scoreboard row was set at this time (the lifetime-7 spacing warning reads it).
    score_sent_at: Option<Instant>,
    /// F52: device armings due (when, object), and the props already armed.
    device_arming: Vec<(Instant, RowKey)>,
    devices_armed: std::collections::HashSet<RowKey>,
    /// `D1_AH_SCRIPTS_INTERACTABLE`: the activity flags the scripts hold raised, in raise order.
    activity_flags: Vec<u32>,
    unsupported: std::collections::HashSet<String>,
    /// The lifetime changed and no frame has carried it yet.
    lifetime_unsent: bool,
    /// `D1_AH_SCRIPTS_START_PHASE` (lab): the lifetime's catch-up after a start at a later phase,
    /// one value per frame as a normal run goes 1, 4, 3: 3 = armed, waiting for the scripts' first
    /// lifetime value (or `ramp_armed_at` + 2 s), 1 = the hold (4) is next, 2 = gameplay (3).
    start_ramp: u8,
    ramp_armed_at: Option<Instant>,
    /// The ramp has finished: the scripts' initial lifetime (below 3) no longer applies.
    ramp_done: bool,
    ramp_swallow_logged: bool,
    /// The last lifetime value a frame carried, and that frame's stamp.
    lifetime_sent: Option<(i8, u32)>,
    /// The sidecar's lifetime numbering disagreed with the host's (logged once).
    wire_disagreed: bool,
    /// Sidecar messages waiting for the pending lifetime value to go out (see [`Self::holds`]).
    held: VecDeque<SidecarMsg>,
    /// The slice set whose bundles were last made active.
    active_slice: Option<u32>,
    /// The slice set whose sensor list the row table serves (the start's at the join).
    list_slice: Option<u32>,
    /// Public slice sets already logged as listed by the public host (`D1_AH_SCRIPTS_PUBLIC_START_LIST`).
    public_slice_logged: Vec<u32>,
    /// The slice set the latest mission phase resolved to (`D1_AH_SCRIPTS_PUBLIC_START_LIST`).
    phase_slice: Option<u32>,
    /// The public start slice set's relist went out (`D1_AH_SCRIPTS_PUBLIC_START_LIST`).
    start_relisted: bool,
    /// When the first full list went out: the start cushion counts from here without the udt 6
    /// gate ([`ScriptConfig::list_retry`] `None`).
    full_at: Option<Instant>,
    /// When the latest full list went out.
    last_full_at: Option<Instant>,
    /// When the client first answered a full list with a udt 6: with the gate, the start cushion
    /// counts from here (run fg2f).
    sensed_at: Option<Instant>,
    /// How many times the unanswered first full list went out again.
    list_retries: u32,
    /// The start is waiting for the cushion (logged once).
    start_held_logged: bool,
    /// Where the client is, or is going: its own last udt 21 report, else the last region the
    /// host moved it to, else the start slice set it loads at the join. A phase whose region is
    /// this one asks for no teleport (run fg2: a teleport to 25 while the client was still
    /// loading its initial 25 de-instantiated it, and the load never finished).
    believed_slice: Option<u32>,
    /// Client events before the scripts start, sent after `start` in order.
    before_start: VecDeque<HostMsg>,
    /// A phase moved the client to another slice set: the script clock is held until it arrives
    /// and the arrival cushion has passed (run fg2e).
    arrival: Option<Arrival>,
}

/// The script clock's hold after a phase's move (see [`ScriptConfig::arrival_cushion`]).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Arrival {
    /// Waiting for the client to reach `region`: its udt 21 report, else the host's teleport
    /// request finished ([`ScriptBridge::teleport_finished`]), else [`ARRIVAL_TIMEOUT`].
    Moving { region: u32, since: Instant },
    /// Arrived; the clock runs again at `until`.
    Cushion { region: u32, until: Instant },
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
                    && c.state.get("state").and_then(Value::as_i64).and_then(|s| lifetime_wire_state(s, self.activity.end))
                        .is_some_and(|s| s != self.lifetime_state)
            }),
            _ => false,
        }
    }
}

/// One instance's bridge. See the module doc.
pub struct ScriptBridge {
    config: Arc<ScriptConfig>,
    /// `D1_AH_SCRIPTS_START_PHASE` (lab): activity index -> the phase its scripts start at.
    start_phase: HashMap<u16, u32>,
    launch: Launcher,
    rows: RowTable,
    encoders: RowEncoders,
    codec: Option<CodecWorker>,
    in_flight: Option<FramePlan>,
    codec_retry: Option<Instant>,
    run: Option<Run>,
    players: Vec<CharacterRef>,
    /// Host udt 18 struct images to send (the completion envelope).
    incidents_out: VecDeque<Vec<u8>>,
    /// Completion chains whose body is being encoded, in submission order.
    bodies_pending: VecDeque<&'static [u32]>,
    region_out: Option<u32>,
    /// The start slice set the instance serves at the join (its membership's region record), set
    /// by the instance before it passes the join on ([`Self::set_served_start`]).
    served_start: Option<u32>,
    /// The activity the client launched, when `D1_AH_ACTIVITY_REMAP` served another in its place
    /// ([`Self::set_remapped_launch`]).
    remapped_launch: Option<u32>,
    /// The served world's bubble count, set by the instance before it passes the join on
    /// ([`Self::set_world_bubbles`]): the join grant gives the client blocks `0..bubbles` and 64.
    world_bubbles: Option<usize>,
    host_id: u64,
    pub frames_sent: u64,
    /// Stream O2: the scripts' phases, wipes and completion as activity-framework events, for the
    /// instance's `activity_driver::RunDriver` ([`Self::take_framework_events`]).
    framework_events: Vec<crate::encounter_framework::Event>,
    /// `D1_AH_SCRIPTS_SCOREBOARD=1`: the kind-15 scoreboard row goes out at the completion.
    scoreboard: bool,
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
            start_phase: start_phase_table(),
            launch,
            rows,
            encoders,
            codec: None,
            in_flight: None,
            codec_retry: None,
            run: None,
            players: Vec::new(),
            incidents_out: VecDeque::new(),
            bodies_pending: VecDeque::new(),
            region_out: None,
            served_start: None,
            remapped_launch: None,
            world_bubbles: None,
            host_id: 0,
            frames_sent: 0,
            framework_events: Vec::new(),
            scoreboard: scoreboard_knob(),
        }
    }

    /// Lab/tests: replaces the knob (`D1_AH_SCRIPTS_SCOREBOARD`).
    pub fn set_scoreboard(&mut self, on: bool) {
        self.scoreboard = on;
    }

    /// Lab/tests: replaces the knob (`D1_AH_SCRIPTS_INTERACTABLE`).
    pub fn set_interactable(&mut self, on: bool) {
        self.encoders.set_interactable(on);
    }

    /// The activity-framework events since the last call (stream O2): `phase` →
    /// `ScriptPhase(name)`, `all_dead` → `AllPlayersDead`, `activity_complete` →
    /// `ActivityCompleted`.
    pub fn take_framework_events(&mut self) -> Vec<crate::encounter_framework::Event> {
        std::mem::take(&mut self.framework_events)
    }

    /// Whether the activity now served is scripted (the bridge owns its udt 5 stream).
    pub fn scripted(&self) -> bool {
        self.run.is_some()
    }

    pub fn rows(&self) -> &RowTable {
        &self.rows
    }

    /// A host udt 18 struct image to send (the `activity_complete` envelope).
    pub fn take_incident(&mut self) -> Option<Vec<u8>> {
        self.incidents_out.pop_front()
    }

    /// A region the scripts' phase wants the client in (the host teleport).
    pub fn take_region_request(&mut self) -> Option<u32> {
        self.region_out.take()
    }

    /// Lab/tests: replaces the knob's table (`D1_AH_SCRIPTS_START_PHASE`).
    pub fn set_start_phase(&mut self, table: HashMap<u16, u32>) {
        self.start_phase = table;
    }

    /// The start slice set the instance serves this join (the region its membership publishes):
    /// the client loads it at the join, so a phase there needs no teleport. Call before passing
    /// the join on; `None` leaves the activity table's `start=`.
    pub fn set_served_start(&mut self, region: Option<u32>) {
        self.served_start = region;
    }

    /// ⚠ A LAB AFFORDANCE (run fg2i): the hash the client launched when the instance served another
    /// activity in its place (`D1_AH_ACTIVITY_REMAP=164:165`). The client's current activity is the
    /// launched one, and it chains to the next story activity only once that one shows completed,
    /// so `activity_complete` completes it too (the grant stays the served activity's). Call before
    /// passing the join on; `None` when no remap served this join.
    pub fn set_remapped_launch(&mut self, hash: Option<u32>) {
        self.remapped_launch = hash;
    }

    /// The served world's bubble count, for the join grant (see [`Self::world_bubbles`]).
    pub fn set_world_bubbles(&mut self, bubbles: Option<usize>) {
        self.world_bubbles = bubbles;
    }

    /// What the live mission host served a joining player that a list may lack (run fg6d, F32):
    /// - its player bundle `<world group>/0xC/5` naming the player's key, when the list gives that
    ///   row no value (A Guardian Rises' hand lists already do; generated worlds do not);
    /// - the join grant, blocks `0..bubbles` and 64 to the player's machine, seq 1, not applied,
    ///   when the list names no `@authority`.
    ///
    /// Returns the rows and the authority to serve.
    fn personalise(
        rows: &mut [(crate::rows::RowKey, crate::rows::Row)],
        authority: Option<d1_oracle::sensor::SensorAuthority>,
        world_group: u32,
        player: Option<&CharacterRef>,
        bubbles: Option<usize>,
    ) -> Option<d1_oracle::sensor::SensorAuthority> {
        use crate::row_encoders as re;
        let Some(p) = player else { return authority };
        if let Some(key) = re::player_key_of_identity(&p.identity) {
            for (k, r) in rows.iter_mut() {
                if k.group == world_group && k.kind == re::KIND_PLAYER_BUNDLE && k.index == re::MISSION_PLAYER_SUB && r.a.is_none() && r.has_a {
                    r.a = Some((re::TAG_PLAYER_BUNDLE, re::player_bundle_image(key, true)));
                    tracing::info!("⭐ script bridge: the player bundle {k} names the joining player (F32)");
                }
            }
        }
        if authority.is_some() {
            return authority;
        }
        let (Some(machine), Some(bubbles)) = (re::machine_of_identity(&p.identity), bubbles) else { return None };
        tracing::info!("⭐ script bridge: the join grant: authority blocks 0..{bubbles} and 64 to the player's machine, seq 1 (F32)");
        Some(d1_oracle::sensor::SensorAuthority::join_grant(machine, bubbles))
    }

    /// The instance's teleport request to `region` finished (`state.rs`: state 1, then 3, then
    /// dropped, on the host's own 20 s / 30 s timers; the client sends nothing when it arrives).
    /// Taken as the client's arrival when no udt 21 report came first.
    pub fn teleport_finished(&mut self, region: u32, now: Instant) {
        if matches!(self.run.as_ref().and_then(|r| r.arrival), Some(Arrival::Moving { region: r, .. }) if r == region) {
            self.arrived(region, now, "the host's teleport request finished");
        }
    }

    /// The client's udt 6 (`sensor_sense_update`), any payload: the first one after a full list
    /// is its answer to it, the closest "the client took the list" signal there is (fg3b 0.2 s
    /// after its first list, fg2e 11 s; fg2 and fg2f never, and neither started).
    pub fn client_sensed(&mut self, now: Instant) {
        let Some(run) = self.run.as_mut() else { return };
        if run.sensed_at.is_none() {
            run.sensed_at = Some(now);
            tracing::info!(
                "script bridge: the client answered the full list (udt 6, +{:.1} s after it){}",
                run.last_full_at.map_or(0.0, |t| now.duration_since(t).as_secs_f64()),
                if self.config.list_retry.is_some() { "; the start cushion counts from here" } else { "" }
            );
        }
    }

    /// Whether the client has answered a full list with a udt 6.
    pub fn client_answered(&self) -> bool {
        self.run.as_ref().is_some_and(|r| r.sensed_at.is_some())
    }

    /// The first full list unanswered for `list_retry`: it goes out again, with a fresh stamp.
    fn list_retry_due(&mut self, now: Instant) {
        let Some(every) = self.config.list_retry else { return };
        let delivered = self.rows.full_delivered();
        let Some(run) = self.run.as_mut() else { return };
        let Some(last) = run.last_full_at.filter(|_| run.sensed_at.is_none() && delivered) else { return };
        if now < last + every {
            return;
        }
        if run.list_retries < LIST_RETRIES {
            run.list_retries += 1;
            tracing::warn!(
                "script bridge: no udt 6 from the client {:.0} s after the full list; it goes out again \
                 (retry {}/{LIST_RETRIES}, D1_AH_SCRIPTS_LIST_RETRY, ⚠ a stand-in)",
                every.as_secs_f64(),
                run.list_retries
            );
            self.rows.request_full();
        } else {
            tracing::warn!(
                "script bridge: no udt 6 from the client after {LIST_RETRIES} re-sends; the start cushion counts from now"
            );
            run.sensed_at = Some(now);
        }
    }

    /// Whether the script clock is held for a phase's move.
    pub fn holding_for_arrival(&self) -> bool {
        self.run.as_ref().is_some_and(|r| r.arrival.is_some())
    }

    /// The client is taken to be in `region`: its full list goes out again with current values
    /// (a cinematic already marked playing is seen playing once the set has loaded), and the
    /// script clock runs again after the arrival cushion.
    fn arrived(&mut self, region: u32, now: Instant, how: &str) {
        let cushion = self.config.arrival_cushion;
        let Some(run) = self.run.as_mut() else { return };
        run.arrival = Some(Arrival::Cushion { region, until: now + cushion });
        tracing::info!(
            "script bridge: arrival in slice set {region} ({how}); its list goes out in full again, the script clock runs in {:.0} s",
            cushion.as_secs_f64()
        );
        self.rows.request_full();
        if self.run.as_ref().is_some_and(|r| r.started) {
            self.send_active_bundles(region);
        }
    }

    // ── inputs ────────────────────────────────────────────────────────────────────────────────

    /// Every client message, after the instance has handled it (`activity_index` is the activity it
    /// then serves, `host_id` this activity host's id).
    pub fn on_client_message(&mut self, msg: &ClientMessage, activity_index: u16, host_id: u64, now: Instant) {
        self.host_id = host_id;
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
                // Run fg2j: the served activity is the character's current one from the join, as
                // the live host's profile push at the join named it (completed 0).
                if let (Some(run), Some(id)) = (self.run.as_ref(), struct_image.get(JOIN_REQUEST_IDENTITY..JOIN_REQUEST_IDENTITY + IDENTITY_SIZE)) {
                    if run.activity.index == activity_index && run.activity.hash != 0 {
                        let (c, hash) = (CharacterRef { identity: id.to_vec() }, run.activity.hash);
                        if let Ok(mut rewards) = self.config.rewards.lock() {
                            if let Err(e) = rewards.activity_started(&c, hash) {
                                tracing::warn!("start of {hash:#010x} for {c:?}: {e}");
                            }
                        }
                    }
                }
            }
            ClientMessage::Other { user_defined_type: 18, struct_image } => self.on_incidents(struct_image),
            ClientMessage::ClientAuthoritativeDataUpdate { struct_image } => {
                let u = |o: usize| struct_image.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
                if let (Some(index), Some(hash)) = (u(0xB8), u(0xBC)) {
                    let region = index & 0x3FF;
                    if !matches!(hash, 0 | 0x811C_9DC5) && region != 0x3FF {
                        if let Some(run) = self.run.as_mut() {
                            run.current_slice = Some(region);
                            run.believed_slice = Some(region);
                        }
                        self.activate_slice(region, now);
                    } else if region != 0x3FF {
                        // A public start slice set reports the empty-name hash (run rst7): the knob's
                        // start relist still goes out; nothing else about the report changes.
                        self.relist_public_start(region);
                    }
                }
            }
            _ => {}
        }
    }

    /// `D1_AH_SCRIPTS_PUBLIC_START_LIST`: the client is in the activity's public START slice set:
    /// the live mission host's full relist (the start's globals and the shared rows), once per run.
    fn relist_public_start(&mut self, region: u32) {
        let Some(run) = self.run.as_mut() else { return };
        let start_list = run
            .activity
            .mission_list
            .as_ref()
            .filter(|m| m.start_relist.0 == region && !run.start_relisted)
            .map(|m| m.start_relist.1.clone());
        if let Some(mut list) = start_list {
            run.start_relisted = true;
            live_trigger_defaults(&mut list);
            tracing::info!(
                "⭐ script bridge: slice set {region} (the public start): mission host relist = the start's globals + private shared rows ({} rows) (D1_AH_SCRIPTS_PUBLIC_START_LIST)",
                list.len() + 1
            );
            let lifetime = run.activity.lifetime;
            self.rows.relist(list, &[lifetime]);
        }
    }

    /// The client entered slice set `region`: its bundles' threads run (the slice -> bundle table
    /// from the generated world tables). Sent once per change, and only once the scripts run.
    fn activate_slice(&mut self, region: u32, now: Instant) {
        self.relist_public_start(region);
        // The new slice set's own list (generated tables): the live host sent its full list after
        // the move, with the rows' current values.
        if let Some(run) = self.run.as_mut() {
            if run.list_slice.is_some() && run.list_slice != Some(region) {
                // The live mission host relisted only for a private slice set a mission phase names.
                let phase_named = run.phase_slice == Some(region);
                if run.activity.mission_list.is_some() && !phase_named && !run.public_slice_logged.contains(&region) {
                    run.public_slice_logged.push(region);
                    tracing::info!(
                        "script bridge: slice set {region}: no relist (public or not a phase's slice) (D1_AH_SCRIPTS_PUBLIC_START_LIST)"
                    );
                }
                let list = if run.activity.mission_list.is_some() && !phase_named { None } else { run.activity.slice_list(region) };
                if let Some(list) = list {
                    tracing::info!("script bridge: slice set {region}: its list ({} rows) goes out in full", list.len() + 1);
                    run.list_slice = Some(region);
                    let lifetime = run.activity.lifetime;
                    self.rows.relist(list, &[lifetime]);
                }
            }
        }
        if matches!(self.run.as_ref().and_then(|r| r.arrival), Some(Arrival::Moving { region: r, .. }) if r == region) {
            self.arrived(region, now, "the client reports it (udt 21)");
        }
        if self.run.as_ref().is_some_and(|r| r.started) {
            self.send_active_bundles(region);
        }
    }

    /// Make `region`'s bundles the active set (once per change). F44 (run agr24): BullShcript runs
    /// every bundle until it is told the active set, and our client's reports of its first slice
    /// sets (113, 112) carry no name hash, so none went out until the walk into 24. Every
    /// encounter ran from the start: the hangar's `sc_hangar_main` placed its squads while the
    /// client was on the highway, and they never appeared in the hangar. The scripts' start now
    /// sends the start slice set's bundles before `start`, and a teleport's arrival its own.
    fn send_active_bundles(&mut self, region: u32) {
        let on = self.config.active_bundles;
        let Some(run) = self.run.as_mut() else { return };
        if !on || run.active_slice == Some(region) {
            return;
        }
        let Some(groups) = run.activity.slice_groups.get(&region).filter(|g| !g.is_empty()) else { return };
        let bundles: Vec<u32> = groups.iter().map(|g| run.activity.bundle_of(*g)).collect();
        run.active_slice = Some(region);
        tracing::info!("script bridge: slice set {region} makes {} bundles active", bundles.len());
        self.send(HostMsg::ActiveBundles { bundles });
    }

    /// A decoded udt 6 update, in the instance's current epoch.
    pub fn on_sense(&mut self, counts: &[crate::sense::SquadCount], squads: &[SquadSense]) {
        let Some(run) = self.run.as_ref() else { return };
        let out: Vec<HostMsg> = counts
            .iter()
            .filter(|c| c.kind == 1 && c.live >= 0)
            .map(|c| HostMsg::SquadAlive { bundle: run.activity.bundle_of(c.group), type_index: c.index, count: c.live })
            .collect();
        // Protocol 1 has no message for these, and what they mean in play is open: logged only.
        for s in squads {
            tracing::info!("squad {:#010x}/0x1/{} sense state {} flag {}", s.group, s.index, s.state, s.flag);
        }
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
        let start = activity.start;
        let first = activity.initial_lifetime();
        let mut rows = activity.rows.clone();
        let armed = live_trigger_defaults(&mut rows);
        if armed > 0 {
            tracing::info!("⭐ script bridge: {armed} player triggers in the world's list go out enabled at arming 0, as live (F36)");
        }
        let authority = Self::personalise(
            &mut rows,
            activity.authority.clone(),
            activity.lifetime.group,
            self.players.last(),
            self.world_bubbles,
        );
        // F54: a Guardian who starts the activity unarmed is served unarmed (D1_AH_UNARMED_UNTIL_PHASE).
        if unarmed_until(activity_index).is_some() {
            for (k, r) in rows.iter_mut() {
                if k.group == activity.lifetime.group && k.kind == crate::row_encoders::KIND_PLAYER_BUNDLE
                    && k.index == crate::row_encoders::MISSION_PLAYER_SUB
                {
                    if let Some((_, image)) = r.a.as_mut() {
                        set_bundle_armed(image, false);
                        tracing::info!("⭐ script bridge: the player bundle {k} goes out unarmed (+8, +9 = 0) until the scripts arm it (F54)");
                    }
                }
            }
        }
        rows.push((activity.lifetime, self.lifetime_row(first, None)));
        self.rows.load_world(rows, authority);
        self.encoders.reset();
        self.in_flight = None;
        tracing::info!(
            "⭐ script bridge: activity {activity_index} is {} ({} rows, {} with values, lifetime {first}{}); starting its sidecar",
            activity.activity,
            activity.rows.len() + 1,
            activity.rows.iter().filter(|(_, r)| r.a.is_some() || r.b.is_some()).count() + 1,
            start.map(|s| format!(", start slice set {s}")).unwrap_or_default()
        );
        if activity.mission_list.is_some() {
            tracing::info!(
                "⭐ script bridge: {} starts public: mission host list = globals + private shared rows ({} rows) (D1_AH_SCRIPTS_PUBLIC_START_LIST)",
                activity.activity,
                activity.rows.len() + 1
            );
        }
        self.run = Some(Run {
            lifetime_state: first,
            activity,
            link: None,
            attempts: 0,
            retry_at: Some(now),
            loaded: false,
            stand_in: false,
            started: false,
            halted: None,
            last_tick: None,
            ticks_out: 0,
            mission_phase: None,
            complete: false,
            ended: false,
            rewarded: false,
            current_slice: None,
            loot_announced: Default::default(),
            special_weapon_granted: false,
            score_kills: 0,
            score_deaths: 0,
            score_unattributed_logged: false,
            score_sent_at: None,
            device_arming: Vec::new(),
            devices_armed: Default::default(),
            activity_flags: Vec::new(),
            unsupported: Default::default(),
            lifetime_unsent: false,
            start_ramp: 0,
            ramp_armed_at: None,
            ramp_done: false,
            ramp_swallow_logged: false,
            lifetime_sent: None,
            wire_disagreed: false,
            held: VecDeque::new(),
            active_slice: None,
            list_slice: start,
            public_slice_logged: Vec::new(),
            phase_slice: None,
            start_relisted: false,
            full_at: None,
            last_full_at: None,
            sensed_at: None,
            list_retries: 0,
            start_held_logged: false,
            believed_slice: self.served_start.or(start),
            before_start: VecDeque::new(),
            arrival: None,
        });
    }

    fn stop_run(&mut self, mut run: Run, _now: Instant) {
        if let Some(link) = run.link.as_mut() {
            let _ = link.send(&HostMsg::End);
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
        if self.scoreboard {
            self.count_reports(image, n);
        }
        for k in 0..n.min(8) as usize {
            let Some(t) = image.get(4 + 4 * k..8 + 4 * k).map(|b| u32::from_le_bytes(b.try_into().unwrap())) else { break };
            if t == known::TUTORIAL_WEAPON_SPECIAL.index {
                self.tutorial_special_weapon();
            }
            if t == known::PLAYER_TRIGGER.index {
                if let Some((group, kind, index)) = crate::state::trigger_report(image) {
                    self.devices_triggered(RowKey { group, kind, index: index as i16 });
                }
            }
            let msg = self.translate_incident(t, image);
            if let Some(m) = msg {
                self.send(m);
            }
        }
    }

    /// `D1_AH_SCRIPTS_SCOREBOARD`: one udt 18 chain's kills and deaths, by the Crucible driver's
    /// classification. The bridge cannot tell which player sent a report, so it is counted only
    /// with exactly one player joined. Precision kills are not counted (no client incident found).
    fn count_reports(&mut self, image: &[u8], n: u32) {
        use crate::pvp_driver::{classify, Report};
        let names: Vec<u32> = (0..n.min(8) as usize)
            .filter_map(|k| image.get(4 + 4 * k..8 + 4 * k))
            .filter_map(|b| self.config.incidents.name_hash(u32::from_le_bytes(b.try_into().unwrap())))
            .collect();
        let (mut kills, mut deaths) = (0, 0);
        for r in classify(&names) {
            match r {
                Report::Kill => kills += 1,
                Report::Died | Report::Suicide => deaths += 1,
                _ => {}
            }
        }
        if kills == 0 && deaths == 0 {
            return;
        }
        let joined = self.players.len();
        let Some(run) = self.run.as_mut() else { return };
        if joined != 1 {
            if !std::mem::replace(&mut run.score_unattributed_logged, true) {
                tracing::info!(
                    "script bridge: scoreboard: a kill/death report with {joined} players joined cannot be attributed; not counted (D1_AH_SCRIPTS_SCOREBOARD)"
                );
            }
            return;
        }
        run.score_kills += kills;
        run.score_deaths += deaths;
    }

    /// F49: the client opened the loot cache. Each player's class special weapon is granted once a
    /// run (the cache is one object; a second report is the same opening).
    fn tutorial_special_weapon(&mut self) {
        let Some(run) = self.run.as_mut() else { return };
        if std::mem::replace(&mut run.special_weapon_granted, true) {
            tracing::info!("script bridge: tutorial_weapon_special again: already granted this run");
            return;
        }
        tracing::info!("⭐ script bridge: tutorial_weapon_special: the loot cache opened; the special weapon is granted (F49)");
        let players = self.players.clone();
        if let Ok(mut rewards) = self.config.rewards.lock() {
            for c in &players {
                if let Err(e) = rewards.tutorial_special_weapon(c) {
                    tracing::warn!("special weapon for {c:?}: {e}");
                }
            }
        }
    }

    /// F52: a reported player trigger arms the props its rules name, `order` x 0.2 s apart.
    fn devices_triggered(&mut self, trigger: RowKey) {
        let Some(run) = self.run.as_mut() else { return };
        let now = Instant::now();
        let mut queued = 0;
        for r in self.config.device_rules.iter().filter(|r| r.activity == run.activity.index && r.trigger == trigger) {
            if !run.devices_armed.contains(&r.object) && !run.device_arming.iter().any(|(_, k)| *k == r.object) {
                run.device_arming.push((now + Duration::from_millis(200 * u64::from(r.order)), r.object));
                queued += 1;
            }
        }
        if queued > 0 {
            tracing::info!("⭐ script bridge: trigger {trigger} reported: {queued} props arm, 0.2 s apart, as the live host's tripwires (F52)");
        }
    }

    /// F52: the armings that are due: channel 2 (the position) goes to 1, animated.
    fn devices_due(&mut self, now: Instant) {
        let Some(run) = self.run.as_mut() else { return };
        let due: Vec<RowKey> = run.device_arming.iter().filter(|(t, _)| *t <= now).map(|(_, k)| *k).collect();
        if due.is_empty() {
            return;
        }
        run.device_arming.retain(|(t, _)| *t > now);
        for key in due {
            run.devices_armed.insert(key);
            let Some(mut row) = self.rows.get(&key).cloned() else { continue };
            let Some((_, image)) = row.a.as_mut() else {
                tracing::info!("script bridge: prop {key} armed before the scripts created it; it goes out armed when they do (F52)");
                continue;
            };
            if crate::row_encoders::set_device_command(image, crate::row_encoders::DEVICE_ARMED) {
                self.rows.set(key, row);
                tracing::info!("⭐ script bridge: prop {key} armed: device channel 2 = (1, 0, 1.0) (F52)");
            }
        }
    }

    fn translate_incident(&self, t: u32, image: &[u8]) -> Option<HostMsg> {
        let run = self.run.as_ref()?;
        let item = |(group, _kind, index): (u32, u8, u16)| (run.activity.bundle_of(group), index as i16);
        let key = self.players.first().map(player_key).unwrap_or_default();
        let named = crate::state::trigger_report(image);
        Some(match t {
            _ if t == known::PLAYER_TRIGGER.index => match named {
                Some(n) => {
                    let (bundle, type_index) = item(n);
                    HostMsg::Trigger { bundle, type_index }
                }
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
                let (bundle, type_index) = row;
                if t == known::CINEMATIC_FINISHED.index {
                    HostMsg::CinematicFinished { bundle, type_index }
                } else {
                    HostMsg::CinematicSkipped { bundle, type_index, key }
                }
            }
            _ if t == known::PLAYER_SPAWNED.index || t == known::PLAYER_SPAWNED_INITIAL.index => HostMsg::PlayerSpawned { key },
            _ if t == known::PLAYER_DIED.index => HostMsg::PlayerDied { key },
            _ => match self.config.incidents.name_hash(t) {
                Some(name_hash) => HostMsg::Incident { id: t, name_hash },
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
        // Client events before the scripts run wait for `start` (players and the start itself
        // open it): the scripts see nothing of the client before the start cushion has passed.
        if !run.started && !matches!(msg, HostMsg::Players { .. } | HostMsg::Start) {
            run.before_start.push_back(msg);
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
                        bundles_path: run.activity.bundles_path.clone(),
                        incidents_path: self.config.incidents_path.clone(),
                        tick_hz: TICK_HZ,
                        phase_index: self.start_phase.get(&run.activity.index).copied(),
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
        // D1_AH_SCRIPTS_START_PHASE (lab): the catch-up goes one value per frame, each once the last went out.
        // Armed at the phase message, which the real sidecar sends before the scripts' first
        // lifetime value (run gh61: 3 -> 1 undid the ramp): wait for that value, or 2 s.
        if run.start_ramp == 3 && run.ramp_armed_at.is_some_and(|t| now >= t + Duration::from_secs(2)) {
            run.start_ramp = 1;
        }
        if matches!(run.start_ramp, 1 | 2) && !run.lifetime_unsent {
            run.lifetime_state = if run.start_ramp == 1 { 4 } else { 3 };
            run.ramp_done = run.start_ramp == 2;
            run.start_ramp = if run.start_ramp == 1 { 2 } else { 0 };
            run.lifetime_unsent = true;
            self.set_lifetime();
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
                        run.loaded = false;
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
        // Start once the sidecar has loaded the scripts, the world's full list has gone out, and
        // the start cushion after it has passed (D1_AH_SCRIPTS_START_DELAY, a stand-in: the client
        // sends no "loaded" signal; run fg2 started at once, during the initial slice load, and
        // never left it). Until then the world's own list stands, lifetime 0, and heartbeats run.
        let delay = self.config.start_delay;
        let anchor = if self.config.list_retry.is_some() { run.sensed_at } else { run.full_at };
        let cushion_over = anchor.is_some_and(|t| now >= t + delay);
        if !run.started && run.loaded && self.rows.full_delivered() && !cushion_over && !run.start_held_logged {
            run.start_held_logged = true;
            tracing::info!(
                "script bridge: {} scripts loaded; their start waits {:.0} s after the {} \
                 (D1_AH_SCRIPTS_START_DELAY, ⚠ a stand-in cushion while the client loads its initial slice set)",
                run.activity.activity,
                delay.as_secs_f64(),
                if self.config.list_retry.is_some() { "client's first udt 6" } else { "full list" }
            );
        }
        if !run.started && run.loaded && self.rows.full_delivered() && cushion_over {
            run.started = true;
            run.last_tick = Some(self.rows.tick(now));
            tracing::info!(
                "⭐ script bridge: {} scripts start (+{:.0} s after the full list, +{:.0} s after the client's first udt 6)",
                run.activity.activity,
                run.full_at.map_or(0.0, |t| now.duration_since(t).as_secs_f64()),
                run.sensed_at.map_or(0.0, |t| now.duration_since(t).as_secs_f64())
            );
            let ready = run.sensed_at.is_some();
            let players = self
                .players
                .iter()
                .enumerate()
                .map(|(slot, c)| PlayerInfo { key: player_key(c), slot: slot as u32, ready })
                .collect();
            self.send(HostMsg::Players { players });
            // F44: the start slice set's bundles are the active set before the scripts start.
            if let Some(region) = self.run.as_ref().and_then(|r| r.current_slice.or(r.believed_slice)) {
                self.send_active_bundles(region);
            }
            self.send(HostMsg::Start);
            let waiting: Vec<HostMsg> = self.run.as_mut().map(|r| r.before_start.drain(..).collect()).unwrap_or_default();
            for m in waiting {
                self.send(m);
            }
            if let Some(region) = self.run.as_ref().and_then(|r| r.current_slice) {
                self.activate_slice(region, now);
            }
            return;
        }
        // The script clock: one `tick` per udt 5 stamp step (10 Hz), carrying the stamp, so a row's
        // recorded time is the stamp its frame goes out near. Polls are coarser than 100 ms, so each
        // sends the steps since the last; a sidecar 20 s behind holds the clock rather than flooding.
        let stamp = self.rows.tick(now);
        let Some(run) = self.run.as_ref() else { return };
        if !run.started || run.ended {
            return;
        }
        // The hold after a phase's move (run fg2e): no ticks, so the scripts' sleeps do not run
        // while the client loads the new slice set; the stamps it misses are skipped, not caught up.
        if let Some(Arrival::Moving { region, since }) = run.arrival {
            if now >= since + ARRIVAL_TIMEOUT {
                tracing::warn!("script bridge: no arrival in slice set {region} seen in {} s; taken as arrived", ARRIVAL_TIMEOUT.as_secs());
                self.arrived(region, now, "the arrival timeout");
            }
        }
        let Some(run) = self.run.as_mut() else { return };
        if let Some(Arrival::Cushion { region, until }) = run.arrival {
            if now >= until {
                tracing::info!("script bridge: the client is in slice set {region}; the script clock runs again");
                run.arrival = None;
            }
        }
        if run.arrival.is_some() {
            run.last_tick = Some(stamp);
            return;
        }
        let from = run.last_tick.map_or(stamp, |t| t + 1);
        for s in from..=stamp {
            let Some(run) = self.run.as_mut() else { return };
            if run.halted.is_some() {
                return;
            }
            if run.ticks_out >= MAX_TICKS_OUT {
                tracing::warn!("script bridge: the sidecar is {} ticks behind; holding the clock", run.ticks_out);
                return;
            }
            run.last_tick = Some(s);
            run.ticks_out += 1;
            self.send(HostMsg::Tick { now: u64::from(s) });
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
            SidecarMsg::Loaded { stand_in, stub, drives_lifetime, compile_errors } => {
                run.loaded = true;
                run.stand_in = stand_in;
                tracing::info!(
                    "script bridge: {} scripts loaded ({} compile errors{}{}{})",
                    run.activity.activity,
                    compile_errors.len(),
                    if stub { "; a start-only stub" } else { "" },
                    if drives_lifetime { "" } else { "; the scripts do not drive the lifetime" },
                    if stand_in { "; STAND-IN rows: only the lifetime goes on the wire" } else { "" },
                );
            }
            SidecarMsg::Ready { unresolved, .. } => {
                if !unresolved.is_empty() {
                    tracing::warn!("script bridge: {} has {} unresolved bindings", run.activity.activity, unresolved.len());
                }
            }
            SidecarMsg::Rows { changes } => {
                // Each lifetime state in its own frame, within one message too (pregame and the
                // cinematic hold set in one tick go out 1, then 4, as the live hosts sent them):
                // the changes after a lifetime change wait for its frame when they move it again.
                let mut rest = changes.into_iter();
                while let Some(c) = rest.next() {
                    let lifetime = c.kind == "lifetime";
                    self.apply_change(c, now);
                    let Some(run) = self.run.as_mut() else { return };
                    if lifetime && run.lifetime_unsent {
                        let left: Vec<ItemChange> = rest.collect();
                        if !left.is_empty() {
                            let m = SidecarMsg::Rows { changes: left };
                            if run.holds(&m) {
                                run.held.push_front(m);
                            } else if let SidecarMsg::Rows { changes } = m {
                                for c in changes {
                                    self.apply_change(c, now);
                                }
                            }
                        }
                        return;
                    }
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
            SidecarMsg::Phase { index, name, name_hash, slice_hash, region } => {
                let name_hash = name_hash
                    .as_ref()
                    .and_then(hash_value)
                    .unwrap_or_else(|| crate::incident_catalog::fnv1(name.as_deref().unwrap_or("")));
                let region = region.or_else(|| run.activity.region_of_phase(index, name_hash, slice_hash.as_ref().and_then(hash_value)));
                run.mission_phase = Some(MissionPhase {
                    index: index as i32,
                    name_hash,
                    bubble: region.map_or(-1, |r| (r >> 3) as i32),
                });
                tracing::info!(
                    "⭐ script bridge: {} mission phase {index} {:?} (region {region:?})",
                    run.activity.activity,
                    name.as_deref().unwrap_or("")
                );
                if let Some(n) = name.clone().filter(|n| !n.is_empty()) {
                    self.framework_events.push(crate::encounter_framework::Event::ScriptPhase(n));
                }
                // F54: the live A Guardian Rises host armed the player bundle (+8, +9 = 1) 3 s after
                // the rifle's checkpoint (527 s); our scripts' phase 3 is the rifle in hand.
                if unarmed_until(run.activity.index).is_some_and(|p| index as i32 >= p) {
                    let key = RowKey {
                        group: run.activity.lifetime.group,
                        kind: crate::row_encoders::KIND_PLAYER_BUNDLE,
                        index: crate::row_encoders::MISSION_PLAYER_SUB,
                    };
                    if let Some(mut row) = self.rows.get(&key).cloned() {
                        if let Some((_, image)) = row.a.as_mut() {
                            if !bundle_armed(image) {
                                set_bundle_armed(image, true);
                                self.rows.set(key, row);
                                tracing::info!("⭐ script bridge: phase {index}: the player bundle {key} is armed (+8, +9 = 1), as the live host after the rifle (F54)");
                            }
                        }
                    }
                }
                // F37: the live AGR host asked for a slice set only within the bubble the client
                // was in (113 -> 112 in bubble 14, 104 -> 105 in 13; the reveal's 25 -> 26 in 3).
                // The client walked into 24, 96 and 104 itself, and the host sent no request; the
                // list followed the client's own report (`activate_slice`). Run agr3's teleport
                // into 24 reloaded the world under the player.
                let here = run.current_slice.or(run.believed_slice);
                let walked = region.zip(here).filter(|&(r, c)| r != c && r >> 3 != c >> 3);
                match region {
                    Some(r) if walked.is_some() => tracing::info!(
                        "script bridge: phase {index} runs in region {r}, another bubble than the client's slice set {}: \
                         the client walks there, so no teleport, and the list follows its report (F37)",
                        walked.map_or(0, |(_, c)| c)
                    ),
                    Some(r) if run.believed_slice == Some(r) => tracing::info!(
                        "script bridge: phase {index} runs in region {r}, where the client already is (or is loading); no teleport"
                    ),
                    Some(r) => {
                        run.believed_slice = Some(r);
                        self.region_out = Some(r);
                    }
                    None => {}
                }
                // The list follows the slice set the phase runs in: a move sends that set's full
                // list (the world's records and the set's own, current values) with the teleport,
                // as the step schedule's move list does, so a row the scripts set there (fg2c: the
                // Tower cinematic /6/1) is one the client holds, not one held back.
                if run.activity.mission_list.is_some() {
                    // The mission host's rule: the phase names the slice set; the list goes out
                    // when the client is in it (now, or at its move: `activate_slice`).
                    // A phase that names no slice set clears it: no relist until one does.
                    run.phase_slice = region;
                    if let Some(r) = region {
                        if run.current_slice == Some(r) && run.list_slice.is_some() && run.list_slice != Some(r) {
                            if let Some(list) = run.activity.slice_list(r) {
                                tracing::info!(
                                    "script bridge: phase {index} resolves to slice set {r}, where the client is: its list ({} rows) goes out in full",
                                    list.len() + 1
                                );
                                run.list_slice = Some(r);
                                let lifetime = run.activity.lifetime;
                                self.rows.relist(list, &[lifetime]);
                            }
                        }
                    }
                } else if let Some(r) = region.filter(|r| walked.is_none() && run.list_slice.is_some() && run.list_slice != Some(*r)) {
                    if let Some(list) = run.activity.slice_list(r) {
                        tracing::info!(
                            "script bridge: phase {index} moves the list to slice set {r} ({} rows, in full)",
                            list.len() + 1
                        );
                        run.list_slice = Some(r);
                        let lifetime = run.activity.lifetime;
                        self.rows.relist(list, &[lifetime]);
                    }
                }
                // Run fg2e: the scripts' next steps (the reveal's sleep(4), then the Tower play)
                // wait until the client is in the new slice set and the arrival cushion has passed.
                if self.region_out.is_some_and(|o| Some(o) == region) {
                    let r = self.region_out.expect("checked");
                    tracing::info!(
                        "script bridge: phase {index} moves the client to {r}; the script clock holds until it arrives, then {:.0} s \
                         (D1_AH_SCRIPTS_ARRIVAL_CUSHION, ⚠ a stand-in)",
                        self.config.arrival_cushion.as_secs_f64()
                    );
                    run.arrival = Some(Arrival::Moving { region: r, since: now });
                }
                // The cinematic hold ends with the phase that held it (F35). BullShcript puts the
                // lifetime in the hold when a phase's cinematic plays and never takes it out (only
                // a script's set_state does; A Guardian Rises' never sets active), so run agr1 sat
                // at 4 in the highway with no player. The live host went 4 → 3 as the intro phase
                // ended: cinematics finished 473.3 s and 522.8 s, active 527.9 s. A playable
                // activity whose new phase begins with no cinematic playing goes active; a
                // cinematic-only one (the reveal) keeps the hold to its end (4, 6, 8).
                // ⚠ The runtime itself still believes it is in the hold, so a later phase cinematic
                // is not re-held on the wire.
                if run.lifetime_state == 4
                    && run.activity.end == EndKind::Gameplay
                    && self.encoders.playing_cinematics().is_empty()
                {
                    tracing::info!(
                        "⭐ script bridge: phase {index} begins with no cinematic playing: the cinematic hold ends, lifetime 4 -> 3 (F35)"
                    );
                    run.lifetime_state = 3;
                    run.lifetime_unsent = true;
                }
                if index == 0 {
                    // A script restart (a wipe, a new pass through phase 0): the initial state applies again.
                    run.ramp_done = false;
                    run.ramp_swallow_logged = false;
                    run.start_ramp = 0;
                }
                // D1_AH_SCRIPTS_START_PHASE (lab): the skipped phases never set the lifetime to the
                // hold or released it (F35), so the client sat in pregame with no spawn (run gh60).
                // The first phase of a later start brings it there as a normal run does: 4, then 3.
                if index > 0
                    && self.start_phase.get(&run.activity.index) == Some(&index)
                    && run.start_ramp == 0
                    && run.lifetime_state < 3
                    && run.activity.end == EndKind::Gameplay
                {
                    tracing::info!(
                        "⭐ script bridge: {} starts at phase {index}: lifetime brought to gameplay as the skipped phases would have (D1_AH_SCRIPTS_START_PHASE, lab)",
                        run.activity.activity
                    );
                    run.start_ramp = 3;
                    run.ramp_armed_at = Some(now);
                }
                self.set_lifetime();
            }
            SidecarMsg::Event { bundle, event_id, state, reason } => {
                tracing::info!(
                    "script bridge: public event {event_id:?} of bundle {bundle:#010x}: {state}{}",
                    reason.map(|r| format!(" ({r})")).unwrap_or_default()
                );
            }
            SidecarMsg::ActivityFlag { hash, active } if self.encoders.interactable() => {
                // The flag is a list of raw u32 name hashes in the player bundle (+0x4C count,
                // +0x50 hashes, capacity 8). In one live capture the host set it in every slot row at
                // once, around the objective's start, and cleared it at its completion: here it follows
                // the scripts' own order, with no delay added.
                if active {
                    if !run.activity_flags.contains(&hash) {
                        run.activity_flags.push(hash);
                    }
                } else {
                    run.activity_flags.retain(|h| *h != hash);
                }
                use crate::row_encoders as re;
                let group = run.activity.lifetime.group;
                for index in re::MISSION_PLAYER_SUB..re::MISSION_PLAYER_SUB + 16 {
                    let key = RowKey { group, kind: re::KIND_PLAYER_BUNDLE, index };
                    let Some(mut row) = self.rows.get(&key).cloned() else { continue };
                    let Some((tag, image)) = row.a.as_mut() else { continue };
                    if *tag != re::TAG_PLAYER_BUNDLE {
                        continue;
                    }
                    if let Some(n) = re::set_player_flags(image, &run.activity_flags) {
                        if n < run.activity_flags.len() {
                            tracing::warn!("script bridge: {} activity flags held; the player rows hold {n}", run.activity_flags.len());
                        }
                        self.rows.set(key, row);
                    }
                }
                tracing::info!(
                    "⭐ script bridge: activity flag {hash:#010x} {}: {} in the player rows (D1_AH_SCRIPTS_INTERACTABLE)",
                    if active { "set" } else { "cleared" },
                    run.activity_flags.len(),
                );
            }
            SidecarMsg::ActivityFlag { hash, active } => {
                tracing::warn!("script bridge: activity flag {hash:#010x} = {active} has no 56578 encoding yet; not sent");
            }
            SidecarMsg::ActivityComplete {} => {
                run.complete = true;
                if self.scoreboard {
                    Self::send_scoreboard(&mut self.rows, run, &self.players, now);
                }
                self.framework_events.push(crate::encounter_framework::Event::ActivityCompleted);
                // Run fg2h: the profile's completion goes now, in the 965 chain's tick, as the
                // live host's did; the grant waits for the results.
                // ⚠ Lab only: a remap served this activity for another the client launched; the
                // client's current activity is that one, so it is completed too (run fg2i).
                let stood_in = self.remapped_launch.filter(|h| *h != 0 && *h != run.activity.hash);
                if let Some(h) = stood_in {
                    tracing::warn!(
                        "⚠ script bridge: the remap stood in for {h:#010x} (D1_AH_ACTIVITY_REMAP): completing it too, \
                         aside (lab only; field [6] and the grant stay {}'s)",
                        run.activity.activity
                    );
                }
                if let Ok(mut rewards) = self.config.rewards.lock() {
                    for c in &self.players {
                        if run.activity.hash != 0 {
                            if let Err(e) = rewards.activity_completed(c, run.activity.hash) {
                                tracing::warn!("completion of {:#010x} for {c:?}: {e}", run.activity.hash);
                            }
                        }
                        // Aside (run fg2j): the profile's current activity stays the served one.
                        if let Some(h) = stood_in {
                            if let Err(e) = rewards.activity_completed_aside(c, h) {
                                tracing::warn!("completion aside of {h:#010x} for {c:?}: {e}");
                            }
                        }
                    }
                }
                let player = self.players.first().map(|c| {
                    u64::from_le_bytes(c.identity.get(..8).and_then(|b| b.try_into().ok()).unwrap_or([0; 8]))
                });
                match (run.activity.hashes, player) {
                    (Some(hashes), Some(player)) => {
                        let body = CompletionBody {
                            // A stand-in: the live word is per instance and consecutive per member.
                            instance_word: self.host_id as u32,
                            host: self.host_id,
                            player,
                            hashes,
                            end: run.activity.end,
                        };
                        tracing::info!(
                            "⭐ script bridge: {} complete; host incidents {:?} with one 0x80800595 body",
                            run.activity.activity,
                            crate::completion::chain(run.activity.end)
                        );
                        self.bodies_pending.push_back(crate::completion::chain(run.activity.end));
                        let image = body.image();
                        let codec = self.codec.get_or_insert_with(|| (self.config.codec)());
                        codec.submit(CodecJob::Object { tag: crate::completion::BODY_TAG, image });
                    }
                    (None, _) => tracing::warn!(
                        "** NAMED GATE ** script bridge: the scripts completed {}, but incident 965 is not \
                         sent: its body needs the activity's manifest hashes (hash=, type=, type2=, place=)",
                        run.activity.activity
                    ),
                    (_, None) => tracing::warn!(
                        "** NAMED GATE ** script bridge: {} completed with no joined player; incident 965 not sent",
                        run.activity.activity
                    ),
                }
            }
            SidecarMsg::Ended {} => {
                run.ended = true;
                let outcome = if run.complete { ActivityOutcome::Completed } else { ActivityOutcome::EndedIncomplete };
                tracing::info!("⭐ script bridge: {} ended ({outcome:?})", run.activity.activity);
                // Granted at the results (6) already, unless the run ended without them.
                let mut run = self.run.take().expect("checked");
                self.reward(&mut run, outcome);
                // Run fg2j: a cinematic-only activity's 8 ends it for the client; the live host
                // then reset the profile's current activity (`ffff`).
                if run.activity.end == EndKind::Cinematic && run.activity.hash != 0 {
                    if let Ok(mut rewards) = self.config.rewards.lock() {
                        for c in &self.players {
                            if let Err(e) = rewards.activity_ended(c, run.activity.hash) {
                                tracing::warn!("end of {} for {c:?}: {e}", run.activity.activity);
                            }
                        }
                    }
                }
                self.run = Some(run);
            }
            SidecarMsg::HardWipeArmed {} => {
                tracing::warn!("script bridge: the scripts asked for a hard wipe, which has no 56578 handshake yet");
            }
            SidecarMsg::AllDead {} => {
                tracing::warn!("script bridge: the scripts report every player dead (the activity framework decides the wipe; no 56578 handshake yet)");
                self.framework_events.push(crate::encounter_framework::Event::AllPlayersDead);
            }
            SidecarMsg::TickDone { .. } => run.ticks_out = run.ticks_out.saturating_sub(1),
            SidecarMsg::Log { text } => tracing::info!("sidecar: {text}"),
            SidecarMsg::Error { text, fatal } => {
                if fatal {
                    Self::halt(run, format!("the sidecar refused the session: {text}"));
                } else {
                    tracing::warn!("sidecar error: {text}");
                }
            }
            SidecarMsg::Done {} | SidecarMsg::Snapshot {} | SidecarMsg::Bye {} => {}
        }
    }

    /// `D1_AH_SCRIPTS_SCOREBOARD`: the world's kind-15 scoreboard row (A), one player row per joined
    /// player with this run's kills and deaths. Set once at the completion, before the completion
    /// incidents and the lifetime-7 change (sense49: it must reach the client at least one sensor
    /// resend, ~10 s, before the 7); kept set afterwards so a later full list carries it.
    fn send_scoreboard(rows: &mut RowTable, run: &mut Run, players: &[CharacterRef], now: Instant) {
        use crate::pvp::{fnv1, PlayerStatsEntry, ScoreboardState, StatEntry};
        if run.lifetime_state >= 7 {
            tracing::warn!("script bridge: scoreboard not sent: the lifetime is already {} (D1_AH_SCRIPTS_SCOREBOARD)", run.lifetime_state);
            return;
        }
        // The activity's own sensor group (its lifetime row's): 0x4786C0E0 for most activities,
        // not all of them (worlds/activities.tsv).
        let key = RowKey { group: run.activity.lifetime.group, kind: crate::row_encoders::KIND_SCOREBOARD, index: 0 };
        let Some(held) = rows.get(&key).cloned() else {
            tracing::warn!("script bridge: scoreboard not sent: the world's list has no row {key} (D1_AH_SCRIPTS_SCOREBOARD)");
            return;
        };
        let (kills, deaths) = (fnv1("kills"), fnv1("deaths"));
        let state = ScoreboardState {
            global: Vec::new(),
            notifications: Vec::new(),
            players: players
                .iter()
                .map(|c| PlayerStatsEntry {
                    character_key: player_key(c),
                    stats: vec![
                        StatEntry { name_hash: kills, value: f64::from(run.score_kills) },
                        StatEntry { name_hash: deaths, value: f64::from(run.score_deaths) },
                    ],
                    team: None,
                })
                .collect(),
            teams: Vec::new(),
        };
        let by_key: HashMap<String, [u8; 8]> =
            players.iter().filter_map(|c| Some((player_key(c), crate::scoreboard_rows::native_identity(&c.identity)?))).collect();
        let image = crate::scoreboard_rows::image(&state, &crate::scoreboard_rows::Columns::default(), &|k| by_key.get(k).copied());
        rows.set(key, Row { has_a: true, has_b: held.has_b, a: Some((crate::scoreboard_rows::TAG, image)), b: held.b });
        // The table keeps the held row's shape: a list that names the row without an A value
        // would drop the image, so say so instead of claiming a send.
        if rows.get(&key).is_none_or(|r| r.a.is_none()) {
            tracing::warn!("script bridge: scoreboard not sent: the world's list names row {key} without an A value (D1_AH_SCRIPTS_SCOREBOARD)");
            return;
        }
        run.score_sent_at = Some(now);
        tracing::info!(
            "⭐ script bridge: scoreboard: {} player(s), kills {}, deaths {} (D1_AH_SCRIPTS_SCOREBOARD; precision kills not counted)",
            players.len(),
            run.score_kills,
            run.score_deaths
        );
    }

    fn apply_change(&mut self, c: ItemChange, now: Instant) {
        let Some(run) = self.run.as_mut() else { return };
        if c.kind == "lifetime" {
            let state = c.state.get("state").and_then(Value::as_i64).unwrap_or(-1);
            let ours = lifetime_wire_state(state, run.activity.end);
            if let Some(wire) = c.wire.as_ref().and_then(|w| w.get("phase")) {
                if wire.as_i64() != ours.map(i64::from) && !run.wire_disagreed {
                    run.wire_disagreed = true;
                    tracing::warn!(
                        "script bridge: the sidecar maps lifetime {state} to {wire}, the host to {ours:?} \
                         ({:?} activity; notes/ps4-56578-wire-catalog-2026-11-05.md); the host's stands",
                        run.activity.end
                    );
                }
            }
            match ours {
                // After the ramp, and while it steps, the scripts' initial state (below 3) does not
                // drag the lifetime back; the next phase 0 (a restart) clears this.
                Some(s) if (run.ramp_done || matches!(run.start_ramp, 1 | 2)) && s < 3 => {
                    if run.ramp_swallow_logged {
                        tracing::debug!("script bridge: lifetime {s} (script state {state}) skipped by the start phase; it stays {}", run.lifetime_state);
                    } else {
                        run.ramp_swallow_logged = true;
                        tracing::info!(
                            "script bridge: lifetime {s} (script state {state}) is the scripts' initial state, skipped by the start phase; it stays {} (D1_AH_SCRIPTS_START_PHASE, lab)",
                            run.lifetime_state
                        );
                    }
                }
                Some(s) => {
                    if matches!(run.start_ramp, 1 | 2) {
                        // A genuine scripted value (3 or more) mid-ramp ends the ramp and applies.
                        run.start_ramp = 0;
                        run.ramp_done = true;
                    }
                    if s != run.lifetime_state {
                        tracing::info!("⭐ script bridge: lifetime {} -> {s} (script state {state})", run.lifetime_state);
                    }
                    if run.start_ramp == 3 {
                        run.start_ramp = 1; // the scripts' first value is in: the ramp follows it
                    }
                    if s != run.lifetime_state {
                        run.lifetime_unsent = true;
                    }
                    run.lifetime_state = s;
                    if let Some(at) = run.score_sent_at.filter(|_| s == 7) {
                        let secs = now.saturating_duration_since(at).as_secs_f32();
                        if secs < 10.0 {
                            tracing::warn!("script bridge: scoreboard sent {secs:.1} s before lifetime 7 (needs >= ~10 s, sense49)");
                        }
                    }
                    self.set_lifetime();
                    if s == 6 {
                        // Rewards at the results: the post-game (7) shows what the profile holds.
                        let mut run = self.run.take().expect("checked");
                        let outcome = if run.complete { ActivityOutcome::Completed } else { ActivityOutcome::EndedIncomplete };
                        self.reward(&mut run, outcome);
                        self.run = Some(run);
                    }
                }
                None => tracing::warn!("script bridge: lifetime state {state} has no 56578 value; not sent"),
            }
            return;
        }
        if run.stand_in {
            if run.unsupported.insert(format!("stand-in {}", c.kind)) {
                tracing::warn!("script bridge: {} rows are stand-ins (no content export); not sent", c.kind);
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
        // The base image a change builds on: the row's served value (the world's list, the table's
        // current value), else the step schedule's value for the record, else the bare row. The
        // scripts change only the fields they own (a cinematic's start, counter and playing).
        let held = self.rows.get(&key).cloned();
        let base = held
            .clone()
            .filter(|r| r.a.is_some() || r.b.is_some())
            .or_else(|| run.activity.served_value(&key))
            .or(held);
        match self.encoders.encode(key, &c.kind, &c.state, base.as_ref(), ctx, &map) {
            Encoded::Row(mut row) => {
                // F52: a rule's prop carries the live host's device command (at rest until armed).
                if key.kind == crate::row_encoders::KIND_OBJECT {
                    let run = self.run.as_ref().expect("checked");
                    if self.config.device_rules.iter().any(|r| r.activity == run.activity.index && r.object == key) {
                        let armed = run.devices_armed.contains(&key);
                        if let Some((_, image)) = row.a.as_mut() {
                            crate::row_encoders::set_device_command(
                                image,
                                if armed { crate::row_encoders::DEVICE_ARMED } else { crate::row_encoders::DEVICE_AT_REST },
                            );
                        }
                    }
                }
                let created = key.kind == crate::row_encoders::KIND_OBJECT
                    && row.a.as_ref().is_some_and(|(_, image)| image.get(8) == Some(&1));
                let objective = (key.kind == crate::row_encoders::KIND_SQUAD).then(|| squad_objective(&row)).flatten();
                self.rows.set(key, row);
                if let Some(o) = objective {
                    self.objective_due(o);
                }
                if created {
                    self.world_loot_due(key);
                }
            }
            Encoded::Unsupported(why) => {
                let run = self.run.as_mut().expect("checked");
                if run.unsupported.insert(c.kind.clone()) {
                    tracing::warn!("script bridge: {} row {key} not sent: {why}", c.kind);
                }
            }
        }
    }

    /// F46 (run agr30): a placed squad names its objective at `+0` (a kind-3 row of its area). The
    /// live host gave that row a value (28 zero bytes, `0x808004FA`) when the area's squads were
    /// first placed; ours never did, and the hangar's placed squads never got a member. A named
    /// objective with no value yet gets the live one.
    fn objective_due(&mut self, key: RowKey) {
        if self.rows.get(&key).is_some_and(|r| r.a.is_some()) {
            return;
        }
        let has_b = self.rows.get(&key).is_some_and(|r| r.has_b);
        tracing::info!("script bridge: objective {key} named by a placed squad: its live value (28 zero bytes) goes out (F46)");
        self.rows.set(key, Row { has_a: true, has_b, a: Some((crate::row_encoders::TAG_OBJECTIVE, vec![0; crate::row_encoders::OBJECTIVE_SIZE])), b: None });
    }

    fn reward(&mut self, run: &mut Run, outcome: ActivityOutcome) {
        if run.rewarded {
            return;
        }
        run.rewarded = true;
        let Ok(mut rewards) = self.config.rewards.lock() else {
            tracing::warn!("rewards for {}: the rewards service lock is poisoned", run.activity.activity);
            return;
        };
        for c in &self.players {
            if let Err(e) = rewards.grant_activity_rewards(c, run.activity.hash, &outcome) {
                tracing::warn!("rewards for {c:?}: {e}");
            }
            // The arc: a completed activity hands off to the next one its row names.
            if let (ActivityOutcome::Completed, Some((index, hash))) = (outcome, run.activity.next) {
                tracing::info!("⭐ script bridge: {} hands off to activity {index} ({hash:#010x})", run.activity.activity);
                if let Err(e) = rewards.activity_hand_off(c, run.activity.hash, index, hash) {
                    tracing::warn!("hand-off for {c:?}: {e}");
                }
            }
        }
    }

    // ── udt 5 ─────────────────────────────────────────────────────────────────────────────────

    /// The next udt 5 frame for a scripted activity, if one is due: the full list, a delta, or a
    /// heartbeat. Also runs the sidecar (events in, the clock). Call it on every poll.
    pub fn sensor_due(&mut self, now: Instant) -> Option<Vec<u8>> {
        self.run.as_ref()?;
        self.devices_due(now);
        self.list_retry_due(now);
        self.drive(now);
        let mut frame = None;
        // Results come back in submission order: frames and completion bodies.
        while let Some(done) = self.codec.as_mut().and_then(CodecWorker::poll) {
            match done {
                Ok(CodecDone::Frame(r)) => {
                    if let Some(plan) = self.in_flight.take() {
                        frame = frame.or(self.finish(plan, r, now));
                    }
                }
                Ok(CodecDone::Object(r)) => self.body_done(r),
                Err(e) => {
                    if let Some(plan) = self.in_flight.take() {
                        self.finish(plan, Err(e.clone()), now);
                    }
                    for chain in self.bodies_pending.drain(..) {
                        tracing::warn!("incident chain {chain:?} not sent: {e}");
                    }
                    self.codec = None;
                    break;
                }
            }
        }
        if frame.is_some() {
            return frame;
        }
        let backing_off = self.codec_retry.is_some_and(|t| now < t);
        if self.in_flight.is_none() && !backing_off {
            if let Some(plan) = self.rows.plan() {
                let codec = self.codec.get_or_insert_with(|| (self.config.codec)());
                codec.submit(CodecJob::Frame(plan.clone()));
                self.in_flight = Some(plan);
                // An inline codec answers at once.
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

    /// A completion body came back from the codec: its envelope is ready to send.
    /// F42: the scripts created `key`; if it is a world-loot drop of this activity not yet
    /// announced in this run, announce it (incident 1851 with its `0x80800CB6` body, recipient =
    /// this host's id).
    fn world_loot_due(&mut self, key: RowKey) {
        let Some(run) = self.run.as_mut() else { return };
        let Some(drop) = self.config.world_loot.iter().find(|d| d.activity == run.activity.index && d.row == key).cloned() else {
            return;
        };
        if !run.loot_announced.insert(key) {
            return;
        }
        tracing::info!(
            "⭐ script bridge: {key} created: world-loot drop {{{}, {:#x}}} at {:?} in world {} (incident 1851, F42)",
            drop.key, drop.id, drop.position, drop.world
        );
        self.bodies_pending.push_back(WORLD_LOOT_CHAIN);
        let image = drop.image(self.host_id);
        let codec = self.codec.get_or_insert_with(|| (self.config.codec)());
        codec.submit(CodecJob::Object { tag: WORLD_LOOT_TAG, image });
    }

    fn body_done(&mut self, r: Result<(u32, Vec<u8>), String>) {
        let Some(chain) = self.bodies_pending.pop_front() else { return };
        match r.and_then(|(bits, payload)| crate::completion::envelope(chain, &payload, bits)) {
            Ok(image) => self.incidents_out.push_back(image),
            Err(e) => tracing::warn!("** NAMED GATE ** incident chain {chain:?} not sent: its body did not encode: {e}"),
        }
    }

    fn finish(&mut self, plan: FramePlan, result: Result<crate::row_encoders::EncodedFrame, String>, now: Instant) -> Option<Vec<u8>> {
        let failed = |this: &mut Self, plan: FramePlan, e: String| {
            tracing::warn!("udt 5 {} not sent: {e}", if plan.full { "full list" } else { "delta" });
            this.rows.failed(plan);
            this.codec_retry = Some(now + CODEC_BACKOFF);
            None
        };
        match result {
            Ok(mut f) => {
                // A cinematic-only activity's 6 and 8 went out in one tick on the live hosts: the
                // 8 takes the 6's stamp.
                let lifetime = self.run.as_ref().and_then(|run| {
                    let (_, row) = plan.entries.iter().find(|(k, _)| *k == run.activity.lifetime)?;
                    row.a.as_ref().map(|(_, image)| image[0] as i8)
                });
                let pinned = self.run.as_ref().and_then(|run| match (run.activity.end, run.lifetime_sent, lifetime) {
                    (EndKind::Cinematic, Some((6, at)), Some(8)) => Some(at),
                    _ => None,
                });
                // Never stamp 0 (run fg2f's unanswered list was stamp 0; every answered one was not).
                let stamp = pinned.unwrap_or_else(|| self.rows.tick(now)).max(1);
                match self.rows.stamp_at(&mut f.bytes, f.stamp_bit, plan.init, stamp) {
                Ok(()) => {
                    self.rows.sent(&plan, now);
                    if let (Some(run), true) = (self.run.as_mut(), plan.full) {
                        run.full_at.get_or_insert(now);
                        run.last_full_at = Some(now);
                    }
                    if let (Some(run), Some(value)) = (self.run.as_mut(), lifetime) {
                        run.lifetime_unsent = false;
                        run.lifetime_sent = Some((value, stamp));
                    }
                    self.codec_retry = None;
                    self.frames_sent += 1;
                    dump_frame(stamp, &plan, &f.bytes);
                    tracing::info!(
                        "udt 5 {} ({} rows, stamp {stamp})",
                        if plan.full { "full list, init" } else { "delta" },
                        plan.entries.len(),
                    );
                    Some(f.bytes)
                }
                Err(e) => failed(self, plan, e),
                }
            }
            Err(e) => failed(self, plan, e),
        }
    }
}

/// Stream G's seam (`public_events::BubbleScriptBridge`): a public bubble's host asks the bridge
/// to run its bundles (`active_bundles`) and to start or stop one event (`event_start` /
/// `event_stop`): an event runs from its own bundle's scripts, so its bundle joins or leaves the
/// active set. The sidecar answers `event`; a bundle the activity does not load is refused there.
impl crate::public_events::BubbleScriptBridge for ScriptBridge {
    fn start_bundles(&mut self, host: u64, bundles: &[u32]) -> Result<(), String> {
        if !self.run.as_ref().is_some_and(|r| r.started && r.halted.is_none()) {
            return Err("no scripted activity is running on this bridge".into());
        }
        self.host_id = host;
        self.send(HostMsg::ActiveBundles { bundles: bundles.to_vec() });
        Ok(())
    }

    fn start_event(&mut self, host: u64, bundle: u32, event: &crate::public_events::ScheduledEvent) -> Result<(), String> {
        if !self.run.as_ref().is_some_and(|r| r.started && r.halted.is_none()) {
            return Err(format!("no scripted activity is running on this bridge to start event {}", event.id));
        }
        self.host_id = host;
        self.send(HostMsg::EventStart { bundle, event_id: event.id });
        Ok(())
    }

    fn stop_event(&mut self, host: u64, bundle: u32, event_id: u32) -> Result<(), String> {
        if !self.run.as_ref().is_some_and(|r| r.started && r.halted.is_none()) {
            return Err(format!("no scripted activity is running on this bridge to stop event {event_id}"));
        }
        self.host_id = host;
        self.send(HostMsg::EventStop { bundle, event_id });
        Ok(())
    }
}

/// ⭐ `D1_AH_UNARMED_UNTIL_PHASE=INDEX:PHASE[,...]`: the activities a Guardian starts unarmed, and
/// the script phase that arms them. F54: the live A Guardian Rises host served the player bundle
/// (`0x808022A5`) with its field-0 bools `+8` and `+9` at 0 from the spawn and set both to 1 three
/// seconds after the rifle's checkpoint (527 s; Codex's #340 audit, notes/ps4-56578-ghost-option-
/// 2026-11-08.md). Our lists served 1 from the start, and the client showed the Ghost over an empty
/// hand until the rifle. `164:3`: A Guardian Rises, armed at its phase 3 (the rifle in hand).
pub fn unarmed_until(activity: u16) -> Option<i32> {
    static TABLE: std::sync::OnceLock<HashMap<u16, i32>> = std::sync::OnceLock::new();
    TABLE
        .get_or_init(|| std::env::var("D1_AH_UNARMED_UNTIL_PHASE").map(|t| parse_unarmed_until(&t)).unwrap_or_default())
        .get(&activity)
        .copied()
}

/// `D1_AH_SCRIPTS_SCOREBOARD=1` (lab; anything else is off).
fn scoreboard_knob() -> bool {
    std::env::var("D1_AH_SCRIPTS_SCOREBOARD").is_ok_and(|v| v.trim() == "1")
}

/// ⭐ `D1_AH_SCRIPTS_START_PHASE=INDEX:PHASE[,...]` (lab; process-wide: the world-table region override
/// in main.rs applies to every join of that activity, later players and public instances too): the activities whose mission scripts start
/// at a later phase (the sidecar's `hello.phase_index`; earlier phases never run) and the phase's
/// ordinal, for example `164:5` (A Guardian Rises at its phase 5, the hangar; an example, not a profile setting). Malformed entries come back in
/// the second list and are ignored. Nothing else is invented for the skipped phases.
pub fn parse_start_phase(text: &str) -> (HashMap<u16, u32>, Vec<String>) {
    let (mut table, mut bad) = (HashMap::new(), Vec::new());
    for entry in text.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        match entry.split_once(':').and_then(|(a, p)| Some((a.trim().parse::<u16>().ok()?, p.trim().parse::<u32>().ok()?))) {
            Some((a, p)) => {
                table.insert(a, p);
            }
            None => bad.push(entry.to_string()),
        }
    }
    (table, bad)
}

/// The knob, read once (a warning per malformed entry).
pub fn start_phase_table() -> HashMap<u16, u32> {
    static TABLE: std::sync::OnceLock<HashMap<u16, u32>> = std::sync::OnceLock::new();
    TABLE
        .get_or_init(|| {
            let text = std::env::var("D1_AH_SCRIPTS_START_PHASE").unwrap_or_default();
            let (table, bad) = parse_start_phase(&text);
            for b in bad {
                tracing::warn!("D1_AH_SCRIPTS_START_PHASE: ignoring {b:?} (want INDEX:PHASE, e.g. 164:5)");
            }
            table
        })
        .clone()
}

impl ScriptConfig {
    /// Lab: for each activity the knob names that this table drives, `(index, start region)`: the
    /// region of its start phase (`regions=` in the table, else content's, as at the phase's arrival),
    /// which the join serves instead of `start=`. `None` (and one warning) when it cannot be
    /// determined offline: `start=` stands. Logs the start line for each.
    pub fn start_phase_regions(&self, table: &HashMap<u16, u32>) -> Vec<(u16, Option<u32>)> {
        let mut out = Vec::new();
        let mut indexes: Vec<_> = table.keys().copied().collect();
        indexes.sort_unstable();
        for i in indexes {
            let Some(a) = self.activities.get(&i) else { continue };
            let phase = table[&i];
            // Only the table's regions= is known before the sidecar has loaded the scripts (their
            // phase names are content); the phase's own report names the region otherwise.
            match a.regions.get(&phase).copied() {
                Some(r) => {
                    tracing::info!(
                        "⭐ script bridge: {} starts at phase {phase} in region {r} (D1_AH_SCRIPTS_START_PHASE, lab)",
                        a.activity
                    );
                    out.push((i, Some(r)));
                }
                None => {
                    tracing::warn!(
                        "script bridge: D1_AH_SCRIPTS_START_PHASE {i}:{phase}: no `regions=` entry for phase {phase} in the activity table, so the region cannot be determined before the scripts load; the join keeps `start=`"
                    );
                    out.push((i, None));
                }
            }
        }
        out
    }
}

pub fn parse_unarmed_until(text: &str) -> HashMap<u16, i32> {
    text.split(',')
        .filter_map(|p| {
            let (a, ph) = p.trim().split_once(':')?;
            Some((a.trim().parse().ok()?, ph.trim().parse().ok()?))
        })
        .collect()
}

/// The player bundle's field-0 bools `+8` and `+9` (`0x80800540`), which the live host raised
/// together when the player took the rifle.
pub fn set_bundle_armed(image: &mut [u8], armed: bool) {
    if image.len() > 9 {
        image[8] = u8::from(armed);
        image[9] = u8::from(armed);
    }
}

pub fn bundle_armed(image: &[u8]) -> bool {
    image.len() > 9 && image[8] == 1 && image[9] == 1
}

#[cfg(test)]
mod tests {
    /// F52: the device rules table parses, and a prop's command goes from at rest to armed.
    #[test]
    fn device_rules_parse_and_the_command_arms_channel_2() {
        let rules = DeviceRule::parse_table("# c
164  0xfc0e45a3/0x4/13  0xfc0e45a3/0x1d/104  0
").unwrap();
        assert_eq!(rules, vec![DeviceRule {
            activity: 164,
            object: RowKey { group: 0xFC0E_45A3, kind: 4, index: 13 },
            trigger: RowKey { group: 0xFC0E_45A3, kind: 0x1D, index: 104 },
            order: 0,
        }]);
        assert!(DeviceRule::parse_table("164 0xfc0e45a3/0x4/13").is_err());
        let mut image = vec![0u8; 240];
        assert!(crate::row_encoders::set_device_command(&mut image, crate::row_encoders::DEVICE_AT_REST));
        assert_eq!(u32::from_le_bytes(image[0x20..0x24].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(image[0x30..0x34].try_into().unwrap()), 0x8080_279A);
        assert_eq!(f32::from_le_bytes(image[0x48..0x4C].try_into().unwrap()), 1.0, "channel 0 at rest: (0, 0, 1.0)");
        assert!(crate::row_encoders::set_device_command(&mut image, crate::row_encoders::DEVICE_ARMED));
        assert_eq!(&image[0x58..0x64], &[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x80, 0x3F], "channel 2 armed: (1, 0, 1.0)");
        assert!(!crate::row_encoders::set_device_command(&mut [0u8; 16], crate::row_encoders::DEVICE_ARMED));
        let table = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../fixtures/56578/mission/device-rules.tsv")).unwrap();
        assert_eq!(DeviceRule::parse_table(&table).unwrap().len(), 9, "the committed table: 7 tripwires and 2 grates");
    }

    /// F54: `164:3` names A Guardian Rises, armed at phase 3; the bundle's +8/+9 flip together.
    #[test]
    fn an_unarmed_start_flips_the_bundles_two_bools() {
        let t = parse_unarmed_until("164:3, 168:x, junk");
        assert_eq!(t.get(&164), Some(&3));
        assert_eq!(t.len(), 1);
        let mut image = crate::row_encoders::player_bundle_image([1; 8], true);
        assert!(bundle_armed(&image), "the hand lists' mission form starts armed");
        set_bundle_armed(&mut image, false);
        assert_eq!((image[8], image[9]), (0, 0));
        assert!(!bundle_armed(&image));
        set_bundle_armed(&mut image, true);
        assert!(bundle_armed(&image));
    }

    use super::*;
    use crate::row_encoders::EncodedFrame;
    use std::sync::Mutex;

    // ── the protocol (tools/script-host/PROTOCOL.md) ──

    #[test]
    fn host_messages_are_protocol_1_json_lines() {
        assert_eq!(host_line(&HostMsg::Start), r#"{"type":"start"}"#);
        assert_eq!(host_line(&HostMsg::End), r#"{"type":"end"}"#);
        assert_eq!(host_line(&HostMsg::Tick { now: 30 }), r#"{"type":"tick","now":30}"#);
        assert_eq!(
            host_line(&HostMsg::CinematicFinished { bundle: 0x2FE7_DBE6, type_index: 1 }),
            r#"{"type":"cinematic_finished","bundle":803724262,"type_index":1}"#
        );
        assert_eq!(
            host_line(&HostMsg::Incident { id: 2899, name_hash: 0xBD1F_B418 }),
            r#"{"type":"incident","id":2899,"name_hash":3172971544}"#
        );
        let hello = host_line(&HostMsg::Hello {
            proto: 1, build: "56578".into(), activity: "cine_reveal".into(),
            bundles_path: None, incidents_path: Some("i.tsv".into()), tick_hz: TICK_HZ, phase_index: None,
        });
        assert_eq!(hello, r#"{"type":"hello","proto":1,"build":"56578","activity":"cine_reveal","incidents_path":"i.tsv","tick_hz":10}"#);
    }

    #[test]
    fn start_phase_knob_parses_and_ignores_malformed() {
        let (t, bad) = parse_start_phase("164:5, 165:0");
        assert_eq!((t.get(&164), t.get(&165), bad.len()), (Some(&5), Some(&0), 0));
        for m in ["164", "x:5", "164:-1", "164:"] {
            let (t, bad) = parse_start_phase(m);
            assert!(t.is_empty() && bad.len() == 1, "{m:?}");
        }
        let (t, bad) = parse_start_phase("");
        assert!(t.is_empty() && bad.is_empty(), "unset is silent");
    }

    #[test]
    fn start_phase_hello_json_and_region() {
        let hello = |p| host_line(&HostMsg::Hello {
            proto: 1, build: "56578".into(), activity: "a".into(),
            bundles_path: None, incidents_path: None, tick_hz: TICK_HZ, phase_index: p,
        });
        assert_eq!(hello(None), r#"{"type":"hello","proto":1,"build":"56578","activity":"a","tick_hz":10}"#);
        assert_eq!(hello(Some(5)), r#"{"type":"hello","proto":1,"build":"56578","activity":"a","tick_hz":10,"phase_index":5}"#);

        // Through the bridge (activity 165 here): unset and another index send none; 165:1 sends 1.
        for (table, want) in [(vec![], None), (vec![(164u16, 5u32)], None), (vec![(165, 1)], Some(1))] {
            let shared = Arc::new(Mutex::new(Shared::default()));
            let mut b = bridge(&shared, false);
            b.set_start_phase(table.into_iter().collect());
            let mut t = Instant::now();
            let mut wire = Vec::new();
            b.on_client_message(&join(7), 165, HOST, t);
            run_for(&mut b, &mut t, 0.25, &mut wire);
            let s = shared.lock().unwrap();
            assert!(matches!(&s.sent[0], HostMsg::Hello { phase_index, .. } if *phase_index == want), "{:?}", s.sent[0]);
        }

        // The served region: the table's regions= (cine_reveal: 0:25, 1:26). The unset knob and
        // another activity's entry change nothing; a phase with no entry keeps start=.
        let shared = Arc::new(Mutex::new(Shared::default()));
        let b = bridge(&shared, false);
        assert!(b.config.start_phase_regions(&HashMap::new()).is_empty());
        assert!(b.config.start_phase_regions(&HashMap::from([(164, 1)])).is_empty());
        assert_eq!(b.config.start_phase_regions(&HashMap::from([(165, 1)])), vec![(165, Some(26))]);
        assert_eq!(b.config.start_phase_regions(&HashMap::from([(165, 9)])), vec![(165, None)]);
    }

    #[test]
    fn a_later_start_phase_brings_the_lifetime_to_gameplay() {
        // A later start skips phase 0's pregame cinematic hold and the F35 release; the first phase
        // of the start catches the lifetime up, one value per frame as a normal run: 1, 4, 3.
        let run = |table: Vec<(u16, u32)>| {
            let mut seen = Vec::new();
            let shared = Arc::new(Mutex::new(Shared::default()));
            let activity = ScriptedActivity::parse_with(GAMEPLAY_LINE, &base(), &|_| Ok(LIST.to_string()), None).unwrap();
            // A start delay that never passes: the fake script stays silent; the test plays the sidecar.
            let mut b = bridge_delayed(&shared, false, false, activity, Duration::from_secs(3600));
            b.set_start_phase(table.into_iter().collect());
            let mut t = Instant::now();
            b.on_client_message(&join(9), 165, HOST, t);
            b.on_client_message(&slice(25), 165, HOST, t);
            run_for(&mut b, &mut t, 1.0, &mut Vec::new());
            // The real sidecar's order (run gh61): the phase message first, then the scripts' first lifetime value.
            b.apply(phase(5, "hangar"), t);
            b.apply(SidecarMsg::Rows { changes: vec![lifetime(0)] }, t);
            run_for(&mut b, &mut t, 3.0, &mut Vec::new());
            // The scripts' state 0 again does not drag it back; a real transition still applies.
            b.apply(SidecarMsg::Rows { changes: vec![lifetime(0)] }, t);
            run_for(&mut b, &mut t, 1.0, &mut Vec::new());
            let held = shared.lock().unwrap().frames.iter().filter_map(lifetime_of).last();
            seen.push(held);
            b.apply(SidecarMsg::Rows { changes: vec![lifetime(3)] }, t);
            run_for(&mut b, &mut t, 1.0, &mut Vec::new());
            let s = shared.lock().unwrap();
            let mut seq: Vec<i8> = s.frames.iter().filter_map(lifetime_of).collect();
            seq.dedup();
            (seq, seen)
        };
        // Script state 3 is the later genuine transition (wire 4); it still applies after the ramp.
        let (seq, seen) = run(vec![(165, 5)]);
        assert_eq!(seq, [0, 1, 4, 3, 4], "the values a normal run reaches by phase 1, held at 3 against the scripts' state 0");
        assert_eq!(seen, [Some(3)], "the repeated state 0 does not drag it back");
        assert_eq!(run(vec![]).0, [0, 1, 4], "unset: unchanged, no catch-up");
        assert_eq!(run(vec![(164, 5)]).0, [0, 1, 4], "another activity's entry changes nothing");
    }

    /// Plays the sidecar's side for the start-phase tests: `steps` run in order, then 4 s pass.
    /// Returns the deduplicated wire lifetime sequence.
    fn start_phase_run(table: Vec<(u16, u32)>, steps: &dyn Fn(&mut ScriptBridge, Instant, &mut dyn FnMut(&mut ScriptBridge, f32))) -> Vec<i8> {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let activity = ScriptedActivity::parse_with(GAMEPLAY_LINE, &base(), &|_| Ok(LIST.to_string()), None).unwrap();
        let mut b = bridge_delayed(&shared, false, false, activity, Duration::from_secs(3600));
        b.set_start_phase(table.into_iter().collect());
        let t = std::cell::Cell::new(Instant::now());
        b.on_client_message(&join(9), 165, HOST, t.get());
        b.on_client_message(&slice(25), 165, HOST, t.get());
        let mut wait = |b: &mut ScriptBridge, secs: f32| {
            let mut now = t.get();
            run_for(b, &mut now, secs, &mut Vec::new());
            t.set(now);
        };
        wait(&mut b, 1.0);
        steps(&mut b, t.get(), &mut wait);
        let s = shared.lock().unwrap();
        let mut seq: Vec<i8> = s.frames.iter().filter_map(lifetime_of).collect();
        seq.dedup();
        seq
    }

    #[test]
    fn start_phase_ramp_without_a_scripted_value_starts_after_two_seconds() {
        let seq = start_phase_run(vec![(165, 5)], &|b, t, wait| {
            b.apply(phase(5, "hangar"), t);
            wait(b, 1.0);
        });
        assert_eq!(seq, [0], "no scripted value and under 2 s: the ramp waits");
        let seq = start_phase_run(vec![(165, 5)], &|b, t, wait| {
            b.apply(phase(5, "hangar"), t);
            wait(b, 4.0);
        });
        assert_eq!(seq, [0, 4, 3], "no scripted value: the ramp starts anyway after 2 s");
    }

    #[test]
    fn start_phase_ramp_arms_at_phase_1_and_a_restart_clears_the_skip() {
        let seq = start_phase_run(vec![(165, 1)], &|b, t, wait| {
            b.apply(phase(1, "p1"), t);
            b.apply(SidecarMsg::Rows { changes: vec![lifetime(0)] }, t);
            wait(b, 3.0);
        });
        assert_eq!(seq, [0, 1, 4, 3], "a ramp that arms at phase 1");
        let seq = start_phase_run(vec![(165, 5)], &|b, t, wait| {
            b.apply(phase(5, "hangar"), t);
            b.apply(SidecarMsg::Rows { changes: vec![lifetime(0)] }, t);
            wait(b, 3.0);
            b.apply(SidecarMsg::Rows { changes: vec![lifetime(0)] }, t);
            wait(b, 1.0);
            // The scripts restart at phase 0: their initial state applies again.
            b.apply(phase(0, "p0"), t);
            b.apply(SidecarMsg::Rows { changes: vec![lifetime(0)] }, t);
            wait(b, 1.0);
        });
        assert_eq!(seq, [0, 1, 4, 3, 1], "swallowed at 3 until phase 0, then state 0 applies");
    }

    #[test]
    fn start_phase_mid_ramp_values() {
        // A scripted value below 3 mid-ramp is swallowed; the ramp still ends at 3.
        let seq = start_phase_run(vec![(165, 5)], &|b, t, wait| {
            b.apply(phase(5, "hangar"), t);
            b.apply(SidecarMsg::Rows { changes: vec![lifetime(0)] }, t);
            wait(b, 0.3); // 1 is out, the hold 4 is next or out
            b.apply(SidecarMsg::Rows { changes: vec![lifetime(0)] }, t);
            wait(b, 3.0);
        });
        assert_eq!(seq.last(), Some(&3), "{seq:?}");
        assert!(!seq.windows(2).any(|w| w == [4, 1] || w == [3, 1]), "no drag back: {seq:?}");
        // A scripted state 3 (wire 4) mid-ramp ends the ramp and applies; the ramp's 3 does not overwrite it.
        let seq = start_phase_run(vec![(165, 5)], &|b, t, wait| {
            b.apply(phase(5, "hangar"), t);
            b.apply(SidecarMsg::Rows { changes: vec![lifetime(0)] }, t);
            wait(b, 0.3);
            b.apply(SidecarMsg::Rows { changes: vec![lifetime(3)] }, t);
            wait(b, 3.0);
        });
        assert_eq!(seq.last(), Some(&4), "{seq:?}");
    }

    #[test]
    fn sidecar_lines_parse_protocol_1() {
        let rows = parse_sidecar_line(
            r#"{"type":"rows","changes":[{"item":{"bundle":1,"typeId":16,"typeIndex":0},"kind":"lifetime","state":{"state":3,"joinFlag":true},"wire":{"phase":4},"initial":true}]}"#,
        )
        .unwrap()
        .unwrap();
        let SidecarMsg::Rows { changes } = rows else { panic!() };
        assert_eq!(changes[0].item, ItemRef { bundle: 1, type_id: 16, type_index: 0 });
        assert_eq!(changes[0].wire.as_ref().and_then(|w| w.get("phase")).and_then(Value::as_i64), Some(4));
        let loaded = parse_sidecar_line(r#"{"type":"loaded","activity":"cine_reveal","proto":1,"stand_in":true,"stub":false,"drives_lifetime":true,"compile_errors":[],"phases":[{"index":0}]}"#).unwrap();
        assert!(matches!(loaded, Some(SidecarMsg::Loaded { stand_in: true, drives_lifetime: true, .. })));
        assert_eq!(parse_sidecar_line(r#"{"type":"activity_complete","id":965,"name":"activity_complete","name_hash":1113982608}"#).unwrap(), Some(SidecarMsg::ActivityComplete {}));
        assert_eq!(parse_sidecar_line(r#"{"type":"ended"}"#).unwrap(), Some(SidecarMsg::Ended {}));
        let phase = parse_sidecar_line(r#"{"type":"phase","index":1,"name":"cine_107_twr_phase","name_hash":3615303679,"decl":"x","slice":"plaza","slice_hash":"0x1ad5e415"}"#).unwrap().unwrap();
        let SidecarMsg::Phase { index: 1, name_hash: Some(h), slice_hash: Some(sh), region: None, .. } = phase else { panic!("{phase:?}") };
        assert_eq!((hash_value(&h), hash_value(&sh)), (Some(0xD77D_27FF), Some(0x1AD5_E415)));
        assert_eq!(parse_sidecar_line(r#"{"type":"error","text":"no script","fatal":true}"#).unwrap(), Some(SidecarMsg::Error { text: "no script".into(), fatal: true }));
        assert_eq!(parse_sidecar_line(r#"{"type":"tick_done","now":3,"ticks":3,"time":0.3}"#).unwrap(), Some(SidecarMsg::TickDone { now: 3 }));
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
        bodies: Vec<(u32, Vec<u8>)>,
        rewards: Vec<(CharacterRef, u32, ActivityOutcome)>,
        /// How many frames had been planned when each grant was made.
        rewarded_after_frames: Vec<usize>,
        hand_offs: Vec<(u64, u32, u16, u32)>,
        /// `(character key, activity hash, frames planned when it was recorded)`.
        completions: Vec<(u64, u32, usize)>,
        /// `(kind, activity hash, frames planned)`: "aside", "started", "ended".
        lifecycle: Vec<(&'static str, u32, usize)>,
        launches: u32,
        /// The fake sidecar sets pregame and the cinematic hold in its answer to `start`.
        combined_start: bool,
    }

    /// The synthetic script, in the shape of the reveal: pregame; cinematic 0 at the cinematic
    /// hold 2 s in; on its finish, phase 1; 4 s later complete and cinematic 1; on its finish,
    /// results, ended.
    struct FakeSidecar {
        shared: Arc<Mutex<Shared>>,
        out: VecDeque<LinkEvent>,
        stage: u32,
        stage_at: u64,
        die_after_start: bool,
        stand_in: bool,
    }

    fn change(kind: &str, index: i16, type_id: u8, state: Value) -> ItemChange {
        ItemChange { item: ItemRef { bundle: CINE, type_id, type_index: index }, kind: kind.into(), state, initial: false, wire: None }
    }

    fn lifetime(state: i64) -> ItemChange {
        ItemChange {
            item: ItemRef { bundle: 0x1234, type_id: 16, type_index: 0 },
            kind: "lifetime".into(),
            state: serde_json::json!({"joinFlag": true, "state": state}),
            initial: false,
            wire: Some(serde_json::json!({"phase": lifetime_wire_state(state, EndKind::Cinematic)})), // the sidecar always ends on 8
        }
    }

    fn playing(index: i16, on: bool) -> ItemChange {
        let state = if on { serde_json::json!({"playing": {"startedAt": 99}}) } else { serde_json::json!({}) };
        change("cinematic", index, 6, state)
    }

    fn phase(index: u32, name: &str) -> SidecarMsg {
        SidecarMsg::Phase { index, name: Some(name.into()), name_hash: None, slice_hash: None, region: None }
    }

    impl SidecarLink for FakeSidecar {
        fn send(&mut self, msg: &HostMsg) -> Result<(), String> {
            self.shared.lock().unwrap().sent.push(msg.clone());
            let mut reply = |m: SidecarMsg| self.out.push_back(LinkEvent::Msg(m));
            match msg {
                HostMsg::Hello { .. } => reply(SidecarMsg::Loaded {
                    stand_in: self.stand_in, stub: false, drives_lifetime: true, compile_errors: Vec::new(),
                }),
                HostMsg::Start => {
                    if self.die_after_start {
                        self.out.push_back(LinkEvent::Closed("crashed".into()));
                        return Ok(());
                    }
                    reply(SidecarMsg::Ready { unresolved: Vec::new(), drives_lifetime: true });
                    if self.shared.lock().unwrap().combined_start {
                        reply(SidecarMsg::Rows { changes: vec![lifetime(0), lifetime(3), playing(0, false), playing(1, false)] });
                    } else {
                        reply(SidecarMsg::Rows { changes: vec![lifetime(0), playing(0, false), playing(1, false)] });
                    }
                    reply(phase(0, "cine_106_trv_phase"));
                }
                HostMsg::Tick { now } => {
                    if self.stage == 0 && *now >= 20 {
                        self.stage = 1;
                        reply(SidecarMsg::Rows { changes: vec![lifetime(3), playing(0, true)] });
                    }
                    if self.stage == 2 && *now >= self.stage_at + 40 {
                        self.stage = 3;
                        reply(SidecarMsg::ActivityComplete {});
                        reply(SidecarMsg::Rows { changes: vec![playing(1, true)] });
                    }
                    self.stage_at = if self.stage == 1 { *now } else { self.stage_at };
                    reply(SidecarMsg::TickDone { now: *now });
                }
                HostMsg::CinematicFinished { type_index: 0, .. } if self.stage == 1 => {
                    self.stage = 2;
                    reply(SidecarMsg::Rows { changes: vec![playing(0, false)] });
                    reply(phase(1, "cine_107_twr_phase"));
                }
                HostMsg::CinematicFinished { type_index: 1, .. } if self.stage == 3 => {
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

    /// Records each job; a frame's bytes are a heartbeat-shaped header (stamp at bit 9) plus a
    /// count, a body encodes to 725 bits of 0xCC.
    fn recording(shared: Arc<Mutex<Shared>>) -> crate::row_encoders::InlineCodec {
        Box::new(move |job| match job {
            CodecJob::Frame(plan) => {
                shared.lock().unwrap().frames.push(plan.clone());
                let mut bytes = d1_oracle::sensor::heartbeat_frame(plan.gen, 0);
                bytes.push(plan.entries.len() as u8);
                CodecDone::Frame(Ok(EncodedFrame { bytes, stamp_bit: 9 }))
            }
            CodecJob::Object { tag, image } => {
                shared.lock().unwrap().bodies.push((*tag, image.clone()));
                CodecDone::Object(Ok((725, vec![0xCC; 91])))
            }
        })
    }

    struct RecordingRewards(Arc<Mutex<Shared>>);
    impl ActivityRewards for RecordingRewards {
        fn grant_activity_rewards(&mut self, c: &CharacterRef, hash: u32, outcome: &ActivityOutcome) -> Result<(), String> {
            let mut s = self.0.lock().unwrap();
            // When: the lifetime value the bridge holds at the call.
            let at = s.frames.len();
            s.rewards.push((c.clone(), hash, *outcome));
            s.rewarded_after_frames.push(at);
            Ok(())
        }
        fn activity_hand_off(&mut self, c: &CharacterRef, from: u32, index: u16, hash: u32) -> Result<(), String> {
            self.0.lock().unwrap().hand_offs.push((c.key(), from, index, hash));
            Ok(())
        }
        fn activity_completed(&mut self, c: &CharacterRef, hash: u32) -> Result<(), String> {
            let mut s = self.0.lock().unwrap();
            let at = s.frames.len();
            s.completions.push((c.key(), hash, at));
            Ok(())
        }
        fn activity_completed_aside(&mut self, _: &CharacterRef, hash: u32) -> Result<(), String> {
            let mut s = self.0.lock().unwrap();
            let at = s.frames.len();
            s.lifecycle.push(("aside", hash, at));
            Ok(())
        }
        fn activity_started(&mut self, _: &CharacterRef, hash: u32) -> Result<(), String> {
            let mut s = self.0.lock().unwrap();
            let at = s.frames.len();
            s.lifecycle.push(("started", hash, at));
            Ok(())
        }
        fn activity_ended(&mut self, _: &CharacterRef, hash: u32) -> Result<(), String> {
            let mut s = self.0.lock().unwrap();
            let at = s.frames.len();
            s.lifecycle.push(("ended", hash, at));
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

    /// The reveal: cinematic-only, with its manifest hashes (synthetic values).
    const CINE_LINE: &str = "165 cine_reveal 0x4786c0e0 list.txt hash=0x0BBD0E26 kind=cinematic \
        type=0x648999F4 type2=0x4D78882E place=0xB22417AE regions=0:25,1:26";
    /// The same world served as a gameplay activity.
    const GAMEPLAY_LINE: &str = "165 cine_reveal 0x4786c0e0 list.txt hash=0x0BBD0E26 \
        type=0x648999F4 type2=0x648999F4 place=0xB22417AE regions=0:25,1:26";
    /// No manifest hashes: no completion body.
    const BARE_LINE: &str = "165 cine_reveal 0x4786c0e0 list.txt hash=0x0BBD0E26 kind=cinematic regions=0:25,1:26";
    const HOST: u64 = 0xD1A0_0000_0000_0042;

    fn bridge_with(shared: &Arc<Mutex<Shared>>, die_after_start: bool, stand_in: bool, line: &str) -> ScriptBridge {
        bridge_worlds(shared, die_after_start, stand_in, line, None)
    }

    fn bridge_worlds(
        shared: &Arc<Mutex<Shared>>,
        die_after_start: bool,
        stand_in: bool,
        line: &str,
        worlds: Option<&crate::generated::GeneratedWorlds>,
    ) -> ScriptBridge {
        let activity = ScriptedActivity::parse_with(line, &base(), &|_| Ok(LIST.to_string()), worlds).unwrap();
        if worlds.is_some() {
            return bridge_of(shared, die_after_start, stand_in, activity);
        }
        assert_eq!(activity.lifetime, RowKey { group: GROUP, kind: 0x10, index: 3 });
        assert_eq!(activity.machine(), 13);
        bridge_of(shared, die_after_start, stand_in, activity)
    }

    fn bridge_of(shared: &Arc<Mutex<Shared>>, die_after_start: bool, stand_in: bool, activity: ScriptedActivity) -> ScriptBridge {
        bridge_delayed(shared, die_after_start, stand_in, activity, Duration::ZERO)
    }

    fn bridge_delayed(
        shared: &Arc<Mutex<Shared>>,
        die_after_start: bool,
        stand_in: bool,
        activity: ScriptedActivity,
        start_delay: Duration,
    ) -> ScriptBridge {
        let s = shared.clone();
        let config = ScriptConfig {
            sidecar: vec!["fake".into()],
            activities: HashMap::from([(165, activity)]),
            gen: 0,
            tail: None,
            incidents: crate::incident_catalog::tests::synthetic(),
            incidents_path: None,
            mission_phase: true,
            send_unlisted: false,
            delta_init: false,
            scoreboard_columns: Vec::new(),
            rewards: Arc::new(Mutex::new(RecordingRewards(shared.clone()))),
            codec: Arc::new(move || CodecWorker::inline(recording(s.clone()))),
            active_bundles: true,
            start_delay,
            arrival_cushion: Duration::ZERO,
            list_retry: None,
            world_loot: Vec::new(),
            device_rules: Vec::new(),
        };
        let s = shared.clone();
        ScriptBridge::with_launcher(
            Arc::new(config),
            Box::new(move |_| {
                s.lock().unwrap().launches += 1;
                Ok(Box::new(FakeSidecar { shared: s.clone(), out: VecDeque::new(), stage: 0, stage_at: 0, die_after_start, stand_in })
                    as Box<dyn SidecarLink>)
            }),
        )
    }

    fn bridge(shared: &Arc<Mutex<Shared>>, die_after_start: bool) -> ScriptBridge {
        bridge_with(shared, die_after_start, false, CINE_LINE)
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

    /// A udt 21 image naming `region` with the empty-name hash.
    fn slice_unnamed(region: u32) -> ClientMessage {
        let mut struct_image = vec![0u8; 0x220];
        struct_image[0xB8..0xBC].copy_from_slice(&region.to_le_bytes());
        struct_image[0xBC..0xC0].copy_from_slice(&0x811C_9DC5u32.to_le_bytes());
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

    fn cinematic_of_row(r: &Row) -> Option<(u32, u32, u8)> {
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

    // ── D1_AH_SCRIPTS_SCOREBOARD: the kind-15 scoreboard at the completion ──

    /// The synthetic catalogue with the kill chain's two incidents at their real indices
    /// (734 `any_kill`, 1825 `kill`) and the real name hashes.
    fn kill_catalogue() -> crate::incident_catalog::IncidentCatalog {
        use crate::pvp_driver::incidents;
        let mut text = String::from("index\tname_hash\tname\tpayload_selector\taux\n");
        for i in 0..=3403u32 {
            let hash = match i {
                734 => incidents::ANY_KILL,
                1825 => incidents::KILL,
                _ => match crate::incident_catalog::known::ALL.iter().find(|k| k.index == i) {
                    Some(k) => k.hash,
                    None => 0x1000_0000 + i,
                },
            };
            text.push_str(&format!("{i}\t{hash:#010x}\t\t17\t65535,65535,65535,65535\n"));
        }
        crate::incident_catalog::IncidentCatalog::parse(&text).expect("catalogue")
    }

    /// A udt 18 reporting a whole chain of incidents.
    fn chain(indices: &[u32]) -> ClientMessage {
        let mut image = crate::state::incident_image(indices[0], &[0u8; 8]);
        image[0..4].copy_from_slice(&(indices.len() as u32).to_le_bytes());
        for (k, t) in indices.iter().enumerate() {
            image[4 + 4 * k..8 + 4 * k].copy_from_slice(&t.to_le_bytes());
        }
        ClientMessage::Other { user_defined_type: 18, struct_image: image }
    }

    fn scoreboard_bridge(shared: &Arc<Mutex<Shared>>, on: bool) -> ScriptBridge {
        let mut b = bridge(shared, false);
        Arc::get_mut(&mut b.config).expect("one owner").incidents = kill_catalogue();
        b.set_scoreboard(on);
        b
    }

    /// The scripted reveal from a join to its end, `reports` arriving while it plays.
    fn play_reveal(b: &mut ScriptBridge, t: &mut Instant, wire: &mut Vec<Vec<u8>>, reports: &[ClientMessage]) {
        run_for(b, t, 0.75, wire);
        b.on_client_message(&slice(25), 165, HOST, *t);
        run_for(b, t, 3.0, wire);
        for r in reports {
            b.on_client_message(r, 165, HOST, *t);
        }
        run_for(b, t, 6.5, wire);
        b.on_client_message(&incident(891, Some((CINE, 6, 0))), 165, HOST, *t);
        run_for(b, t, 0.5, wire);
        b.on_client_message(&slice(26), 165, HOST, *t);
        run_for(b, t, 4.5, wire);
        b.on_client_message(&incident(891, None), 165, HOST, *t);
        run_for(b, t, 1.0, wire);
    }

    /// The frames that carried the kind-15 row's value: (frame index, A image).
    fn scoreboard_frames(shared: &Arc<Mutex<Shared>>) -> Vec<(usize, Vec<u8>)> {
        let s = shared.lock().unwrap();
        s.frames
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                let (_, r) = p.entries.iter().find(|(k, _)| k.kind == 0xF && k.group == GROUP)?;
                let (tag, image) = r.a.as_ref()?;
                (*tag == crate::scoreboard_rows::TAG).then(|| (i, image.clone()))
            })
            .collect()
    }

    fn f32_at(b: &[u8], at: usize) -> f32 {
        f32::from_le_bytes(b[at..at + 4].try_into().unwrap())
    }

    fn stat_at(col: usize) -> usize {
        crate::scoreboard_rows::PLAYER_ROWS + crate::scoreboard_rows::ROW_STATS + 4 * col
    }

    #[test]
    fn the_scoreboard_knob_off_sends_no_scoreboard_row() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = scoreboard_bridge(&shared, false);
        let (mut t, mut wire) = (Instant::now(), Vec::new());
        b.on_client_message(&join(7), 165, HOST, t);
        play_reveal(&mut b, &mut t, &mut wire, &[chain(&[734, 1825]), chain(&[2079])]);
        assert!(scoreboard_frames(&shared).is_empty(), "off: no 0xF row value");
        assert!(b.take_incident().is_some(), "the completion still goes");
        assert!(b.rows().get(&RowKey { group: GROUP, kind: 0xF, index: 0 }).is_some_and(|r| r.a.is_none()));
    }

    #[test]
    fn the_scoreboard_row_carries_the_players_kills_and_deaths_at_the_completion() {
        use crate::scoreboard_rows as sr;
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = scoreboard_bridge(&shared, true);
        let (mut t, mut wire) = (Instant::now(), Vec::new());
        b.on_client_message(&join(7), 165, HOST, t);
        let kill = chain(&[734, 1825]);
        play_reveal(&mut b, &mut t, &mut wire, &[kill.clone(), kill, chain(&[2079])]);
        let sent = scoreboard_frames(&shared);
        assert!(!sent.is_empty(), "the row went out");
        let image = &sent[0].1;
        let row = sr::PLAYER_ROWS;
        assert_eq!(image.len(), sr::SIZE);
        assert_eq!(u32_at(image, sr::PLAYER_COUNT), 1);
        assert_eq!(&image[row..row + 8], [7u8; 8], "the player key (uniform here; scoreboard_rows pins the field and byte order)");
        assert_eq!(u32_at(image, row + sr::ROW_STAT_COUNT), 23);
        assert_eq!(f32_at(image, stat_at(0)), 2.0, "kills");
        assert_eq!(f32_at(image, stat_at(22)), 1.0, "deaths");
        assert_eq!(f32_at(image, stat_at(18)), 0.0, "precision kills are not counted");
        assert_eq!(u32_at(image, sr::CATEGORY), 0x811C_9DC5);
        assert_eq!(u32_at(image, sr::TEAM_COUNT), 0);
        assert_eq!(u32_at(image, row + sr::ROW_REWARD_COUNT), 0, "no rewards array");
        // Kept set: the table still holds it after the end.
        assert!(b.rows().get(&RowKey { group: GROUP, kind: 0xF, index: 0 }).is_some_and(|r| r.a.is_some()));
    }

    #[test]
    fn the_scoreboard_row_goes_before_the_results_lifetime() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = scoreboard_bridge(&shared, true);
        let (mut t, mut wire) = (Instant::now(), Vec::new());
        b.on_client_message(&join(7), 165, HOST, t);
        play_reveal(&mut b, &mut t, &mut wire, &[]);
        let first = scoreboard_frames(&shared)[0].0;
        let s = shared.lock().unwrap();
        let late = s.frames.iter().position(|p| matches!(lifetime_of(p), Some(l) if l >= 6)).expect("results frame");
        assert!(first < late, "the row (frame {first}) precedes the lifetime's results (frame {late})");
        // The completion body is submitted after the row was set (same apply).
        assert_eq!(s.bodies.len(), 1);
        drop(s);
        assert!(b.take_incident().is_some());
    }

    #[test]
    fn the_scoreboard_counts_start_again_with_a_new_run() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = scoreboard_bridge(&shared, true);
        let (mut t, mut wire) = (Instant::now(), Vec::new());
        b.on_client_message(&join(7), 165, HOST, t);
        play_reveal(&mut b, &mut t, &mut wire, &[chain(&[734, 1825])]);
        assert_eq!(f32_at(&scoreboard_frames(&shared)[0].1, stat_at(0)), 1.0);
        // The run ended; the same activity joined again is a new run with zero counts.
        b.on_client_message(&join(7), 165, HOST, t);
        assert_eq!(b.run.as_ref().map(|r| (r.score_kills, r.score_deaths)), Some((0, 0)));
        // Two players joined: a report cannot be attributed and is not counted.
        b.on_client_message(&join(8), 165, HOST, t);
        b.on_client_message(&chain(&[734, 1825]), 165, HOST, t);
        assert_eq!(b.run.as_ref().map(|r| (r.score_kills, r.score_deaths)), Some((0, 0)));
    }

    // ── D1_AH_SCRIPTS_INTERACTABLE: the activity flag and the placement ──

    /// Synthetic values: a placement bundle, and a flag hash that is the test's own.
    const PLACED: u32 = 0xD7F6_8DA0;
    const TEST_FLAG: u32 = 0x1234_ABCD;

    fn interactable_bridge(shared: &Arc<Mutex<Shared>>, on: bool) -> ScriptBridge {
        let mut list = String::from("@authority 13\n@phase 0\n0x2fe7dbe6 0x6 0 1 0\n0x4786c0e0 0xf 0 1 0\n");
        for i in 5..21 {
            list.push_str(&format!("0x4786c0e0 0xc {i} 1 0\n"));
        }
        list.push_str("0xd7f68da0 0x4 52 1 0\n0xd7f68da0 0x21 71 1 0\n");
        let activity = ScriptedActivity::parse_with(GAMEPLAY_LINE, &base(), &|_| Ok(list.clone()), None).unwrap();
        let mut b = bridge_delayed(shared, false, false, activity, Duration::from_secs(3600));
        b.set_interactable(on);
        b
    }

    /// The A images of the player slot rows now in the table: (index, image).
    fn player_rows(b: &ScriptBridge) -> Vec<(i16, Vec<u8>)> {
        (5..21)
            .filter_map(|index| {
                let r = b.rows().get(&RowKey { group: GROUP, kind: 0xC, index })?;
                r.a.as_ref().map(|(_, i)| (index, i.clone()))
            })
            .collect()
    }

    fn flag(hash: u32, active: bool) -> SidecarMsg {
        SidecarMsg::ActivityFlag { hash, active }
    }

    #[test]
    fn an_activity_flag_is_set_and_cleared_in_every_player_row() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = interactable_bridge(&shared, true);
        let (mut t, mut wire) = (Instant::now(), Vec::new());
        b.on_client_message(&join(7), 165, HOST, t);
        // The bridge serves one image per slot here: the joiner's, and the others' as the test gives them.
        for index in 6..21 {
            let key = RowKey { group: GROUP, kind: 0xC, index };
            let mut row = b.rows().get(&key).cloned().expect("listed");
            row.a = Some((crate::row_encoders::TAG_PLAYER_BUNDLE, crate::row_encoders::player_bundle_image([index as u8; 8], true)));
            b.rows.set(key, row);
        }
        run_for(&mut b, &mut t, 2.0, &mut wire);
        assert_eq!(player_rows(&b).len(), 16, "all 16 slot rows hold an image");
        let list = |b: &ScriptBridge| player_rows(b).into_iter().map(|(_, i)| (u32_at(&i, 0x4C), u32_at(&i, 0x50))).collect::<Vec<_>>();
        assert!(list(&b).iter().all(|l| *l == (0, 0)));

        b.apply(flag(TEST_FLAG, true), t);
        assert!(list(&b).iter().all(|l| *l == (1, TEST_FLAG)), "set: count 1 with the hash at +0x50, in every row");
        // A delta carries it: one frame holds the 16 rows.
        let before = shared.lock().unwrap().frames.len();
        run_for(&mut b, &mut t, 1.0, &mut wire);
        let sent: Vec<usize> = shared.lock().unwrap().frames[before..]
            .iter()
            .map(|p| p.entries.iter().filter(|(k, r)| k.kind == 0xC && r.a.as_ref().is_some_and(|(_, i)| u32_at(i, 0x4C) == 1)).count())
            .collect();
        assert_eq!(sent.iter().sum::<usize>(), 16, "sent as a delta, once per row: {sent:?}");
        // The same flag again changes nothing; a second flag appends.
        b.apply(flag(TEST_FLAG, true), t);
        assert!(list(&b).iter().all(|l| l.0 == 1));
        b.apply(flag(7, true), t);
        assert!(player_rows(&b).iter().all(|(_, i)| (u32_at(i, 0x4C), u32_at(i, 0x54)) == (2, 7)));
        b.apply(flag(TEST_FLAG, false), t);
        assert!(player_rows(&b).iter().all(|(_, i)| (u32_at(i, 0x4C), u32_at(i, 0x50), u32_at(i, 0x54)) == (1, 7, 0)), "removed and compacted");
        b.apply(flag(7, false), t);
        assert!(list(&b).iter().all(|l| *l == (0, 0)), "cleared: count 0");
        // Nothing but the list changed.
        let want = crate::row_encoders::player_bundle_image([6; 8], true);
        assert_eq!(player_rows(&b)[1].1, want);
    }

    #[test]
    fn the_flag_knob_off_leaves_the_player_rows_alone() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = interactable_bridge(&shared, false);
        let t = Instant::now();
        b.on_client_message(&join(7), 165, HOST, t);
        let before = player_rows(&b);
        b.apply(flag(TEST_FLAG, true), t);
        assert_eq!(player_rows(&b), before, "off: the warning only, no change");
        assert!(b.run.as_ref().is_some_and(|r| r.activity_flags.is_empty()));
    }

    fn placement_change() -> ItemChange {
        ItemChange {
            item: ItemRef { bundle: PLACED, type_id: 4, type_index: 52 },
            kind: "object".into(),
            state: serde_json::json!({"incarnation": 1, "present": true, "interactable": {
                "enabled": false,
                "filter": {"bundle": PLACED, "typeId": 33, "typeIndex": 71},
                "used": {"stamp": 0, "value": false}}}),
            initial: false,
            wire: None,
        }
    }

    fn filter_change() -> ItemChange {
        ItemChange {
            item: ItemRef { bundle: PLACED, type_id: 33, type_index: 71 },
            kind: "objectFilter".into(),
            state: serde_json::json!({"predicates": [{"kind": "playersWithActivityFlag", "flagNameHash": TEST_FLAG}]}),
            initial: false,
            wire: None,
        }
    }

    #[test]
    fn a_placement_and_its_filter_reach_the_table_with_the_knob_and_not_without() {
        for on in [true, false] {
            let shared = Arc::new(Mutex::new(Shared::default()));
            let mut b = interactable_bridge(&shared, on);
            let t = Instant::now();
            b.on_client_message(&join(7), 165, HOST, t);
            // The sidecar's order: the object created, then the filter filled, then enabled for it.
            let mut created = placement_change();
            created.state = serde_json::json!({"incarnation": 1, "present": true});
            b.apply(SidecarMsg::Rows { changes: vec![created, filter_change(), placement_change()] }, t);
            let group = b.run.as_ref().expect("run").activity.group_of(PLACED);
            let object = b.rows().get(&RowKey { group, kind: 4, index: 52 }).and_then(|r| r.a.clone());
            let filter = b.rows().get(&RowKey { group, kind: 33, index: 71 }).and_then(|r| r.a.clone());
            if on {
                let (tag, image) = object.expect("the object has an image");
                assert_eq!(tag, crate::row_encoders::TAG_OBJECT);
                assert_eq!((u32_at(&image, 0x20), u32_at(&image, 0x30), image[0x40] as i8), (2, 0x8080_2140, 1), "the later push kept the command: never count 0");
                assert_eq!((u32_at(&image, 0x44), image[0x48], image[0x4A]), (group, 0x21, 71));
                let (tag, image) = filter.expect("the filter has an image");
                assert_eq!((tag, u32_at(&image, 0), u32_at(&image, 0x10), u32_at(&image, 0x24)), (crate::row_encoders::TAG_OBJECT_FILTER, 1, 0x8080_0952, TEST_FLAG));
            } else {
                let (_, image) = object.expect("the object is still created");
                assert_eq!(u32_at(&image, 0x20), 0, "off: no command, as before");
                assert!(filter.is_none(), "off: the filter row keeps its served (empty) value");
            }
        }
    }

    #[test]
    fn a_fake_sidecar_drives_a_whole_synthetic_activity() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge(&shared, false);
        let mut t = Instant::now();
        let mut wire = Vec::new();
        assert!(b.sensor_due(t).is_none(), "nothing before a join");
        b.on_client_message(&join(7), 99, HOST, t);
        assert!(!b.scripted(), "an activity the table does not name stays unscripted");
        b.on_client_message(&join(7), 165, HOST, t);
        assert!(b.scripted());

        // The join: the full list (every row, init, authority) goes out first; then start.
        run_for(&mut b, &mut t, 0.25, &mut wire);
        {
            let s = shared.lock().unwrap();
            let full = &s.frames[0];
            assert!(full.full && full.init && full.authority.is_some());
            assert_eq!(full.entries.len(), 4, "three listed rows and the lifetime");
            assert_eq!(lifetime_of(full), Some(0), "the list's @phase until the scripts set one");
            assert!(matches!(&s.sent[0], HostMsg::Hello { activity, tick_hz: 10, .. } if activity == "cine_reveal"));
        }
        run_for(&mut b, &mut t, 0.5, &mut wire);
        {
            let s = shared.lock().unwrap();
            let start = s.sent.iter().position(|m| matches!(m, HostMsg::Start)).expect("start after loaded and the full list");
            // F44: the start slice set's bundles go out between the players and the start.
            let before = if matches!(s.sent[start - 1], HostMsg::ActiveBundles { .. }) { start - 2 } else { start - 1 };
            assert!(matches!(s.sent[before], HostMsg::Players { ref players } if players.len() == 1));
            let pregame = s.frames.last().unwrap();
            assert!(!pregame.full && !pregame.init, "a delta");
            assert_eq!(lifetime_of(pregame), Some(1), "pregame -> 1");
            assert_eq!(pregame.entries.len(), 3, "the lifetime and the two cinematics it set");
        }
        assert_eq!(b.take_region_request(), Some(25), "phase 0 runs in region 25 (the table's), the client is not there yet");
        b.on_client_message(&slice(25), 165, HOST, t);

        // Ticks follow the 10 Hz stamp: at stamp 20 the first cinematic plays at the hold.
        run_for(&mut b, &mut t, 3.0, &mut wire);
        {
            let s = shared.lock().unwrap();
            let ticks: Vec<u64> = s.sent.iter().filter_map(|m| match m { HostMsg::Tick { now } => Some(*now), _ => None }).collect();
            assert!(ticks.windows(2).all(|w| w[1] == w[0] + 1), "one tick per stamp step, none skipped: {ticks:?}");
            assert!(*ticks.last().unwrap() >= 30);
            let play = s.frames.iter().find(|p| lifetime_of(p) == Some(4)).expect("the cinematic hold");
            let (start, counter, on) = cinematic_of(play, 0).expect("cinematic 0 in the same delta");
            assert_eq!((counter, on), (2, 1), "a new counter, playing");
            assert!(start >= 19, "start = the stamp - 1 ({start})");
        }

        // Idle: heartbeats every 3 s.
        let before = b.rows().heartbeats;
        run_for(&mut b, &mut t, 6.5, &mut wire);
        assert!(b.rows().heartbeats >= before + 2);
        assert!(wire.iter().any(|f| f.len() == 6), "heartbeats are the 6-byte live idle frame");

        // The client reports the first cinematic finished, naming its row; phase 1 is region 26.
        b.on_client_message(&incident(891, Some((CINE, 6, 0))), 165, HOST, t);
        run_for(&mut b, &mut t, 0.5, &mut wire);
        assert_eq!(b.take_region_request(), Some(26), "phase 1 wants region 26");
        b.on_client_message(&slice(26), 165, HOST, t);
        run_for(&mut b, &mut t, 4.5, &mut wire);
        let envelope = b.take_incident().expect("the completion envelope");
        assert_eq!(b.take_incident(), None);
        let u32_at_e = |at: usize| u32::from_le_bytes(envelope[at..at + 4].try_into().unwrap());
        assert_eq!(envelope.len(), crate::completion::ENVELOPE_SIZE);
        assert_eq!([u32_at_e(0), u32_at_e(4), u32_at_e(8), u32_at_e(12), u32_at_e(16)], [4, 965, 2937, 2009, 614], "cinematic-only: no 3235");
        assert_eq!(u32_at_e(0xD4), 91, "one 725-bit body");
        {
            let s = shared.lock().unwrap();
            let (tag, body) = &s.bodies[0];
            assert_eq!(*tag, 0x8080_0595);
            assert_eq!(u32_at(body, 0x44), 0x0BBD_0E26, "the activity hash from the table");
            assert_eq!(u32_at(body, 0x4c), 0x4D78_882E);
            assert_eq!(&body[0x10..0x18], HOST.to_le_bytes(), "this host");
            assert_eq!(&body[0x18..0x20], [7; 8], "the joined player");
            assert_eq!(&body[0x68..0x70], [0xFF; 8], "cinematic-only counters -1, -1");
        }
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
        b.on_client_message(&incident(891, None), 165, HOST, t);
        run_for(&mut b, &mut t, 1.0, &mut wire);
        let s = shared.lock().unwrap();
        assert!(s.sent.iter().any(|m| matches!(m, HostMsg::CinematicFinished { type_index: 1, .. })));
        let mut seq: Vec<i8> = s.frames.iter().filter_map(lifetime_of).collect();
        seq.dedup();
        assert_eq!(seq, [0, 1, 4, 6, 8], "cinematic-only: 0 -> 1 -> 4 -> 6 -> 8, results and ended in frames of their own");
        // ... in one tick: the 8 frame carries the 6 frame's stamp.
        let coded: Vec<&Vec<u8>> = wire.iter().filter(|f| f.len() == 7).collect();
        assert_eq!(coded.len(), s.frames.len(), "every encoded frame was sent, in order");
        let stamp = |f: &[u8]| ((u64::from_be_bytes([0, 0, 0, f[1], f[2], f[3], f[4], f[5]]) >> 7) & 0xFFFF_FFFF) as u32;
        let at = |v: i8| s.frames.iter().rposition(|p| lifetime_of(p) == Some(v)).unwrap();
        assert_eq!(stamp(coded[at(6)]), stamp(coded[at(8)]));
        assert_eq!(s.rewards.len(), 1, "rewards once, for the one character");
        assert_eq!((s.rewards[0].0.identity[0], s.rewards[0].1, s.rewards[0].2), (7, 0x0BBD_0E26, ActivityOutcome::Completed));
        // ... at the results: the 6 had been planned, the 8 not yet.
        let granted_at = s.rewarded_after_frames[0];
        assert!(s.frames[..granted_at].iter().all(|p| lifetime_of(p) != Some(8)), "granted before the end");
        assert_eq!(s.frames[..granted_at].iter().filter_map(lifetime_of).next_back(), Some(4), "granted as 6 was set, before its frame");
        let ticks = s.sent.iter().filter(|m| matches!(m, HostMsg::Tick { .. })).count();
        drop(s);
        run_for(&mut b, &mut t, 3.0, &mut wire);
        let after = shared.lock().unwrap().sent.iter().filter(|m| matches!(m, HostMsg::Tick { .. })).count();
        assert_eq!(after, ticks, "no ticks after the end");
        // Every frame the bridge returned is stamped (the 10 Hz clock, nondecreasing).
        let stamps: Vec<u32> = wire.iter().map(|f| ((u64::from_be_bytes([0, 0, 0, f[1], f[2], f[3], f[4], f[5]]) >> 7) & 0xFFFF_FFFF) as u32).collect();
        assert!(stamps.windows(2).all(|w| w[0] <= w[1]), "{stamps:?}");
    }

    // ── the reveal as the full-game profile serves it (runs fg2 and fg3b, 6 November) ──

    fn fixtures() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/56578")
    }

    /// The launch parameters as lab-base.env builds them: `D1_AH_SENSOR_PHASE=3` (the knob that
    /// arms the sensor path; its value is not the reveal's first lifetime).
    fn lab_base() -> d1_oracle::sensor::SensorPhaseUpdate {
        d1_oracle::sensor::SensorPhaseUpdate { phase: 3, name_hash: 0xC2DA_CAF7, ..base() }
    }

    /// Activity 165 from the profile's own tables: stream K's `@generated start=25` row, the
    /// generated worlds and slice states, and lab-base's hand world table (`world-table-cine-live.txt`,
    /// the live list run fg3b served).
    fn profile_reveal(with_world_table: bool) -> ScriptedActivity {
        let f = fixtures();
        let mut worlds = crate::generated::GeneratedWorlds::load(&f.join("worlds")).expect("the generated worlds");
        worlds.load_slice_states(&std::fs::read_to_string(f.join("scripts/slice-states.tsv")).unwrap()).unwrap();
        let table = std::fs::read_to_string(f.join("scripts/activities.txt")).unwrap();
        let line = table.lines().find(|l| l.starts_with("165 ")).expect("the table's 165 row");
        let a = ScriptedActivity::parse_with(line, &lab_base(), &|_| Err("no file".into()), Some(&worlds)).unwrap();
        let mut activities = HashMap::from([(165, a)]);
        if with_world_table {
            let mission = f.join("mission");
            let world_table = std::fs::read_to_string(mission.join("world-table-cine-live.txt")).unwrap();
            let read = |p: &str| std::fs::read_to_string(mission.join(p)).map_err(|e| e.to_string());
            let lines = apply_world_table(&mut activities, &world_table, &lab_base(), &read);
            assert_eq!(lines.len(), 1, "one row of the table names a scripted activity: {lines:?}");
            assert!(lines[0].starts_with("⭐"), "{}", lines[0]);
            // The profile's D1_AH_SCRIPTS_SLICE_LISTS: the step schedule that plays.
            let steps = std::fs::read_to_string(mission.join("cine-reveal-steps.txt")).unwrap();
            let lines = apply_slice_lists(&mut activities, &steps, &lab_base(), &read);
            assert_eq!(lines.len(), 1, "only the move-26 list adds a record: {lines:?}");
            assert!(lines[0].contains("1 served values from cine_reveal-move-26.txt (slice set 26)"), "{}", lines[0]);
        }
        activities.remove(&165).unwrap()
    }

    /// The profile's `@generated` row for `index`, with the committed tables.
    fn profile_activity(index: u16) -> (ScriptedActivity, crate::generated::GeneratedWorlds) {
        let f = fixtures();
        let mut worlds = crate::generated::GeneratedWorlds::load(&f.join("worlds")).expect("the generated worlds");
        worlds.load_slice_states(&std::fs::read_to_string(f.join("scripts/slice-states.tsv")).unwrap()).unwrap();
        let table = std::fs::read_to_string(f.join("scripts/activities.txt")).unwrap();
        let line = table.lines().find(|l| l.starts_with(&format!("{index} "))).expect("the table's row");
        let a = ScriptedActivity::parse_with(line, &lab_base(), &|_| Err("no file".into()), Some(&worlds)).unwrap();
        (a, worlds)
    }

    /// D1_AH_SCRIPTS_PUBLIC_START_LIST (offline): Restoration's join list and relists.
    #[test]
    fn a_public_start_story_mission_serves_the_mission_hosts_list() {
        let (mut a, worlds) = profile_activity(166);
        assert_eq!(a.rows.len() + 1, 351, "knob off: the public start's whole list");
        let n = a.use_public_start_list(&worlds, &lab_base()).unwrap();
        assert_eq!(n, Some(67));
        assert_eq!(a.rows.len(), 66, "the lifetime record is not among the rows");
        assert!(a.rows.iter().all(|(k, _)| ![0xd7f6_8da0, 0xed3c_5b0a, 0xeaaf_16e2].contains(&k.group)));
        // A private slice set's relist: its list and the shared rows (100 with the lifetime).
        assert_eq!(a.slice_list(104).map(|l| l.len() + 1), Some(100));
        // A public slice set: no relist.
        assert_eq!(a.slice_list(48), None);
        assert_eq!(a.slice_list(56), None);
    }

    /// A bridge on Restoration's mission host list (the table's 166 under the test's key 165), joined.
    fn mission_host_bridge() -> (ScriptBridge, Instant) {
        let (mut a, worlds) = profile_activity(166);
        a.use_public_start_list(&worlds, &lab_base()).unwrap();
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge_of(&shared, false, false, a);
        let t = Instant::now();
        b.on_client_message(&join(7), 165, HOST, t);
        assert_eq!(b.run.as_ref().map(|r| r.list_slice), Some(Some(56)), "the join list is the public start's slot");
        (b, t)
    }

    fn phase_in(region: Option<u32>) -> SidecarMsg {
        SidecarMsg::Phase { index: 1, name: Some("hangar".into()), name_hash: None, slice_hash: None, region }
    }

    #[test]
    fn the_mission_host_relists_the_public_start_once_when_the_client_is_there() {
        let (a, worlds) = profile_activity(166);
        let mut a = a;
        a.use_public_start_list(&worlds, &lab_base()).unwrap();
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge_of(&shared, false, false, a);
        let (mut t, mut wire) = (Instant::now(), Vec::new());
        b.on_client_message(&join(7), 165, HOST, t);
        run_for(&mut b, &mut t, 0.25, &mut wire);
        let fulls = |s: &Shared| s.frames.iter().filter(|p| p.full).count();
        let (n0, last_len) = {
            let s = shared.lock().unwrap();
            (fulls(&s), s.frames.iter().rev().find(|p| p.full).unwrap().entries.len())
        };
        assert_eq!(last_len, 67, "the join list");
        // (b) a public slice set that is not the start: nothing.
        b.on_client_message(&slice(48), 165, HOST, t);
        run_for(&mut b, &mut t, 0.5, &mut wire);
        assert_eq!(fulls(&shared.lock().unwrap()), n0, "no relist into 48");
        // (a) the client reports the start slice set: its globals and the shared rows, once.
        b.on_client_message(&slice(56), 165, HOST, t);
        run_for(&mut b, &mut t, 0.5, &mut wire);
        {
            let s = shared.lock().unwrap();
            assert_eq!(fulls(&s), n0 + 1, "one full relist");
            let full = s.frames.iter().rev().find(|p| p.full).unwrap();
            assert!(full.init, "an init frame");
            eprintln!("start relist: {} rows", full.entries.len());
            assert_eq!(full.entries.len(), 70, "67 + the start's player-bundle variant rows");
            let keys: Vec<RowKey> = full.entries.iter().map(|(k, _)| *k).collect();
            for (kind, index) in [(0x4, 0), (0x1d, 22), (0x1c, 23)] {
                assert!(keys.contains(&RowKey { group: 0x1200_a031, kind, index }), "{kind:#x}/{index}");
            }
            assert!(keys.iter().all(|k| ![0xd7f6_8da0, 0xed3c_5b0a, 0xeaaf_16e2].contains(&k.group)));
        }
        b.on_client_message(&slice(56), 165, HOST, t);
        b.on_client_message(&slice(48), 165, HOST, t);
        b.on_client_message(&slice(56), 165, HOST, t);
        run_for(&mut b, &mut t, 0.5, &mut wire);
        assert_eq!(fulls(&shared.lock().unwrap()), n0 + 1, "a later return to the start does not relist again");
    }

    #[test]
    fn an_empty_name_report_of_the_public_start_still_relists_it_once() {
        for knob in [true, false] {
            let (mut a, worlds) = profile_activity(166);
            if knob {
                a.use_public_start_list(&worlds, &lab_base()).unwrap();
            }
            let shared = Arc::new(Mutex::new(Shared::default()));
            let mut b = bridge_of(&shared, false, false, a);
            let (mut t, mut wire) = (Instant::now(), Vec::new());
            b.on_client_message(&join(7), 165, HOST, t);
            run_for(&mut b, &mut t, 0.25, &mut wire);
            let fulls = |s: &Shared| s.frames.iter().filter(|p| p.full).count();
            let n0 = fulls(&shared.lock().unwrap());
            for _ in 0..2 {
                b.on_client_message(&slice_unnamed(56), 165, HOST, t);
                run_for(&mut b, &mut t, 0.5, &mut wire);
            }
            let s = shared.lock().unwrap();
            if knob {
                assert_eq!(fulls(&s), n0 + 1, "one relist for two reports");
                let full = s.frames.iter().rev().find(|p| p.full).unwrap();
                assert!(full.init);
                assert_eq!(full.entries.len(), 70);
            } else {
                assert_eq!(fulls(&s), n0, "knob off: nothing");
            }
            assert_eq!(b.run.as_ref().unwrap().current_slice, None, "the hash guard still keeps the report out of current_slice");
        }
    }

    #[test]
    fn the_mission_host_relists_only_for_a_slice_set_a_phase_names() {
        // (a) a private slice set no phase names: no relist.
        let (mut b, t) = mission_host_bridge();
        b.on_client_message(&slice(64), 165, HOST, t);
        assert_eq!(b.run.as_ref().unwrap().list_slice, Some(56), "private, not a phase's: no relist");
        // A public slice set: none either.
        b.on_client_message(&slice(48), 165, HOST, t);
        assert_eq!(b.run.as_ref().unwrap().list_slice, Some(56));
        // (b) the phase names 104 first, then the client moves in: the relist.
        let (mut b, t) = mission_host_bridge();
        b.apply(phase_in(Some(104)), t);
        assert_eq!(b.run.as_ref().unwrap().list_slice, Some(56), "the client is not in 104 yet");
        b.on_client_message(&slice(64), 165, HOST, t);
        assert_eq!(b.run.as_ref().unwrap().list_slice, Some(56));
        b.on_client_message(&slice(104), 165, HOST, t);
        assert_eq!(b.run.as_ref().unwrap().list_slice, Some(104));
        assert_eq!(b.run.as_ref().unwrap().activity.slice_list(104).map(|l| l.len() + 1), Some(100));
        // (c) the client moves in first, then the phase resolves to 104: relisted once.
        let (mut b, t) = mission_host_bridge();
        b.on_client_message(&slice(104), 165, HOST, t);
        assert_eq!(b.run.as_ref().unwrap().list_slice, Some(56), "no phase names 104 yet");
        b.apply(phase_in(Some(104)), t);
        assert_eq!(b.run.as_ref().unwrap().list_slice, Some(104));
        b.on_client_message(&slice(104), 165, HOST, t);
        assert_eq!(b.run.as_ref().unwrap().list_slice, Some(104), "once");
    }

    #[test]
    fn a_phase_that_names_no_slice_set_clears_the_phase_slice() {
        let (mut b, t) = mission_host_bridge();
        b.apply(phase_in(Some(104)), t);
        assert_eq!(b.run.as_ref().unwrap().phase_slice, Some(104));
        b.apply(phase_in(None), t);
        assert_eq!(b.run.as_ref().unwrap().phase_slice, None);
        b.on_client_message(&slice(104), 165, HOST, t);
        assert_eq!(b.run.as_ref().unwrap().list_slice, Some(56), "the current phase names no slice set: no relist");
    }

    /// Every activity of the profile's table, the knob on: the config loads, and an activity that
    /// cannot take the mission host's list keeps today's.
    #[test]
    fn every_public_start_activity_loads_with_the_knob_on() {
        let f = fixtures();
        let mut worlds = crate::generated::GeneratedWorlds::load(&f.join("worlds")).expect("the generated worlds");
        worlds.load_slice_states(&std::fs::read_to_string(f.join("scripts/slice-states.tsv")).unwrap()).unwrap();
        let table = std::fs::read_to_string(f.join("scripts/activities.txt")).unwrap();
        let mut cache = SliceRowsCache::new();
        let mut activities = HashMap::new();
        for line in table.lines().map(|l| l.split('#').next().unwrap_or("").trim()).filter(|l| !l.is_empty()) {
            let a = ScriptedActivity::parse_cached(line, &lab_base(), &|_| Err("no file".into()), Some(&worlds), &mut cache)
                .unwrap_or_else(|e| panic!("{e}"));
            activities.insert(a.index, a);
        }
        let before: HashMap<u16, usize> = activities.iter().map(|(i, a)| (*i, a.rows.len())).collect();
        let public: Vec<u16> = activities
            .values()
            .filter(|a| a.start.is_some_and(|s| worlds.slices.get(&(a.world.clone(), s)).is_some_and(|x| x.public)))
            .map(|a| a.index)
            .collect();
        let (given, fell_back) = apply_public_start_list(&mut activities, &worlds, &lab_base());
        eprintln!("{} activities, {} start public: {given} take the mission host list, {fell_back} fall back", activities.len(), public.len());
        assert_eq!(given + fell_back, public.len(), "every public-start activity is either given the list or falls back");
        assert!(given >= 2, "Restoration and The Dark Within at least ({given})");
        for (i, a) in &activities {
            if a.mission_list.is_none() {
                assert_eq!(a.rows.len(), before[i], "activity {i} keeps today's list");
            }
        }
        let smaller = activities.iter().filter(|(i, a)| a.mission_list.is_some() && a.rows.len() < before[*i]).count();
        eprintln!("{smaller} of the {given} mission host lists are smaller than the public start's whole list");
        assert!(activities[&166].rows.len() < before[&166]);
    }

    #[test]
    fn a_private_start_activity_is_unaffected_by_the_public_start_list() {
        let (mut a, worlds) = profile_activity(164);
        let before = (a.rows.clone().len(), a.slice_list(113).map(|l| l.len()));
        assert_eq!(a.use_public_start_list(&worlds, &lab_base()).unwrap(), None);
        assert!(a.mission_list.is_none());
        assert_eq!((a.rows.len(), a.slice_list(113).map(|l| l.len())), before);
    }

    #[test]
    fn the_reveal_starts_from_the_world_list_at_lifetime_0_not_the_launch_phase() {
        // Without the world table: the generated list names no @phase. Its lifetime record used to
        // start at the launch phase, 3 (run fg2: "lifetime 3 -> 1").
        let generated = profile_reveal(false);
        assert_eq!(generated.lifetime_state, 3, "params_for_list falls back to D1_AH_SENSOR_PHASE");
        assert_eq!(generated.listed_phase, None);
        assert_eq!(generated.initial_lifetime(), 0, "the bridge starts at the live first value");
        assert_eq!(generated.rows.len(), 21, "21 sensor clients and the lifetime: fg2's 22 rows");
        assert!(generated.rows.iter().all(|(_, r)| r.a.is_none()), "the generated list carries no values");
        assert_eq!(generated.start, Some(25));

        // With it: the same 21 records, now with the world list's values, and its @phase 0.
        let reveal = profile_reveal(true);
        assert_eq!(reveal.listed_phase, Some(0));
        assert_eq!(reveal.initial_lifetime(), 0);
        let live = crate::sensor_list::params_for_list(
            &lab_base(),
            GROUP,
            -1,
            &std::fs::read_to_string(fixtures().join("mission/sensor-clients-cine_reveal-live.txt")).unwrap(),
        )
        .unwrap();
        assert_eq!(live.clients.len(), 21, "the live list: 21 sensor clients (fg3b's step 0)");
        let want: Vec<(RowKey, Row)> = live.clients.iter().map(Row::from_client).collect();
        assert_eq!(reveal.rows, want, "every record of the world's list, in its order, with its values");
        let get = |k: RowKey| reveal.rows.iter().find(|(r, _)| *r == k).map(|(_, r)| r.clone()).unwrap();
        let cine = get(RowKey { group: CINE, kind: 6, index: 0 });
        assert_eq!(cine.a.as_ref().map(|(t, i)| (*t, u32_at(i, 4), i[8])), Some((TAG_CINEMATIC_FOR_TEST, 1, 0)), "/6/0 idle, counter 1");
        let player = get(RowKey { group: GROUP, kind: 0xC, index: 5 });
        let (tag, image) = player.a.unwrap();
        assert_eq!(tag, 0x8080_22A5, "the player bundle's spawn value");
        assert_eq!(&image[0x38..0x3C], &(-1i32).to_le_bytes(), "spawn team -1 for a story activity");
        assert_eq!(reveal.lifetime, RowKey { group: GROUP, kind: 0x10, index: 3 });
        assert!(!reveal.rows.iter().any(|(k, _)| *k == reveal.lifetime), "the lifetime is the bridge's own row");
        // The slice lists stay the generated ones (slice set 26 names /6/1).
        assert!(reveal.slice_rows.get(&26).is_some_and(|r| r.iter().any(|(k, _)| *k == RowKey { group: CINE, kind: 6, index: 1 })));
    }

    const TAG_CINEMATIC_FOR_TEST: u32 = 0x8080_254D;

    #[test]
    fn a_world_list_merges_into_the_bridges_own_rows() {
        let mut a = ScriptedActivity::parse(CINE_LINE, &base(), &|_| Ok(LIST.to_string())).unwrap();
        let image = vec![7u8; 0xD4];
        let world = format!("@phase 0\n0x2fe7dbe6 0x6 0 1 0 0x8080254D {}\n0x4786c0e0 0x22 1 1 0\n0x4786c0e0 0x10 3 1 0\n", "07".repeat(0xD4));
        let (from_world, only_ours, valued) = a.merge_world_list(&base(), GROUP, &world, Some(25)).unwrap();
        assert_eq!((from_world, only_ours, valued), (2, 2, 1), "/6/0 and /0x22/1 from the world; /6/1 and /0xf/0 kept");
        let keys: Vec<RowKey> = a.rows.iter().map(|(k, _)| *k).collect();
        assert_eq!(
            keys,
            [
                RowKey { group: CINE, kind: 6, index: 0 },
                RowKey { group: GROUP, kind: 0x22, index: 1 },
                RowKey { group: CINE, kind: 6, index: 1 },
                RowKey { group: GROUP, kind: 0xF, index: 0 },
            ],
            "the world's records first, in its order, then the rows only the bridge names"
        );
        assert_eq!(a.rows[0].1.a, Some((TAG_CINEMATIC_FOR_TEST, image)), "the world list's value");
        assert_eq!(a.authority, ScriptedActivity::parse(CINE_LINE, &base(), &|_| Ok(LIST.to_string())).unwrap().authority, "no @authority: the bridge's stands");
        assert_eq!(a.start, Some(25), "the world table's start stands in for a missing start=");
        // A world table for another group or lifetime record is refused, and nothing changes.
        let before = a.rows.clone();
        assert!(a.merge_world_list(&base(), 0x1234, &world, None).is_err());
        assert!(a.merge_world_list(&base(), GROUP, "0x4786c0e0 0x10 2 1 0\n", None).is_err(), "the Tower's sub 2");
        assert_eq!(a.rows, before);
        // A world table row for another world is not used.
        let mut acts = HashMap::from([(165, a)]);
        let lines = apply_world_table(&mut acts, "165 city_tower_default1 0x4786c0e0 x.txt 0\n7 cine_reveal 0x1 y.txt", &base(), &|_| Ok(world.clone()));
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with('⚠'), "{}", lines[0]);
    }

    #[test]
    fn the_start_delay_knob_reads_seconds() {
        assert_eq!(start_delay_from(None), Ok(Duration::from_secs(60)), "a 60 s stand-in by default");
        assert_eq!(start_delay_from(Some(" ")), Ok(Duration::from_secs(60)));
        assert_eq!(start_delay_from(Some("0")), Ok(Duration::ZERO));
        assert_eq!(start_delay_from(Some("2.5")), Ok(Duration::from_millis(2500)));
        assert!(start_delay_from(Some("-1")).is_err());
        assert!(start_delay_from(Some("soon")).is_err());
    }

    /// Runs fg2 and fg3b: the bridge must start the reveal the way the working step schedule did.
    /// The first full list is the world's own (21 records with their values) at lifetime 0; for
    /// the start cushion only heartbeats follow (no phase, no lifetime past 0, no teleport, nothing
    /// to the scripts but `hello`); then pregame 1 and the hold 4; phase 0 runs in the start slice
    /// set 25 the client loaded, so no teleport; phase 1 moves it to 26; and the live order
    /// 0 → 1 → 4 → 6 → 8.
    #[test]
    fn the_reveal_waits_for_the_cushion_then_plays_in_the_live_order() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let reveal = profile_reveal(true);
        assert_eq!(reveal.region_of_phase(0, crate::incident_catalog::fnv1("cine_106_trv_phase"), None), Some(25), "content puts phase 0 in 25");
        let mut b = bridge_delayed(&shared, false, false, reveal, Duration::from_secs(60));
        // The grants go through stream O's forwarder to the store's command file, as in the profile.
        let dir = std::env::temp_dir().join(format!("d1-reveal-grants-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let control = dir.join("store-control.txt");
        let _ = std::fs::remove_file(&control);
        {
            let c = Arc::get_mut(&mut b.config).expect("one owner");
            c.rewards = Arc::new(Mutex::new(Forwarding(crate::reward_forward::GrantForwarder::new(&control), shared.clone())));
            c.arrival_cushion = Duration::from_secs(20);
        }
        b.set_served_start(Some(25));
        let mut t = Instant::now();
        let mut wire = Vec::new();
        b.on_client_message(&join(7), 165, HOST, t);
        run_for(&mut b, &mut t, 0.25, &mut wire);
        {
            let s = shared.lock().unwrap();
            assert_eq!(s.frames.len(), 1);
            let full = &s.frames[0];
            assert!(full.full && full.init);
            assert_eq!(full.entries.len(), 22, "the world's 21 records and the lifetime");
            assert_eq!(lifetime_of(full), Some(0), "lifetime 0 first, never the launch phase 3");
            assert_eq!(cinematic_of(full, 0), Some((0, 1, 0)), "/6/0 with the world list's idle value");
            let player = full.entries.iter().find(|(k, _)| k.kind == 0xC && k.index == 5).unwrap();
            assert_eq!(player.1.a.as_ref().map(|(t, _)| *t), Some(0x8080_22A5), "the spawn value goes out");
        }

        // The cushion: the client loads its initial slice set. A client incident waits.
        run_for(&mut b, &mut t, 30.0, &mut wire);
        b.on_client_message(&incident(566, None), 165, HOST, t);
        run_for(&mut b, &mut t, 29.0, &mut wire);
        {
            let s = shared.lock().unwrap();
            assert_eq!(s.sent.len(), 1, "only hello before the start: {:?}", s.sent);
            assert!(matches!(s.sent[0], HostMsg::Hello { .. }));
            assert_eq!(s.frames.len(), 1, "no delta in the cushion");
            assert!(wire[1..].iter().all(|f| f.len() == 6), "heartbeats only");
            assert!(wire.len() >= 15, "a heartbeat every 3 s ({})", wire.len());
        }
        assert_eq!(b.take_region_request(), None, "no teleport during the initial load");

        // +60 s: players, start, then the waiting incident; pregame 1, then the hold 4.
        run_for(&mut b, &mut t, 2.0, &mut wire);
        {
            let s = shared.lock().unwrap();
            let start = s.sent.iter().position(|m| matches!(m, HostMsg::Start)).expect("the scripts start after the cushion");
            let before = if matches!(s.sent[start - 1], HostMsg::ActiveBundles { .. }) { start - 2 } else { start - 1 };
            assert!(matches!(s.sent[before], HostMsg::Players { ref players } if players.len() == 1), "players, (F44: the start's active bundles,) then start");
            assert!(matches!(s.sent[start + 1], HostMsg::PlayerSpawned { .. }), "the incident from the cushion follows the start");
            let mut seq: Vec<i8> = s.frames.iter().filter_map(lifetime_of).collect();
            seq.dedup();
            assert_eq!(seq, [0, 1, 4], "pregame and the hold in frames of their own");
            let play = s.frames.iter().find(|p| lifetime_of(p) == Some(4)).unwrap();
            let (_, counter, on) = cinematic_of(play, 0).expect("/6/0 plays at the hold");
            assert_eq!((counter, on), (2, 1), "the world value's counter 1 advanced");
        }
        assert_eq!(b.take_region_request(), None, "phase 0 is region 25, the start slice set the client loaded");

        // The first cinematic finishes: phase 1 is region 26 (content's slice states). Run fg2c:
        // the client never reports a slice set (no udt 21), so the list must follow the move.
        let tower = RowKey { group: CINE, kind: 6, index: 1 };
        let before = shared.lock().unwrap().frames.len();
        b.on_client_message(&incident(891, Some((CINE, 6, 0))), 165, HOST, t);
        run_for(&mut b, &mut t, 0.5, &mut wire);
        assert_eq!(b.take_region_request(), Some(26), "phase 1 moves the client to 26");
        {
            let s = shared.lock().unwrap();
            let moved = s.frames[before..].iter().find(|p| p.full).expect("the move sends slice set 26's list in full");
            assert!(moved.init);
            let keys: std::collections::BTreeSet<RowKey> = moved.entries.iter().map(|(k, _)| *k).collect();
            let want: std::collections::BTreeSet<RowKey> =
                move_26_list().iter().map(|(k, _)| *k).chain([RowKey { group: GROUP, kind: 0x10, index: 3 }]).collect();
            assert_eq!(keys, want, "the step schedule's move-26 list, record for record, and the lifetime");
            assert_eq!(cinematic_of(moved, 1).map(|c| (c.1, c.2)), Some((1, 0)), "/6/1 idle, its current value");
            // Run fg2d: /6/1 is the step schedule's idle image byte for byte (its one-element cast).
            let image = |rows: &[(RowKey, Row)]| rows.iter().find(|(k, _)| *k == tower).and_then(|(_, r)| r.a.clone());
            assert_eq!(image(&moved.entries), image(&move_26_list()), "/6/1 idle = cine_reveal-move-26.txt's");
            assert_eq!(lifetime_of(moved), Some(4), "the current lifetime");
        }
        assert!(b.rows().is_listed(&tower), "/6/1 is in the list the client holds");

        // Run fg2e: the scripts' sleep(4) and the Tower play wait while the client loads 26. The
        // client reports nothing; the host's teleport request finishes on its 30 s timers.
        let ticks = |s: &Shared| s.sent.iter().filter(|m| matches!(m, HostMsg::Tick { .. })).count();
        let playing = |s: &Shared| s.frames.iter().any(|p| cinematic_of(p, 1).is_some_and(|c| c.2 == 1));
        let fulls = |s: &Shared| s.frames.iter().filter(|p| p.full).count();
        let (ticks_at_move, fulls_at_move) = {
            let s = shared.lock().unwrap();
            (ticks(&s), fulls(&s))
        };
        run_for(&mut b, &mut t, 29.0, &mut wire);
        assert!(b.holding_for_arrival());
        {
            let s = shared.lock().unwrap();
            assert_eq!(ticks(&s), ticks_at_move, "no script time while the client moves");
            assert!(!playing(&s), "no Tower play before the arrival");
        }
        b.teleport_finished(26, t);
        run_for(&mut b, &mut t, 0.5, &mut wire);
        {
            let s = shared.lock().unwrap();
            assert_eq!(fulls(&s), fulls_at_move + 1, "on arrival slice set 26's list goes out in full again");
            let again = s.frames.iter().rev().find(|p| p.full).unwrap();
            assert_eq!(again.entries.len(), 23, "the 22 records and the lifetime, current values");
            assert!(again.entries.iter().any(|(k, _)| *k == tower));
        }
        run_for(&mut b, &mut t, 19.0, &mut wire);
        {
            let s = shared.lock().unwrap();
            assert_eq!(ticks(&s), ticks_at_move, "the arrival cushion holds the clock too");
            assert!(!playing(&s));
        }
        assert!(b.holding_for_arrival());
        run_for(&mut b, &mut t, 5.0, &mut wire);
        assert!(!b.holding_for_arrival(), "20 s after the arrival the scripts run again");
        {
            let s = shared.lock().unwrap();
            let play = s.frames.iter().rev().find(|p| cinematic_of(p, 1).is_some_and(|c| c.2 == 1)).expect("the Tower cinematic plays");
            assert!(!play.full, "a delta, once the client holds the record");
            assert_eq!(cinematic_of(play, 1).map(|c| c.1), Some(2), "counter 1 -> 2");
            // The play is built on that idle image: cine_reveal-play-1.txt's /6/1 byte for byte but
            // the start (+0), which is the stamp - 1 as the live hosts sent it (the fixture's 0;
            // /6/0's play was built the same way and the client started it in fg2c and fg2d).
            let (tag, ours) = play.entries.iter().find(|(k, _)| *k == tower).and_then(|(_, r)| r.a.clone()).unwrap();
            let (want_tag, want) = play_1_list().into_iter().find(|(k, _)| *k == tower).and_then(|(_, r)| r.a).unwrap();
            assert_eq!(tag, want_tag);
            assert_eq!(ours.len(), want.len());
            assert_eq!(&ours[4..], &want[4..], "counter 2, playing, and the same cast");
            assert_eq!(&want[..4], &[0; 4]);
            assert!(u32_at(&ours, 0) > 0, "start = the stamp - 1");
            assert_eq!(ours[0x9C], 1, "a one-element cast");
        }
        assert!(b.take_incident().is_some(), "the completion envelope");

        // It finishes: results 6 (the grant, through the forwarder), then 8 on 6's stamp, and the
        // hand-off to the Tower.
        b.on_client_message(&incident(891, None), 165, HOST, t);
        run_for(&mut b, &mut t, 1.0, &mut wire);
        let s = shared.lock().unwrap();
        let mut seq: Vec<i8> = s.frames.iter().filter_map(lifetime_of).collect();
        seq.dedup();
        assert_eq!(seq, [0, 1, 4, 6, 8], "the live order");
        let coded: Vec<&Vec<u8>> = wire.iter().filter(|f| f.len() == 7).collect();
        assert_eq!(coded.len(), s.frames.len());
        let stamp = |f: &[u8]| ((u64::from_be_bytes([0, 0, 0, f[1], f[2], f[3], f[4], f[5]]) >> 7) & 0xFFFF_FFFF) as u32;
        let at = |v: i8| s.frames.iter().rposition(|p| lifetime_of(p) == Some(v)).unwrap();
        assert_eq!(stamp(coded[at(6)]), stamp(coded[at(8)]), "6 and 8 in one tick");
        assert_eq!(s.rewards.len(), 1);
        assert_eq!((s.rewards[0].1, s.rewards[0].2), (0x6EAF_0959, ActivityOutcome::Completed));
        let granted_at = s.rewarded_after_frames[0];
        assert!(s.frames[..granted_at].iter().all(|p| lifetime_of(p) != Some(8)), "granted at 6, before 8");
        // Run fg2j: the start went at the join (before any frame), the end after the 8 frame.
        let started = s.lifecycle.iter().find(|l| l.0 == "started").expect("a start");
        assert_eq!((started.1, started.2), (0x6EAF_0959, 0), "at the join, before the first frame");
        let ended = s.lifecycle.iter().find(|l| l.0 == "ended").expect("an end");
        assert!(s.frames[..ended.2].iter().any(|p| lifetime_of(p) == Some(8)), "after the 8 went out");
        // Run fg2h: the completion was recorded at activity_complete, long before the results:
        // while the lifetime was still 4, before the Tower cinematic played.
        assert_eq!(s.completions.len(), 1);
        let (_, hash, completed_at) = s.completions[0];
        assert_eq!(hash, 0x6EAF_0959);
        assert!(s.frames[..completed_at].iter().all(|p| lifetime_of(p).is_none_or(|v| v <= 4)), "recorded before 6");
        assert!(s.frames[..completed_at].iter().all(|p| cinematic_of(p, 1).is_none_or(|c| c.2 == 0)), "before /6/1 plays");
        assert_eq!(s.hand_offs, vec![(u64::from_le_bytes([7; 8]), 0x6EAF_0959, 0, 0x8AE8_DA69)], "the hand-off to the Tower");
        drop(s);
        let text = (0..200)
            .map(|_| {
                std::thread::sleep(Duration::from_millis(10));
                std::fs::read_to_string(&control).unwrap_or_default()
            })
            .find(|t| t.lines().count() >= 5)
            .unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 5, "{text}");
        // Run fg2j: the start at the join; run fg2h: the completion at activity_complete; then the
        // grant at 6, the hand-off, and the end after the 8.
        assert!(lines[0].starts_with("started id:0707") && lines[0].ends_with(" 0x6eaf0959"), "{}", lines[0]);
        assert!(lines[1].starts_with("completed id:0707") && lines[1].ends_with(" 0x6eaf0959"), "{}", lines[1]);
        assert!(lines[2].starts_with("grant id:0707") && lines[2].ends_with(" 0x6eaf0959 completed source=script"), "{}", lines[2]);
        assert!(lines[3].starts_with("handoff id:0707") && lines[3].ends_with(" 0 from=0x6eaf0959 hash=0x8ae8da69"), "{}", lines[3]);
        assert!(lines[4].starts_with("ended id:0707") && lines[4].ends_with(" 0x6eaf0959"), "{}", lines[4]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The step schedule's list at the move to 26 (`cine_reveal-move-26.txt`), as the world table
    /// builds it.
    fn move_26_list() -> Vec<(RowKey, Row)> {
        let text = std::fs::read_to_string(fixtures().join("mission/cine_reveal-move-26.txt")).unwrap();
        let p = crate::sensor_list::params_for_list(&lab_base(), GROUP, -1, &text).unwrap();
        p.clients.iter().map(Row::from_client).collect()
    }

    /// Run fg2e: a phase's move holds the script clock until the client arrives. Here the client
    /// reports slice set 26 itself (udt 21): the list goes out in full again at once, and the clock
    /// runs after the arrival cushion. A move with no arrival at all is taken as arrived after
    /// [`ARRIVAL_TIMEOUT`].
    #[test]
    fn a_phase_move_holds_the_script_clock_until_the_client_arrives() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge_delayed(&shared, false, false, profile_reveal(true), Duration::ZERO);
        Arc::get_mut(&mut b.config).unwrap().arrival_cushion = Duration::from_secs(2);
        b.set_served_start(Some(25));
        let mut t = Instant::now();
        b.on_client_message(&join(3), 165, HOST, t);
        run_for(&mut b, &mut t, 3.0, &mut Vec::new());
        assert!(!b.holding_for_arrival(), "phase 0 in the start slice set holds nothing");
        b.on_client_message(&incident(891, Some((CINE, 6, 0))), 165, HOST, t);
        run_for(&mut b, &mut t, 0.5, &mut Vec::new());
        assert_eq!(b.take_region_request(), Some(26));
        assert!(b.holding_for_arrival());
        let tower = |s: &Shared| s.frames.iter().any(|p| cinematic_of(p, 1).is_some_and(|c| c.2 == 1));
        run_for(&mut b, &mut t, 10.0, &mut Vec::new());
        assert!(!tower(&shared.lock().unwrap()), "held: the scripts' 4 s sleep has not run");
        b.teleport_finished(25, t);
        assert!(b.holding_for_arrival(), "another region's teleport is not this arrival");
        let fulls = shared.lock().unwrap().frames.iter().filter(|p| p.full).count();
        b.on_client_message(&slice(26), 165, HOST, t);
        run_for(&mut b, &mut t, 0.5, &mut Vec::new());
        assert_eq!(shared.lock().unwrap().frames.iter().filter(|p| p.full).count(), fulls + 1, "the full list again on arrival");
        run_for(&mut b, &mut t, 1.0, &mut Vec::new());
        assert!(b.holding_for_arrival(), "the cushion");
        assert!(!tower(&shared.lock().unwrap()));
        run_for(&mut b, &mut t, 1.5, &mut Vec::new());
        assert!(!b.holding_for_arrival());
        run_for(&mut b, &mut t, 1.0, &mut Vec::new());
        assert!(tower(&shared.lock().unwrap()), "then the Tower cinematic plays");

        // No arrival ever seen: the timeout releases the clock.
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge_delayed(&shared, false, false, profile_reveal(true), Duration::ZERO);
        b.set_served_start(Some(25));
        let mut t = Instant::now();
        b.on_client_message(&join(3), 165, HOST, t);
        run_for(&mut b, &mut t, 3.0, &mut Vec::new());
        b.on_client_message(&incident(891, Some((CINE, 6, 0))), 165, HOST, t);
        run_for(&mut b, &mut t, 119.0, &mut Vec::new());
        assert!(b.holding_for_arrival());
        run_for(&mut b, &mut t, 2.0, &mut Vec::new());
        assert!(!b.holding_for_arrival(), "taken as arrived after the timeout");
    }

    /// F37 (run agr3): the live host moved the client only within its bubble; a phase in another
    /// bubble waits for the client to walk there, with no teleport and no held clock.
    #[test]
    fn a_phase_in_another_bubble_is_walked_to_not_teleported() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge_delayed(&shared, false, false, profile_reveal(true), Duration::ZERO);
        b.set_served_start(Some(25));
        let mut t = Instant::now();
        b.on_client_message(&join(3), 165, HOST, t);
        run_for(&mut b, &mut t, 3.0, &mut Vec::new());
        b.on_client_message(&slice(8), 165, HOST, t);
        run_for(&mut b, &mut t, 0.5, &mut Vec::new());
        b.on_client_message(&incident(891, Some((CINE, 6, 0))), 165, HOST, t);
        run_for(&mut b, &mut t, 0.5, &mut Vec::new());
        assert_eq!(b.take_region_request(), None, "26 is in bubble 3, the client in bubble 1: no teleport");
        assert!(!b.holding_for_arrival(), "the scripts' clock runs on");
    }

    fn stamp_of(f: &[u8]) -> u32 {
        ((u64::from_be_bytes([0, 0, 0, f[1], f[2], f[3], f[4], f[5]]) >> 7) & 0xFFFF_FFFF) as u32
    }

    /// Run fg2f: its first full list went out at stamp 0 and was never answered. No frame is
    /// stamped 0: the first poll, right at the join, still stamps 1.
    #[test]
    fn no_frame_is_stamped_0() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge(&shared, false);
        let t = Instant::now();
        b.on_client_message(&join(1), 165, HOST, t);
        let first = b.sensor_due(t).expect("the full list at once");
        assert!(shared.lock().unwrap().frames[0].full);
        assert_eq!(stamp_of(&first), 1, "the stamp clock's 0 goes out as 1");
        let mut t = t;
        let mut wire = vec![first];
        run_for(&mut b, &mut t, 8.0, &mut wire);
        let stamps: Vec<u32> = wire.iter().map(|f| stamp_of(f)).collect();
        assert!(stamps.iter().all(|s| *s >= 1) && stamps.windows(2).all(|w| w[0] <= w[1]), "{stamps:?}");
    }

    /// Run fg2f: an unanswered first full list goes out again every `list_retry` (a fresh stamp
    /// each time) until the client's first udt 6, and the start cushion counts from that answer.
    #[test]
    fn an_unanswered_full_list_goes_out_again_and_the_start_waits_for_the_answer() {
        let fulls = |s: &Arc<Mutex<Shared>>| s.lock().unwrap().frames.iter().filter(|p| p.full).count();
        let started = |s: &Arc<Mutex<Shared>>| s.lock().unwrap().sent.iter().any(|m| matches!(m, HostMsg::Start));
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge_delayed(&shared, false, false, profile_reveal(true), Duration::from_secs(60));
        Arc::get_mut(&mut b.config).unwrap().list_retry = Some(Duration::from_secs(10));
        b.set_served_start(Some(25));
        let mut t = Instant::now();
        let mut wire = Vec::new();
        b.on_client_message(&join(2), 165, HOST, t);
        run_for(&mut b, &mut t, 0.25, &mut wire);
        assert_eq!(fulls(&shared), 1);
        run_for(&mut b, &mut t, 9.5, &mut wire);
        assert_eq!(fulls(&shared), 1, "not before 10 s");
        run_for(&mut b, &mut t, 0.5, &mut wire);
        assert_eq!(fulls(&shared), 2, "no udt 6 in 10 s: the full list again");
        run_for(&mut b, &mut t, 10.0, &mut wire);
        assert_eq!(fulls(&shared), 3, "and again");
        let coded: Vec<u32> = wire.iter().filter(|f| f.len() == 7).map(|f| stamp_of(f)).collect();
        assert!(coded.len() == 3 && coded.windows(2).all(|w| w[0] < w[1]), "each with a fresh stamp: {coded:?}");
        // The client answers (fg2e: 11 s after its list): no more re-sends; the cushion counts.
        b.client_sensed(t);
        assert!(b.client_answered());
        let answered = t;
        run_for(&mut b, &mut t, 30.0, &mut wire);
        assert_eq!(fulls(&shared), 3, "answered: nothing more");
        assert!(!started(&shared), "60 s from the answer, not from the first list (+20 s)");
        while t < answered + Duration::from_secs(59) {
            run_for(&mut b, &mut t, 1.0, &mut wire);
        }
        assert!(!started(&shared));
        run_for(&mut b, &mut t, 2.0, &mut wire);
        assert!(started(&shared), "60 s after the client's first udt 6");

        // Never answered: five re-sends, then the cushion counts anyway.
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge_delayed(&shared, false, false, profile_reveal(true), Duration::from_secs(60));
        Arc::get_mut(&mut b.config).unwrap().list_retry = Some(Duration::from_secs(10));
        let mut t = Instant::now();
        b.on_client_message(&join(2), 165, HOST, t);
        run_for(&mut b, &mut t, 70.0, &mut Vec::new());
        assert_eq!(fulls(&shared), 1 + LIST_RETRIES as usize, "the first list and {LIST_RETRIES} re-sends");
        assert!(b.client_answered(), "taken as answered after the last");
        run_for(&mut b, &mut t, 30.0, &mut Vec::new());
        assert_eq!(fulls(&shared), 1 + LIST_RETRIES as usize, "no more");
        assert!(!started(&shared));
        run_for(&mut b, &mut t, 35.0, &mut Vec::new());
        assert!(started(&shared), "the cushion after the last re-send");
    }

    #[test]
    fn the_list_retry_knob_reads_seconds_and_0_turns_it_off() {
        assert_eq!(list_retry_from(None), Ok(Some(Duration::from_secs(10))));
        assert_eq!(list_retry_from(Some("0")), Ok(None));
        assert_eq!(list_retry_from(Some("15")), Ok(Some(Duration::from_secs(15))));
        assert!(list_retry_from(Some("-2")).unwrap_err().contains("D1_AH_SCRIPTS_LIST_RETRY"));
    }

    /// Run fg2i: the client launched 164 and a remap served 165 (`D1_AH_ACTIVITY_REMAP=164:165`).
    /// `activity_complete` completes 165 and the launched 164 for the player; the grant and the
    /// hand-off stay 165's. Without a remap only 165 is completed.
    #[test]
    fn a_remapped_launch_is_completed_too_and_the_grant_stays_the_served_activitys() {
        const AGR: u32 = 0x6E0D_AE89;
        for remapped in [Some(AGR), None, Some(0x6EAF_0959)] {
            let shared = Arc::new(Mutex::new(Shared::default()));
            let mut b = bridge_delayed(&shared, false, false, profile_reveal(true), Duration::ZERO);
            b.set_served_start(Some(25));
            b.set_remapped_launch(remapped);
            let mut t = Instant::now();
            b.on_client_message(&join(5), 165, HOST, t);
            run_for(&mut b, &mut t, 3.0, &mut Vec::new());
            b.on_client_message(&incident(891, Some((CINE, 6, 0))), 165, HOST, t);
            run_for(&mut b, &mut t, 0.5, &mut Vec::new());
            b.teleport_finished(26, t);
            run_for(&mut b, &mut t, 6.0, &mut Vec::new());
            b.on_client_message(&incident(891, None), 165, HOST, t);
            run_for(&mut b, &mut t, 1.0, &mut Vec::new());
            let s = shared.lock().unwrap();
            let done: Vec<u32> = s.completions.iter().map(|c| c.1).collect();
            assert_eq!(done, [0x6EAF_0959], "the served 165 is the one completion that moves field [6]");
            let asides: Vec<u32> = s.lifecycle.iter().filter(|l| l.0 == "aside").map(|l| l.1).collect();
            match remapped {
                Some(AGR) => assert_eq!(asides, [AGR], "the launched 164, aside (run fg2j)"),
                _ => assert!(asides.is_empty(), "no remap (or the launch is the served one): nothing aside"),
            }
            let kinds: Vec<&str> = s.lifecycle.iter().filter(|l| l.0 != "aside").map(|l| l.0).collect();
            assert_eq!(kinds, ["started", "ended"], "the join's start, the 8's end");
            assert_eq!(s.rewards.iter().map(|r| r.1).collect::<Vec<_>>(), [0x6EAF_0959], "the grant: 165 only");
            assert_eq!(s.hand_offs.iter().map(|h| (h.1, h.2)).collect::<Vec<_>>(), [(0x6EAF_0959, 0)], "the hand-off: 165 to the Tower");
        }
    }

    #[test]
    fn the_arrival_cushion_knob_reads_seconds() {
        assert_eq!(arrival_cushion_from(None), Ok(Duration::from_secs(20)), "a 20 s stand-in by default");
        assert_eq!(arrival_cushion_from(Some("0")), Ok(Duration::ZERO));
        assert!(arrival_cushion_from(Some("x")).unwrap_err().contains("D1_AH_SCRIPTS_ARRIVAL_CUSHION"));
    }

    /// The step schedule's list when /6/1 plays (`cine_reveal-play-1.txt`).
    fn play_1_list() -> Vec<(RowKey, Row)> {
        let text = std::fs::read_to_string(fixtures().join("mission/cine_reveal-play-1.txt")).unwrap();
        let p = crate::sensor_list::params_for_list(&lab_base(), GROUP, -1, &text).unwrap();
        p.clients.iter().map(Row::from_client).collect()
    }

    /// Slice set 26's generated list has /6/1, and the world's list plus it is the step schedule's
    /// move-26 list record for record (22): the same records and flags, and the same values but
    /// the two cinematics' (move-26 serves /6/0 done and /6/1 idle; ours carry the script's).
    #[test]
    fn slice_set_26s_list_is_the_move_26_list_record_for_record() {
        let reveal = profile_reveal(true);
        let tower = RowKey { group: CINE, kind: 6, index: 1 };
        assert!(reveal.slice_rows[&26].iter().any(|(k, _)| *k == tower), "the generated list of 26 names /6/1");
        assert!(!reveal.rows.iter().any(|(k, _)| *k == tower), "the start's list does not");
        let ours = reveal.slice_list(26).unwrap();
        let theirs = move_26_list();
        assert_eq!(theirs.len(), 22);
        let sorted = |v: &[(RowKey, Row)]| {
            let mut v: Vec<(RowKey, bool, bool)> = v.iter().map(|(k, r)| (*k, r.has_a, r.has_b)).collect();
            v.sort();
            v
        };
        assert_eq!(sorted(&ours), sorted(&theirs), "the same records with the same flags");
        for (k, r) in &theirs {
            let mine = &ours.iter().find(|(m, _)| m == k).unwrap().1;
            if *k == (RowKey { group: CINE, kind: 6, index: 0 }) {
                assert_eq!(cinematic_of_row(r), Some((0, 4, 0)), "move-26 serves /6/0 done");
                assert_eq!(cinematic_of_row(mine), Some((0, 1, 0)), "the world's /6/0 idle; the scripts set it");
            } else {
                assert_eq!(mine, r, "{k}: the same value, /6/1's from the step schedule's move-26 list");
            }
        }
        assert_eq!(reveal.slice_list(25).map(|l| l.len()), Some(21), "25 adds nothing to the world's list");
        // Without the step schedule's lists /6/1 has no served value: the fg2d case.
        let bare = profile_reveal(false);
        assert!(bare.served_value(&tower).is_none());
        assert_eq!(reveal.slice_list(99), None);
    }

    /// The forwarder and the recording, both (the reveal test).
    struct Forwarding(crate::reward_forward::GrantForwarder, Arc<Mutex<Shared>>);
    impl ActivityRewards for Forwarding {
        fn grant_activity_rewards(&mut self, c: &CharacterRef, hash: u32, outcome: &ActivityOutcome) -> Result<(), String> {
            RecordingRewards(self.1.clone()).grant_activity_rewards(c, hash, outcome)?;
            self.0.grant_activity_rewards(c, hash, outcome)
        }
        fn activity_hand_off(&mut self, c: &CharacterRef, from: u32, index: u16, hash: u32) -> Result<(), String> {
            RecordingRewards(self.1.clone()).activity_hand_off(c, from, index, hash)?;
            self.0.activity_hand_off(c, from, index, hash)
        }
        fn activity_completed(&mut self, c: &CharacterRef, hash: u32) -> Result<(), String> {
            RecordingRewards(self.1.clone()).activity_completed(c, hash)?;
            self.0.activity_completed(c, hash)
        }
        fn activity_started(&mut self, c: &CharacterRef, hash: u32) -> Result<(), String> {
            RecordingRewards(self.1.clone()).activity_started(c, hash)?;
            self.0.activity_started(c, hash)
        }
        fn activity_ended(&mut self, c: &CharacterRef, hash: u32) -> Result<(), String> {
            RecordingRewards(self.1.clone()).activity_ended(c, hash)?;
            self.0.activity_ended(c, hash)
        }
    }

    /// Stream O3 (integration review, finding 9): two connections of one shared instance share one
    /// bridge. One sidecar runs; either member's reports drive the one script; every frame, region
    /// request and completion envelope reaches both; and the results pay each member once.
    #[test]
    fn a_shared_instance_runs_one_script_for_both_members_and_pays_each_once() {
        use crate::shared_scripts::{ScriptHandle, SharedScripts};
        fn step(a: &ScriptHandle, b: &ScriptHandle, t: &mut Instant, secs: f32, fa: &mut Vec<Vec<u8>>, fb: &mut Vec<Vec<u8>>) {
            let end = *t + Duration::from_secs_f32(secs);
            while *t < end {
                *t += Duration::from_millis(250);
                fa.extend(a.sensor_due(*t));
                fb.extend(b.sensor_due(*t));
            }
        }
        let shared = Arc::new(Mutex::new(Shared::default()));
        let reg = SharedScripts::new();
        let a = reg.join(HOST, || bridge(&shared, false));
        let b = reg.join(HOST, || panic!("the instance already has its bridge"));
        assert!(a.same_bridge(&b));
        assert_eq!((a.members(), reg.len()), (2, 1));
        let other = reg.join(HOST + 1, || bridge(&shared, false));
        assert!(!other.same_bridge(&a), "another instance has its own");
        drop(other);
        let mut t = Instant::now();
        let (mut fa, mut fb) = (Vec::new(), Vec::new());
        a.on_client_message(&join(7), 165, HOST, t);
        b.on_client_message(&join(8), 165, HOST, t);
        step(&a, &b, &mut t, 0.75, &mut fa, &mut fb);
        assert_eq!((a.take_region_request(), b.take_region_request()), (Some(25), Some(25)), "both members are moved");
        a.on_client_message(&slice(25), 165, HOST, t);
        step(&a, &b, &mut t, 3.0, &mut fa, &mut fb);
        // Member B finishes the first cinematic: the one script moves on for both.
        b.on_client_message(&incident(891, Some((CINE, 6, 0))), 165, HOST, t);
        step(&a, &b, &mut t, 0.5, &mut fa, &mut fb);
        assert_eq!((a.take_region_request(), b.take_region_request()), (Some(26), Some(26)));
        b.on_client_message(&slice(26), 165, HOST, t);
        step(&a, &b, &mut t, 4.5, &mut fa, &mut fb);
        assert!(a.take_incident().is_some() && b.take_incident().is_some(), "each member gets the completion envelope");
        assert_eq!((a.take_incident(), b.take_incident()), (None, None), "once");
        a.on_client_message(&incident(891, None), 165, HOST, t);
        step(&a, &b, &mut t, 1.0, &mut fa, &mut fb);
        let s = shared.lock().unwrap();
        assert_eq!(s.launches, 1, "one sidecar for the instance");
        assert!(!fa.is_empty());
        assert_eq!(fa, fb, "both clients are sent the same rows");
        let mut seq: Vec<i8> = s.frames.iter().filter_map(lifetime_of).collect();
        seq.dedup();
        assert_eq!(seq, [0, 1, 4, 6, 8]);
        let mut paid: Vec<u8> = s.rewards.iter().map(|r| r.0.identity[0]).collect();
        paid.sort_unstable();
        assert_eq!(paid, [7, 8], "each member paid once");
        assert!(s.rewards.iter().all(|r| r.2 == ActivityOutcome::Completed));
        drop(s);
        drop(a);
        assert_eq!(b.members(), 1);
        drop(b);
        assert!(reg.is_empty(), "the bridge goes with its last member");
    }

    #[test]
    fn stand_in_rows_never_reach_the_wire() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge_with(&shared, false, true, CINE_LINE);
        let mut t = Instant::now();
        b.on_client_message(&join(6), 165, HOST, t);
        b.on_client_message(&slice(25), 165, HOST, t);
        run_for(&mut b, &mut t, 3.5, &mut Vec::new());
        let s = shared.lock().unwrap();
        let deltas: Vec<&FramePlan> = s.frames.iter().filter(|p| !p.full).collect();
        assert!(!deltas.is_empty());
        assert!(deltas.iter().all(|p| p.entries.iter().all(|(k, _)| k.kind == 0x10)), "only the lifetime");
        let mut seq: Vec<i8> = s.frames.iter().filter_map(lifetime_of).collect();
        seq.dedup();
        assert_eq!(seq, [0, 1, 4]);
    }

    #[test]
    fn incidents_translate_through_the_catalogue() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge(&shared, false);
        let mut t = Instant::now();
        b.on_client_message(&join(1), 165, HOST, t);
        run_for(&mut b, &mut t, 1.0, &mut Vec::new());
        shared.lock().unwrap().sent.clear();
        b.on_client_message(&incident(3403, Some((0x992B_B471, 0x1D, 34))), 165, HOST, t);
        b.on_client_message(&incident(566, None), 165, HOST, t);
        b.on_client_message(&incident(2079, None), 165, HOST, t);
        b.on_client_message(&incident(2899, None), 165, HOST, t);
        b.on_client_message(&incident(1726, Some((CINE, 6, 1))), 165, HOST, t);
        b.on_client_message(&incident(891, None), 165, HOST, t);
        b.on_client_message(&incident(77777, None), 165, HOST, t);
        let s = shared.lock().unwrap();
        let key = "0101010101010101".to_string();
        assert_eq!(
            s.sent,
            vec![
                HostMsg::Trigger { bundle: 0x992B_B471, type_index: 34 },
                HostMsg::PlayerSpawned { key: key.clone() },
                HostMsg::PlayerDied { key: key.clone() },
                HostMsg::Incident { id: 2899, name_hash: 0xBD1F_B418 },
                HostMsg::CinematicSkipped { bundle: CINE, type_index: 1, key },
            ],
            "891 with no row and no cinematic playing, and an id outside the catalogue, are not passed on"
        );
    }

    #[test]
    fn activity_complete_without_manifest_hashes_is_not_sent() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge_with(&shared, false, false, BARE_LINE);
        let mut t = Instant::now();
        b.on_client_message(&join(1), 165, HOST, t);
        b.on_client_message(&slice(25), 165, HOST, t);
        run_for(&mut b, &mut t, 3.5, &mut Vec::new());
        b.on_client_message(&incident(891, Some((CINE, 6, 0))), 165, HOST, t);
        run_for(&mut b, &mut t, 0.5, &mut Vec::new());
        b.on_client_message(&slice(26), 165, HOST, t);
        run_for(&mut b, &mut t, 5.0, &mut Vec::new());
        assert!(shared.lock().unwrap().frames.iter().any(|p| cinematic_of(p, 1).is_some_and(|c| c.2 == 1)), "the scripts reached completion");
        assert_eq!(b.take_incident(), None);
        assert!(shared.lock().unwrap().bodies.is_empty());
    }

    #[test]
    fn a_gameplay_activity_ends_on_7_and_sends_all_five_incidents() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge_with(&shared, false, false, GAMEPLAY_LINE);
        let mut t = Instant::now();
        b.on_client_message(&join(9), 165, HOST, t);
        b.on_client_message(&slice(25), 165, HOST, t);
        run_for(&mut b, &mut t, 3.5, &mut Vec::new());
        b.on_client_message(&incident(891, Some((CINE, 6, 0))), 165, HOST, t);
        run_for(&mut b, &mut t, 0.5, &mut Vec::new());
        b.on_client_message(&slice(26), 165, HOST, t);
        run_for(&mut b, &mut t, 5.0, &mut Vec::new());
        let envelope = b.take_incident().expect("the completion envelope");
        let u32_at_e = |at: usize| u32::from_le_bytes(envelope[at..at + 4].try_into().unwrap());
        assert_eq!([u32_at_e(0), u32_at_e(4), u32_at_e(8), u32_at_e(12), u32_at_e(16), u32_at_e(20)], [5, 965, 3235, 2937, 2009, 614]);
        b.on_client_message(&incident(891, None), 165, HOST, t);
        run_for(&mut b, &mut t, 1.0, &mut Vec::new());
        let s = shared.lock().unwrap();
        let mut seq: Vec<i8> = s.frames.iter().filter_map(lifetime_of).collect();
        seq.dedup();
        // The phase move to 26 begins with no cinematic playing, so a gameplay activity leaves the
        // cinematic hold there (F35, live A Guardian Rises: 4 → 3 as its intro phase ended).
        assert_eq!(seq, [0, 1, 4, 3, 6, 7], "gameplay: the hold ends with its phase; results 6, then the post-game 7");
        assert_eq!(&s.bodies[0].1[0x68..0x70], [1, 0, 0, 0, 1, 0, 0, 0], "completion counters 1, 1");
    }

    #[test]
    fn a_public_bubble_can_name_its_bundles_but_not_start_one_event() {
        use crate::public_events::BubbleScriptBridge;
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge(&shared, false);
        assert!(b.start_bundles(HOST, &[1, 2]).is_err(), "nothing running yet");
        let mut t = Instant::now();
        b.on_client_message(&join(1), 165, HOST, t);
        run_for(&mut b, &mut t, 1.0, &mut Vec::new());
        assert!(b.start_bundles(HOST, &[1, 2]).is_ok());
        assert_eq!(shared.lock().unwrap().sent.last(), Some(&HostMsg::ActiveBundles { bundles: vec![1, 2] }));
        let event = crate::public_events::ScheduledEvent {
            id: 3, kind: crate::public_events::EventKind::Defend, activity_hash: 0, time_limit: Duration::from_secs(60),
        };
        assert!(b.start_event(HOST, 1, &event).is_ok());
        assert!(b.stop_event(HOST, 1, 3).is_ok());
        let sent = shared.lock().unwrap().sent.clone();
        assert_eq!(&sent[sent.len() - 2..], [HostMsg::EventStart { bundle: 1, event_id: 3 }, HostMsg::EventStop { bundle: 1, event_id: 3 }]);
        assert_eq!(host_line(&sent[sent.len() - 2]), r#"{"type":"event_start","bundle":1,"event_id":3}"#);
        let reply = parse_sidecar_line(r#"{"type":"event","bundle":1,"event_id":3,"state":"refused","reason":"no such bundle"}"#).unwrap();
        assert!(matches!(reply, Some(SidecarMsg::Event { bundle: 1, event_id: Some(3), ref state, .. }) if state == "refused"));
    }

    #[test]
    fn no_event_starts_before_the_scripts_run() {
        use crate::public_events::BubbleScriptBridge;
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge(&shared, false);
        let event = crate::public_events::ScheduledEvent {
            id: 3, kind: crate::public_events::EventKind::Defend, activity_hash: 0, time_limit: Duration::from_secs(60),
        };
        assert!(b.start_event(HOST, 1, &event).is_err());
        assert!(b.stop_event(HOST, 1, 3).is_err());
    }

    #[test]
    fn a_sidecar_that_dies_after_start_halts_progression_but_not_the_heartbeat() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge(&shared, true);
        let mut t = Instant::now();
        b.on_client_message(&join(2), 165, HOST, t);
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
        let mut b = bridge(&shared, false);
        let s = shared.clone();
        b.launch = Box::new(move |_| {
            s.lock().unwrap().launches += 1;
            Err("node: not found".into())
        });
        let mut t = Instant::now();
        b.on_client_message(&join(3), 165, HOST, t);
        run_for(&mut b, &mut t, 10.0, &mut Vec::new());
        assert_eq!(shared.lock().unwrap().launches, 3);
        assert!(b.scripted(), "the world is still served from the table");
    }

    #[test]
    fn a_rejoin_resends_the_full_list_with_current_values() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge(&shared, false);
        let mut t = Instant::now();
        b.on_client_message(&join(4), 165, HOST, t);
        b.on_client_message(&slice(25), 165, HOST, t);
        run_for(&mut b, &mut t, 3.5, &mut Vec::new());
        b.on_client_message(&join(4), 165, HOST, t);
        run_for(&mut b, &mut t, 0.25, &mut Vec::new());
        let s = shared.lock().unwrap();
        let full = s.frames.last().unwrap();
        assert!(full.full && full.init);
        assert_eq!(lifetime_of(full), Some(4), "the current lifetime, not the list's");
        assert_eq!(s.launches, 1, "the same sidecar carries on");
        assert_eq!(s.rewards.len(), 0);
    }

    #[test]
    fn squad_counts_go_to_the_sidecar_by_bundle() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge(&shared, false);
        let mut t = Instant::now();
        b.on_client_message(&join(5), 165, HOST, t);
        run_for(&mut b, &mut t, 0.5, &mut Vec::new());
        shared.lock().unwrap().sent.clear();
        let counts = [crate::sense::SquadCount { group: 0x9243_DFFB, kind: 1, index: 17, live: 0 }];
        let squads = [SquadSense { group: 0x9243_DFFB, index: 17, state: 2, flag: true }];
        b.on_sense(&counts, &squads);
        assert_eq!(
            shared.lock().unwrap().sent,
            vec![HostMsg::SquadAlive { bundle: 0x9243_DFFB, type_index: 17, count: 0 }],
            "squad state bytes are logged, not sent (protocol 1 has no message for them)"
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

    /// Generated world tables for the synthetic reveal: bubble 3 is `plaza`, its states 24, 25 and
    /// 26; 25 and 26 are named by the two phases. 24 loads only the world bundle.
    fn worlds() -> crate::generated::GeneratedWorlds {
        let worlds = "world\tscenario_tag\tprivate_checksum\tbubble_count\tchosen\n\
            cine_reveal\t0x80c3e00a\t0x14cda256\t4\t0,1,2,3\n";
        let slices = "world\tslice_set\tbubble\tvalid\tpublic\tregion_hash\tglobal\tauthoritative\tprivate\n\
            cine_reveal\t24\t3\t1\t0\t0x1ad5e415\t0x80c4e00a\t-\t-\n\
            cine_reveal\t25\t3\t1\t0\t0x1ad5e415\t0x80c3e710,0x80c4e00a\t-\t-\n\
            cine_reveal\t26\t3\t1\t0\t0x1ad5e415\t0x80c3e715,0x80c4e00a\t-\t-\n";
        let bundles = "bundle_tag\tgroup\tlookup_bubble\tclients\n\
            0x80c4e00a\t0x4786c0e0\t-1\t15:0:10 16:3:10\n\
            0x80c3e710\t0x2fe7dbe6\t-1\t6:0:10 6:1:10\n\
            0x80c3e715\t0x2fe7dbe6\t-1\t6:0:10 6:1:10\n";
        let activities = "index\thash\tname\tworld\tstart_region\tstart_public\tprivate_start\tstatus\n\
            165\t0x6eaf0959\tA Guardian Rises\tcine_reveal\t25\t0\t0\tcross-checked\n";
        let mut g = crate::generated::GeneratedWorlds::parse(worlds, slices, bundles, activities).unwrap();
        let states = "world\tslice_set\tbubble\tvalid\tstate_hash\tregion_hash\n\
            cine_reveal\t24\t3\t1\t0x040ebd4d\t0x1ad5e415\n\
            cine_reveal\t25\t3\t1\t0x77a6931d\t0x1ad5e415\n\
            cine_reveal\t26\t3\t1\t0xd77d27ff\t0x1ad5e415\n\
            elsewhere\t0\t0\t1\t0x1\t0x2\n";
        assert_eq!(g.load_slice_states(states).unwrap(), 3, "a world the tables do not have is skipped");
        g
    }

    #[test]
    fn content_gives_each_phase_its_region_and_each_slice_its_bundles() {
        let g = worlds();
        assert_eq!(g.phase_slice_set("cine_reveal", crate::incident_catalog::fnv1("cine_106_trv_phase"), None), Some(25));
        assert_eq!(g.phase_slice_set("cine_reveal", crate::incident_catalog::fnv1("cine_107_twr_phase"), None), Some(26));
        assert_eq!(g.phase_slice_set("cine_reveal", 0x1234, Some(0x1AD5_E415)), Some(24), "else the bubble the slice names");
        assert_eq!(g.phase_slice_set("cine_reveal", 0x1234, None), None);
        assert_eq!(g.slice_groups("cine_reveal", 25), vec![0x2FE7_DBE6, 0x4786_C0E0]);

        // The bridge: no regions= in the line, so content places the phases.
        let line = "165 cine_reveal 0x4786c0e0 @generated start=25 hash=0x0BBD0E26 kind=cinematic \
            type=0x648999F4 type2=0x4D78882E place=0xB22417AE next=0:0x8AE8DA69";
        let a = ScriptedActivity::parse_with(line, &base(), &|_| Err("no file".into()), Some(&g)).unwrap();
        assert_eq!(a.rows.len(), 3, "the generated list of slice set 25, less the lifetime the bridge keys");
        assert_eq!(a.region_of_phase(1, crate::incident_catalog::fnv1("cine_107_twr_phase"), None), Some(26));
        assert_eq!(a.next, Some((0, 0x8AE8_DA69)));
        assert!(ScriptedActivity::parse(line, &base(), &|_| Ok(LIST.to_string())).is_err(), "@generated needs the tables");
        assert!(ScriptedActivity::parse_with(&line.replace("start=25", "start=99"), &base(), &|_| Ok(String::new()), Some(&g)).is_err());

        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge_worlds(&shared, false, false, line, Some(&g));
        let mut t = Instant::now();
        b.on_client_message(&join(7), 165, HOST, t);
        run_for(&mut b, &mut t, 0.75, &mut Vec::new());
        // Phase 0 is state 25, by its name: the start slice set the client loads at the join, so
        // no teleport (run fg2: one sent during the initial load stalled it).
        assert_eq!(b.take_region_request(), None, "phase 0 is state 25, where the client starts");
        {
            // F44: the start slice set's bundles go out before the scripts start, not at a report.
            let s = shared.lock().unwrap();
            let active = s.sent.iter().position(|m| *m == HostMsg::ActiveBundles { bundles: vec![0x2FE7_DBE6, 0x4786_C0E0] });
            let start = s.sent.iter().position(|m| matches!(m, HostMsg::Start));
            assert!(active.is_some() && active < start, "25's bundles are active before the start: {:?}", s.sent);
        }
        b.on_client_message(&slice(25), 165, HOST, t);
        b.on_client_message(&slice(25), 165, HOST, t);
        let actives = shared.lock().unwrap().sent.iter().filter(|m| matches!(m, HostMsg::ActiveBundles { .. })).count();
        assert_eq!(actives, 1, "once per change");
        run_for(&mut b, &mut t, 3.0, &mut Vec::new());
        b.on_client_message(&incident(891, Some((CINE, 6, 0))), 165, HOST, t);
        run_for(&mut b, &mut t, 0.5, &mut Vec::new());
        assert_eq!(b.take_region_request(), Some(26), "phase 1 is state 26, by its name");
        b.on_client_message(&slice(26), 165, HOST, t);
        run_for(&mut b, &mut t, 4.5, &mut Vec::new());
        b.on_client_message(&incident(891, None), 165, HOST, t);
        run_for(&mut b, &mut t, 1.0, &mut Vec::new());
        // The arc: the reveal completes and hands off to the Tower.
        let s = shared.lock().unwrap();
        assert_eq!(s.rewards.len(), 1);
        assert_eq!(s.hand_offs, vec![(u64::from_le_bytes([7; 8]), 0x0BBD_0E26, 0, 0x8AE8_DA69)]);
    }

    #[test]
    fn two_lifetime_states_in_one_message_go_out_in_frames_of_their_own() {
        let shared = Arc::new(Mutex::new(Shared { combined_start: true, ..Default::default() }));
        let mut b = bridge(&shared, false);
        let mut t = Instant::now();
        b.on_client_message(&join(7), 165, HOST, t);
        run_for(&mut b, &mut t, 1.5, &mut Vec::new());
        let s = shared.lock().unwrap();
        let mut seq: Vec<i8> = s.frames.iter().filter_map(lifetime_of).collect();
        seq.dedup();
        assert_eq!(seq, [0, 1, 4], "pregame, then the hold, each in its own frame");
        let hold = s.frames.iter().find(|p| lifetime_of(p) == Some(4)).unwrap();
        assert!(hold.entries.iter().any(|(k, _)| k.kind == 6), "the rows after the second change ride with it");
    }

    #[test]
    fn an_incomplete_run_hands_off_nothing() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut b = bridge_with(&shared, true, false, &format!("{CINE_LINE} next=0:0x8AE8DA69"));
        let mut t = Instant::now();
        b.on_client_message(&join(2), 165, HOST, t);
        run_for(&mut b, &mut t, 5.0, &mut Vec::new());
        let s = shared.lock().unwrap();
        assert_eq!(s.rewards[0].2, ActivityOutcome::Abandoned);
        assert!(s.hand_offs.is_empty());
    }

    #[test]
    fn the_activity_table_line_parses_and_refuses_junk() {
        let read = |_: &str| Ok(LIST.to_string());
        let a = ScriptedActivity::parse(
            "165 cine_reveal 0x4786c0e0 l.txt map=0x1:0x2,3:4 bundles=x.json regions=0:25,1:26 team=0",
            &base(),
            &read,
        )
        .unwrap();
        assert_eq!((a.group_of(1), a.group_of(3), a.group_of(9)), (2, 4, 9));
        assert_eq!((a.bundle_of(2), a.bundle_of(9)), (1, 9));
        assert_eq!((a.regions.get(&0), a.regions.get(&1)), (Some(&25), Some(&26)));
        assert_eq!(a.rows.len(), 3);
        assert_eq!(a.bundles_path.as_deref(), Some("x.json"));
        assert!(ScriptedActivity::parse("165 cine_reveal 0x4786c0e0", &base(), &read).is_err());
        assert!(ScriptedActivity::parse("165 cine_reveal 0x4786c0e0 l.txt colour=red", &base(), &read).is_err());
        assert!(ScriptedActivity::parse("x cine_reveal 0x4786c0e0 l.txt", &base(), &read).is_err());
        let c = ScriptedActivity::parse(CINE_LINE, &base(), &read).unwrap();
        assert_eq!(c.end, EndKind::Cinematic);
        assert_eq!(c.hashes.map(|h| (h.activity, h.kind, h.kind2, h.place)), Some((0x0BBD_0E26, 0x6489_99F4, 0x4D78_882E, 0xB224_17AE)));
        assert_eq!(a.end, EndKind::Gameplay);
        assert!(a.hashes.is_none());
        assert!(ScriptedActivity::parse("165 a 0x1 l.txt hash=0x5 type=0x1", &base(), &read).is_err(), "all four or none");
        assert!(ScriptedActivity::parse("165 a 0x1 l.txt kind=pvp", &base(), &read).is_err());
        let n = ScriptedActivity::parse("165 a 0x1 l.txt world=cine_reveal start=25 next=0:0x8ae8da69", &base(), &read).unwrap();
        assert_eq!((n.world.as_str(), n.start, n.next), ("cine_reveal", Some(25), Some((0, 0x8AE8_DA69))));
        assert_eq!(a.world, "cine_reveal", "the activity name when world= is absent");
        assert!(ScriptedActivity::parse("165 a 0x1 l.txt next=0", &base(), &read).is_err());
        assert!(ScriptedActivity::parse("165 a 0x1 l.txt next=70000:0x1", &base(), &read).is_err());
    }

    #[test]
    fn a_placed_squad_names_its_objective_row() {
        // F46: the hangar's sq_m0_fallen_init names 0x062DD95D/3/4 at +0 (live 866.2 s).
        let mut image = vec![0u8; 124];
        image[0..8].copy_from_slice(&[0x5D, 0xD9, 0x2D, 0x06, 3, 0, 4, 0]);
        let row = Row { has_a: true, has_b: false, a: Some((crate::row_encoders::TAG_SQUAD, image.clone())), b: None };
        assert_eq!(squad_objective(&row), Some(RowKey { group: 0x062D_D95D, kind: 3, index: 4 }));
        image[0..8].copy_from_slice(&[0xC5, 0x9D, 0x1C, 0x81, 0xFF, 0, 0xFF, 0xFF]);
        let none = Row { a: Some((crate::row_encoders::TAG_SQUAD, image)), ..row };
        assert_eq!(squad_objective(&none), None, "an absent objective names nothing");
    }

    #[test]
    fn world_loot_parses_and_builds_the_step_schedules_drop_body_image() {
        let table = "# the starter rifle\n164 0x992bb471/0x4/6 1 0x827dbaf0 410.644 -794.385 17.723 3  # o_weapon\n";
        let drops = WorldLoot::parse_table(table).unwrap();
        assert_eq!(drops.len(), 1);
        let d = &drops[0];
        assert_eq!((d.activity, d.row, d.key, d.id, d.world), (164, RowKey { group: 0x992b_b471, kind: 4, index: 6 }, 1, 0x827d_baf0, 3));
        let img = d.image(0xD1A0_0000_0000_0001);
        assert_eq!(img.len(), 128);
        assert_eq!((&img[0..4], img[8], &img[0x10..0x18]), (&[1u8, 0, 0, 0][..], 1, &0x827d_baf0u64.to_le_bytes()[..]));
        assert_eq!((&img[0x18..0x20], &img[0x40..0x48]), (&[0xFFu8; 8][..], &0xD1A0_0000_0000_0001u64.to_le_bytes()[..]));
        assert_eq!(f32::from_le_bytes(img[0x60..0x64].try_into().unwrap()), 410.644);
        assert_eq!((f32::from_le_bytes(img[0x6C..0x70].try_into().unwrap()), &img[0x70..0x74]), (1.0, &[3u8, 0, 0, 0][..]));
        assert!(WorldLoot::parse_table("164 0x992bb471/0x4 1 2 0 0 0 3").is_err(), "a bad row");
    }

    #[test]
    fn an_unset_player_trigger_goes_out_enabled_at_arming_0_as_live() {
        use crate::row_encoders as re;
        let key = |kind, index| RowKey { group: 0x992b_b471, kind, index };
        let row = |a: Option<(u32, Vec<u8>)>| Row { has_a: true, has_b: false, a, b: None };
        let set = (re::TAG_PLAYER_TRIGGER, vec![1, 0, 0, 0, 7, 7, 0, 0]);
        let mut rows = vec![
            (key(0x1d, 30), row(None)),
            (key(0x1d, 31), row(Some(set.clone()))),
            (key(0x1d, 32), Row { has_a: false, has_b: false, a: None, b: None }),
            (key(0x6, 1), row(None)),
        ];
        assert_eq!(live_trigger_defaults(&mut rows), 1);
        assert_eq!(rows[0].1.a, Some((re::TAG_PLAYER_TRIGGER, LIVE_TRIGGER_IMAGE.to_vec())), "the live image");
        assert_eq!(rows[1].1.a, Some(set), "a served value stays");
        assert_eq!(rows[2].1.a, None, "a record with no A value has none");
        assert_eq!(rows[3].1.a, None, "only triggers");
    }
}
