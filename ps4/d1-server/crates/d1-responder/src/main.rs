// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// Some response shapes follow the shadNet project's WebAPI (GPL-2.0-or-later) - see ATTRIBUTION.md.

//! The answering surface for the sealed lab: **log every request, answer what the client actually
//! calls, and refuse everything else loudly.**
//!
//! ## Reach tag: `synthetic-generic`
//!
//! Nothing learned through this is evidence about a real service. Every finding downstream of an
//! answer given here inherits **`synthetic_ancestor`**. Its job is to let the client walk several
//! gates deep in one run instead of one - i.e. to discover *what it asks for* - never to establish a
//! contract. Anything found this way needs confirming against capture or hardware before it is
//! promoted.
//!
//! ## What it answers, and why those specifically
//!
//! Both endpoints were discovered from the client's own `sceHttpCreateRequestWithURL` calls, not
//! guessed:
//!
//! * **`POST /2.0/oauth/token`** on `auth.api.np.ac.playstation.net` - the PSN OAuth code→token
//!   exchange. Destiny's `retrieveSCEAccessToken` drives it, and `bootflow:account_signin` fails
//!   without it. Observed request body:
//!   ```text
//!   grant_type=authorization_code&client_id=21d1a29e-…&client_secret=…&code=AUTHCODE
//!   &scope=openid,openid:duid,psn:s2s&redirect_uri=orbis://games
//!   ```
//!   **`scope` contains `openid`, so the client expects an `id_token`** - a JWT. Omitting it moved
//!   the guest's result code from 715 to 733 rather than to success, which is what pointed here.
//!
//! * **`GET /v1/users/{npid}/friendList`** on `srv.shadps4.net:31315` - shadNet's HTTP WebAPI, which
//!   is **separate from the ShadNet socket protocol** our `shadnet-server` speaks on 31313. The
//!   emulator's NP toolkit calls it directly with a `Bearer` token.
//!
//! Everything else gets a logged 404 so an unimplemented endpoint is a **named gate**, not a hang.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{Context, Result};
use axum::{
    body::Bytes,
    extract::{Path as AxPath, Request, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{any, delete, get, post, put},
    Json, Router,
};
use base64::Engine;
use clap::Parser;
use serde_json::json;
use tracing::{info, warn};

#[derive(Clone)]
struct Cfg {
    npid: String,
    account_id: u64,
}

#[derive(Parser)]
#[command(name = "d1-responder", about = "Logging answering surface for the sealed lab")]
struct Args {
    /// Plain-HTTP ports to listen on. 31315 is shadNet's WebAPI.
    // 8788 is the loadout editor's control plane - `d1-forge` defaults to it. Listing it here means
    // the GUI's Apply works with no flags on either side; a port that only one half knows is the
    // same class of bug as a path only one half knows, which is why APPLY_PATH is a shared const.
    #[arg(long, default_value = "80,8080,31315,8788")]
    http_ports: String,
    /// TLS ports. 443 is the PSN OAuth exchange; 32556 is Demonware's
    /// `dm-partnernet.upload.deadorbit.net/ticket_drop`, where the game uploads its own crash report.
    #[arg(long, default_value = "443,32556")]
    tls_ports: String,
    /// PEM certificate and key. Whether the client accepts a self-signed cert is part of the experiment.
    #[arg(long)]
    cert: Option<PathBuf>,
    #[arg(long)]
    key: Option<PathBuf>,
    #[arg(long, default_value = "0.0.0.0")]
    bind: String,
    #[arg(long, default_value = "D1Guardian")]
    npid: String,
    #[arg(long, default_value_t = 1)]
    account_id: u64,
    /// UDP port to observe BAP attempts on. MUST NOT be 3074 - the guest binds that itself, so
    /// advertising it would point the client at its own socket and we would capture nothing.
    #[arg(long, default_value_t = 3075)]
    bap_port: u16,
    /// `controls` - run the offline controls and exit. **Exit 2 = a control FAILED; the run's
    /// output may not be cited.** Everything else binds sockets and serves.
    ///
    /// The project rule is that a load-bearing instrument exits 2 on failure, and until 2026-08-10
    /// this crate had no such gate at all: the only offline check on a responder edit was
    /// `cargo test`, which nothing on the boot path runs.
    #[arg(value_name = "MODE")]
    mode: Option<String>,
}

/// A JWT-shaped `id_token`. Unsigned in the sense that the signature is not verifiable - we do not
/// have Sony's key and cannot have it. If the client verifies the signature this will fail, and *that
/// failure is itself the finding*: it tells us signature validation is enforced on this path.
fn id_token(npid: &str, account_id: u64) -> String {
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let header = json!({"typ": "JWT", "alg": "RS256"});
    // Claim names follow PSN's id_token shape. `sub` is the account id as a decimal string.
    let claims = json!({
        "jti": "d1-local-jti",
        "iss": "https://auth.api.np.ac.playstation.net/",
        "aud": ["21d1a29e-6883-4051-bdc9-ed47ab1aa23f"],
        "sub": account_id.to_string(),
        "online_id": npid,
        "country": "US",
        "lang": "en",
        "exp": 4_102_444_800u64, // 2100-01-01; deliberately far future so expiry is never the variable
        "iat": 1_767_225_600u64,
        "at_hash": "d1localathash",
        "duid": format!("{:032x}", account_id),
    });
    format!(
        "{}.{}.{}",
        b64.encode(serde_json::to_vec(&header).unwrap()),
        b64.encode(serde_json::to_vec(&claims).unwrap()),
        b64.encode(b"d1-local-signature-not-verifiable")
    )
}

/// Compact hex for a binary body prefix - enough to recognise a container without a full dump.
fn hexdump(b: &[u8], max: usize) -> String {
    let n = b.len().min(max);
    let hex: Vec<String> = b[..n].iter().map(|x| format!("{x:02x}")).collect();
    let mut s = hex.join(" ");
    if b.len() > n {
        s.push_str(&format!(" … (+{} more)", b.len() - n));
    }
    s
}

fn log_req(method: &str, headers: &HeaderMap, uri: &str, body: &Bytes) {
    let host = headers.get("host").and_then(|v| v.to_str().ok()).unwrap_or("?");
    info!("--> {method} {host}{uri}  {}B", body.len());
    for h in ["authorization", "user-agent", "content-type"] {
        if let Some(v) = headers.get(h).and_then(|v| v.to_str().ok()) {
            info!("    {h}: {}", &v[..v.len().min(160)]);
        }
    }
    if !body.is_empty() {
        let s = String::from_utf8_lossy(body);
        info!("    body: {}", &s[..s.len().min(500)]);
    }
}

async fn oauth_token(
    State(cfg): State<Cfg>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    log_req("POST", &headers, "/2.0/oauth/token", &body);
    let resp = json!({
        "access_token": "d1-local-access-token",
        "token_type": "bearer",
        "expires_in": 3599,
        "scope": "openid openid:duid psn:s2s",
        "refresh_token": "d1-local-refresh-token",
        "refresh_token_expires_in": 5_183_999u64,
        // Present because the request's scope includes `openid`.
        "id_token": id_token(&cfg.npid, cfg.account_id),
    });
    info!("<-- 200 oauth/token with id_token (synthetic-generic)");
    (StatusCode::OK, Json(resp))
}

/// shadNet WebAPI friend list. Empty list is the honest answer: the fixture account has no friends.
async fn friend_list(
    AxPath(npid): AxPath<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    log_req("GET", &headers, &format!("/v1/users/{npid}/friendList"), &Bytes::new());
    info!("<-- 200 friendList (empty)");
    (
        StatusCode::OK,
        Json(json!({"friendList": [], "totalResults": 0, "size": 0, "start": 0})),
    )
}


/// The PSN-shaped session id we issue. `NNN-` + UUID; the client's own zeroed `PUT` gave us the
/// template. Reach tag: `synthetic-minimal` - the *shape* is `[C]` from the client, the value is ours.
const SESSION_ID: &str = "000-d1000000-0000-4000-8000-000000000001";

/// **PSN session creation** - `POST /v1/sessions`, the gate that kept us out of orbit.
///
/// Destiny creates its fireteam's *platform session* through `sceNpWebApi`, not through a
/// `sceNpSession*` syscall - so it arrives here as HTTP. Observed: a **43,531-byte multipart** body
/// (the session data plus its image; the eboot carries `multipart/mixed; boundary=bUnGiEr0x0rS`).
///
/// 404-ing it produced, exactly:
/// ```text
///   managed_session: [#0][7DF825C9:E1D6EFA5-10.77.0.2:127.0.0.1]
///                    Asynchronous session creation failed. Return result: '-1'
///   session: [fireteam:…] idle-host-disband disconnecting the session
///   Prerequisite 'ENUM(11)' failed -> cleanup for reason 'error.platform_session_not_failed'
/// ```
/// and on screen the **plain-English** platform error ("permissions to access online multiplayer
/// gameplay may have changed") - plain English rather than a Bungie animal codename being the tell
/// that this is the platform layer.
///
/// PSN answers with the created session's id. `sessionId` is the field the client needs back;
/// everything else here is shape, not contract. Reach tag: **`synthetic-minimal`** - invented, and
/// therefore `[U]` until a capture or the client's own parser confirms the field set.
async fn create_session(headers: HeaderMap, body: Bytes) -> impl IntoResponse {
    log_req("POST", &headers, "/v1/sessions", &body);
    // ⚠ THE SHAPE MATTERS. The first attempt returned `d1-fireteam-0000000000000001` and the client
    // could not parse it: it immediately did `PUT /v1/sessions/000-00000000-0000-0000-0000-000000000000`
    // - an ALL-ZERO id - proving it had parsed our string into nothing. PSN session ids are
    // `NNN-` + a standard UUID, and the client's own zeroed PUT is what told us the layout.
    let session_id = SESSION_ID;
    warn!("<-- 201 ** PSN SESSION CREATED ** /v1/sessions ({}B body) -> sessionId={session_id}",
          body.len());
    (StatusCode::CREATED, Json(json!({"sessionId": session_id})))
}

/// `DELETE /v1/users/{npid}/presence/gameStatus` - also 404'd in the same run. Presence teardown;
/// the honest answer is "done", and a 404 here is a refusal the client did not ask for.
async fn delete_game_status(
    AxPath(npid): AxPath<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    log_req("DELETE", &headers, &format!("/v1/users/{npid}/presence/gameStatus"), &Bytes::new());
    info!("<-- 204 presence gameStatus cleared");
    StatusCode::NO_CONTENT
}


/// `PUT /v1/sessions/{id}` - the client updates its session immediately after creating it. 404-ing
/// this produced `session operation 'update_session' failed, error= (code=-2137915391)`, which is
/// **our own** `0x80920001` fallback code read back out of the 404 body, then session deletion and
/// `error.lost_bap_connection`.
async fn update_session(
    AxPath(id): AxPath<String>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    log_req("PUT", &headers, &format!("/v1/sessions/{id}"), &body);
    if id != SESSION_ID {
        warn!("<-- 200 session update for UNEXPECTED id {id} (we issued {SESSION_ID}) - \
               accepting anyway, but the id round-trip is broken");
    } else {
        info!("<-- 200 session updated ({}B)", body.len());
    }
    (StatusCode::OK, Json(json!({"sessionId": id})))
}

/// `PUT /v1/users/{npid}/presence/gameData` - presence payload alongside the session. Also 404'd.
async fn put_game_data(
    AxPath(npid): AxPath<String>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    log_req("PUT", &headers, &format!("/v1/users/{npid}/presence/gameData"), &body);
    info!("<-- 204 presence gameData accepted ({}B)", body.len());
    StatusCode::NO_CONTENT
}

/// Anything unimplemented: a **named gate**, in PSN's error shape so the client can parse the refusal.
///
/// It reads the body deliberately: the first version of this did not, and reported Bungie's `/SignOn`
/// POST as `0B` when `content-type: application/octet-stream` means the body is the whole point.
async fn fallback(req: Request) -> impl IntoResponse {
    let uri = req.uri().to_string();
    let method = req.method().to_string();
    let headers = req.headers().clone();
    let body = axum::body::to_bytes(req.into_body(), 4 * 1024 * 1024)
        .await
        .unwrap_or_default();
    log_req(&method, &headers, &uri, &body);
    if !body.is_empty() {
        info!("    hex: {}", hexdump(&body, 96));
    }
    warn!("<-- 404 ** NEW GATE ** {method} {uri}  ({}B body)", body.len());
    (
        StatusCode::NOT_FOUND,
        Json(json!({"error": {"code": 0x8092_0001u32, "message": "not implemented by d1-responder"}})),
    )
}

/// Bungie's signon bootstrap. **The response format is unknown `[U]`.**
///
/// Observed request: `POST https://www{1..8}.signon.{deadorbit,gravityshavings}.net/SignOn
/// ?platform=orbis&build=<build-string>` with `content-type: application/octet-stream`.
///
/// We answer *something* on purpose. The eboot's own parser narrates its expectations -
/// `Parsing signon server response (%d bytes)`, `Unknown signon server response type: %d`,
/// `Signon server did not report a silo id`, `did not report a content build version`,
/// `Unable to parse signon response` - so a wrong answer produces a **specific complaint**, which is
/// worth far more than another 404. The client is a precise oracle that only ever says "no"; this is
/// how we ask it what it wants.
/// Counts SignOn requests within a run, so a single boot can probe several response-type values.
/// The client retries `/SignOn` repeatedly, and each retry is a free experiment.
static SIGNON_SEQ: AtomicU64 = AtomicU64::new(0);

/// Protobuf base-128 varint.
fn varint(out: &mut Vec<u8>, mut n: u64) {
    loop {
        let b = (n & 0x7f) as u8;
        n >>= 7;
        out.push(if n > 0 { b | 0x80 } else { b });
        if n == 0 {
            return;
        }
    }
}

fn pb_varint(out: &mut Vec<u8>, field: u32, v: u64) {
    varint(out, (field as u64) << 3);
    varint(out, v);
}

fn pb_bytes(out: &mut Vec<u8>, field: u32, v: &[u8]) {
    varint(out, ((field as u64) << 3) | 2);
    varint(out, v.len() as u64);
    out.extend_from_slice(v);
}

/// The minimum viable `response_type = 0` body, per `historicaldocs/design/signon-protocol.md` §7.2.
///
/// ## Two things here are counter-intuitive and cost this session real time
///
/// **1. `Unable to parse signon response` does NOT mean the bytes were malformed.** `0x2a9aa5`
/// discards the parser's return value; `0x2a99d0` returns the *dispatch* flag instead, which for
/// `response_type = 0` is set only if **field 2 was present**. So a byte-perfect protobuf that omits
/// field 2 produces the identical four log lines as a corrupt one. It means "not actionable".
///
/// **2. All seven of `f2.1`-`f2.7` are required with no default**, including `f2.1`, which this build
/// never reads. Omit any one and the codec raises `missing required field`.
///
/// Deliberately NOT sent: `client_config.3` (f4.3) - its callback demands a leading varint `0x123456`
/// and aborts the whole parse with `callback failed` otherwise (§7.3 trap 3). We omit field 4
/// entirely, since nothing needs it yet.
///
/// `content_build_version` echoes the client's own `build=` query parameter rather than a value we
/// picked, so a build cross-check - `[U]` whether one exists - cannot be the variable that fails.
///
/// ## ⚠ Reach: `synthetic-generic`, and this one touches auth
///
/// `f2.2`/`f2.3` are `[I]` key material for the BAP `secure_hello` HMAC and `f2.4` is a security
/// token. We send zeros and a placeholder. Anything the client does past this point inherits
/// **`synthetic_ancestor`**, and per `CLAUDE.md` nothing on this path may be promoted to contract
/// without independent confirmation - it is an authentication boundary.
struct SignonCfg {
    silo_id: u64,
    environment: String,
    bap_ip: std::net::Ipv4Addr,
    bap_port: u16,
    client_ip: std::net::Ipv4Addr,
    manifest_url: String,
}

/// `client_config.3` - the **engine-config delta stream**, and the only thing in the entire binary
/// that can set the content-manifest URL. Full analysis: `historicaldocs/design/content-manifest-gate.md`.
///
/// ## This inverts an earlier instruction, deliberately
///
/// `historicaldocs/design/signon-protocol.md` §7.3 trap 3 says *do not emit f4.3* - its callback "demands a leading
/// varint `0x123456`" and otherwise aborts the parse. The magic is real; the framing was wrong.
/// **`0xa2180` is `read_fixed32`, not a varint reader**, so the magic and every key are raw
/// **u32 little-endian**; only strings are LEB128-length-prefixed. Emitting it correctly is now
/// mandatory, because `bootflow:content_check` fails on an **empty URL string** and this is the one
/// runtime writer of it (`0x267b1b8 + 0xa84`, default `""`).
///
/// Framing: `u32 magic 0x00123456`, then `(u32 key, value)` pairs, terminated by `u32 0`.
///
/// ## Traps this function is shaped by
///
/// * **Any unknown key aborts the WHOLE signon parse** with `Client config decoding failed. Check for
///   invalid overrides.` - including the 26 unused holes in the 100..=1254 range. So we send exactly
///   one key and nothing speculative.
/// * `0x1db2e0` **resets the entire Engine Configuration to defaults before applying**, so this blob
///   is a full snapshot, not a patch on prior state.
/// * The URL *prefix* (key 102) defaults to empty with use-prefix (key 894) defaulting to 1, so
///   `prefix + body == body` and **key 100 alone is sufficient.**
/// * The string setters have **no length clamp**; the field is `char[0x100]`, so keep it under 255.
///
/// ## Build 56578 (01.33): the signon external-address fallback
///
/// Measured on 56578 with the retail diagnostics sink: when STUN fails (no STUN server answers in
/// the lab), the client logs `STUN address discovery failed and not configured to use external
/// address from signon.` and `bootflow:package_registration` then fails prerequisite 15 with
/// `error.transport_method_invalid` (marionberry). The decision (`0x2E0560`) reads two engine-config
/// bools: `+0x598` "on STUN failure, use the signon f14 address" and `+0x599` "prefer the signon
/// address over STUN". Their setter cases are SPARSE keys `0xF1048` (`+0x598`) and `0x4409EC`
/// (`+0x599`) in the 56578 setter switch `0x1033F80`; neither exists in 01.29, where an unknown key
/// would abort the whole signon parse. So the fallback key is sent ONLY when the request's build is
/// 56578, and only the conservative one: STUN is still attempted, then our f14 is used.
fn client_config_delta(manifest_url: &str, build: Option<&str>) -> Vec<u8> {
    const MAGIC: u32 = 0x0012_3456;
    const KEY_CONTENT_MANIFEST_URL: u32 = 100;
    const KEY_56578_SIGNON_EXTERNAL_FALLBACK: u32 = 0x000F_1048;

    let mut v = Vec::new();
    v.extend_from_slice(&MAGIC.to_le_bytes());
    v.extend_from_slice(&KEY_CONTENT_MANIFEST_URL.to_le_bytes());
    // String value: LEB128 length then raw bytes. Truncated to fit char[0x100] with room for NUL.
    let url = &manifest_url.as_bytes()[..manifest_url.len().min(0xFE)];
    varint(&mut v, url.len() as u64);
    v.extend_from_slice(url);
    if build.is_some_and(|b| b.starts_with("56578.")) {
        v.extend_from_slice(&KEY_56578_SIGNON_EXTERNAL_FALLBACK.to_le_bytes());
        v.push(1); // bool: one raw byte
    }
    v.extend_from_slice(&0u32.to_le_bytes()); // terminator
    v
}

fn signon_success(build: Option<&str>, c: &SignonCfg) -> Vec<u8> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // ── field 2: the Success payload. All seven sub-fields mandatory. ──
    let mut p = Vec::new();
    pb_bytes(&mut p, 1, &[0u8; 16]); // f2.1 unused_blob - required, never read by this build
    pb_bytes(&mut p, 2, &[0u8; 16]); // f2.2 session_key_a  [I] BAP secure_hello key material
    pb_bytes(&mut p, 3, &[0u8; 16]); // f2.3 session_key_b  [I] ditto
    pb_bytes(&mut p, 4, b"d1-local-security-token"); // f2.4, max 512
    pb_varint(&mut p, 5, 4_102_444_800); // f2.5 token_expiry_unix - 2100, so expiry is never the variable
    // f2.6/f2.7: where the client should look for BAP next.
    //
    // ⚠ **f2.6 is LITTLE-endian-packed, and f14 is BIG-endian - they differ.** Measured, not assumed:
    // with `u32::from(ip)` (big-endian) the client reported
    //   `transport_endpoint_async_connect() [ip address 2.0.77.10:3075]`
    // i.e. 10.77.0.2 byte-reversed, and failed with `error.no_bap_connection`. Meanwhile the STUN
    // log rendered f14 correctly as `10.77.0.2` in the same run. So the two address fields do NOT
    // share a byte order, and `historicaldocs/design/signon-protocol.md` §7.2 calling both "BE-packed" is wrong for
    // f2.6. Anyone changing this: the client prints the address it dialled - read that, do not guess.
    pb_varint(&mut p, 6, u32::from_le_bytes(c.bap_ip.octets()) as u64);
    pb_varint(&mut p, 7, c.bap_port as u64);
    // NOT sending f2.11 - it silently overrides f2.5 with a relative expiry (§7.3 trap 6).

    let mut v = Vec::new();
    pb_varint(&mut v, 1, 0); // f1 response_type = SUCCESS
    pb_bytes(&mut v, 2, &p); // f2 Success
    // f4 client_config. Sub-field 3 is the engine-config delta that carries the manifest URL - the
    // ONLY writer of it in the binary, so `content_check` cannot pass without this.
    if !c.manifest_url.is_empty() {
        let mut cc = Vec::new();
        pb_varint(&mut cc, 1, 0); // f4.1 cookie
        pb_varint(&mut cc, 2, 0); // f4.2 social_matchmaking_id
        pb_bytes(&mut cc, 3, &client_config_delta(&c.manifest_url, build)); // f4.3 config delta
        pb_bytes(&mut v, 4, &cc);
    }
    pb_varint(&mut v, 5, c.silo_id);
    // Strings need `len + 1 <= size`, so a char[64] field takes at most 63 chars (§7.3 trap 4).
    let build = build.unwrap_or("52553.16.09.28.1551.evil");
    pb_bytes(&mut v, 6, &build.as_bytes()[..build.len().min(63)]);
    // The client computes clock_offset = now - this; real time keeps the offset ~0.
    pb_varint(&mut v, 12, now);
    pb_bytes(&mut v, 13, c.environment.as_bytes());
    // f14 client_external_ip: the client's own address as seen by the server. Providing it is what
    // lets STUN be bypassed - worth a great deal here, because our STUN answers are fiction anyway.
    pb_varint(&mut v, 14, u32::from(c.client_ip) as u64);
    v
}

async fn signon(
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let seq = SIGNON_SEQ.fetch_add(1, Ordering::SeqCst);
    info!("=== SIGNON REQUEST - the bootstrap ===");
    // The account/profile instance checksums differ between 01.29 and 56578; every later BAP
    // object for this client is stamped from what it reports here.
    if let Some(b) = q.get("build") {
        d1_bap::set_client_build(b);
        info!("    client build {b} -> account {:#010x}, profile {:#010x}",
              d1_bap::account_instance_type(), d1_bap::profile_instance_type());
    }
    log_req("POST", &headers, "/SignOn", &body);
    if !body.is_empty() {
        info!("    hex: {}", hexdump(&body, 256));
    }
    // The response body is settable at runtime via `D1_SIGNON_HEX`, so probing the schema is a
    // re-run rather than a rebuild. Each probe costs ~2 minutes of emulator boot; a rebuild would
    // double that for no reason.
    let body_out: Vec<u8> = match std::env::var("D1_SIGNON_HEX") {
        Ok(h) if !h.trim().is_empty() => {
            let cleaned: String = h.chars().filter(|c| c.is_ascii_hexdigit()).collect();
            let bytes: Vec<u8> = cleaned
                .as_bytes()
                .chunks(2)
                .filter(|c| c.len() == 2)
                .filter_map(|c| u8::from_str_radix(std::str::from_utf8(c).ok()?, 16).ok())
                .collect();
            info!("    using D1_SIGNON_HEX: {} bytes", bytes.len());
            bytes
        }
        // Sweep mode: `D1_SIGNON_TYPESWEEP=1` returns `{1: varint seq}` on each successive request,
        // so one boot tests response types 0,1,2,3… The client's own
        // `Unknown signon server response type: %d` line tells us which values it rejects, and the
        // absence of that line tells us which it accepts. Turning retries into a search.
        _ if std::env::var("D1_SIGNON_TYPESWEEP").is_ok() => {
            let mut v = Vec::new();
            v.push(0x08u8); // field 1, varint
            let mut n = seq;
            loop {
                let b = (n & 0x7f) as u8;
                n >>= 7;
                v.push(if n > 0 { b | 0x80 } else { b });
                if n == 0 {
                    break;
                }
            }
            info!("    TYPESWEEP: request #{seq} -> response_type={seq}");
            v
        }
        // Default: the minimal candidate success response from the recovered descriptor table.
        // `D1_SIGNON_ENV` / `D1_SIGNON_SILO` stay tunable so a value can be varied without a rebuild.
        _ => {
            let ev = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
            let cfg = SignonCfg {
                silo_id: ev("D1_SIGNON_SILO", "12345").parse().unwrap_or(12345),
                environment: ev("D1_SIGNON_ENV", "live"),
                // Default to the sealed namespace's own address: the next gate is the client trying
                // to reach BAP, and we want that attempt to land somewhere we can observe.
                bap_ip: ev("D1_BAP_IP", "10.77.0.2").parse().unwrap_or([10, 77, 0, 2].into()),
                bap_port: ev("D1_BAP_PORT", "3075").parse().unwrap_or(3075),
                client_ip: ev("D1_CLIENT_IP", "10.77.0.2").parse().unwrap_or([10, 77, 0, 2].into()),
                // http:// deliberately, not https:// - the CDN client's TLS stack is unidentified
                // `[U]`, so plain HTTP removes one variable. `cdn.deadorbit.net` is already
                // hosts-redirected to us by run-signon-experiment.sh.
                manifest_url: ev("D1_MANIFEST_URL", "http://cdn.deadorbit.net/manifest.bin"),
            };
            let out = signon_success(q.get("build").map(|s| s.as_str()), &cfg);
            info!(
                "    success response: type=0 silo={} env={} bap={}:{} client_ip={} manifest_url={} build={:?}",
                cfg.silo_id, cfg.environment, cfg.bap_ip, cfg.bap_port, cfg.client_ip,
                cfg.manifest_url, q.get("build")
            );
            out
        }
    };
    info!("<-- 200 SignOn, {}B placeholder (synthetic-generic) - watching for the parser's complaint",
          body_out.len());
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
        body_out,
    )
}

/// A UDP **observation** surface for the BAP address we advertise in the signon response.
///
/// ## Why this is needed, and why it is not a protocol implementation
///
/// We tell the client where BAP lives via signon `f2.6`/`f2.7`. Whatever it sends there is the first
/// direct evidence we have ever had about the Demonware/BAP handshake *from our own client*, so it
/// must not vanish. This logs datagrams and **never replies** - it does not parse, frame, or encode
/// anything. Per `CLAUDE.md` (*one protocol, one implementer*) all Demonware wire code belongs in a
/// single crate written by a single session; a byte logger is an instrument, not a second
/// implementation, and deliberately stays that way.
///
/// ## ⚠ The trap this exists to close
///
/// The guest binds **UDP 3074 itself** (observed: `0.0.0.0:3074`, `shadPS4:Main`). Advertising BAP at
/// `…:3074` therefore points the client at a port it already owns - it would talk to itself, we would
/// capture nothing, and the silence would read as *"the client never tried BAP"*. That is a false
/// negative of exactly the kind this project keeps getting bitten by. So the advertised BAP port
/// defaults to **3075**, which the guest does not bind (it holds 2001, 2002, 3074 plus ephemerals).
///
/// Expectation to test, not assume `[I]`: per `historicaldocs/design/networking-architecture.md` the transport is
/// SCTP-shaped over UDP with DTLS, so the first datagram should look like either a DTLS ClientHello
/// (`16 fe fd`) or a Demonware `bdInitChunk`. The bytes will say which.
async fn bap_observer(addr: SocketAddr) {
    let sock = match tokio::net::UdpSocket::bind(addr).await {
        Ok(s) => s,
        Err(e) => {
            warn!("bind udp {addr} failed: {e} - BAP attempts will NOT be observed");
            return;
        }
    };
    info!("listening udp://{addr} (BAP observation surface - logs, never replies)");
    let mut buf = vec![0u8; 64 * 1024];
    let mut n_seen = 0u64;
    loop {
        match sock.recv_from(&mut buf).await {
            Ok((n, from)) => {
                n_seen += 1;
                let d = &buf[..n];
                // Name the container on sight, so the log is readable without a second pass.
                let shape = match d {
                    [0x16, 0xfe, 0xfd, ..] => " [DTLS 1.2 handshake]",
                    [0x16, 0xfe, 0xff, ..] => " [DTLS 1.0 handshake]",
                    [0x17, 0xfe, ..] => " [DTLS application data]",
                    _ => "",
                };
                warn!("=== BAP DATAGRAM #{n_seen} from {from}, {n}B{shape} ===");
                info!("    hex: {}", hexdump(d, 128));
            }
            Err(e) => {
                warn!("udp recv error: {e}");
                return;
            }
        }
    }
}


// ══════════════════════════════════════════════════════════════════════════════════════════════
//  THE CORRELATION TABLE, WIRED  -  d1_bap::BapConversation in d1-responder
// ══════════════════════════════════════════════════════════════════════════════════════════════
//
// ⭐⭐ **No handler in this file may write a response header any more.** Handlers produce bytes;
// `BapConversation` decides *whether*, *when* and *in what order* they leave, and `drain` is the
// only thing that lays down a header. That is the other half of the Weasel fix.
//
// The client's receive path (`0xB2E940`) matches an inbound reply against the **head** of each of
// its two in-flight rings and nothing else. A reply the head is not waiting for raises `0x12`
// `_connection_failure_failed_to_find_message_recipient` → `error.lost_bap_connection` → the
// bootflow restarts. On screen: **Weasel**. Both rung-28 runs died exactly there, on message ids
// 11 and 13 - *both of which this responder already answers correctly*. The bug was never the
// bytes; it was the order.
//
// ⇒ **A wrong-but-routable reply is survivable. A right-but-unroutable reply is not.**

/// Counts BAP TCP connections in this process.
///
/// ⭐ It matters now. Phase 3 points the client's **activity host** at an address we choose, and
/// `D1_AH_IP`/`D1_AH_PORT` default to this same listener - so a *second* BAP connection is an
/// expected event rather than an anomaly. Only connection **0** owns the QueueZ character/family
/// delivery; without that guard the second connection would re-push objects the client already has.
static BAP_CONNECTIONS: AtomicU64 = AtomicU64::new(0);

/// ⭐ The PRIMARY BAP connection: the newest one that sent `queuez_register` (0x79), i.e. the
/// client's own investment connection. Only it receives the QueueZ family pushes.
///
/// ⛔ It used to be "connection 0". When the client's first connection is aborted during the secure
/// hello (Windows `os error 10053`, intermittent on the lab host) or the client re-signs-in after a
/// kick, the working connection is #1 or later, the family-4 baseline is never pushed, investment
/// sign-in times out, and the player sees CHICKEN (`error.no_investment_connection`). Measured on
/// 56578, 14 Oct: every boot with an early abort lacked the push, every boot without one had it.
static PRIMARY_BAP_CONNECTION: AtomicU64 = AtomicU64::new(0);

/// BAP id **6** - `client_to_activity_host_manager_request`, header class 1.
///
/// ⚠ These six ids are written **decimal-first** because the decimal/hex collision is a standing
/// trap in this protocol: `16` is `get_activity_host_proxy_request` in decimal and
/// `TYPE_UPO_RESPONSE` in hex. They live here rather than in `d1-bap` only because `d1-bap` does
/// not export `TYPE_*` constants for the activity-host family yet - see the report's change list.
const TYPE_ACTIVITY_HOST_MANAGER_REQUEST: u16 = 6; // 0x0006, class 1
/// BAP id **7** - `client_to_activity_host_manager_response`, header class 2 (8-byte header).
const TYPE_ACTIVITY_HOST_MANAGER_RESPONSE: u16 = 7; // 0x0007, class 2
/// BAP id **8** - `client_to_activity_host_notification`, header class **3 = 14 bytes**.
const TYPE_CLIENT_TO_ACTIVITY_HOST_NOTIFICATION: u16 = 8; // 0x0008, class 3
/// BAP id **9** - `bap_to_client_notification`, header class **0 = 6 bytes**.
///
/// ⭐⭐ **This is the message the client expects an Activity-Host payload to arrive in, and it is
/// also the message the client EMITS at us when we get the addressing wrong.** Both facts come out
/// of the same three functions, read on the pinned 01.29 eboot:
///
/// * `c_investment_bap_message_push_policy_activity_host_to_client_notification::vfunc[5]`
///   (`0x564C20`) - message **100**'s deliver step - does exactly one thing: it reads the
///   **class-6 header's `+0x06` u64** (getter `0x55D1D0`, header-codec slot 5), treats it as a
///   *connection index*, and if `FUN_00558AA0(idx) == 3` (connected) re-emits the identical body
///   as **policy index 9 = message 9** on that connection. It returns `1` either way, so a
///   mis-addressed 100 is **silent in both directions**.
/// * `c_investment_bap_message_push_policy_bap_to_client_notification::vfunc[5]` (`0x5642E0`) -
///   message **9**'s deliver step - calls `FUN_0075CB40` → `FUN_0075CB70(activity_manager, body)`,
///   which resolves the manager slot by the body's `recipient_id` (`FUN_0075AB60`) and hands the
///   payload to `c_activity_client::vfunc[9]` `receive_message_from_host` via `FUN_00594960`.
/// * Both are reached from `handle_message_internal` (`0x563170`), which decodes the push and then
///   calls `policy->vfunc[5]`.
///
/// ⇒ In Bungie's design the client and the Activity Host **never talk directly**: the relay is
/// `client -8-> BAP -160-> AH` and `AH -100-> BAP -9-> client`. Full write-up:
/// `historicaldocs/design/establish-client-gate.md`.
const TYPE_BAP_TO_CLIENT_NOTIFICATION: u16 = 9; // 0x0009, class 0
/// BAP id **16** - `client_to_bap_get_activity_host_proxy_request`, header class 1.
const TYPE_GET_ACTIVITY_HOST_PROXY_REQUEST: u16 = 16; // 0x0010, class 1
/// BAP id **17** - `client_to_bap_get_activity_host_proxy_response`, header class 2.
const TYPE_GET_ACTIVITY_HOST_PROXY_RESPONSE: u16 = 17; // 0x0011, class 2
/// BAP id **42** - `client_to_bap_update_matchmaking_core_request`, header class 1.
///
/// ⛔ **Not 34/35.** The `Sending MM configuration request` line comes from `0x599620`, which passes
/// **policy index 0x1E**; that policy is `..._matchmaking_core_request`, whose vtable reads the id
/// out of `0x55D940 = mov eax, 0x2A` = 42. 34/35 is the *skill* policy.
const TYPE_MATCHMAKING_CORE_REQUEST: u16 = 42; // 0x002A, class 1
/// BAP id **43** - `client_to_bap_update_matchmaking_core_response`, header class 2.
const TYPE_MATCHMAKING_CORE_RESPONSE: u16 = 43; // 0x002B, class 2
/// BAP id **100** - `activity_host_to_client_notification`, header class **6 = 32 bytes**, `[U]`.
const TYPE_ACTIVITY_HOST_TO_CLIENT_NOTIFICATION: u16 = 100; // 0x0064, class 6
/// BAP id **102** - `activity_host_to_client_log_notification`, header class **5 = 24 bytes**, `[U]`.
/// ⛔ Absent from the client's 13-entry push accept-set. Not sendable on the primary connection.
const TYPE_ACTIVITY_HOST_TO_CLIENT_LOG_NOTIFICATION: u16 = 102; // 0x0066, class 5

/// The activity-host id we hand out in message 7 when `D1_AH_HOST_TOKEN` is unset.
///
/// **Must be non-zero**: `FUN_00594320` returns false and issues nothing while the id is 0, so a
/// zero token is indistinguishable from "we never answered".
const DEFAULT_AH_HOST_TOKEN: u64 = 0xD100_0000_0000_0A01;

/// A boolean knob. Unset takes `default`; anything but `0` is on.
fn env_flag(var: &str, default: bool) -> bool {
    match std::env::var(var) {
        Ok(v) => v.trim() != "0",
        Err(_) => default,
    }
}

/// A numeric knob, decimal or `0x`-prefixed.
fn env_num<T: std::str::FromStr>(var: &str, default: T) -> T
where
    T: Copy,
{
    std::env::var(var)
        .ok()
        .and_then(|v| v.trim().parse::<T>().ok())
        .unwrap_or(default)
}

/// Where a request's **body** starts, given its message id.
///
/// ⚠ **Not always 6.** The inner-header length is per *header class*: classes 0/1/4 are 6 bytes,
/// classes 2/7 are 8, class 3 is **14**, class 5 is 24 and class 6 is 32. Every request this
/// responder answered before today was class 1, so the hard-coded `&plain[6..]` happened to be
/// right. Message **8** (`client_to_activity_host_notification`) is class 3 and is the first one
/// where it is not - a silent 8-byte offset error that would look like a codec bug.
fn bap_body_offset(msg_type: u16) -> usize {
    d1_bap::header_len(msg_type).unwrap_or(6)
}

/// Split a `d1-bap` reply frame into `(status, body)`, **verifying** the header it wrote against
/// the ticket the conversation issued.
///
/// Every reply builder in `d1-bap` today emits the same 8-byte class-2 header
/// (`u16be type || u32be sequence || u16be status`). Under the correlation table the header is
/// written by [`d1_bap::BapConversation::drain`] instead, so this strips it - and because it is
/// stripping bytes it did not write, it checks them first. A type or sequence mismatch here is a
/// builder that has drifted from the client's policy table, and on the wire that drift has exactly
/// one symptom: the Weasel, with nothing else to go on.
///
/// ⚠ Transitional. `historicaldocs/design/SERVER-ROLES-PLAN.md` §2.5 has `d1-bap`'s builders return **bodies**;
/// when they do, this function and every `reply_header` call in this file disappear together.
fn split_reply_frame(ticket: &d1_bap::ResponseTicket, frame: &[u8]) -> Option<(u16, Vec<u8>)> {
    if frame.len() < d1_bap::REPLY_HEADER_LEN {
        return None;
    }
    let ty = u16::from_be_bytes([frame[0], frame[1]]);
    let seq = u32::from_be_bytes([frame[2], frame[3], frame[4], frame[5]]);
    let status = u16::from_be_bytes([frame[6], frame[7]]);
    if ty != ticket.response_type() || seq != ticket.sequence() {
        return None;
    }
    Some((status, frame[d1_bap::REPLY_HEADER_LEN..].to_vec()))
}

/// Write everything the correlation table says may legally leave, in the one legal order.
///
/// Returns the sequences that actually went out, so a caller can gate a follow-up **push** on its
/// own ACK having left first (the family-subscribe answer depends on that ordering).
///
/// This is the single secured emission point. `drain` pops the **front only** and synthesises a
/// non-200 reply for a head that has aged past the deadline - because silence is not neutral: it
/// head-of-line blocks everything behind it and then kills the connection with
/// `_connection_failure_timed_out` (`0x0B`), a *different* fatal from the Weasel.
async fn drain_replies(
    sock: &mut tokio::net::TcpStream,
    key: &[u8; 16],
    nonces: &mut d1_bap::ChannelNonces,
    conv: &mut d1_bap::BapConversation,
) -> Vec<u32> {
    use tokio::io::AsyncWriteExt;
    let before = conv.synthesised_count();
    let frames = conv.drain(std::time::Instant::now());
    let synthesised = conv.synthesised_count() - before;
    if synthesised > 0 {
        warn!(
            "      [conv] ⚠ R2 SYNTHESISED {synthesised} non-200 repl{} for aged head(s) - a wrong \
             body is survivable, silence is not. Some handler produced nothing.",
            if synthesised == 1 { "y" } else { "ies" }
        );
    }
    let mut sent = Vec::new();
    for frame in frames {
        let ty = u16::from_be_bytes([frame[0], frame[1]]);
        let seq = u32::from_be_bytes([frame[2], frame[3], frame[4], frame[5]]);
        let hlen = d1_bap::header_len(ty).unwrap_or(frame.len());
        let sn = nonces.peek(d1_bap::Direction::ServerToClient);
        match d1_bap::seal_channel_frame(key, sn, &frame) {
            Ok(f) => {
                nonces.advance(d1_bap::Direction::ServerToClient);
                match sock.write_all(&f).await {
                    Ok(()) => {
                        info!(
                            "      [conv] SEND type {ty:#06x} ({}) seq {seq} status {} \
                             {}B body -> sealed {}B",
                            d1_bap::message_name(ty).unwrap_or("?"),
                            if frame.len() >= 8 {
                                u16::from_be_bytes([frame[6], frame[7]]).to_string()
                            } else {
                                "-".to_string()
                            },
                            frame.len().saturating_sub(hlen),
                            f.len()
                        );
                        sent.push(seq);
                    }
                    Err(e) => {
                        warn!("      [conv] reply write failed: {e}");
                        return sent;
                    }
                }
            }
            // Not `continue`: a seal failure without advancing the nonce leaves the channel
            // consistent, but the reply is now missing and its entry has already been popped.
            Err(e) => warn!("      [conv] ⛔ seal failed for seq {seq}: {e} - that reply is LOST"),
        }
    }
    if !conv.is_idle() {
        let now = std::time::Instant::now();
        let rows: Vec<String> = conv
            .outstanding(now)
            .map(|(seq, ty, age)| {
                format!(
                    "seq {seq} type {ty:#06x} ({}) {}ms",
                    d1_bap::message_name(ty).unwrap_or("?"),
                    age.as_millis()
                )
            })
            .collect();
        info!(
            "      [conv] {} outstanding - the client is head-of-line blocked on the FIRST: {}",
            rows.len(),
            rows.join(" | ")
        );
    }
    sent
}

/// The endpoint string message 7 hands the client, and the address message 17 resolves to.
///
/// ⚠ `[U]` what `host_text` should actually contain. `0x76C1D0` copies it as a string and nothing
/// else in the binary pins it, so we emit the endpoint our activity host is *reachable at* and let
/// the falsifier decide: if message **16** follows, the chain holds; if it does not, the `[I]` link
/// in `historicaldocs/design/SERVER-ROLES-PLAN.md` §4.2 is wrong and the AH id reaches the session parameter some
/// other way. Do not paper over that difference.
fn ah_endpoint() -> (std::net::Ipv4Addr, u16) {
    let ip = std::env::var("D1_AH_IP")
        .or_else(|_| std::env::var("D1_BAP_IP"))
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| std::net::Ipv4Addr::new(10, 77, 0, 2));
    // Defaults to the PRIMARY BAP port on purpose: the client's activity-host connect
    // (`FUN_00559170`) opens an ordinary BAP channel, so pointing it here gives it a real
    // channel-startup/secure-hello conversation instead of a socket that accepts and never speaks.
    // A port that accepts and stays silent is the worst of both worlds - it looks alive and stalls.
    let port = env_num::<u16>("D1_AH_PORT", env_num::<u16>("D1_BAP_PORT", 3075));
    (ip, port)
}

fn ah_host_text() -> String {
    std::env::var("D1_AH_HOST_TEXT").unwrap_or_else(|_| {
        let (ip, port) = ah_endpoint();
        format!("{ip}:{port}")
    })
}

/// The CDN content manifest, served at whatever path `D1_MANIFEST_URL` points at.
///
/// ## Why two bytes is the right first answer
///
/// Per `historicaldocs/design/content-manifest-gate.md` §5.3 the acceptance checks are, in order: codec success →
/// dedup of `entries`/`offers` (empty passes trivially) → `packages` may be absent (it is *derived*
/// by scanning for a non-empty path) → **`guid` (field 5) must be PRESENT** and `strncmp`-equal over
/// `0x25` bytes to Engine Configuration key 101.
///
/// Key 101 defaults to `""` and we do not set it, so an **empty** `guid` on both sides matches. That
/// makes `2a 00` - field 5, length 0 - the minimal accepted manifest, and it keeps this run to one
/// variable instead of also testing whether key 101 is really the GUID slot.
///
/// ## ⚠ Requires the client modification
///
/// With the stock byte at vaddr `0x18c4974` (= 1) this body is rejected as
/// `insufficient download size`: the envelope path demands > `0x107` bytes plus a valid RSA-2048
/// signature we cannot forge. `patches/CUSA00219-lab-manifest.xml` clears that byte. So a run using
/// this endpoint is a **modified client**, and everything downstream is `synthetic_ancestor`.
async fn manifest(headers: HeaderMap) -> impl IntoResponse {
    log_req("GET", &headers, "/manifest.bin", &Bytes::new());
    // A file is the practical path: a real manifest is ~52 KB (1033 packages, one per FILE chunk),
    // which is unreasonable as hex in an env var. `historicaldocs/design/package-registration.md` explains why the
    // list is per-file rather than per-package.
    if let Ok(path) = std::env::var("D1_MANIFEST_FILE") {
        if !path.trim().is_empty() {
            match std::fs::read(&path) {
                Ok(b) => {
                    warn!("<-- 200 MANIFEST from {path}, {}B (unsigned - requires the lab patch)", b.len());
                    return (
                        StatusCode::OK,
                        [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
                        b,
                    );
                }
                // Loudly, not silently: quietly falling back to the 2-byte manifest would look like
                // the manifest "not working" when the real fault is a bad path.
                Err(e) => warn!("D1_MANIFEST_FILE {path} unreadable: {e} - falling back"),
            }
        }
    }
    let body: Vec<u8> = match std::env::var("D1_MANIFEST_HEX") {
        Ok(h) if !h.trim().is_empty() => {
            let cleaned: String = h.chars().filter(|c| c.is_ascii_hexdigit()).collect();
            cleaned
                .as_bytes()
                .chunks(2)
                .filter(|c| c.len() == 2)
                .filter_map(|c| u8::from_str_radix(std::str::from_utf8(c).ok()?, 16).ok())
                .collect()
        }
        // field 5 (`guid`), length 0 - present but empty, matching the config default.
        _ => vec![0x2a, 0x00],
    };
    warn!("<-- 200 MANIFEST, {}B (unsigned - requires the lab patch) {}",
          body.len(), hexdump(&body, 32));
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
        body,
    )
}


/// `dm-partnernet.upload.deadorbit.net:32556/ticket_drop` - Demonware's upload endpoint, and
/// **the single richest diagnostic channel in this project.**
///
/// When Destiny halts it runs its own crash reporter, writes a `crash_archive.zip` plus a
/// `crash_info.txt` containing `halt information:` in plain prose, and POSTs the bundle here. We got
/// the first one by hand out of shadPS4's temp dir; answering this endpoint makes it automatic and
/// survives temp-dir cleanup.
///
/// The `Content-Type` is `application/x-eviltiger-upload` and the metadata sidecar is generated by
/// `TigerFoundation` - Tiger is Bungie's engine, `evil` the branch. Answering 200 also stops the
/// upload failing with `bdHTTPWrapper-ps4.cpp:665 Failed to get HTTP status code`, which previously
/// masked the real halt reason behind a network error.
async fn ticket_drop(headers: HeaderMap, body: Bytes) -> impl IntoResponse {
    log_req("PUT/POST", &headers, "/ticket_drop", &Bytes::new());
    warn!("=== GUEST CRASH UPLOAD: {} bytes ===", body.len());
    let dir = std::env::var("D1_UPLOAD_DIR").unwrap_or_else(|_| "/tmp/d1-uploads".to_string());
    let _ = std::fs::create_dir_all(&dir);
    // Sequence rather than a timestamp so repeated runs cannot collide and ordering is obvious.
    let n = SIGNON_SEQ.load(Ordering::SeqCst);
    let path = format!("{dir}/ticket_drop-{n}-{}.bin", body.len());
    match std::fs::write(&path, &body) {
        Ok(()) => warn!("    saved -> {path}"),
        Err(e) => warn!("    could not save upload: {e}"),
    }
    // Any 2xx: the client only checks that it got a status code.
    (StatusCode::OK, "ok")
}

fn router(cfg: Cfg) -> Router {
    Router::new()
        .route("/SignOn", post(signon))
        .route("/manifest.bin", get(manifest))
        // The game uploads with **PUT**, not POST -- observed `ONLINE PUT https://dm-partnernet…`
        // returning 405 when only POST was routed. `any` also catches whatever else it tries.
        .route("/ticket_drop", any(ticket_drop))
        .route("/2.0/oauth/token", post(oauth_token))
        .route("/v1/users/{npid}/friendList", get(friend_list))
        // PSN session creation - the platform-session gate (see `create_session`).
        .route("/v1/sessions", post(create_session))
        .route("/v1/sessions/{id}", put(update_session))
        .route("/v1/users/{npid}/presence/gameData", put(put_game_data))
        .route("/v1/users/{npid}/presence/gameStatus", delete(delete_game_status))
        // ⭐ The loadout editor's control plane. NOT a game protocol message - a call between our
        // own tools. `d1_loadout::APPLY_PATH` is the single constant both sides use so the path
        // cannot drift in two string literals.
        .route(d1_loadout::APPLY_PATH, post(loadout_apply))
        .fallback(any(fallback))
        .with_state(cfg)
}


/// Build the QueueZ family body for one family.
///
/// ⭐ **Extracted so the SUBSCRIPTION path and the push path emit identical bytes.**
///
/// Why that matters (measured 2026-08-08, run 20260808-024031): pushing family 4 unsolicited at the
/// `echo` trigger gets the FIRST baseline accepted and then every later update ignored -
/// ```text
/// networking:investment:push:queuez: failed to process update for [family_type ENUM(4) ... object_count 1]
/// queuez_family_update_begin: ignoring family-update due to unhandled replication state,
///     family_type=ENUM(4), family_root_soid=0xD100000000000001, replication state=1
/// ```
/// So objects land in the registry slot but nothing downstream re-evaluates, and *every* content
/// experiment reads as "no change" whatever you put in it. `historicaldocs/design/server-object-model.md` predicted
/// exactly this and said **do not harden the unsolicited-push design**.
///
/// Answering the subscription instead is the shape that already works: it took families 0 and 2
/// from `state=4 LOST/TIMED-OUT` to `state=3 REPLICATING` in one run.
/// Set when the client sends investment **501** (`login_create_character`) and we answer it.
///
/// ⭐ After creation the client waits for the ROSTER to contain the Guardian it just made. While
/// `D1_EMPTY_ROSTER=1` keeps serving 0 characters, "Finish" appears to do nothing: the 501 exchange
/// succeeds (no error, connection survives) and then nothing arrives to confirm the character
/// exists. So once 501 is answered, the empty roster must stop being empty.
static CHARACTER_CREATED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The SOID we handed back in a **501** `login_create_character` response, so the character the
/// client just made can be served to it. `0` = none yet.
static CREATED_CHARACTER_SOID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// ⭐⭐ **Does this account have any characters?** - the one predicate that decides whether the
/// client walks character CREATION or logs straight in.
///
/// # Why `chr == 0` had to join `D1_EMPTY_ROSTER` here `[C]` 2026-08-08
///
/// The account object (`0x808018C7`) is where the client reads the roster - **not** the family-3
/// roster object. `cui::c_investment_login_info_component_v2::vfunc[14]` (`0xA110F0`) does:
///
/// ```text
///   lVar9 = FUN_0056CA70(reg, 4, key, &platform_user);   // family 4 = the ACCOUNT
///   if (0 < *(int *)(lVar9 + 8)) { do {                  // <-- account + 0x08 = THE COUNT
///       local_6e8 = *(long *)(lVar9 + 0x10 + i * 8);     // <-- account + 0x10[i] = character SOID
///       lVar11    = FUN_0056CA70(reg, 3, key, &local_6e8);
/// ```
///
/// and the card builder `0xA110F0` gates on `cmp dword [rbx+8], 0; jle` - count 0 means zero cards.
///
/// **The bug this fixes.** `build_family_body_inner` built the list as `&[chr]` unconditionally, so
/// `D1_CHARACTER_SOID=0` produced a list of **one character whose SOID is 0**, not an empty list.
/// Measured through the client's own codec, which is the control that matters:
///
/// ```text
///   account_rsat(acct, &[],  prof) -> decoded object has non-zero at +0x00, +0x68 only
///   account_rsat(acct, &[0], prof) -> decoded object has +0x08 = 1     <-- "one Guardian, SOID 0"
/// ```
///
/// So the "serve an account with no characters" path existed as *dead code* (the `unused variable:
/// char_list` warning) and the client was told, every run, that a Guardian already exists. That is
/// why it has never once asked us to create one.
///
/// ⚠ **Default is unchanged.** `run-boot.sh` pins a non-zero `D1_CHARACTER_SOID`, so every proven
/// route to orbit keeps sending exactly one character. This only takes effect when the operator
/// asks for zero.
fn serves_no_characters(chr: u64) -> bool {
    if CHARACTER_CREATED.load(std::sync::atomic::Ordering::Relaxed) {
        return false;
    }
    // ⛔⛔ EMPTY MUST MEAN OFF, and this is not defensive coding - it is a bug that was live for
    // about ten minutes on 2026-08-10. The obvious way to give a knob a documented home in
    // `run-boot.sh` is `export D1_EMPTY_ROSTER="${D1_EMPTY_ROSTER:-}"`, which is what every other
    // optional knob in that file does. That sets the variable to the EMPTY STRING, so `var()`
    // returns `Ok("")`, and `"" != "0"` is **true** - every ordinary boot would have silently
    // become a character-creation run, served an account with no profile, and stalled at
    // `character:signin` looking exactly like a regression in whatever else was being tested.
    // ⇒ A boolean env knob must treat unset and empty identically. `run-boot.sh` no longer exports
    // this one at all, and this line means it would not matter if it did.
    chr == 0
        || std::env::var("D1_EMPTY_ROSTER").is_ok_and(|v| !v.is_empty() && v != "0")
}

/// The character SOID to serve: the one the operator pinned, or - once we have answered a **501** -
/// the one we minted for the Guardian the client just created.
fn effective_character_soid(chr: u64) -> u64 {
    let created = CREATED_CHARACTER_SOID.load(std::sync::atomic::Ordering::Relaxed);
    if created != 0 { created } else { chr }
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
//  ⭐ THE CHARACTER LIFECYCLE - investment 501 / 502 / 505 (and 506).
//
//  `THE-BLOCKS.md` B11: the client→server COMMAND direction, which this project had named but never
//  implemented. Ids resolved from the shipped definition table's FNV-1 name hashes and confirmed
//  against the literal immediates in the senders - see `d1_bap::character_lifecycle` and
//  `historicaldocs/design/character-lifecycle-messages.md`.
//
//  ⛔ **DEFAULT OFF.** Every run that ever reached orbit did so without these branches. A boot
//  regression must never be attributable to a component the proven route never had. `D1_INV_SILENT`
//  still wins over this, and 504 deliberately keeps its own long-standing branch untouched.
//
//  ⚠ What we do NOT claim: nothing here has ever been on a wire. These are the client's own
//  encoder's bytes for objects we chose - `synthetic-minimal`, and everything downstream of them is
//  `synthetic_ancestor`. The one thing that will be real is the **create transcript**: that is the
//  client specifying a Guardian, which is strictly better evidence than anything we can invent.
// ═══════════════════════════════════════════════════════════════════════════════════════════════

/// Answer 501/502/505/506. **Default ON** since 2026-08-10; `D1_CHARACTER_LIFECYCLE=0` opts out.
///
/// # Why the default flipped, and the measurement that says it is safe `[C]`
///
/// This shipped default-off on the reasoning that *"every run that ever reached orbit did so
/// without these branches, so a boot regression must never be attributable to a component the
/// proven route never had."* That reasoning is sound and the conclusion was still wrong, because
/// the premise is checkable: **the proven route never SENDS these messages.** A census of every
/// `INVESTMENT message id N` arrival across all 249 responder logs in the tree:
///
/// ```text
///   501  9 arrivals over 8 runs - framefix, silent501, char501, suback x2, bapprobe,
///                                 HOLD-SELECT, CREATE-ANSWER, CREATE-ROSTER
///   505  2 arrivals over 2 runs - CACHE-REBUILD, CREATOR-CLEAN
///   502 / 506  never
/// ```
///
/// and **not one of those ten runs contains a single `Entering state 'orbit…'` line.** 501 is sent
/// by `cui::c_character_creation_selection_data_component::vfunc[14]`, i.e. only when the creator
/// is open; 505 only on an explicit signout. So switching this on changes the behaviour of exactly
/// zero runs that were going to reach orbit, and changes a *stall* into an *answer* on the runs
/// that were not. An unanswered investment message is not a legal outcome - policy families 0 and 1
/// both map `0x000A → 0x000B`, so the client always has an in-flight entry waiting.
///
/// ⭐ It is also half of the P1 interlock: [`d1_bap::queuez_family4_account_only_body`] deliberately
/// withholds the profile so the client stays in its creator, and the *only* way out of that state
/// is a 501 we answer. Shipping the builder with the reply still switched off would convert a
/// wasted boot into a wasted boot that also hangs.
fn character_lifecycle_enabled() -> bool {
    // Empty means unset - see `serves_no_characters` for the ten-minute bug that earned this rule.
    !std::env::var("D1_CHARACTER_LIFECYCLE").is_ok_and(|v| v == "0")
}

/// Where a create transcript lands. `run-boot.sh` exports `RUN_OUT`, so with no extra configuration
/// the artifact appears in the run directory next to `responder.log`.
fn transcript_dir() -> String {
    for k in ["D1_CHARACTER_TRANSCRIPT_DIR", "RUN_OUT", "D1_UPLOAD_DIR"] {
        if let Ok(v) = std::env::var(k) {
            if !v.is_empty() {
                return v;
            }
        }
    }
    "/tmp/d1-uploads".to_string()
}

/// ⭐ **The deliverable.** Persist one create-character request, raw bytes first.
///
/// Two files, deliberately: a `.txt` that a human reads and a `.bin` that a tool re-decodes. A
/// decode can always be re-derived from the bytes; the bytes can never be re-derived from a decode,
/// and this project has already been burned by findings that existed only as prose.
fn write_create_transcript(
    req: Option<&d1_bap::character_lifecycle::CreateCharacterRequest>,
    wire_body: &[u8],
    args: &[u8],
    minted_soid: u64,
    decode_error: Option<String>,
) -> Option<String> {
    let dir = transcript_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        warn!("      ⛔ create transcript: cannot create {dir}: {e}");
        return None;
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let base = format!("{dir}/create-character-{stamp}");

    // The raw argument bitstream, byte-exact, no framing of our own.
    if let Err(e) = std::fs::write(format!("{base}.bin"), args) {
        warn!("      ⛔ create transcript: cannot write {base}.bin: {e}");
    }

    let mut text = match req {
        Some(r) => r.transcript(wire_body),
        None => format!(
            "# create_character transcript - ⛔ DECODE FAILED, RAW BYTES ONLY\n\
             #\n\
             # investment message 501 `login_create_character`\n\
             # {}\n\
             #\n\
             # ⭐ The bytes below are still the evidence. The decoder is a convenience; its failure\n\
             # is a statement about the decoder, not about the request.\n\
             \n\
             wire_body_len  {}\n\
             wire_body      {}\n\
             arg_bitstream  {}\n",
            decode_error.as_deref().unwrap_or("(no error recorded)"),
            wire_body.len(),
            wire_body.iter().map(|b| format!("{b:02x}")).collect::<String>(),
            args.iter().map(|b| format!("{b:02x}")).collect::<String>(),
        ),
    };
    text.push_str(&format!("\n[our answer]\nminted_character_soid  {minted_soid:#018x}\n"));
    if let Err(e) = std::fs::write(format!("{base}.txt"), &text) {
        warn!("      ⛔ create transcript: cannot write {base}.txt: {e}");
        return None;
    }
    Some(format!("{base}.txt"))
}

/// Build the reply body for a character-lifecycle message, or `None` to fall through to the
/// existing dispatch.
///
/// ⭐ **Returning `None` when the gate is off is deliberate and loud.** An unanswered investment
/// message is a stall, not a legal outcome (`historicaldocs/design/message-catalogue.md` §4.1: policy families 0
/// and 1 both map `0x000A → 0x000B`, so the client always has an in-flight entry waiting). So if
/// one of these ever arrives with the gate off, the run should say so in the loudest terms
/// available and then behave exactly as it did before - which is what makes the A/B honest.
fn character_lifecycle_reply(id: u16, args: &[u8], wire_body: &[u8]) -> Option<Vec<u8>> {
    use d1_bap::character_lifecycle as cl;
    let name = match id {
        cl::CREATE_CHARACTER => "login_create_character",
        cl::DELETE_CHARACTER => "login_delete_character",
        cl::SIGNOUT_CHARACTER => "signout_character",
        cl::CUSTOMIZE_CHARACTER => "customize_character",
        // 504 `login_character` keeps its own long-standing branch. Do not double-handle it.
        _ => return None,
    };

    if !character_lifecycle_enabled() {
        warn!(
            "      ** NAMED GATE ** investment {id} `{name}` arrived - the CHARACTER LIFECYCLE, \
             and D1_CHARACTER_LIFECYCLE is OFF, so we are NOT answering it. \
             {} arg bytes: {}",
            args.len(),
            args.iter().take(64).map(|b| format!("{b:02x}")).collect::<String>()
        );
        return None;
    }

    match id {
        cl::CREATE_CHARACTER => {
            // ⭐⭐ The client is telling us what a Guardian is. Record it BEFORE anything else can
            // fail - the transcript is the deliverable, the reply is only plumbing.
            let soid = env_soid("D1_CREATED_CHARACTER_SOID", 0xD100_0000_0000_0101);
            let decoded = cl::CreateCharacterRequest::decode(args);
            match &decoded {
                Ok(r) => warn!(
                    "      ⭐⭐ CREATE CHARACTER: race={} gender={} class={} \
                     name_hash={:#010x} online_id={:?} - minting SOID {soid:#018x}",
                    r.race, r.gender, r.class, r.customization.f10_raw32, r.name_str()
                ),
                Err(e) => warn!(
                    "      ⛔ CREATE CHARACTER: {e} - recording the raw bytes anyway and \
                     answering, because silence on an investment message is a stall, not a refusal"
                ),
            }
            let path = write_create_transcript(
                decoded.as_ref().ok(),
                wire_body,
                args,
                soid,
                decoded.as_ref().err().map(|e| e.to_string()),
            );
            if let Some(p) = path {
                warn!("      ⭐ create-character transcript written: {p}");
            }
            CREATED_CHARACTER_SOID.store(soid, std::sync::atomic::Ordering::Relaxed);
            CHARACTER_CREATED.store(true, std::sync::atomic::Ordering::Relaxed);
            Some(cl::create_character_response_body(soid))
        }
        cl::DELETE_CHARACTER => {
            let soid = cl::character_soid_argument(args);
            warn!("      ⭐ DELETE CHARACTER: soid={soid:#018x?} - acknowledging; \
                   the roster is NOT changed (we do not model deletion state)");
            Some(cl::delete_character_response_body())
        }
        cl::SIGNOUT_CHARACTER => {
            // The request is genuinely empty - schema 0x808017E5 has zero fields.
            warn!("      ⭐ SIGNOUT CHARACTER (empty request, {} arg bytes) - acknowledging",
                  args.len());
            Some(cl::signout_character_response_body())
        }
        cl::CUSTOMIZE_CHARACTER => {
            warn!("      ⭐ CUSTOMIZE CHARACTER: {} arg bytes: {} - acknowledging",
                  args.len(),
                  args.iter().take(48).map(|b| format!("{b:02x}")).collect::<String>());
            Some(cl::customize_character_response_body())
        }
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
//  ⭐⭐ INVESTMENT 206 - `fetch_family`, and the PURGE it can trigger.
//
//  The client asks `fetch_family{family, key}` and we reply with that family's QueueZ delivery
//  attached as a wrapper-level SPAN (`d1_bap::fetch_family_response_body`).
//
//  ⛔⛔ WHY THIS STOPPED BEING FAMILY-BLIND. Until 2026-08-10 this arm never parsed the request: it
//  returned the pinned `D1_206_BODY` constant for every 206, whatever was asked. That constant is a
//  **family-4 baseline with `flags = 0x01` and `object_count = 1`**, round-tripped through the
//  client's own codec to prove it:
//
//      d1-oracle message --tag 0x80801869 --msgid 206 \
//        --span 0000000100000004 d1..0001 00000000 01 00000001 64ae75bf d1..0001 00000009 e88..80
//      -> wire 55B 00ce 1801900000000800000026880000000000000800000000080000000b2573adf...
//         (byte-identical to run-boot.sh's pin)
//
//  `FUN_0056AF60` treats a delivery whose family is at replication state 3 with `flags & 1` as a
//  **PURGE to the delivered keep-list**:
//
//      0x56afda  movsx eax, byte [r12+r14+0x88]   ; the family's replication state
//      0x56afe3  cmp   eax, 3
//      0x56afe8  test  byte [r15], 1              ; QUEUEZ_FLAG_BASELINE
//      0x56b023  call  0x56b2d0                   ; ... trim to the keep-list
//
//  At BOOT family 4 is at state 1, so the constant is discarded (harmless - `emulator.log:6666`).
//  At character-creation **FINISH** family 4 is at state 3, so the same constant is ACCEPTED and
//  trims family 4 to one object - and that object's 9-byte payload decodes to `+0x0000` alone, so
//  it would leave the account with `+0x08 = 0` (zero selection cards) and `+0x68 = 0` (slot B dead
//  forever), destroying the profile rung 33 and rung 34's task 7 read. Never observed, because no
//  creation has ever completed. `cui::c_character_creation_selection_data_component::vfunc[14]`
//  (`0x9D1850`) sends 501 at `0x9D1F95` and then 206 at `0x9D1FAE` back to back, without waiting.
//
//  ⚠ THE HAZARD SURVIVES THE FIX IN A SMALLER FORM, and that is deliberate rather than overlooked:
//  the family-3 answer is also a baseline, and family 3 is live at state 3 at Finish, so it takes
//  the same accept-and-purge arm and trims family 3 to *our* keep-list. That is the correct
//  semantics for a baseline - but it means a one-character delivery here would delete a
//  previously-created Guardian. `build_family_body` is the single place that decides what the
//  keep-list is, which is why this routes through it rather than building a body of its own.
// ═══════════════════════════════════════════════════════════════════════════════════════════════

/// The families we can build a delivery for. Anything else gets the bare no-span reply rather than
/// a plausible-looking wrong answer - `find(family, type, key)` (`0x56CA70`) returns 0 for "no slot"
/// and for "no element" alike, so a mis-routed span is invisible to every consumer in the binary.
const FETCH_FAMILY_SERVED: [u32; 2] =
    [d1_bap::QUEUEZ_FAMILY_CHARACTER, d1_bap::QUEUEZ_FAMILY_ACCOUNT];

/// Build the reply body for one investment **206** `fetch_family`, given its argument bitstream.
///
/// # ⚠ WHAT IS AND IS NOT PROVEN ABOUT THE REPLY SHAPE `[C]`, measured over all 249 responder logs
///
/// ```text
///   206 answered with the family-4 constant : 144 runs, 82 reached `setup:orbit`, 0 halted
///   206 answered with NO span (body 0x10)   :  15 runs,  0 reached `setup:orbit`, 0 halted
/// ```
///
/// So **a 206 reply carrying a span is the proven shape** and this change keeps it - it changes
/// *which family* the span delivers, not whether one is attached. ⛔ It also means `D1_206_SPAN=0`
/// is **not a proven-safe fallback**: no run answering 206 without a span has ever reached orbit.
/// (The 15 that did are all early `account-trace` runs that died upstream for unrelated reasons, so
/// this is a confounded comparison, not a demonstration that the no-span reply is fatal - and
/// `run-boot.sh`'s old comment claiming a NULL-span crash at `0x570AAE` is not supported by these
/// logs either: zero halts on either side.) Reach for it to bisect, not to be careful.
///
/// `D1_206_BODY=<hex>` still overrides the whole body, but **only when the span it carries is for
/// the family that was actually asked for** - see the guard below.
fn message_206_body(args: &[u8]) -> Vec<u8> {
    let req = d1_bap::fetch_family_request(args);
    match req {
        Some(r) => warn!(
            "      206 fetch_family: family={} key={:#018x}  raw={}",
            r.family, r.key, hexdump(args, 16)
        ),
        // Loud, because the alternative reading - "the client asked for family 0" - is exactly what
        // an un-biased decode of a malformed request produces, and it is wrong.
        None => warn!(
            "      ⛔ 206 fetch_family: COULD NOT DECODE {}B of arguments ({}) - replying with no \
             span. The request is a 4-bit family leaf (biased +1) then a 64-bit key = 9 bytes.",
            args.len(), hexdump(args, 16)
        ),
    }

    // `D1_206_BODY` - kept as a bisect hook, but it can no longer answer the wrong question.
    let body_override = hex_env("D1_206_BODY");
    if !body_override.is_empty() {
        let ov_family = d1_bap::fetch_family_response_span(&body_override)
            .as_deref()
            .and_then(d1_bap::queuez_span_family);
        match (req, ov_family) {
            (Some(r), Some(f)) if f == r.family => {
                warn!("      -> message-206 D1_206_BODY override ({}B, family {f}): {}",
                      body_override.len(), hexdump(&body_override, 64));
                return body_override;
            }
            (Some(r), Some(f)) => warn!(
                "      ⛔⛔ D1_206_BODY IGNORED: it carries a FAMILY-{f} span and the client asked \
                 for family {}. Answering family {} with family {f}'s baseline is the PURGE that \
                 deletes the profile at creation-finish. Unset it, or generate one for family {}.",
                r.family, r.family, r.family
            ),
            (_, None) => warn!(
                "      ⛔ D1_206_BODY IGNORED: no single-family QueueZ span could be read back out \
                 of it ({}B). Generate it with `d1-oracle message --tag 0x80801869 --msgid 206 \
                 --span <queuez hex>` and take the bytes after the 2-byte msgid.",
                body_override.len()
            ),
            (None, _) => warn!("      ⛔ D1_206_BODY IGNORED: the request itself did not decode."),
        }
    }

    let Some(r) = req else { return d1_bap::fetch_family_response_body(None) };

    if std::env::var("D1_206_SPAN").is_ok_and(|v| v == "0") {
        warn!("      -> message-206 no-span reply (D1_206_SPAN=0) - ⚠ a BISECT shape, not a safe \
               one: 0 of the 15 runs that ever answered 206 without a span reached setup:orbit");
        return d1_bap::fetch_family_response_body(None);
    }
    if !FETCH_FAMILY_SERVED.contains(&r.family) {
        warn!("      -> message-206 no-span reply: family {} is not one we build ({:?})",
              r.family, FETCH_FAMILY_SERVED);
        return d1_bap::fetch_family_response_body(None);
    }

    let acct = env_soid("D1_ACCOUNT_SOID", 0xD100_0000_0000_0001);
    let chr = env_soid("D1_CHARACTER_SOID", 0xD100_0000_0000_0101);
    let span = build_family_body(r.family, acct, chr, "");
    // object_count lives at span[21..25] - `queuez_family_header_for` is
    // `u32 family_count | u32 family | u64 root | u32 version | u8 flags | u32 object_count`.
    warn!("      -> message-206 family-{} span ({}B, object_count {}) - answering the family the \
           client ASKED for",
          r.family, span.len(),
          span.get(21..25).map(|b| u32::from_be_bytes(b.try_into().unwrap())).unwrap_or(0));
    d1_bap::fetch_family_response_body(Some(&span))
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
//  ⭐⭐ P2 - THE SECOND FAMILY-4 DELIVERY, and why it is a DELTA.
//
//  `0x25FF40(ctx)` hands out the availability cache and rebuilds its `0x4000`-byte evaluation plane
//  ONLY when the cache's validity byte is zero. `0x260200` stamps that byte on success AND on
//  failure alike, so the plane is computed once - from whatever the profile held at the time - and
//  never re-asked. The clearers are `0x574440(family)` and `0x574460(family)`, both `cmp edi,4`.
//
//  `0x574440(4)` sits on the family-update END chain, which every ACCEPTED family-4 update walks:
//
//      0x565140 push:queuez  --(0x565569, unconditional on accept)-->  0x56B5E0
//      0x56B5E0  mov edi,esi / mov rsi,rdx / mov rdx,r9 / jmp 0x56D0F0     (4 instrs, no prologue)
//      0x56D0F0  --(0x56D113, edi untouched)-->  0x56D1A0
//      0x56D1A0  if (family == 4) { …; FUN_00574440(4); … }
//      0x574440  cmp edi,4 / lea rdi,[0x26A34B0] / call [vtable+0xC8] / mov byte [rax],0
//
//  ⭐ The per-object classifier `0x56B5F0` hangs off `0x5655B0` and is NOT on that chain. So the
//  invalidation does not depend on any object being classified as an UPDATE, which is why
//  [`Family4Invalidate::Ping`] - a delta with `object_count = 0` - is the default: it is the only
//  shape with no decode, no keep-list and no classifier, hence nothing it can corrupt.
// ═══════════════════════════════════════════════════════════════════════════════════════════════

/// The shape of the family-4 re-delivery that invalidates the availability cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family4Invalidate {
    /// `D1_FAMILY4_INVALIDATE=0` - do not re-deliver. The pre-2026-08-10 behaviour, kept as the
    /// bisect arm: the cache stays valid and the plane keeps whatever it computed at boot.
    Off,
    /// A delta carrying **no objects**. Purge-free by construction and still reaches `0x574440(4)`.
    Ping,
    /// A verbatim re-stamp of the family-4 baseline we sent, as a delta. Same objects, same bytes.
    Objects,
}

/// ⚠ Unset and EMPTY both mean the default. `export D1_FAMILY4_INVALIDATE="${…:-}"` is how every
/// optional knob gets a home in `run-boot.sh`, and a `v != "0"` read of that is how
/// `D1_EMPTY_ROSTER` silently turned every boot into a creation run for ten minutes on 2026-08-10.
fn family4_invalidate_mode() -> Family4Invalidate {
    match std::env::var("D1_FAMILY4_INVALIDATE").unwrap_or_default().trim() {
        "0" => Family4Invalidate::Off,
        "objects" => Family4Invalidate::Objects,
        "" | "1" | "ping" => Family4Invalidate::Ping,
        other => {
            warn!("      D1_FAMILY4_INVALIDATE={other:?} is not one of 0|ping|objects - using ping");
            Family4Invalidate::Ping
        }
    }
}

/// Build the follow-up family-4 delta from the baseline span we actually put on the wire.
///
/// `None` means *do not send anything* - the caller says so loudly rather than downgrading to a
/// shape the operator did not ask for.
///
/// ⚠ `D1_FAMILY4_SERIAL=N` (default 0 = off) additionally re-encodes the ACCOUNT object with
/// account schema field [2] - the 32-bit leaf at object `+0x60` - set to `N`, which is the smallest
/// change that makes `0x56B5F0`'s `memcmp(old, decoded, stride)` differ and therefore routes the
/// object to the UPDATE announcer `0x56D3F0` instead of announcing nothing. **It is not needed for
/// the cache invalidation** (see the block comment above) and whether `+0x60` is inert is `[U]`, so
/// it is default OFF. See `d1_bap::account_rsat_with_serial`.
fn build_family4_delta(base: &[u8], mode: Family4Invalidate, version: u32) -> Option<Vec<u8>> {
    let acct = env_soid("D1_ACCOUNT_SOID", 0xD100_0000_0000_0001);
    match mode {
        Family4Invalidate::Off => None,
        Family4Invalidate::Ping => Some(d1_bap::queuez_family4_delta_ping(acct, version)),
        Family4Invalidate::Objects => {
            let serial = env_num::<u32>("D1_FAMILY4_SERIAL", 0);
            let span = if serial == 0 {
                base.to_vec()
            } else {
                // ⛔ Two shapes we must NOT re-encode over. `D1_ACCOUNT_RSAT` is a verbatim payload
                // override, and the empty-roster account is the field-3-ABSENT form - re-deriving
                // either from the SOIDs would silently ship a different account object than the
                // baseline did, which is precisely the drift the re-stamp design exists to avoid.
                let chr = effective_character_soid(
                    env_soid("D1_CHARACTER_SOID", 0xD100_0000_0000_0101));
                if std::env::var("D1_ACCOUNT_RSAT").is_ok() || serves_no_characters(chr) {
                    warn!("      ⛔ D1_FAMILY4_SERIAL={serial} REFUSED: the baseline's account object \
                           is an override or the field-3-ABSENT empty-roster form, and re-encoding it \
                           would change more than the serial. Sending the verbatim re-stamp instead.");
                    base.to_vec()
                } else {
                    let prof = env_soid("D1_PROFILE_SOID", 0xD100_0000_0000_0201);
                    let bumped = d1_bap::account_rsat_with_serial(acct, &[chr], prof, serial);
                    match d1_bap::queuez_span_replace_object_payload(
                        base, 0, d1_bap::account_instance_type(), &bumped)
                    {
                        Some(s) => {
                            warn!("      ⭐ D1_FAMILY4_SERIAL={serial}: account field [2] (+0x60) set \
                                   - 0x56B5F0's memcmp now differs, so the object is announced as an \
                                   UPDATE (0x56D3F0) rather than as nothing");
                            s
                        }
                        None => {
                            warn!("      ⛔ D1_FAMILY4_SERIAL={serial} REFUSED: object 0 of the \
                                   recorded family-4 span is not {:#010x} - sending the verbatim \
                                   re-stamp instead", d1_bap::account_instance_type());
                            base.to_vec()
                        }
                    }
                }
            };
            d1_bap::queuez_span_as_delta(&span, version)
        }
    }
}

fn build_family_body(family: u32, acct: u64, chr: u64, probe: &str) -> Vec<u8> {
    let chr = effective_character_soid(chr);
    if serves_no_characters(chr) {
        if family == d1_bap::QUEUEZ_FAMILY_CHARACTER {
            warn!("      ⭐ QUEUEZ family 3: EMPTY ROSTER (0 characters) - client should offer CREATION");
            return d1_bap::queuez_family3_empty_roster_body(acct);
        }
        // family 4: account with ZERO characters. Can't pass through build_family_body_inner
        // because it constructs &[chr] and chr=0 produces a one-element list with SOID 0.
        //
        // ⭐⭐ AND IT MUST CARRY NO PROFILE OBJECT AT ALL. Until 2026-08-10 this line built the
        // ordinary two-object baseline, so the empty-roster delivery still named a profile in
        // account field 3 and still shipped a `PROFILE_INSTANCE_TYPE` object beside it. That
        // resolves `0x585820`'s slot B on the very FIRST family-4 delivery of every boot:
        //
        //     0x585885  cmp qword [r14+0x68], 0        ; the profile SOID we were sending
        //     0x585893  call 0x5833C0(&account+0x68)   ; find(4, TYPE 1, *(acct+0x68)) - it hits
        //     0x58589f  mov qword [rbx+0x20], rax      ; slot B filled
        //
        // and a filled slot B makes ladder `0x78C5F0` return 6, which
        // `c_world_controller_state_character_signin_handler::vfunc[7]` reads at 0x7A01D1/0x7A01D6
        // as "signin is done". The creator is pulled out from under the player on any tick where
        // `0x578450` is false - which is why all three historical creation runs (CREATE-ROSTER,
        // CREATE-ANSWER, HOLD-SELECT) end at `Entering state 'character:signin'`.
        //
        // ⛔ DO NOT "fix" this by passing `prof = 0`. That yields a profile object KEYED 0, which is
        // a live element of the family-4/type-1 slot, not an absent one - see
        // `d1_bap::account_rsat_no_profile`'s doc comment, which measures the difference through the
        // client's own codec.
        warn!("      ⭐⭐ QUEUEZ family 4: account with **0** characters and **NO PROFILE OBJECT** \
               (account+0x08 count = 0, account+0x68 ABSENT) - this is what makes the client run \
               character CREATION *and stay in it*");
        return d1_bap::queuez_family4_account_only_body(acct);
    }
    build_family_body_inner(family, acct, chr, probe)
}

fn build_family_body_inner(family: u32, acct: u64, chr: u64, probe: &str) -> Vec<u8> {

                                                                if probe == "empty" {
                                                                    warn!("      QUEUEZ PROBE: family {family}, object_count=0 (family/root lookup discriminator)");
                                                                    d1_bap::queuez_empty_family_body_for(family, acct)
                                                                } else if family == d1_bap::QUEUEZ_FAMILY_ACCOUNT {
                                                                    // The account object carries the CHARACTER LIST, and
                                                                    // `0xA110F0` - the selection-card builder - reads the
                                                                    // count out of THIS object, not out of the roster:
                                                                    //     0a112cd  cmp dword [rbx+8], 0
                                                                    //     0a112d1  jle 0xa116bb        ; 0 -> zero cards
                                                                    // An account whose list is empty therefore yields the
                                                                    // SELECTION screen with no guardian on it, which is
                                                                    // exactly what the operator saw (§26.66).
                                                                    //
                                                                    // `D1_ACCOUNT_RSAT` remains an override for experiments;
                                                                    // the default is now derived from the schema.
                                                                    // ⚠ The "when chr == 0, pass an empty character list"
                                                                    // intent used to live here as a `char_list` local that
                                                                    // NOTHING READ - the `unused variable` warning in every
                                                                    // build. It now lives in `serves_no_characters`, which
                                                                    // short-circuits before this function is entered.
                                                                    // `D1_ACCOUNT_RSAT` remains an override for single-object
                                                                    // experiments; without it we send the family-4 BASELINE:
                                                                    // account + profile in one transaction.
                                                                    //
                                                                    // The PROFILE (instance 0x8127F15A) is on the orbit path.
                                                                    // `setup:activity_world_transition` says so itself:
                                                                    //   "Updating orbit-destination to '%s --> %s' in the
                                                                    //    character profile."
                                                                    //   "Unable to update orbit-destination -- cannot find
                                                                    //    character profile."
                                                                    // and `setup:orbit_loading` (state 26) then resolves a
                                                                    // slice-set from that orbit-destination. A missing profile
                                                                    // is a gate two states past the one killing us now.
                                                                    if std::env::var("D1_ACCOUNT_RSAT").is_ok() {
                                                                        let pay = hex_env("D1_ACCOUNT_RSAT");
                                                                        warn!("      QUEUEZ ACCOUNT override (D1_ACCOUNT_RSAT) {}B - profile NOT sent",
                                                                              pay.len());
                                                                        d1_bap::queuez_object_body(
                                                                            d1_bap::QUEUEZ_FAMILY_ACCOUNT, acct,
                                                                            d1_bap::account_instance_type(), acct, &pay)
                                                                    } else {
                                                                        // ⭐⭐ PHASE-AWARE, and it has to be. Measured 2026-08-10
                                                                        // across three boots (CREATE-1/2/3):
                                                                        //   BEFORE creation the account must carry +0x68 = 0, or
                                                                        //     `0x585885 cmp qword [r14+0x68],0` passes, `0x78C5F0`
                                                                        //     returns 6, `0x7A01D6 cmp eax,6` reads that as "signin
                                                                        //     done", and character:signin exits to
                                                                        //     character:prologue ONE TICK BEFORE the create-vs-select
                                                                        //     branch `0x78C9F0` is evaluated. The creator is pulled
                                                                        //     out from under the player. 501 never goes out.
                                                                        //   AFTER creation it must carry the REAL profile SOID, or
                                                                        //     slot B can never resolve and the client sits in
                                                                        //     character:signin forever with a finished Guardian and
                                                                        //     nowhere to put her (measured: 60 s parked, 0 error.*).
                                                                        // ⛔ DO NOT do this with an env var. `D1_PROFILE_SOID=0`
                                                                        //   reached the creator on CREATE-3 and then made the second
                                                                        //   half impossible, because one value cannot be right for
                                                                        //   both phases. `serves_no_characters()` already returns
                                                                        //   false once CHARACTER_CREATED is set - key off THAT.
                                                                        let chr_now = effective_character_soid(
                                                                            env_soid("D1_CHARACTER_SOID", 0xD100_0000_0000_0101));
                                                                        let prof = if serves_no_characters(chr_now) {
                                                                            warn!("      ⭐⭐ QUEUEZ family 4: profile SOID withheld \
                                                                                   (+0x68 = 0) - pre-creation, so character:signin's \
                                                                                   slot B stays UNRESOLVED and the creator holds");
                                                                            0
                                                                        } else {
                                                                            env_soid("D1_PROFILE_SOID", 0xD100_0000_0000_0201)
                                                                        };
                                                                        // ⭐ last_orbited_destination. The client reads it at
                                                                        // view+0x480 (0x752AE0) and compares against
                                                                        // 0x811C9DC5 - the FNV-1a-32 basis, i.e. the hash of
                                                                        // the EMPTY STRING, i.e. "unset". Live reads found
                                                                        // that sentinel at profile object +0x28, which is
                                                                        // profile field [2] (0x8080181A) sub-field [10], a
                                                                        // 32-bit leaf @+0x01c. 0x00c + 0x01c = 0x28. ✔
                                                                        //
                                                                        // Set D1_ORBIT_DESTINATION=0 to send the old
                                                                        // SOID-only profile and reproduce the fallback.
                                                                        let dest = std::env::var("D1_ORBIT_DESTINATION").ok()
                                                                            .and_then(|v| v.trim_start_matches("0x").parse::<u32>().ok()
                                                                                .or_else(|| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok()))
                                                                            .unwrap_or(d1_bap::DESTINATION_OLD_RUSSIA);
                                                                        let dest = (dest != 0).then_some(dest);
                                                                        match dest {
                                                                            Some(d) => warn!("      QUEUEZ family 4 baseline: account (instance {:#010x}, character list [{chr:#018x}]) + profile (instance {:#010x}, soid {prof:#018x}) ⭐ last_orbited_destination = {d} ({d:#010x})",
                                                                                             d1_bap::account_instance_type(),
                                                                                             d1_bap::profile_instance_type()),
                                                                            None => warn!("      QUEUEZ family 4 baseline: account + profile (soid {prof:#018x}), destination NOT set (D1_ORBIT_DESTINATION=0)"),
                                                                        }
                                                                        // D1_PROFILE_FILE: a verbatim profile payload produced
                                                                        // by `d1-oracle object --tag 0x80801743 --obj <C struct
                                                                        // with +0x28 = destination>`. Preferred over the sparse
                                                                        // encoder, which is under-length once field [2] is
                                                                        // present (§26.67) and silently writes 0.
                                                                        // D1_UNLOCKS: add the family-4/type-4 object the
                                                                        // director's activity enumerators look up
                                                                        // (`0x574220`/`0x574340` -> find(family=4, type=4,
                                                                        // key=0x7FFFFFFFFFFFFFFE)). We have NEVER sent it; the
                                                                        // lookup executes on a live orbit client and returns
                                                                        // NULL purely because the object is absent.
                                                                        //
                                                                        // ⚠ DEFAULT OFF, deliberately. The proven route to
                                                                        // orbit ran with this object ABSENT, and family 4/type 4
                                                                        // is also involved in the character-prologue path -
                                                                        // installing it before character selection could alter
                                                                        // a route that currently works. Opt in per run.
                                                                        //   D1_UNLOCKS=key      key-only, 9 B (see below)
                                                                        //   D1_UNLOCKS=<path>   a payload file (e.g. a populated
                                                                        //                       object from `d1-oracle object
                                                                        //                       --tag 0x8080102E --obj …`)
                                                                        // Design + evidence: historicaldocs/design/director-and-unlocks.md.
                                                                        let unlocks: Option<Vec<u8>> = match std::env::var("D1_UNLOCKS").ok().as_deref() {
                                                                            None | Some("") | Some("0") => None,
                                                                            Some("key") => {
                                                                                warn!("      ⭐ family 4/type 4: KEY-ONLY object (instance {:#010x}, key {:#018x}, 9 B) - empty-but-present",
                                                                                      d1_bap::UNLOCKS_INSTANCE_TYPE, d1_bap::UNLOCKS_OBJECT_KEY);
                                                                                Some(d1_bap::UNLOCKS_KEY_ONLY_PAYLOAD.to_vec())
                                                                            }
                                                                            Some(path) => match std::fs::read(path) {
                                                                                Ok(raw) => {
                                                                                    warn!("      ⭐ family 4/type 4: payload from {path} ({} B, instance {:#010x}, key {:#018x})",
                                                                                          raw.len(), d1_bap::UNLOCKS_INSTANCE_TYPE, d1_bap::UNLOCKS_OBJECT_KEY);
                                                                                    Some(raw)
                                                                                }
                                                                                // Loud, and NOT a silent fallback to key-only:
                                                                                // a run that asked for a populated object and
                                                                                // quietly got an empty one would read as
                                                                                // "contents don't matter", which is a false
                                                                                // negative pointing at the wrong conclusion.
                                                                                Err(e) => {
                                                                                    warn!("      ⛔ D1_UNLOCKS={path}: {e} - sending NO family-4/type-4 object");
                                                                                    None
                                                                                }
                                                                            },
                                                                        };
                                                                        // ⭐ D1_FAMILY42=1 - family 4/type 2, the object whose
                                                                        // absence is a MEASURED NULL at selection-manager
                                                                        // `+0x28` on a live orbit client. `0x574340` files six
                                                                        // arguments into the manager; five are non-NULL and this
                                                                        // one is zero because we have never sent it. Both of the
                                                                        // enumerator's early exits were replayed on a snapshot of
                                                                        // that client and both PASS, so it is not bailing out -
                                                                        // it is being handed a hole. See d1_bap::FAMILY42_*.
                                                                        // ⚠ The ACCOUNT carries the character list, and `0xA110F0`
                                                                        // - the selection-card builder - reads the count from
                                                                        // THIS object, not from the roster. So an empty roster
                                                                        // with a populated account list is incoherent: the client
                                                                        // is told "no Guardians" and "one Guardian" at once.
                                                                        // Keep them in agreement.
                                                                        // ⚠ Reachable only when `serves_no_characters` said NO,
                                                                        // so this list is non-empty by construction. Kept as a
                                                                        // slice (not `&[chr]` inline) so the shape stays obvious.
                                                                        let chars: &[u64] = &[chr];
                                                                        let mut extras: Vec<(u32, u64, Vec<u8>)> = Vec::new();
                                                                        if std::env::var("D1_FAMILY42").map(|v| v != "0").unwrap_or(false) {
                                                                            warn!("      ⭐ family 4/type 2: key-only object (instance {:#010x}, key {acct:#018x}, {} B) - the measured NULL at manager +0x28",
                                                                                  d1_bap::FAMILY42_INSTANCE_TYPE,
                                                                                  d1_bap::FAMILY42_KEY_ONLY_PAYLOAD.len());
                                                                            extras.push((
                                                                                d1_bap::FAMILY42_INSTANCE_TYPE,
                                                                                acct,
                                                                                d1_bap::FAMILY42_KEY_ONLY_PAYLOAD.to_vec(),
                                                                            ));
                                                                        }
                                                                        // ⭐ D1_INVENTORY=<dir-or-file> - family 4/type 3 items.
                                                                        // Each file is ONE 184-byte element encoded by
                                                                        // `d1-oracle object --tag 0x80801732`; each rides as its
                                                                        // own keyed object (capacity 666, so the registry holds
                                                                        // one per item, keyed by the item's instance SOID).
                                                                        //
                                                                        // Item hashes from the archived 2020 manifest DO resolve
                                                                        // on this 2016 client - measured 2026-08-08, Gjallarhorn
                                                                        // Y1 `0x4BF4BE3F` -> slot 0x3a5 index 7743 via
                                                                        // `d1guest tag`. That premise was "believed, not
                                                                        // measured" until then, and it is what makes the whole
                                                                        // gear lane viable.
                                                                        if let Ok(spec) = std::env::var("D1_INVENTORY") {
                                                                            let mut files: Vec<std::path::PathBuf> = Vec::new();
                                                                            let path = std::path::Path::new(&spec);
                                                                            if path.is_dir() {
                                                                                if let Ok(rd) = std::fs::read_dir(path) {
                                                                                    let mut v: Vec<_> = rd.filter_map(|e| e.ok().map(|e| e.path()))
                                                                                        .filter(|p| p.extension().map(|e| e == "bin").unwrap_or(false))
                                                                                        .collect();
                                                                                    v.sort();
                                                                                    files = v;
                                                                                }
                                                                            } else if !spec.is_empty() {
                                                                                files.push(path.to_path_buf());
                                                                            }
                                                                            // Item instance SOIDs are ours to mint; the client
                                                                            // keys the registry slot by them, and they must be
                                                                            // distinct or later items overwrite earlier ones.
                                                                            let base = env_soid("D1_ITEM_SOID_BASE", 0xD100_0000_0000_0301);
                                                                            for (i, f) in files.iter().enumerate() {
                                                                                match std::fs::read(f) {
                                                                                    Ok(raw) => {
                                                                                        let key = base + i as u64;
                                                                                        warn!("      ⭐ inventory item {i}: {} ({} B, instance {:#010x}, key {key:#018x})",
                                                                                              f.file_name().unwrap_or_default().to_string_lossy(),
                                                                                              raw.len(), d1_bap::INVENTORY_INSTANCE_TYPE);
                                                                                        extras.push((d1_bap::INVENTORY_INSTANCE_TYPE, key, raw));
                                                                                    }
                                                                                    Err(e) => warn!("      ⛔ D1_INVENTORY {}: {e}", f.display()),
                                                                                }
                                                                            }
                                                                        }
                                                                        // ⚠ BOTH branches must receive `unlocks`. The live boot
                                                                        // route goes through D1_PROFILE_FILE; wiring only the
                                                                        // other branch would silently do nothing on the path
                                                                        // that actually runs.
                                                                        match std::env::var("D1_PROFILE_FILE").ok()
                                                                            .and_then(|p| std::fs::read(&p).ok())
                                                                        {
                                                                            Some(raw) => {
                                                                                warn!("      ⭐ profile from D1_PROFILE_FILE ({} B, oracle-encoded full object)", raw.len());
                                                                                d1_bap::queuez_family4_baseline_body_raw_profile_with_unlocks(
                                                                                    acct, chars, prof, &raw, unlocks.as_deref(), &extras)
                                                                            }
                                                                            None => d1_bap::queuez_family4_baseline_body_with_unlocks(
                                                                                acct, chars, prof, dest, unlocks.as_deref(), &extras),
                                                                        }
                                                                    }
                                                                } else if family == 2 {
                                                                    // ⭐ FAMILY 2 - PROGRESSIONS, i.e. LEVEL and LIGHT.
                                                                    //
                                                                    // ⚠ Added 2026-08-08 because this arm did not exist, and its
                                                                    // absence was SILENT: `D1_PUSH_FAMILY=4,3,2` fell through to
                                                                    // the `else` below and pushed **family 3 twice**, logging
                                                                    // "family 3 baseline" for both. Nothing said "family 2 is not
                                                                    // a thing I can build".
                                                                    //
                                                                    // `build_family2_body` already existed and was complete - it
                                                                    // was reachable ONLY from the subscription-answer path
                                                                    // (`D1_ANSWER_FAMILIES`) and the resend path. But the client
                                                                    // subscribes class 4 and class 3 only (`0x584EB0` →
                                                                    // `0x584F74`/`0x584F8B`); it never subscribes family 2, so the
                                                                    // answer path can never deliver it. **Push is the only route.**
                                                                    //
                                                                    // Symptom when family 2 is missing, measured in
                                                                    // `runs/20260808-193351-GRANT-ANSWERED`: once the client
                                                                    // actually consumes a full family-4 answer (inventory +
                                                                    // unlocks), it looks the progressions up by hash and dies with
                                                                    //   server_object instance could not be found or created for
                                                                    //   family_type=ENUM(2) instance_type=ENUM(1) soid=0x2975AEF1
                                                                    // → `error.investment_server_object_instance_not_found` → KTL.
                                                                    // ⭐ `0x2975AEF1` = 695578353 and `0xE3547A64` = 3813964388 -
                                                                    // EXACTLY two of the three files in `capture-rig/progressions`.
                                                                    // The content was right; only the delivery was missing.
                                                                    // ⭐⭐ THE ROOT IS THE **CHARACTER**, NOT THE ACCOUNT.
                                                                    // Progressions belong to a character; families 3 and 4 are
                                                                    // account-rooted and that is what made `acct` look right here.
                                                                    // Keyed to the account, the client answers:
                                                                    //   push:queuez: failed to process update for
                                                                    //     [family_type ENUM(2), family_root_soid 0xD100000000000001,
                                                                    //      version 0, flags 0x01, object_count 4]
                                                                    // and earlier, louder:
                                                                    //   server_object instance could not be found or created for
                                                                    //     family_type=ENUM(2) instance_type=ENUM(1) soid=0x2975AEF1
                                                                    // - it was looking up progressions under a root that has none.
                                                                    // Measured in runs/20260808-194706-TITAN-TEST: the payloads are
                                                                    // valid (the oracle round-trips 3813964388 to level 40) and the
                                                                    // slot is right (family 2/1, cap 231, stride 0x30 = the 48-byte
                                                                    // schema 0x80801810) - only the root was wrong.
                                                                    // ⭐⭐ ROOT = THE ACCOUNT. Measured, not reasoned.
                                                                    // The client's own family table (object manager +0x60 type,
                                                                    // +0x80 root, +0x88 state, stride 0x40) registers exactly one
                                                                    // family-2 slot, at index 21:
                                                                    //     i=21  type 2  root 0xD100000000000001  state 2
                                                                    // - the ACCOUNT soid. Families 3/4/5 sit at 252/253/254 in
                                                                    // state 3; family 2 is the only one still at 2.
                                                                    // `0x56AF60` matches on (type AND root) and returns SILENTLY at
                                                                    // lVar9 == 0x4040 when no slot matches, so a character-rooted
                                                                    // family-2 update is discarded with no log at all.
                                                                    // ⚠ I rooted this to `chr` on the theory that progressions
                                                                    // belong to a character. The table says otherwise.
                                                                    build_family2_body(acct)
                                                                } else {
                                                                    // Family 3 baseline = the ROSTER (family root, the object
                                                                    // `0x78C9F0` counts to pick CREATION vs SELECTION) plus
                                                                    // the CHARACTER itself, in one transaction. We had never
                                                                    // sent the roster at all - it was only ever poked into
                                                                    // memory by `capture-rig/inject-roster.py`.
                                                                    //
                                                                    // `D1_NO_ROSTER=1` falls back to character-only, to keep
                                                                    // the previous behaviour available for A/B.
                                                                    // ⚠ The zero-character case is handled in
                                                                    // `build_family_body` before this function runs, so that
                                                                    // families 3 and 4 can never disagree about the count.
                                                                    if std::env::var("D1_NO_ROSTER").is_ok() {
                                                                        warn!("      QUEUEZ family 3: character only (D1_NO_ROSTER)");
                                                                        d1_bap::queuez_character_body(
                                                                            acct, chr,
                                                                            d1_bap::Identity::TITAN_HUMAN_FEMALE)
                                                                    } else {
                                                                        // D1_CHARACTER_FILE: a verbatim character payload from
                                                                        // `d1-oracle object --tag 0x80801954 --obj <2688-byte C
                                                                        // struct>`. The built-in encoder emits identity only -
                                                                        // no level, no light, no gear - which is exactly what
                                                                        // the operator sees on a blank character.
                                                                        let raw_char = std::env::var("D1_CHARACTER_FILE").ok()
                                                                            .and_then(|p| match std::fs::read(&p) {
                                                                                Ok(b) => { warn!("      ⭐ character from D1_CHARACTER_FILE ({} B, oracle-encoded full object)", b.len()); Some(b) }
                                                                                // Loud: silently falling back to the identity-only
                                                                                // character would look like "the full object made
                                                                                // no difference".
                                                                                Err(e) => { warn!("      ⛔ D1_CHARACTER_FILE={p}: {e} - falling back to identity-only"); None }
                                                                            });
                                                                        warn!("      QUEUEZ family 3 baseline: roster (instance {:#010x}) + character (instance {:#010x})",
                                                                              d1_bap::ROSTER_INSTANCE_TYPE,
                                                                              d1_bap::CHARACTER_INSTANCE_TYPE);
                                                                        d1_bap::queuez_family3_baseline_body_raw_character(
                                                                            acct, chr,
                                                                            d1_bap::Identity::TITAN_HUMAN_FEMALE,
                                                                            raw_char.as_deref())
                                                                    }
                                                                }
}


/// Family-2 (progression) body: the root plus one object per progression file.
///
/// ⭐ **This is where LEVEL and LIGHT live.** The 01.29 client has no `level` and no `light` field
/// anywhere - both are *progressions*, named in the archived manifest:
/// `1716568313 character_level` (scope 1, **40 steps**), `695578353 actual_level_light`,
/// `3813964388 actual_level_character`, `1149018548 actual_level`, `71851446 actual_level_gear`.
/// The registry gives family 2 **231 slots** against the manifest's 252 progression definitions.
///
/// Until 2026-08-08 we answered family 2 with `object_count = 0`, which is exactly "a character with
/// no level and no light" - the symptom the operator reported all night.
///
/// Element schema `0x80801810` (48 B): `{u64 progression_hash, u16, u16, u8, {u32, u32, …}}`.
/// Root schema `0x808018B7` (80 B). Each element rides as its own keyed object, keyed by the
/// progression hash - the registry is keyed and capacity is per-family, same as inventory.
fn build_family2_body(root_soid: u64) -> Vec<u8> {
    let mut items: Vec<(u32, u64, Vec<u8>)> = Vec::new();
    if let Ok(dir) = std::env::var("D1_PROGRESSIONS") {
        let path = std::path::Path::new(&dir);
        let mut files: Vec<std::path::PathBuf> = Vec::new();
        if path.is_dir() {
            if let Ok(rd) = std::fs::read_dir(path) {
                let mut v: Vec<_> = rd.filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.extension().map(|e| e == "bin").unwrap_or(false)).collect();
                v.sort();
                files = v;
            }
        } else if !dir.is_empty() {
            files.push(path.to_path_buf());
        }
        // The key must be the progression hash the client looks the object up by; it is the first
        // u64 of the DECODED struct, but the payload is bit-packed, so take it from the filename's
        // companion rather than parsing bits here. We mint sequential keys and ALSO carry the hash
        // inside the payload, so a mismatch shows up as "object present, progression unknown"
        // rather than as silence.
        // ⭐ THE KEY IS THE PROGRESSION HASH, not a minted SOID. The registry is keyed and the
        // client asks for "progression <hash>"; a sequential key would put the object in the family
        // but under a name nothing looks up. So each file is named `<progression_hash>.bin` and the
        // stem IS the key - which also means the key and the payload's own id field cannot drift.
        let base = env_soid("D1_PROGRESSION_KEY_BASE", 0xD100_0000_0000_0401);
        for (i, f) in files.iter().enumerate() {
            match std::fs::read(f) {
                Ok(raw) => {
                    let key = f.file_stem().and_then(|s| s.to_str())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(base + i as u64);
                    warn!("      ⭐ progression {i}: {} ({} B, key {key})",
                          f.file_name().unwrap_or_default().to_string_lossy(), raw.len());
                    items.push((PROGRESSION_INSTANCE_TYPE, key, raw));
                }
                Err(e) => warn!("      ⛔ D1_PROGRESSIONS {}: {e}", f.display()),
            }
        }
    }
    if items.is_empty() {
        return d1_bap::queuez_empty_family_body_for(2, root_soid);
    }
    let root_payload = std::env::var("D1_PROGRESSION_ROOT").ok()
        .and_then(|p| std::fs::read(p).ok());
    let count = items.len() as u32 + root_payload.is_some() as u32;
    let mut b = d1_bap::queuez_family_header_for(2, root_soid, count);
    if let Some(rp) = &root_payload {
        warn!("      ⭐ family-2 ROOT ({} B, instance {:#010x}, key {root_soid:#018x})",
              rp.len(), PROGRESSION_ROOT_INSTANCE_TYPE);
        b.extend_from_slice(&PROGRESSION_ROOT_INSTANCE_TYPE.to_be_bytes());
        b.extend_from_slice(&root_soid.to_be_bytes());
        b.extend_from_slice(&(rp.len() as u32).to_be_bytes());
        b.extend_from_slice(rp);
    }
    for (ty, key, payload) in &items {
        b.extend_from_slice(&ty.to_be_bytes());
        b.extend_from_slice(&key.to_be_bytes());
        b.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        b.extend_from_slice(payload);
    }
    b
}

/// Instance checksum for family 2 / type 1 (schema `0x80801810`, 48 B, capacity 231).
/// Resolve with `d1guest tag 0x80801810` and check `+0x08`; control against
/// `0x8080102E -> 0xC4060975`.
const PROGRESSION_INSTANCE_TYPE: u32 = 0x0444_D2D9;

/// Family 2 / type 0 - the family ROOT (schema `0x808018B7`, 80 B, capacity 231).
/// `d1guest tag 0x808018B7` -> object `0x8013574B8`, `+0x08 = 0x3E3ACE74`, `+0x14 = 0x50`.
/// Every family we have made replicate carries its root; family 4's root is the account and
/// family 3's is the roster. Sending elements without one is the shape that has failed before.
const PROGRESSION_ROOT_INSTANCE_TYPE: u32 = 0x3E3A_CE74;

/// Instance checksums for the roster families, `u32[descriptor_anchor - 0x50 + 8]`.
/// Derived and controlled by `d1-oracle roster controls`, whose `instance-checksums` check
/// reproduces the two constants above from the same expression before believing the family-0 pair.
/// ⛔ Family 0 is listed for COMPLETENESS ONLY - see `build_roster_family_body` for why we must not
/// send it.
const ROSTER_INSTANCE_TYPES: &[((u32, u32), u32)] = &[
    ((2, 0), PROGRESSION_ROOT_INSTANCE_TYPE), // 0x808018B7  contact       80 B, cap 231
    ((2, 1), PROGRESSION_INSTANCE_TYPE),      // 0x80801810  card          48 B, cap 231
    ((0, 0), 0x1D7B_B8F6),                    // 0x80801921  peer contact  80 B, cap 20
    ((0, 1), 0x67D9_7FC4),                    // 0x808017FD  peer view   2632 B, cap 20
];

/// ⭐ `D1_ROSTER_DIR` - serve real family-2 objects so the orbit nameplate BINDS.
///
/// The orbit plate is a ROSTER view. `cui::c_roster_contact_id_container` reads
/// `{key0 = 0, key1 = 0, family = 0xFF}` on a live client - and `0xFF` is the literal constant
/// `0xA3F000` writes on its NULL branch, because the family-2 two-hop `0x57F6F0` finds nothing.
/// `0xA3D4E0` guards every `0x56CA70` call on a non-zero key, so level/light/emblem are never
/// written at all. They are not "read as zero"; the write path never runs.
/// ⇒ families 3/1 and 4/1 - the objects we spent a night filling - are never read by this path.
///
/// Fixtures come from `d1-oracle roster build`, named `f<family>t<type>-<key:016x>.bin`, so the
/// object header's key lives in the filename and cannot drift from the payload's own leading `u64`
/// (the oracle asserts that equality when it writes them). Filename order IS send order: `f2t0-…`
/// sorts before `f2t1-…`, the family root before its elements.
///
/// Returns `None` when unset or when the directory holds nothing for this family, so the caller
/// falls through to the existing behaviour unchanged. **Default OFF.**
///
/// ⚠ `D1_ANSWER_FAMILIES` is still required - this supplies CONTENTS, not the subscription answer.
fn build_roster_family_body(family: u32, root_soid: u64) -> Option<Vec<u8>> {
    let dir = std::env::var("D1_ROSTER_DIR").ok().filter(|d| !d.is_empty())?;
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| warn!("      ⛔ D1_ROSTER_DIR {dir}: {e}"))
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "bin").unwrap_or(false))
        .collect();
    files.sort();

    let mut items: Vec<(u32, u64, Vec<u8>)> = Vec::new();
    for f in &files {
        let name = f.file_name().and_then(|s| s.to_str()).unwrap_or_default();
        let parsed = name
            .strip_suffix(".bin")
            .and_then(|s| s.strip_prefix('f'))
            .and_then(|s| s.split_once('t'))
            .and_then(|(fam, rest)| rest.split_once('-').map(|(ty, key)| (fam, ty, key)))
            .and_then(|(fam, ty, key)| {
                Some((
                    fam.parse::<u32>().ok()?,
                    ty.parse::<u32>().ok()?,
                    u64::from_str_radix(key, 16).ok()?,
                ))
            });
        let Some((fam, ty, key)) = parsed else {
            warn!("      ⛔ D1_ROSTER_DIR: {name} is not f<family>t<type>-<key:016x>.bin - SKIPPED");
            continue;
        };
        if fam != family {
            continue;
        }
        let Some(&(_, instance)) = ROSTER_INSTANCE_TYPES.iter().find(|&&(k, _)| k == (fam, ty))
        else {
            warn!("      ⛔ D1_ROSTER_DIR: family {fam}/type {ty} has no known instance checksum - SKIPPED");
            continue;
        };
        match std::fs::read(f) {
            Ok(raw) => {
                warn!("      ⭐ roster {fam}/{ty}: {name} ({} B, instance {instance:#010x}, key {key:#018x})",
                      raw.len());
                items.push((instance, key, raw));
            }
            Err(e) => warn!("      ⛔ D1_ROSTER_DIR {}: {e}", f.display()),
        }
    }
    if items.is_empty() {
        return None;
    }
    warn!("      ⭐ FAMILY {family} ROSTER BODY: {} object(s), root {root_soid:#018x}", items.len());
    let mut b = d1_bap::queuez_family_header_for(family, root_soid, items.len() as u32);
    for (ty, key, payload) in &items {
        b.extend_from_slice(&ty.to_be_bytes());
        b.extend_from_slice(&key.to_be_bytes());
        b.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        b.extend_from_slice(payload);
    }
    Some(b)
}


/// `POST /control/loadout/apply` - take a loadout document from `d1-forge` and encode each equipped
/// item into a 184-byte family-4/type-3 element, written to `D1_INVENTORY` for the next boot.
///
/// ⚠ **What this does and does not do.** It closes the editor→wire gap: after this, the items the
/// operator picked are real encoded elements the responder will serve. It does **not** make them
/// appear in game - the element's world-tag field is still `[U]` (`historicaldocs/design/light-and-gear.md` §2),
/// and the profile must also *name* each item in one of its 340 slots. So `accepted` here means
/// "encoded and staged", never "equipped".
///
/// Loopback only; `ssh -L` is the auth layer, as `interface-architecture.md` §0 specifies.
async fn loadout_apply(
    axum::Json(req): axum::Json<d1_loadout::apply::ApplyRequest>,
) -> axum::Json<d1_loadout::apply::ApplyResponse> {
    use d1_loadout::apply::ApplyResponse;
    use d1_loadout::{Severity, ValidationIssue};
    // The client is not the authority - re-validate server-side even though d1-forge already did.
    let mut issues: Vec<ValidationIssue> = req.loadout.validate();
    let fatal = issues.iter().any(|i| matches!(i.severity, Severity::Error));

    let dir = std::env::var("D1_INVENTORY").unwrap_or_else(|_| "capture-rig/inventory".into());
    let detail = if fatal {
        "loadout has errors; nothing staged".to_string()
    } else if req.dry_run {
        format!("dry run: {} item(s) would be staged to {dir}", req.loadout.equipped.len())
    } else {
        match stage_loadout(&req.loadout, &dir) {
            Ok(n) => format!("staged {n} item(s) to {dir} - serve with D1_INVENTORY={dir} on the next boot"),
            Err(e) => {
                        issues.push(ValidationIssue { severity: Severity::Error, code: "stage_failed".into(), message: e.to_string() });
                format!("staging failed: {e}")
            }
        }
    };
    let accepted = !fatal && !issues.iter().any(|i| matches!(i.severity, Severity::Error));
    warn!("      ⭐ /control/loadout/apply [{}] dry_run={} -> accepted={accepted}: {detail}",
          req.request_id, req.dry_run);
    axum::Json(ApplyResponse {
        contract_version: 1,
        request_id: req.request_id,
        accepted,
        detail,
        issues,
    })
}

/// Encode each equipped item to a 184-byte element file. Returns how many were written.
///
/// ⚠ **Goes through `d1-oracle`, deliberately.** The element is RSAT bit-packed; hand-rolling it
/// here is exactly the §26.67 under-length trap, where a short buffer "decodes fine" and reads
/// zeros past the end. The oracle runs the client's own encoder and refuses on any round-trip
/// mismatch, so a staged file is one the client would accept.
fn stage_loadout(loadout: &d1_loadout::Loadout, dir: &str) -> anyhow::Result<usize> {
    std::fs::create_dir_all(dir)?;
    for old in std::fs::read_dir(dir)?.flatten() {
        if old.path().extension().map(|e| e == "bin").unwrap_or(false) {
            std::fs::remove_file(old.path())?;
        }
    }
    let oracle = std::env::var("D1_ORACLE")
        .unwrap_or_else(|_| "d1-server/target/release/d1-oracle".into());
    let base = env_soid("D1_ITEM_SOID_BASE", 0xD100_0000_0000_0301);
    let mut n = 0usize;
    for (i, e) in loadout.equipped.iter().enumerate() {
        // 184-byte C struct: [0] item-instance SOID @+0x000, then the two u32s at +0x018/+0x01C.
        // ⚠ WHICH of those carries the world tag is still `[U]` - see historicaldocs/design/light-and-gear.md §2.
        // We write hash and bucket into the two candidates; when the tag field is identified this
        // is the one place to change.
        let mut o = vec![0u8; 184];
        o[0x000..0x008].copy_from_slice(&(base + i as u64).to_le_bytes());
        o[0x018..0x01c].copy_from_slice(&e.item.hash.to_le_bytes());
        o[0x01c..0x020].copy_from_slice(&e.item.bucket_hash.to_le_bytes());
        o[0x020..0x022].copy_from_slice(&1u16.to_le_bytes());
        o[0x024..0x026].copy_from_slice(&1u16.to_le_bytes());
        o[0x010] = 1;
        o[0x086] = 1;
        let hex: String = o.iter().map(|b| format!("{b:02x}")).collect();
        let out = std::process::Command::new(&oracle)
            .args(["object", "--tag", "0x80801732", "--obj", &hex])
            .output()?;
        let text = String::from_utf8_lossy(&out.stdout);
        if !text.contains("object IDENTICAL") {
            anyhow::bail!("oracle refused {}: {}", e.slot.display_name(),
                          String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
        let payload = text.lines().filter(|l| !l.trim().is_empty()).nth(2)
            .ok_or_else(|| anyhow::anyhow!("oracle produced no payload"))?.trim();
        let bytes: Vec<u8> = (0..payload.len()/2)
            .map(|k| u8::from_str_radix(&payload[k*2..k*2+2], 16).unwrap_or(0)).collect();
        std::fs::write(format!("{dir}/{i:02}-{}.bin", e.slot.display_name()), &bytes)?;
        n += 1;
    }

    // ⭐⭐ 2026-08-10 - THE STAGED ELEMENT ABOVE IS THE *LEGACY* ONE, ON PURPOSE.
    //
    // The three fields it writes are all wrong in the client's own terms: `+0x18` is the item
    // LEVEL (not a hash), `+0x1C` is the equip MINIMUM STAT (not a bucketTypeHash - a nine-digit
    // one is the PADLOCK), and `+0x24` is the CONTENT INDEX that resolves the definition (not the
    // literal 1, which is 'Bronze Tier': no stats, no geometry, not equippable).
    //
    // Rather than duplicate `d1_oracle::inventory` here - this crate deliberately does **not**
    // depend on the oracle, because that would pull `unicorn-engine`'s GPL-2.0 across the crate
    // boundary - we hand the staged directory straight to the tool that owns those fields, whose
    // `to_object` REFUSES a hash in a small-number field and whose nine controls gate the answer.
    // `inventory rebuild` takes exactly this legacy shape as its input.
    //
    // ⛔ A failure here is loud on purpose. Silently serving un-equippable items is what cost the
    // operator a night of staring at padlocks.
    let out = std::process::Command::new(&oracle)
        .args(["inventory", "rebuild", "--dir", dir])
        .output()?;
    if !out.status.success() {
        anyhow::bail!(
            "d1-oracle inventory rebuild failed on {dir}: {}",
            String::from_utf8_lossy(&out.stderr).trim().to_string()
        );
    }
    warn!("      ⭐ rebuilt {n} element(s) through d1-oracle - +0x24 content index resolved, \
           +0x18 level, +0x1C equip minimum 0, 0xFFFF sentinels");
    Ok(n)
}

/// Every `D1_*` knob this process reads, with its documented default and one line of meaning.
///
/// ⚠ **Why this exists.** A past session's default was *not* the last-known-good value, so an
/// unrelated experiment silently changed behaviour and read as a regression in the patch under
/// test. A run log must carry its own provenance - the same reason `run-boot.sh` prints its
/// environment block at startup. If a knob is added below and not added here, that is the bug.
const KNOBS: &[(&str, &str, &str)] = &[
    // ── ⭐ the correlation table (new, 2026-08-08) ────────────────────────────────────────────
    ("D1_BAP_DUMP_DIR", "<unset>", "write every decrypted inbound BAP body, whole, to this directory"),
    ("D1_BAP_FIFO", "1", "⭐ correlation table ON. 0 = legacy immediate-write (reproduces the Weasel)"),
    ("D1_BAP_DEADLINE_MS", "2000", "⭐ R2: synthesise a 500 for a head older than this. [U] guess"),
    ("D1_BAP_TICK_MS", "250", "⭐ how often to check the deadline with no inbound frame"),
    // ── ⭐ the activity-host lane (new, 2026-08-08) ───────────────────────────────────────────
    ("D1_AH_MANAGER", "1", "⭐ answer BAP msg 6 with msg 7 (hands the client an activity-host id)"),
    ("D1_AH_HOST_TOKEN", "0xD100000000000A01", "⭐ msg 7 host_token. MUST be non-zero or 0x594320 issues nothing"),
    ("D1_AH_HOST_TEXT", "<D1_AH_IP>:<D1_AH_PORT>", "⭐ msg 7 host_text, 128B NUL-padded. Contents are [U]"),
    ("D1_AH_PROXY", "1", "⭐ answer BAP msg 16 with msg 17 (resolves the AH id to an address)"),
    ("D1_AH_IP", "<D1_BAP_IP>", "⭐ address msg 17 resolves to"),
    ("D1_AH_PORT", "<D1_BAP_PORT>", "⭐ port msg 17 resolves to; defaults to the primary BAP port"),
    ("D1_AH_ENDIAN", "be", "⛔ msg 17 byte order. `le` REPRODUCES A FAILED RUN - see the note at the send site"),
    ("D1_MM_CONFIG", "1", "⭐ answer BAP msg 42 with msg 43 (matchmaking configuration, f4 present)"),
    ("D1_AH_PUSH_100", "0", "⭐ send the msg 100 push. ✅ UNBLOCKED - header class 6 is 32 bytes"),
    ("D1_AH_PUSH_102", "0", "⭐ attempt the msg 102 push. Refused: 102 is not on the accept-set"),
    ("D1_AH_RECIPIENT", "<D1_ACCOUNT_SOID>", "⭐ msg 100 recipient_id. A wrong value is a SILENT drop"),
    ("D1_AH_UDT", "10", "⭐ msg 100 user_defined_type"),
    ("D1_AH_PAYLOAD", "<empty>", "⭐ msg 100 payload, hex"),
    ("D1_AH_LOG_TEXT", "d1-responder", "⭐ msg 102 text"),
    // ── signon / bootstrap ───────────────────────────────────────────────────────────────────
    ("D1_SIGNON_HEX", "<unset>", "raw /SignOn response body override"),
    ("D1_SIGNON_TYPESWEEP", "<unset>", "sweep response_type across retries"),
    ("D1_SIGNON_SILO", "12345", "signon f5 silo_id"),
    ("D1_SIGNON_ENV", "live", "signon f13 environment"),
    ("D1_BAP_IP", "10.77.0.2", "signon f2.6 - where the client dials BAP (LITTLE-endian packed)"),
    ("D1_BAP_PORT", "3075", "signon f2.7 - NOT 3074, the guest binds that itself"),
    ("D1_CLIENT_IP", "10.77.0.2", "signon f14 client_external_ip (BIG-endian packed)"),
    ("D1_MANIFEST_URL", "http://cdn.deadorbit.net/manifest.bin", "signon f4.3 engine-config key 100"),
    ("D1_MANIFEST_FILE", "<unset>", "⚠ UNSET SERVES A 2-BYTE MANIFEST - 0 packages, dies in bap_signin"),
    ("D1_MANIFEST_HEX", "<unset>", "manifest body override"),
    ("D1_UPLOAD_DIR", "/tmp/d1-uploads", "where guest crash uploads land"),
    // ── BAP conversation knobs ───────────────────────────────────────────────────────────────
    ("D1_BAP_REPLY_HEX", "<unset>", "raw unsecured BAP reply (probe escape hatch)"),
    ("D1_BAP_SHELLO", "<unset>", "answer channel-startup with a secure hello instead"),
    ("D1_BAP_SEQ", "<hello seq + 1>", "secure-hello response sequence"),
    ("D1_SUB_ACK", "1", "answer 0x000C. echo = ACK + echoed body. ⚠ 0 is a 500 at the deadline under FIFO, not silence"),
    ("D1_ANSWER_FAMILIES", "<unset>", "families to answer with a body on their subscription, e.g. 0,2"),
    ("D1_ROSTER_DIR", "<unset>", "⭐ family-2 roster fixtures from `d1-oracle roster build` (f<fam>t<type>-<key>.bin). Default OFF; FAMILY 2 ONLY - a family-0 object substitutes the rich fill and BLANKS the plate"),
    // ⚠ CORRECTED 2026-08-10. This row used to read "a repeated baseline is REJECTED", which is
    // wrong in a way that matters: the historical rejections were families 0/2 and a family-4 push
    // at replication state **1**. At state 3 a repeated baseline is *ACCEPTED* and takes
    // `0x56B2D0`, the purge-to-keep-list arm. Rejected would have been the safe failure.
    ("D1_RESEND_FAMILIES", "<unset>", "⛔ re-send family answers as BASELINES - at state 3 that PURGES to the keep-list (0x56B2D0). Use D1_FAMILY4_INVALIDATE for family 4"),
    ("D1_ACK_PROFILE_SETUP", "1", "answer 0x0015 (UPO) and 0x0020 (user message). ⚠ 0 becomes a 500 under FIFO"),
    ("D1_CFG_COOKIE", "0", "0x0013 client_config cookie"),
    ("D1_CFG_SOCIAL_MM_ID", "0", "0x0013 social_matchmaking_id"),
    ("D1_AIDT", "<unset>", "online id for platform_to_investment_response"),
    // ── investment messages ──────────────────────────────────────────────────────────────────
    ("D1_INV_STATUS", "200", "⚠ ignored when D1_BAP_FIFO=1 - drain writes 200"),
    ("D1_INV_PAD", "0", "zero bytes of argument bitstream to append"),
    ("D1_INV_SILENT", "<unset>", "⚠ violates R2 by construction; under FIFO these become a 500, not silence"),
    ("D1_503_SOID", "<unset>", "answer 503 with the account SOID per its own schema"),
    ("D1_503_RAW", "<unset>", "exact 503 argument bitstream, hex"),
    ("D1_503_FIELDS", "<unset>", "503 field probe, value:bits pairs"),
    ("D1_205_RAW", "<unset>", "⭐ 205 body override - MUST agree with D1_503_RAW on the shared 0x80801941"),
    ("D1_ACTIVITY_FOLLOW_FILE", "<unset>", "write each kind-4 matchmaking request's activity hash here (read by the Activity Host)"),
    ("D1_UNLOCK_FLAGS", "<unset>", "⭐ unlock-flag INDICES (<= 50) served on BOTH 503 and 205, decoded back before use"),
    ("D1_504_RAW", "<unset>", "504 login_character body override"),
    ("D1_701_RAW", "<unset>", "701 body override"),
    ("D1_702_RAW", "<unset>", "702 body override"),
    ("D1_206_BODY", "<unset>", "206 body OVERRIDE - ⚠ REFUSED unless its span is for the family the client asked for"),
    ("D1_206_SPAN", "1", "⭐ answer fetch_family with the requested family's span. =0 restores the pre-2026-08-10 no-span 0x10"),
    ("D1_206_TAIL", "<unset>", "206 body 0x10 + tail - CANNOT express a span"),
    // ── objects ──────────────────────────────────────────────────────────────────────────────
    ("D1_ACCOUNT_SOID", "0xD100000000000001", "account SOID; must be stable across runs"),
    ("D1_CHARACTER_SOID", "0xD100000000000101", "character SOID"),
    ("D1_PROFILE_SOID", "0xD100000000000201", "profile SOID"),
    ("D1_PROFILE_FILE", "<unset>", "oracle-encoded profile object"),
    ("D1_CHARACTER_FILE", "<unset>", "oracle-encoded character object"),
    ("D1_ACCOUNT_RSAT", "<unset>", "account payload override - profile NOT sent"),
    ("D1_ORBIT_DESTINATION", "2468353467", "last_orbited_destination; 0 = unset"),
    ("D1_PUSH_CHARACTER", "1", "push the character family at the trigger"),
    ("D1_PUSH_AT", "relay", "push trigger: relay | investment | echo"),
    ("D1_PUSH_FAMILY", "3", "families to push, in order (4 then 3 is the subscribe order)"),
    ("D1_QUEUEZ_PROBE", "<unset>", "empty = family header with 0 objects"),
    ("D1_FAMILY4_INVALIDATE", "ping", "⭐⭐ re-deliver family 4 as a DELTA on the first echo after the baseline, so 0x574440(4) clears the availability cache and 0x25FF40 rebuilds the plane. ping = 0 objects (purge-free) | objects = verbatim re-stamp | 0 = off"),
    ("D1_FAMILY4_VERSION", "<baseline version + 1>", "the delta's family version. 0x56AF60 accepts only prev+1 and NAMES the value it expected when it refuses"),
    ("D1_FAMILY4_SERIAL", "0", "⚠ objects mode only: account field [2] (+0x60) = N, which makes 0x56B5F0's memcmp differ so the object is announced as an UPDATE. NOT needed for the invalidation; +0x60's inertness is [U]"),
    ("D1_EMPTY_ROSTER", "<unset>", "⭐⭐ roster with 0 characters AND an account-only family 4 (no profile object, +0x68 ABSENT) - client offers CREATION and STAYS in it. ⭐ D1_CHARACTER_SOID=0 does the same"),
    ("D1_NO_ROSTER", "<unset>", "family 3 = character only"),
    ("D1_CHARACTER_LIFECYCLE", "1", "⭐ answer investment 501/502/505/506 (create/delete/signout/customize). DEFAULT ON since 2026-08-10 - measured: the proven route never SENDS them. =0 opts out"),
    ("D1_CREATED_CHARACTER_SOID", "0xD100000000000101", "SOID minted in the 501 create_character reply, then served as family 3/1 + 4/1"),
    ("D1_CHARACTER_TRANSCRIPT_DIR", "$RUN_OUT", "where create-character-<t>.txt/.bin land"),
    ("D1_UNLOCKS", "<unset>", "family 4/type 4: key | <path>. ⚠ default OFF, orbit was reached without it"),
    ("D1_FAMILY42", "<unset>", "family 4/type 2 key-only object"),
    ("D1_INVENTORY", "capture-rig/inventory", "family 4/type 3 item elements (dir or file)"),
    ("D1_ITEM_SOID_BASE", "0xD100000000000301", "first item instance SOID"),
    ("D1_PROGRESSIONS", "<unset>", "family 2 elements - where LEVEL and LIGHT live"),
    ("D1_PROGRESSION_ROOT", "<unset>", "family 2 root payload"),
    ("D1_PROGRESSION_KEY_BASE", "0xD100000000000401", "fallback progression key"),
    ("D1_ORACLE", "d1-server/target/release/d1-oracle", "native encoder used to stage loadouts"),
];

/// Print [`KNOBS`] with each knob's effective value.
fn print_knobs() {
    info!("=== d1-responder environment (⭐ = new for the rung 28-30 activity-host lane) ===");
    for (name, default, meaning) in KNOBS {
        match std::env::var(name) {
            Ok(v) => {
                let v: String = v.chars().take(48).collect();
                info!("  {name:<24} = {v:<40}  SET       - {meaning}");
            }
            Err(_) => info!("  {name:<24} = {default:<40}  [default] - {meaning}"),
        }
    }
    // The per-id escape hatch is not a fixed name, so it cannot be tabulated; report the ones set.
    let per_id: Vec<String> = std::env::vars()
        .filter(|(k, _)| k.starts_with("D1_INV_BODY_"))
        .map(|(k, v)| format!("{k}={}B", v.len() / 2))
        .collect();
    if !per_id.is_empty() {
        info!("  D1_INV_BODY_<id> overrides: {}", per_id.join(" "));
    }
    check_205_503_agree();
    info!("=== end environment ===");
}

/// ⭐⭐ **The guard for "same struct, two answers".**
///
/// Investment messages **205** (`0x80801827`, 952 B) and **503** (`0x80801825`, 968 B) embed the
/// *same* nested 944-byte `0x80801941` - 503 at C offset `+0x018`, 205 at `+0x008`. On the wire that
/// struct's encoding therefore starts at byte **21** of 503's argument bitstream and byte **1** of
/// 205's body, and **the two must be byte-identical from there on.**
///
/// This has already cost one run: a 205 whose field [0] was zero declared "family 5 rooted at 0" and
/// the client died with `error.investment_server_object_family_record_not_found` - six minutes after
/// boot, in a state that reads like a completely different bug.
///
/// The compile-time test `d1_bap::tests::family5_root_tail_matches_503` guards the *constants*. It
/// cannot see `D1_503_RAW` / `D1_205_RAW`, which is precisely how a disagreement gets shipped. So
/// this checks the **effective** bodies, at startup, and says so loudly.
/// (`CLAUDE.md`: *a lesson without a guard is a lesson you pay for twice.*)
fn shared_0x80801941_agrees(b503: &[u8], b205: &[u8]) -> Option<bool> {
    if b503.len() < 21 || b205.is_empty() {
        return None;
    }
    Some(b503[21..] == b205[1..])
}

/// ⭐⭐⭐ **The 503 argument bitstream this responder actually sends.**
///
/// The 503 arm and [`check_205_503_agree`] both call this, and that is the entire point. Before
/// 2026-08-10 they were different objects: the guard compared `INVESTMENT_503_PINNED_RAW` against
/// `investment_205_response_body()` and printed `✅ 205/503 agree`, while the arm emitted the
/// **9-byte** hand-packed `investment_503_account_soid_body()`, which contains no `0x80801941` at
/// all. Every control in the tree went green on a run that sent `count = 0`.
/// ⭐ *A server-side receipt proves you BUILT it, never that the client KEPT it* - and that one did
/// not even prove we built it. One function, one answer, or the guard is decoration.
fn served_503_body() -> Vec<u8> {
    let raw = hex_env("D1_503_RAW");
    if !raw.is_empty() {
        warn!("      503 RAW BODY override ({}B) - the ten-flag default is NOT in play", raw.len());
        return raw;
    }
    if let Some((b503, _)) = unlock_flag_override() {
        return splice_503_soid(b503, env_soid("D1_ACCOUNT_SOID", 0xD100_0000_0000_0001));
    }
    // `D1_503_FIELDS="0:64,0xD1..:64"` - comma-separated `value:bits`, each emitted as
    // `present=1 || value`. Lets us bisect which fields the action graph needs without
    // hand-decoding the descriptor. Still a probe: it carries no `0x80801941`.
    if let Some(spec) = std::env::var("D1_503_FIELDS").ok().filter(|v| !v.is_empty()) {
        let fields: Vec<(u64, u32)> = spec
            .split(',')
            .filter_map(|f| {
                let (v, w) = f.split_once(':')?;
                let v = v.trim();
                let val = v
                    .strip_prefix("0x")
                    .map(|h| u64::from_str_radix(h, 16))
                    .unwrap_or_else(|| v.parse())
                    .ok()?;
                Some((val, w.trim().parse().ok()?))
            })
            .collect();
        warn!("      503 FIELD PROBE: {fields:?} - carries NO unlock flags");
        return d1_bap::investment_503_fields(&fields);
    }
    // ⭐ THE DEFAULT: ten unlock flags at value 2, with the run's own account SOID spliced in.
    d1_bap::investment_503_unlocks_raw_for(env_soid("D1_ACCOUNT_SOID", 0xD100_0000_0000_0001))
}

/// The 205 response body this responder actually sends. Same contract as [`served_503_body`].
fn served_205_body() -> Vec<u8> {
    let raw = hex_env("D1_205_RAW");
    if !raw.is_empty() {
        warn!("      205 RAW BODY override ({}B) - the ten-flag default is NOT in play", raw.len());
        return raw;
    }
    if let Some((_, b205)) = unlock_flag_override() {
        return b205;
    }
    d1_bap::investment_205_unlocks_body().to_vec()
}

/// ⭐ `D1_UNLOCK_FLAGS=i,j,...` - serve THIS unlock-flag set (client master-table INDICES, value 2)
/// on both 503 and 205, built by `d1_bap::unlock_flag_bodies` from the pinned count-0 pair.
///
/// This is how a kind-0 flag (no dense home - e.g. 56578's Director gates 2539/2549/3558/3587) is
/// served at all: the sparse pair list is its only channel. Indices are build-specific; resolve
/// them with `d1-flagtable.py`, never from a manifest `value` (the §2.2 drift).
///
/// ⛔ More than 50 is refused (container `0x80804717` is 50 x 4 B), and the bodies are decoded back
/// before use: a body that reads back as anything other than exactly this list is not served.
fn unlock_flag_override() -> Option<(Vec<u8>, Vec<u8>)> {
    let spec = std::env::var("D1_UNLOCK_FLAGS").ok().filter(|v| !v.is_empty())?;
    let indices: Vec<u16> = match spec.split(',').map(|s| s.trim().parse::<u16>()).collect() {
        Ok(v) => v,
        Err(e) => {
            warn!("      ⛔ D1_UNLOCK_FLAGS={spec}: {e} - serving the default flags instead");
            return None;
        }
    };
    let Some((b503, b205)) = d1_bap::unlock_flag_bodies(&indices) else {
        warn!("      ⛔ D1_UNLOCK_FLAGS has {} flags; the container holds 50 - serving the default flags", indices.len());
        return None;
    };
    let want: Vec<(u16, u8)> = indices.iter().map(|&i| (i, d1_bap::UNLOCK_FLAG_VALUE)).collect();
    if d1_bap::unlock_flags_in_shared_tail(&b205[1..]).as_ref() != Some(&want)
        || d1_bap::unlock_flags_in_shared_tail(&b503[21..]).as_ref() != Some(&want)
    {
        warn!("      ⛔ D1_UNLOCK_FLAGS bodies do not decode back to the requested list - serving the default flags");
        return None;
    }
    warn!("      ⭐ D1_UNLOCK_FLAGS: serving {} unlock flags on 503+205: {indices:?}", indices.len());
    Some((b503, b205))
}

/// The 503 body with `account_soid` at [`d1_bap::INV_503_ACCOUNT_SOID_BIT_OFFSET`], MSB-first.
fn splice_503_soid(body: Vec<u8>, account_soid: u64) -> Vec<u8> {
    let mut out = body;
    for i in 0..64 {
        let bit = d1_bap::INV_503_ACCOUNT_SOID_BIT_OFFSET + i;
        let (byte, mask) = (bit / 8, 0x80u8 >> (bit % 8));
        if account_soid >> (63 - i) & 1 == 1 { out[byte] |= mask } else { out[byte] &= !mask }
    }
    out
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
//  ⭐ THE OFFLINE CONTROLS - `d1-responder controls`, exit 2 on failure.
//
//  These assert the things a WRONG EDIT would break SILENTLY, i.e. the ones whose failure mode is a
//  healthy-looking log. Each row is written so that its own inversion is visible: where a control
//  says "X must not be present", the RED side is asserted in the same function against the builder
//  that DOES produce it, so the check can be seen to discriminate rather than merely to pass.
// ═══════════════════════════════════════════════════════════════════════════════════════════════

/// Run the offline controls. Returns the number of FAILURES.
fn run_controls() -> usize {
    let acct = 0xD100_0000_0000_0001u64;
    let mut fails = 0usize;
    let mut check = |name: &str, ok: bool, detail: String| {
        println!("{}  {name}{}{detail}", if ok { "PASS" } else { "FAIL" },
                 if detail.is_empty() { "" } else { "  " });
        if !ok {
            fails += 1;
        }
    };

    // ── P1a. "The family-4 body served WHILE THE ROSTER IS EMPTY carries no family-4/type-1
    //    object." The empty-roster branch is what `serves_no_characters` selects, so drive it the
    //    way the wire does - through `build_family_body`, not through the d1-bap builder directly.
    std::env::set_var("D1_EMPTY_ROSTER", "1");
    let f4 = build_family_body(d1_bap::QUEUEZ_FAMILY_ACCOUNT, acct, 0, "");
    let count = f4.get(21..25).map(|b| u32::from_be_bytes(b.try_into().unwrap()));
    let has_profile = f4.windows(4).any(|w| w == d1_bap::profile_instance_type().to_be_bytes());
    check("P1a empty-roster family 4: object_count == 1", count == Some(1),
          format!("[count {count:?}]"));
    check("P1a empty-roster family 4: NO family-4/type-1 profile object", !has_profile,
          format!("[{} bytes, 0x{:08x} {}]", f4.len(), d1_bap::profile_instance_type(),
                  if has_profile { "PRESENT" } else { "absent" }));
    // The account payload's field 3 must be ABSENT, not zero. 93 bits -> 12 bytes; the
    // present-with-value form is 157 bits -> 20 bytes and is indistinguishable at `+0x68` once
    // decoded, which is precisely why the length is the thing to assert here and the DECODE is
    // asserted out of process by `d1-oracle object --tag 0x808018c7 --decode`.
    let paylen = f4.get(37..41).map(|b| u32::from_be_bytes(b.try_into().unwrap()));
    check("P1a account payload is the field-3-ABSENT form (12 B, 93 bits)", paylen == Some(12),
          format!("[payload {paylen:?} B; the present-with-zero form would be 20]"));
    // ⭐ THE INVERSION, run rather than promised: the builder we replaced is still reachable and
    // still emits the profile, so this control can be SEEN to discriminate.
    let old = d1_bap::queuez_family4_baseline_body_with_unlocks(
        acct, &[], 0xD100_0000_0000_0201, None, None, &[]);
    let old_count = old.get(21..25).map(|b| u32::from_be_bytes(b.try_into().unwrap()));
    let old_has_profile = old.windows(4).any(|w| w == d1_bap::profile_instance_type().to_be_bytes());
    check("P1a INVERTED: the pre-2026-08-10 builder DOES emit the profile (control discriminates)",
          old_count == Some(2) && old_has_profile, format!("[count {old_count:?}]"));

    // ── P1c. "The span we put in a 206 reply is a delivery for the family the REQUEST asked for."
    //    Driven with the byte-exact request the client has sent 195 times.
    std::env::remove_var("D1_206_BODY");
    std::env::remove_var("D1_206_SPAN");
    const OBSERVED_206: [u8; 9] = [0x4d, 0x10, 0, 0, 0, 0, 0, 0, 0x10];
    let req = d1_bap::fetch_family_request(&OBSERVED_206);
    check("P1c the observed 206 request decodes as family 3 (leaf biased +1)",
          req.map(|r| (r.family, r.key)) == Some((3, acct)), format!("[{req:?}]"));
    let body = message_206_body(&OBSERVED_206);
    let served_family = d1_bap::fetch_family_response_span(&body)
        .as_deref()
        .and_then(d1_bap::queuez_span_family);
    check("P1c the served 206 span is a FAMILY-3 delivery", served_family == Some(3),
          format!("[family {served_family:?}, body {} B]", body.len()));

    // ⭐ THE INVERSION: hand the arm the pinned family-4 constant against the same family-3
    // request. It must be REFUSED - accepting it is the purge that deletes the profile.
    std::env::set_var("D1_206_BODY",
        "1801900000000800000026880000000000000800000000080000000b\
         2573adfe88000000000000080000004f440000000000000400".replace(' ', ""));
    let body_ov = message_206_body(&OBSERVED_206);
    let ov_family = d1_bap::fetch_family_response_span(&body_ov)
        .as_deref()
        .and_then(d1_bap::queuez_span_family);
    check("P1c INVERTED: the pinned family-4 D1_206_BODY is REFUSED for a family-3 request",
          ov_family == Some(3),
          format!("[family {ov_family:?} - Some(4) here would be the PURGE]"));
    std::env::remove_var("D1_206_BODY");

    // The bisect lever must still reproduce the historical reply exactly.
    std::env::set_var("D1_206_SPAN", "0");
    let nospan = message_206_body(&OBSERVED_206);
    check("P1c D1_206_SPAN=0 still reproduces the code's no-span body 0x10 (bisect lever)",
          nospan == vec![0x10],
          format!("[{}]", hexdump(&nospan, 8)));
    std::env::remove_var("D1_206_SPAN");
    std::env::remove_var("D1_EMPTY_ROSTER");

    // ── P1d. 105 is answered with the same `0x80801796` composite as 104/504/701/702.
    check("P1d investment 105 body == RESULT_OK_BODY (1800000000)",
          d1_bap::character_lifecycle::RESULT_OK_BODY.to_vec() == vec![0x18, 0, 0, 0, 0],
          String::new());

    // ── P1b. The lifecycle answers must be ON by default, or the creator the P1a builder opens has
    //    no exit. `=0` must still opt out, or there is no bisect.
    std::env::remove_var("D1_CHARACTER_LIFECYCLE");
    let on = character_lifecycle_enabled();
    std::env::set_var("D1_CHARACTER_LIFECYCLE", "0");
    let off = character_lifecycle_enabled();
    std::env::remove_var("D1_CHARACTER_LIFECYCLE");
    check("P1b 501/502/505/506 answered by DEFAULT, and =0 still opts out", on && !off,
          format!("[default {on}, =0 {off}]"));

    // ── The empty-string trap. `export KNOB="${KNOB:-}"` sets a knob to "", and a boolean read of
    //    `v != "0"` calls that TRUE. For D1_EMPTY_ROSTER that silently turns every ordinary boot
    //    into a character-creation run. Assert unset == empty == off for both boolean knobs.
    std::env::remove_var("D1_EMPTY_ROSTER");
    let unset = serves_no_characters(0xD100_0000_0000_0101);
    std::env::set_var("D1_EMPTY_ROSTER", "");
    let empty = serves_no_characters(0xD100_0000_0000_0101);
    std::env::set_var("D1_EMPTY_ROSTER", "1");
    let one = serves_no_characters(0xD100_0000_0000_0101);
    std::env::remove_var("D1_EMPTY_ROSTER");
    check("D1_EMPTY_ROSTER: unset == empty == OFF, and =1 is ON",
          !unset && !empty && one, format!("[unset {unset}, \"\" {empty}, \"1\" {one}]"));
    std::env::set_var("D1_CHARACTER_LIFECYCLE", "");
    let lc_empty = character_lifecycle_enabled();
    std::env::remove_var("D1_CHARACTER_LIFECYCLE");
    check("D1_CHARACTER_LIFECYCLE: empty == unset == ON", lc_empty, String::new());

    // ── P0 (do not regress). The ten unlock flags must be what 503 and 205 actually carry.
    let b205 = served_205_body();
    let flags = b205.len().checked_sub(1)
        .and_then(|_| d1_bap::unlock_flags_in_shared_tail(&b205[1..]))
        .unwrap_or_default();
    let missing: Vec<u16> = d1_bap::UNLOCK_FLAG_INDICES.iter().copied()
        .filter(|i| !flags.iter().any(|&(f, _)| f == *i)).collect();
    check("P0 served 205 carries the ten known-good unlock flags at value 2",
          missing.is_empty() && flags.iter().all(|&(_, v)| v == 2),
          format!("[{} flags, missing {missing:?}]", flags.len()));
    let b503 = served_503_body();
    check("P0 205/503 agree on the shared 0x80801941",
          shared_0x80801941_agrees(&b503, &b205) == Some(true), String::new());

    // ── ⭐⭐ P2. THE CONTROL AXIS, stated before the assertions:
    //
    //   "The second family-4 delivery is a DELTA the client accepts at replication state 3 - flags
    //    bit 0 CLEAR and version == the baseline's version + 1 - and in `objects` mode it carries
    //    the baseline's objects BYTE-FOR-BYTE, so nothing can be purged and no field dropped."
    //
    // ⛔ The brief's own axis - *"classified UPDATE, and the changed byte is not load-bearing"* -
    // is NOT the axis this ships on, and its proposed inversion is a broken control. `0x565140`
    // calls `0x56B5E0` on EVERY accepted family update (`0x565569`), `0x56B5E0` is a
    // four-instruction unconditional tail-jump, and `0x56D1A0` reaches `0x574440(4)` without ever
    // consulting the per-object classifier. So a byte-identical re-send still invalidates the
    // cache: *"byte-identical -> classified NEITHER -> red"* would go **GREEN**, proving nothing
    // and buying false confidence that the changed byte mattered. The classifier axis is kept
    // below as P2b, on the knob that actually owns it.
    std::env::remove_var("D1_FAMILY4_INVALIDATE");
    std::env::remove_var("D1_FAMILY4_SERIAL");
    let chr = 0xD100_0000_0000_0101u64;
    let base = build_family_body(d1_bap::QUEUEZ_FAMILY_ACCOUNT, acct, chr, "");
    let base_ver = d1_bap::queuez_span_version(&base);
    let objs = build_family4_delta(&base, Family4Invalidate::Objects, base_ver.unwrap_or(0) + 1);
    let ok = objs.as_deref().map(|d| (
        d1_bap::queuez_span_flags(d).unwrap_or(0xFF) & 1 == 0,
        d1_bap::queuez_span_version(d) == base_ver.map(|v| v + 1),
        d.len() == base.len() && d[21..] == base[21..] && d[..16] == base[..16],
        d1_bap::queuez_span_objects(d) == d1_bap::queuez_span_objects(&base),
    ));
    check("P2a family-4 delta: DELTA flags, version +1, objects byte-identical to the baseline",
          ok == Some((true, true, true, true)),
          format!("[{ok:?}; baseline version {base_ver:?}, {} objects]",
                  d1_bap::queuez_span_object_count(&base).unwrap_or(0)));
    // ⭐ THE INVERSION, run rather than promised: the shape `D1_RESEND_FAMILIES` still emits - the
    // untouched baseline - has flags bit 0 SET, which is `0x56B2D0`'s purge arm. If the re-stamp
    // ever stopped clearing that bit this control would be the only thing that noticed.
    check("P2a INVERTED: the un-restamped baseline is flags&1 = PURGE arm (control discriminates)",
          d1_bap::queuez_span_flags(&base).unwrap_or(0) & 1 == 1,
          format!("[flags {:#04x}]", d1_bap::queuez_span_flags(&base).unwrap_or(0)));

    // ── P2b. The BRIEF'S axis, on the knob that owns it:
    //
    //   "With D1_FAMILY4_SERIAL != 0 the delta's ACCOUNT object differs from the baseline's, so
    //    0x56B5F0's memcmp(old, decoded, stride) != 0 and the object is announced as an UPDATE
    //    (0x56D3F0); with serial 0 it is byte-identical, so the classifier announces NOTHING."
    //
    // Both arms are asserted, so the inversion is shipped rather than described.
    let acct_payload = |span: &[u8]| -> Option<Vec<u8>> {
        let o = *d1_bap::queuez_span_objects(span)?.first()?;
        (o.instance_type == d1_bap::account_instance_type())
            .then(|| span[o.payload.0..o.payload.1].to_vec())
    };
    std::env::set_var("D1_FAMILY4_SERIAL", "7");
    let bumped = build_family4_delta(&base, Family4Invalidate::Objects, 1);
    std::env::set_var("D1_FAMILY4_SERIAL", "0");
    let plain = build_family4_delta(&base, Family4Invalidate::Objects, 1);
    std::env::remove_var("D1_FAMILY4_SERIAL");
    let (pb, pp, p0) = (acct_payload(&base), bumped.as_deref().and_then(acct_payload),
                        plain.as_deref().and_then(acct_payload));
    check("P2b D1_FAMILY4_SERIAL=7 changes the ACCOUNT object -> memcmp != 0 -> UPDATE (0x56D3F0)",
          pp.is_some() && pp != pb,
          format!("[baseline {:?}B, serial-7 {:?}B]", pb.as_ref().map(|v| v.len()),
                  pp.as_ref().map(|v| v.len())));
    check("P2b INVERTED: serial 0 is byte-identical -> memcmp == 0 -> NOTHING is announced",
          p0.is_some() && p0 == pb, String::new());
    // Only the account moved. Every other object must survive the splice untouched, or the
    // "object-complete by construction" claim is false for exactly the objects nobody checked.
    let tail_intact = match (d1_bap::queuez_span_objects(&base),
                             bumped.as_deref().and_then(d1_bap::queuez_span_objects)) {
        (Some(b), Some(d)) => b.len() == d.len() && b.iter().zip(&d).skip(1).all(|(x, y)| {
            x.instance_type == y.instance_type && x.key == y.key
                && base[x.payload.0..x.payload.1] == bumped.as_ref().unwrap()[y.payload.0..y.payload.1]
        }),
        _ => false,
    };
    check("P2b the splice moves ONLY object 0 - every other object is byte-identical", tail_intact,
          format!("[{} objects]", d1_bap::queuez_span_object_count(&base).unwrap_or(0)));

    // ── P2c. The mode knob. Empty must mean the DEFAULT, not "true" and not "off" - the
    //    `D1_EMPTY_ROSTER` trap, which was live for ten minutes on 2026-08-10.
    std::env::remove_var("D1_FAMILY4_INVALIDATE");
    let unset = family4_invalidate_mode();
    std::env::set_var("D1_FAMILY4_INVALIDATE", "");
    let empty = family4_invalidate_mode();
    std::env::set_var("D1_FAMILY4_INVALIDATE", "0");
    let off = family4_invalidate_mode();
    std::env::set_var("D1_FAMILY4_INVALIDATE", "objects");
    let objects = family4_invalidate_mode();
    std::env::remove_var("D1_FAMILY4_INVALIDATE");
    check("P2c D1_FAMILY4_INVALIDATE: unset == empty == ping, =0 off, =objects objects",
          unset == Family4Invalidate::Ping && empty == Family4Invalidate::Ping
              && off == Family4Invalidate::Off && objects == Family4Invalidate::Objects,
          format!("[unset {unset:?}, \"\" {empty:?}, 0 {off:?}, objects {objects:?}]"));
    // The default shape carries no object at all - the only delivery with no decode, no keep-list
    // and no classifier, hence nothing it can corrupt. `0x565140` still calls `0x56B5E0`.
    let ping = build_family4_delta(&base, Family4Invalidate::Ping, 1);
    check("P2c ping is a 25-byte header: family 4, 0 objects, DELTA, version 1",
          ping.as_deref().map(|p| (p.len(), d1_bap::queuez_span_family(p),
                                   d1_bap::queuez_span_object_count(p),
                                   d1_bap::queuez_span_flags(p).map(|f| f & 1),
                                   d1_bap::queuez_span_version(p)))
              == Some((d1_bap::QUEUEZ_SPAN_HEADER_LEN, Some(4), Some(0), Some(0), Some(1))),
          format!("[{}]", ping.as_deref().map(|p| hexdump(p, 25)).unwrap_or_default()));
    check("P2c =0 sends NOTHING (the pre-2026-08-10 behaviour stays reachable as a bisect arm)",
          build_family4_delta(&base, Family4Invalidate::Off, 1).is_none(), String::new());

    fails
}

fn check_205_503_agree() {
    let b503 = served_503_body();
    let b205 = served_205_body();
    let (b503, b205) = (&b503[..], &b205[..]);

    // ⭐⭐ WHAT THE OLD GUARD COULD NOT SAY: two byte-identical **empty** bodies agree perfectly.
    // Decode the `0x80804681` array back out and name what is in it, so "we shipped the flags" is
    // a measurement rather than an assumption about which constant the arm picked.
    match b205.len().checked_sub(1).and_then(|_| d1_bap::unlock_flags_in_shared_tail(&b205[1..])) {
        Some(flags) if flags.is_empty() => warn!(
            "  ⛔⛔ THE SERVED 205/503 CARRY **ZERO** UNLOCK FLAGS. Rung 33 will refuse with \
             `activity_failed_to_load_missing_dlc` (baboon) and there is no wire evidence to read. \
             Unset D1_503_RAW/D1_205_RAW to get the ten-flag default back."
        ),
        Some(flags) => {
            let bad: Vec<_> = flags.iter().filter(|&&(_, v)| v != 2).collect();
            info!(
                "  ✅ served unlock flags: {} at value 2 -> {:?}",
                flags.len(),
                flags.iter().map(|&(i, _)| i).collect::<Vec<_>>()
            );
            if !bad.is_empty() {
                warn!("  ⛔ {} flag(s) are NOT value 2 - 0x28CACD is an exact `cmp byte, 2`: {bad:?}",
                      bad.len());
            }
            let missing: Vec<u16> = d1_bap::UNLOCK_FLAG_INDICES
                .iter()
                .copied()
                .filter(|i| !flags.iter().any(|&(f, _)| f == *i))
                .collect();
            if !missing.is_empty() {
                warn!("  ⚠ missing from the known-good ten: {missing:?} - 2667/5987 are the two \
                       that rung 33's PERMISSIONS wall (urchin) needs; without them the run is \
                       back to the plane poke.");
            }
        }
        None => warn!("  ⛔ cannot read the served 205's unlock array - body too short"),
    }

    let Some(agree) = shared_0x80801941_agrees(b503, b205) else {
        warn!("  ⛔ 205/503 SHARED-STRUCT CHECK SKIPPED - a body is too short to contain 0x80801941 \
               (503 {}B, need >=21; 205 {}B, need >=1)", b503.len(), b205.len());
        return;
    };
    if agree {
        info!("  ✅ 205/503 agree on the shared 0x80801941 ({} bytes)", b205.len() - 1);
    } else {
        warn!("  ⛔⛔ 205 AND 503 DISAGREE ON THE SHARED 0x80801941 STRUCT - THIS RUN WILL LIE TO YOU.");
        warn!("      503[21..] = {}", b503[21..].iter().map(|x| format!("{x:02x}")).collect::<String>());
        warn!("      205[ 1..] = {}", b205[1..].iter().map(|x| format!("{x:02x}")).collect::<String>());
        warn!("      205 is answered AFTER 503, so 205 wins. Set BOTH D1_503_RAW and D1_205_RAW.");
        warn!("      The matching unlock pair is d1_bap::INVESTMENT_503_UNLOCKS_RAW / \
               INVESTMENT_205_UNLOCKS_BODY - see historicaldocs/design/rung33-unlock-gate.md §6.");
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_target(false).without_time().init();

    // ⚠ RUSTLS PROVIDER - MUST be explicit, and this is not cosmetic.
    //
    // Both `ring` and `aws-lc-rs` are in the workspace lock. rustls 0.23 refuses to guess when
    // both are reachable, and the refusal is a **panic inside the TLS accept task**, not a startup
    // error. The process keeps running and keeps serving plain HTTP, so the log looks healthy -
    // only :443 is dead. The client then fails PSN OAuth, `bootflow:account_signin` gives up, and
    // the screen says **"Destiny servers are not available"** with `error.bungie_server_connection`.
    // That reads exactly like a networking/containment problem and not like a build problem.
    //
    // How it appeared (2026-08-08, run 20260808-021524): building the WHOLE WORKSPACE
    // (`cargo build --release`) unifies features across members, and `d1-loadout`/`d1-forge` pull
    // rustls in through `ureq`. So `cargo build --release -p d1-responder` and
    // `cargo build --release` produced DIFFERENT binaries from identical source, and only the
    // second one was broken. Nothing failed to compile.
    //
    // Installing a provider by name makes the binary independent of feature unification.
    if rustls::crypto::ring::default_provider().install_default().is_err() {
        // Already installed by something else - fine, and not worth failing over.
        tracing::debug!("rustls CryptoProvider was already installed");
    }

    let args = Args::parse();

    // ⭐ `d1-responder controls` - offline, binds nothing, exits 2 on failure. It runs BEFORE
    // `print_knobs` so that a control run is not mistaken for a server that started and stopped.
    if args.mode.as_deref() == Some("controls") {
        let fails = run_controls();
        if fails == 0 {
            println!("\nALL CONTROLS PASS");
            return Ok(());
        }
        println!("\n⛔ {fails} CONTROL(S) FAILED - do not cite this build's output");
        std::process::exit(2);
    }
    if let Some(m) = args.mode.as_deref() {
        eprintln!("unknown mode {m:?} - the only mode is `controls`");
        std::process::exit(2);
    }

    print_knobs();

    // ⭐ GUARD: prove the create-character transcript can be written NOW, not at 501-time.
    //
    // `status/LESSONS.md` #8: *a lesson without a guard is a lesson you pay for twice.* The
    // transcript is the deliverable of the whole character-creation experiment; discovering at the
    // end of a 90-second boot that the directory was unwritable would cost the run and - worse -
    // would look like "the client never sent a 501", which is a false negative pointing at exactly
    // the wrong conclusion. So write and delete a probe file at startup and say which it is.
    if character_lifecycle_enabled() {
        let dir = transcript_dir();
        let probe = format!("{dir}/.d1-transcript-writable");
        match std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&probe, b"ok")) {
            Ok(()) => {
                let _ = std::fs::remove_file(&probe);
                warn!("⭐ D1_CHARACTER_LIFECYCLE is ON - 501/502/505/506 will be answered; \
                       create transcripts go to {dir} (verified writable)");
            }
            Err(e) => warn!("⛔ D1_CHARACTER_LIFECYCLE is ON but the transcript directory {dir} is \
                             NOT writable ({e}). A create-character request will still be answered \
                             and logged, but THE ARTIFACT WILL BE LOST. Set \
                             D1_CHARACTER_TRANSCRIPT_DIR."),
        }
    }
    let cfg = Cfg { npid: args.npid.clone(), account_id: args.account_id };

    let mut tasks = Vec::new();

    for p in args.http_ports.split(',').filter(|s| !s.is_empty()) {
        let port: u16 = p.trim().parse().with_context(|| format!("bad port {p}"))?;
        let addr: SocketAddr = format!("{}:{port}", args.bind).parse()?;
        let app = router(cfg.clone());
        tasks.push(tokio::spawn(async move {
            match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => {
                    info!("listening http://{addr}");
                    let _ = axum::serve(l, app).await;
                }
                Err(e) => warn!("bind http {addr} failed: {e}"),
            }
        }));
    }

    {
        let addr: SocketAddr = format!("{}:{}", args.bind, args.bap_port).parse()?;
        tasks.push(tokio::spawn(bap_observer(addr)));
        // Also listen on TCP: the client calls `transport_endpoint_async_connect`, and nothing yet
        // proves that is UDP. A refused TCP connect and a silent UDP port look identical from the
        // guest's side, so cover both rather than infer.
        tasks.push(tokio::spawn(async move {
            match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => {
                    info!("listening tcp://{addr} (BAP observation surface)");
                    loop {
                        match l.accept().await {
                            Ok((mut sock, peer)) => {
                                // ⭐ See `BAP_CONNECTIONS`: a second BAP connection is now EXPECTED
                                // (the activity host we advertise in messages 7/17 points here),
                                // and only connection 0 owns the QueueZ character/family delivery.
                                let conn_index = BAP_CONNECTIONS.fetch_add(1, Ordering::SeqCst);
                                warn!("=== BAP TCP CONNECT #{conn_index} from {peer} ===");
                                tokio::spawn(async move {
                                    use tokio::io::AsyncReadExt;
                                    // HOLD THE CONNECTION OPEN. The first version read once and
                                    // dropped the socket, and the client reported
                                    // `Failed to read data, endpoint closed` / `Failed to receive
                                    // data by offset [size 6][offset 0]` -- i.e. our close, not its
                                    // own protocol, ended the exchange. Observation must not
                                    // perturb the thing observed.
                                    //
                                    // Framing `[I]`, from the first captured hello (140 bytes):
                                    //   01 02 | 00 00 00 86 | <134-byte payload>
                                    //   ^type ^ big-endian length      0x86 == 140 - 6
                                    // which matches the client's own "[size 6][offset 0]" read.
                                    // Payload begins `00 1e 00 00 00 00 "DESTINY" 01 ...`.
                                    // A reply we can vary WITHOUT a rebuild, same pattern as
                                    // D1_SIGNON_HEX. The client is a precise oracle that only ever
                                    // says no, so the cheapest way to learn the contract is to send
                                    // a candidate and read which complaint changes. `{LEN}` is
                                    // substituted with the big-endian length of the remainder, so a
                                    // probe body can be edited without hand-computing the envelope.
                                    let reply_hex = std::env::var("D1_BAP_REPLY_HEX").unwrap_or_default();
                                    let reply: Vec<u8> = {
                                        let cleaned: String =
                                            reply_hex.chars().filter(|c| c.is_ascii_hexdigit()).collect();
                                        cleaned.as_bytes().chunks(2).filter(|c| c.len() == 2)
                                            .filter_map(|c| u8::from_str_radix(std::str::from_utf8(c).ok()?, 16).ok())
                                            .collect()
                                    };
                                    // ⭐⭐ THE CORRELATION TABLE - one per BAP TCP connection.
                                    //
                                    // `D1_BAP_FIFO=0` restores the pre-2026-08-08 behaviour: every
                                    // handler's reply written to the socket the instant it is
                                    // produced, in handler order. That is what killed both rung-28
                                    // runs, and it is kept reachable ONLY so the negative stays
                                    // reproducible - an A/B of the Weasel fix has to be one env var.
                                    let fifo = env_flag("D1_BAP_FIFO", true);
                                    // ⚠ `[U]` and knowingly a guess. The client's own tunable is a
                                    // u32 at absolute `[0x267B1B8] + 0x2684`; 0x267B1B8 is an eboot
                                    // BSS vaddr and gets the base, the pointer read OUT of it is
                                    // already absolute - `d1guest read --abs`. Until that read
                                    // happens, "answer promptly" has no number. 2 s errs in the
                                    // survivable direction: synthesising early costs a wrong body,
                                    // synthesising late costs `_connection_failure_timed_out`.
                                    let deadline = Duration::from_millis(
                                        env_num::<u64>("D1_BAP_DEADLINE_MS",
                                                       d1_bap::DEFAULT_DEADLINE.as_millis() as u64));
                                    // How often we wake with no inbound frame to check the deadline.
                                    // Without this, `drain` would only ever run when the client
                                    // spoke - so a stalled head would never be synthesised and R2
                                    // would be a comment rather than a mechanism.
                                    let tick = Duration::from_millis(env_num::<u64>("D1_BAP_TICK_MS", 250));
                                    let mut conv = d1_bap::BapConversation::new(deadline);
                                    warn!("    [conv] connection #{conn_index}: correlation table \
                                           {} (D1_BAP_FIFO), deadline {}ms, tick {}ms",
                                          if fifo { "ON" } else { "⛔ OFF - legacy immediate-write" },
                                          deadline.as_millis(), tick.as_millis());
                                    // Secured-channel state, per connection. Populated when we send
                                    // the secure-hello response, because that is the message that
                                    // hands the client the session key and channel seed.
                                    let mut chan: Option<(
                                        [u8; 16],
                                        d1_bap::ChannelNonces,
                                    )> = None;
                                    let mut buf = vec![0u8; 65536];
                                    // ⚠ TCP IS A BYTE STREAM, NOT A MESSAGE STREAM.
                                    //
                                    // This loop used to treat every `sock.read()` as exactly one BAP
                                    // frame. It computed the frame's declared length only to LOG it,
                                    // and then handed `&d[6..]` - the whole remainder of the read -
                                    // to the GCM open. That works only while every frame happens to
                                    // arrive in its own segment.
                                    //
                                    // Measured (run 20260807-142606-operator): 22 framed reads, one
                                    // mismatch - `msg#21: read 119B but the frame declares 80B`, i.e.
                                    // two frames coalesced. The first was decrypted with 39 bytes of
                                    // the second appended, the GCM tag failed, the nonce was (correctly)
                                    // not advanced, and the second frame was DROPPED. From then on
                                    // every frame decrypts against a stale counter, we answer nothing,
                                    // and the client hangs up: `error.lost_bap_connection`, the Weasel.
                                    // This is why long sessions ALWAYS ended in it - the more traffic,
                                    // the likelier the coalesce.
                                    //
                                    // So: accumulate, and split on the declared length. This also fixes
                                    // the mirror-image bug (a short read delivering a partial frame),
                                    // which had the same fatal consequence and had never been hit.
                                    let mut pending: Vec<u8> = Vec::with_capacity(65536);
                                    let mut total = 0usize;
                                    let mut msg = 0u32;
                                    // Have we handed the client its character yet? The character
                                    // list is delivered by a server-originated QueueZ push, not by
                                    // answering login_character (501) -
                                    // `historicaldocs/design/rsat-character-object.md`.
                                    let mut pushed_character = false;
                                    // Sequence for messages *we* originate; the client's own
                                    // sequences are echoed and are a separate space.
                                    let mut server_seq: u32 = 0;
                                    // Armed by a family-subscribe ACK for a class in
                                    // `D1_ANSWER_FAMILIES`, drained right after that ACK is written.
                                    // Declared out here only so the arming and the write can be in
                                    // the correct ORDER; it never holds a value across iterations.
                                    // ⭐ Now carries the SEQUENCE of the 0x000D ACK it must follow.
                                    // Under the correlation table a reply can be legally DEFERRED
                                    // (an older request is still unanswered at the head), so
                                    // "write the family answer straight after the ACK" is no longer
                                    // the same statement as "write it after this handler returns".
                                    let mut answer_family: Option<(u32, u64, u32)> = None;
                                    // Family answers armed but whose ACK has not left the socket yet.
                                    let mut deferred_family_answers: Vec<(u32, u64, u32)> = Vec::new();
                                    // The 100/102 push attempt is one-shot per connection.
                                    let mut ah_push_attempted = false;
                                    // ⭐⭐ P2 - THE AVAILABILITY-CACHE INVALIDATION.
                                    //
                                    // The family-4 span we actually PUT ON THE WIRE, kept verbatim
                                    // so the follow-up delta is a *re-stamp* of it rather than a
                                    // rebuild. `d1_bap::queuez_span_as_delta` explains why that
                                    // distinction is the whole design: a rebuild is how the rung-34
                                    // task-8 bug happened, and a memcpy cannot drop a field.
                                    let mut family4_span: Option<Vec<u8>> = None;
                                    let mut family4_invalidated = false;
                                    // The family-4 version the client last accepted from us (the
                                    // baseline's, then each delta's): a delta must carry it + 1.
                                    let mut family4_version: Option<u32> = None;
                                    // 601 claims answered OK whose item still has to be delivered
                                    // (see `D1_LOOT_GRANT`), by nonce id.
                                    let mut loot_grants_due: Vec<u64> = Vec::new();
                                    // ⭐ THE STATE-1 RACE. `historicaldocs/design/subscription-race-verdict.md`:
                                    // the client's manager tick sends 0x000C at 0x56A887 and
                                    // UNCONDITIONALLY writes descriptor state 2 at 0x56A8A7; a later
                                    // baseline (flags & 1) advances 2 -> 3 at 0x56B046. No server
                                    // message causes 1 -> 2 - it is a LOCAL store on the game thread.
                                    //
                                    // So a reply sent immediately after the ACK can land while the
                                    // family is still at state 1, and the client logs
                                    //     queuez_family_update_begin: ignoring family-update due to
                                    //     unhandled replication state ... replication state=1
                                    // and DROPS it. Answering instantly maximises that race, which is
                                    // exactly what this responder started doing tonight.
                                    //
                                    // Belt and braces: answer at once (cheap if it lands) AND re-send
                                    // once on the next inbound frame, by which point the client's tick
                                    // has certainly stored state 2.
                                    let mut resend_families: Vec<(u32, u64)> = Vec::new();
                                    loop {
                                        // ⭐ R2 NEEDS A CLOCK. `drain` can only synthesise a reply
                                        // for an aged head when something calls it, and before this
                                        // the only caller was "a frame arrived". A client that is
                                        // waiting on us sends nothing - which is precisely the state
                                        // where the deadline matters - so the timeout is what makes
                                        // R2 a mechanism instead of a comment.
                                        let read = tokio::time::timeout(tick, sock.read(&mut buf)).await;
                                        match read {
                                            Err(_elapsed) => {
                                                if fifo && !conv.is_idle() {
                                                    if let Some((key, nonces)) = chan.as_mut() {
                                                        let out = drain_replies(&mut sock, key, nonces, &mut conv).await;
                                                        if !out.is_empty() {
                                                            warn!("    [conv] idle drain released {} repl{} - \
                                                                   the client was waiting on us",
                                                                  out.len(),
                                                                  if out.len() == 1 { "y" } else { "ies" });
                                                        }
                                                    }
                                                }
                                                continue;
                                            }
                                            Ok(Ok(0)) => {
                                                warn!("    BAP TCP peer closed after {total}B");
                                                return;
                                            }
                                            Ok(Ok(nread)) => {
                                                total += nread;
                                                pending.extend_from_slice(&buf[..nread]);
                                                // Drain every COMPLETE frame this read made available.
                                                // A frame is `u8 mode || u8 type || u32be len || len bytes`.
                                                while pending.len() >= 6 {
                                                    let want = 6 + u32::from_be_bytes(
                                                        [pending[2], pending[3], pending[4], pending[5]]) as usize;
                                                    if want > pending.len() {
                                                        break; // partial frame - wait for the rest
                                                    }
                                                    let frame: Vec<u8> = pending.drain(..want).collect();
                                                msg += 1;
                                                let n = frame.len();
                                                let d = &frame[..];
                                                let hdr = if n >= 6 {
                                                    let len = u32::from_be_bytes([d[2], d[3], d[4], d[5]]);
                                                    format!(" [type {:02x}{:02x} len {} (+6 = {})]",
                                                            d[0], d[1], len, len + 6)
                                                } else { String::new() };
                                                warn!("    BAP TCP msg#{msg} {n}B{hdr}");
                                                info!("      {}", hexdump(d, 160));
                                                use tokio::io::AsyncWriteExt;
                                                // A hand-set hex reply still wins, for probing.
                                                // Otherwise: parse the frame and answer properly.
                                                // Mode 1 = the secured channel. Decrypt with the
                                                // session material we issued, dispatch on the inner
                                                // header, re-seal the reply.
                                                if d[1] == d1_bap::ENVELOPE_MODE_CHANNEL {
                                                    let Some((key, nonces)) = chan.as_mut() else {
                                                        warn!("      mode-1 frame before we issued session material - cannot decrypt");
                                                        continue;
                                                    };
                                                    let n = nonces.peek(d1_bap::Direction::ClientToServer);
                                                    match d1_bap::open_channel_payload(key, n, &d[6..]) {
                                                        Ok(plain) => {
                                                            nonces.advance(d1_bap::Direction::ClientToServer);
                                                            let inner = d1_bap::parse_inner(&plain);
                                                            warn!("      SECURED ({}B): {} -> {:?}", plain.len(), hexdump(&plain, 512), inner);
                                                            // D1_BAP_DUMP_DIR: every decrypted inbound body, whole, as
                                                            // <arrival>-<type>-<seq>.bin. The log line above stops at 512 B.
                                                            if let (Ok(dir), Ok(i)) = (std::env::var("D1_BAP_DUMP_DIR"), inner.as_ref()) {
                                                                static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                                                                let k = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                                                let path = std::path::Path::new(&dir)
                                                                    .join(format!("{k:05}-{:04x}-{}.bin", i.msg_type, i.sequence));
                                                                if let Err(e) = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, &plain)) {
                                                                    warn!("      ⛔ D1_BAP_DUMP_DIR {}: {e}", path.display());
                                                                }
                                                            }
                                                            let inner_type = inner.as_ref().ok().map(|i| i.msg_type);
                                                            if inner_type == Some(d1_bap::TYPE_QUEUEZ_REGISTER_REQUEST) {
                                                                let prev = PRIMARY_BAP_CONNECTION.swap(conn_index, Ordering::SeqCst);
                                                                if prev != conn_index {
                                                                    warn!("    ⭐ BAP connection #{conn_index} sent queuez_register - it is now the PRIMARY (was #{prev})");
                                                                }
                                                            }
                                                            // ⭐⭐ ENQUEUE FIRST, BEFORE ANY HANDLER RUNS.
                                                            //
                                                            // That ordering is the whole point: the FIFO records ARRIVAL
                                                            // order at the moment arrival actually happens, so R1 is
                                                            // structural rather than a convention a handler could forget.
                                                            // The ticket it hands back is the only way to name a sequence,
                                                            // it is not Clone, and `park` consumes it - so "reply twice"
                                                            // and "reply to something that was never asked" are values
                                                            // that cannot be constructed, not checks that can be skipped.
                                                            let ticket = match (fifo, inner.as_ref().ok()) {
                                                                (true, Some(i)) => {
                                                                    let name = d1_bap::message_name(i.msg_type).unwrap_or("?");
                                                                    match conv.enqueue(i.sequence, i.msg_type) {
                                                                        d1_bap::EnqueueOutcome::Awaiting(t) => {
                                                                            info!("      [conv] RECV type {:#06x} ({name}) seq {} -> AWAITING \
                                                                                   response {:#06x}; queue depth {}",
                                                                                  i.msg_type, i.sequence, t.response_type(),
                                                                                  conv.pending_len());
                                                                            Some(t)
                                                                        }
                                                                        // R6. The client created NO in-flight entry for this
                                                                        // send, so any reply we make is unroutable and fatal.
                                                                        // No ticket exists, so there is nothing to reply with.
                                                                        d1_bap::EnqueueOutcome::FireAndForget => {
                                                                            info!("      [conv] RECV type {:#06x} ({name}) seq {} -> \
                                                                                   NO REPLY (client policy: fire-and-forget, R6)",
                                                                                  i.msg_type, i.sequence);
                                                                            None
                                                                        }
                                                                        d1_bap::EnqueueOutcome::Unknown => {
                                                                            warn!("      [conv] ** NAMED GATE ** type {:#06x} seq {} is in \
                                                                                   NEITHER the policy table nor the observed-live set - \
                                                                                   dropping. Do NOT guess N -> N+1 (that heuristic is what \
                                                                                   R6 exists to forbid); walk its policy vtable instead.",
                                                                                  i.msg_type, i.sequence);
                                                                            None
                                                                        }
                                                                        d1_bap::EnqueueOutcome::UnwritableResponse { response_type, header_class } => {
                                                                            warn!("      [conv] ** NAMED GATE ** type {:#06x} ({name}) pairs to \
                                                                                   response {response_type:#06x}, header class {header_class:?} - \
                                                                                   a layout NOBODY HAS READ. Writing a 6-byte header there does \
                                                                                   not error, it parks the client in its resumable header read \
                                                                                   (0xB2ED40) with no timeout, no log and no error branch. \
                                                                                   Read 0x19234A0 for that class first.",
                                                                                  i.msg_type);
                                                                            None
                                                                        }
                                                                        d1_bap::EnqueueOutcome::DuplicateSequence(s) => {
                                                                            warn!("      [conv] ⛔ OUR BUG: sequence {s} is already outstanding. \
                                                                                   The client's counter is monotonic per connection, so this \
                                                                                   means our framing double-delivered a frame.");
                                                                            None
                                                                        }
                                                                    }
                                                                }
                                                                _ => None,
                                                            };
                                                            let reply_body = match inner {
                                                                Ok(i) if i.msg_type == d1_bap::TYPE_QUEUEZ_REGISTER_REQUEST => {
                                                                    info!("      -> register subscriber rsp (0x007a), seq {}", i.sequence);
                                                                    Some(d1_bap::reply_header(
                                                                        d1_bap::TYPE_QUEUEZ_REGISTER_RESPONSE, i.sequence))
                                                                }
                                                                Ok(i) if i.msg_type == d1_bap::TYPE_ECHO_REQUEST => {
                                                                    info!("      -> echo rsp (0x00fb), seq {}", i.sequence);
                                                                    Some(d1_bap::reply_header(d1_bap::TYPE_ECHO_RESPONSE, i.sequence))
                                                                }
                                                                // ---- the `profile_setup` conversation (§26.49) --------------------
                                                                // Once investment_signin completes, the client opens three exchanges
                                                                // we had never implemented, and hangs up when they go unanswered -
                                                                // which is what `error.lost_bap_connection` (the Weasel) actually is.
                                                                // It is NOT a keepalive failure; the keepalives are answered.
                                                                //
                                                                // These two are header-only ACKs on the protocol's established
                                                                // request/response pairing (N -> N+1: 0x79/0x7a, 0xfa/0xfb,
                                                                // 0x0a/0x0b, 0x12e/0x12f). That pairing is a REGULARITY, not a
                                                                // decoded contract - `[I]`, and the reason these are `warn!` and
                                                                // labelled rather than silent. If either turns out to have no
                                                                // paired response class, replying invents a message and corrupts
                                                                // the in-flight queue, exactly as 0x00AB would (see XETROV below).
                                                                // Set D1_ACK_PROFILE_SETUP=0 to go back to not answering.
                                                                // ⚠ MEASURED HARMFUL, DEFAULT OFF (§26.50). Answering 0x0020
                                                                // with 0x0021 made the client hang up IMMEDIATELY - 650B and
                                                                // 4 messages, where leaving it unanswered let the same client
                                                                // reach 2477B and go on to send 0x0015 and investment id 701.
                                                                // So 0x0020 has NO paired response class and inventing one
                                                                // corrupts the in-flight queue, exactly as 0x00AB would.
                                                                // The N -> N+1 pairing is a regularity, not a rule. Kept behind
                                                                // D1_ACK_PROFILE_SETUP=1 only so the negative is reproducible.
                                                                // ⭐ DEFAULT ON. These two sit in the in-flight queue directly
                                                                // ahead of investment 701 and 504 at `character:signin`, and
                                                                // only the queue HEAD is ever examined - so until they are
                                                                // discharged, IN ORDER, no later reply of ours can match and
                                                                // the queue times out regardless. Run 20260807-165310 proved
                                                                // this the hard way: a CORRECT 504 body still produced
                                                                // `failed to find recipient [sequence 13]` because 0x0020 at
                                                                // sequence 10 was sitting at the head.
                                                                //
                                                                // The old code here sent a header-only reply to BOTH, on the
                                                                // N->N+1 pairing guess. The reply types were right; one of the
                                                                // two bodies was fatally wrong:
                                                                //   0x0016  protobuf 0x15AB3E0 = one `repeated varint`, NO
                                                                //           required field -> empty body is a COMPLETE and
                                                                //           valid "no purchased offers". Header-only is right.
                                                                //   0x0021  protobuf 0x15AB800 = 3 optional strings + a
                                                                //           **REQUIRED varint #4** -> an empty body CANNOT
                                                                //           parse, `decode_payload` (0x5699A0) returns 0.
                                                                //           Needs `20 00` at minimum.
                                                                // See d1_bap::purchased_offers_response /
                                                                // user_message_response_body for the walks and their controls.
                                                                //
                                                                // ⚠ §26.50 recorded "0x0020 has no paired response class" from
                                                                // a wire measurement. The class table contradicts it and that
                                                                // run was doubly confounded - empty body AND out of order
                                                                // behind an unanswered 0x000C. D1_ACK_PROFILE_SETUP=0 keeps
                                                                // the negative reproducible.
                                                                // ---- 0x0012: CLIENT CONFIG (policy 15) --------------------------
                                                                // FIRST SEEN 2026-08-07 in 20260807-223920-AIDT-0018-ORBIT - the
                                                                // run that first entered `setup:orbit`. Sent FROM orbit, so it is
                                                                // gated behind everything above it.
                                                                //
                                                                // ⚠ NOT header-only. Payload descriptor 0x15A7BA0 (via the response
                                                                // class vtable 0x1922A60 slot +0x18 = decode_payload 0x55B860) has
                                                                // TWO REQUIRED varints, so an empty body cannot parse. This is the
                                                                // 0x0021 case, not the 0x0016 case - getting that backwards costs a
                                                                // run and looks like a payload bug.
                                                                //
                                                                // Field 3 (the engine-config delta) is OMITTED on purpose: it is
                                                                // optional, and its callback aborts the whole parse unless it opens
                                                                // with varint 0x123456. Same trap already documented for signon f4.3.
                                                                //
                                                                // The values are the ones the client has already accepted 185 times
                                                                // as signon field 4 - `cookie=0 and social_matchmaking_id=0`.
                                                                Ok(i) if i.msg_type == d1_bap::TYPE_CLIENT_CONFIG_REQUEST => {
                                                                    let cookie = std::env::var("D1_CFG_COOKIE").ok()
                                                                        .and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
                                                                    let smid = std::env::var("D1_CFG_SOCIAL_MM_ID").ok()
                                                                        .and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
                                                                    let b = d1_bap::client_config_response_body(cookie, smid);
                                                                    warn!("      -> 0x0013 client_config rsp (cookie={cookie}, \
                                                                           social_matchmaking_id={smid}, {}B body: {}), seq {}",
                                                                          b.len(),
                                                                          b.iter().map(|x| format!("{x:02x}")).collect::<String>(),
                                                                          i.sequence);
                                                                    Some(d1_bap::client_config_response(i.sequence, cookie, smid))
                                                                }
                                                                // ---- 0x0017: ACCOUNT ID TRANSLATION (policy 12) ------------------
                                                                // FIRST SEEN 2026-08-07 in 20260807-222725-MSG205-FAMILY5-ROOT -
                                                                // it had never appeared in ANY of 142 prior runs. Unlocked by the
                                                                // family-5 root landing, i.e. by the fireteam session existing.
                                                                //
                                                                // ⚠ Leaving it unanswered is FATAL, not merely slow. The client
                                                                // examines only the HEAD of each in-flight queue, so our reply to a
                                                                // LATER request (701 at seq 18) reached a head still expecting
                                                                // 0x0018, mismatched, and raised
                                                                //   _connection_failure_failed_to_find_message_recipient
                                                                // => error.lost_bap_connection. That is the whole cause of death of
                                                                // run 222725; nothing was wrong with the 701 body.
                                                                //
                                                                // The body encoder already existed (`platform_to_investment_response`,
                                                                // written from the decoder at 0x55A490) and had simply never been
                                                                // wired to a delivery path - that path is now PROVEN to be this
                                                                // message. It is TWO PARALLEL LISTS: 36-byte identifiers, then
                                                                // u64 SOIDs. 0x55A8E3 searches list A for the client's own id to
                                                                // find its row, so a SOID-only reply is not a smaller answer, it is
                                                                // an unusable one.
                                                                Ok(i) if i.msg_type == d1_bap::TYPE_ACCOUNT_ID_XLAT_REQUEST => {
                                                                    let body = plain.get(6..).unwrap_or(&[]);
                                                                    let acct = env_soid("D1_ACCOUNT_SOID", 0xD100_0000_0000_0001);
                                                                    match d1_bap::account_id_translation_request(body) {
                                                                        Some((count, kind_a, kind_b)) => {
                                                                            let ids = d1_bap::account_id_translation_request_ids(body);
                                                                            warn!("      0x0017 account_id_translation: count={count} \
                                                                                   kind_a={kind_a:#04x} kind_b={kind_b:#04x} ids={:?}",
                                                                                  ids.iter()
                                                                                     .map(|d| String::from_utf8_lossy(
                                                                                         &d[..d.iter().position(|&c| c == 0).unwrap_or(d.len())])
                                                                                         .to_string())
                                                                                     .collect::<Vec<_>>());
                                                                            let entries: Vec<_> =
                                                                                ids.iter().map(|id| (*id, acct)).collect();
                                                                            warn!("      -> 0x0018 rsp: {} id(s) -> account SOID {acct:#018x}, \
                                                                                   seq {}", entries.len(), i.sequence);
                                                                            Some(d1_bap::account_id_translation_response(
                                                                                i.sequence, &entries))
                                                                        }
                                                                        // The decoder rejects kinds outside {7, 0xFF} before it reads
                                                                        // any payload, so a request we cannot parse is one we cannot
                                                                        // answer correctly. Say so loudly rather than invent a body:
                                                                        // this reply lands at a queue head, and a wrong one there is
                                                                        // exactly as fatal as none.
                                                                        None => {
                                                                            warn!("      ** NAMED GATE ** 0x0017 body did not parse \
                                                                                   ({}B): {}", body.len(), hexdump(body, 64));
                                                                            None
                                                                        }
                                                                    }
                                                                }
                                                                Ok(i) if (i.msg_type == d1_bap::TYPE_USER_MSG_REQUEST
                                                                          || i.msg_type == d1_bap::TYPE_UPO_REQUEST)
                                                                    && std::env::var("D1_ACK_PROFILE_SETUP")
                                                                        .map(|v| v != "0").unwrap_or(true) => {
                                                                    if i.msg_type == d1_bap::TYPE_UPO_REQUEST {
                                                                        warn!("      -> 0x0016 update_purchased_offers rsp \
                                                                               (empty IdList - no required field), seq {}",
                                                                              i.sequence);
                                                                        Some(d1_bap::purchased_offers_response(i.sequence))
                                                                    } else {
                                                                        let b = d1_bap::user_message_response_body();
                                                                        warn!("      -> 0x0021 user_message rsp (required field \
                                                                               #4 = 0, {}B body: {}), seq {}", b.len(),
                                                                              b.iter().map(|x| format!("{x:02x}")).collect::<String>(),
                                                                              i.sequence);
                                                                        Some(d1_bap::user_message_response(i.sequence))
                                                                    }
                                                                }
                                                                // ---- 0x000C: THE FAMILY SUBSCRIPTION REQUEST (§26.57) -------------
                                                                // Body is `u8 family_class || u64be root_soid`; observed
                                                                // `04 d1 00 00 00 00 00 00 01` = family 4, our account SOID, at
                                                                // sequence 7 - BEFORE the keepalive at 8 where we push.
                                                                //
                                                                // We have never answered it, in any run. The client's own
                                                                // diagnostic then says:
                                                                //   queuez_family_update_begin: ignoring family-update due to
                                                                //   unhandled replication state, family_type=ENUM(4),
                                                                //   family_root_soid=0xD100000000000001, replication state=1
                                                                // and `0x56AB40(.., mode=1)` is exactly what leaves a family in
                                                                // state 1, with nothing in the registry subsystem advancing it.
                                                                // So the push registers the OBJECT but the FAMILY is never opened
                                                                // for replication.
                                                                //
                                                                // ⭐ DEFAULT IS NOW **ON** (2026-08-07). This is not a guess and it
                                                                // is not optional: leaving 0x000C unanswered is what
                                                                // `error.lost_bap_connection` actually was.
                                                                //
                                                                // 0x000C is `c->b sub req` and it goes into one of the client's two
                                                                // IN-FLIGHT REQUEST QUEUES (conn+0x1b80, stride 0x198). Those
                                                                // queues are walked head-only, and they time out. So an unanswered
                                                                // 0x000C at sequence 7 does two things, both measured:
                                                                //   (a) `update_receiving()` (0xB2D500) eventually reports
                                                                //       "message queue '1' contains timed out elements, bailing"
                                                                //       -> `_connection_failure_timed_out`. Run
                                                                //       20260807-162406-silent504 died exactly this way with
                                                                //       0x000C as the ONLY unanswered message in the transcript.
                                                                //   (b) it HEAD-OF-LINE BLOCKS every later reply: `update_receive()`
                                                                //       (0xB2E940) compares the queue HEAD's expected reply type
                                                                //       against ours and `jne`s away, so our 0x000B reply to
                                                                //       investment 504 produced "failed to find recipient for header
                                                                //       [sequence 13][type c->ws rsp][status 200]" ->
                                                                //       `_connection_failure_failed_to_find_message_recipient`.
                                                                //
                                                                // ⇒ §26.50's "answering is worse than silence" measurements for
                                                                // 0x0020 / 701 / 504 were CONFOUNDED by this. Those replies were
                                                                // rejected for arriving out of order behind 0x000C, not for being
                                                                // wrong. See `d1_bap::family_subscribe_response` for the full
                                                                // derivation and the eboot addresses.
                                                                //
                                                                // §26.58 measured "0x000D did nothing" - correct, and explained:
                                                                // its handler is `0x50FE20 = ret`. The dequeue is the whole point
                                                                // and it is invisible in the guest log.
                                                                //   D1_SUB_ACK=0        go back to silence (keeps the negative)
                                                                //   D1_SUB_ACK=echo     reply 0x000D + the request body echoed
                                                                Ok(i) if i.msg_type == d1_bap::TYPE_CLIENT_TO_BAP_SUBSCRIPTION_REQUEST
                                                                    && std::env::var("D1_SUB_ACK").map(|v| v != "0").unwrap_or(true) => {
                                                                    let mode = std::env::var("D1_SUB_ACK").unwrap_or_default();
                                                                    let body = &plain[6..];
                                                                    warn!("      -> FAMILY SUBSCRIBE ACK 0x000D ({mode}) for class {} root {:#018x}, seq {}",
                                                                          body.first().copied().unwrap_or(0),
                                                                          body.get(1..9).map(|b| u64::from_be_bytes(b.try_into().unwrap())).unwrap_or(0),
                                                                          i.sequence);
                                                                    // ⭐ THE FAMILY-0/2 TIMEOUT. Measured 2026-08-08 on run
                                                                    // 20260808-011129: classes 0 and 2 subscribe **56 times
                                                                    // each** and we ACK all 112 and deliver NOTHING. The client
                                                                    // logs `networking:investment:peer: Failed peer-subscription
                                                                    // validation [error error_timeout]` **548 times**, and its
                                                                    // own replication table ends up at
                                                                    //     [ 0] class=0  state=4 LOST/TIMED-OUT (update REFUSED)
                                                                    //     [21] class=2  state=4 LOST/TIMED-OUT (update REFUSED)
                                                                    // while 3, 4 and 5 sit at state=3 REPLICATING.
                                                                    //
                                                                    // ⚠ **State 4 REFUSES updates**, so once it times out no
                                                                    // later delivery - and no memory poke - can rescue it. The
                                                                    // answer has to arrive on the subscription.
                                                                    //
                                                                    // ⚠ AND IT CANNOT BE `D1_PUSH_FAMILY`. That push is
                                                                    // one-shot on a trigger and fires at seq 7; classes 0 and 2
                                                                    // do not subscribe until seq 20/21. A trigger-based push
                                                                    // is structurally too early - which is why listing them
                                                                    // there would have looked wired and done nothing.
                                                                    //
                                                                    // `D1_ANSWER_FAMILIES=0,2` answers each subscription for
                                                                    // those classes with an EMPTY family body (object_count=0)
                                                                    // written immediately after this ACK. Empty first on
                                                                    // purpose: it separates "the subscription needs an answer
                                                                    // at all" from "the answer needs contents", and we do not
                                                                    // yet know either family's instance types.
                                                                    // Default OFF. Evidence: historicaldocs/design/director-and-unlocks.md.
                                                                    let class = body.first().copied().unwrap_or(0) as u32;
                                                                    let root = body.get(1..9)
                                                                        .map(|b| u64::from_be_bytes(b.try_into().unwrap()))
                                                                        .unwrap_or(0);
                                                                    if std::env::var("D1_ANSWER_FAMILIES").unwrap_or_default()
                                                                        .split(',')
                                                                        .filter_map(|v| v.trim().parse::<u32>().ok())
                                                                        .any(|f| f == class)
                                                                    {
                                                                        // Ordering is NOT free: the ACK dequeues the in-flight
                                                                        // request, and a reply that arrives ahead of it makes
                                                                        // the next reply mismatch and strands the queue
                                                                        // (`_connection_failure_failed_to_find_message_recipient`).
                                                                        // So this is only ARMED here; it is written after the
                                                                        // ACK goes out.
                                                                        answer_family = Some((class, root, i.sequence));
                                                                        // ⛔ RE-SEND DISABLED, 2026-08-08 - it was my own regression.
                                                                        // It was added to beat the state-1 race, but the answer is
                                                                        // a BASELINE (flags 0x01) and a second baseline on a family
                                                                        // that already has one is rejected:
                                                                        //     failed to process update for [family_type ENUM(0) ... object_count 0]
                                                                        //     failed to process update for [family_type ENUM(2) ... object_count 3]
                                                                        // Family 0's EMPTY body - which worked perfectly in run
                                                                        // 20260808-024546 - started failing the moment the re-send
                                                                        // existed, which is what identified it.
                                                                        //
                                                                        // Answer-once is the proven shape: that run took all five
                                                                        // families to state=3 REPLICATING with zero complaints. The
                                                                        // state-1 race is real but the client resolves 1 -> 2 LOCALLY
                                                                        // at send time (historicaldocs/design/subscription-race-verdict.md), so a
                                                                        // single answer is enough. Re-enable only as a non-baseline
                                                                        // update, never as a repeated baseline.
                                                                        if std::env::var("D1_RESEND_FAMILIES").is_ok() {
                                                                            resend_families.push((class, root));
                                                                        }
                                                                    }
                                                                    let mut r = d1_bap::client_to_bap_subscription_response(i.sequence);
                                                                    if mode == "echo" {
                                                                        r.extend_from_slice(body);
                                                                    }
                                                                    Some(r)
                                                                }
                                                                Ok(i) if i.msg_type == d1_bap::TYPE_CLIENT_TO_XETROV_REQUEST => {
                                                                    // 0x00ab is client_to_xetrov_request: a one-way
                                                                    // protobuf telemetry message. The client message
                                                                    // table contains no paired response class, so
                                                                    // inventing 0x00ac would corrupt the in-flight
                                                                    // queue. Decode and record it, but do not reply.
                                                                    match d1_bap::parse_xetrov_request(&plain[6..]) {
                                                                        Ok(req) => info!(
                                                                            "      XETROV one-way: top=({}, {}) wrapper={} \
                                                                             account={:?} field2={:?} field3={:?} \
                                                                             build={:?} build_string={:?} platform={:?}",
                                                                            req.field1,
                                                                            req.field2,
                                                                            req.envelope.field1,
                                                                            req.envelope.event.account_soid,
                                                                            req.envelope.event.field2,
                                                                            req.envelope.event.field3,
                                                                            req.envelope.event.field4,
                                                                            req.envelope.event.field5,
                                                                            req.envelope.event.field6,
                                                                        ),
                                                                        Err(e) => warn!("      XETROV protobuf decode failed: {e}"),
                                                                    }
                                                                    None
                                                                }
                                                                Ok(i) if i.msg_type == d1_bap::TYPE_CLIENT_TO_WORLD_SERVER_REQUEST => {
                                                                    // Body is `u16be message_id` then a bit-packed,
                                                                    // content-defined payload we cannot yet author.
                                                                    let id = if plain.len() >= 8 {
                                                                        u16::from_be_bytes([plain[6], plain[7]])
                                                                    } else { 0 };
                                                                    warn!("      INVESTMENT message id {id} ({} body bytes)",
                                                                          plain.len().saturating_sub(6));
                                                                    // `D1_INV_STATUS` drives the decode-skip probe.
                                                                    let st: u16 = std::env::var("D1_INV_STATUS").ok()
                                                                        .and_then(|v| v.parse().ok()).unwrap_or(200);
                                                                    if st != 200 {
                                                                        warn!("      DECODE-SKIP PROBE: replying status {st} (!=200) - client should skip the body");
                                                                    }
                                                                    // `D1_INV_PAD=N` appends N zero
                                                                    // bytes of argument bitstream.
                                                                    // Empty args is a plausible read
                                                                    // of the bitstream error; zeros
                                                                    // mean "every optional field
                                                                    // absent" in these grammars.
                                                                    let pad: usize = std::env::var("D1_INV_PAD").ok()
                                                                        .and_then(|v| v.parse().ok()).unwrap_or(0);
                                                                    if pad > 0 {
                                                                        warn!("      ARG-PAD PROBE: {pad} zero bytes of argument bitstream");
                                                                    }
                                                                    // `D1_AIDT=<online-id>` replies with a real
                                                                    // `platform_to_investment_response` instead of zero
                                                                    // padding - the account-SOID translation, encoded
                                                                    // from the client's own decoder grammar at
                                                                    // `0x55A490`. This is the value `0x584EB0` needs in
                                                                    // order to subscribe family 4 then family 3.
                                                                    // ⚠ `[U]` It is NOT proven that 503's response is
                                                                    // what feeds that decoder - see
                                                                    // `historicaldocs/design/character-and-orbit.md` §10.7.
                                                                    // `D1_503_SOID=1` replies per message 503's OWN
                                                                    // schema descriptor: a single optional `u64`
                                                                    // (`0x80801825` -> 1 field of `0x80801796`, size 8).
                                                                    // Leading presence bit is 1, so unlike every prior
                                                                    // reply the decoder is FORCED to commit - see
                                                                    // `historicaldocs/design/character-and-orbit.md` §11.5/§13.
                                                                    // A 503 body belongs ONLY to message 503. Before
                                                                    // this guard, D1_503_RAW was attached to every
                                                                    // investment request: the first post-pig message
                                                                    // was id 206 and received a 503 object, immediately
                                                                    // preceding the guest crash.
                                                                    // `D1_INV_SILENT=701,...` - do not answer these investment
                                                                    // ids at all.
                                                                    //
                                                                    // THE DISCRIMINATOR for §26.49/§26.51. Investment id 701 is the
                                                                    // last message before every `error.lost_bap_connection`, but
                                                                    // "last before the close" is correlation. If staying silent
                                                                    // ALSO closes the connection, our reply is not the trigger and
                                                                    // the Weasel is something else. If silence survives, the reply
                                                                    // is the trigger and the body is what needs authoring.
                                                                    // Silence is already known to be correct for 0x0020, where
                                                                    // answering was measurably worse (§26.50).
                                                                    let silent: Vec<u32> = std::env::var("D1_INV_SILENT")
                                                                        .unwrap_or_default()
                                                                        .split(',')
                                                                        .filter_map(|v| v.trim().parse().ok())
                                                                        .collect();
                                                                    if silent.contains(&(id as u32)) {
                                                                        warn!("      D1_INV_SILENT: NOT answering investment id {id}");
                                                                        None
                                                                    } else if let Some(body) = character_lifecycle_reply(
                                                                        id,
                                                                        plain.get(8..).unwrap_or(&[]),
                                                                        plain.get(6..).unwrap_or(&[]),
                                                                    ) {
                                                                        // ⭐ 501 answered: the client just made a Guardian and we
                                                                        // gave it a SOID. Now it needs to SEE it - flip the
                                                                        // zero-character account off and re-arm the push so the
                                                                        // next trigger delivers roster + character.
                                                                        if id == d1_bap::character_lifecycle::CREATE_CHARACTER {
                                                                            pushed_character = false;
                                                                            // ⭐ P2: the next family-4 baseline restarts the
                                                                            // version sequence, so the recorded span and the
                                                                            // one-shot invalidation must both re-arm with it.
                                                                            family4_span = None;
                                                                            family4_invalidated = false;
                                                                            warn!("      ⭐ 501 answered - account now reports 1 character; family push re-armed");
                                                                        }
                                                                        Some(d1_bap::investment_message_response_with_body(
                                                                            i.sequence, id, st, &body))
                                                                    } else {
                                                                    let schema_body = if id == d1_bap::INVESTMENT_MESSAGE_ID_503 {
                                                                    // ⛔⛔ THIS ARM USED TO BE NESTED INSIDE `D1_503_SOID`
                                                                    // BEING SET, so an unset environment answered 503 with
                                                                    // nothing at all - and the ten-flag default would have
                                                                    // been inert exactly the way `D1_503_RAW` alone is.
                                                                    // `D1_503_SOID=0` remains the explicit opt-out for
                                                                    // bisecting; *unset* now means "serve the good default".
                                                                    (!std::env::var("D1_503_SOID").is_ok_and(|v| v == "0"))
                                                                        .then(|| {
                                                                            let mut b = served_503_body();
                                                                            // `D1_INV_PAD` now also applies here. Discriminator:
                                                                            // if the complaint vanishes with trailing zeros, the
                                                                            // decoder wanted MORE fields (zeros = absent) and our
                                                                            // first field is fine; if it persists, the first
                                                                            // field's own encoding is wrong.
                                                                            b.resize(b.len() + pad, 0);
                                                                            warn!("      503 SCHEMA REPLY: +{pad}B pad -> {}B [{}]",
                                                                                  b.len(),
                                                                                  b.iter().map(|x| format!("{x:02x}")).collect::<String>());
                                                                            b
                                                                        })
                                                                    } else if id == d1_bap::INVESTMENT_MESSAGE_ID_504 {
                                                                        // 504 = `login_character`, sent when the player presses
                                                                        // cross on a character card at `character:signin`.
                                                                        //
                                                                        // Policy families 0 and 1 both map 0x000A -> 0x000B, so
                                                                        // the client ALWAYS has an in-flight entry waiting for an
                                                                        // investment response - 504 included. Silence therefore
                                                                        // does not "leave it alone"; it leaves the entry in queue
                                                                        // 1 until `update_receiving()` (0xB2D500) reports
                                                                        // "contains timed out elements" and kills the connection.
                                                                        // So this one has to be ANSWERED, and answered WELL: the
                                                                        // header-only 32-byte reply we used to send is what
                                                                        // `character:signin` weaseled on.
                                                                        //
                                                                        // Body derived from the client's own encoder 0x261BE0 over
                                                                        // response schema 0x80801845 and round-tripped back
                                                                        // through its own decoder 0x261FB0. Do NOT hand-edit -
                                                                        // regenerate with rsat-schema-encode-oracle.py. See
                                                                        // `d1_bap::investment_504_response_body`.
                                                                        //
                                                                        // `D1_504_RAW=<hex>` overrides the argument bitstream, and
                                                                        // `D1_INV_SILENT=504` still suppresses the reply entirely,
                                                                        // so both negatives stay reproducible.
                                                                        let raw = hex_env("D1_504_RAW");
                                                                        let b = if raw.is_empty() {
                                                                            d1_bap::investment_504_response_body()
                                                                        } else {
                                                                            warn!("      504 RAW BODY override ({}B)", raw.len());
                                                                            raw
                                                                        };
                                                                        warn!("      -> native-oracle 504 login_character response \
                                                                               body ({}B): {}", b.len(),
                                                                              b.iter().map(|x| format!("{x:02x}")).collect::<String>());
                                                                        Some(b)
                                                                    } else if id == 701 {
                                                                        // 701's response schema 0x8080189B is the SAME
                                                                        // one-composite shape as 504's, so the client's own
                                                                        // encoder emits the same argument bitstream.
                                                                        // This replaces `D1_INV_SILENT=701`: silence never
                                                                        // discharged 701's in-flight entry, it only deferred
                                                                        // the timeout (see d1_bap::investment_701_response_body).
                                                                        let raw = hex_env("D1_701_RAW");
                                                                        let b = if raw.is_empty() {
                                                                            d1_bap::investment_701_response_body()
                                                                        } else {
                                                                            warn!("      701 RAW BODY override ({}B)", raw.len());
                                                                            raw
                                                                        };
                                                                        warn!("      -> native-oracle 701 response body ({}B): {}",
                                                                              b.len(),
                                                                              b.iter().map(|x| format!("{x:02x}")).collect::<String>());
                                                                        Some(b)
                                                                    } else if id == 205 {
                                                                        // 205 appears only once the
                                                                        // PSN session exists, and it
                                                                        // was the last message before
                                                                        // the connection died -
                                                                        // head-of-line blocking again.
                                                                        // Response schema 0x80801827
                                                                        // is a 952-byte object, NOT
                                                                        // the 8-byte shape 504/701/702
                                                                        // share, so it needs its own
                                                                        // oracle-derived body.
                                                                        // ⭐ `D1_205_RAW` mirrors
                                                                        // `D1_503_RAW`. The two
                                                                        // messages embed the SAME
                                                                        // 944-byte `0x80801941`
                                                                        // (503 at C +0x018, 205 at
                                                                        // C +0x008), and 205 is
                                                                        // answered LAST, so a 205
                                                                        // that disagrees can undo
                                                                        // what 503 delivered - or,
                                                                        // with a zero root, kill
                                                                        // the boot outright.
                                                                        // `historicaldocs/design/rung33-unlock-gate.md` §6.
                                                                        // ⚠⚠ AND 205 IS ANSWERED MORE THAN ONCE PER RUN -
                                                                        // L254/L1098/L2953 against a single 503 at L143 in
                                                                        // `runs/20260810-145553-X11-ORBIT`. Every one of them
                                                                        // is a chance to overwrite 503's flags with count=0,
                                                                        // so the flag body has to be the DEFAULT here, not an
                                                                        // env the operator might set once and forget.
                                                                        let b = served_205_body();
                                                                        warn!("      -> native-oracle 205 response body ({}B) [schema 0x80801827, 952-byte object]", b.len());
                                                                        Some(b)
                                                                    } else if id == 702 {
                                                                        // 702 appeared for the FIRST TIME once the account
                                                                        // began naming a profile in field 3 (§26.76) - the
                                                                        // client takes a different branch in
                                                                        // `bootflow:profile_setup` and uploads a 2467-byte
                                                                        // body. A 32-byte header-only reply killed the
                                                                        // connection there, which is the 701 disease.
                                                                        //
                                                                        // Catalogue row 83: request `0x808016CF`, response
                                                                        // **`0x8080194D`** - and that descriptor is
                                                                        // `struct 8 bytes, 1 field -> 0x80801796`, i.e.
                                                                        // byte-for-byte the SAME one-composite shape as
                                                                        // 504's `0x80801845` and 701's `0x8080189B`. So the
                                                                        // client's own encoder emits the same bitstream.
                                                                        let raw = hex_env("D1_702_RAW");
                                                                        let b = if raw.is_empty() {
                                                                            d1_bap::investment_701_response_body()
                                                                        } else {
                                                                            warn!("      702 RAW BODY override ({}B)", raw.len());
                                                                            raw
                                                                        };
                                                                        warn!("      -> native-oracle 702 response body ({}B): {} \
                                                                               [schema 0x8080194D, same shape as 504/701]",
                                                                              b.len(),
                                                                              b.iter().map(|x| format!("{x:02x}")).collect::<String>());
                                                                        Some(b)
                                                                    } else if id == 104 {
                                                                        // FIRST SEEN at `setup:orbit`, 2026-08-07. Response
                                                                        // 0x808018F8's field [0] is the SAME 0x80801796
                                                                        // {u4,u32} composite as 504/701/702, so the body is
                                                                        // the familiar five bytes - confirmed by the oracle
                                                                        // (`--tag 0x808018f8 --msgid 104` -> 0068 1800000000).
                                                                        //
                                                                        // ⚠ Before this branch existed 104 fell through to
                                                                        // `None`, so we replied with a message id and NO
                                                                        // object. That is what produced
                                                                        //   handle_response_internal: Failed to decode [policy 1]
                                                                        // - policy 1 being client_to_world_server_deterministic
                                                                        // (0x000A->0x000B), i.e. this very channel.
                                                                        let b = d1_bap::investment_104_response_body();
                                                                        warn!("      -> native-oracle 104 response body ({}B): {} \
                                                                               [schema 0x808018F8, same 0x80801796 composite as 504]",
                                                                              b.len(),
                                                                              b.iter().map(|x| format!("{x:02x}")).collect::<String>());
                                                                        Some(b)
                                                                    } else if id == 105 {
                                                                        // 105 is 104's sibling. Request `0x808018AF` is a single
                                                                        // u16; response `0x80801702` is one field of the SAME
                                                                        // `0x80801796` {u4, u32} composite as 104/504/701/702, so
                                                                        // the body is the familiar five bytes - and the client's
                                                                        // own encoder says so, rather than the analogy:
                                                                        //   d1-oracle message --tag 0x80801702 --msgid 105
                                                                        //   -> wire 7B 0069 1800000000
                                                                        //
                                                                        // ⚠ 105 has NEVER been observed. A census of every
                                                                        // `INVESTMENT message id N` line across all 249 responder
                                                                        // logs gives {702:2058, 701:1898, 503:264, 206:195,
                                                                        // 205:171, 104:162, 501:9, 504:8, 505:2, 406:1} and no
                                                                        // 105 - a NOT-SEEN from an instrument whose positive
                                                                        // controls (104, 206, 501) all fire, and which does not
                                                                        // depend on the stale diagnostic-catalogue TSV. Without
                                                                        // this arm it falls to the padded default, which is a
                                                                        // message id and NO object: exactly the shape that
                                                                        // produced `Failed to decode [policy 1]` for 104.
                                                                        let b = d1_bap::character_lifecycle::RESULT_OK_BODY.to_vec();
                                                                        warn!("      -> native-oracle 105 response body ({}B): {} \
                                                                               [schema 0x80801702, same 0x80801796 composite as 104] \
                                                                               ⭐ FIRST 105 EVER SEEN",
                                                                              b.len(),
                                                                              b.iter().map(|x| format!("{x:02x}")).collect::<String>());
                                                                        Some(b)
                                                                    } else if id == d1_bap::loot::LOOT_PICKUP_ITEM {
                                                                        // 601 loot_pickup_item: the client walked onto a world
                                                                        // drop. Answer OK with the claimed nonce echoed
                                                                        // (response 0x808017AB, see d1_bap::loot).
                                                                        let req = plain.get(8..).unwrap_or(&[]);
                                                                        match d1_bap::loot::parse_loot_pickup_request(req) {
                                                                            Some(c) => {
                                                                                warn!("      ⭐ LOOT CLAIM (601): source kind {} nonce {{{}, {:#x}}} word {}",
                                                                                      c.source_kind, c.key, c.id, c.word);
                                                                                loot_grants_due.push(c.id);
                                                                            }
                                                                            None => warn!("      601 request did not decode ({}B)", req.len()),
                                                                        }
                                                                        d1_bap::loot::loot_pickup_response_body(req)
                                                                    } else {
                                                                        // ⚠ DO NOT warn "unhandled" here. This is the FIRST of
                                                                        // two dispatch chains; 206 (and anything else below) is
                                                                        // handled by the second one, so a warning at this point
                                                                        // fires for messages that are answered correctly one line
                                                                        // later. A warning added here on 2026-08-07 did exactly
                                                                        // that and had to be removed the same night - it reported
                                                                        // `investment id 206 has no body` immediately before the
                                                                        // 53-byte D1_206_BODY override went out.
                                                                        None
                                                                    };
                                                                    if let Some(body) = schema_body {
                                                                        Some(d1_bap::investment_message_response_with_body(
                                                                            i.sequence, id, st, &body))
                                                                    } else if id == d1_bap::INVESTMENT_MESSAGE_ID_206 {
                                                                        // Native encoder 0x261be0 over response schema
                                                                        // 0x80801869 emits body 0x10 for a zero-valued
                                                                        // response object; native decode + re-encode is
                                                                        // byte-identical. Do not hand-fit this from the
                                                                        // failed live frame.
                                                                        // `D1_206_TAIL=<hex>` - EXPERIMENT HOOK, mirrors the
                                                                        // D1_503_RAW pattern. Message 206's response schema
                                                                        // 0x80801869 is a ONE-BYTE struct (a single 4-bit leaf),
                                                                        // so the span the client dies on at 0x570AAE cannot come
                                                                        // from the investment body -- it must be a WRAPPER-level
                                                                        // attachment that `type|seq|status|msgid|body` never
                                                                        // emits. This appends raw bytes after the body so that
                                                                        // hypothesis can be tested against the client's own
                                                                        // decoder. Success signal is span count != 0 in the
                                                                        // tracer, NOT merely the absence of a crash.
                                                                        // NOT protocol modelling: no d1-bap change, passthrough
                                                                        // only. See historicaldocs/design/character-and-orbit.md 26.21.
                                                                        // `D1_206_BODY=<hex>` - body OVERRIDE, not append.
                                                                        // ⚠ The span is BIT-PACKED, not concatenated: the
                                                                        // presence bit sits at bit 4 of the body, so the u16
                                                                        // length and every span byte straddle byte boundaries.
                                                                        // D1_206_TAIL (which emits `10 || tail`) therefore
                                                                        // CANNOT express a span, and that is why every
                                                                        // append-based experiment failed. Generate the value
                                                                        // with the client's own encoder:
                                                                        //   capture-rig/oodle/rsat-oracle.py --schema 0x80801869 \
                                                                        //     --msgid 206 --encode --span <queuez hex>
                                                                        // and take its `body =` line. Passthrough only; no
                                                                        // d1-bap change and no protocol modelling here.
                                                                        // See historicaldocs/design/span-sweep.md §6.
                                                                        let tail = hex_env("D1_206_TAIL");
                                                                        if !tail.is_empty() {
                                                                            let mut body = d1_bap::fetch_family_response_body(None);
                                                                            body.extend_from_slice(&tail);
                                                                            warn!("      -> message-206 body 10 + D1_206_TAIL ({}B tail)", tail.len());
                                                                            Some(d1_bap::investment_message_response_with_body(
                                                                                i.sequence, d1_bap::INVESTMENT_MESSAGE_ID_206, st, &body))
                                                                        } else {
                                                                            let body = message_206_body(plain.get(8..).unwrap_or(&[]));
                                                                            Some(d1_bap::investment_message_response_with_body(
                                                                                i.sequence, d1_bap::INVESTMENT_MESSAGE_ID_206, st, &body))
                                                                        }
                                                                    } else {
                                                                    match std::env::var("D1_AIDT").ok().filter(|v| !v.is_empty()) {
                                                                        Some(online_id) => {
                                                                            let acct = env_soid("D1_ACCOUNT_SOID", 0xD100_0000_0000_0001);
                                                                            let ident = d1_bap::platform_identifier(&online_id);
                                                                            let body = d1_bap::platform_to_investment_response(&[(ident, acct)]);
                                                                            warn!("      AIDT: platform_to_investment_response \
                                                                                   '{online_id}' -> account SOID {acct:#018x} ({}B)",
                                                                                  body.len());
                                                                            info!("      aidt body: {}",
                                                                                  body.iter().map(|b| format!("{b:02x}")).collect::<String>());
                                                                            Some(d1_bap::investment_message_response_with_body(
                                                                                i.sequence, id, st, &body))
                                                                        }
                                                                        // `D1_INV_BODY_<id>=<hex>` - a real response body for any
                                                                        // investment message, generated by the client's own encoder
                                                                        // (`rsat-oracle.py --schema <rsp tag> --msgid <id> --encode
                                                                        // --span <hex>`), not hand-authored.
                                                                        //
                                                                        // WHY THIS EXISTS (§26.55). Answering investment **501**
                                                                        // with the padded EMPTY body makes the client RST; staying
                                                                        // silent (`D1_INV_SILENT=501`) does not save it either -
                                                                        // measured, it still closes. So 501 is on the far side of
                                                                        // the "silence is safer" law that holds for 0x0020 and 701:
                                                                        // it genuinely wants a reply, with content.
                                                                        //
                                                                        // ⛔ CORRECTED 2026-08-08. This comment used to say "501 is
                                                                        // `login_character`" and that 501's response has **0 schema
                                                                        // fields**, so its payload must ride as a wrapper-level SPAN
                                                                        // like 206's. BOTH ARE WRONG, and they compounded:
                                                                        //   * 501 is `login_create_character`; `login_character` is
                                                                        //     504. (FNV-1 name hash in the shipped definition table,
                                                                        //     confirmed by `mov edi,0x1F5` at 0x551040 and
                                                                        //     `mov edi,0x1F8` at 0x551640.)
                                                                        //   * 501's response schema 0x8080185D has **2** fields, not
                                                                        //     0 - `{0x80801796 result, u64}` - and the u64 is the new
                                                                        //     character's SOID. There is no span.
                                                                        // ⇒ the empty body was fatal because it was a *malformed
                                                                        // create reply*, not because a span was missing. Handled
                                                                        // properly now by `character_lifecycle_reply` above.
                                                                        // historicaldocs/design/character-lifecycle-messages.md §1, §4.
                                                                        None => {
                                                                            let per_id = hex_env(&format!("D1_INV_BODY_{id}"));
                                                                            if !per_id.is_empty() {
                                                                                warn!("      -> investment {id} D1_INV_BODY_{id} override ({}B)",
                                                                                      per_id.len());
                                                                                if id == 501 {
                                                                                    // ⭐ The client just created a Guardian and we
                                                                                    // gave it a SOID. Now it needs to SEE it: flip
                                                                                    // off the empty roster and re-arm the push so
                                                                                    // the next trigger delivers roster+character.
                                                                                    CHARACTER_CREATED.store(true, std::sync::atomic::Ordering::Relaxed);
                                                                                    pushed_character = false;
                                                                                    family4_span = None;
                                                                                    family4_invalidated = false;
                                                                                    warn!("      ⭐ 501 answered - roster now reports 1 character; family push re-armed");
                                                                                }
                                                                                Some(d1_bap::investment_message_response_with_body(
                                                                                    i.sequence, id, st, &per_id))
                                                                            } else {
                                                                                Some(d1_bap::investment_message_response_padded(
                                                                                    i.sequence, id, st, pad))
                                                                            }
                                                                        }
                                                                    }
                                                                    }
                                                                    }
                                                                }
                                                                // ═══════════════════════════════════════════════════════════
                                                                //  RUNGS 28-30 - the activity-host lane.
                                                                //
                                                                //  The server owns exactly TWO levers in the whole activity
                                                                //  launch: **message 7** and **message 17**. Everything
                                                                //  between them is the client talking to itself over its own
                                                                //  peer-to-peer session channel (msg 0x25/0x26), and the
                                                                //  server is not a peer - it holds no `c_network_session`,
                                                                //  has no peer index and appears in no peer bitmask.
                                                                //
                                                                //  ⛔ So there is no "deliver the session parameter" branch
                                                                //  here and there never can be. `historicaldocs/design/SERVER-ROLES-PLAN.md`
                                                                //  §4 deletes that plan shape wherever it appears. What we do
                                                                //  is hand over an AH id (7) and then resolve it (17).
                                                                // ═══════════════════════════════════════════════════════════

                                                                // ---- 6 -> 7: activity_host_manager (Phase 2) ---------------
                                                                // ⭐ THE REPLY THAT HANDS THE CLIENT ITS ACTIVITY HOST.
                                                                // Body is ALWAYS 0x100F bytes whatever the inner type; the
                                                                // args field is the activity-host LAUNCH COMMAND LINE
                                                                // ("/ah_type:group|fireteam ..."), which is worth logging
                                                                // verbatim even though we only decode it.
                                                                Ok(i) if i.msg_type == TYPE_ACTIVITY_HOST_MANAGER_REQUEST
                                                                    && env_flag("D1_AH_MANAGER", true) => {
                                                                    let off = bap_body_offset(i.msg_type).min(plain.len());
                                                                    let body = &plain[off..];
                                                                    match d1_bap::activity_host::ActivityHostManagerRequest::decode(body) {
                                                                        Ok(req) => warn!("      ⭐⭐ ACTIVITY-HOST MANAGER REQUEST (msg 6): \
                                                                                          inner_type={} args={:?} nested_msg={}B \
                                                                                          bitstream={}B trailing_id={:#018x}",
                                                                                         req.inner_type, req.args_str(),
                                                                                         req.nested_msg.len(), req.bitstream.len(),
                                                                                         req.trailing_id),
                                                                        // R2 still applies: answer anyway. A wrong body is
                                                                        // survivable; an absent reply head-of-line blocks and
                                                                        // then times out fatally.
                                                                        Err(e) => warn!("      ** NAMED GATE ** msg 6 body did not decode \
                                                                                         ({}B, want 0x100F): {e} - answering anyway (R2)",
                                                                                        body.len()),
                                                                    }
                                                                    let token = env_soid("D1_AH_HOST_TOKEN", DEFAULT_AH_HOST_TOKEN);
                                                                    let text = ah_host_text();
                                                                    match d1_bap::activity_host::ActivityHostManagerResponse::new(token, &text) {
                                                                        Some(r) => {
                                                                            let b = r.encode();
                                                                            warn!("      -> msg 7 activity_host_manager_response: \
                                                                                   host_token={token:#018x} host_text={text:?} ({}B body)",
                                                                                  b.len());
                                                                            warn!("      ⭐ FALSIFIER ARMED (§4.2): if BAP message 16 never \
                                                                                   follows, the [I] link 'the client-as-session-host writes \
                                                                                   the AH id into s_activity_host_connection_information' is \
                                                                                   WRONG and the id reaches that parameter some other way. \
                                                                                   Do not paper over the difference.");
                                                                            let mut f = d1_bap::reply_header(
                                                                                TYPE_ACTIVITY_HOST_MANAGER_RESPONSE, i.sequence);
                                                                            f.extend_from_slice(&b);
                                                                            Some(f)
                                                                        }
                                                                        None => {
                                                                            warn!("      ⛔ D1_AH_HOST_TEXT {text:?} does not fit in 128 bytes \
                                                                                   with its NUL - NOT answering msg 6, which will synthesise \
                                                                                   a 500 at the deadline");
                                                                            None
                                                                        }
                                                                    }
                                                                }
                                                                // ---- 16 -> 17: get_activity_host_proxy (Phase 3) -----------
                                                                // ⭐⭐ Message 16 ARRIVING AT ALL is the falsifier for the whole
                                                                // Phase 2 -> Phase 3 chain: `FUN_00594320` returns false and
                                                                // issues nothing while the session parameter's AH id is zero,
                                                                // so 16 cannot appear until something wrote that field.
                                                                Ok(i) if i.msg_type == TYPE_GET_ACTIVITY_HOST_PROXY_REQUEST
                                                                    && env_flag("D1_AH_PROXY", true) => {
                                                                    let off = bap_body_offset(i.msg_type).min(plain.len());
                                                                    let body = &plain[off..];
                                                                    match d1_bap::activity_host::GetActivityHostProxyRequest::decode(body) {
                                                                        Ok(req) => {
                                                                            warn!("      ⭐⭐ FALSIFIER PASSED - BAP MESSAGE 16 ARRIVED. The \
                                                                                   client has a non-zero activity-host id in its session \
                                                                                   parameter, so §4.2's [I] link HOLDS. activity_host_id={:#018x}{}",
                                                                                  req.activity_host_id,
                                                                                  if req.activity_host_id == 0 {
                                                                                      " (0 is legal and means 'the proxy itself')"
                                                                                  } else { "" });
                                                                            let (ip, port) = ah_endpoint();
                                                                            let rsp = d1_bap::activity_host::GetActivityHostProxyResponse::new(
                                                                                req, ip, port);
                                                                            warn!("      -> msg 17 get_activity_host_proxy_response: \
                                                                                   id {:#018x} -> {}:{}. ⚠ THE ID ECHO IS MANDATORY - \
                                                                                   0x594E70 DROPS a mismatch SILENTLY, which looks exactly \
                                                                                   like 'the server never answered'.",
                                                                                  rsp.activity_host_id, rsp.ipv4(), rsp.port());
                                                                            warn!("      watch the guest log for: 'AH IP resolution \
                                                                                   successful' - the cheapest success signal in the plan");
                                                                            // ⛔⛔ D1_AH_ENDIAN - DEFAULT `be`, AND THE `le` EXPERIMENT
                                                                            // FAILED. Read this before touching the byte order again.
                                                                            //
                                                                            // `AH IP resolution successful: 'proxy -> 2.0.77.10:780'`
                                                                            // is a BYTE-REVERSED **PRINT**, not a byte-reversed value.
                                                                            // The connect path uses the same field correctly:
                                                                            //   BE run (20260808-195418): resolver printed 2.0.77.10:780
                                                                            //     but the channel reached `_connected[10.77.0.2:3075]` x11.
                                                                            //   LE run (20260808-202647): resolver printed 10.77.0.2:3075
                                                                            //     and transport_endpoint_async_connect() was called with
                                                                            //     **[ip address 2.0.77.10:780]** and failed outright ->
                                                                            //     error.starting_private_activity_instance_error_connecting_to_bap.
                                                                            // ⇒ Two consumers, ONE swap between them. Making the printer
                                                                            // look right makes the CONNECT wrong. The original doc note on
                                                                            // GetActivityHostProxyResponse::proxy_ipv4 ("three sites, one
                                                                            // consistent reading", incl. 0x594320 swapping it back out)
                                                                            // WAS CORRECT and its retraction is itself retracted.
                                                                            // D1_AH_ENDIAN=le reproduces the failure.
                                                                            let be = std::env::var("D1_AH_ENDIAN")
                                                                                .map(|v| !v.eq_ignore_ascii_case("le")).unwrap_or(true);
                                                                            let body = if be { rsp.encode() } else { rsp.encode_client_order() };
                                                                            warn!("      -> msg 17 body ({}): {}", if be { "BE - negative control" } else { "LE - client order" },
                                                                                  body.iter().map(|b| format!("{b:02x}")).collect::<String>());
                                                                            let mut f = d1_bap::reply_header(
                                                                                TYPE_GET_ACTIVITY_HOST_PROXY_RESPONSE, i.sequence);
                                                                            f.extend_from_slice(&body);
                                                                            Some(f)
                                                                        }
                                                                        Err(e) => {
                                                                            warn!("      ** NAMED GATE ** msg 16 body did not decode \
                                                                                   ({}B, want 8): {e} - cannot echo an id we did not read, \
                                                                                   and a wrong echo is a SILENT drop. Letting R2 synthesise.",
                                                                                  body.len());
                                                                            None
                                                                        }
                                                                    }
                                                                }
                                                                // ---- 8: client_to_activity_host_notification (decode only) --
                                                                // ⚠ HEADER CLASS 3 = 14 BYTES. This is the first inbound
                                                                // message in this responder whose body does NOT start at +6.
                                                                // The policy table says it expects no response, so `enqueue`
                                                                // already classified it FireAndForget and issued no ticket -
                                                                // returning None here is the contract, not a gap.
                                                                Ok(i) if i.msg_type == TYPE_CLIENT_TO_ACTIVITY_HOST_NOTIFICATION => {
                                                                    let off = bap_body_offset(i.msg_type).min(plain.len());
                                                                    let body = &plain[off..];
                                                                    match d1_bap::activity_host::ClientToActivityHostNotification::decode(body) {
                                                                        Ok(n) => warn!("      ⭐ ACTIVITY-HOST NOTIFICATION (msg 8): \
                                                                                        inner_type={} user_defined_type={} aux={:?} \
                                                                                        payload={}B - one-way, NOT answered (R6)",
                                                                                       n.inner_type, n.user_defined_type, n.aux,
                                                                                       n.payload.len()),
                                                                        Err(e) => warn!("      ** NAMED GATE ** msg 8 body did not decode \
                                                                                         ({}B at offset {off}): {e}", body.len()),
                                                                    }
                                                                    None
                                                                }
                                                                // ---- 9: bap_to_client_notification (decode only) -----------
                                                                // ⭐⭐ THE MOST INFORMATIVE FRAME ON THIS WIRE, AND WE USED TO
                                                                // THROW IT AWAY 18 TIMES A RUN.
                                                                //
                                                                // The client only emits this when message 100's push policy
                                                                // (`0x564C20`) relays: it reads the class-6 header's `+0x06`
                                                                // u64 as a CONNECTION INDEX and re-sends the identical body as
                                                                // policy 9 = message 9 on that connection. We leave `+0x06`
                                                                // at zero, connection 0 is this responder, so every message
                                                                // 100 we push comes straight back here - 1:1, byte for byte.
                                                                //
                                                                // ⇒ A message 9 arriving INBOUND is proof of two things at
                                                                // once: the push reached the client intact, and the client
                                                                // did NOT deliver it to its activity client. It is a receipt
                                                                // for a misroute, not a mirror.
                                                                //
                                                                // Body layout is message 100's, and deliberately decoded with
                                                                // message 100's decoder: the two message classes share vtable
                                                                // slots 2/3/4/6 in the client, so there is exactly one codec
                                                                // (`design/format-description.md` - no format twice).
                                                                //
                                                                // Fire-and-forget. Header class 0 has no status field and the
                                                                // policy table expects no response; replying would put a frame
                                                                // on the wire with a sequence the client has already retired,
                                                                // which is the Weasel (`historicaldocs/design/RUNG-28-REACHED.md`).
                                                                Ok(i) if i.msg_type == TYPE_BAP_TO_CLIENT_NOTIFICATION => {
                                                                    let off = bap_body_offset(i.msg_type).min(plain.len());
                                                                    let body = &plain[off..];
                                                                    match d1_bap::activity_host::BapToClientNotification::decode(body) {
                                                                        Ok(n) => {
                                                                            let udt = n.user_defined_type;
                                                                            let name = d1_bap::activity_host::ah_to_client_udt_name(udt);
                                                                            warn!("      ⭐⭐ BAP-TO-CLIENT NOTIFICATION (msg 9, seq {}): \
                                                                                   recipient_id=0x{:016x} udt={} ({}) payload={}B",
                                                                                  i.sequence, n.recipient_id, udt,
                                                                                  name.unwrap_or("⚠ NOT an AH->client udt"),
                                                                                  n.payload.len());
                                                                            // The first 32 payload bytes: enough to tell one
                                                                            // schema-coded object from another without pulling
                                                                            // `d1-oracle` (and `unicorn`) into this binary.
                                                                            let head: String = n.payload.iter().take(32)
                                                                                .map(|b| format!("{b:02x}")).collect();
                                                                            warn!("         payload[0..32]: {head}{}",
                                                                                  if n.payload.len() > 32 { " …" } else { "" });
                                                                            // ⚠ Same variable and same default as the message-7 path
                                                                            // at `let token = …` - reading it differently here would
                                                                            // make this diagnostic disagree with what we handed out,
                                                                            // which is `SERVER-SURFACE-LEDGER` L7 all over again.
                                                                            let ours = env_soid("D1_AH_HOST_TOKEN", DEFAULT_AH_HOST_TOKEN);
                                                                            if n.recipient_id == ours {
                                                                                warn!("         ⛔ recipient_id == OUR activity-host id, and this frame \
                                                                                       arrived INBOUND on the primary connection. That is message 100's \
                                                                                       relay policy (0x564C20) firing: the client received the push, \
                                                                                       read header +0x06 (= 0) as a connection index, and forwarded it \
                                                                                       here instead of delivering it. Push BAP 9, not BAP 100 - \
                                                                                       historicaldocs/design/establish-client-gate.md.");
                                                                            } else {
                                                                                warn!("         ⚠ recipient_id is NOT our activity-host id (0x{ours:016x}). \
                                                                                       FUN_0075AB60 matches this field against [ac+0xC98], the ESTABLISHED \
                                                                                       AH id - a mismatch is a SILENT drop with no diagnostic at all.");
                                                                            }
                                                                        }
                                                                        Err(e) => warn!("      ** NAMED GATE ** msg 9 body did not decode \
                                                                                         ({}B at offset {off}): {e} - the body layout is \
                                                                                         message 100's; if this fires, the shared-codec \
                                                                                         reading (vtable slots 2/3/4/6) is wrong.",
                                                                                        body.len()),
                                                                    }
                                                                    None
                                                                }
                                                                // ---- 42 -> 43: matchmaking configuration (Phase 1) ----------
                                                                // The client reads exactly ONE thing for kind 4: the PRESENCE
                                                                // of response field 4 (`0x599F80` tests the descriptor's own
                                                                // presence flag). On the wire the load-bearing part is four
                                                                // bytes: `22 02 0A 00`.
                                                                //
                                                                // ⭐ THE NONCE IS CLIENT-INTERNAL. `FUN_00598710` compares the
                                                                // live interface's nonce against the one the client stashed
                                                                // out of its OWN outgoing request; the response body is never
                                                                // consulted for it. So `current request nonce: '0x0'` means
                                                                // the interface had already been ZEROED when the callback
                                                                // fired - i.e. by the lost-BAP-connection cleanup. That
                                                                // failure is DOWNSTREAM of the Weasel, not an independent
                                                                // gate, and a nonce mismatch cannot be caused by us. We echo
                                                                // it anyway because a real server surely does.
                                                                Ok(i) if i.msg_type == TYPE_MATCHMAKING_CORE_REQUEST
                                                                    && env_flag("D1_MM_CONFIG", true) => {
                                                                    let off = bap_body_offset(i.msg_type).min(plain.len());
                                                                    let body = &plain[off..];
                                                                    let rsp = match d1_bap::matchmaking::MatchmakingCoreRequest::decode(body) {
                                                                        Ok(req) => {
                                                                            warn!("      ⭐ MATCHMAKING CORE REQUEST (msg 42): version={:?} \
                                                                                   kind={:?} nonce={:?} search_slot={:?} set_id={:?} \
                                                                                   ad_slot={:?} key={:?}",
                                                                                  req.version, req.request_kind, req.nonce,
                                                                                  req.search_slot, req.matchmaking_set_id,
                                                                                  req.advertisement_slot, req.matchmaking_key);
                                                                            // ⭐ D1_ACTIVITY_FOLLOW_FILE: the kind-4 key names the
                                                                            // activity the player LAUNCHED (Director -> LAUNCH sends it
                                                                            // before the AH join). Hand its hash to the Activity Host,
                                                                            // which maps it to the activity index it publishes.
                                                                            if req.request_kind
                                                                                == Some(d1_bap::matchmaking::MM_KIND_MATCHMAKING_CONFIGURATION)
                                                                            {
                                                                                if let (Ok(path), Some(key)) = (
                                                                                    std::env::var("D1_ACTIVITY_FOLLOW_FILE"),
                                                                                    req.matchmaking_key,
                                                                                ) {
                                                                                    match std::fs::write(&path, format!("{}\n", key.activity_hash)) {
                                                                                        Ok(()) => warn!("      ⭐ activity follow: wrote launched activity hash {:#010x} to {path}",
                                                                                                        key.activity_hash),
                                                                                        Err(e) => warn!("      ⛔ activity follow: {path}: {e}"),
                                                                                    }
                                                                                }
                                                                            }
                                                                            if req.request_kind
                                                                                != Some(d1_bap::matchmaking::MM_KIND_MATCHMAKING_CONFIGURATION)
                                                                            {
                                                                                warn!("      ⚠ kind is not 4 (matchmaking_configuration) - \
                                                                                       kinds 1/2/3 are session search and advertisement, and \
                                                                                       we answer them with the same minimal configuration. \
                                                                                       That is a stub, not a contract.");
                                                                            }
                                                                            d1_bap::matchmaking::MatchmakingCoreResponse::echoing(&req)
                                                                        }
                                                                        Err(e) => {
                                                                            warn!("      ** NAMED GATE ** msg 42 body did not decode ({}B): \
                                                                                   {e} - answering with the minimal configuration anyway (R2)",
                                                                                  body.len());
                                                                            d1_bap::matchmaking::MatchmakingCoreResponse::minimal_configuration(
                                                                                d1_bap::matchmaking::MM_KIND_MATCHMAKING_CONFIGURATION, 0)
                                                                        }
                                                                    };
                                                                    let b = rsp.encode();
                                                                    warn!("      -> msg 43 matchmaking_core_response: kind={:?} nonce={:?} \
                                                                           f4 present={} ({}B body: {})",
                                                                          rsp.kind, rsp.nonce, rsp.configuration.is_some(), b.len(),
                                                                          b.iter().map(|x| format!("{x:02x}")).collect::<String>());
                                                                    let mut f = d1_bap::reply_header(
                                                                        TYPE_MATCHMAKING_CORE_RESPONSE, i.sequence);
                                                                    f.extend_from_slice(&b);
                                                                    Some(f)
                                                                }
                                                                Ok(i) if i.msg_type == d1_bap::TYPE_REGISTER_RELAY_CLIENT_REQUEST => {
                                                                    // Last step of BAP setup. The 40-byte body
                                                                    // (`u16be 0x24 || 36 opaque || u16be 0x0d`) needs
                                                                    // nothing echoed; the response is header-only,
                                                                    // and `update(_setup_primary_register_relay_client)`
                                                                    // then goes straight to `_connected`.
                                                                    info!("      -> register relay client rsp (0x012f), seq {}", i.sequence);
                                                                    Some(d1_bap::reply_header(
                                                                        d1_bap::TYPE_REGISTER_RELAY_CLIENT_RESPONSE, i.sequence))
                                                                }
                                                                Ok(i) => {
                                                                    warn!("      ** NAMED GATE ** no secured handler for inner type 0x{:04x} seq {}",
                                                                          i.msg_type, i.sequence);
                                                                    None
                                                                }
                                                                Err(e) => { warn!("      inner parse failed: {e}"); None }
                                                            };
                                                            // Drain any family armed on a PREVIOUS frame: one inbound
                                                            // message has now been fully processed by the client, so its
                                                            // manager tick has run and the descriptor is at state 2.
                                                            if !resend_families.is_empty() && answer_family.is_none() {
                                                                let acct = env_soid("D1_ACCOUNT_SOID", 0xD100_0000_0000_0001);
                                                                let chr = env_soid("D1_CHARACTER_SOID", 0xD100_0000_0000_0101);
                                                                for (class, root) in std::mem::take(&mut resend_families) {
                                                                    // ⚠ This match MUST mirror the one in the answer path. It
                                                                    // did not, and family 2 fell to the `_` arm: the re-send
                                                                    // would have followed real progressions with an EMPTY
                                                                    // family-2 body and undone them - a fix that silently
                                                                    // destroys what it just delivered.
                                                                    let fbody = match class {
                                                                        d1_bap::QUEUEZ_FAMILY_ACCOUNT | d1_bap::QUEUEZ_FAMILY_CHARACTER =>
                                                                            build_family_body(class, acct, chr, ""),
                                                                                                                                                // ⭐ D1_ROSTER_DIR (default OFF) - real family-2 objects so the orbit
                                                                        //    nameplate BINDS. Unset/empty falls through to EXACTLY the previous
                                                                        //    behaviour for both families.
                                                                        // ⛔ FAMILY 2 ONLY. 0xA3D4E0 blanks on family 2 (0xA3DD16 test r12,r12),
                                                                        //    but when family 0/type 1 ALSO resolves it runs the RICH fill 0xA2D9F0
                                                                        //    INSTEAD of the compact one - a substitution, not an augmentation. So a
                                                                        //    half-built family-0 object BLANKS a plate family 2 alone would have
                                                                        //    filled. historicaldocs/design/orbit-hud-blank.md §7.2.
                                                                        0 | 2 => build_roster_family_body(class, root).unwrap_or_else(|| match class {
                                                                            2 => build_family2_body(root),
                                                                            _ => d1_bap::queuez_empty_family_body_for(class, root),
                                                                        }),
                                                                        _ => d1_bap::queuez_empty_family_body_for(class, root),
                                                                    };
                                                                    let Ok(push) = conv.push_with_sequence(
                                                                        d1_bap::TYPE_QUEUEZ_OBJECT, server_seq, &fbody)
                                                                    else {
                                                                        warn!("      [conv] ⛔ family {class} re-send REFUSED by the push accept-set");
                                                                        continue;
                                                                    };
                                                                    warn!("      ⭐ FAMILY {class} RE-SEND seq {server_seq} ({}B) - past the state-1 race", fbody.len());
                                                                    let sn = nonces.peek(d1_bap::Direction::ServerToClient);
                                                                    if let Ok(f) = d1_bap::seal_channel_frame(key, sn, &push) {
                                                                        nonces.advance(d1_bap::Direction::ServerToClient);
                                                                        if sock.write_all(&f).await.is_ok() { server_seq += 1; }
                                                                    }
                                                                }
                                                            }

                                                            // ⭐⭐ THE SINGLE EMISSION POINT.
                                                                //
                                                                // A handler produced BYTES. It did not decide when they
                                                                // leave and it did not write a header. Under the correlation
                                                                // table the body is PARKED against its ticket and `drain`
                                                                // releases it only when it reaches the front - which is the
                                                                // one order the client's head-only matcher will accept.
                                                            let mut drained: Vec<u32> = Vec::new();
                                                            match (fifo, reply_body, ticket) {
                                                                (true, Some(frame), Some(t)) => {
                                                                    let seq = t.sequence();
                                                                    let rty = t.response_type();
                                                                    match split_reply_frame(&t, &frame) {
                                                                        Some((status, body)) => {
                                                                            if status != d1_bap::STATUS_OK {
                                                                                // ⚠ NOT SILENT. `drain` writes 200 (or 500 when
                                                                                // it synthesises), so a probe status set by
                                                                                // D1_INV_STATUS cannot survive the correlation
                                                                                // table. Saying so is the point: a knob that
                                                                                // quietly stops working is exactly the failure
                                                                                // mode this project keeps paying for.
                                                                                warn!("      [conv] ⚠ handler asked for status {status} on \
                                                                                       seq {seq}, but `drain` writes 200. The correlation \
                                                                                       table has no park-with-status API yet - run the \
                                                                                       decode-skip probe with D1_BAP_FIFO=0.");
                                                                            }
                                                                            match conv.park(t, body) {
                                                                                Ok(()) => info!("      [conv] PARK seq {seq} type {rty:#06x} \
                                                                                                 ({}) {}B body - leaves when it reaches the front",
                                                                                                d1_bap::message_name(rty).unwrap_or("?"),
                                                                                                frame.len() - d1_bap::REPLY_HEADER_LEN),
                                                                                Err(e) => warn!("      [conv] ⛔ PARK REFUSED for seq {seq}: {e} \
                                                                                                 - THIS IS THE WEASEL, caught one layer before \
                                                                                                 the socket instead of on the wire"),
                                                                            }
                                                                        }
                                                                        None => warn!("      [conv] ⛔ handler's frame does not match its ticket \
                                                                                       (want type {rty:#06x} seq {seq}, got {:#06x}/{}) - \
                                                                                       DROPPED. A builder has drifted from the policy table.",
                                                                                      if frame.len() >= 2 { u16::from_be_bytes([frame[0], frame[1]]) } else { 0 },
                                                                                      if frame.len() >= 6 { u32::from_be_bytes([frame[2], frame[3], frame[4], frame[5]]) } else { 0 }),
                                                                    }
                                                                }
                                                                // A reply with no ticket: the client is not waiting for one.
                                                                // Sending it is precisely `0x12`. Refuse and name it.
                                                                (true, Some(frame), None) => {
                                                                    warn!("      [conv] ⛔ REFUSED to send a {}B reply with no outstanding \
                                                                           request - this is exactly the frame that killed both rung-28 runs. \
                                                                           (The handler ran because its match arm fired; the policy table says \
                                                                           the client is not waiting.)", frame.len());
                                                                }
                                                                (true, None, Some(_t)) => {
                                                                    info!("      [conv] no body from the handler; the entry stays outstanding \
                                                                           and R2 will synthesise a {} at the deadline rather than let it rot",
                                                                          d1_bap::STATUS_SYNTHESISED);
                                                                }
                                                                (true, None, None) => {}
                                                                // ⛔ LEGACY PATH, D1_BAP_FIFO=0: write immediately, in handler
                                                                // order, exactly as before 2026-08-08. Kept only so the
                                                                // Weasel remains reproducible as an A/B.
                                                                (false, Some(body), _) => {
                                                                    let sn = nonces.peek(d1_bap::Direction::ServerToClient);
                                                                    match d1_bap::seal_channel_frame(key, sn, &body) {
                                                                        Ok(f) => {
                                                                            nonces.advance(d1_bap::Direction::ServerToClient);
                                                                            if sock.write_all(&f).await.is_ok() {
                                                                                warn!("    --> BAP SECURED REPLY (legacy, unordered) {}B: {}",
                                                                                      f.len(), hexdump(&f, 64));
                                                                            }
                                                                        }
                                                                        Err(e) => warn!("      seal failed: {e}"),
                                                                    }
                                                                }
                                                                (false, None, _) => {}
                                                            }
                                                            if fifo {
                                                                drained = drain_replies(&mut sock, key, nonces, &mut conv).await;
                                                            }

                                                            // The armed family answer, written AFTER the ACK so the
                                                            // in-flight queue stays ordered. See the arming site.
                                                            //
                                                            // ⭐ Under the correlation table "after the ACK returns from the
                                                            // handler" is no longer the same statement as "after the ACK
                                                            // leaves the socket" - a reply can be legally deferred behind an
                                                            // older outstanding request. So the answer is gated on its own
                                                            // ACK sequence actually appearing in the drain.
                                                            if let Some(entry) = answer_family.take() {
                                                                deferred_family_answers.push(entry);
                                                            }
                                                            let mut still_deferred: Vec<(u32, u64, u32)> = Vec::new();
                                                            for (class, root, gate_seq) in std::mem::take(&mut deferred_family_answers) {
                                                                if fifo && !drained.contains(&gate_seq) {
                                                                    info!("      [conv] family {class} answer HELD: its 0x000D ACK \
                                                                           (seq {gate_seq}) has not left the socket yet");
                                                                    still_deferred.push((class, root, gate_seq));
                                                                    continue;
                                                                }
                                                                // ⭐ REAL bodies for families we can build (3, 4); empty for the
                                                                // rest. Answering the SUBSCRIPTION is what gets an update
                                                                // processed - an unsolicited push leaves family 4 at
                                                                // `replication state=1` and every later update is ignored.
                                                                let acct = env_soid("D1_ACCOUNT_SOID", 0xD100_0000_0000_0001);
                                                                let chr = env_soid("D1_CHARACTER_SOID", 0xD100_0000_0000_0101);
                                                                let fbody = match class {
                                                                    d1_bap::QUEUEZ_FAMILY_ACCOUNT | d1_bap::QUEUEZ_FAMILY_CHARACTER =>
                                                                        build_family_body(class, acct, chr, ""),
                                                                                                                                        // ⭐ D1_ROSTER_DIR (default OFF) - real family-2 objects so the orbit
                                                                    //    nameplate BINDS. Unset/empty falls through to EXACTLY the previous
                                                                    //    behaviour for both families.
                                                                    // ⛔ FAMILY 2 ONLY. 0xA3D4E0 blanks on family 2 (0xA3DD16 test r12,r12),
                                                                    //    but when family 0/type 1 ALSO resolves it runs the RICH fill 0xA2D9F0
                                                                    //    INSTEAD of the compact one - a substitution, not an augmentation. So a
                                                                    //    half-built family-0 object BLANKS a plate family 2 alone would have
                                                                    //    filled. historicaldocs/design/orbit-hud-blank.md §7.2.
                                                                    0 | 2 => build_roster_family_body(class, root).unwrap_or_else(|| match class {
                                                                        2 => build_family2_body(root),
                                                                        _ => d1_bap::queuez_empty_family_body_for(class, root),
                                                                    }),
                                                                    _ => d1_bap::queuez_empty_family_body_for(class, root),
                                                                };
                                                                // R5: routed through the accept-set check rather than a
                                                                // hand-written header. 0x007B is header class 4 - 6 bytes,
                                                                // no status - and `push_with_sequence` writes exactly the
                                                                // frame `queuez_push` used to, byte for byte.
                                                                let push = match conv.push_with_sequence(
                                                                    d1_bap::TYPE_QUEUEZ_OBJECT, server_seq, &fbody) {
                                                                    Ok(p) => p,
                                                                    Err(e) => {
                                                                        warn!("      [conv] ⛔ family {class} answer REFUSED: {e}");
                                                                        continue;
                                                                    }
                                                                };
                                                                warn!("      ⭐ FAMILY {class} ANSWER seq {server_seq} root {root:#018x} \
                                                                       ({}B body) - answering the SUBSCRIPTION, not pushing",
                                                                      fbody.len());
                                                                let sn = nonces.peek(d1_bap::Direction::ServerToClient);
                                                                match d1_bap::seal_channel_frame(key, sn, &push) {
                                                                    Ok(f) => {
                                                                        nonces.advance(d1_bap::Direction::ServerToClient);
                                                                        if sock.write_all(&f).await.is_ok() {
                                                                            warn!("    --> BAP FAMILY {class} ANSWER {}B", f.len());
                                                                            server_seq += 1;
                                                                        }
                                                                    }
                                                                    Err(e) => warn!("      family answer seal failed: {e}"),
                                                                }
                                                            }
                                                            deferred_family_answers = still_deferred;

                                                            // Hand the client a character. Doing it
                                                            // once the investment channel is live
                                                            // means `character:signin` sees count 1
                                                            // and takes SELECTION, not creation -
                                                            // which keeps character creation, starter
                                                            // inventory and the prologue mission off
                                                            // the critical path.
                                                            // `D1_PUSH_CHARACTER=0` disables, so the
                                                            // previous behaviour stays reachable as a
                                                            // baseline to diff against.
                                                            //
                                                            // ⚠ TIMING IS A CONFOUND. Pushing after
                                                            // the investment 503 exchange means the
                                                            // push lands on a connection that is
                                                            // already tearing down - the 503 reply
                                                            // has no body, so
                                                            // `_connection_failure_consumer_failure`
                                                            // fires first and any queuez complaint
                                                            // afterwards may be a symptom of that
                                                            // rather than a verdict on our bytes.
                                                            // `D1_PUSH_AT=relay` (the default) pushes
                                                            // at BAP `_connected` instead, which is
                                                            // the earliest clean moment.
                                                            let want_push = std::env::var("D1_PUSH_CHARACTER")
                                                                .map(|v| v != "0").unwrap_or(true);
                                                            let push_at = std::env::var("D1_PUSH_AT")
                                                                .unwrap_or_else(|_| "relay".into());
                                                            // MEASURED (§26.43/§26.45): pushing
                                                            // at `relay` OR at the FIRST
                                                            // `investment` message kills the
                                                            // connection at 350B / 5 messages,
                                                            // for BOTH family 3 and family 4 - so
                                                            // it is the TIMING, not the payload.
                                                            // At those points the client has not
                                                            // yet run `0x584EB0`, so it holds no
                                                            // subscription for the family/root and
                                                            // an unasked-for object is fatal.
                                                            //
                                                            // `echo` waits for the keepalive loop,
                                                            // which in the reference transcript
                                                            // begins only AFTER 503 (seq 4),
                                                            // msg 171, the 206 request (seq 6) and
                                                            // msg 12 - i.e. after the client has
                                                            // subscribed and is idle-waiting for
                                                            // exactly this object.
                                                            let trigger = match push_at.as_str() {
                                                                "investment" => Some(d1_bap::TYPE_CLIENT_TO_WORLD_SERVER_REQUEST),
                                                                "echo" => Some(d1_bap::TYPE_ECHO_REQUEST),
                                                                "relay" => Some(d1_bap::TYPE_REGISTER_RELAY_CLIENT_REQUEST),
                                                                other => {
                                                                    warn!("      D1_PUSH_AT={other:?} is not a known trigger \
                                                                           (relay|investment|echo) - NOT pushing");
                                                                    None
                                                                }
                                                            };
                                                            // ⭐ `conn_index == 0`: only the PRIMARY BAP connection delivers
                                                            // objects. The activity-host address we hand out in messages
                                                            // 7/17 points at this same listener by default, so a second
                                                            // connection is expected - and re-pushing the account and
                                                            // character onto a client that already has them is a baseline
                                                            // on a family that already has one, which the client rejects
                                                            // (`failed to process update for [family_type ...]`).
                                                            // ── ⭐⭐ P2: RE-DELIVER FAMILY 4 AS A DELTA ──────────────────
                                                            //
                                                            // The director, activity selectability, the prologue arm and
                                                            // rung 33's permission plane are ONE mechanism: the
                                                            // **availability cache** at `[0x26A34B0+0x38]`. `0x25FF40`
                                                            // rebuilds its `0x4000`-byte evaluation plane (`+0x0C`) only
                                                            // when the VALIDITY byte (`+0x08`) is zero, and `0x260200`
                                                            // stamps that byte on success AND on failure alike. So the
                                                            // plane is computed ONCE, from whatever the profile held at
                                                            // the time, and never re-asked.
                                                            //
                                                            // `0x574440(4)` is what clears it, and it is reached from the
                                                            // family-update END chain - `0x565140` calls `0x56B5E0` at
                                                            // `0x565569` on EVERY accepted family update, `0x56B5E0` is a
                                                            // four-instruction unconditional tail-jump to `0x56D0F0`,
                                                            // which calls `0x56D1A0`, which opens `if (family == 4)`.
                                                            // ⭐ The per-object classifier `0x56B5F0` is NOT on that path,
                                                            // so this does not need any object to change - which is why
                                                            // the default carries none.
                                                            //
                                                            // ⛔ AND IT MUST NOT BE A SECOND BASELINE. At replication
                                                            // state 3 `0x56AF60` sends `flags & 1` down `0x56B2D0`, which
                                                            // memsets and DELETE-announces every family-4 object whose
                                                            // `{instance_type, soid}` is absent from the delivered
                                                            // keep-list - the client's 4/2 display name and any 4/3 item
                                                            // it holds that we did not re-list. A delta never enters it.
                                                            //
                                                            // ⚠ Fired on an ECHO, not on `D1_PUSH_AT`'s trigger: `relay`
                                                            // (the default) occurs exactly once per connection, so
                                                            // reusing it would mean the delta silently never went. The
                                                            // keepalive echo recurs, and this block sits BEFORE the push
                                                            // block so `family4_span` can only have been set on an
                                                            // EARLIER frame - the client has demonstrably processed the
                                                            // baseline by then, i.e. the descriptor is at state 3.
                                                            let f4mode = family4_invalidate_mode();
                                                            if f4mode != Family4Invalidate::Off
                                                                && conn_index == PRIMARY_BAP_CONNECTION.load(Ordering::SeqCst)
                                                                && !family4_invalidated
                                                                && inner_type == Some(d1_bap::TYPE_ECHO_REQUEST)
                                                            {
                                                                if let Some(base) = family4_span.clone() {
                                                                    // The version the client accepted is the one we sent in
                                                                    // the baseline; `0x56AF60` requires exactly `+1` and
                                                                    // names the value it expected when it refuses.
                                                                    let want = d1_bap::queuez_span_version(&base)
                                                                        .map(|v| v.wrapping_add(1))
                                                                        .unwrap_or(1);
                                                                    let version = env_num::<u32>("D1_FAMILY4_VERSION", want);
                                                                    let body = build_family4_delta(&base, f4mode, version);
                                                                    match body {
                                                                        Some(b) => {
                                                                            match conv.push_with_sequence(
                                                                                d1_bap::TYPE_QUEUEZ_OBJECT, server_seq, &b) {
                                                                                Ok(push) => {
                                                                                    warn!("      ⭐⭐ FAMILY 4 DELTA seq {server_seq} \
                                                                                           ({f4mode:?}, version {version}, {} objects, {}B) \
                                                                                           - invalidating the availability cache at \
                                                                                           [0x26A34B0+0x38]+8 via 0x574440(4)",
                                                                                          d1_bap::queuez_span_object_count(&b).unwrap_or(0),
                                                                                          b.len());
                                                                                    let sn = nonces.peek(d1_bap::Direction::ServerToClient);
                                                                                    if let Ok(f) = d1_bap::seal_channel_frame(key, sn, &push) {
                                                                                        nonces.advance(d1_bap::Direction::ServerToClient);
                                                                                        if sock.write_all(&f).await.is_ok() {
                                                                                            family4_invalidated = true;
                                                                                            family4_version = Some(version);
                                                                                            server_seq += 1;
                                                                                        }
                                                                                    }
                                                                                }
                                                                                Err(e) => warn!("      [conv] ⛔ family-4 delta REFUSED \
                                                                                                 by the push accept-set: {e}"),
                                                                            }
                                                                        }
                                                                        // Loud, and NOT a silent downgrade to the ping: a run
                                                                        // that asked for the object-carrying delta and quietly
                                                                        // got an empty one would read as "the objects don't
                                                                        // matter", which is a false negative pointing at the
                                                                        // wrong conclusion.
                                                                        None => {
                                                                            family4_invalidated = true;
                                                                            warn!("      ⛔ family-4 delta NOT SENT: the recorded \
                                                                                   family-4 span ({}B) could not be re-stamped in \
                                                                                   {f4mode:?} mode. The availability cache stays \
                                                                                   VALID and the plane will not rebuild.", base.len());
                                                                        }
                                                                    }
                                                                }
                                                            }

                                                            // ⭐ THE ITEM GRANT. After a 601 claim has been answered (the
                                                            // reply went out before this echo), hand the claimed item over
                                                            // as a family-4 delta carrying its 4/3 inventory element.
                                                            if !loot_grants_due.is_empty()
                                                                && conn_index == PRIMARY_BAP_CONNECTION.load(Ordering::SeqCst)
                                                                && inner_type == Some(d1_bap::TYPE_ECHO_REQUEST)
                                                            {
                                                                if family4_version.is_none() {
                                                                    warn!("      ⛔ loot grant waits: no family-4 baseline accepted yet");
                                                                } else {
                                                                    for nonce in std::mem::take(&mut loot_grants_due) {
                                                                        let Some((soid, element, profile)) = loot_grant_for(nonce) else {
                                                                            warn!("      ⛔ no D1_LOOT_GRANT entry for claimed nonce {nonce:#x}: nothing delivered");
                                                                            continue;
                                                                        };
                                                                        let version = family4_version.unwrap_or(0).wrapping_add(1);
                                                                        let acct = env_soid("D1_ACCOUNT_SOID", 0xD100_0000_0000_0001);
                                                                        let prof_soid = env_soid("D1_PROFILE_SOID", 0xD100_0000_0000_0201);
                                                                        let b = d1_bap::loot::item_grant_delta(acct, version, soid, &element,
                                                                            profile.as_deref().map(|p| (prof_soid, p)));
                                                                        match conv.push_with_sequence(d1_bap::TYPE_QUEUEZ_OBJECT, server_seq, &b) {
                                                                            Ok(push) => {
                                                                                let sn = nonces.peek(d1_bap::Direction::ServerToClient);
                                                                                if let Ok(f) = d1_bap::seal_channel_frame(key, sn, &push) {
                                                                                    nonces.advance(d1_bap::Direction::ServerToClient);
                                                                                    if sock.write_all(&f).await.is_ok() {
                                                                                        warn!("      ⭐⭐ LOOT GRANT: nonce {nonce:#x} -> item {soid:#018x} ({}B element{}) as family-4 delta version {version}, seq {server_seq}",
                                                                                              element.len(),
                                                                                              profile.as_ref().map(|p| format!(" + {}B profile", p.len())).unwrap_or_default());
                                                                                        family4_version = Some(version);
                                                                                        server_seq += 1;
                                                                                    }
                                                                                }
                                                                            }
                                                                            Err(e) => warn!("      [conv] ⛔ loot grant delta REFUSED by the push accept-set: {e}"),
                                                                        }
                                                                        // The equipment change: the peer cards after the pickup,
                                                                        // as family-0/2 deltas (the loadout is read from 0/1).
                                                                        for (family, body) in roster_grant_deltas(acct) {
                                                                            match conv.push_with_sequence(d1_bap::TYPE_QUEUEZ_OBJECT, server_seq, &body) {
                                                                                Ok(push) => {
                                                                                    let sn = nonces.peek(d1_bap::Direction::ServerToClient);
                                                                                    if let Ok(f) = d1_bap::seal_channel_frame(key, sn, &push) {
                                                                                        nonces.advance(d1_bap::Direction::ServerToClient);
                                                                                        if sock.write_all(&f).await.is_ok() {
                                                                                            warn!("      ⭐⭐ LOOT GRANT: family {family} peer-card delta ({}B, {} object(s)), seq {server_seq}",
                                                                                                  body.len(), d1_bap::queuez_span_object_count(&body).unwrap_or(0));
                                                                                            server_seq += 1;
                                                                                        }
                                                                                    }
                                                                                }
                                                                                Err(e) => warn!("      [conv] ⛔ family {family} grant delta REFUSED by the push accept-set: {e}"),
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }

                                                            // ⭐ AN ON-DEMAND PROFILE. `D1_PROFILE_PUSH_FILE=<path>`: when that
                                                            // file appears, the next echo pushes its bytes (a client-encoded
                                                            // profile payload) as a family-4 delta, then renames it `.sent`.
                                                            // The activity completion is a profile change: the client
                                                            // announces it when profile +0x2c38 (the completed flag of field
                                                            // [6], struct 0x808017b9 at +0x2c20) goes from 0 to 1.
                                                            if conn_index == PRIMARY_BAP_CONNECTION.load(Ordering::SeqCst)
                                                                && inner_type == Some(d1_bap::TYPE_ECHO_REQUEST)
                                                                && family4_version.is_some()
                                                            {
                                                                if let Some(path) = std::env::var_os("D1_PROFILE_PUSH_FILE").map(std::path::PathBuf::from) {
                                                                    if let Ok(payload) = std::fs::read(&path) {
                                                                        let _ = std::fs::rename(&path, path.with_extension("sent"));
                                                                        let version = family4_version.unwrap_or(0).wrapping_add(1);
                                                                        let acct = env_soid("D1_ACCOUNT_SOID", 0xD100_0000_0000_0001);
                                                                        let prof_soid = env_soid("D1_PROFILE_SOID", 0xD100_0000_0000_0201);
                                                                        let b = d1_bap::loot::profile_delta(acct, version, prof_soid, &payload);
                                                                        match conv.push_with_sequence(d1_bap::TYPE_QUEUEZ_OBJECT, server_seq, &b) {
                                                                            Ok(push) => {
                                                                                let sn = nonces.peek(d1_bap::Direction::ServerToClient);
                                                                                if let Ok(f) = d1_bap::seal_channel_frame(key, sn, &push) {
                                                                                    nonces.advance(d1_bap::Direction::ServerToClient);
                                                                                    if sock.write_all(&f).await.is_ok() {
                                                                                        warn!("      ⭐⭐ PROFILE PUSH: {} ({}B) as family-4 delta version {version}, seq {server_seq}",
                                                                                              path.display(), payload.len());
                                                                                        family4_version = Some(version);
                                                                                        server_seq += 1;
                                                                                    }
                                                                                }
                                                                            }
                                                                            Err(e) => warn!("      [conv] ⛔ profile push REFUSED by the push accept-set: {e}"),
                                                                        }
                                                                    }
                                                                }
                                                            }

                                                            if want_push
                                                                && conn_index == PRIMARY_BAP_CONNECTION.load(Ordering::SeqCst)
                                                                && !pushed_character
                                                                && inner_type == trigger
                                                            {
                                                                let acct = env_soid("D1_ACCOUNT_SOID", 0xD100_0000_0000_0001);
                                                                let chr = env_soid("D1_CHARACTER_SOID", 0xD100_0000_0000_0101);
                                                                // `D1_QUEUEZ_PROBE=empty` sends the
                                                                // same family header with NO objects.
                                                                // Still complains -> the failure is at
                                                                // family/root lookup, before the object
                                                                // loop. Goes quiet -> it is the object.
                                                                let probe = std::env::var("D1_QUEUEZ_PROBE")
                                                                    .unwrap_or_default();
                                                                // `0x584EB0` subscribes the account
                                                                // root to family 4 FIRST, then family
                                                                // 3 - so family 4 (the account object)
                                                                // is what clears the boot gate, and
                                                                // family 3 is downstream character
                                                                // state.
                                                                // `D1_PUSH_FAMILY=4,3` - push SEVERAL objects, in order.
                                                                //
                                                                // Family 4 (account) is what clears the investment_signin gate
                                                                // (§26.47). But pushing ONLY family 4 leaves the client with an
                                                                // account and ZERO characters, so `character:signin` counts 0 and
                                                                // takes CREATION rather than SELECTION - which is where the
                                                                // operator got stuck: the create screen renders, and confirming
                                                                // needs a character-creation conversation we have not written.
                                                                //
                                                                // Order is not free: `0x584EB0` subscribes class 4 first
                                                                // (`0x584F74`) and class 3 second (`0x584F8B`), so push 4 then 3.
                                                                let families: Vec<u32> = std::env::var("D1_PUSH_FAMILY")
                                                                    .unwrap_or_else(|_| d1_bap::QUEUEZ_FAMILY_CHARACTER.to_string())
                                                                    .split(',')
                                                                    .filter_map(|v| v.trim().parse().ok())
                                                                    .collect();
                                                                for family in families {
                                                                let body = build_family_body(family, acct, chr, &probe);
                                                                // R5, through the client's own 13-entry accept-set. Frame is
                                                                // byte-identical to the old hand-written one (class 4, 6-byte
                                                                // header, no status) - asserted in d1-bap's own tests.
                                                                let Ok(push) = conv.push_with_sequence(
                                                                    d1_bap::TYPE_QUEUEZ_OBJECT, server_seq, &body)
                                                                else {
                                                                    warn!("      [conv] ⛔ QueueZ family {family} push REFUSED by the accept-set");
                                                                    continue;
                                                                };
                                                                // The send line below MUST name the family. A census that
                                                                // greps only "--> BAP FAMILY n" (the ANSWER-path label)
                                                                // cannot see this push, and that exact blind spot produced
                                                                // a published "families 3/4 were never delivered" finding
                                                                // on 2026-08-11 while both were on the wire and accepted.
                                                                // A family-n delivery census must match BOTH
                                                                // "--> BAP FAMILY n ANSWER" and
                                                                // "--> BAP QUEUEZ PUSH .* family n".
                                                                // (Kept as "QUEUEZ <words> <len>B" so send-receive-ledger's
                                                                // PUSH_WRITE regex still matches.)
                                                                warn!("      QUEUEZ FAMILY {family} PUSH seq {server_seq} acct {acct:#018x} char {chr:#018x} ({}B body)",
                                                                      push.len() - 6);
                                                                info!("      push plaintext: {}", hexdump(&push, 128));
                                                                let sn = nonces.peek(d1_bap::Direction::ServerToClient);
                                                                match d1_bap::seal_channel_frame(key, sn, &push) {
                                                                    Ok(f) => {
                                                                        nonces.advance(d1_bap::Direction::ServerToClient);
                                                                        if sock.write_all(&f).await.is_ok() {
                                                                            warn!("    --> BAP QUEUEZ PUSH {}B family {family}: {}", f.len(), hexdump(&f, 64));
                                                                            pushed_character = true;
                                                                            // ⭐ P2: record the family-4 span WE ACTUALLY WROTE,
                                                                            // not a rebuild of it. The invalidation delta is a
                                                                            // re-stamp of exactly these bytes, so it carries every
                                                                            // object this delivery carried, by construction.
                                                                            if family == d1_bap::QUEUEZ_FAMILY_ACCOUNT {
                                                                                family4_version = d1_bap::queuez_span_version(&body);
                                                                                family4_span = Some(body.clone());
                                                                            }
                                                                            server_seq += 1;
                                                                        }
                                                                    }
                                                                    Err(e) => warn!("      queuez seal failed: {e}"),
                                                                }
                                                                } // for family
                                                            }

                                                            // ── 100 / 102: the activity-host notification PUSHES ────────
                                                            //
                                                            // ⭐ THE REFUSAL IS THE DELIVERABLE HERE, and it is why these
                                                            // are wired at all rather than left as a TODO.
                                                            //
                                                            //  * **102 is not pushable.** It is absent from the client's
                                                            //    13-entry accept-set at `0x1925A90`. The brief's
                                                            //    "100/102 - we push" is half wrong and the wrong half
                                                            //    kills the connection.
                                                            //  * **100 IS on the accept-set and we still cannot send it.**
                                                            //    It is header class **6 = 32 bytes**, a layout nobody has
                                                            //    read. A 6-byte header there does not error - it parks the
                                                            //    client in its resumable header read (`0xB2ED40`) with no
                                                            //    timeout, no log and no error branch, which is
                                                            //    indistinguishable from "the game never got there".
                                                            //
                                                            // Both bodies encode correctly today; only the headers block
                                                            // them. `D1_AH_PUSH_100=1` / `D1_AH_PUSH_102=1` make the
                                                            // attempt so the refusal is a NAMED GATE in the run log with
                                                            // the exact next step, instead of silence in a design doc.
                                                            if !ah_push_attempted
                                                                && (env_flag("D1_AH_PUSH_100", false)
                                                                    || env_flag("D1_AH_PUSH_102", false))
                                                            {
                                                                ah_push_attempted = true;
                                                                if env_flag("D1_AH_PUSH_100", false) {
                                                                    let n = d1_bap::activity_host::ActivityHostToClientNotification {
                                                                        recipient_id: env_soid("D1_AH_RECIPIENT",
                                                                                               env_soid("D1_ACCOUNT_SOID", 0xD100_0000_0000_0001)),
                                                                        // 10 is what the message-7 handler injects; `[U]`
                                                                        // otherwise, and not a BAP question - the switch
                                                                        // lives behind the activity-client messaging vtable.
                                                                        user_defined_type: env_num::<u32>("D1_AH_UDT", 10),
                                                                        payload: hex_env("D1_AH_PAYLOAD"),
                                                                    };
                                                                    let body = n.encode();
                                                                    match conv.push(TYPE_ACTIVITY_HOST_TO_CLIENT_NOTIFICATION, &body) {
                                                                        Ok(frame) => {
                                                                            let sn = nonces.peek(d1_bap::Direction::ServerToClient);
                                                                            if let Ok(f) = d1_bap::seal_channel_frame(key, sn, &frame) {
                                                                                nonces.advance(d1_bap::Direction::ServerToClient);
                                                                                if sock.write_all(&f).await.is_ok() {
                                                                                    warn!("    --> BAP msg 100 activity_host_to_client_notification \
                                                                                           {}B (udt {}, {}B payload)",
                                                                                          f.len(), n.user_defined_type, n.payload.len());
                                                                                }
                                                                            }
                                                                        }
                                                                        // ✅ Header class 6 is implemented (32 B) as of
                                                                        // 2026-08-08 - see historicaldocs/design/bap-header-classes.md.
                                                                        // A refusal here is now a real surprise.
                                                                        Err(e) => warn!("      ** NAMED GATE ** msg 100 push refused: {e}. \
                                                                                         Body encodes fine ({}B) and header class 6 IS \
                                                                                         implemented, so this is not the old framing gate - \
                                                                                         check is_pushable/is_ours_to_push.", body.len()),
                                                                    }
                                                                }
                                                                if env_flag("D1_AH_PUSH_102", false) {
                                                                    let text = std::env::var("D1_AH_LOG_TEXT")
                                                                        .unwrap_or_else(|_| "d1-responder".into());
                                                                    match d1_bap::activity_host::ActivityHostToClientLogNotification::new(&text) {
                                                                        Some(l) => match conv.push(
                                                                            TYPE_ACTIVITY_HOST_TO_CLIENT_LOG_NOTIFICATION, &l.encode()) {
                                                                            Ok(_) => warn!("      ⛔ msg 102 push was ACCEPTED - that contradicts \
                                                                                            the 13-entry accept-set at 0x1925A90 and is a FINDING, \
                                                                                            not a success. Two instruments disagree."),
                                                                            Err(e) => warn!("      ** NAMED GATE ** msg 102 push refused: {e}. \
                                                                                             Expected: 102 is absent from the accept-set. Where it \
                                                                                             IS legally delivered is [U] - probably an OUT FAH / \
                                                                                             OUT GAH* connection, not this one."),
                                                                        },
                                                                        None => warn!("      D1_AH_LOG_TEXT is longer than 255 bytes"),
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        // A GCM failure here is almost certainly the
                                                        // nonce increment rule, which is `[U]`.
                                                        Err(e) => warn!("      SECURED DECRYPT FAILED: {e} (nonce rule is unconfirmed)"),
                                                    }
                                                    continue;
                                                }

                                                let out: Vec<u8> = if !reply.is_empty() {
                                                    reply.clone()
                                                } else {
                                                    match (d1_bap::parse_envelope(d), d1_bap::parse_inner(d.get(6..).unwrap_or(&[]))) {
                                                        (Ok(env), Ok(inner)) => {
                                                            info!("      envelope v{} mode {} payload {} | inner type 0x{:04x} seq {}",
                                                                  env.version, env.mode, env.payload_size, inner.msg_type, inner.sequence);
                                                            if inner.msg_type == d1_bap::TYPE_CHANNEL_STARTUP_REQUEST
                                                                && std::env::var("D1_BAP_SHELLO").is_err()
                                                            {
                                                                // `c->b start req` (0x001e) -> `c->b start rsp`
                                                                // (0x001f). Unsealed. The sealed secure-hello
                                                                // belongs to the LATER `_authenticating`
                                                                // exchange, answering the client's 0x0019.
                                                                info!("      -> start response (0x001f), seq {}", inner.sequence);
                                                                d1_bap::channel_startup_response(inner.sequence)
                                                            } else if inner.msg_type == d1_bap::TYPE_CHANNEL_STARTUP_REQUEST {
                                                                // Keys MUST match the signon f2.2/f2.3 blobs we sent
                                                                // (both zeros today) or the client's MAC check fails.
                                                                let keys = d1_bap::SealKeys::default();
                                                                let mat = d1_bap::SessionMaterial::default();
                                                                // Fixed IV: determinism over freshness, so two runs diff
                                                                // cleanly. A real server would randomise per message.
                                                                // ⚠ The response sequence is NOT simply the hello's.
                                                                // Echoing 0 gave `failed to find recipient for
                                                                // header [sequence 0][type b->c shello rsp]` - the
                                                                // frame parsed and typed correctly, but no receiver
                                                                // is registered under that sequence. Sweepable
                                                                // without a rebuild while we pin the rule down.
                                                                let seq = std::env::var("D1_BAP_SEQ").ok()
                                                                    .and_then(|v| v.parse().ok())
                                                                    .unwrap_or(inner.sequence.wrapping_add(1));
                                                                info!("      replying with sequence {seq} (hello had {})", inner.sequence);
                                                                d1_bap::secure_hello_response(seq, &keys, &mat, [0x5A; 16])
                                                            } else if inner.msg_type == d1_bap::TYPE_SECURE_HELLO_REQUEST {
                                                                // `_authenticating`: the client hands back the
                                                                // security token we issued in signon f2.4, so the
                                                                // whole chain signon -> BAP -> auth is one thread.
                                                                // Keys are the signon f2.2/f2.3 blobs (zeros today).
                                                                let keys = d1_bap::SealKeys::default();
                                                                let mat = d1_bap::SessionMaterial::default();
                                                                info!("      -> secure hello response (0x001a), seq {}", inner.sequence);
                                                                // This is the message that establishes the channel,
                                                                // so latch the material it carries.
                                                                chan = Some((mat.session_key,
                                                                             d1_bap::ChannelNonces::from_seed(mat.channel_seed)));
                                                                d1_bap::secure_hello_response(inner.sequence, &keys, &mat, [0x5A; 16])
                                                            } else {
                                                                warn!("      no handler for inner type 0x{:04x} - NAMED GATE", inner.msg_type);
                                                                Vec::new()
                                                            }
                                                        }
                                                        (Err(e), _) => { warn!("      envelope parse failed: {e}"); Vec::new() }
                                                        (_, Err(e)) => { warn!("      inner parse failed: {e}"); Vec::new() }
                                                    }
                                                };
                                                if !out.is_empty() {
                                                    match sock.write_all(&out).await {
                                                        Ok(()) => warn!("    --> BAP REPLY {}B: {}",
                                                                        out.len(), hexdump(&out, 96)),
                                                        Err(e) => warn!("    BAP reply write failed: {e}"),
                                                    }
                                                }
                                                } // while pending - next complete frame
                                            }
                                            Ok(Err(e)) => { warn!("    BAP TCP read error: {e}"); return }
                                        }
                                    }
                                });
                            }
                            Err(e) => { warn!("bap tcp accept: {e}"); return }
                        }
                    }
                }
                Err(e) => warn!("bind tcp {addr} failed: {e}"),
            }
        }));
    }

    if let (Some(cert), Some(key)) = (args.cert.clone(), args.key.clone()) {
        for p in args.tls_ports.split(',').filter(|s| !s.trim().is_empty()) {
            let port: u16 = p.trim().parse().with_context(|| format!("bad tls port {p}"))?;
            let addr: SocketAddr = format!("{}:{port}", args.bind).parse()?;
            let app = router(cfg.clone());
            let (cert, key) = (cert.clone(), key.clone());
            tasks.push(tokio::spawn(async move {
                match axum_server::tls_rustls::RustlsConfig::from_pem_file(&cert, &key).await {
                    Ok(tls) => {
                        info!("listening https://{addr}");
                        let _ = axum_server::bind_rustls(addr, tls)
                            .serve(app.into_make_service())
                            .await;
                    }
                    Err(e) => warn!("tls setup failed on {addr}: {e}"),
                }
            }));
        }
    }

    for t in tasks {
        let _ = t.await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⭐⭐ **A COMPILE-TIME GUARD ON THE `tls12` FEATURE. Do not turn this into a comment.**
    ///
    /// The 2016 PS4 client speaks **TLS 1.2**. `rustls` 0.23 gates TLS 1.2 behind the `tls12`
    /// feature, and `rustls::version::TLS12` is `#[cfg(feature = "tls12")]` - so if that feature is
    /// ever dropped from `Cargo.toml`, **this test fails to COMPILE.** That is the point: an
    /// assertion that only fails at runtime would never run, because the failure mode is a
    /// handshake rejection inside `axum-server` that we never see.
    ///
    /// ## Why prose was not enough - this is `status/LESSONS.md` #8, third time
    ///
    /// The feature was originally present only by **feature unification**: `d1-forge` pulls `ureq`,
    /// which enables `rustls/tls12` workspace-wide. So `cargo build --release` shipped a working
    /// listener and `cargo build --release -p d1-responder` shipped a **silently broken** one -
    /// same source, same tests green, different binary. Declaring the feature fixed it; nothing
    /// then *guarded* it, and a 20-line comment is exactly what failed to protect
    /// `D1_MANIFEST_FILE` twice. **A lesson without a guard is a lesson you pay for again.**
    ///
    /// TLS 1.3 is the positive control: naming both proves the test can distinguish them rather
    /// than passing vacuously.
    #[test]
    fn tls12_is_compiled_in_and_tls13_is_the_control() {
        assert_eq!(rustls::version::TLS12.version, rustls::ProtocolVersion::TLSv1_2);
        assert_eq!(rustls::version::TLS13.version, rustls::ProtocolVersion::TLSv1_3);
        assert_ne!(rustls::version::TLS12.version, rustls::version::TLS13.version);
    }

    fn read_varint(b: &[u8], i: &mut usize) -> u64 {
        let (mut v, mut sh) = (0u64, 0);
        loop {
            let c = b[*i];
            *i += 1;
            v |= ((c & 0x7f) as u64) << sh;
            sh += 7;
            if c & 0x80 == 0 {
                return v;
            }
        }
    }

    /// Walk a protobuf message, returning (field, payload-or-varint) pairs.
    fn fields(b: &[u8]) -> Vec<(u32, Result<u64, Vec<u8>>)> {
        let (mut i, mut out) = (0usize, Vec::new());
        while i < b.len() {
            let tag = read_varint(b, &mut i);
            let (f, w) = ((tag >> 3) as u32, tag & 7);
            match w {
                0 => {
                    let v = read_varint(b, &mut i);
                    out.push((f, Ok(v)));
                }
                2 => {
                    let n = read_varint(b, &mut i) as usize;
                    out.push((f, Err(b[i..i + n].to_vec())));
                    i += n;
                }
                _ => panic!("unexpected wire type {w} on field {f}"),
            }
        }
        out
    }

    /// ⚠ The regression this exists for: **`f2.6` and `f14` do NOT share a byte order.**
    ///
    /// Packing `bap_server_ip` big-endian (`u32::from(ip)`) made the client dial **2.0.77.10** -
    /// 10.77.0.2 reversed - and fail with `error.no_bap_connection`, while `client_external_ip` in
    /// the very same response rendered correctly as 10.77.0.2. Both are `uint32` addresses in the
    /// same message, so nothing about the schema warns you. Assert both render 10.77.0.2 through
    /// their own decoding path.
    /// ⭐ The startup guard's arithmetic, asserted in **both** directions - a check that can only
    /// pass is decoration. 503's argument bitstream carries the shared `0x80801941` from byte 21,
    /// 205's body from byte 1, because the struct sits at C `+0x018` in one and `+0x008` in the
    /// other.
    #[test]
    fn shared_struct_guard_discriminates_a_205_503_disagreement() {
        // Positive: the two matched pairs agree.
        assert_eq!(
            shared_0x80801941_agrees(
                d1_bap::INVESTMENT_503_PINNED_RAW,
                &d1_bap::investment_205_response_body()
            ),
            Some(true),
            "the pinned pair must agree"
        );
        assert_eq!(
            shared_0x80801941_agrees(
                d1_bap::INVESTMENT_503_UNLOCKS_RAW,
                d1_bap::INVESTMENT_205_UNLOCKS_BODY
            ),
            Some(true),
            "the unlock pair must agree"
        );
        // ⭐ Negative: the exact mix-up this guard exists to catch - flags in 503, none in 205.
        assert_eq!(
            shared_0x80801941_agrees(
                d1_bap::INVESTMENT_503_UNLOCKS_RAW,
                &d1_bap::investment_205_response_body()
            ),
            Some(false),
            "a flagged 503 with a flagless 205 MUST be reported as a disagreement"
        );
        // And a body too short to contain the struct is reported as "cannot tell", never as agreement.
        assert_eq!(shared_0x80801941_agrees(&[0u8; 4], &[0u8; 4]), None);
    }

    /// ⭐⭐⭐ **The assertion about the WIRE, not about a constant sitting next to it.**
    ///
    /// `served_503_body()` / `served_205_body()` are the exact expressions the 503 and 205 arms
    /// evaluate. This test asks the question the old guard could not: *what does the responder
    /// actually put on the wire when nobody sets anything?*
    ///
    /// ⚠ It reads its answer through `d1_bap::unlock_flags_in_shared_tail`, i.e. it decodes the
    /// `0x80804681` array rather than comparing hex. Reverting either arm to the flagless body -
    /// the single most likely way this change lands as a no-op - fails here, and so does shipping
    /// eight flags instead of ten.
    ///
    /// ⚠ Env-sensitive by construction, so it must run alone: `cargo test -p d1-responder --
    /// --test-threads=1` is how the suite is invoked. It sets no variables itself; it asserts the
    /// *unset* default, which is the state `run-boot.sh` must now leave them in.
    #[test]
    fn the_served_default_bodies_carry_the_ten_unlock_flags() {
        if std::env::var_os("D1_503_RAW").is_some() || std::env::var_os("D1_205_RAW").is_some() {
            // Never silently pass: a set override means this test cannot see the default.
            panic!("D1_503_RAW/D1_205_RAW are set in the test environment - unset them; this test \
                    exists to assert the DEFAULT, and an override makes it untestable, not green");
        }
        let b503 = served_503_body();
        let b205 = served_205_body();
        assert_eq!(b503.len(), 70, "the served 503 must be the ten-flag body");
        assert_eq!(b205.len(), 50, "the served 205 must be the ten-flag body");
        assert_eq!(
            shared_0x80801941_agrees(&b503, &b205),
            Some(true),
            "the two SERVED bodies must agree over the shared 0x80801941 - 205 lands last, and \
             more than once"
        );
        for (name, tail) in [("503", &b503[21..]), ("205", &b205[1..])] {
            let flags = d1_bap::unlock_flags_in_shared_tail(tail)
                .unwrap_or_else(|| panic!("{name}: served body too short to hold the flag array"));
            assert_eq!(
                flags,
                d1_bap::UNLOCK_FLAG_INDICES.iter().map(|&i| (i, 2u8)).collect::<Vec<_>>(),
                "{name}: the served body must decode to the ten indices at value 2"
            );
        }
        // ⭐ The negative that names the retired poke: 2667 and 5987 are the two the union-8 body
        // lacked, and the only reason rung 33's permissions wall ever needed a d1guest write.
        let flags = d1_bap::unlock_flags_in_shared_tail(&b205[1..]).unwrap();
        for urchin in [2667u16, 5987] {
            assert!(
                flags.iter().any(|&(i, _)| i == urchin),
                "{urchin} must be on the WIRE - without it the run is back to poking \
                 [0x26A34E8]+0xC+{urchin} = 0x02"
            );
        }
        // …and the superseded union-8 body must NOT be what we serve.
        assert_ne!(b503.as_slice(), d1_bap::INVESTMENT_503_UNLOCKS_RAW_V2_UNION8);
        assert_ne!(b205.as_slice(), d1_bap::INVESTMENT_205_UNLOCKS_BODY_V2_UNION8);
        assert_ne!(b205.as_slice(), d1_bap::investment_205_response_body().as_slice());
    }

    #[test]
    fn bap_ip_and_client_ip_use_opposite_byte_orders() {
        let ip: std::net::Ipv4Addr = "10.77.0.2".parse().unwrap();
        let cfg = SignonCfg {
            silo_id: 1,
            environment: "live".into(),
            bap_ip: ip,
            bap_port: 3075,
            client_ip: ip,
            manifest_url: String::new(), // omit f4 so the walk stays flat
        };
        let msg = signon_success(Some("b"), &cfg);
        let top = fields(&msg);

        // f14 client_external_ip: BIG-endian packed. Observed rendering correctly in the STUN log.
        let f14 = top.iter().find(|(f, _)| *f == 14).expect("f14 present");
        let v14 = *f14.1.as_ref().expect("f14 is a varint") as u32;
        assert_eq!(
            std::net::Ipv4Addr::from(v14.to_be_bytes()),
            ip,
            "f14 must decode big-endian to 10.77.0.2"
        );

        // f2.6 bap_server_ip: LITTLE-endian packed. Measured from the address the client dialled.
        let f2 = top.iter().find(|(f, _)| *f == 2).expect("f2 present");
        let inner = fields(f2.1.as_ref().unwrap_err());
        let sub6 = inner.iter().find(|(f, _)| *f == 6).expect("f2.6 present");
        let v6 = *sub6.1.as_ref().expect("f2.6 is a varint") as u32;
        assert_eq!(
            std::net::Ipv4Addr::from(v6.to_le_bytes()),
            ip,
            "f2.6 must decode little-endian to 10.77.0.2 - big-endian made the client dial 2.0.77.10"
        );
        assert_eq!(v6, 0x0200_4D0A, "the on-wire integer for 10.77.0.2 in f2.6");
        assert_ne!(v6, v14, "the two address fields must not be packed identically");
    }

    /// All seven `f2.1`-`f2.7` are required with no default; omitting one yields
    /// `missing required field`, and omitting field 2 entirely yields the misleading
    /// `Unable to parse signon response`.
    #[test]
    fn success_response_carries_all_seven_required_subfields() {
        let cfg = SignonCfg {
            silo_id: 12345,
            environment: "live".into(),
            bap_ip: "10.77.0.2".parse().unwrap(),
            bap_port: 3075,
            client_ip: "10.77.0.2".parse().unwrap(),
            manifest_url: "http://cdn.deadorbit.net/manifest.bin".into(),
        };
        let msg = signon_success(Some("52553.16.09.28.1551.evil"), &cfg);
        let top = fields(&msg);
        let f2 = top.iter().find(|(f, _)| *f == 2).expect("field 2 present");
        let present: Vec<u32> = fields(f2.1.as_ref().unwrap_err()).iter().map(|(f, _)| *f).collect();
        for need in 1..=7u32 {
            assert!(present.contains(&need), "f2.{need} missing");
        }
        assert_eq!(top.iter().find(|(f, _)| *f == 1).unwrap().1.as_ref().ok(), Some(&0u64),
                   "response_type must be 0 (success); 1 is the queue path, 2 is denial");
    }

    // ── the correlation table, as wired here ─────────────────────────────────────────────────

    /// ⚠ The body of an inbound message does **not** always start at `+6`.
    ///
    /// Every request this responder answered before the activity-host lane was header class 1, so
    /// the hard-coded `&plain[6..]` was right by accident. Message **8** is class 3 = 14 bytes; a
    /// silent 8-byte offset error there would read as a codec bug in `d1-bap`, not as a bug here.
    #[test]
    fn message_8_body_does_not_start_at_plus_six() {
        assert_eq!(bap_body_offset(TYPE_CLIENT_TO_ACTIVITY_HOST_NOTIFICATION), 14);
        // Positive control on the same lookup: the class-1 requests we already answer.
        for id in [
            TYPE_ACTIVITY_HOST_MANAGER_REQUEST,
            TYPE_GET_ACTIVITY_HOST_PROXY_REQUEST,
            TYPE_MATCHMAKING_CORE_REQUEST,
            d1_bap::TYPE_CLIENT_TO_WORLD_SERVER_REQUEST,
            d1_bap::TYPE_CLIENT_TO_BAP_SUBSCRIPTION_REQUEST,
        ] {
            assert_eq!(bap_body_offset(id), 6, "id {id} is header class 1");
        }
        // …and the class-2 replies we write are 8, which is what `split_reply_frame` strips.
        assert_eq!(bap_body_offset(TYPE_ACTIVITY_HOST_MANAGER_RESPONSE), 8);
        assert_eq!(bap_body_offset(TYPE_GET_ACTIVITY_HOST_PROXY_RESPONSE), 8);
        assert_eq!(bap_body_offset(TYPE_MATCHMAKING_CORE_RESPONSE), 8);
    }

    /// ⭐ The check that catches a builder drifting from the client's policy table.
    ///
    /// `split_reply_frame` strips a header it did not write, so it verifies it first. A type or
    /// sequence mismatch has exactly one symptom on the wire -
    /// `_connection_failure_failed_to_find_message_recipient`, the Weasel - and no other evidence.
    #[test]
    fn a_frame_that_does_not_match_its_ticket_is_refused() {
        let mut conv = d1_bap::BapConversation::with_default_deadline();
        let ticket = match conv.enqueue(7, d1_bap::TYPE_CLIENT_TO_BAP_SUBSCRIPTION_REQUEST) {
            d1_bap::EnqueueOutcome::Awaiting(t) => t,
            other => panic!("expected Awaiting, got {other:?}"),
        };
        // Positive control: the builder that actually answers 0x000C round-trips.
        let good = d1_bap::client_to_bap_subscription_response(7);
        assert_eq!(split_reply_frame(&ticket, &good), Some((d1_bap::STATUS_OK, Vec::new())));
        // Wrong sequence - the rung-28 killer, one layer before the socket.
        let wrong_seq = d1_bap::client_to_bap_subscription_response(8);
        assert_eq!(split_reply_frame(&ticket, &wrong_seq), None);
        // Wrong response type for this request.
        let wrong_type = d1_bap::reply_header(d1_bap::TYPE_CLIENT_TO_WORLD_SERVER_RESPONSE, 7);
        assert_eq!(split_reply_frame(&ticket, &wrong_type), None);
        // Truncated.
        assert_eq!(split_reply_frame(&ticket, &good[..4]), None);
    }

    /// The body survives the header strip byte for byte - otherwise `drain` would re-emit a
    /// truncated or shifted payload and every downstream decode would fail for the wrong reason.
    #[test]
    fn the_body_survives_the_header_strip() {
        let mut conv = d1_bap::BapConversation::with_default_deadline();
        let ticket = match conv.enqueue(41, d1_bap::TYPE_CLIENT_TO_WORLD_SERVER_REQUEST) {
            d1_bap::EnqueueOutcome::Awaiting(t) => t,
            other => panic!("expected Awaiting, got {other:?}"),
        };
        let frame =
            d1_bap::investment_message_response_with_body(41, 206, d1_bap::STATUS_OK, &[0x10]);
        let (status, body) = split_reply_frame(&ticket, &frame).expect("matches its ticket");
        assert_eq!(status, d1_bap::STATUS_OK);
        // The echoed u16be message id, then the body - everything after the 8-byte header.
        assert_eq!(body, vec![0x00, 0xCE, 0x10]);
        // …and re-attaching the header the conversation writes reproduces the original frame.
        conv.park(ticket, body).unwrap();
        assert_eq!(conv.drain(std::time::Instant::now())[0], frame);
    }

    /// ⚠ A knob that is read but never printed is a run whose log does not carry its own
    /// provenance - the exact failure that made an unrelated experiment read as a regression in the
    /// patch under test. This is the control that stops the table going stale.
    #[test]
    fn every_knob_the_source_reads_is_printed_at_startup() {
        let src = include_str!("main.rs");
        let mut missing: Vec<String> = Vec::new();
        let mut i = 0usize;
        while let Some(p) = src[i..].find("\"D1_") {
            let start = i + p + 1;
            let end = start + src[start..].find('"').expect("closing quote");
            let name = &src[start..end];
            i = end;
            // The per-id escape hatch is a format string, and `print_knobs` reports whichever of
            // them are actually set rather than tabulating an open-ended family.
            // Skip: the per-id escape hatch (a format string, reported separately by
            // `print_knobs`), and this test's own scan literals.
            let well_formed = name.len() > 3
                && name.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
                && !name.ends_with('_');
            if !well_formed || name.starts_with("D1_INV_BODY") {
                continue;
            }
            if !KNOBS.iter().any(|(k, _, _)| *k == name) && !missing.iter().any(|m| m == name) {
                missing.push(name.to_string());
            }
        }
        assert!(missing.is_empty(), "knobs read but never printed at startup: {missing:?}");
        // Positive control on the same scan: it does find things.
        assert!(src.contains("\"D1_BAP_FIFO\""), "the scan must be looking at this file");
        assert!(KNOBS.len() > 40, "the table should cover the whole surface, not a sample");
    }

    /// The engine-config delta is fixed32-framed, not varint - the trap that made an earlier doc say
    /// "do not emit f4.3". Magic and key are raw u32 LE; only the string is LEB128-prefixed.
    #[test]
    fn client_config_delta_is_fixed32_framed() {
        let url = "http://cdn.deadorbit.net/manifest.bin";
        let blob = client_config_delta(url, Some("52553.16.09.28.1551.evil"));
        assert_eq!(&blob[0..4], &0x0012_3456u32.to_le_bytes(), "magic, u32 LE");
        assert_eq!(&blob[4..8], &100u32.to_le_bytes(), "key 100 = content manifest URL, u32 LE");
        assert_eq!(blob[8], 37, "LEB128 string length");
        assert_eq!(&blob[blob.len() - 4..], &[0, 0, 0, 0], "u32 terminator");
        // 01.29 (and an absent build) get NO extra key: unknown keys abort the whole signon parse.
        assert_eq!(blob.len(), 4 + 4 + 1 + 37 + 4);
        assert_eq!(client_config_delta(url, None), blob);
        // 56578 additionally gets the sparse fallback key 0xF1048 with a one-byte bool 1.
        let later = client_config_delta(url, Some("56578.17.04.12.1251.evil"));
        assert_eq!(&later[..46], &blob[..46], "magic + key 100 unchanged");
        assert_eq!(&later[46..50], &0x000F_1048u32.to_le_bytes());
        assert_eq!(later[50], 1);
        assert_eq!(&later[51..], &[0, 0, 0, 0]);
    }
}

/// Read a SOID from the environment (decimal or `0x`-prefixed), falling back to `default`.
///
/// SOIDs must be **stable across runs** - the client checks the object-header SOID against the one
/// serialized inside the RSAT payload, and downstream state is keyed on them.
fn env_soid(var: &str, default: u64) -> u64 {
    std::env::var(var)
        .ok()
        .and_then(|v| {
            let v = v.trim().to_string();
            v.strip_prefix("0x")
                .map(|h| u64::from_str_radix(h, 16))
                .unwrap_or_else(|| v.parse())
                .ok()
        })
        .unwrap_or(default)
}

/// `D1_LOOT_GRANT=NONCE=SOID:FILE[|PROFILE][;…]`: the item a claimed world drop delivers. `NONCE`
/// is the pending-loot nonce id the 601 claim names, `SOID` the new item instance, `FILE` its 4/3
/// inventory element as `d1-oracle object --tag 0x80801732` encodes it, and `PROFILE` (optional)
/// the character profile after the pickup, delivered in the same delta.
fn loot_grant_for(nonce: u64) -> Option<(u64, Vec<u8>, Option<Vec<u8>>)> {
    let spec = std::env::var("D1_LOOT_GRANT").ok()?;
    let num = |t: &str| -> Option<u64> {
        let t = t.trim();
        t.strip_prefix("0x").map_or_else(|| t.parse().ok(), |h| u64::from_str_radix(h, 16).ok())
    };
    spec.split(';').find_map(|entry| {
        let (n, rest) = entry.split_once('=')?;
        let (soid, file) = rest.split_once(':')?;
        if num(n)? != nonce {
            return None;
        }
        let (file, profile) = match file.split_once('|') {
            Some((f, p)) => (f.trim(), Some(p.trim())),
            None => (file.trim(), None),
        };
        let read = |f: &str| match std::fs::read(f) {
            Ok(b) => Some(b),
            Err(e) => {
                warn!("      ⛔ D1_LOOT_GRANT {f}: {e}");
                None
            }
        };
        let element = read(file)?;
        let profile = match profile {
            Some(p) => Some(read(p)?),
            None => None,
        };
        Some((num(soid)?, element, profile))
    })
}

/// `D1_LOOT_GRANT_ROSTER=<dir>`: the roster fixtures after a loot grant (`d1-oracle character
/// build-peer` output, `f<family>t<type>-<key>.bin`). Each family's type-1 card is pushed as a
/// version-1 delta on top of the version-0 roster baseline, so the client sees the new equipment.
fn roster_grant_deltas(root_soid: u64) -> Vec<(u32, Vec<u8>)> {
    let Some(dir) = std::env::var("D1_LOOT_GRANT_ROSTER").ok().filter(|d| !d.is_empty()) else {
        return Vec::new();
    };
    let mut files: Vec<std::path::PathBuf> = match std::fs::read_dir(&dir) {
        Ok(rd) => rd.flatten().map(|e| e.path()).collect(),
        Err(e) => {
            warn!("      ⛔ D1_LOOT_GRANT_ROSTER {dir}: {e}");
            return Vec::new();
        }
    };
    files.sort();
    let mut out = Vec::new();
    for f in files {
        let name = f.file_name().and_then(|s| s.to_str()).unwrap_or_default().to_string();
        let parsed = name
            .strip_suffix(".bin")
            .and_then(|s| s.strip_prefix('f'))
            .and_then(|s| s.split_once('t'))
            .and_then(|(fam, rest)| rest.split_once('-').map(|(ty, key)| (fam, ty, key)))
            .and_then(|(fam, ty, key)| {
                Some((fam.parse::<u32>().ok()?, ty.parse::<u32>().ok()?, u64::from_str_radix(key, 16).ok()?))
            });
        let Some((fam, ty, key)) = parsed else { continue };
        if ty != 1 {
            continue;
        }
        let Some(&(_, instance)) = ROSTER_INSTANCE_TYPES.iter().find(|&&(k, _)| k == (fam, ty)) else {
            continue;
        };
        let Ok(payload) = std::fs::read(&f) else { continue };
        let mut b = d1_bap::queuez_family_header_versioned(fam, root_soid, 1, 1, d1_bap::QUEUEZ_FLAG_DELTA);
        b.extend_from_slice(&instance.to_be_bytes());
        b.extend_from_slice(&key.to_be_bytes());
        b.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        b.extend_from_slice(&payload);
        out.push((fam, b));
    }
    out
}

/// Read a hex-string payload from the environment (empty if unset/invalid).
///
/// Lets a payload authored by the client's own RSAT encoder be dropped in without a rebuild.
fn hex_env(var: &str) -> Vec<u8> {
    let Ok(v) = std::env::var(var) else { return Vec::new() };
    let v: String = v.chars().filter(|c| !c.is_whitespace()).collect();
    (0..v.len() / 2)
        .map(|i| u8::from_str_radix(&v[i * 2..i * 2 + 2], 16))
        .collect::<Result<Vec<u8>, _>>()
        .unwrap_or_default()
}

#[cfg(test)]
mod character_lifecycle_dispatch_tests {
    use super::*;

    /// ⭐ **504 must NOT be double-handled.** It is on the proven route to orbit and has had its own
    /// branch since 2026-08-07; routing it through `character_lifecycle_reply` would change a body
    /// that already works. This arm returns before any env read, so the assertion holds regardless
    /// of how the process is configured - which is what makes it a control rather than a mood.
    #[test]
    fn the_lifecycle_dispatch_never_claims_504_or_anything_else() {
        for id in [
            d1_bap::character_lifecycle::LOGIN_CHARACTER, // 504 - the one that must fall through
            104, 205, 206, 503, 601, 701, 702, 0, 1, 65535,
        ] {
            assert!(
                character_lifecycle_reply(id, &[0u8; 64], &[0u8; 66]).is_none(),
                "investment {id} must fall through to the existing dispatch"
            );
        }
    }

    /// The zero-character predicate, which is the whole trigger for the character-creation route.
    ///
    /// ⚠ Reads `D1_EMPTY_ROSTER`, so it asserts only the branch that does not depend on it: a
    /// non-zero SOID with no created character must NEVER report "no characters" - that is the
    /// direction a regression would take the proven route to orbit.
    #[test]
    fn a_pinned_character_soid_is_never_reported_as_zero_characters() {
        CHARACTER_CREATED.store(false, std::sync::atomic::Ordering::Relaxed);
        if std::env::var("D1_EMPTY_ROSTER").map(|v| v != "0").unwrap_or(false) {
            return; // the operator asked for an empty roster; nothing to assert
        }
        assert!(!serves_no_characters(0xD100_0000_0000_0101));
        assert!(serves_no_characters(0), "chr == 0 means NO characters");
    }

    /// Once we have answered a 501, the SOID we minted is what gets served - not the pinned one.
    #[test]
    fn a_created_character_soid_wins() {
        CREATED_CHARACTER_SOID.store(0, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(effective_character_soid(7), 7);
        CREATED_CHARACTER_SOID.store(0xD100_0000_0000_0101, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(effective_character_soid(7), 0xD100_0000_0000_0101);
        assert_eq!(effective_character_soid(0), 0xD100_0000_0000_0101);
        CREATED_CHARACTER_SOID.store(0, std::sync::atomic::Ordering::Relaxed);
    }
}
