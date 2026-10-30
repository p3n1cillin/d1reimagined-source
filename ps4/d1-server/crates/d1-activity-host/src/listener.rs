// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The Activity Host's own listener - the second BAP endpoint.
//!
//! ## What the client actually does, and why this is a BAP endpoint at all
//!
//! After message 17 hands it an address, the client *"builds a transport address, acquires a BAP
//! channel (indices 1..3), and calls `FUN_00559170` to connect"*. The connection it opens is a
//! full BAP connection in slot 1 `OUT FAH` or 2/3 `OUT GAH0`/`GAH1`, built by the same
//! `FUN_00559650(index)` that builds `OUT PRIMARY`. `[C]`
//!
//! ⇒ The Activity Host is not a bespoke socket protocol. It is **BAP framing carrying
//! `user_defined_type` payloads inside messages 8 and 100** - which is why this module contains no
//! codec at all and calls `d1-bap` for every byte of framing and crypto.
//!
//! ## The socket handoff, and the `[U]` behind it
//!
//! `bap_to_activity_host_socket_handoff_request` (BAP id **153**) says that in Bungie's own
//! deployment BAP accepts the connection and hands the *socket* to the Activity Host process. Our
//! topology can be either shape, so it is a value rather than an assumption:
//!
//! * [`AhBind::Listen`] - we own the listening socket. Correct when message 17 hands out our own
//!   address, which is what [`crate::registry`] does today.
//! * [`AhBind::Handoff`] - someone else accepted, and hands us an established stream. This is the
//!   shape that matches id 153, and it is what `d1-responder` would use if the AH ever needs to
//!   live behind the primary BAP front door.
//!
//! `[U]` **which of the two Bungie used, and whether the resolved address is the AH process or a
//! BAP-side relay.** A retail capture answers it - endpoint topology survives encryption, which is
//! the one thing a passive tap on this protocol is actually good for.
//!
//! ## The Weasel applies here too
//!
//! Plan §7 Phase 4 names the regression: `_connection_failure_failed_to_find_message_recipient` on
//! the *second* connection would mean the correlation table is not per-connection. So every
//! connection here gets **its own** [`BapConversation`], and the request path is
//! `enqueue → handler → park → drain` exactly as on the primary.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use d1_bap::conversation::{BapConversation, EnqueueOutcome};
use d1_bap::{ChannelNonces, Direction, SealKeys, SessionMaterial};

use crate::codec::AhWireCodec;
use crate::messages::ClientMessage;
use crate::push::AhPush;
use crate::state::ActivityInstance;

/// Where the Activity Host gets its connections.
pub enum AhBind {
    /// We listen. `SocketAddr` is what [`crate::registry::HostRegistry`] hands out in message 17.
    Listen(SocketAddr),
    /// Someone else accepted and hands us established streams - the shape BAP id 153 implies.
    Handoff(tokio::sync::mpsc::Receiver<tokio::net::TcpStream>),
}

/// One thing that happened on an AH connection, for the run log and for tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AhEvent {
    /// A plaintext BAP handshake message was answered. `(msg_type, reply_len)`.
    Handshake { msg_type: u16, reply_len: usize },
    /// The secured channel is up. From here on, everything is sealed.
    ChannelEstablished,
    /// A BAP message 8 arrived and we got as far as its `user_defined_type`.
    ClientNotification { user_defined_type: u32, payload_len: usize },
    /// ⭐ An inbound message this Activity Host **chose** not to decode, with the reason.
    ///
    /// Distinct from [`AhEvent::NamedGate`] on purpose: a gate is *"something correct stopped here
    /// and we can state why"*, which invites work. This is *"we read the descriptor, we know
    /// exactly what it would take, and it changes nothing"*. Run
    /// `20260809-054417-RUNG33-BYPASS` spent its whole log shouting the first when it meant the
    /// second.
    NotDecoded { user_defined_type: u32, name: &'static str, payload_len: usize, why: String },
    /// ⭐ The hex of an inbound payload we could not act on, so the **next** session has a captured
    /// vector to decode instead of a length. `20260809-054417-RUNG33-BYPASS` recorded four udts by
    /// length only, and a length cannot be a control.
    Payload { user_defined_type: u32, hex: String, truncated: bool },
    /// The AH-protocol layer produced something to say.
    WantsToPush { user_defined_type: u32 },
    /// ⭐ A **named gate**: something correct stopped here for a reason we can state. This is the
    /// crate's primary output today, not a failure.
    NamedGate { what: &'static str, detail: String },
    /// ⭐⭐ **OUR CODEC BROKE.** Not a gate - a host defect that has stopped the run producing.
    ///
    /// It exists because run `20260809-120451-MOONBASE-FRESH` reported its encoder running out of
    /// memory using the same words, and at the same level, as *"this udt's schema tag is not pinned
    /// yet"*: `** NAMED GATE ** push blocked`, at INFO on the heartbeat path. The host then went
    /// silent and the client declared `error.private_activity_host_host_crashed`, so every
    /// observable in the run said *the client rejected us* when the truth was *we stopped talking*.
    ///
    /// `occurrence` counts from 1 so a reader can tell the first fault from the thousandth.
    CodecFault { user_defined_type: u32, detail: String, occurrence: usize },
    /// A request arrived that BAP says expects a reply. Recorded so the FIFO discipline is visible.
    Enqueued { msg_type: u16, sequence: u32 },
}

/// How many payload bytes an [`AhEvent::Payload`] carries before it truncates.
///
/// 2 KiB of hex is 4 KiB of log line. Big enough for every message on the establish path (the
/// largest inbound one measured is udt 37 at 1,319 bytes) and small enough that a chatty udt does
/// not bury the run log.
pub const PAYLOAD_CAPTURE_LIMIT: usize = 2048;

/// Extra changed client-update payloads per connection (udt 21, 47 and the udt 6 sense update),
/// beyond the normal first capture.
/// At 2 KiB per payload this bounds additional raw payload log text to about 8 MiB. Run sense35
/// spent 256 before its first enemy kill, so the kill's sense update went unrecorded.
pub const CLIENT_UPDATE_CAPTURE_LIMIT: usize = 2048;

/// Opt-in diagnostics only. No protocol state, authority or outgoing message is changed.
struct ClientUpdateCapture {
    remaining: usize,
    after: Duration,
    started: Option<Instant>,
    previous: [Option<Vec<u8>>; 3],
    window_seen: [bool; 3],
}

impl ClientUpdateCapture {
    fn observe(&mut self, udt: u32, payload: &[u8], now: Instant, first: bool) -> bool {
        let slot = match udt {
            21 => 0,
            47 => 1,
            // The client's raw sensor_sense_update: squad and object state it reports back. Only
            // the first one was ever logged, which is not enough to decode it.
            6 => 2,
            _ => return false,
        };
        if self.remaining == 0 {
            return false;
        }
        let started = *self.started.get_or_insert(now);
        let changed = self.previous[slot].as_deref() != Some(payload);
        // Do not retain an unbounded payload. Oversized updates are always eligible and
        // explicitly truncated, so a change beyond the captured prefix is never hidden.
        self.previous[slot] = if payload.len() <= PAYLOAD_CAPTURE_LIMIT {
            Some(payload.to_vec())
        } else {
            None
        };
        if now.saturating_duration_since(started) < self.after {
            return false;
        }
        // Establish a baseline inside the selected window even if this payload did not
        // change at the boundary. The startup capture may be minutes out of date.
        let baseline = !self.window_seen[slot];
        self.window_seen[slot] = true;
        if first || (!changed && !baseline) {
            return false;
        }
        self.remaining -= 1;
        true
    }
}

/// Hex of an inbound payload, for the run log. See [`AhEvent::Payload`].
fn payload_event(user_defined_type: u32, payload: &[u8]) -> AhEvent {
    let n = payload.len().min(PAYLOAD_CAPTURE_LIMIT);
    let mut hex = String::with_capacity(n * 2);
    for b in &payload[..n] {
        hex.push_str(&format!("{b:02x}"));
    }
    AhEvent::Payload {
        user_defined_type,
        hex,
        truncated: payload.len() > PAYLOAD_CAPTURE_LIMIT,
    }
}

/// A single Activity-Host connection, **sans-io**.
///
/// Everything the listener does to bytes lives here so it can be tested without a socket. The
/// tokio wrapper below only moves bytes in and out.
pub struct AhConnection {
    conv: BapConversation,
    channel: Option<([u8; 16], ChannelNonces)>,
    instance: ActivityInstance,
    codec: Arc<dyn AhWireCodec>,
    /// Everything we wanted to push but could not. Kept rather than dropped: an Activity Host that
    /// silently discards its own output is indistinguishable from one that has nothing to say.
    pub blocked_pushes: Vec<(u32, String)>,
    /// ⭐ Frames produced by a **push**, waiting to go out.
    ///
    /// A push is not a reply: `BapConversation::push_with_sequence` takes `&self` and parks
    /// nothing, so `drain` will never return it. Before 2026-08-08 `try_push` matched
    /// `Ok(_frame) => Vec::new()` and the bytes were **built correctly and then dropped on the
    /// floor** - the join_result was assembled, framed, and never sent, which on screen is
    /// indistinguishable from never having built it.
    pending_pushes: Vec<Vec<u8>>,
    /// The host registry, so this connection can answer BAP message **6** itself.
    ///
    /// ⚠ It must: once message 17 points the client's activity-host channel at *this* listener,
    /// message 6 arrives **here**, not at `d1-responder`. Leaving it unanswered produces
    /// `error.starting_private_activity_instance_timed_out_waiting_for_host_startup_response`
    /// (on screen: **"cacao"**) - measured in run `20260808-205039-LVL40-AH`.
    registry: crate::registry::HostRegistry,
    /// Which `user_defined_type`s have already had their payload logged. See [`Self::capture_once`].
    captured: std::collections::BTreeSet<u32>,
    client_update_capture: Option<ClientUpdateCapture>,
    /// udt 21 is a replicated image, not independent zero-based telemetry. Its
    /// target region and transition token can arrive in different deltas. Keep
    /// the baseline on the connection, never in the process-wide codec worker.
    client_authoritative_image: Vec<u8>,
    /// Native sense baselines and queues belong to this connection/world epoch only.
    sense: Option<crate::sense::SenseWorker>,
    /// ⭐ How many [`AhEvent::CodecFault`]s this connection has produced. Once this is non-zero the
    /// host is no longer saying anything the client can act on, and the run should be read as
    /// **truncated**, not as a protocol result.
    pub codec_faults: usize,
}

/// BAP id 6, `client_to_activity_host_manager_request`.
const BAP_TYPE_AH_MANAGER_REQUEST: u16 = 6;

/// BAP id 8, `client_to_activity_host_notification`. Decimal 8 / `0x0008`.
pub const BAP_TYPE_CLIENT_TO_AH_NOTIFICATION: u16 = 8;

impl AhConnection {
    pub fn new(instance: ActivityInstance, codec: Arc<dyn AhWireCodec>) -> Self {
        let sense = crate::sense::SenseWorker::from_env(instance.squad_epoch());
        Self {
            conv: BapConversation::with_default_deadline(),
            channel: None,
            instance,
            codec,
            blocked_pushes: Vec::new(),
            pending_pushes: Vec::new(),
            // The endpoint here only feeds message 17, which `d1-responder` answers on the primary
            // channel; this connection never hands out an address. Placeholder, not a claim.
            registry: crate::registry::HostRegistry::new(std::net::SocketAddrV4::new(
                std::net::Ipv4Addr::UNSPECIFIED,
                0,
            )),
            captured: std::collections::BTreeSet::new(),
            client_update_capture: None,
            client_authoritative_image: Vec::new(),
            sense,
            codec_faults: 0,
        }
    }

    /// Capture up to `count` additional changed udt-6/21/47 payloads, capped at [`CLIENT_UPDATE_CAPTURE_LIMIT`].
    /// The delay starts at the first received 6/21/47 payload, not process startup.
    /// Existing first-payload and every-incident captures remain unchanged.
    pub fn with_client_update_capture(mut self, count: usize, after: Duration) -> Self {
        self.client_update_capture = (count != 0).then(|| ClientUpdateCapture {
            remaining: count.min(CLIENT_UPDATE_CAPTURE_LIMIT),
            after,
            started: None,
            previous: [None, None, None],
            window_seen: [false, false, false],
        });
        self
    }

    pub fn instance(&self) -> &ActivityInstance {
        &self.instance
    }

    pub fn conversation(&self) -> &BapConversation {
        &self.conv
    }

    /// Feed one complete BAP frame (envelope **and** payload). Returns `(replies, events)`.
    ///
    /// Replies are complete frames ready for the socket; this function never writes a header
    /// itself - `d1-bap` builds every one of them.
    pub fn on_frame(&mut self, frame: &[u8], now: Instant) -> (Vec<Vec<u8>>, Vec<AhEvent>) {
        let mut events = Vec::new();
        let mut replies = Vec::new();

        let Ok(env) = d1_bap::parse_envelope(frame) else {
            events.push(AhEvent::NamedGate {
                what: "envelope parse",
                detail: format!("{} bytes did not parse as an envelope", frame.len()),
            });
            return (replies, events);
        };

        // ── the sealed path ───────────────────────────────────────────────────────────────────
        if env.mode == d1_bap::ENVELOPE_MODE_CHANNEL {
            let Some((key, nonces)) = self.channel.as_mut() else {
                events.push(AhEvent::NamedGate {
                    what: "sealed frame before the channel exists",
                    detail: "a mode-1 frame arrived before the secure hello established a key"
                        .to_string(),
                });
                return (replies, events);
            };
            let nonce = nonces.peek(Direction::ClientToServer);
            match d1_bap::open_channel_payload(key, nonce, &frame[6..]) {
                Ok(plain) => {
                    nonces.advance(Direction::ClientToServer);
                    let (mut r, mut e) = self.on_inner(&plain, now);
                    replies.append(&mut r);
                    events.append(&mut e);
                }
                Err(e) => events.push(AhEvent::NamedGate {
                    what: "sealed decrypt",
                    detail: format!("{e} (the nonce increment rule is `[U]`)"),
                }),
            }
            // Seal whatever the inner handling produced. ⭐ One sealing path, shared with
            // [`Self::on_tick`] - see [`Self::seal_all`] for what having two of them cost.
            let inner = replies.drain(..).collect::<Vec<_>>();
            let sealed = self.seal_all(inner, &mut events);
            return (sealed, events);
        }

        // ── the plaintext handshake ───────────────────────────────────────────────────────────
        let Ok(inner) = d1_bap::parse_inner(frame.get(6..).unwrap_or(&[])) else {
            events.push(AhEvent::NamedGate {
                what: "inner parse",
                detail: "the payload did not carry an inner header".to_string(),
            });
            return (replies, events);
        };

        // Fixed keys and material, matching `d1-responder`'s primary connection. A real server
        // would randomise; determinism makes two runs diff cleanly, and these bytes are
        // recognisable on sight in a capture.
        let keys = SealKeys::default();
        let material = SessionMaterial::default();

        if inner.msg_type == d1_bap::TYPE_CHANNEL_STARTUP_REQUEST {
            let reply = d1_bap::channel_startup_response(inner.sequence);
            events.push(AhEvent::Handshake { msg_type: inner.msg_type, reply_len: reply.len() });
            replies.push(reply);
        } else if inner.msg_type == d1_bap::TYPE_SECURE_HELLO_REQUEST {
            let reply = d1_bap::secure_hello_response(inner.sequence, &keys, &material, [0x5A; 16]);
            self.channel =
                Some((material.session_key, ChannelNonces::from_seed(material.channel_seed)));
            self.instance.on_connect(now);
            events.push(AhEvent::Handshake { msg_type: inner.msg_type, reply_len: reply.len() });
            events.push(AhEvent::ChannelEstablished);
            replies.push(reply);
        } else {
            events.push(AhEvent::NamedGate {
                what: "unhandled plaintext BAP type on the AH connection",
                detail: format!(
                    "type 0x{:04x} ({})",
                    inner.msg_type,
                    d1_bap::message_name(inner.msg_type).unwrap_or("unnamed")
                ),
            });
        }
        (replies, events)
    }

    /// One decrypted inner frame: header, then body.
    fn on_inner(&mut self, plain: &[u8], now: Instant) -> (Vec<Vec<u8>>, Vec<AhEvent>) {
        let mut events = Vec::new();
        let Ok(inner) = d1_bap::parse_inner(plain) else {
            events.push(AhEvent::NamedGate {
                what: "inner parse",
                detail: "decrypted payload was shorter than an inner header".to_string(),
            });
            return (Vec::new(), events);
        };

        // ⚠ The body does **not** start at +6 for every type. Message 8 is header class 3 = 14
        // bytes. Ask the table rather than hard-coding an offset that happens to be right for the
        // classes we saw first.
        let body_off = d1_bap::header_len(inner.msg_type).unwrap_or(6);
        let body = plain.get(body_off..).unwrap_or(&[]);

        // Every inbound message goes through the correlation table before any handler runs - that
        // is what makes ordering structural rather than a convention, on this connection too.
        match self.conv.enqueue(inner.sequence, inner.msg_type) {
            EnqueueOutcome::Awaiting(ticket) => {
                events.push(AhEvent::Enqueued {
                    msg_type: inner.msg_type,
                    sequence: inner.sequence,
                });
                // ⭐ BAP 6 -> 7, the activity-host STARTUP response. Answered here because message
                // 17 pointed the client's AH channel at this listener, so this is where 6 lands.
                if inner.msg_type == BAP_TYPE_AH_MANAGER_REQUEST {
                    match d1_bap::activity_host::ActivityHostManagerRequest::decode(body)
                        .map_err(|e| e.to_string())
                        .and_then(|req| {
                            self.registry
                                .answer_manager_request(&req)
                                .map_err(|e| e.to_string())
                        }) {
                        Ok((rsp, entry)) => {
                            match self.conv.park(ticket, rsp.encode()) {
                                Ok(()) => events.push(AhEvent::Handshake {
                                    msg_type: 7,
                                    reply_len: entry.session_id.len(),
                                }),
                                Err(e) => events.push(AhEvent::NamedGate {
                                    what: "park message 7",
                                    detail: e.to_string(),
                                }),
                            }
                        }
                        Err(detail) => events.push(AhEvent::NamedGate {
                            what: "message 6 -> 7",
                            detail,
                        }),
                    }
                } else {
                    // Parking nothing means `drain` will synthesise a non-200 at the deadline,
                    // which is R2 working: a wrong body is survivable, silence is not.
                    events.push(AhEvent::NamedGate {
                        what: "an AH-connection request expects a reply we do not model",
                        detail: format!(
                            "type 0x{:04x} seq {} - the FIFO will synthesise a non-200 at the deadline",
                            inner.msg_type, inner.sequence
                        ),
                    });
                }
            }
            EnqueueOutcome::FireAndForget => {}
            other => events.push(AhEvent::NamedGate {
                what: "enqueue",
                detail: format!("type 0x{:04x}: {other:?}", inner.msg_type),
            }),
        }

        if inner.msg_type == BAP_TYPE_CLIENT_TO_AH_NOTIFICATION {
            match d1_bap::activity_host::ClientToActivityHostNotification::decode(body) {
                Ok(n) => {
                    events.push(AhEvent::ClientNotification {
                        user_defined_type: n.user_defined_type,
                        payload_len: n.payload.len(),
                    });
                    events.extend(self.on_client_notification(n.user_defined_type, &n.payload, now));
                }
                Err(e) => events.push(AhEvent::NamedGate {
                    what: "message 8 decode",
                    detail: e.to_string(),
                }),
            }
        }

        // Replies = parked responses the FIFO releases, THEN any push frames. Order matters:
        // a push that jumps ahead of an outstanding reply is what produced the Weasel.
        let mut out = self.conv.drain(now);
        out.extend(self.pending_pushes.drain(..));
        (out, events)
    }

    /// The AH-protocol layer: decode the payload, run the state machine, try to push the answers.
    fn on_client_notification(&mut self, udt: u32, payload: &[u8], now: Instant) -> Vec<AhEvent> {
        let mut events = Vec::new();
        // ⭐ Deliberate non-decode, stated as such. See `AhEvent::NotDecoded`.
        events.extend(self.capture_once(udt, payload, now));
        // ⭐ Squad live counts (opt-in, D1_AH_SENSE_SCHEMAS): udt 6 is decoded off this thread and
        // the counts land on a later frame. See `crate::sense`.
        self.drain_sense();
        if udt == 6 {
            if let Some(sense) = self.sense.as_mut() { sense.submit(payload); }
            self.instance.set_sense_ready(false);
        }
        if let Some(why) = crate::codec::undecodable_reason(udt) {
            events.push(AhEvent::NotDecoded {
                user_defined_type: udt,
                name: crate::protocol::name_or_unknown(udt),
                payload_len: payload.len(),
                why,
            });
            return events;
        }
        let decoded = if udt == crate::protocol::UDT_CLIENT_AUTHORITATIVE_DATA_UPDATE {
            self.codec.decode_seeded(udt, payload, &self.client_authoritative_image)
        } else {
            self.codec.decode(udt, payload)
        };
        let image = match decoded {
            Ok(i) => i,
            Err(e) => {
                events.push(AhEvent::NamedGate { what: "AH wire codec", detail: e.to_string() });
                return events;
            }
        };
        let msg = match ClientMessage::from_struct_image(udt, &image) {
            Ok(m) => m,
            Err(e) => {
                events.push(AhEvent::NamedGate { what: "AH message", detail: e.to_string() });
                return events;
            }
        };
        if udt == crate::protocol::UDT_CLIENT_AUTHORITATIVE_DATA_UPDATE {
            self.client_authoritative_image = image;
        }
        for push in self.instance.on_client_message(&msg, now) {
            events.push(AhEvent::WantsToPush {
                user_defined_type: push.message.user_defined_type(),
            });
            events.extend(self.try_push(push));
        }
        self.drain_sense();
        events
    }

    fn drain_sense(&mut self) {
        let epoch = self.instance.squad_epoch();
        if self.sense.as_ref().is_some_and(|s| s.epoch() != epoch) {
            self.sense = crate::sense::SenseWorker::from_env(epoch);
        }
        if let Some(sense) = self.sense.as_mut() {
            for update in sense.drain() { self.instance.on_sense_update(update); }
            self.instance.set_sense_ready(sense.ready());
        } else {
            self.instance.set_sense_ready(false);
        }
    }

    /// ⭐⭐ **The one place inner frames become wire frames.** Envelope and seal, in that order.
    ///
    /// > ## ⛔⛔ THIS EXISTS BECAUSE THERE WERE TWO OF THEM, AND ONE WAS MISSING
    /// >
    /// > `d1-bap`'s `BapConversation::drain` / `push` return **inner** frames - a bare BAP header
    /// > and body - and say so: *"the caller seals each frame and advances the channel nonce;
    /// > `drain` deliberately knows nothing about the envelope or the cipher."* Until 2026-08-09
    /// > only [`Self::on_frame`] honoured that. [`Self::on_tick`] - the heartbeat's *only* path -
    /// > returned `conv.drain()` and `pending_pushes` **straight to the socket**.
    /// >
    /// > A message-9 inner frame starts `00 09 …`, so the client read the high byte of the
    /// > big-endian message id as the **envelope version**. Run
    /// > `20260809-075545-RUNG33-UNLOCK-98`, 0.52 s after the first heartbeat:
    /// >
    /// > ```text
    /// > networking:server:bap: [BAP ID 1 desc:OUT FAH] Failed to parse envelope version '0' (expected 1).
    /// > networking:server:bap: [BAP ID 1 desc:OUT FAH] Failed to call update_receiving().
    /// > networking:server:bap: [BAP ID 1 desc:OUT FAH] Fatal error '_connection_failure_unknown_message_type' raised.
    /// > world_controller:activity_manager: 'PRIVATE CURRENT' activity client lost its connection to BAP.
    /// > world_controller:state_manager: resolved instance error to change reason [status 33]…[change reason 101]…
    /// > ```
    /// >
    /// > ⇒ `error.private_activity_host_lost_connection_to_bap` - **"anteater"** - was ours.
    /// > `FUN_00B2D9F0` reads a six-byte envelope `{u8 version; u8 kind; u32be length}` and
    /// > refuses any version but 1; the `'0'` it printed is literally our own `msg_type >> 8`.
    ///
    /// Frames produced with no channel are **dropped and named**, never written: writing an
    /// unsealed frame is not a degraded send, it is a stream desynchronisation the client kills
    /// the connection over.
    fn seal_all(&mut self, inner: Vec<Vec<u8>>, events: &mut Vec<AhEvent>) -> Vec<Vec<u8>> {
        if inner.is_empty() {
            return Vec::new();
        }
        let Some((key, nonces)) = self.channel.as_mut() else {
            events.push(AhEvent::NamedGate {
                what: "inner frame with no channel",
                detail: format!(
                    "{} inner frame(s) produced before the secure hello established a key - \
                     DROPPED rather than written. A bare inner frame's byte 0 is the message id's \
                     high byte, which the client reads as the envelope version: `Failed to parse \
                     envelope version '0' (expected 1)` -> \
                     `_connection_failure_unknown_message_type` -> \
                     `error.private_activity_host_lost_connection_to_bap`",
                    inner.len()
                ),
            });
            return Vec::new();
        };
        let mut sealed = Vec::with_capacity(inner.len());
        for r in inner {
            let n = nonces.peek(Direction::ServerToClient);
            match d1_bap::seal_channel_frame(key, n, &r) {
                Ok(f) => {
                    nonces.advance(Direction::ServerToClient);
                    sealed.push(f);
                }
                Err(e) => events.push(AhEvent::NamedGate { what: "seal", detail: e }),
            }
        }
        sealed
    }

    /// ⭐⭐ The timer half of the lease. Nothing arrived; decide whether to speak anyway.
    ///
    /// The inbound path calls the same [`ActivityInstance::heartbeat_due`], so there is one
    /// mechanism with two triggers rather than two implementations that can disagree - and since
    /// 2026-08-09 it also shares [`Self::seal_all`], so there is one *framing* with two triggers
    /// too. That second half is what was missing.
    pub fn on_tick(&mut self, now: Instant) -> (Vec<Vec<u8>>, Vec<AhEvent>) {
        let mut events = Vec::new();
        self.drain_sense();
        // ⛔ Before the secure hello there is nothing to seal *with*, and an unsealed frame is
        // worse than silence - it is the connection reset that killed run
        // `20260809-075545-RUNG33-UNLOCK-98`. A host that has not finished a handshake owes no
        // heartbeat either: `ActivityInstance::on_connect` is called on the hello.
        if self.channel.is_none() {
            return (Vec::new(), events);
        }
        if let Some(push) = self.instance.heartbeat_due(now) {
            events.push(AhEvent::WantsToPush {
                user_defined_type: push.message.user_defined_type(),
            });
            events.extend(self.try_push(push));
        }
        // ⭐⭐ The udt-5 sensor frame, once the instance is Live. Armed only by `D1_AH_SENSOR_PHASE`;
        // driven from the tick as well as inbound traffic so it does not depend on the client
        // talking. Sent exactly once.
        if let Some(push) = self.instance.sensor_due(now) {
            events.push(AhEvent::WantsToPush {
                user_defined_type: push.message.user_defined_type(),
            });
            events.extend(self.try_push(push));
        }
        if let Some(push) = self.instance.membership_due(now) {
            events.push(AhEvent::WantsToPush {
                user_defined_type: push.message.user_defined_type(),
            });
            events.extend(self.try_push(push));
        }
        while let Some(push) = self.instance.incident_due() {
            events.push(AhEvent::WantsToPush {
                user_defined_type: push.message.user_defined_type(),
            });
            events.extend(self.try_push(push));
        }
        let mut out = self.conv.drain(now);
        out.extend(self.pending_pushes.drain(..));
        let sealed = self.seal_all(out, &mut events);
        (sealed, events)
    }

    /// The **first** payload seen for each `user_defined_type` on this connection, as hex.
    /// Opt-in diagnostics also record bounded 6/21/47 baselines/changes in a selected window.
    ///
    /// Once per udt, not once per message: the keepalive alone arrived ten times in the run that
    /// motivated this, and ten copies of one byte is not ten vectors. The first is the one worth
    /// keeping - it is the one sent from a known state.
    fn capture_once(&mut self, udt: u32, payload: &[u8], now: Instant) -> Vec<AhEvent> {
        // ⭐ udt 18 (incident) is the client's mission report: cinematic ended, player entered a
        // trigger. Every one is kept, not only the first - a mission is driven by them.
        if udt == 18 {
            self.captured.insert(udt);
            return vec![payload_event(udt, payload)];
        }
        let first = self.captured.insert(udt);
        let changed = self.client_update_capture.as_mut()
            .is_some_and(|capture| capture.observe(udt, payload, now, first));
        if first || changed {
            vec![payload_event(udt, payload)]
        } else {
            Vec::new()
        }
    }

    /// Attempt one push. Failures are recorded, never swallowed.
    fn try_push(&mut self, push: AhPush) -> Vec<AhEvent> {
        let udt = push.message.user_defined_type();
        match push.emit(&mut self.conv, self.codec.as_ref()) {
            // ⭐ KEEP THE BYTES. `push_with_sequence` takes `&self` and parks nothing, so `drain`
            // will never produce this frame - discarding it here built a correct join_result and
            // threw it away.
            Ok(frame) => {
                self.pending_pushes.push(frame);
                Vec::new()
            }
            Err(blocked) => {
                let detail = blocked.to_string();
                self.blocked_pushes.push((udt, detail.clone()));
                // ⭐⭐ A codec that BROKE is not a gate. See [`AhEvent::CodecFault`].
                if blocked.is_host_defect() {
                    self.codec_faults += 1;
                    return vec![AhEvent::CodecFault {
                        user_defined_type: udt,
                        detail,
                        occurrence: self.codec_faults,
                    }];
                }
                vec![AhEvent::NamedGate { what: "push blocked", detail }]
            }
        }
    }
}

/// Run the Activity Host.
///
/// Deliberately thin: accept, split frames on the envelope's own length field, hand each complete
/// frame to [`AhConnection::on_frame`], write what comes back.
/// How often the read loop wakes up when nothing is arriving: a quarter of the instance's own
/// heartbeat period, so three consecutive missed polls still leave the client's host-crash timeout
/// un-tripped. Never zero.
fn conn_heartbeat_poll(instance: &ActivityInstance) -> std::time::Duration {
    (instance.host_heartbeat() / 4).max(std::time::Duration::from_millis(100))
}

/// ⭐ One event, one log line, at the level its meaning deserves - **and the same routing on the
/// frame path and the heartbeat path.**
///
/// Before 2026-08-09 the heartbeat path printed `tracing::info!("{e:?}")` for everything, so the
/// codec fault that ended run `20260809-120451-MOONBASE-FRESH` appeared as
/// `INFO … heartbeat NamedGate { what: "push blocked", … }`. Two different bugs at two different
/// levels with one wording. See [`AhEvent::CodecFault`].
fn log_event(peer: &str, e: &AhEvent, prefix: &str) {
    match e {
        // ⛔⛔ OUR DEFECT. The run is no longer producing and the reason is on this side.
        AhEvent::CodecFault { user_defined_type, detail, occurrence } => tracing::error!(
            "[{peer}] {prefix}⛔⛔ CODEC FAULT #{occurrence} - THE ACTIVITY HOST HAS STOPPED \
             PUBLISHING udt {user_defined_type} ({}). This is OUR crash, not a protocol gate: the \
             client will see silence, time out, and report \
             error.private_activity_host_host_crashed. Everything after this line in this run is \
             TRUNCATED, not evidence. {detail}",
            crate::protocol::name_or_unknown(*user_defined_type),
        ),
        AhEvent::NamedGate { what, detail } => {
            tracing::warn!("[{peer}] {prefix}** NAMED GATE ** {what}: {detail}")
        }
        // ⭐ Not a gate. Deliberate, with the reason, at INFO - because a WARN that
        // repeats every two seconds trains a reader to stop looking at WARNs.
        AhEvent::NotDecoded { user_defined_type, name, payload_len, why } => {
            tracing::info!(
                "[{peer}] {prefix}not decoded, ON PURPOSE: udt {user_defined_type} ({name}), \
                 {payload_len} B - {why}"
            )
        }
        AhEvent::Payload { user_defined_type, hex, truncated } => tracing::info!(
            "[{peer}] {prefix}⭐ CAPTURED udt {user_defined_type} ({}) payload{}: {hex}",
            crate::protocol::name_or_unknown(*user_defined_type),
            if *truncated { " (TRUNCATED)" } else { "" }
        ),
        other => tracing::info!("[{peer}] {prefix}{other:?}"),
    }
}

/// One connection's read loop.
async fn drive(
    mut sock: tokio::net::TcpStream,
    peer: String,
    instance: ActivityInstance,
    codec: Arc<dyn AhWireCodec>,
) -> anyhow::Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    // ⭐ Poll at a quarter of the heartbeat, so three consecutive missed polls still leave the
    // client's 20,500 ms host-crash timeout un-tripped.
    let poll = conn_heartbeat_poll(&instance);
    let capture_count = std::env::var("D1_AH_CAPTURE_CLIENT_UPDATES")
        .ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
    let capture_after = std::env::var("D1_AH_CAPTURE_CLIENT_UPDATES_AFTER_S")
        .ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    let mut conn = AhConnection::new(instance, codec)
        .with_client_update_capture(capture_count, Duration::from_secs(capture_after));
    if capture_count != 0 {
        tracing::info!("AH client-update diagnostics: up to {} extra changed payloads, after {capture_after}s from first udt 21/47; private raw data, no protocol change",
            capture_count.min(CLIENT_UPDATE_CAPTURE_LIMIT));
    }
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 8192];
    tracing::info!("AH connection from {peer}");
    let mut last_tick = Instant::now();
    loop {
        let n = match tokio::time::timeout(poll, sock.read(&mut chunk)).await {
            Ok(r) => r?,
            // ⭐⭐ Nothing arrived. The client's liveness test is OUR transmit idle time, so this
            // is precisely the case that killed run 20260809-054417-RUNG33-BYPASS.
            Err(_elapsed) => {
                last_tick = Instant::now();
                let (replies, events) = conn.on_tick(last_tick);
                for e in events {
                    // ⭐⭐ The heartbeat path used to print EVERY event at INFO, including the
                    // codec fault that killed run 20260809-120451-MOONBASE-FRESH. Route it the
                    // same way as the frame path, or the loudest failure in the crate stays the
                    // quietest line in the log.
                    log_event(&peer, &e, "heartbeat ");
                }
                for r in replies {
                    sock.write_all(&r).await?;
                }
                continue;
            }
        };
        if n == 0 {
            tracing::info!("AH connection from {peer} closed by the client");
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
        // The envelope's own u32be length is the only framing there is.
        while buf.len() >= 6 {
            let size = u32::from_be_bytes([buf[2], buf[3], buf[4], buf[5]]) as usize;
            if buf.len() < 6 + size {
                break;
            }
            let frame: Vec<u8> = buf.drain(..6 + size).collect();
            let (replies, events) = conn.on_frame(&frame, Instant::now());
            for e in events {
                log_event(&peer, &e, "");
            }
            for r in replies {
                sock.write_all(&r).await?;
            }
        }
        // ⭐ The tick also runs when it is overdue under steady client traffic. Mission steps, the
        // sensor repeat and the control file all hang off it, and an armed player trigger can
        // report a dozen times a second (sense36: 3,000 incident-3403 reports in five minutes,
        // and no read ever timed out, so the control file that would disarm it was never read).
        if last_tick.elapsed() >= poll {
            last_tick = Instant::now();
            let (replies, events) = conn.on_tick(last_tick);
            for e in events {
                log_event(&peer, &e, "heartbeat ");
            }
            for r in replies {
                sock.write_all(&r).await?;
            }
        }
    }
}

/// Run the Activity Host.
///
/// Deliberately thin: accept, split frames on the envelope's own length field, hand each complete
/// frame to [`AhConnection::on_frame`], write what comes back.
pub async fn serve(
    bind: AhBind,
    instance_factory: impl Fn() -> ActivityInstance + Send + Sync + 'static,
    codec: Arc<dyn AhWireCodec>,
) -> anyhow::Result<()> {
    let factory = Arc::new(instance_factory);

    match bind {
        AhBind::Listen(addr) => {
            let listener = tokio::net::TcpListener::bind(addr).await?;
            let push_id = crate::push::push_bap_message_id();
            tracing::info!(
                "activity-host listening on {} - codec: {}",
                listener.local_addr()?,
                codec.name()
            );
            // ⭐ Provenance for every push this process makes. `9` is the id whose deliver step
            // (`0x5642E0` → `0x75CB40` → `0x75CB70`) routes into
            // `c_activity_client::receive_message_from_host`; `100`'s (`0x564C20`) only relays to
            // the connection named in its class-6 header. See `push.rs`'s banner.
            tracing::info!(
                "pushes ride BAP {push_id} ({}) - D1_AH_BAP_PUSH_ID overrides",
                if push_id == crate::push::BAP_TYPE_BAP_TO_CLIENT_NOTIFICATION {
                    "bap_to_client_notification - DELIVERS to the activity client"
                } else {
                    "activity_host_to_client_notification - ⛔ SPENT: the client RELAYS this away"
                }
            );
            loop {
                let (sock, peer) = listener.accept().await?;
                let codec = codec.clone();
                let factory = factory.clone();
                tokio::spawn(async move {
                    if let Err(e) = drive(sock, peer.to_string(), factory(), codec).await {
                        tracing::warn!("AH connection from {peer} ended: {e}");
                    }
                });
            }
        }
        AhBind::Handoff(mut rx) => {
            tracing::info!("activity-host awaiting handed-off sockets - codec: {}", codec.name());
            while let Some(sock) = rx.recv().await {
                let peer = sock.peer_addr().map(|a| a.to_string()).unwrap_or_default();
                let codec = codec.clone();
                let factory = factory.clone();
                tokio::spawn(async move {
                    if let Err(e) = drive(sock, peer.clone(), factory(), codec).await {
                        tracing::warn!("handed-off AH connection {peer} ended: {e}");
                    }
                });
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::{RawStructCodec, SchemaCodec};

    fn conn(codec: Arc<dyn AhWireCodec>) -> AhConnection {
        AhConnection::new(ActivityInstance::new(0xD1A0_0001, "d1ah-test", Instant::now()), codec)
    }

    #[test]
    fn client_update_capture_is_opt_in_and_preserves_incidents() {
        let now = Instant::now();
        let mut c = conn(Arc::new(RawStructCodec));
        for udt in [21, 47, 37] {
            assert_eq!(c.capture_once(udt, &[1], now).len(), 1);
            assert!(c.capture_once(udt, &[2], now).is_empty());
        }
        assert_eq!(c.capture_once(18, &[1], now).len(), 1);
        assert_eq!(c.capture_once(18, &[1], now).len(), 1);
        assert!(c.pending_pushes.is_empty());
    }

    #[test]
    fn client_update_capture_tracks_both_types_without_repeating_unchanged_data() {
        let now = Instant::now();
        let mut c = conn(Arc::new(RawStructCodec))
            .with_client_update_capture(3, Duration::ZERO);
        for udt in [21, 47] {
            assert_eq!(c.capture_once(udt, &[1], now).len(), 1);
            assert!(c.capture_once(udt, &[1], now).is_empty());
            assert_eq!(c.capture_once(udt, &[2], now).len(), 1);
        }
        // An unrelated type keeps the old once-per-connection rule.
        assert_eq!(c.capture_once(37, &[1], now).len(), 1);
        assert!(c.capture_once(37, &[2], now).is_empty());
        assert_eq!(c.client_update_capture.as_ref().unwrap().remaining, 1);
        assert!(c.pending_pushes.is_empty());
    }

    #[test]
    fn client_update_capture_includes_the_sense_update() {
        let now = Instant::now();
        let mut c = conn(Arc::new(RawStructCodec))
            .with_client_update_capture(2, Duration::ZERO);
        // udt 6 (sensor_sense_update): the first, then only changed payloads, from the shared budget.
        assert_eq!(c.capture_once(6, &[0x80, 1], now).len(), 1);
        assert!(c.capture_once(6, &[0x80, 1], now).is_empty());
        assert_eq!(c.capture_once(6, &[0x80, 2], now).len(), 1);
        assert_eq!(c.capture_once(6, &[0x80, 3], now).len(), 1);
        assert!(c.capture_once(6, &[0x80, 4], now).is_empty(), "the budget of 2 is spent");
        assert!(c.pending_pushes.is_empty());
    }

    #[test]
    fn client_update_capture_waits_for_the_requested_window() {
        let now = Instant::now();
        let mut c = conn(Arc::new(RawStructCodec))
            .with_client_update_capture(2, Duration::from_secs(30));
        assert_eq!(c.capture_once(21, &[1], now).len(), 1);
        assert!(c.capture_once(21, &[2], now + Duration::from_secs(29)).is_empty());
        assert_eq!(c.capture_once(21, &[2], now + Duration::from_secs(30)).len(), 1);
        assert!(c.capture_once(21, &[2], now + Duration::from_secs(30)).is_empty());
        assert_eq!(c.capture_once(21, &[3], now + Duration::from_secs(30)).len(), 1);
        assert_eq!(c.client_update_capture.as_ref().unwrap().remaining, 0);
    }

    #[test]
    fn client_update_capture_budget_is_shared_and_hard_capped() {
        let now = Instant::now();
        let mut c = conn(Arc::new(RawStructCodec))
            .with_client_update_capture(usize::MAX, Duration::ZERO);
        assert_eq!(c.client_update_capture.as_ref().unwrap().remaining, CLIENT_UPDATE_CAPTURE_LIMIT);
        c.capture_once(21, &[0], now);
        c.capture_once(47, &[0], now);
        for n in 1..=CLIENT_UPDATE_CAPTURE_LIMIT {
            let udt = if n % 2 == 0 { 21 } else { 47 };
            assert_eq!(c.capture_once(udt, &n.to_le_bytes(), now).len(), 1);
        }
        assert!(c.capture_once(21, &[99], now).is_empty());
        assert!(c.capture_once(47, &[99], now).is_empty());
        assert_eq!(c.capture_once(18, &[1], now).len(), 1);
    }

    #[test]
    fn client_update_capture_bounds_storage_and_labels_oversized_payloads() {
        let now = Instant::now();
        let mut c = conn(Arc::new(RawStructCodec))
            .with_client_update_capture(2, Duration::ZERO);
        let mut payload = vec![1; PAYLOAD_CAPTURE_LIMIT + 10];
        c.capture_once(21, &payload, now);
        payload[PAYLOAD_CAPTURE_LIMIT + 1] = 2;
        let event = c.capture_once(21, &payload, now);
        assert!(matches!(&event[0], AhEvent::Payload { hex, truncated: true, .. }
            if hex.len() == 2 * PAYLOAD_CAPTURE_LIMIT));
        assert!(c.client_update_capture.as_ref().unwrap().previous[0].is_none());
    }

    #[test]
    fn client_update_capture_zero_disables_the_extra_diagnostics() {
        let c = conn(Arc::new(RawStructCodec))
            .with_client_update_capture(0, Duration::ZERO);
        assert!(c.client_update_capture.is_none());
    }

    #[test]
    fn client_update_capture_adds_no_replies_on_the_notification_path() {
        let now = Instant::now();
        let mut c = conn(Arc::new(RawStructCodec))
            .with_client_update_capture(4, Duration::ZERO);
        for udt in [21, 47] {
            for value in [0, 1] {
                let events = c.on_client_notification(udt, &[value; 0x220], now);
                assert!(events.iter().any(|e| matches!(e, AhEvent::Payload { .. })));
                assert!(!events.iter().any(|e| matches!(e,
                    AhEvent::WantsToPush { .. } | AhEvent::Enqueued { .. })));
                assert!(c.pending_pushes.is_empty());
            }
        }
    }

    /// Wrap an inner frame in a plaintext envelope.
    fn plain_envelope(inner: &[u8]) -> Vec<u8> {
        let mut f = vec![d1_bap::ENVELOPE_VERSION, d1_bap::ENVELOPE_MODE_SECURE];
        f.extend_from_slice(&(inner.len() as u32).to_be_bytes());
        f.extend_from_slice(inner);
        f
    }

    fn inner_header(msg_type: u16, sequence: u32) -> Vec<u8> {
        let mut h = msg_type.to_be_bytes().to_vec();
        h.extend_from_slice(&sequence.to_be_bytes());
        h
    }

    /// The AH connection is a BAP endpoint, so it must complete the same handshake the primary
    /// does. Both halves, with the channel latched on the secure hello and not before.
    #[test]
    fn the_ah_connection_answers_the_two_plaintext_handshake_messages() {
        let mut c = conn(Arc::new(SchemaCodec));
        let now = Instant::now();

        let (r, e) = c.on_frame(&plain_envelope(&inner_header(d1_bap::TYPE_CHANNEL_STARTUP_REQUEST, 1)), now);
        assert_eq!(r.len(), 1);
        // ⚠ `d1-bap` hands back a COMPLETE frame - envelope and all - not a bare inner payload.
        // The listener therefore writes it verbatim and never touches an envelope byte.
        assert_eq!(&r[0][0..2], &[d1_bap::ENVELOPE_VERSION, d1_bap::ENVELOPE_MODE_SECURE]);
        assert_eq!(&r[0][6..8], &d1_bap::TYPE_CHANNEL_STARTUP_RESPONSE.to_be_bytes());
        assert!(matches!(e[0], AhEvent::Handshake { .. }));
        assert!(c.channel.is_none(), "startup alone does not establish a channel");

        let (r, e) = c.on_frame(&plain_envelope(&inner_header(d1_bap::TYPE_SECURE_HELLO_REQUEST, 2)), now);
        assert_eq!(r.len(), 1);
        assert!(e.contains(&AhEvent::ChannelEstablished));
        assert!(c.channel.is_some());
    }

    /// ⭐ A sealed frame before the hello is a named gate, not a panic and not a silent drop -
    /// this is the shape that made "the game never got there" unfalsifiable in past sessions.
    #[test]
    fn a_sealed_frame_before_the_channel_exists_is_a_named_gate() {
        let mut c = conn(Arc::new(SchemaCodec));
        let mut f = vec![d1_bap::ENVELOPE_VERSION, d1_bap::ENVELOPE_MODE_CHANNEL];
        f.extend_from_slice(&4u32.to_be_bytes());
        f.extend_from_slice(&[0; 4]);
        let (r, e) = c.on_frame(&f, Instant::now());
        assert!(r.is_empty());
        assert!(matches!(e[0], AhEvent::NamedGate { .. }));
    }

    /// ⚠ Message 8 is header class 3 = **14** bytes, not 6. Every request this project answered
    /// before was class 1, so a hard-coded `&plain[6..]` was right by accident.
    #[test]
    fn message_eight_body_starts_at_fourteen_not_six() {
        assert_eq!(d1_bap::header_len(8), Some(14));
        // Positive control on the same call: the classes we already ship are unchanged.
        assert_eq!(d1_bap::header_len(123), Some(6));
        assert_eq!(d1_bap::header_len(0x00FB), Some(8));
    }

    /// ⭐⭐ **INVERTED 2026-08-08 - and this is the one that matters.**
    ///
    /// It used to assert that a join request *"gets as far as the codec gate and names it"*. It now
    /// drives **the real 238 bytes the live client sent on 2026-08-08** through the whole path and
    /// asserts that the gate is gone: the payload decodes, the state machine runs, both pushes are
    /// framed, and **nothing is blocked**.
    ///
    /// It still discriminates hard. The payload is 238 bytes, not a round number - a codec that
    /// silently accepted anything would still have to produce the captured nonce back in the
    /// `join_result` it frames, and the assertion below reads it out of the outgoing body.
    #[test]
    fn the_captured_live_join_request_decodes_and_both_pushes_are_framed() {
        let mut c = conn(Arc::new(SchemaCodec));
        let now = Instant::now();

        let wire: Vec<u8> = {
            let s = d1_oracle::activity_host::CAPTURED_JOIN_REQUEST;
            (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
        };
        assert_eq!(wire.len(), 238);

        // Build message 8's body with d1-bap's own encoder, so the test cannot drift from it.
        let body = d1_bap::activity_host::ClientToActivityHostNotification {
            inner_type: 1,
            user_defined_type: crate::protocol::UDT_JOIN_REQUEST,
            aux: Some(0),
            payload: wire,
        }
        .encode();
        let mut plain = vec![0u8; 14];
        plain[0..2].copy_from_slice(&8u16.to_be_bytes());
        plain[2..6].copy_from_slice(&7u32.to_be_bytes());
        plain.extend_from_slice(&body);

        let (replies, events) = c.on_inner(&plain, now);
        assert!(events.iter().any(|e| matches!(
            e,
            AhEvent::ClientNotification { user_defined_type: 3, payload_len: 238 }
        )));
        assert!(
            !events.iter().any(|e| matches!(e, AhEvent::NamedGate { .. })),
            "no gate may be named on this path any more: {events:?}"
        );
        // ⭐⭐ ALL FOUR. This asserted two until 2026-08-09, and two is exactly what run
        // `20260808-234535-CODEC-LIVE2` put on the wire before the client timed out at
        // `EST-N AH->0 MEM-0`. `FUN_00298EF0` waits on udt 4, 0, 1 AND 11.
        for udt in [4u32, 0, 1, 11] {
            assert!(
                events.iter().any(|e| matches!(e, AhEvent::WantsToPush { user_defined_type } if *user_defined_type == udt)),
                "udt {udt} must be pushed: {events:?}"
            );
        }
        assert!(c.blocked_pushes.is_empty(), "{:?}", c.blocked_pushes);
        assert_eq!(replies.len(), 4, "all four of establish-client's messages are framed");

        // ⭐ The frame must be message 9, not message 100 - and the offset is DERIVED from the id
        // the frame actually carries, so a regression to 100 fails on the id rather than silently
        // reading the payload from the wrong offset.
        let framed_id = u16::from_be_bytes(replies[0][0..2].try_into().unwrap());
        assert_eq!(
            framed_id,
            crate::push::BAP_TYPE_BAP_TO_CLIENT_NOTIFICATION,
            "pushes must ride BAP 9: BAP 100's deliver step (0x564C20) only relays"
        );
        let hdr = d1_bap::header_len(framed_id).expect("the id must have a header class");
        assert_eq!(hdr, 6, "message 9 is header class 0");

        // ⭐ The captured nonce must come back out in the framed join_result. Decode our own frame
        // with the client's own codec rather than trusting the bytes we just built.
        let payload = &replies[0][hdr + 0x11..];
        let back = SchemaCodec.decode(3, payload).expect("our join_result must decode");
        assert_eq!(
            u32::from_le_bytes(back[0..4].try_into().unwrap()),
            d1_oracle::activity_host::CAPTURED_NONCE,
            "the join_result must echo the captured nonce at +0x00"
        );
    }

    /// ⭐ **INVERTED 2026-08-08.** The same path with the **experiment** codec used to run the
    /// state machine and then stop on the class-6 header. `d1-bap` writes class 6 now, so both
    /// pushes are framed and **nothing is blocked** - that is the assertion.
    ///
    /// It still discriminates: it pins the two `WantsToPush` udts and the resulting instance
    /// state, so a regression that framed nothing (or framed the wrong message) fails here.
    #[test]
    fn with_the_experimental_codec_the_state_machine_runs_and_both_pushes_are_framed() {
        let mut c = conn(Arc::new(RawStructCodec));
        let now = Instant::now();
        let body = d1_bap::activity_host::ClientToActivityHostNotification {
            inner_type: 1,
            user_defined_type: crate::protocol::UDT_JOIN_REQUEST,
            aux: Some(0),
            payload: {
                let mut p = vec![0u8; 0x100];
                p[..4].copy_from_slice(&[9, 8, 7, 6]);
                p
            },
        }
        .encode();
        let mut plain = vec![0u8; 14];
        plain[0..2].copy_from_slice(&8u16.to_be_bytes());
        plain[2..6].copy_from_slice(&1u32.to_be_bytes());
        plain.extend_from_slice(&body);

        let (_r, events) = c.on_inner(&plain, now);
        assert!(events.iter().any(|e| matches!(e, AhEvent::WantsToPush { user_defined_type: 4 })));
        assert!(events.iter().any(|e| matches!(e, AhEvent::WantsToPush { user_defined_type: 1 })));
        assert_eq!(
            c.blocked_pushes.len(),
            0,
            "d1-bap writes header class 6 now, so nothing may be blocked: {:?}",
            c.blocked_pushes
        );
        assert!(
            !events.iter().any(|e| matches!(e, AhEvent::NamedGate { what: "push blocked", .. })),
            "no push may be reported blocked: {events:?}"
        );
        assert_eq!(
            c.instance().state(),
            crate::state::InstanceState::Live,
            "all four of FUN_00298EF0's messages went out, so the instance is Live"
        );
    }

    /// Message 8 is fire-and-forget, so nothing is enqueued for it and nothing may be replied to
    /// it. Positive control on the same policy table: message 16 does expect a reply.
    #[test]
    fn message_eight_creates_no_in_flight_entry() {
        assert_eq!(
            d1_bap::response_for(8),
            d1_bap::tables::ResponsePolicy::FireAndForget
        );
        assert_eq!(d1_bap::response_for(16), d1_bap::tables::ResponsePolicy::Expects(17));
    }
}
