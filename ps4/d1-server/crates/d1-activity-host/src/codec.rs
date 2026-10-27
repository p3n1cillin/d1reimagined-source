// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! ⭐⭐ **THE GATE THIS MODULE NAMED IS OPEN.** 2026-08-08.
//!
//! # The retraction, at the top, in place
//!
//! This module used to say:
//!
//! > *"`SchemaCodec` - the honest one, and the default everywhere in this crate - returns
//! > `AhError::SchemaCodecMissing` naming the exact descriptor global to go and read."*
//!
//! It was right to refuse, and the refusal is what kept the crate honest through the run that
//! falsified [`RawStructCodec`]. **It is no longer true.** The schema codec is implemented, and it
//! is implemented by *running the client's own codec* under Unicorn
//! ([`d1_oracle::activity_host`]) rather than by reimplementing a bit grammar.
//!
//! Two things closed it, in this order:
//!
//! 1. **`FUN_002A4090` is the QueueZ object codec.** Its whole body is
//!    `bs[0x14]=1; BS_CTOR(bs, wire, len); BS_BEGIN_READ(bs); FUN_002C9000(tag, bs, out, out, 1);
//!    BS_END(bs); return rc && bits <= len*8` - argument for argument the decode `d1-oracle` has
//!    driven since the roster work. Nothing new had to be written; only *which tag* was missing.
//! 2. **The tags are statically derivable.** `DAT_019022F0` &co. point at BSS records filled at
//!    runtime, but each record has exactly one *other* relocation aiming at it - its entry in a
//!    sorted checksum table - and that checksum occurs exactly once more in the image, inside the
//!    **static** schema record whose `+0x50` is the descriptor anchor. Five steps, all static, all
//!    in [`d1_oracle::activity_host::derive`]. 27 messages, and every one of the 26 that states a
//!    struct size at its call site agrees with the descriptor's.
//!
//! ⇒ `join_request` is tag **`0x80800E98`** and `join_result` is tag **`0x80800A58`**.
//!
//! # The control that settles it, and it is not ours
//!
//! On 2026-08-08 the live client sent a real 238-byte `join_request` to this listener and printed
//! what it had put in it. Decoding those bytes with `0x80800E98` reproduces the nonce
//! `0x4DB6999F`, the AH id, both SOIDs, the machine and player ids and the session string; and
//! **re-encoding the decoded struct reproduces the 238 bytes byte for byte.** Neither half of that
//! was produced by this project. Run it with:
//!
//! ```text
//! d1-oracle activity-host controls        # exit 2 if any of the 15 fails
//! ```
//!
//! # What still refuses, and why that is the point
//!
//! [`SchemaCodec`] refuses any `user_defined_type` whose descriptor global we have **not** read out
//! of a call site - [`d1_oracle::activity_host::CALL_SITES`] has 27 of the 49. That refusal is the
//! same refusal as before, narrowed to where it is still true. **An unpinned udt must refuse, not
//! improvise**: a tag guessed from a struct-size match is exactly the "one instance ⇒ a category"
//! trap, and there are ten descriptors of size `0x100` alone.
//!
//! # ⚠ This crate now links Unicorn, through `d1-oracle`
//!
//! `d1-oracle`'s own module doc says the GPL-2.0 dependency is "confined to one crate boundary on
//! purpose". That confinement is now two crates wide. The licence maths is unchanged - the whole
//! workspace is GPL-2.0-**or-later** and GPL-2.0 is compatible with it - but the practical costs
//! are real and are stated here rather than discovered:
//!
//! * The **pinned eboot must exist at runtime** (`$D1_EBOOT`, else
//!   `dumps/patch-01.29-CUSA00219/eboot.bin`). Without it every encode returns
//!   [`AhError::SchemaCodec`] naming the missing image - a named gate, not a silent empty push.
//! * A Unicorn instance is **not `Send`**, so it lives on one dedicated thread ([`OracleWorker`])
//!   that this module talks to over a channel. Roughly 200 ms for the first message of a udt (image
//!   map + the pinned positive control) and well under a millisecond thereafter. A join happens
//!   once per activity; this is not a hot path.

use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, OnceLock};

use crate::messages::AhMessage;
use crate::AhError;

/// The descriptor globals the client's own call sites name **in 01.29** (the reference build).
/// [`descriptor_global`] follows the selected build; 56578's rows are
/// [`d1_oracle::activity_host::CALL_SITES_56578`].
pub const DESCRIPTOR_GLOBAL_JOIN_REQUEST: u64 = 0x0190_22F0;
/// See [`DESCRIPTOR_GLOBAL_JOIN_REQUEST`].
pub const DESCRIPTOR_GLOBAL_JOIN_RESULT: u64 = 0x0190_22F8;
/// See [`DESCRIPTOR_GLOBAL_JOIN_REQUEST`].
pub const DESCRIPTOR_GLOBAL_ENTITY_SLOTS: u64 = 0x0190_2310;
/// See [`DESCRIPTOR_GLOBAL_JOIN_REQUEST`].
pub const DESCRIPTOR_GLOBAL_GLOBAL_STATE: u64 = 0x0190_2380;
/// The client-side decoder every AH receive handler funnels through (01.29; the oracle calls the
/// selected build's, see [`d1_oracle::codec`]).
pub const SCHEMA_DECODE_FN: u64 = d1_oracle::activity_host::DECODE_FN;
/// The client-side encoder `activity_client_send_message` funnels through.
pub const SCHEMA_ENCODE_FN: u64 = d1_oracle::activity_host::ENCODE_FN;

/// ⭐ `join_request` - proven by executing the client's own codec on 238 captured bytes.
pub const TAG_JOIN_REQUEST: u32 = 0x8080_0E98;
/// ⭐ `join_result` - its descriptor's field `[0]` **is** [`TAG_JOIN_REQUEST`] at `+0x00`.
pub const TAG_JOIN_RESULT: u32 = 0x8080_0A58;

/// Which descriptor global a udt is decoded through, where we have read the call site.
pub fn descriptor_global(user_defined_type: u32) -> u64 {
    d1_oracle::activity_host::call_sites()
        .iter()
        .find(|(u, _, _)| *u == user_defined_type)
        .map(|(_, g, _)| *g)
        .unwrap_or(0)
}

/// How an AH message struct becomes the bytes inside a BAP 8/100 payload, and back.
///
/// Implementations are deliberately swappable so that "we do not know the wire encoding" is a
/// value in the program rather than a comment in a document. That value is now `false` for the 27
/// messages in [`d1_oracle::activity_host::CALL_SITES`] and still `true` for the other 22.
pub trait AhWireCodec: Send + Sync {
    /// A short name for the run log. It appears in every line the listener writes.
    fn name(&self) -> &'static str;

    /// Serialise for the wire.
    fn encode(&self, msg: &AhMessage) -> Result<Vec<u8>, AhError>;

    /// Deserialise from the wire into the client's struct image.
    fn decode(&self, user_defined_type: u32, wire: &[u8]) -> Result<Vec<u8>, AhError>;

    /// Apply a replicated update to this connection's previous image. Snapshot-only
    /// diagnostic codecs may keep their normal decode behaviour; SchemaCodec uses
    /// the client's replicated decoder and its presence bits.
    fn decode_seeded(&self, user_defined_type: u32, wire: &[u8], _seed: &[u8]) -> Result<Vec<u8>, AhError> {
        self.decode(user_defined_type, wire)
    }
}

// ── the oracle worker ────────────────────────────────────────────────────────────────────────────

enum Job {
    Encode { udt: u32, image: Vec<u8>, reply: Sender<Result<Vec<u8>, String>> },
    Decode { udt: u32, wire: Vec<u8>, seed: Vec<u8>, reply: Sender<Result<Vec<u8>, String>> },
    StructSize { udt: u32, reply: Sender<Result<Vec<u8>, String>> },
    /// ⭐⭐ Build the bespoke udt-5 `sensor_auth_update` frame. Runs on the oracle thread because
    /// [`d1_oracle::sensor`] uses `ObjectOracle` (Unicorn, `!Send`).
    BuildSensor {
        params: d1_oracle::sensor::SensorPhaseUpdate,
        reply: Sender<Result<Vec<u8>, String>>,
    },
    /// ⭐ The oracle's bump-arena accounting, per udt. See [`OracleWorker::arena_stats`].
    Arena { reply: Sender<Vec<ArenaStats>> },
}

pub use d1_oracle::activity_host::ArenaStats;

/// One thread owning one [`d1_oracle::activity_host::AhCodec`].
///
/// ⚠ A Unicorn instance is `!Send`, so it may never leave this thread. The channel is the only way
/// in. `Mutex<Sender<_>>` is what makes the handle `Sync`; contention is irrelevant because the
/// whole protocol is one join and a handful of grants.
pub struct OracleWorker {
    tx: Mutex<Sender<Job>>,
}

impl OracleWorker {
    fn spawn() -> Self {
        let (tx, rx) = channel::<Job>();
        std::thread::Builder::new()
            .name("d1-ah-oracle".into())
            .spawn(move || {
                let mut codec = d1_oracle::activity_host::AhCodec::new();
                while let Ok(job) = rx.recv() {
                    match job {
                        Job::Encode { udt, image, reply } => {
                            let r = codec
                                .encode(udt, &image)
                                .map(|e| e.payload)
                                .map_err(|e| e.to_string());
                            let _ = reply.send(r);
                        }
                        Job::Decode { udt, wire, seed, reply } => {
                            let r = codec
                                .decode_seeded(udt, &wire, &seed)
                                .map(|d| d.object)
                                .map_err(|e| e.to_string());
                            let _ = reply.send(r);
                        }
                        Job::StructSize { udt, reply } => {
                            let r = codec
                                .struct_size(udt)
                                .map(|n| (n as u64).to_le_bytes().to_vec())
                                .map_err(|e| e.to_string());
                            let _ = reply.send(r);
                        }
                        Job::BuildSensor { params, reply } => {
                            let r = params.encode().map_err(|e| e.to_string());
                            let _ = reply.send(r);
                        }
                        Job::Arena { reply } => {
                            let _ = reply.send(codec.arena_stats());
                        }
                    }
                }
            })
            .expect("spawning the AH oracle thread");
        Self { tx: Mutex::new(tx) }
    }

    fn call(&self, make: impl FnOnce(Sender<Result<Vec<u8>, String>>) -> Job) -> Result<Vec<u8>, String> {
        let (rtx, rrx) = channel();
        self.tx
            .lock()
            .map_err(|_| "the AH oracle thread panicked and poisoned its channel".to_string())?
            .send(make(rtx))
            .map_err(|_| "the AH oracle thread is gone".to_string())?;
        rrx.recv().map_err(|_| "the AH oracle thread died mid-call".to_string())?
    }

    /// ⭐⭐ **What the oracle's bump arena is doing right now**, per udt.
    ///
    /// The number that matters is `high_water`: the Unicorn arena is a bump pointer whose `free` is
    /// a no-op, so before 2026-08-09 every encode leaked its whole working set and the 132nd
    /// `global_activity_state` push died of `scratch heap exhausted` - thirteen minutes into run
    /// `20260809-120451-MOONBASE-FRESH`, with `error.private_activity_host_host_crashed` as the
    /// only observable. A flat high-water mark is what says that cannot happen again.
    ///
    /// Returns an empty vector if the worker thread is gone, which is itself a signal.
    pub fn arena_stats(&self) -> Vec<ArenaStats> {
        let (rtx, rrx) = channel();
        let Ok(tx) = self.tx.lock() else { return Vec::new() };
        if tx.send(Job::Arena { reply: rtx }).is_err() {
            return Vec::new();
        }
        drop(tx);
        rrx.recv().unwrap_or_default()
    }

    /// The struct size the client's own decoder allocates for this udt.
    pub fn struct_size(&self, udt: u32) -> Result<usize, String> {
        let b = self.call(|reply| Job::StructSize { udt, reply })?;
        Ok(u64::from_le_bytes(b[..8].try_into().unwrap()) as usize)
    }
}

/// The process-wide worker, started on first use.
pub fn oracle() -> &'static OracleWorker {
    static W: OnceLock<OracleWorker> = OnceLock::new();
    W.get_or_init(OracleWorker::spawn)
}

// ── the codecs ───────────────────────────────────────────────────────────────────────────────────

/// ⭐ **The real codec.** It runs `FUN_002A4090` / `FUN_002A4140` - the client's own functions -
/// through [`d1_oracle::activity_host`].
///
/// It is a unit struct on purpose: every call site in this crate says `Arc::new(SchemaCodec)` and
/// the emulator lives in a process-global worker, so nothing has to thread a handle around.
#[derive(Debug, Default, Clone, Copy)]
pub struct SchemaCodec;

impl SchemaCodec {
    /// The udts a stub Activity Host actually uses. Everything on the path from `join_request` to
    /// `establish client → ok`.
    /// ⭐ udt 11 joined the list on 2026-08-09: it is the fourth message `FUN_00298EF0` waits on
    /// and it is now schema-coded (tag `0x808009ED`, struct `0x5038`) through the *replicated*
    /// wrapper `FUN_002C9050`/`FUN_002C9290`. Its oracle is the most expensive of the five, so
    /// leaving it lazy would put the whole warm-up inside the join handler.
    pub const HOT_PATH: [u32; 5] = [3, 4, 1, 0, 11];

    /// ⭐ Build the oracles **before** the client connects, and say so in the run log.
    ///
    /// Two reasons, both learnt by measurement rather than assumed:
    ///
    /// 1. **Latency.** The first use of a udt costs ~2 s - mapping a 29 MB image, applying 70,682
    ///    relocations, building the descriptor index and running the pinned object control. Doing
    ///    four of those *inside* the join handler would put ~4 s between `join_request` and
    ///    `join_result`, in a rung whose failure mode is literally a timeout.
    /// 2. **Fail loudly, at the right time.** With no pinned eboot the codec cannot run at all.
    ///    Discovering that at startup is a refused launch; discovering it at the join is a run
    ///    spent, and a log that looks like the client never got there.
    ///
    /// Returns one `(udt, result)` per entry so the caller can log each.
    pub fn prewarm(udts: &[u32]) -> Vec<(u32, Result<usize, AhError>)> {
        udts.iter().map(|u| (*u, Self::struct_size(*u))).collect()
    }

    /// ⭐ The oracle's bump-arena accounting. See [`OracleWorker::arena_stats`].
    pub fn arena_stats() -> Vec<ArenaStats> {
        oracle().arena_stats()
    }

    /// ⭐⭐ Build the udt-5 `sensor_auth_update` wire frame ONCE, on the oracle thread. The result
    /// is finished bytes to carry in [`AhMessage::SensorAuthUpdate`] and send verbatim - so the
    /// ~4 s of `ObjectOracle` setup never lands on the heartbeat or the join path. Call it at
    /// instance construction, like [`Self::prewarm`].
    pub fn build_sensor(params: &d1_oracle::sensor::SensorPhaseUpdate) -> Result<Vec<u8>, AhError> {
        oracle()
            .call(|reply| Job::BuildSensor { params: params.clone(), reply })
            .map_err(|detail| AhError::SchemaCodec {
                user_defined_type: crate::protocol::UDT_SENSOR_AUTH_UPDATE,
                detail,
            })
    }

    /// The struct size the schema declares, for callers that must build an exact image.
    pub fn struct_size(user_defined_type: u32) -> Result<usize, AhError> {
        oracle().struct_size(user_defined_type).map_err(|detail| {
            SchemaCodec::classify(user_defined_type, detail)
        })
    }

    /// Turn the oracle's string into the right [`AhError`]. An **unpinned tag** keeps the old
    /// `SchemaCodecMissing` shape - that error is still exactly true for 22 of the 49 ids - and
    /// everything else is a codec failure with the oracle's own wording preserved.
    fn classify(user_defined_type: u32, detail: String) -> AhError {
        if detail.contains("UNPINNED") {
            AhError::SchemaCodecMissing {
                user_defined_type,
                descriptor_global: descriptor_global(user_defined_type),
            }
        } else {
            AhError::SchemaCodec { user_defined_type, detail }
        }
    }
}

impl AhWireCodec for SchemaCodec {
    fn name(&self) -> &'static str {
        "schema - the CLIENT'S OWN codec (FUN_002A4090/FUN_002A4140) via d1-oracle"
    }

    fn encode(&self, msg: &AhMessage) -> Result<Vec<u8>, AhError> {
        let udt = msg.user_defined_type();
        crate::protocol::check_sendable(udt)?;
        // ⭐⭐ `sensor_auth_update` is a bespoke replicated frame, not a single-descriptor struct,
        // so it does NOT go through `FUN_002A4140`. It is pre-built once by [`Self::build_sensor`]
        // (off the send path) and carried as finished wire bytes; here they go out verbatim.
        if let AhMessage::SensorAuthUpdate { payload } = msg {
            return Ok(payload.clone());
        }
        let image = msg.try_struct_bytes()?;
        oracle()
            .call(|reply| Job::Encode { udt, image, reply })
            .map_err(|d| SchemaCodec::classify(udt, d))
    }

    fn decode(&self, user_defined_type: u32, wire: &[u8]) -> Result<Vec<u8>, AhError> {
        self.decode_seeded(user_defined_type, wire, &[])
    }

    fn decode_seeded(&self, user_defined_type: u32, wire: &[u8], seed: &[u8]) -> Result<Vec<u8>, AhError> {
        crate::protocol::check_receivable(user_defined_type)?;
        // ⭐⭐ THE RAW ARM. This is **not** the falsified `RawStructCodec` guess wearing a new hat.
        // `activity_client_send_message` has two arms and the client picks between them at the call
        // site: `if ((*flags & 1) == 0) { encode through the descriptor } else { send the struct
        // image verbatim }`. Three ids take the second arm and pass descriptor `0xFFFFFFFF` -
        // 6, 8 and **15, the keepalive** - so for those the payload *is* the image, read out of the
        // client's own branch rather than assumed for all messages.
        // See `d1_oracle::activity_host::RAW_SEND`.
        if d1_oracle::activity_host::is_raw_send(user_defined_type).is_some() {
            return Ok(wire.to_vec());
        }
        let wire = wire.to_vec();
        let seed = seed.to_vec();
        oracle()
            .call(|reply| Job::Decode { udt: user_defined_type, wire, seed, reply })
            .map_err(|d| SchemaCodec::classify(user_defined_type, d))
    }
}

/// ⭐ Inbound ids this Activity Host **deliberately does not decode**, with the reason - so the run
/// log can distinguish *"we chose not to"* from *"the codec broke"*.
///
/// Today there is exactly one class: messages whose descriptor is a **dynamic flags** bitset
/// (`kind 0x22`), whose backing array comes from a runtime pool a statically mapped image does not
/// have. `d1-oracle` derives the set from the descriptor tree; nothing here is a list.
///
/// ⚠ These are all client → AH with **no client-side handler**, so nothing an Activity Host does
/// depends on decoding them. If that ever stops being true, this function is the wrong answer and
/// the right one is to bring the pool up in the harness.
pub fn undecodable_reason(user_defined_type: u32) -> Option<String> {
    let tag = d1_oracle::activity_host::tag_for_udt(user_defined_type).ok()?;
    d1_oracle::activity_host::dynamic_flags_reason(tag).ok().flatten()
}

/// ⚠ **Build [`undecodable_reason`]'s memoised set NOW, at startup.**
///
/// It walks the descriptor tree, which costs ~2 s on a cold process - and the first message that
/// would trigger it lazily is the **`join_request`**, i.e. it would land ~2 s of descriptor work
/// between the join and the `join_result`, in a rung whose failure mode is a timeout. Exactly the
/// reason [`SchemaCodec::prewarm`] exists; the same rule, one call later.
pub fn warm_undecodable_set() -> Vec<u32> {
    d1_oracle::activity_host::dynamic_flags_udts().unwrap_or_default()
}

/// ⛔ **FALSIFIED 2026-08-08, and kept as the negative control.**
///
/// Emits the struct image verbatim. It was run once, deliberately, against the live client: the
/// pushes went out, the client rejected them, and rung 30 died on
/// `error.starting_private_activity_instance_timed_out_waiting_for_join_response` ("elderberry").
/// The schema codec above then re-encoded a *captured* request to the byte, which is the positive
/// half of the same experiment.
///
/// It stays in the tree because a codec that only ever agrees with itself proves nothing: the
/// controls assert that this encoding and [`SchemaCodec`]'s **differ**, which is what makes
/// "the schema codec produced bytes" a claim rather than a tautology.
#[derive(Debug, Default, Clone, Copy)]
pub struct RawStructCodec;

impl RawStructCodec {
    /// Asserted by a test. If someone deletes this constant to make the type look respectable,
    /// the test that reads it stops compiling, which is the point.
    pub const IS_AN_EXPERIMENT: bool = true;
    /// ⭐ It was run, and it failed. Recorded in the type so nobody re-spends the boot.
    pub const WAS_FALSIFIED_ON_THE_LIVE_CLIENT: bool = true;
}

impl AhWireCodec for RawStructCodec {
    fn name(&self) -> &'static str {
        "raw-struct - ⚠ EXPERIMENT, FALSIFIED on the live client 2026-08-08"
    }

    fn encode(&self, msg: &AhMessage) -> Result<Vec<u8>, AhError> {
        crate::protocol::check_sendable(msg.user_defined_type())?;
        Ok(msg.struct_bytes())
    }

    fn decode(&self, user_defined_type: u32, wire: &[u8]) -> Result<Vec<u8>, AhError> {
        crate::protocol::check_receivable(user_defined_type)?;
        Ok(wire.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::JoinResult;

    /// ⭐⭐ **INVERTED 2026-08-08.** This test used to assert that the default codec *refuses* and
    /// names the descriptor global. It now asserts that it **works**, on the one payload we did not
    /// make up - and it still discriminates in three ways: the byte count, the bit-exact match
    /// against the captured wire, and the nonce landing at `+0x00` of the decode.
    #[test]
    fn the_default_codec_round_trips_the_captured_live_join_request() {
        let wire = hex(d1_oracle::activity_host::CAPTURED_JOIN_REQUEST);
        assert_eq!(wire.len(), 238);

        let image = SchemaCodec.decode(3, &wire).expect("the captured payload must decode");
        assert_eq!(image.len(), 0x100, "join_request's struct is 0x100");
        assert_eq!(
            u32::from_le_bytes(image[0..4].try_into().unwrap()),
            d1_oracle::activity_host::CAPTURED_NONCE,
            "the nonce the client's own log printed"
        );

        // The encode direction, on the same bytes, through the message type this crate ships.
        let echoed = AhMessage::Opaque { user_defined_type: 3, bytes: image.clone() };
        // udt 3 is client->AH, so `encode` refuses it by direction - that check runs first and is
        // its own assertion.
        assert!(matches!(
            SchemaCodec.encode(&echoed),
            Err(AhError::WrongDirection { user_defined_type: 3, .. })
        ));
    }

    /// The message that actually has to go out. It must encode, and it must **not** be the struct.
    #[test]
    fn a_join_result_encodes_to_schema_bytes_that_are_not_the_struct_image() {
        let msg = AhMessage::JoinResult(JoinResult::accepted([1, 2, 3, 4], 0xD1, "sess", 0));
        let wire = SchemaCodec.encode(&msg).expect("join_result must encode");
        let raw = RawStructCodec.encode(&msg).expect("the experiment always encodes");
        assert_eq!(raw.len(), 0x1A8, "the raw experiment is the struct verbatim");
        assert_ne!(wire, raw, "if these agreed, the schema codec would be doing nothing");
        // And the schema bytes decode back to the struct we handed in.
        let back = SchemaCodec.decode(4, &wire);
        // udt 4 is AH->client, so `decode` refuses by direction. Prove it through the oracle
        // instead, which has no direction opinion.
        assert!(matches!(back, Err(AhError::WrongDirection { user_defined_type: 4, .. })));
    }

    /// ⭐ The refusal did not go away - it narrowed. 22 of the 49 ids have no call site we have
    /// read, and for those the codec must still name the gate rather than improvise a tag.
    #[test]
    fn an_unpinned_user_defined_type_still_refuses_and_says_so() {
        // udt 42 `advance_replication_epoch` is sendable and has NO CALL_SITES row.
        let msg = AhMessage::AdvanceReplicationEpoch { epoch: 1, suppress: 0 };
        let err = SchemaCodec.encode(&msg).unwrap_err();
        assert!(
            matches!(err, AhError::SchemaCodecMissing { user_defined_type: 42, .. }),
            "{err}"
        );
        // Positive control on the same call: a pinned id goes through.
        assert!(SchemaCodec
            .encode(&AhMessage::JoinResult(JoinResult::accepted([0; 4], 1, "s", 0)))
            .is_ok());
    }

    #[test]
    fn the_raw_codec_is_labelled_an_experiment_and_records_that_it_was_falsified() {
        assert!(RawStructCodec::IS_AN_EXPERIMENT);
        assert!(RawStructCodec::WAS_FALSIFIED_ON_THE_LIVE_CLIENT);
        assert!(RawStructCodec.name().contains("FALSIFIED"));
        assert!(SchemaCodec.name().contains("CLIENT'S OWN"));
    }

    /// Even the experiment refuses to send an id the client cannot receive - the direction check
    /// is in the codec, not sprinkled through call sites.
    #[test]
    fn the_raw_codec_still_refuses_an_unsendable_id() {
        let msg = AhMessage::Opaque { user_defined_type: 16, bytes: vec![0; 4] };
        assert!(matches!(RawStructCodec.encode(&msg), Err(AhError::NoHandler { .. })));
        // Positive control on the same call: a legal AH→client id goes through.
        let ok = AhMessage::Opaque { user_defined_type: 42, bytes: vec![7, 0] };
        assert_eq!(RawStructCodec.encode(&ok).unwrap(), vec![7, 0]);
    }

    /// The tags this crate pins as literals must equal the ones the oracle derives from the eboot.
    /// Two instruments, one answer - and the literals are what a reader of this file sees.
    #[test]
    fn the_pinned_tags_equal_the_derived_ones() {
        assert_eq!(d1_oracle::activity_host::tag_for_udt(3).unwrap(), TAG_JOIN_REQUEST);
        assert_eq!(d1_oracle::activity_host::tag_for_udt(4).unwrap(), TAG_JOIN_RESULT);
        // The DESCRIPTOR_GLOBAL_* pins are 01.29's; `descriptor_global` follows the selected build.
        let row_0129 = |udt: u32| {
            d1_oracle::activity_host::CALL_SITES.iter().find(|(u, _, _)| *u == udt).map(|r| r.1)
        };
        assert_eq!(row_0129(3), Some(DESCRIPTOR_GLOBAL_JOIN_REQUEST));
        assert_eq!(row_0129(4), Some(DESCRIPTOR_GLOBAL_JOIN_RESULT));
        assert_eq!(row_0129(0), Some(DESCRIPTOR_GLOBAL_ENTITY_SLOTS));
        assert_eq!(row_0129(1), Some(DESCRIPTOR_GLOBAL_GLOBAL_STATE));
        let selected = d1_oracle::activity_host::call_sites();
        for udt in [3, 4, 0, 1] {
            let want = selected.iter().find(|(u, _, _)| *u == udt).map(|r| r.1);
            assert_eq!(Some(descriptor_global(udt)), want, "udt {udt}");
        }
        // An id with no row reads 0, which is what makes the error message honest.
        assert_eq!(descriptor_global(42), 0);
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }
}
