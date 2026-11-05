//! Messages **42 / 43** - `client_to_bap_update_matchmaking_core_request` / `_response`.
//!
//! ⛔ **Not 34/35.** The `Sending MM configuration request` line comes from `0x599620`, which passes
//! **policy index `0x1E`**; that policy is
//! `c_investment_bap_message_policy_client_to_bap_matchmaking_core_request`, whose vtable
//! (`0x1924CA0`) reads its request id out of `0x55D940 = mov eax, 0x2A ; ret` = **42**, reply
//! **43**. This crate's [`crate::tables::POLICIES`] carries that row from the same walk. 34/35 is
//! the *skill* policy, driven by `c_network_leaderboard_skillz`.
//!
//! These are **ordinary protobuf**, driven by static descriptor tables (request `0x1929E30`, struct
//! `0x410`; response `0x192A110`, struct `0x2290`), walked with the record format in
//! `historicaldocs/design/signon-protocol.md` §2.1. Header classes 1 and 2 - no new header work.
//!
//! # ⭐ The nonce rule - a correction that removes a phantom gate
//!
//! The nonce is **client-internal**. `FUN_00598710` compares the *live interface's* nonce against
//! the one the client stashed at enqueue time out of its **own outgoing request** (policy
//! `vfunc[1]` = `0x560560` reads request offsets `+0x0C/+0x18/+0x24/+0x34`, which match the
//! REQUEST struct, not the response). The response body is never consulted for it.
//!
//! ⇒ The logged `current request nonce: '0x0'` means the interface had already been **zeroed** when
//! the callback fired - i.e. by the lost-BAP-connection cleanup. **That failure is downstream of
//! the Weasel, not an independent gate,** and it may vanish entirely once
//! [`crate::conversation::BapConversation`] is wired in. Echo `f1` and `f2` anyway, because a real
//! server surely does; understand that a nonce mismatch cannot be caused by us.
//!
//! Reach: **`static-client-inference`**.

use crate::{optional_varint, parse_proto_fields, u32_field, XetrovDecodeError};

/// Request kind 1 - `session_search`.
pub const MM_KIND_SESSION_SEARCH: u32 = 1;
/// Request kind 2 - `update_advertisement`.
pub const MM_KIND_UPDATE_ADVERTISEMENT: u32 = 2;
/// Request kind 3 - `delete_advertisement`.
pub const MM_KIND_DELETE_ADVERTISEMENT: u32 = 3;
/// ⭐ Request kind 4 - `matchmaking_configuration`. The only kind on the rung-28 path.
pub const MM_KIND_MATCHMAKING_CONFIGURATION: u32 = 4;

/// `[C]` The FNV-1 offset basis, used project-wide as the **"unset" sentinel** - confirmed in code
/// rather than by convention: `0x544F70` and `0x544ED0` both initialise to it.
///
/// `f11.3` (`skill_bucket_hash`) is **omitted** by the client when the bucket equals this.
pub const SKILL_BUCKET_UNSET: u32 = 0x811C_9DC5;

/// `[C]` `fnv1("social")`. Observed as the matchmaking **content** hash at rung 28 - i.e. the
/// activity in play resolved to matchmaking content *"social"*.
pub const MM_CONTENT_HASH_SOCIAL: u32 = 0x5EC0_25C8;

/// `f11` - the matchmaking key. `[C]` shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MatchmakingKey {
    /// `f11.1` - a **content-database activity hash**, not an FNV of any eboot string. The one seen
    /// at rung 28 is `0xDF283AFB`, and it is `[U]`: corpus *and* code-immediate searches were both
    /// empty, with the corpus search positively controlled. Cheaper closers than another scan: read
    /// the record live out of the client's definition manager, or look up 3743483643 in the public
    /// `DestinyActivityDefinition` manifest.
    pub activity_hash: u32,
    /// `f11.2` - e.g. [`MM_CONTENT_HASH_SOCIAL`].
    pub matchmaking_content_hash: u32,
    /// `f11.3` - absent when the bucket is [`SKILL_BUCKET_UNSET`].
    pub skill_bucket_hash: Option<u32>,
}

/// `[C]` id **42** request - **decode only**. All fields are optional and presence-flagged.
///
/// ```text
/// f1  u32   version              always 13 in all four senders `[I]` a service/protocol tag
/// f2  u32   request_kind         1|2|3|4 `[C]`
/// f3  u64   nonce                `[C]`
/// f4  u32   search_slot          `[C]`
/// f5  u32   matchmaking_set_id   `[I]`
/// f6  u32   advertisement_slot   `[C]` (<= 9)
/// f7  msg   session_search_params    (0x1929D70) `[C]` shape, `[U]` semantics
/// f8  msg   session_advertisement    (0x1929DD0) `[C]` shape
/// f9  msg   delete_advertisement { 1: u64 }      `[C]`
/// f10 msg   { 1: u32, 2: u32 }                   `[U]` - no sender sets it
/// f11 msg   matchmaking_key                      `[C]`
/// ```
///
/// Fields 7-10 are **not modelled**. Their shapes are known and their semantics are not, and a
/// struct with named-but-meaningless fields reads as knowledge we do not have. Nothing on the
/// rung-28 path needs them; when something does, model them then.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MatchmakingCoreRequest {
    pub version: Option<u32>,
    pub request_kind: Option<u32>,
    pub nonce: Option<u64>,
    pub search_slot: Option<u32>,
    pub matchmaking_set_id: Option<u32>,
    pub advertisement_slot: Option<u32>,
    pub matchmaking_key: Option<MatchmakingKey>,
    /// `f8.1` - the advertisement id the client repeats in a kind-2 update once the service has
    /// acknowledged its advertisement (`f5.1` of the reply; live-server capture of Restoration,
    /// notes/d1-live-captures-2026-11-05.md). Absent on the first advertisement.
    pub advertisement_id: Option<u64>,
}

impl MatchmakingCoreRequest {
    /// Decode the protobuf body that follows message 42's 6-byte class-1 header.
    ///
    /// Unknown fields are skipped exactly as the client's own reader does.
    pub fn decode(body: &[u8]) -> Result<Self, XetrovDecodeError> {
        let fields = parse_proto_fields(body)?;
        let key = match crate::optional_bytes(&fields, 11)? {
            Some(bytes) => {
                let inner = parse_proto_fields(bytes)?;
                Some(MatchmakingKey {
                    activity_hash: optional_varint(&inner, 1)?
                        .map(|v| u32_field(v, 1))
                        .transpose()?
                        .unwrap_or(0),
                    matchmaking_content_hash: optional_varint(&inner, 2)?
                        .map(|v| u32_field(v, 2))
                        .transpose()?
                        .unwrap_or(0),
                    skill_bucket_hash: optional_varint(&inner, 3)?
                        .map(|v| u32_field(v, 3))
                        .transpose()?,
                })
            }
            None => None,
        };
        let advertisement_id = match crate::optional_bytes(&fields, 8)? {
            Some(bytes) => optional_varint(&parse_proto_fields(bytes)?, 1)?,
            None => None,
        };
        Ok(Self {
            version: optional_varint(&fields, 1)?.map(|v| u32_field(v, 1)).transpose()?,
            request_kind: optional_varint(&fields, 2)?.map(|v| u32_field(v, 2)).transpose()?,
            nonce: optional_varint(&fields, 3)?,
            search_slot: optional_varint(&fields, 4)?.map(|v| u32_field(v, 4)).transpose()?,
            matchmaking_set_id: optional_varint(&fields, 5)?.map(|v| u32_field(v, 5)).transpose()?,
            advertisement_slot: optional_varint(&fields, 6)?
                .map(|v| u32_field(v, 6))
                .transpose()?,
            matchmaking_key: key,
            advertisement_id,
        })
    }

    /// The client builds these; encoding exists so the decoder can be tested against bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::new();
        push_opt_varint(&mut b, 1, self.version.map(u64::from));
        push_opt_varint(&mut b, 2, self.request_kind.map(u64::from));
        push_opt_varint(&mut b, 3, self.nonce);
        push_opt_varint(&mut b, 4, self.search_slot.map(u64::from));
        push_opt_varint(&mut b, 5, self.matchmaking_set_id.map(u64::from));
        push_opt_varint(&mut b, 6, self.advertisement_slot.map(u64::from));
        if let Some(key) = self.matchmaking_key {
            let mut inner = Vec::new();
            push_varint(&mut inner, 1, u64::from(key.activity_hash));
            push_varint(&mut inner, 2, u64::from(key.matchmaking_content_hash));
            if let Some(skill) = key.skill_bucket_hash {
                push_varint(&mut inner, 3, u64::from(skill));
            }
            push_bytes(&mut b, 11, &inner);
        }
        if let Some(id) = self.advertisement_id {
            let mut inner = Vec::new();
            push_varint(&mut inner, 1, id);
            push_bytes(&mut b, 8, &inner);
        }
        b
    }
}

/// `[C]` id **43** response - **encode**. ⭐ The client reads exactly **one** thing for kind 4: the
/// **presence** of field 4. `0x599F80` tests `*(char*)(rsp + 0x1DE0)`, the descriptor's presence
/// flag, and there is no validation of the contents at that site.
///
/// ```text
/// f1  u32   echoed kind        `[I]` - not read by the config path
/// f2  u64   echoed nonce       `[C]` - and NOT read by the nonce check, see the module docs
/// f3  msg   session_search_results (0x1929FD0), f3.1 repeated cap **50**
/// f4  msg   ⭐ matchmaking_configuration (0x192A070), memcpy'd 0x420 bytes to iface+0x38
///           f4.1 tuning  (0x15B48B0): u32 x4, u8, u32 x4, f32, f32
///           f4.2 buckets (0x192A020): f4.2.1 one 0x38 entry, f4.2.2 repeated cap **16**
/// f5  msg   { 1: u64 }         `[I]` advertisement ack
/// f6  msg   { 1: string[0x20], 2: msg 0x40 }
/// ```
///
/// ⚠ Repeated-field capacities are **hard**: an overrun raises `array overflow` and the whole decode
/// fails. That is why this type carries no repeated fields - the minimal reply has none, and adding
/// one without knowing its cap is how a reply becomes a decode failure.
///
/// `[U]` who consumes the 0x420 configuration blob at `iface+0x38`. ⚠ The search that failed to
/// find a consumer had **no positive control**, so that is `[U]`, not "nobody" - and it blocks
/// turning a *minimum-viable* 43 into a *correct* 43.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MatchmakingCoreResponse {
    /// `f1`. Echo the request's kind.
    pub kind: Option<u32>,
    /// `f2`. Echo the request's nonce.
    pub nonce: Option<u64>,
    /// `f4`. **Presence is the entire contract for kind 4.**
    ///
    /// `Some(raw)` emits `f4` with `raw` as its body; [`Self::minimal_configuration`] uses the
    /// non-empty form `0A 00` (an empty `f4.1` tuning submessage).
    ///
    /// `[I]` on whether an *empty* `f4` (`22 00`) would set the presence flag too: that depends on
    /// the generic parse loop `0xA17D0` rather than the nested-message handler `0xA2920`, and the
    /// latter is what was disassembled. The non-empty form removes the risk at no cost.
    pub configuration: Option<Vec<u8>>,
    /// `f3` raw body: the session search results. The live services answered a search with
    /// `{1: one candidate session, 2: 400}`; [`Self::search_found_none`] sends `{2: 400}`.
    pub search_results: Option<Vec<u8>>,
    /// `f5.1`: the advertisement id that acknowledges a kind-2 advertisement. The live services
    /// gave one per advertisement, and echoed it to every update that repeated it.
    pub advertisement_ack: Option<u64>,
}

/// The live `f3.2` beside the search results (`0x190`). `[U]` meaning.
pub const SEARCH_RESULTS_F2: u64 = 400;

impl MatchmakingCoreResponse {
    /// ⭐ A kind-1 search answered with no sessions found: the solo-lab answer, after which the
    /// client creates its own public session (public-session plan step 3). The live services sent
    /// `f3 = {1: candidate, 2: 400}`; this keeps `f3.2` and leaves the candidates out. `[I]`.
    pub fn search_found_none(nonce: u64) -> Self {
        let mut f3 = Vec::new();
        push_varint(&mut f3, 2, SEARCH_RESULTS_F2);
        Self { kind: Some(MM_KIND_SESSION_SEARCH), nonce: Some(nonce), search_results: Some(f3), ..Self::default() }
    }

    /// ⭐ A kind-2 advertisement acknowledged with `id` (`f5 = {1: id}`), as the live services did.
    pub fn advertisement_acknowledged(nonce: u64, id: u64) -> Self {
        Self { kind: Some(MM_KIND_UPDATE_ADVERTISEMENT), nonce: Some(nonce), advertisement_ack: Some(id), ..Self::default() }
    }

    /// ⭐ The minimal safe stub: echo the kind and nonce, and emit an `f4` whose only content is an
    /// empty `f4.1`. On the wire the load-bearing part is four bytes: `22 02 0A 00`.
    pub fn minimal_configuration(kind: u32, nonce: u64) -> Self {
        Self { kind: Some(kind), nonce: Some(nonce), configuration: Some(vec![0x0A, 0x00]), ..Self::default() }
    }

    /// The reply to a decoded request, echoing both fields the client sent.
    pub fn echoing(request: &MatchmakingCoreRequest) -> Self {
        Self::minimal_configuration(
            request.request_kind.unwrap_or(MM_KIND_MATCHMAKING_CONFIGURATION),
            request.nonce.unwrap_or(0),
        )
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::new();
        push_opt_varint(&mut b, 1, self.kind.map(u64::from));
        push_opt_varint(&mut b, 2, self.nonce);
        if let Some(results) = &self.search_results {
            push_bytes(&mut b, 3, results);
        }
        if let Some(config) = &self.configuration {
            push_bytes(&mut b, 4, config);
        }
        if let Some(id) = self.advertisement_ack {
            let mut inner = Vec::new();
            push_varint(&mut inner, 1, id);
            push_bytes(&mut b, 5, &inner);
        }
        b
    }

    pub fn decode(body: &[u8]) -> Result<Self, XetrovDecodeError> {
        let fields = parse_proto_fields(body)?;
        Ok(Self {
            kind: optional_varint(&fields, 1)?.map(|v| u32_field(v, 1)).transpose()?,
            nonce: optional_varint(&fields, 2)?,
            configuration: crate::optional_bytes(&fields, 4)?.map(<[u8]>::to_vec),
            search_results: crate::optional_bytes(&fields, 3)?.map(<[u8]>::to_vec),
            advertisement_ack: match crate::optional_bytes(&fields, 5)? {
                Some(bytes) => optional_varint(&parse_proto_fields(bytes)?, 1)?,
                None => None,
            },
        })
    }
}

fn push_varint(out: &mut Vec<u8>, field: u32, mut value: u64) {
    push_tag(out, field, 0);
    loop {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn push_opt_varint(out: &mut Vec<u8>, field: u32, value: Option<u64>) {
    if let Some(value) = value {
        push_varint(out, field, value);
    }
}

fn push_bytes(out: &mut Vec<u8>, field: u32, value: &[u8]) {
    push_tag(out, field, 2);
    let mut len = value.len() as u64;
    loop {
        let byte = (len & 0x7F) as u8;
        len >>= 7;
        if len == 0 {
            out.push(byte);
            break;
        }
        out.push(byte | 0x80);
    }
    out.extend_from_slice(value);
}

fn push_tag(out: &mut Vec<u8>, field: u32, wire: u8) {
    let mut tag = u64::from(field) << 3 | u64::from(wire);
    loop {
        let byte = (tag & 0x7F) as u8;
        tag >>= 7;
        if tag == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

#[cfg(test)]
mod tests {
    /// ⭐ The live services' reply to the first advertisement in the Restoration capture (body
    /// after message 43's header): kind 2, the request's nonce, and `f5 = {1: 0x33CE1A5}`.
    #[test]
    fn an_advertisement_ack_encodes_as_the_live_reply() {
        let live = [0x08, 0x02, 0x10, 0xa6, 0xb1, 0xb5, 0xb3, 0xad, 0xc5, 0xc0, 0xfd, 0xa1, 0x01, 0x2a, 0x05, 0x08,
                    0xa5, 0xc3, 0xf3, 0x19];
        let rsp = MatchmakingCoreResponse::decode(&live).unwrap();
        assert_eq!(rsp.advertisement_ack, Some(0x33C_E1A5));
        assert_eq!(MatchmakingCoreResponse::advertisement_acknowledged(rsp.nonce.unwrap(), 0x33C_E1A5).encode(), live);
    }

    /// A search with nothing found keeps the live `f3.2` and carries no candidates.
    #[test]
    fn a_search_found_none_carries_only_f3_2() {
        let b = MatchmakingCoreResponse::search_found_none(7).encode();
        assert_eq!(b, [0x08, 0x01, 0x10, 0x07, 0x1a, 0x03, 0x10, 0x90, 0x03]);
        assert_eq!(MatchmakingCoreResponse::decode(&b).unwrap().search_results, Some(vec![0x10, 0x90, 0x03]));
    }

    /// An update repeats the acknowledged id in `f8.1`; a first advertisement has none.
    #[test]
    fn the_advertisement_id_round_trips_in_f8() {
        let req = MatchmakingCoreRequest { request_kind: Some(2), nonce: Some(9), advertisement_id: Some(0x33C_E1A5), ..Default::default() };
        assert_eq!(MatchmakingCoreRequest::decode(&req.encode()).unwrap().advertisement_id, Some(0x33C_E1A5));
        let first = MatchmakingCoreRequest { request_kind: Some(2), nonce: Some(9), ..Default::default() };
        assert_eq!(MatchmakingCoreRequest::decode(&first.encode()).unwrap().advertisement_id, None);
    }

    use super::*;

    #[test]
    fn request_round_trips() {
        let req = MatchmakingCoreRequest {
            version: Some(13),
            request_kind: Some(MM_KIND_MATCHMAKING_CONFIGURATION),
            nonce: Some(0x0123_4567_89AB_CDEF),
            search_slot: Some(0),
            matchmaking_set_id: Some(2),
            advertisement_slot: Some(9),
            matchmaking_key: Some(MatchmakingKey {
                activity_hash: 0xDF28_3AFB,
                matchmaking_content_hash: MM_CONTENT_HASH_SOCIAL,
                skill_bucket_hash: None,
            }),
        advertisement_id: None,
    };
        let bytes = req.encode();
        assert_eq!(MatchmakingCoreRequest::decode(&bytes), Ok(req));
        assert_eq!(MatchmakingCoreRequest::decode(&bytes).unwrap().encode(), bytes);
    }

    /// `f11.3` is omitted when the skill bucket is the FNV-1 offset basis. That is the client's
    /// behaviour, so a decoder that invented a `Some(0x811C9DC5)` would be reporting a field the
    /// client did not send.
    #[test]
    fn an_absent_skill_bucket_stays_absent() {
        let bytes = MatchmakingCoreRequest {
            matchmaking_key: Some(MatchmakingKey {
                activity_hash: 1,
                matchmaking_content_hash: 2,
                skill_bucket_hash: None,
            }),
            ..Default::default()
        }
        .encode();
        let back = MatchmakingCoreRequest::decode(&bytes).unwrap();
        assert_eq!(back.matchmaking_key.unwrap().skill_bucket_hash, None);
        assert_eq!(SKILL_BUCKET_UNSET, 0x811C_9DC5);
    }

    /// ⭐ The four bytes that are the whole of Phase 1's minimum: field 4, length 2, containing an
    /// empty field 4.1. `0x599F80` reads nothing but the presence flag they set.
    #[test]
    fn the_minimal_response_carries_the_four_load_bearing_bytes() {
        let rsp = MatchmakingCoreResponse::minimal_configuration(
            MM_KIND_MATCHMAKING_CONFIGURATION,
            0xAABB,
        );
        let bytes = rsp.encode();
        let needle = [0x22u8, 0x02, 0x0A, 0x00];
        assert!(
            bytes.windows(4).any(|w| w == needle),
            "the f4 presence encoding must be present: {bytes:02x?}"
        );
        assert_eq!(MatchmakingCoreResponse::decode(&bytes), Ok(rsp));
    }

    #[test]
    fn the_response_echoes_the_requests_kind_and_nonce() {
        let req = MatchmakingCoreRequest {
            request_kind: Some(MM_KIND_MATCHMAKING_CONFIGURATION),
            nonce: Some(0xDEAD_BEEF_CAFE_F00D),
            ..Default::default()
        };
        let rsp = MatchmakingCoreResponse::echoing(&req);
        assert_eq!(rsp.kind, Some(4));
        assert_eq!(rsp.nonce, Some(0xDEAD_BEEF_CAFE_F00D));
    }

    /// The pairing this module claims, against the table walked out of the policy vtables.
    #[test]
    fn forty_two_pairs_with_forty_three_not_thirty_four_with_thirty_five() {
        use crate::tables::{response_for, ResponsePolicy};
        assert_eq!(response_for(42), ResponsePolicy::Expects(43));
        // The skill policy is a different message pair and is nobody's configuration request.
        assert_eq!(response_for(34), ResponsePolicy::Expects(35));
    }

    /// Presence bits are part of the request contract: omitted fields and present zeros differ.
    /// These literals are independent of the production protobuf field writer.
    #[test]
    fn request_zero_presence_and_integer_limits_round_trip() {
        let absent = MatchmakingCoreRequest::default();
        assert!(absent.encode().is_empty());
        assert_eq!(MatchmakingCoreRequest::decode(&[]), Ok(absent));

        let zeros = MatchmakingCoreRequest {
            version: Some(0),
            request_kind: Some(0),
            nonce: Some(0),
            search_slot: Some(0),
            matchmaking_set_id: Some(0),
            advertisement_slot: Some(0),
            matchmaking_key: Some(MatchmakingKey {
                activity_hash: 0,
                matchmaking_content_hash: 0,
                skill_bucket_hash: Some(0),
            }),
        advertisement_id: None,
    };
        let zero_wire: [u8; 20] = [
            0x08, 0x00, 0x10, 0x00, 0x18, 0x00, 0x20, 0x00, 0x28, 0x00, 0x30, 0x00, 0x5a, 0x06,
            0x08, 0x00, 0x10, 0x00, 0x18, 0x00,
        ];
        assert_eq!(zeros.encode().as_slice(), zero_wire);
        assert_eq!(MatchmakingCoreRequest::decode(&zero_wire), Ok(zeros));

        let maximum = MatchmakingCoreRequest {
            version: Some(u32::MAX),
            request_kind: Some(u32::MAX),
            nonce: Some(u64::MAX),
            search_slot: Some(u32::MAX),
            matchmaking_set_id: Some(u32::MAX),
            advertisement_slot: Some(u32::MAX),
            matchmaking_key: Some(MatchmakingKey {
                activity_hash: u32::MAX,
                matchmaking_content_hash: u32::MAX,
                skill_bucket_hash: Some(u32::MAX),
            }),
        advertisement_id: None,
    };
        assert_eq!(
            MatchmakingCoreRequest::decode(&maximum.encode()),
            Ok(maximum)
        );
    }

    /// The response codec must not collapse missing f4 into a present, empty submessage.
    /// The large size is a parser exercise, NOT a claim that the client accepts this payload.
    #[test]
    fn response_configuration_presence_and_length_boundaries_round_trip() {
        let missing = MatchmakingCoreResponse::default();
        assert!(missing.encode().is_empty());
        assert_eq!(MatchmakingCoreResponse::decode(&[]), Ok(missing));

        for (size, header) in [
            (0usize, vec![0x22, 0x00]),
            (1, vec![0x22, 0x01]),
            (127, vec![0x22, 0x7f]),
            (128, vec![0x22, 0x80, 0x01]),
            (65_535, vec![0x22, 0xff, 0xff, 0x03]),
        ] {
            let response = MatchmakingCoreResponse {
                kind: None,
                nonce: None,
                configuration: Some(vec![0xa5; size]),
                search_results: None,
                advertisement_ack: None,
            };
            let wire = response.encode();
            assert!(wire.starts_with(&header), "wrong length prefix for {size}");
            assert_eq!(wire.len(), header.len() + size);
            assert_eq!(MatchmakingCoreResponse::decode(&wire), Ok(response));
        }

        let max_echo = MatchmakingCoreResponse {
            kind: Some(u32::MAX),
            nonce: Some(u64::MAX),
            configuration: Some(vec![0x0a, 0x00]),
            search_results: None,
            advertisement_ack: None,
        };
        assert_eq!(
            MatchmakingCoreResponse::decode(&max_echo.encode()),
            Ok(max_echo)
        );
    }

    /// No truncated value, duplicate singular field, wrong wire type or overflowing u32
    /// may silently turn into a valid request/response.
    #[test]
    fn malformed_matchmaking_fields_are_rejected() {
        let bad_requests: &[&[u8]] = &[
            &[0x08],                               // missing f1 value
            &[0x08, 0x80],                         // unfinished varint
            &[0x08, 0x00, 0x08, 0x01],             // duplicate f1
            &[0x0a, 0x00],                         // f1 with bytes, not varint
            &[0x08, 0x80, 0x80, 0x80, 0x80, 0x10], // f1 = 2^32
            &[0x5a, 0x01, 0x08],                   // truncated nested f11.1
        ];
        for &wire in bad_requests {
            assert!(MatchmakingCoreRequest::decode(wire).is_err(), "{wire:02x?}");
        }
        let bad_responses: &[&[u8]] = &[
            &[0x22, 0x81],                         // truncated length
            &[0x22, 0x03, 0xaa],                   // truncated f4 payload
            &[0x08, 0x01, 0x08, 0x02],             // duplicate f1
            &[0x0a, 0x00],                         // f1 with bytes, not varint
            &[0x08, 0x80, 0x80, 0x80, 0x80, 0x10], // f1 = 2^32
        ];
        for &wire in bad_responses {
            assert!(
                MatchmakingCoreResponse::decode(wire).is_err(),
                "{wire:02x?}"
            );
        }
    }

    #[test]
    fn unknown_matchmaking_fields_are_ignored_without_losing_known_fields() {
        let request = MatchmakingCoreRequest {
            version: Some(13),
            nonce: Some(42),
            ..Default::default()
        };
        let mut request_wire = request.encode();
        request_wire.extend_from_slice(&[0x78, 0x7f]); // unknown f15 varint
        assert_eq!(MatchmakingCoreRequest::decode(&request_wire), Ok(request));

        let response = MatchmakingCoreResponse {
            kind: Some(4),
            nonce: Some(42),
            configuration: Some(vec![0x0a, 0x00]),
            search_results: None,
            advertisement_ack: None,
        };
        let mut response_wire = response.encode();
        response_wire.extend_from_slice(&[0x78, 0x7f]);
        assert_eq!(
            MatchmakingCoreResponse::decode(&response_wire),
            Ok(response)
        );
    }
}
