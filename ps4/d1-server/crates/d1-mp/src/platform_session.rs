// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! ⭐ **The platform sessions**: the PSN Session WebAPI the client uses to set up its
//! peer-to-peer game sessions, as a store several clients share.
//!
//! D1 never calls `sceNpMatching2`; its managed session is the PSN Session WebAPI over HTTP, which
//! shadPS4 sends to its configured WebAPI server, the responder (`ps4/design/peer-and-platform.md`
//! §3.2, §4). The host posts a session whose `session-data` part is a 78-byte record holding its
//! secure-channel key material; a joiner fetches that record (`GET .../sessionData`, into a
//! `0x4E`-byte buffer), adds itself as a member and opens the channel to the host. An invite
//! carries the same record as `invitation-data`. So the service is **store and forward of opaque
//! bytes**, keyed by session id; the client mints its own keys.
//!
//! The responder answered `POST /v1/sessions` with one constant id, which is right for one client
//! and wrong for two: the second host would overwrite the first's record. [`PlatformSessions`]
//! gives every session its own id and record, members, invitations and presence.
//!
//! ## What is the client's, and what is ours
//!
//! * Endpoints and the multipart parts: the client's literals (§4.2). The parts are found by their
//!   `Content-Description` (`session-request`, `session-image`, `session-data`,
//!   `invitation-request`, `invitation-data`), else their `name`.
//! * The session id's shape, `NNN-` and a UUID, 40 characters: the client's own zeroed `PUT`
//!   showed it, and the responder's first id (`000-d1000000-0000-4000-8000-000000000001`) has
//!   carried the client to orbit since. Ours continue that sequence.
//! * The JSON reply bodies beyond `sessionId` and `invitationId` follow PSN's published shapes and
//!   are `[I]`: the client's parsers for them were not disassembled.

use std::collections::BTreeMap;

use serde_json::{json, Value};

/// The record a session carries (`0x4E` bytes, `ps4/design/peer-and-platform.md` §3.2).
pub const SESSION_DATA_LEN: usize = 0x4E;

/// One part of a `multipart/mixed` body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Part {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    /// The part's role: its `Content-Description`, else the `name` of its `Content-Disposition`.
    pub fn role(&self) -> Option<String> {
        if let Some(d) = self.header("content-description") {
            return Some(d.trim().to_ascii_lowercase());
        }
        let disp = self.header("content-disposition")?;
        let name = disp.split(';').map(str::trim).find_map(|p| p.strip_prefix("name="))?;
        Some(name.trim_matches('"').to_ascii_lowercase())
    }
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from > hay.len() {
        return None;
    }
    hay[from..].windows(needle.len()).position(|w| w == needle).map(|p| p + from)
}

/// The boundary of a `multipart/...; boundary=X` content type.
pub fn boundary_of(content_type: &str) -> Option<String> {
    content_type
        .split(';')
        .map(str::trim)
        .find_map(|p| p.strip_prefix("boundary=").or_else(|| p.strip_prefix("BOUNDARY=")))
        .map(|b| b.trim_matches('"').to_owned())
        .filter(|b| !b.is_empty())
}

/// Split a multipart body. With no usable content type, the boundary is read from the body's
/// first line (`--X`).
pub fn parse_multipart(content_type: Option<&str>, body: &[u8]) -> Vec<Part> {
    let boundary = match content_type.and_then(boundary_of) {
        Some(b) => b,
        None => {
            let start = find(body, b"--", 0).unwrap_or(0) + 2;
            let end = find(body, b"\r\n", start).unwrap_or(body.len());
            match std::str::from_utf8(&body[start.min(end)..end]) {
                Ok(b) if !b.is_empty() => b.to_owned(),
                _ => return Vec::new(),
            }
        }
    };
    let delim = format!("--{boundary}").into_bytes();
    let mut parts = Vec::new();
    let Some(mut pos) = find(body, &delim, 0) else { return parts };
    loop {
        pos += delim.len();
        if body.get(pos..pos + 2) == Some(b"--") {
            break;
        }
        // Past the line break after the delimiter.
        if body.get(pos..pos + 2) == Some(b"\r\n") {
            pos += 2;
        } else if body.get(pos) == Some(&b'\n') {
            pos += 1;
        }
        let Some(next) = find(body, &delim, pos) else { break };
        let chunk = &body[pos..next];
        let (head, mut content) = match find(chunk, b"\r\n\r\n", 0) {
            Some(h) => (&chunk[..h], &chunk[h + 4..]),
            None => match find(chunk, b"\n\n", 0) {
                Some(h) => (&chunk[..h], &chunk[h + 2..]),
                None => (&chunk[..0], chunk),
            },
        };
        if content.ends_with(b"\r\n") {
            content = &content[..content.len() - 2];
        } else if content.ends_with(b"\n") {
            content = &content[..content.len() - 1];
        }
        let headers = String::from_utf8_lossy(head)
            .lines()
            .filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned())))
            .collect();
        parts.push(Part { headers, body: content.to_vec() });
        pos = next;
    }
    parts
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub id: String,
    /// Who created it: the requester key the responder passes (the bearer token, or an online id).
    pub owner: String,
    /// The `session-data` record, verbatim.
    pub data: Vec<u8>,
    pub changeable_data: Vec<u8>,
    /// The `session-request` JSON, as sent (`sessionPrivacy`, `sessionType`, `sessionMaxUser`...).
    pub request: Value,
    /// Online ids that joined (`POST .../members`), in order.
    pub members: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invitation {
    pub id: String,
    pub session: String,
    pub from: String,
    pub to: String,
    /// The `invitation-data` part, verbatim: the host's session record.
    pub data: Vec<u8>,
    pub used: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    NoSuchSession(String),
    NoSuchInvitation(String),
    /// A `POST /v1/sessions` without a `session-data` part.
    NoSessionData,
    /// An invitation without anyone to send it to.
    NoInvitees,
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSuchSession(s) => write!(f, "no session {s}"),
            Self::NoSuchInvitation(s) => write!(f, "no invitation {s}"),
            Self::NoSessionData => write!(f, "the session request carried no session-data part"),
            Self::NoInvitees => write!(f, "the invitation request names nobody (\"to\")"),
        }
    }
}

impl std::error::Error for SessionError {}

/// Every platform session, invitation and presence record.
#[derive(Debug, Clone, Default)]
pub struct PlatformSessions {
    sessions: BTreeMap<String, Session>,
    invitations: BTreeMap<String, Invitation>,
    /// `PUT /v1/users/{npid}/presence/gameData`, per online id.
    game_data: BTreeMap<String, Vec<u8>>,
    next_session: u64,
    next_invitation: u64,
}

/// The `n`th session id (1-based): the responder's original constant, then onwards.
pub fn session_id(n: u64) -> String {
    format!("000-d1000000-0000-4000-8000-{n:012x}")
}

impl PlatformSessions {
    pub fn new() -> Self {
        Self { next_session: 1, next_invitation: 1, ..Self::default() }
    }

    pub fn session(&self, id: &str) -> Option<&Session> {
        self.sessions.get(id)
    }

    pub fn sessions(&self) -> impl Iterator<Item = &Session> {
        self.sessions.values()
    }

    /// `POST /v1/sessions`. Returns the new id and the reply body.
    pub fn create(&mut self, owner: &str, content_type: Option<&str>, body: &[u8]) -> Result<(String, Value), SessionError> {
        let parts = parse_multipart(content_type, body);
        let data = parts.iter().find(|p| p.role().as_deref() == Some("session-data")).ok_or(SessionError::NoSessionData)?;
        let request = parts
            .iter()
            .find(|p| p.role().as_deref() == Some("session-request"))
            .and_then(|p| serde_json::from_slice(&p.body).ok())
            .unwrap_or(Value::Null);
        let id = session_id(self.next_session);
        self.next_session += 1;
        self.sessions.insert(id.clone(), Session {
            id: id.clone(),
            owner: owner.to_owned(),
            data: data.body.clone(),
            changeable_data: Vec::new(),
            request,
            members: Vec::new(),
        });
        Ok((id.clone(), json!({ "sessionId": id })))
    }

    /// `PUT /v1/sessions/{id}`: a session update (its request JSON). An unknown id is accepted
    /// with no effect, as before, so a lone client never fails here.
    pub fn update(&mut self, id: &str, body: &[u8]) -> Value {
        if let (Some(s), Ok(v)) = (self.sessions.get_mut(id), serde_json::from_slice::<Value>(body)) {
            if let (Some(base), Some(patch)) = (s.request.as_object_mut(), v.as_object()) {
                for (k, val) in patch {
                    base.insert(k.clone(), val.clone());
                }
            } else {
                s.request = v;
            }
        }
        json!({ "sessionId": id })
    }

    /// `GET /v1/sessions/{id}/sessionData`: the host's record, byte for byte.
    pub fn session_data(&self, id: &str) -> Result<&[u8], SessionError> {
        self.sessions.get(id).map(|s| s.data.as_slice()).ok_or_else(|| SessionError::NoSuchSession(id.into()))
    }

    pub fn put_changeable_data(&mut self, id: &str, body: &[u8]) -> Result<(), SessionError> {
        let s = self.sessions.get_mut(id).ok_or_else(|| SessionError::NoSuchSession(id.into()))?;
        s.changeable_data = body.to_vec();
        Ok(())
    }

    pub fn changeable_data(&self, id: &str) -> Result<&[u8], SessionError> {
        self.sessions.get(id).map(|s| s.changeable_data.as_slice()).ok_or_else(|| SessionError::NoSuchSession(id.into()))
    }

    /// `POST /v1/sessions/{id}/members`: `npid` joins.
    pub fn join(&mut self, id: &str, npid: &str) -> Result<(), SessionError> {
        let s = self.sessions.get_mut(id).ok_or_else(|| SessionError::NoSuchSession(id.into()))?;
        if !s.members.iter().any(|m| m == npid) {
            s.members.push(npid.to_owned());
        }
        Ok(())
    }

    /// `DELETE /v1/sessions/{id}/members/{npid}`. The last member out ends the session (PSN's
    /// rule for player sessions; `[I]` for D1's owner-bind sessions).
    pub fn leave(&mut self, id: &str, npid: &str) -> Result<bool, SessionError> {
        let s = self.sessions.get_mut(id).ok_or_else(|| SessionError::NoSuchSession(id.into()))?;
        s.members.retain(|m| m != npid);
        let ended = s.members.is_empty() && s.owner == npid;
        if ended {
            self.sessions.remove(id);
            self.invitations.retain(|_, i| i.session != id);
        }
        Ok(ended)
    }

    /// `GET /v1/sessions/{id}`.
    pub fn describe(&self, id: &str) -> Result<Value, SessionError> {
        let s = self.sessions.get(id).ok_or_else(|| SessionError::NoSuchSession(id.into()))?;
        let field = |k: &str| s.request.get(k).cloned().unwrap_or(Value::Null);
        Ok(json!({
            "sessionId": s.id,
            "sessionType": field("sessionType"),
            "sessionPrivacy": field("sessionPrivacy"),
            "sessionMaxUser": field("sessionMaxUser"),
            "members": s.members.iter().map(|m| json!({ "onlineId": m })).collect::<Vec<_>>(),
        }))
    }

    /// `GET /v1/users/{npid}/sessions`.
    pub fn sessions_of(&self, npid: &str) -> Value {
        let list: Vec<Value> = self
            .sessions
            .values()
            .filter(|s| s.owner == npid || s.members.iter().any(|m| m == npid))
            .map(|s| json!({ "sessionId": s.id }))
            .collect();
        json!({ "start": 0, "size": list.len(), "totalResults": list.len(), "sessions": list })
    }

    /// `POST /v1/sessions/{id}/invitations`: an `invitation-request` JSON (`{"to": [online ids],
    /// ...}`) and an `invitation-data` part. One invitation per invitee.
    pub fn invite(&mut self, id: &str, from: &str, content_type: Option<&str>, body: &[u8]) -> Result<Vec<String>, SessionError> {
        if !self.sessions.contains_key(id) {
            return Err(SessionError::NoSuchSession(id.into()));
        }
        let parts = parse_multipart(content_type, body);
        let to: Vec<String> = parts
            .iter()
            .find(|p| p.role().as_deref() == Some("invitation-request"))
            .and_then(|p| serde_json::from_slice::<Value>(&p.body).ok())
            .and_then(|v| v.get("to").and_then(Value::as_array).cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned).or_else(|| v.get("onlineId").and_then(Value::as_str).map(str::to_owned)))
            .collect();
        if to.is_empty() {
            return Err(SessionError::NoInvitees);
        }
        let data = parts.iter().find(|p| p.role().as_deref() == Some("invitation-data")).map(|p| p.body.clone()).unwrap_or_default();
        let mut ids = Vec::new();
        for t in to {
            let inv_id = format!("{:016X}", 0xD1A1_0000_0000_0000u64 | self.next_invitation);
            self.next_invitation += 1;
            self.invitations.insert(inv_id.clone(), Invitation { id: inv_id.clone(), session: id.to_owned(), from: from.to_owned(), to: t, data: data.clone(), used: false });
            ids.push(inv_id);
        }
        Ok(ids)
    }

    /// `GET /v1/users/{npid}/invitations`.
    pub fn invitations_for(&self, npid: &str) -> Value {
        let list: Vec<Value> = self
            .invitations
            .values()
            .filter(|i| i.to == npid)
            .map(|i| json!({ "invitationId": i.id, "sessionId": i.session, "fromUser": { "onlineId": i.from }, "usedFlag": i.used, "expired": false }))
            .collect();
        json!({ "start": 0, "size": list.len(), "totalResults": list.len(), "invitations": list })
    }

    /// `GET /v1/users/{npid}/invitations/{id}/invitationData`: the record the inviter attached.
    /// Fetching it marks the invitation used.
    pub fn invitation_data(&mut self, npid: &str, invitation: &str) -> Result<Vec<u8>, SessionError> {
        let inv = self.invitations.get_mut(invitation).filter(|i| i.to == npid).ok_or_else(|| SessionError::NoSuchInvitation(invitation.into()))?;
        inv.used = true;
        Ok(inv.data.clone())
    }

    pub fn put_game_data(&mut self, npid: &str, body: &[u8]) {
        self.game_data.insert(npid.to_owned(), body.to_vec());
    }

    pub fn game_data(&self, npid: &str) -> Option<&[u8]> {
        self.game_data.get(npid).map(Vec::as_slice)
    }

    pub fn clear_game_status(&mut self, npid: &str) {
        self.game_data.remove(npid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CT: &str = "multipart/mixed; boundary=BUNGIEr0x0rz";

    fn multipart(parts: &[(&str, &str, &[u8])]) -> Vec<u8> {
        let mut b = Vec::new();
        for (desc, ty, body) in parts {
            b.extend_from_slice(b"--BUNGIEr0x0rz\r\n");
            b.extend_from_slice(format!("Content-Type: {ty}\r\nContent-Description: {desc}\r\nContent-Length: {}\r\n\r\n", body.len()).as_bytes());
            b.extend_from_slice(body);
            b.extend_from_slice(b"\r\n");
        }
        b.extend_from_slice(b"--BUNGIEr0x0rz--\r\n");
        b
    }

    fn host_record(tag: u8) -> Vec<u8> {
        let mut r = vec![tag; SESSION_DATA_LEN];
        r[0] = 0x0D; // a CRLF-looking byte inside binary data must not end the part early
        r[1] = 0x0A;
        r
    }

    fn create(s: &mut PlatformSessions, owner: &str, tag: u8) -> String {
        let body = multipart(&[
            ("session-request", "application/json", br#"{"sessionType":"owner-bind","sessionPrivacy":"public","sessionMaxUser":1}"#),
            ("session-image", "image/jpeg", &[0xFF, 0xD8, 0xFF, 0xD9]),
            ("session-data", "application/octet-stream", &host_record(tag)),
        ]);
        s.create(owner, Some(CT), &body).unwrap().0
    }

    #[test]
    fn two_hosts_get_two_sessions_with_their_own_records() {
        let mut s = PlatformSessions::new();
        let a = create(&mut s, "HostA", 0xAA);
        let b = create(&mut s, "HostB", 0xBB);
        assert_eq!(a, "000-d1000000-0000-4000-8000-000000000001", "the first is the responder's original id");
        assert_eq!(a.len(), 40);
        assert_ne!(a, b);
        assert_eq!(s.session_data(&a).unwrap(), host_record(0xAA).as_slice());
        assert_eq!(s.session_data(&b).unwrap().len(), SESSION_DATA_LEN);
        assert_eq!(s.describe(&a).unwrap()["sessionType"], "owner-bind");
    }

    #[test]
    fn invite_fetch_join_leave() {
        let mut s = PlatformSessions::new();
        let a = create(&mut s, "HostA", 0xAA);
        s.join(&a, "HostA").unwrap();
        let body = multipart(&[
            ("invitation-request", "application/json", br#"{"to":["Friend"],"message":""}"#),
            ("invitation-data", "application/octet-stream", &host_record(0xAA)),
        ]);
        let ids = s.invite(&a, "HostA", Some(CT), &body).unwrap();
        assert_eq!(ids.len(), 1);
        let listed = s.invitations_for("Friend");
        assert_eq!(listed["totalResults"], 1);
        assert_eq!(listed["invitations"][0]["sessionId"], a.as_str());
        assert!(s.invitation_data("Stranger", &ids[0]).is_err(), "only the invitee reads it");
        assert_eq!(s.invitation_data("Friend", &ids[0]).unwrap(), host_record(0xAA));
        assert_eq!(s.invitations_for("Friend")["invitations"][0]["usedFlag"], true);
        // The invitee joins the host's session.
        s.join(&a, "Friend").unwrap();
        assert_eq!(s.describe(&a).unwrap()["members"].as_array().unwrap().len(), 2);
        assert_eq!(s.sessions_of("Friend")["totalResults"], 1);
        assert!(!s.leave(&a, "Friend").unwrap());
        assert!(s.leave(&a, "HostA").unwrap(), "the owner leaving last ends it");
        assert!(s.session(&a).is_none());
        assert!(s.invitations_for("Friend")["invitations"].as_array().unwrap().is_empty());
    }

    #[test]
    fn a_session_without_its_record_is_refused_and_updates_merge() {
        let mut s = PlatformSessions::new();
        let body = multipart(&[("session-request", "application/json", b"{}")]);
        assert_eq!(s.create("x", Some(CT), &body), Err(SessionError::NoSessionData));
        let a = create(&mut s, "HostA", 1);
        s.update(&a, br#"{"sessionMaxUser":6}"#);
        assert_eq!(s.describe(&a).unwrap()["sessionMaxUser"], 6);
        assert_eq!(s.describe(&a).unwrap()["sessionType"], "owner-bind");
        assert_eq!(s.update("000-00000000-0000-0000-0000-000000000000", b"{}")["sessionId"], "000-00000000-0000-0000-0000-000000000000");
        s.put_changeable_data(&a, b"xyz").unwrap();
        assert_eq!(s.changeable_data(&a).unwrap(), b"xyz");
    }

    #[test]
    fn multipart_parts_are_found_by_description_or_name_and_the_boundary_by_sniffing() {
        let body = b"--X\r\nContent-Disposition: form-data; name=\"session-data\"\r\n\r\nabc\r\n--X--\r\n";
        let parts = parse_multipart(None, body);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].role().as_deref(), Some("session-data"));
        assert_eq!(parts[0].body, b"abc");
        assert_eq!(boundary_of("multipart/mixed; boundary=\"bUnGiEr0x0rS\""), Some("bUnGiEr0x0rS".into()));
        assert!(parse_multipart(Some("text/plain"), b"").is_empty());
    }

    #[test]
    fn presence_is_per_player() {
        let mut s = PlatformSessions::new();
        s.put_game_data("A", b"in orbit");
        s.put_game_data("B", b"tower");
        assert_eq!(s.game_data("A"), Some(&b"in orbit"[..]));
        s.clear_game_status("A");
        assert_eq!(s.game_data("A"), None);
        assert_eq!(s.game_data("B"), Some(&b"tower"[..]));
    }
}
