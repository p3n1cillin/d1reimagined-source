// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! # `d1-activity-host` - the fourth server role
//!
//! Layer A is the ShadNet/HTTP surface, B1 is Demonware, B2 is Bungie game services. The
//! **Activity Host** is none of them. Per Truman (GDC 2015) an Activity Host is a *cloud machine
//! running the Mission Script*, and it is **always the authority over Activity State**. The client
//! reaches it on **its own BAP connection** - slot 1 `OUT FAH` or slots 2/3 `OUT GAH0`/`GAH1`,
//! distinct from slot 0 `OUT PRIMARY` - at an address the *primary* connection hands out in BAP
//! message 17.
//!
//! ## ⛔⛔ THE CEILING - read this before mistaking an empty world for a bug
//!
//! **A stub Activity Host cannot populate a world, and this crate will never be able to.** The
//! encounter flow, spawn tables, wave pacing, objective state and boss phases are **server-side Lua
//! content**. They are not in the eboot, not in the PKGs, and no amount of static analysis of the
//! client recovers them. If every gate below is passed, the honest outcome is:
//!
//! > geometry, sky, your character, physics, movement - **and nothing else**. No enemies, no
//! > objectives, no encounter progression, no completion.
//!
//! That is the shape of the result, not a defect. What it would prove is worth a great deal - that
//! the whole boot → orbit → activity-session-creation → world-load → in-world walk is understood
//! and reproducible against a private server - but it is not "the game works".
//!
//! ## ✅ THE SECOND CEILING IS GONE - 2026-08-08
//!
//! > ⛔ **RETRACTED, IN PLACE.** This section used to say *"We cannot yet encode a single
//! > Activity-Host message"* and the whole crate refused to serialise. That was correct at the
//! > time and it is what kept the raw-struct experiment honest. It is **no longer true.**
//!
//! Every AH payload is **schema-serialised on the wire**, not a raw struct copy. Each receive
//! handler decodes it itself, through the same table-driven descriptor machinery that carries the
//! investment/RSAT grammar:
//!
//! ```text
//! FUN_01058020  (udt  4 join_result)            FUN_002A4090(desc @DAT_019022F8, wire, len, &out[0x1A8])
//! FUN_010579F0  (udt  0 entity_slots_allocated) FUN_002A4090(desc @DAT_01902310, wire, len, &out[0x600])
//! FUN_01057B80  (udt  1 global_activity_state)  FUN_002A4090(desc @DAT_01902380, wire, len, &out[...])
//! ```
//!
//! Only **after** that decode does the handler memcpy the struct
//! (`0x29BD10`: `memcpy(ac+0xCA0, decoded, 0x1A8)`).
//!
//! ⇒ ⚠ **The sizes in `historicaldocs/design/activity-host-contract.md` §2 and §4 are STRUCT sizes in memory, not
//! wire sizes.** That much stands - emitting `0x1A8` raw bytes for `join_result` *is* fabricating a
//! protocol, and the live client rejected exactly that on 2026-08-08.
//!
//! What changed is that `FUN_002A4090` turned out to **be** the QueueZ object codec `d1-oracle`
//! has driven since the roster work, and the missing descriptor tags are statically derivable. So
//! [`codec::SchemaCodec`] now serialises by **running the client's own encoder**, and the 238 bytes
//! a real client sent us decode to the exact values that client's own log printed. Full method,
//! controls and standing `[U]`s: `historicaldocs/design/ah-schema-codec.md` and [`codec`].
//!
//! [`codec::RawStructCodec`] stays, falsified, as the negative control.
//!
//! ## What this crate owns, and what it must never contain
//!
//! Owns (`historicaldocs/design/SERVER-ROLES-PLAN.md` §6):
//!
//! * **AH id allocation and the endpoint registry** - the id that goes into BAP message 7 and the
//!   address that comes back out of BAP message 17 ([`registry`]).
//! * **The `user_defined_type` application protocol** carried inside BAP messages 8 and 100 - the
//!   49-entry id space at `0x18F8780` ([`protocol`], [`messages`]).
//! * **The AH's own listener and session model** ([`listener`], [`state`]).
//!
//! Must never contain: **any framing.** No envelope, no inner header, no cipher. It consumes and
//! produces [`d1_bap`] types. The one place that hurts today is documented in [`push`].
//!
//! ## Evidence
//!
//! Reach is **`static-client-inference`** throughout, licensed by `d1-eboot controls` **22/22 PASS**
//! on the pinned 01.29 eboot. **Nothing in this crate is `observed-live`** - no client has ever
//! completed an Activity-Host handshake against anything we built. The moment one does, everything
//! downstream is `synthetic_ancestor`.

pub mod codec;
pub mod controls;
pub mod entity;
pub mod fireteam;
pub mod listener;
pub mod messages;
pub mod protocol;
pub mod push;
pub mod registry;
pub mod sense;
pub mod sensor_list;
pub mod state;
pub mod worlds;

pub use codec::{AhWireCodec, RawStructCodec, SchemaCodec};
pub use entity::{EntitySlotMask, ENTITY_INDEX_COUNT, ENTITY_MASK_BYTES};
pub use messages::{AhMessage, ClientMessage, JoinResult};
pub use protocol::{AhDirection, AH_MESSAGE_COUNT, AH_MESSAGE_NAMES};
pub use push::{AhPush, PushBlocked};
pub use registry::{HostEntry, HostRegistry};
pub use state::{ActivityInstance, InstanceState};

/// Everything this crate can refuse to do, with the reason a reader can act on.
#[derive(Debug, PartialEq, Eq)]
pub enum AhError {
    /// ⭐ **Narrowed 2026-08-08.** The schema codec is implemented (see [`codec`]); this now means
    /// only *"this `user_defined_type`'s descriptor global has not been read out of a call site, so
    /// its schema tag is UNPINNED."* 22 of the 49 ids are still in that state, and improvising a
    /// tag for one of them from a struct-size match is the trap this error exists to prevent.
    SchemaCodecMissing { user_defined_type: u32, descriptor_global: u64 },
    /// The schema codec ran and refused, or could not run at all (no pinned eboot, a struct image
    /// of the wrong size, the client's own codec returning false). The oracle's own wording is
    /// preserved verbatim - it names the failing predicate.
    SchemaCodec { user_defined_type: u32, detail: String },
    /// The id is not one of the 49 in the table at `0x18F8780`.
    UnknownUserDefinedType(u32),
    /// The id exists but the client has no handler for it in either direction - `9, 16, 21, 34,
    /// 40`. Sending one is a silent drop at best.
    NoHandler { user_defined_type: u32, name: &'static str },
    /// We tried to send an id the client only ever *sends*, or receive one it only ever *gets*.
    WrongDirection { user_defined_type: u32, name: &'static str, expected: AhDirection },
    /// A decoded struct was not the size the client's handler allocates for it.
    BadStructSize { user_defined_type: u32, got: usize, want: usize },
    /// A field would not fit its fixed-width slot.
    FieldOverflow { field: &'static str, len: usize, max: usize },
    /// An AH id of zero. `0` means "no activity host" in the session parameter, and
    /// `FUN_00594320` returns false while it is zero - so it can never be allocated.
    ZeroActivityHostId,
}

impl std::fmt::Display for AhError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SchemaCodecMissing { user_defined_type, descriptor_global } => write!(
                f,
                "udt {user_defined_type} ({}) has an UNPINNED schema tag: no descriptor global \
                 (0x{descriptor_global:X}) has been read out of a call site for it, so \
                 FUN_002A4090 cannot be pointed at a descriptor. Read its FUN_00299310 call site \
                 and add the row to d1_oracle::activity_host::CALL_SITES - do NOT guess a tag from \
                 a struct-size match",
                protocol::name_or_unknown(*user_defined_type),
            ),
            Self::SchemaCodec { user_defined_type, detail } => write!(
                f,
                "udt {user_defined_type} ({}): the client's own schema codec (FUN_002A4090 / \
                 FUN_002A4140) could not be run: {detail}",
                protocol::name_or_unknown(*user_defined_type),
            ),
            Self::UnknownUserDefinedType(t) => {
                write!(f, "udt {t} is not one of the 49 ids at 0x18F8780")
            }
            Self::NoHandler { user_defined_type, name } => write!(
                f,
                "udt {user_defined_type} ({name}) has no handler in either direction - the client \
                 will drop it silently"
            ),
            Self::WrongDirection { user_defined_type, name, expected } => write!(
                f,
                "udt {user_defined_type} ({name}) is {expected:?}, not what this path carries"
            ),
            Self::BadStructSize { user_defined_type, got, want } => {
                write!(f, "udt {user_defined_type}: struct is {got} bytes, handler allocates {want}")
            }
            Self::FieldOverflow { field, len, max } => {
                write!(f, "field {field}: {len} bytes does not fit {max}")
            }
            Self::ZeroActivityHostId => write!(
                f,
                "activity-host id 0 is the 'no activity host' sentinel; FUN_00594320 refuses to \
                 issue message 16 while it is zero"
            ),
        }
    }
}

impl AhError {
    /// ⭐⭐ **Is this OUR defect, or an honest gate?**
    ///
    /// The distinction is not cosmetic; it is the one that cost analysis time on 2026-08-09. Run
    /// `20260809-120451-MOONBASE-FRESH` published `global_activity_state` 131 times and then the
    /// oracle's bump arena ran out. The host logged `** NAMED GATE ** push blocked` - the same
    /// wording it uses for *"this udt's tag is not pinned yet"* - at **INFO** on the heartbeat
    /// path, went quiet, and the client eventually said
    /// `error.private_activity_host_host_crashed`. Every observable pointed at the protocol.
    ///
    /// * [`AhError::SchemaCodec`] is **the codec broke**: no pinned eboot, a guest fault, the
    ///   client's own encoder returning false. Nothing about the protocol is in question and the
    ///   run is now producing nothing. ⇒ ERROR, and say the word CRASH.
    /// * Everything else is a **gate**: an unpinned tag, a wrong direction, an id with no handler.
    ///   Correct behaviour, stated. ⇒ WARN, as before.
    pub fn is_host_defect(&self) -> bool {
        matches!(self, Self::SchemaCodec { .. })
    }
}

impl std::error::Error for AhError {}
