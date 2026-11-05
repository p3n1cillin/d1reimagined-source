// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The session halves of messages **42 / 43**: what a search asks for (`f7`), what an
//! advertisement carries (`f8`), what a delete names (`f9`), and what the service answers (`f3`
//! search results, `f5` acknowledgements, `f4` configuration).
//!
//! [`crate::matchmaking`] models the fields the configuration path reads. This module adds the
//! parts the session directory needs, read from the private live-server captures (Xbox 360 build
//! 51539, 1,011 exchanges across 52 captures; notes/ps4-56578-multiplayer-2026-11-05.md). Their
//! shapes, by field number:
//!
//! ```text
//! 42 request
//!   f4  search slot: 0 the activity (fireteam) session, 1 the public bubble session
//!   f5  matchmaking set: the activity hash for slot 0, a bubble id (0x8001_00xx) for slot 1
//!   f7  search {1: attributes, 2: u32 configuration hash}
//!   f8  advertisement {1: u64 id (absent on the first), 2: {1: attributes, 2: session, 3: name},
//!                      3: u32 configuration hash}
//!   f9  delete {1: u64 advertisement id}
//! attributes
//!   f3 u32 (3 in searches, 1 in advertisements)   f5 u32 0|1   f7 u32 open slots
//!   f8 location   f11 f32   f13 u32 search elapsed (ms, searches; grows ~5,200 per retry)
//! 43 response
//!   f3  {1: repeated candidate {1: session, 2: name, 3: f32, 5: f32}, 2: u32 (100..400)}
//!   f4  configuration {1: tuning, 2: buckets}
//!   f5  {1: u64 advertisement id}, or present and EMPTY for a delete (`2a 00`)
//! ```
//!
//! The session (`f8.2.2`) and name (`f8.2.3`) are the advertiser's: the platform session and its
//! display name. They are personal data in the captures and opaque to the service, which stores
//! them and hands them back in a search result **byte for byte**. On build 56578 their contents
//! are the PS4 client's and are not modelled here. `[I]` that the 56578 descriptors keep the
//! 51539 field numbers: the codec in [`crate::matchmaking`] already decodes both builds' `f1`-`f6`
//! and `f11` alike.

use crate::matchmaking::{push_bytes, push_fixed32, push_varint};
use crate::{optional_bytes, optional_varint, parse_proto_fields, u32_field, ProtoValue, XetrovDecodeError};

/// `f4` value of a search or advertisement made for the activity session.
pub const SLOT_ACTIVITY: u32 = 0;
/// `f4` value of a search or advertisement made for a public bubble.
pub const SLOT_BUBBLE: u32 = 1;

/// The attributes record (`f7.1` of a search, `f8.2.1` of an advertisement). Every field is
/// optional, and the raw bytes are kept so nothing the client said is lost.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SessionAttributes {
    /// The record exactly as the client sent it.
    pub raw: Vec<u8>,
    /// `f3`: 3 in every search, 1 in every advertisement captured. `[U]` meaning.
    pub role: Option<u32>,
    /// `f5`: 0 or 1. `[U]`; in advertisements it turned 1 once the session was in play.
    pub flag_5: Option<u32>,
    /// `f7`: slots. In an advertisement, the slots still open (2 for a solo player's strike,
    /// 6 for a fresh bubble); in a bubble search, the slots the searcher needs (3: its fireteam's
    /// reservation, Bungie GDC 2015). Absent in activity searches.
    pub slots: Option<u32>,
    /// `f13`: in a search, milliseconds since the search began (0, ~5,200, ~10,400 ...).
    pub elapsed_ms: Option<u64>,
}

impl SessionAttributes {
    pub fn decode(raw: &[u8]) -> Result<Self, XetrovDecodeError> {
        let f = parse_proto_fields(raw)?;
        Ok(Self {
            raw: raw.to_vec(),
            role: optional_varint(&f, 3)?.map(|v| u32_field(v, 3)).transpose()?,
            flag_5: optional_varint(&f, 5)?.map(|v| u32_field(v, 5)).transpose()?,
            slots: optional_varint(&f, 7)?.map(|v| u32_field(v, 7)).transpose()?,
            elapsed_ms: optional_varint(&f, 13)?,
        })
    }
}

/// `f7` of a kind-1 request.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SearchParams {
    pub attributes: Option<SessionAttributes>,
    /// `f7.2`: the hash of the configuration this search runs under (`f4.1.1` of the kind-4
    /// reply: the strike configuration for strike searches, the social one for bubbles).
    pub configuration_hash: Option<u32>,
}

/// `f8` of a kind-2 request.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Advertisement {
    /// `f8.1`: the id the service gave this advertisement. Absent on the first.
    pub id: Option<u64>,
    pub attributes: Option<SessionAttributes>,
    /// `f8.2.2`, the whole submessage body: the platform session the advertiser hosts.
    pub session: Option<Vec<u8>>,
    /// `f8.2.3`: the advertiser's display name, as bytes.
    pub name: Option<Vec<u8>>,
    /// `f8.3`: the configuration hash (0 before the configuration arrived).
    pub configuration_hash: Option<u32>,
}

/// The search parameters of a 42 body, if it has any.
pub fn decode_search(body: &[u8]) -> Result<Option<SearchParams>, XetrovDecodeError> {
    let fields = parse_proto_fields(body)?;
    let Some(f7) = optional_bytes(&fields, 7)? else { return Ok(None) };
    let inner = parse_proto_fields(f7)?;
    Ok(Some(SearchParams {
        attributes: optional_bytes(&inner, 1)?.map(SessionAttributes::decode).transpose()?,
        configuration_hash: optional_varint(&inner, 2)?.map(|v| u32_field(v, 2)).transpose()?,
    }))
}

/// The advertisement of a 42 body, if it has one.
pub fn decode_advertisement(body: &[u8]) -> Result<Option<Advertisement>, XetrovDecodeError> {
    let fields = parse_proto_fields(body)?;
    let Some(f8) = optional_bytes(&fields, 8)? else { return Ok(None) };
    let inner = parse_proto_fields(f8)?;
    let mut ad = Advertisement {
        id: optional_varint(&inner, 1)?,
        configuration_hash: optional_varint(&inner, 3)?.map(|v| u32_field(v, 3)).transpose()?,
        ..Advertisement::default()
    };
    if let Some(desc) = optional_bytes(&inner, 2)? {
        let d = parse_proto_fields(desc)?;
        ad.attributes = match optional_bytes(&d, 1)? {
            Some(a) if !a.is_empty() => Some(SessionAttributes::decode(a)?),
            _ => None,
        };
        ad.session = optional_bytes(&d, 2)?.map(<[u8]>::to_vec);
        ad.name = optional_bytes(&d, 3)?.map(<[u8]>::to_vec);
    }
    Ok(Some(ad))
}

/// The advertisement id a kind-3 delete names (`f9.1`).
pub fn decode_delete(body: &[u8]) -> Result<Option<u64>, XetrovDecodeError> {
    let fields = parse_proto_fields(body)?;
    match optional_bytes(&fields, 9)? {
        Some(f9) => optional_varint(&parse_proto_fields(f9)?, 1),
        None => Ok(None),
    }
}

/// One `f3.1` search result.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchCandidate {
    /// The advertised session, verbatim (`f8.2.2` of the advertisement).
    pub session: Vec<u8>,
    /// The advertiser's name, verbatim (`f8.2.3`).
    pub name: Vec<u8>,
    /// `f3`. Live values ran 0.906-1.0. `[U]` meaning.
    pub score: f32,
    /// `f5`. Live values were 0.99 or 1.0. `[U]` meaning.
    pub score_5: f32,
}

/// The repeated-field capacity of `f3.1` (descriptor `0x1929FD0`); more is a decode failure.
pub const SEARCH_RESULTS_CAPACITY: usize = 50;

/// The body of a 43's `f3`: `candidates` then `f2 = level`.
///
/// Refuses (returns `None`) above [`SEARCH_RESULTS_CAPACITY`] candidates: the client's decoder
/// raises "array overflow" and fails the whole reply.
pub fn encode_search_results(candidates: &[SearchCandidate], level: u64) -> Option<Vec<u8>> {
    if candidates.len() > SEARCH_RESULTS_CAPACITY {
        return None;
    }
    let mut out = Vec::new();
    for c in candidates {
        let mut cand = Vec::new();
        push_bytes(&mut cand, 1, &c.session);
        push_bytes(&mut cand, 2, &c.name);
        push_fixed32(&mut cand, 3, c.score.to_bits());
        push_fixed32(&mut cand, 5, c.score_5.to_bits());
        push_bytes(&mut out, 1, &cand);
    }
    push_varint(&mut out, 2, level);
    Some(out)
}

/// The search results of a 43 body: `(candidates, level)`.
pub fn decode_search_results(body: &[u8]) -> Result<Option<(Vec<SearchCandidate>, Option<u64>)>, XetrovDecodeError> {
    let fields = parse_proto_fields(body)?;
    let Some(f3) = optional_bytes(&fields, 3)? else { return Ok(None) };
    let inner = parse_proto_fields(f3)?;
    let mut out = Vec::new();
    for f in inner.iter().filter(|f| f.number == 1) {
        let ProtoValue::Bytes(c) = f.value else {
            return Err(XetrovDecodeError::WrongWireType { field: 1, got: f.wire, expected: 2 });
        };
        let cf = parse_proto_fields(c)?;
        out.push(SearchCandidate {
            session: optional_bytes(&cf, 1)?.unwrap_or_default().to_vec(),
            name: optional_bytes(&cf, 2)?.unwrap_or_default().to_vec(),
            score: fixed32(c, 3).map(f32::from_bits).unwrap_or(0.0),
            score_5: fixed32(c, 5).map(f32::from_bits).unwrap_or(0.0),
        });
    }
    Ok(Some((out, optional_varint(&inner, 2)?)))
}

/// The 43 body that acknowledges a kind-3 delete: kind, nonce, and an empty `f5`, as the live
/// services answered every delete (`08 03 10 <nonce> 2a 00`).
pub fn delete_acknowledged(nonce: u64) -> Vec<u8> {
    let mut b = Vec::new();
    push_varint(&mut b, 1, u64::from(crate::matchmaking::MM_KIND_DELETE_ADVERTISEMENT));
    push_varint(&mut b, 2, nonce);
    push_bytes(&mut b, 5, &[]);
    b
}

/// A protobuf value tree, for configuration replies built from data rather than code.
#[derive(Debug, Clone, PartialEq)]
pub enum ProtoTree {
    Varint(u32, u64),
    Fixed32(u32, f32),
    Message(u32, Vec<ProtoTree>),
}

impl ProtoTree {
    pub fn encode_all(items: &[ProtoTree]) -> Vec<u8> {
        let mut out = Vec::new();
        for item in items {
            item.encode_into(&mut out);
        }
        out
    }

    fn encode_into(&self, out: &mut Vec<u8>) {
        match self {
            Self::Varint(n, v) => push_varint(out, *n, *v),
            Self::Fixed32(n, v) => push_fixed32(out, *n, v.to_bits()),
            Self::Message(n, inner) => push_bytes(out, *n, &Self::encode_all(inner)),
        }
    }
}

/// The 4 bytes of a wire-type-5 field `number`, scanning `msg` (the shared parser does not keep them).
fn fixed32(msg: &[u8], number: u32) -> Option<u32> {
    let mut pos = 0usize;
    while pos < msg.len() {
        let tag = crate::read_proto_varint(msg, &mut pos).ok()?;
        let (n, wire) = ((tag >> 3) as u32, (tag & 7) as u8);
        match wire {
            0 => {
                crate::read_proto_varint(msg, &mut pos).ok()?;
            }
            1 => pos += 8,
            2 => {
                let len = crate::read_proto_varint(msg, &mut pos).ok()? as usize;
                pos += len;
            }
            5 => {
                let v = u32::from_le_bytes(msg.get(pos..pos + 4)?.try_into().ok()?);
                if n == number {
                    return Some(v);
                }
                pos += 4;
            }
            _ => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matchmaking::{MatchmakingCoreRequest, MatchmakingCoreResponse};

    /// A synthetic advertisement in the captured shape (no capture bytes: the session and name
    /// here are placeholders).
    fn advertisement_body(id: Option<u64>, slots: u32) -> Vec<u8> {
        let mut attrs = Vec::new();
        push_varint(&mut attrs, 3, 1);
        push_varint(&mut attrs, 5, 0);
        push_varint(&mut attrs, 7, u64::from(slots));
        push_varint(&mut attrs, 13, 33);
        let mut session = Vec::new();
        push_bytes(&mut session, 1, &[0xAB; 60]);
        let mut desc = Vec::new();
        push_bytes(&mut desc, 1, &attrs);
        push_bytes(&mut desc, 2, &session);
        push_bytes(&mut desc, 3, b"Guardian-01");
        let mut f8 = Vec::new();
        if let Some(id) = id {
            push_varint(&mut f8, 1, id);
        }
        push_bytes(&mut f8, 2, &desc);
        push_varint(&mut f8, 3, 0xF1B7_AD41);
        let mut body = MatchmakingCoreRequest {
            version: Some(13),
            request_kind: Some(2),
            nonce: Some(5),
            search_slot: Some(0),
            matchmaking_set_id: Some(0x1234),
            ..Default::default()
        }
        .encode();
        // f8 sits between f6 and f11 on the wire; the reader does not care about order.
        push_bytes(&mut body, 8, &f8);
        body
    }

    #[test]
    fn an_advertisement_keeps_its_session_and_name_verbatim() {
        let ad = decode_advertisement(&advertisement_body(Some(77), 2)).unwrap().unwrap();
        assert_eq!(ad.id, Some(77));
        assert_eq!(ad.configuration_hash, Some(0xF1B7_AD41));
        assert_eq!(ad.name.as_deref(), Some(&b"Guardian-01"[..]));
        let session = ad.session.unwrap();
        assert_eq!(session.len(), 62, "the whole {{1: 60 bytes}} submessage body");
        let attrs = ad.attributes.unwrap();
        assert_eq!((attrs.role, attrs.slots, attrs.elapsed_ms), (Some(1), Some(2), Some(33)));
        // The base codec still reads the same body, and sees the advertisement id too.
        let req = MatchmakingCoreRequest::decode(&advertisement_body(Some(77), 2)).unwrap();
        assert_eq!(req.advertisement_id, Some(77));
    }

    #[test]
    fn a_first_advertisement_has_no_id() {
        assert_eq!(decode_advertisement(&advertisement_body(None, 6)).unwrap().unwrap().id, None);
    }

    #[test]
    fn a_search_reads_its_slots_elapsed_time_and_configuration() {
        let mut attrs = Vec::new();
        push_varint(&mut attrs, 3, 3);
        push_varint(&mut attrs, 7, 3);
        push_varint(&mut attrs, 13, 15_033);
        let mut f7 = Vec::new();
        push_bytes(&mut f7, 1, &attrs);
        push_varint(&mut f7, 2, 0xFAB9_FD14);
        let mut body = Vec::new();
        push_varint(&mut body, 2, 1);
        push_bytes(&mut body, 7, &f7);
        let s = decode_search(&body).unwrap().unwrap();
        assert_eq!(s.configuration_hash, Some(0xFAB9_FD14));
        let a = s.attributes.unwrap();
        assert_eq!((a.role, a.slots, a.elapsed_ms), (Some(3), Some(3), Some(15_033)));
        assert_eq!(decode_search(&[0x08, 0x01]).unwrap(), None);
    }

    #[test]
    fn a_delete_names_its_advertisement() {
        let mut f9 = Vec::new();
        push_varint(&mut f9, 1, 0x347_A74A);
        let mut body = Vec::new();
        push_varint(&mut body, 2, 3);
        push_bytes(&mut body, 9, &f9);
        assert_eq!(decode_delete(&body).unwrap(), Some(0x347_A74A));
    }

    /// The live delete acknowledgement carries `f5` present and empty. The base response codec
    /// cannot express that (it has `Option<u64>` for the id), which is why this builder exists.
    #[test]
    fn a_delete_acknowledgement_ends_in_an_empty_f5() {
        let b = delete_acknowledged(0x0A);
        assert_eq!(b, [0x08, 0x03, 0x10, 0x0A, 0x2A, 0x00]);
        let r = MatchmakingCoreResponse::decode(&b).unwrap();
        assert_eq!((r.kind, r.nonce, r.advertisement_ack), (Some(3), Some(10), None));
    }

    #[test]
    fn search_results_round_trip_with_their_level() {
        let c = SearchCandidate { session: vec![0x0A, 0x02, 1, 2], name: b"x".to_vec(), score: 0.99, score_5: 1.0 };
        let f3 = encode_search_results(&[c.clone()], 400).unwrap();
        let body = MatchmakingCoreResponse { kind: Some(1), nonce: Some(9), search_results: Some(f3), ..Default::default() }.encode();
        let (cands, level) = decode_search_results(&body).unwrap().unwrap();
        assert_eq!(level, Some(400));
        assert_eq!(cands, vec![c]);
        // None found: only f2, which is MatchmakingCoreResponse::search_found_none's shape.
        assert_eq!(encode_search_results(&[], 400).unwrap(), [0x10, 0x90, 0x03]);
    }

    #[test]
    fn more_candidates_than_the_descriptor_holds_are_refused() {
        let c = SearchCandidate { session: vec![], name: vec![], score: 1.0, score_5: 1.0 };
        assert!(encode_search_results(&vec![c.clone(); SEARCH_RESULTS_CAPACITY], 400).is_some());
        assert!(encode_search_results(&vec![c; SEARCH_RESULTS_CAPACITY + 1], 400).is_none());
    }

    #[test]
    fn a_proto_tree_encodes_floats_little_endian() {
        let b = ProtoTree::encode_all(&[ProtoTree::Message(1, vec![ProtoTree::Varint(2, 6), ProtoTree::Fixed32(11, 0.7)])]);
        assert_eq!(b, [0x0A, 0x07, 0x10, 0x06, 0x5D, 0x33, 0x33, 0x33, 0x3F]);
    }
}
