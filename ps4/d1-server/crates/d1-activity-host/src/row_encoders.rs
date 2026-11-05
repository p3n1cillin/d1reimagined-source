// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! ⭐ Row encoders: BullShcript's item states (`ItemChange`, MIT, `src/host/state/items.ts`) into
//! build 56578 sensor-row value images, and the codec that turns a [`FramePlan`] into udt 5 bytes.
//!
//! # Two layers
//!
//! * **[`RowEncoders`]** turns one script row change (`kind` + `state` JSON) into the row's A value
//!   image. Only fields with 56578 evidence are written; everything else in the image is kept from
//!   the world's base list (or the client's default), and each script field we cannot place is
//!   logged once as not encoded. A kind with no 56578 field map returns [`Encoded::Unsupported`]
//!   and the row keeps its served value. The per-kind evidence is in each encoder's doc.
//! * **The host's wire codec** ([`AhWireCodec`]) turns a planned frame into wire bytes, through
//!   its `encode_sensor_entries` and `encode_object` methods: the client's own codec by default
//!   (`d1_oracle::sensor::SensorFrameEncoder`, codecs kept between frames), stream B's native RSAT
//!   codec (`native::NativeCodec`, `D1_RSAT_NATIVE=1`) where it covers every schema. There is no
//!   second codec abstraction. [`CodecWorker`] runs it off the connection loop.
//!
//! # What the evidence supports, per row kind
//!
//! | Kind (type id) | A tag | Written from the script | Evidence |
//! |---|---|---|---|
//! | lifetime (16) | `0x808008A3` | state (+0), optional mission-phase tuple (+4/+8/+0xC) | 52 live captures: 0→1→3, 4 around cinematics, gameplay ends 6→(30 s)→7, cinematic-only 6,8; tuple geometry from the descriptor, meaning from TU0 (unverified) |
//! | cinematic (6) | `0x8080254D` | start (+0), change counter (+4), playing (+8) | `0x403140` setter; live start = stamp − 1 |
//! | player trigger (29) | `0x80800463` | enabled (+0), arming (+4, 16 bits) | our armed images; live +4 changes |
//! | squad (1) | `0x8080071E` | objective ref (+0), count (+0x2C/+0x30), generation (+0x44) | d1-squad-states.py layout; live objective ref |
//! | player objective (32) | `0x80802A1B` | mode (+0), nav point (+0x60..), machine (+0x4C) | sensor_objective.rs (native divide16) |
//! | object (4) | `0x808007D9` | incarnation (+0), created (+8) | blockers-2026-10-15 layout |
//! | scoreboard (15) | `0x808008D0` | player row 0 stats, f32 at +0x20 + 4·column | the post-game scoreboard (kills 0, precision 18, deaths 22) |
//! | timer, sequence, trigger group, object filter, machine, hard-wipe globals, actor, … | | nothing | no 56578 field map yet |

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Arc;

use crate::codec::AhWireCodec;

use serde_json::Value;

use crate::rows::{FramePlan, Row, RowKey};

// ── the codec ────────────────────────────────────────────────────────────────────────────────────

/// An encoded frame: the bytes (stamp 0) and where the stamp sits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedFrame {
    pub bytes: Vec<u8>,
    pub stamp_bit: usize,
}

/// Work for the codec: a planned frame, or one object (an incident's metadata body).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodecJob {
    Frame(FramePlan),
    Object { tag: u32, image: Vec<u8> },
}

/// A finished job, in submission order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodecDone {
    Frame(Result<EncodedFrame, String>),
    /// `(bits, payload)`.
    Object(Result<(u32, Vec<u8>), String>),
}

/// Run one job through the host's wire codec: [`AhWireCodec::encode_sensor_entries`] and
/// [`AhWireCodec::encode_object`], the client's own codec by default and stream B's native one
/// (`native::NativeCodec`, `D1_RSAT_NATIVE=1`) where it covers the schemas.
pub fn run_job(codec: &dyn AhWireCodec, job: &CodecJob) -> CodecDone {
    match job {
        CodecJob::Frame(plan) => CodecDone::Frame(
            codec
                .encode_sensor_entries(&plan.to_entry_frame())
                .map(|(bytes, stamp_bit)| EncodedFrame { bytes, stamp_bit })
                .map_err(|e| e.to_string()),
        ),
        CodecJob::Object { tag, image } => {
            CodecDone::Object(codec.encode_object(*tag, image).map_err(|e| e.to_string()))
        }
    }
}

/// A codec run on the caller's thread (tests).
pub type InlineCodec = Box<dyn FnMut(&CodecJob) -> CodecDone + Send>;

/// Runs the host's codec off the connection loop: the client's codec waits on its oracle worker,
/// so [`Self::submit`] queues a job and returns at once, and [`Self::poll`] takes each result when
/// it is ready. `Err` from [`Self::poll`]: the worker is gone.
pub struct CodecWorker {
    tx: Option<Sender<CodecJob>>,
    rx: Option<Receiver<CodecDone>>,
    inline: Option<(InlineCodec, std::collections::VecDeque<CodecDone>)>,
}

impl CodecWorker {
    /// A worker thread running `codec`.
    pub fn spawn(codec: Arc<dyn AhWireCodec>) -> Self {
        let (tx, jobs) = channel::<CodecJob>();
        let (done, rx) = channel();
        let spawned = std::thread::Builder::new().name("d1-ah-rows".into()).spawn(move || {
            tracing::info!("udt 5 rows and incident bodies: {}", codec.name());
            while let Ok(job) = jobs.recv() {
                if done.send(run_job(&*codec, &job)).is_err() {
                    return;
                }
            }
        });
        match spawned {
            Ok(_) => Self { tx: Some(tx), rx: Some(rx), inline: None },
            Err(e) => {
                tracing::error!("the row codec thread did not start: {e}");
                Self { tx: None, rx: None, inline: None }
            }
        }
    }

    /// Encode on the caller's thread.
    pub fn inline(codec: InlineCodec) -> Self {
        Self { tx: None, rx: None, inline: Some((codec, Default::default())) }
    }

    pub fn submit(&mut self, job: CodecJob) {
        if let Some((codec, out)) = self.inline.as_mut() {
            out.push_back(codec(&job));
            return;
        }
        let sent = self.tx.as_ref().is_some_and(|tx| tx.send(job).is_ok());
        if !sent {
            self.tx = None;
        }
    }

    pub fn poll(&mut self) -> Option<Result<CodecDone, String>> {
        if let Some((_, out)) = self.inline.as_mut() {
            return out.pop_front().map(Ok);
        }
        let Some(rx) = self.rx.as_ref() else {
            return Some(Err("the row codec thread is gone".into()));
        };
        match rx.try_recv() {
            Ok(r) => Some(Ok(r)),
            Err(TryRecvError::Empty) if self.tx.is_some() => None,
            Err(_) => {
                self.rx = None;
                Some(Err("the row codec thread is gone".into()))
            }
        }
    }
}

// ── the encoders ─────────────────────────────────────────────────────────────────────────────────

pub const TAG_LIFETIME: u32 = d1_oracle::sensor::TAG_SENSOR_VALUE;
pub const TAG_CINEMATIC: u32 = 0x8080_254D;
pub const TAG_PLAYER_TRIGGER: u32 = 0x8080_0463;
pub const TAG_SQUAD: u32 = 0x8080_071E;
pub const TAG_PLAYER_OBJECTIVE: u32 = d1_oracle::sensor_objective::TAG;
pub const TAG_OBJECT: u32 = 0x8080_07D9;
pub const TAG_SCOREBOARD: u32 = 0x8080_08D0;

pub const KIND_SQUAD: u8 = 1;
pub const KIND_OBJECT: u8 = 4;
pub const KIND_CINEMATIC: u8 = 6;
pub const KIND_SCOREBOARD: u8 = 15;
pub const KIND_LIFETIME: u8 = 16;
pub const KIND_PLAYER_TRIGGER: u8 = 29;
pub const KIND_PLAYER_OBJECTIVE: u8 = 32;

const NONE_HASH: u32 = 0x811C_9DC5;

/// How an activity ends: the live captures end the two kinds differently
/// (notes/ps4-56578-wire-catalog-2026-11-05.md, "Who reads the value").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EndKind {
    /// A playable activity: results 6, then 7 about 30 s later (36 of the 39 that reached 7; the
    /// scripts' `end_game(30)` is the delay). 7 opens the post-game screens and freezes the
    /// scoreboard.
    #[default]
    Gameplay,
    /// A cinematic-only activity (The Speaker, the reveal): 6 and 8 in one tick, no 7. 8 is the
    /// client's own end-of-activity transition (back to orbit, or the rematch decision).
    Cinematic,
}

/// ⭐ BullShcript's lifetime state (`native_support.ts`: pregame 0, countdown 1, active 2, the
/// cinematic hold 3, results 5, ended 6) as the 56578 decoded value (the first byte of the
/// `0x808008A3` image; the oracle adds the wire's bias of one). Live order 0 → 1 → 3, with 4 around
/// each cinematic:
///
/// | runtime | 56578 | evidence |
/// |---|---|---|
/// | pregame 0 | 1 | about 20 s after 0 in every live activity |
/// | countdown 1 | 2 | never served in PvE (BullShcript's +1 only) |
/// | active 2 | 3 | the spawn gate accepts 3 |
/// | cinematic hold 3 | 4 | before each scripted cinematic |
/// | results 5 | 6 | the completion tick |
/// | ended 6 | **7** (gameplay) / **8** (cinematic-only) | see [`EndKind`] |
///
/// Runtime 4 has no name in BullShcript and no mapping here; 56578's 5 and 9 are never served in
/// PvE (the client treats 9 like 7).
pub fn lifetime_wire_state(runtime: i64, end: EndKind) -> Option<i8> {
    Some(match (runtime, end) {
        (0, _) => 1,
        (1, _) => 2,
        (2, _) => 3,
        (3, _) => 4,
        (5, _) => 6,
        (6, EndKind::Gameplay) => 7,
        (6, EndKind::Cinematic) => 8,
        _ => return None,
    })
}

/// The running mission phase the lifetime may carry: `{phase index, FNV-1 of its name, bubble}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissionPhase {
    pub index: i32,
    pub name_hash: u32,
    pub bubble: i32,
}

/// The lifetime value image: the oracle's audited image (state, tail) plus, when given, the
/// mission-phase tuple at `+0x04` (i32), `+0x08` (name hash), `+0x0C` (i32). The geometry is the
/// `0x808008A3` descriptor's; that these fields name the running phase is TU0's meaning, not yet
/// verified on 56578 (notes/ps4-56578-fukugen-daihon-2026-11-05.md), so the bridge writes them only
/// under `D1_AH_SCRIPTS_MISSION_PHASE=1`.
pub fn lifetime_image(state: i8, tail: Option<(u32, u32)>, phase: Option<MissionPhase>) -> Vec<u8> {
    let mut image = d1_oracle::sensor::SensorPhaseUpdate {
        gen: 0,
        name_hash: 0,
        name_i8: 0,
        name_sub: 0,
        phase: state,
        tail,
        clients: Vec::new(),
        authority: None,
    }
    .value_image();
    if let Some(p) = phase {
        image[0x04..0x08].copy_from_slice(&p.index.to_le_bytes());
        image[0x08..0x0C].copy_from_slice(&p.name_hash.to_le_bytes());
        image[0x0C..0x10].copy_from_slice(&p.bubble.to_le_bytes());
    }
    image
}

/// What an encoder made of a change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Encoded {
    /// The row's new value.
    Row(Row),
    /// No 56578 encoding for this kind (or this state): the row keeps its served value.
    Unsupported(String),
}

/// What the encoders need from the host at encode time.
#[derive(Clone, Copy, Debug)]
pub struct EncodeContext {
    /// The 10 Hz udt 5 stamp now (the cinematic start is the stamp before the frame's).
    pub stamp_tick: u32,
    /// The local machine id the authority table gives the blocks to (the objective's `+0x4C`).
    pub machine: u8,
}

/// A script item reference, already mapped to the live group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MappedRef {
    pub group: u32,
    pub kind: u8,
    pub index: i16,
}

fn put_u32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn put_ref(b: &mut [u8], at: usize, r: Option<MappedRef>) {
    match r {
        Some(r) => {
            put_u32(b, at, r.group);
            b[at + 4] = r.kind;
            b[at + 5] = 0;
            b[at + 6..at + 8].copy_from_slice(&r.index.to_le_bytes());
        }
        None => {
            put_u32(b, at, NONE_HASH);
            b[at + 4] = 0xFF;
            b[at + 5] = 0;
            b[at + 6..at + 8].copy_from_slice(&(-1i16).to_le_bytes());
        }
    }
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap()))
}

/// The per-kind encoders, with the state some of them keep between changes.
pub struct RowEncoders {
    /// Each cinematic row's last `(counter, playing)`. The client acts only on a counter change.
    cinematics: HashMap<RowKey, (u32, bool)>,
    /// The scoreboard's player stat columns, by name hash, in column order.
    scoreboard_columns: Vec<u32>,
    unencoded: HashSet<(String, String)>,
}

impl Default for RowEncoders {
    fn default() -> Self {
        Self::new()
    }
}

impl RowEncoders {
    pub fn new() -> Self {
        Self { cinematics: HashMap::new(), scoreboard_columns: Vec::new(), unencoded: HashSet::new() }
    }

    /// The scoreboard's player stat columns, by name hash (`D1_AH_SCRIPTS_SCOREBOARD_COLUMNS`).
    pub fn set_scoreboard_columns(&mut self, columns: Vec<u32>) {
        self.scoreboard_columns = columns;
    }

    /// A world loaded: forget the per-row history.
    pub fn reset(&mut self) {
        self.cinematics.clear();
    }

    /// The cinematic rows last encoded as playing.
    pub fn playing_cinematics(&self) -> Vec<RowKey> {
        let mut v: Vec<RowKey> = self.cinematics.iter().filter(|(_, (_, p))| *p).map(|(k, _)| *k).collect();
        v.sort();
        v
    }

    fn not_encoded(&mut self, kind: &str, field: &str) {
        if self.unencoded.insert((kind.into(), field.into())) {
            tracing::warn!("script {kind} field {field:?} has no 56578 encoding yet; not sent");
        }
    }

    fn note_fields(&mut self, kind: &str, state: &Value, encoded: &[&str]) {
        if let Some(obj) = state.as_object() {
            for k in obj.keys().filter(|k| !encoded.contains(&k.as_str())) {
                self.not_encoded(kind, k);
            }
        }
    }

    /// Encode one change. `base` is the row as the table holds it (its served value), `map`
    /// resolves a script item reference (bundle → live group).
    pub fn encode(
        &mut self,
        key: RowKey,
        kind: &str,
        state: &Value,
        base: Option<&Row>,
        ctx: EncodeContext,
        map: &dyn Fn(&Value) -> Option<MappedRef>,
    ) -> Encoded {
        let expect = |want: u8| {
            (key.kind != want).then(|| Encoded::Unsupported(format!("{kind} row {key} is not type id {want}")))
        };
        let base_a = |tag: u32| base.and_then(|r| r.a.as_ref()).filter(|(t, _)| *t == tag).map(|(_, i)| i.clone());
        let with_a = |tag: u32, image: Vec<u8>| {
            Encoded::Row(Row {
                has_a: true,
                has_b: base.is_some_and(|r| r.has_b),
                a: Some((tag, image)),
                b: base.and_then(|r| r.b.clone()),
            })
        };
        match kind {
            "cinematic" => {
                if let Some(e) = expect(KIND_CINEMATIC) {
                    return e;
                }
                self.note_fields(kind, state, &["playing"]);
                let playing = state.get("playing").is_some_and(|p| !p.is_null());
                let mut image = base_a(TAG_CINEMATIC).unwrap_or_else(cinematic_default);
                let (counter, was) = *self
                    .cinematics
                    .entry(key)
                    .or_insert((u32_at(&image, 4).unwrap_or(1), image.get(8) == Some(&1)));
                if playing == was {
                    return with_a(TAG_CINEMATIC, image_with_cinematic(&mut image, counter, playing, ctx));
                }
                let counter = counter.wrapping_add(1).max(1);
                self.cinematics.insert(key, (counter, playing));
                with_a(TAG_CINEMATIC, image_with_cinematic(&mut image, counter, playing, ctx))
            }
            "lifetime" => {
                if let Some(e) = expect(KIND_LIFETIME) {
                    return e;
                }
                // The lifetime image is built by the bridge, which owns the mission phase and tail
                // ([`lifetime_image`]); only the state is read here.
                Encoded::Unsupported("the lifetime row is built by the bridge".into())
            }
            "playerTrigger" => {
                if let Some(e) = expect(KIND_PLAYER_TRIGGER) {
                    return e;
                }
                self.note_fields(kind, state, &["armedAt", "enabled"]);
                let enabled = state.get("enabled").and_then(Value::as_bool).unwrap_or(false);
                let armed = state.get("armedAt").and_then(Value::as_u64).unwrap_or(0) as u32 & 0xFFFF;
                let mut image = base_a(TAG_PLAYER_TRIGGER).unwrap_or_else(|| vec![0; 8]);
                put_u32(&mut image, 0, u32::from(enabled));
                // +4 is a 16-bit value the live host changed per arming (01.29: "fires at most
                // once per arming"); BullShcript's armedAt is the arming tick. Zero while enabled
                // would look unarmed, so it is kept nonzero.
                put_u32(&mut image, 4, if enabled { armed.max(1) } else { armed });
                with_a(TAG_PLAYER_TRIGGER, image)
            }
            "squad" => {
                if let Some(e) = expect(KIND_SQUAD) {
                    return e;
                }
                self.note_fields(kind, state, &["wave", "bodies", "objective"]);
                let wave = state.get("wave").and_then(Value::as_u64).unwrap_or(0);
                if wave == 0 {
                    // Never placed: the client's own default, as an unplaced squad is served.
                    return Encoded::Row(base.map_or(
                        Row { has_a: true, has_b: false, a: None, b: None },
                        Row::defaulted,
                    ));
                }
                let mut image = base_a(TAG_SQUAD).unwrap_or_else(squad_default);
                // +0x44 is the generation; a new value re-runs the squad. Our proven placement is
                // generation 2, BullShcript's first placement is wave 1: generation = wave + 1.
                put_u32(&mut image, 0x44, (wave + 1) as u32);
                match state.get("bodies").and_then(Value::as_i64) {
                    Some(n) => {
                        put_u32(&mut image, 0x2C, 1);
                        image[0x30..0x32].copy_from_slice(&(n.clamp(0, i16::MAX as i64) as i16).to_le_bytes());
                    }
                    None => {
                        // Unset leaves the authored count: no count entry, the client's default.
                        put_u32(&mut image, 0x2C, 0);
                        image[0x30..0x32].copy_from_slice(&0i16.to_le_bytes());
                    }
                }
                // The live host's squads named a kind-3 objective row at +0 (the group's own hash,
                // kind 3, an index): BullShcript's SquadAuth ref0, the objective.
                put_ref(&mut image, 0, state.get("objective").and_then(map));
                with_a(TAG_SQUAD, image)
            }
            "playerObjective" => {
                if let Some(e) = expect(KIND_PLAYER_OBJECTIVE) {
                    return e;
                }
                self.note_fields(kind, state, &["state", "navPoints"]);
                // PlayerObjectiveStage: None 0, Incoming 1, Active 2, Complete 3, Failed 4.
                // 56578 mode 1 shows the NEW MISSION banner with the title, 3 the title alone
                // (native divide16). Incoming → 1 and Active → 3 is our reading of those; the
                // finished stages clear the objective.
                let stage = state.get("state").and_then(Value::as_u64).unwrap_or(0);
                let mode = match stage {
                    1 => 1u8,
                    2 => 3,
                    _ => return with_a(TAG_PLAYER_OBJECTIVE, d1_oracle::sensor_objective::cleared()),
                };
                let nav = state
                    .get("navPoints")
                    .and_then(Value::as_array)
                    .and_then(|a| a.first())
                    .and_then(|p| p.get("ref"))
                    .and_then(map);
                let image = match nav {
                    Some(n) if n.index >= 0 => {
                        let mut image =
                            match d1_oracle::sensor_objective::visible(n.group, n.index as u16, ctx.machine) {
                                Ok(i) => i,
                                Err(e) => return Encoded::Unsupported(e.to_string()),
                            };
                        image[0] = mode;
                        image[0x64] = n.kind;
                        image
                    }
                    _ => {
                        let mut image = d1_oracle::sensor_objective::cleared();
                        image[0] = mode;
                        image[0x40] = 1;
                        put_u32(&mut image, 0x4C, u32::from(ctx.machine));
                        image[0x5C..0x60].copy_from_slice(&1f32.to_le_bytes());
                        put_ref(&mut image, 0x60, None);
                        image
                    }
                };
                with_a(TAG_PLAYER_OBJECTIVE, image)
            }
            "object" => {
                if let Some(e) = expect(KIND_OBJECT) {
                    return e;
                }
                self.note_fields(kind, state, &["incarnation", "present"]);
                let mut image = base_a(TAG_OBJECT).unwrap_or_else(object_default);
                let incarnation = state.get("incarnation").and_then(Value::as_u64).unwrap_or(1) as u32;
                put_u32(&mut image, 0, incarnation.max(1));
                image[8] = u8::from(state.get("present").and_then(Value::as_bool).unwrap_or(false));
                with_a(TAG_OBJECT, image)
            }
            "scoreboard" => {
                if let Some(e) = expect(KIND_SCOREBOARD) {
                    return e;
                }
                self.note_fields(kind, state, &["players", "global", "teams", "notifications"]);
                let Some(mut image) = base_a(TAG_SCOREBOARD) else {
                    return Encoded::Unsupported(
                        "the scoreboard needs its served image (player identity, rewards) in the world's list".into(),
                    );
                };
                for (what, list) in [("global", "global"), ("teams", "teams"), ("notifications", "notifications")] {
                    if state.get(list).and_then(Value::as_array).is_some_and(|a| !a.is_empty()) {
                        self.not_encoded(kind, what);
                    }
                }
                let players = state.get("players").and_then(Value::as_array).cloned().unwrap_or_default();
                if players.len() > 1 {
                    self.not_encoded(kind, "players[1..] (only player row 0's layout is known)");
                }
                if let Some(stats) = players.first().and_then(|p| p.get("stats")).and_then(Value::as_array) {
                    for s in stats {
                        let (Some(hash), Some(value)) = (
                            s.get("nameHash").and_then(Value::as_u64),
                            s.get("value").and_then(Value::as_f64),
                        ) else {
                            continue;
                        };
                        match self.scoreboard_columns.iter().position(|c| u64::from(*c) == hash) {
                            Some(col) if 0x20 + 4 * col + 4 <= image.len() => {
                                let at = 0x20 + 4 * col;
                                image[at..at + 4].copy_from_slice(&(value as f32).to_le_bytes());
                            }
                            _ => self.not_encoded(kind, &format!("stat {hash:#010x} (no column)")),
                        }
                    }
                }
                with_a(TAG_SCOREBOARD, image)
            }
            other => Encoded::Unsupported(format!("no 56578 field map for {other} rows yet")),
        }
    }
}

fn image_with_cinematic(image: &mut [u8], counter: u32, playing: bool, ctx: EncodeContext) -> Vec<u8> {
    // Live: every play's start was the host's frame stamp minus one (The Speaker 0xC7 at 200).
    // Ours is the stamp when the change is encoded, which goes out at the next frame.
    put_u32(image, 0, if playing { ctx.stamp_tick.saturating_sub(1) } else { 0 });
    put_u32(image, 4, counter);
    image[8] = u8::from(playing);
    image.to_vec()
}

/// A cinematic value with no object and no filter (`+0xC` none ref, `+0x14 = -1` bypass), the
/// shape the cinematic-start note found reaches start.
fn cinematic_default() -> Vec<u8> {
    let mut b = vec![0u8; 0xD4];
    put_u32(&mut b, 4, 1);
    put_ref(&mut b, 0xC, None);
    b[0x14] = 0xFF;
    b
}

/// d1-squad-states.py's squad image: four refs absent, three hashes at the client's default,
/// existing-members mode -1 (keep).
fn squad_default() -> Vec<u8> {
    let mut b = vec![0u8; 124];
    for at in [0x00, 0x08, 0x50, 0x58] {
        put_ref(&mut b, at, None);
    }
    for at in [0x48, 0x4C, 0x78] {
        put_u32(&mut b, at, NONE_HASH);
    }
    b[0x74] = 0xFF;
    b
}

/// "Created, no reference": `01000000 00000000 01000000 c59d1c81 ff00ffff`.
fn object_default() -> Vec<u8> {
    let mut b = vec![0u8; 240];
    put_u32(&mut b, 0, 1);
    b[8] = 1;
    put_ref(&mut b, 0xC, None);
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const CTX: EncodeContext = EncodeContext { stamp_tick: 200, machine: 14 };

    fn no_map(_: &Value) -> Option<MappedRef> {
        None
    }

    fn map(v: &Value) -> Option<MappedRef> {
        Some(MappedRef {
            group: v.get("bundle")?.as_u64()? as u32 ^ 0xFFFF_0000,
            kind: v.get("typeId")?.as_u64()? as u8,
            index: v.get("typeIndex")?.as_i64()? as i16,
        })
    }

    fn image(e: &Encoded) -> &[u8] {
        match e {
            Encoded::Row(Row { a: Some((_, i)), .. }) => i,
            other => panic!("not an A image: {other:?}"),
        }
    }

    #[test]
    fn lifetime_states_map_as_the_live_hosts_numbered_them() {
        let gameplay: Vec<_> = (0..=7).map(|s| lifetime_wire_state(s, EndKind::Gameplay)).collect();
        assert_eq!(gameplay, [Some(1), Some(2), Some(3), Some(4), None, Some(6), Some(7), None]);
        let cinematic: Vec<_> = (0..=7).map(|s| lifetime_wire_state(s, EndKind::Cinematic)).collect();
        assert_eq!(cinematic, [Some(1), Some(2), Some(3), Some(4), None, Some(6), Some(8), None]);
    }

    #[test]
    fn the_lifetime_image_is_the_audited_one_until_a_mission_phase_is_given() {
        let plain = lifetime_image(4, Some((0, 0xC2DA_CAF7)), None);
        let audited = d1_oracle::sensor::SensorPhaseUpdate {
            gen: 0, name_hash: 1, name_i8: 0x10, name_sub: 3, phase: 4, tail: Some((0, 0xC2DA_CAF7)),
            clients: Vec::new(), authority: None,
        }
        .value_image();
        assert_eq!(plain, audited);
        let p = MissionPhase { index: 1, name_hash: crate::incident_catalog::fnv1("cine_107_twr_phase"), bubble: 3 };
        let with = lifetime_image(4, Some((0, 0xC2DA_CAF7)), Some(p));
        assert_eq!(&with[4..16], [1, 0, 0, 0, 0xFF, 0x27, 0x7D, 0xD7, 3, 0, 0, 0]);
        assert_eq!((with[0], &with[16..]), (4, &plain[16..]));
    }

    #[test]
    fn a_cinematic_advances_its_counter_only_when_it_starts_or_stops() {
        let mut e = RowEncoders::new();
        let key = RowKey { group: 0x2FE7_DBE6, kind: 6, index: 0 };
        let idle = e.encode(key, "cinematic", &json!({}), None, CTX, &no_map);
        assert_eq!(&image(&idle)[0..9], [0, 0, 0, 0, 1, 0, 0, 0, 0], "idle at counter 1");
        let again = e.encode(key, "cinematic", &json!({}), None, CTX, &no_map);
        assert_eq!(idle, again, "an unchanged state keeps the counter");
        let play = e.encode(key, "cinematic", &json!({"playing": {"startedAt": 9000}}), None, CTX, &no_map);
        assert_eq!(&image(&play)[0..9], [199, 0, 0, 0, 2, 0, 0, 0, 1], "start = stamp - 1, counter 2, playing");
        assert_eq!(&image(&play)[0xC..0x15], [0xC5, 0x9D, 0x1C, 0x81, 0xFF, 0, 0xFF, 0xFF, 0xFF]);
        let done = e.encode(key, "cinematic", &json!({}), None, CTX, &no_map);
        assert_eq!(&image(&done)[0..9], [0, 0, 0, 0, 3, 0, 0, 0, 0]);
        // A served image's own counter is where a row starts.
        let served = Row { has_a: true, has_b: false, a: Some((TAG_CINEMATIC, { let mut b = cinematic_default(); b[4] = 4; b })), b: None };
        let other = RowKey { index: 1, ..key };
        let p = e.encode(other, "cinematic", &json!({"playing": {"startedAt": 1}}), Some(&served), CTX, &no_map);
        assert_eq!(image(&p)[4], 5);
        assert!(matches!(e.encode(RowKey { kind: 5, ..key }, "cinematic", &json!({}), None, CTX, &no_map), Encoded::Unsupported(_)));
    }

    #[test]
    fn triggers_squads_objects_and_objectives_write_only_their_mapped_fields() {
        let mut e = RowEncoders::new();
        let t = e.encode(RowKey { group: 1, kind: 29, index: 34 }, "playerTrigger", &json!({"armedAt": 0x12345, "enabled": true}), None, CTX, &no_map);
        assert_eq!(image(&t), [1, 0, 0, 0, 0x45, 0x23, 0, 0]);
        let t = e.encode(RowKey { group: 1, kind: 29, index: 34 }, "playerTrigger", &json!({"armedAt": 0, "enabled": true}), None, CTX, &no_map);
        assert_eq!(image(&t), [1, 0, 0, 0, 1, 0, 0, 0], "armed stays nonzero");

        let sq = RowKey { group: 0xFC0E_45A3, kind: 1, index: 21 };
        let unplaced = e.encode(sq, "squad", &json!({"carried": false, "wave": 0}), None, CTX, &map);
        assert_eq!(unplaced, Encoded::Row(Row { has_a: true, has_b: false, a: None, b: None }));
        let placed = e.encode(sq, "squad", &json!({"carried": false, "wave": 1, "bodies": 1,
            "objective": {"bundle": 0x062D, "typeId": 3, "typeIndex": 4}, "task": 77}), None, CTX, &map);
        let i = image(&placed);
        assert_eq!(i.len(), 124);
        assert_eq!(&i[0..8], [0x2D, 0x06, 0xFF, 0xFF, 3, 0, 4, 0]);
        assert_eq!((u32_at(i, 0x2C), &i[0x30..0x32], u32_at(i, 0x44)), (Some(1), &[1u8, 0][..], Some(2)));
        assert_eq!((u32_at(i, 0x48), i[0x74]), (Some(NONE_HASH), 0xFF));

        let ob = e.encode(RowKey { group: 1, kind: 4, index: 2 }, "object", &json!({"incarnation": 3, "present": true, "device": {"position": 1, "revision": 1}}), None, CTX, &no_map);
        assert_eq!(&image(&ob)[0..0x14], &hex("030000000000000001000000c59d1c81ff00ffff"));

        let k = RowKey { group: 0x992B_B471, kind: 32, index: 7 };
        let active = e.encode(k, "playerObjective", &json!({"state": 2, "navPoints": [{"ref": {"bundle": 0x992B, "typeId": 0x2C, "typeIndex": 52}, "x": 0, "y": 0, "z": 0}]}), None, CTX, &map);
        let mut want = d1_oracle::sensor_objective::visible(0xFFFF_992B, 52, 14).unwrap();
        want[0] = 3;
        assert_eq!(image(&active), want);
        let incoming = e.encode(k, "playerObjective", &json!({"state": 1, "navPoints": []}), None, CTX, &map);
        assert_eq!((image(&incoming)[0], image(&incoming)[0x40], u32_at(image(&incoming), 0x4C)), (1, 1, Some(14)));
        let done = e.encode(k, "playerObjective", &json!({"state": 3, "navPoints": []}), None, CTX, &map);
        assert_eq!(image(&done), d1_oracle::sensor_objective::cleared());
    }

    #[test]
    fn the_scoreboard_writes_player_stats_into_its_served_image() {
        let mut e = RowEncoders::new();
        let key = RowKey { group: 0x4786_C0E0, kind: 15, index: 0 };
        let st = json!({"global": [], "notifications": [], "teams": [],
            "players": [{"characterKey": "a", "stats": [{"nameHash": 0xAAAA, "value": 50}, {"nameHash": 0xBBBB, "value": 2}]}]});
        assert!(matches!(e.encode(key, "scoreboard", &st, None, CTX, &no_map), Encoded::Unsupported(_)));
        let mut cols = vec![0u32; 23];
        cols[0] = 0xAAAA;
        cols[22] = 0xBBBB;
        e.set_scoreboard_columns(cols);
        let served = Row { has_a: true, has_b: false, a: Some((TAG_SCOREBOARD, vec![0; 0x100])), b: None };
        let i = e.encode(key, "scoreboard", &st, Some(&served), CTX, &no_map);
        let i = image(&i);
        assert_eq!(f32::from_le_bytes(i[0x20..0x24].try_into().unwrap()), 50.0);
        assert_eq!(f32::from_le_bytes(i[0x78..0x7C].try_into().unwrap()), 2.0, "deaths, column 22");
    }

    #[test]
    fn kinds_without_a_field_map_are_refused_not_guessed() {
        let mut e = RowEncoders::new();
        for kind in ["timer", "sequence", "triggerGroup", "objectFilter", "machine", "hardWipeGlobals", "actor"] {
            let got = e.encode(RowKey { group: 1, kind: 2, index: 0 }, kind, &json!({}), None, CTX, &no_map);
            assert!(matches!(got, Encoded::Unsupported(_)), "{kind}");
        }
    }

    /// A wire codec that only builds frames, recording nothing.
    struct Fake;
    impl AhWireCodec for Fake {
        fn name(&self) -> &'static str {
            "fake"
        }
        fn encode(&self, _: &crate::messages::AhMessage) -> Result<Vec<u8>, crate::AhError> {
            Ok(Vec::new())
        }
        fn decode(&self, _: u32, w: &[u8]) -> Result<Vec<u8>, crate::AhError> {
            Ok(w.to_vec())
        }
        fn encode_sensor_entries(&self, f: &d1_oracle::sensor::SensorEntryFrame) -> Result<(Vec<u8>, usize), crate::AhError> {
            Ok((vec![f.gen, f.entries.len() as u8], 9))
        }
        fn encode_object(&self, tag: u32, _: &[u8]) -> Result<(u32, Vec<u8>), crate::AhError> {
            Err(crate::AhError::SchemaCodec { user_defined_type: 18, detail: format!("no {tag:#x}") })
        }
    }

    #[test]
    fn the_worker_runs_the_host_codec_off_the_calling_thread_in_order() {
        let plan = FramePlan { gen: 4, full: true, init: true, authority: None, entries: Vec::new() };
        let mut w = CodecWorker::spawn(Arc::new(Fake));
        w.submit(CodecJob::Frame(plan));
        w.submit(CodecJob::Object { tag: 0x8080_0595, image: vec![0; 128] });
        let mut got = Vec::new();
        while got.len() < 2 {
            if let Some(r) = w.poll() {
                got.push(r.expect("worker alive"));
            }
            std::thread::yield_now();
        }
        assert_eq!(got[0], CodecDone::Frame(Ok(EncodedFrame { bytes: vec![4, 0], stamp_bit: 9 })));
        assert!(matches!(&got[1], CodecDone::Object(Err(e)) if e.contains("0x80800595")));
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
    }
}
