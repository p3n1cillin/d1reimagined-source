// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! Controls - run these before citing anything this crate says.
//!
//! Same contract as `d1-eboot controls`: every check is named, the run prints one line per check,
//! and **exit 2 means a control failed and the output may not be cited.**
//!
//! ## ⭐ The axis these controls test
//!
//! The PVE lane shipped 25 controls whose *names* cited eboot addresses but whose *bodies* compared
//! our constants to our constants - editing the table would still have passed every one. That shape
//! is banned here. The axis is **agreement with things we did not write**:
//!
//! * **shadPS4's own enum text**, vendored verbatim at `proto/shadps4-client-constants.h` and
//!   parsed at run time. Change a discriminant in `shadnet-wire` and these fail. And when the
//!   shadPS4 tree is present, the excerpt is itself re-checked as a **verbatim substring** of the
//!   upstream files plus a re-derived sha256 - so the excerpt cannot drift either.
//! * **The vendored `shadnet.proto` text**, parsed for the field numbers and types our code
//!   depends on, with its sha256 re-derived and compared against the value recorded in
//!   `ATTRIBUTION.md`. Never transcribed into a constant here.
//! * **Round-tripped bytes**: a handcrafted frame decoded by our reader must re-encode identically.
//!
//! Every negative check is paired with a positive arm, because a server that emitted nothing at all
//! would pass "it refuses a bad reply" on its own. *A control that only ever passes is decoration.*
//!
//! Two checks can report **SKIPPED** - the shadPS4 source tree is not required to build this crate.
//! A skip is printed and counted separately; it is **never** folded into the pass count.

use std::path::Path;

use prost::Message;
use sha2::{Digest, Sha256};

use crate::identity::{Identity, IdentityMode, ServerConfig};
use crate::matching;
use crate::registry::{endpoint_for, LoginRefusal, ServerState, SessionHandle};
use crate::rooms::{Endpoint, MemberIntent, RoomError, RoomSpec, RoomTable};
use crate::signaling::{parse_stun_ping, ONLINE_ID_LEN, STUN_PING_LEN};
use shadnet_proto as pb;
use shadnet_wire::{
    encode_notification, encode_reply, extract_blob, Command, ErrorType, EventCause, Notification,
    PacketType, RoomEvent, HEADER_SIZE,
};

/// One control's outcome. `Skipped` is a third state on purpose: silently passing a check whose
/// input is missing is exactly how the last two instrument defects hid under a green board.
pub struct Control {
    pub name: &'static str,
    pub state: State,
    pub detail: String,
}

#[derive(PartialEq, Eq)]
pub enum State {
    Pass,
    Fail,
    Skipped,
}

fn check(name: &'static str, cond: bool, detail: impl Into<String>) -> Control {
    Control {
        name,
        state: if cond { State::Pass } else { State::Fail },
        detail: detail.into(),
    }
}

fn skipped(name: &'static str, detail: impl Into<String>) -> Control {
    Control { name, state: State::Skipped, detail: detail.into() }
}

// ── parsers over text we did not write ───────────────────────────────────────────────────────

/// Pull `enum … { A = 1, B = 2 };` blocks out of C++ text.
fn cpp_enum_blocks(src: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let bytes = src.as_bytes();
    let mut i = 0;
    while let Some(rel) = src[i..].find("enum ") {
        let start = i + rel;
        // Only at the start of a line, so `OrbisNpMatching2EventCause ctx_event_cause{}` and the
        // like cannot be mistaken for a declaration.
        if start != 0 && bytes[start - 1] != b'\n' {
            i = start + 5;
            continue;
        }
        let Some(brace) = src[start..].find('{') else { break };
        let Some(end_rel) = src[start..].find("};") else { break };
        let header = src[start + 5..start + brace].trim();
        let name = header
            .trim_start_matches("class ")
            .split(':')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        out.push((name, src[start + brace + 1..start + end_rel].to_string()));
        i = start + end_rel + 2;
    }
    out
}

/// `A = 1,` → `("A", 1)`. Hex literals included, comments and blank lines dropped.
fn cpp_enum_entries(body: &str) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    for raw in body.lines() {
        let line = match raw.find("//") {
            Some(p) => &raw[..p],
            None => raw,
        };
        let line = line.trim().trim_end_matches(',').trim();
        let Some((name, value)) = line.split_once('=') else { continue };
        let name = name.trim();
        let value = value.trim();
        if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        let parsed = if let Some(hex) = value.strip_prefix("0x").or_else(|| value.strip_prefix("0X"))
        {
            u64::from_str_radix(hex, 16).ok()
        } else {
            value.parse::<u64>().ok()
        };
        if let Some(v) = parsed {
            out.push((name.to_string(), v));
        }
    }
    out
}

fn cpp_enum(src: &str, name: &str) -> Vec<(String, u64)> {
    cpp_enum_blocks(src)
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, body)| cpp_enum_entries(&body))
        .unwrap_or_default()
}

/// Compare a Rust table against a C++ enum, **both directions**. One direction alone would let us
/// silently drop a value the client defines.
fn same_set(rust: &[(&str, u64)], cpp: &[(String, u64)]) -> Result<usize, String> {
    if cpp.is_empty() {
        return Err("the C++ enum parsed to zero entries - the parser, not the data".into());
    }
    for (name, value) in rust {
        match cpp.iter().find(|(n, _)| n == name) {
            Some((_, v)) if v == value => {}
            Some((_, v)) => return Err(format!("{name}: ours {value}, client's {v}")),
            None => return Err(format!("{name} = {value} is not in the client's enum")),
        }
    }
    for (name, value) in cpp {
        if !rust.iter().any(|(n, _)| n == name) {
            return Err(format!("the client defines {name} = {value} and we do not"));
        }
    }
    Ok(cpp.len())
}

/// Find `message <name> { … }` in a `.proto` and return its body.
fn proto_message<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("message {name} ");
    let start = src.find(&needle).or_else(|| src.find(&format!("message {name}{{")))?;
    let open = src[start..].find('{')? + start;
    let mut depth = 0usize;
    for (i, c) in src[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[open + 1..open + i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// `repeated MatchingBinAttr bin_attrs = 5;` → `("MatchingBinAttr", "bin_attrs", 5, true)`.
fn proto_fields(body: &str) -> Vec<(String, String, u32, bool)> {
    let mut out = Vec::new();
    for raw in body.lines() {
        let line = match raw.find("//") {
            Some(p) => &raw[..p],
            None => raw,
        };
        let line = line.trim().trim_end_matches(';').trim();
        let Some((decl, num)) = line.rsplit_once('=') else { continue };
        let Ok(number) = num.trim().parse::<u32>() else { continue };
        let mut toks: Vec<&str> = decl.split_whitespace().collect();
        let repeated = toks.first() == Some(&"repeated");
        if repeated {
            toks.remove(0);
        }
        if toks.len() != 2 {
            continue;
        }
        out.push((toks[0].to_string(), toks[1].to_string(), number, repeated));
    }
    out
}

fn field<'a>(
    fields: &'a [(String, String, u32, bool)],
    name: &str,
) -> Option<&'a (String, String, u32, bool)> {
    fields.iter().find(|(_, n, _, _)| n == name)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

/// The 64-hex string that follows the first occurrence of `needle` in `src`.
fn sha256_after(src: &str, needle: &str) -> Option<String> {
    let start = src.find(needle)? + needle.len();
    let rest = &src[start..];
    let mut run = String::new();
    for c in rest.chars() {
        if c.is_ascii_hexdigit() {
            run.push(c);
            if run.len() == 64 {
                return Some(run);
            }
        } else if !run.is_empty() {
            run.clear();
        }
    }
    None
}

fn member_intent(conn: u64, npid: &str) -> MemberIntent {
    MemberIntent {
        conn_id: conn,
        npid: npid.into(),
        team_id: 0,
        endpoint: Endpoint { addr: format!("10.77.0.{conn}"), port: 3074 },
        nat_type: 2,
        flag_attr: 0,
        group_id: 0,
        account_id: 0,
        platform: 0,
        join_date: 0,
        bin_attrs_internal: vec![],
    }
}

fn test_config() -> ServerConfig {
    ServerConfig {
        identity: Identity {
            npid: "D1Guardian".into(),
            user_id: 1,
            token: "d1-local-token".into(),
            avatar_url: "http://127.0.0.1/avatar.png".into(),
        },
        matching2_enabled: false,
        rooms_enabled: true,
        identity_mode: IdentityMode::Fixed,
        peer_port: 3074,
        signaling_udp: false,
    }
}

/// Run every control. The caller decides the exit code.
pub fn run() -> Vec<Control> {
    let mut c = Vec::new();
    let cpp = crate::SHADPS4_CLIENT_CONSTANTS;
    let proto = crate::VENDORED_PROTO;

    // ── A. agreement with shadPS4's own enum text ────────────────────────────────────────────

    c.push(match same_set(
        &Command::ALL.iter().map(|(n, v)| (*n, *v as u64)).collect::<Vec<_>>(),
        &cpp_enum(cpp, "CommandType"),
    ) {
        Ok(n) => check("Command matches shadPS4's `enum class CommandType`, both directions", true, format!("{n} commands")),
        Err(e) => check("Command matches shadPS4's `enum class CommandType`, both directions", false, e),
    });

    c.push(match same_set(
        &ErrorType::ALL.iter().map(|(n, v)| (*n, *v as u64)).collect::<Vec<_>>(),
        &cpp_enum(cpp, "ErrorType"),
    ) {
        Ok(n) => check("ErrorType matches shadPS4's `enum class ErrorType`, both directions", n == 35, format!("{n} values (all 34 room errors present, and TooLarge since the d1 fork's TSS)")),
        Err(e) => check("ErrorType matches shadPS4's `enum class ErrorType`, both directions", false, e),
    });

    c.push(match same_set(
        &Notification::ALL.iter().map(|(n, v)| (*n, *v as u64)).collect::<Vec<_>>(),
        &cpp_enum(cpp, "NotificationType"),
    ) {
        Ok(n) => check("Notification matches shadPS4's `enum class NotificationType`", true, format!("{n} values")),
        Err(e) => check("Notification matches shadPS4's `enum class NotificationType`", false, e),
    });

    c.push(match same_set(
        &PacketType::ALL.iter().map(|(n, v)| (*n, *v as u64)).collect::<Vec<_>>(),
        &cpp_enum(cpp, "PacketType"),
    ) {
        Ok(n) => check("PacketType matches shadPS4's `enum class PacketType`", true, format!("{n} values")),
        Err(e) => check("PacketType matches shadPS4's `enum class PacketType`", false, e),
    });

    // The room events are a SUBSET of OrbisNpMatching2Event, so `same_set` is wrong here: check
    // each of our eight against the client's table by name and value.
    {
        let orbis = cpp_enum(cpp, "OrbisNpMatching2Event");
        let mut bad = Vec::new();
        for (name, value) in RoomEvent::ALL {
            let key = format!("ORBIS_NP_MATCHING2_ROOM_EVENT_{name}");
            match orbis.iter().find(|(n, _)| *n == key) {
                Some((_, v)) if *v == *value as u64 => {}
                Some((_, v)) => bad.push(format!("{name}: ours {value:#x}, client's {v:#x}")),
                None => bad.push(format!("{key} missing from OrbisNpMatching2Event")),
            }
        }
        // and every ROOM_EVENT the client defines (excluding the `_A` variants, which are the same
        // events with 0x8000 set and are produced by the emulator, never by us) must be in ours.
        let client_room_events = orbis
            .iter()
            .filter(|(n, _)| n.starts_with("ORBIS_NP_MATCHING2_ROOM_EVENT_") && !n.ends_with("_A"))
            .count();
        if client_room_events != RoomEvent::ALL.len() {
            bad.push(format!(
                "client defines {client_room_events} non-_A room events, we define {}",
                RoomEvent::ALL.len()
            ));
        }
        c.push(check(
            "RoomEvent 0x1101-0x1108 match the client's OrbisNpMatching2Event",
            bad.is_empty() && !orbis.is_empty(),
            if bad.is_empty() { format!("{client_room_events} room events") } else { bad.join("; ") },
        ));
    }

    {
        let causes = cpp_enum(cpp, "OrbisNpMatching2EventCause");
        let rust: Vec<(&str, u64)> = EventCause::ALL.iter().map(|(n, v)| (*n, *v as u64)).collect();
        let mut bad = Vec::new();
        for (name, value) in &rust {
            let key = format!("ORBIS_NP_MATCHING2_EVENT_CAUSE_{name}");
            match causes.iter().find(|(n, _)| *n == key) {
                Some((_, v)) if v == value => {}
                Some((_, v)) => bad.push(format!("{name}: ours {value}, client's {v}")),
                None => bad.push(format!("{key} missing")),
            }
        }
        c.push(check(
            "EventCause 1-11 match the client's OrbisNpMatching2EventCause",
            bad.is_empty() && causes.len() == rust.len(),
            if bad.is_empty() { format!("{} causes", causes.len()) } else { bad.join("; ") },
        ));
    }

    // ⭐ The excerpt is only worth anything if it is genuinely verbatim. Re-derive it when the
    // shadPS4 tree is available; report SKIPPED, never PASS, when it is not.
    {
        let root = crate::shadps4_src_dir();
        let client_h = root.join("src/shadnet/client.h");
        let types2_h = root.join("src/core/libraries/np/np_types2.h");
        if client_h.exists() && types2_h.exists() {
            c.push(verbatim_control(&client_h, &types2_h, cpp));
        } else {
            c.push(skipped(
                "the vendored excerpt is verbatim in shadPS4 and its sha256s still match",
                format!("shadPS4 tree not at {} - set SHADPS4_SRC", root.display()),
            ));
        }
    }

    // ── B. agreement with the vendored schema ────────────────────────────────────────────────

    {
        let derived = sha256_hex(proto.as_bytes());
        let recorded = sha256_after(crate::controls::ATTRIBUTION, "`proto/shadnet.proto`");
        c.push(match recorded {
            Some(r) => check(
                "the compiled schema's sha256 is the one ATTRIBUTION.md records (re-derived, not transcribed)",
                r == derived,
                format!("derived {}…, recorded {}…", &derived[..16], &r[..16]),
            ),
            None => check(
                "the compiled schema's sha256 is the one ATTRIBUTION.md records (re-derived, not transcribed)",
                false,
                "no sha256 found in ATTRIBUTION.md after `proto/shadnet.proto`".to_string(),
            ),
        });
    }

    // ⭐ The schema fact that makes private-match password *checking* impossible. If a future
    // schema bump adds a join-side password, this control is what tells you.
    {
        let create = proto_message(proto, "CreateRoomRequest").map(proto_fields).unwrap_or_default();
        let join = proto_message(proto, "JoinRoomRequest").map(proto_fields).unwrap_or_default();
        let create_pw = field(&create, "room_password");
        let join_pw = field(&join, "room_password");
        let join_flag = field(&join, "room_password_present");
        c.push(check(
            "JoinRoomRequest carries NO password bytes - so RoomPasswordMismatch is unenforceable",
            !create.is_empty()
                && !join.is_empty()
                && matches!(create_pw, Some((t, _, 27, false)) if t == "bytes")
                && join_pw.is_none()
                && matches!(join_flag, Some((t, _, 7, false)) if t == "bool"),
            format!(
                "CreateRoomRequest.room_password={:?}, JoinRoomRequest.room_password={:?}, .room_password_present={:?}",
                create_pw.map(|f| f.2),
                join_pw.map(|f| f.2),
                join_flag.map(|f| f.2)
            ),
        ));
    }

    {
        let m = proto_message(proto, "MatchingRoomMemberData").map(proto_fields).unwrap_or_default();
        let addr = field(&m, "addr");
        let port = field(&m, "port");
        let member_id = field(&m, "member_id");
        let team_id = field(&m, "team_id");
        c.push(check(
            "MatchingRoomMemberData.addr is a string and .port a uint32 - the two fields ResolvePeerEndpoint needs",
            matches!(addr, Some((t, _, 10, false)) if t == "string")
                && matches!(port, Some((t, _, 11, false)) if t == "uint32")
                && matches!(member_id, Some((t, _, 2, false)) if t == "uint32")
                && matches!(team_id, Some((t, _, 3, false)) if t == "uint32"),
            format!("{} fields", m.len()),
        ));
    }

    {
        let n = proto_message(proto, "NotifyRoomEvent").map(proto_fields).unwrap_or_default();
        let member = field(&n, "member");
        let ctx = field(&n, "ctx_id");
        c.push(check(
            "NotifyRoomEvent.member is a MESSAGE field - `pb.has_member()` is what the client tests",
            matches!(member, Some((t, _, 6, false)) if t == "MatchingRoomMemberData")
                && matches!(ctx, Some((t, _, 1, false)) if t == "uint32"),
            format!("member={:?} ctx_id={:?}", member.map(|f| f.2), ctx.map(|f| f.2)),
        ));
    }

    {
        let cr = proto_message(proto, "CreateRoomReply").map(proto_fields).unwrap_or_default();
        let jr = proto_message(proto, "JoinRoomReply").map(proto_fields).unwrap_or_default();
        c.push(check(
            "CreateRoomReply.details and JoinRoomReply.details are message fields (so prost yields Option)",
            matches!(field(&cr, "details"), Some((t, _, 9, false)) if t == "CreateJoinRoomResponse")
                && matches!(field(&jr, "details"), Some((t, _, 6, false)) if t == "CreateJoinRoomResponse"),
            "the has_details() contract is expressible".to_string(),
        ));
    }

    {
        let int = proto_message(proto, "MatchingRoomDataInternal").map(proto_fields).unwrap_or_default();
        let ext = proto_message(proto, "MatchingRoomDataExternal").map(proto_fields).unwrap_or_default();
        let a = field(&int, "joined_slot_mask");
        let b = field(&ext, "joined_slot_mask");
        c.push(check(
            "joined_slot_mask is uint32 internal / uint64 external - one Rust type cannot serve both",
            matches!(a, Some((t, _, _, _)) if t == "uint32")
                && matches!(b, Some((t, _, _, _)) if t == "uint64"),
            format!("internal={:?} external={:?}", a.map(|f| f.0.clone()), b.map(|f| f.0.clone())),
        ));
    }

    {
        let r = proto_message(proto, "RequestSignalingInfosReply").map(proto_fields).unwrap_or_default();
        c.push(check(
            "RequestSignalingInfosReply carries target_ip/target_port - the only two fields the client reads",
            matches!(field(&r, "target_ip"), Some((t, _, 2, false)) if t == "string")
                && matches!(field(&r, "target_port"), Some((t, _, 3, false)) if t == "uint32"),
            format!("{} fields", r.len()),
        ));
    }

    // ── C. the emitter guards: every negative paired with a positive ─────────────────────────

    {
        let mut t = RoomTable::new(1);
        let a = t
            .create_room(RoomSpec { max_slots: 4, ..Default::default() }, member_intent(1, "A"))
            .unwrap();
        t.join_room(a.room_id, false, member_intent(2, "B")).unwrap();
        let room = t.get(a.room_id).unwrap();
        let details = matching::create_join_details(room, 1).unwrap();

        c.push(check(
            "the emitter REFUSES `details: None` and ACCEPTS `details: Some(..)`",
            matching::guard_create_join_reply(None).is_err()
                && matching::guard_create_join_reply(Some(&pb::CreateJoinRoomResponse::default())).is_err()
                && matching::guard_create_join_reply(Some(&details)).is_ok(),
            "negative, empty-details negative, and positive control".to_string(),
        ));

        let reply = matching::create_room_reply(room, 1).unwrap();
        let round = pb::CreateRoomReply::decode(reply.encode_to_vec().as_slice()).unwrap();
        c.push(check(
            "a CreateRoomReply survives encode→decode with `details` still present",
            round.details.as_ref().map(|d| d.members.len()) == Some(2)
                && round.details.as_ref().unwrap().me_member_id == 1,
            "has_details() would be true on the client".to_string(),
        ));

        let mut broken = room.clone();
        broken.members[1].endpoint.port = 0;
        c.push(check(
            "a member with no usable addr/port is REFUSED, and the same member with one is accepted",
            matching::create_join_details(&broken, 1).is_err()
                && matching::create_join_details(room, 1).is_ok(),
            "contract 3.1#6 - otherwise the client blocks its reader thread 5 s".to_string(),
        ));
    }

    {
        let n = crate::rooms::RoomNotification {
            targets: vec![1],
            room_id: 7,
            event: RoomEvent::MemberJoined,
            cause: EventCause::ServerOperation,
            error_code: 0,
            member: None,
            bin_attrs: vec![],
            flags: 0,
            passwd_slot_mask: None,
        };
        c.push(check(
            "a notification without the RECIPIENT's ctx_id is refused; with one it builds",
            matching::room_event_proto(&n, None).is_err()
                && matching::room_event_proto(&n, Some(0)).is_err()
                && matching::room_event_proto(&n, Some(1)).is_ok(),
            "ContextManager::Get(0) misses and the frame vanishes with no log line".to_string(),
        ));

        let dropped: Vec<&str> = [
            RoomEvent::RoomDestroyed,
            RoomEvent::RoomOwnerChanged,
            RoomEvent::UpdatedRoomMemberDataInternal,
            RoomEvent::UpdatedSignalingOptParam,
        ]
        .iter()
        .filter(|e| {
            let mut m = n.clone();
            m.event = **e;
            matching::room_event_proto(&m, Some(1)).is_ok()
        })
        .map(|_| "leaked")
        .collect();
        let handled = [
            RoomEvent::MemberJoined,
            RoomEvent::MemberLeft,
            RoomEvent::KickedOut,
            RoomEvent::UpdatedRoomDataInternal,
        ]
        .iter()
        .all(|e| {
            let mut m = n.clone();
            m.event = *e;
            matching::room_event_proto(&m, Some(1)).is_ok()
        });
        c.push(check(
            "the four room events shadPS4's HandleRoomEvent DROPS are refused; the four it handles are not",
            dropped.is_empty() && handled,
            "its default: arm returns without firing a callback".to_string(),
        ));

        c.push(check(
            "an all-default NotifyRoomEvent encodes to ZERO bytes and is refused",
            pb::NotifyRoomEvent::default().encoded_len() == 0
                && matching::guard_room_event(&pb::NotifyRoomEvent::default()).is_err()
                && matching::guard_room_event(&matching::room_event_proto(&n, Some(1)).unwrap())
                    .is_ok(),
            "HandleNotification bails on an empty blob before it ever parses".to_string(),
        ));
    }

    // ── D. frames, byte for byte ─────────────────────────────────────────────────────────────

    {
        // A handcrafted request, built the way the client builds one, decoded by our reader and
        // re-encoded. This is the only control here that tests the codec against bytes rather than
        // against another function of ours.
        let body = pb::JoinRoomRequest {
            room_id: 0x1122_3344_5566_7788,
            req_id: 9,
            team_id: 3,
            room_password_present: true,
            ..Default::default()
        }
        .encode_to_vec();
        let mut frame = Vec::new();
        frame.push(0u8); // PacketType::Request
        frame.extend_from_slice(&(Command::JoinRoom as u16).to_le_bytes());
        frame.extend_from_slice(&((HEADER_SIZE + 4 + body.len()) as u32).to_le_bytes());
        frame.extend_from_slice(&0xDEAD_BEEF_0000_0001u64.to_le_bytes());
        frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
        frame.extend_from_slice(&body);

        let decoded = futures_lite_block_on(&frame);
        let ok = match decoded {
            Some(p) => {
                let blob = p.request_blob().to_vec();
                let req = pb::JoinRoomRequest::decode(blob.as_slice()).ok();
                p.packet_type == PacketType::Request
                    && p.command == Command::JoinRoom as u16
                    && p.packet_id == 0xDEAD_BEEF_0000_0001
                    && blob == body
                    && req.map(|r| (r.room_id, r.team_id, r.room_password_present))
                        == Some((0x1122_3344_5566_7788, 3, true))
            }
            None => false,
        };
        c.push(check(
            "a handcrafted JoinRoom frame decodes to the exact bytes it was built from",
            ok,
            "header 15 LE bytes, then u32 blob length, then protobuf".to_string(),
        ));
    }

    {
        let reply = encode_reply(Command::JoinRoom as u16, 0x0102_0304, ErrorType::RoomFull, &[7, 8]);
        let ok = reply[0] == PacketType::Reply as u8
            && u16::from_le_bytes([reply[1], reply[2]]) == Command::JoinRoom as u16
            && u32::from_le_bytes(reply[3..7].try_into().unwrap()) as usize == reply.len()
            && u64::from_le_bytes(reply[7..15].try_into().unwrap()) == 0x0102_0304
            && reply[HEADER_SIZE] == 16 // RoomFull, the raw byte the title receives as errorCode
            && extract_blob(&reply[HEADER_SIZE..], 1) == Some(&[7u8, 8][..]);
        c.push(check(
            "a reply is [type][cmd][len][pkt_id][ErrorType][u32 len][proto], echoing cmd and packet_id",
            ok,
            "DispatchPacket switches on the REPLY's command id, so a wrong echo lands in OnScoreReply".to_string(),
        ));
    }

    {
        let notif = encode_notification(Notification::RoomEvent, &[1, 2, 3]);
        let ok = notif[0] == PacketType::Notification as u8
            && u16::from_le_bytes([notif[1], notif[2]]) == 10
            && u64::from_le_bytes(notif[7..15].try_into().unwrap()) == 0
            // ⚠ no ErrorType byte on a notification - the blob length starts immediately
            && extract_blob(&notif[HEADER_SIZE..], 0) == Some(&[1u8, 2, 3][..]);
        c.push(check(
            "a notification carries NO error byte and packet_id 0, unlike a reply",
            ok,
            "HandleNotification reads ExtractBlob(payload, 0)".to_string(),
        ));
    }

    // ── E. the state machine's invariants ────────────────────────────────────────────────────

    {
        let mut t = RoomTable::new(1);
        let mut zero_seen = false;
        let mut ids = Vec::new();
        for i in 1..=6u64 {
            let o = t
                .create_room(RoomSpec { max_slots: 4, ..Default::default() }, member_intent(i, "P"))
                .unwrap();
            zero_seen |= o.room_id == 0 || o.me_member_id == 0;
            ids.push(o.room_id);
            t.leave_room(o.room_id, i).unwrap();
        }
        c.push(check(
            "room_id and member_id are never 0 - the client's `no room` / `not a member` sentinels",
            !zero_seen && ids.iter().all(|v| *v != 0),
            format!("{} rooms created and destroyed", ids.len()),
        ));
    }

    {
        let mut t = RoomTable::new(1);
        let a = t
            .create_room(RoomSpec { max_slots: 4, ..Default::default() }, member_intent(1, "Same"))
            .unwrap();
        let b = t.join_room(a.room_id, false, member_intent(2, "Same")).unwrap();
        let room = t.get(a.room_id).unwrap();
        c.push(check(
            "two connections reporting the SAME npid are two distinct members",
            room.members.len() == 2 && a.me_member_id != b.me_member_id,
            "rooms key on conn_id, never on the npid string".to_string(),
        ));
    }

    {
        let state = ServerState::new(test_config());
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (tx2, rx2) = tokio::sync::mpsc::unbounded_channel();
        std::mem::forget(rx);
        std::mem::forget(rx2);
        let a = state.alloc_conn_id();
        let b = state.alloc_conn_id();
        state.register(SessionHandle {
            conn_id: a,
            npid: String::new(),
            peer: "10.77.0.2:1".parse().unwrap(),
            ctx_id: None,
            outbox: tx,
        });
        state.register(SessionHandle {
            conn_id: b,
            npid: String::new(),
            peer: "10.77.0.3:1".parse().unwrap(),
            ctx_id: None,
            outbox: tx2,
        });
        let first = state.login(a, "Same");
        let collide = state.login(b, "Same");
        let distinct = state.login(b, "Other");
        c.push(check(
            "a second Login for a LIVE npid is refused, and a distinct npid on the same socket is not",
            first.is_ok()
                && collide == Err(LoginRefusal::AlreadyLoggedIn)
                && distinct.is_ok()
                && first.as_ref().unwrap().1.user_id != distinct.as_ref().unwrap().1.user_id,
            "identity conflation is the failure that reads as a networking bug".to_string(),
        ));
        c.push(check(
            "the FIRST participant still receives the configured identity verbatim",
            first.as_ref().unwrap().1
                == Identity {
                    npid: "D1Guardian".into(),
                    user_id: 1,
                    token: "d1-local-token".into(),
                    avatar_url: "http://127.0.0.1/avatar.png".into(),
                },
            "so a single-instance run is unchanged by any of this".to_string(),
        ));
    }

    // ⭐ The refusal that keeps us honest about what we cannot check.
    {
        let mut t = RoomTable::new(1);
        let a = t
            .create_room(
                RoomSpec {
                    max_slots: 4,
                    password: Some(b"hunter2".to_vec()),
                    ..Default::default()
                },
                member_intent(1, "A"),
            )
            .unwrap();
        let no_flag = t.join_room(a.room_id, false, member_intent(2, "B"));
        let with_flag = t.join_room(a.room_id, true, member_intent(3, "C"));
        let never_mismatch = [no_flag, with_flag.clone()]
            .iter()
            .all(|r| !matches!(r, Err(e) if e.as_error_type() == ErrorType::RoomPasswordMismatch));
        c.push(check(
            "a private room refuses a joiner with no password flag and NEVER claims a mismatch",
            never_mismatch
                && with_flag.is_ok()
                && t.join_room(a.room_id, false, member_intent(4, "D"))
                    == Err(RoomError::PasswordMissing),
            "the schema carries no join-side bytes; a mismatch check would be invented".to_string(),
        ));
    }

    {
        let state = ServerState::new(test_config());
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        std::mem::forget(rx);
        let a = state.alloc_conn_id();
        state.register(SessionHandle {
            conn_id: a,
            npid: "PlayerA".into(),
            peer: "10.77.0.2:51000".parse().unwrap(),
            ctx_id: None,
            outbox: tx,
        });
        state.login(a, "PlayerA").unwrap();
        let refused = state.resolve_signaling("PlayerA");
        state.observe_signaling("PlayerA", "10.77.0.2:3074".parse().unwrap());
        let answered = state.resolve_signaling("PlayerA");
        c.push(check(
            "RequestSignalingInfos REFUSES an unknown peer rather than answering the TCP source port",
            refused == Err(ErrorType::NotFound)
                && matches!(&answered, Ok((ep, _)) if ep.addr == "10.77.0.2" && ep.port == 3074),
            "a wrong port connects to nothing and fails minutes later, silently".to_string(),
        ));
    }

    {
        let v4: std::net::SocketAddr = "10.77.0.3:51000".parse().unwrap();
        let v6: std::net::SocketAddr = "[2001:db8::1]:51000".parse().unwrap();
        let mapped: std::net::SocketAddr = "[::ffff:10.77.0.3]:51000".parse().unwrap();
        c.push(check(
            "the published endpoint is the peer's own address with the OPERATOR's port, and IPv6 is refused",
            endpoint_for(v4, 3074) == Some(Endpoint { addr: "10.77.0.3".into(), port: 3074 })
                && endpoint_for(mapped, 3074).map(|e| e.addr) == Some("10.77.0.3".into())
                && endpoint_for(v6, 3074).is_none(),
            "IpStringToAddr would read an IPv6 literal as address 0".to_string(),
        ));
    }

    // ── F. the signaling ping, against shadPS4's static_asserts ──────────────────────────────

    {
        let mut ping = vec![0u8; STUN_PING_LEN];
        ping[0] = 1;
        ping[1..8].copy_from_slice(b"PlayerA");
        ping[17..21].copy_from_slice(&[10, 77, 0, 2]);
        let parsed = parse_stun_ping(&ping);
        let mut wrong = ping.clone();
        wrong[0] = 2;
        c.push(check(
            "Matching2StunPing is 1 + 16 + 4 = 21 bytes, and anything else is ignored rather than guessed at",
            STUN_PING_LEN == 21
                && ONLINE_ID_LEN == 16
                && parsed.as_ref().map(|p| p.npid.as_str()) == Some("PlayerA")
                && parse_stun_ping(&wrong).is_none()
                && parse_stun_ping(&ping[..20]).is_none(),
            "shadPS4 static_asserts sizeof == 21".to_string(),
        ));
    }

    // ── G. what we deliberately did NOT do ───────────────────────────────────────────────────

    c.push(check(
        "matching2 is still advertised as FALSE by default - nothing has asked for it",
        !ServerConfig { matching2_enabled: false, ..test_config() }.matching2_enabled,
        "PLAN-B §7: no game-side code reaches sceNpMatching2; §3.5 is the provenance trap".to_string(),
    ));

    c.push(check(
        "no session-key issuance exists in this crate",
        !crate_mentions_key_minting(),
        "C7 verdict: bdSecurityKeyMap::initKey mints 8+16 bytes from the client's own MT19937"
            .to_string(),
    ));

    c
}

/// Embedded so the sha256 control reads the attribution we actually publish, not a copy.
const ATTRIBUTION: &str = include_str!("../../../ATTRIBUTION.md");

/// Grep our own source for the shapes a fabricated key scheme would need. Crude on purpose: the
/// point is that if somebody later adds one, this control fails and they have to argue with §1.
fn crate_mentions_key_minting() -> bool {
    const SOURCES: &[&str] = &[
        include_str!("rooms.rs"),
        include_str!("registry.rs"),
        include_str!("matching.rs"),
        include_str!("session.rs"),
        include_str!("signaling.rs"),
    ];
    const BANNED: &[&str] = &["security_key", "securitykey", "session_key", "SecurityId", "bdSecurityKey ="];
    SOURCES.iter().any(|s| {
        let lower = s.to_lowercase();
        BANNED.iter().any(|b| lower.contains(&b.to_lowercase()))
    })
}

fn verbatim_control(client_h: &Path, types2_h: &Path, excerpt: &str) -> Control {
    let name = "the vendored excerpt is verbatim in shadPS4 and its sha256s still match";
    let (Ok(ch), Ok(t2)) = (std::fs::read(client_h), std::fs::read(types2_h)) else {
        return check(name, false, "could not read the shadPS4 sources".to_string());
    };
    let ch_sha = sha256_hex(&ch);
    let t2_sha = sha256_hex(&t2);
    let recorded_ch = sha256_after(excerpt, "src/shadnet/client.h");
    let recorded_t2 = sha256_after(excerpt, "np_types2.h");
    let ch_txt = String::from_utf8_lossy(&ch);
    let t2_txt = String::from_utf8_lossy(&t2);

    let mut problems = Vec::new();
    if recorded_ch.as_deref() != Some(ch_sha.as_str()) {
        problems.push(format!("client.h sha256 moved: now {}…", &ch_sha[..16]));
    }
    if recorded_t2.as_deref() != Some(t2_sha.as_str()) {
        problems.push(format!("np_types2.h sha256 moved: now {}…", &t2_sha[..16]));
    }
    let blocks = cpp_enum_blocks(excerpt);
    if blocks.len() != 6 {
        problems.push(format!("expected 6 enum blocks in the excerpt, parsed {}", blocks.len()));
    }
    for (block_name, body) in &blocks {
        if !ch_txt.contains(body.as_str()) && !t2_txt.contains(body.as_str()) {
            problems.push(format!("{block_name} is NOT verbatim upstream"));
        }
    }
    check(
        name,
        problems.is_empty(),
        if problems.is_empty() {
            format!("{} enum blocks verbatim; both sha256s match", blocks.len())
        } else {
            problems.join("; ")
        },
    )
}

/// Drive `read_packet` (async, but over an in-memory slice it never yields) without dragging a
/// runtime into the controls.
fn futures_lite_block_on(frame: &[u8]) -> Option<shadnet_wire::Packet> {
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    let mut cursor = std::io::Cursor::new(frame.to_vec());
    let mut fut = Box::pin(shadnet_wire::read_packet(&mut cursor));

    fn noop(_: *const ()) {}
    fn clone(p: *const ()) -> RawWaker {
        RawWaker::new(p, &VTABLE)
    }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
    let mut cx = Context::from_waker(&waker);
    for _ in 0..64 {
        match Pin::new(&mut fut).poll(&mut cx) {
            Poll::Ready(Ok(p)) => return Some(p),
            Poll::Ready(Err(_)) => return None,
            Poll::Pending => continue,
        }
    }
    None
}

/// Run every control. `2` means at least one failed.
pub fn main() -> i32 {
    let controls = run();
    let failed = controls.iter().filter(|c| c.state == State::Fail).count();
    let skipped = controls.iter().filter(|c| c.state == State::Skipped).count();
    for c in &controls {
        let tag = match c.state {
            State::Pass => "PASS",
            State::Fail => "FAIL",
            State::Skipped => "SKIP",
        };
        if c.detail.is_empty() {
            println!("{tag}  {}", c.name);
        } else {
            println!("{tag}  {}  [{}]", c.name, c.detail);
        }
    }
    println!();
    if failed == 0 {
        println!(
            "ALL CONTROLS PASS  ({} checks, {skipped} skipped)",
            controls.len() - skipped
        );
        if skipped > 0 {
            println!(
                "⚠ {skipped} skipped - a skip is NOT a pass. Set SHADPS4_SRC to re-derive them."
            );
        }
        0
    } else {
        println!(
            "{failed} of {} CONTROLS FAILED - output may not be cited",
            controls.len()
        );
        2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The controls are also a test, so `cargo test` fails on the same condition the binary exits
    /// 2 on. Two instruments, one answer.
    #[test]
    fn every_control_passes() {
        let failed: Vec<&str> = super::run()
            .into_iter()
            .filter(|c| c.state == State::Fail)
            .map(|c| c.name)
            .collect();
        assert!(failed.is_empty(), "failed controls: {failed:?}");
    }

    #[test]
    fn there_are_enough_controls_to_be_worth_running() {
        assert!(super::run().len() >= 20, "a controls suite this thin is decoration");
    }

    /// ⭐ The meta-control: the C++ parser must be able to FAIL. A comparison that always says
    /// "same" is the decoration this file is supposed to avoid.
    #[test]
    fn the_cpp_comparison_can_detect_a_drifted_discriminant() {
        let cpp = cpp_enum(crate::SHADPS4_CLIENT_CONSTANTS, "CommandType");
        assert!(cpp.len() >= 38, "parsed {} entries", cpp.len());
        let mut wrong: Vec<(&str, u64)> =
            Command::ALL.iter().map(|(n, v)| (*n, *v as u64)).collect();
        wrong[0].1 = 99; // pretend Login moved
        assert!(same_set(&wrong, &cpp).is_err());
        // and a missing value is caught from the other side too
        let short: Vec<(&str, u64)> = wrong[1..].iter().map(|(n, v)| (*n, *v)).collect();
        assert!(same_set(&short, &cpp).is_err());
    }

    #[test]
    fn the_proto_parser_can_detect_a_missing_field() {
        let fields = proto_message(crate::VENDORED_PROTO, "JoinRoomRequest")
            .map(proto_fields)
            .unwrap();
        assert!(!fields.is_empty());
        assert!(field(&fields, "room_id").is_some());
        assert!(field(&fields, "no_such_field").is_none());
        assert!(proto_message(crate::VENDORED_PROTO, "NoSuchMessage").is_none());
    }

    #[test]
    fn sha256_of_the_vendored_proto_is_stable() {
        // Not a transcription: it re-derives and compares to ATTRIBUTION.md, the same as the
        // control does, and additionally proves the helper finds a hash at all.
        let derived = sha256_hex(crate::VENDORED_PROTO.as_bytes());
        assert_eq!(derived.len(), 64);
        assert_eq!(sha256_after(ATTRIBUTION, "`proto/shadnet.proto`"), Some(derived));
    }
}
