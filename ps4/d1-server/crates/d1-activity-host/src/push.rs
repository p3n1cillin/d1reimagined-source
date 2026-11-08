// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The push channel - BAP **9** `bap_to_client_notification`, why **100** does not work, and why
//! **102** is dead.
//!
//! > ## ⛔⛔ RETRACTED IN PLACE, 2026-08-09 - **MESSAGE 100 NEVER REACHES THE ACTIVITY CLIENT**
//! >
//! > Everything below about framing message 100 is correct and stays. What was wrong is the
//! > premise: that 100 is the message an Activity Host sends *to a client*. It is the message an
//! > Activity Host sends **to the BAP server**, and the BAP server re-emits it to the client as
//! > **message 9 `bap_to_client_notification`**. The two share a body codec (the message classes
//! > `0x1923850` and `0x19237E0` share vtable slots 2/3/4/6 - encoder `0x55E690`, decoder
//! > `0x55E710`→`0x55E130`), so the bytes after the header are identical; only the push policy
//! > differs, and the policies are opposites:
//! >
//! > | | push policy vtable | slot 1 (decode?) | slot 5 (deliver) |
//! > |---|---|---|---|
//! > | **9** | `0x1925B10` | `0x1F70` = true | `0x5642E0` → `0x75CB40` → `0x75CB70` → recipient lookup `0x75AB60` → `0x594960` → `c_activity_client::vfunc[9]` |
//! > | **100** | `0x1925DB0` | `0x14D0` = false | `0x564C20` - reads the class-6 header `+0x06` as a **connection index** and re-sends the same body as **policy 9 = message 9** on it |
//! >
//! > `handle_message_internal` (`0x563170`) is what calls `policy->vfunc[5]` after decoding.
//! >
//! > ⭐ **Measured, not inferred.** Run `20260808-234535-CODEC-LIVE2` pushed udt 4, 0, 1 and 11 as
//! > four message-100 frames on the AH connection. The client relayed **all four, byte for byte,
//! > 1:1** to the *primary* BAP connection as message 9 (`d1-responder` seq 462-465, udts 4/0/1/11,
//! > sizes 398 / 0x600 / 154 / 7297) - because we leave header `+0x06` at zero and connection 0 was
//! > in state 3. `c_activity_client::vfunc[9]`'s own two diagnostics (*"unknown message type"*,
//! > *"'%d' message could not be handled."*) never fired, and neither did `0x29BD10`'s
//! > *"Received join result from AH"*: **nothing ever arrived at the activity client.**
//! >
//! > ⇒ [`AhPush::emit`] now sends **9**. `D1_AH_BAP_PUSH_ID=100` reproduces the failure.
//! > Full write-up: `historicaldocs/design/establish-client-gate.md`.
//!
//! An Activity Host talks to the client entirely in unsolicited frames: it has no outstanding
//! request to answer, so everything it says is a *push*. That makes the push accept-set the
//! gatekeeper for the whole role.
//!
//! ## What is legal
//!
//! * **100 is pushable.** It is in the client's 13-entry accept-set at `0x1925A90`
//!   (`d1_bap::tables::PUSH_ACCEPT_SET`) and in `d1-bap`'s narrower `OURS_TO_PUSH`.
//! * **102 `activity_host_to_client_log_notification` is pushable nowhere.** It is absent from the
//!   accept-set - and the hope that some *other* connection installs a different push handler is
//!   now closed: all four BAP connections are built by `FUN_00559650(index)`, which takes its push
//!   handler from **one pool with one factory** and no dependence on the index
//!   (`crate::protocol::CONNECTION_SLOT_NAMES`). ⇒ The plan §9 Tier-2 `[U]` *"where message 102 is
//!   legally delivered"* answers **nowhere on any BAP connection**. Its codec exists in `d1-bap`;
//!   [`AhPush::log_notification`] refuses to send it and says why.
//!
//! ## ✅ 100 IS NO LONGER BLOCKED - `d1-bap` writes header class 6 as of 2026-08-08
//!
//! Message 100 is **header class 6 = 32 bytes**. `d1_bap::BapConversation::push` used to refuse it
//! with `PushError::UnwritableHeader { msg_type: 100, header_class: Some(6) }` because nobody had
//! read that layout. The layout below was read here; the **encoder lives in `d1-bap`**
//! (`d1_bap::HeaderExtras`, `conversation::write_header_with`) - *one protocol, one implementer*,
//! and this crate still never touches a header byte (`historicaldocs/design/SERVER-ROLES-PLAN.md` §6).
//! Full evidence, including classes 3 and 5: **`historicaldocs/design/bap-header-classes.md`**.
//!
//! ⚠ The remaining gate is **addressing, not framing**: a wrong header field or a wrong body
//! `recipient_id` is a **silent drop**, not an error.
//!
//! ### Header class 6, `[C]`, re-derived from the pinned 01.29 eboot
//!
//! Codec singleton table `0x19234A0` → class 6's vtable **`0x1923210`**. Slot 1 = `0xB1BA0`
//! (`mov eax, 0x20; ret`) = 32 bytes, which is the value already in `d1_bap::HEADER_LEN[6]`.
//! The field accessors tile those 32 bytes **exactly, with no gap and no overlap** - that is the
//! positive control for the layout being complete:
//!
//! | offset | width | vtable slots (set/get) | accessor | wire order |
//! |---|---|---|---|---|
//! | `+0x00` | u16 | (base) | `0xB327E0` `rol ax,8` | **big-endian** `msg_type` |
//! | `+0x02` | u32 | 12/13 `0x55D7D0`/`0x55D7E0` | `movbe` | **big-endian** `sequence` |
//! | `+0x06` | u64 | 4/5 `0x55D1C0`/`0x55D1D0` | `movbe [rbx+6]` | **big-endian**, `[U]` semantics |
//! | `+0x0E` | u64 | 6/7 `0x55D1E0`/`0x55D1F0` | `movbe [rbx+0xE]` | **big-endian**, `[U]` semantics |
//! | `+0x16` | u16 | 10/11 `0x55D200`/`0x55D210` | ⚠ **never byte-swapped** | native/LE, `[U]` |
//! | `+0x18` | u64 | 8/9 `0x55D530`/`0x55D540` | `0x2A2BF0` = `movbe [rdi]` | **big-endian**, `[U]` |
//!
//! Encoder `0x55D240`, decoder `0x55D2D0`; both perform the same three `movbe`s in mirror, so the
//! reading is confirmed in both directions. The `+0x16` `u16` really is untouched by either - it
//! is the one field where "emit it little-endian" and "emit it as two opaque bytes" are the same
//! instruction.
//!
//! Sanity control on the same method: applying it to class 2 reproduces the layout `d1-bap`
//! already ships and uses live - 8 bytes, `u16be type`, `u32be sequence`, `u16be status` at `+6`
//! (`0x55D5C3  rol word [rbx+6], 8`).
//!
//! Bonus control, unasked for: slot 0 of each of the eight class vtables is a `mov al, <flags>;
//! ret`, and reading all eight reproduces `d1_bap::HEADER_FLAGS = [0x00, 0x00, 0x18, 0x04, 0x00,
//! 0x43, 0x4F, 0x18]` **byte for byte**. That table was transcribed rather than derived in
//! `d1-bap`'s `tables.rs`; it is now re-derived.
//!
//! > ### ⛔ RETRACTED, 2026-08-08 - the `HEADER_FLAGS` gloss was right and this paragraph was wrong
//! >
//! > This module previously claimed *"`HEADER_FLAGS` bit `0x10` is documented as 'carries a `u16be`
//! > status', and class 6 has that bit set while having no status accessor - so the bits are a
//! > capability mask, not a status flag."* **The arithmetic refuses it: class 6's flag byte is
//! > `0x4F` (`0x55D220: mov al, 0x4F; ret`) and `0x4F & 0x10 == 0`.** The bit is *clear*.
//! >
//! > Across all eight classes, bit `0x10` set ⟺ a real status accessor exists, with no exception:
//! > classes **2 and 7** are `0x18` and are the only two whose slots 14/15 are
//! > `0x55D7B0`/`0x55D7C0` (`mov [rdx+6], si` / `movzx eax, word [rsi+6]`); classes 3, 5 and 6 are
//! > `0x04`, `0x43`, `0x4F` and all three have the `0x50FE20` / `0x1DF0` stubs there. `d1-bap`'s
//! > original gloss stands and `write_header_with` relies on it.
//! >
//! > *One instance is not a category - and a hex AND is cheaper than a retraction.*
//!
//! ⚠ The four `[U]` fields default to zero here.
//!
//! ⚠⚠ **`recipient_id` is NOT gated by the push policy `0x55F6B0`.** An earlier note here said it
//! was. It is not on message 100's receive path at all: `0x55F6B0` belongs to
//! `c_investment_bap_message_policy_…`, a *different* class from the `push_policy_…` the receive
//! path uses, and message 100's push policy (vtable `0x1925DB0`) has **slot 1 = `0x14D0`
//! (`xor al, al; ret` = false)**. `c_investment_bap_message_push_handler::vfunc[2]` (`0x563590`)
//! tests exactly that slot before decoding, so for message 100 it **never decodes the body and
//! never calls the policy** - it copies the 132-byte header struct and the raw payload straight
//! into the simulation queue (`0xBAB380`). *Positive control on the same slot:* message **123**'s
//! push policy (vtable `0x1925E10`) has slot 1 = `0x1F70` (`mov al, 1; ret` = true), so 123 **is**
//! decoded and delivered through its policy. Same slot, opposite values, opposite behaviour.
//! ⇒ Whatever validates `recipient_id`, it is downstream of the queue, not in the BAP layer. `[C]`

use d1_bap::conversation::{BapConversation, PushError};
use d1_bap::activity_host::ActivityHostToClientNotification;

use crate::codec::AhWireCodec;
use crate::messages::AhMessage;
use crate::AhError;

/// BAP message id for `activity_host_to_client_notification`. Decimal 100 / `0x0064`.
///
/// ⛔ **Measured NOT to reach the client.** Kept so `D1_AH_BAP_PUSH_ID=100` can reproduce the
/// failure of run `20260808-234535-CODEC-LIVE2` exactly.
pub const BAP_TYPE_AH_TO_CLIENT_NOTIFICATION: u16 = 100;
/// ⭐ BAP message id for `bap_to_client_notification`. Decimal 9 / `0x0009`, header class 0.
/// **The default since 2026-08-09** - the only one of the pair whose deliver step routes into
/// `c_activity_client::receive_message_from_host`.
pub const BAP_TYPE_BAP_TO_CLIENT_NOTIFICATION: u16 = 9;

/// Which BAP id our pushes ride. **`9` by default**; `D1_AH_BAP_PUSH_ID=100` restores the
/// pre-2026-08-09 behaviour, which is a *documented negative*, not an alternative.
///
/// Read once. Anything other than 9 or 100 is refused here rather than framed, because the two
/// are the only ids that share this body codec and a third would be a fabricated frame.
pub fn push_bap_message_id() -> u16 {
    use std::sync::OnceLock;
    static ID: OnceLock<u16> = OnceLock::new();
    *ID.get_or_init(|| match std::env::var("D1_AH_BAP_PUSH_ID").ok().as_deref() {
        Some("100") => {
            eprintln!(
                "⛔ D1_AH_BAP_PUSH_ID=100 - this is the SPENT configuration. Run \
                 20260808-234535-CODEC-LIVE2 pushed all four establish-client messages as BAP 100 \
                 and the client relayed every one of them straight back to the primary connection \
                 as BAP 9 (0x564C20), never delivering any of them to the activity client. \
                 Expect error.starting_private_activity_instance_timed_out_waiting_for_join_response."
            );
            BAP_TYPE_AH_TO_CLIENT_NOTIFICATION
        }
        Some(other) if !other.is_empty() && other != "9" => {
            eprintln!(
                "⛔ D1_AH_BAP_PUSH_ID={other} is not one of the two ids that share this body \
                 codec (9, 100). Using 9."
            );
            BAP_TYPE_BAP_TO_CLIENT_NOTIFICATION
        }
        _ => BAP_TYPE_BAP_TO_CLIENT_NOTIFICATION,
    })
}
/// BAP message id for `activity_host_to_client_log_notification`. Decimal 102 / `0x0066`.
/// ⛔ Not pushable anywhere. Kept so the refusal can name it.
pub const BAP_TYPE_AH_TO_CLIENT_LOG_NOTIFICATION: u16 = 102;

/// The class-6 header field map, as **data for the `d1-bap` implementer** - deliberately not an
/// encoder. `(name, offset, width, big_endian_on_the_wire)`.
pub const CLASS6_HEADER_FIELDS: [(&str, usize, usize, bool); 6] = [
    ("msg_type", 0x00, 2, true),
    ("sequence", 0x02, 4, true),
    ("field_06", 0x06, 8, true),
    ("field_0e", 0x0E, 8, true),
    ("field_16", 0x16, 2, false),
    ("field_18", 0x18, 8, true),
];

/// One thing an Activity Host wants to say to the client.
///
/// It carries an AH message, not bytes: serialisation happens at the last moment, through the
/// codec, so a crate that has no codec cannot accidentally emit a fabricated frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AhPush {
    /// `[I]` The addressing key at body `+0x01`. A wrong value is a **silent drop**.
    pub recipient_id: u64,
    pub message: AhMessage,
}

/// Why a push did not go out.
#[derive(Debug, PartialEq, Eq)]
pub enum PushBlocked {
    /// The BAP layer refused the frame. `UnwritableHeader { msg_type: 100, .. }` is the expected
    /// one today - see the module doc.
    Bap(PushError),
    /// The AH-protocol layer refused: wrong direction, unknown id, or (the usual case) the wire
    /// codec is not implemented.
    Ah(AhError),
    /// ⛔ 102, always. Recorded as its own variant so a run log can distinguish "we tried and BAP
    /// said no" from "we know better than to try".
    LogNotificationIsDeliverableNowhere,
}

impl PushBlocked {
    /// ⭐ **Our defect, not a gate** - see [`AhError::is_host_defect`]. A `true` here means the run
    /// has stopped producing and the reason is on this side of the wire.
    pub fn is_host_defect(&self) -> bool {
        matches!(self, Self::Ah(e) if e.is_host_defect())
    }
}

impl std::fmt::Display for PushBlocked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bap(e) => write!(f, "BAP refused the push: {e:?}"),
            Self::Ah(e) => write!(f, "{e}"),
            Self::LogNotificationIsDeliverableNowhere => write!(
                f,
                "BAP 102 activity_host_to_client_log_notification is absent from the client's \
                 13-entry push accept-set at 0x1925A90, and all four BAP connections share one \
                 push handler (FUN_00559650) - so there is no connection on which it is legal"
            ),
        }
    }
}

impl AhPush {
    pub fn new(recipient_id: u64, message: AhMessage) -> Self {
        Self { recipient_id, message }
    }

    /// Build the **BAP message 100 body** - `d1-bap`'s type, `d1-bap`'s encoding.
    ///
    /// This is as far as this crate is allowed to go on its own: the body is application payload,
    /// the header is framing.
    pub fn to_bap_body(&self, codec: &dyn AhWireCodec) -> Result<Vec<u8>, PushBlocked> {
        let udt = self.message.user_defined_type();
        crate::protocol::check_sendable(udt).map_err(PushBlocked::Ah)?;
        let payload = codec.encode(&self.message).map_err(PushBlocked::Ah)?;
        Ok(ActivityHostToClientNotification {
            recipient_id: self.recipient_id,
            user_defined_type: udt,
            payload,
        }
        .encode())
    }

    /// Hand the frame to `d1-bap` to write, under [`push_bap_message_id`].
    ///
    /// ⭐⭐ **Since 2026-08-09 the default id is 9, not 100** - see the module banner. `emit_as`
    /// is the testable half; `emit` only chooses the id.
    pub fn emit(
        &self,
        conv: &mut BapConversation,
        codec: &dyn AhWireCodec,
    ) -> Result<Vec<u8>, PushBlocked> {
        self.emit_as(conv, codec, push_bap_message_id())
    }

    /// [`Self::emit`] with the BAP message id named explicitly.
    ///
    /// Only `9` and `100` are meaningful: they share the body codec, so the bytes after the header
    /// are identical and the *only* variable is which push policy the client runs.
    pub fn emit_as(
        &self,
        conv: &mut BapConversation,
        codec: &dyn AhWireCodec,
        msg_type: u16,
    ) -> Result<Vec<u8>, PushBlocked> {
        let body = self.to_bap_body(codec)?;
        conv.push(msg_type, &body).map_err(PushBlocked::Bap)
    }

    /// ⛔ BAP 102. Always refused, and the refusal is the finding.
    ///
    /// The codec exists in `d1-bap` (`ActivityHostToClientLogNotification`, 256 bytes, shared
    /// verbatim with message 27). Nothing stops you *building* one; there is simply no connection
    /// on which sending it is legal.
    pub fn log_notification(_text: &str) -> Result<Vec<u8>, PushBlocked> {
        Err(PushBlocked::LogNotificationIsDeliverableNowhere)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::{RawStructCodec, SchemaCodec};
    use crate::messages::JoinResult;

    fn a_push() -> AhPush {
        AhPush::new(0xD100_0000_0000_0001, AhMessage::JoinResult(JoinResult::accepted([1, 2, 3, 4], 9, "s", 0)))
    }

    /// ⭐⭐ **INVERTED 2026-08-09.** This used to assert that a push goes out as message 100 with a
    /// 32-byte class-6 header. Message 100 is now known to be the AH→**BAP** hop, so the assertion
    /// is flipped to the id we actually send - and the class-6 knowledge is kept, not deleted, as
    /// the *negative* half of the same test.
    ///
    /// It discriminates three ways: the default id, the header length that goes with it, and the
    /// fact that **the bodies are byte-identical** - which is the whole reason the id is the only
    /// variable worth changing.
    #[test]
    fn a_push_goes_out_as_message_9_and_message_100_is_the_kept_negative() {
        let mut conv = BapConversation::with_default_deadline();
        let body = a_push().to_bap_body(&RawStructCodec).unwrap();

        let nine = a_push().emit_as(&mut conv, &RawStructCodec, BAP_TYPE_BAP_TO_CLIENT_NOTIFICATION).unwrap();
        assert_eq!(&nine[0..2], &[0x00, 0x09], "u16be type = 9");
        assert_eq!(nine.len(), 6 + body.len(), "message 9 is header class 0 = 6 bytes");
        assert_eq!(&nine[6..], &body[..], "the body starts at +6");

        // The spent configuration still frames correctly - class 6 is 32 bytes and the four `[U]`
        // fields are zero, which is exactly why the client relayed it to connection 0.
        let hundred = a_push().emit_as(&mut conv, &RawStructCodec, BAP_TYPE_AH_TO_CLIENT_NOTIFICATION).unwrap();
        assert_eq!(&hundred[0..2], &[0x00, 0x64], "u16be type = 100");
        assert_eq!(hundred.len(), 32 + body.len(), "class 6 is 32 bytes, not 6");
        assert_eq!(&hundred[6..32], &[0u8; 26]);
        assert_eq!(&hundred[32..], &body[..]);

        // ⭐ Same payload, two ids: the ONLY difference is the header.
        assert_eq!(&nine[6..], &hundred[32..]);

        // Positive control on the same call: the push type we *do* send every run still goes
        // through, and with its own 6-byte class-4 header - so this cannot pass by writing 32
        // bytes for everything.
        let q = conv.push(d1_bap::TYPE_QUEUEZ_OBJECT, &[0u8; 4]).unwrap();
        assert_eq!(q.len(), 6 + 4);
    }

    /// The default is 9 unless the environment overrides it, and the override is refused for
    /// anything that is not one of the two ids that share this body codec.
    #[test]
    fn the_default_push_id_is_nine() {
        // ⚠ Asserted on the *constant*, not on `push_bap_message_id()`: that reads the process
        // environment through a `OnceLock`, and a test that mutates it would leak into every other
        // test in this binary. The env path is exercised by the run, and by the ⛔ line it prints.
        assert_eq!(BAP_TYPE_BAP_TO_CLIENT_NOTIFICATION, 9);
        assert!(d1_bap::tables::is_ours_to_push(9), "9 must be ours to originate");
        assert!(d1_bap::tables::is_pushable(9), "9 must be on the client's accept-set");
    }

    /// The class-6 layout tiles 32 bytes exactly. That is the control for having read the whole
    /// header rather than most of it.
    #[test]
    fn the_class_six_field_map_tiles_thirty_two_bytes_with_no_gap_and_no_overlap() {
        let mut covered = [false; 32];
        for (name, off, len, _) in CLASS6_HEADER_FIELDS {
            for b in off..off + len {
                assert!(!covered[b], "{name} overlaps at +{b:#x}");
                covered[b] = true;
            }
        }
        assert!(covered.iter().all(|c| *c), "class 6 has an unaccounted byte");
        assert_eq!(d1_bap::HEADER_LEN[6], 32);
    }

    /// The one field neither the encoder nor the decoder byte-swaps.
    #[test]
    fn exactly_one_class_six_field_is_not_byte_swapped() {
        let native: Vec<&str> = CLASS6_HEADER_FIELDS
            .iter()
            .filter(|(_, _, _, be)| !be)
            .map(|(n, _, _, _)| *n)
            .collect();
        assert_eq!(native, vec!["field_16"]);
    }

    #[test]
    fn the_body_is_built_by_d1_bap_and_carries_the_udt() {
        let body = a_push().to_bap_body(&RawStructCodec).unwrap();
        assert_eq!(body[0], 1, "message 100 has exactly one inner type");
        assert_eq!(&body[1..9], &0xD100_0000_0000_0001u64.to_be_bytes());
        assert_eq!(&body[9..13], &4u32.to_be_bytes(), "udt 4 = join_result");
        assert_eq!(&body[13..17], &(0x1A8u32).to_be_bytes(), "payload size");
        assert_eq!(body.len(), 0x11 + 0x1A8);
    }

    /// ⭐⭐ **INVERTED 2026-08-08.** This asserted that the default codec produced *no body at all*
    /// and named the descriptor global. The schema codec is implemented, so it now asserts the body
    /// **exists and is schema-coded** - and it still discriminates three ways: the payload length
    /// declared in the body must match the bytes that follow, the body must be shorter than the
    /// raw-struct experiment's, and a message with an unpinned tag must still be blocked.
    #[test]
    fn the_default_codec_now_produces_a_schema_body_that_is_not_the_struct() {
        if !crate::eboot_or_skip("the_default_codec_now_produces_a_schema_body_that_is_not_the_struct") {
            return;
        }
        let body = a_push().to_bap_body(&SchemaCodec).unwrap();
        let raw = a_push().to_bap_body(&RawStructCodec).unwrap();
        assert_eq!(&body[9..13], &4u32.to_be_bytes(), "udt 4 = join_result");
        let declared = u32::from_be_bytes(body[13..17].try_into().unwrap()) as usize;
        assert_eq!(body.len(), 0x11 + declared, "the declared size must match the bytes");
        assert_ne!(declared, 0x1A8, "0x1A8 would be the raw struct, which the client REJECTED");
        assert!(body.len() < raw.len(), "the schema encoding is denser than the struct");

        // The refusal did not vanish, it narrowed: an unpinned udt is still blocked here.
        let unpinned =
            AhPush::new(1, AhMessage::AdvanceReplicationEpoch { epoch: 1, suppress: 0 });
        assert!(matches!(
            unpinned.to_bap_body(&SchemaCodec),
            Err(PushBlocked::Ah(AhError::SchemaCodecMissing { .. }))
        ));
    }

    #[test]
    fn message_102_is_refused_without_even_asking_bap() {
        let err = AhPush::log_notification("hello").unwrap_err();
        assert_eq!(err, PushBlocked::LogNotificationIsDeliverableNowhere);
        assert!(err.to_string().contains("0x1925A90"));
        // And BAP would refuse it too, for the same reason - two instruments agreeing.
        let mut conv = BapConversation::with_default_deadline();
        assert_eq!(
            conv.push(BAP_TYPE_AH_TO_CLIENT_LOG_NOTIFICATION, &[0u8; 256]),
            Err(PushError::NotPushable(102))
        );
    }

    #[test]
    fn a_message_the_client_cannot_receive_is_refused_before_framing() {
        let p = AhPush::new(1, AhMessage::Opaque { user_defined_type: 16, bytes: vec![0] });
        assert!(matches!(p.to_bap_body(&RawStructCodec), Err(PushBlocked::Ah(AhError::NoHandler { .. }))));
    }
}
