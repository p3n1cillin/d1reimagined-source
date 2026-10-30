// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! Controls - run these before citing anything this crate says.
//!
//! Same contract as `d1-eboot controls`: every check is named, the run prints one line per check,
//! and **exit 2 means a control failed and the output may not be cited**.
//!
//! What they are and are not: these are *pinned-fact* controls over the crate's own tables and its
//! agreement with `d1-bap`. They do **not** re-read the eboot - this crate has no ELF reader and
//! must not grow one (`d1-eboot` owns that). The eboot-side derivations each carry their exact
//! `d1-eboot` invocation in the doc-comment above the constant they produced, so they are
//! re-runnable by hand; §7 of the report names the ones that should move into
//! `d1-eboot bap-header-classes` when someone adds it.
//!
//! Several controls are **negative-with-a-positive-control**: "message 100 is refused" is only
//! meaningful next to "message 123 is accepted", because a `push` that refused everything would
//! pass the first half.

use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Instant;

use crate::codec::{AhWireCodec, RawStructCodec, SchemaCodec};
use crate::entity::{EntitySlotAllocator, EntitySlotMask, ENTITY_INDEX_COUNT, ENTITY_MASK_BYTES};
use crate::messages::{
    AhMessage, ClientMessage, GlobalActivityState, JoinResult, ReplicateMembership,
};
use crate::protocol;
use crate::push::{AhPush, PushBlocked, CLASS6_HEADER_FIELDS};
use crate::registry::HostRegistry;
use crate::state::{ActivityInstance, InstanceState};

/// One control's outcome.
pub struct Control {
    pub name: &'static str,
    pub passed: bool,
    pub detail: String,
}

fn ok(name: &'static str, detail: impl Into<String>) -> Control {
    Control { name, passed: true, detail: detail.into() }
}

fn check(name: &'static str, cond: bool, detail: impl Into<String>) -> Control {
    Control { name, passed: cond, detail: detail.into() }
}

/// Run every control. The caller decides the exit code.
pub fn run() -> Vec<Control> {
    let mut c = Vec::new();

    // ── the id space ──────────────────────────────────────────────────────────────────────────

    c.push(check(
        "the activity-host id space is the 49-entry table at 0x18F8780",
        protocol::AH_MESSAGE_NAMES.len() == 49
            && protocol::AH_MESSAGE_NAMES[0] == "entity_slots_allocated"
            && protocol::AH_MESSAGE_NAMES[48] == "refresh_inspirations",
        format!("{} names, [0]={}", protocol::AH_MESSAGE_NAMES.len(), protocol::AH_MESSAGE_NAMES[0]),
    ));

    c.push(check(
        "the client's receive dispatch table (0x18F84A0) has 17 entries",
        protocol::AH_TO_CLIENT.len() == 17,
        format!("{:?}", protocol::AH_TO_CLIENT),
    ));

    c.push(check(
        "TX and RX are disjoint except `18 incident`",
        {
            let both: Vec<u8> = protocol::AH_TO_CLIENT
                .iter()
                .copied()
                .filter(|i| protocol::CLIENT_TO_AH.contains(i))
                .collect();
            both == vec![18]
        },
        "one schema, both directions",
    ));

    // ⭐ The control that ties `user_defined_type` to this table at all.
    c.push(check(
        "udt 8/10 pin the table to BAP messages 6/7 (two independent facts, sizes agree)",
        protocol::name(8) == Some("request_activity_host")
            && protocol::name(10) == Some("start_activity_host_response")
            && d1_bap::activity_host::AH_MANAGER_RESPONSE_BODY_LEN
                == 1 + protocol::START_ACTIVITY_HOST_RESPONSE_SIZE,
        format!(
            "BAP 7 body = 1 + 0x{:X}; AH udt 10 = 0x{:X}",
            protocol::START_ACTIVITY_HOST_RESPONSE_SIZE,
            protocol::START_ACTIVITY_HOST_RESPONSE_SIZE
        ),
    ));

    c.push(check(
        "udt 16 keepalive_response is unsendable (no handler), and udt 4 is sendable",
        protocol::check_sendable(protocol::UDT_KEEPALIVE_RESPONSE).is_err()
            && protocol::check_sendable(protocol::UDT_JOIN_RESULT).is_ok(),
        "negative + positive control on the same call",
    ));

    c.push(check(
        "ids 30/31 are the eboot's order, not historicaldocs/design/activity-host-contract.md's",
        protocol::name(30) == Some("query_entity_slot_authority_mask_per_bubble_response")
            && protocol::name(31) == Some("query_entity_slot_authority_mask_response"),
        "the doc has them swapped - see protocol.rs",
    ));

    // ── the wire-codec gate: the crate's most important honest refusal ─────────────────────────

    // ⭐⭐ **INVERTED 2026-08-08.** These two asserted that the default codec REFUSED. It is
    // implemented now - `FUN_002A4090`/`FUN_002A4140` run through `d1-oracle` - so they assert that
    // it works, on the one payload nobody here made up, and that it is still distinguishable from
    // the raw-struct encoding the live client rejected.

    let captured: Vec<u8> = {
        let s = d1_oracle::activity_host::CAPTURED_JOIN_REQUEST;
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap_or(0)).collect()
    };
    let decoded = SchemaCodec.decode(protocol::UDT_JOIN_REQUEST, &captured);
    let nonce_ok = decoded
        .as_ref()
        .ok()
        .and_then(|i| i.get(0..4))
        .map(|n| u32::from_le_bytes(n.try_into().unwrap()))
        == Some(d1_oracle::activity_host::CAPTURED_NONCE);
    c.push(check(
        "⭐⭐ the DEFAULT codec decodes the 238 bytes the LIVE CLIENT sent, to the nonce it logged",
        captured.len() == 238 && nonce_ok && decoded.as_ref().map(|i| i.len()) == Ok(0x100),
        match &decoded {
            Ok(i) => format!(
                "238 wire bytes -> {} struct bytes, nonce {:#010x} (run: d1-oracle activity-host controls)",
                i.len(),
                u32::from_le_bytes(i[0..4].try_into().unwrap())
            ),
            Err(e) => format!("{e}"),
        },
    ));

    let msg = AhMessage::JoinResult(JoinResult::accepted([1, 2, 3, 4], 0xD1, "s", 0));
    let schema_wire = SchemaCodec.encode(&msg);
    let raw_wire = RawStructCodec.encode(&msg);
    c.push(check(
        "the schema encoding of join_result exists and DIFFERS from the falsified raw-struct one",
        matches!((&schema_wire, &raw_wire), (Ok(s), Ok(r)) if !s.is_empty() && s != r && r.len() == 0x1A8),
        match (&schema_wire, &raw_wire) {
            (Ok(s), Ok(r)) => format!("schema {} bytes vs raw struct {} bytes", s.len(), r.len()),
            _ => "one of the two codecs refused".to_string(),
        },
    ));

    // ⚠ A control that only ever passes is decoration. The refusal did not go away - it narrowed to
    // the 22 ids with no call site, and this is the half that must keep failing.
    let unpinned = SchemaCodec.encode(&AhMessage::AdvanceReplicationEpoch { epoch: 1, suppress: 0 });
    c.push(check(
        "NEGATIVE: an unpinned user_defined_type still refuses rather than improvising a tag",
        matches!(unpinned, Err(crate::AhError::SchemaCodecMissing { user_defined_type: 42, .. }))
            && schema_wire.is_ok(),
        "udt 42 has no FUN_00299310 call site we have read; udt 4 does - positive control alongside",
    ));

    c.push(check(
        "the raw-struct codec is labelled an experiment, records its falsification, honours direction",
        RawStructCodec::IS_AN_EXPERIMENT
            && RawStructCodec::WAS_FALSIFIED_ON_THE_LIVE_CLIENT
            && RawStructCodec.name().contains("FALSIFIED")
            && RawStructCodec
                .encode(&AhMessage::Opaque { user_defined_type: 16, bytes: vec![0] })
                .is_err()
            && raw_wire.is_ok(),
        "run 20260808-210910-AH-RAWCODEC: the pushes went out and the client rejected them",
    ));

    c.push(check(
        "the two join tags this crate pins equal the ones derived from the eboot",
        d1_oracle::activity_host::tag_for_udt(3) == Ok(crate::codec::TAG_JOIN_REQUEST)
            && d1_oracle::activity_host::tag_for_udt(4) == Ok(crate::codec::TAG_JOIN_RESULT),
        "join_request 0x80800E98, join_result 0x80800A58 - two instruments, one answer",
    ));

    // ── framing: what d1-bap will and will not write ──────────────────────────────────────────

    let mut conv = d1_bap::conversation::BapConversation::with_default_deadline();
    let hundred = conv.push(crate::push::BAP_TYPE_AH_TO_CLIENT_NOTIFICATION, &[0u8; 8]);
    let queuez = conv.push(d1_bap::TYPE_QUEUEZ_OBJECT, &[0u8; 8]);
    // ⭐ INVERTED 2026-08-08: `d1-bap` now writes header class 6. This asserted the refusal until
    // then; it asserts the frame instead, and still discriminates - it pins the 32-byte header and
    // the body's position, so a regression to a 6-byte header fails here rather than hanging a run.
    c.push(check(
        "BAP 100 goes out with a 32-byte class-6 header, and BAP 123 still goes out with 6",
        match (&hundred, &queuez) {
            (Ok(h), Ok(q)) => {
                h.len() == 32 + 8
                    && h[0..2] == [0x00, 0x64]
                    && h[32..] == [0u8; 8]
                    && q.len() == 6 + 8
            }
            _ => false,
        },
        "positive control on both, discriminating on length; the class-6 layout is push.rs's map",
    ));

    // ⭐⭐ 2026-08-09 - the routing control. The axis it tests is *which push policy the client
    // runs*, which is selected by the message id alone: the two ids share a body codec (the
    // client's message classes 0x1923850 / 0x19237E0 share vtable slots 2/3/4/6), so a frame that
    // differs only in its header is the whole experiment.
    let nine = conv.push(crate::push::BAP_TYPE_BAP_TO_CLIENT_NOTIFICATION, &[0u8; 8]);
    c.push(check(
        "⭐ BAP 9 is ours to push, header class 0, and its body sits where BAP 100's does",
        match (&nine, &hundred) {
            (Ok(n), Ok(h)) => {
                n.len() == 6 + 8
                    && n[0..2] == [0x00, 0x09]
                    && n[6..] == [0u8; 8]
                    && n[6..] == h[32..]
                    && d1_bap::tables::is_ours_to_push(9)
                    && d1_bap::tables::is_pushable(9)
            }
            _ => false,
        },
        "same 8 body bytes under both ids; only the header differs - 0x5642E0 delivers, 0x564C20 relays",
    ));

    // NEGATIVE with the axis stated: `is_ours_to_push` must not have become "yes to everything".
    c.push(check(
        "NEGATIVE: adding 9 did not open the accept-set - 160 and 161 are still not ours",
        !d1_bap::tables::is_ours_to_push(160)
            && !d1_bap::tables::is_ours_to_push(161)
            && d1_bap::tables::is_pushable(160),
        "160/161 are pushable by the client's table and still refused by ours",
    ));

    // The drift control for the name table `d1-responder` reads. `d1-bap` cannot depend on this
    // crate, and `d1-responder` must not depend on this crate (it would drag in `unicorn`), so the
    // 17 AH→client names exist twice. This is what stops them diverging.
    c.push(check(
        "the 17 AH->client udt names in d1-bap agree with this crate's authority table",
        {
            let mut agree = d1_bap::activity_host::AH_TO_CLIENT_UDT_NAMES.len()
                == protocol::AH_TO_CLIENT.len();
            for (udt, name) in d1_bap::activity_host::AH_TO_CLIENT_UDT_NAMES {
                agree &= protocol::name(udt) == Some(name);
                agree &= protocol::AH_TO_CLIENT.contains(&(udt as u8));
            }
            // NEGATIVE on the same call: a client→AH-only id must not resolve.
            agree && d1_bap::activity_host::ah_to_client_udt_name(3).is_none()
                && d1_bap::activity_host::ah_to_client_udt_name(4) == Some("join_result")
        },
        "17 names, both directions of the check, plus udt 3 (join_request, client->AH) refusing",
    ));

    let one_o_two = conv.push(crate::push::BAP_TYPE_AH_TO_CLIENT_LOG_NOTIFICATION, &[0u8; 256]);
    c.push(check(
        "BAP 102 is not pushable anywhere, and both layers say so",
        matches!(one_o_two, Err(d1_bap::conversation::PushError::NotPushable(102)))
            && AhPush::log_notification("x")
                == Err(PushBlocked::LogNotificationIsDeliverableNowhere),
        "absent from the 13-entry accept-set; all four connections share one push handler",
    ));

    c.push(check(
        "the class-6 header map tiles exactly 32 bytes, and d1-bap agrees on the length",
        {
            let mut covered = [false; 32];
            let mut okay = true;
            for (_, off, len, _) in CLASS6_HEADER_FIELDS {
                for b in off..off + len {
                    okay &= !covered[b];
                    covered[b] = true;
                }
            }
            okay && covered.iter().all(|x| *x) && d1_bap::HEADER_LEN[6] == 32
        },
        "no gap, no overlap - the control for having read the whole header",
    ));

    c.push(check(
        "message 8's body starts at +14 (header class 3), not +6",
        d1_bap::header_len(8) == Some(14)
            && d1_bap::header_len(123) == Some(6)
            && d1_bap::header_len(0x00FB) == Some(8),
        "with the two classes we already ship live as the positive control",
    ));

    c.push(check(
        "message 8 is fire-and-forget; message 16 expects 17",
        d1_bap::response_for(8) == d1_bap::tables::ResponsePolicy::FireAndForget
            && d1_bap::response_for(16) == d1_bap::tables::ResponsePolicy::Expects(17),
        "R6: replying to a fire-and-forget is instantly unroutable",
    ));

    // ── entity indices ────────────────────────────────────────────────────────────────────────

    c.push(check(
        "12,288 entity indices in a 1,536-byte mask",
        ENTITY_INDEX_COUNT == 12_288
            && ENTITY_MASK_BYTES == 1_536
            && ENTITY_INDEX_COUNT == ENTITY_MASK_BYTES * 8,
        "FUN_0008F800(ac+0x5EA8, 0x3000); the apply loop bound is 0x180 words",
    ));

    c.push(check(
        "the offline seam is reproducible: grant_all == memset(0xFF, 0x600)",
        EntitySlotMask::grant_all().to_struct_bytes() == vec![0xFFu8; ENTITY_MASK_BYTES]
            && EntitySlotMask::empty().count() == 0,
        "the client's own no-AH branch, mirrored so a run can compare them",
    ));

    let mut alloc = EntitySlotAllocator::new();
    let first = alloc.allocate(4);
    let second = alloc.allocate(4);
    c.push(check(
        "the allocator never re-grants an index",
        first.count() == 4 && second.count() == 4 && !second.get(0) && alloc.granted().count() == 8,
        "lowest-free policy; a re-grant would corrupt the client's OR-in apply",
    ));

    // ── the message structs ───────────────────────────────────────────────────────────────────

    let mut j = JoinResult::accepted([0xDE, 0xAD, 0xBE, 0xEF], 0x1122_3344_5566_7788, "sess", 3);
    j.grace_ms = 0x3344;
    j.keepalive_interval_ms = 0x5566;
    let jb = j.struct_bytes().unwrap_or_default();
    c.push(check(
        "join_result is 0x1A8 and every field lands where 0x29BD10 reads it",
        jb.len() == 0x1A8
            && jb[0x000..0x004] == [0xDE, 0xAD, 0xBE, 0xEF]
            && jb[0x008..0x010] == 0x1122_3344_5566_7788u64.to_le_bytes()
            && jb[0x100] == 0
            && &jb[0x101..0x105] == b"sess"
            && jb[0x194] == 3
            && jb[0x196..0x198] == 0x3344u16.to_le_bytes()
            && jb[0x198..0x19A] == 0x5566u16.to_le_bytes(),
        "nonce +0x00, ah_id +0x08, result +0x100, session_id +0x101, epoch +0x194, ms +0x196/+0x198",
    ));

    // ⚠⚠ CORRECTED 2026-08-08: this asserted `8 + 0x6B0`. Schema tag 0x80800E76's `struct_size` is
    // `0x6B0` TOTAL; `0xBAFB50`'s `payload+8` is unpacking a simulation-queue element, not this.
    c.push(check(
        "global_activity_state is exactly 1,712 bytes - the descriptor's struct_size, not 8 more",
        GlobalActivityState::empty().struct_bytes().map(|b| b.len()) == Ok(0x6B0)
            && SchemaCodec::struct_size(protocol::UDT_GLOBAL_ACTIVITY_STATE) == Ok(0x6B0),
        "schema tag 0x80800E76; our constant and the eboot's descriptor agree",
    ));

    // ⭐ udt 1 carries the rung-31 ACTIVITY NAME, and `FUN_01057B80` sets `ac+0x130 |= 0x40` only
    // on a successful decode of it. Three separate things depend on this message.
    let named = GlobalActivityState::for_activity("act_cosmo_m0_highway");
    let name_ok = named.as_ref().map(|s| s.name()) == Ok("act_cosmo_m0_highway".to_string());
    c.push(check(
        "⭐ the activity NAME is a char[0x28] at blob +0x688 and tiles the struct to its END",
        GlobalActivityState::NAME == 0x688
            && GlobalActivityState::NAME_CAPACITY == 0x28
            && GlobalActivityState::NAME + GlobalActivityState::NAME_CAPACITY == 0x6B0
            && name_ok,
        "0x80800B8F[9] @+0x060 -> 0x80800CC5[10] @+0x628 -> 0x8080409F (40 B char array)",
    ));

    c.push(check(
        "the state we PUBLISH is not the all-zero one: FLAG_0 set and a name written",
        named.as_ref().map(|s| s.blob[GlobalActivityState::FLAG_0]) == Ok(1)
            && GlobalActivityState::empty().blob[GlobalActivityState::FLAG_0] == 0,
        "[R] the client requires blob+0x00 non-zero; empty() stays all-zero as the bisection control",
    ));

    c.push(check(
        "NEGATIVE: an over-long activity name is REFUSED, not truncated",
        {
            let mut g = GlobalActivityState::empty();
            g.set_name(&"a".repeat(GlobalActivityState::NAME_CAPACITY)).is_err()
                && g.set_name(&"a".repeat(GlobalActivityState::NAME_CAPACITY - 1)).is_ok()
        },
        "a truncated activity name is a DIFFERENT activity name and the client finds nothing",
    ));

    // ⭐⭐ INVERTED 2026-08-09. These two used to assert *"NAMED GAP: udt 11 is unimplemented AND
    // not schema-coded"* and *"the THREE messages … that ARE schema-coded"*. The gap is closed and
    // the premise behind it was wrong: `0x29CDE0` is udt **7** `sensor_message`, not udt 11. The
    // receive dispatch table `0x18F84A0` stores its own type ids and is **not** sorted, so the
    // index-to-udt mapping that produced the claim never held. udt 11's handler is
    // `0x29CF50 -> 0x29C730`, and that is the same object codec plus one leading presence bit.
    c.push(check(
        "⭐ ALL FOUR of establish-client's messages are schema-coded and encode",
        [
            protocol::UDT_JOIN_RESULT,
            protocol::UDT_ENTITY_SLOTS_ALLOCATED,
            protocol::UDT_GLOBAL_ACTIVITY_STATE,
            protocol::UDT_REPLICATE_MEMBERSHIP,
        ]
        .iter()
        .all(|u| SchemaCodec::struct_size(*u).is_ok()),
        "udt 4 (0x1A8), udt 0 (0x600), udt 1 (0x6B0), udt 11 (0x5038) - FUN_00298EF0's four",
    ));

    c.push(check(
        "⭐ udt 11 is tag 0x808009ED / struct 0x5038, and its RX handler is 0x29CF50 not 0x29CDE0",
        d1_oracle::activity_host::tag_for_udt(11) == Ok(0x8080_09ED)
            && SchemaCodec::struct_size(protocol::UDT_REPLICATE_MEMBERSHIP)
                == Ok(ReplicateMembership::STRUCT_SIZE)
            && ReplicateMembership::STRUCT_SIZE == 0x5038,
        "0x29C730 states 0x5038 twice - `local_528c` and `memcpy(image, ac+0xE70, 0x5038)` - and \
         the schema record, reached by a chain that never sees either, agrees",
    ));

    // NEGATIVE, kept: closing udt 11 (and udt 2, `world_globals_state`, 8 bytes, pinned on 56578
    // at call site 0x191A418) must not have opened the 20 other unpinned ids.
    c.push(check(
        "NEGATIVE: the 20 remaining unpinned udts still REFUSE rather than improvise a tag",
        SchemaCodec::struct_size(42).is_err()
            && SchemaCodec::struct_size(2) == Ok(8)
            && SchemaCodec::struct_size(protocol::UDT_REPLICATE_MEMBERSHIP).is_ok(),
        "one closed gate is one closed gate; a codec that started answering for everything would \
         be the fabrication this crate exists to refuse",
    ));

    // ── the registry: the two levers ──────────────────────────────────────────────────────────

    let mut reg = HostRegistry::new(SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 1), 4110));
    let entry = reg.allocate("/ah_type:fireteam").clone();
    let echo = reg.answer_proxy_request(d1_bap::activity_host::GetActivityHostProxyRequest {
        activity_host_id: entry.ah_id,
    });
    c.push(check(
        "an allocated AH id is non-zero and message 17 echoes it verbatim",
        entry.ah_id != 0
            && echo.activity_host_id == entry.ah_id
            && echo.ipv4() == Ipv4Addr::new(10, 0, 0, 1)
            && echo.port() == 4110,
        "0 is the `no activity host` sentinel; a wrong echo is a SILENT drop at 0x594E70",
    ));

    c.push(check(
        "message 7's host_text is the session id, not an address",
        !entry.session_id.contains(':') && entry.session_id.starts_with("d1ah-"),
        "udt 10 = {u64 ah_id, char session_id[]} = 0x88 = BAP 7's payload",
    ));

    // ⚠ A control must test the axis you are worried about. The worry here is a falsifier that
    // reports "passed" without the client ever having asked - so check a FRESH registry reads
    // unfired, and that answering message 7 alone does not arm it. `reg` above has already served
    // a proxy request, which is the positive half.
    let mut fresh = HostRegistry::new(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 1));
    let unfired_when_new = !fresh.falsifier_passed();
    fresh
        .answer_manager_request(&d1_bap::activity_host::ActivityHostManagerRequest {
            inner_type: 3,
            args: Vec::new(),
            nested_msg: Vec::new(),
            bitstream: Vec::new(),
            trailing_id: 0,
        })
        .ok();
    c.push(check(
        "the §4.2 falsifier fires only on message 16, never on answering message 7",
        unfired_when_new && !fresh.falsifier_passed() && reg.falsifier_passed(),
        format!(
            "fresh: {} manager responses, {} proxy requests; the armed one has {}",
            fresh.manager_responses, fresh.proxy_requests, reg.proxy_requests
        ),
    ));

    // ── the state machine ─────────────────────────────────────────────────────────────────────

    let now = Instant::now();
    let mut inst = ActivityInstance::new(entry.ah_id, entry.session_id.clone(), now);
    inst.on_connect(now);
    let mut img = vec![0u8; 0x100];
    img[..4].copy_from_slice(&[7, 7, 7, 7]);
    for (i, byte) in img[ReplicateMembership::JOIN_REQUEST_PROPERTIES
        ..ReplicateMembership::JOIN_REQUEST_PROPERTIES
            + ReplicateMembership::PROPERTIES_PREFIX_SIZE]
        .iter_mut()
        .enumerate()
    {
        *byte = (i as u8).wrapping_add(1);
    }
    let join = ClientMessage::from_struct_image(3, &img).unwrap();
    let out = inst.on_client_message(&join, now);
    let echoed = match out.first().map(|p| &p.message) {
        Some(AhMessage::JoinResult(j)) => j.nonce == [7, 7, 7, 7] && j.keepalive_interval_ms > 0,
        _ => false,
    };
    c.push(check(
        "join_request -> join_result echoes the nonce and advertises a non-zero keepalive",
        echoed && inst.state() == InstanceState::Live,
        "the client has NO default keepalive interval; ac+0x6C74 starts at 0",
    ));

    // ⭐⭐ INVERTED 2026-08-09. This used to assert `out.len() == 2`. Two is what run
    // `20260808-234535-CODEC-LIVE2` sent, and the client answered by sitting in
    // `[AC … CON-Y EST-N AH->0 MEM-0]` for 22 s and timing out. `FUN_00298EF0` is a conjunction of
    // six live flags and **four** of them are messages, so two was never going to be enough.
    let joined: Vec<u32> = out.iter().map(|p| p.message.user_defined_type()).collect();
    c.push(check(
        "⭐⭐ a join pushes ALL FOUR of FUN_00298EF0's messages, in FUN_00760470's wait order",
        joined == vec![4, 0, 1, 11],
        "4 join_result -> 0 entity_slots_allocated -> 1 global_activity_state -> \
         11 replicate_membership. udt 0 and udt 11 are UNSOLICITED: nothing ever asks for them",
    ));

    // The entity grant is the whole pool, in one message, unrequested.
    c.push(check(
        "⭐ the unsolicited entity grant is the FULL 12,288-index mask, 0x600 bytes",
        match out.get(1).map(|p| &p.message) {
            Some(AhMessage::EntitySlotsAllocated(m)) => {
                m.count() == 12_288 && m.to_struct_bytes().len() == 0x600
            }
            _ => false,
        },
        "FUN_010579F0 has no 'was it requested?' check, and ac+0x64AA - the flag FUN_00298EF0 \
         reads - is written in exactly one place in all 47,304 decompiled files: that handler",
    ));

    // ⭐ The membership is an ECHO of the client's own identity, not a struct we invented.
    let ident_ok = match out.get(3).map(|p| &p.message) {
        Some(AhMessage::ReplicateMembership(m)) => {
            m.member_identity() == &img[ReplicateMembership::JOIN_REQUEST_IDENTITY
                ..ReplicateMembership::JOIN_REQUEST_IDENTITY + ReplicateMembership::IDENTITY_SIZE]
                && m.image.len() == ReplicateMembership::STRUCT_SIZE
                && m.image[ReplicateMembership::SIMULATION_OCCUPANCY_MASK] == 1
                && m.image[ReplicateMembership::OCCUPANCY_MASK] == 1
                && m.member_properties_prefix()
                    == &img[ReplicateMembership::JOIN_REQUEST_PROPERTIES
                        ..ReplicateMembership::JOIN_REQUEST_PROPERTIES
                            + ReplicateMembership::PROPERTIES_PREFIX_SIZE]
                && m.member_fireteam_id()
                    == u64::from_le_bytes(
                        img[ReplicateMembership::JOIN_REQUEST_FIRETEAM_ID
                            ..ReplicateMembership::JOIN_REQUEST_FIRETEAM_ID + 8]
                            .try_into()
                            .unwrap(),
                    )
                && m.member_fireteam_id() != 0
                && m.acknowledged_session_host()
                    == &img[ReplicateMembership::JOIN_REQUEST_IDENTITY
                        ..ReplicateMembership::JOIN_REQUEST_IDENTITY + 8]
        }
        _ => false,
    };
    c.push(check(
        "⭐ replicate_membership echoes identity + properties, publishes both member masks, and acknowledges member[0] as host",
        ident_ok && inst.published_membership(),
        "join_request+0x10 and a membership record's identity are the SAME struct (0x80800C2D, \
         also udt 22), while join_request+0x38 and member+0x28 share 0x80800B1F; its +0xA0 \
         fireteam id is what FUN_0075DB30 returns and FUN_00BA3D60 requires non-zero; +0x5028 \
         bit 0 makes FUN_00BA0B10 copy member[0]'s machine id into the simulation cache, while \
         +0x502C bit 0 is what FUN_0029FE70 tests; FUN_0029FE40 requires membership+0x5020 to \
         equal the local host player identifier",
    ));

    // NEGATIVE on the same path: with nothing to echo, the push is LOST, never faked.
    let mut starved = ActivityInstance::new(entry.ah_id, entry.session_id.clone(), now);
    starved.on_connect(now);
    let starved_out =
        starved.on_client_message(&ClientMessage::from_struct_image(3, &[0u8; 8]).unwrap(), now);
    c.push(check(
        "NEGATIVE: a join_request too short to carry an identity loses udt 11 rather than faking it",
        starved_out.iter().map(|p| p.message.user_defined_type()).collect::<Vec<_>>() == vec![4, 0, 1]
            && !starved.published_membership()
            && starved.state() == InstanceState::Joined,
        "0x29C730 sets ac+0x131 |= 1 UNCONDITIONALLY, even on a decode failure - so a fabricated \
         membership would pass the gate and then fail FUN_0029FE70's slot search in silence",
    ));

    let idle = inst.on_client_message(&ClientMessage::Keepalive, now);
    c.push(check(
        "a keepalive produces no reply, and a join does",
        idle.is_empty() && !out.is_empty(),
        "udt 16 has no client handler; the reply to a keepalive is not evicting",
    ));

    // ⚠ On a **pre-join** instance, because `inst` has already been granted the whole pool and an
    // allocator with nothing left correctly returns nothing. Asserting the batch path on the
    // exhausted instance would have measured the grant, not the allocator.
    let mut fresh = ActivityInstance::new(entry.ah_id, entry.session_id.clone(), now);
    fresh.on_connect(now);
    let grant = fresh.on_client_message(
        &ClientMessage::from_struct_image(19, &12u32.to_le_bytes()).unwrap(),
        now,
    );
    c.push(check(
        "allocate_entity_indices -> a 0x600 mask with exactly the requested count",
        match grant.first().map(|p| &p.message) {
            Some(AhMessage::EntitySlotsAllocated(m)) => {
                m.count() == 12 && m.to_struct_bytes().len() == 0x600
            }
            _ => false,
        },
        "the client ORs it into ac+0x5EA8",
    ));

    c.push(check(
        "…and after the full unsolicited grant there is nothing left to hand out",
        match inst
            .on_client_message(
                &ClientMessage::from_struct_image(19, &12u32.to_le_bytes()).unwrap(),
                now,
            )
            .first()
            .map(|p| &p.message)
        {
            Some(AhMessage::EntitySlotsAllocated(m)) => m.count() == 0,
            _ => false,
        } && inst.entities().granted().count() == 12_288,
        "the pool policy frees back down over its high-water mark; an empty grant is the honest \
         answer when the client already holds all 12,288",
    ));

    // ── ⭐⭐ THE LEASE, 2026-08-09 ─────────────────────────────────────────────────────────────
    //
    // The failure run 20260809-054417-RUNG33-BYPASS answered the join perfectly and then went
    // quiet. The client's own line: `Lost connection to activity host (possible crash) (last
    // received 20626 ms > timeout (20000 + 500) ms)`. Everything below tests the axis that killed
    // it: **our transmit silence**, not the keepalive we were being sent.

    let mut live = ActivityInstance::new(0xD1A0_0000_0000_0001, "d1ah-lease", now);
    live.on_connect(now);
    let mut img = vec![0u8; 0x100];
    img[0x10..0x38].copy_from_slice(&[0x5Au8; 0x28]);
    live.on_client_message(&ClientMessage::from_struct_image(3, &img).unwrap(), now);

    // Sixty seconds of the client doing exactly what it did in that run: udt 15 every 2 s and
    // nothing else. Record when we transmit, and take the worst gap.
    let step = std::time::Duration::from_millis(2_000);
    let mut last_tx = now;
    let mut worst = std::time::Duration::ZERO;
    let mut spoke = 0usize;
    for i in 1..=30u32 {
        let t = now + step * i;
        if live.on_client_message(&ClientMessage::Keepalive, t).is_empty() {
            worst = worst.max(t.saturating_duration_since(last_tx));
        } else {
            worst = worst.max(t.saturating_duration_since(last_tx));
            last_tx = t;
            spoke += 1;
        }
    }
    let ceiling = std::time::Duration::from_millis(crate::state::HOST_SILENCE_LIMIT_MS);
    c.push(check(
        "⭐⭐ THE LEASE: over 60 s of keepalive-only traffic the AH never stays silent for the \
         20,500 ms the client calls a host crash",
        worst < ceiling && spoke >= 8,
        format!(
            "worst transmit gap {} ms vs the client's {} ms ceiling; {spoke} unprompted \
             re-publications. FUN_0075E440: `timeout + grace <= FUN_0029CA00(ac)`, and \
             FUN_0029CA00 is `now - messaging->vfunc[5]()` = `now - conn+0x1EF8` - the same field \
             the drought line prints as `received … last %lld ms ago`",
            worst.as_millis(),
            ceiling.as_millis()
        ),
    ));

    // ⭐ The TIMER half. Inbound traffic is not guaranteed - the client's own keepalive is
    // suppressed whenever it sends anything else (`FUN_00298C40` compares `now - ac+0xE48`, which
    // any successful send refreshes) - so silence on the socket must still produce a frame.
    let mut ticked = ActivityInstance::new(2, "s", now);
    ticked.on_connect(now);
    ticked.on_client_message(&ClientMessage::from_struct_image(3, &img).unwrap(), now);
    let hb = ticked.host_heartbeat();
    let too_soon = ticked.heartbeat_due(now + hb / 2).is_none();
    let due = ticked.heartbeat_due(now + hb);
    c.push(check(
        "⭐ the heartbeat also fires on a TIMER, and only once the interval has passed",
        too_soon
            && due.map(|p| p.message.user_defined_type()) == Some(1)
            && ticked.heartbeat_due(now + hb).is_none()
            && !ticked.set_host_heartbeat(ceiling),
        format!(
            "period {} ms; a period at or above the {} ms ceiling is REFUSED - that is a countdown, \
             not a heartbeat",
            hb.as_millis(),
            ceiling.as_millis()
        ),
    ));

    // ⛔ NEGATIVE, and it is the one that stops this becoming decoration: the heartbeat must not
    // have been bought by "ack everything". A keepalive still produces **no reply of its own**, and
    // udt 16 is still unsendable.
    let mut quiet = ActivityInstance::new(1, "s", now);
    quiet.on_connect(now);
    quiet.on_client_message(&ClientMessage::from_struct_image(3, &img).unwrap(), now);
    c.push(check(
        "NEGATIVE: the heartbeat is not an ACK - a keepalive inside the interval still answers \
         nothing, and udt 16 is still unsendable",
        quiet
            .on_client_message(&ClientMessage::Keepalive, now + step)
            .is_empty()
            && protocol::check_sendable(protocol::UDT_KEEPALIVE_RESPONSE).is_err(),
        "the reply to a keepalive is still `not evicting`; what changed is that the HOST now \
         speaks on its own schedule, which is a different mechanism answering a different timer",
    ));

    // ⭐ udt 17 - the client's own cue. It must re-publish the three unsolicited messages even
    // though the join already sent them, which is exactly what `published_state` blocks elsewhere.
    let refresh = quiet.on_client_message(
        &ClientMessage::from_struct_image(17, &[0u8; 8]).unwrap(),
        now + step * 2,
    );
    let refresh_udts: Vec<u32> =
        refresh.iter().map(|p| p.message.user_defined_type()).collect();
    c.push(check(
        "⭐ udt 17 activity_client_request_state_refresh re-publishes 0, 1 and 11 - past the \
         published-once guard",
        refresh_udts == vec![0, 1, 11] && quiet.state_refreshes == 1,
        "c_activity_client::vfunc[5] (0x29AB80) sends it from the accept branch of the \
         join_result handler 0x29BD10; its 8-byte body leads with the membership counter the \
         client holds",
    ));

    // ⭐ udt 21 - was `NoHandler`, is `ClientToAh`, and is answered with nothing on purpose.
    let cad = ClientMessage::from_struct_image(21, &[0u8; 0x220]);
    let cad_reply = cad
        .as_ref()
        .map(|m| quiet.on_client_message(m, now + step * 3))
        .unwrap_or_default();
    c.push(check(
        "⭐ udt 21 client_authoritative_data_update is RECEIVABLE and answered with nothing - \
         fire-and-forget, not a gate",
        cad.is_ok()
            && cad_reply.is_empty()
            && protocol::check_receivable(21).is_ok()
            && protocol::check_sendable(21).is_err()
            && !protocol::NO_HANDLER.contains(&21),
        "FUN_0029CC20 encodes it with FUN_002C9290 and calls messaging->vfunc[6] DIRECTLY, which \
         is why a scan of FUN_00299310's call sites could not see it; FUN_002987E0's success \
         branch only memcpys 0x220 and clears the dirty mask",
    ));

    // ⭐ udt 14 - recorded, never answered.
    let leave = quiet.on_client_message(
        &ClientMessage::from_struct_image(14, &[0u8; 0x14]).unwrap(),
        now + step * 4,
    );
    c.push(check(
        "⛔ udt 14 peer_leave_request is recorded and NOT answered - it is the funeral, not the \
         illness",
        leave.is_empty()
            && matches!(
                quiet.state(),
                InstanceState::Evicted { reason: crate::state::EvictionReason::ClientLeft }
            ),
        "in run 20260809-054417-RUNG33-BYPASS it arrived 0.3 s AFTER the client had already \
         logged `Lost connection to activity host` and resolved the instance to error_host_crashed",
    ));

    // ⭐ udt 15's raw arm, with the discriminating negative on the same call.
    let ka_wire = [0xABu8];
    let ka_image = SchemaCodec.decode(15, &ka_wire);
    let jr_image = SchemaCodec.decode(3, &crate::controls::captured_join_request());
    c.push(check(
        "⭐ udt 15 decodes through the client's own RAW arm - the payload IS the image - while \
         udt 3 does not",
        ka_image.as_deref() == Ok(&ka_wire[..])
            && jr_image.as_ref().map(|i| i.len()) == Ok(0x100)
            && jr_image.as_deref() != Ok(&crate::controls::captured_join_request()[..]),
        "FUN_00299310's seventh argument selects the arm: `if ((*flags & 1) == 0) encode through \
         the descriptor else send the struct verbatim`. FUN_00298C40 sets the flag and passes \
         descriptor 0xFFFFFFFF - so this is READ, not the falsified RawStructCodec guess",
    ));

    // ⭐ udt 37 - a harness limit with a named cause, kept apart from "unknown wire format".
    let df = d1_oracle::activity_host::dynamic_flags_udts().unwrap_or_default();
    let why37 = crate::codec::undecodable_reason(37).unwrap_or_default();
    c.push(check(
        "⭐ udt 37 send_client_heartbeat is NOT DECODED on purpose, with its cause, and udt 1 \
         still decodes",
        df == vec![33, 37]
            && why37.contains("DYNAMIC FLAGS")
            && crate::codec::undecodable_reason(1).is_none()
            && crate::codec::undecodable_reason(3).is_none(),
        "descriptor 0x80800D06 is one field of kind 0x22; its bitset is grown by 0x2C6BA0 out of \
         the game's dynamic-flags pool, which a statically mapped image does not have, so \
         0x1065850 (`mov edx,[rax+rcx*4]`) faults. Both ids are client→AH with no client handler",
    ));

    c.extend(envelope_controls());
    c.extend(activity_index_controls());
    c.extend(served_world_controls());
    c.extend(served_team_controls());

    c.push(ok(
        "THE CEILING is stated in the crate's own module doc",
        "a stub AH yields a loaded world with no spawns, encounters or objectives - the mission \
         script is cloud-side content we do not have",
    ));

    // ── ⭐⭐ THE LEAK. 2026-08-09. Deliberately LAST. ─────────────────────────────────────────
    //
    // The axis, in words: **this proves the Activity Host can encode its message set indefinitely
    // without the oracle's arena growing.**
    //
    // Every control above this line was green through run `20260809-120451-MOONBASE-FRESH`, in
    // which the host encoded `global_activity_state` 131 times over thirteen minutes and then
    // stopped: the oracle's Unicorn bump arena was exhausted, `free` being a no-op, and the client
    // answered with `error.private_activity_host_host_crashed`. **Every one of those controls
    // encodes each message exactly once, and a per-call leak is invisible to a one-shot control.**
    // Only a loop can see it - so here is the loop, through the real `SchemaCodec`, the real
    // oracle worker thread and the real `AhMessage` types.
    //
    // ⚠ **It runs last on purpose.** The oracle worker is process-global, so a leaking build has
    // an exhausted arena by the time this returns and every control after it fails too. Measured:
    // with the rewind reverted, this suite goes 68/75 and four of the seven failures are innocent
    // controls downstream. Putting the loop at the end keeps the first red line the true one.
    c.extend(leak_controls());

    c
}

/// ⭐⭐ **THE AXIS: does a frame produced on the TIMER path leave this connection in the same
/// shape as one produced on the INBOUND path?**
///
/// The lease control above proves the host *decides* to speak. It never checked *what it says* -
/// it tests [`ActivityInstance`], which has no sockets, no envelope and no cipher. Run
/// `20260809-075545-RUNG33-UNLOCK-98` died on precisely the gap between those two questions:
/// `AhConnection::on_tick` returned `d1-bap` **inner** frames straight to the socket, with no
/// envelope and no seal, and the client read the message id's high byte as the envelope version.
///
/// ```text
/// 15:58:32.767  heartbeat WantsToPush { user_defined_type: 1 }        <- ours
/// 15:58:33.28x  Failed to parse envelope version '0' (expected 1).    <- FUN_00B2D9F0
///               Failed to call update_receiving().
///               Fatal error '_connection_failure_unknown_message_type' raised.
///               'PRIVATE CURRENT' activity client lost its connection to BAP.
///               [status 33] … [change reason 101]  error.private_activity_host_lost_connection_to_bap
/// 15:58:33.289  AH connection … ended: Connection reset by peer (os error 104)
/// ```
///
/// ⭐ Its own negative control is in the run corpus, not invented here:
/// `20260809-054417-RUNG33-BYPASS` had **no** heartbeat, therefore never called `on_tick`'s push
/// path, and logged **zero** envelope-version failures across 22 connections - dying instead of
/// `[status 36][change reason 104]`, the *silence* timeout. One line of code, two different
/// client-side fatal errors, and the run that has it is the only run that has the error.
fn envelope_controls() -> Vec<Control> {
    use crate::listener::{AhConnection, AhEvent};
    use d1_bap::{ChannelNonces, Direction, SessionMaterial};
    use std::sync::Arc;

    let mut c = Vec::new();
    let now = Instant::now();
    let material = SessionMaterial::default();

    // A connection driven exactly as the listener drives one: the two plaintext handshake
    // messages, then the real 238-byte join_request, sealed, as the client sends it.
    let mut conn = AhConnection::new(
        ActivityInstance::new(0xD1A0_0000_0000_0001, "d1ah-controls", now),
        Arc::new(SchemaCodec) as Arc<dyn AhWireCodec>,
    );
    let plain_envelope = |inner: &[u8]| {
        let mut f = vec![d1_bap::ENVELOPE_VERSION, d1_bap::ENVELOPE_MODE_SECURE];
        f.extend_from_slice(&(inner.len() as u32).to_be_bytes());
        f.extend_from_slice(inner);
        f
    };
    let inner_header = |msg_type: u16, sequence: u32| {
        let mut h = msg_type.to_be_bytes().to_vec();
        h.extend_from_slice(&sequence.to_be_bytes());
        h
    };

    // ⛔ NEGATIVE, taken BEFORE the handshake: a tick with no channel must produce **nothing**.
    // Not a raw frame, not a panic. This is the state the old code would have written plaintext
    // from if the heartbeat had ever come due first.
    let (pre, _pre_events) = conn.on_tick(now + std::time::Duration::from_secs(60));
    let quiet_before_hello = pre.is_empty();

    conn.on_frame(&plain_envelope(&inner_header(d1_bap::TYPE_CHANNEL_STARTUP_REQUEST, 1)), now);
    conn.on_frame(&plain_envelope(&inner_header(d1_bap::TYPE_SECURE_HELLO_REQUEST, 2)), now);

    // Our own copy of the nonce chain, from the same seed the connection latched. Decrypting the
    // whole conversation with ONE chain is what makes this a cross-path control: if the tick path
    // sealed with a different nonce, or skipped the advance, frame N+1 stops opening.
    let mut nonces = ChannelNonces::from_seed(material.channel_seed);

    // The join, sealed by us with the client's direction of the same chain.
    let join_body = d1_bap::activity_host::ClientToActivityHostNotification {
        inner_type: 1,
        user_defined_type: protocol::UDT_JOIN_REQUEST,
        aux: Some(0),
        payload: captured_join_request(),
    }
    .encode();
    let mut join_inner = vec![0u8; 14];
    join_inner[0..2].copy_from_slice(&8u16.to_be_bytes());
    join_inner[2..6].copy_from_slice(&7u32.to_be_bytes());
    join_inner.extend_from_slice(&join_body);
    let sealed_join = d1_bap::seal_channel_frame(
        &material.session_key,
        nonces.peek(Direction::ClientToServer),
        &join_inner,
    )
    .expect("seal the join");
    nonces.advance(Direction::ClientToServer);

    let (join_replies, join_events) = conn.on_frame(&sealed_join, now);

    // ⭐ Now the timer path, past the heartbeat interval, with nothing else arriving.
    let hb = conn.instance().host_heartbeat();
    let (tick_replies, tick_events) = conn.on_tick(now + hb + std::time::Duration::from_millis(1));

    // ── 1. the envelope itself ────────────────────────────────────────────────────────────────
    let well_formed = |f: &Vec<u8>| {
        f.len() > 6
            && f[0] == d1_bap::ENVELOPE_VERSION
            && f[1] == d1_bap::ENVELOPE_MODE_CHANNEL
            && u32::from_be_bytes([f[2], f[3], f[4], f[5]]) as usize == f.len() - 6
    };
    c.push(check(
        "⭐⭐ THE ENVELOPE: every frame the HEARTBEAT (timer) path emits carries the six-byte \
         envelope `{version=1, mode=1, u32be len}` - the framing run 20260809-075545 was missing",
        tick_replies.len() == 1
            && well_formed(&tick_replies[0])
            && tick_events
                .iter()
                .any(|e| matches!(e, AhEvent::WantsToPush { user_defined_type: 1 })),
        format!(
            "{} frame(s) from on_tick; first begins {:02x} {:02x} - FUN_00B2D9F0 refuses any \
             version but 1 and raises _connection_failure_unknown_message_type",
            tick_replies.len(),
            tick_replies.first().map(|f| f[0]).unwrap_or(0xFF),
            tick_replies.first().map(|f| f[1]).unwrap_or(0xFF),
        ),
    ));

    // ── 2. ⛔ THE REPRODUCTION, so this control cannot pass by being vacuous ──────────────────
    // The inner frame the old code wrote is reconstructed here and asserted to be *exactly* what
    // the client complained about: byte 0 = 0x00, and `parse_envelope` refuses it. A control that
    // only checks the fix is decoration; this one states the bug in the same breath.
    let bare_inner = {
        let mut inst = ActivityInstance::new(1, "d1ah-repro", now);
        inst.on_connect(now);
        let mut img = [0u8; 0x100];
        img[0x10..0x38].copy_from_slice(&[0x5Au8; 0x28]);
        inst.on_client_message(&ClientMessage::from_struct_image(3, &img).unwrap(), now);
        let hb_period = inst.host_heartbeat();
        let push = inst.heartbeat_due(now + hb_period).expect("the heartbeat is due");
        let mut conv = d1_bap::conversation::BapConversation::with_default_deadline();
        push.emit(&mut conv, &SchemaCodec).expect("frame the heartbeat push")
    };
    c.push(check(
        "⛔ THE REPRODUCTION: a bare `d1-bap` inner frame begins 0x00 and is NOT a valid \
         envelope - `Failed to parse envelope version '0' (expected 1)` was literally our own \
         `msg_type >> 8`",
        bare_inner[0] == 0x00
            && bare_inner[1] == crate::push::BAP_TYPE_BAP_TO_CLIENT_NOTIFICATION as u8
            && d1_bap::parse_envelope(&bare_inner).is_err()
            && d1_bap::parse_envelope(&tick_replies[0]).is_ok(),
        format!(
            "inner starts {:02x} {:02x} (u16be message id {}); the sealed frame starts {:02x} \
             {:02x}. `BapConversation::drain`/`push` return INNER frames and say so - the caller \
             seals",
            bare_inner[0],
            bare_inner[1],
            crate::push::BAP_TYPE_BAP_TO_CLIENT_NOTIFICATION,
            tick_replies[0][0],
            tick_replies[0][1],
        ),
    ));

    // ── 3. ⭐⭐ ONE NONCE CHAIN ACROSS BOTH PATHS ─────────────────────────────────────────────
    // The four join replies and the heartbeat are decrypted in order under a single
    // server→client chain. This is the axis a shape check alone cannot reach: a tick path that
    // sealed correctly but with its own nonce state would pass control 1 and desync the client
    // on the very next inbound frame.
    let mut opened: Vec<Vec<u8>> = Vec::new();
    let mut chain_ok = true;
    for f in join_replies.iter().chain(tick_replies.iter()) {
        match d1_bap::open_channel_payload(
            &material.session_key,
            nonces.peek(Direction::ServerToClient),
            &f[6..],
        ) {
            Ok(p) => {
                nonces.advance(Direction::ServerToClient);
                opened.push(p);
            }
            Err(_) => {
                chain_ok = false;
                break;
            }
        }
    }
    let udts: Vec<u32> = opened
        .iter()
        .filter(|p| p.len() > 6 + 13)
        .map(|p| u32::from_be_bytes(p[6 + 9..6 + 13].try_into().unwrap()))
        .collect();
    c.push(check(
        "⭐⭐ ONE NONCE CHAIN: the four join pushes and the heartbeat all open under successive \
         server→client nonces - the timer path shares the inbound path's cipher state",
        chain_ok && opened.len() == 5 && udts == vec![4, 0, 1, 11, 1],
        format!(
            "{} frame(s) opened, udts {udts:?}; ChannelNonces::advance is +1 little-endian from \
             byte[0] and is called only after the GCM op succeeds, so a second sealing path with \
             its own state desynchronises the client silently",
            opened.len()
        ),
    ));

    // ── 4. every frame the connection ever writes, on either path, is enveloped ───────────────
    c.push(check(
        "NEGATIVE: on_tick before the secure hello emits ZERO frames rather than an unsealed \
         one, and no frame on either path escapes the envelope",
        quiet_before_hello
            && join_replies.iter().all(well_formed)
            && tick_replies.iter().all(well_formed)
            && !join_events.iter().any(|e| matches!(e, AhEvent::NamedGate { .. })),
        "there is now ONE sealing path (`AhConnection::seal_all`) with two triggers; having two \
         is what shipped a heartbeat the client killed the connection over",
    ));

    c
}

/// ⭐⭐ **THE AXIS: does the number rung 33 evaluates survive the schema encoder?**
///
/// `FUN_0078C770` reads a `u16` at `global_activity_state + 0x680` and `FUN_007A7420` hands it to
/// both `FUN_0054F410` (the unlock gate) and `activity_table[idx]` (the name it prints). Every run
/// so far published **0**, so the client evaluated `activity_table[0]` - `HASH(0x8AE8DA69)` =
/// 2,330,516,073 = **"Tower"** - whose prerequisites are unmet, and refused. Nine refusals across
/// two runs all named that one hash *because the index was a constant we sent*, not because a
/// repair machine substituted a fallback.
///
/// A shape check on the struct image is not enough: the field only matters if the **encoder** puts
/// it on the wire. So the control round-trips through the client's own codec.
fn activity_index_controls() -> Vec<Control> {
    let mut c = Vec::new();
    let now = Instant::now();

    // The descriptor's own statement, from a table that has never seen FUN_0078C770.
    c.push(check(
        "⭐ the index and the name are ADJACENT, INDEPENDENT fields - 0x80800CC5 [7] @+0x620 is a \
         16-bit leaf and [10] @+0x628 is the char[0x28]; 0x060 + those are 0x680 and 0x688",
        GlobalActivityState::ACTIVITY_INDEX == 0x680
            && GlobalActivityState::NAME == 0x688
            && GlobalActivityState::NAME + GlobalActivityState::NAME_CAPACITY == 0x6B0,
        "FUN_002A1F50 is `return param_1 + 0x50B0`, so FUN_0078C770's `+0x680` is this blob's; \
         and the name is nowhere derived from the index, so neither may be invented from the other",
    ));

    // ⭐⭐ Round-trip through the client's own encoder AND decoder, at two different indices.
    let encode_at = |idx: u16| -> Option<Vec<u8>> {
        let mut inst = ActivityInstance::new(7, "d1ah-idx", now);
        inst.on_connect(now);
        inst.set_activity_index(idx);
        let mut img = [0u8; 0x100];
        img[0x10..0x38].copy_from_slice(&[0x5Au8; 0x28]);
        let pushes = inst.on_client_message(&ClientMessage::from_struct_image(3, &img).ok()?, now);
        let one = pushes.into_iter().find(|p| p.message.user_defined_type() == 1)?;
        one.to_bap_body(&SchemaCodec).ok()
    };
    let zero = encode_at(0);
    let clash = encode_at(393);
    // ⚠ `SchemaCodec::decode` refuses udt 1 on purpose - it is AhToClient, and that direction
    // guard is a feature. Read it back with the oracle directly: the *same* `FUN_002A4090` the
    // client runs, just without this crate's send/receive policy on top.
    let mut oracle = d1_oracle::activity_host::AhCodec::new();
    let mut read_back = |body: &Vec<u8>| -> Option<u16> {
        // body = {u8 kind}{u64be recipient}{u32be udt}{u32be len}{payload}
        let d = oracle.decode(1, &body[0x11..]).ok()?;
        Some(u16::from_le_bytes(
            d.object[GlobalActivityState::ACTIVITY_INDEX..GlobalActivityState::ACTIVITY_INDEX + 2]
                .try_into()
                .ok()?,
        ))
    };
    let zero_idx = zero.as_ref().and_then(&mut read_back);
    let clash_idx = clash.as_ref().and_then(&mut read_back);
    c.push(check(
        "⭐⭐ THE INDEX SURVIVES THE ENCODER: a published global_activity_state carries the \
         activity index at +0x680 through the CLIENT'S OWN codec, both ways",
        zero_idx == Some(0) && clash_idx == Some(393),
        format!(
            "index 0 -> {zero_idx:?}, index 393 -> {clash_idx:?} after FUN_002A4140 then \
             FUN_002A4090. 0 is what every run before 2026-08-09 sent, and it is \
             activity_table[0] = HASH(0x8AE8DA69) = \"Tower\"",
        ),
    ));
    c.push(check(
        "⛔ NEGATIVE: changing only the index changes the WIRE - same length, different bytes",
        match (&zero, &clash) {
            (Some(a), Some(b)) => a.len() == b.len() && a != b,
            _ => false,
        },
        "a field the encoder ignores would produce identical bytes, which is exactly how a \
         write-only struct offset ships unnoticed",
    ));

    // ⭐⭐ THE CHECKSUM at +0x5C - the field that aborts rung 33's world transition.
    // AXIS THIS CONTROL PROTECTS, stated before it was written: "a checksum we set must survive
    // the client's own encoder AND be distinguishable on the wire." A setter that writes a struct
    // offset the schema does not carry would round-trip as 0 and ship silently - which is exactly
    // how `blob+0x5C = 0` reached every run in project history without anyone noticing.
    let encode_ck = |ck: u32| -> Option<Vec<u8>> {
        let mut inst = ActivityInstance::new(7, "d1ah-ck", now);
        inst.on_connect(now);
        inst.set_activity_checksum(ck);
        let mut img = [0u8; 0x100];
        img[0x10..0x38].copy_from_slice(&[0x5Au8; 0x28]);
        let pushes = inst.on_client_message(&ClientMessage::from_struct_image(3, &img).ok()?, now);
        let one = pushes.into_iter().find(|p| p.message.user_defined_type() == 1)?;
        one.to_bap_body(&SchemaCodec).ok()
    };
    let ck_zero = encode_ck(0);
    let ck_real = encode_ck(0xEBB8_1A5F);
    let mut read_ck = |body: &Vec<u8>| -> Option<u32> {
        let d = oracle.decode(1, &body[0x11..]).ok()?;
        Some(u32::from_le_bytes(
            d.object[GlobalActivityState::CHECKSUM..GlobalActivityState::CHECKSUM + 4]
                .try_into()
                .ok()?,
        ))
    };
    let got_zero = ck_zero.as_ref().and_then(&mut read_ck);
    let got_real = ck_real.as_ref().and_then(&mut read_ck);
    c.push(check(
        "⭐⭐ THE CHECKSUM SURVIVES THE ENCODER: global_activity_state carries the activity \
         checksum at +0x5C through the CLIENT'S OWN codec, both ways",
        got_zero == Some(0) && got_real == Some(0xEBB8_1A5F),
        format!(
            "checksum 0 -> {got_zero:?}, 0xEBB81A5F -> {got_real:?} after FUN_002A4140 then \
             FUN_002A4090. 0 is what every run to date sent, and FUN_00299100 rejects it with \
             `Activity checksum mismatch!` -> transition_error -> the world change completes to \
             the EMPTY STRING (runs/20260809-084932, 3 of 3)",
        ),
    ));
    c.push(check(
        "⛔ NEGATIVE: changing only the checksum changes the WIRE - same length, different bytes",
        match (&ck_zero, &ck_real) {
            (Some(a), Some(b)) => a.len() == b.len() && a != b,
            _ => false,
        },
        "+0x5C must be a field the schema actually carries; if it were not, both encodings would \
         be byte-identical and the knob would be a no-op that looks like a fix",
    ));

    // ⭐⭐ THE BUBBLE COUNT at +0x10 and the UNCHOSEN bytes at +0x14 - rung 33's SECOND world-
    // transition gate, `FUN_002A0320`, which runs the instant `FUN_00299100` (the checksum) passes.
    // AXIS THIS CONTROL PROTECTS, stated before writing it: "the count AND the 0xFF bubble bytes
    // must survive the client's own encoder." `blob+0x14` is a 64-byte nested struct (0x80804050)
    // whose descriptor names a *single* 8-bit field - so it is entirely plausible the codec carries
    // only one byte and zeroes the other 63. If so, publishing all-0xFF is a no-op that looks like a
    // fix, exactly like `blob+0x5C = 0` did. This control is the empirical answer, both ways.
    let encode_bubbles = |count: u32| -> Option<Vec<u8>> {
        let mut inst = ActivityInstance::new(7, "d1ah-bub", now);
        inst.on_connect(now);
        inst.set_bubble_count(count);
        let mut img = [0u8; 0x100];
        img[0x10..0x38].copy_from_slice(&[0x5Au8; 0x28]);
        let pushes = inst.on_client_message(&ClientMessage::from_struct_image(3, &img).ok()?, now);
        let one = pushes.into_iter().find(|p| p.message.user_defined_type() == 1)?;
        one.to_bap_body(&SchemaCodec).ok()
    };
    // Decode a udt-1 body back into the 1712-byte struct image with the client's own FUN_002A4090.
    let decode_state = |body: &Vec<u8>| -> Option<Vec<u8>> {
        let mut o = d1_oracle::activity_host::AhCodec::new();
        o.decode(1, &body[0x11..]).ok().map(|d| d.object)
    };
    let bub16 = encode_bubbles(16);
    let decoded16 = bub16.as_ref().and_then(decode_state);
    // Read the count (+0x10) and the 16 unchosen bytes (+0x14..+0x24) out of the round-tripped image.
    let count_back = decoded16.as_ref().map(|img| {
        u32::from_le_bytes(
            img[GlobalActivityState::BUBBLE_COUNT..GlobalActivityState::BUBBLE_COUNT + 4]
                .try_into()
                .unwrap(),
        )
    });
    let bytes_all_ff = decoded16.as_ref().map(|img| {
        (0..16).all(|i| img[GlobalActivityState::BUBBLE_STATE + i] == 0xFF)
    });
    let surviving = decoded16.as_ref().map(|img| {
        (0..16).filter(|&i| img[GlobalActivityState::BUBBLE_STATE + i] == 0xFF).count()
    });
    c.push(check(
        "⭐⭐ THE BUBBLE COUNT SURVIVES THE ENCODER: global_activity_state carries the u32 at +0x10 \
         through the CLIENT'S OWN codec, both ways",
        count_back == Some(16),
        format!(
            "count 16 -> {count_back:?} after FUN_002A4140 then FUN_002A4090. FUN_002A0320 requires \
             this == scenario_record+0x48 (orbit = 16, live); 0 -> `invalid host activity state`",
        ),
    ));
    c.push(check(
        "⭐⭐ THE 0xFF UNCHOSEN BYTES SURVIVE THE ENCODER: all 16 of blob+0x14+i round-trip as 0xFF \
         (the benign `bubble not chosen` path in FUN_002A0320)",
        bytes_all_ff == Some(true),
        format!(
            "{}/16 bytes survived as 0xFF. blob+0x14 is the 64-byte struct 0x80804050 whose \
             descriptor names a single 8-bit field - if the codec carried only that byte, the \
             other bubbles would decode 0 (`state 0 chosen`) and FUN_002A0320 would reject the \
             ones whose scenario state-0 is invalid",
            surviving.map(|n| n.to_string()).unwrap_or_else(|| "?".into())
        ),
    ));
    c.push(check(
        "⛔ NEGATIVE: publishing a bubble count changes the WIRE - count 0 and count 16 differ in \
         both length-or-bytes",
        match (&encode_bubbles(0), &bub16) {
            (Some(a), Some(b)) => a != b,
            _ => false,
        },
        "if +0x10/+0x14 were fields the encoder ignores, the two encodings would be byte-identical \
         and D1_AH_BUBBLE_COUNT would be a no-op that looks like a fix",
    ));

    // ⭐⭐ THE CHOSEN BUBBLES at +0x14 - rung 34's gate, `error.sstm_slice_not_found`.
    // AXIS THIS CONTROL PROTECTS, stated before writing it: "a bubble marked CHOSEN (0x00) must
    // arrive at the client as 0x00, and the unchosen ones must still arrive as 0xFF, THROUGH THE
    // CLIENT'S OWN CODEC." The failure this guards against is specific and has a precedent in this
    // very file: `blob+0x14` is a 64-byte nested struct whose descriptor names one 8-bit field, so
    // an encoder that carried only the first byte would make D1_AH_BUBBLE_CHOSEN look like it
    // worked for bubble 0 and silently do nothing for every other bubble. It is inverted below.
    let encode_chosen = |count: u32, chosen: &[usize]| -> Option<Vec<u8>> {
        let mut inst = ActivityInstance::new(7, "d1ah-bub", now);
        inst.on_connect(now);
        inst.set_bubble_count(count);
        inst.set_bubble_chosen(chosen.to_vec());
        let mut img = [0u8; 0x100];
        img[0x10..0x38].copy_from_slice(&[0x5Au8; 0x28]);
        let pushes = inst.on_client_message(&ClientMessage::from_struct_image(3, &img).ok()?, now);
        let one = pushes.into_iter().find(|p| p.message.user_defined_type() == 1)?;
        one.to_bap_body(&SchemaCodec).ok()
    };
    // `orbit`'s real shape, measured live 2026-08-09: bubbles 0-4 and 9-14 have a non-zero
    // sub-bubble count and are the exact 11 the live slice-set registry has registered; 5-8 and 15
    // have count 0 and MUST stay unchosen or FUN_002A0320 emits "…state itself was invalid".
    let orbit_chosen: Vec<usize> = (0..=4).chain(9..=14).collect();
    let decoded_orbit = encode_chosen(16, &orbit_chosen).as_ref().and_then(decode_state);
    let chosen_ok = decoded_orbit.as_ref().map(|img| {
        (0..16).all(|i| {
            let want = if orbit_chosen.contains(&i) { 0x00 } else { 0xFF };
            img[GlobalActivityState::BUBBLE_STATE + i] == want
        })
    });
    let chosen_got = decoded_orbit
        .as_ref()
        .map(|img| (0..16).map(|i| img[GlobalActivityState::BUBBLE_STATE + i]).collect::<Vec<_>>());
    c.push(check(
        "⭐⭐ THE CHOSEN/UNCHOSEN PATTERN SURVIVES THE ENCODER: orbit's measured shape \
         (0-4,9-14 chosen = 0x00; 5-8,15 unchosen = 0xFF) round-trips through the CLIENT'S OWN codec",
        chosen_ok == Some(true),
        format!(
            "decoded blob+0x14[0..16] = {chosen_got:02X?}. A CHOSEN bubble is what makes \
             FUN_002A0320 call FUN_00343020 with the scenario tag and REGISTER its slice-set \
             region; an unchosen one calls it with -1 and UNREGISTERS the region, which is rung \
             34's `Region 'PRV00.00' cannot be found!` = error.sstm_slice_not_found"
        ),
    ));
    c.push(check(
        "⛔ NEGATIVE: choosing a bubble changes the WIRE - chosen-none and chosen-{0} differ",
        match (&encode_chosen(1, &[]), &encode_chosen(1, &[0])) {
            (Some(a), Some(b)) => a != b,
            _ => false,
        },
        "if the codec dropped the chosen byte, both encodings would be byte-identical and \
         D1_AH_BUBBLE_CHOSEN would be a no-op that looks exactly like a fix - the precise failure \
         mode that made D1_AH_ACTIVITY_CHECKSUM worth controlling",
    ));
    c.push(check(
        "⛔ REFUSAL: a chosen index at or beyond the published bubble count is an ERROR, not a \
         silent no-op",
        {
            let mut s = GlobalActivityState::empty();
            s.set_bubble_count(1).is_ok()
                && s.set_bubble_chosen(&[0]).is_ok()
                && s.set_bubble_chosen(&[1]).is_err()
        },
        "the client walks only `[0, bubble_count)` bubble-state bytes, so an out-of-range index \
         would be published and never read - a knob that appears set and does nothing",
    ));

    // ─────────────────────────────────────────────────────────────────────────────────────────
    // ⭐⭐ RUNG 34 - THE PAH-TABULATED REGION RECORD in replicate_membership+0x2E48.
    //
    // AXIS THIS CONTROL PROTECTS, stated before writing it: "the region id at record+0x00 and the
    // per-peer token byte at record+0x38+peer must SURVIVE THE CLIENT'S OWN CODEC, and the array
    // terminator at record[1]+0x00 must survive as -1."
    //
    // The failure it guards against is the same shape that made D1_AH_BUBBLE_CHOSEN worth
    // controlling, only worse: `0x80800BD7` is 8,672 bytes of nested arrays, and `0x80805491` (the
    // 20-byte per-peer token array) is declared as a struct with ONE 8-bit field. An encoder that
    // carried only the declared field would emit a record whose region id is right and whose token
    // byte is zero - and the client would silently ignore it, exactly as it has ignored the all-zero
    // array every run to date. That is a fix that looks like a fix and is not one.
    // ⇒ `historicaldocs/design/rung34-task7.md`.
    let decode_membership = |body: &Vec<u8>| -> Option<Vec<u8>> {
        let mut o = d1_oracle::activity_host::AhCodec::new();
        o.decode(11, &body[0x11..]).ok().map(|d| d.object)
    };
    let encode_membership = |region: Option<(u32, usize, u8)>| -> Option<Vec<u8>> {
        let mut inst = ActivityInstance::new(7, "d1ah-pah", now);
        inst.on_connect(now);
        if let Some((r, p, t)) = region {
            inst.set_pah_region(r, p, t);
        }
        let mut img = [0u8; 0x100];
        img[0x10..0x38].copy_from_slice(&[0x5Au8; 0x28]);
        let pushes = inst.on_client_message(&ClientMessage::from_struct_image(3, &img).ok()?, now);
        let m = pushes.into_iter().find(|p| p.message.user_defined_type() == 11)?;
        m.to_bap_body(&SchemaCodec).ok()
    };
    const PAH: usize = ReplicateMembership::PAH_REGIONS;
    const STRIDE: usize = ReplicateMembership::PAH_REGION_STRIDE;
    const TOKENS: usize = ReplicateMembership::PAH_PEER_TOKENS;
    const ACK_HOST: usize = ReplicateMembership::ACKNOWLEDGED_SESSION_HOST;
    let decoded_host_ack = encode_membership(Some((0, 0, 1)))
        .as_ref()
        .and_then(decode_membership)
        .map(|img| img[ACK_HOST..ACK_HOST + 8].to_vec());
    c.push(check(
        "⭐⭐ THE SESSION-HOST ACKNOWLEDGEMENT SURVIVES THE ENCODER at membership+0x5020",
        decoded_host_ack == Some(vec![0x5A; 8]),
        format!(
            "decoded host identity = {decoded_host_ack:02x?}, want the first qword of member[0] \
             ([5a; 8]). FUN_0029FE40 -> FUN_002A4970 compares exactly these eight bytes with the \
             local host identifier; zero produces the repeating 'AH has not acknowledged this' \
             world-controller warning"
        ),
    ));
    let decoded_pah = encode_membership(Some((0, 0, 1))).as_ref().and_then(decode_membership);
    let pah_read = decoded_pah.as_ref().map(|img| {
        (
            u32::from_le_bytes(img[PAH..PAH + 4].try_into().unwrap()),
            img[PAH + TOKENS],
            u32::from_le_bytes(img[PAH + STRIDE..PAH + STRIDE + 4].try_into().unwrap()),
        )
    });
    c.push(check(
        "⭐⭐ THE PAH-TABULATED REGION RECORD SURVIVES THE ENCODER: region id, the per-peer token \
         byte at +0x38, and the -1 terminator all round-trip through the CLIENT'S OWN codec",
        pah_read == Some((0, 1, u32::MAX)),
        format!(
            "decoded (region_id, peer_token[0], next_region_id) = {pah_read:?}, want \
             (0, 1, 0xFFFFFFFF). FUN_007775B0 copies a record into sstm+0x490 iff region_id == \
             sstm+0x138 AND record[0x38 + local_peer_index] == sstm+0x1D0; FUN_0077F6B0 then \
             requires sstm+0x490 != -1 or it prints \"we do not yet have valid host-tabulated data \
             for it. Waiting.\" - which is where rung 34 has stopped on every run"
        ),
    ));
    c.push(check(
        "⛔ NEGATIVE: publishing the record CHANGES THE WIRE - a zeroed tabulated array and a \
         populated one differ",
        match (&encode_membership(None), &encode_membership(Some((0, 0, 1)))) {
            (Some(a), Some(b)) => a != b,
            _ => false,
        },
        "if the codec dropped replicate_membership field [3] (0x80800BD7), both encodings would be \
         byte-identical and D1_AH_SLICE_TOKEN would be a no-op that looks exactly like a fix",
    ));
    c.push(check(
        "⛔ NEGATIVE: the TOKEN is carried, not just the region id - token 1 and token 2 differ",
        match (&encode_membership(Some((0, 0, 1))), &encode_membership(Some((0, 0, 2)))) {
            (Some(a), Some(b)) => a != b,
            _ => false,
        },
        "the per-peer array 0x80805491 declares ONE 8-bit field for 20 bytes; an encoder that \
         carried only the declared byte would still pass the region-id check above and publish a \
         zero token, which the client silently ignores",
    ));
    c.push(check(
        "⛔ NEGATIVE: a token written for peer 1 does NOT land in peer 0's byte",
        match (&encode_membership(Some((0, 0, 1))), &encode_membership(Some((0, 1, 1)))) {
            (Some(a), Some(b)) => a != b,
            _ => false,
        },
        "the byte is indexed by FUN_0029FE70's local peer index; an encoder that collapsed the \
         20-byte array to one byte would make every peer index look like peer 0",
    ));
    c.push(check(
        "⛔ REFUSAL: a peer index at or beyond the 20-slot per-peer array is an ERROR, not a \
         silent write past the record",
        {
            let mut m = ReplicateMembership { image: vec![0u8; ReplicateMembership::STRUCT_SIZE] };
            m.set_pah_region(0, 19, 1).is_ok() && m.set_pah_region(0, 20, 1).is_err()
        },
        "0x80805491 is 20 bytes with stride 1; index 20 would land in the record's next field \
         (0x808043BC, the advertised public session) and corrupt it silently",
    ));

    // ⭐⭐ THE AXIS THE FIVE CONTROLS ABOVE DO NOT TEST: a STATE REFRESH must not zero the record.
    //
    // Every control above drives the JOIN path. The client sends `RequestStateRefresh` **0.37 s
    // after the join** (measured, `20260809-120451-MOONBASE-FRESH` 20:57:41.264 → .632), and that
    // handler rebuilt the membership from `solo()` - a pure echo - WITHOUT re-applying the PAH
    // record. Last writer wins, so the client's own copy at `c_activity_client+0x3CB8` read ALL
    // ZEROS while the identity echo in the same object was byte-correct, and rung 34 task 8 parked
    // forever on "we do not yet have valid host-tabulated data for it. Waiting."
    //
    // ⚠ The reason this hid for a whole session: `publish_activity_state()` is re-derived from
    // `self` on both paths, so every BLOB knob (checksum, bubble count, bubble chosen) survived a
    // refresh and worked. Only the membership is rebuilt from the echo. A green join-path control
    // is not evidence about the refresh path. ⇒ historicaldocs/design/rung34-task7.md, status/LESSONS.md.
    let encode_membership_after_refresh = |region: Option<(u32, usize, u8)>| -> Option<Vec<u8>> {
        let mut inst = ActivityInstance::new(7, "d1ah-pah-refresh", now);
        inst.on_connect(now);
        if let Some((r, p, t)) = region {
            inst.set_pah_region(r, p, t);
        }
        let mut img = [0u8; 0x100];
        img[0x10..0x38].copy_from_slice(&[0x5Au8; 0x28]);
        // The join first - this is what stores the request image the refresh echoes.
        let _ = inst.on_client_message(&ClientMessage::from_struct_image(3, &img).ok()?, now);
        // Then the refresh, exactly as the live client sends it.
        let pushes = inst.on_client_message(
            &ClientMessage::RequestStateRefresh { struct_image: vec![0u8; 8] },
            now,
        );
        let m = pushes.into_iter().find(|p| p.message.user_defined_type() == 11)?;
        m.to_bap_body(&SchemaCodec).ok()
    };
    let refreshed_pah = encode_membership_after_refresh(Some((0, 0, 1)))
        .as_ref()
        .and_then(decode_membership)
        .map(|img| {
            (
                u32::from_le_bytes(img[PAH..PAH + 4].try_into().unwrap()),
                img[PAH + TOKENS],
                u32::from_le_bytes(img[PAH + STRIDE..PAH + STRIDE + 4].try_into().unwrap()),
            )
        });
    c.push(check(
        "⭐⭐ A STATE REFRESH PRESERVES THE PAH-TABULATED RECORD - the refresh re-publishes the \
         membership 0.37 s after the join, and must not overwrite it with a zeroed echo",
        refreshed_pah == Some((0, 1, u32::MAX)),
        format!(
            "after RequestStateRefresh the decoded (region_id, peer_token[0], next_region_id) = \
             {refreshed_pah:?}, want (0, 1, 0xFFFFFFFF). This is the bug that parked rung 34 task \
             8 on 2026-08-09: the join published the record, the refresh replaced it with a pure \
             solo() echo, and the client's copy at c_activity_client+0x3CB8 read all zeros"
        ),
    ));
    c.push(check(
        "⛔ NEGATIVE: the refresh's membership is the SAME on the wire as the join's - proving the \
         control above compares a real re-publish, not a dropped push",
        match (&encode_membership(Some((0, 0, 1))), &encode_membership_after_refresh(Some((0, 0, 1))))
        {
            (Some(a), Some(b)) => a == b,
            _ => false,
        },
        "if the refresh stopped pushing udt 11 entirely the control above would still be green \
         (nothing overwrites the record), so it must be paired with proof that a push happened",
    ));

    c
}

/// The 238 bytes the live client sent on 2026-08-08, as bytes. Owned by `d1-oracle`; re-exposed
/// here so a control can use it without another hex helper.
pub fn captured_join_request() -> Vec<u8> {
    let s = d1_oracle::activity_host::CAPTURED_JOIN_REQUEST;
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

/// ⭐⭐ Full passes over the establish-client message set run by [`leak_controls`].
///
/// At the instance's 5,125 ms heartbeat this is a simulated **17 minutes** - comfortably past the
/// thirteen at which run `20260809-120451-MOONBASE-FRESH` died, and cheap enough to live in
/// `controls`. `d1-oracle activity-host leak --cycles 2000` is the multi-hour version.
pub const LEAK_CYCLES: usize = 200;

/// ⭐⭐ **THE AXIS: does an Activity Host started with NO environment publish a world the client
/// can actually transition into - and does everything that is OURS to assert survive the refresh?**
///
/// Stated before any assertion below was written, because that is the only order in which a control
/// can fail honestly:
///
/// > *"With an empty environment, the four values rung 33 gates on (MAP name, MODE index, scenario
/// > checksum, bubble count + chosen set) and the one value rung 34 gates on (the PAH-tabulated
/// > region record) all reach the wire through the CLIENT'S OWN codec carrying the measured
/// > `pvp_moonbase` / 407 configuration - and the `udt 17` state refresh, which rebuilds the
/// > membership from a pure echo 0.37 s later, does not lose any of it."*
///
/// ## Why this group exists at all, and what it is inverted against
///
/// The crate's default used to be `act_cosmo_m0_highway` / index 0 / checksum 0 / bubble count 0 /
/// no chosen bubble / no slice token - **every documented rung-33 and rung-34 wall at once**. The
/// good values lived only in the operator's shell, no launcher exported them, and that default had
/// already silently shipped in runs that meant to use the good set. So the axis a mere round-trip
/// control does NOT test is *which values*: an encoder that faithfully round-trips zeros passes
/// every existing control in this file. Each assertion below therefore names the **exact expected
/// value**, and reverting `DEFAULT_ACTIVITY_NAME`, `DEFAULT_ACTIVITY_INDEX` or `DEFAULT_SLICE_TOKEN`
/// makes it red - watched, not assumed.
///
/// ⚠ **What this group is NOT.** It is not evidence that the client accepts the configuration. It
/// asserts what we PUBLISH, decoded by the client's own `FUN_002A4090`. A server-side receipt
/// proves you BUILT it, never that the client KEPT it. The live discriminator stays the operator's.
fn served_world_controls() -> Vec<Control> {
    use crate::worlds;
    let mut c = Vec::new();
    let now = Instant::now();

    // ── 1. the table itself ───────────────────────────────────────────────────────────────────
    c.push(check(
        "⭐⭐ THE DEFAULT WORLD IS pvp_moonbase - the ONLY configuration this project has ever \
         walked to activity:in_world, not the act_* name that published every wall at once",
        ActivityInstance::DEFAULT_ACTIVITY_NAME == "pvp_moonbase"
            && worlds::DEFAULT_WORLD.name == "pvp_moonbase"
            && ActivityInstance::DEFAULT_ACTIVITY_NAME != "act_cosmo_m0_highway",
        format!(
            "DEFAULT_ACTIVITY_NAME = {:?}. The previous default paired with checksum 0 / bubble \
             count 0 / no chosen bubble - every documented rung-33/34 wall at once - and runs \
             20260810-144332-RESTART-CLEANNAME, 20260810-145553-X11-ORBIT and \
             20260810-161501-RDOC-ORBIT-JANK each opened their activity-host.log with \
             `publishing activity name \"act_cosmo_m0_highway\"` + `WARN activity index 0`, \
             because the good values were only ever the operator's shell variables and NO \
             launcher exports them (grep D1_AH_ACTIVITY over capture-rig/*.sh: no hits)",
            ActivityInstance::DEFAULT_ACTIVITY_NAME
        ),
    ));
    c.push(check(
        "⭐⭐ THE MODE IS 407 = ELIMINATION, and it is a SEPARATE constant from the MAP - a launch \
         is TWO values and neither is derived from the other",
        ActivityInstance::DEFAULT_ACTIVITY_INDEX == 407
            && (393..=413).contains(&ActivityInstance::DEFAULT_ACTIVITY_INDEX),
        format!(
            "index {} at blob+0x{:03X}, name at blob+0x{:03X} - adjacent, independent fields \
             ([7] @+0x620 u16 and [10] @+0x628 char[0x28] of 0x80800CC5). Only indices 393-413 \
             are launchable (activity_def[+0xA9]); everything else stalls at rung 29 matchmaking",
            ActivityInstance::DEFAULT_ACTIVITY_INDEX,
            GlobalActivityState::ACTIVITY_INDEX,
            GlobalActivityState::NAME
        ),
    ));
    c.push(check(
        "⭐ the world table carries the MEASURED scenario-record values for both captured worlds",
        worlds::PVP_MOONBASE.checksum == 0x5966_E491
            && worlds::PVP_MOONBASE.bubble_count == 1
            && worlds::PVP_MOONBASE.chosen == [0]
            && worlds::ORBIT.checksum == 0x33F0_44D3
            && worlds::ORBIT.bubble_count == 16
            && !worlds::ORBIT.chosen.contains(&5)
            && !worlds::ORBIT.chosen.contains(&15),
        "pvp_moonbase +0x0C=0x5966E491 +0x48=1; orbit +0x0C=0x33F044D3 +0x48=16 with bubbles 5-8 \
         and 15 NOT choosable (their scenario sub-bubble count is 0, measured live 16/16, which is \
         why a blanket 0x00 failed there)",
    ));

    // ── 2. the resolver: one name decides three fields, and a contradiction is LOUD ────────────
    let empty_env = worlds::resolve(None, None, None, None);
    c.push(check(
        "⭐⭐ WITH NO ENVIRONMENT AT ALL the resolver yields the full measured moonbase set - this \
         is what an Activity Host started with a bare `serve` now publishes",
        empty_env.name == "pvp_moonbase"
            && empty_env.checksum == 0x5966_E491
            && empty_env.bubble_count == 1
            && empty_env.chosen == vec![0]
            && empty_env.disagreements.is_empty(),
        format!("{empty_env:?}"),
    ));
    let orbit_env = worlds::resolve(Some("orbit"), None, None, None);
    c.push(check(
        "⭐⭐ NAMING A DIFFERENT WORLD MOVES ALL THREE FIELDS TOGETHER - the cross-world \
         misconfiguration that re-fails gate 1 is now unrepresentable by omission",
        orbit_env.checksum == 0x33F0_44D3
            && orbit_env.bubble_count == 16
            && orbit_env.chosen == vec![0, 1, 2, 3, 4, 9, 10, 11, 12, 13, 14]
            && orbit_env.disagreements.is_empty(),
        format!(
            "resolve(\"orbit\") = checksum {:#010x}, {} bubbles, chosen {:?}. Publishing one \
             world's checksum while naming another re-fails FUN_00299100, because the client \
             compares against the record of the world it ACTUALLY LOADED",
            orbit_env.checksum, orbit_env.bubble_count, orbit_env.chosen
        ),
    ));
    let crossed = worlds::resolve(Some("orbit"), Some(0x5966_E491), Some(1), None);
    c.push(check(
        "⛔ NEGATIVE: naming `orbit` while overriding moonbase's checksum AND bubble count is \
         reported as TWO disagreements at startup, not discovered three rungs later",
        crossed.disagreements.len() == 2
            && crossed.checksum == 0x5966_E491
            && crossed.bubble_count == 1,
        format!("{:?}", crossed.disagreements),
    ));
    let lobster = worlds::resolve(None, None, None, Some(Vec::new()));
    c.push(check(
        "⛔ THE LOBSTER NEGATIVE CONTROL SURVIVES THE DEFAULTING: an EXPLICITLY empty chosen set \
         still publishes all-0xFF, and says so - `unset` and `set to empty` are different facts",
        lobster.chosen.is_empty() && lobster.disagreements.iter().any(|d| d.contains("LOBSTER")),
        format!(
            "resolve(chosen = Some(vec![])) -> chosen {:?}, {} disagreement(s). Collapsing this \
             into the default would delete the only reachable reproduction of \
             error.sstm_slice_not_found",
            lobster.chosen,
            lobster.disagreements.len()
        ),
    ));
    let unmeasured = worlds::resolve(Some("pvp_vertigo"), None, None, None);
    c.push(check(
        "⛔ REFUSAL: an UNMEASURED world gets zeros and a warning, NOT a guessed checksum - an \
         invented value is byte-indistinguishable on the wire from a stale one",
        unmeasured.world.is_none()
            && unmeasured.checksum == 0
            && unmeasured.bubble_count == 0
            && unmeasured.chosen.is_empty(),
        "a world's scenario record is resident only while it loads; a new row costs one live \
         capture at --hz 500, and `lookup` returns None rather than interpolating",
    ));

    // ── 3. the whole served set, THROUGH THE CLIENT'S OWN CODEC, both ways ─────────────────────
    //
    // ⚠ Everything above is arithmetic over our own tables. This is the part that can fail for a
    // reason nobody predicted: a field the schema does not carry round-trips as 0 and ships
    // silently, which is exactly how `blob+0x5C = 0` reached every run in project history.
    let served_state = {
        let r = worlds::resolve(None, None, None, None);
        let mut inst = ActivityInstance::new(7, "d1ah-served", now);
        inst.on_connect(now);
        inst.set_activity_name(r.name.clone());
        inst.set_activity_index(ActivityInstance::DEFAULT_ACTIVITY_INDEX);
        inst.set_activity_checksum(r.checksum);
        inst.set_bubble_count(r.bubble_count);
        inst.set_bubble_chosen(r.chosen.clone());
        inst.set_pah_region(0, 0, ActivityInstance::DEFAULT_SLICE_TOKEN);
        let mut img = [0u8; 0x100];
        img[0x10..0x38].copy_from_slice(&[0x5Au8; 0x28]);
        ClientMessage::from_struct_image(3, &img)
            .ok()
            .map(|m| (inst.on_client_message(&m, now), inst))
    };
    let decoded_served = served_state.as_ref().and_then(|(pushes, _)| {
        let body = pushes
            .iter()
            .find(|p| p.message.user_defined_type() == 1)?
            .to_bap_body(&SchemaCodec)
            .ok()?;
        let mut o = d1_oracle::activity_host::AhCodec::new();
        o.decode(1, &body[0x11..]).ok().map(|d| d.object)
    });
    let served_fields = decoded_served.as_ref().map(|img| {
        let name_field = &img[GlobalActivityState::NAME
            ..GlobalActivityState::NAME + GlobalActivityState::NAME_CAPACITY];
        let end = name_field.iter().position(|b| *b == 0).unwrap_or(name_field.len());
        (
            String::from_utf8_lossy(&name_field[..end]).to_string(),
            u16::from_le_bytes(
                img[GlobalActivityState::ACTIVITY_INDEX..GlobalActivityState::ACTIVITY_INDEX + 2]
                    .try_into()
                    .unwrap(),
            ),
            u32::from_le_bytes(
                img[GlobalActivityState::CHECKSUM..GlobalActivityState::CHECKSUM + 4]
                    .try_into()
                    .unwrap(),
            ),
            u32::from_le_bytes(
                img[GlobalActivityState::BUBBLE_COUNT..GlobalActivityState::BUBBLE_COUNT + 4]
                    .try_into()
                    .unwrap(),
            ),
            img[GlobalActivityState::BUBBLE_STATE],
            img[GlobalActivityState::BUBBLE_STATE + 1],
        )
    });
    c.push(check(
        "⭐⭐⭐ THE WHOLE DEFAULT SET SURVIVES THE CLIENT'S OWN CODEC: name `pvp_moonbase`, index \
         407, checksum 0x5966E491, bubble count 1, bubble 0 CHOSEN (0x00) and bubble 1 untouched",
        served_fields == Some(("pvp_moonbase".to_string(), 407, 0x5966_E491, 1, 0x00, 0x00)),
        format!(
            "decoded (name, index, checksum, count, bubble[0], bubble[1]) = {served_fields:?}, \
             want (\"pvp_moonbase\", 407, 0x5966E491, 1, 0x00, 0x00) after FUN_002A4140 then \
             FUN_002A4090. bubble[0] = 0x00 is CHOSEN, which is what REGISTERS the slice-set \
             region rung 34 then asks for; 0xFF there is LOBSTER"
        ),
    ));
    c.push(check(
        "⛔ NEGATIVE: the served default is DISTINGUISHABLE on the wire from the old all-walls \
         default - if it were not, this whole change would be a no-op that looks like a fix",
        {
            let old = GlobalActivityState::for_activity("act_cosmo_m0_highway")
                .and_then(|s| s.struct_bytes());
            let new = GlobalActivityState::for_activity("pvp_moonbase").map(|mut s| {
                s.set_activity_index(407);
                s.set_activity_checksum(0x5966_E491);
                let _ = s.set_bubble_count(1);
                let _ = s.set_bubble_chosen(&[0]);
                s
            });
            match (old, new.and_then(|s| s.struct_bytes())) {
                (Ok(a), Ok(b)) => a != b,
                _ => false,
            }
        },
        "the two blobs differ in the name, the index, the checksum, the bubble count and the \
         bubble-state byte - five fields, one world",
    ));

    // ── 4. ⭐⭐ THE REFRESH AXIS, made STRUCTURAL rather than enumerated ────────────────────────
    //
    // The bug this replaces: `ReplicateMembership::solo()` is a pure ECHO of the client's own
    // join_request, the client sends `udt 17` 0.37 s after the join, and the refresh handler
    // rebuilt the membership from `solo()` WITHOUT re-applying the one field that is ours. Last
    // writer wins, so the client's own copy read all zeros while the identity echo was byte-
    // correct, and rung 34 task 8 parked forever.
    //
    // The existing controls above assert the PAH record specifically. This one asserts the
    // *property* that made the bug possible - the join and the refresh must produce the SAME
    // BYTES - which is what `ActivityInstance::build_membership` now guarantees by construction.
    // Applying a future field in the JoinRequest arm instead of in that builder makes this red.
    let membership_both_paths = |region: Option<(u32, usize, u8)>| -> (Option<Vec<u8>>, Option<Vec<u8>>) {
        let mut inst = ActivityInstance::new(7, "d1ah-both-paths", now);
        inst.on_connect(now);
        if let Some((r, p, t)) = region {
            inst.set_pah_region(r, p, t);
        }
        let mut img = [0u8; 0x100];
        img[0x10..0x38].copy_from_slice(&[0x5Au8; 0x28]);
        let joined = match ClientMessage::from_struct_image(3, &img) {
            Ok(m) => inst
                .on_client_message(&m, now)
                .into_iter()
                .find(|p| p.message.user_defined_type() == 11)
                .and_then(|p| p.to_bap_body(&SchemaCodec).ok()),
            Err(_) => None,
        };
        let refreshed = inst
            .on_client_message(&ClientMessage::RequestStateRefresh { struct_image: vec![0u8; 8] }, now)
            .into_iter()
            .find(|p| p.message.user_defined_type() == 11)
            .and_then(|p| p.to_bap_body(&SchemaCodec).ok());
        (joined, refreshed)
    };
    let (join_body, refresh_body) = membership_both_paths(Some((
        0,
        0,
        ActivityInstance::DEFAULT_SLICE_TOKEN,
    )));
    c.push(check(
        "⭐⭐⭐ ONE BUILDER, TWO PATHS: the udt-11 the JOIN pushes and the udt-11 the udt-17 \
         REFRESH pushes are BYTE-IDENTICAL on the wire, for the served slice token",
        match (&join_body, &refresh_body) {
            (Some(a), Some(b)) => a == b && !a.is_empty(),
            _ => false,
        },
        format!(
            "join {} bytes, refresh {} bytes, IDENTICAL={}. ⚠ Equal LENGTHS are not the assertion - \
             the inversion (rebuilding the refresh from solo() alone) produces two 7314-byte \
             bodies that DIFFER, which is precisely why this compares content. `solo()` is a pure \
             echo and the refresh rebuilds from it, so anything applied only in the JoinRequest \
             arm is zeroed 0.37 s later - the bug that parked rung 34 task 8 on 2026-08-09. \
             ActivityInstance::build_membership is now the ONLY place a membership field may be \
             applied, and this control is what makes applying one elsewhere go red",
            join_body.as_ref().map_or(0, |b| b.len()),
            refresh_body.as_ref().map_or(0, |b| b.len()),
            join_body.is_some() && join_body == refresh_body
        ),
    ));
    c.push(check(
        "⛔ NEGATIVE: the two paths differ when the SERVED VALUE differs - proving the control \
         above compares real content, not two empty pushes",
        {
            let (a, _) = membership_both_paths(Some((0, 0, 1)));
            let (b, _) = membership_both_paths(Some((0, 0, 2)));
            let (z, _) = membership_both_paths(None);
            match (a, b, z) {
                (Some(a), Some(b), Some(z)) => a != b && a != z,
                _ => false,
            }
        },
        "token 1, token 2 and a zeroed array must be three distinct encodings; if they were not, \
         byte-identity between the paths would be trivially true and the control decoration",
    ));

    // ── 5. ⛔⛔ THE LAUNCH-SLOT REFUSAL, with the measurement that closed it ────────────────────
    //
    // AXIS, stated first: "can this crate publish a populated activity slot at blob+0x68?"
    // The answer is NO, and the control pins WHY so nobody re-attempts it blind. See
    // `GlobalActivityState::LAUNCH_SLOT_COUNT` for the three findings.
    c.push(check(
        "⭐ the launch-slot array's geometry is derived from the descriptor tree and cross-checked \
         by the two constants either side of it",
        GlobalActivityState::LAUNCH_SLOT_COUNT == 0x64
            && GlobalActivityState::LAUNCH_SLOTS == 0x68
            && GlobalActivityState::LAUNCH_SLOT_STRIDE == 0x18
            && GlobalActivityState::LAUNCH_SLOT_CAPACITY == 50
            && GlobalActivityState::LAUNCH_SLOTS
                + GlobalActivityState::LAUNCH_SLOT_CAPACITY * GlobalActivityState::LAUNCH_SLOT_STRIDE
                == 0x518,
        "0x80800CC5 [1] @+0x004 -> 0x8080533D {count: kind 0x5 LEAF 6 bits; array 0x8080522B, \
         1200 B, element 0x80800D17, 24 B}; blob+0x60 + 0x04 = 0x64 and + 0x08 = 0x68, and the \
         array ends exactly where 0x80800CC5's next field [2] @+0x4b8 begins",
    ));
    let slot_refusal = {
        let mut s = GlobalActivityState::for_activity("pvp_moonbase").expect("blob");
        let set = s.set_launch_slot_unencodable(0, 0xDEAD_BEEF);
        (set.is_ok(), s.launch_slot_count(), s.struct_bytes().is_err())
    };
    c.push(check(
        "⛔⛔ REFUSED IN THE CRATE: a blob with a non-zero activity-slot COUNT cannot be turned \
         into wire bytes - a named error at build time, never a Unicorn fault at PUSH time",
        slot_refusal == (true, 1, true),
        format!(
            "(set ok, count, struct_bytes refused) = {slot_refusal:?}. The live Activity Host \
             encodes through the same oracle, so an unguarded non-zero count would fail every \
             global_activity_state push mid-session - the shape that already killed a run once as \
             error.private_activity_host_host_crashed"
        ),
    ));
    // ⭐ THE EMPIRICAL RECEIPT, not our own guard testing our own guard: drive the CLIENT'S OWN
    // encoder directly, with a positive control on the same call.
    let slot_encode = |count: u32, hash: u32| -> Result<usize, String> {
        let mut img = vec![0u8; 0x6B0];
        img[0] = 1;
        img[GlobalActivityState::LAUNCH_SLOT_COUNT..GlobalActivityState::LAUNCH_SLOT_COUNT + 4]
            .copy_from_slice(&count.to_le_bytes());
        img[GlobalActivityState::LAUNCH_SLOTS..GlobalActivityState::LAUNCH_SLOTS + 4]
            .copy_from_slice(&hash.to_le_bytes());
        img[GlobalActivityState::ACTIVITY_INDEX..GlobalActivityState::ACTIVITY_INDEX + 2]
            .copy_from_slice(&407u16.to_le_bytes());
        img[GlobalActivityState::NAME..GlobalActivityState::NAME + 12]
            .copy_from_slice(b"pvp_moonbase");
        let mut o = d1_oracle::activity_host::AhCodec::new();
        o.encode(1, &img).map(|w| w.payload.len()).map_err(|e| e.to_string())
    };
    // fnv1("") - what an unset slot reads on the live client.
    let empty_ok = slot_encode(0, 0x811C_9DC5);
    let populated = slot_encode(1, 0x811C_9DC5);
    c.push(check(
        "⭐⭐ THE MEASUREMENT THAT CLOSED THE LAUNCH-SLOT CANDIDATE: count 0 ENCODES (the positive \
         control on the same call) and ANY non-zero count FAULTS the client's own encoder",
        empty_ok.is_ok() && populated.is_err(),
        format!(
            "count=0 -> {empty_ok:?}; count=1 -> {populated:?}. The element's second field \
             (0x80800D17 [1] @+0x004, kind 0x29) is a DYNAMIC-WIDTH leaf: 0x105B410 derefs the \
             content registry *DAT_01A0B420 to size it, and a statically mapped image does not \
             carry it. Same class as the kind-0x22 udts in warm_undecodable_set()"
        ),
    ));
    c.push(check(
        "⛔ NEGATIVE: with the count at 0 the slot bytes DO NOT REACH THE WIRE AT ALL - three \
         different slot-0 hashes encode byte-identically, so the count is the gate, not the hash",
        {
            let mut o = d1_oracle::activity_host::AhCodec::new();
            let enc = |o: &mut d1_oracle::activity_host::AhCodec, hash: u32| -> Option<Vec<u8>> {
                let mut img = vec![0u8; 0x6B0];
                img[0] = 1;
                img[GlobalActivityState::LAUNCH_SLOTS..GlobalActivityState::LAUNCH_SLOTS + 4]
                    .copy_from_slice(&hash.to_le_bytes());
                o.encode(1, &img).ok().map(|w| w.payload)
            };
            match (enc(&mut o, 0), enc(&mut o, 0x811C_9DC5), enc(&mut o, 0xDEAD_BEEF)) {
                (Some(a), Some(b), Some(d)) => a == b && b == d,
                _ => false,
            }
        },
        "so `populate slot 0 and see what happens` cannot be tried by writing the hash alone - it \
         needs the count, and the count is what faults. ⇒ the candidate is closed offline, and \
         separately the KTOBUCKET ACTNAME print reads the ACTIVITY LOADER (FUN_0033FF30), not this \
         array, so populating a slot could not have made ACTNAME real either",
    ));

    c
}

/// ⭐⭐ **THE TEAM AXIS** - what an ACTIVITY ID means for `Multiplayer Team`.
///
/// **The axis, in one sentence:** *for a given activity ID, the crate names which of the client's
/// two non-matchmaking team assigners `FUN_007ABB50` will run - COPY-FROM-FIRETEAM for 407,
/// SEQUENTIAL for 413 - and separately asserts that the ID the assigner reads is **not** a field
/// any Activity Host writes.*
///
/// ⛔⛔ **THE SECOND HALF OF THAT SENTENCE IS A REPAIR, 2026-08-10.** The first revision of this
/// block said *"for the activity index this Activity Host publishes"* and shipped
/// `D1_AH_ACTIVITY_INDEX=413` as *"a one-variable change to a field this crate already
/// publishes"*. `d1-eboot disasm 0x7abb50` says otherwise: the ID comes from
/// `network_session_parameters(0)+0x5A8` (`desired_activity`) `+2` at `0x7ABC42`, while
/// `D1_AH_ACTIVITY_INDEX` writes `global_activity_state+0x680`. An operator following that
/// recipe would have spent a boot changing a field the assigner never reads and read the null
/// result as *"sequential does not work"*. ⇒ [`fireteam::ASSIGNER_ID_SESSION_PARAM_OFFSET`].
///
/// ⛔ **The axis a round-trip control does NOT test**, and it is the one that matters here: the
/// encoder is perfectly happy to serialise index 407 forever while the client's dump prints
/// `Multiplayer Team=-1` on every single run, because the team byte is **not in any message**. So
/// these controls assert a *derivation over measured client bytes*, and the inversion that proves
/// they can fail is stubbing [`crate::fireteam::LaunchableActivity::team_assignment`] to a
/// constant - which makes four of them red at once. The three repair controls have their own,
/// sharper inversion: put either mislabelled offset back and they go red individually.
///
/// ⚠ **What a green run here does NOT license.** It says a gametype *can* assign for some ID. It
/// says nothing about whether that ID is reachable, nothing about the activity-launch countdown,
/// and nothing about anything this server sends. All three gates are client-local.
fn served_team_controls() -> Vec<Control> {
    use crate::fireteam::{self, TeamAssignment};
    let mut c = Vec::new();

    // ── 1. the table, and the partition three independent measurements agree on ────────────────
    c.push(check(
        // ⛔⛔ TWO OBJECTS, ONE NAME - and the inherited phrasing conflated them. FUN_005441C0
        // reaches its record through vtable slot 0x320 and tests `*(char*)(o+0xA9) != -1`; the
        // "activity def" whose +0x10/+0x15 THIS TABLE measures is a DIFFERENT record, reached by
        // FUN_005444A0 through slot 0x328 and a self-relative pointer (`*(i64*)(o+0x20)+o+0x20`),
        // stride 0x30 - +0xA9 is outside it entirely. So the band is named by the refusal predicate,
        // not by a field of the row.
        "⭐ the launchable band is the 21 contiguous indices 393-413 - the ones FUN_005441C0 admits \
         (`*(char*)(slot_0x320_record + 0xA9) != -1`), i.e. the only ones that skip rung 29 \
         matchmaking. ⚠ that record is NOT the slot-0x328 activity def whose +0x10/+0x15 this table \
         carries; the band is INHERITED here, not re-derived",
        fireteam::LAUNCHABLE.len() == 21
            && fireteam::LAUNCHABLE.first().map(|a| a.index) == Some(393)
            && fireteam::LAUNCHABLE.last().map(|a| a.index) == Some(413)
            && fireteam::LAUNCHABLE.windows(2).all(|w| w[1].index == w[0].index + 1),
        format!(
            "{} row(s), {}..={}",
            fireteam::LAUNCHABLE.len(),
            fireteam::LAUNCHABLE.first().map(|a| a.index).unwrap_or_default(),
            fireteam::LAUNCHABLE.last().map(|a| a.index).unwrap_or_default()
        ),
    ));
    c.push(check(
        "⭐⭐ THREE INDEPENDENTLY MEASURED FIELDS PARTITION THE 21 IDS THE SAME WAY: \
         def+0x10 == 6 <=> def+0x15 == 0 <=> gametype+0x14 == 2 <=> SEQUENTIAL. A transcription \
         slip in any one row breaks the agreement",
        fireteam::LAUNCHABLE.iter().all(|a| {
            let by_mode = a.gametype_team_mode == 2;
            let by_override = a.def_gametype_override == 0;
            let by_field = a.def_field_0x10 == 6;
            by_mode == by_override
                && by_override == by_field
                && by_mode == (a.team_assignment() == TeamAssignment::Sequential)
        }),
        format!(
            "sequential rows: {:?}; all other rows read (mode 3, override 1, +0x10 in {{12, 4}}). \
             Every value replayed out of FUN_00544AC0 on runs/snapshots/orbit-all.snap \
             (20 PASS / 0 FAIL / 0 SKIP)",
            fireteam::sequential_team_indices()
        ),
    ));
    c.push(check(
        "⭐ the 21 ids share exactly THREE gametype records, and the mode-2 set is exactly the one \
         that shares 0x380AAFF40 - so the split is a property of the CONTENT, not of our table",
        {
            let mut recs: Vec<u64> = fireteam::LAUNCHABLE.iter().map(|a| a.gametype_record).collect();
            recs.sort_unstable();
            recs.dedup();
            recs.len() == 3
                && fireteam::LAUNCHABLE.iter().all(|a| {
                    (a.gametype_record == 0x3_80AA_FF40)
                        == (a.team_assignment() == TeamAssignment::Sequential)
                })
        },
        "0x380AAFE80 (mode 3, 15 ids) · 0x380AAFF40 (mode 2, 5 ids) · 0x380AB0000 (mode 3, id 411 \
         alone). Absolute addresses in runs/snapshots/orbit-all.snap, carried as provenance so a \
         stale row is falsifiable against a fresh capture",
    ));

    // ── 2. ⭐⭐ THE AXIS ITSELF, on the two indices that matter ─────────────────────────────────
    c.push(check(
        "⛔⛔ THE DEFAULT ID SELECTS THE ASSIGNER THAT NEEDS A SOURCE: activity 407 = ELIMINATION \
         is gametype team-mode 3 COPY-FROM-FIRETEAM, whose source is fireteam player +0x7A - a \
         DIFFERENT field from the +0xB0 the dump prints, and one whose writer set is [U], not empty",
        fireteam::team_assignment(ActivityInstance::DEFAULT_ACTIVITY_INDEX)
            == TeamAssignment::CopyFromFireteam
            && !fireteam::team_assignment(ActivityInstance::DEFAULT_ACTIVITY_INDEX)
                .assigns_a_team(),
        format!(
            "id {} -> {:?}. FUN_00B97EA0 sets every posse player's +0xB0 to 0xFF (0xB97F42) and \
             then copies the FIRETEAM player's +0x7A (0xB97F8B -> 0xB97F93) - a DIFFERENT field. \
             The only visible write to that source is FUN_00B805F0 find_or_add_player setting it \
             to 0xFF (0xB80974), so an untouched player copies 0xFF and the dump movsx-es it to \
             -1. ⚠ [U] the full writer set for +0x7A is unknown, not empty: an indexed heap \
             element is invisible to xref AND field-xref. ⇒ `Multiplayer Team=-1` is CORRECT \
             BEHAVIOUR for this id, and no Activity Host change can move it - separately from the \
             fact that no Activity Host writes the id the assigner reads at all",
            ActivityInstance::DEFAULT_ACTIVITY_INDEX,
            fireteam::team_assignment(ActivityInstance::DEFAULT_ACTIVITY_INDEX)
        ),
    ));
    c.push(check(
        "⭐⭐ AND THE A/B EXISTS: 413 is gametype team-mode 2 SEQUENTIAL, which needs no source - \
         FUN_00B97E40 writes 0,1,2… and a solo posse gets team 0",
        fireteam::team_assignment(413) == TeamAssignment::Sequential
            && fireteam::team_assignment(413).assigns_a_team()
            && fireteam::sequential_team_indices() == vec![398, 399, 408, 412, 413],
        format!(
            "sequential ids {:?}. ⛔ NOT AN A/B THIS SERVER CAN RUN, but the OPERATOR can: \
             FUN_007ABB50 keys on network_session_parameters(0)+0x{:03X} (`desired_activity`) +{}, \
             which D1_AH_ACTIVITY_INDEX (global_activity_state+0x{:03X}) does not write - and which \
             FUN_00789D90 DOES write, copying *(u16*)(value_of(&params->activity)+2). \
             d1-eboot xref 0x789d90 = 9 confirmed callers; 0x7A5250 \
             (setup_activity_host_setup_handler::vfunc[7]) and 0x795AF0 pass value_of(&activity) \
             literally. params->activity is params+0x9B8, m_value+2 = exactly the word \
             `session-params.py --launch-poke --activity-id N` writes (+0x9B8+0xEA). ⇒ THE A/B IS \
             `--launch-poke --activity-id 413` WITH D1_AH_ACTIVITY_INDEX LEFT AT 407, because +0x680 \
             feeds the rung-33 unlock evaluator (0x78C6D0/0x78C770/0x78C8B0) and moving it perturbs \
             a solved gate while measuring nothing here. ⚠ [U] narrows to whether those callers' \
             world-controller-state guards open on our route - a READ settles it, not a poke",
            fireteam::sequential_team_indices(),
            fireteam::ASSIGNER_ID_SESSION_PARAM_OFFSET,
            fireteam::ASSIGNER_ID_VALUE_OFFSET,
            GlobalActivityState::ACTIVITY_INDEX
        ),
    ));
    c.push(check(
        "⛔ NEGATIVE WITH A POSITIVE CONTROL: the two answers are DIFFERENT for 407 and 413 - a \
         table that returned one verdict for everything would pass every assertion above it",
        fireteam::team_assignment(407) != fireteam::team_assignment(413)
            && fireteam::team_assignment(407).assigns_a_team()
                != fireteam::team_assignment(413).assigns_a_team(),
        format!(
            "407 -> {:?}, 413 -> {:?}",
            fireteam::team_assignment(407),
            fireteam::team_assignment(413)
        ),
    ));

    // ── 2b. ⛔⛔ THE REPAIR CONTROLS - the axes the first revision of this block got WRONG ──────
    //
    // A wrong LABEL on a right MEASUREMENT is invisible to every control that only checks the
    // measurement, which is exactly how the two defects below shipped green. Each of these pins a
    // literal instruction displacement, so the inversion is putting the mislabelled number back.
    c.push(check(
        "⛔⛔ THE ID THE ASSIGNER READS IS NOT THE INDEX WE PUBLISH - two offsets, two objects. \
         FUN_007ABB50 takes it from network_session_parameters(0)+0x5A8 (`desired_activity`) +2; \
         D1_AH_ACTIVITY_INDEX writes global_activity_state+0x680",
        fireteam::ASSIGNER_ID_SESSION_PARAM_OFFSET == 0x5A8
            && fireteam::ASSIGNER_ID_VALUE_OFFSET == 0x02
            && fireteam::PUBLISHED_ACTIVITY_INDEX_OFFSET
                == GlobalActivityState::ACTIVITY_INDEX as u32
            && fireteam::ASSIGNER_ID_SESSION_PARAM_OFFSET
                != fireteam::PUBLISHED_ACTIVITY_INDEX_OFFSET,
        format!(
            "assigner reads +0x{:03X}+{}, we publish +0x{:03X}. Instruction-level: `0x7ABC25 call \
             0x773FC0` (network_session_parameters - it reads [0x26B1850], stride 0x131C8, inner \
             0x98E0, state +0x8C58 mask 0x7F0, params +0x6B38, every constant identical to \
             capture-rig/session-params.py), `0x7ABC2A lea rdi,[rax+0x5A8]`, `0x7ABC31 call \
             0x1203040` = c_generic_network_session_parameter<T>::vfunc[18] by RTTI, `0x7ABC3C je` \
             on NULL (unset ⇒ the whole assignment block is skipped), `0x7ABC42 movzx edi, word \
             [rbx+2]`. ⭐ Its WRITER is settled offline: FUN_00789D90 copies \
             *(u16*)(value_of(&params->activity)+2) into it, 9 confirmed callers, two passing \
             value_of(&activity) literally (0x7A5250, 0x795AF0). ⚠ [U] narrows to whether those \
             callers' world-controller-state guards open on OUR route; the live discriminator is a \
             READ - `session-params.py --session 0` before and after --launch-poke",
            fireteam::ASSIGNER_ID_SESSION_PARAM_OFFSET,
            fireteam::ASSIGNER_ID_VALUE_OFFSET,
            fireteam::PUBLISHED_ACTIVITY_INDEX_OFFSET
        ),
    ));
    c.push(check(
        "⛔⛔ THE COPY SOURCE AND DESTINATION ARE DIFFERENT FIELDS - +0x7A in, +0xB0 out - and each \
         constant reconstructs the absolute displacement of the instruction it was read from",
        fireteam::FIRETEAM_PLAYER_TEAM_SOURCE_OFFSET != fireteam::POSSE_PLAYER_TEAM_OFFSET
            && fireteam::SESSION_PLAYER_ARRAY_OFFSET
                + fireteam::FIRETEAM_PLAYER_TEAM_SOURCE_OFFSET
                == 0x1CD2
            && fireteam::SESSION_PLAYER_ARRAY_OFFSET + fireteam::POSSE_PLAYER_TEAM_OFFSET == 0x1D08
            && fireteam::SESSION_PLAYER_STRIDE == 0xE8,
        format!(
            "src player+0x{:02X} (abs 0x{:04X}), dst player+0x{:02X} (abs 0x{:04X}), array +0x{:04X} \
             stride 0x{:X}. `0xB97F8B mov al,[r13+rcx+0x1CD2]` -> `0xB97F93 mov [r12+0xB0],al`, \
             with r12 = `0xB97F3A lea r12,[r14+rax+0x1C58]` and rax = `0xB97F33 imul rax,0xE8`. \
             ⛔ FUN_00B97FC0 stores to `0xB9807D [r15+rdx+0x1D08]` = +0xB0 = the DESTINATION, so \
             naming it \"the only writer of the source\" was a non-sequitur about a different \
             field, and the `Matchmaking for activity done` census attached to it was evidence \
             about matchmaking. FUN_00B805F0 writes BOTH at player-add: `0xB80974` byte 0xFF to \
             +0x7A, `0xB80890`/`0xB8097D` dword 0xFFFFFFFF to +0xB0. The dump reads +0xB0 as a \
             SIGNED BYTE - FUN_00AE94D0 `0xAE966F movsx r9d, byte [r15+rbx+0x1D08]` - which is why \
             0xFF prints -1 and a sequential 0 prints 0",
            fireteam::FIRETEAM_PLAYER_TEAM_SOURCE_OFFSET,
            fireteam::SESSION_PLAYER_ARRAY_OFFSET + fireteam::FIRETEAM_PLAYER_TEAM_SOURCE_OFFSET,
            fireteam::POSSE_PLAYER_TEAM_OFFSET,
            fireteam::SESSION_PLAYER_ARRAY_OFFSET + fireteam::POSSE_PLAYER_TEAM_OFFSET,
            fireteam::SESSION_PLAYER_ARRAY_OFFSET,
            fireteam::SESSION_PLAYER_STRIDE
        ),
    ));
    c.push(check(
        "⛔⛔ NO OPERATOR-FACING RECEIPT MAY RECOMMEND A LEVER THAT CANNOT MOVE THE FIELD - not one \
         of the 21 receipts prescribes `D1_AH_ACTIVITY_INDEX=<id>`, and the mode-3 receipt names \
         `desired_activity` and its offset instead. This is the control the first revision needed: \
         the defect WAS shipped control text, and no assertion over the measurement could see it",
        {
            let all: Vec<crate::fireteam::TeamReceipt> =
                fireteam::LAUNCHABLE.iter().map(|a| fireteam::receipt(a.index)).collect();
            let none_prescribes = all.iter().all(|r| !r.message.contains("D1_AH_ACTIVITY_INDEX="));
            let m3 = fireteam::receipt(ActivityInstance::DEFAULT_ACTIVITY_INDEX);
            let m2 = fireteam::receipt(413);
            none_prescribes
                && m3.message.contains("desired_activity")
                && m3.message.contains("0x5A8")
                && m3.message.contains("params+0x9B8")
                && m3.message.contains("[U]")
                && m2.message.contains("desired_activity")
                && m2.message.contains("NO Activity Host writes")
        },
        format!(
            "21/21 receipts free of a prescriptive `D1_AH_ACTIVITY_INDEX=`; the mode-3 receipt \
             names desired_activity, 0x5A8, params+0x9B8 and carries its [U]. Replaced text: \
             \"`D1_AH_ACTIVITY_INDEX=413` is the one-variable A/B\" - which would have cost a boot \
             and returned a false negative, because it writes \
             global_activity_state+0x{:03X} and FUN_007ABB50 reads \
             network_session_parameters(0)+0x{:03X}",
            fireteam::PUBLISHED_ACTIVITY_INDEX_OFFSET,
            fireteam::ASSIGNER_ID_SESSION_PARAM_OFFSET
        ),
    ));

    // ── 3. the refusal, and the words the operator actually reads ──────────────────────────────
    c.push(check(
        "⛔ REFUSAL: an index outside the measured band is Unmeasured, NOT rounded to a mode - an \
         invented team mode reads exactly like a measured one at the call site that consumes it",
        fireteam::team_assignment(7) == TeamAssignment::Unmeasured
            && fireteam::team_assignment(0) == TeamAssignment::Unmeasured
            && fireteam::team_assignment(414) == TeamAssignment::Unmeasured
            && !fireteam::team_assignment(7).assigns_a_team()
            && fireteam::lookup(7).is_none(),
        "activity 7 = Patrol Cosmodrome (activity_def[+0xA9] == 0xFF). Replaying FUN_00544AC0(7) \
         on the snapshot returns rec[1] == 0 - NO gametype record at all - which is the inverted \
         control for the whole measurement: a lookup that fabricated rows would have produced one",
    ));
    let default_receipt = fireteam::receipt(ActivityInstance::DEFAULT_ACTIVITY_INDEX);
    c.push(check(
        "⭐⭐ THE STARTUP RECEIPT NAMES THE SYMPTOM, NOT JUST THE MODE - it prints \
         `Multiplayer Team=-1`, the assigner, why its source is empty, and the indices that are \
         not. This failure is SILENT by construction: nothing refuses and nothing errors",
        default_receipt.is_warning
            && default_receipt.message.contains("Multiplayer Team=-1")
            && default_receipt.message.contains("FUN_00B97EA0")
            && default_receipt.message.contains("413")
            && !fireteam::receipt(413).is_warning
            && fireteam::receipt(413).message.contains("FUN_00B97E40")
            && fireteam::receipt(413).message.contains("is_activity_launch_countdown_active"),
        format!("default receipt: {}", default_receipt.message),
    ));
    c.push(check(
        "⚠ AND THE SEQUENTIAL RECEIPT REFUSES TO OVERSELL: it names the SECOND gate - the \
         activity-launch countdown - which is a session parameter and is the OPERATOR'S, not a \
         byte any Activity Host can send",
        {
            let r = fireteam::receipt(413);
            r.message.contains("setup:orbit")
                && r.message.contains("countdown_timer")
                && r.message.contains("operator")
        },
        "FUN_0078A070 requires FUN_00750A70(3) && world-controller state 0x1B setup:orbit && \
         network_session_parameters(0)->countdown_timer with FUN_00B8DF20(...) != -1. Our launch \
         route forges the goal instead, so handler+0x40 stays -1 and FUN_007ABB50's assignment \
         block is never entered - the client printed \
         `'is_activity_launch_countdown_active' failed, because we're not in state 'setup:orbit'` \
         in 2 of 255 run logs, one of them the rung-36 success",
    ));

    // ── 4. ⛔⛔ SCOPE, AND THE LEVER - the two things a green run here must NOT be read as ───────
    //
    // AXIS: "all controls pass" must not be readable as "the measurement was re-run", and no
    // operator-facing string may name a lever that cannot move the field. Both defects are SHIPPED
    // TEXT, so no assertion over the measured numbers can see either - which is exactly why they
    // shipped green once already. INVERSION: gut either constant and the matching check goes red.
    c.push(check(
        "⛔⛔ THE CRATE STATES ITS OWN SCOPE: LAUNCHABLE is a HAND-TRANSCRIBED table, nothing here \
         reaches the snapshot, and a UNIFORMLY WRONG transcription would pass every check above. \
         The re-measurement command ships with the crate so \"102 controls pass\" cannot be quoted \
         as \"the client was re-executed\"",
        {
            let r = fireteam::REMEASURE_RECIPE;
            r.contains("NOT RE-MEASURED")
                && r.contains("hand-transcribed")
                && r.contains("d1-replay")
                && r.contains("orbit-all.snap")
                && r.contains("0x544ac0")
                && r.contains("--stack-canary auto")
                && r.contains("id 7")
                // ⛔ and the band predicate, whose inherited phrasing named the WRONG OBJECT and
                // the WRONG SENSE: FUN_005441C0 admits on `!= -1` against the slot-0x320 record,
                // not `== 0` against the slot-0x328 def these rows measure.
                && r.contains("INHERITED, NOT DERIVED HERE")
                && r.contains("slot_0x320_record + 0xA9) != -1")
                && r.contains("slot-0x328")
        },
        format!("scope statement carried by the crate: {}", fireteam::REMEASURE_RECIPE),
    ));
    c.push(check(
        "⭐⭐ AND THE LEVER IS NAMED CORRECTLY: the A/B recipe prescribes `--launch-poke \
         --activity-id 413`, names the FUN_00789D90 -> desired_activity feed that makes it work, \
         and explicitly says to LEAVE D1_AH_ACTIVITY_INDEX AT 407 so the rung-33 unlock evaluator \
         is not perturbed while measuring something else",
        {
            let r = fireteam::TEAM_LANE_AB_RECIPE;
            r.contains("--launch-poke --activity-id 413")
                && r.contains("LEFT AT 407")
                && r.contains("FUN_00789D90")
                && r.contains("params+0x9B8")
                && r.contains("0x78C770")
                && !r.contains("D1_AH_ACTIVITY_INDEX=413")
        },
        format!(
            "the lever: {}. ⛔ The refuted text this replaces: \"D1_AH_ACTIVITY_INDEX=413 is the \
             one-variable A/B\" - a boot spent writing global_activity_state+0x{:03X} while \
             FUN_007ABB50 reads network_session_parameters(0)+0x{:03X}, returning a null result \
             that would have read as a refutation of the (sound) table",
            fireteam::TEAM_LANE_AB_RECIPE,
            fireteam::PUBLISHED_ACTIVITY_INDEX_OFFSET,
            fireteam::ASSIGNER_ID_SESSION_PARAM_OFFSET
        ),
    ));

    // ── udt 5 sensor_auth_update (the lifetime-phase writer) ──────────────────────────────────
    //
    // ⚠ These are pinned-fact / state-machine controls only. The byte-exact frame and the codec
    // round-trips live in `d1-oracle sensor controls`, which owns the Unicorn/eboot boundary this
    // crate must not grow.

    c.push(check(
        "udt 5 sensor_auth_update is AH→client and is what SensorAuthUpdate reports",
        protocol::direction(protocol::UDT_SENSOR_AUTH_UPDATE) == Some(protocol::AhDirection::AhToClient)
            && protocol::check_sendable(protocol::UDT_SENSOR_AUTH_UPDATE).is_ok()
            && AhMessage::SensorAuthUpdate { payload: vec![0u8; 59] }.user_defined_type()
                == protocol::UDT_SENSOR_AUTH_UPDATE,
        format!(
            "udt {} direction {:?}",
            protocol::UDT_SENSOR_AUTH_UPDATE,
            protocol::direction(protocol::UDT_SENSOR_AUTH_UPDATE)
        ),
    ));

    // The codec sends the pre-built sensor bytes VERBATIM - no re-encode, no schema path - because
    // the frame is bespoke and already assembled. The inversion: a plain schematized message that
    // is NOT passed through verbatim, so this cannot pass by echoing everything.
    {
        let marker: Vec<u8> = (0..59u8).collect();
        let sensor = AhMessage::SensorAuthUpdate { payload: marker.clone() };
        // SchemaCodec special-cases SensorAuthUpdate and returns the pre-built bytes verbatim,
        // WITHOUT touching the oracle (the special case returns before any oracle call), so this
        // is safe to run without the eboot. The inversion: RawStructCodec has no special case, so
        // it routes through `struct_bytes()`, which for this variant defaults to EMPTY - it does
        // NOT reproduce the frame. Same message, two codecs, only one passes it through.
        let via_schema = SchemaCodec.encode(&sensor);
        let via_raw = RawStructCodec.encode(&sensor);
        c.push(check(
            "SchemaCodec sends a pre-built sensor frame VERBATIM (no oracle, no re-encode)",
            via_schema.as_deref() == Ok(marker.as_slice())
                && via_raw.as_deref() != Ok(marker.as_slice()),
            format!(
                "schema_verbatim={} raw_reproduced={}",
                via_schema.as_deref() == Ok(marker.as_slice()),
                via_raw.as_deref() == Ok(marker.as_slice())
            ),
        ));
    }

    // The send trigger fires exactly once, and only when the instance is Live, armed, AND at least
    // one heartbeat has fired (so the world is resident, not merely joined). Four inversions in one:
    // silent before Live, silent at join time (Live but no heartbeat yet), silent when unarmed,
    // and silent on the second call after it has fired once.
    {
        use crate::messages::ClientMessage;
        let t0 = Instant::now();
        let hb = ActivityInstance::HOST_HEARTBEAT_MS;
        let mut armed = ActivityInstance::new(1, "s", t0);
        armed.set_sensor_payload(vec![0xAB; 59]);
        let before_live = armed.sensor_due(t0).is_none();
        // Join ⇒ Live in the same turn, but heartbeats == 0, so the sensor must NOT ride the join.
        let join_burst = armed.on_client_message(
            &ClientMessage::JoinRequest { struct_image: vec![0u8; protocol::JOIN_REQUEST_STRUCT_SIZE] },
            t0,
        );
        let is_live_after_join = matches!(armed.state(), InstanceState::Live);
        let no_sensor_on_join = !join_burst
            .iter()
            .any(|p| p.message.user_defined_type() == protocol::UDT_SENSOR_AUTH_UPDATE);
        let silent_at_join = armed.sensor_due(t0).is_none();
        // Let one heartbeat fire (a host_heartbeat later, on the quiet tick), then the sensor is due.
        let later = t0 + std::time::Duration::from_millis(hb + 1);
        let _ = armed.heartbeat_due(later);
        let first = armed.sensor_due(later);
        let second = armed.sensor_due(later);
        // ⭐ Repeat mode (D1_AH_SENSOR_REPEAT_S): still silent before Live and before the first
        // heartbeat, then fires, is paced by the interval (silent at the same instant, silent
        // inside the interval), and fires AGAIN past it - the send-once latch must not apply.
        let mut rep = ActivityInstance::new(1, "s", t0);
        rep.set_sensor_payload(vec![0xCD; 59]);
        rep.set_sensor_repeat(std::time::Duration::from_secs(10));
        let rep_before_live = rep.sensor_due(t0).is_none();
        let _ = rep.on_client_message(
            &ClientMessage::JoinRequest { struct_image: vec![0u8; protocol::JOIN_REQUEST_STRUCT_SIZE] },
            t0,
        );
        let rep_silent_at_join = rep.sensor_due(t0).is_none();
        let _ = rep.heartbeat_due(later);
        let rep_first = rep.sensor_due(later).is_some();
        let rep_paced_same_instant = rep.sensor_due(later).is_none();
        let rep_paced_inside = rep
            .sensor_due(later + std::time::Duration::from_secs(9))
            .is_none();
        let rep_second = rep
            .sensor_due(later + std::time::Duration::from_secs(10))
            .is_some();
        let rep_counted = rep.sensors_sent == 2;
        // A never-armed instance never sends, even Live and past a heartbeat.
        let mut unarmed = ActivityInstance::new(1, "s", t0);
        let _ = unarmed.on_client_message(
            &ClientMessage::JoinRequest { struct_image: vec![0u8; protocol::JOIN_REQUEST_STRUCT_SIZE] },
            t0,
        );
        let _ = unarmed.heartbeat_due(later);
        let unarmed_silent = unarmed.sensor_due(later).is_none();
        c.push(check(
            "sensor frame is sent exactly once, only when Live+armed+past-first-heartbeat; never unarmed",
            before_live
                && is_live_after_join
                && no_sensor_on_join
                && silent_at_join
                && matches!(&first, Some(p) if p.message == AhMessage::SensorAuthUpdate { payload: vec![0xAB; 59] })
                && second.is_none()
                && unarmed_silent
                && armed.sensors_sent == 1,
            format!(
                "before_live_silent={before_live} live={is_live_after_join} no_sensor_on_join={no_sensor_on_join} silent_at_join={silent_at_join} first={} second_none={} unarmed_silent={unarmed_silent}",
                first.is_some(),
                second.is_none()
            ),
        ));
        c.push(check(
            "sensor repeat mode (D1_AH_SENSOR_REPEAT_S): gated like send-once, paced by the interval, re-fires past it",
            rep_before_live
                && rep_silent_at_join
                && rep_first
                && rep_paced_same_instant
                && rep_paced_inside
                && rep_second
                && rep_counted,
            format!(
                "before_live_silent={rep_before_live} silent_at_join={rep_silent_at_join} first={rep_first} paced_same_instant={rep_paced_same_instant} paced_inside={rep_paced_inside} second_past_interval={rep_second} sensors_sent_2={rep_counted}"
            ),
        ));
    }

    c
}

/// ⭐⭐ **The control that would have caught the scratch-heap exhaustion**, plus its inversion.
///
/// Encodes all four establish-client messages `LEAK_CYCLES` times through [`SchemaCodec`] and
/// requires:
///
/// 1. every encode to **succeed** - the failure mode is a *later* call failing, so a loop is the
///    only shape that can see it;
/// 2. the oracle arena's **high-water mark to be identical** after cycle 1 and after cycle N;
/// 3. the **bytes to be identical** every cycle - reusing arena memory must not let one encode
///    read another's leftovers, which would be a silent wrong-wire bug worse than the crash;
/// 4. a **codec fault to be distinguishable from a protocol gate** ([`AhError::is_host_defect`]),
///    because in the run that motivated this they wore the same words at the same log level.
///
/// ⚠ The inversion for (1)-(3) lives in `d1-oracle` (`activity-host leak`), where the rewind can be
/// switched off; it is not reachable from here because this crate's codec is a unit struct with no
/// knob, which is the correct shape for a server.
fn leak_controls() -> Vec<Control> {
    use std::collections::BTreeMap;
    let mut c = Vec::new();

    let membership = match ReplicateMembership::solo(&[0u8; 0x100]) {
        Ok(m) => m,
        Err(e) => return vec![check("the leak control can build a solo membership", false, e.to_string())],
    };
    let state = match GlobalActivityState::for_activity("act_cosmo_m0_highway") {
        Ok(s) => s,
        Err(e) => return vec![check("the leak control can build a global_activity_state", false, e.to_string())],
    };
    let establish: Vec<AhMessage> = vec![
        AhMessage::JoinResult(JoinResult::accepted([1, 2, 3, 4], 0xD1, "leak-control", 0)),
        AhMessage::EntitySlotsAllocated(EntitySlotMask::empty()),
        AhMessage::GlobalActivityState(state),
        AhMessage::ReplicateMembership(membership),
    ];

    let mut reference: Option<Vec<Vec<u8>>> = None;
    let mut first_high: BTreeMap<u32, u64> = BTreeMap::new();
    let mut failure: Option<(usize, String)> = None;
    let mut divergent: Option<usize> = None;
    let mut done = 0usize;
    for i in 0..LEAK_CYCLES {
        let mut out = Vec::with_capacity(establish.len());
        for m in &establish {
            match SchemaCodec.encode(m) {
                Ok(w) => out.push(w),
                Err(e) => {
                    failure = Some((i, e.to_string()));
                    break;
                }
            }
        }
        if failure.is_some() {
            break;
        }
        match &reference {
            None => reference = Some(out),
            Some(r) if *r != out && divergent.is_none() => divergent = Some(i),
            Some(_) => {}
        }
        done = i + 1;
        if i == 0 {
            for s in SchemaCodec::arena_stats() {
                first_high.insert(s.user_defined_type, s.high_water);
            }
        }
    }

    let stats = SchemaCodec::arena_stats();
    let grew: Vec<String> = stats
        .iter()
        .filter(|s| first_high.get(&s.user_defined_type).is_some_and(|f| *f != s.high_water))
        .map(|s| {
            format!(
                "udt {}: {:#x}→{:#x}",
                s.user_defined_type, first_high[&s.user_defined_type], s.high_water
            )
        })
        .collect();

    c.push(check(
        "⭐⭐ THE ESTABLISH SET ENCODES 200× WITHOUT ONE FAILING - a per-call leak only shows up in          a loop, which is why 62 one-shot controls missed the one that killed MOONBASE-FRESH",
        done == LEAK_CYCLES && failure.is_none(),
        match &failure {
            Some((i, e)) => format!("** FAILED ON CYCLE {i} of {LEAK_CYCLES} ** {e}"),
            None => format!(
                "{done} cycles × {} messages = {} encodes, ≈{} min of a 5,125 ms heartbeat",
                establish.len(),
                done * establish.len(),
                done * 5125 / 60_000
            ),
        },
    ));

    c.push(check(
        "⭐⭐ the oracle arena's HIGH-WATER MARK IS FLAT across all of them",
        grew.is_empty() && !stats.is_empty(),
        if grew.is_empty() {
            stats.iter().map(|s| s.line()).collect::<Vec<_>>().join(" | ")
        } else {
            format!("** GROWING ** {}", grew.join("  "))
        },
    ));

    c.push(check(
        "⭐ every cycle emits IDENTICAL bytes - reusing the arena did not let one encode read          another's leftovers",
        divergent.is_none(),
        match divergent {
            Some(i) => format!("** DIVERGED AT CYCLE {i} - the wire changed, which is worse than the crash **"),
            None => "byte-for-byte identical; `alloc_aligned` zeroes what it hands out".to_string(),
        },
    ));

    c.push(check(
        "⭐ the guest's own code allocates NOTHING per call - the precondition that makes reusing          the arena sound rather than lucky",
        stats.iter().all(|s| s.guest_allocs == 0) && !stats.is_empty(),
        format!(
            "{} guest allocations across {} udts",
            stats.iter().map(|s| s.guest_allocs).sum::<u64>(),
            stats.len()
        ),
    ));

    // ⭐ The observability half. A codec that BROKE must not read as a protocol gate.
    let defect = crate::AhError::SchemaCodec {
        user_defined_type: 1,
        detail: "guest fault: scratch heap exhausted".into(),
    };
    let gate = crate::AhError::SchemaCodecMissing { user_defined_type: 42, descriptor_global: 0 };
    c.push(check(
        "⭐⭐ a CODEC FAULT is distinguishable from a NAMED GATE - with the negative control on the          same call",
        defect.is_host_defect()
            && !gate.is_host_defect()
            && !crate::AhError::ZeroActivityHostId.is_host_defect()
            && PushBlocked::Ah(crate::AhError::SchemaCodec {
                user_defined_type: 1,
                detail: String::new(),
            })
            .is_host_defect()
            && !PushBlocked::LogNotificationIsDeliverableNowhere.is_host_defect(),
        "MOONBASE-FRESH logged `scratch heap exhausted` as `** NAMED GATE ** push blocked`, at          INFO on the heartbeat path - the same wording and level as an unpinned tag. It now logs          at ERROR as AhEvent::CodecFault",
    ));

    c
}

/// Print the controls and return the process exit code (`0` pass, `2` fail).
pub fn main() -> i32 {
    let controls = run();
    let failed = controls.iter().filter(|c| !c.passed).count();
    for c in &controls {
        let tag = if c.passed { "PASS" } else { "FAIL" };
        if c.detail.is_empty() {
            println!("{tag}  {}", c.name);
        } else {
            println!("{tag}  {}  [{}]", c.name, c.detail);
        }
    }
    println!();
    if failed == 0 {
        println!("ALL CONTROLS PASS  ({} checks)", controls.len());
        0
    } else {
        println!("{failed} of {} CONTROLS FAILED - output may not be cited", controls.len());
        2
    }
}

#[cfg(test)]
mod tests {
    /// The controls are also a test, so `cargo test` fails on the same condition the binary
    /// exits 2 on. Two instruments, one answer.
    #[test]
    fn every_control_passes() {
        let failed: Vec<&str> =
            super::run().into_iter().filter(|c| !c.passed).map(|c| c.name).collect();
        assert!(failed.is_empty(), "failed controls: {failed:?}");
    }

    #[test]
    fn there_are_enough_controls_to_be_worth_running() {
        assert!(super::run().len() >= 20, "a controls suite this thin is decoration");
    }
}
