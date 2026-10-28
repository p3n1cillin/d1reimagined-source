// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! `d1-oracle` - **ask the client, don't guess.**
//!
//! This crate maps `dumps/patch-01.29-CUSA00219/eboot.bin` into Unicorn and calls Destiny's *own*
//! RSAT codecs. "The client accepts this payload" is therefore an **execution**, not an inference:
//! nothing here reimplements Bungie's codec, and there is no hand-written parser to drift.
//! `.claude/skills/native-oracle/SKILL.md` is the method; this crate is the method made executable.
//!
//! > **Rule of the house:** in this protocol *answering wrongly is worse than not answering*
//! > (`status/LESSONS.md`). A payload that is merely plausible can time out an in-flight queue and
//! > take the whole BAP connection down - which then looks like a regression in whatever you were
//! > actually testing. The oracle removes the guess *before* the run, for free.
//!
//! # The point of the Rust port: deriving a payload is a `cargo test`
//!
//! ```no_run
//! use d1_oracle::{control, ObjectOracle};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // The control has already run - `new` refuses to hand back an oracle otherwise.
//! let mut o = ObjectOracle::new(0x8080_1803)?;
//! let d = o.decode(&control::hex("e880000000000000c74400000000000405d100000000000201000000")?)?;
//! assert_eq!(d.bits, 221);
//! d.require_nonzero_qwords(&[0x00, 0x08, 0x10, 0x68])?;
//! # Ok(()) }
//! ```
//!
//! # Two codecs, and picking the wrong one is silent
//!
//! | module | drives | use for |
//! |---|---|---|
//! | [`object`] | `0x2C9000` / `0x2C8FF0` | **QueueZ server objects** (roster, character, account, profile). Raw `rsat[len]`, no message id, no trailing span bit. |
//! | [`message`] | `0x261BE0` / `0x261FB0` | **investment message bodies** (503, 504, 701, 702 …). Wraps a `u16` id + a trailing span-presence bit. |
//!
//! They differ by exactly that id and that bit. There is no error when you choose wrong - just a
//! wrong answer that looks right.
//!
//! # What the oracle is *not*
//!
//! ⚠ **A decoder, not a semantics engine.** It proves *shape*: that N bits decode into an object
//! with values at the offsets you expect. It cannot tell you whether the value is *meaningful* to
//! the game. Keep those claims separate - `[C]` on the encoding, `[U]` on the meaning.
//!
//! # The three checks that are enforced in code rather than remembered
//!
//! 1. **The control runs first.** [`ObjectOracle::new`] / [`MessageOracle::new`] return
//!    [`OracleError::ControlFailed`] unless the pinned control has passed. A decode obtained
//!    without its control is not evidence, so this crate will not produce one.
//! 2. **The bit count is asserted, not just the bytes.** An under-length payload is
//!    [`OracleError::ShortPayload`] - a hard error, never a quiet success. (`§26.67`: a 17-byte
//!    account decoded fine while the client read 157 bits out of 136.)
//! 3. **The exact set of non-zero qwords** is available and assertable
//!    ([`object::Decoded::require_nonzero_qwords`]). A stray non-zero qword is a bit-grammar bug
//!    that would otherwise reach the wire.
//!
//! # Licence
//!
//! GPL-2.0-or-later, like the rest of the workspace. This crate is the **only** one that depends on
//! `unicorn-engine` (GPL-2.0, bundling QEMU GPL-2.0 and `glib_compat` LGPL-2.1) - the strongest
//! copyleft in the tree is confined to one crate boundary on purpose. See `Cargo.toml`.

pub mod activity_host;
pub mod activity_modes;
pub mod art_arrangement;
pub mod character;
pub mod codec;
pub mod control;
pub mod error;
pub mod harness;
pub mod inventory;
pub mod item_index;
pub mod message;
pub mod object;
pub mod oodle;
pub mod profile;
pub mod sensor;
pub mod sensor_objective;
pub mod sensor_sense;
pub mod registry;
pub mod roster;
pub mod schema;
pub mod weapon_stats;

pub use activity_host::{AhCodec, AhSchema};
pub use character::{BuiltCharacter, CharacterSpec, EquipSlot};
pub use error::{OracleError, Result};
pub use harness::Harness;
pub use inventory::{BuiltItem, ItemSpec, StagedElement};
pub use item_index::IndexTable;
pub use message::{DecodedMessage, MessageOracle};
pub use object::{Decoded, Encoded, ObjectOracle};
pub use profile::{BuiltProfile, Progression, ProfileSpec};
pub use roster::{BuiltObject, BuiltRoster, RosterSpec};

/// SOIDs used across the pinned vectors.
///
/// ⚠ **Never transcribe one of these by eye.** They are all `0xD1` + six zero bytes + two
/// significant ones, so they are maximally easy to misread - and one such misread manufactured a
/// false "unknown identity" lead. In particular the account schema's field 3 is
/// [`PROFILE_SOID`] = `0xD100000000000201`; an earlier write-up said `…0002`, and that was a
/// hand-transcription error (`historicaldocs/design/character-and-orbit.md` §26.76.1).
pub const ACCOUNT_SOID: u64 = 0xD100_0000_0000_0001;
pub const CHARACTER_SOID: u64 = 0xD100_0000_0000_0101;
pub const PROFILE_SOID: u64 = 0xD100_0000_0000_0201;
