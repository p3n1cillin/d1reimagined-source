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
//! | sequence (5) | `0x808029A1` | start (+0, i32), step (+4, u8), target ref (+0x18) | 56578 apply `0x40CE50`/evaluator `0x40D000` (native, breach-sequences 31 Oct); the reference's move to +0x18 (final-build note); TU0 SequenceAuth [3] is the target |
//! | actor (2) | `0x80800868` | generation (+0), +4 = -1, +6 set, binding absent (+0x50/+0x58), command script (+0xB0: cookie, pc, count, 64-byte atoms at +0xC0) | d1-member-states.py's image, native-tested (the Breach entrances play, 1 Nov); `scripted_sequence` atoms only |
//! | machine (40) | `0x80800556` | the performance: cue (+0), override (+4, empty), generation (+8) | 56578 apply `0x287230` and updater `0x21e730`; JOIN111's dispatch (ghost-light note) |
//! | trigger group (30) | `0x8080040E` | state (+8, i8 2-bit, bias 1: -1..2) | the descriptor: the only field with BullShcript's range (-1 until written); the volume ref at +0 is kept |
//! | timer (17), hard-wipe globals (34), object filter (33) | | nothing | layout from the descriptor ([`ROW_LAYOUTS`]), meaning open: refused |

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
/// A kind-3 objective row's value (live capture: A Guardian Rises' three, each sent as 28 zero bytes
/// when its area's squads were first placed: 0xFC0E45A3/3/20 at 581 s, 0x9243DFFB/3/3 at 772.6 s,
/// 0x062DD95D/3/4 at 866.2 s).
pub const TAG_OBJECTIVE: u32 = 0x8080_04FA;
pub const OBJECTIVE_SIZE: usize = 28;
pub const KIND_OBJECTIVE: u8 = 3;
/// A squad value's body-count cells: u16s from `+0x30` to the generation at `+0x44`.
pub const SQUAD_CELLS: usize = 10;
/// A squad value's temperament hashes: u32s from `+0x14` to the cell count at `+0x2C`.
pub const SQUAD_TEMPERAMENTS: usize = 6;
pub const TAG_PLAYER_OBJECTIVE: u32 = d1_oracle::sensor_objective::TAG;
pub const TAG_OBJECT: u32 = 0x8080_07D9;
/// A prop command's typed value: three (i32, i32, f32) channels (JOIN210, 01.24; the Breach's
/// activation in the step schedule). F41.
pub const TAG_DEVICE_COMMAND: u32 = 0x8080_279A;
/// The prop state image must reach past the command's third channel.
const DEVICE_COMMAND_END: usize = 0x40 + 3 * 0xC;
pub const TAG_SCOREBOARD: u32 = 0x8080_08D0;
pub const TAG_SEQUENCE: u32 = 0x8080_29A1;
pub const TAG_ACTOR: u32 = 0x8080_0868;
pub const TAG_MACHINE: u32 = 0x8080_0556;
pub const TAG_TRIGGER_GROUP: u32 = 0x8080_040E;
pub const TAG_TIMER: u32 = 0x8080_0725;
pub const TAG_HARD_WIPE: u32 = 0x8080_04A4;
pub const TAG_OBJECT_FILTER: u32 = 0x8080_0750;

pub const KIND_SQUAD: u8 = 1;
pub const KIND_ACTOR: u8 = 2;
pub const KIND_SEQUENCE: u8 = 5;
pub const KIND_TIMER: u8 = 17;
pub const KIND_TRIGGER_GROUP: u8 = 30;
pub const KIND_OBJECT_FILTER: u8 = 33;
pub const KIND_HARD_WIPE: u8 = 34;
pub const KIND_MACHINE: u8 = 40;
pub const KIND_OBJECT: u8 = 4;
pub const KIND_CINEMATIC: u8 = 6;
pub const KIND_SCOREBOARD: u8 = 15;
pub const KIND_LIFETIME: u8 = 16;
pub const KIND_PLAYER_TRIGGER: u8 = 29;
pub const KIND_PLAYER_OBJECTIVE: u8 = 32;

pub const NONE_HASH: u32 = 0x811C_9DC5;

/// The type-12 player bundle's A value (`0x808022A5`, 0x78 bytes; B `0x80802C30` has the same
/// layout). The client links its player to the bundle whose A `+0x40` equals the player's key,
/// and rung 34's arrival hold (task 9) waits for that link. A Guardian Rises' hand lists carry one
/// for `0x4786C0E0/0xC/5`; generated worlds and the public lists had none (run fg6d, F32).
pub const TAG_PLAYER_BUNDLE: u32 = 0x8080_22A5;
pub const KIND_PLAYER_BUNDLE: u8 = 12;
pub const PLAYER_BUNDLE_LEN: usize = 0x78;
/// The mission host's player bundle: `players[0]` is sub 5 of the world group (live, and A
/// Guardian Rises' hand lists).
pub const MISSION_PLAYER_SUB: i16 = 5;
/// A public instance's players group (16 records of kind 12), led by every live public list.
pub const PUBLIC_PLAYERS_GROUP: u32 = 0xEAAF_16E2;

/// The player key a join names: the 8 bytes at `join_request+0x28` (identity `+0x18`), which a
/// player bundle holds byte-reversed at `+0x40` (A Guardian Rises' lists; the live hosts).
pub fn player_key_of_identity(identity: &[u8]) -> Option<[u8; 8]> {
    let mut k: [u8; 8] = identity.get(0x18..0x20)?.try_into().ok()?;
    k.reverse();
    Some(k)
}

/// The client's 8-byte machine id from a join's identity (`join_request+0x10`, identity `+0`),
/// the value the live hosts put in every authority entry they granted it.
pub fn machine_of_identity(identity: &[u8]) -> Option<[u8; 8]> {
    identity.get(0..8)?.try_into().ok()
}

/// A player bundle's A image naming `key`. `mission` is the mission host's form, as A Guardian
/// Rises' working lists serve it (`+8`, `+9` = 1; `+0x72`, `+0x74` = 0); otherwise the public
/// instance's live default (`+8`, `+9` = 0; `+0x72` = 0xBC00; `+0x74` = -1). Both: `+0` = -1,
/// `+4` = the empty-name hash, `+0x38` = -1 (the spawn team), `+0x40` = the key.
pub fn player_bundle_image(key: [u8; 8], mission: bool) -> Vec<u8> {
    let mut v = vec![0u8; PLAYER_BUNDLE_LEN];
    v[0..4].copy_from_slice(&(-1i32).to_le_bytes());
    v[4..8].copy_from_slice(&NONE_HASH.to_le_bytes());
    v[0x38..0x3C].copy_from_slice(&(-1i32).to_le_bytes());
    v[0x40..0x48].copy_from_slice(&key);
    if mission {
        v[8] = 1;
        v[9] = 1;
    } else {
        v[0x72..0x74].copy_from_slice(&0xBC00u16.to_le_bytes());
        v[0x74..0x78].copy_from_slice(&(-1i32).to_le_bytes());
    }
    v
}

/// One row kind's A value as build 56578's own descriptor lays it out (`ps4/fixtures/56578/rtti/`
/// `schemas.tsv` and `fields.tsv`, #308): the tag, the struct size, and the top-level fields as
/// `(offset, codec kind)` (2 bool, 3 i8, 5 i32, 7 u8, 9 raw u32, 11 f32, 1 struct, 257 optional
/// struct). A test checks every entry against the committed fixture, so a layout here is the
/// executable's, not ours. What each field means is in the encoders (and the module table).
/// `(script kind, row type id, A tag, struct size, [(offset, codec kind)])`.
pub type RowLayout = (&'static str, u8, u32, usize, &'static [(usize, u32)]);

pub const ROW_LAYOUTS: &[RowLayout] = &[
    // +0 i32 start, +4 u8 step, +8 a counted list (0x80805cf5), +0x18 a name ref (0x808004b1).
    ("sequence", KIND_SEQUENCE, TAG_SEQUENCE, 32, &[(0, 5), (4, 7), (8, 1), (24, 1)]),
    // +0 i32 (31 bits) generation, ..., +0xB0 the optional command script (0x80800512).
    ("actor", KIND_ACTOR, TAG_ACTOR, 2312, &[(0, 261), (4, 3), (5, 3), (6, 2), (8, 257), (44, 257), (176, 257), (2240, 257)]),
    // +0 the optional performance (0x80800538: cue u32, override u32, generation i32), +0xC cues.
    ("machine", KIND_MACHINE, TAG_MACHINE, 48, &[(0, 257), (12, 257)]),
    // +0 the volume (a name ref), +8 the state.
    ("triggerGroup", KIND_TRIGGER_GROUP, TAG_TRIGGER_GROUP, 12, &[(0, 1), (8, 3)]),
    // +0 a timer struct (0x80800ab3: bool, five i32, f32), +0x1C bool, +0x20 i32.
    ("timer", KIND_TIMER, TAG_TIMER, 36, &[(0, 1), (28, 2), (32, 5)]),
    // +0 bool, +1 bool, +2 i8 (2 bits), +4 the same timer struct.
    ("hardWipeGlobals", KIND_HARD_WIPE, TAG_HARD_WIPE, 32, &[(0, 2), (1, 2), (2, 3), (4, 1)]),
    // +0 a predicate list (0x808042e0: a 4-bit count, up to 8 typed predicates).
    ("objectFilter", KIND_OBJECT_FILTER, TAG_OBJECT_FILTER, 400, &[(0, 1)]),
];

/// The command script's place in the actor image, and its atoms (d1-member-states.py; breach note).
const ACTOR_SCRIPT: usize = 0xB0;
const ACTOR_ATOMS: usize = 0xC0;
const ACTOR_ATOM_SIZE: usize = 0x40;
const ACTOR_MAX_ATOMS: usize = 32;
/// BullShcript's command kind 1, `scripted_sequence(name)`, is the 56578 atom kind 1.
const ATOM_SCRIPTED_SEQUENCE: u64 = 1;

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
                self.note_fields(kind, state, &["wave", "bodies", "objective", "roster", "task", "taskInfluencer", "temperaments", "placedAt"]);
                let wave = state.get("wave").and_then(Value::as_u64).unwrap_or(0);
                let roster: Option<Vec<i64>> = state
                    .get("roster")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().map(|v| v.as_i64().unwrap_or(0)).collect())
                    .filter(|r: &Vec<i64>| !r.is_empty() && r.len() <= SQUAD_CELLS);
                // F46 (offline replay of A Guardian Rises' scripts, 7 Nov): BullShcript's place()
                // leaves the wave at 0 and sets the bodies, the roster and placedAt; only a drop
                // squad's respawn bumps the wave. Taking wave 0 for "never placed" sent every
                // placed squad as the client's default: the hangar's never got a member (the Wall's
                // and the Divide's spawn from the client's own defaults).
                let placed = wave > 0
                    || state.get("bodies").and_then(Value::as_i64).is_some_and(|n| n > 0)
                    || roster.as_ref().is_some_and(|r| r.iter().any(|n| *n > 0))
                    || state.get("placedAt").is_some_and(|p| !p.is_null());
                if !placed {
                    // Never placed: the client's own default, as an unplaced squad is served.
                    // (The live host sent unplaced squads explicitly: the objective, the roster's
                    // cell count, tasks -1. Open: the Breach's and the Divide's default spawns.)
                    return Encoded::Row(base.map_or(
                        Row { has_a: true, has_b: false, a: None, b: None },
                        Row::defaulted,
                    ));
                }
                let mut image = base_a(TAG_SQUAD).unwrap_or_else(squad_default);
                // +0x44 is the generation; a new value re-runs the squad. The live host's first
                // placement was 1 (hangar, 866 s), BullShcript's place() wave 0: generation =
                // wave + 1.
                put_u32(&mut image, 0x44, (wave + 1) as u32);
                // The live host's tasks: +0x68 the task's influencer, +0x6C the task (both -1
                // unplaced); hangar squads 6, 11 and 12 at 866/938 s match BullShcript's
                // (taskInfluencer, task) = (2, 1), (6, 7) and (7, 5).
                let num = |k: &str| state.get(k).and_then(Value::as_i64);
                put_u32(&mut image, 0x68, num("taskInfluencer").unwrap_or(-1) as i32 as u32);
                put_u32(&mut image, 0x6C, num("task").unwrap_or(-1) as i32 as u32);
                // Temperaments: +0x10 the count, +0x14 the hashes (squad 6's 0x8AEFBA13 live, the
                // script's activate_temperament("0x8AEFBA13")).
                let temperaments: Vec<u32> = state
                    .get("temperaments")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(Value::as_u64).map(|h| h as u32).take(SQUAD_TEMPERAMENTS).collect())
                    .unwrap_or_default();
                put_u32(&mut image, 0x10, temperaments.len() as u32);
                for i in 0..SQUAD_TEMPERAMENTS {
                    put_u32(&mut image, 0x14 + 4 * i, temperaments.get(i).copied().unwrap_or(0));
                }
                match (roster, state.get("bodies").and_then(Value::as_i64)) {
                    (Some(cells), _) => {
                        // F46 (run agr28): a squad whose script authored its counts (@member_counts)
                        // goes out as the live host's did: +0x2C the number of cells, +0x30 one u16
                        // body count per cell (A Guardian Rises' hangar, 866 s and 938 s into the
                        // live capture: sq_m0_fallen_init [1, 1], sq_m0_fallen_1 [3], sq_m0_fallen_2
                        // [1, 2]). Sent as no cells, the hangar's squads were placed with no one.
                        put_u32(&mut image, 0x2C, cells.len() as u32);
                        image[0x30..0x30 + 2 * SQUAD_CELLS].fill(0);
                        for (i, n) in cells.iter().enumerate() {
                            let at = 0x30 + 2 * i;
                            image[at..at + 2].copy_from_slice(&(*n).clamp(0, i16::MAX as i64).to_le_bytes()[..2]);
                        }
                    }
                    (None, Some(n)) => {
                        put_u32(&mut image, 0x2C, 1);
                        image[0x30..0x32].copy_from_slice(&(n.clamp(0, i16::MAX as i64) as i16).to_le_bytes());
                    }
                    (None, None) => {
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
                self.note_fields(kind, state, &["incarnation", "present", "device"]);
                let mut image = base_a(TAG_OBJECT).unwrap_or_else(object_default);
                let incarnation = state.get("incarnation").and_then(Value::as_u64).unwrap_or(1) as u32;
                put_u32(&mut image, 0, incarnation.max(1));
                image[8] = u8::from(state.get("present").and_then(Value::as_bool).unwrap_or(false));
                // F41: a device's position. The step schedule's activation of the Breach props
                // (agr-b-activate-c.txt, from the 01.24 reference, JOIN210: the command that opens the
                // grey barrier) is `+4` = 1 and one command in the block at `+0x20`: count 1 at
                // `+0x20`, the typed value `0x8080279A` at `+0x30`, and its three (i32, i32, f32)
                // channels at `+0x40`, each (1, 1, 1.0). The light-up sets o_breach_control and
                // o_breach_gating_lights to position 1 with BullShcript's stamp (`revision`) 1.
                // ⚠ Inferred beyond those values: `+4` and each channel's first i32 carry the stamp,
                // the second the position as an integer, the float the position.
                if let Some(d) = state.get("device") {
                    let position = d.get("position").and_then(Value::as_f64).unwrap_or(0.0);
                    let revision = d.get("revision").and_then(Value::as_u64).unwrap_or(1) as u32;
                    if image.len() >= DEVICE_COMMAND_END {
                        put_u32(&mut image, 4, revision);
                        put_u32(&mut image, 0x20, 1);
                        put_u32(&mut image, 0x30, TAG_DEVICE_COMMAND);
                        for c in 0..3 {
                            let at = 0x40 + 0xC * c;
                            put_u32(&mut image, at, revision);
                            put_u32(&mut image, at + 4, position.round() as i32 as u32);
                            image[at + 8..at + 12].copy_from_slice(&(position as f32).to_le_bytes());
                        }
                    }
                }
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
            "sequence" => {
                if let Some(e) = expect(KIND_SEQUENCE) {
                    return e;
                }
                self.note_fields(kind, state, &["playing", "target"]);
                if state.get("pairs").and_then(Value::as_array).is_some_and(|a| !a.is_empty()) {
                    self.not_encoded(kind, "pairs (the +8 list's elements are not mapped)");
                }
                let mut image = base_a(TAG_SEQUENCE).unwrap_or_else(sequence_default);
                if let Some(p) = state.get("playing").filter(|p| !p.is_null()) {
                    // +4 is a step: a new value other than 0xFF re-evaluates (plays) the sequence;
                    // +0 is its start in host ticks (the udt 5 stamp the sidecar copies).
                    let id = p.get("id").and_then(Value::as_u64).unwrap_or(1);
                    let step = (id % 0xFF) as u8;
                    image[4] = if step == image[4] { step.wrapping_add(1) % 0xFF } else { step };
                    let at = p.get("startedAt").and_then(Value::as_u64).unwrap_or(u64::from(ctx.stamp_tick));
                    put_u32(&mut image, 0, (at as u32) & 0x7FFF_FFFF);
                }
                if let Some(t) = state.get("target") {
                    put_ref(&mut image, 0x18, map(t));
                }
                with_a(TAG_SEQUENCE, image)
            }
            "actor" => {
                if let Some(e) = expect(KIND_ACTOR) {
                    return e;
                }
                self.note_fields(kind, state, &["placed", "incarnation", "commandScript"]);
                if !state.get("placed").and_then(Value::as_bool).unwrap_or(false) {
                    // Not placed: the client's own default, as an unplaced member is served.
                    return Encoded::Row(base.map_or(Row { has_a: true, has_b: false, a: None, b: None }, Row::defaulted));
                }
                let mut image = base_a(TAG_ACTOR).unwrap_or_else(actor_default);
                put_u32(&mut image, 0, state.get("incarnation").and_then(Value::as_u64).unwrap_or(1).clamp(1, 0x7FFF_FFFF) as u32);
                if let Some(cs) = state.get("commandScript").filter(|c| !c.is_null()) {
                    match command_script(cs) {
                        Ok((cookie, pc, atoms)) => {
                            put_u32(&mut image, ACTOR_SCRIPT, cookie & 0x7FFF_FFFF);
                            put_u32(&mut image, ACTOR_SCRIPT + 4, pc);
                            put_u32(&mut image, ACTOR_SCRIPT + 8, atoms.len() as u32);
                            image[ACTOR_ATOMS..ACTOR_ATOMS + ACTOR_MAX_ATOMS * ACTOR_ATOM_SIZE].fill(0);
                            for (i, (end, name)) in atoms.iter().enumerate() {
                                let at = ACTOR_ATOMS + i * ACTOR_ATOM_SIZE;
                                put_u32(&mut image, at, ATOM_SCRIPTED_SEQUENCE as u32);
                                put_u32(&mut image, at + 4, *end);
                                put_u32(&mut image, at + 0x10, *name);
                            }
                        }
                        Err(why) => self.not_encoded(kind, &format!("commandScript ({why})")),
                    }
                }
                with_a(TAG_ACTOR, image)
            }
            "machine" => {
                if let Some(e) = expect(KIND_MACHINE) {
                    return e;
                }
                self.note_fields(kind, state, &["performance", "cues"]);
                if state.get("cues").and_then(Value::as_array).is_some_and(|a| !a.is_empty()) {
                    self.not_encoded(kind, "cues (the +0xC list's elements are not mapped)");
                }
                let mut image = base_a(TAG_MACHINE).unwrap_or_else(|| vec![0; 48]);
                // `0x2873c0` reads no performance while the cue is the empty hash.
                match state.get("performance").filter(|p| !p.is_null()) {
                    Some(p) => {
                        put_u32(&mut image, 0, p.get("name").and_then(Value::as_u64).unwrap_or(u64::from(NONE_HASH)) as u32);
                        put_u32(&mut image, 4, NONE_HASH);
                        put_u32(&mut image, 8, p.get("generation").and_then(Value::as_u64).unwrap_or(0) as u32);
                    }
                    None => {
                        put_u32(&mut image, 0, NONE_HASH);
                        put_u32(&mut image, 4, NONE_HASH);
                        put_u32(&mut image, 8, 0);
                    }
                }
                with_a(TAG_MACHINE, image)
            }
            "triggerGroup" => {
                if let Some(e) = expect(KIND_TRIGGER_GROUP) {
                    return e;
                }
                self.note_fields(kind, state, &["state"]);
                let v = state.get("state").and_then(Value::as_i64).unwrap_or(-1);
                if !(-1..=2).contains(&v) {
                    return Encoded::Unsupported(format!("trigger group state {v} does not fit the 2-bit field (-1..2)"));
                }
                let mut image = base_a(TAG_TRIGGER_GROUP).unwrap_or_else(|| {
                    let mut b = vec![0u8; 12];
                    put_ref(&mut b, 0, None);
                    b
                });
                image[8] = v as i8 as u8;
                with_a(TAG_TRIGGER_GROUP, image)
            }
            "timer" | "hardWipeGlobals" | "objectFilter" => Encoded::Unsupported(format!(
                "{kind}: the 56578 layout is known (row_encoders::ROW_LAYOUTS) but not which field the script's state is"
            )),
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

/// BullShcript's command script (`{cookie, pc?, atoms: [{kind, name, end: {kind}}]}`) as the 56578
/// atoms: `(cookie, pc, [(end condition, action hash)])`. Only `scripted_sequence` atoms with an
/// end condition of 0 or 1 have a known 56578 shape; anything else refuses the whole script.
/// `(cookie, pc, [(end condition, action hash)])`.
type CommandScript = (u32, u32, Vec<(u32, u32)>);

fn command_script(cs: &Value) -> Result<CommandScript, String> {
    let cookie = cs.get("cookie").and_then(Value::as_u64).ok_or("no cookie")? as u32;
    let pc = cs.get("pc").and_then(Value::as_u64).unwrap_or(0) as u32;
    let atoms = cs.get("atoms").and_then(Value::as_array).ok_or("no atoms")?;
    if atoms.len() > ACTOR_MAX_ATOMS {
        return Err(format!("{} atoms, at most {ACTOR_MAX_ATOMS}", atoms.len()));
    }
    atoms
        .iter()
        .map(|a| {
            let k = a.get("kind").and_then(Value::as_u64).ok_or("an atom with no kind")?;
            if k != ATOM_SCRIPTED_SEQUENCE {
                return Err(format!("atom kind {k} has no 56578 layout yet"));
            }
            let end = a.get("end").and_then(|e| e.get("kind")).and_then(Value::as_u64).unwrap_or(0);
            if end > 1 {
                return Err(format!("end condition {end} carries seconds, whose place is not known"));
            }
            let name = a.get("name").and_then(Value::as_u64).ok_or("a scripted_sequence with no name")? as u32;
            Ok((end as u32, name))
        })
        .collect::<Result<Vec<_>, String>>()
        .map(|atoms| (cookie, pc, atoms))
}

/// A sequence never played: step 0xFF (no evaluation), no target.
fn sequence_default() -> Vec<u8> {
    let mut b = vec![0u8; 32];
    b[4] = 0xFF;
    put_ref(&mut b, 0x18, None);
    b
}

/// d1-member-states.py's member image: generation 1, +4 = -1, +6 set, the optional binding
/// absent (+0x50 empty ref, +0x58 = -1).
fn actor_default() -> Vec<u8> {
    let mut b = vec![0u8; 2312];
    put_u32(&mut b, 0, 1);
    b[4] = 0xFF;
    b[6] = 1;
    put_ref(&mut b, 0x50, None);
    put_u32(&mut b, 0x58, 0xFFFF_FFFF);
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

    /// The mission form reproduces A Guardian Rises' working hand row for `0x4786C0E0/0xC/5`
    /// from a join identity naming the lab player (key `0xD100000000000201` at identity `+0x18`,
    /// little-endian as the join carries it).
    #[test]
    fn the_player_bundle_matches_the_working_hand_row() {
        let hand = "ffffffffc59d1c81010100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ffffffff00000000d100000000000201000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
        let mut identity = vec![0u8; 40];
        identity[0..8].copy_from_slice(&[9, 8, 7, 6, 5, 4, 3, 2]);
        identity[0x18..0x20].copy_from_slice(&0xD100_0000_0000_0201u64.to_le_bytes());
        let key = player_key_of_identity(&identity).unwrap();
        let image: String = player_bundle_image(key, true).iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(image, hand);
        assert_eq!(machine_of_identity(&identity), Some([9, 8, 7, 6, 5, 4, 3, 2]));
        let public = player_bundle_image(key, false);
        assert_eq!((public[8], public[9], &public[0x72..0x78]), (0, 0, &[0x00, 0xBC, 0xFF, 0xFF, 0xFF, 0xFF][..]));
        assert!(player_key_of_identity(&identity[..0x10]).is_none());
    }

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

        // F46: BullShcript's place() leaves wave 0: the hangar's sq_m0_fallen_init_b as the offline
        // replay emits it, and as the live host sent squad 6 at 866 s (+0x10 one temperament
        // 0x8AEFBA13, cells [1], generation 1, influencer 2 at +0x68, task 1 at +0x6C).
        let init_b = e.encode(RowKey { group: 0x062D_D95D, kind: 1, index: 6 }, "squad", &json!({"carried": false, "wave": 0,
            "bodies": 1, "roster": [1], "temperaments": [2330966547u32], "task": 1, "taskInfluencer": 2, "placedAt": 1}), None, CTX, &map);
        let i = image(&init_b);
        assert_eq!((u32_at(i, 0x10), u32_at(i, 0x14), u32_at(i, 0x2C), &i[0x30..0x32]), (Some(1), Some(0x8AEF_BA13), Some(1), &[1u8, 0][..]));
        assert_eq!((u32_at(i, 0x44), u32_at(i, 0x68), u32_at(i, 0x6C)), (Some(1), Some(2), Some(1)));
        let unplaced0 = e.encode(sq, "squad", &json!({"carried": false, "wave": 0, "bodies": 0, "roster": [0, 0]}), None, CTX, &map);
        assert_eq!(unplaced0, Encoded::Row(Row { has_a: true, has_b: false, a: None, b: None }), "no bodies: still unplaced");

        // F46: an authored roster goes out as the live host's hangar squads did (sq_m0_fallen_2,
        // @member_counts dreg 1, shank 2: +0x2C 2 cells, +0x30 [1, 2], live 938 s).
        let hangar = RowKey { group: 0x062D_D95D, kind: 1, index: 12 };
        let placed = e.encode(hangar, "squad", &json!({"carried": false, "wave": 1, "roster": [1, 2]}), None, CTX, &map);
        let i = image(&placed);
        assert_eq!((u32_at(i, 0x2C), &i[0x30..0x36]), (Some(2), &[1u8, 0, 2, 0, 0, 0][..]));
        let named = e.encode(hangar, "squad", &json!({"carried": false, "wave": 2, "roster": [0, 3], "bodies": 3}), None, CTX, &map);
        assert_eq!((u32_at(image(&named), 0x2C), &image(&named)[0x30..0x34]), (Some(2), &[0u8, 0, 3, 0][..]), "the roster wins over bodies");

        let ob = e.encode(RowKey { group: 1, kind: 4, index: 2 }, "object", &json!({"incarnation": 3, "present": true, "device": {"position": 1, "revision": 1}}), None, CTX, &no_map);
        assert_eq!(&image(&ob)[0..0x14], &hex("030000000100000001000000c59d1c81ff00ffff"), "the stamp at +4 (F41)");
        // The step schedule's activation image (agr-b-activate-c.txt) from +0x20 to the third channel.
        assert_eq!(
            &image(&ob)[0x20..0x64],
            &hex("010000000000000000000000000000009a27808000000000000000000000000001000000010000000000803f01000000010000000000803f01000000010000000000803f")
        );
        let plain = e.encode(RowKey { group: 1, kind: 4, index: 3 }, "object", &json!({"incarnation": 1, "present": true}), None, CTX, &no_map);
        assert_eq!(&image(&plain)[0..0x0C], &hex("010000000000000001000000"), "no device, +4 stays 0");

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
    fn kinds_without_a_known_meaning_are_refused_not_guessed() {
        let mut e = RowEncoders::new();
        for (kind, id) in [("timer", 17), ("objectFilter", 33), ("hardWipeGlobals", 34), ("health", 19)] {
            let got = e.encode(RowKey { group: 1, kind: id, index: 0 }, kind, &json!({}), None, CTX, &no_map);
            assert!(matches!(got, Encoded::Unsupported(_)), "{kind}");
        }
        // A mapped kind on the wrong row type is refused too.
        for (kind, id) in [("sequence", 2), ("actor", 5), ("machine", 1), ("triggerGroup", 29)] {
            let got = e.encode(RowKey { group: 1, kind: id, index: 0 }, kind, &json!({}), None, CTX, &no_map);
            assert!(matches!(got, Encoded::Unsupported(_)), "{kind} on {id}");
        }
    }

    #[test]
    fn sequences_actors_machines_and_trigger_groups_write_their_mapped_fields() {
        let mut e = RowEncoders::new();
        let seq = RowKey { group: 0x992B_B471, kind: 5, index: 4 };
        let idle = e.encode(seq, "sequence", &json!({}), None, CTX, &map);
        assert_eq!(image(&idle), sequence_default());
        let play = e.encode(seq, "sequence", &json!({"playing": {"id": 1, "startedAt": 305},
            "target": {"bundle": 0x062D, "typeId": 4, "typeIndex": 9}, "pairs": [{"name": 1, "value": 2}]}), None, CTX, &map);
        let i = image(&play);
        assert_eq!((u32_at(i, 0), i[4]), (Some(305), 1), "start, step 1");
        assert_eq!(&i[0x18..0x20], [0x2D, 0x06, 0xFF, 0xFF, 4, 0, 9, 0], "the target");
        let served = Row { has_a: true, has_b: false, a: Some((TAG_SEQUENCE, i.to_vec())), b: None };
        let again = e.encode(seq, "sequence", &json!({"playing": {"id": 1, "startedAt": 400}}), Some(&served), CTX, &map);
        assert_ne!(image(&again)[4], 1, "a replay with the same id still moves the step");
        assert_ne!(image(&again)[4], 0xFF);

        let actor = RowKey { group: 0xFC0E_45A3, kind: 2, index: 39 };
        let unplaced = e.encode(actor, "actor", &json!({"placed": false}), None, CTX, &map);
        assert_eq!(unplaced, Encoded::Row(Row { has_a: true, has_b: false, a: None, b: None }));
        let act = 0x2AC3_BDBAu64;
        let entered = e.encode(actor, "actor", &json!({"placed": true, "incarnation": 2,
            "commandScript": {"cookie": 0x8000_0005u64, "atoms": [{"kind": 1, "name": act, "end": {"kind": 0}}]}}), None, CTX, &map);
        let i = image(&entered);
        assert_eq!(i.len(), 2312);
        assert_eq!((u32_at(i, 0), i[4], i[6]), (Some(2), 0xFF, 1));
        assert_eq!(&i[0x50..0x5C], [0xC5, 0x9D, 0x1C, 0x81, 0xFF, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF], "binding absent");
        assert_eq!((u32_at(i, 0xB0), u32_at(i, 0xB4), u32_at(i, 0xB8)), (Some(5), Some(0), Some(1)));
        assert_eq!((u32_at(i, 0xC0), u32_at(i, 0xC4), u32_at(i, 0xD0)), (Some(1), Some(0), Some(act as u32)));
        let moving = e.encode(actor, "actor", &json!({"placed": true,
            "commandScript": {"cookie": 6, "atoms": [{"kind": 0, "point": {}, "index": 0, "end": {"kind": 0}}]}}), None, CTX, &map);
        assert_eq!(u32_at(image(&moving), 0xB8), Some(0), "a move_to atom has no 56578 layout: no script");

        let m = RowKey { group: 0xFC0E_45A3, kind: 40, index: 125 };
        let perf = e.encode(m, "machine", &json!({"cues": [], "performance": {"generation": 1, "name": 0x9B1D_6A71u64}}), None, CTX, &map);
        assert_eq!(&image(&perf)[0..12], [0x71, 0x6A, 0x1D, 0x9B, 0xC5, 0x9D, 0x1C, 0x81, 1, 0, 0, 0], "JOIN111's image");
        let none = e.encode(m, "machine", &json!({"cues": []}), None, CTX, &map);
        assert_eq!(u32_at(image(&none), 0), Some(NONE_HASH));

        let tg = RowKey { group: 1, kind: 30, index: 3 };
        assert_eq!(image(&e.encode(tg, "triggerGroup", &json!({"state": -1}), None, CTX, &map))[8], 0xFF);
        assert_eq!(image(&e.encode(tg, "triggerGroup", &json!({"state": 2}), None, CTX, &map))[8], 2);
        assert!(matches!(e.encode(tg, "triggerGroup", &json!({"state": 5}), None, CTX, &map), Encoded::Unsupported(_)));
    }

    /// Every layout the encoders rely on is the executable's own: checked against the committed
    /// RTTI tables (#308), which `d1_rtti.py` reads from the 56578 eboot.
    #[test]
    fn the_row_layouts_are_the_56578_descriptors() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/56578/rtti");
        let schemas = std::fs::read_to_string(dir.join("schemas.tsv")).expect("schemas.tsv");
        let fields = std::fs::read_to_string(dir.join("fields.tsv")).expect("fields.tsv");
        let tag = |s: &str| u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap();
        for (name, _, t, size, layout) in ROW_LAYOUTS {
            let row = schemas
                .lines()
                .skip(1)
                .map(|l| l.trim_end_matches('\r').split('\t').collect::<Vec<_>>())
                .find(|f| tag(f[0]) == *t)
                .unwrap_or_else(|| panic!("{name}: {t:#x} not in schemas.tsv"));
            assert_eq!(row[2].parse::<usize>().unwrap(), *size, "{name}: struct size");
            let got: Vec<(usize, u32)> = fields
                .lines()
                .skip(1)
                .map(|l| l.trim_end_matches('\r').split('\t').collect::<Vec<_>>())
                .filter(|f| tag(f[0]) == *t)
                .map(|f| (f[4].parse().unwrap(), f[5].parse().unwrap()))
                .collect();
            assert_eq!(&got, layout, "{name}: fields (offset, kind)");
        }
    }

    /// The new images go through the client's own codec and come back with the mapped fields
    /// (needs the eboot).
    #[test]
    fn the_new_images_survive_the_clients_codec() {
        if std::env::var("D1_EBOOT").map_or(true, |p| !std::path::Path::new(&p).is_file()) {
            eprintln!("SKIP: set D1_EBOOT to the owned 56578 executable");
            return;
        }
        let mut e = RowEncoders::new();
        let cases: Vec<(u32, RowKey, &str, Value, Vec<usize>)> = vec![
            (TAG_SEQUENCE, RowKey { group: 1, kind: 5, index: 0 }, "sequence",
                json!({"playing": {"id": 1, "startedAt": 305}, "target": {"bundle": 0x062D, "typeId": 4, "typeIndex": 9}}), vec![0, 4, 0x18, 0x1C, 0x1E]),
            (TAG_ACTOR, RowKey { group: 1, kind: 2, index: 0 }, "actor",
                json!({"placed": true, "commandScript": {"cookie": 5, "atoms": [{"kind": 1, "name": 0x2AC3_BDBAu64, "end": {"kind": 0}}]}}),
                vec![0, 4, 6, 0xB0, 0xB4, 0xB8, 0xC0, 0xC4, 0xD0]),
            (TAG_MACHINE, RowKey { group: 1, kind: 40, index: 0 }, "machine",
                json!({"performance": {"generation": 1, "name": 0x9B1D_6A71u64}}), vec![0, 4, 8]),
            (TAG_TRIGGER_GROUP, RowKey { group: 1, kind: 30, index: 0 }, "triggerGroup", json!({"state": 2}), vec![0, 4, 8]),
        ];
        for (tag, key, kind, state, offsets) in cases {
            let img = image(&e.encode(key, kind, &state, None, CTX, &map)).to_vec();
            let mut o = d1_oracle::object::ObjectOracle::new(tag).expect("oracle");
            let enc = o.encode(&img, 4096).expect("encode");
            let dec = o.decode(&enc.payload).expect("decode");
            for at in offsets {
                assert_eq!(dec.object[at], img[at], "{kind} {tag:#x}: +{at:#x} after the client's codec");
            }
            eprintln!("{kind}: {} bits through the client's codec", enc.bits);
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
