// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The **minimum authoritative Activity State** - explicitly not a Lua interpreter and not a
//! mission script.
//!
//! ## What "minimum" means here
//!
//! The client's own lifecycle (`historicaldocs/design/activity-host-contract.md` §3, state enum `0x18F8640`) is:
//!
//! ```text
//! … create client → connect to BAP → wait all session peers establish
//!   → send  8 request_activity_host   (session host ONLY; arrives as BAP message 6)
//!   → wait 10 start_activity_host_response  (injected by the BAP message-7 handler)
//!   → wait host identifier
//!   → send  3 join_request
//!   → wait  4 join_result
//!   → wait  0 entity_slots_allocated        ⭐ the entity-index gate
//!   → establish client → ok
//! ```
//!
//! Everything after `→ ok` is the Mission Script, and we do not have it. So this state machine
//! implements **exactly four obligations**, the ones between `join_request` and `ok`:
//!
//! 1. Accept the join and hand out the lease ([`crate::messages::JoinResult`]).
//! 2. Grant entity indices - ⭐ **unsolicited, the whole pool** ([`crate::entity`]).
//! 3. Publish an Activity State blob so the world has one ([`crate::messages::GlobalActivityState`]).
//! 4. ⭐ Replicate the membership ([`crate::messages::ReplicateMembership`]) - one member, the
//!    client, echoed out of its own `join_request`.
//! 5. Hold the lease: track the keepalive and evict on silence.
//!
//! ⭐⭐ **1-4 all go out on the join, together.** `FUN_00298EF0` is a conjunction of six live flags,
//! four of which are these messages, and **nothing ever asks for udt 0, udt 1 or udt 11**. Run
//! `20260808-234535-CODEC-LIVE2` sent two of the four and the client sat at
//! `[AC … CON-Y EST-N AH->0 MEM-0]` for 22 s before timing out; that measurement is what this
//! ordering answers.
//!
//! It deliberately does **not** implement: reservations beyond accepting them, membership
//! replication beyond a single local peer, the mask query/reset/abdicate sub-protocol, sensors,
//! script state, or incidents. Each of those is a real part of the protocol; none of them is on
//! the path from `join_request` to `ok` for a **solo fireteam**, which is one peer - ourselves.
//!
//! ## ⛔ The honest outcome, again, because this is the file people will read
//!
//! Satisfying all four gets a loaded world with **no spawns, no encounters and no objectives**. The
//! 1,712-byte Activity State we publish is a valid description of an activity in which nothing has
//! been scheduled, because scheduling is what the missing Lua does.
//!
//! ## ✅ It can run - 2026-08-08
//!
//! > ⛔ **RETRACTED, IN PLACE.** This said *"And it cannot run yet … the default implementation
//! > refuses."* [`crate::codec::SchemaCodec`] is implemented: it runs the client's own
//! > `FUN_002A4090`/`FUN_002A4140`. [`ActivityInstance`] is still driven by **decoded structs** -
//! > that separation is right and unchanged - but the decode now happens.

use std::time::{Duration, Instant};

use crate::entity::{EntitySlotAllocator, EntitySlotMask};
use crate::messages::{
    AhMessage, ClientMessage, GlobalActivityState, JoinResult, ReplicateMembership,
};
use crate::push::AhPush;

/// Where one activity instance is in its life.
///
/// Named for *our* obligations, not mirrored from the client's 39-entry enum - mirroring an enum
/// we do not drive would assert a correspondence we have not established.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceState {
    /// An id and a session id exist; nobody has connected.
    Allocated,
    /// A client is connected and has not sent `join_request` yet.
    Connected,
    /// We accepted a join. The lease is running.
    Joined,
    /// Entity indices have been granted and the Activity State published. This is as far as a stub
    /// Activity Host can take the client, and the client's own next step is `establish client → ok`.
    Live,
    /// The lease expired or the client reported a connectivity failure.
    Evicted { reason: EvictionReason },
}

/// ⭐⭐ **THE LEASE, MEASURED - and it runs the other way round from the keepalive.**
///
/// Run `20260809-054417-RUNG33-BYPASS` established the session, pushed all four messages, then
/// went silent while the client sent `udt 15 activity_client_keepalive_request` every two seconds.
/// Twenty seconds later the client tore the connection down and reported
/// `error.private_activity_host_host_crashed`. Its own line:
///
/// ```text
/// world_controller:activity_manager: [PRIVATE CURRENT] Lost connection to activity host
///   (possible crash) (last received 20626 ms > timeout (20000 + 500) ms).
/// ```
///
/// The predicate is `FUN_0075E440`'s, and every term is named at the instruction:
///
/// ```c
/// grace   = FUN_001DB260();                    // the "+ %lld"      -> 500   observed
/// timeout = *(int*)(FUN_0034D8D0() + 0x28);    // the "%d"          -> 20000 observed
/// idle    = FUN_0029CA00(ac);                  // ⭐ ms since we last SENT
/// if (timeout + grace <= idle) { …; *status = 0x24; }   // 36 = error_host_crashed
/// ```
///
/// and `FUN_0029CA00` is `now - ac->messaging->vfunc[5]()`, where `vfunc[5]` (`0x594540`) is
/// `mov edi,[rdi+0x7c]; jmp 0x559A30` - the BAP **connection index**, into a function that returns
/// `*(u64*)(conn + 0x1EF8)`. That is the *same* field the BAP manager's drought line renders as
/// `[received … last %lld ms ago]` (`FUN_00B2CC90` reads `conn + 0x1EF8`). ⭐ **Two readers, one
/// field** - and the run's own drought line agreed: `OUT FAH … received 7 messages … last 20362 ms
/// ago`.
///
/// ⇒ **Nothing "answers" a keepalive.** `udt 16 activity_client_keepalive_response` has no client
/// handler and never did. What holds the session open is the Activity Host **transmitting
/// something, anything, on the AH connection** more often than this.
pub const HOST_SILENCE_LIMIT_MS: u64 = 20_000 + 500;

/// Why we stopped hosting.
///
/// `[R]` The client has an 11-entry `activity_host_boot_reason` table (`0x263CF30`, fn `0x2A4BB0`)
/// - `none · debug-command · debug-command-no-rejoin · session-lock-timeout ·
/// ambassador-revocation-ack-timeout · disconnect · connectivity-failure · inactivity-timeout ·
/// session-split · world-server-connectivity-failure · world-server-validation-failure`. These two
/// are the only ones a stub can legitimately reach; the rest are named there so nobody invents a
/// twelfth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvictionReason {
    /// The keepalive lease lapsed. Maps to the client's `inactivity-timeout`.
    InactivityTimeout,
    /// The client told us (`udt 35 connectivity_failure`).
    ConnectivityFailure,
    /// ⭐ The client sent `udt 14 peer_leave_request`. **A consequence, never a cause.** In the run
    /// that named it, it arrived 0.3 s *after* the client had already declared the host crashed -
    /// so an Activity Host that "implements" it is answering the funeral notice. If the host
    /// heartbeat below is working, this never arrives at all.
    ClientLeft,
}

/// ⭐ One timed mission step (`D1_AH_MISSION_STEPS`): from `at` after the first sensor frame, serve
/// these object states (prebuilt for both spawn teams) and, if set, move the player to `region`
/// (the host teleport, carried by the next global-state publish).
#[derive(Debug, Clone)]
pub struct MissionStep {
    pub when: StepWhen,
    pub team0: Vec<u8>,
    pub team_minus1: Vec<u8>,
    pub region: Option<u32>,
    /// ⭐ An incident the host sends when the step fires (`incident=TYPE:BODYHEX` on the steps
    /// line): udt 18 is also AH → client. E.g. 1504, the world-loot drop that announces the
    /// starter rifle in The Breach (body = a `0x80800CB6` object).
    pub incident: Option<(u32, Vec<u8>)>,
}

/// When a [`MissionStep`] fires. Steps fire in order, one after another.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StepWhen {
    /// `N`: N seconds after the first sensor frame.
    At(Duration),
    /// `+N`: N seconds after the previous step.
    AfterPrevious(Duration),
    /// `@T[,T2..]|N`: when the client reports one of the incident types (udt 18), or N seconds
    /// after the previous step if it never does. 56578: 2681 = a cinematic started, 891 = one
    /// stopped, 1726 = the player asked to skip it (the host decides; the client keeps playing).
    /// Incidents within [`ActivityInstance::INCIDENT_ECHO`] of the previous step are the client
    /// echoing that step's own changes and do not count. Up to four types.
    OnIncident { incidents: [u32; 4], timeout: Duration },
    /// `%S|N`: when the client reports slice set S as CURRENT (udt 21), or N seconds after the
    /// previous step. A walk into another bubble (a z-leg) switches on the client's own; this is
    /// the 01.24 mission's `{"on": {"type": 21, "slice_set": 24}}` (The Breach).
    OnSlice { slice: u32, timeout: Duration },
    /// `^G/K/I|N`: when the client reports player trigger `G/K/I` (incident 3403 naming it, see
    /// [`trigger_report`]), or N seconds after the previous step. Each report is used once, so a
    /// trigger that fires before its step is due (two armed at once) still counts.
    OnTrigger { group: u32, kind: u8, index: u16, timeout: Duration },
    /// `!G|N`: when every kind-1 squad of sensor group G that the client has reported alive
    /// (udt 6 squad B `+0x0C` > 0, see [`crate::sense`]) reports 0 live members, or N seconds after
    /// the previous step. A kill-gated objective ("Clear Out the Fallen"); needs `D1_AH_SENSE_SCHEMAS`.
    OnSquadsCleared { group: u32, timeout: Duration },
}

/// A udt-18 struct image carrying one incident (see [`ActivityInstance::incident_due`]).
pub fn incident_image(incident: u32, body: &[u8]) -> Vec<u8> {
    let mut img = vec![0u8; 0x2CC];
    img[0..4].copy_from_slice(&1u32.to_le_bytes());
    img[4..8].copy_from_slice(&incident.to_le_bytes());
    let n = body.len().min(500);
    img[0xD4..0xD8].copy_from_slice(&(n as u32).to_le_bytes());
    img[0xD8..0xD8 + n].copy_from_slice(&body[..n]);
    img
}

/// The player trigger an incident-3403 struct image names: its body (a bit buffer at `+0xD8`,
/// read MSB first) holds the trigger's name at bit 303: u32 group hash, the kind in 7 bits biased
/// by one, then the i16 index in offset binary (+0x8000) (run agr-breach13: `0x992BB471/0x1D/34`).
pub fn trigger_report(image: &[u8]) -> Option<(u32, u8, u16)> {
    let body = image.get(0xD8..)?;
    let bits = |at: usize, n: usize| -> Option<u64> {
        let mut v = 0u64;
        for k in at..at + n {
            v = (v << 1) | u64::from((body.get(k / 8)? >> (7 - k % 8)) & 1);
        }
        Some(v)
    };
    let hash = bits(303, 32)? as u32;
    let kind = (bits(335, 7)? as u8).checked_sub(1)?;
    let index = u16::try_from(bits(342, 16)?.checked_sub(0x8000)?).ok()?;
    (!matches!(hash, 0 | 0x811C_9DC5)).then_some((hash, kind, index))
}

impl StepWhen {
    /// Parse `N`, `+N`, `@T|N` or `%S|N` (seconds may be fractional).
    pub fn parse(s: &str) -> Option<Self> {
        let secs = |t: &str| t.parse::<f32>().ok().filter(|v| *v >= 0.0).map(Duration::from_secs_f32);
        if let Some(rest) = s.strip_prefix('^') {
            let (name, n) = rest.split_once('|')?;
            let mut f = name.split('/');
            let num = |t: &str| {
                let t = t.trim();
                t.strip_prefix("0x").map_or_else(|| t.parse::<u32>().ok(), |h| u32::from_str_radix(h, 16).ok())
            };
            let (group, kind, index) = (num(f.next()?)?, num(f.next()?)?, num(f.next()?)?);
            return Some(Self::OnTrigger { group, kind: kind.try_into().ok()?, index: index.try_into().ok()?, timeout: secs(n)? });
        }
        if let Some(rest) = s.strip_prefix('!') {
            let (group, n) = rest.split_once('|')?;
            let group = group.trim();
            let group = group.strip_prefix("0x").map_or_else(|| group.parse().ok(), |h| u32::from_str_radix(h, 16).ok())?;
            return Some(Self::OnSquadsCleared { group, timeout: secs(n)? });
        }
        if let Some(rest) = s.strip_prefix('%') {
            let (slice, n) = rest.split_once('|')?;
            return Some(Self::OnSlice { slice: slice.trim().parse().ok()?, timeout: secs(n)? });
        }
        if let Some(rest) = s.strip_prefix('@') {
            let (t, n) = rest.split_once('|')?;
            let mut incidents = [u32::MAX; 4];
            for (k, v) in t.split(',').enumerate() {
                *incidents.get_mut(k)? = v.trim().parse().ok()?;
            }
            return Some(Self::OnIncident { incidents, timeout: secs(n)? });
        }
        if let Some(rest) = s.strip_prefix('+') {
            return secs(rest).map(Self::AfterPrevious);
        }
        secs(s).map(Self::At)
    }
}

/// One row of the per-activity world table: everything rung 33/34 and the spawn read that differs
/// per world, with the udt-5 sensor frame prebuilt for both spawn teams.
#[derive(Debug, Clone)]
pub struct ServedWorld {
    pub name: String,
    pub checksum: u32,
    pub bubble_count: u32,
    pub chosen: Vec<usize>,
    /// The slice-set id (`8 * bubble + n`) the mission starts in, or `None` for 0.
    pub start_region: Option<u32>,
    pub team0: Vec<u8>,
    pub team_minus1: Vec<u8>,
}

/// Encodes a sensor-client list file into a frame for the given spawn team (`clients GROUP PATH`
/// control lines, see [`ActivityInstance::set_control_builder`]).
#[derive(Clone)]
pub struct ControlBuilder(pub std::sync::Arc<dyn Fn(u32, &str, i32) -> Result<Vec<u8>, String> + Send + Sync>);

impl std::fmt::Debug for ControlBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ControlBuilder")
    }
}

/// One activity instance, authoritative over its own Activity State.
#[derive(Debug)]
pub struct ActivityInstance {
    ah_id: u64,
    session_id: String,
    state: InstanceState,
    entities: EntitySlotAllocator,
    replication_epoch: u8,
    /// What we told the client in `join_result+0x198`. The client has no default; this number is
    /// entirely ours.
    keepalive_interval: Duration,
    /// ⭐ **The eviction deadline lives server-side.** The client stores only the interval we gave
    /// it; it never learns how long we will wait. `[I]` from §9 of the contract doc - the three
    /// boot delays are runtime config at `FUN_0034DB20()+0x120/+0x128/+0x140` and their defaults
    /// are `[U]`. A generous multiple of the interval is the safe choice while that is unknown.
    lease: Duration,
    last_heard: Instant,
    /// Set once, when the Activity State has been published.
    published_state: bool,
    /// ⭐ Set once, when `11 replicate_membership` has gone out. Distinct from
    /// [`Self::published_state`] because the membership is the one push that can be *refused*
    /// (a `join_request` too short to carry an identity), and a run needs to be able to tell
    /// "we sent three of four" from "we sent four".
    published_membership: bool,
    /// ⭐ The activity name published in `global_activity_state+0x688`. Rung 31 reads it.
    /// `[U]` whether it must match the `act_*` world name the change-world command used - but they
    /// are the only two names in the system and assuming they differ would be the odder claim.
    activity_name: String,
    /// ⭐⭐ The `u16` published at `global_activity_state+0x680`, which is the **only** thing
    /// rung 33 uses to choose which activity's prerequisites to evaluate (`FUN_0078C770` →
    /// `FUN_007A7420` → `FUN_0054F410`). Default 0 = `activity_table[0]` = **"Tower"**, whose
    /// prerequisites are unmet - the `activity_failed_to_load_missing_dlc` of runs
    /// `20260809-054417` and `20260809-075545`, in one field.
    activity_index: u16,
    /// ⭐⭐ The activity checksum published at `global_activity_state+0x5C`. `0` reproduces every
    /// run to date and is rejected by `FUN_00299100` with `Activity checksum mismatch!`.
    activity_checksum: u32,
    /// ⭐⭐ The bubble count published at `global_activity_state+0x10` (and the `0xFF` unchosen
    /// bytes at `+0x14`). `FUN_002A0320` runs right after the checksum passes and requires this to
    /// equal the world's scenario bubble count (`orbit` = 16). `0` = every run to date =
    /// `"invalid host activity state"`.
    bubble_count: u32,
    /// ⭐⭐ Which bubbles are published **chosen** (`0x00`) at `global_activity_state+0x14`; every
    /// other bubble stays `0xFF`. Empty = every bubble unchosen = the pre-2026-08-09 behaviour,
    /// which passes rung 33 and then guarantees rung 34's `error.sstm_slice_not_found` because the
    /// unchosen branch UNREGISTERS the region. `D1_AH_BUBBLE_CHOSEN` sets it.
    /// ⇒ `historicaldocs/design/slice-set-residency.md`.
    bubble_chosen: Vec<usize>,
    /// ⭐⭐ The **PAH-tabulated region** we publish in `replicate_membership+0x2E48`, slot 0.
    /// `None` = the pre-2026-08-09 behaviour: a 40-record array of zeros, which makes
    /// `FUN_007775B0` find nothing, leaves `sstm+0x490` at `-1`, and parks rung 34 on
    /// *"we do not yet have valid host-tabulated data for it. Waiting."* forever.
    /// `Some((region_id, peer_index, token))`. ⇒ `historicaldocs/design/rung34-task7.md`.
    pah_region: Option<(u32, usize, u8)>,
    /// ⭐ The client's own `join_request` struct image, kept because a state refresh has to
    /// re-publish the membership and a membership is an **echo** of exactly these bytes. Without it
    /// a refresh would have to fabricate an identity, which `state.rs` refuses to do anywhere else.
    join_request_image: Option<Vec<u8>>,
    /// ⭐⭐ When we last put a frame on this connection. The client's host-crash test reads
    /// **this** quantity, not the keepalive - see [`HOST_SILENCE_LIMIT_MS`].
    last_pushed: Instant,
    /// How long we may stay silent before speaking unprompted. Must be comfortably under
    /// [`HOST_SILENCE_LIMIT_MS`]; the default is a quarter of it.
    host_heartbeat: Duration,
    /// Counters for the run log.
    pub keepalives: u64,
    pub granted_batches: u64,
    /// ⭐ How many unprompted `global_activity_state` re-publications the heartbeat has made.
    pub heartbeats: u64,
    /// How many `udt 17 activity_client_request_state_refresh` the client asked for.
    pub state_refreshes: u64,
    /// ⭐⭐ The pre-built udt-5 `sensor_auth_update` wire frame that writes the activity lifetime
    /// phase. `None` unless the run set `D1_AH_SENSOR_PHASE`. Built ONCE at setup
    /// ([`crate::codec::SchemaCodec::build_sensor`]) so the ~4 s of oracle setup never lands on the
    /// send path. **Ships `synthetic-minimal`, off by default** - the post-pass dispatch is `[U]`
    /// until the live watch on `record+0x1AC` confirms it (task #4). ⇒ `design/activity-lifetime-state.md`.
    sensor_payload: Option<Vec<u8>>,
    /// ⭐ Per-activity spawn-bundle team: `(team-0 frame, team -1 frame, team-0 activity indices)`.
    /// The type-12 bundle's `A+0x38` must equal the player's team, which the client sets per
    /// activity type: a Private Match picks team 0, a story mission leaves -1. Chosen at each join
    /// from the activity index then in force (after [`Self::activity_follow`]).
    sensor_team_frames: Option<(Vec<u8>, Vec<u8>, std::collections::HashSet<u16>)>,
    /// ⭐ Per-activity WORLD: activity index -> the world served for it. Applied at each join from
    /// the activity index then in force, so one Activity Host serves whatever world the launched
    /// activity loads. See [`ServedWorld`].
    world_table: std::collections::HashMap<u16, ServedWorld>,
    /// ⭐ The served start region (`GlobalActivityState::START_REGION`), with its name hash.
    start_region: Option<(u32, u32)>,
    /// ⭐ The served world clock base in ms (`GlobalActivityState::TIME_BASE_MS`).
    time_base_ms: Option<u64>,
    /// ⭐ Timed mission steps per activity index, the clock start, and the step in force.
    mission_steps: std::collections::HashMap<u16, Vec<MissionStep>>,
    mission_t0: Option<Instant>,
    mission_step: Option<usize>,
    /// When the step in force fired, and every incident type the client reported, with its time.
    mission_step_at: Option<Instant>,
    incidents: Vec<(u32, Instant)>,
    /// The slice set the client last reported as current (udt 21), for [`StepWhen::OnSlice`].
    current_slice: Option<u32>,
    /// Player-trigger reports (incident 3403) by name, and whether a step has used each.
    trigger_reports: Vec<((u32, u8, u16), bool)>,
    /// Squad live-member counts from udt 6 (see [`crate::sense`]): name -> (count, ever above 0).
    squad_live: std::collections::HashMap<(u32, u8, i16), (i32, bool)>,
    /// Host incidents waiting to be sent (see [`MissionStep::incident`]).
    pending_incidents: Vec<(u32, Vec<u8>)>,
    /// ⭐ `D1_AH_CONTROL_FILE`: lines `incident TYPE BODYHEX` queue host incidents on demand
    /// (the file is consumed: read, then deleted). A diagnostic lever, like the 01.24 control file.
    control_file: Option<std::path::PathBuf>,
    /// Builds the frame for a `clients GROUP PATH` control line.
    control_builder: Option<ControlBuilder>,
    /// ⭐ The host teleport in progress: (state, slice-set index, when it was asked).
    slice_request: Option<(u8, u32, Instant)>,
    /// Re-publish `replicate_membership` on the next poll (a teleport state changed).
    membership_dirty: bool,
    /// ⭐ The membership update number at image `+0x00` (the client's `MEM-%d`). The client's
    /// slice-set transition manager (56578 `0x778D60`) re-reads the PAH-tabulated region record
    /// only when this differs from the last one it processed (`sstm+0x3B0`), so every changed
    /// re-publish advances it. Join and udt-17 refresh re-send the same number.
    membership_update: u32,
    /// Set once the sensor frame has gone out, so it is sent exactly once per instance
    /// (send-once mode; see [`Self::set_sensor_repeat`] for the repeating mode).
    sensor_sent: bool,
    /// How many `sensor_auth_update` frames this instance has put on the wire (0 or 1 in
    /// send-once mode; unbounded in repeat mode).
    pub sensors_sent: u64,
    /// ⭐ Re-send interval for the sensor frame (`D1_AH_SENSOR_REPEAT_S`). `None` (default) keeps
    /// the send-once behavior. The once-after-Live send lands during world load, **before the
    /// content record exists**, and the applier drops it silently (task #4) - so a repeating send
    /// is how at least one copy arrives after the world is resident.
    sensor_repeat: Option<Duration>,
    /// When the sensor frame last went out (repeat mode pacing).
    sensor_last_sent: Option<Instant>,
    /// `D1_AH_SENSOR_PHASE_FILE`: a file naming the phase to send, re-read on every send, and one
    /// pre-built frame per allowed phase. An unreadable file or an unlisted phase falls back to
    /// [`Self::sensor_payload`]. Lets an operator step the lifetime phase during one live run.
    sensor_phase_file: Option<(std::path::PathBuf, std::collections::BTreeMap<i8, Vec<u8>>)>,
    /// ⭐ Follow the activity the client launched: a file holding the activity HASH (written by
    /// the responder from the kind-4 matchmaking key) and the build's activity table
    /// (hash -> index). Re-read on every join, so one Activity Host serves whatever was launched.
    activity_follow: Option<(std::path::PathBuf, std::collections::HashMap<u32, u16>)>,
}

impl ActivityInstance {
    /// How many keepalive intervals of silence before eviction. `[U]` - a choice, and a
    /// deliberately forgiving one, because evicting early looks identical to a crash.
    pub const LEASE_INTERVALS: u32 = 5;

    /// ⭐ The activity (the **MAP**) published in `global_activity_state+0x688` unless a caller
    /// overrides it. See [`crate::worlds::DEFAULT_WORLD`] for why it is `pvp_moonbase` and for the
    /// three scenario-record values that move with it.
    ///
    /// > ⛔ **CHANGED 2026-08-10.** This was `act_cosmo_m0_highway`, and paired with the other
    /// > defaults (index 0, checksum 0, bubble count 0, no chosen bubble, no slice token) it
    /// > published **every documented rung-33 and rung-34 wall at once**. Because the good values
    /// > lived only in the operator's shell and no launcher exported them, that default had already
    /// > silently shipped in runs that meant to use the good set. A default that reproduces every
    /// > known failure is not a neutral default.
    pub const DEFAULT_ACTIVITY_NAME: &'static str = crate::worlds::DEFAULT_WORLD.name;

    /// ⭐⭐ The **MODE** - the `u16` index into the client's 540-entry activity table published at
    /// `global_activity_state+0x680`.
    ///
    /// **407 = ELIMINATION**, the project's endgame mode and the one every rung-36 walk was taken
    /// on. Only **21** of 540 activities are launchable at all (`activity_def[+0xA9]`, indices
    /// 393-413, all Crucible private-match modes) because only that band skips rung 29's
    /// matchmaking; a patrol stalls at rung 29, three rungs before everything solved here.
    ///
    /// ⚠ **Deliberately NOT in [`crate::worlds`] and deliberately not derived from the name.** A
    /// launch is TWO values - MODE and MAP - and neither is derived from the other. Setting one and
    /// not the other is a *silent* misconfiguration.
    ///
    /// ⚠ `0` is not a neutral index: the client evaluates `activity_table[0]` = HASH(0x8AE8DA69) =
    /// "Tower", whose prerequisites are unmet, and rung 33 emits
    /// `activity_failed_to_load_missing_dlc` no matter which activity was actually poked.
    pub const DEFAULT_ACTIVITY_INDEX: u16 = 407;

    /// ⭐⭐ The rung-34 slice-set **transition token** published in the PAH-tabulated region record.
    ///
    /// `FUN_0077E5A0` does `token = sstm[0x1D0] + 1; if (token == 0) token = 1;` on **every**
    /// transition, so it is `1` on the first slice-set transition after a boot.
    ///
    /// ⛔ **REFUTED, in place: the token is NOT stale-per-transition.** It **resets on the
    /// fallback** - the client printed `token advanced to 001` on *both* transitions of a two-poke
    /// run and `sstm+0x1D0` reads **0 at rest**. So `1` is correct for **every** poke, not only the
    /// first after a boot, and defaulting it is safe rather than optimistic.
    ///
    /// ⚠ The escape hatch is preserved: `D1_AH_SLICE_TOKEN=0` publishes a **zeroed** PAH array,
    /// which is the pre-2026-08-09 behaviour and the negative control for rung 34 task 8.
    pub const DEFAULT_SLICE_TOKEN: u8 = 1;

    /// ⭐⭐ How long the Activity Host may stay silent before speaking unprompted.
    ///
    /// A quarter of [`HOST_SILENCE_LIMIT_MS`]. Three things went into the number and none of them
    /// is taste:
    ///
    /// * The ceiling is **measured**, not guessed - 20,500 ms, from the client's own log line.
    /// * The margin has to survive *three* consecutive lost or delayed heartbeats, because the
    ///   consequence of one late frame is the whole session, and the cost of an early one is 154
    ///   bytes.
    /// * It must be larger than the client's keepalive cadence (2 s) so the heartbeat is driven by
    ///   an event that certainly happens, rather than needing a timer thread of its own.
    pub const HOST_HEARTBEAT_MS: u64 = HOST_SILENCE_LIMIT_MS / 4;

    pub fn new(ah_id: u64, session_id: impl Into<String>, now: Instant) -> Self {
        let keepalive_interval = Duration::from_millis(u64::from(JoinResult::DEFAULT_KEEPALIVE_MS));
        Self {
            ah_id,
            session_id: session_id.into(),
            state: InstanceState::Allocated,
            entities: EntitySlotAllocator::new(),
            replication_epoch: 0,
            keepalive_interval,
            lease: keepalive_interval * Self::LEASE_INTERVALS,
            last_heard: now,
            published_state: false,
            published_membership: false,
            activity_name: Self::DEFAULT_ACTIVITY_NAME.to_string(),
            // ⚠ Deliberately NOT derived from the activity name: the name and the index are two
            // independent fields of this blob and no mapping between them exists in this tree.
            // 0 is what every run so far sent, and naming what 0 means is the point.
            activity_index: 0,
            activity_checksum: 0,
            bubble_count: 0,
            bubble_chosen: Vec::new(),
            pah_region: None,
            join_request_image: None,
            last_pushed: now,
            host_heartbeat: Duration::from_millis(Self::HOST_HEARTBEAT_MS),
            keepalives: 0,
            granted_batches: 0,
            heartbeats: 0,
            state_refreshes: 0,
            sensor_payload: None,
            sensor_team_frames: None,
            world_table: std::collections::HashMap::new(),
            start_region: None,
            time_base_ms: None,
            mission_steps: std::collections::HashMap::new(),
            mission_t0: None,
            mission_step: None,
            mission_step_at: None,
            incidents: Vec::new(),
            current_slice: None,
            trigger_reports: Vec::new(),
            squad_live: std::collections::HashMap::new(),
            pending_incidents: Vec::new(),
            control_file: None,
            control_builder: None,
            slice_request: None,
            membership_dirty: false,
            membership_update: 1,
            sensor_sent: false,
            sensors_sent: 0,
            sensor_repeat: None,
            sensor_phase_file: None,
            activity_follow: None,
            sensor_last_sent: None,
        }
    }

    pub fn ah_id(&self) -> u64 {
        self.ah_id
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn state(&self) -> InstanceState {
        self.state
    }

    pub fn replication_epoch(&self) -> u8 {
        self.replication_epoch
    }

    pub fn entities(&self) -> &EntitySlotAllocator {
        &self.entities
    }

    /// Whether `11 replicate_membership` has gone out - the last of `FUN_00298EF0`'s four.
    pub fn published_membership(&self) -> bool {
        self.published_membership
    }

    /// A client's BAP connection arrived and finished its handshake.
    pub fn on_connect(&mut self, now: Instant) {
        self.last_heard = now;
        if matches!(self.state, InstanceState::Allocated) {
            self.state = InstanceState::Connected;
        }
    }

    /// The whole state machine, in one function so the ladder is readable top to bottom.
    ///
    /// Returns what to push. An empty vector is a normal outcome - most client → AH messages are
    /// notifications with no reply, and inventing one would be the AH-layer equivalent of the
    /// Weasel.
    pub fn on_client_message(&mut self, msg: &ClientMessage, now: Instant) -> Vec<AhPush> {
        // ⭐ udt 21: the client's slice-set record (struct `0x80800D9F` at `+0xB8`: current
        // `{10-bit index, hash, ..}` at `+0xB8`, TARGET at `+0x158`). When the client walks into
        // another bubble it targets that slice set and waits for "valid host-tabulated data": the
        // region record must name the target with the CLIENT'S token (+0x1F8).
        // Retries can advance it without changing the target: route6 reached 96/token 5
        // while the host's inferred next token was 4. The listener seeds these deltas.
        if let ClientMessage::ClientAuthoritativeDataUpdate { struct_image } = msg {
            self.track_current_slice(struct_image);
            self.follow_slice_target(struct_image);
        }
        // ⭐ udt 18 incidents: {u32 count, u32 types[count], ...}. Kept for incident-driven steps.
        if let ClientMessage::Other { user_defined_type: 18, struct_image } = msg {
            if let Some(n) = struct_image.get(0..4).map(|b| u32::from_le_bytes(b.try_into().unwrap())) {
                for k in 0..n.min(8) as usize {
                    if let Some(b) = struct_image.get(4 + 4 * k..8 + 4 * k) {
                        let t = u32::from_le_bytes(b.try_into().unwrap());
                        tracing::info!("incident {t} reported by the client");
                        self.incidents.push((t, now));
                        if t == 3403 {
                            if let Some(name) = trigger_report(struct_image) {
                                if !self.trigger_reports.iter().any(|(n, used)| *n == name && !used) {
                                    tracing::info!("⭐ player trigger {:#010x}/{:#x}/{} reported", name.0, name.1, name.2);
                                    self.trigger_reports.push((name, false));
                                }
                            }
                        }
                    }
                }
            }
        }
        let mut out = self.reply_to(msg, now);
        // ⭐⭐ THE HOST HEARTBEAT. If this turn produced nothing to say, and we have been silent
        // longer than [`Self::HOST_HEARTBEAT_MS`], say something anyway - because the client's
        // liveness test is `now - (when we last transmitted)` and NOT the keepalive it is sending
        // us. See [`HOST_SILENCE_LIMIT_MS`] for the measurement and the instruction that reads it.
        //
        // ⭐ WHY udt 1 AND NOT ONE OF THE OTHER THREE. The carrier was chosen by reading each
        // handler's success branch, because a heartbeat is by definition sent over and over:
        //
        // | udt | handler | what repeating it does |
        // |---:|---|---|
        // | **1** | `0x1057B80` | `FUN_002407B0(ah_id, image)` then `ac+0x130 \|= 0x40`. A state **snapshot** replacing itself with identical bytes, and an idempotent bit set. ⭐ the least it could do |
        // | 0 | `0x10579F0` | also sets `ac+0xC95`, ORs the mask, and calls `FUN_00240830` - a notification, re-fired every time |
        // | 11 | `0x29C730` | compares the generation and can **zero the peer table** `ac+0x64B4`; 7,297 bytes |
        // | 4 | `0x29BD10` | a *join* result. Re-joining every five seconds is not a heartbeat |
        //
        // It is also the smallest of the four at 154 bytes on the wire, and it is exactly what the
        // client's own `udt 17 activity_client_request_state_refresh` asks for - so repeating it is
        // a thing the protocol already has a name for.
        if out.is_empty() {
            out.extend(self.heartbeat_due(now));
        }
        // ⭐⭐ The udt-5 sensor frame rides after the reply, once the instance is Live. It is armed
        // only when the run set `D1_AH_SENSOR_PHASE`, and it goes out exactly once.
        out.extend(self.sensor_due(now));
        if !out.is_empty() {
            self.last_pushed = now;
        }
        out
    }

    /// ⭐⭐ The host heartbeat, on its own. **Call this on a timer as well as on inbound traffic.**
    ///
    /// Driving it only from inbound messages would be a bet that the client keeps talking - and the
    /// client's keepalive is suppressed by *its* own send-side timer (`FUN_00298C40` compares
    /// `now - ac+0xE48`, which any successful send refreshes). A quiet stretch on the client is
    /// exactly the moment our silence becomes fatal, so the listener also ticks.
    pub fn heartbeat_due(&mut self, now: Instant) -> Option<AhPush> {
        if !matches!(self.state, InstanceState::Live) {
            return None;
        }
        if now.saturating_duration_since(self.last_pushed) < self.host_heartbeat {
            return None;
        }
        self.heartbeats += 1;
        self.last_pushed = now;
        Some(self.publish_activity_state())
    }

    /// ⭐⭐ Arm the udt-5 `sensor_auth_update` frame. Pass the finished wire bytes from
    /// [`crate::codec::SchemaCodec::build_sensor`]; the instance sends them once it is [`Live`].
    ///
    /// [`Live`]: InstanceState::Live
    /// Arm the per-activity world table (see [`Self::world_table`]).
    pub fn set_world_table(&mut self, table: std::collections::HashMap<u16, ServedWorld>) {
        self.world_table = table;
    }

    /// Arm the per-activity team choice (see [`Self::sensor_team_frames`]).
    pub fn set_sensor_team_frames(
        &mut self,
        team0: Vec<u8>,
        team_minus1: Vec<u8>,
        team0_activities: std::collections::HashSet<u16>,
    ) {
        self.sensor_team_frames = Some((team0, team_minus1, team0_activities));
    }

    pub fn set_sensor_payload(&mut self, payload: Vec<u8>) {
        self.sensor_payload = Some(payload);
    }

    /// Whether a sensor frame is armed for this instance.
    pub fn sensor_armed(&self) -> bool {
        self.sensor_payload.is_some()
    }

    /// ⭐ Switch the sensor from send-once to repeating: one frame every `every` while [`Live`],
    /// still gated on the first heartbeat. `D1_AH_SENSOR_REPEAT_S` arms this. The udt-5 applier
    /// ignores a redundant phase write (same value, same generation), so repetition is safe on the
    /// client side; it exists to get one copy delivered *after* the world is resident.
    ///
    /// [`Live`]: InstanceState::Live
    pub fn set_sensor_repeat(&mut self, every: Duration) {
        self.sensor_repeat = Some(every);
    }

    /// Choose the sensor frame per send from the phase named in `path` (see
    /// [`Self::sensor_phase_file`]). `frames` holds one finished frame per allowed phase.
    /// Publish the activity index of the hash in `path`, looked up in `table`, on each join.
    /// Falls back to [`Self::activity_index`] when the file is missing or the hash is unknown.
    pub fn set_activity_follow(
        &mut self,
        path: std::path::PathBuf,
        table: std::collections::HashMap<u32, u16>,
    ) {
        self.activity_follow = Some((path, table));
    }

    /// The `(hash, index)` the followed file names, if the table knows it.
    fn followed_activity_index(&self) -> Option<(u32, u16)> {
        let (path, table) = self.activity_follow.as_ref()?;
        let text = std::fs::read_to_string(path).ok()?;
        let hash: u32 = text.trim().parse().ok()?;
        table.get(&hash).map(|&i| (hash, i))
    }

    pub fn set_sensor_phase_file(
        &mut self,
        path: std::path::PathBuf,
        frames: std::collections::BTreeMap<i8, Vec<u8>>,
    ) {
        self.sensor_phase_file = Some((path, frames));
    }

    fn current_sensor_payload(&self) -> Option<(Vec<u8>, Option<i8>)> {
        if let Some((path, frames)) = &self.sensor_phase_file {
            let phase = std::fs::read_to_string(path)
                .ok()
                .and_then(|s| s.trim().parse::<i8>().ok());
            if let Some(bytes) = phase.and_then(|p| frames.get(&p)) {
                return Some((bytes.clone(), phase));
            }
        }
        self.sensor_payload.clone().map(|b| (b, None))
    }

    /// ⭐⭐ The udt-5 `sensor_auth_update`, sent **once** after the instance reaches [`Live`]
    /// (or on an interval, in [`Self::set_sensor_repeat`] mode).
    ///
    /// It is deliberately **not** on the join path: the applier `FUN_00BAF230` resolves the
    /// activity-client world from the AH id and **drops** the frame if the world does not exist
    /// yet, and the world is not built until the activity-manager ladder reaches state 3. Reaching
    /// `Live` (all four establish messages queued) happens in the *same turn* as the join, before
    /// the client has built anything - so the trigger additionally waits for the **first
    /// heartbeat** to have fired (`heartbeats >= 1`, one `host_heartbeat` period ≈ 5 s later), by
    /// which point the run logs show the ladder has reached state 6 RUNNING and the world is
    /// resident. Gated behind the payload being armed, so a run without `D1_AH_SENSOR_PHASE` never
    /// sends it.
    ///
    /// [`Live`]: InstanceState::Live
    pub fn sensor_due(&mut self, now: Instant) -> Option<AhPush> {
        if !matches!(self.state, InstanceState::Live) || self.heartbeats == 0 {
            return None;
        }
        if self.advance_mission(now) {
            self.sensor_last_sent = None;
        }
        match self.sensor_repeat {
            // Send-once mode (default): silent forever after the first frame.
            None => {
                if self.sensor_sent {
                    return None;
                }
            }
            // Repeat mode: pace on the interval since the last send.
            Some(every) => {
                if let Some(last) = self.sensor_last_sent {
                    if now.saturating_duration_since(last) < every {
                        return None;
                    }
                }
            }
        }
        let (payload, phase) = self.current_sensor_payload()?;
        self.sensor_sent = true;
        self.sensor_last_sent = Some(now);
        self.sensors_sent += 1;
        self.last_pushed = now;
        // Visible send accounting: RADV-12 (2026-08-11) could not tell whether the repeat sends
        // were happening at all - the arming was logged, the sends were not.
        match phase {
            Some(p) => tracing::info!("udt-5 sensor_auth_update send #{} (phase file: {p})", self.sensors_sent),
            None => tracing::info!("udt-5 sensor_auth_update send #{}", self.sensors_sent),
        }
        Some(AhPush::new(self.ah_id, AhMessage::SensorAuthUpdate { payload }))
    }

    /// The heartbeat period. The listener polls at this rate.
    pub fn host_heartbeat(&self) -> Duration {
        self.host_heartbeat
    }

    /// Override the heartbeat period. ⛔ Refused at or above [`HOST_SILENCE_LIMIT_MS`]: a heartbeat
    /// slower than the client's own host-crash timeout is not a heartbeat, it is a countdown.
    pub fn set_host_heartbeat(&mut self, period: Duration) -> bool {
        if period >= Duration::from_millis(HOST_SILENCE_LIMIT_MS) {
            return false;
        }
        self.host_heartbeat = period;
        true
    }

    /// The Activity Host's own transmit-side idle time - the quantity `FUN_0029CA00` measures on
    /// the client. Exposed so a run log can print it next to [`HOST_SILENCE_LIMIT_MS`].
    pub fn silence_on_the_wire(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.last_pushed)
    }

    /// `global_activity_state` for this instance, falling back to an empty one rather than
    /// dropping the push. Used by the join, by `udt 17`, and by the heartbeat.
    fn publish_activity_state(&self) -> AhPush {
        let mut s = GlobalActivityState::for_activity(&self.activity_name)
            .unwrap_or_else(|_| GlobalActivityState::empty());
        // ⭐⭐ The field rung 33 gates on. See `GlobalActivityState::ACTIVITY_INDEX`.
        s.set_activity_index(self.activity_index);
        // ⭐⭐ The field that aborts rung 33's world transition when it disagrees with content.
        s.set_activity_checksum(self.activity_checksum);
        // ⭐⭐ The bubble count + unchosen bytes that FUN_002A0320 validates right after. Bounded
        // to the 64-byte field; a too-large count is a config error, so publish what we can and let
        // the loud "bubble count mismatch" name a wrong number rather than silently truncating.
        let _ = s.set_bubble_count(self.bubble_count.min(GlobalActivityState::BUBBLE_STATE_CAPACITY as u32));
        // ⭐⭐ THEN mark the chosen bubbles `0x00`. `set_bubble_count` rewrites the whole 64-byte
        // field, so order is load-bearing. An all-`0xFF` blob passes rung 33 and then UNREGISTERS
        // every slice-set region, which is rung 34's `error.sstm_slice_not_found`.
        let _ = s.set_bubble_chosen(&self.bubble_chosen);
        if let Some(ms) = self.time_base_ms {
            s.set_time_base_ms(ms);
        }
        if let Some((region, hash)) = self.start_region {
            s.set_start_region(region, hash);
        }
        AhPush::new(self.ah_id, AhMessage::GlobalActivityState(s))
    }

    /// ⭐⭐ `replicate_membership` for this instance - **the ONE place every membership field that
    /// is OURS to assert gets applied**, used by the join path AND by the `udt 17` refresh path.
    ///
    /// ## Why this is a function and not two code paths
    ///
    /// `ReplicateMembership::solo()` is a **pure ECHO** of the client's own `join_request` bytes.
    /// The client sends `udt 17 activity_client_request_state_refresh` **0.37 s after the join**
    /// (measured, `20260809-120451-MOONBASE-FRESH` 20:57:41.264 → .632) and that handler rebuilds
    /// the membership from `solo()` again - so **last writer wins**, and any field applied only on
    /// the join path is silently zeroed a third of a second later. That is exactly what happened
    /// to the PAH-tabulated region record: the client's own copy at `c_activity_client+0x3CB8`
    /// read ALL ZEROS while the identity echo in the same object was byte-correct, and rung 34
    /// task 8 parked forever on *"we do not yet have valid host-tabulated data for it. Waiting."*
    ///
    /// ⭐⭐ **THE TRANSFERABLE RULE, and the reason this is now structural rather than a second
    /// copy of the same three lines: when a re-publish path rebuilds an object from the client's
    /// own ECHO, every field that is ours to assert must be re-applied there too - ENUMERATE them,
    /// do not spot-check.** Three blob fixes in a row worked and confirmed the method while the
    /// fourth silently did not, because [`Self::publish_activity_state`] is re-derived from `self`
    /// on both paths and only the membership is an echo. Enumerating by hand is what failed; a
    /// single builder is what makes it impossible to fail again. **A new membership field belongs
    /// HERE and nowhere else** - and [`crate::controls`] asserts the two paths are byte-identical
    /// on the wire, so applying one in an arm instead goes red.
    ///
    /// ⛔ Refused, not improvised, when the request is too short: a membership with a zero identity
    /// still sets `ac+0x131 & 1` (`0x29C730` sets it unconditionally), so a fabricated one would
    /// pass the gate and then fail `FUN_0029FE70`'s slot search silently. Losing the push is louder.
    fn build_membership(&self, join_request_image: &[u8]) -> Option<ReplicateMembership> {
        let mut m = ReplicateMembership::solo(join_request_image).ok()?;
        let first = !self.published_membership;
        m.image[..4].copy_from_slice(&self.membership_update.to_le_bytes());

        // ── field 1 of 2 that is OURS: acknowledge the peer-session host ──────────────────────
        //
        // `FUN_0029FE40 -> FUN_002A4970` compares membership+0x5020 with the local host player
        // identifier. A zero here is the exact cause of the repeating matchmaking-manager line
        // "We are being identified as the host of this session, but the AH has not acknowledged
        // this". The client supplied the identity in its join request; for this deliberately solo
        // host, member 0 is the session host. As with the PAH record below, this belongs in the
        // shared builder because udt 17 re-publishes the membership immediately after the join.
        if let Err(e) = m.acknowledge_session_host(0) {
            tracing::warn!(
                "⚠ peer-session host NOT acknowledged in replicate_membership ({e}) - the world \
                 controller will remain in the host/Activity-Host disagreement loop"
            );
            return None;
        }
        if first {
            tracing::info!(
                "⭐ acknowledging membership member 0 as peer-session host at \
                 replicate_membership+0x{:04X}; FUN_0029FE40 compares this identity with the \
                 local host and the world controller will not advance while they differ",
                ReplicateMembership::ACKNOWLEDGED_SESSION_HOST
            );
        }

        // ── field 2 of 2 that is OURS: the PAH-tabulated region record (rung 34 task 8) ────────
        //
        // Without it `FUN_007775B0` copies nothing into `sstm+0x490`, `FUN_0077F6B0` refuses to
        // assign the slice-set-switch task, `FUN_0077A150()` stays false and task 8 returns 0
        // forever. Applied here rather than inside `solo()` because `solo()` is the echo and this
        // is one of the two parts of the membership that are ours to assert.
        // ⚠ The FIRST build (the join) logs at INFO because that line is a run's provenance and
        // the operator reads it; every re-publish after it logs at DEBUG, because a refresh every
        // few seconds repeating the same receipt turns the run log into noise and hides the
        // warnings. The condition is the join/refresh discriminator we already track.
        if let Some((state, index, _)) = self.slice_request {
            if let Err(e) = m.set_slice_request(state, index, 0x811C_9DC5) {
                tracing::warn!("⚠ teleport request not written: {e}");
            }
        }
        match self.pah_region {
            Some((region, peer, token)) => match m.set_pah_region(region, peer, token) {
                Ok(()) if first => tracing::info!(
                    "⭐ publishing PAH-tabulated region {region} for peer {peer} with transition \
                     token {token} at replicate_membership+0x{:04X} - FUN_007775B0 copies this \
                     record into sstm+0x490 iff region == sstm+0x138 AND record[0x38+{peer}] == \
                     sstm+0x1D0. ⭐ Re-applied identically on every udt-17 state refresh; a \
                     control asserts the two encodings are byte-identical.",
                    ReplicateMembership::PAH_REGIONS
                ),
                Ok(()) => tracing::debug!(
                    "PAH-tabulated region {region} re-applied for peer {peer}, token {token}"
                ),
                Err(e) => tracing::warn!(
                    "⚠ PAH-tabulated region NOT published ({e}) - rung 34 will park on 'we do not \
                     yet have valid host-tabulated data'"
                ),
            },
            None if first => tracing::warn!(
                "⚠ D1_AH_SLICE_TOKEN=0 - publishing a ZEROED PAH tabulated region array at \
                 replicate_membership+0x{:04X}. FUN_007775B0 will match nothing, sstm+0x490 stays \
                 -1, and rung 34 parks forever on 'Region … is ready for switch, but we do not yet \
                 have valid host-tabulated data for it. Waiting.' That is every run before \
                 2026-08-09, kept as the bisect lever. ⇒ historicaldocs/design/rung34-task7.md",
                ReplicateMembership::PAH_REGIONS
            ),
            None => {}
        }

        // ⚠ ANY FUTURE MEMBERSHIP FIELD GOES ABOVE THIS LINE. If you are about to add one to the
        // JoinRequest arm of `reply_to`, that is the bug this function exists to prevent.
        Some(m)
    }

    /// The protocol reply, before the heartbeat is considered.
    fn reply_to(&mut self, msg: &ClientMessage, now: Instant) -> Vec<AhPush> {
        self.last_heard = now;
        let recipient = self.ah_id;
        match msg {
            // ── rung: wait 4 join_result ───────────────────────────────────────────────────────
            ClientMessage::JoinRequest { struct_image } => {
                if let Some((hash, index)) = self.followed_activity_index() {
                    if index != self.activity_index {
                        tracing::info!(
                            "⭐ activity follow: the client launched activity hash {hash:#010x} = index \
                             {index}; publishing it instead of {}",
                            self.activity_index
                        );
                    }
                    self.activity_index = index;
                } else if self.activity_follow.is_some() {
                    tracing::warn!(
                        "activity follow: no known launched activity; publishing the configured index {}",
                        self.activity_index
                    );
                }
                if let Some(w) = self.world_table.get(&self.activity_index).cloned() {
                    tracing::info!(
                        "⭐ world table: activity index {} -> world {:?} (checksum {:#010x}, {} bubble(s))",
                        self.activity_index, w.name, w.checksum, w.bubble_count
                    );
                    self.activity_name = w.name;
                    self.activity_checksum = w.checksum;
                    self.bubble_count = w.bubble_count;
                    self.bubble_chosen = w.chosen;
                    // The PAH region record must name the same region the client will wait on.
                    self.start_region = w.start_region.map(|r| (r, 0x811C_9DC5));
                    let region = w.start_region.unwrap_or(0);
                    if let Some((_, peer, token)) = self.pah_region {
                        self.pah_region = Some((region, peer, token));
                    }
                    self.sensor_payload = Some(w.team_minus1.clone());
                    if let Some((_, _, set)) = self.sensor_team_frames.take() {
                        self.sensor_team_frames = Some((w.team0, w.team_minus1, set));
                    }
                }
                if let Some((t0, tm1, set)) = &self.sensor_team_frames {
                    let team0 = set.contains(&self.activity_index);
                    self.sensor_payload = Some(if team0 { t0.clone() } else { tm1.clone() });
                    tracing::info!(
                        "⭐ spawn bundle team {} for activity index {}",
                        if team0 { 0 } else { -1 },
                        self.activity_index
                    );
                }
                // ⭐ Echo the client's whole request struct: `join_result`'s descriptor field [0]
                // IS `join_request` (tag 0x80800E98) embedded at +0x00, which is why the nonce and
                // AH id live where they do. `accepted_echoing` takes the nonce from the image, so
                // there is one source for it rather than two that can disagree.
                let mut result = JoinResult::accepted_echoing(
                    struct_image,
                    self.ah_id,
                    &self.session_id,
                    self.replication_epoch,
                );
                result.keepalive_interval_ms = self.keepalive_interval.as_millis().min(
                    u128::from(u16::MAX),
                ) as u16;
                self.state = InstanceState::Joined;
                // Kept for `udt 17`: a membership is an echo of these bytes, so a refresh that did
                // not have them would have to fabricate an identity.
                self.join_request_image = Some(struct_image.clone());
                let mut out = vec![AhPush::new(recipient, AhMessage::JoinResult(result))];

                // ⭐⭐ ALL FOUR, IN ONE BREATH - `FUN_00298EF0` is a conjunction, not a sequence.
                //
                // Read term by term, its whole body is a per-frame poll of six live flags:
                //
                // ```c
                // ac[0xE58] != 0                             // the world exists
                //   && FUN_002A2220(world)                   // udt 1  arrived  (world+0x50A8)
                //   && blob[0] != 0 && (blob[1] == 0 || *(i32*)(blob+0x10) >= 1)
                //   && vfunc[3]()                            // udt 4  ACCEPTED (ac+0x130 & 0x20)
                //   && ac+0x64AA != 0                        // udt 0  arrived
                //   && (ac+0x131 & 1)                        // udt 11 arrived
                // ```
                //
                // There is no sequencing state anywhere in it, so what the client needs is
                // **arrival-completeness, not order**. We still send them in `4 → 0 → 1 → 11`
                // because that is the order the client's *own* substep machine `FUN_00760470`
                // waits in - status 26 join, 27 entity indices, 28 instantiation - and matching a
                // ladder that only ever waits costs nothing while making the log read in ladder
                // order.
                //
                // ⭐ And all three of these are UNSOLICITED on purpose. `FUN_010579F0` (udt 0) has
                // no "was it requested?" check of any kind - it decodes, sets `ac+0x64AA = 1` and
                // ORs the mask into `ac+0x5EA8` - so waiting for udt 19, as this file used to,
                // made the whole gate depend on `*(char*)(FUN_0034D950()+0x81)`, which is `[U]`.
                // In run `20260808-234535-CODEC-LIVE2` udt 19 never came and the client timed out
                // with `EST-N AH->0 MEM-0`.
                if !self.published_state {
                    self.published_state = true;

                    // udt 0 - the entity-index gate. A full grant: the client asks for batches and
                    // returns what it does not use, and `0xB4DF80` sizes the pool at 12,288 either
                    // way. ⚠ `activity-host-contract.md` §5's "offline seam" (`memset 0xFF`) is
                    // NOT an alternative - that branch writes `db+0x120B8`, never `ac+0x64AA`,
                    // and `ac+0x64AA` is written in exactly one place in the whole image: here.
                    out.push(AhPush::new(
                        recipient,
                        AhMessage::EntitySlotsAllocated(self.entities.grant_all()),
                    ));

                    // udt 1 - ⭐ NOT `empty()`. Load-bearing three times over: establish-client,
                    // the rung-31 activity NAME, and the rung-33 world transition. `FUN_01057B80`
                    // sets `ac+0x130 |= 0x40` only on a successful decode of it.
                    out.push(self.publish_activity_state());

                    // udt 11 - ⭐ the last gate, and an ECHO: the member is the client, and the
                    // client's identity is the 40 bytes it just sent us at `join_request+0x10`.
                    //
                    // ⭐⭐ Built by [`Self::build_membership`], which is ALSO what the `udt 17`
                    // refresh arm below calls. Do not apply a membership field here - the refresh
                    // rebuilds this object 0.37 s later and would zero it. See that function.
                    if let Some(m) = self.build_membership(struct_image) {
                        self.published_membership = true;
                        out.push(AhPush::new(recipient, AhMessage::ReplicateMembership(m)));
                    }
                }
                // All four of `FUN_00298EF0`'s messages are out. Anything less stays `Joined`, so
                // a run log distinguishes "we answered" from "we answered completely".
                if self.published_state && self.published_membership {
                    self.state = InstanceState::Live;
                }
                out
            }

            // ── rung: wait 0 entity_slots_allocated ───────────────────────────────────────────
            //
            // Still honoured if it comes: the client's pool policy frees down to a high-water mark
            // and re-requests below a low-water one, so a live activity does keep asking.
            ClientMessage::AllocateEntityIndices { count } => {
                let grant = self.entities.allocate(*count);
                self.granted_batches += 1;
                if matches!(self.state, InstanceState::Joined) {
                    self.state = InstanceState::Live;
                }
                vec![AhPush::new(recipient, AhMessage::EntitySlotsAllocated(grant))]
            }
            ClientMessage::FreeEntityIndices(mask) => {
                self.entities.free(mask);
                Vec::new()
            }

            // ── the lease ─────────────────────────────────────────────────────────────────────
            //
            // ⛔ NOTHING GOES BACK **FOR THIS MESSAGE**. `activity_client_keepalive_response`
            // (udt 16) is in the name table and in **no** dispatch table - the client cannot
            // handle it - and udt 15 itself has no schema tag at all: `FUN_00298C40` passes
            // descriptor `0xFFFFFFFF`, sets the raw flag, and sends one *uninitialised* stack byte.
            // There is nothing in it to read and nothing to answer.
            //
            // ⭐⭐ What keeps the session alive is [`Self::on_client_message`]'s heartbeat, which
            // this arm *reaches* by returning nothing: the client's liveness test is our transmit
            // idle time, not this message. Answering udt 15 specifically would have been the right
            // shape by accident.
            ClientMessage::Keepalive => {
                self.keepalives += 1;
                Vec::new()
            }

            // ── the client asking us to re-publish ────────────────────────────────────────────
            //
            // ⭐ `c_activity_client::vfunc[5]` (`0x29AB80`) sends this from the accept branch of the
            // `join_result` handler, and its 8-byte body leads with the membership counter the
            // client currently holds. It is the client's own cue that it wants state - so a refresh
            // re-sends the three unsolicited messages, deliberately bypassing the `published_state`
            // guard that stops the *join* from repeating them.
            ClientMessage::RequestStateRefresh { .. } => {
                self.state_refreshes += 1;
                // ⭐⭐ udt 0 goes back EMPTY. The client's apply is a DELTA, not a state:
                // `0x1018800` ORs the mask into the inbox at `ac+0x5EA8`, and `0xBB57F0` →
                // `0xB53BB0` ORs the inbox straight into each replication db's free-slot pool at
                // `db+0x120B8` with no "already in use?" check. Re-sending `granted()` here
                // re-freed every live slot, so the next allocation handed slot 0 out a second time
                // (orbit entity at `0xE2A5B`, world entity at `0x227400`), and teardown faulted in
                // `0xBC09AE` reading the record table at index -1 (run 2026-10-14-slots). An empty
                // mask still decodes and still sets `ac+0x64AA`, which is all the gate wants.
                let mut out = vec![
                    AhPush::new(recipient, AhMessage::EntitySlotsAllocated(EntitySlotMask::empty())),
                    self.publish_activity_state(),
                ];
                // The membership stays an ECHO. With no stored request there is nothing to echo
                // and the push is lost rather than fabricated - the same rule as the join.
                //
                // ⭐⭐ AND THE PAH-TABULATED REGION MUST BE RE-APPLIED HERE, or this refresh
                // OVERWRITES the good record the join just published with a zeroed one.
                // `solo()` is a pure echo of the client's own bytes; the PAH record is the one
                // part of the membership that is ours to assert, so it does not survive a rebuild;
                // the host acknowledgement at +0x5020 has the same last-writer constraint.
                //
                // Measured live (`20260809-120451-MOONBASE-FRESH`, 2026-08-09): the client sends
                // `RequestStateRefresh` **0.37 s after the join push**, and its own copy of the
                // array at `c_activity_client+0x3CB8` (= membership object `+0x2E50`) then reads
                // ALL ZEROS - region 0, token byte 0 where we sent 1, and no `-1` terminator -
                // while the identity echo in the same object is byte-correct. `FUN_007775B0`
                // therefore matched nothing, `sstm+0x490` stayed `-1`, and rung 34 task 8 parked
                // on "we do not yet have valid host-tabulated data for it. Waiting." forever.
                //
                // ⚠ This is the "last writer wins" trap: `publish_activity_state()` above is
                // re-derived from `self`, so every *blob* knob (checksum, bubble count, bubble
                // chosen) survived a refresh - which is precisely why those three fixes worked and
                // this one silently did not. ⇒ historicaldocs/design/rung34-task7.md, status/LESSONS.md.
                //
                // ⭐⭐ THE STRUCTURAL FIX: this calls the SAME [`Self::build_membership`] the join
                // does, so the two paths cannot diverge by omission. A control asserts the two
                // pushes are byte-identical on the wire; applying a field in one arm goes red.
                if let Some(img) = self.join_request_image.clone() {
                    if let Some(m) = self.build_membership(&img) {
                        out.push(AhPush::new(recipient, AhMessage::ReplicateMembership(m)));
                    }
                }
                out
            }

            ClientMessage::ConnectivityFailure { .. } => {
                self.state =
                    InstanceState::Evicted { reason: EvictionReason::ConnectivityFailure };
                Vec::new()
            }

            // ⛔ **NOT A GATE, AND DELIBERATELY NOT ANSWERED.** `udt 14 peer_leave_request` is the
            // client giving up. In run `20260809-054417-RUNG33-BYPASS` it arrived 0.3 s *after* the
            // client had already logged `Lost connection to activity host` and resolved the
            // instance to `error_host_crashed` - so it is downstream of the failure by a third of a
            // second. Recording it is worth doing; "handling" it would be treating the symptom.
            ClientMessage::PeerLeaveRequest { .. } => {
                self.state = InstanceState::Evicted { reason: EvictionReason::ClientLeft };
                Vec::new()
            }

            // ⭐ `udt 21 client_authoritative_data_update` - the client telling us **its own**
            // authoritative state (Aldridge's model: a client holds limited authority over its own
            // biped). `FUN_0029CC20` sends it and `FUN_002987E0`'s success branch does nothing but
            // `memcpy(ac+0x6748, ac+0x6968, 0x220)` and clear the dirty mask; there is no reply
            // message for it in the 49-id space and no client handler that could receive one.
            // ⇒ **fire-and-forget, and answering it would be the udt-16 mistake again.**
            //
            // Accepted and recorded, not acted on. A stub with one peer has nothing to reserve
            // against, and `43 reservations_failed` is for failures we cannot have.
            ClientMessage::ClientAuthoritativeDataUpdate { .. }
            | ClientMessage::MembershipAcknowledgement { .. }
            | ClientMessage::ClientIdentityUpdate { .. }
            | ClientMessage::Other { .. } => Vec::new(),
        }
    }

    /// Advance the replication epoch. `[C]` a `u8`; the wire payload is `{u8 epoch, u8 suppress}`.
    ///
    /// Nothing in a stub needs to call this - it exists because the epoch is authoritative state
    /// and the AH is the only thing entitled to move it.
    pub fn advance_replication_epoch(&mut self, suppress: bool) -> AhPush {
        self.replication_epoch = self.replication_epoch.wrapping_add(1);
        AhPush::new(
            self.ah_id,
            AhMessage::AdvanceReplicationEpoch {
                epoch: self.replication_epoch,
                suppress: u8::from(suppress),
            },
        )
    }

    /// Lease enforcement. Call on a timer.
    ///
    /// Returns `Some(reason)` the first time the lease lapses; afterwards the instance stays
    /// evicted and this returns `None`, so a caller cannot log the same eviction twice.
    pub fn tick(&mut self, now: Instant) -> Option<EvictionReason> {
        if matches!(self.state, InstanceState::Evicted { .. } | InstanceState::Allocated) {
            return None;
        }
        if now.saturating_duration_since(self.last_heard) >= self.lease {
            self.state = InstanceState::Evicted { reason: EvictionReason::InactivityTimeout };
            return Some(EvictionReason::InactivityTimeout);
        }
        None
    }

    /// How long since we last heard anything, for the run log.
    pub fn silence(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.last_heard)
    }

    /// The activity name this instance publishes.
    pub fn activity_name(&self) -> &str {
        &self.activity_name
    }

    /// Override the activity name. Takes effect on the next `global_activity_state` publication.
    pub fn set_activity_name(&mut self, name: impl Into<String>) {
        self.activity_name = name.into();
    }

    /// The activity index this instance publishes at `global_activity_state+0x680`.
    pub fn activity_index(&self) -> u16 {
        self.activity_index
    }

    /// ⭐⭐ Override the activity index. Takes effect on the next `global_activity_state`.
    ///
    /// This is the number rung 33 evaluates. `D1_AH_ACTIVITY_INDEX` sets it.
    pub fn set_activity_index(&mut self, index: u16) {
        self.activity_index = index;
    }

    /// The activity checksum this instance publishes at `global_activity_state+0x5C`.
    pub fn activity_checksum(&self) -> u32 {
        self.activity_checksum
    }

    /// ⭐⭐ Override the activity checksum. Takes effect on the next `global_activity_state`.
    ///
    /// `FUN_00299100` compares this against a value resolved out of the content registry and
    /// **aborts the world transition** on disagreement. `D1_AH_ACTIVITY_CHECKSUM` sets it.
    pub fn set_activity_checksum(&mut self, checksum: u32) {
        self.activity_checksum = checksum;
    }

    /// The bubble count this instance publishes at `global_activity_state+0x10`.
    pub fn bubble_count(&self) -> u32 {
        self.bubble_count
    }

    /// ⭐⭐ Override the bubble count. Takes effect on the next `global_activity_state`.
    ///
    /// `FUN_002A0320` requires `blob+0x10 == scenario_record+0x48` (the world's bubble count) and
    /// then walks that many `blob+0x14+i` bytes, which we set to `0xFF` (unchosen). A mismatch
    /// aborts the world transition with `"invalid host activity state"` (ours 0) or
    /// `"activity client bubble count mismatch"`. `D1_AH_BUBBLE_COUNT` sets it.
    pub fn set_bubble_count(&mut self, count: u32) {
        self.bubble_count = count;
    }

    /// The bubbles this instance publishes as CHOSEN (`0x00`) at `global_activity_state+0x14`.
    pub fn bubble_chosen(&self) -> &[usize] {
        &self.bubble_chosen
    }

    /// ⭐⭐ Choose bubbles. Takes effect on the next `global_activity_state`.
    ///
    /// This is the rung-34 knob: an unchosen (`0xFF`) bubble makes `FUN_002A0320` **unregister**
    /// its slice-set region, and rung 34's `FUN_0077E440` then cannot find it
    /// (`error.sstm_slice_not_found`). `D1_AH_BUBBLE_CHOSEN` sets it.
    pub fn set_bubble_chosen(&mut self, chosen: Vec<usize>) {
        self.bubble_chosen = chosen;
    }

    /// The PAH-tabulated region record this instance publishes, if any.
    pub fn pah_region(&self) -> Option<(u32, usize, u8)> {
        self.pah_region
    }

    /// ⭐⭐ The rung-34 knob: publish a PAH-tabulated region record in `replicate_membership`.
    ///
    /// Takes effect on the next membership push - which, because `FUN_0077E5A0` resets
    /// `sstm+0x3B0` to `-1` at the start of **every** slice-set transition, is re-scanned by
    /// `FUN_007775B0` at each transition without us having to re-send anything.
    /// `D1_AH_SLICE_REGION` / `D1_AH_SLICE_TOKEN` set it. ⇒ `historicaldocs/design/rung34-task7.md`.
    /// How long the teleport request stays in state 1 before it is finished with state 3, and how
    /// long state 3 stays before the request is dropped. Fixed times stand in for the 01.24
    /// stand-in's wait on the client's slice record (state 4, then current).
    /// Incidents this soon after a step are the client echoing that step (a stop we sent comes
    /// back as incident 891 within half a second) and do not fire the next step.
    pub const INCIDENT_ECHO: Duration = Duration::from_secs(2);
    pub const TELEPORT_FINISH_AFTER: Duration = Duration::from_secs(20);
    pub const TELEPORT_DROP_AFTER: Duration = Duration::from_secs(30);

    /// ⭐ `replicate_membership` again when a teleport state changed: state 1 → 3 → dropped.
    pub fn membership_due(&mut self, now: Instant) -> Option<AhPush> {
        if let Some((state, index, at)) = self.slice_request {
            let age = now.saturating_duration_since(at);
            if state == 1 && age >= Self::TELEPORT_FINISH_AFTER {
                self.slice_request = Some((3, index, at));
                self.membership_dirty = true;
            } else if state == 3 && age >= Self::TELEPORT_DROP_AFTER {
                self.slice_request = None;
                self.membership_dirty = true;
            }
        }
        if !self.membership_dirty || !matches!(self.state, InstanceState::Live) {
            return None;
        }
        let img = self.join_request_image.clone()?;
        self.membership_update = self.membership_update.wrapping_add(1).max(1);
        let m = self.build_membership(&img)?;
        self.membership_dirty = false;
        self.last_pushed = now;
        tracing::info!(
            "⭐ replicate_membership re-published as update {}: teleport request {:?}, region record {:?}",
            self.membership_update,
            self.slice_request.map(|(s, i, _)| (s, i)),
            self.pah_region
        );
        Some(AhPush::new(self.ah_id, AhMessage::ReplicateMembership(m)))
    }

    /// The next host incident to send, as udt 18 (`0x8080097D`, 716 bytes): `+0` the count (5 bits),
    /// `+4` the 12-bit types, `+0x68` an optional list (left empty), `+0xD4` the body length in bytes
    /// (9 bits) and `+0xD8` the body (up to 500 bytes).
    pub fn incident_due(&mut self) -> Option<AhPush> {
        self.poll_control();
        if !matches!(self.state, InstanceState::Live) || self.pending_incidents.is_empty() {
            return None;
        }
        let (t, body) = self.pending_incidents.remove(0);
        Some(AhPush::new(self.ah_id, AhMessage::Opaque { user_defined_type: 18, bytes: incident_image(t, &body) }))
    }

    /// Use `path` as the control file (see [`ActivityInstance::incident_due`]).
    pub fn set_control_file(&mut self, path: std::path::PathBuf) {
        self.control_file = Some(path);
    }

    /// Let `clients GROUP PATH` control lines publish a sensor-client list at once (the same frame a
    /// mission step builds), for trying states without a new run.
    pub fn set_control_builder(&mut self, b: ControlBuilder) {
        self.control_builder = Some(b);
    }

    fn poll_control(&mut self) {
        let Some(path) = self.control_file.as_ref() else { return };
        let Ok(text) = std::fs::read_to_string(path) else { return };
        let _ = std::fs::remove_file(path);
        for line in text.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            if let ["clients", g, file] = f.as_slice() {
                let group = u32::from_str_radix(g.trim_start_matches("0x"), 16).ok();
                match (group, self.control_builder.clone()) {
                    (Some(group), Some(b)) => {
                        let team0 = matches!(&self.sensor_team_frames, Some((_, _, set)) if set.contains(&self.activity_index));
                        match (b.0)(group, file, if team0 { 0 } else { -1 }) {
                            Ok(bytes) => {
                                tracing::info!("⭐ control: sensor clients {file} ({} bytes)", bytes.len());
                                self.sensor_payload = Some(bytes);
                            }
                            Err(e) => tracing::warn!("control: clients {file}: {e}"),
                        }
                    }
                    _ => tracing::warn!("control: ignored line {line:?} (bad group, or no builder)"),
                }
                continue;
            }
            let parsed = match f.as_slice() {
                ["incident", t, hex] => t.parse::<u32>().ok().zip(
                    (0..hex.len()).step_by(2)
                        .map(|k| hex.get(k..k + 2).and_then(|b| u8::from_str_radix(b, 16).ok()))
                        .collect::<Option<Vec<u8>>>(),
                ),
                _ => None,
            };
            match parsed {
                Some((t, body)) => {
                    tracing::info!("⭐ control: host incident {t} ({} body bytes)", body.len());
                    self.pending_incidents.push((t, body));
                }
                None if line.trim().is_empty() => {}
                None => tracing::warn!("control: ignored line {line:?} (want `incident TYPE BODYHEX` or `clients GROUP PATH`)"),
            }
        }
    }

    /// Record squad live-member counts the client reported (udt 6, see [`crate::sense`]).
    pub fn on_squad_counts(&mut self, counts: &[crate::sense::SquadCount]) {
        for c in counts {
            let e = self.squad_live.entry((c.group, c.kind, c.index)).or_insert((0, false));
            if e.0 != c.live {
                tracing::info!("⭐ squad {:#010x}/{:#x}/{} live members {} -> {}", c.group, c.kind, c.index, e.0, c.live);
            }
            *e = (c.live, e.1 || c.live > 0);
        }
    }

    /// True when group G has at least one kind-1 squad that was reported alive, and every such
    /// squad now reports 0 live members.
    pub fn squads_cleared(&self, group: u32) -> bool {
        let seen: Vec<i32> = self
            .squad_live
            .iter()
            .filter(|((g, k, _), (_, alive))| *g == group && *k == 1 && *alive)
            .map(|(_, (n, _))| *n)
            .collect();
        !seen.is_empty() && seen.iter().all(|n| *n <= 0)
    }

    /// Arm the timed mission steps (see [`MissionStep`]).
    pub fn set_mission_steps(&mut self, steps: std::collections::HashMap<u16, Vec<MissionStep>>) {
        self.mission_steps = steps;
    }

    /// Move to the latest mission step that is due. Returns true when the step changed, so the
    /// caller sends its state now instead of waiting for the repeat interval.
    fn advance_mission(&mut self, now: Instant) -> bool {
        let Some(steps) = self.mission_steps.get(&self.activity_index) else {
            return false;
        };
        let t0 = *self.mission_t0.get_or_insert(now);
        let elapsed = now.saturating_duration_since(t0);
        let i = self.mission_step.map_or(0, |s| s + 1);
        let Some(step) = steps.get(i).cloned() else {
            return false;
        };
        let prev = self.mission_step_at.unwrap_or(t0);
        let due = match step.when {
            StepWhen::At(d) => elapsed >= d,
            StepWhen::AfterPrevious(d) => now.saturating_duration_since(prev) >= d,
            StepWhen::OnTrigger { group, kind, index, timeout } => {
                let hit = self.trigger_reports.iter_mut().find(|(n, used)| *n == (group, kind, index) && !used);
                let reported = hit.is_some();
                if let Some((_, used)) = hit {
                    *used = true;
                    tracing::info!("⭐ mission step {i}: player trigger {group:#010x}/{kind:#x}/{index}");
                }
                reported || now.saturating_duration_since(prev) >= timeout
            }
            StepWhen::OnSquadsCleared { group, timeout } => {
                let cleared = self.squads_cleared(group);
                if cleared {
                    tracing::info!("⭐ mission step {i}: every squad of {group:#010x} the client reported alive is down");
                }
                cleared || now.saturating_duration_since(prev) >= timeout
            }
            StepWhen::OnSlice { slice, timeout } => {
                let here = self.current_slice == Some(slice);
                if here {
                    tracing::info!("⭐ mission step {i}: the client is in slice set {slice}");
                }
                here || now.saturating_duration_since(prev) >= timeout
            }
            StepWhen::OnIncident { incidents, timeout } => {
                let reported = self.incidents.iter().find(|(t, at)| {
                    incidents.contains(t) && at.saturating_duration_since(prev) >= Self::INCIDENT_ECHO
                });
                if let Some((t, _)) = reported {
                    tracing::info!("⭐ mission step {i}: the client reported incident {t}");
                }
                reported.is_some() || now.saturating_duration_since(prev) >= timeout
            }
        };
        if !due {
            return false;
        }
        let team0 = matches!(&self.sensor_team_frames, Some((_, _, set)) if set.contains(&self.activity_index));
        if let Some((t, body)) = step.incident.clone() {
            tracing::info!("⭐ mission step {i}: host incident {t} ({} body bytes)", body.len());
            self.pending_incidents.push((t, body));
        }
        self.sensor_payload = Some(if team0 { step.team0 } else { step.team_minus1 });
        if let Some(r) = step.region {
            // The host teleport: ask the group to move to slice set `r` and table its region.
            self.set_start_region(r);
            // Each transition advances the client's token (`t + 1`, skipping 0); the region record
            // matches only with the token of THIS transition (run agr8: token 1 left the client
            // "waiting for valid host-tabulated data" on PRV112.1).
            if let Some((region, peer, token)) = self.pah_region {
                self.pah_region = Some((region, peer, if token == u8::MAX { 1 } else { token + 1 }));
            }
            self.slice_request = Some((1, r, now));
            self.membership_dirty = true;
        }
        tracing::info!(
            "⭐ mission step {i} at +{:.0} s for activity index {}{}",
            elapsed.as_secs_f32(),
            self.activity_index,
            step.region.map(|r| format!(", region {r}")).unwrap_or_default()
        );
        self.mission_step = Some(i);
        self.mission_step_at = Some(now);
        true
    }

    /// Serve a world clock base in ms (see `GlobalActivityState::TIME_BASE_MS`).
    pub fn set_time_base_ms(&mut self, ms: u64) {
        self.time_base_ms = Some(ms);
    }

    /// Record the client's current slice set from udt 21 (`+0xB8` = {10-bit index, name hash}).
    /// The update is a delta: the record is present only when its hash is (not 0 / FNV basis).
    fn track_current_slice(&mut self, image: &[u8]) {
        let u = |o: usize| image.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
        let (Some(index), Some(hash)) = (u(0xB8), u(0xBC)) else { return };
        let index = index & 0x3FF;
        if matches!(hash, 0 | 0x811C_9DC5) || index == 0x3FF || self.current_slice == Some(index) {
            return;
        }
        tracing::info!("⭐ client is now in slice set {index} (hash {hash:#010x}, was {:?})", self.current_slice);
        self.current_slice = Some(index);
    }

    /// Move the region record to the slice set the client targets (see [`Self::on_client_message`]).
    /// Returns true when it moved.
    fn follow_slice_target(&mut self, image: &[u8]) -> bool {
        const TARGET: usize = 0x158;
        // 0x80800C56[1] @0xB8 -> 0x80800D9F[2] @0x140, an 8-bit field.
        // Native 56578: this matches sstm+0x1D0, including a same-region retry.
        const TOKEN: usize = 0x1F8;
        let index = |o: usize| image.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()) & 0x3FF);
        let (Some(target), Some(&client_token)) = (index(TARGET), image.get(TOKEN)) else {
            return false;
        };
        // Do not act on an uninitialised baseline or a cleared target. Token zero is
        // reserved; the client skips it on wrap. Preserve the explicit no-PAH bisect.
        let target_hash = image.get(TARGET + 4..TARGET + 8).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
        if matches!(target_hash, None | Some(0) | Some(0x811C_9DC5)) || target == 0x3FF || client_token == 0 {
            return false;
        }
        match self.pah_region {
            Some((region, peer, token)) if (region, token) != (target, client_token) => {
                self.pah_region = Some((target, peer, client_token));
                self.membership_dirty = true;
                tracing::info!(
                    "⭐ client targets slice set {target}: region record {region} -> {target}, client token {token} -> {client_token}"
                );
                true
            }
            _ => false,
        }
    }

    /// Serve a start region (see [`ServedWorld::start_region`]); also moves the PAH region record.
    pub fn set_start_region(&mut self, region: u32) {
        self.start_region = Some((region, 0x811C_9DC5));
        if let Some((_, peer, token)) = self.pah_region {
            self.pah_region = Some((region, peer, token));
        }
    }

    pub fn set_pah_region(&mut self, region_id: u32, peer_index: usize, token: u8) {
        self.pah_region = Some((region_id, peer_index, token));
    }

    /// Override the cadence we advertise. Takes effect on the next `join_result`.
    pub fn set_keepalive_interval(&mut self, interval: Duration) {
        self.keepalive_interval = interval;
        self.lease = interval * Self::LEASE_INTERVALS;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instance() -> (ActivityInstance, Instant) {
        let now = Instant::now();
        let mut i = ActivityInstance::new(0xD1A0_0000_0000_0001, "d1ah-test", now);
        i.on_connect(now);
        (i, now)
    }

    fn join_request(nonce: [u8; 4]) -> ClientMessage {
        let mut img = vec![0u8; 0x100];
        img[..4].copy_from_slice(&nonce);
        ClientMessage::from_struct_image(3, &img).unwrap()
    }

    /// ⭐ The nonce echo. `0x29BD10` compares `join_result+0x00` against a value the client set
    /// itself; a derived or zeroed nonce logs *"Received a join response that we didn't request?"*
    /// and the join is dropped.
    #[test]
    fn the_join_result_echoes_the_requests_nonce_byte_for_byte() {
        let (mut i, now) = instance();
        let out = i.on_client_message(&join_request([0xAA, 0xBB, 0xCC, 0xDD]), now);
        let AhMessage::JoinResult(j) = &out[0].message else { panic!("expected a join result") };
        assert_eq!(j.nonce, [0xAA, 0xBB, 0xCC, 0xDD]);
        assert_eq!(j.result, 0, "0 = accepted");
        assert_eq!(j.ah_id, 0xD1A0_0000_0000_0001);
        assert_eq!(j.session_id, b"d1ah-test");
        assert_eq!(i.state(), InstanceState::Live, "all four of 298EF0's messages went out");
    }

    /// ⭐⭐ **The regression that run `20260808-234535-CODEC-LIVE2` measured.** That run pushed
    /// two messages - udt 4 and udt 1 - and the client sat in `EST-N AH->0 MEM-0` for 22 s and
    /// gave up with `error.starting_private_activity_instance_timed_out_waiting_for_join_response`.
    /// `FUN_00298EF0` wants **four**, and none of the missing two is ever *asked* for.
    #[test]
    fn a_join_pushes_all_four_of_the_messages_establish_client_waits_on_in_ladder_order() {
        let (mut i, now) = instance();
        let out = i.on_client_message(&join_request([1, 2, 3, 4]), now);
        let udts: Vec<u32> = out.iter().map(|p| p.message.user_defined_type()).collect();
        assert_eq!(
            udts,
            vec![4, 0, 1, 11],
            "join_result -> entity_slots_allocated -> global_activity_state -> \
             replicate_membership, the order FUN_00760470's status ladder waits in"
        );
        // udt 0 is the WHOLE pool, unsolicited: FUN_010579F0 has no 'was it requested' check and
        // ac+0x64AA is written nowhere else in the image.
        let AhMessage::EntitySlotsAllocated(m) = &out[1].message else { panic!() };
        assert_eq!(m.count(), 12_288, "a full grant, not a batch");
        assert!(i.published_membership());
        // Not re-sent on a second join.
        assert_eq!(i.on_client_message(&join_request([1, 2, 3, 4]), now).len(), 1);
    }

    /// The membership is an **echo**, and it is refused rather than improvised when there is
    /// nothing to echo. A zero identity would still set `ac+0x131 & 1` (`0x29C730` sets it
    /// unconditionally) and then fail `FUN_0029FE70`'s slot search silently - the worst outcome,
    /// because it looks like it worked.
    #[test]
    fn a_join_request_too_short_to_carry_an_identity_loses_the_membership_push_not_fakes_it() {
        let (mut i, now) = instance();
        let short = ClientMessage::from_struct_image(3, &[0u8; 8]).unwrap();
        let udts: Vec<u32> =
            i.on_client_message(&short, now).iter().map(|p| p.message.user_defined_type()).collect();
        assert_eq!(udts, vec![4, 0, 1], "three of four, and the fourth is absent not invented");
        assert!(!i.published_membership());
        assert_eq!(i.state(), InstanceState::Joined, "not Live - we did not answer completely");
    }

    /// The identity in the membership is the client's own 40 bytes, at the offset the descriptor
    /// says a member record's identity lives - not a struct we built.
    #[test]
    fn the_membership_echoes_the_clients_identity_verbatim() {
        let (mut i, now) = instance();
        let mut img = vec![0u8; 0x100];
        for (k, b) in img.iter_mut().enumerate().skip(0x10).take(0x28) {
            *b = 0x40 + k as u8;
        }
        let out = i.on_client_message(&ClientMessage::from_struct_image(3, &img).unwrap(), now);
        let AhMessage::ReplicateMembership(m) = &out[3].message else { panic!() };
        assert_eq!(m.member_identity(), &img[0x10..0x38]);
        assert_eq!(
            m.acknowledged_session_host(),
            &img[0x10..0x18],
            "the AH acknowledges the solo member's player identifier as the peer-session host"
        );
        assert_eq!(m.image.len(), ReplicateMembership::STRUCT_SIZE);
        assert_eq!(m.image[ReplicateMembership::OCCUPANCY_MASK], 1, "slot 0, and only slot 0");
    }

    /// The client has no default keepalive cadence - its constructor writes 0 to `ac+0x6C74` - so
    /// an Activity Host that sends 0 has told the client to talk continuously.
    #[test]
    fn the_join_result_never_advertises_a_zero_keepalive_interval() {
        let (mut i, now) = instance();
        let out = i.on_client_message(&join_request([0; 4]), now);
        let AhMessage::JoinResult(j) = &out[0].message else { panic!() };
        assert!(j.keepalive_interval_ms > 0);
        assert_eq!(j.keepalive_interval_ms, JoinResult::DEFAULT_KEEPALIVE_MS);
    }

    #[test]
    fn the_activity_state_is_published_exactly_once_and_it_is_1712_bytes() {
        let (mut i, now) = instance();
        let first = i.on_client_message(&join_request([0; 4]), now);
        assert_eq!(first.len(), 4, "join_result, entity grant, activity state, membership");
        let AhMessage::GlobalActivityState(s) = &first[2].message else { panic!() };
        assert_eq!(s.blob.len(), 0x6B0);
        // A second join does not re-publish any of the three unsolicited ones.
        let second = i.on_client_message(&join_request([0; 4]), now);
        assert_eq!(second.len(), 1);
    }

    /// ⚠ On a **pre-join** instance. Once the full pool has gone out unsolicited there is nothing
    /// left to allocate, and that is the correct answer - asserting the batch path on an exhausted
    /// allocator would be measuring the grant, not the allocator.
    #[test]
    fn allocating_entity_indices_grants_exactly_the_requested_count_in_a_0x600_mask() {
        let (mut i, now) = instance();
        let out = i.on_client_message(
            &ClientMessage::from_struct_image(19, &12u32.to_le_bytes()).unwrap(),
            now,
        );
        let AhMessage::EntitySlotsAllocated(m) = &out[0].message else { panic!() };
        assert_eq!(m.count(), 12);
        assert_eq!(m.to_struct_bytes().len(), 0x600);
        assert_eq!(i.entities().granted().count(), 12);
        // ⭐ And entity indices alone do NOT make an instance Live any more: `Live` means all four
        // of `FUN_00298EF0`'s messages went out, which only the join path does.
        assert_eq!(i.state(), InstanceState::Connected);
    }

    /// The unsolicited grant is the whole pool, and a later request honestly returns nothing.
    #[test]
    fn the_unsolicited_grant_is_the_whole_pool_and_leaves_nothing_to_allocate() {
        let (mut i, now) = instance();
        i.on_client_message(&join_request([0; 4]), now);
        assert_eq!(i.entities().granted().count(), 12_288);
        let out = i.on_client_message(
            &ClientMessage::from_struct_image(19, &12u32.to_le_bytes()).unwrap(),
            now,
        );
        let AhMessage::EntitySlotsAllocated(m) = &out[0].message else { panic!() };
        assert_eq!(m.count(), 0, "the client already holds all 12,288");
    }

    /// ⭐ Activity follow: the join publishes the index the followed hash names, re-read per join,
    /// and an unknown hash (or no file) keeps the configured index.
    #[test]
    fn a_join_publishes_the_launched_activity_when_following() {
        let dir = std::env::temp_dir().join(format!("d1ah-follow-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("launched.txt");
        let table: std::collections::HashMap<u32, u16> =
            [(0xFB5D_13A3u32, 410u16), (0x6D71_4C3A, 416)].into_iter().collect();

        let (mut i, now) = instance();
        i.set_activity_index(416);
        i.set_activity_follow(file.clone(), table.clone());
        let _ = std::fs::remove_file(&file);
        i.on_client_message(&join_request([0; 4]), now);
        assert_eq!(i.activity_index(), 416, "no file: the configured index stands");

        std::fs::write(&file, format!("{}\n", 0xFB5D_13A3u32)).unwrap();
        let (mut j, now) = instance();
        j.set_activity_index(416);
        j.set_activity_follow(file.clone(), table.clone());
        j.on_client_message(&join_request([0; 4]), now);
        assert_eq!(j.activity_index(), 410, "the launched Skirmish hash names index 410");

        std::fs::write(&file, "12345\n").unwrap();
        let (mut k, now) = instance();
        k.set_activity_index(416);
        k.set_activity_follow(file.clone(), table);
        k.on_client_message(&join_request([0; 4]), now);
        assert_eq!(k.activity_index(), 416, "an unknown hash keeps the configured index");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// ⭐ A world served with a start region publishes it at blob `+0x54` and moves the PAH region
    /// record with it; a world without one leaves region 0.
    #[test]
    fn a_world_with_a_start_region_publishes_it() {
        let (mut i, _) = instance();
        i.set_pah_region(0, 0, 1);
        let AhMessage::GlobalActivityState(s) = &i.publish_activity_state().message else { panic!() };
        assert_eq!(s.start_region(), 0);
        i.set_start_region(8);
        let AhMessage::GlobalActivityState(s) = &i.publish_activity_state().message else { panic!() };
        assert_eq!(s.start_region(), 8, "bubble 8 `dome_a`");
        assert_eq!(i.pah_region(), Some((8, 0, 1)), "the PAH record names the same region");
    }

    /// ⭐ Mission steps fire in order on the sensor clock, and a region step moves the PAH record.
    #[test]
    fn mission_steps_switch_the_served_state_and_region() {
        let (mut i, now) = instance();
        i.set_pah_region(113, 0, 1);
        let step = |at: u64, b: u8, region| MissionStep {
            when: StepWhen::At(Duration::from_secs(at)),
            team0: vec![b],
            team_minus1: vec![b],
            region,
            incident: None,
        };
        let mut steps = std::collections::HashMap::new();
        steps.insert(i.activity_index(), vec![step(0, 1, None), step(70, 2, None), step(120, 3, Some(112))]);
        i.set_mission_steps(steps);
        assert!(i.advance_mission(now));
        assert_eq!(i.sensor_payload.as_deref(), Some(&[1u8][..]));
        assert!(!i.advance_mission(now + Duration::from_secs(10)), "no new step yet");
        assert!(i.advance_mission(now + Duration::from_secs(71)));
        assert_eq!(i.sensor_payload.as_deref(), Some(&[2u8][..]));
        assert!(i.advance_mission(now + Duration::from_secs(130)));
        assert_eq!(i.sensor_payload.as_deref(), Some(&[3u8][..]));
        assert_eq!(i.pah_region(), Some((112, 0, 2)), "the teleport step moves the region");
    }

    /// ⭐ A client walking into another bubble gets the region record for its target, once.
    #[test]
    fn the_region_record_follows_the_clients_slice_target() {
        let (mut i, now) = instance();
        i.set_pah_region(112, 0, 2);
        let mut img = vec![0u8; 0x220];
        img[0xB8..0xBC].copy_from_slice(&112u32.to_le_bytes());
        img[0x158..0x15C].copy_from_slice(&24u32.to_le_bytes());
        let stray = ClientMessage::ClientAuthoritativeDataUpdate { struct_image: img.clone() };
        i.on_client_message(&stray, now);
        assert_eq!(i.pah_region(), Some((112, 0, 2)), "a target without its bubble hash is a delta artefact");
        img[0x15C..0x160].copy_from_slice(&0x4415_15C2u32.to_le_bytes());
        let zero = ClientMessage::ClientAuthoritativeDataUpdate { struct_image: img.clone() };
        i.on_client_message(&zero, now);
        assert_eq!(i.pah_region(), Some((112, 0, 2)), "token 0 is reserved: no transition yet");
        img[0x1F8] = 3;
        let msg = ClientMessage::ClientAuthoritativeDataUpdate { struct_image: img.clone() };
        i.on_client_message(&msg, now);
        assert_eq!(i.pah_region(), Some((24, 0, 3)), "target 24 with the client's token");
        i.on_client_message(&msg, now);
        assert_eq!(i.pah_region(), Some((24, 0, 3)), "the same target and token change nothing");
        // A retried transition to the same target advances the client's token (route6: 96/5 while
        // the host had inferred 4); the record must follow it.
        img[0x1F8] = 4;
        i.on_client_message(&ClientMessage::ClientAuthoritativeDataUpdate { struct_image: img.clone() }, now);
        assert_eq!(i.pah_region(), Some((24, 0, 4)), "a same-target retry follows the client's token");
    }

    /// ⭐ A changed membership carries the next update number, or the client never re-reads the
    /// region record (`0x778D60` skips an update number it already processed).
    #[test]
    fn a_changed_membership_advances_the_update_number() {
        let (mut i, now) = instance();
        let out = i.on_client_message(&join_request([1, 2, 3, 4]), now);
        let number = |m: &ReplicateMembership| u32::from_le_bytes(m.image[..4].try_into().unwrap());
        let joined = out
            .iter()
            .find_map(|p| match &p.message { AhMessage::ReplicateMembership(m) => Some(number(m)), _ => None })
            .expect("the join publishes a membership");
        assert_eq!(joined, 1);
        i.set_pah_region(112, 0, 2);
        let mut img = vec![0u8; 0x220];
        img[0xB8..0xBC].copy_from_slice(&112u32.to_le_bytes());
        img[0x158..0x15C].copy_from_slice(&24u32.to_le_bytes());
        img[0x15C..0x160].copy_from_slice(&0x4415_15C2u32.to_le_bytes());
        img[0x1F8] = 3;
        i.on_client_message(&ClientMessage::ClientAuthoritativeDataUpdate { struct_image: img }, now);
        let AhMessage::ReplicateMembership(m) = i.membership_due(now).expect("re-published").message else {
            panic!("expected a membership")
        };
        assert_eq!(number(&m), 2, "the next update number");
        assert_eq!(m.pah_region_id(), 24);
    }

    /// ⭐ A host incident goes out as udt 18 with its type and body in place.
    #[test]
    fn a_host_incident_is_a_udt_18_image() {
        let img = incident_image(1504, &[0xAA, 0xBB]);
        assert_eq!(img.len(), 0x2CC);
        assert_eq!(&img[0..8], &[1, 0, 0, 0, 0xE0, 0x05, 0, 0]);
        assert_eq!(&img[0xD4..0xDA], &[2, 0, 0, 0, 0xAA, 0xBB]);
    }

    /// ⭐ The trigger name in a 3403 report, from the run agr-breach13 incident (trigger 34).
    #[test]
    fn a_trigger_report_names_its_trigger() {
        let mut img = vec![0u8; 0x2CC];
        let body = hex_bytes("1009a7015a200000000000403a2000000000004021e8d0000000000000a16d3f4906599a3d1b325768e27a008a04727714");
        img[0xD8..0xD8 + body.len()].copy_from_slice(&body);
        // Bytes +0xD8.. of the decoded struct; the name sits at body bit 303.
        assert_eq!(trigger_report(&img), Some((0x992B_B471, 0x1D, 34)));
        assert_eq!(
            StepWhen::parse("^0x992bb471/0x1d/34|900"),
            Some(StepWhen::OnTrigger { group: 0x992B_B471, kind: 0x1D, index: 34, timeout: Duration::from_secs(900) })
        );
    }

    fn hex_bytes(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|k| u8::from_str_radix(&s[k..k + 2], 16).unwrap()).collect()
    }

    /// ⭐ A slice step fires when the client reports that slice set as current.
    #[test]
    fn a_slice_step_fires_when_the_client_is_in_that_slice_set() {
        assert_eq!(StepWhen::parse("%24|900"), Some(StepWhen::OnSlice { slice: 24, timeout: Duration::from_secs(900) }));
        let (mut i, now) = instance();
        let mut img = vec![0u8; 0x220];
        img[0xB8..0xBC].copy_from_slice(&24u32.to_le_bytes());
        i.on_client_message(&ClientMessage::ClientAuthoritativeDataUpdate { struct_image: img.clone() }, now);
        assert_eq!(i.current_slice, None, "a current record without its hash is a delta artefact");
        img[0xBC..0xC0].copy_from_slice(&0x4415_15C2u32.to_le_bytes());
        i.on_client_message(&ClientMessage::ClientAuthoritativeDataUpdate { struct_image: img }, now);
        assert_eq!(i.current_slice, Some(24));
    }

    /// ⭐ An incident step fires on the client's report (not on its own echo), else on the timeout.
    #[test]
    fn an_incident_step_fires_on_the_report_or_the_timeout() {
        assert_eq!(
            StepWhen::parse("@891,1726|70"),
            Some(StepWhen::OnIncident { incidents: [891, 1726, u32::MAX, u32::MAX], timeout: Duration::from_secs(70) })
        );
        assert_eq!(StepWhen::parse("+5"), Some(StepWhen::AfterPrevious(Duration::from_secs(5))));
        assert_eq!(StepWhen::parse("120"), Some(StepWhen::At(Duration::from_secs(120))));
        let (mut i, now) = instance();
        let step = |when, b: u8| MissionStep { when, team0: vec![b], team_minus1: vec![b], region: None, incident: None };
        let on891 = StepWhen::parse("@891|70").unwrap();
        let mut steps = std::collections::HashMap::new();
        steps.insert(i.activity_index(), vec![step(StepWhen::At(Duration::ZERO), 1), step(on891, 2), step(on891, 3)]);
        i.set_mission_steps(steps);
        assert!(i.advance_mission(now));
        let incident = |t: u32| {
            let mut img = vec![0u8; 0x2CC];
            img[0..4].copy_from_slice(&1u32.to_le_bytes());
            img[4..8].copy_from_slice(&t.to_le_bytes());
            ClientMessage::Other { user_defined_type: 18, struct_image: img }
        };
        i.on_client_message(&incident(891), now + Duration::from_millis(500));
        assert!(!i.advance_mission(now + Duration::from_secs(1)), "an echo of the step does not count");
        i.on_client_message(&incident(891), now + Duration::from_secs(20));
        assert!(i.advance_mission(now + Duration::from_secs(20)), "the skip/end report fires step 1");
        assert_eq!(i.sensor_payload.as_deref(), Some(&[2u8][..]));
        assert!(!i.advance_mission(now + Duration::from_secs(30)));
        assert!(i.advance_mission(now + Duration::from_secs(91)), "no report: the timeout fires step 2");
        assert_eq!(i.sensor_payload.as_deref(), Some(&[3u8][..]));
    }

    /// ⭐ `!G|N` fires once every squad of G reported alive is down - not on a squad never seen
    /// alive, and not while one is still up (run sense35: Divide squad 17 went 1 -> 0 on a kill).
    #[test]
    fn a_squads_cleared_step_waits_for_every_squad_seen_alive() {
        use crate::sense::SquadCount;
        assert_eq!(StepWhen::parse("!0x9243dffb|600"),
            Some(StepWhen::OnSquadsCleared { group: 0x9243_DFFB, timeout: Duration::from_secs(600) }));
        let (mut i, now) = instance();
        let step = |when, b: u8| MissionStep { when, team0: vec![b], team_minus1: vec![b], region: None, incident: None };
        let mut steps = std::collections::HashMap::new();
        steps.insert(i.activity_index(), vec![step(StepWhen::At(Duration::ZERO), 1), step(StepWhen::parse("!0x9243dffb|600").unwrap(), 2)]);
        i.set_mission_steps(steps);
        assert!(i.advance_mission(now));
        let squad = |index, live| SquadCount { group: 0x9243_DFFB, kind: 1, index, live };
        assert!(!i.advance_mission(now + Duration::from_secs(1)), "nothing reported alive yet");
        i.on_squad_counts(&[squad(17, 1), squad(18, 1), SquadCount { group: 0x1234, kind: 1, index: 4, live: 3 }]);
        i.on_squad_counts(&[squad(17, 0)]);
        assert!(!i.advance_mission(now + Duration::from_secs(2)), "squad 18 is still up");
        i.on_squad_counts(&[squad(20, 0), squad(18, 0)]);
        assert!(i.advance_mission(now + Duration::from_secs(3)), "17 and 18 are down; 20 was never seen alive");
        assert_eq!(i.sensor_payload.as_deref(), Some(&[2u8][..]));
    }

    /// ⭐ A teleport step asks (state 1), finishes (state 3) and drops the group slice-set request,
    /// re-publishing the membership each time.
    #[test]
    fn a_teleport_step_runs_the_slice_request_handshake() {
        let (mut i, now) = instance();
        i.set_pah_region(113, 0, 1);
        i.on_client_message(&join_request([0; 4]), now);
        let mut steps = std::collections::HashMap::new();
        steps.insert(
            i.activity_index(),
            vec![MissionStep { when: StepWhen::At(Duration::ZERO), team0: vec![1], team_minus1: vec![1], region: Some(112), incident: None }],
        );
        i.set_mission_steps(steps);
        assert!(i.advance_mission(now));
        let req = |p: Option<AhPush>| match p.map(|p| p.message) {
            Some(AhMessage::ReplicateMembership(m)) => m.slice_request(),
            _ => None,
        };
        assert_eq!(req(i.membership_due(now)), Some((1, 112)), "state 1 asks for 112");
        assert!(i.membership_due(now + Duration::from_secs(5)).is_none(), "nothing new yet");
        assert_eq!(req(i.membership_due(now + Duration::from_secs(21))), Some((3, 112)), "state 3 finishes");
        assert_eq!(req(i.membership_due(now + Duration::from_secs(31))), Some((0, 0)), "then the request is dropped");
        assert_eq!(i.pah_region(), Some((112, 0, 2)), "the teleport is the second transition");
    }

    /// ⭐⭐ A state refresh must not re-grant indices the client already holds: its apply ORs the
    /// mask into the free-slot pool, so a re-grant frees live slots and the next allocation reuses
    /// them (teardown crash at `0xBC09AE`, run 2026-10-14-slots).
    #[test]
    fn a_state_refresh_re_grants_no_entity_indices() {
        let (mut i, now) = instance();
        i.on_client_message(&join_request([0; 4]), now);
        assert_eq!(i.entities().granted().count(), 12_288);
        let out = i.on_client_message(&ClientMessage::from_struct_image(17, &[0u8; 8]).unwrap(), now);
        let AhMessage::EntitySlotsAllocated(m) = &out[0].message else { panic!() };
        assert_eq!(m.count(), 0, "the refresh carries an empty mask");
        assert_eq!(m.to_struct_bytes().len(), 0x600, "but still a full 0x600 struct");
        assert_eq!(i.entities().granted().count(), 12_288, "and the ledger is unchanged");
    }

    /// ⭐ The named regression from `protocol.rs`, at the level that would actually have committed
    /// the mistake: a stub that "acks" the lease.
    #[test]
    fn a_keepalive_produces_no_reply_at_all() {
        let (mut i, now) = instance();
        let out = i.on_client_message(&ClientMessage::Keepalive, now);
        assert!(out.is_empty(), "udt 16 has no client handler - the reply is not evicting");
        assert_eq!(i.keepalives, 1);
        // Positive control on the same call shape: a join *does* produce output.
        assert!(!i.on_client_message(&join_request([0; 4]), now).is_empty());
    }

    #[test]
    fn the_lease_lapses_after_five_intervals_of_silence_and_only_reports_once() {
        let (mut i, now) = instance();
        i.on_client_message(&join_request([0; 4]), now);
        let interval = Duration::from_millis(u64::from(JoinResult::DEFAULT_KEEPALIVE_MS));
        assert_eq!(i.tick(now + interval * 4), None, "still inside the lease");
        assert_eq!(i.tick(now + interval * 5), Some(EvictionReason::InactivityTimeout));
        assert_eq!(i.tick(now + interval * 50), None, "an eviction is reported once");
        assert!(matches!(i.state(), InstanceState::Evicted { .. }));
    }

    #[test]
    fn a_keepalive_renews_the_lease() {
        let (mut i, now) = instance();
        i.on_client_message(&join_request([0; 4]), now);
        let interval = Duration::from_millis(u64::from(JoinResult::DEFAULT_KEEPALIVE_MS));
        i.on_client_message(&ClientMessage::Keepalive, now + interval * 4);
        assert_eq!(i.tick(now + interval * 8), None);
        assert_eq!(i.tick(now + interval * 9), Some(EvictionReason::InactivityTimeout));
    }

    #[test]
    fn an_unconnected_instance_is_never_evicted_for_silence() {
        let now = Instant::now();
        let mut i = ActivityInstance::new(1, "s", now);
        assert_eq!(i.tick(now + Duration::from_secs(3600)), None);
        assert_eq!(i.state(), InstanceState::Allocated);
    }

    #[test]
    fn the_replication_epoch_is_a_u8_and_the_push_is_two_bytes() {
        let (mut i, _) = instance();
        let p = i.advance_replication_epoch(false);
        assert_eq!(i.replication_epoch(), 1);
        assert_eq!(p.message.try_struct_bytes().unwrap(), vec![1, 0]);
        for _ in 0..255 {
            i.advance_replication_epoch(false);
        }
        assert_eq!(i.replication_epoch(), 0, "a u8 wraps; it is not widened");
    }

    #[test]
    fn a_connectivity_failure_evicts_with_the_clients_own_reason_name() {
        let (mut i, now) = instance();
        i.on_client_message(&join_request([0; 4]), now);
        i.on_client_message(
            &ClientMessage::from_struct_image(35, &[0u8; 8]).unwrap(),
            now,
        );
        assert_eq!(
            i.state(),
            InstanceState::Evicted { reason: EvictionReason::ConnectivityFailure }
        );
    }
}
