// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! One client connection: read frames, answer what we implement, log what we don't.
//!
//! ## Why the stream is split
//!
//! Until rooms existed this was a strict `read → dispatch → reply` loop over an unsplit
//! `TcpStream`, and `shadnet_wire::encode_notification` had **zero callers in the workspace**.
//! That shape cannot push: while a task is parked in `read_packet` there is no writer to call, and
//! a room protocol that cannot deliver `MEMBER_JOINED` stalls the second participant forever.
//!
//! So the socket is split, every outbound frame goes through a per-connection `mpsc` outbox, and a
//! writer task drains it. One channel, so ordering is preserved for free - which matters, because
//! the client requires that a member notification never overtake the reply that populates its
//! `ctx->room_cache`.

use std::sync::Arc;
use std::net::SocketAddr;

use prost::Message;
use shadnet_wire::{
    encode_notification, encode_reply, read_packet, write_all, Command, ErrorType, Notification,
    Packet, PacketType, ReadError,
};
use tokio::net::TcpStream;
use tracing::{debug, info, warn};

use crate::gates::GateLedger;
use crate::matching;
use crate::registry::{endpoint_for, LoginRefusal, Outbox, ServerState, SessionHandle};
use crate::rooms::{MemberIntent, Outcome, RoomError, RoomSpec};

pub struct Session {
    conn_id: u64,
    peer: SocketAddr,
    state: Arc<ServerState>,
    outbox: Outbox,
    ledger: GateLedger,
    authenticated: bool,
    /// The npid this connection claimed at `Login`. Empty until then.
    npid: String,
}

impl Session {
    pub async fn run(stream: TcpStream, peer: SocketAddr, state: Arc<ServerState>) {
        // Destiny's traffic is small and latency-sensitive; Nagle only muddies the trace timing.
        if let Err(e) = stream.set_nodelay(true) {
            warn!(peer = %peer, "set_nodelay failed: {e}");
        }
        let (mut reader, mut writer) = stream.into_split();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

        let writer_task = tokio::spawn(async move {
            while let Some(bytes) = rx.recv().await {
                if let Err(e) = write_all(&mut writer, &bytes).await {
                    warn!("write failed: {e}");
                    return;
                }
            }
        });

        let conn_id = state.alloc_conn_id();
        state.register(SessionHandle {
            conn_id,
            npid: String::new(),
            peer,
            ctx_id: None,
            outbox: tx.clone(),
        });
        info!(peer = %peer, conn_id, "client connected");

        let mut session = Session {
            conn_id,
            peer,
            state: state.clone(),
            outbox: tx,
            ledger: GateLedger::new(),
            authenticated: false,
            npid: String::new(),
        };

        // The server speaks first. shadPS4 blocks on this immediately after connect() and drops the
        // socket if it doesn't arrive, so nothing else can happen until it's sent.
        let hello = shadnet_wire::encode_server_info(shadnet_wire::PROTOCOL_VERSION);
        if session.send(hello).is_err() {
            warn!(peer = %peer, "failed to queue ServerInfo handshake");
            return;
        }
        debug!(
            peer = %peer,
            protocol_version = shadnet_wire::PROTOCOL_VERSION,
            "--> ServerInfo handshake sent"
        );

        loop {
            match read_packet(&mut reader).await {
                Ok(packet) => {
                    if session.dispatch(packet).is_err() {
                        break;
                    }
                }
                Err(ReadError::Eof) => break,
                Err(e) => {
                    warn!(peer = %peer, "read failed: {e}");
                    break;
                }
            }
        }

        // Leaving the room on disconnect is not housekeeping: without it, the peer that stayed
        // holds a member that will never answer, and the failure shows up much later as a peer
        // that never handshakes.
        if let Some(outcome) = state.disconnect(conn_id) {
            info!(
                peer = %peer,
                conn_id,
                room_id = outcome.room_id,
                "connection dropped while in a room - telling the remaining members"
            );
            deliver(&state, &outcome);
        }
        drop(session.outbox);
        let _ = writer_task.await;

        if session.ledger.is_empty() {
            info!(peer = %peer, "client disconnected without sending a request");
        } else {
            info!(
                peer = %peer,
                conn_id,
                npid = %session.npid,
                authenticated = session.authenticated,
                "client disconnected. session ledger:{}",
                session.ledger.summary()
            );
        }
    }

    /// Queue an already-framed packet. `Err` only when the writer task is gone.
    fn send(&self, bytes: Vec<u8>) -> Result<(), ()> {
        self.outbox.send(bytes).map_err(|_| ())
    }

    fn reply(&self, packet: &Packet, error: ErrorType, proto: &[u8]) -> Result<(), ()> {
        self.send(encode_reply(packet.command, packet.packet_id, error, proto))
    }

    fn dispatch(&mut self, packet: Packet) -> Result<(), ()> {
        if packet.packet_type != PacketType::Request {
            warn!(
                peer = %self.peer,
                "unexpected packet_type={:?} cmd={} - ignoring",
                packet.packet_type, packet.command_name()
            );
            return Ok(());
        }

        let name = packet.command_name();
        let first = self.ledger.record(&name);
        let blob_len = packet.request_blob().len();

        if first {
            info!(peer = %self.peer, cmd = %name, packet_id = packet.packet_id, bytes = blob_len, "--> request");
        } else {
            let n = self.ledger.count(&name);
            if n % 100 == 0 {
                debug!(peer = %self.peer, cmd = %name, count = n, "--> request (repeating)");
            }
        }

        let cmd = Command::from_u16(packet.command);
        match cmd {
            Some(Command::Login) => self.handle_login(&packet),
            Some(Command::GetToken) => self.handle_get_token(&packet),
            Some(Command::GetServerFeatures) => self.handle_server_features(&packet),
            Some(Command::SetClientVersion) => self.handle_client_version(&packet),
            Some(Command::Terminate) => {
                info!(peer = %self.peer, "client requested Terminate");
                self.reply(&packet, ErrorType::NoError, &[])
            }
            Some(c) if is_room_command(c) => {
                if self.state.config.rooms_enabled {
                    self.handle_room(&packet, c)
                } else {
                    // Unchanged from before rooms existed, and deliberately so: `--rooms` is off by
                    // default because nothing has asked (PLAN-B §0/§7). The gate stays named.
                    self.handle_unimplemented(&packet, &name)
                }
            }
            _ => self.handle_unimplemented(&packet, &name),
        }
    }

    fn handle_login(&mut self, packet: &Packet) -> Result<(), ()> {
        let reported = match shadnet_proto::LoginRequest::decode(packet.request_blob()) {
            Ok(req) => {
                info!(
                    peer = %self.peer,
                    npid = %req.npid,
                    title_id = %req.title_id,
                    title_name = %req.title_name,
                    appear_offline = req.appear_offline,
                    has_token = !req.token.is_empty(),
                    "LOGIN request"
                );
                req.npid
            }
            Err(e) => {
                warn!(peer = %self.peer, "LoginRequest failed to decode: {e}");
                String::new()
            }
        };

        let (npid, identity) = match self.state.login(self.conn_id, &reported) {
            Ok(v) => v,
            Err(LoginRefusal::AlreadyLoggedIn) => {
                // ⭐ Not a nuisance error: in a two-instance run this is the fixture telling you
                // both instances were handed the same identity, which §4 of the plan calls the
                // conflation that reads as a networking bug.
                warn!(
                    peer = %self.peer,
                    npid = %reported,
                    "⛔ REFUSING LOGIN - that npid already has a live connection. Two participants \
                     sharing one identity is the conflation the harness contract forbids; give the \
                     second instance its own NP online id."
                );
                return self.reply(packet, ErrorType::LoginAlreadyLoggedIn, &[]);
            }
        };

        self.npid = npid.clone();
        self.state.set_npid(self.conn_id, &npid);

        // Must carry at least one populated field: shadPS4 rejects an empty blob as malformed.
        let reply = shadnet_proto::LoginReply {
            avatar_url: identity.avatar_url.clone(),
            user_id: identity.user_id,
            ..Default::default()
        };
        self.authenticated = true;
        info!(
            peer = %self.peer,
            conn_id = self.conn_id,
            room_key_npid = %npid,
            issued_npid = %identity.npid,
            user_id = identity.user_id,
            "<-- LoginReply - NP identity granted (gate A satisfied)"
        );
        self.reply(packet, ErrorType::NoError, &reply.encode_to_vec())
    }

    fn handle_get_token(&mut self, packet: &Packet) -> Result<(), ()> {
        let identity = self.state.identity_for(&self.npid);
        let reply = shadnet_proto::GetTokenReply {
            token: identity.token,
            user_id: identity.user_id,
            npid: identity.npid,
        };
        info!(peer = %self.peer, "<-- GetTokenReply");
        self.reply(packet, ErrorType::NoError, &reply.encode_to_vec())
    }

    fn handle_server_features(&mut self, packet: &Packet) -> Result<(), ()> {
        // Trophies stay off: nothing here stores them (UnlockTrophy and SyncTrophies are Unsupported).
        let reply = shadnet_proto::ServerFeaturesReply {
            matching2_enabled: self.state.config.matching2_enabled,
            trophies_enabled: false,
        };
        info!(
            peer = %self.peer,
            matching2_enabled = self.state.config.matching2_enabled,
            "<-- ServerFeaturesReply"
        );
        self.reply(packet, ErrorType::NoError, &reply.encode_to_vec())
    }

    /// The emulator says which build it is after signing in (shadNet's `SetClientVersion`, 13).
    /// Upstream records it and can refuse versions it doesn't allow; we log it, so every run says
    /// which emulator build it ran on, and accept it. Until 1 November 2026 it was `Unsupported`.
    fn handle_client_version(&mut self, packet: &Packet) -> Result<(), ()> {
        let Ok(req) = shadnet_proto::SetClientVersionRequest::decode(packet.request_blob()) else {
            return self.reply(packet, ErrorType::Malformed, &[]);
        };
        info!(peer = %self.peer, version = %client_version(&req.version), "client version");
        self.reply(packet, ErrorType::NoError, &[])
    }

    /// Refuse cleanly and loudly. A clean `Unsupported` beats a hang or a plausible lie: the client
    /// gets a definite answer, and we get a named gate instead of a mystery.
    fn handle_unimplemented(&mut self, packet: &Packet, name: &str) -> Result<(), ()> {
        if self.ledger.count(name) == 1 {
            warn!(
                peer = %self.peer,
                cmd = %name,
                bytes = packet.request_blob().len(),
                "** NEW GATE ** unimplemented - answering Unsupported"
            );
        }
        self.ledger.record_unimplemented(name);
        self.reply(packet, ErrorType::Unsupported, &[])
    }

    // ── the room commands ────────────────────────────────────────────────────────────────────

    fn member_intent(&self, team_id: u32, bin_attrs: &[shadnet_proto::MatchingBinAttr]) -> Option<MemberIntent> {
        let endpoint = endpoint_for(self.peer, self.state.config.peer_port)?;
        Some(MemberIntent {
            conn_id: self.conn_id,
            npid: self.npid.clone(),
            team_id,
            endpoint,
            // NAT type 2 = "moderate". `[I]` - nothing on the wire tells us, and the client only
            // republishes it. 0 would read as "unknown/strict" and is a stronger claim than we have.
            nat_type: 2,
            flag_attr: 0,
            group_id: 0,
            account_id: 0,
            platform: 0,
            join_date: now_ms(),
            bin_attrs_internal: matching::bin_attrs_from(bin_attrs),
        })
    }

    fn handle_room(&mut self, packet: &Packet, cmd: Command) -> Result<(), ()> {
        if !self.authenticated {
            warn!(peer = %self.peer, cmd = %cmd, "room command before Login - refusing");
            return self.reply(packet, ErrorType::Unauthorized, &[]);
        }
        let blob = packet.request_blob();
        match cmd {
            Command::ContextStart => {
                let Ok(req) = shadnet_proto::ContextStartRequest::decode(blob) else {
                    return self.reply(packet, ErrorType::Malformed, &[]);
                };
                // ctx_id 0 is not a wildcard: `ContextManager::Get(0)` misses and every
                // notification we later addressed to it would vanish with no log line on either
                // side. Checked FIRST so the refusal names the real problem.
                if req.ctx_id == 0 {
                    warn!(peer = %self.peer, "ContextStart with ctx_id 0 - valid ids are 1..10");
                    return self.reply(packet, ErrorType::InvalidInput, &[]);
                }
                // ⚠ A second ContextStart on one connection is REFUSED rather than guessed at.
                // Notifications carry exactly one ctx_id and the schema comment says the server
                // tracks it per connection; multi-context attribution is a standing unknown
                // (PLAN-B §9.22) and the client never exercises it.
                if let Some(existing) = self.state.ctx_id(self.conn_id) {
                    if existing != req.ctx_id {
                        warn!(
                            peer = %self.peer,
                            existing,
                            requested = req.ctx_id,
                            "second ContextStart on one connection - refusing rather than guessing \
                             which context a notification belongs to"
                        );
                        return self.reply(packet, ErrorType::Invalid, &[]);
                    }
                }
                self.state.set_ctx_id(self.conn_id, req.ctx_id);
                info!(peer = %self.peer, ctx_id = req.ctx_id, "<-- ContextStart accepted");
                // No reply body: `DispatchRequestComplete` routes CONTEXT_EVENT_STARTED without
                // parsing one.
                self.reply(packet, ErrorType::NoError, &[])
            }

            Command::ContextStop => {
                let outcome = {
                    let mut rooms = self.state.rooms.lock().unwrap();
                    rooms.drop_connection(self.conn_id)
                };
                self.state.clear_ctx_id(self.conn_id);
                info!(peer = %self.peer, "<-- ContextStop accepted");
                self.reply(packet, ErrorType::NoError, &[])?;
                if let Some(o) = outcome {
                    deliver(&self.state, &o);
                }
                Ok(())
            }

            Command::CreateRoom => {
                let Ok(req) = shadnet_proto::CreateRoomRequest::decode(blob) else {
                    return self.reply(packet, ErrorType::Malformed, &[]);
                };
                let Some(intent) = self.member_intent(req.team_id, &req.member_bin_attrs) else {
                    return self.reply(packet, ErrorType::InvalidInput, &[]);
                };
                let spec = RoomSpec {
                    world_id: req.world_id,
                    lobby_id: req.lobby_id,
                    max_slots: req.max_slots,
                    flags: req.flags,
                    // Bytes only arrive here. `JoinRoomRequest` has no password field at all, so
                    // this is the only place a password ever exists on the wire.
                    password: (req.room_password_present && !req.room_password.is_empty())
                        .then(|| req.room_password.clone()),
                    passwd_slot_mask: req.has_passwd_slot_mask.then_some(req.passwd_slot_mask),
                    groups: matching::groups_from_configs(&req.group_configs),
                    bin_attrs_internal: matching::bin_attrs_from(&req.internal_bin_attrs),
                    search_int_attrs: matching::int_attrs_from(&req.external_search_int_attrs),
                    search_bin_attrs: matching::bin_attrs_from(&req.external_search_bin_attrs),
                    ext_bin_attrs: matching::bin_attrs_from(&req.external_bin_attrs),
                };
                let private = spec.password.is_some();
                let result = {
                    let mut rooms = self.state.rooms.lock().unwrap();
                    match rooms.create_room(spec, intent) {
                        Ok(o) => {
                            let room = rooms.get(o.room_id).expect("just created").clone();
                            Ok((o, room))
                        }
                        Err(e) => Err(e),
                    }
                };
                match result {
                    Ok((o, room)) => {
                        let reply = match matching::create_room_reply(&room, o.me_member_id) {
                            Ok(r) => r,
                            Err(e) => return self.refuse_emit(packet, e),
                        };
                        if let Err(e) = matching::guard_create_join_reply(reply.details.as_ref()) {
                            return self.refuse_emit(packet, e);
                        }
                        info!(
                            peer = %self.peer,
                            room_id = o.room_id,
                            member_id = o.me_member_id,
                            max_slots = room.max_slots,
                            private,
                            "<-- CreateRoomReply"
                        );
                        self.reply(packet, ErrorType::NoError, &reply.encode_to_vec())
                    }
                    Err(e) => self.refuse_room(packet, cmd, e),
                }
            }

            Command::JoinRoom => {
                let Ok(req) = shadnet_proto::JoinRoomRequest::decode(blob) else {
                    return self.reply(packet, ErrorType::Malformed, &[]);
                };
                let Some(intent) = self.member_intent(req.team_id, &req.member_bin_attrs) else {
                    return self.reply(packet, ErrorType::InvalidInput, &[]);
                };
                let result = {
                    let mut rooms = self.state.rooms.lock().unwrap();
                    match rooms.join_room(req.room_id, req.room_password_present, intent) {
                        Ok(o) => {
                            let room = rooms.get(o.room_id).expect("just joined").clone();
                            Ok((o, room))
                        }
                        Err(e) => Err(e),
                    }
                };
                match result {
                    Ok((o, room)) => {
                        let reply = match matching::join_room_reply(&room, o.me_member_id) {
                            Ok(r) => r,
                            Err(e) => return self.refuse_emit(packet, e),
                        };
                        if let Err(e) = matching::guard_create_join_reply(reply.details.as_ref()) {
                            return self.refuse_emit(packet, e);
                        }
                        info!(
                            peer = %self.peer,
                            room_id = o.room_id,
                            member_id = o.me_member_id,
                            cur_members = room.members.len(),
                            "<-- JoinRoomReply"
                        );
                        // ⚠ ORDER IS A CONTRACT: the reply first, THEN the fan-out. A member
                        // notification that arrives before the reply finds an empty
                        // `ctx->room_cache` and is discarded without a log line.
                        self.reply(packet, ErrorType::NoError, &reply.encode_to_vec())?;
                        deliver(&self.state, &o);
                        Ok(())
                    }
                    Err(e) => self.refuse_room(packet, cmd, e),
                }
            }

            Command::LeaveRoom => {
                let Ok(req) = shadnet_proto::LeaveRoomRequest::decode(blob) else {
                    return self.reply(packet, ErrorType::Malformed, &[]);
                };
                let result = {
                    let mut rooms = self.state.rooms.lock().unwrap();
                    rooms.leave_room(req.room_id, self.conn_id)
                };
                match result {
                    Ok(o) => {
                        let reply = shadnet_proto::LeaveRoomReply { room_id: o.room_id };
                        info!(peer = %self.peer, room_id = o.room_id, destroyed = o.room_destroyed, "<-- LeaveRoomReply");
                        self.reply(packet, ErrorType::NoError, &reply.encode_to_vec())?;
                        deliver(&self.state, &o);
                        Ok(())
                    }
                    Err(e) => self.refuse_room(packet, cmd, e),
                }
            }

            Command::SearchRoom => {
                let Ok(req) = shadnet_proto::SearchRoomRequest::decode(blob) else {
                    return self.reply(packet, ErrorType::Malformed, &[]);
                };
                let filter = matching::search_filter_from(&req);
                let reply = {
                    let rooms = self.state.rooms.lock().unwrap();
                    let (window, start, total) = rooms.search(&filter);
                    matching::search_room_reply(&window, start, total)
                };
                info!(
                    peer = %self.peer,
                    matched = reply.range_result,
                    total = reply.range_total,
                    "<-- SearchRoomReply"
                );
                self.reply(packet, ErrorType::NoError, &reply.encode_to_vec())
            }

            Command::RequestSignalingInfos => {
                let Ok(req) = shadnet_proto::RequestSignalingInfosRequest::decode(blob) else {
                    return self.reply(packet, ErrorType::Malformed, &[]);
                };
                // ⚠ The client is BLOCKED on this, on its own ShadNet reader thread, for up to 5 s.
                // Answering fast - even with a refusal - is worth more than answering well.
                match self.state.resolve_signaling(&req.target_npid) {
                    Ok((ep, member_id)) => {
                        let reply = match matching::signaling_reply(&req.target_npid, &ep, member_id)
                        {
                            Ok(r) => r,
                            Err(e) => return self.refuse_emit(packet, e),
                        };
                        info!(
                            peer = %self.peer,
                            target = %req.target_npid,
                            ip = %ep.addr,
                            port = ep.port,
                            "<-- RequestSignalingInfosReply"
                        );
                        self.reply(packet, ErrorType::NoError, &reply.encode_to_vec())
                    }
                    Err(err) => {
                        warn!(
                            peer = %self.peer,
                            target = %req.target_npid,
                            "RequestSignalingInfos: no endpoint for that npid - refusing rather \
                             than answering with the TCP source port, which would be wrong and \
                             would fail silently minutes later"
                        );
                        self.reply(packet, err, &[])
                    }
                }
            }

            Command::SetRoomDataInternal => {
                let Ok(req) = shadnet_proto::SetRoomDataInternalRequest::decode(blob) else {
                    return self.reply(packet, ErrorType::Malformed, &[]);
                };
                let result = {
                    let mut rooms = self.state.rooms.lock().unwrap();
                    rooms.set_room_data_internal(
                        req.room_id,
                        self.conn_id,
                        req.flag_filter,
                        req.flag_attr,
                        matching::bin_attrs_from(&req.bin_attrs),
                        req.has_passwd_mask.then_some(req.passwd_slot_mask),
                    )
                };
                match result {
                    Ok(o) => {
                        let reply = shadnet_proto::SetRoomDataInternalReply { room_id: o.room_id };
                        info!(peer = %self.peer, room_id = o.room_id, "<-- SetRoomDataInternalReply");
                        self.reply(packet, ErrorType::NoError, &reply.encode_to_vec())?;
                        deliver(&self.state, &o);
                        Ok(())
                    }
                    Err(e) => self.refuse_room(packet, cmd, e),
                }
            }

            Command::SetRoomDataExternal => {
                let Ok(req) = shadnet_proto::SetRoomDataExternalRequest::decode(blob) else {
                    return self.reply(packet, ErrorType::Malformed, &[]);
                };
                let result = {
                    let mut rooms = self.state.rooms.lock().unwrap();
                    rooms.set_room_data_external(
                        req.room_id,
                        self.conn_id,
                        matching::int_attrs_from(&req.search_int_attrs),
                        matching::bin_attrs_from(&req.search_bin_attrs),
                        matching::bin_attrs_from(&req.ext_bin_attrs),
                    )
                };
                match result {
                    Ok(o) => {
                        let reply = shadnet_proto::SetRoomDataExternalReply { room_id: o.room_id };
                        info!(peer = %self.peer, room_id = o.room_id, "<-- SetRoomDataExternalReply");
                        // No notification is defined for this event; searchers re-query.
                        self.reply(packet, ErrorType::NoError, &reply.encode_to_vec())
                    }
                    Err(e) => self.refuse_room(packet, cmd, e),
                }
            }

            Command::KickoutRoomMember => {
                let Ok(req) = shadnet_proto::KickoutRoomMemberRequest::decode(blob) else {
                    return self.reply(packet, ErrorType::Malformed, &[]);
                };
                let result = {
                    let mut rooms = self.state.rooms.lock().unwrap();
                    rooms.kickout(req.room_id, self.conn_id, req.target_member_id)
                };
                match result {
                    Ok(o) => {
                        let reply = shadnet_proto::KickoutRoomMemberReply { room_id: o.room_id };
                        info!(
                            peer = %self.peer,
                            room_id = o.room_id,
                            target = req.target_member_id,
                            "<-- KickoutRoomMemberReply"
                        );
                        self.reply(packet, ErrorType::NoError, &reply.encode_to_vec())?;
                        deliver(&self.state, &o);
                        Ok(())
                    }
                    Err(e) => self.refuse_room(packet, cmd, e),
                }
            }

            Command::SetUserInfo => {
                // `SetUserInfoReply` is an empty message by design. Accepting is honest: the
                // attributes are per-npid and nothing reads them back today.
                let reply = shadnet_proto::SetUserInfoReply {};
                info!(peer = %self.peer, "<-- SetUserInfoReply");
                self.reply(packet, ErrorType::NoError, &reply.encode_to_vec())
            }

            Command::GetUserInfoList => {
                let Ok(req) = shadnet_proto::GetUserInfoListRequest::decode(blob) else {
                    return self.reply(packet, ErrorType::Malformed, &[]);
                };
                let users = req
                    .npids
                    .iter()
                    .map(|n| shadnet_proto::MatchingUserInfo {
                        npid: n.clone(),
                        account_id: 0,
                        platform: 0,
                        user_bin_attrs: Vec::new(),
                    })
                    .collect();
                let reply = shadnet_proto::GetUserInfoListReply { users };
                info!(peer = %self.peer, "<-- GetUserInfoListReply");
                self.reply(packet, ErrorType::NoError, &reply.encode_to_vec())
            }

            Command::GetRoomDataExternalList => {
                let Ok(req) = shadnet_proto::GetRoomDataExternalListRequest::decode(blob) else {
                    return self.reply(packet, ErrorType::Malformed, &[]);
                };
                let reply = {
                    let rooms = self.state.rooms.lock().unwrap();
                    shadnet_proto::GetRoomDataExternalListReply {
                        rooms: req
                            .room_ids
                            .iter()
                            .filter_map(|id| rooms.get(*id))
                            .map(matching::room_data_external)
                            .collect(),
                    }
                };
                info!(peer = %self.peer, rooms = reply.rooms.len(), "<-- GetRoomDataExternalListReply");
                self.reply(packet, ErrorType::NoError, &reply.encode_to_vec())
            }

            Command::GetWorldInfoList => {
                // ⛔ PROVENANCE TRAP (PLAN-B §3.5): with `matching2_enabled = false` the EMULATOR
                // fabricates a one-world reply and never sends this command. So an advance past a
                // world-list gate seen while the flag is off was caused by shadPS4, not by us.
                // What we answer here is only ever seen with the flag ON.
                let reply = shadnet_proto::GetWorldInfoListReply {
                    worlds: vec![shadnet_proto::MatchingWorld {
                        world_id: 1,
                        lobbies_num: 0,
                        max_lobby_members: 0,
                        lobby_members_num: 0,
                        rooms_num: self.state.rooms.lock().unwrap().len() as u32,
                        room_members_num: 0,
                    }],
                };
                warn!(
                    peer = %self.peer,
                    "<-- GetWorldInfoListReply - ⚠ note that with matching2_enabled=false shadPS4 \
                     answers this ITSELF and this reply never happens; anything downstream of a \
                     world list is synthetic_ancestor either way"
                );
                self.reply(packet, ErrorType::NoError, &reply.encode_to_vec())
            }

            Command::SendRoomMessage => {
                let Ok(req) = shadnet_proto::SendRoomMessageRequest::decode(blob) else {
                    return self.reply(packet, ErrorType::Malformed, &[]);
                };
                let relayed = self.relay_room_message(&req);
                let reply = shadnet_proto::SendRoomMessageReply { room_id: req.room_id };
                info!(peer = %self.peer, room_id = req.room_id, relayed, "<-- SendRoomMessageReply");
                self.reply(packet, ErrorType::NoError, &reply.encode_to_vec())
            }

            // 114. D1 does not import this NID, so the client cannot send it. Left refused rather
            // than implemented against nothing - the gate ledger will say if that is ever wrong.
            Command::GetRoomMemberDataExternalList => {
                self.handle_unimplemented(packet, &packet.command_name())
            }

            _ => self.handle_unimplemented(packet, &packet.command_name()),
        }
    }

    fn relay_room_message(&self, req: &shadnet_proto::SendRoomMessageRequest) -> usize {
        let rooms = self.state.rooms.lock().unwrap();
        let Some(room) = rooms.get(req.room_id) else { return 0 };
        let Some(me) = room.member_by_conn(self.conn_id) else { return 0 };
        // cast_type 1 = ME, 2 = ROOM, 3 = TEAM, 4 = LIST. Only LIST names its targets; anything we
        // do not recognise fans out to the room, which is the client's own default shape.
        let targets: Vec<u64> = if req.cast_type == 4 && !req.dst_member_ids.is_empty() {
            room.members
                .iter()
                .filter(|m| req.dst_member_ids.contains(&m.member_id))
                .map(|m| m.conn_id)
                .collect()
        } else {
            room.others(self.conn_id)
        };
        let mut sent = 0;
        for conn in targets {
            let Some(handle) = self.state.session(conn) else { continue };
            let Some(ctx_id) = handle.ctx_id else { continue };
            let proto = shadnet_proto::NotifyRoomMessage {
                ctx_id,
                room_id: req.room_id,
                src_member_id: me.member_id,
                event: 0,
                cast_type: req.cast_type,
                dst_member_ids: req.dst_member_ids.clone(),
                src_npid: me.npid.clone(),
                src_account_id: me.account_id,
                src_platform: me.platform,
                msg: req.msg.clone(),
            };
            if handle
                .outbox
                .send(encode_notification(Notification::RoomMessage, &proto.encode_to_vec()))
                .is_ok()
            {
                sent += 1;
            }
        }
        sent
    }

    fn refuse_room(&mut self, packet: &Packet, cmd: Command, e: RoomError) -> Result<(), ()> {
        let err = e.as_error_type();
        warn!(peer = %self.peer, cmd = %cmd, error = %err, "room command refused: {e:?}");
        self.reply(packet, err, &[])
    }

    /// We built something the client would mis-handle. That is our bug, and it gets a loud line -
    /// a silent send would have produced a null-pointer callback or a five-second stall instead.
    fn refuse_emit(&mut self, packet: &Packet, e: matching::EmitError) -> Result<(), ()> {
        warn!(peer = %self.peer, "⛔ REFUSING TO SEND: {e}");
        self.reply(packet, matching::emit_error_type(&e), &[])
    }
}

fn is_room_command(c: Command) -> bool {
    (c as u16) >= 100 && (c as u16) <= 115
}

/// Milliseconds since the Unix epoch, for `join_date`.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Fan a room outcome out to the connections it names.
///
/// Each recipient's **own** `ctx_id` is looked up here, per target. A recipient that never called
/// `ContextStart` is skipped with a warning rather than sent a notification carrying somebody
/// else's context: `ContextManager::Get` would miss and the frame would vanish with no log line on
/// either side, which is the worst of both.
pub fn deliver(state: &ServerState, outcome: &Outcome) {
    for n in &outcome.notifications {
        for conn in &n.targets {
            let Some(handle) = state.session(*conn) else { continue };
            let proto = match matching::room_event_proto(n, handle.ctx_id) {
                Ok(p) => p,
                Err(e) => {
                    warn!(conn_id = conn, "not delivering room event: {e}");
                    continue;
                }
            };
            if let Err(e) = matching::guard_room_event(&proto) {
                warn!(conn_id = conn, "⛔ REFUSING TO SEND notification: {e}");
                continue;
            }
            let bytes = encode_notification(Notification::RoomEvent, &proto.encode_to_vec());
            if handle.outbox.send(bytes).is_err() {
                debug!(conn_id = conn, "notification dropped - connection already gone");
            } else {
                debug!(
                    conn_id = conn,
                    event = format!("{:?}", n.event),
                    ctx_id = proto.ctx_id,
                    "--> room event"
                );
            }
        }
    }
}

/// The reported version as upstream keeps it: whitespace collapsed, control characters dropped,
/// and clipped at 256 characters with an ellipsis, so a clipped value is never taken for the real one.
fn client_version(raw: &str) -> String {
    const MAX: usize = 256;
    let clean: String =
        raw.split_whitespace().collect::<Vec<_>>().join(" ").chars().filter(|c| !c.is_control()).collect();
    if clean.chars().count() > MAX {
        clean.chars().take(MAX - 1).chain(std::iter::once('…')).collect()
    } else {
        clean
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_client_version_is_cleaned_and_clipped_as_upstream_does() {
        assert_eq!(client_version("  shadPS4 v0.19.0\tWIP d1-34c579f\u{7} "), "shadPS4 v0.19.0 WIP d1-34c579f");
        assert_eq!(client_version(""), "");
        let long = client_version(&"x".repeat(300));
        assert_eq!(long.chars().count(), 256);
        assert!(long.ends_with('…'));
    }

    #[test]
    fn the_room_command_range_is_exactly_the_matchmaking_block() {
        assert!(is_room_command(Command::ContextStart));
        assert!(is_room_command(Command::SendRoomMessage));
        // The client's own dispatch routes 100..=200 to OnMatchingReply and everything else to
        // OnScoreReply, so a command outside the block answered as a room command would land in
        // the wrong subsystem.
        assert!(!is_room_command(Command::Login));
        assert!(!is_room_command(Command::GetToken));
        assert!(!is_room_command(Command::GetServerFeatures));
        assert!(!is_room_command(Command::SetAppearOffline));
        let in_block: Vec<u16> =
            Command::ALL.iter().filter(|(_, v)| (100..=115).contains(v)).map(|(_, v)| *v).collect();
        assert_eq!(in_block.len(), 16);
    }
}
