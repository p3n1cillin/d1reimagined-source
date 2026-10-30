// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! BAP (Bungie Application Protocol) wire codec.
//!
//! **This crate is the single home for BAP/Demonware framing**, per `CLAUDE.md` → *one protocol, one
//! implementer*: two sessions implementing parts of one protocol produce a silent model mismatch that
//! surfaces months later as mysterious behaviour rather than a compile error.
//!
//! # Reach and provenance
//!
//! The layout below is **`static-client-inference`** - read out of an eboot we own, cross-read by two
//! independent analyses. Nothing here has been observed against a real Bungie service, and the client
//! advancing on bytes we generate does **not** promote it to a contract. Everything downstream is
//! **`synthetic_ancestor`**, and this touches authentication, so per `CLAUDE.md` it needs
//! unconditional confirmation against capture or hardware before it is treated as the real protocol.
//!
//! # Why this wall is crossable when the others were not
//!
//! `sku_config.txt` and the CDN content manifest are sealed with **RSA-2048** signatures whose private
//! keys are Bungie's - unforgeable, full stop. This handshake is different: it is keyed by two 16-byte
//! blobs that **the server chooses** and hands to the client in the `/SignOn` response (`f2.2`, `f2.3`).
//! We are the server. The keys are ours, so the MAC is reproducible rather than a barrier.
//!
//! # Frame layout
//!
//! ```text
//! outer envelope (6 bytes)          parsed at eboot vaddr 0xb2da3e..0xb2db2b
//!   u8    version        // must equal 1
//!   u8    envelope_mode  // 0, 1 or 2 accepted; the client hello uses 2
//!   u32be payload_size   // big-endian (the client uses `movbe`)
//!
//! inner header (6 bytes)
//!   u16be type           // see the exchange order below
//!   u32be sequence
//!
//! sealed body (secure-hello response)
//!   u32be sealed_length  // ciphertext_len + 48  (16 IV + 32 MAC)
//!   u8    iv[16]
//!   u8    ciphertext[C]  // AES-128-CBC, PKCS#7 padding
//!   u8    hmac[32]       // HMAC-SHA256 over  sealed_length_be || iv || ciphertext
//! ```
//!
//! # The exchange order - TWO exchanges, not one
//!
//! Getting this wrong cost two runs. The `DESTINY` packet the client opens with is **not** a secure
//! hello; it is a channel-startup request, and the sealed AES/HMAC response belongs to a *later*
//! exchange that the client itself initiates.
//!
//! ```text
//! _channel_starting:
//!     0x001e  c->b start req    (client -> us)   the 140-byte "DESTINY" packet
//!     0x001f  c->b start rsp    (us -> client)
//!
//! _authenticating:
//!     0x0019  b->c shello req   (client -> us)
//!     0x001a  b->c shello rsp   (us -> client)   the 96-byte sealed frame
//! ```
//!
//! **Dispatch is the client's own in-flight-request queue**, not a registry of handlers
//! (`update_receiving_header`, `0xb2e0dc`-`0xb2e145`). It inspects only the HEAD entry of each queue
//! and compares **expected-reply-type first** (`0xb2e121`), sequence second (`0xb2e12a`). The
//! generator at `0xb316c0` creates the startup request as `{request 0x001e, class 5, expected reply
//! 0x001f}`.
//!
//! Two consequences worth keeping:
//!
//! * Answering `0x001a` during `_channel_starting` fails the **type** test and jumps away *before the
//!   sequence compare runs*. That is why replying with sequence 0 and sequence 1 produced
//!   byte-identical client output - the sequence was never examined, so neither value was tested.
//!   `[C]` by experiment: `failed to find recipient` went 1, 2 → 0 the moment the type became `0x001f`.
//! * The four bytes really are a **sequence**, explicitly compared. An earlier "hidden handle id"
//!   hypothesis of mine is ruled out.
//!
//! Envelope mode is **not** consulted by the recipient lookup; `conn[0x16d7]` is read once at
//! `0xb2dbc8` only to choose the next read state, so mode 0 and mode 2 behave alike.
//!
//! Secure-hello codec pair in the client: decoder `0xb32fb0`, encoder `0xb330c0`; post-decode channel
//! setup `0xb32d50 → 0x6cf80 → 0x6cc60`. Primitives are **libtomcrypt**, identified byte-for-byte
//! from its descriptor structs.
//!
//! Decrypted secure-hello plaintext, as consumed immediately after decode:
//!
//! ```text
//!   [0..12]   channel counter / nonce state
//!   [12..28]  16-byte session cipher key
//! ```
//!
//! # Determinism
//!
//! Nothing here draws randomness. The IV and the session material are **inputs**, so a run is
//! byte-reproducible and two runs can be diffed meaningfully - the same discipline the rest of the
//! harness follows. A real server would use a fresh random IV per message; that is a deliberate
//! difference and is recorded rather than hidden.

use aes::cipher::{block_padding::Pkcs7, BlockEncryptMut, BlockDecryptMut, KeyIvInit};
use hmac::{Hmac, Mac};
use sha2::Sha256;

type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;
type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;
type HmacSha256 = Hmac<Sha256>;

// ─── the activity-host layer, added 2026-08-08 ────────────────────────────────────────────────
//
// ⭐⭐ **`conversation` is the Weasel fix and it is the most important thing in this crate.** The
// rest of this file builds individual replies; `conversation` decides *whether* and *in what order*
// any of them may leave. `0xB2E940` matches only the HEAD of each of the client's two in-flight
// rings, so a correct reply sent at the wrong moment kills the connection exactly as a wrong one
// would: `_connection_failure_failed_to_find_message_recipient`, on screen **Weasel**.
//
// New code should build replies through [`conversation::BapConversation`] rather than calling
// [`reply_header`] directly. `reply_header` stays public because `d1-responder` uses it at three
// sites today and breaking it would put working, live-confirmed code at risk mid-refactor - but it
// hard-codes the 8-byte class-2 shape, which is right for every reply we send **today** and wrong
// the moment anything class-3/5/6 is sent. `conversation` asks [`tables::HEADER_LEN`] instead.

/// ⭐⭐ The correlation table - one per BAP TCP connection. **Read this module before answering
/// anything.**
pub mod conversation;
/// The generated tables: header classes, the request→response policy, and the push accept-set.
pub mod tables;
/// ⭐ **The generator for [`tables`]** - re-derives every const table from the pinned eboot
/// (port of `capture-rig/bap-message-idspace.py` and `capture-rig/bap-message-table.py`), so the
/// id space is described once and drift between the consts and the binary is a red test.
pub mod derive;

/// Messages 6/7, 8, 16/17, 100 and 102 - the activity-host family.
pub mod activity_host;
/// ⭐ Investment messages 501/502/504/505 - create / delete / login / signout character. The
/// client→server command direction (`THE-BLOCKS.md` B11). See `historicaldocs/design/character-lifecycle-messages.md`.
pub mod character_lifecycle;
/// 601 `loot_pickup_item`: the world-drop claim and its reply.
pub mod loot;
/// Messages 42/43 - matchmaking configuration.
pub mod matchmaking;
/// Session-parameter payloads. **Reference only - the server can never set one.**
pub mod session_params;

pub use conversation::{
    BapConversation, EnqueueOutcome, HeaderExtras, ParkError, PushError, ResponseTicket,
    DEFAULT_DEADLINE, STATUS_SYNTHESISED,
};
pub use tables::{
    header_carries_status, header_class, header_len, is_ours_to_push, is_pushable, message_name,
    policy_for, response_for, BapPolicy, PolicySource, ResponsePolicy, HEADER_FLAGS, HEADER_LEN,
    MESSAGE_CLASS, OURS_TO_PUSH, POLICIES, PUSH_ACCEPT_SET,
};

/// Envelope version the client requires. Anything else is rejected outright.
pub const ENVELOPE_VERSION: u8 = 1;
/// Envelope mode used by the client hello. The parser accepts 0, 1 or 2.
pub const ENVELOPE_MODE_SECURE: u8 = 2;
/// `c->b start req` - the client's channel-startup request. The 140-byte `DESTINY` packet.
///
/// Deliberately **not** called a "hello": it was mistaken for the secure hello once already, and the
/// two belong to different exchanges.
pub const TYPE_CHANNEL_STARTUP_REQUEST: u16 = 0x001e;
/// `c->b start rsp` - what the client actually waits for in `_channel_starting`.
///
/// ⚠ **Not** the secure-hello response. Dispatch works off the client's own in-flight-request queue
/// (`update_receiving_header`, `0xb2e0dc`-`0xb2e145`): it inspects only the HEAD entry, compares the
/// expected reply type first and the sequence second. A `0x001a` fails the type test and jumps away
/// *before the sequence compare executes* - which is why replying with sequence 0 and sequence 1
/// produced byte-identical client output. A `0x001a` recipient cannot exist until the client has
/// sent the login request in `_authenticating`.
pub const TYPE_CHANNEL_STARTUP_RESPONSE: u16 = 0x001f;
/// `b->c shello req` - the client's secure-hello request, sent in `_authenticating`.
pub const TYPE_SECURE_HELLO_REQUEST: u16 = 0x0019;
/// `b->c shello rsp` - our sealed reply to `0x0019`. **Belongs to `_authenticating`, not
/// `_channel_starting`.**
pub const TYPE_SECURE_HELLO_RESPONSE: u16 = 0x001a;
/// IV(16) + MAC(32) - the fixed overhead `sealed_length` adds to the ciphertext.
pub const SEAL_OVERHEAD: usize = 48;

/// ⚠ **Inner headers are NOT all 6 bytes.** Eight header classes exist (6/6/**8**/14/6/24/32/8),
/// resolved from the singleton table at `0x19234a0`. Getting this wrong does not produce an error -
/// it produces **silence**, which is far worse to debug.
///
/// * type `0x001e` (client's startup request) uses class 1 → **6 bytes**. Cross-checked against the
///   captured 140-byte frame.
/// * types `0x001f` and `0x001a` (both of *our* replies) use class 2 → **8 bytes** = the 6-byte base
///   plus a `u16be status`, byteswapped in place by `rol word[dst+6],8`.
///
/// Sending a 6-byte header where 8 is expected parks the client in its resumable header-read state
/// (`0xb2ed40`) waiting for 2 more bytes. That path has **no timeout, no log and no error branch** -
/// it just returns "not complete" and is re-entered next tick. We burned a run on exactly this.
pub const REPLY_HEADER_LEN: usize = 8;
/// The status value both of our replies must carry. `0xb31ef9: cmp r12w, 0xc8`.
pub const STATUS_OK: u16 = 200;
/// `0x001f`'s payload length is gated to exactly this. `0x561937`.
pub const STARTUP_RESPONSE_PAYLOAD_LEN: usize = 128;

/// A parsed BAP frame header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Envelope {
    pub version: u8,
    pub mode: u8,
    pub payload_size: u32,
}

/// A parsed inner header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inner {
    pub msg_type: u16,
    pub sequence: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    Short { need: usize, got: usize },
    BadVersion(u8),
    BadMode(u8),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Short { need, got } => write!(f, "short frame: need {need}, got {got}"),
            ParseError::BadVersion(v) => write!(f, "bad envelope version {v} (must be 1)"),
            ParseError::BadMode(m) => write!(f, "bad envelope mode {m} (must be 0, 1 or 2)"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse the 6-byte outer envelope.
pub fn parse_envelope(b: &[u8]) -> Result<Envelope, ParseError> {
    if b.len() < 6 {
        return Err(ParseError::Short { need: 6, got: b.len() });
    }
    let version = b[0];
    if version != ENVELOPE_VERSION {
        return Err(ParseError::BadVersion(version));
    }
    let mode = b[1];
    if mode > 2 {
        return Err(ParseError::BadMode(mode));
    }
    Ok(Envelope {
        version,
        mode,
        payload_size: u32::from_be_bytes([b[2], b[3], b[4], b[5]]),
    })
}

/// Parse the 6-byte inner header that follows the envelope.
pub fn parse_inner(b: &[u8]) -> Result<Inner, ParseError> {
    if b.len() < 6 {
        return Err(ParseError::Short { need: 6, got: b.len() });
    }
    Ok(Inner {
        msg_type: u16::from_be_bytes([b[0], b[1]]),
        sequence: u32::from_be_bytes([b[2], b[3], b[4], b[5]]),
    })
}

/// The material a secure-hello response hands the client for the rest of the channel.
///
/// These are **our choices as the server**. Deliberately non-zero by default: if they were zeros a
/// downstream failure could be caused either by wrong values or by an all-zero key being rejected,
/// and we would not be able to tell which.
#[derive(Debug, Clone)]
pub struct SessionMaterial {
    /// `plaintext[0..12]` - channel counter / nonce state.
    pub channel_seed: [u8; 12],
    /// `plaintext[12..28]` - the session cipher key the client will use from here on.
    pub session_key: [u8; 16],
}

impl Default for SessionMaterial {
    fn default() -> Self {
        // Recognisable-on-sight constants, so these bytes are obvious in a hex dump and can never be
        // mistaken for something the client produced.
        Self {
            channel_seed: [0xD1; 12],
            session_key: *b"D1PRESERVE-SESS1",
        }
    }
}

impl SessionMaterial {
    /// The 28-byte plaintext the client's decoder consumes.
    /// The plaintext the client's decoder consumes.
    ///
    /// The 28-byte plaintext the client's decoder consumes. **Do not pad it here.**
    ///
    /// `secure_hello_response::decode_payload` (`0x561ae0`) gates on
    /// `L == srclen - 4 && L < 93` - the limit is **less-than 93**, and the buffer cap is
    /// inclusive (`0x561b97: cmp r14d, ebx; jge success`). With `L = ciphertext + 48` that admits a
    /// ciphertext of 32, i.e. a plaintext of 17..=32 - 28 fits naturally.
    ///
    /// ⚠ An earlier version resized this to exactly 32 "to satisfy a `>= 93` floor". The floor was a
    /// misread `jb`; worse, 32 is already block-aligned, so PKCS#7 appended a **whole extra block**,
    /// yielding a 48-byte ciphertext and `L = 96`, which the client rejected on the wire with
    /// `failed to decode [bad size 100]`. Padding is the cipher's job - let 28 → 32 happen there.
    pub fn to_plaintext(&self) -> Vec<u8> {
        let mut p = Vec::with_capacity(28);
        p.extend_from_slice(&self.channel_seed);
        p.extend_from_slice(&self.session_key);
        p
    }
}

/// Keys for the sealed body, both taken from the `/SignOn` response we sent.
///
/// `aes_key` is signon `f2.2`, `hmac_key` is `f2.3` - the first and second 16-byte blobs, in that
/// order. The responder currently sends both as **zeros**, which is why this handshake is
/// reproducible at all; keep the two in sync or the MAC will not verify.
#[derive(Debug, Clone, Copy)]
pub struct SealKeys {
    pub aes_key: [u8; 16],
    pub hmac_key: [u8; 16],
}

impl Default for SealKeys {
    fn default() -> Self {
        Self { aes_key: [0u8; 16], hmac_key: [0u8; 16] }
    }
}

/// Build a complete secure-hello response frame, ready to write to the socket.
///
/// `sequence` must match the client hello's sequence.
pub fn secure_hello_response(
    sequence: u32,
    keys: &SealKeys,
    material: &SessionMaterial,
    iv: [u8; 16],
) -> Vec<u8> {
    let plaintext = material.to_plaintext();

    // AES-128-CBC with PKCS#7. 28 bytes of plaintext pads to 32.
    let ciphertext = Aes128CbcEnc::new(&keys.aes_key.into(), &iv.into())
        .encrypt_padded_vec_mut::<Pkcs7>(&plaintext);

    let sealed_length = (ciphertext.len() + SEAL_OVERHEAD) as u32;

    // HMAC-SHA256 over sealed_length_be || iv || ciphertext.
    //
    // The length is INSIDE the MAC. That matters: it is what stops a peer from re-framing a valid
    // ciphertext under a different declared length.
    let mut mac = <HmacSha256 as Mac>::new_from_slice(&keys.hmac_key).expect("hmac accepts any key length");
    mac.update(&sealed_length.to_be_bytes());
    mac.update(&iv);
    mac.update(&ciphertext);
    let tag = mac.finalize().into_bytes();

    let mut body = Vec::new();
    body.extend_from_slice(&sealed_length.to_be_bytes());
    body.extend_from_slice(&iv);
    body.extend_from_slice(&ciphertext);
    body.extend_from_slice(&tag);

    let mut payload = Vec::with_capacity(REPLY_HEADER_LEN + body.len());
    payload.extend_from_slice(&TYPE_SECURE_HELLO_RESPONSE.to_be_bytes());
    payload.extend_from_slice(&sequence.to_be_bytes());
    payload.extend_from_slice(&STATUS_OK.to_be_bytes()); // header class 2, same as 0x001f
    payload.extend_from_slice(&body);

    let mut frame = Vec::with_capacity(6 + payload.len());
    frame.push(ENVELOPE_VERSION);
    frame.push(ENVELOPE_MODE_SECURE);
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(&payload);
    frame
}

/// `c->b start rsp` - a bare 12-byte frame with **no crypto**.
///
/// `update(_channel_starting)` at `0xb2bd40` is 158 bytes and contains no cryptographic call at all,
/// so nothing here is sealed. The sequence must echo the request's.
///
/// `[U]` **Whether an empty body is sufficient is NOT proven.** The startup-response handler
/// generic-decodes at most `0x800` bytes and does not appear to retain or consume the decoded body,
/// which makes empty plausible - but on the wire the client accepts this frame at dispatch, emits no
/// complaint, and still times out in `_channel_starting`. Either the body needs fields or something
/// else must follow. Do not treat this as complete.
pub fn channel_startup_response(sequence: u32) -> Vec<u8> {
    let mut payload = Vec::with_capacity(REPLY_HEADER_LEN + STARTUP_RESPONSE_PAYLOAD_LEN);
    payload.extend_from_slice(&TYPE_CHANNEL_STARTUP_RESPONSE.to_be_bytes());
    payload.extend_from_slice(&sequence.to_be_bytes());
    payload.extend_from_slice(&STATUS_OK.to_be_bytes()); // header class 2: the extra u16be status
    // `[I]` The 128 bytes are opaque - a length gate then a `memcpy`, with no magic and no `DESTINY`
    // check. Zeros keep the number of assumptions at one (the length); if they turn out to be read,
    // that will surface as a specific complaint rather than as this silent stall.
    payload.extend_from_slice(&[0u8; STARTUP_RESPONSE_PAYLOAD_LEN]);

    let mut frame = Vec::with_capacity(6 + payload.len());
    frame.push(ENVELOPE_VERSION);
    frame.push(ENVELOPE_MODE_SECURE); // mode is not consulted for dispatch; 0 and 2 behave alike
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(&payload);
    frame
}

/// Decrypt+verify a sealed body, as the client's decoder would. Used by tests to prove the encoder
/// is self-consistent, and available for decoding anything the client seals back to us.
pub fn open_sealed(keys: &SealKeys, body: &[u8]) -> Result<Vec<u8>, String> {
    if body.len() < 4 + SEAL_OVERHEAD {
        return Err(format!("sealed body too short: {}", body.len()));
    }
    let declared = u32::from_be_bytes([body[0], body[1], body[2], body[3]]) as usize;
    let rest = &body[4..];
    if declared != rest.len() {
        return Err(format!("sealed_length {declared} != actual {}", rest.len()));
    }
    let iv: [u8; 16] = rest[..16].try_into().map_err(|_| "bad iv".to_string())?;
    let ct = &rest[16..rest.len() - 32];
    let tag = &rest[rest.len() - 32..];

    let mut mac = <HmacSha256 as Mac>::new_from_slice(&keys.hmac_key).expect("hmac accepts any key length");
    mac.update(&(declared as u32).to_be_bytes());
    mac.update(&iv);
    mac.update(ct);
    mac.verify_slice(tag).map_err(|_| "HMAC mismatch".to_string())?;

    Aes128CbcDec::new(&keys.aes_key.into(), &iv.into())
        .decrypt_padded_vec_mut::<Pkcs7>(ct)
        .map_err(|e| format!("decrypt failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact bytes the client sent us, from `runs/20260806-152657-bap-hold/`. Parsing real
    /// captured bytes is the only thing that keeps this codec honest.
    const CAPTURED_STARTUP_REQUEST_PREFIX: [u8; 20] = [
        0x01, 0x02, 0x00, 0x00, 0x00, 0x86, // envelope: v1, mode 2, 134 bytes
        0x00, 0x1e, 0x00, 0x00, 0x00, 0x00, // inner: type 0x001e (c->b start req), sequence 0
        0x44, 0x45, 0x53, 0x54, 0x49, 0x4e, 0x59, 0x01, // "DESTINY" 01
    ];

    #[test]
    fn parses_the_real_captured_channel_startup_request() {
        let e = parse_envelope(&CAPTURED_STARTUP_REQUEST_PREFIX).expect("envelope parses");
        assert_eq!(e.version, 1);
        assert_eq!(e.mode, ENVELOPE_MODE_SECURE);
        assert_eq!(e.payload_size, 134, "0x86 big-endian");
        // The captured frame was 140 bytes on the wire: 6 envelope + 134 payload.
        assert_eq!(6 + e.payload_size as usize, 140);

        let i = parse_inner(&CAPTURED_STARTUP_REQUEST_PREFIX[6..]).expect("inner parses");
        assert_eq!(i.msg_type, TYPE_CHANNEL_STARTUP_REQUEST);
        assert_eq!(i.sequence, 0);
        assert_eq!(&CAPTURED_STARTUP_REQUEST_PREFIX[12..19], b"DESTINY");
    }

    #[test]
    fn rejects_wrong_version_and_mode() {
        assert_eq!(parse_envelope(&[9, 2, 0, 0, 0, 0]), Err(ParseError::BadVersion(9)));
        assert_eq!(parse_envelope(&[1, 3, 0, 0, 0, 0]), Err(ParseError::BadMode(3)));
        assert!(matches!(parse_envelope(&[1, 2, 0]), Err(ParseError::Short { need: 6, got: 3 })));
    }

    /// The decoder gate is `L == srclen - 4 && L < 93` (`0x561ae0`), with an inclusive buffer cap.
    ///
    /// Regression: we once shipped `L = 96` and the client answered
    /// `failed to decode [bad size 100]` - note it prints the **header-stripped body** size (100),
    /// not the envelope payload (108). Both numbers are asserted here so a future reader can tell
    /// which quantity the client is talking about.
    #[test]
    fn secure_hello_response_satisfies_the_decoder_size_constraints() {
        let f = secure_hello_response(0, &SealKeys::default(), &SessionMaterial::default(), [0xAB; 16]);
        assert_eq!(f.len(), 98, "whole frame: 6 envelope + 8 header + 84 body");
        assert_eq!(&f[6..14], &[0x00, 0x1A, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC8]);
        let payload_size = parse_envelope(&f).unwrap().payload_size as usize;
        assert_eq!(payload_size, 92, "envelope payload = 8 header + 84 body");
        let body_len = payload_size - REPLY_HEADER_LEN; // what the client calls the size
        assert_eq!(body_len, 84);
        let sealed_len = u32::from_be_bytes([f[14], f[15], f[16], f[17]]) as usize;
        assert_eq!(sealed_len, body_len - 4, "L == srclen - 4");
        assert!(sealed_len < 93, "L must be BELOW 93 - the floor was a misread `jb`");
        assert_eq!(sealed_len, 80, "32 ciphertext + 48 overhead");
    }

    #[test]
    fn sealed_body_round_trips_and_carries_the_session_material() {
        let keys = SealKeys::default();
        let mat = SessionMaterial::default();
        let f = secure_hello_response(7, &keys, &mat, [0x5A; 16]);
        let plain = open_sealed(&keys, &f[14..]).expect("round trip");
        assert_eq!(plain.len(), 28, "12-byte channel seed + 16-byte session key, unpadded");
        assert_eq!(&plain[..12], &mat.channel_seed);
        assert_eq!(&plain[12..28], &mat.session_key);
        assert_eq!(u32::from_be_bytes([f[8], f[9], f[10], f[11]]), 7, "sequence is echoed");
    }

    /// The declared length is inside the MAC, so re-framing a valid ciphertext must fail.
    #[test]
    fn tampering_is_detected() {
        let keys = SealKeys::default();
        let f = secure_hello_response(0, &keys, &SessionMaterial::default(), [1u8; 16]);
        let mut body = f[14..].to_vec();
        let last = body.len() - 1;
        body[last] ^= 0xFF; // flip a MAC bit
        assert!(open_sealed(&keys, &body).is_err(), "MAC must reject a flipped tag");

        let mut body2 = f[14..].to_vec();
        body2[20] ^= 0xFF; // flip a ciphertext bit
        assert!(open_sealed(&keys, &body2).is_err(), "MAC must cover the ciphertext");
    }

    /// A wrong HMAC key must fail - proves the MAC actually depends on signon `f2.3`.
    #[test]
    fn mac_depends_on_the_signon_blob() {
        let good = SealKeys::default();
        let bad = SealKeys { aes_key: [0u8; 16], hmac_key: [0x77; 16] };
        let f = secure_hello_response(0, &good, &SessionMaterial::default(), [2u8; 16]);
        assert!(open_sealed(&bad, &f[14..]).is_err());
        assert!(open_sealed(&good, &f[14..]).is_ok());
    }

    /// The regression this guards: a 6-byte header made the client stall **silently** in its
    /// resumable header read, with no timeout and no log line. Reply headers are 8 bytes.
    #[test]
    fn channel_startup_response_has_an_eight_byte_header_and_128_byte_payload() {
        let f = channel_startup_response(0);
        assert_eq!(f.len(), 6 + 8 + 128, "envelope + class-2 header + payload");
        assert_eq!(&f[..6], &[0x01, 0x02, 0x00, 0x00, 0x00, 0x88], "payload_size 0x88 = 136");
        assert_eq!(&f[6..14], &[0x00, 0x1F, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC8],
                   "type, sequence, then u16be status 200");
        let e = parse_envelope(&f).unwrap();
        assert_eq!(6 + e.payload_size as usize, f.len(), "declared length matches reality");
    }

    /// The sequence must echo the request's - dispatch compares it after the type.
    #[test]
    fn channel_startup_response_echoes_the_sequence() {
        let f = channel_startup_response(42);
        assert_eq!(parse_inner(&f[6..]).unwrap().sequence, 42);
        assert_eq!(u16::from_be_bytes([f[12], f[13]]), STATUS_OK);
    }

    /// Byte-for-byte reproducible: no randomness anywhere in the encoder.
    #[test]
    fn output_is_deterministic() {
        let a = secure_hello_response(3, &SealKeys::default(), &SessionMaterial::default(), [9u8; 16]);
        let b = secure_hello_response(3, &SealKeys::default(), &SessionMaterial::default(), [9u8; 16]);
        assert_eq!(a, b);
    }
}

// ─── Mode 1: the secured channel ──────────────────────────────────────────────────────────────────

// `KeyInit` is aliased: `aes::cipher::KeyIvInit` is already in scope for the CBC path and both
// crates expose a `new`, which is ambiguous at the call site otherwise.
use aes_gcm::aead::{Aead, KeyInit as GcmKeyInit, Payload};
use aes_gcm::{Aes128Gcm, Nonce};

/// Envelope mode used once the secure channel is up.
pub const ENVELOPE_MODE_CHANNEL: u8 = 1;
/// `c->b queuez_register` - first message on the secured channel. Bodyless.
///
/// ⚠ **MISNAMED UNTIL 2026-08-07.** These were called `REGISTER_SUBSCRIBER`, which is wrong and was
/// actively misleading: the client's own class name for `0x0079`/`0x007A` is **`queuez_register`**.
/// The real *subscription* request is **`0x000C`** - which is why "we already answered the
/// subscriber registration" never explained anything about subscription state. The VALUES were
/// always right; only the story was wrong. Names kept as aliases so call sites keep compiling.
/// Source: `historicaldocs/design/message-catalogue.md`, RTTI walk + vtable shape scan (two instruments, agreeing).
pub const TYPE_QUEUEZ_REGISTER_REQUEST: u16 = 0x0079;
/// `c->b queuez_register` reply. Header class 2, status 200, empty body.
pub const TYPE_QUEUEZ_REGISTER_RESPONSE: u16 = 0x007a;
/// Deprecated alias - see [`TYPE_QUEUEZ_REGISTER_REQUEST`].
pub const TYPE_REGISTER_SUBSCRIBER_REQUEST: u16 = TYPE_QUEUEZ_REGISTER_REQUEST;
/// Deprecated alias - see [`TYPE_QUEUEZ_REGISTER_RESPONSE`].
pub const TYPE_REGISTER_SUBSCRIBER_RESPONSE: u16 = TYPE_QUEUEZ_REGISTER_RESPONSE;

// ⛔ `TYPE_QUEUEZ_SUBSCRIBE: u16 = 0x000c` was DELETED 2026-08-08. It had zero call sites and the
// name was a live trap: `0x000C` is `client_to_bap_subscription_request` and has **nothing to do
// with QueueZ**. The client's QueueZ family is `0x79` (register req) / `0x7A` (register rsp) /
// `0x7B` (object) / `0x7C` (subscription lost) - see the four constants either side of this
// comment. The deleted name sat four lines from the *correct* `TYPE_QUEUEZ_REGISTER_REQUEST`, and
// it was the only one of the three names for `0x000C` with no `_RESPONSE` twin, so any call site
// using it had to reach across to `TYPE_FAMILY_SUBSCRIBE_RESPONSE` - precisely the split that gives
// one message two reply paths and produces a silent model mismatch.
// Use [`TYPE_CLIENT_TO_BAP_SUBSCRIPTION_REQUEST`].

/// `b->c queuez_to_client_subscription_lost_notification`. We have never sent or seen one.
pub const TYPE_QUEUEZ_SUBSCRIPTION_LOST: u16 = 0x007c;
/// `b->c` server challenge (a push) and `c->b` its one-way answer.
///
/// ⚠ `historicaldocs/design/bap-protocol.md` §8 previously named these "b->c log not" - that is wrong.
pub const TYPE_SERVER_CHALLENGE: u16 = 0x001c;
/// See [`TYPE_SERVER_CHALLENGE`].
pub const TYPE_SERVER_CHALLENGE_ANSWER: u16 = 0x001d;
/// `b->c echo req` - BAP keepalive. Reply `0x00fb`, header class 2, zero payload.
pub const TYPE_ECHO_REQUEST: u16 = 0x00fa;
/// `b->c echo rsp`.
pub const TYPE_ECHO_RESPONSE: u16 = 0x00fb;
// ─── the 0x000A/0x000B and 0x000C/0x000D reconciliation ────────────────────────────────────────
//
// ⭐ **The brief's ids are DECIMAL; this crate writes hex. They reconcile 1:1**, and every doc
// comment below carries **both** spellings, forever. The collision is a standing trap: `16` is
// `get_activity_host_proxy_request` in decimal and `TYPE_UPO_RESPONSE` in hex.
//
// The names below are the CLIENT'S OWN, from RTTI (control: 2,633 typeinfos, all named), with ids
// read out of vtable slot 0 and independently confirmed by the *policy* table - slot 2 (`+0x10`) is
// the request id, slot 6 (`+0x30`) the response id - which this crate re-walked into
// [`tables::POLICIES`]:
//
//   client_to_bap_subscription_request   0x1925550  mov eax,0x0C  → 12
//   client_to_bap_subscription_response  0x19255C0  mov eax,0x0D  → 13
//   client_to_bap_queuez_register_request  0x1925470  mov eax,0x79 → 121
//   queuez_to_client_update_notification   0x1926020  mov eax,0x7B → 123
//
// Our code had **not** conflated distinct messages: all three old names pointed at one real message
// and every value was right. Only the story was wrong - and one name (`TYPE_QUEUEZ_SUBSCRIBE`) was
// actively dangerous, which is why it is deleted above rather than aliased.

/// `c->ws req` - id **10** decimal / **`0x000A`** hex, `client_to_world_server_request`. Header
/// class 1 (6-byte header).
///
/// The client's own log short-string for the reply is literally `c->ws rsp`: **ws = world server**.
/// The "investment message channel" and the "world server channel" are the same message pair.
///
/// ⚠ The investment critical path is **hand-rolled bit/byte format, not protobuf** - the descriptor
/// codec is only used by peripheral messages (clan, friends, matchmaking, purchased offers). One
/// shared codec (`encode 0x261BE0`, `decode 0x261FB0`) selects the request-args or response-args
/// RSAT schema of the *same* inner index with a direction flag:
/// `16 bits index || RSAT args || 1 bit has_payload || [16 bits len || len*8]`, flushed to a byte
/// boundary. **Three** policies map onto this one wire pair; the deterministic and nondeterministic
/// variants differ **only** in a client-side flag with **no wire representation**, so the server
/// cannot distinguish them and must not try. `[C]`
pub const TYPE_CLIENT_TO_WORLD_SERVER_REQUEST: u16 = 0x000a;
/// `c->ws rsp` - id **11** decimal / **`0x000B`** hex, `client_to_world_server_response`. Header
/// class 2 (8-byte header, status 200), **echoing the same message id** in the body.
pub const TYPE_CLIENT_TO_WORLD_SERVER_RESPONSE: u16 = 0x000b;

/// `c->b sub req` - id **12** decimal / **`0x000C`** hex, `client_to_bap_subscription_request`.
/// Header class 1 (6-byte header). Body **exactly 9 bytes**: `u8 family_class || u64be root_soid`.
/// Decoder `0x561E70` rejects any other declared size
/// (`subscribe_request: failed to decode [bad size %d/9]`).
///
/// **THIS IS THE HEAD-OF-LINE MESSAGE.** Leaving it unanswered is what `error.lost_bap_connection`
/// at `bootflow:profile_setup` / `character:signin` actually was - see
/// [`client_to_bap_subscription_response`] for the mechanism and the measurement.
///
/// `observed-live`: family = 4, root = `0xD100000000000001`, three runs
/// (`historicaldocs/design/subscription-race-verdict.md`).
pub const TYPE_CLIENT_TO_BAP_SUBSCRIPTION_REQUEST: u16 = 0x000c;
/// `c->b sub rsp` - id **13** decimal / **`0x000D`** hex, `client_to_bap_subscription_response`.
/// Header class 2 (8-byte header, status 200), **zero-length body**.
///
/// The empty body is a positive statement, not an omission: its encode *and* decode vtable slots
/// are both `0x1F70` = `mov al,1 ; ret` - succeed, touch nothing. The client spells *unsupported*
/// differently, as `0x14D0` = `xor al,al ; ret`. `[C]`
pub const TYPE_CLIENT_TO_BAP_SUBSCRIPTION_RESPONSE: u16 = 0x000d;

// ── DEPRECATED ALIASES. Kept because they have live call sites in `d1-responder`. ──────────────

/// Investment server-message envelope: `u16be message_id` then a schematized bit-packed body.
#[deprecated(
    since = "0.1.0",
    note = "renamed to TYPE_CLIENT_TO_WORLD_SERVER_REQUEST - the client's own short string for the \
            reply is literally `c->ws rsp`, ws = world server. `investment message` is our word, \
            not Bungie's, and it made the world-server channel look like a second channel."
)]
pub const TYPE_INVESTMENT_MESSAGE: u16 = TYPE_CLIENT_TO_WORLD_SERVER_REQUEST;
/// Our reply to `0x000a`.
#[deprecated(
    since = "0.1.0",
    note = "renamed to TYPE_CLIENT_TO_WORLD_SERVER_RESPONSE"
)]
pub const TYPE_INVESTMENT_MESSAGE_RESPONSE: u16 = TYPE_CLIENT_TO_WORLD_SERVER_RESPONSE;
/// The family subscription request.
#[deprecated(
    since = "0.1.0",
    note = "renamed to TYPE_CLIENT_TO_BAP_SUBSCRIPTION_REQUEST - the client's own class name. \
            `family subscribe` is our word, not Bungie's; the message is not family-specific and \
            the name obscured that 0x000C is the head-of-line killer."
)]
pub const TYPE_FAMILY_SUBSCRIBE_REQUEST: u16 = TYPE_CLIENT_TO_BAP_SUBSCRIPTION_REQUEST;
/// Our reply to `0x000c`.
#[deprecated(
    since = "0.1.0",
    note = "renamed to TYPE_CLIENT_TO_BAP_SUBSCRIPTION_RESPONSE"
)]
pub const TYPE_FAMILY_SUBSCRIBE_RESPONSE: u16 = TYPE_CLIENT_TO_BAP_SUBSCRIPTION_RESPONSE;
/// `c->b upo req` - carries a JSON OAuth blob (`access_token` / `id_token`). Paired response
/// `0x0016`. Seen once, at `profile_setup`. `[U]` body grammar; we do not answer it yet.
pub const TYPE_UPO_REQUEST: u16 = 0x0015;
/// `c->b upo rsp`.
pub const TYPE_UPO_RESPONSE: u16 = 0x0016;
/// `c->b user msg req` - 6-byte header-only request. Paired response `0x0021` (`b->c user msg rsp`).
/// Seen once, at `profile_setup`. `[U]` whether a body is required.
pub const TYPE_USER_MSG_REQUEST: u16 = 0x0020;
/// `b->c user msg rsp`.
pub const TYPE_USER_MSG_RESPONSE: u16 = 0x0021;
/// `c->b client config req` - policy **15**, `client_to_bap_client_config_request`,
/// **reply required**. Sent from `setup:orbit`; its body is 8 zero bytes.
///
/// `[C]` First seen 2026-08-07 in `20260807-223920-AIDT-0018-ORBIT`, the run that first entered
/// orbit. Like `0x0017` before it, leaving it unanswered strands a queue head and the next reply
/// kills the connection.
pub const TYPE_CLIENT_CONFIG_REQUEST: u16 = 0x0012;
/// `c->b client config rsp`. Payload descriptor `0x15A7BA0` - see [`client_config_response_body`].
pub const TYPE_CLIENT_CONFIG_RESPONSE: u16 = 0x0013;

/// `c->b account id translation req` - policy **12**,
/// `client_to_bap_account_id_translation_platform_to_investment_request`, **reply required**.
///
/// `[C]` First seen 2026-08-07 in run `20260807-222725-MSG205-FAMILY5-ROOT`, at `character:prologue`
/// once the family-5 root landed. It had **never appeared in any of the project's 142 prior runs** -
/// it is unlocked by the fireteam session existing, and leaving it unanswered is fatal: the client
/// examines only the HEAD of each in-flight queue, so the next reply we send (to a *later* request)
/// finds a head expecting `0x0018`, mismatches, and raises
/// `_connection_failure_failed_to_find_message_recipient`.
pub const TYPE_ACCOUNT_ID_XLAT_REQUEST: u16 = 0x0017;
/// `c->b account id translation rsp`.
pub const TYPE_ACCOUNT_ID_XLAT_RESPONSE: u16 = 0x0018;

// The list-kind discriminators for this message already exist as `AIDT_KIND_PLATFORM_ID` and
// `AIDT_KIND_ACCOUNT_SOID`, next to the encoder. Do not add a second pair here.

/// Fire-and-forget telemetry sent from the client to Xetrov.
///
/// `[C]` The 63-entry message-class table at `0x19222e0` maps `0x00ab` to
/// `c_investment_bap_message_client_to_xetrov_request` (header class 1, encoder `0x569ef0`). There
/// is no `0x00ac` class or other paired response in that table. Despite the word `request` in the
/// class name, this is a one-way message: parsing it is useful, replying to it is not protocol.
pub const TYPE_CLIENT_TO_XETROV_REQUEST: u16 = 0x00ab;

/// Message IDs that currently have native-oracle-verified response bodies.
pub const INVESTMENT_MESSAGE_ID_206: u16 = 206;
pub const INVESTMENT_MESSAGE_ID_503: u16 = 503;
/// `login_character` - "select this existing character". Sent when the player presses cross on a
/// card at `character:signin`; body is `u16be 504 || u64be character_soid`.
pub const INVESTMENT_MESSAGE_ID_504: u16 = 504;

/// `c->b register relay client req` - 40-byte body: `u16be 0x0024 || 36 opaque || u16be 0x000d`.
pub const TYPE_REGISTER_RELAY_CLIENT_REQUEST: u16 = 0x012e;
/// `c->b register relay client rsp` - header class 2, status 200, **zero payload**. Its
/// `encode_payload`/`decode_payload` are the same stub (`0x1f70`) as `0x007a`'s.
///
/// Answering it is the last step of BAP setup: `update(_setup_primary_register_relay_client)`
/// (`0xb2bf60`) checks only `protocol_source_status == 2` and then `change_state(8 = _connected)`.
pub const TYPE_REGISTER_RELAY_CLIENT_RESPONSE: u16 = 0x012f;
/// AES-GCM tag length. The tag **precedes** the ciphertext on the wire.
pub const GCM_TAG_LEN: usize = 16;

// ─── client_to_xetrov_request (0x00ab) ──────────────────────────────────────────────────────────

/// The top-level protobuf carried by BAP type `0x00ab`.
///
/// Field names remain numeric where the client provides no semantic name. The nested values are
/// still decoded into their actual wire types rather than logged as an opaque hex blob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XetrovRequest {
    pub field1: u32,
    pub field2: u32,
    pub field3: Option<u64>,
    pub envelope: XetrovEnvelope,
}

/// Required wrapper embedded as top-level field 4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XetrovEnvelope {
    pub field1: u32,
    pub event: XetrovEvent,
}

/// Optional fields in the innermost Xetrov event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XetrovEvent {
    pub account_soid: Option<u64>,
    pub field2: Option<u64>,
    pub field3: Option<String>,
    pub field4: Option<u32>,
    pub field5: Option<String>,
    pub field6: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XetrovDecodeError {
    Truncated,
    VarintOverflow,
    InvalidTag(u64),
    UnsupportedWireType(u8),
    WrongWireType { field: u32, got: u8, expected: u8 },
    MissingField(&'static str),
    DuplicateField(u32),
    ValueOverflow { field: u32, value: u64 },
    InvalidUtf8(u32),
}

impl std::fmt::Display for XetrovDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Truncated => write!(f, "truncated protobuf value"),
            Self::VarintOverflow => write!(f, "protobuf varint overflow"),
            Self::InvalidTag(tag) => write!(f, "invalid protobuf tag {tag}"),
            Self::UnsupportedWireType(wire) => write!(f, "unsupported protobuf wire type {wire}"),
            Self::WrongWireType { field, got, expected } => {
                write!(f, "protobuf field {field} has wire type {got}, expected {expected}")
            }
            Self::MissingField(name) => write!(f, "missing required protobuf field {name}"),
            Self::DuplicateField(field) => write!(f, "duplicate singular protobuf field {field}"),
            Self::ValueOverflow { field, value } => {
                write!(f, "protobuf field {field} value {value} does not fit its C field")
            }
            Self::InvalidUtf8(field) => write!(f, "protobuf field {field} is not UTF-8"),
        }
    }
}

impl std::error::Error for XetrovDecodeError {}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ProtoValue<'a> {
    Varint(u64),
    Fixed64,
    Bytes(&'a [u8]),
    Fixed32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ProtoField<'a> {
    pub(crate) number: u32,
    pub(crate) wire: u8,
    pub(crate) value: ProtoValue<'a>,
}

pub(crate) fn read_proto_varint(input: &[u8], pos: &mut usize) -> Result<u64, XetrovDecodeError> {
    let mut value = 0u64;
    for shift in (0..=63).step_by(7) {
        let byte = *input.get(*pos).ok_or(XetrovDecodeError::Truncated)?;
        *pos += 1;
        if shift == 63 && byte > 1 {
            return Err(XetrovDecodeError::VarintOverflow);
        }
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(XetrovDecodeError::VarintOverflow)
}

pub(crate) fn parse_proto_fields(input: &[u8]) -> Result<Vec<ProtoField<'_>>, XetrovDecodeError> {
    let mut fields = Vec::new();
    let mut pos = 0usize;
    while pos < input.len() {
        let tag = read_proto_varint(input, &mut pos)?;
        let number = u32::try_from(tag >> 3).map_err(|_| XetrovDecodeError::InvalidTag(tag))?;
        let wire = (tag & 7) as u8;
        if number == 0 {
            return Err(XetrovDecodeError::InvalidTag(tag));
        }
        let value = match wire {
            0 => ProtoValue::Varint(read_proto_varint(input, &mut pos)?),
            1 => {
                pos = pos.checked_add(8).ok_or(XetrovDecodeError::Truncated)?;
                if pos > input.len() { return Err(XetrovDecodeError::Truncated); }
                ProtoValue::Fixed64
            }
            2 => {
                let len = usize::try_from(read_proto_varint(input, &mut pos)?)
                    .map_err(|_| XetrovDecodeError::Truncated)?;
                let end = pos.checked_add(len).ok_or(XetrovDecodeError::Truncated)?;
                let bytes = input.get(pos..end).ok_or(XetrovDecodeError::Truncated)?;
                pos = end;
                ProtoValue::Bytes(bytes)
            }
            5 => {
                pos = pos.checked_add(4).ok_or(XetrovDecodeError::Truncated)?;
                if pos > input.len() { return Err(XetrovDecodeError::Truncated); }
                ProtoValue::Fixed32
            }
            _ => return Err(XetrovDecodeError::UnsupportedWireType(wire)),
        };
        fields.push(ProtoField { number, wire, value });
    }
    Ok(fields)
}

pub(crate) fn optional_varint(
    fields: &[ProtoField<'_>],
    number: u32,
) -> Result<Option<u64>, XetrovDecodeError> {
    let mut out = None;
    for field in fields.iter().filter(|field| field.number == number) {
        if out.is_some() { return Err(XetrovDecodeError::DuplicateField(number)); }
        out = Some(match field.value {
            ProtoValue::Varint(value) => value,
            _ => return Err(XetrovDecodeError::WrongWireType { field: number, got: field.wire, expected: 0 }),
        });
    }
    Ok(out)
}

pub(crate) fn optional_bytes<'a>(
    fields: &[ProtoField<'a>],
    number: u32,
) -> Result<Option<&'a [u8]>, XetrovDecodeError> {
    let mut out = None;
    for field in fields.iter().filter(|field| field.number == number) {
        if out.is_some() { return Err(XetrovDecodeError::DuplicateField(number)); }
        out = Some(match field.value {
            ProtoValue::Bytes(value) => value,
            _ => return Err(XetrovDecodeError::WrongWireType { field: number, got: field.wire, expected: 2 }),
        });
    }
    Ok(out)
}

pub(crate) fn u32_field(value: u64, field: u32) -> Result<u32, XetrovDecodeError> {
    u32::try_from(value).map_err(|_| XetrovDecodeError::ValueOverflow { field, value })
}

fn optional_string(
    fields: &[ProtoField<'_>],
    number: u32,
) -> Result<Option<String>, XetrovDecodeError> {
    optional_bytes(fields, number)?
        .map(|value| {
            std::str::from_utf8(value)
                .map(str::to_owned)
                .map_err(|_| XetrovDecodeError::InvalidUtf8(number))
        })
        .transpose()
}

/// Decode the protobuf body after the six-byte BAP inner header of type `0x00ab`.
///
/// The three descriptor tables are selected directly by the client:
/// `0x15b3a50` (top level), `0x15aa7c0` (field-4 wrapper), and `0x15aa810` (event). Unknown fields
/// are skipped exactly as the client protobuf reader does; known singular fields are type-checked.
pub fn parse_xetrov_request(body: &[u8]) -> Result<XetrovRequest, XetrovDecodeError> {
    let top = parse_proto_fields(body)?;
    let field1 = optional_varint(&top, 1)?.ok_or(XetrovDecodeError::MissingField("top.1"))?;
    let field2 = optional_varint(&top, 2)?.ok_or(XetrovDecodeError::MissingField("top.2"))?;
    let envelope_bytes = optional_bytes(&top, 4)?
        .ok_or(XetrovDecodeError::MissingField("top.4"))?;

    let wrapper = parse_proto_fields(envelope_bytes)?;
    let wrapper_field1 = optional_varint(&wrapper, 1)?
        .ok_or(XetrovDecodeError::MissingField("top.4.1"))?;
    let event_bytes = optional_bytes(&wrapper, 2)?
        .ok_or(XetrovDecodeError::MissingField("top.4.2"))?;

    let event_fields = parse_proto_fields(event_bytes)?;
    let field4 = optional_varint(&event_fields, 4)?
        .map(|value| u32_field(value, 4)).transpose()?;
    let field6 = optional_varint(&event_fields, 6)?
        .map(|value| u32_field(value, 6)).transpose()?;

    Ok(XetrovRequest {
        field1: u32_field(field1, 1)?,
        field2: u32_field(field2, 2)?,
        field3: optional_varint(&top, 3)?,
        envelope: XetrovEnvelope {
            field1: u32_field(wrapper_field1, 1)?,
            event: XetrovEvent {
                account_soid: optional_varint(&event_fields, 1)?,
                field2: optional_varint(&event_fields, 2)?,
                field3: optional_string(&event_fields, 3)?,
                field4,
                field5: optional_string(&event_fields, 5)?,
                field6,
            },
        },
    })
}

/// Which way a secured frame is travelling. The two directions have **independent** nonce counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    ClientToServer,
    ServerToClient,
}

/// Nonce state for a secured BAP channel.
///
/// Read directly out of `install_channel_key` (`0xb32d50` → `0x6cf80`) - no longer inferred `[C]`:
///
/// ```c
/// ctx[0x12..0x1e] = seed[0..12];   // nonce A -- ENCRYPT path (0x6cd20)
/// ctx[0x1e..0x2a] = seed[0..12];   // nonce B -- DECRYPT path (0x6ce20)
/// if (dir) ctx[0x29] ^= 1; else ctx[0x1d] ^= 1;   // byte[11] of A or B
/// ```
///
/// The client calls it with `dir = 0` (`xor ecx,ecx` at `0xb32d54`), so **nonce A takes the flip**.
/// Nonce A is the client's *encrypt* nonce. Therefore:
///
/// * **client → server** = seed with `byte[11] ^= 1`
/// * **server → client** = the seed **verbatim**
///
/// The increment (`0x6cde0`, `0x6cf03`) is a **little-endian 96-bit counter, +1 from byte[0]**:
///
/// ```c
/// for (i = 0; i <= 0xb; i++) if (++nonce[i] != 0) break;
/// ```
///
/// and it happens **only after the GCM operation succeeds**. The direction bit lives in `byte[11]`,
/// the *top* counter byte, so it survives ~2^88 messages without any masking.
///
/// ⚠ An earlier model here had this wrong twice over: "+2, big-endian, on the last byte", with the
/// direction derived by masking the LSB rather than XOR-ing it. Masking happens to agree for a seed
/// whose `byte[11]` LSB is 1 (ours is `0xd1`) and is **backwards** for any seed where it is 0.
#[derive(Debug, Clone)]
pub struct ChannelNonces {
    c2s: [u8; 12],
    s2c: [u8; 12],
}

impl ChannelNonces {
    /// Derive both starting nonces from the channel seed handed to the client.
    pub fn from_seed(seed: [u8; 12]) -> Self {
        let mut c2s = seed;
        c2s[11] ^= 1; // the client's encrypt nonce is the flipped one
        Self { c2s, s2c: seed }
    }

    pub fn peek(&self, dir: Direction) -> [u8; 12] {
        match dir {
            Direction::ClientToServer => self.c2s,
            Direction::ServerToClient => self.s2c,
        }
    }

    /// Advance one direction's counter: **+1, little-endian, from byte[0]**.
    ///
    /// Call this only after the GCM operation succeeded - the client does the same, so incrementing
    /// on a failed op would desynchronise the channel.
    pub fn advance(&mut self, dir: Direction) {
        let n = match dir {
            Direction::ClientToServer => &mut self.c2s,
            Direction::ServerToClient => &mut self.s2c,
        };
        for b in n.iter_mut() {
            *b = b.wrapping_add(1);
            if *b != 0 {
                break;
            }
        }
    }
}

/// Decrypt a mode-1 payload: `tag[16] || ciphertext`, AES-128-GCM, no AAD.
pub fn open_channel_payload(
    session_key: &[u8; 16],
    nonce: [u8; 12],
    payload: &[u8],
) -> Result<Vec<u8>, String> {
    if payload.len() < GCM_TAG_LEN {
        return Err(format!("payload {} shorter than the tag", payload.len()));
    }
    // ⚠ Tag FIRST, then ciphertext. `aes-gcm` expects ciphertext||tag, so reorder.
    let (tag, ct) = payload.split_at(GCM_TAG_LEN);
    let mut buf = Vec::with_capacity(ct.len() + GCM_TAG_LEN);
    buf.extend_from_slice(ct);
    buf.extend_from_slice(tag);
    <Aes128Gcm as GcmKeyInit>::new(session_key.into())
        .decrypt(Nonce::from_slice(&nonce), Payload { msg: &buf, aad: b"" })
        .map_err(|_| "GCM authentication failed".to_string())
}

/// Seal a mode-1 payload and wrap it in an envelope. Returns the whole frame.
pub fn seal_channel_frame(
    session_key: &[u8; 16],
    nonce: [u8; 12],
    plaintext: &[u8],
) -> Result<Vec<u8>, String> {
    let sealed = <Aes128Gcm as GcmKeyInit>::new(session_key.into())
        .encrypt(Nonce::from_slice(&nonce), Payload { msg: plaintext, aad: b"" })
        .map_err(|_| "GCM encrypt failed".to_string())?;
    let (ct, tag) = sealed.split_at(sealed.len() - GCM_TAG_LEN);
    let mut payload = Vec::with_capacity(sealed.len());
    payload.extend_from_slice(tag); // tag precedes ciphertext on the wire
    payload.extend_from_slice(ct);

    let mut frame = Vec::with_capacity(6 + payload.len());
    frame.push(ENVELOPE_VERSION);
    frame.push(ENVELOPE_MODE_CHANNEL);
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

/// An 8-byte class-2 reply header with status 200 and no body - the shape both known secured
/// responses use.
pub fn reply_header(msg_type: u16, sequence: u32) -> Vec<u8> {
    let mut p = Vec::with_capacity(REPLY_HEADER_LEN);
    p.extend_from_slice(&msg_type.to_be_bytes());
    p.extend_from_slice(&sequence.to_be_bytes());
    p.extend_from_slice(&STATUS_OK.to_be_bytes());
    p
}

/// Reply to `0x000c` (`c->b sub req`) - class-2 header, status 200, **zero payload**.
///
/// # Why this is the cure for `error.lost_bap_connection`
///
/// `[C]` The client's BAP connection keeps **two in-flight-request queues** at `conn+0x1b80`,
/// stride `0x198` (`historicaldocs/design/bap-protocol.md` §3.2). They are not a "recipient registry": an entry
/// exists for exactly as long as a request the *client* sent is outstanding. Two functions walk
/// them, and both were read out of the eboot:
///
/// * `0xb2e940` (`update_receive`) / `0xb2e030` (`update_receiving_header`) - for each queue,
///   examine **only the head** (`e = q[*(i32*)(q-0xc)]`) and accept an inbound reply only if
///   `head.expected_reply_type == rx.type` **and** `head.sequence == rx.sequence`. No match in
///   either queue, and no push handler at `conn+0x1138` claiming the type, ⇒ error `0x12`
///   `_connection_failure_failed_to_find_message_recipient`.
/// * `0xb2d500` (`update_receiving`) - same two queues, same stride, calling `0xb2d7d0` per queue;
///   a queue holding a timed-out element ⇒ `_connection_failure_timed_out`
///   (`"message queue '%d' contains timed out elements, bailing."`, fmt `0x167ad83`).
///
/// `0x000c` arrives at **sequence 7**, before the keepalive loop, and every run this project has
/// ever made left it unanswered. Two consequences follow mechanically, and both were measured:
///
/// 1. **It times the connection out on its own.** Run `20260807-162406-silent504` (10/10 gates)
///    left `0x000c` as the *only* unanswered message in the entire transcript - no `0x0020`, no
///    `0x0015`, no `701`, no `504`, the client never got that far - and BAP died at 25 messages /
///    1014 B with `update_receiving() message queue '1' contains timed out elements`.
/// 2. **It head-of-line blocks every later reply.** While it sits at the head of queue 1, any
///    `c->ws rsp` (`0x000b`) we send fails the *type* comparison against the head's expected
///    `0x000d` and is fatal. That is exactly what killed run `20260807-161008-roster-account-2`
///    cycle 2: our 32-byte reply to investment id **504** at sequence 13 produced
///    `update_receive() failed to find recipient for header [sequence 13][type c->ws rsp]
///    [status 200]` and `_connection_failure_failed_to_find_message_recipient`.
///
/// `[C]` The reply shape is not a guess. `historicaldocs/design/character-and-orbit.md` §26.64 read policy `0x0d`
/// (`client_to_bap_subscription_request`, object `0x1924550`) out of the client's own policy
/// descriptor table: request wire id `0x000c` (`0x55f9e0`), **response wire id `0x000d`**
/// (`0x55fa20`), **response payload size 0** (`0x1df0`). Its handler at `+0x58` is `0x50fe20`,
/// a bare `ret` - so the reply changes no client state. That is *why* §26.58 measured "answering
/// `0x000d` did nothing": it was looking for a subscription-state change, which cannot happen.
/// The effect that matters is invisible from the guest log - it **dequeues the entry**.
///
/// ⇒ Answering this is not "inventing a message". Silence here is what breaks the transport, and
/// it is the confound underneath the earlier `0x0020`/`701`/`504` "answering wrongly is worse than
/// not answering" measurements: those replies were rejected for being *out of order*, not for being
/// wrong.
///
/// Reach: `observed-live`, `synthetic_ancestor` (our account/character objects are minted).
pub fn client_to_bap_subscription_response(sequence: u32) -> Vec<u8> {
    reply_header(TYPE_CLIENT_TO_BAP_SUBSCRIPTION_RESPONSE, sequence)
}

/// Deprecated shim - see [`client_to_bap_subscription_response`].
#[deprecated(
    since = "0.1.0",
    note = "renamed to client_to_bap_subscription_response, after the client's own class name \
            `client_to_bap_subscription_response` (id 13 decimal / 0x000D hex)."
)]
pub fn family_subscribe_response(sequence: u32) -> Vec<u8> {
    client_to_bap_subscription_response(sequence)
}

/// Reply to an investment server message: class-2 header, then the echoed `u16be` message id.
///
/// `[U]` The **body** for a given message id is content-defined, not readable from the eboot, so this
/// carries the id and nothing else. If a body is required the client will say so - and at diagnostic
/// threshold T<=3 it names the type and status outright (`Request failed, type=%d, status=%s`,
/// vaddr `0x162da7a`), which is cheaper than guessing.
pub fn investment_message_response(sequence: u32, message_id: u16) -> Vec<u8> {
    investment_message_response_status(sequence, message_id, STATUS_OK)
}

/// As above, with an explicit status - the **decode-skip probe**.
///
/// `handle_response_internal` (`0x5681e0`) reads the response status and, when the policy carries a
/// status code (`test al,0x10`), compares it to 200 (`cmp r14w,0xc8`) and **`jne`s past the decode
/// entirely** (`0x568289`). So a status ≠ 200 means the client never looks at the body - which is
/// worth knowing because the 503 argument schema is **behind Oodle compression** and not currently
/// readable.
///
/// Three mutually exclusive outcomes, one run decides:
/// 1. request-failed logged **and the bootflow moves on** → 503 is not the gate; iterate to find it.
/// 2. request-failed **then a halt naming 503** → 503 is mandatory and Oodle is unavoidable -
///    decided on evidence rather than assumed.
/// 3. **unchanged** → this policy lacks flag `0x10` and the status is never read. Also decisive.
pub fn investment_message_response_status(sequence: u32, message_id: u16, status: u16) -> Vec<u8> {
    investment_message_response_padded(sequence, message_id, status, 0)
}

/// As above, plus `pad` zero bytes of **argument bitstream**.
///
/// Why this is worth a run. `0x261fb0` decodes a response body by reading a **16-bit message id off
/// the bitstream** (`mov esi,0x10; call 0x72c70`) and *then* decoding argument fields against the
/// schema tag in the definition record. Our reply carries the id and **nothing after it**, so the
/// argument decoder starts already out of data - which is a very plausible reading of
/// `bitstream error when decoding message-arguments`.
///
/// These are hand-rolled bit grammars with **leading presence bits** - the character RSAT opens
/// `1 · u64be(soid) · 1 · …`, where each `1` means "present". A run of **zero** bits is therefore the
/// natural encoding of *every optional field absent*. If 503's arguments are all optional, zero
/// padding is a legitimate empty response rather than a guess at content.
///
/// It is still a probe, not a contract: a schema with any *required* field will reject it, and the
/// complaint should then change in a way that tells us how much it wanted.
pub fn investment_message_response_padded(
    sequence: u32,
    message_id: u16,
    status: u16,
    pad: usize,
) -> Vec<u8> {
    let mut p = Vec::with_capacity(REPLY_HEADER_LEN + 2 + pad);
    p.extend_from_slice(&TYPE_CLIENT_TO_WORLD_SERVER_RESPONSE.to_be_bytes());
    p.extend_from_slice(&sequence.to_be_bytes());
    p.extend_from_slice(&status.to_be_bytes());
    p.extend_from_slice(&message_id.to_be_bytes());
    p.resize(p.len() + pad, 0);
    p
}

// ────────────────────── account_id_translation - platform id → account SOID ──────────────────────

/// List-kind tag: entries are **36-byte platform identifiers**. `0x55A8A4` (`cmp edi, 7`).
pub const AIDT_KIND_PLATFORM_ID: u8 = 7;
/// List-kind tag: entries are **`u64` investment account SOIDs**. `0x55A838` (`cmp edi, 0xff`).
pub const AIDT_KIND_ACCOUNT_SOID: u8 = 0xff;

/// Wire size of one platform identifier, in **bits**. `0x2B79F0` reads `0x120` bits via `0x730B0`.
pub const AIDT_PLATFORM_ID_BITS: u32 = 0x120;
/// Wire size of one platform identifier, in bytes (`0x120 / 8`).
pub const AIDT_PLATFORM_ID_LEN: usize = 36;

/// Encode a `platform_to_investment_response` - **the account-SOID translation the client is
/// waiting for**.
///
/// `[C]` `static-client-inference`, read out of the client's own decoder at `0x55A490` (the
/// subsystem that names itself `networking:investment:account_id_translation`). Grammar:
///
/// ```text
/// u16  count                       0x55A5CA  read_bits(16)
/// u8   kind_a                      0x55A5DD  read_bits(8)   must be 7 or 0xFF
/// u8   kind_b                      0x55A5ED  read_bits(8)   must be 7 or 0xFF
/// list A: count entries of kind_a  0x55A7D7  helper(hdr[0], …)
/// list B: count entries of kind_b  0x55A7EF  helper(hdr[4], …)
/// ```
///
/// The helper `0x55A810` switches on the kind: `0xFF` reads `count × 0x40` bits (`0x73170`) as
/// `u64` SOIDs; `7` reads `count × 0x120` bits (`0x2B79F0` → `0x730B0`) as flat 36-byte platform
/// identifiers. **The two lists are parallel arrays** - entry *i* of A corresponds to entry *i* of
/// B - and `0x55A8E3` walks list A comparing each identifier against the client's own to find its
/// row.
///
/// The 36-byte identifier is exactly the flat field the client puts in its **503 request**
/// (`"D1Guardian"` NUL-padded), so the natural response echoes it back verbatim.
///
/// ⚠ **`[U]` This encodes a message whose delivery path is not yet proven.** The decoder is
/// standalone; whether it is fed by the 503 response body or by a separate BAP message is open (see
/// `historicaldocs/design/character-and-orbit.md` §10.6). Anything the client accepts on the back of this is
/// `synthetic-minimal` and taints everything downstream as `synthetic_ancestor`.
pub fn platform_to_investment_response(entries: &[([u8; AIDT_PLATFORM_ID_LEN], u64)]) -> Vec<u8> {
    let mut bits: Vec<bool> = Vec::with_capacity(32 + entries.len() * (0x120 + 0x40));
    let push = |bits: &mut Vec<bool>, v: u64, n: u32| {
        for i in (0..n).rev() {
            bits.push((v >> i) & 1 == 1);
        }
    };

    push(&mut bits, entries.len() as u64, 16);
    push(&mut bits, AIDT_KIND_PLATFORM_ID as u64, 8);
    push(&mut bits, AIDT_KIND_ACCOUNT_SOID as u64, 8);

    // List A, then list B - the decoder consumes them in declared order, not interleaved.
    for (id, _) in entries {
        for &byte in id.iter() {
            push(&mut bits, byte as u64, 8);
        }
    }
    for (_, soid) in entries {
        push(&mut bits, *soid, 64);
    }

    while bits.len() % 8 != 0 {
        bits.push(false);
    }
    bits.chunks(8)
        .map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | b as u8))
        .collect()
}

/// The 36-byte platform identifier for an online ID, NUL-padded - the shape `0x2B79F0` reads.
pub fn platform_identifier(online_id: &str) -> [u8; AIDT_PLATFORM_ID_LEN] {
    let mut out = [0u8; AIDT_PLATFORM_ID_LEN];
    let src = online_id.as_bytes();
    let n = src.len().min(AIDT_PLATFORM_ID_LEN);
    out[..n].copy_from_slice(&src[..n]);
    out
}

/// Message-503 response body per its **own schema descriptor** - a single optional `u64`.
///
/// `[C]` `static-client-inference`, 2026-08-06. Resolved through the definition table
/// (`historicaldocs/design/character-and-orbit.md` §12): message 503's response schema class is `0x80801825`, whose
/// descriptor sits at eboot vaddr `0x1324080` and declares **`nfields = 1`, `field0_class =
/// 0x80801796`**; that class's own descriptor (`0x12DA950`) is **`size = 8`, `nfields = 0`** - a bare
/// `u64` leaf.
///
/// The descriptor format was validated against the **request** side as a Rosetta stone: class
/// `0x80801726` at `0x12FE950` declares `size = 0x30` with one field of class `0x80800b1a`, and
/// `0x30` = 48 is exactly the platform-identifier struct we observe on the wire (36 flat bytes + pad
/// + `u64`). Descriptor layout: `+0x00` class tag · `+0x04` decoded object size · `+0x28` field count
/// · `+0x2C` field-0 class. **This region carries no relocations**, unlike the signon descriptor
/// tables - the records are flat and self-contained.
///
/// Grammar, following the leading-presence-bit convention these bitstreams use everywhere:
/// `1 · u64be(account_soid)`, zero-padded to a byte boundary ⇒ **9 bytes**.
/// The argument bitstream for **investment message 504** (`login_character`) - the reply the client
/// waits for after the player selects an existing character at `character:signin`.
///
/// # Provenance - the client's own encoder, not the wire
///
/// `[C]` Definition row (`capture-rig/schemas/investment-message-definitions.bin`, id 504):
/// request schema `0x808017CE`, response schema `0x80801845`.
///
/// `[C]` Both resolved through the **live client's** package registry
/// (`capture-rig/dump-registry.py --schema`, run `20260807-162724-subackhdr-1`):
///
/// ```text
/// 0x808017ce  struct 8 bytes, 1 field    [0] kind 0x6 LEAF 64 bits  @struct+0x0   <- REQUEST
/// 0x80801845  struct 8 bytes, 1 field    [0] kind 0x1 -> 0x80801796 @struct+0x0   <- RESPONSE
///   0x80801796  struct 8 bytes, 2 fields  [0] kind 0x3 LEAF  4 bits @+0x0
///                                         [1] kind 0x5 LEAF 32 bits @+0x4
/// ```
///
/// **Positive control on the request side:** the observed 504 request body is
/// `01 f8 | d1 00 00 00 00 00 01 01 | 00` = `u16be 504` then the 64-bit leaf holding character SOID
/// `0xD100000000000101`. The schema predicts that shape exactly, which is what licenses trusting the
/// response side of the same row.
///
/// The bytes below were produced by driving the client's **own encoder** `0x261BE0` over schema
/// `0x80801845` with an all-zero C object under Unicorn
/// (`capture-rig/oodle/rsat-schema-encode-oracle.py --tag 0x80801845 --msgid 504 --obj
/// 0000000000000000`), whose `--selftest` reproduces the pinned 503 and 206 vectors first. The
/// result was fed straight back through the client's **own decoder** `0x261FB0` and round-tripped to
/// the same zero object (`rc=1`). Byte-flip sensitivity over the C struct pins the wire layout:
/// `+0x00` (the 4-bit leaf) → wire bit 19; `+0x07..+0x04` (the 32-bit leaf) → wire bits 20..51,
/// big-endian. 16 + 4 + 32 = 52 bits ⇒ 7 bytes on the wire including the `u16be` message id.
///
/// **Second positive control:** the confirmed-working 503 response body begins with the byte `0x18`
/// too, because 503's response schema `0x80801825` has this *same* `0x80801796` composite as its
/// field `[0]` and we send it zeroed. So a zeroed `0x80801796` is already known to be accepted by
/// this client on a live wire.
///
/// `[U]` The **semantics** of the two leaves (result code? character index?) are unknown. Zero is
/// what 503 sends and 503 is accepted; that is the whole basis for choosing it.
///
/// # ⚠ Correct bytes are not sufficient - this reply is ORDER-DEPENDENT
///
/// `[C]` `observed-live`, run `20260807-165310-operator-504` (operator drove the character card by
/// hand). This body went on the wire and the client **still** hung up:
///
/// ```text
/// ** NAMED GATE ** no secured handler for inner type 0x0020 seq 10   <- unanswered
/// ** NAMED GATE ** no secured handler for inner type 0x0015 seq 11   <- unanswered
/// INVESTMENT message id 701                                          <- unanswered
/// INVESTMENT message id 504  -> native-oracle body 1800000000        <- answered, correctly
/// update_receive() failed to find recipient for header [sequence 13][type c->ws rsp][status 200]
/// _connection_failure_failed_to_find_message_recipient
/// ```
///
/// **`failed to find recipient` is not a decode complaint.** A wrong body produces
/// `handle_response_internal: Failed to decode [policy N]`; this fired before any decode, in the
/// queue walk. `0x0020` (seq 10) was sitting at the head of in-flight queue 1 expecting type
/// `0x0021`, our reply was type `0x000B` at sequence 13, the type comparison at `0xb2e9d1` missed,
/// and only the head is ever examined ([`client_to_bap_subscription_response`]).
///
/// ⇒ **The remaining `character:signin` weasel is an ORDERING problem, not a payload problem.**
/// This body cannot land until `0x0020` (→ `0x0021`), `0x0015` (→ `0x0016`) and investment **701**
/// (→ `0x000B`, response schema `0x8080189B`) are answered first, in the order the client sent
/// them. Set `D1_INV_SILENT=504` to suppress this reply meanwhile: silence does not fix anything
/// either, but it trades an instant disconnect for a slower `_connection_failure_timed_out` and so
/// leaves the character screen up for longer.
/// The argument bitstream for **investment message 205**, which the client sends **only after its
/// PSN platform session exists** - it appeared for the first time in run
/// `20260807-215331-PSN-SESSION-ORBIT-3`, immediately before the connection died.
///
/// Catalogue row 62: request `0x80801874` (0 fields), response **`0x80801827`** - and unlike 504 /
/// 701 / 702 (which share an 8-byte one-composite shape) this one decodes to a **952-byte** object:
/// a 4-bit leaf plus a nested 944-byte, 8-field struct.
///
/// Derived by driving the client's own encoder `0x261BE0` over a zeroed C object
/// (`d1-oracle message --tag 0x80801827 --msgid 205 --request-tag 0x80801874`), then stripping the
/// leading 2-byte message id - the same convention `investment_504_response_body` uses, verified by
/// re-deriving 504's wire (`01f8` + `1800000000`) against the stored constant.
///
/// ## ⭐ The all-zero version of this body CAUSED `family_record_not_found` `[C]` 2026-08-07
///
/// Run `20260807-215628-MSG205-ORBIT` shipped a **zeroed** object here and the client died with
/// `error.investment_server_object_family_record_not_found` - `handle_queuez_object_update,
/// family_type=ENUM(5), family_root_soid=0x0000000000000000`. The archive A/B is clean: run
/// `213854-VOICE-FIX-ORBIT` reached the identical instruction (`creating psn session as host`,
/// `ui_stage ENUM(3)→ENUM(5)`) with **zero** family-5 lines, and differed only in not answering 205.
///
/// **The mechanism, read out of the descriptor tables:** 205's response `0x80801827` and *503's*
/// response `0x80801825` **embed the same nested 944-byte struct `0x80801941`** - 503 carries it at
/// field [4] (C offset `+0x018`), 205 at field [1] (C offset `+0x008`). `0x80801941`'s field [0] is
/// a 64-bit leaf holding the **QueueZ family-5 root SOID**. We had been populating it correctly in
/// 503 (`D1_503_RAW` pins `0x7FFFFFFFFFFFFFFF`, which matches the live registry's
/// `class=5 key=0x7FFFFFFFFFFFFFFF state=3 REPLICATING`) and sending it **zero** in 205. Same
/// struct, two answers, one of them declaring family 5 rooted at nothing.
///
/// **Positive control for the offset model** - the oracle rebuilds 503's known-good body from a
/// struct, byte-for-byte, before this was trusted:
/// ```text
/// d1-oracle message --tag 0x80801825 --msgid 503 --request-tag 0x80801726 \
///     --obj <968B zeros, +0x008=0xD100000000000001, +0x018=0x7FFFFFFFFFFFFFFF>
///   -> 01f7 180000000d100000000000001000000000000000 0bfffffffffffffffc0...  == D1_503_RAW ✓
/// ```
/// Then the same method at 205's offset (`+0x008`) gives the constant below. The two bodies' shared
/// `0x80801941` tail is byte-identical, which `family5_root_tail_matches_503` asserts.
///
/// ⚠ Reach: `synthetic-minimal`. The encoding is `[C]` - it round-trips through the client's own
/// codec and allocates exactly `0x3D8` = 952 bytes. Field [0]'s *identification* as the family-5
/// root is `[I]` from the 503 parallel; the other seven fields remain `[U]` and are sent absent.
pub fn investment_205_response_body() -> Vec<u8> {
    // `d1-oracle message --tag 0x80801827 --msgid 205 --request-tag 0x80801874 --obj
    //  <952B zeros, +0x008=0x7FFFFFFFFFFFFFFF>` -> 00cd + these 25 bytes.
    vec![
        0x1b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfc, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x02, 0x08, 0x10, 0x22, 0x00, 0x00, 0x00, 0x01, 0x00,
    ]
}

/// ⭐⭐ **The UNLOCK-FLAG pair - 205 and 503 carrying the same four unlock flags.**
///
/// `historicaldocs/design/rung33-unlock-gate.md`. Rung 33's `activity_failed_to_load_missing_dlc` is an **unlock**
/// gate, not an ownership gate (`0x28CACD  cmp byte [rdi+rax], 2`, `rdi` = the derived availability
/// plane, `rax` = a u16 flag index). **All 21 matchmaking-bypass activities 393-413 gate on flag
/// index 98** = `director_book_visible:The Crucible`. Replaying the client's own `0x54F410` on
/// `runs/snapshots/orbit-all.snap`, the set `{98, 834, 5015, 6018}` clears **21/21**; an empty plane
/// clears 0/21 and the off-by-one set `{99, 835, 5016, 6019}` clears 0/21.
///
/// The flags ride `0x80801941` **field [3]** - `0x80804681 { u6 count; 50 x {u16 index, u2 value} }`
/// - at C offset `+0x3C` inside that struct, i.e. **`+0x054` in 503's 968-byte object** and
/// **`+0x044` in 205's 952-byte object**. Both bodies were emitted by the client's own encoder:
///
/// ⚠ **The count leaf is SIX bits, not five** (corrected 2026-08-10). `d1-eboot descriptors --tag
/// 0x80804681` → `[0] @+0x000 kind 0x5 LEAF 6 bits` (anchor `0x131b588`). A 5-bit count caps at 31
/// and still "works" for every set we have ever shipped, so no passing test would have caught it.
/// The `50 x` half is right: container header `0x132a290` `+0x20 = 0x32`, `+0x28 = 4`.
///
/// ```text
/// d1-oracle message --tag 0x80801825 --msgid 503 --request-tag 0x80801726 --obj <968B …>
/// d1-oracle message --tag 0x80801827 --msgid 205 --request-tag 0x80801874 --obj <952B …>
/// ```
///
/// **Controls, both directions, for each message:**
/// * an object with the flag array *absent* reproduces the pinned body **byte-for-byte**;
/// * ⭐ the **negative**: slots filled but `count = 0` also reproduces the pinned body byte-for-byte
///   - so the count is what serialises and a stale slot is never emitted;
/// * changing the indices to `{99, 835, 5016, 6019}` changes exactly the index fields.
///
/// ⚠ Reach: **`synthetic-minimal`**, and it *identifies* rather than satisfies - a real client is
/// granted these flags by playing the campaign, not by a server asserting them.
/// ⛔ **v1, SUPERSEDED - `{98, 834, 5015, 6018}` alone. Shipped live and it was a NO-OP.**
///
/// Run `20260809-075545-RUNG33-UNLOCK-98`: all four bytes read `0x02` in the live plane at the exact
/// addresses the evaluator uses - **and rung 33 still refused.** Cause, measured offline: the live
/// client gates on activity index **0**, whose expression reads `{3389, 308, 419, 440}` and **never
/// touches 98 at all.** Kept as the record of a real falsification; use
/// [`INVESTMENT_503_UNLOCKS_RAW`] instead.
pub const INVESTMENT_503_UNLOCKS_RAW_V1_ACT393_ONLY: &[u8] = &[
    0x18, 0x00, 0x00, 0x00, 0x0d, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xfc, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x08, 0x98, 0x06, 0x2f, 0x83,
    0x42, 0xf9, 0x39, 0x7f, 0x97, 0x82, 0xf0, 0x22, 0x00, 0x00, 0x00, 0x01, 0x00,
];

/// ⭐ **v2 - the UNION of activity index 0 and activities 393-413:
/// `{98, 308, 419, 440, 834, 3389, 5015, 6018}`.**
///
/// Rung 33 takes its activity index from `0x78C770` → the udt-1 `global_activity_state` blob at
/// `+0x680`, which the Activity Host currently sends as **0**. So the index it evaluates is whatever
/// survives the *private-activity repair* teardown, not the activity that was launched. Covering
/// both index sets makes rung 33 pass regardless of which one the blob carries - i.e. it removes
/// rung 33 as a confound rather than betting on the blob being fixed first.
///
/// Offline (`d1-replay`, the client's own `0x54F410`): clears index 0-6 **and** 393-413; empty plane
/// clears none; every index shifted by +1 clears none.
///
/// ⛔ **v2, SUPERSEDED 2026-08-10 - it is MISSING `{2667, 5987}`.** Rung 33's *second* wall
/// (**urchin**, `activity_failed_to_load_missing_permissions`) was cleared on `pvp_moonbase` only by
/// poking those two indices straight into the derived plane (`[0x26A34E8]+0xC+idx = 0x02`). Kept
/// under this name so run `20260809-225119-UNION8-MOONBASE` stays reproducible; the shipped default
/// is now [`INVESTMENT_503_UNLOCKS_RAW`], which carries all ten.
pub const INVESTMENT_503_UNLOCKS_RAW_V2_UNION8: &[u8] = &[
    0x18, 0x00, 0x00, 0x00, 0x0d, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xfc, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x09, 0x18, 0x06, 0x2f, 0x81,
    0x34, 0xf8, 0x1a, 0x3f, 0x81, 0xb8, 0xf8, 0x34, 0x2f, 0x8d, 0x3d, 0xf9, 0x39, 0x7f,
    0x97, 0x82, 0xf0, 0x22, 0x00, 0x00, 0x00, 0x01, 0x00,
];

/// The 205 half of [`INVESTMENT_503_UNLOCKS_RAW_V2_UNION8`]. **These two must ship together** - see
/// [`family5_root_tail_matches_503`] and the responder's runtime guard. 205 is answered *after* 503
/// (`observed-live`: `runs/20260809-054417-RUNG33-BYPASS/responder.log`, 503 at line 139, 205 at
/// line 249), so a 205 that still carries `count = 0` can reset what 503 delivered.
pub const INVESTMENT_205_UNLOCKS_BODY_V2_UNION8: &[u8] = &[
    0x1b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfc, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x02, 0x09, 0x18, 0x06, 0x2f, 0x81, 0x34, 0xf8, 0x1a, 0x3f, 0x81, 0xb8,
    0xf8, 0x34, 0x2f, 0x8d, 0x3d, 0xf9, 0x39, 0x7f, 0x97, 0x82, 0xf0, 0x22, 0x00, 0x00,
    0x00, 0x01, 0x00,
];

/// The v1 205 half, kept alongside [`INVESTMENT_503_UNLOCKS_RAW_V1_ACT393_ONLY`] so the pair that
/// was actually shipped on 2026-08-09 stays reproducible.
pub const INVESTMENT_205_UNLOCKS_BODY_V1_ACT393_ONLY: &[u8] = &[
    0x1b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfc, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x02, 0x08, 0x98, 0x06, 0x2f, 0x83, 0x42, 0xf9, 0x39, 0x7f, 0x97, 0x82,
    0xf0, 0x22, 0x00, 0x00, 0x00, 0x01, 0x00,
];

/// The known-good `D1_503_RAW` argument bitstream that `capture-rig/run-boot.sh` pins, kept here so
/// the 205 body can be tested against it. Its trailing `0x80801941` bytes are the family-5 root.
pub const INVESTMENT_503_PINNED_RAW: &[u8] = &[
    0x18, 0x00, 0x00, 0x00, 0x0d, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xfc, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x08, 0x10, 0x22, 0x00, 0x00,
    0x00, 0x01, 0x00,
];

// ── the TEN-flag unlock pair: the shipped default, and the retirement of the plane poke ─────────

/// ⭐⭐⭐ **The ten unlock-flag indices rung 33 needs - both walls, on a real PvP map.**
///
/// `{98, 308, 419, 440, 834, 3389, 5015, 6018}` (the union-8, [`INVESTMENT_503_UNLOCKS_RAW_V2_UNION8`])
/// cleared **baboon** (`activity_failed_to_load_missing_dlc`) but not **urchin**
/// (`activity_failed_to_load_missing_permissions`). Run `20260809-225119-UNION8-MOONBASE` cleared
/// urchin on `pvp_moonbase` / activity 407 by **poking `2667` and `5987` into the derived plane**
/// (`[0x26A34E8]+0xC+idx = 0x02`). Shipping all ten on the wire is what retires that poke.
///
/// ⚠ **Ten, not "everything".** `historicaldocs/design/grants/rung33-permissions-gate.md` §3.2 measured that
/// setting all 8,192 plane bytes to `2` makes the DLC gate pass and the PERMISSIONS gate **fail**,
/// reporting reason `0x18` instead of `0x17` - i.e. a brand-new problem wearing a new codename. The
/// poison indices `{306, 3872, 3876, 3877}` must stay clear, and none of them is in this set.
///
/// ⚠ Reach: **`synthetic-minimal`**. This *identifies* the gate; it does not satisfy it the way a
/// real service would (a retail account earns these by playing the campaign). Everything downstream
/// of rung 33 stays `synthetic_ancestor`.
pub const UNLOCK_FLAG_INDICES: [u16; 10] =
    [98, 308, 419, 440, 834, 2667, 3389, 5015, 5987, 6018];

/// The value the evaluator demands, and it is an **exact equality**, not a threshold:
/// `0x28CACD  cmp byte ptr [rdi+rax], 2` … `sete al`. Measured on the pinned eboot - `1` fires the
/// refusal, `3` fires the refusal, only `2` passes.
pub const UNLOCK_FLAG_VALUE: u8 = 2;

/// The account SOID the pinned 503 bodies carry, and the one `run-boot.sh` exports.
const UNLOCK_ANCHOR_ACCOUNT_SOID: u64 = 0xD100_0000_0000_0001;

/// Bit offset of the 64-bit account SOID inside a 503 argument bitstream, MSB-first.
///
/// `[C]` by execution and **re-measured on the ten-flag body** (2026-08-10): encoding the same
/// object with SOID `0xD100000000000002` moves exactly bits `{98, 99}`, and encoding
/// `0x0123456789ABCDEF` puts that value's 64 MSB-first bits at `[36, 100)` verbatim. Same value
/// `d1_harness::layout::INV_503_ACCOUNT_SOID_BIT_OFFSET` derives independently for the 45-byte
/// template, so the flag array did not move it.
pub const INV_503_ACCOUNT_SOID_BIT_OFFSET: usize = 36;

/// ⭐⭐⭐ **The 503 half of the ten-flag pair - the body this responder serves by DEFAULT.**
///
/// Emitted by the client's own encoder, not hand-packed:
/// ```text
/// d1-oracle message --tag 0x80801825 --msgid 503 --request-tag 0x80801726 --obj <968 B image:
///     +0x008 = 0xD100000000000001      account SOID
///     +0x018 = 0x7FFFFFFFFFFFFFFF      family-5 root
///     +0x054 = 10                      0x80804681 count
///     +0x058 + 4k = {u16 index, u8 2}  the ten elements >
/// -> wire 72B  01f7 1800…0100          decode rc=1 alloc=0x3e8
/// ```
/// **Three positive controls stand behind that image**, all by execution: with `count = 0` it
/// reproduces [`INVESTMENT_503_PINNED_RAW`] byte-for-byte; with the union-8 index set it reproduces
/// [`INVESTMENT_503_UNLOCKS_RAW_V2_UNION8`] byte-for-byte; and with the v1 set it reproduces
/// [`INVESTMENT_503_UNLOCKS_RAW_V1_ACT393_ONLY`]. Three constants pinned by three earlier sessions,
/// all regenerated from one model.
pub const INVESTMENT_503_UNLOCKS_RAW: &[u8] = &[
    0x18, 0x00, 0x00, 0x00, 0x0d, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xfc, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x09, 0x58, 0x06, 0x2f, 0x81,
    0x34, 0xf8, 0x1a, 0x3f, 0x81, 0xb8, 0xf8, 0x34, 0x2f, 0x8a, 0x6b, 0xf8, 0xd3, 0xdf,
    0x93, 0x97, 0xf9, 0x76, 0x3f, 0x97, 0x82, 0xf0, 0x22, 0x00, 0x00, 0x00, 0x01, 0x00,
];

/// ⭐⭐⭐ **The 205 half - and it is the half that decides the run.**
///
/// 205 is answered **after** 503 and, on today's evidence, **more than once**:
/// `runs/20260810-145553-X11-ORBIT/responder.log` answers it at L254, L1098 and L2953 against a
/// single 503 at L143. Each of those is an independent chance to overwrite what 503 delivered with
/// `count = 0`. **The identical flag body must be on this arm, not merely "also available".**
pub const INVESTMENT_205_UNLOCKS_BODY: &[u8] = &[
    0x1b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfc, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x02, 0x09, 0x58, 0x06, 0x2f, 0x81, 0x34, 0xf8, 0x1a, 0x3f, 0x81, 0xb8,
    0xf8, 0x34, 0x2f, 0x8a, 0x6b, 0xf8, 0xd3, 0xdf, 0x93, 0x97, 0xf9, 0x76, 0x3f, 0x97,
    0x82, 0xf0, 0x22, 0x00, 0x00, 0x00, 0x01, 0x00,
];

/// The ten unlock flags for build **56578**, as that client's own ordinals.
///
/// The 01.29 set `{98, 308, 419, 440, 834, 2667, 3389, 5015, 5987, 6018}` is a list of positions
/// in the client's master flag table, and **positions are not stable across content versions**.
/// The 56578 table (content tag `0x80A5FFCD`, class `0x8080179C`, 7,731 rows of
/// `{u32 hash, u16 kind, u16 sub}`, element class `0x808018B8`) was read from both builds' packages
/// with the client's own decoder. Each 01.29 ordinal was then resolved to its hash and the hash
/// looked up in 56578's table. Controls: 01.29[98] = `3638915056`, and the 01.29 ordinal of
/// `1681748763` = 2538. Both are documented values. 56578 places that hash at 2539, which is the
/// 2020 manifest's `value`. Five flags kept their ordinal and five moved, by +1 to +11. Every
/// moved flag kept the same kind and subindex.
///
/// ⛔ **On 56578, 308 and 3391 lock the Tower.** Activity index 0 (`0x8AE8DA69`, the Tower's own
/// activity) requires `NOT f[308] AND NOT f[3391] AND (f[419] OR f[440]) AND (f[2668] OR f[440])`.
/// The evaluator is `0x28E590`, a stack machine where op 1 pushes `flag[row] == 2`, op 2 is NOT and
/// op 3 is OR; its flag array is indexed by master-table row. Serving this set made the client leave
/// the Tower with `activity_failed_to_load_missing_permissions`. Without 308 and 3391 it stays and
/// loads the social menus (run sense66; `notes/ps4-56578-tower-social-vendors-2026-10-30.md`). The
/// launchers no longer serve those two; this constant stays as the faithful 01.29 → 56578
/// translation.
pub const UNLOCK_FLAGS_56578: [u16; 10] = [98, 308, 419, 440, 834, 2668, 3391, 5021, 5998, 6029];

/// [`INVESTMENT_503_UNLOCKS_RAW`] for build 56578 (flags [`UNLOCK_FLAGS_56578`]).
///
/// Emitted by the 56578 client's own message codec, from the same object image as the 01.29
/// constant with only the indices changed. The positive control ran first: `count = 0` reproduced
/// [`INVESTMENT_503_PINNED_RAW`] and the pinned 205 body byte for byte on 56578.
pub const INVESTMENT_503_UNLOCKS_RAW_56578: &[u8] = &[
    0x18, 0x00, 0x00, 0x00, 0x0d, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xfc, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x09, 0x58, 0x06, 0x2f, 0x81,
    0x34, 0xf8, 0x1a, 0x3f, 0x81, 0xb8, 0xf8, 0x34, 0x2f, 0x8a, 0x6c, 0xf8, 0xd3, 0xff,
    0x93, 0x9d, 0xf9, 0x76, 0xef, 0x97, 0x8d, 0xf0, 0x22, 0x00, 0x00, 0x00, 0x01, 0x00,
];

/// [`INVESTMENT_205_UNLOCKS_BODY`] for build 56578; agrees with
/// [`INVESTMENT_503_UNLOCKS_RAW_56578`] on the shared `0x80801941` tail.
pub const INVESTMENT_205_UNLOCKS_BODY_56578: &[u8] = &[
    0x1b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfc, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x02, 0x09, 0x58, 0x06, 0x2f, 0x81, 0x34, 0xf8, 0x1a, 0x3f, 0x81, 0xb8,
    0xf8, 0x34, 0x2f, 0x8a, 0x6c, 0xf8, 0xd3, 0xff, 0x93, 0x9d, 0xf9, 0x76, 0xef, 0x97,
    0x8d, 0xf0, 0x22, 0x00, 0x00, 0x00, 0x01, 0x00,
];

/// The default 205 body for the client build reported at SignOn (see [`set_client_build`]).
pub fn investment_205_unlocks_body() -> &'static [u8] {
    if client_is_56578() { INVESTMENT_205_UNLOCKS_BODY_56578 } else { INVESTMENT_205_UNLOCKS_BODY }
}

// ── the `0x80804681` flag array on the wire: measured, then implemented ──────────────────────────
//
// ⛔ EVERYTHING BELOW IS A BIT GRAMMAR, WHICH THIS PROJECT DOES NOT GUESS. Each field was measured
// by encoding one object per hypothesis through the client's own encoder and diffing, 2026-08-10:
//
//   * count     - a 6-bit field at bit 301 of a 503 argument bitstream / bit 141 of a 205 body.
//                 Read 0 / 4 / 8 / 10 across the four pinned bodies, matching their four index sets.
//   * element   - exactly **20 bits**, `1 ++ (index ^ 0x8000)[16] ++ 1 ++ ((value + 1) & 3)[2]`,
//                 MSB-first. Reproduces all 9 index probes (0, 1, 0x1fff, 0x2000, 0x2001, 0x4000,
//                 0x7fff, 0x8000, 0xffff, 98) and all 4 value probes (0, 1, 2, 3) exactly.
//                 ⚠ A hand-packed `{u16, u2}` = 18-bit element is WRONG, and wrong in the way this
//                 project has been burned by before: the short body still "decodes", reading zeros
//                 past the end. The two extra bits are the two fields' presence bits, and the index
//                 is stored in **offset binary** (`^ 0x8000`), which is why an 18-bit guess also
//                 gets every index wrong by 0x8000.
//   * tail      - a constant 53 bits after the array, in every body. `301 + 6 + 53 = 360` = the
//                 45-byte pinned 503 exactly, and `141 + 6 + 53 = 200` = the 25-byte pinned 205
//                 exactly, so the pinned pair IS the `count = 0` case and the builder below can
//                 derive prefix and tail from them rather than carrying a magic hex string.
//
// The builder is therefore a convenience, never the source of truth: `unlock_bodies_match_the_pinned
// _oracle_output` asserts it reproduces all four independently-pinned oracle-derived bodies, and
// `unlock_flag_bodies_decode_through_the_clients_own_codec` decodes its output back with the client.

/// Bit offset of the `0x80804681` count inside a **503 argument bitstream**.
const UNLOCK_COUNT_BIT_503: usize = 301;
/// Bit offset of the `0x80804681` count inside a **205 response body**.
const UNLOCK_COUNT_BIT_205: usize = 141;
/// Bit offset of the count inside the **shared `0x80801941` tail** (`503[21..]` / `205[1..]`).
/// `301 - 21*8 == 141 - 1*8 == 133` - the two agree, which is the whole point of the shared struct.
const UNLOCK_COUNT_BIT_SHARED: usize = 133;
const UNLOCK_COUNT_BITS: usize = 6;
const UNLOCK_ELEMENT_BITS: usize = 20;

fn to_bits(bytes: &[u8]) -> Vec<bool> {
    bytes.iter().flat_map(|b| (0..8).map(move |i| b >> (7 - i) & 1 == 1)).collect()
}

fn from_bits(bits: &[bool]) -> Vec<u8> {
    let mut out = vec![0u8; bits.len().div_ceil(8)];
    for (i, &b) in bits.iter().enumerate() {
        if b {
            out[i / 8] |= 1 << (7 - i % 8);
        }
    }
    out
}

fn push_be(out: &mut Vec<bool>, value: u64, width: usize) {
    for i in (0..width).rev() {
        out.push(value >> i & 1 == 1);
    }
}

fn take_be(bits: &[bool], at: usize, width: usize) -> Option<u64> {
    bits.get(at..at + width)?.iter().fold(Some(0u64), |acc, &b| Some(acc? << 1 | u64::from(b)))
}

/// Splice a flag array into one of the pinned `count = 0` bodies.
fn splice_flag_array(pinned: &[u8], count_bit: usize, entries: &[(u16, u8)]) -> Vec<u8> {
    let src = to_bits(pinned);
    let mut out: Vec<bool> = src[..count_bit].to_vec();
    push_be(&mut out, entries.len() as u64, UNLOCK_COUNT_BITS);
    for &(index, value) in entries {
        out.push(true); // field [0] present
        push_be(&mut out, u64::from(index ^ 0x8000), 16);
        out.push(true); // field [1] present
        push_be(&mut out, u64::from(value.wrapping_add(1) & 3), 2);
    }
    // The 53-bit tail, taken from the pinned body rather than restated.
    out.extend_from_slice(&src[count_bit + UNLOCK_COUNT_BITS..]);
    from_bits(&out)
}

/// ⭐ **Build the 503 argument bitstream and the 205 response body for an arbitrary flag set.**
///
/// Returns `(body_503, body_205)`, already agreeing over the shared `0x80801941`
/// (`body_503[21..] == body_205[1..]`) by construction - the two are spliced from the same pinned
/// pair at the two offsets that name the same struct.
///
/// The 503 half carries [`UNLOCK_ANCHOR_ACCOUNT_SOID`]; use [`investment_503_unlocks_raw_for`] when
/// the run's account SOID differs.
///
/// ⚠ `indices` is not deduplicated or sorted - it is emitted verbatim, because "the client's own
/// encoder wrote this array in this order" is the only claim we can make about it. More than 50
/// entries overflows the `0x80804717` container (50 × 4 B) and is refused.
pub fn unlock_flag_bodies(indices: &[u16]) -> Option<(Vec<u8>, Vec<u8>)> {
    if indices.len() > 50 {
        return None;
    }
    let entries: Vec<(u16, u8)> = indices.iter().map(|&i| (i, UNLOCK_FLAG_VALUE)).collect();
    Some((
        splice_flag_array(INVESTMENT_503_PINNED_RAW, UNLOCK_COUNT_BIT_503, &entries),
        splice_flag_array(&investment_205_response_body(), UNLOCK_COUNT_BIT_205, &entries),
    ))
}

/// [`INVESTMENT_503_UNLOCKS_RAW`] with `account_soid` spliced in at
/// [`INV_503_ACCOUNT_SOID_BIT_OFFSET`].
///
/// ⚠ **Why this exists rather than "just serve the constant".** The constant bakes in
/// `0xD100000000000001`. The moment it became the *default* body, a run that exports a different
/// `D1_ACCOUNT_SOID` would have been handed someone else's account id with no diagnostic anywhere -
/// the same silent-identity hazard `d1-harness` already guards for multi-participant runs.
pub fn investment_503_unlocks_raw_for(account_soid: u64) -> Vec<u8> {
    // The flag indices are build-specific (see [`UNLOCK_FLAGS_56578`]); the SOID splice is not.
    let template =
        if client_is_56578() { INVESTMENT_503_UNLOCKS_RAW_56578 } else { INVESTMENT_503_UNLOCKS_RAW };
    if account_soid == UNLOCK_ANCHOR_ACCOUNT_SOID {
        return template.to_vec();
    }
    let mut bits = to_bits(template);
    for i in 0..64 {
        bits[INV_503_ACCOUNT_SOID_BIT_OFFSET + i] = account_soid >> (63 - i) & 1 == 1;
    }
    from_bits(&bits)
}

/// ⭐⭐ **Read the flag array back out of a body we are about to send.**
///
/// This is the assertion the startup guard was missing. `check_205_503_agree` proves the two bodies
/// **match**; it has never proved **what they contain**, and two byte-identical *empty* bodies pass
/// it happily - which is exactly what shipped, unnoticed, on every run to date.
///
/// `tail` is the shared `0x80801941` encoding: `body_503[21..]` or `body_205[1..]`. Returns `None`
/// when the tail is too short to hold the count, so "cannot tell" is never reported as "empty".
pub fn unlock_flags_in_shared_tail(tail: &[u8]) -> Option<Vec<(u16, u8)>> {
    let bits = to_bits(tail);
    let count = take_be(&bits, UNLOCK_COUNT_BIT_SHARED, UNLOCK_COUNT_BITS)? as usize;
    let base = UNLOCK_COUNT_BIT_SHARED + UNLOCK_COUNT_BITS;
    (0..count)
        .map(|k| {
            let at = base + k * UNLOCK_ELEMENT_BITS;
            let index = (take_be(&bits, at + 1, 16)? ^ 0x8000) as u16;
            let value = (take_be(&bits, at + 18, 2)?.wrapping_sub(1) & 3) as u8;
            Some((index, value))
        })
        .collect()
}

pub fn investment_504_response_body() -> Vec<u8> {
    // 4-bit leaf = 0, 32-bit leaf = 0, zero-padded to a byte boundary.
    vec![0x18, 0x00, 0x00, 0x00, 0x00]
}

/// The argument bitstream for **investment message 104**, first seen 2026-08-07 at `setup:orbit`
/// (runs `223920-AIDT-0018-ORBIT` and `224651-CLIENTCONFIG-ORBIT` - it appears in no earlier run).
///
/// `[C]` Catalogue row 47: request `0x80801811` (one 0-bit leaf) → response **`0x808018F8`**, a
/// 12-byte / 2-field object whose field [0] is `0x80801796` - **the same one-composite
/// `{u4 @+0x00, u32 @+0x04}` that 504, 701 and 702 all use**. So the body is the familiar five
/// bytes, and the oracle confirms it: `d1-oracle message --tag 0x808018f8 --msgid 104
/// --request-tag 0x80801811` emits `0068` + `1800000000` and decodes it back with `rc=1`.
///
/// # Why this mattered
///
/// Unanswered *ids* were already handled, but 104 fell through the dispatch to `None`, so the reply
/// went out as `class-2 header || u16 msgid` with **no object at all** - a 10-byte plaintext. The
/// client's own complaint was `handle_response_internal: Failed to decode [policy 1]`, policy 1
/// being `client_to_world_server_deterministic_request` (`0x000A`→`0x000B`), i.e. this channel.
/// Note the shape of that evidence: a *decode* complaint means the reply reached the right queue
/// entry and its body was wrong - distinct from `failed to find recipient`, which is ordering.
pub fn investment_104_response_body() -> Vec<u8> {
    investment_504_response_body()
}

/// The argument bitstream for **investment message 701**, the 1092-byte request the client sends at
/// `prepare_for_orbit`.
///
/// `[C]` Definition row 82 gives request `0x808016DD` / response **`0x8080189B`**, and
/// `0x8080189B`'s descriptor is *byte-for-byte the same shape as 504's* - one field of the shared
/// `0x80801796` composite `{u4 @+0x00, u32 @+0x04}`. The oracle confirms it: driving `0x261BE0`
/// with a zero object emits `02bd 18 00 00 00 00` (`0x02BD` = 701) and `0x261FB0` decodes it back to
/// the zero object, `rc=1`. So the body is identical to 504's and the two are pinned equal by test.
///
/// ⚠ **This supersedes `D1_INV_SILENT=701` as the recommended handling**, and the reason matters.
/// §26.51 measured silence-on-701 as "the weasel cure" and it did roughly double the client's
/// survival - but 701 rides envelope type `0x000A`, policy families 0 and 1 both map
/// `0x000A -> 0x000B`, so the client *always* has an in-flight entry waiting. Silence does not
/// discharge that entry; it leaves it in queue 1 until `update_receiving()` (`0xB2D500`) reports
/// `contains timed out elements`. Silence was never a cure, only a slower death - which is exactly
/// why the §26.51 run "held at `prepare_for_orbit`" and then died anyway.
pub fn investment_701_response_body() -> Vec<u8> {
    investment_504_response_body()
}

/// Reply to `0x0015` (`client_to_bap_update_purchased_offers_request`) - class-2 header, **no
/// payload**.
///
/// `[C]` `0x0016`'s body is a **protobuf**, descriptor `0x15AB3E0` - the same `IdList` table as
/// signon response field 10 (`historicaldocs/design/investment-profile-contract.md` §5.1). Walked with
/// `capture-rig/proto-descriptor.py` (whose control reproduces the XETROV descriptor 4/4 against
/// `parse_xetrov_request`), it is exactly one field:
///
/// ```text
/// #1  repeated varint  size 4  cap 64
/// ```
///
/// **No required field**, so a zero-length body is a complete, valid encoding of "the purchased-offer
/// id list is empty" - which is the truth for an account with no entitlements to sync. This is the
/// one case in the whole in-flight queue where header-only is not a guess but the correct answer.
pub fn purchased_offers_response(sequence: u32) -> Vec<u8> {
    reply_header(TYPE_UPO_RESPONSE, sequence)
}

/// The protobuf body for `0x0021` (`bap_to_client_user_message_response`).
///
/// `[C]` Descriptor `0x15AB800`, reached by `0x5699D3: lea rsi,[rip+0x1041E26]` and fed to the
/// generic parser `0xA2020` by `0x0021::decode_payload` (`0x5699A0`). Walked with
/// `capture-rig/proto-descriptor.py`:
///
/// ```text
/// #1  optional string  size 512     <- message text
/// #2  optional string  size 64
/// #3  optional string  size 64
/// #4  REQUIRED varint  size 4
/// ```
///
/// **Independent arithmetic control:** the §2.2 offset walk over those four records yields a struct
/// end of `1 + 512 + 1 + 64 + 1 + 64 + 1 + 4` = **648**, and `0x5699A0` writes exactly
/// `mov dword [r14], 0x288` (= 648) on a successful parse. The walk and the code agree, so the four
/// records are the right bytes.
///
/// ⇒ `[C]` **Field 4 is REQUIRED, so an empty `0x0021` body cannot parse** - the codec has a
/// `missing required field` error and `decode_payload` returns 0 without setting the size. That is
/// the real reason §26.50's header-only `0x0021` was destructive, and it is a stronger statement
/// than the one that experiment supported: §26.50 concluded "`0x0020` has no paired response class",
/// which the class table flatly contradicts (`0x0021`
/// `c_investment_bap_message_bap_to_client_user_message_response` exists, policy family 25 maps
/// `0x0020 -> 0x0021`). ⚠ That run was *also* confounded - `0x000C` was unanswered at the time, so
/// the reply was rejected for being out of order before its body was ever looked at.
///
/// The minimal legal encoding is therefore field 4 alone: key `(4 << 3) | 0` = `0x20`, varint `0`.
///
/// `[U]` **The VALUE of field 4 is unknown** - plausibly a message count or a result enum, and zero
/// is chosen as the natural "nothing to report" for a request that carries no payload at all (the
/// client's `0x0020` is a bodyless 6-byte header). The three optional strings are omitted, which is
/// consistent with "no user message". This is the same standing as 504's zeroed leaves: the
/// *structure* is derived from the client's own table, the *value* is a default.
pub fn user_message_response_body() -> Vec<u8> {
    vec![0x20, 0x00]
}

/// Reply to `0x0020` (`bap_to_client_user_message_request`) - class-2 header then
/// [`user_message_response_body`].
pub fn user_message_response(sequence: u32) -> Vec<u8> {
    let mut r = reply_header(TYPE_USER_MSG_RESPONSE, sequence);
    r.extend_from_slice(&user_message_response_body());
    r
}

/// The protobuf body for `0x0013` (`client_to_bap_client_config_response`).
///
/// # Why this may not be header-only `[C]`
///
/// The response class vtable is `0x1922A60` (reached name → typeinfo `0x1922AA0` → the *single*
/// vtable referencing it, all via relocations; slot `+0x20` is the generic `type_name()`
/// `0x559D30`, which is the control that the right table was read). Slot `+0x18`
/// `decode_payload` = `0x55B860`, which feeds descriptor **`0x15A7BA0`** to the generic protobuf
/// parser `0xA2020` - the same parser `0x0021` uses. Walking that descriptor:
///
/// ```text
/// #1  REQUIRED varint  size 8      <- `cookie`               (%lld)
/// #2  REQUIRED varint  size 4      <- `social_matchmaking_id` (%d)
/// #3  callback bytes   size 16     <- the engine-config delta, OPTIONAL
/// ```
///
/// **Two required fields ⇒ a zero-length body cannot parse**, so this is the `0x0021` case, not the
/// `0x0016` case. The field names come from the client's own diagnostic at `0x16161E0`:
/// `client_config received with cookie=%lld and social_matchmaking_id=%d`.
///
/// # Why field 3 is deliberately omitted
///
/// It is not required, and its callback demands a leading varint `0x123456` or it aborts the entire
/// parse with `callback failed` - see the signon `client_config.3` note in `d1-responder`. Sending a
/// malformed optional field would turn a working reply into a total parse failure.
///
/// # Reach
///
/// `observed-live` for the *values*: this is the same three-field `client_config` we already deliver
/// as signon field 4, and the client has logged `cookie=0 and social_matchmaking_id=0` **185 times**
/// across the run archive without complaint. `[C]` for the structure, from the descriptor walk.
/// One format, one description - if the signon builder changes, this must change with it.
pub fn client_config_response_body(cookie: u64, social_matchmaking_id: u32) -> Vec<u8> {
    fn varint(out: &mut Vec<u8>, mut n: u64) {
        loop {
            let b = (n & 0x7f) as u8;
            n >>= 7;
            out.push(if n != 0 { b | 0x80 } else { b });
            if n == 0 {
                break;
            }
        }
    }
    let mut b = Vec::new();
    b.push(1 << 3); // field 1, wire type 0
    varint(&mut b, cookie);
    b.push(2 << 3); // field 2, wire type 0
    varint(&mut b, social_matchmaking_id as u64);
    b
}

/// Reply to `0x0012` (`client_to_bap_client_config_request`) - class-2 header then
/// [`client_config_response_body`].
pub fn client_config_response(sequence: u32, cookie: u64, social_matchmaking_id: u32) -> Vec<u8> {
    let mut r = reply_header(TYPE_CLIENT_CONFIG_RESPONSE, sequence);
    r.extend_from_slice(&client_config_response_body(cookie, social_matchmaking_id));
    r
}

/// How many ids the `0x0017` request is asking us to translate, and in which direction.
///
/// Returns `(count, from_kind, to_kind)`. The body's first four bytes are the same header the
/// response uses - `u16 count`, `u8 from_kind`, `u8 to_kind` - after which the request carries the
/// platform ids themselves (fixed-width, zero-padded text; the real capture held `"D1Guardian"`).
///
/// `[C]` Read out of the decoder `0x55A490`, which reads exactly `0x10`, `8`, `8` bits in that order
/// via the bitstream reader `0x72C70`.
pub fn account_id_translation_request(body: &[u8]) -> Option<(u16, u8, u8)> {
    if body.len() < 4 {
        return None;
    }
    let count = u16::from_be_bytes([body[0], body[1]]);
    let (kind_a, kind_b) = (body[2], body[3]);
    // The decoder rejects anything outside {7, 0xFF} for BOTH discriminators - `cmp r12d,7` /
    // `cmp r12d,0xff` / `jne <reject>` at 0x55A679, and the same pair for r13d at 0x55A68C.
    let valid = |k: u8| k == AIDT_KIND_PLATFORM_ID || k == AIDT_KIND_ACCOUNT_SOID;
    (valid(kind_a) && valid(kind_b)).then_some((count, kind_a, kind_b))
}

/// The 36-byte platform identifiers the `0x0017` request is asking us to translate.
///
/// List A of the request body, per the same grammar the response uses - `count` entries of
/// `AIDT_PLATFORM_ID_BITS` (0x120) bits each, immediately after the 4-byte header.
pub fn account_id_translation_request_ids(body: &[u8]) -> Vec<[u8; AIDT_PLATFORM_ID_LEN]> {
    let Some((count, kind_a, _)) = account_id_translation_request(body) else {
        return Vec::new();
    };
    if kind_a != AIDT_KIND_PLATFORM_ID {
        return Vec::new();
    }
    (0..count as usize)
        .map(|i| 4 + i * AIDT_PLATFORM_ID_LEN)
        .take_while(|&off| off + AIDT_PLATFORM_ID_LEN <= body.len())
        .map(|off| {
            let mut id = [0u8; AIDT_PLATFORM_ID_LEN];
            id.copy_from_slice(&body[off..off + AIDT_PLATFORM_ID_LEN]);
            id
        })
        .collect()
}

/// The full `0x0018` reply: class-2 header echoing the request's sequence, then a
/// [`platform_to_investment_response`] body mapping each requested identifier to a SOID.
///
/// ⚠ **The body encoder is [`platform_to_investment_response`], which already existed and is the
/// authority on this grammar.** It is *two parallel lists* (identifiers, then SOIDs), not a bare
/// SOID array - `0x55A8E3` walks list A comparing each identifier against the client's own to find
/// its row, so dropping list A leaves the client unable to locate itself. Do not re-derive it here.
///
/// The sequence echo is what puts this reply on the right in-flight queue entry; getting it wrong
/// reproduces the `failed_to_find_message_recipient` this message exists to cure.
pub fn account_id_translation_response(
    sequence: u32,
    entries: &[([u8; AIDT_PLATFORM_ID_LEN], u64)],
) -> Vec<u8> {
    let mut r = reply_header(TYPE_ACCOUNT_ID_XLAT_RESPONSE, sequence);
    r.extend_from_slice(&platform_to_investment_response(entries));
    r
}

pub fn investment_503_account_soid_body(account_soid: u64) -> Vec<u8> {
    investment_503_fields(&[(account_soid, 64)])
}

/// Generic 503 argument bitstream: a run of **present-flagged** fields of arbitrary bit width.
///
/// `[C]` §15: `present=1 · u64 · <further optional fields>` decodes without complaint, so the first
/// field is accepted and the remainder may be left absent (zeros). This lets us bisect *which*
/// fields the action graph needs without hand-decoding the descriptor - each candidate is one
/// unattended ~4-minute run against the §13 feedback channel.
///
/// Each `(value, bits)` emits `1` then `bits` of `value`, MSB first. Zero-padded to a byte boundary.
pub fn investment_503_fields(fields: &[(u64, u32)]) -> Vec<u8> {
    let mut bits: Vec<bool> = Vec::with_capacity(8 + fields.len() * 72);
    for &(value, width) in fields {
        bits.push(true); // present
        for i in (0..width).rev() {
            bits.push((value >> i) & 1 == 1);
        }
    }
    while bits.len() % 8 != 0 {
        bits.push(false);
    }
    bits.chunks(8)
        .map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | b as u8))
        .collect()
}

/// A message-503 response carrying a real `platform_to_investment_response` body.
///
/// Same envelope as [`investment_message_response_padded`], but the bytes after the `u16` message
/// id are the encoded translation rather than zero padding.
pub fn investment_message_response_with_body(
    sequence: u32,
    message_id: u16,
    status: u16,
    body: &[u8],
) -> Vec<u8> {
    let mut p = Vec::with_capacity(REPLY_HEADER_LEN + 2 + body.len());
    p.extend_from_slice(&TYPE_CLIENT_TO_WORLD_SERVER_RESPONSE.to_be_bytes());
    p.extend_from_slice(&sequence.to_be_bytes());
    p.extend_from_slice(&status.to_be_bytes());
    p.extend_from_slice(&message_id.to_be_bytes());
    p.extend_from_slice(body);
    p
}

/// Native-oracle-verified response for investment message 206.
///
/// Definition row 206 maps request tag `0x80801774` to response tag `0x80801869`. The response C
/// object is one byte. Setting that byte to zero, running client encoder `0x261be0`, decoding through
/// `0x261fb0`, and re-encoding produces the fixed point `00ce 10`; therefore the body following the
/// echoed message id is exactly `0x10`. This is not hand-fitted from the failed live run.
pub fn investment_206_response(sequence: u32, status: u16) -> Vec<u8> {
    investment_message_response_with_body(
        sequence,
        INVESTMENT_MESSAGE_ID_206,
        status,
        &fetch_family_response_body(None),
    )
}

// ───────────────────────── investment 206 `fetch_family` - request and reply ─────────────────────

/// The client's **`fetch_family`** request - investment message **206**.
///
/// Request schema `0x80801774` is `struct 16 bytes, 2 fields`:
///
/// ```text
///   [0] @+0x000  kind 0x3  LEAF  4 bits     the FAMILY
///   [1] @+0x008  kind 0x6  LEAF 64 bits     the family ROOT key (the account SOID)
/// ```
///
/// so the whole argument bitstream is 68 bits = 9 bytes on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FetchFamilyRequest {
    /// The QueueZ family the client is asking us to deliver. **Already un-biased** - see
    /// [`fetch_family_request`].
    pub family: u32,
    /// The family root the client keyed the request by. Every observation to date is the account
    /// SOID.
    pub key: u64,
}

/// Decode the 9-byte argument bitstream of investment **206** `fetch_family`.
///
/// # ⛔ The 4-bit leaf is BIASED by +1, and reading it raw gives the wrong family
///
/// `historicaldocs/design/CREATION-BUILD-ORDER.md` §3a stated `family = v >> 68` and tabulated
/// `4d 10 …` as *family 4* (boot) against `3d 10 …` as *family 3* (creation finish), "one nibble
/// apart". **Both rows are wrong.** Driving the client's own encoder over the request tag for
/// families 0..4 emits wire nibbles **1/2/3/4/5**:
///
/// ```text
///   d1-oracle message --tag 0x80801774 --msgid 206 --obj 03…  ->  00ce 4d10000000000000 10
///   d1-oracle message --tag 0x80801774 --msgid 206 --obj 04…  ->  00ce 5d10000000000000 10
/// ```
///
/// This is the same +1 bias `character_lifecycle::RESULT_OK_BODY` documents for its own 4-bit leaf:
/// `0` on the wire would mean `-1`. So `family = leaf - 1`, and a leaf of `0` is not family 0 - it
/// is a malformed request, which is why this returns `None` for it rather than saturating.
///
/// # What the census says the client actually asks for `[C]`
///
/// **195 arrivals across 162 runs, and every single one is byte-identical
/// `4d 10 00 00 00 00 00 00 10`** - boot-time and creation-finish alike. So every 206 ever observed
/// is `fetch_family{family = 3, key = 0xD100000000000001}`, a **family-4 fetch has never been
/// seen**, and there is no nibble discriminator between the two moments to log.
pub fn fetch_family_request(args: &[u8]) -> Option<FetchFamilyRequest> {
    // 4-bit leaf + 64-bit leaf = 68 bits. Nine bytes is the minimum that can hold them; a shorter
    // buffer must be refused rather than zero-extended, because "read zeros past the end" is the
    // §26.67 under-length trap and it decodes as a perfectly plausible `family 0, key 0`.
    if args.len() < 9 {
        return None;
    }
    let leaf = args[0] >> 4;
    if leaf == 0 {
        return None; // biased by +1: a zero leaf encodes -1, i.e. not a family at all
    }
    let mut key = 0u64;
    for i in 0..64 {
        let bit = 4 + i;
        key = (key << 1) | ((args[bit / 8] >> (7 - (bit % 8))) & 1) as u64;
    }
    Some(FetchFamilyRequest { family: (leaf - 1) as u32, key })
}

/// Build the body of a **206** reply, optionally carrying a QueueZ family delivery as a **span**.
///
/// # Why the payload cannot ride in a schema field
///
/// Both the response tag `0x80801869` and its base `0x8080175F` are `struct 1 byte, 1 field,
/// LEAF 4 bits`. There is nowhere in the schema to put a family. The delivery therefore rides at
/// the **wrapper** level, and the wrapper layout - read back out of the client's own encoder - is
///
/// ```text
///   [4 bits]  the response leaf, biased +1 (so `1` on the wire = 0)
///   [1 bit ]  span present
///   [16 bits] span length, big-endian
///   [n bytes] the span
///                                          … all MSB-first, zero-padded to a byte boundary.
/// ```
///
/// ⚠ **It is BIT-PACKED, not concatenated.** The presence bit sits at bit 4 of the body, so the
/// length and every span byte straddle byte boundaries - which is exactly why every
/// `D1_206_TAIL`-style append experiment failed: `10 || tail` cannot express a span.
///
/// `None` reproduces the historical `0x10` fixed point byte for byte.
pub fn fetch_family_response_body(span: Option<&[u8]>) -> Vec<u8> {
    let mut bits: Vec<bool> = Vec::with_capacity(24 + span.map_or(0, |s| s.len() * 8));
    for i in (0..4).rev() {
        bits.push((1u64 >> i) & 1 == 1); // the 4-bit leaf at its zero value, biased +1
    }
    match span {
        None => bits.push(false),
        Some(s) => {
            bits.push(true);
            for i in (0..16).rev() {
                bits.push(((s.len() as u64) >> i) & 1 == 1);
            }
            for &b in s {
                for i in (0..8).rev() {
                    bits.push((b >> i) & 1 == 1);
                }
            }
        }
    }
    pack_msb_first(bits)
}

/// The inverse of [`fetch_family_response_body`] - pull the span back out of a 206 body.
///
/// This exists so that a **raw `D1_206_BODY` override can be checked against the request it is
/// about to answer** instead of being trusted. See [`queuez_span_family`].
pub fn fetch_family_response_span(body: &[u8]) -> Option<Vec<u8>> {
    if body.is_empty() || body[0] >> 4 == 0 {
        return None;
    }
    let bit = |n: usize| -> Option<bool> {
        body.get(n / 8).map(|b| (b >> (7 - (n % 8))) & 1 == 1)
    };
    if !bit(4)? {
        return None; // no span attached
    }
    let mut len = 0usize;
    for i in 0..16 {
        len = (len << 1) | bit(5 + i)? as usize;
    }
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let mut v = 0u8;
        for j in 0..8 {
            v = (v << 1) | bit(21 + i * 8 + j)? as u8;
        }
        out.push(v);
    }
    Some(out)
}

/// The FAMILY a QueueZ span is a delivery for, read out of its own header.
///
/// Header layout is [`queuez_family_header_for`]: `u32 family_count | u32 family | u64 root |
/// u32 version | u8 flags | u32 object_count`. Returns `None` unless the span is a single-family
/// transaction, because that is the only shape this crate emits and a multi-family span would need
/// a different guard, not a guess.
pub fn queuez_span_family(span: &[u8]) -> Option<u32> {
    if span.len() < 8 || u32::from_be_bytes(span[0..4].try_into().ok()?) != 1 {
        return None;
    }
    Some(u32::from_be_bytes(span[4..8].try_into().ok()?))
}

/// Byte length of the single-family span header this crate emits.
///
/// ⚠ **25, not 21.** The client reads `0xa8` bits = **21 bytes** for the per-family struct
/// (`FUN_000730b0(bs, &hdr, 0xa8)`, first instruction of `0x565140`); the leading `family_count`
/// sits OUTSIDE that struct and is consumed by the `0x007B` dispatcher before it. Any builder that
/// loses the split desynchronises the whole stream **silently**.
pub const QUEUEZ_SPAN_HEADER_LEN: usize = 25;

/// Byte length of one object header inside a span: `u32 instance_type | u64 key | u32 payload_len`.
/// The client reads `0x80` bits = 16 bytes each (`0x565140`'s object-header preview loop).
pub const QUEUEZ_OBJECT_HEADER_LEN: usize = 16;

/// The family VERSION a span declares - `0x56AF60` compares it against the accepted version it
/// already holds for this family (`descriptor + 0x3C`). See [`queuez_span_as_delta`].
pub fn queuez_span_version(span: &[u8]) -> Option<u32> {
    queuez_span_family(span)?;
    Some(u32::from_be_bytes(span.get(16..20)?.try_into().ok()?))
}

/// The span's FLAGS byte. Bit 0 is [`QUEUEZ_FLAG_BASELINE`]; clear means [`QUEUEZ_FLAG_DELTA`].
pub fn queuez_span_flags(span: &[u8]) -> Option<u8> {
    queuez_span_family(span)?;
    span.get(20).copied()
}

/// The span's declared `object_count`.
pub fn queuez_span_object_count(span: &[u8]) -> Option<u32> {
    queuez_span_family(span)?;
    Some(u32::from_be_bytes(span.get(21..25)?.try_into().ok()?))
}

/// One object located inside a single-family span, by walking the object headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueuezObject {
    pub instance_type: u32,
    pub key: u64,
    /// Byte range of the PAYLOAD within the span. Its 16-byte header is `start - 16 .. start`.
    pub payload: (usize, usize),
}

/// Walk a single-family span's objects. `None` if the span is malformed or truncated - a partial
/// walk is worse than no walk, because every offset after the break is wrong but plausible.
pub fn queuez_span_objects(span: &[u8]) -> Option<Vec<QueuezObject>> {
    let count = queuez_span_object_count(span)?;
    let mut out = Vec::with_capacity(count as usize);
    let mut at = QUEUEZ_SPAN_HEADER_LEN;
    for _ in 0..count {
        let hdr = span.get(at..at + QUEUEZ_OBJECT_HEADER_LEN)?;
        let len = u32::from_be_bytes(hdr[12..16].try_into().ok()?) as usize;
        let start = at + QUEUEZ_OBJECT_HEADER_LEN;
        let end = start.checked_add(len)?;
        if end > span.len() {
            return None;
        }
        out.push(QueuezObject {
            instance_type: u32::from_be_bytes(hdr[0..4].try_into().ok()?),
            key: u64::from_be_bytes(hdr[4..12].try_into().ok()?),
            payload: (start, end),
        });
        at = end;
    }
    (at == span.len()).then_some(out)
}

/// ⭐⭐ Re-stamp a span this crate already built as a **DELTA at `version`**, leaving every object
/// byte-for-byte unchanged.
///
/// # Why re-stamping, and not a parallel builder `[C]`
///
/// CLAUDE.md's rung-34 task-8 rule: *when a re-publish path rebuilds an object, every field that is
/// OURS to assert must be re-applied there too - enumerate, do not spot-check.* That bug cost a
/// session because `RequestStateRefresh` rebuilt `replicate_membership` from the client's echo and
/// silently dropped a field. **The way to be immune to it is to not rebuild.** This copies the
/// objects verbatim and touches four version bytes and one flags byte, so there is nothing to
/// enumerate: a field cannot be dropped by a memcpy.
///
/// # Why a DELTA and not a repeated baseline `⛔`
///
/// `FUN_0056AF60` (`queuez_family_update_begin`), family at replication state **3**:
///
/// ```text
///   flags & 1  (BASELINE) -> FUN_0056B2D0(...)  ; PURGE to the delivered keep-list, then accept
///   !(flags&1) (DELTA)    -> accept iff version == descriptor[+0x3C] + 1
/// ```
///
/// `0x56B2D0` walks the family's six type slots and, for every occupied slot whose
/// `{instance_type, soid}` is **not** in the delivered object-header keep-list, memsets it to zero
/// and fires the DELETE announcer `0x56E7B0`. Our family-4 delivery carries 4/0, 4/1 and
/// (optionally) 4/3 and 4/4 - so a repeated baseline would **delete the client's 4/2 display-name
/// object and any 4/3 item it holds that we did not re-list.** A delta never enters `0x56B2D0`.
///
/// # What it is FOR - the availability cache `[C]`
///
/// On accept, `0x565140` calls `0x56B5E0` **unconditionally** (`0x565569`), and `0x56B5E0` is four
/// instructions with no prologue: `mov edi,esi / mov rsi,rdx / mov rdx,r9 / jmp 0x56D0F0`.
/// `0x56D0F0` calls `0x56D1A0` at `0x56D113` with `edi` untouched, and `0x56D1A0` opens
/// `if (family == 4) { …; FUN_00574440(4); … }`. `0x574440` is
/// `cmp edi,4 / lea rdi,[0x26A34B0] / call [vtable+0xC8] / mov byte [rax],0` - it clears the
/// availability cache's **validity byte** at `[0x26A34B0+0x38]+8`, which is the only thing that
/// makes `0x25FF40` rebuild the `0x4000`-byte evaluation plane the unlock evaluator reads.
///
/// ⭐ **The per-object classifier `0x56B5F0` is NOT on that path** - it hangs off `0x5655B0`. So the
/// invalidation fires for an accepted delta *whatever* the objects say, including `object_count = 0`
/// ([`queuez_family4_delta_ping`]). A "changed byte" is not required for it and this crate does not
/// invent one; see [`account_rsat_with_serial`] for the opt-in that does.
///
/// Returns `None` unless `span` is a single-family span this crate could have emitted.
pub fn queuez_span_as_delta(span: &[u8], version: u32) -> Option<Vec<u8>> {
    queuez_span_objects(span)?; // structural guard: refuse to re-stamp bytes we cannot walk
    let mut out = span.to_vec();
    out[16..20].copy_from_slice(&version.to_be_bytes());
    out[20] = QUEUEZ_FLAG_DELTA;
    Some(out)
}

/// Replace object `index`'s payload in a single-family span, leaving every other byte unchanged.
///
/// `expect_instance_type` is a guard, not documentation: object 0 of a family-4 delivery is the
/// account today, but `build_family_body`'s overrides can change what is emitted, and splicing a
/// re-encoded account payload over a *profile* object would produce a span that decodes cleanly and
/// means something else entirely. Returns `None` if the walk fails or the guard does not hold.
pub fn queuez_span_replace_object_payload(
    span: &[u8],
    index: usize,
    expect_instance_type: u32,
    payload: &[u8],
) -> Option<Vec<u8>> {
    let objs = queuez_span_objects(span)?;
    let obj = objs.get(index)?;
    if obj.instance_type != expect_instance_type {
        return None;
    }
    let (start, end) = obj.payload;
    let mut out = Vec::with_capacity(span.len() - (end - start) + payload.len());
    out.extend_from_slice(&span[..start - 4]); // up to, but not including, the payload LENGTH
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    out.extend_from_slice(&span[end..]);
    Some(out)
}

// ─────────────────────────── QueueZ server objects - the character ──────────────────────────

/// Server→client QueueZ object push. This is how an **existing character** is delivered.
///
/// `[R]` 2026-08-06, `historicaldocs/design/rsat-character-object.md`. The character list is *not* produced by
/// answering `login_character` (501); the client is handed a family-3 QueueZ object, and
/// `character:signin` then sees count 1 and takes character **selection** rather than creation.
pub const TYPE_QUEUEZ_OBJECT: u16 = 0x007b;

/// Family 3, slot 1 - the only object kind that family accepts. Mapper at `0x56C550`.
///
/// ⚠ **Downstream of family 4.** `0x584EB0` subscribes the account root to **family 4 first, then
/// family 3**, so family 3 is character-list state and is *not* what clears `investment_signin`.
pub const QUEUEZ_FAMILY_CHARACTER: u32 = 3;

/// Family 4 - the **account** family, and the one the boot gate actually needs. `[R]`
///
/// Slot 0 is the boot-critical account object: `0x583450` hardcodes family 4 / type-index 0; the
/// `0x56C550` mapper gives capacity 1 and decoded size `0x7338`; the registry at `0x1970970` pairs it
/// with checksum [`ACCOUNT_INSTANCE_TYPE`], and its static schema record is `0x12FA3F8`.
///
/// Slot 1 is resolved separately by `0x5833C0` and feeds the selected-character profile path, not
/// the immediate account gate.
pub const QUEUEZ_FAMILY_ACCOUNT: u32 = 4;

/// Instance checksum of the family-4 slot-0 account object. `[R]`
pub const ACCOUNT_INSTANCE_TYPE: u32 = 0x64AE_75BF;
/// Instance checksum of the character schema (`0x1350758`), paired in the registry at `0x196F1F0`.
pub const CHARACTER_INSTANCE_TYPE: u32 = 0x3431_F1BA;
/// Instance checksum of the **family-3 root** - the character *roster*. Schema record `0x139ADD8`,
/// descriptor `0x139AE28`, decoded size `0xA0`, capacity 1.
///
/// ⚠ "Family 3 type 0" is not a distinct type: the `0x56C550` mapper routes every type index except
/// 1 to a single default leaf, which *is* the family root. See `design/eboot-map.md` §3.
pub const ROSTER_INSTANCE_TYPE: u32 = 0xA2AC_2C74;
/// Instance checksum of the family-4 slot-1 **profile** object. Schema record `0x1304808`.
pub const PROFILE_INSTANCE_TYPE: u32 = 0x8127_F15A;

/// Build 56578 (01.33) account checksum: wrapper tag `0x80801803` (anchor `0x130A440`), inner
/// `0x808018C7` - still 25 top-level fields with the same leading fields, struct `0x75C8`
/// (01.29: `0x7338`). Read from the 56578 descriptor at `anchor - 0x50 + 8`.
pub const ACCOUNT_INSTANCE_TYPE_56578: u32 = 0x6B30_D2B4;
/// Build 56578 profile checksum: wrapper tag `0x80801743` (anchor `0x12E7250`), inner
/// `0x80801714` - still 18 top-level fields, SOID first, struct `0x7378` (01.29: `0x7078`).
pub const PROFILE_INSTANCE_TYPE_56578: u32 = 0x003B_C6E5;

static CLIENT_BUILD_56578: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Record the client build from its SignOn request (`build=` query). Only the account and profile
/// checksums differ between 01.29 and 56578; the roster, character and family-4/4 objects, and the
/// minimal encodings of account and profile, are unchanged (measured on both descriptor tables).
pub fn set_client_build(build: &str) {
    CLIENT_BUILD_56578.store(build.starts_with("56578."), std::sync::atomic::Ordering::Relaxed);
}

/// Whether the connected client reported build 56578 at SignOn.
pub fn client_is_56578() -> bool {
    CLIENT_BUILD_56578.load(std::sync::atomic::Ordering::Relaxed)
}

/// The account instance checksum for the connected client's build.
pub fn account_instance_type() -> u32 {
    if client_is_56578() { ACCOUNT_INSTANCE_TYPE_56578 } else { ACCOUNT_INSTANCE_TYPE }
}

/// The profile instance checksum for the connected client's build.
pub fn profile_instance_type() -> u32 {
    if client_is_56578() { PROFILE_INSTANCE_TYPE_56578 } else { PROFILE_INSTANCE_TYPE }
}

/// Top-level field count of the **account** schema (`0x1322B68`), needed for the absent-field tail.
const ACCOUNT_FIELD_COUNT: u32 = 25;
/// Top-level field count of the **roster** schema (`0x139AE28`).
const ROSTER_FIELD_COUNT: u32 = 4;
/// `flags = 0x01` - baseline (as opposed to an incremental update). Confirmed.
pub const QUEUEZ_FLAG_BASELINE: u8 = 0x01;

/// `flags = 0x00` - a **DELTA** family update. `0x56AF60` branches on `flags & 1` alone
/// (`0x56AFE8 test byte [r15], 1`), so this is "not a baseline" rather than a distinct opcode.
///
/// ⚠ At replication state **2** a delta is refused - *"ignoring delta-update due to lack of
/// baseline"* - and at state **3** it is accepted only when its version is exactly the accepted
/// version plus one. See [`queuez_span_as_delta`].
pub const QUEUEZ_FLAG_DELTA: u8 = 0x00;

/// Identity triple as it is ordered **on the wire**: `{race, gender, class}` at object `+0x08`.
///
/// ⚠ Not the `{class, race, gender}` order the public manifest lists them in. Locked for this
/// project: Titan / Human / Female ⇒ `{0, 1, 0}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    pub race: u8,
    pub gender: u8,
    pub class: u8,
}

impl Identity {
    /// Human / Female / Titan.
    pub const TITAN_HUMAN_FEMALE: Identity = Identity { race: 0, gender: 1, class: 0 };
}

/// Encode the character RSAT payload - the bit grammar the client's **own** serializer emitted.
///
/// Produced by driving the generic RSAT writer at `0x105B410` against the static character
/// descriptor at `0x13507A8` under the Unicorn harness (`capture-rig/oodle/`), *not* guessed. Bit
/// grammar, then zero-padded to a byte boundary:
///
/// ```text
/// 1 · u64be(soid) · 1 · 1+u8(race+0x80) · 1+u8(gender+0x80) · 1+u8(class+0x80) · 0000
/// ```
///
/// The reference vector for SOID `0xD100000000000101` + Titan/Human/Female is
/// `e880000000000080f0181c0000` (13 bytes), which this reproduces exactly - see the unit test.
pub fn character_rsat(soid: u64, id: Identity) -> Vec<u8> {
    let mut bits: Vec<bool> = Vec::with_capacity(128);
    let push = |bits: &mut Vec<bool>, v: u64, n: u32| {
        for i in (0..n).rev() {
            bits.push((v >> i) & 1 == 1);
        }
    };
    bits.push(true); // present
    push(&mut bits, soid, 64);
    bits.push(true); // identity block present
    for v in [id.race, id.gender, id.class] {
        bits.push(true);
        push(&mut bits, (v as u64) + 0x80, 8);
    }
    push(&mut bits, 0, 4); // terminator
    while bits.len() % 8 != 0 {
        bits.push(false); // zero-pad to a byte boundary
    }
    bits.chunks(8)
        .map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | b as u8))
        .collect()
}

/// Encode a `{ SOID, character list }` object - the shape **both** the roster and the account use.
///
/// # There are two character lists, and they gate different things
///
/// The roster (family-3 root) and the account (family 4 / type 0) carry the *identical* schema child
/// `0x80804621` = `{ u4 count @+0x00 ; u64 storage[10] @+0x08 }` as their field 1, on top of a
/// 64-bit SOID as field 0. So both decode to `{ +0x00 soid, +0x08 count, +0x10 u64[] }` - but
/// different code reads each:
///
/// | object | reader | decides |
/// |---|---|---|
/// | roster, family-3 root | `0x78C9F0` | character CREATION vs SELECTION |
/// | account, family 4/0 | `0xA110F0` | how many selection **cards** actually get built |
///
/// `0xA110F0` is explicit about it:
///
/// ```text
/// 0a112cd  cmp  dword ptr [rbx + 8], 0              ; rbx = the family-4/type-0 account object
/// 0a112d1  jle  0xa116bb                            ; count <= 0  -> build ZERO cards
/// 0a112f0  mov  rax, qword ptr [rbx + r12*8 + 0x10] ; SOID = array[i]
/// 0a1132b  call 0x56ca70                            ; -> the character for that SOID
/// 0a116a3  mov  rbx, qword ptr [rbp - 0x738]        ; reload the SAME account object
/// ```
///
/// Filling only the roster therefore produces the selection screen **with no guardian on it** -
/// which is exactly what the operator saw on 2026-08-07.
///
/// # Grammar
///
/// ```text
/// 1 · u64be(key)                          field 0 present
/// 1 · u4(count) · count × u64be(soid)     field 1 present
/// 0 × (field_count - 2)                   every remaining top-level field absent
/// ```
///
/// then zero-padded to a byte boundary. `trailing_absent` is `field_count - 2`; it differs between
/// the two objects (roster 4 fields, account 25) and getting it wrong yields a payload the client
/// reads **past the end of** - see the note on the short-payload trap in the tests.
///
/// Verified by decoding through the client's own codec (`0x2C9000`, driven by
/// `capture-rig/oodle/queuez-object-oracle.py`), not by analogy.
pub fn character_list_rsat(key: u64, characters: &[u64], trailing_absent: u32) -> Vec<u8> {
    let mut bits = character_list_bits(key, characters);
    for _ in 0..trailing_absent {
        bits.push(false);
    }
    pack_msb_first(bits)
}

/// Fields 0 and 1 only - the key and the `{u4 count; u64 storage[10]}` composite - with **no**
/// trailing absent bits. Split out so the account can append its own field 3 (the profile SOID)
/// while the roster keeps padding straight to its 4-field count. See [`account_rsat`].
fn character_list_bits(key: u64, characters: &[u64]) -> Vec<bool> {
    assert!(characters.len() <= 10, "storage[10] - the schema holds at most 10 characters");
    let mut bits: Vec<bool> = Vec::with_capacity(256);
    let push = |bits: &mut Vec<bool>, v: u64, n: u32| {
        for i in (0..n).rev() {
            bits.push((v >> i) & 1 == 1);
        }
    };
    bits.push(true); // field 0 present
    push(&mut bits, key, 64);
    bits.push(true); // field 1 present
    push(&mut bits, characters.len() as u64, 4);
    for &c in characters {
        push(&mut bits, c, 64);
    }
    bits
}

/// Zero-pad to a byte boundary and pack MSB-first - the bit order the client's own codec reads.
fn pack_msb_first(mut bits: Vec<bool>) -> Vec<u8> {
    while bits.len() % 8 != 0 {
        bits.push(false);
    }
    bits.chunks(8)
        .map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | b as u8))
        .collect()
}

/// The family-4 **account** RSAT: the character list **and the profile SOID**.
///
/// # Why field 3 exists - it is the `character:signin` gate `[C]`
///
/// `character:signin` (handler `0x7A0160`) exits only when the ladder `0x78C5F0` returns 6, and its
/// rung 3 requires `0x5853C0` - `qword [selection_node + 0x20]` - to be non-null. The **only**
/// writer of that qword is `0x585820`, and it sources the value like this:
///
/// ```text
///   0x585840  cmp byte [rbx], 3          ; the node must be the tag-3 (character-selected) variant
///   0x585873  cmp qword [rbx+0x20], 0    ; ... and slot B still empty
///   0x58587d  call 0x5834A0(&slot_A)     ; slot A -> registry(family 4, type 0) = THE ACCOUNT
///   0x585885  cmp qword [r14+0x68], 0    ; <-- account + 0x68 MUST BE NON-ZERO
///   0x585893  call 0x5833C0(&account+0x68) ; ... and must resolve as (family 4, type 1) = PROFILE
///   0x58589f  mov qword [rbx+0x20], rax  ; slot B = *(account + 0x68)
/// ```
///
/// The two resolvers are unambiguous because of their constant thunks: `0x5834A0` calls
/// `0x56CA70(reg, 4, 0x1DF0()=0, key)` and `0x5833C0` calls `0x56CA70(reg, 4, 0x1048A40()=1, key)`.
/// So `account + 0x68` holds a **SOID that must name a live profile object** - not a pointer.
///
/// Schema `0x808018C7` (25 fields) puts a 64-bit leaf at exactly that offset:
///
/// ```text
///   [0] @+0x000  LEAF 64 bits        account SOID
///   [1] @+0x008  -> 0x80804621       {u4 count; u64 storage[10]}, 88 bytes
///   [2] @+0x060  LEAF 32 bits        absent - not known to be needed
///   [3] @+0x068  LEAF 64 bits        <-- THE PROFILE SOID, the slot-B source
/// ```
///
/// Verified by decoding the emitted payload through the client's own codec, not by analogy: the
/// decoded 29,496-byte object has exactly four non-zero qwords, at `+0x00`, `+0x08`, `+0x10` and
/// `+0x68`. Before this, every account we sent left `+0x68` zero, so `0x585820` returned at its
/// first check and the gate could only ever return 3.
pub fn account_rsat(account_soid: u64, characters: &[u64], profile_soid: u64) -> Vec<u8> {
    account_rsat_with_serial(account_soid, characters, profile_soid, 0)
}

/// [`account_rsat`] with account schema field **[2]** - the 32-bit leaf at object `+0x60` - set to
/// `serial`. `serial == 0` emits the field ABSENT and is byte-identical to [`account_rsat`].
///
/// # What it is for: forcing the classifier to announce an UPDATE `[C]`
///
/// `0x56B5F0` decides what a re-delivered object *is*, and it decides on the **decoded** object:
///
/// ```text
///   lVar6 = *plVar7;                                  ; slot's first qword, BEFORE the decode
///   if (lVar6) memcpy(old, plVar7, stride);           ; stride from the slot descriptor +0x08
///   FUN_002C9000(...);                                ; decode into the slot
///   if (*plVar7 == header_soid) {
///       if (lVar6 == 0)                    FUN_0056D290(...)   ; CREATE
///       else if (memcmp(old, plVar7, stride)) FUN_0056D3F0(...); UPDATE
///   }                                                          ; equal -> NOTHING is announced
/// ```
///
/// So a re-sent account that decodes to the same 29,496 bytes is classified as **neither**. This
/// field is the smallest change that makes the memcmp differ, measured through the client's own
/// codec (`d1-oracle object --tag 0x808018c7 --decode`) rather than by analogy:
///
/// ```text
///   serial 0 : 221 bits -> +0x0000 +0x0008 +0x0010 +0x0068
///   serial 7 : 253 bits -> +0x0000 +0x0008 +0x0010 +0x0060 = 7  +0x0068
/// ```
///
/// One 32-bit word differs; every other decoded qword is identical.
///
/// ⚠⚠ **`[U]` THAT `+0x60` IS INERT, AND THIS IS THEREFORE DEFAULT-OFF.** The account object is an
/// indexed heap element reached through `find(4, 0, key)` (`0x56CA70`), so neither `d1-eboot xref`
/// (address-based) nor `field-xref` (register-relative on a known class) can enumerate its readers -
/// the honest instrument statement, not an omission. What *is* measured is a one-hop bound: none of
/// the **seven** direct callers of the account accessor `0x5834A0` (`0x5716C0`, `0x57D5F0`,
/// `0x57D6A0`, `0x585820`, `0x586540`, `0x5896C0`, `0x78C5F0`) dereferences `+0x60`. That is a
/// bounded negative, not proof: the pointer escapes into further calls.
///
/// ⭐ **The availability-cache invalidation does not need this.** `0x574440(4)` is reached from the
/// family-update *end* chain, which never consults the classifier - see [`queuez_span_as_delta`].
/// Ship the serial only when the question is specifically *"does the client announce an UPDATE"*.
pub fn account_rsat_with_serial(
    account_soid: u64,
    characters: &[u64],
    profile_soid: u64,
    serial: u32,
) -> Vec<u8> {
    let mut bits = character_list_bits(account_soid, characters);
    // field 2 - the u32 at +0x60. Absent at serial 0, so the default path is unchanged: a
    // *present* field carrying zero decodes to the same object but is different WIRE bytes, and
    // "same decode, different wire" is exactly the distinction this crate keeps getting wrong.
    if serial == 0 {
        bits.push(false);
    } else {
        bits.push(true);
        for i in (0..32).rev() {
            bits.push((serial >> i) & 1 == 1);
        }
    }
    bits.push(true); // field 3 present
    for i in (0..64).rev() {
        bits.push((profile_soid >> i) & 1 == 1);
    }
    for _ in 0..(ACCOUNT_FIELD_COUNT - 4) {
        bits.push(false);
    }
    pack_msb_first(bits)
}

/// ⭐⭐ The family-4 account RSAT **with field 3 ABSENT** - the one shape that leaves the
/// `character:signin` slot-B gate unsatisfied, which is what lets the client open its CREATOR.
///
/// # Why this has to exist as its own builder `[C]`
///
/// [`account_rsat`] always emits field 3, so `account + 0x68` always names a profile, so
/// `0x585820` always fills slot B:
///
/// ```text
///   0x585885  cmp qword [r14+0x68], 0        ; slot B source - ZERO means "give up here"
///   0x585893  call 0x5833C0(&account+0x68)   ; ... find(family 4, TYPE 1, key = *(acct+0x68))
///   0x58589f  mov qword [rbx+0x20], rax      ; slot B = the profile
/// ```
///
/// and the moment slot B resolves, ladder `0x78C5F0` rung 3 returns **6**, which
/// `c_world_controller_state_character_signin_handler::vfunc[7]` reads at `0x7A01D1`/`0x7A01D6` as
/// *"signin is finished"* - pulling the state machine out from under the creator. That is why all
/// three historical creation runs ended at `Entering state 'character:signin'`.
///
/// ⛔⛔ **Passing `profile_soid = 0` to [`account_rsat`] is NOT the same thing and does NOT work.**
/// Measured through the client's own codec, which is the only control that can tell them apart:
///
/// ```text
///   account_rsat(acct, &[], 0xD1…0201)  -> 157 bits, decoded object has +0x0000 AND +0x0068
///   account_rsat(acct, &[], 0)          -> 157 bits, decoded object has +0x0000 only
///   account_rsat_no_profile(acct, &[])  ->  93 bits, decoded object has +0x0000 only
/// ```
///
/// The last two are **indistinguishable to the client** at `+0x68`, so a `prof = 0` argument would
/// in fact also clear the gate - but it does so by shipping a *present field whose value is zero*,
/// and the family-4 transaction it rides in would still carry a `PROFILE_INSTANCE_TYPE` object
/// **keyed 0**. `find(4, 1, key)` is a linear scan comparing `element[0]` against the key, so a
/// keyed-0 profile is a live element that a later keyed-0 lookup can hit. Omitting the object is
/// the claim we actually mean; zeroing an argument is a coincidence that happens to decode the same.
/// ⇒ see [`queuez_family4_account_only_body`], and do not "simplify" this into `prof = 0`.
pub fn account_rsat_no_profile(account_soid: u64, characters: &[u64]) -> Vec<u8> {
    let mut bits = character_list_bits(account_soid, characters);
    bits.push(false); // field 2 absent - the u32 at +0x60
    // fields 3..24 all absent - field 3 is the profile SOID and its ABSENCE is the whole point.
    for _ in 0..(ACCOUNT_FIELD_COUNT - 3) {
        bits.push(false);
    }
    pack_msb_first(bits)
}

/// ⭐⭐ The family-4 baseline that delivers **the account and NOTHING ELSE** - no profile object.
///
/// This is the pre-501 shape for a boot that is supposed to reach character CREATION. It differs
/// from [`queuez_family4_baseline_body_with_unlocks`] in two coupled ways, and both are required:
///
/// | | baseline | account-only |
/// |---|---|---|
/// | `object_count` | 2 (account + profile) | **1** |
/// | account field 3 (`+0x68`) | the profile SOID | **absent** |
///
/// Changing only one of them leaves the gate satisfiable: an account naming a profile that was
/// never delivered still fails `0x5833C0`, but a delivered profile keyed 0 alongside a zeroed
/// `+0x68` still resolves. Emit neither.
///
/// ⚠ **This is deliberately a REGRESSION for the orbit path** and must never be reached by it: the
/// family-4/type-1 profile is what carries `last_orbited_destination` (rung 33) and what rung 34's
/// task 7 reads. It is only correct while the roster is empty, i.e. exactly while we are asking the
/// client to make a Guardian. `serves_no_characters` is the predicate that decides, and it flips to
/// `false` the moment a **501** is answered.
pub fn queuez_family4_account_only_body(account_soid: u64) -> Vec<u8> {
    let acct = account_rsat_no_profile(account_soid, &[]);
    let mut b = queuez_family_header_for(QUEUEZ_FAMILY_ACCOUNT, account_soid, 1);
    b.extend_from_slice(&account_instance_type().to_be_bytes());
    b.extend_from_slice(&account_soid.to_be_bytes());
    b.extend_from_slice(&(acct.len() as u32).to_be_bytes());
    b.extend_from_slice(&acct);
    b
}

/// The family-3 **roster** RSAT carrying the character list. See [`character_list_rsat`].
pub fn roster_rsat(account_soid: u64, characters: &[u64]) -> Vec<u8> {
    character_list_rsat(account_soid, characters, ROSTER_FIELD_COUNT - 2)
}

/// The complete QueueZ body carrying the family-3 **roster** baseline.
pub fn queuez_roster_body(account_soid: u64, characters: &[u64]) -> Vec<u8> {
    queuez_object_body(
        QUEUEZ_FAMILY_CHARACTER,
        account_soid,
        ROSTER_INSTANCE_TYPE,
        account_soid, // the roster is keyed by the ACCOUNT soid - it is the family root
        &roster_rsat(account_soid, characters),
    )
}

/// Top-level field count of the **profile** schema - the wrapper at `0x1304858` presents the real
/// 18-field schema (`0x13781B8`, desc_tag `0x80801714`) through a single `kind 0x001` field, which
/// carries no presence bit, so encoding via either tag yields byte-identical output (measured).
const PROFILE_FIELD_COUNT: u32 = 18;

/// The family-4 **character profile** RSAT - minimal: the SOID, every other field absent.
///
/// # Why this object matters for orbit
///
/// `setup:activity_world_transition` says so in the client's own words:
///
/// ```text
/// Updating orbit-destination to '%s --> %s' in the character profile.
/// Unable to update orbit-destination -- cannot find character profile.
/// ```
///
/// The **orbit-destination lives in the character profile**, and `setup:orbit_loading` (state 26)
/// then resolves a slice-set from it:
///
/// ```text
/// Slice-set '%s' doesn't exist (orbiting destination '%s'), so we're falling back to
/// slice-set '%s' (orbiting destination '%s').
/// ```
///
/// So a missing profile is a gate on the orbit path, two states past the one that currently kills
/// us. This sends the minimal one; whether the orbit-destination *field* also has to be populated is
/// `[U]` and cannot be settled until a run gets that far.
///
/// Reference vector, produced by decoding through the client's own codec
/// (`capture-rig/oodle/queuez-object-oracle.py`): SOID `0xD100000000000201` ⇒
/// `e880000000000100800000`, 11 bytes / 82 bits.
pub fn profile_rsat(profile_soid: u64) -> Vec<u8> {
    let mut bits: Vec<bool> = Vec::with_capacity(128);
    bits.push(true); // field 0 present: the profile SOID
    for i in (0..64).rev() {
        bits.push((profile_soid >> i) & 1 == 1);
    }
    for _ in 0..(PROFILE_FIELD_COUNT - 1) {
        bits.push(false); // every other top-level field absent
    }
    while bits.len() % 8 != 0 {
        bits.push(false);
    }
    bits.chunks(8)
        .map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | b as u8))
        .collect()
}

/// ⭐ The profile, **with `last_orbited_destination` populated** - the field the director needs.
///
/// # How the field was located `[C]` `observed-live` 2026-08-08
///
/// The consumer is `0x752AE0` (`world_controller:orbit_map_loader`), and it is five instructions:
/// ```asm
/// call 0x5863D0            ; profile manager
/// call 0x57CD30            ; -> character profile view
/// mov  eax, [rax+0x480]    ; last_orbited_destination - 32 BITS, not 64
/// cmp  eax, 0x811C9DC5     ; the FNV-1a-32 basis = hash of the empty string = "unset"
/// ```
/// Reading the **live** profile object in orbit found that sentinel at object offset **`+0x28`**
/// (2 occurrences in the whole 28,792-byte object; this is the one whose position matches a
/// descriptor field exactly). Profile body `0x80801714` field **[2]** sits at `+0x00c` and is
/// `0x8080181A`, whose field **[10] @+0x01c is a 32-bit leaf** - and `0x00c + 0x01c = 0x28`. ✔
///
/// So the wire path is: profile field **[2]** present → its subfield **[10]** = the destination hash.
/// (`+0x480` is the offset in the client's *view* struct, which `0x57CD30` builds from a different
/// global; it is not the QueueZ object offset. Both refer to the same value.)
///
/// This is a **rank-1 hop** - we change what the server *serves*, so the finding stays `observed-live`
/// and is not tainted the way a memory poke would be.
///
/// ⚠ `[I]` that this specific field is what feeds `view+0x480`. The offset arithmetic and the live
/// sentinel agree, but the writer has not been traced. The run either clears
/// `The character's last-orbited-destination is invalid.` or it does not - that log line is the test.
///
/// Destination hashes (`historicaldocs/design/character-and-orbit.md` §3): Old Russia, Earth `2468353467` ·
/// The Last City `2612139675` · Ishtar Sink, Venus `518553403` · Ocean of Storms, Moon `4233735899`.
pub fn profile_rsat_with_destination(profile_soid: u64, destination: u32) -> Vec<u8> {
    let mut bits: Vec<bool> = Vec::with_capacity(160);
    let push = |bits: &mut Vec<bool>, v: u64, n: u32| {
        for i in (0..n).rev() {
            bits.push((v >> i) & 1 == 1);
        }
    };

    bits.push(true); // field [0] present - the profile SOID
    push(&mut bits, profile_soid, 64);
    bits.push(false); // field [1] absent

    // field [2] PRESENT -> 0x8080181A, 12 sub-fields, each with its own presence bit.
    bits.push(true);
    for i in 0..PROFILE_SETTINGS_FIELD_COUNT {
        if i == PROFILE_SETTINGS_DESTINATION_FIELD {
            bits.push(true);
            push(&mut bits, destination as u64, 32);
        } else {
            bits.push(false);
        }
    }

    for _ in 0..(PROFILE_FIELD_COUNT - 3) {
        bits.push(false); // fields [3]..[17] absent
    }
    while bits.len() % 8 != 0 {
        bits.push(false);
    }
    bits.chunks(8)
        .map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | b as u8))
        .collect()
}

/// Sub-field count of `0x8080181A`, the profile's 36-byte settings block.
pub const PROFILE_SETTINGS_FIELD_COUNT: usize = 12;
/// Index of the 32-bit `last_orbited_destination` leaf within `0x8080181A` (`@+0x01c`).
pub const PROFILE_SETTINGS_DESTINATION_FIELD: usize = 10;

/// `Old Russia, Earth` - the destination we default to, from the archived manifest.
pub const DESTINATION_OLD_RUSSIA: u32 = 2468353467;

/// Instance checksum of the **family-4 / type-4** object - the one the director's activity
/// enumerators look up and we have never sent.
///
/// `0x574220` / `0x574340` (`c_investment_activity_selection_manager_game_system`) do
/// `find(registry, family=4, type=4, &key)` with the key at [`UNLOCKS_OBJECT_KEY`]. The lookup
/// **executes** on a live orbit client and returns NULL purely because the object is absent.
///
/// **Verified here, not taken on report** - `d1-replay read orbit-all.snap v:0x131AC30`
/// (10/10 controls, 0 SKIP) shows an instance descriptor of exactly the same shape as the four
/// we already serve: `+0x08 = 0xC4060975`, `+0x14 = 0xF90` (3,984-byte C struct, matching
/// `d1-oracle object --tag 0x8080102E`). Four positive controls at the same offsets:
/// character `0x3431F1BA`, account `0x64AE75BF`, roster `0xA2AC2C74`, profile `0x8127F15A`.
///
/// ⚠ `+0x1C` is `0x00040004` on **all five** records, so it is NOT the (family, type) pair -
/// a reading this session made and its own control refuted.
pub const UNLOCKS_INSTANCE_TYPE: u32 = 0xC406_0975;

/// The key the selection manager looks the family-4/type-4 object up by, held in the guarded lazy
/// static at `0x2639EA8` and **already correct on a live client** - we have simply never sent an
/// object under it.
///
/// One less than the family-5 root `0x7FFFFFFFFFFFFFFF` at `0x2639EA0`, which is why the two are
/// so easy to confuse; see [`UNLOCKS_KEY_ONLY_PAYLOAD`] for the one-bit trap.
pub const UNLOCKS_OBJECT_KEY: u64 = 0x7FFF_FFFF_FFFF_FFFE;

/// The minimal RSAT payload for the family-4/type-4 object: **presence bit + the key, nothing
/// else.** 9 bytes, 67 bits.
///
/// Round-tripped through the client's own decoder -
/// `d1-oracle object --tag 0x8080102E --decode bfffffffffffffff00` prints
/// `decoded 67 bits -> 3984-byte object / +0x0000 0x7ffffffffffffffe`.
///
/// ⚠ **Two neighbours that decode cleanly to the wrong thing**, both checked:
/// * `e8 80 …80` → `0xD100000000000001`, the **account SOID**. `DESTINATION-PLAN.md` cited this
///   as the pinned key-only vector; it is the account, and its own test says so.
/// * `bf ff … 80` (one bit) → `0x7FFFFFFFFFFFFFFF`, the **family-5 root**, which we already serve.
///
/// An empty-but-present object is a legitimate first experiment: it separates "the registry lookup
/// fails" from "the lookup succeeds and the contents are empty". It is **not** expected to make a
/// destination selectable on its own - with no unlock entries it still means *no unlocks*.
pub const UNLOCKS_KEY_ONLY_PAYLOAD: &[u8] =
    &[0xbf, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00];

/// ⭐ **Family 4 / type 2 - the object whose absence is a MEASURED NULL in the director's
/// selection manager.** Schema tag `0x80800FB2`, 72-byte struct, capacity 1.
///
/// Found by execution, not by guessing. `0x574340` files six arguments into the selection manager
/// (`0x2609A0` is a pure setter), and on a live orbit client the manager reads:
/// ```text
/// +0x08 0x37f8c27f0   +0x20 0x37f92e340  (our unlocks object)
/// +0x10 0x37f902098   +0x28 0x0          ⚠ NULL
/// +0x18 0x37f9093d0   +0x38 0x37f8bd330
/// ```
/// `+0x28` is `0x56CAF0(registry, family=4, type=<0x59710>)`, and `0x59710` is `mov eax,2; ret`
/// - so it is **family 4 / type 2**, and `d1guest obj 4:2` shows 72 zero bytes. We have never sent
/// it. Both of the enumerator's early exits (`0x541850` → AL=1, `0x5867D0` → non-NULL) were
/// **replayed on a snapshot of that client and both pass**, so the enumerator is not bailing -
/// it is being handed a NULL.
///
/// Instance checksum resolved through the package registry (`d1guest tag 0x80800FB2` →
/// object `0x8012DE190`, `+0x08`), with `0x8080102E → 0xC4060975` as the passing control.
pub const FAMILY42_INSTANCE_TYPE: u32 = 0x0ECC_D1F8;

/// Key-only payload for [`FAMILY42_INSTANCE_TYPE`], keyed by the **account SOID**.
///
/// `d1-oracle object --tag 0x80800FB2 --obj <72-byte C struct, +0x00 = account SOID>` →
/// 73 bytes / 579 bits, `round trip: object IDENTICAL`, and `--decode` reads back
/// `+0x0000 0xd100000000000001`.
///
/// ⚠ Regenerate if `D1_ACCOUNT_SOID` changes - the key is baked in. `0x56CAF0` takes **no key
/// argument** (unlike `0x56CA70`), so with capacity 1 the presence test is the object existing at
/// all; the account SOID is the natural key for a family-4 object and is what this encodes.
pub const FAMILY42_KEY_ONLY_PAYLOAD: &[u8] = &[
    0xe8, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xe0, 0x00, 0x20, 0x00, 0x20, 0x00, 0x20, 0x00,
    0x20, 0x00, 0x20, 0x00, 0x20, 0x00, 0x20, 0x00, 0x20, 0x00, 0x20, 0x00, 0x20, 0x00, 0x20, 0x00,
    0x20, 0x00, 0x20, 0x00, 0x20, 0x00, 0x20, 0x00, 0x20, 0x00, 0x20, 0x00, 0x20, 0x00, 0x20, 0x00,
    0x20, 0x00, 0x20, 0x00, 0x20, 0x00, 0x20, 0x00, 0x20, 0x00, 0x20, 0x00, 0x20, 0x00, 0x30, 0x00,
    0x10, 0x00, 0x10, 0x00, 0x10, 0x00, 0x10, 0x00, 0x00,
];

/// Inventory: **family 4 / type 3**, capacity **666**, element **184 bytes** (`0xB8`).
/// Schema tag `0x80801732`; checksum from `d1guest tag 0x80801732` → `0x8013CCA50 +0x08`.
/// Nothing builds these elements yet - recorded so the next session does not re-derive it.
pub const INVENTORY_INSTANCE_TYPE: u32 = 0xEF38_431D;

/// The complete family-4 baseline: the **account** (with its character list) and the **profile**.
///
/// Same two-objects-one-transaction shape as [`queuez_family3_baseline_body`]. The account is the
/// family root (keyed by the account SOID); the profile is keyed by its own SOID.
/// ⚠ **The baseline must carry BOTH objects, always.** A family-4 baseline containing only one of
/// them can remove or invalidate the other - the account and the profile are delivered as one
/// transaction with `object_count = 2`. When adding a third object here (e.g. an inventory or
/// unlock object), ADD it; never replace.
pub fn queuez_family4_baseline_body(
    account_soid: u64,
    characters: &[u64],
    profile_soid: u64,
) -> Vec<u8> {
    queuez_family4_baseline_body_with_destination(account_soid, characters, profile_soid, None)
}

/// As [`queuez_family4_baseline_body`], but optionally populating the profile's
/// `last_orbited_destination` - see [`profile_rsat_with_destination`].
pub fn queuez_family4_baseline_body_with_destination(
    account_soid: u64,
    characters: &[u64],
    profile_soid: u64,
    destination: Option<u32>,
) -> Vec<u8> {
    queuez_family4_baseline_body_with_unlocks(
        account_soid,
        characters,
        profile_soid,
        destination,
        None,
        &[],
    )
}

/// As above, but with the profile payload supplied verbatim - for the oracle-produced **full**
/// profile object (`capture-rig/profiles/*.bin`).
///
/// ⚠ **Why a raw file and not a hand-packed sparse object.** `profile_rsat_with_destination`'s
/// sparse encoding is WRONG and is kept only as a documented negative: once profile field [2] is
/// present the client reads **316 bits** of it, and a 128-bit buffer made the decode "succeed" by
/// returning zeros past the end - the §26.67 under-length trap, caught by
/// `d1-oracle object --decode` before it cost a second run. The observable symptom was
/// `last_orbited_destination` landing as **0** rather than our value, which reads exactly like
/// "the field is not the right one".
///
/// The oracle's full encode is correct and round-trips: encode a 28,792-byte C object with
/// `+0x28 = <destination>`, and `--decode` returns `+0x0028 0x00000000932015bb`. 20,318 bytes.
pub fn queuez_family4_baseline_body_raw_profile(
    account_soid: u64,
    characters: &[u64],
    profile_soid: u64,
    profile_payload: &[u8],
) -> Vec<u8> {
    queuez_family4_baseline_body_raw_profile_with_unlocks(
        account_soid,
        characters,
        profile_soid,
        profile_payload,
        None,
        &[],
    )
}

/// As [`queuez_family4_baseline_body_with_destination`], plus the **family-4/type-4 object** the
/// director's activity enumerators look up. See [`UNLOCKS_INSTANCE_TYPE`].
///
/// `unlocks: None` reproduces the known-good two-object baseline byte for byte.
pub fn queuez_family4_baseline_body_with_unlocks(
    account_soid: u64,
    characters: &[u64],
    profile_soid: u64,
    destination: Option<u32>,
    unlocks: Option<&[u8]>,
    extras: &[(u32, u64, Vec<u8>)],
) -> Vec<u8> {
    let acct = account_rsat(account_soid, characters, profile_soid);
    let prof = match destination {
        Some(d) => profile_rsat_with_destination(profile_soid, d),
        None => profile_rsat(profile_soid),
    };
    queuez_family4_from_parts(account_soid, profile_soid, &acct, &prof, unlocks, extras)
}

/// As [`queuez_family4_baseline_body_raw_profile`], plus the family-4/type-4 object.
///
/// ⚠ **Both raw-profile and hand-built-profile paths must accept `unlocks`.** The live boot route
/// goes through the raw-profile branch (`D1_PROFILE_FILE`); adding the third object to only one
/// branch would silently do nothing on the path that actually runs, which is the same class of
/// mistake as the two-chain dispatch that produced a false-alarm gate warning.
pub fn queuez_family4_baseline_body_raw_profile_with_unlocks(
    account_soid: u64,
    characters: &[u64],
    profile_soid: u64,
    profile_payload: &[u8],
    unlocks: Option<&[u8]>,
    extras: &[(u32, u64, Vec<u8>)],
) -> Vec<u8> {
    let acct = account_rsat(account_soid, characters, profile_soid);
    queuez_family4_from_parts(account_soid, profile_soid, &acct, profile_payload, unlocks, extras)
}

fn queuez_family4_from_parts(
    account_soid: u64,
    profile_soid: u64,
    acct: &[u8],
    prof: &[u8],
    unlocks: Option<&[u8]>,
    extras: &[(u32, u64, Vec<u8>)],
) -> Vec<u8> {
    let count = 2 + unlocks.is_some() as u32 + extras.len() as u32;
    let mut b = queuez_family_header_for(QUEUEZ_FAMILY_ACCOUNT, account_soid, count);
    b.extend_from_slice(&account_instance_type().to_be_bytes());
    b.extend_from_slice(&account_soid.to_be_bytes());
    b.extend_from_slice(&(acct.len() as u32).to_be_bytes());
    b.extend_from_slice(&acct);
    b.extend_from_slice(&profile_instance_type().to_be_bytes());
    b.extend_from_slice(&profile_soid.to_be_bytes());
    b.extend_from_slice(&(prof.len() as u32).to_be_bytes());
    b.extend_from_slice(&prof);
    if let Some(u) = unlocks {
        // ADDED alongside the other two, never replacing either - the family-4 baseline is one
        // transaction and dropping account or profile from it invalidates them.
        b.extend_from_slice(&UNLOCKS_INSTANCE_TYPE.to_be_bytes());
        b.extend_from_slice(&UNLOCKS_OBJECT_KEY.to_be_bytes());
        b.extend_from_slice(&(u.len() as u32).to_be_bytes());
        b.extend_from_slice(u);
    }
    for (instance_type, key, payload) in extras {
        b.extend_from_slice(&instance_type.to_be_bytes());
        b.extend_from_slice(&key.to_be_bytes());
        b.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        b.extend_from_slice(payload);
    }
    b
}

/// Family 3 carrying **only an EMPTY roster** - a roster object whose character list is zero-length,
/// and no character object at all.
///
/// ⭐ **This is what makes the client offer character CREATION.** There are three distinct states and
/// only two of them were reachable before:
///
/// | family 3 | `character:signin` does |
/// |---|---|
/// | not sent at all | walks straight through - no UI, no creator, no selection |
/// | roster with 0 characters | **shows the character creator** |
/// | roster + 1 character | selection |
///
/// `D1_PUSH_CHARACTER=0` produces the FIRST row, not the second - measured 2026-08-08, run
/// `LET-ME-CREATE`: zero family-3 objects served and the client went
/// `character:signin -> character:prologue -> cleanup -> orbit` without ever stopping.
///
/// Why creation matters: the client's own `..._fill_desired_items_default_gear` runs there, and it
/// is the only thing in the game that grants a new Guardian its starting kit. `run-boot.sh`'s push
/// exists specifically to skip creation - its comment says it keeps "character creation, starter
/// inventory and the prologue mission off the critical path" - which is exactly why the resulting
/// character has no inventory.
pub fn queuez_family3_empty_roster_body(account_soid: u64) -> Vec<u8> {
    let roster = roster_rsat(account_soid, &[]);
    let mut b = queuez_family_header_for(QUEUEZ_FAMILY_CHARACTER, account_soid, 1);
    b.extend_from_slice(&ROSTER_INSTANCE_TYPE.to_be_bytes());
    b.extend_from_slice(&account_soid.to_be_bytes());
    b.extend_from_slice(&(roster.len() as u32).to_be_bytes());
    b.extend_from_slice(&roster);
    b
}

/// The complete family-3 baseline: the **roster** and the **character** in one transaction.
///
/// Both objects belong to family 3 and the wire format already carries an `object_count`, so a real
/// baseline would deliver them together rather than as two transactions. The roster is the family
/// *root* (keyed by the account SOID); the character is the family *element* (keyed by its own).
pub fn queuez_family3_baseline_body(
    account_soid: u64,
    character_soid: u64,
    id: Identity,
) -> Vec<u8> {
    queuez_family3_baseline_body_raw_character(account_soid, character_soid, id, None)
}

/// As [`queuez_family3_baseline_body`], but with the character payload supplied verbatim - for an
/// oracle-produced **full** character object (`capture-rig/characters/*.bin`).
///
/// Why a raw file rather than extending `character_rsat`: the hand-built encoder emits identity and
/// little else, and the fields we now want (character schema `0x80801954` field [3] ->
/// `0x80801755`, four 32-bit leaves at C `+0x30/+0x34/+0x38/+0x3c`) are nested two levels down.
/// The oracle emits **every** field and round-trips - exactly how the working 20,318-byte profile
/// payload was produced, and the same reason `queuez_family4_baseline_body_raw_profile` exists.
/// The §26.67 under-length trap is the alternative: a short hand-packed buffer decodes "fine" and
/// silently reads zeros past the end.
pub fn queuez_family3_baseline_body_raw_character(
    account_soid: u64,
    character_soid: u64,
    id: Identity,
    character_payload: Option<&[u8]>,
) -> Vec<u8> {
    let roster = roster_rsat(account_soid, &[character_soid]);
    let chr = match character_payload {
        Some(raw) => raw.to_vec(),
        None => character_rsat(character_soid, id),
    };
    let mut b = queuez_family_header_for(QUEUEZ_FAMILY_CHARACTER, account_soid, 2);
    // object 1 - the roster (family root)
    b.extend_from_slice(&ROSTER_INSTANCE_TYPE.to_be_bytes());
    b.extend_from_slice(&account_soid.to_be_bytes());
    b.extend_from_slice(&(roster.len() as u32).to_be_bytes());
    b.extend_from_slice(&roster);
    // object 2 - the character itself
    b.extend_from_slice(&CHARACTER_INSTANCE_TYPE.to_be_bytes());
    b.extend_from_slice(&character_soid.to_be_bytes());
    b.extend_from_slice(&(chr.len() as u32).to_be_bytes());
    b.extend_from_slice(&chr);
    b
}

/// The complete QueueZ body carrying the family-4 **account** baseline, with its character list.
pub fn queuez_account_body(account_soid: u64, characters: &[u64], profile_soid: u64) -> Vec<u8> {
    queuez_object_body(
        QUEUEZ_FAMILY_ACCOUNT,
        account_soid,
        account_instance_type(),
        account_soid,
        &account_rsat(account_soid, characters, profile_soid),
    )
}

/// The complete QueueZ message body carrying one baseline character.
///
/// `account_soid` must equal the SOID account-id translation issued, and `character_soid` must equal
/// the SOID serialized inside the RSAT payload - the object header and the payload have to agree.
pub fn queuez_character_body(account_soid: u64, character_soid: u64, id: Identity) -> Vec<u8> {
    let rsat = character_rsat(character_soid, id);
    let mut b = queuez_family_header(account_soid, 1);
    b.extend_from_slice(&CHARACTER_INSTANCE_TYPE.to_be_bytes());
    b.extend_from_slice(&character_soid.to_be_bytes());
    b.extend_from_slice(&(rsat.len() as u32).to_be_bytes());
    b.extend_from_slice(&rsat);
    b
}

/// The family header alone, up to and including `object_count`.
///
/// Split out so `object_count = 0` can be sent - see [`queuez_empty_family_body`].
fn queuez_family_header(account_soid: u64, object_count: u32) -> Vec<u8> {
    queuez_family_header_for(QUEUEZ_FAMILY_CHARACTER, account_soid, object_count)
}

/// The family header for an arbitrary family type - a **baseline at version 0**, which is every
/// family body this crate emitted before 2026-08-10. [`queuez_family_header_versioned`] is the
/// general form; this stays the default so the 130-odd builders and tests above it are unchanged.
pub fn queuez_family_header_for(family: u32, root_soid: u64, object_count: u32) -> Vec<u8> {
    queuez_family_header_versioned(family, root_soid, object_count, 0, QUEUEZ_FLAG_BASELINE)
}

/// The family header with `version` and `flags` chosen by the caller.
///
/// ⚠ **`version = 0xFFFFFFFF` is rejected before anything else looks at it** - `0x565140` reads the
/// version as a signed int and `-1` is *"received update with a family-version of NONE"*, a refusal
/// that never reaches `0x56AF60`. This function does not forbid it; the caller must not send it.
pub fn queuez_family_header_versioned(
    family: u32,
    root_soid: u64,
    object_count: u32,
    version: u32,
    flags: u8,
) -> Vec<u8> {
    let mut b = Vec::with_capacity(54);
    b.extend_from_slice(&1u32.to_be_bytes()); // family count
    b.extend_from_slice(&family.to_be_bytes());
    b.extend_from_slice(&root_soid.to_be_bytes()); // family root
    b.extend_from_slice(&version.to_be_bytes()); // family version
    b.push(flags);
    b.extend_from_slice(&object_count.to_be_bytes());
    b
}

/// ⭐ The **availability-cache invalidation ping**: a family-4 delta carrying NO objects.
///
/// This is the whole P2 mechanism in its minimal, purge-free form. `0x565140` logs
/// *"object-count of zero in [family_type …]"* at level 4, skips `0x5655B0` entirely, and then
/// calls `0x56B5E0` **anyway** - which is the tail chain that ends in `FUN_00574440(4)` clearing
/// the availability cache's validity byte. See [`queuez_span_as_delta`] for the full receipt.
///
/// With zero objects there is no decode, no classifier and no keep-list, so none of the three ways
/// a family-4 re-delivery can destroy state (purge, mis-typed splice, short payload) exist here.
///
/// `version` must be the accepted family version **plus one** - for a client that has taken exactly
/// our version-0 baseline, that is `1`.
pub fn queuez_family4_delta_ping(account_soid: u64, version: u32) -> Vec<u8> {
    queuez_family_header_versioned(QUEUEZ_FAMILY_ACCOUNT, account_soid, 0, version, QUEUEZ_FLAG_DELTA)
}

/// A QueueZ body for any family carrying one object - the general form.
///
/// `queuez_character_body` is this with family 3 and the character RSAT; the account gate needs
/// family 4 with [`ACCOUNT_INSTANCE_TYPE`].
pub fn queuez_object_body(
    family: u32,
    root_soid: u64,
    instance_type: u32,
    object_soid: u64,
    payload: &[u8],
) -> Vec<u8> {
    let mut b = queuez_family_header_for(family, root_soid, 1);
    b.extend_from_slice(&instance_type.to_be_bytes());
    b.extend_from_slice(&object_soid.to_be_bytes());
    b.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    b.extend_from_slice(payload);
    b
}

/// Wrap any QueueZ body as a server-originated push.
pub fn queuez_push(sequence: u32, body: &[u8]) -> Vec<u8> {
    let mut p = Vec::with_capacity(6 + body.len());
    p.extend_from_slice(&TYPE_QUEUEZ_OBJECT.to_be_bytes());
    p.extend_from_slice(&sequence.to_be_bytes());
    p.extend_from_slice(body);
    p
}

/// **A discriminating probe: the same family header carrying NO objects.**
///
/// The question it settles is one a full push cannot. When a character push is rejected with
/// `push:queuez: failed to process update`, the failure could be either
///
/// 1. at **family/root lookup** - the root SOID names an account the client has never been issued,
///    so the object loop is never entered and the character bytes are never examined; or
/// 2. inside the **object loop** - `instance_type` or the RSAT payload was rejected.
///
/// With `object_count = 0` there is no object loop to fail in. So:
/// **still complains ⇒ cause (1); goes quiet ⇒ cause (2).**
///
/// This exists because reading a failure ordering alone gave the wrong answer once already -
/// `historicaldocs/design/character-and-orbit.md` §8.
pub fn queuez_empty_family_body(account_soid: u64) -> Vec<u8> {
    queuez_family_header(account_soid, 0)
}

/// The same discriminator for an arbitrary family - e.g. family 4, to ask whether the **account**
/// root is subscribed yet.
pub fn queuez_empty_family_body_for(family: u32, root_soid: u64) -> Vec<u8> {
    queuez_family_header_for(family, root_soid, 0)
}

/// A server-initiated QueueZ push: base inner header (type + sequence), then the body.
///
/// ⚠ This is a **request** we originate, not a response, so it carries no `status` field - unlike
/// [`reply_header`], whose 8-byte class-2 shape is only correct for replies.
pub fn queuez_character_push(
    sequence: u32,
    account_soid: u64,
    character_soid: u64,
    id: Identity,
) -> Vec<u8> {
    let body = queuez_character_body(account_soid, character_soid, id);
    let mut p = Vec::with_capacity(6 + body.len());
    p.extend_from_slice(&TYPE_QUEUEZ_OBJECT.to_be_bytes());
    p.extend_from_slice(&sequence.to_be_bytes());
    p.extend_from_slice(&body);
    p
}

#[cfg(test)]
mod queuez_tests {
    use super::*;

    /// The RSAT bytes Destiny's own serializer emitted. If this test fails, our bit grammar has
    /// drifted from the oracle and nothing downstream can be trusted.
    #[test]
    fn character_rsat_reproduces_the_serializer_oracle_vector() {
        let got = character_rsat(0xD100_0000_0000_0101, Identity::TITAN_HUMAN_FEMALE);
        assert_eq!(
            got,
            vec![0xe8, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0xf0, 0x18, 0x1c, 0x00, 0x00],
            "RSAT payload must match the eboot-emitted reference exactly"
        );
        assert_eq!(got.len(), 13);
    }

    #[test]
    fn investment_503_body_is_a_presence_bit_plus_u64() {
        let got = investment_503_account_soid_body(0xD100_0000_0000_0001);
        // 1 + 64 = 65 bits -> 9 bytes. Same shape as the request tail the client sends us.
        assert_eq!(got.len(), 9);
        assert_eq!(got, vec![0xe8, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80]);
        // The leading bit MUST be 1 - a zero-leading body is read as "field absent" and silently
        // discarded (historicaldocs/design/character-and-orbit.md §11.5).
        assert_eq!(got[0] & 0x80, 0x80);
    }

    #[test]
    fn platform_identifier_is_36_bytes_nul_padded() {
        let id = platform_identifier("D1Guardian");
        assert_eq!(id.len(), AIDT_PLATFORM_ID_LEN);
        assert_eq!(&id[..10], b"D1Guardian");
        assert!(id[10..].iter().all(|&b| b == 0), "must be NUL-padded");
        // The predicate `0x2B79C0` is `cmp byte [rdi],0; setne al` - byte 0 must be non-zero or the
        // client treats the identifier as empty.
        assert_ne!(id[0], 0);
    }

    #[test]
    fn platform_to_investment_response_matches_the_decoder_grammar() {
        let id = platform_identifier("D1Guardian");
        let got = platform_to_investment_response(&[(id, 0xD100_0000_0000_0001)]);

        // Header: u16 count=1, u8 kind_a=7, u8 kind_b=0xFF - 32 bits, byte-aligned.
        assert_eq!(&got[..4], &[0x00, 0x01, 0x07, 0xff]);
        // List A is one 36-byte identifier, still byte-aligned because the header is 32 bits.
        assert_eq!(&got[4..4 + 36], &id[..]);
        // List B is one u64 SOID, big-endian.
        assert_eq!(&got[40..48], &0xD100_0000_0000_0001u64.to_be_bytes());
        // 32 + 288 + 64 bits = 384 bits = 48 bytes, no padding needed.
        assert_eq!(got.len(), 48);
    }

    #[test]
    fn platform_to_investment_response_keeps_lists_parallel_not_interleaved() {
        let a = platform_identifier("AAA");
        let b = platform_identifier("BBB");
        let got = platform_to_investment_response(&[(a, 0x1111), (b, 0x2222)]);
        assert_eq!(&got[..4], &[0x00, 0x02, 0x07, 0xff]);
        // Both identifiers first (list A), THEN both SOIDs (list B). `0x55A7D7`/`0x55A7EF` decode
        // one whole list before the other; interleaving would desync the bitstream.
        assert_eq!(&got[4..40], &a[..]);
        assert_eq!(&got[40..76], &b[..]);
        assert_eq!(&got[76..84], &0x1111u64.to_be_bytes());
        assert_eq!(&got[84..92], &0x2222u64.to_be_bytes());
    }

    #[test]
    fn queuez_body_matches_the_reference_54_byte_frame() {
        let body = queuez_character_body(
            0xD100_0000_0000_0001,
            0xD100_0000_0000_0101,
            Identity::TITAN_HUMAN_FEMALE,
        );
        let want = "0000000100000003d10000000000000100000000010000000\
                    13431f1bad1000000000001010000000de880000000000080f0181c0000";
        assert_eq!(
            body.iter().map(|b| format!("{b:02x}")).collect::<String>(),
            want.replace(['\n', ' '], "")
        );
        assert_eq!(body.len(), 54);
    }

    /// Both vectors were produced by decoding through the CLIENT'S OWN codec (`0x2C9000`) via
    /// `capture-rig/oodle/queuez-object-oracle.py`, independently of each other and of this code.
    /// They share the first 134 bits, which is what pins the grammar: same field 0 (SOID) and
    /// field 1 (`u4 count` + `count × u64be`), different absent-field tails.
    #[test]
    fn character_list_rsat_matches_both_oracle_vectors() {
        let acct = 0xD100_0000_0000_0001u64;
        let chr = 0xD100_0000_0000_0101u64;
        let hex = |v: &[u8]| v.iter().map(|b| format!("{b:02x}")).collect::<String>();

        // roster: 4 top-level fields -> 2 absent bits -> 134 + 2 = 136 bits = 17 bytes exactly.
        let r = roster_rsat(acct, &[chr]);
        assert_eq!(hex(&r), "e880000000000000c74400000000000404");
        assert_eq!(r.len(), 17);

        // account: fields 0,1 then field 2 absent, field 3 = the profile SOID, then 21 absent
        // -> 134 + 1 + 1 + 64 + 21 = 221 bits -> 28 bytes.
        //
        // ⚠ Pinned against a decode through the client's own codec, NOT by analogy: the decoded
        // 29,496-byte object has exactly four non-zero qwords - `+0x00` key, `+0x08` count,
        // `+0x10` character SOID, and **`+0x68` the profile SOID**. `+0x68` is the source
        // `0x585820` copies into the `character:signin` gate's slot B.
        let prof = 0xD100_0000_0000_0002u64;
        let a = account_rsat(acct, &[chr], prof);
        assert_eq!(hex(&a), "e880000000000000c74400000000000405d100000000000002000000");
        assert_eq!(a.len(), 28);

        // The shared prefix IS the grammar for fields 0-1. If these ever diverge, one of the two
        // field counts is wrong and the client will read past the end of the shorter payload.
        // The account diverges only *after* the roster's 17 bytes, in its trailing-field region.
        assert_eq!(&a[..16], &r[..16]);
    }

    /// The gate this whole encoder change exists for. `0x78C5F0` rung 3 needs
    /// `qword [selection_node + 0x20]`, whose only writer (`0x585820`) sources it from
    /// `*(account + 0x68)` - and refuses unless that value resolves as a family-4/type-1 object.
    /// So the account's field 3 must carry **the profile SOID we actually send**, not the account's
    /// own SOID and not the character's.
    #[test]
    fn account_field_3_carries_the_profile_soid_that_the_baseline_also_sends() {
        let acct = 0xD100_0000_0000_0001u64;
        let chr = 0xD100_0000_0000_0101u64;
        let prof = 0xD100_0000_0000_0002u64;

        // field 3 begins after fields 0-1 (134 bits) + field 2's absent bit + field 3's present bit
        let a = account_rsat(acct, &[chr], prof);
        let bits: Vec<bool> =
            a.iter().flat_map(|b| (0..8).rev().map(move |i| (b >> i) & 1 == 1)).collect();
        assert!(!bits[134], "field 2 (the u32 at +0x60) must be absent");
        assert!(bits[135], "field 3 (the profile SOID at +0x68) must be present");
        let v = bits[136..200].iter().fold(0u64, |a, &b| (a << 1) | b as u64);
        assert_eq!(v, prof, "field 3 must be the PROFILE soid - 0x5833C0 resolves it as family 4 type 1");

        // and it must match what queuez_family4_baseline_body puts in the profile object header,
        // or 0x5833C0's registry lookup finds nothing and slot B is never written.
        let body = queuez_family4_baseline_body(acct, &[chr], prof);
        assert!(
            body.windows(8).any(|w| w == prof.to_be_bytes()),
            "the profile SOID named by account field 3 must also be delivered as an object"
        );
    }

    /// Pinned against the vector the RSAT lane produced by decoding through the client's own object
    /// codec (`0x2C9000`): 11 bytes / 82 bits, SOID at +0x00, everything else zero.
    #[test]
    fn profile_rsat_matches_the_oracle_vector() {
        let p = profile_rsat(0xD100_0000_0000_0201);
        assert_eq!(p.iter().map(|b| format!("{b:02x}")).collect::<String>(),
                   "e880000000000100800000");
        assert_eq!(p.len(), 11);
        // 1 presence bit + 64 SOID bits + 17 absent = 82 bits; the buffer must COVER that, or the
        // client reads past the end and we get the short-payload trap all over again.
        let bits_read = 1 + 64 + (PROFILE_FIELD_COUNT - 1);
        assert_eq!(bits_read, 82);
        assert!(p.len() * 8 >= bits_read as usize);
    }

    #[test]
    fn family4_baseline_carries_account_then_profile() {
        let acct = 0xD100_0000_0000_0001u64;
        let chr = 0xD100_0000_0000_0101u64;
        let prof = 0xD100_0000_0000_0201u64;
        let b = queuez_family4_baseline_body(acct, &[chr], prof);
        let u32at = |o: usize| u32::from_be_bytes(b[o..o + 4].try_into().unwrap());
        let u64at = |o: usize| u64::from_be_bytes(b[o..o + 8].try_into().unwrap());

        assert_eq!(u32at(4), QUEUEZ_FAMILY_ACCOUNT);
        assert_eq!(u64at(8), acct, "family root");
        assert_eq!(b[20], QUEUEZ_FLAG_BASELINE);
        assert_eq!(u32at(21), 2, "object_count");

        let mut o = 25;
        assert_eq!(u32at(o), ACCOUNT_INSTANCE_TYPE);
        assert_eq!(u64at(o + 4), acct);
        let n0 = u32at(o + 12) as usize;
        assert_eq!(n0, 28, "account payload carries the character list AND the profile SOID");
        o += 16 + n0;

        assert_eq!(u32at(o), PROFILE_INSTANCE_TYPE);
        assert_eq!(u64at(o + 4), prof, "the profile is keyed by its OWN soid, not the account's");
        let n1 = u32at(o + 12) as usize;
        assert_eq!(n1, 11);
        o += 16 + n1;

        assert_eq!(o, b.len(), "the two objects must consume the body exactly");
    }

    // ── P1a: the CONTROL AXIS is "the empty-roster family-4 body carries no family-4/type-1 object,
    //    and its account payload leaves +0x68 ABSENT". ────────────────────────────────────────────
    //
    // ⚠⚠ THE TRAP THIS IS BUILT AGAINST: asserting on our own packer's hex instead of on the
    // client's DECODE. `profile_soid = 0` and `field 3 absent` produce different bytes (157 vs 93
    // bits) and **identical objects** to the client - `+0x0000` only in both. A hex-comparison test
    // would go green while the wrong builder shipped, and the difference is invisible at `+0x68`.
    // So the bit-length is asserted here (it is the thing the client's decoder consumes) and the
    // decode itself is asserted OUT OF PROCESS, by `d1-oracle object --tag 0x808018c7 --decode`,
    // whose two measured outputs are quoted below.

    /// `account_rsat_no_profile` must OMIT field 3, not zero it.
    ///
    /// Measured through the client's own codec, both directions:
    /// ```text
    ///   --decode e880000000000000c0000000            -> 93 bits: +0x0000 ONLY          (this)
    ///   --decode e880000000000000c1d100000000000201000000 -> 157 bits: +0x0000 AND +0x0068  (the inversion)
    /// ```
    #[test]
    fn account_rsat_no_profile_omits_field_three_rather_than_zeroing_it() {
        let acct = 0xD100_0000_0000_0001u64;
        let none = account_rsat_no_profile(acct, &[]);
        let zero = account_rsat(acct, &[], 0);
        let with = account_rsat(acct, &[], 0xD100_0000_0000_0201);

        // 1 + 64 (soid) + 1 + 4 (empty list) + 22 absent = 93 bits -> 12 bytes.
        assert_eq!(none.len(), 12, "field 3 absent is 93 bits");
        // The inversion, stated as an assertion rather than as a comment: the two builders that
        // both decode to "+0x0000 only" are NOT interchangeable on the wire, and the reason the
        // absent form is the correct one is the OBJECT it lets us drop, not the bits it saves.
        assert_eq!(zero.len(), 20, "field 3 present-with-zero is still 157 bits");
        assert_eq!(with.len(), 20);
        assert_ne!(none, zero, "'absent' and 'present, value 0' must not be the same bytes");

        // The shared prefix is the account SOID and the empty character list; they diverge at the
        // byte holding fields 1-3's presence bits.
        assert_eq!(none[..8], with[..8]);
        assert_eq!(none[8], 0xC0, "field 1 present, field 2 absent, field 3 ABSENT");
        assert_eq!(with[8], 0xC1, "... against field 3 PRESENT in the baseline builder");
    }

    /// ⭐ **The P1a control.** The empty-roster family-4 delivery must contain exactly one object,
    /// and it must not be the profile.
    #[test]
    fn family4_account_only_body_emits_no_profile_object() {
        let acct = 0xD100_0000_0000_0001u64;
        let b = queuez_family4_account_only_body(acct);
        let u32at = |s: &[u8], o: usize| u32::from_be_bytes(s[o..o + 4].try_into().unwrap());
        let u64at = |s: &[u8], o: usize| u64::from_be_bytes(s[o..o + 8].try_into().unwrap());

        assert_eq!(u32at(&b, 0), 1, "transaction_count");
        assert_eq!(u32at(&b, 4), QUEUEZ_FAMILY_ACCOUNT);
        assert_eq!(u64at(&b, 8), acct, "family root");
        assert_eq!(b[20], QUEUEZ_FLAG_BASELINE);
        assert_eq!(u32at(&b, 21), 1, "object_count MUST be 1 - account only");

        let mut o = 25;
        assert_eq!(u32at(&b, o), ACCOUNT_INSTANCE_TYPE);
        assert_eq!(u64at(&b, o + 4), acct);
        let n = u32at(&b, o + 12) as usize;
        assert_eq!(n, 12, "the account payload is the field-3-absent form");
        o += 16 + n;
        assert_eq!(o, b.len(), "one object must consume the body exactly");

        // The inversion, asserted rather than promised: today's baseline builder DOES emit the
        // profile, so a swap back is caught here and not on a boot.
        let base = queuez_family4_baseline_body_with_unlocks(
            acct, &[], 0xD100_0000_0000_0201, None, None, &[]);
        assert_eq!(u32at(&base, 21), 2, "the baseline builder is the RED side of this control");
        assert!(base.windows(4).any(|w| w == PROFILE_INSTANCE_TYPE.to_be_bytes()),
                "... and it carries a PROFILE_INSTANCE_TYPE object");
        assert!(!b.windows(4).any(|w| w == PROFILE_INSTANCE_TYPE.to_be_bytes()),
                "⛔ the account-only body must contain NO family-4/type-1 object");
    }

    // ── P1c: the CONTROL AXIS is "the span we put in a 206 reply is a delivery for the family the
    //    REQUEST asked for, and the request's family is `(leaf) - 1`". ────────────────────────────

    /// The observed request, and the un-biased misreading that `CREATION-BUILD-ORDER.md` §3a shipped.
    #[test]
    fn fetch_family_request_unbiases_the_four_bit_leaf() {
        // 195 arrivals across 162 responder logs, all byte-identical.
        let observed = [0x4du8, 0x10, 0, 0, 0, 0, 0, 0, 0x10];
        let r = fetch_family_request(&observed).expect("the observed request must decode");
        assert_eq!(r.family, 3, "leaf 4 -> family 3; the naive `v >> 68` reads 4 and is WRONG");
        assert_eq!(r.key, 0xD100_0000_0000_0001);

        // The client's own encoder emits nibbles 1..5 for families 0..4 - the vectors this is
        // derived from, so that a future edit cannot re-introduce the bias by "simplifying".
        for (family, leaf) in [(0u32, 0x1u8), (1, 0x2), (2, 0x3), (3, 0x4), (4, 0x5)] {
            let mut args = [0u8; 9];
            args[0] = leaf << 4;
            assert_eq!(fetch_family_request(&args).unwrap().family, family);
        }
        // A zero leaf encodes -1, not family 0. Refuse it rather than saturate.
        assert!(fetch_family_request(&[0u8; 9]).is_none(), "leaf 0 is not a family");
        // And an under-length request must be refused, never zero-extended (§26.67).
        assert!(fetch_family_request(&[0x4d, 0x10, 0, 0, 0, 0, 0, 0]).is_none());
    }

    /// ⭐ **The P1c control.** The span envelope is the client's own encoder's, byte for byte.
    ///
    /// The RED side is the shape this replaces: feed the historical `D1_206_BODY` constant back
    /// through the decoder and it names **family 4**, i.e. the delivery that purges the profile at
    /// creation-finish. That is asserted here so the two are never conflated again.
    #[test]
    fn fetch_family_response_envelope_matches_the_native_encoder() {
        // `d1-oracle message --tag 0x80801869 --msgid 206 --span <this>` -> wire `00ce` + `PINNED`.
        let canned_span: Vec<u8> = [
            &1u32.to_be_bytes()[..], &QUEUEZ_FAMILY_ACCOUNT.to_be_bytes()[..],
            &0xD100_0000_0000_0001u64.to_be_bytes()[..], &0u32.to_be_bytes()[..],
            &[QUEUEZ_FLAG_BASELINE][..], &1u32.to_be_bytes()[..],
            &ACCOUNT_INSTANCE_TYPE.to_be_bytes()[..], &0xD100_0000_0000_0001u64.to_be_bytes()[..],
            &9u32.to_be_bytes()[..],
            &[0xe8, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80][..],
        ].concat();
        assert_eq!(canned_span.len(), 50);
        // ⚠ Held as the HEX STRING, copied verbatim from `capture-rig/run-boot.sh`'s
        // `D1_206_BODY` pin, and unhexed here. A byte-array transcription of this got the length
        // wrong on the first attempt; a string cannot be mis-grouped, and a wrong length fails the
        // parse instead of silently shifting every subsequent byte.
        const PINNED_HEX: &str = "1801900000000800000026880000000000000800000000080000000b\
                                  2573adfe88000000000000080000004f440000000000000400";
        let pinned: Vec<u8> = (0..PINNED_HEX.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&PINNED_HEX[i..i + 2], 16).unwrap())
            .collect();
        assert_eq!(pinned.len(), 53, "run-boot.sh pins a 53-byte D1_206_BODY");
        assert_eq!(fetch_family_response_body(Some(&canned_span)), pinned,
                   "the span envelope must reproduce the client's own encoder byte for byte");

        // No span reproduces the historical fixed point, and `investment_206_response` is built
        // from the same function so the two can never drift apart.
        assert_eq!(fetch_family_response_body(None), vec![0x10]);

        // Round-trip, and the RED side: the pinned constant is a FAMILY-4 delivery.
        let back = fetch_family_response_span(&pinned).expect("the pinned body carries a span");
        assert_eq!(back, canned_span);
        assert_eq!(queuez_span_family(&back), Some(QUEUEZ_FAMILY_ACCOUNT),
                   "⛔ D1_206_BODY is family 4 - answering the observed family-3 request with it \
                    is the purge that deletes the profile");
        assert_eq!(fetch_family_response_span(&[0x10]), None, "no presence bit, no span");

        // And a family-3 delivery reads back as family 3, so the guard can actually discriminate.
        let f3 = queuez_family3_empty_roster_body(0xD100_0000_0000_0001);
        let body3 = fetch_family_response_body(Some(&f3));
        assert_eq!(queuez_span_family(&fetch_family_response_span(&body3).unwrap()),
                   Some(QUEUEZ_FAMILY_CHARACTER));
        assert_ne!(body3, pinned, "the two families must not encode to the same body");
    }

    #[test]
    fn family3_baseline_carries_two_objects_and_parses_by_its_own_length_fields() {
        let acct = 0xD100_0000_0000_0001u64;
        let chr = 0xD100_0000_0000_0101u64;
        let b = queuez_family3_baseline_body(acct, chr, Identity::TITAN_HUMAN_FEMALE);

        let u32at = |o: usize| u32::from_be_bytes(b[o..o + 4].try_into().unwrap());
        let u64at = |o: usize| u64::from_be_bytes(b[o..o + 8].try_into().unwrap());

        assert_eq!(u32at(0), 1, "transaction_count");
        assert_eq!(u32at(4), QUEUEZ_FAMILY_CHARACTER, "family_type");
        assert_eq!(u64at(8), acct, "family_root_soid");
        assert_eq!(b[20], QUEUEZ_FLAG_BASELINE, "first family update must be a baseline");
        assert_eq!(u32at(21), 2, "object_count");

        // Walk the objects using only the body's own length fields - if the framing is wrong this
        // lands off the end rather than quietly reading the next object's header as payload.
        let mut o = 25;
        assert_eq!(u32at(o), ROSTER_INSTANCE_TYPE);
        assert_eq!(u64at(o + 4), acct, "the roster is keyed by the ACCOUNT soid - it is the root");
        let n0 = u32at(o + 12) as usize;
        assert_eq!(n0, 17, "roster payload length");
        o += 16 + n0;

        assert_eq!(u32at(o), CHARACTER_INSTANCE_TYPE);
        assert_eq!(u64at(o + 4), chr, "the character is keyed by its OWN soid");
        let n1 = u32at(o + 12) as usize;
        assert_eq!(n1, 13, "character payload length");
        o += 16 + n1;

        assert_eq!(o, b.len(), "the two objects must consume the body exactly");
    }

    /// ⚠ Regression guard for a trap that already produced a wrong answer once.
    ///
    /// The client's bitstream reads past the end of a short buffer and returns zeros, which is
    /// indistinguishable from "every remaining field absent". A 17-byte account payload therefore
    /// DECODES CORRECTLY while the client reads 157 bits out of 136 - 21 bits of overrun. The only
    /// defence is to size the payload from the schema's own field count.
    #[test]
    fn account_payload_is_long_enough_for_every_field_the_client_reads() {
        // fields 0-1 (134) + field 2 absent (1) + field 3 present with its u64 (65) + 21 absent
        let bits_read = 1 + 64 + 1 + 4 + 64 + 1 + 1 + 64 + (ACCOUNT_FIELD_COUNT - 4);
        assert_eq!(bits_read, 221);
        let a =
            account_rsat(0xD100_0000_0000_0001, &[0xD100_0000_0000_0101], 0xD100_0000_0000_0002);
        assert!(
            a.len() * 8 >= bits_read as usize,
            "account payload {}B = {} bits, but the client reads {bits_read}",
            a.len(),
            a.len() * 8
        );
        // and the roster's shorter tail must NOT be reused for the account
        let short = character_list_rsat(0xD100_0000_0000_0001, &[0xD100_0000_0000_0101], 2);
        assert!(short.len() * 8 < bits_read as usize, "the trap must remain reproducible");
    }

    #[test]
    fn substituting_a_soid_changes_only_the_soid_fields() {
        // The grammar is SOID-parameterised, so a different stable SOID must still encode cleanly
        // and stay the same length.
        let a = queuez_character_body(1, 2, Identity::TITAN_HUMAN_FEMALE);
        assert_eq!(a.len(), 54);
        // Header SOID and payload SOID must agree - the client checks them against each other.
        // Layout: 4 count | 4 family | 8 root | 4 version | 1 flags | 4 objcount | 4 instance
        //         | 8 char soid | 4 len | payload
        assert_eq!(&a[8..16], &1u64.to_be_bytes(), "family root = account SOID");
        assert_eq!(&a[29..37], &2u64.to_be_bytes(), "object header = character SOID");
        assert_eq!(&a[37..41], &13u32.to_be_bytes(), "payload length");
    }

    #[test]
    fn a_push_is_not_a_reply_and_carries_no_status() {
        let p = queuez_character_push(7, 1, 2, Identity::TITAN_HUMAN_FEMALE);
        assert_eq!(&p[0..2], &TYPE_QUEUEZ_OBJECT.to_be_bytes());
        assert_eq!(&p[2..6], &7u32.to_be_bytes());
        assert_eq!(p.len(), 6 + 54, "no status field on an originated request");
    }
}

#[cfg(test)]
mod xetrov_tests {
    use super::*;

    /// Exact plaintext body captured in `runs/20260807-072856-account-trace/`, after the six-byte
    /// BAP inner header. This pins all three nested protobuf layers against client-emitted bytes.
    const CAPTURED_XETROV_BODY: [u8; 84] = [
        0x08, 0x02, 0x10, 0x02, 0x22, 0x4e, 0x08, 0x03, 0x12, 0x4a, 0x08, 0x81, 0x80, 0x80,
        0x80, 0x80, 0x80, 0x80, 0x80, 0xd1, 0x01, 0x10, 0xe6, 0xca, 0xf3, 0xc3, 0xcb, 0xa5,
        0x0d, 0x1a, 0x0c, 0x36, 0x36, 0x45, 0x35, 0x37, 0x43, 0x42, 0x38, 0x32, 0x43, 0x33,
        0x35, 0x20, 0xc9, 0x9a, 0x03, 0x2a, 0x21, 0x35, 0x32, 0x35, 0x35, 0x33, 0x2e, 0x31,
        0x36, 0x2e, 0x30, 0x39, 0x2e, 0x32, 0x38, 0x2e, 0x31, 0x35, 0x35, 0x31, 0x2e, 0x65,
        0x76, 0x69, 0x6c, 0x5f, 0x5f, 0x72, 0x65, 0x6c, 0x65, 0x61, 0x73, 0x65, 0x30, 0x0d,
    ];

    #[test]
    fn parses_the_real_xetrov_request() {
        let request = parse_xetrov_request(&CAPTURED_XETROV_BODY).expect("captured protobuf parses");
        assert_eq!(request.field1, 2);
        assert_eq!(request.field2, 2);
        assert_eq!(request.field3, None);
        assert_eq!(request.envelope.field1, 3);

        let event = request.envelope.event;
        assert_eq!(event.account_soid, Some(0xD100_0000_0000_0001));
        assert_eq!(event.field2, Some(0x352c_b87c_e566));
        assert_eq!(event.field3.as_deref(), Some("66E57CB82C35"));
        assert_eq!(event.field4, Some(52_553));
        assert_eq!(event.field5.as_deref(), Some("52553.16.09.28.1551.evil__release"));
        assert_eq!(event.field6, Some(13));
    }

    #[test]
    fn xetrov_parser_rejects_missing_required_layers() {
        assert_eq!(
            parse_xetrov_request(&[0x08, 0x02, 0x10, 0x02]),
            Err(XetrovDecodeError::MissingField("top.4"))
        );
    }

    #[test]
    fn investment_206_response_matches_the_native_oracle() {
        assert_eq!(
            investment_206_response(6, STATUS_OK),
            vec![0x00, 0x0b, 0x00, 0x00, 0x00, 0x06, 0x00, 0xc8, 0x00, 0xce, 0x10]
        );
    }
}

#[cfg(test)]
mod unlocks_object_tests {
    use super::*;

    fn h(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    /// **The regression control, and the most important test here.** Passing `None` must leave the
    /// two-object baseline byte-for-byte identical. The route that reached orbit ran WITHOUT this
    /// object, and family 4/type 4 is also involved in the character-prologue path - so the default
    /// must stay exactly what it was, provably, not by inspection.
    #[test]
    fn none_is_byte_identical_to_the_two_object_baseline() {
        let prof = vec![0xAA; 32];
        assert_eq!(
            queuez_family4_baseline_body_raw_profile(1, &[2], 3, &prof),
            queuez_family4_baseline_body_raw_profile_with_unlocks(1, &[2], 3, &prof, None, &[]),
        );
        assert_eq!(
            queuez_family4_baseline_body_with_destination(1, &[2], 3, None),
            queuez_family4_baseline_body_with_unlocks(1, &[2], 3, None, None, &[]),
        );
        // object_count must still read 2 - the field the client's family-update path consumes.
        let b = queuez_family4_baseline_body(1, &[2], 3);
        assert_eq!(u32::from_be_bytes(b[HDR_COUNT..HDR_LEN].try_into().unwrap()), 2);
    }

    /// The header offsets the tests below index by, pinned against `queuez_family_header_for`:
    /// `[0..4] family count · [4..8] family · [8..16] root SOID · [16..20] version · [20] flag ·
    /// [21..25] object_count`.
    const HDR_COUNT: usize = 21;
    const HDR_LEN: usize = 25;

    #[test]
    fn header_offsets_are_what_the_other_tests_assume() {
        let b = queuez_family_header_for(QUEUEZ_FAMILY_ACCOUNT, 0xD1, 7);
        assert_eq!(b.len(), HDR_LEN);
        assert_eq!(u32::from_be_bytes(b[HDR_COUNT..HDR_LEN].try_into().unwrap()), 7);
        assert_eq!(HDR_LEN, QUEUEZ_SPAN_HEADER_LEN);
    }

    /// The versioned builder must reproduce the old one exactly at `(0, BASELINE)` - otherwise
    /// every existing body silently changes shape and the 136 tests above are testing a new format.
    #[test]
    fn versioned_header_at_version_0_baseline_is_the_old_header() {
        for (fam, root, n) in [(4u32, 0xD1u64, 2u32), (3, 0xD100_0000_0000_0001, 0), (0, 7, 9)] {
            assert_eq!(
                queuez_family_header_for(fam, root, n),
                queuez_family_header_versioned(fam, root, n, 0, QUEUEZ_FLAG_BASELINE),
            );
        }
        let d = queuez_family_header_versioned(4, 0xD1, 0, 1, QUEUEZ_FLAG_DELTA);
        assert_eq!(u32::from_be_bytes(d[16..20].try_into().unwrap()), 1);
        assert_eq!(d[20] & 1, 0, "bit 0 clear is what 0x56AFE8 `test byte [r15],1` reads as DELTA");
    }

    /// ⭐⭐ **P2's control axis, at the wire-format layer.**
    ///
    /// *"The second family-4 delivery is a DELTA the client accepts at replication state 3 - flags
    /// bit 0 CLEAR and version == the baseline's version + 1 - and it carries the baseline's
    /// objects byte-for-byte, so nothing can be purged and no field can be dropped."*
    ///
    /// The inversion is shipped as its own assertion rather than promised: re-stamping with
    /// `QUEUEZ_FLAG_BASELINE` puts the delivery on `0x56B2D0`'s purge arm, and this test would go
    /// red on the `flags & 1 == 0` line.
    #[test]
    fn family4_delta_is_a_verbatim_restamp_of_the_baseline() {
        let base = queuez_family4_baseline_body_with_unlocks(
            0xD100_0000_0000_0001,
            &[0xD100_0000_0000_0101],
            0xD100_0000_0000_0201,
            Some(DESTINATION_OLD_RUSSIA),
            Some(UNLOCKS_KEY_ONLY_PAYLOAD),
            &[(0x1234_5678, 0xD100_0000_0000_0301, vec![0xAA; 7])],
        );
        assert_eq!(queuez_span_version(&base), Some(0));
        assert_eq!(queuez_span_flags(&base), Some(QUEUEZ_FLAG_BASELINE));
        assert_eq!(queuez_span_object_count(&base), Some(4));

        let delta = queuez_span_as_delta(&base, 1).expect("the baseline must be walkable");
        assert_eq!(delta.len(), base.len());
        assert_eq!(queuez_span_flags(&delta).unwrap() & 1, 0, "DELTA, not a purge-arm baseline");
        assert_eq!(queuez_span_version(&delta), Some(queuez_span_version(&base).unwrap() + 1));
        // Everything that is not the version or the flags byte is untouched: the objects, the
        // family, the root and the count. This is the "enumerate, do not spot-check" guarantee -
        // there is nothing to enumerate because nothing was rebuilt.
        assert_eq!(delta[..16], base[..16]);
        assert_eq!(delta[21..], base[21..]);
        assert_eq!(queuez_span_objects(&delta), queuez_span_objects(&base));

        // The walker is the guard, so prove it discriminates: a truncated span must not walk.
        assert!(queuez_span_objects(&base[..base.len() - 1]).is_none());
        assert!(queuez_span_as_delta(&base[..base.len() - 1], 1).is_none());
    }

    /// The purge-free minimum: zero objects still reaches `0x56B5E0`, so it still invalidates.
    #[test]
    fn family4_delta_ping_is_a_zero_object_delta() {
        let p = queuez_family4_delta_ping(0xD100_0000_0000_0001, 1);
        assert_eq!(p.len(), QUEUEZ_SPAN_HEADER_LEN, "header only - no object may ride along");
        assert_eq!(queuez_span_family(&p), Some(QUEUEZ_FAMILY_ACCOUNT));
        assert_eq!(queuez_span_object_count(&p), Some(0));
        assert_eq!(queuez_span_flags(&p).unwrap() & 1, 0);
        assert_eq!(queuez_span_version(&p), Some(1));
        assert_eq!(queuez_span_objects(&p), Some(vec![]));
    }

    /// ⭐ **The brief's own axis: does the re-delivery classify as UPDATE?**
    ///
    /// `0x56B5F0` announces `0x56D3F0` (UPDATE) only when `memcmp(old, decoded, stride) != 0`. The
    /// serial is the opt-in that makes it differ; **serial 0 is the inversion**, asserted here in
    /// the same test so it cannot be quietly dropped: byte-identical objects ⇒ the classifier
    /// announces NOTHING, and any control that claims otherwise is measuring the wrong function.
    ///
    /// Decoded through the client's own codec, not by analogy:
    /// `d1-oracle object --tag 0x808018c7 --decode <payload>` gives `+0x0060 = 7` for serial 7 and
    /// no `+0x0060` row at all for serial 0, with every other decoded qword identical.
    #[test]
    fn account_serial_is_what_routes_the_second_delivery_to_the_update_announcer() {
        let (acct, chr, prof) =
            (0xD100_0000_0000_0001u64, 0xD100_0000_0000_0101u64, 0xD100_0000_0000_0201u64);
        let plain = account_rsat(acct, &[chr], prof);

        // INVERSION, shipped: serial 0 is byte-identical, so memcmp == 0, so NOTHING is announced.
        assert_eq!(account_rsat_with_serial(acct, &[chr], prof, 0), plain);
        // And the positive: one 32-bit leaf differs, which is all memcmp needs.
        let bumped = account_rsat_with_serial(acct, &[chr], prof, 7);
        assert_ne!(bumped, plain);
        assert_eq!(h(&plain), "e880000000000000c74400000000000405d100000000000201000000");
        assert_eq!(h(&bumped), "e880000000000000c744000000000004060000000fd100000000000201000000");

        // The splice must land on the ACCOUNT object and change nothing else in the span.
        let base = queuez_family4_baseline_body(acct, &[chr], prof);
        let spliced =
            queuez_span_replace_object_payload(&base, 0, ACCOUNT_INSTANCE_TYPE, &bumped).unwrap();
        let (bo, so) = (queuez_span_objects(&base).unwrap(), queuez_span_objects(&spliced).unwrap());
        assert_eq!(bo.len(), so.len());
        assert_ne!(&base[bo[0].payload.0..bo[0].payload.1], &spliced[so[0].payload.0..so[0].payload.1]);
        assert_eq!(&base[bo[1].payload.0..bo[1].payload.1], &spliced[so[1].payload.0..so[1].payload.1]);
        assert_eq!(so[1].instance_type, PROFILE_INSTANCE_TYPE);
        // The guard is a guard: splicing an account payload over the PROFILE object is refused.
        assert!(queuez_span_replace_object_payload(&base, 1, ACCOUNT_INSTANCE_TYPE, &bumped).is_none());
        assert!(queuez_span_replace_object_payload(&base, 9, ACCOUNT_INSTANCE_TYPE, &bumped).is_none());
    }

    /// The payload is the client's own decoder's answer, not ours:
    /// `d1-oracle object --tag 0x8080102E --decode bfffffffffffffff00`
    /// -> `decoded 67 bits -> 3984-byte object / +0x0000 0x7ffffffffffffffe`.
    #[test]
    fn key_only_payload_is_the_pinned_vector() {
        assert_eq!(h(UNLOCKS_KEY_ONLY_PAYLOAD), "bfffffffffffffff00");
        assert_eq!(UNLOCKS_KEY_ONLY_PAYLOAD.len(), 9, "67 bits rounds up to 9 bytes");
    }

    /// ⚠ **Negative control for a documented near-miss.** `DESTINATION-PLAN.md` cited
    /// `e88000000000000080` as this object's key-only vector. That payload decodes to
    /// `0xD100000000000001` - the ACCOUNT SOID. And a single flipped bit,
    /// `bfffffffffffffff80`, decodes to `0x7FFFFFFFFFFFFFFF`, the family-5 root we already serve.
    /// All three were checked through the oracle. Assert we are not carrying either neighbour.
    #[test]
    fn key_is_not_the_account_soid_nor_the_family5_root() {
        assert_ne!(h(UNLOCKS_KEY_ONLY_PAYLOAD), "e88000000000000080", "that is the account SOID");
        assert_ne!(h(UNLOCKS_KEY_ONLY_PAYLOAD), "bfffffffffffffff80", "that is the family-5 root");
        assert_eq!(UNLOCKS_OBJECT_KEY, 0x7FFF_FFFF_FFFF_FFFE);
        assert_eq!(UNLOCKS_OBJECT_KEY + 1, 0x7FFF_FFFF_FFFF_FFFF, "one below the family-5 root");
    }

    /// With unlocks present: count becomes 3, the first two records are untouched, and the third
    /// carries the verified instance checksum and key.
    #[test]
    fn third_object_is_appended_not_substituted() {
        let prof = vec![0xAA; 32];
        let two = queuez_family4_baseline_body_raw_profile(1, &[2], 3, &prof);
        let three = queuez_family4_baseline_body_raw_profile_with_unlocks(
            1, &[2], 3, &prof, Some(UNLOCKS_KEY_ONLY_PAYLOAD), &[],
        );

        // The three-object body is the two-object body with a record appended - same prefix except
        // for object_count. That is what "ADD it; never replace" has to mean in bytes.
        assert_eq!(&three[..HDR_COUNT], &two[..HDR_COUNT]);
        assert_eq!(u32::from_be_bytes(three[HDR_COUNT..HDR_LEN].try_into().unwrap()), 3);
        assert_eq!(&three[HDR_LEN..two.len()], &two[HDR_LEN..], "account and profile records untouched");

        let tail = &three[two.len()..];
        assert_eq!(u32::from_be_bytes(tail[0..4].try_into().unwrap()), UNLOCKS_INSTANCE_TYPE);
        assert_eq!(u64::from_be_bytes(tail[4..12].try_into().unwrap()), UNLOCKS_OBJECT_KEY);
        assert_eq!(u32::from_be_bytes(tail[12..16].try_into().unwrap()), 9);
        assert_eq!(&tail[16..], UNLOCKS_KEY_ONLY_PAYLOAD);
        assert_eq!(tail.len(), 4 + 8 + 4 + 9);
    }

    /// The family header's root must stay the ACCOUNT SOID. The object key goes in the object
    /// record; putting it in the header instead is the obvious wrong move, so pin against it.
    #[test]
    fn family_root_stays_the_account_soid() {
        let acct = 0xD100_0000_0000_0001u64;
        let b = queuez_family4_baseline_body_with_unlocks(
            acct, &[2], 3, None, Some(UNLOCKS_KEY_ONLY_PAYLOAD), &[],
        );
        assert_eq!(u32::from_be_bytes(b[4..8].try_into().unwrap()), QUEUEZ_FAMILY_ACCOUNT);
        assert_eq!(u64::from_be_bytes(b[8..16].try_into().unwrap()), acct);
        assert_ne!(u64::from_be_bytes(b[8..16].try_into().unwrap()), UNLOCKS_OBJECT_KEY);
    }

    /// Family 4/type 2 rides as an `extras` entry and pushes `object_count` to 4, without
    /// disturbing the three objects in front of it.
    #[test]
    fn family42_appends_as_a_fourth_object() {
        let prof = vec![0xAA; 32];
        let three = queuez_family4_baseline_body_raw_profile_with_unlocks(
            1, &[2], 3, &prof, Some(UNLOCKS_KEY_ONLY_PAYLOAD), &[],
        );
        let four = queuez_family4_baseline_body_raw_profile_with_unlocks(
            1, &[2], 3, &prof, Some(UNLOCKS_KEY_ONLY_PAYLOAD),
            &[(FAMILY42_INSTANCE_TYPE, 1, FAMILY42_KEY_ONLY_PAYLOAD.to_vec())],
        );
        assert_eq!(u32::from_be_bytes(four[HDR_COUNT..HDR_LEN].try_into().unwrap()), 4);
        assert_eq!(&four[HDR_LEN..three.len()], &three[HDR_LEN..], "first three untouched");
        let tail = &four[three.len()..];
        assert_eq!(u32::from_be_bytes(tail[0..4].try_into().unwrap()), FAMILY42_INSTANCE_TYPE);
        assert_eq!(u64::from_be_bytes(tail[4..12].try_into().unwrap()), 1, "keyed by the account SOID");
        assert_eq!(&tail[16..], FAMILY42_KEY_ONLY_PAYLOAD);
    }

    /// The two new instance types must be distinct from every one we already serve, and from each
    /// other - a copy-paste that reused a checksum would look wired and address the wrong slot.
    #[test]
    fn family42_and_inventory_checksums_are_distinct() {
        assert_eq!(FAMILY42_INSTANCE_TYPE, 0x0ECC_D1F8);
        assert_eq!(INVENTORY_INSTANCE_TYPE, 0xEF38_431D);
        let all = [ACCOUNT_INSTANCE_TYPE, PROFILE_INSTANCE_TYPE, CHARACTER_INSTANCE_TYPE,
                   ROSTER_INSTANCE_TYPE, UNLOCKS_INSTANCE_TYPE, FAMILY42_INSTANCE_TYPE,
                   INVENTORY_INSTANCE_TYPE];
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] { assert_ne!(a, b, "instance types must be unique"); }
        }
    }

    /// The 73-byte payload is the oracle's, and it decodes to the ACCOUNT SOID - not to either of
    /// the two sentinels that bit us on family 4/4.
    #[test]
    fn family42_payload_is_the_oracle_vector() {
        assert_eq!(FAMILY42_KEY_ONLY_PAYLOAD.len(), 73, "579 bits rounds up to 73 bytes");
        assert!(FAMILY42_KEY_ONLY_PAYLOAD.starts_with(&[0xe8, 0x80]), "field-0 presence + SOID");
        assert_ne!(FAMILY42_KEY_ONLY_PAYLOAD, UNLOCKS_KEY_ONLY_PAYLOAD);
    }

    /// The checksum, and its four positive controls - the same four objects we already serve, read
    /// at the same `+0x08` offset of the same kind of instance descriptor
    /// (`d1-replay read orbit-all.snap v:0x131AC30`, 10/10 snapshot controls).
    #[test]
    fn instance_checksum_sits_alongside_the_four_known_good_ones() {
        assert_eq!(UNLOCKS_INSTANCE_TYPE, 0xC406_0975);
        for known in [
            ACCOUNT_INSTANCE_TYPE,
            CHARACTER_INSTANCE_TYPE,
            ROSTER_INSTANCE_TYPE,
            PROFILE_INSTANCE_TYPE,
        ] {
            assert_ne!(UNLOCKS_INSTANCE_TYPE, known, "must be a distinct instance type");
        }
    }
}

#[cfg(test)]
mod orbit_destination_tests {
    use super::*;

    /// **The control.** The SOID-only profile must still be the pinned 11-byte / 82-bit vector, so a
    /// change to the destination path cannot silently alter the object that already works.
    #[test]
    fn plain_profile_is_unchanged() {
        let h = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
        assert_eq!(h(&profile_rsat(0xD100_0000_0000_0201)), "e880000000000100800000");
        assert_eq!(
            queuez_family4_baseline_body(1, &[2], 3),
            queuez_family4_baseline_body_with_destination(1, &[2], 3, None),
            "None must be byte-identical to the old builder"
        );
    }

    /// Bit budget, derived from the grammar rather than from the output:
    /// `1 (f0) + 64 (SOID) + 1 (f1 absent) + 1 (f2 present) + 12 sub-presence + 32 (destination)
    ///  + 15 (f3..f17 absent) = 126 bits -> 16 bytes`.
    #[test]
    fn destination_profile_has_the_predicted_bit_budget() {
        let b = profile_rsat_with_destination(0xD100_0000_0000_0201, DESTINATION_OLD_RUSSIA);
        assert_eq!(b.len(), 16, "126 bits rounds up to 16 bytes");
        // The SOID prefix is shared with the plain form, so the head must still match.
        let h = b.iter().map(|x| format!("{x:02x}")).collect::<String>();
        assert!(h.starts_with("e88000000000010"), "field 0 still carries the profile SOID");
    }

    /// The destination must appear on the wire MSB-first, like every other id in this protocol.
    /// Searching for the raw bytes is not enough - they are bit-shifted - so this asserts the value
    /// round-trips out of the bitstream at the position the grammar predicts.
    #[test]
    fn destination_lands_at_the_predicted_bit_offset() {
        let b = profile_rsat_with_destination(0xD100_0000_0000_0201, DESTINATION_OLD_RUSSIA);
        let bits: Vec<bool> = b.iter().flat_map(|byte| (0..8).rev().map(move |i| (byte >> i) & 1 == 1)).collect();
        // 1 + 64 + 1 + 1 = 67 bits of header, then 10 absent sub-fields, then the presence bit.
        let start = 1 + 64 + 1 + 1 + PROFILE_SETTINGS_DESTINATION_FIELD + 1;
        let got = bits[start..start + 32].iter().fold(0u32, |a, &b| (a << 1) | b as u32);
        assert_eq!(got, DESTINATION_OLD_RUSSIA, "Old Russia, Earth, MSB-first");
        assert!(bits[start - 1], "the destination sub-field's presence bit is set");
    }

    /// The sentinel must never be what we send - that value *is* "unset" and reproduces the bug.
    #[test]
    fn we_never_send_the_unset_sentinel() {
        const FNV_BASIS: u32 = 0x811C_9DC5;
        assert_ne!(DESTINATION_OLD_RUSSIA, FNV_BASIS);
        let b = profile_rsat_with_destination(1, DESTINATION_OLD_RUSSIA);
        let plain = profile_rsat(1);
        assert_ne!(b, plain, "populating the destination must change the bytes");
    }
}

#[cfg(test)]
mod investment_104_tests {
    use super::*;

    /// Pinned against the oracle's wire for 104 (`0068` + body), and against 504's body, since both
    /// responses carry the same `0x80801796` composite. If the two ever diverge, one of them was
    /// changed without re-deriving it.
    #[test]
    fn body_matches_the_oracle_and_the_shared_composite() {
        assert_eq!(investment_104_response_body(), vec![0x18, 0x00, 0x00, 0x00, 0x00]);
        assert_eq!(
            investment_104_response_body(),
            investment_504_response_body(),
            "0x808018F8[0] and 0x80801845 are both the 0x80801796 {{u4,u32}} composite"
        );
    }

    /// The regression guard for the actual defect: 104 used to fall through to `None`, so the reply
    /// carried a message id and no object and the client answered `Failed to decode [policy 1]`.
    #[test]
    fn body_is_never_empty() {
        assert!(
            !investment_104_response_body().is_empty(),
            "an id with no object is what produced `Failed to decode [policy 1]`"
        );
    }
}

#[cfg(test)]
mod client_config_tests {
    use super::*;

    /// Descriptor `0x15A7BA0` marks fields #1 and #2 REQUIRED, so both must be on the wire. A
    /// header-only reply - which is correct for `0x0016` - cannot parse here.
    #[test]
    fn body_carries_both_required_fields() {
        let b = client_config_response_body(0, 0);
        assert_eq!(b, vec![0x08, 0x00, 0x10, 0x00], "field 1 varint 0, field 2 varint 0");
        assert_eq!(b[0] >> 3, 1, "protobuf field number 1 - cookie");
        assert_eq!(b[0] & 7, 0, "wire type 0 = varint");
        assert_eq!(b[2] >> 3, 2, "protobuf field number 2 - social_matchmaking_id");
        assert_eq!(b[2] & 7, 0, "wire type 0 = varint");
        assert!(!b.is_empty(), "two REQUIRED fields ⇒ never header-only");
    }

    /// Field #3 is an OPTIONAL callback whose handler demands a leading varint `0x123456` and aborts
    /// the whole parse otherwise. Omitting it is deliberate - sending it malformed would convert a
    /// working reply into a total parse failure.
    #[test]
    fn body_omits_the_callback_field() {
        let b = client_config_response_body(0, 0);
        assert!(!b.iter().any(|&x| x >> 3 == 3 && x & 7 == 2), "no field 3 on the wire");
    }

    /// Multi-byte values must be LEB128, not fixed width - `cookie` is a `%lld`.
    #[test]
    fn cookie_is_leb128_not_fixed_width() {
        assert_eq!(client_config_response_body(300, 0)[..3], [0x08, 0xac, 0x02]);
        assert_eq!(client_config_response_body(0, 300)[2..], [0x10, 0xac, 0x02]);
    }

    #[test]
    fn reply_has_a_class_2_header_echoing_the_sequence() {
        let r = client_config_response(26, 0, 0);
        assert_eq!(u16::from_be_bytes([r[0], r[1]]), TYPE_CLIENT_CONFIG_RESPONSE);
        assert_eq!(u32::from_be_bytes([r[2], r[3], r[4], r[5]]), 26, "sequence 26, as captured");
        assert_eq!(r.len(), REPLY_HEADER_LEN + 4);
    }
}

#[cfg(test)]
mod account_id_translation_tests {
    use super::*;

    /// The real `0x0017` the client sent, captured verbatim from
    /// `runs/20260807-222725-MSG205-FAMILY5-ROOT/responder.log` - 46 B, inner sequence 17.
    /// `00 17` type, `00 00 00 11` sequence, then the 40-byte body.
    const CAPTURED_0017: [u8; 46] = [
        0x00, 0x17, 0x00, 0x00, 0x00, 0x11, // header: type 0x0017, sequence 17
        0x00, 0x01, // count = 1
        0x07, 0xff, // from = platform, to = SOID
        0x44, 0x31, 0x47, 0x75, 0x61, 0x72, 0x64, 0x69, 0x61, 0x6e, // "D1Guardian"
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    /// **The load-bearing test: real captured bytes, parsed.** If the header layout read out of
    /// `0x55A490` were wrong, the count or the discriminators would come back nonsense here.
    #[test]
    fn parses_the_real_captured_request() {
        let body = &CAPTURED_0017[6..];
        assert_eq!(body.len(), 40, "the captured 0x0017 body is 40 B");

        let (count, kind_a, kind_b) = account_id_translation_request(body)
            .expect("the real request must parse, and its discriminators must be in {7, 0xFF}");
        assert_eq!(count, 1, "the client asked us to translate exactly one id");
        assert_eq!(kind_a, AIDT_KIND_PLATFORM_ID, "list A is platform identifiers");
        assert_eq!(kind_b, AIDT_KIND_ACCOUNT_SOID, "list B is investment SOIDs");

        // The 36-byte identifier is the online id we hand out at Layer A, so the mapping is ours.
        let ids = account_id_translation_request_ids(body);
        assert_eq!(ids.len(), 1, "one identifier, exactly as the count promised");
        assert_eq!(ids[0], platform_identifier("D1Guardian"),
                   "the requested identifier is our own online id, 36 B NUL-padded");
    }

    /// The decoder rejects any discriminator outside {7, 0xFF} before touching the payload, so our
    /// parser must too - otherwise we would answer a request the client would never have sent.
    #[test]
    fn rejects_discriminators_the_client_would_reject() {
        assert!(account_id_translation_request(&[0x00, 0x01, 0x07, 0xff]).is_some());
        assert!(account_id_translation_request(&[0x00, 0x01, 0x00, 0xff]).is_none(), "0x00 is not 7 or 0xFF");
        assert!(account_id_translation_request(&[0x00, 0x01, 0x07, 0x08]).is_none(), "0x08 is not 7 or 0xFF");
        assert!(account_id_translation_request(&[0x00, 0x01]).is_none(), "short body");
    }

    /// **The regression guard for the wrong answer I nearly shipped.** Reading only the encoder's
    /// `cmp [hdr+0], 0xFF` branch suggests a bare SOID array with the kinds swapped to `ff 07`. The
    /// decoder reads TWO parallel lists (`0x55A7D7`, `0x55A7EF`) and `0x55A8E3` searches list A for
    /// the client's own identifier - so a SOID-only body leaves it unable to find its row, and the
    /// 36-byte read would run off the end of an 8-byte payload.
    #[test]
    fn reply_carries_both_lists_not_a_bare_soid_array() {
        let seq = 17u32;
        let entries = [(platform_identifier("D1Guardian"), 0xD100_0000_0000_0001u64)];
        let r = account_id_translation_response(seq, &entries);
        let body = &r[REPLY_HEADER_LEN..];

        assert_eq!(body[2], AIDT_KIND_PLATFORM_ID, "list A is identifiers - NOT 0xFF");
        assert_eq!(body[3], AIDT_KIND_ACCOUNT_SOID, "list B is SOIDs");
        assert_eq!(
            body.len(),
            4 + AIDT_PLATFORM_ID_LEN + 8,
            "header + one 36-byte identifier + one 64-bit SOID - a bare SOID array would be 12 B"
        );
        assert!(body[4..].starts_with(b"D1Guardian"), "list A echoes the identifier verbatim");
        assert_eq!(
            &body[4 + AIDT_PLATFORM_ID_LEN..],
            &0xD100_0000_0000_0001u64.to_be_bytes(),
            "list B follows list A - parallel, not interleaved"
        );
    }

    /// The reply must echo the request's sequence, or it lands on the wrong in-flight queue entry -
    /// which is the very failure (`failed_to_find_message_recipient`) this message exists to cure.
    #[test]
    fn reply_echoes_the_requests_sequence() {
        let seq = u32::from_be_bytes([CAPTURED_0017[2], CAPTURED_0017[3], CAPTURED_0017[4], CAPTURED_0017[5]]);
        assert_eq!(seq, 17);

        let r = account_id_translation_response(seq, &[(platform_identifier("D1Guardian"), 1)]);
        assert_eq!(u16::from_be_bytes([r[0], r[1]]), TYPE_ACCOUNT_ID_XLAT_RESPONSE);
        assert_eq!(u32::from_be_bytes([r[2], r[3], r[4], r[5]]), seq, "sequence is echoed");
    }

    /// End-to-end over the captured request: whatever identifiers it asked about come back paired
    /// with our account SOID, in the same order and the same count.
    #[test]
    fn answers_the_captured_request_with_our_account_soid() {
        const ACCT: u64 = 0xD100_0000_0000_0001;
        let body = &CAPTURED_0017[6..];
        let ids = account_id_translation_request_ids(body);
        let entries: Vec<_> = ids.iter().map(|id| (*id, ACCT)).collect();

        let r = account_id_translation_response(17, &entries);
        assert_eq!(
            u16::from_be_bytes([r[REPLY_HEADER_LEN], r[REPLY_HEADER_LEN + 1]]),
            1,
            "the reply's count matches the request's count"
        );
        assert_eq!(r.len(), REPLY_HEADER_LEN + 4 + AIDT_PLATFORM_ID_LEN + 8);
    }
}

#[cfg(test)]
mod investment_205_tests {
    use super::*;

    /// **The load-bearing invariant.** 205's response `0x80801827` and 503's response `0x80801825`
    /// embed the *same* nested 944-byte struct `0x80801941`, whose field [0] is the QueueZ family-5
    /// root SOID. So the two bodies must agree byte-for-byte over that struct's encoding - 503 from
    /// index 21, 205 from index 1 (the leading byte differs only in the presence bits packed above
    /// it, since the struct starts at a different C offset in each message).
    ///
    /// This is the regression guard for `error.investment_server_object_family_record_not_found`:
    /// the body that produced it had zeros here, which the client read as "family 5, root 0".
    #[test]
    fn family5_root_tail_matches_503() {
        let b205 = investment_205_response_body();
        assert_eq!(b205.len(), 25, "205 body is 25 B after the 2-byte message id");
        assert_eq!(INVESTMENT_503_PINNED_RAW.len(), 45, "503's pinned raw body is 45 B");

        assert_eq!(
            &b205[1..],
            &INVESTMENT_503_PINNED_RAW[21..],
            "205 and 503 must encode the shared 0x80801941 struct identically - \
             if this fails, one of the two is declaring a different family-5 root"
        );
    }

    /// ⭐ The **same** invariant for the unlock-carrying pair. This is the one that matters now:
    /// a `D1_503_RAW` carrying flags with a 205 that does not is exactly the "same struct, two
    /// answers" shape that produced `family_record_not_found` - and 205 lands *last*.
    #[test]
    fn unlock_pair_shares_the_same_0x80801941_tail() {
        assert_eq!(INVESTMENT_205_UNLOCKS_BODY.len(), 50);
        assert_eq!(INVESTMENT_503_UNLOCKS_RAW.len(), 70);
        assert_eq!(
            &INVESTMENT_205_UNLOCKS_BODY[1..],
            &INVESTMENT_503_UNLOCKS_RAW[21..],
            "the unlock 205/503 pair must encode the shared 0x80801941 struct identically"
        );
        // The superseded union-8 pair must still agree with ITSELF, for the same reason v1 must.
        assert_eq!(INVESTMENT_205_UNLOCKS_BODY_V2_UNION8.len(), 45);
        assert_eq!(INVESTMENT_503_UNLOCKS_RAW_V2_UNION8.len(), 65);
        assert_eq!(
            &INVESTMENT_205_UNLOCKS_BODY_V2_UNION8[1..],
            &INVESTMENT_503_UNLOCKS_RAW_V2_UNION8[21..],
        );
        // The superseded v1 pair must still agree with ITSELF - it is kept as a reproducible
        // record of the 20260809-075545 run, and a v1 that no longer round-trips is a lost record.
        assert_eq!(
            &INVESTMENT_205_UNLOCKS_BODY_V1_ACT393_ONLY[1..],
            &INVESTMENT_503_UNLOCKS_RAW_V1_ACT393_ONLY[21..],
        );
        // ⭐ And v2 must NOT equal v1 - the whole point of the revision.
        assert_ne!(INVESTMENT_503_UNLOCKS_RAW, INVESTMENT_503_UNLOCKS_RAW_V1_ACT393_ONLY);
        assert_ne!(INVESTMENT_205_UNLOCKS_BODY, INVESTMENT_205_UNLOCKS_BODY_V1_ACT393_ONLY);
    }

    /// ⭐ The **negative** half: the unlock pair must differ from the pinned pair in exactly the
    /// flag region and nowhere else. Without this, a copy-paste that dropped the flags would pass
    /// every other test in this file.
    #[test]
    fn unlock_pair_actually_carries_flags_and_keeps_the_family5_root() {
        assert_ne!(
            INVESTMENT_205_UNLOCKS_BODY,
            investment_205_response_body().as_slice(),
            "the unlock body must not be the flagless pinned body"
        );
        assert_ne!(INVESTMENT_503_UNLOCKS_RAW, INVESTMENT_503_PINNED_RAW);
        // The family-5 root is the leading `ff ff ff ff ff ff ff fc` of the shared struct and is
        // untouched by the flag insertion - the whole point of the 20260807-215628 regression.
        assert_eq!(&INVESTMENT_205_UNLOCKS_BODY[1..9], &investment_205_response_body()[1..9]);
        assert_eq!(&INVESTMENT_503_UNLOCKS_RAW[21..29], &INVESTMENT_503_PINNED_RAW[21..29]);
        // …and the flagged body must be identical to the pinned one up to the point where the
        // `0x80804681` count begins, then diverge and stay longer. Computed, not hardcoded: a
        // literal prefix length silently becomes wrong the moment the flag list changes size.
        let first_diff = INVESTMENT_503_UNLOCKS_RAW
            .iter()
            .zip(INVESTMENT_503_PINNED_RAW)
            .position(|(a, b)| a != b)
            .expect("the flagged 503 body must differ from the pinned one somewhere");
        assert_eq!(
            first_diff, 37,
            "the two bodies must agree byte-for-byte up to the unlock-flag count, which the \
             descriptor puts at wire byte 37 - everything before it is the account SOID, the \
             family-5 root and the leading presence bits"
        );
        assert!(INVESTMENT_503_UNLOCKS_RAW.len() > INVESTMENT_503_PINNED_RAW.len());
    }

    /// ⭐⭐⭐ **THE CONTROL AXIS OF THE TEN-FLAG CHANGE, stated as a decode.**
    ///
    /// *The 503 body and the 205 body each decode to the same 10-entry `0x80804681` array with
    /// value 2 at exactly `{98, 308, 419, 440, 834, 2667, 3389, 5015, 5987, 6018}`.*
    ///
    /// ⚠ This is deliberately **not** a byte-comparison against a hex string. Every other assertion
    /// in this file compares one constant to another, which is why eight flags could masquerade as
    /// ten for as long as the lengths happened to line up. Reading the array back out asks the only
    /// question the client asks.
    ///
    /// The inverted half is `unlock_control_goes_red_when_an_index_is_dropped` below.
    #[test]
    fn the_56578_bodies_carry_the_56578_ordinals_and_agree() {
        let want: Vec<(u16, u8)> = UNLOCK_FLAGS_56578.iter().map(|&i| (i, UNLOCK_FLAG_VALUE)).collect();
        assert_eq!(unlock_flags_in_shared_tail(&INVESTMENT_503_UNLOCKS_RAW_56578[21..]), Some(want.clone()));
        assert_eq!(unlock_flags_in_shared_tail(&INVESTMENT_205_UNLOCKS_BODY_56578[1..]), Some(want));
        assert_eq!(INVESTMENT_503_UNLOCKS_RAW_56578[21..], INVESTMENT_205_UNLOCKS_BODY_56578[1..]);
        // Five ordinals differ from 01.29's, so the 01.29 body must NOT decode to this set.
        assert_ne!(INVESTMENT_503_UNLOCKS_RAW_56578, INVESTMENT_503_UNLOCKS_RAW);
    }

    #[test]
    fn served_bodies_decode_to_the_ten_flags_at_value_two() {
        for (name, tail) in [
            ("503", &INVESTMENT_503_UNLOCKS_RAW[21..]),
            ("205", &INVESTMENT_205_UNLOCKS_BODY[1..]),
        ] {
            let flags = unlock_flags_in_shared_tail(tail)
                .unwrap_or_else(|| panic!("{name}: body too short to hold the 0x80804681 count"));
            assert_eq!(flags.len(), 10, "{name}: must carry exactly ten flags");
            assert_eq!(
                flags.iter().map(|&(i, _)| i).collect::<Vec<_>>(),
                UNLOCK_FLAG_INDICES.to_vec(),
                "{name}: the ten indices, in order"
            );
            assert!(
                flags.iter().all(|&(_, v)| v == UNLOCK_FLAG_VALUE),
                "{name}: 0x28CACD is an EXACT `cmp byte, 2` - 1 and 3 both fire the refusal"
            );
            // The two poison indices measured in rung33-permissions-gate.md §3.2, which flip the
            // failure from reason 0x17 to 0x18 and read as a brand-new codename.
            for poison in [306u16, 3872, 3876, 3877] {
                assert!(!flags.iter().any(|&(i, _)| i == poison), "{name}: poison flag {poison}");
            }
        }
    }

    /// ⭐ **The inverted control, kept in the tree rather than only performed once.**
    ///
    /// Drop `2667` and the same assertion must fail on every axis at once: nine entries, a shorter
    /// body, and a different index list. A control whose failure mode has never been executed is
    /// decoration, and this one is executed on every `cargo test`.
    #[test]
    fn unlock_control_goes_red_when_an_index_is_dropped() {
        let nine: Vec<u16> =
            UNLOCK_FLAG_INDICES.iter().copied().filter(|&i| i != 2667).collect();
        let (b503, b205) = unlock_flag_bodies(&nine).expect("nine flags fit the container");
        // Length: an odd count shifts the 53-bit tail off its byte phase, so 68/48 not 70/50.
        assert_eq!((b503.len(), b205.len()), (68, 48));
        assert_ne!(b503.as_slice(), INVESTMENT_503_UNLOCKS_RAW);
        assert_ne!(b205.as_slice(), INVESTMENT_205_UNLOCKS_BODY);
        let flags = unlock_flags_in_shared_tail(&b205[1..]).unwrap();
        assert_eq!(flags.len(), 9, "the decode must SEE the missing flag, not round it up");
        assert!(!flags.iter().any(|&(i, _)| i == 2667));
        // And the ten-flag body must still pass the assertion the nine-flag body fails.
        assert_eq!(unlock_flags_in_shared_tail(&INVESTMENT_205_UNLOCKS_BODY[1..]).unwrap().len(), 10);
    }

    /// ⭐⭐ **The builder is checked against FOUR independently-pinned oracle-derived bodies.**
    ///
    /// The v1 (4), union-8 (8) and pinned (0) constants were emitted by the client's own encoder in
    /// three earlier sessions; the ten-flag pair was emitted by it on 2026-08-10. If the measured
    /// bit grammar were wrong in any field - element width, the `^ 0x8000` offset binary, the
    /// `(value + 1) & 3` encoding, the count offset - at least one of the four would disagree.
    ///
    /// ⚠ This is what keeps [`unlock_flag_bodies`] subordinate to the oracle. It is a convenience
    /// for regenerating a set offline; it is never the reason we believe a body is correct.
    #[test]
    fn unlock_bodies_match_the_pinned_oracle_output() {
        let cases: [(&[u16], &[u8], &[u8]); 4] = [
            (&[], INVESTMENT_503_PINNED_RAW, &investment_205_response_body()),
            (
                &[98, 834, 5015, 6018],
                INVESTMENT_503_UNLOCKS_RAW_V1_ACT393_ONLY,
                INVESTMENT_205_UNLOCKS_BODY_V1_ACT393_ONLY,
            ),
            (
                &[98, 308, 419, 440, 834, 3389, 5015, 6018],
                INVESTMENT_503_UNLOCKS_RAW_V2_UNION8,
                INVESTMENT_205_UNLOCKS_BODY_V2_UNION8,
            ),
            (&UNLOCK_FLAG_INDICES, INVESTMENT_503_UNLOCKS_RAW, INVESTMENT_205_UNLOCKS_BODY),
        ];
        for (indices, want503, want205) in cases {
            let (b503, b205) = unlock_flag_bodies(indices).expect("<= 50 flags");
            assert_eq!(b503, want503, "503 for {} flags", indices.len());
            assert_eq!(b205, want205, "205 for {} flags", indices.len());
            // Round trip: what the builder wrote is what the reader reads.
            let flags = unlock_flags_in_shared_tail(&b205[1..]).unwrap();
            assert_eq!(flags.iter().map(|&(i, _)| i).collect::<Vec<_>>(), indices.to_vec());
        }
        // The container is 50 x 4 B; refuse rather than silently truncate.
        assert!(unlock_flag_bodies(&vec![1u16; 51]).is_none());
        assert!(unlock_flag_bodies(&vec![1u16; 50]).is_some());
    }

    /// ⚠ The identity hazard the ten-flag constant introduced by becoming the DEFAULT: it bakes in
    /// account SOID `0xD100000000000001`. Splicing must move exactly the 64 bits at offset 36 and
    /// nothing else - a wrong offset here hands a run someone else's account with no diagnostic.
    #[test]
    fn account_soid_splices_into_the_ten_flag_503_body() {
        assert_eq!(
            investment_503_unlocks_raw_for(0xD100_0000_0000_0001),
            INVESTMENT_503_UNLOCKS_RAW,
            "the anchor SOID must return the pinned constant untouched"
        );
        let other = investment_503_unlocks_raw_for(0x0123_4567_89AB_CDEF);
        assert_eq!(other.len(), INVESTMENT_503_UNLOCKS_RAW.len());
        // Only the SOID field moved: the flags decode identically either way.
        assert_eq!(&other[21..], &INVESTMENT_503_UNLOCKS_RAW[21..]);
        assert_eq!(unlock_flags_in_shared_tail(&other[21..]).unwrap().len(), 10);
        // …and the spliced value reads back at bit 36, MSB-first.
        let bits = to_bits(&other);
        assert_eq!(take_be(&bits, INV_503_ACCOUNT_SOID_BIT_OFFSET, 64), Some(0x0123_4567_89AB_CDEF));
    }

    /// The exact bytes that caused the wall, asserted as *not* shipped. Named rather than described
    /// so a future edit that reverts to a zeroed object fails loudly instead of booting and dying
    /// six minutes later.
    #[test]
    fn rejects_the_zero_rooted_body_that_caused_family_record_not_found() {
        const ZERO_ROOTED: [u8; 25] = [
            0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x02, 0x08, 0x10, 0x22, 0x00, 0x00, 0x00, 0x01, 0x00,
        ];
        assert_ne!(
            investment_205_response_body().as_slice(),
            &ZERO_ROOTED[..],
            "this is the run-20260807-215628 body; it declares family 5 rooted at 0x0"
        );
    }
}

#[cfg(test)]
mod channel_tests {
    use super::*;

    /// The real 22-byte mode-1 payload the client sent, from `runs/20260806-161059-bap-shello98/`.
    const CAPTURED_MODE1_PAYLOAD: [u8; 22] = [
        0xb8, 0xd4, 0x0f, 0x98, 0x99, 0xbc, 0xb6, 0x62, 0x57, 0x08, 0xf8, 0xf8, 0x00, 0x7c, 0x9d,
        0x2b, // tag[16]
        0xde, 0x29, 0x39, 0xbb, 0xd3, 0xa8, // ciphertext[6]
    ];

    /// **The load-bearing test.** Real captured bytes, real keys we issued, must authenticate and
    /// decrypt to a bodyless `0x0079` at sequence 2. If GCM/nonce/tag-order were wrong, the AEAD tag
    /// would fail - so this simultaneously confirms cipher, nonce derivation and wire order.
    #[test]
    fn captured_mode1_packet_authenticates_and_decodes() {
        let mat = SessionMaterial::default();
        let nonces = ChannelNonces::from_seed(mat.channel_seed);
        let n = nonces.peek(Direction::ClientToServer);
        assert_eq!(n[11], 0xD0, "client->server nonce has the direction bit clear");

        let plain = open_channel_payload(&mat.session_key, n, &CAPTURED_MODE1_PAYLOAD)
            .expect("captured packet must authenticate under the key we issued");
        assert_eq!(plain, vec![0x00, 0x79, 0x00, 0x00, 0x00, 0x02],
                   "bodyless register-subscriber request, sequence 2");

        let inner = parse_inner(&plain).unwrap();
        assert_eq!(inner.msg_type, TYPE_REGISTER_SUBSCRIBER_REQUEST);
        assert_eq!(inner.sequence, 2);
    }

    #[test]
    fn wrong_nonce_or_key_fails_authentication() {
        let mat = SessionMaterial::default();
        let good = ChannelNonces::from_seed(mat.channel_seed).peek(Direction::ClientToServer);
        let s2c = ChannelNonces::from_seed(mat.channel_seed).peek(Direction::ServerToClient);
        assert!(open_channel_payload(&mat.session_key, s2c, &CAPTURED_MODE1_PAYLOAD).is_err(),
                "the server->client nonce must not authenticate a client->server frame");
        assert!(open_channel_payload(&[0u8; 16], good, &CAPTURED_MODE1_PAYLOAD).is_err(),
                "a wrong session key must fail");
    }

    /// Pins the exact 8 bytes we must put on the wire for `0x000c`, and the request body we
    /// observed it carrying. Regression guard for the cure described on
    /// [`client_to_bap_subscription_response`]: the client's policy descriptor (§26.64) gives response wire
    /// id `0x000d` with **payload size 0**, so anything longer than the class-2 header is wrong,
    /// and the in-flight-queue match at `0xb2e940` is keyed on `(type, sequence)` - so the
    /// sequence must be **echoed**, never incremented (`historicaldocs/design/bap-protocol.md` §3.1).
    /// Pins the 504 (`login_character`) response body emitted by the client's own encoder, and the
    /// request grammar its schema predicts. Regenerate with
    /// `capture-rig/oodle/rsat-schema-encode-oracle.py --tag 0x80801845 --msgid 504
    /// --obj 0000000000000000` - its output is `01f8` ‖ this body.
    #[test]
    fn investment_504_response_body_matches_the_native_encoder() {
        let body = investment_504_response_body();
        assert_eq!(body, vec![0x18, 0x00, 0x00, 0x00, 0x00]);
        // The wire frame the responder builds is `u16be msgid || body`; the oracle emits the same.
        let mut wire = INVESTMENT_MESSAGE_ID_504.to_be_bytes().to_vec();
        wire.extend_from_slice(&body);
        assert_eq!(wire, vec![0x01, 0xf8, 0x18, 0x00, 0x00, 0x00, 0x00],
                   "native encoder 0x261BE0 over schema 0x80801845, zero C object");
        // 16 bits msgid + 4-bit leaf + 32-bit leaf = 52 bits, padded to 56 = 7 bytes.
        assert_eq!(wire.len() * 8, 56);

        // Positive control tying the response side to the request side of the SAME definition row:
        // request schema 0x808017CE is one 64-bit leaf, and the observed request body is exactly
        // `u16be 504 || u64be character_soid`.
        let observed_req = [0x01u8, 0xf8, 0xd1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01, 0x00];
        assert_eq!(u16::from_be_bytes([observed_req[0], observed_req[1]]),
                   INVESTMENT_MESSAGE_ID_504);
        assert_eq!(u64::from_be_bytes(observed_req[2..10].try_into().unwrap()),
                   0xD100_0000_0000_0101, "the character SOID, per schema 0x808017CE");
    }

    /// The rest of the in-flight queue at `character:signin`, pinned. Every body here came from the
    /// client's own tables; see each function's doc comment for the derivation and controls.
    #[test]
    fn the_in_flight_queue_replies_are_pinned() {
        // 701 and 504 share the response schema shape (one `0x80801796` composite), so the oracle
        // emits the same argument bitstream for both. If these ever diverge, one of the two
        // definition rows changed and both doc comments are stale.
        assert_eq!(investment_701_response_body(), investment_504_response_body(),
                   "0x8080189B and 0x80801845 are the same one-composite shape");
        assert_eq!(investment_701_response_body(), vec![0x18, 0x00, 0x00, 0x00, 0x00]);

        // 0x0016: descriptor 0x15AB3E0 is one `repeated varint` and NO required field, so the
        // correct encoding of an empty purchased-offer list is an empty body.
        let upo = purchased_offers_response(11);
        assert_eq!(upo, vec![0x00, 0x16, 0x00, 0x00, 0x00, 0x0b, 0x00, 0xc8]);
        assert_eq!(upo.len(), REPLY_HEADER_LEN, "0x0016 takes no payload");

        // 0x0021: descriptor 0x15AB800 field #4 is REQUIRED varint, so the body may not be empty.
        // Minimal legal encoding = key (4<<3)|0 then varint 0.
        assert_eq!(user_message_response_body(), vec![0x20, 0x00]);
        assert_eq!(user_message_response_body()[0] >> 3, 4, "protobuf field number 4");
        assert_eq!(user_message_response_body()[0] & 7, 0, "wire type 0 = varint");
        assert_eq!(user_message_response(10),
                   vec![0x00, 0x21, 0x00, 0x00, 0x00, 0x0a, 0x00, 0xc8, 0x20, 0x00]);
    }

    #[test]
    fn client_to_bap_subscription_response_is_header_only_and_echoes_the_sequence() {
        // Observed request, run 20260807-162724-subackhdr-1, msg#8:
        //   inner header `00 0c 00 00 00 07`  then body `04 d1 00 00 00 00 00 00 01`
        //   = family class 4, root SOID 0xD100000000000001, at sequence 7.
        let req = [0x00u8, 0x0c, 0x00, 0x00, 0x00, 0x07,
                   0x04, 0xd1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01];
        let inner = parse_inner(&req).unwrap();
        assert_eq!(inner.msg_type, TYPE_CLIENT_TO_BAP_SUBSCRIPTION_REQUEST);
        assert_eq!(inner.sequence, 7);
        assert_eq!(req[6], 4, "family class");
        assert_eq!(u64::from_be_bytes(req[7..15].try_into().unwrap()), 0xD100_0000_0000_0001);

        let rsp = client_to_bap_subscription_response(inner.sequence);
        assert_eq!(rsp, vec![0x00, 0x0d,               // c->b sub rsp
                             0x00, 0x00, 0x00, 0x07,   // the REQUEST's sequence, echoed
                             0x00, 0xc8],              // status 200
                   "0x000d is header-only: response payload size is 0 in the client's policy table");
        assert_eq!(rsp.len(), REPLY_HEADER_LEN, "no payload may follow the class-2 header");
    }

    #[test]
    fn register_subscriber_response_round_trips() {
        let mat = SessionMaterial::default();
        let mut nonces = ChannelNonces::from_seed(mat.channel_seed);
        let n = nonces.peek(Direction::ServerToClient);
        assert_eq!(n[11], 0xD1, "server->client nonce has the direction bit set");

        let body = reply_header(TYPE_REGISTER_SUBSCRIBER_RESPONSE, 2);
        assert_eq!(body, vec![0x00, 0x7a, 0x00, 0x00, 0x00, 0x02, 0x00, 0xc8]);

        let frame = seal_channel_frame(&mat.session_key, n, &body).unwrap();
        assert_eq!(frame.len(), 6 + GCM_TAG_LEN + body.len(), "envelope + tag + ciphertext");
        assert_eq!(&frame[..2], &[0x01, 0x01], "version 1, mode 1");
        assert_eq!(parse_envelope(&frame).unwrap().payload_size as usize, GCM_TAG_LEN + body.len());

        // Round-trip under the same nonce.
        let back = open_channel_payload(&mat.session_key, n, &frame[6..]).unwrap();
        assert_eq!(back, body);
        nonces.advance(Direction::ServerToClient);
        assert_ne!(nonces.peek(Direction::ServerToClient), n, "counter must move");
    }

    /// The nonce rule, read out of `0x6cf80`/`0x6cde0` rather than guessed.
    #[test]
    fn nonce_derivation_and_increment_match_the_client() {
        // Direction is an XOR of byte[11], not a mask. With a seed whose low bit is 0 the two
        // differ, and the old masking version was backwards.
        let mut seed = [0x11u8; 12];
        seed[11] = 0x40; // low bit CLEAR
        let n = ChannelNonces::from_seed(seed);
        assert_eq!(n.peek(Direction::ServerToClient)[11], 0x40, "server->client uses the seed verbatim");
        assert_eq!(n.peek(Direction::ClientToServer)[11], 0x41, "client->server flips byte[11]");

        // +1 little-endian from byte[0], with carry.
        let mut c = ChannelNonces::from_seed([0u8; 12]);
        c.advance(Direction::ServerToClient);
        assert_eq!(c.peek(Direction::ServerToClient)[0], 1, "increments byte[0], not byte[11]");
        let mut c2 = ChannelNonces::from_seed([0xFFu8; 12]);
        let before = c2.peek(Direction::ServerToClient);
        c2.advance(Direction::ServerToClient);
        let after = c2.peek(Direction::ServerToClient);
        assert_eq!(after[0], 0x00, "byte[0] wraps");
        assert_eq!(after[1], 0x00, "carry propagates upward");
        assert_ne!(after, before);
    }

    #[test]
    fn relay_client_response_is_header_only() {
        let body = reply_header(TYPE_REGISTER_RELAY_CLIENT_RESPONSE, 3);
        assert_eq!(body, vec![0x01, 0x2F, 0x00, 0x00, 0x00, 0x03, 0x00, 0xC8],
                   "class 2 header, status 200, zero payload");
    }

    #[test]
    fn investment_and_echo_replies_have_the_right_shape() {
        // Echo: class-2 header, zero payload.
        assert_eq!(reply_header(TYPE_ECHO_RESPONSE, 5),
                   vec![0x00, 0xFB, 0x00, 0x00, 0x00, 0x05, 0x00, 0xC8]);
        // Investment: class-2 header then the echoed u16be message id (503 = 0x01F7 observed).
        assert_eq!(investment_message_response(4, 503),
                   vec![0x00, 0x0B, 0x00, 0x00, 0x00, 0x04, 0x00, 0xC8, 0x01, 0xF7]);
    }

    #[test]
    fn direction_counters_are_independent() {
        let mut n = ChannelNonces::from_seed([0xD1; 12]);
        let c0 = n.peek(Direction::ClientToServer);
        n.advance(Direction::ServerToClient);
        assert_eq!(n.peek(Direction::ClientToServer), c0, "advancing one side must not move the other");
    }
}
