// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! MP(H): the responder's side of the multiplayer services (`d1-mp`), behind **`D1_MP=1`**.
//!
//! Off (the default), nothing here runs and every answer is the single-player one it was. On, one
//! [`d1_mp::Services`] is shared by every connection:
//!
//! * **BAP 23** (account-id translation): each platform id gets its own account SOID, not
//!   `D1_ACCOUNT_SOID` for everyone. The first id a connection asks about is taken as its own
//!   and signs it on `[I]`: a client translates itself before anyone else, and no other sign-on
//!   message names the online id on this connection.
//! * **BAP 42/43** (matchmaking): searches, advertisements and deletes go to the shared directory,
//!   so one client finds another's session; kind 4 answers from `D1_MM_CONFIG_FILE` if set.
//! * **The PSN Session WebAPI**: each `POST /v1/sessions` gets its own id and record, and the
//!   members, `sessionData`, invitation and presence routes are served from the store.
//! * A closed BAP connection signs its player off ([`ConnGuard`]).
//!
//! Every hook in `main.rs` is one line marked `MP(H)`.

use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use axum::{
    body::Bytes,
    extract::Path as AxPath,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use d1_mp::identity::{ConnectionId, OnlineId};
use d1_mp::Services;
use tracing::{info, warn};

static SERVICES: OnceLock<Option<Mutex<Services>>> = OnceLock::new();
static EPOCH: OnceLock<Instant> = OnceLock::new();

fn now_ms() -> u64 {
    EPOCH.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// Read `D1_MP` once and build the services. `npid`/`account` bind the responder's own account.
pub fn init(npid: &str, account: u64) {
    SERVICES.get_or_init(|| {
        if std::env::var("D1_MP").map(|v| v.trim() == "1").unwrap_or(false) {
            match Services::from_env(Some((npid, account))) {
                Ok(mut s) => {
                    // One identity scheme: with the player store on, an online id's account is the
                    // one `identities.json` files it under. The responder's own npid is the store's
                    // player (`D1_STORE_PLAYER`, "local"), filed first so it keeps the knobs.
                    if let Some(cfg) = crate::player_store::Config::from_env() {
                        let first = crate::player_store::Identity::from_env();
                        if let Err(e) = crate::player_store::identity_for(&cfg.dir, &cfg.player, first) {
                            warn!("⚠ MP: player store identity for '{}': {e:#}", cfg.player);
                        }
                        s.identity.set_resolver(d1_mp::identity::Resolver(std::sync::Arc::new(move |id: &OnlineId| {
                            match crate::player_store::identity_for(&cfg.dir, id.as_str(), first) {
                                Ok(i) => Some(d1_mp::identity::AccountSoid(i.account)),
                                Err(e) => {
                                    warn!("⚠ MP: player store identity for '{id}': {e:#}; allocating in memory");
                                    None
                                }
                            }
                        })));
                        warn!("⭐ MP: accounts resolve through the player store's identities.json");
                    }
                    // Stream O2: the activity framework's playlists (the host's data: this
                    // process reads the table generated from it beside the definitions).
                    match playlists_path().map(|p| (std::fs::read_to_string(&p), p)) {
                        Some((Ok(text), p)) => match d1_mp::matchmaking::parse_playlists(&text) {
                            Ok(list) => {
                                let n = list.len();
                                for pl in list {
                                    s.matchmaker.add_playlist(pl);
                                }
                                warn!("⭐ MP: {n} matchmaking playlists from {}", p.display());
                            }
                            Err(e) => warn!("⚠ MP: {}: {e}; no playlists", p.display()),
                        },
                        Some((Err(e), p)) => warn!("⚠ MP: {}: {e}; no playlists", p.display()),
                        None => {}
                    }
                    warn!("⭐ D1_MP=1: multiplayer services on (identities, fireteams, matchmaking directory, platform sessions); \
                           '{npid}' is account {account:#018x}");
                    Some(Mutex::new(s))
                }
                Err(e) => {
                    warn!("⛔ D1_MP=1 but the services did not configure ({e}); serving single-player");
                    None
                }
            }
        } else {
            None
        }
    });
}

/// Stream O2: where the matchmaker's playlists come from: `matchmaking-playlists.tsv` in the
/// activity framework's folder (`D1_AH_ACTIVITY_FRAMEWORK`, a folder, or `1` for this checkout's
/// `ps4/fixtures/56578/activities`). None when the framework is off.
pub fn playlists_path() -> Option<std::path::PathBuf> {
    playlists_path_for(std::env::var("D1_AH_ACTIVITY_FRAMEWORK").ok().as_deref())
}

fn playlists_path_for(knob: Option<&str>) -> Option<std::path::PathBuf> {
    let v = knob.map(str::trim).filter(|v| !v.is_empty() && *v != "0")?;
    let dir = if v == "1" {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/56578/activities")
    } else {
        std::path::PathBuf::from(v)
    };
    Some(dir.join("matchmaking-playlists.tsv"))
}

pub fn enabled() -> bool {
    SERVICES.get().is_some_and(Option::is_some)
}

fn with<R>(f: impl FnOnce(&mut Services) -> R) -> Option<R> {
    let m = SERVICES.get()?.as_ref()?;
    let mut s = m.lock().unwrap_or_else(|e| e.into_inner());
    Some(f(&mut s))
}

/// Signs the connection's player off when the BAP connection task ends, however it ends.
pub struct ConnGuard(pub u64);

impl Drop for ConnGuard {
    fn drop(&mut self) {
        let conn = ConnectionId(self.0);
        crate::players::forget(self.0); // stream O3
        with(|s| {
            if let Some(who) = s.identity.session(conn).map(|w| w.online_id.clone()) {
                s.sign_off(conn, now_ms());
                warn!("⭐ MP: BAP connection #{} closed: '{who}' signed off", self.0);
            }
        });
    }
}

/// BAP 23: `None` when MP is off (the caller keeps its single-account answer).
pub fn translate(conn: u64, ids: &[[u8; 36]]) -> Option<Vec<([u8; 36], u64)>> {
    with(|s| {
        let c = ConnectionId(conn);
        if s.identity.session(c).is_none() {
            if let Some(me) = ids.first().and_then(|p| OnlineId::from_padded(p)) {
                match s.sign_on(c, &me, now_ms()) {
                    Ok((account, fireteam)) => {
                        // Stream O3: the connection's whole identity (account, character, profile),
                        // so everything it is served is its own (players.rs).
                        let id = crate::players::identity_for(me.as_str(), account.0);
                        crate::players::register(conn, id);
                        warn!("⭐ MP: connection #{conn} is '{me}', {account}, in {fireteam}: character {:#018x}, profile {:#018x}",
                              id.character, id.profile);
                    }
                    Err(e) => warn!("⚠ MP: connection #{conn} could not sign on as '{me}': {e}"),
                }
            }
        }
        let out = s.translate_accounts(ids);
        for (p, soid) in &out {
            info!("      MP: '{}' -> account {soid:#018x}", OnlineId::from_padded(p).map(|i| i.to_string()).unwrap_or_default());
        }
        out
    })
}

/// BAP 42: the 43 body, or `None` when MP is off or the request did not decode (the caller then
/// answers as before).
pub fn matchmaking_reply(conn: u64, body: &[u8]) -> Option<Vec<u8>> {
    // Stream O2/O3: a public-bubble search (slot 1) places the fireteam in the activity host's
    // public instances (its destination's, emptiest first), holds its slots there, and is answered
    // with that instance's advertisement, or none when a new one starts (public_feed.rs).
    let mut place = |w: &d1_mp::matchmaking::Want, searcher: Option<d1_mp::identity::AccountSoid>| {
        if w.slot != d1_bap::matchmaking_sessions::SLOT_BUBBLE {
            return None;
        }
        crate::public_feed::on_bubble_search_by(w.set_id, w.needed as usize, searcher.map(|a| a.0))
            .map(|accounts| accounts.into_iter().map(d1_mp::identity::AccountSoid).collect())
    };
    with(|s| match s.matchmaking_placed(ConnectionId(conn), body, now_ms(), &mut place) {
        Ok(r) => {
            warn!("      ⭐ MP matchmaking (connection #{conn}): {:?}", r.action);
            Some(r.body)
        }
        Err(e) => {
            warn!("      ⚠ MP matchmaking (connection #{conn}): {e}; answering as before");
            None
        }
    })
    .flatten()
}

/// Who a WebAPI request is from: the bearer token shadNet issued (one per participant). `[U]`
/// whether the client also names its online id; the URL does on the per-user routes.
fn requester(headers: &HeaderMap) -> String {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim_start_matches("Bearer ").trim().to_owned())
        .unwrap_or_else(|| "?".into())
}

fn content_type(headers: &HeaderMap) -> Option<String> {
    headers.get("content-type").and_then(|v| v.to_str().ok()).map(str::to_owned)
}

fn not_found(what: String) -> Response {
    warn!("<-- 404 MP: {what}");
    (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": {"code": 0x8092_0001u32, "message": what}}))).into_response()
}

/// `POST /v1/sessions`: `None` when MP is off.
pub fn create_session(headers: &HeaderMap, body: &Bytes) -> Option<Response> {
    with(|s| match s.sessions.create(&requester(headers), content_type(headers).as_deref(), body) {
        Ok((id, reply)) => {
            warn!("<-- 201 ** MP PSN SESSION CREATED ** {id} ({}B body)", body.len());
            (StatusCode::CREATED, Json(reply)).into_response()
        }
        Err(e) => {
            warn!("<-- 400 MP: POST /v1/sessions: {e}");
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": {"code": 0x8092_0002u32, "message": e.to_string()}}))).into_response()
        }
    })
}

/// `PUT /v1/sessions/{id}`.
pub fn update_session(id: &str, body: &Bytes) -> Option<Response> {
    with(|s| (StatusCode::OK, Json(s.sessions.update(id, body))).into_response())
}

/// `PUT /v1/users/{npid}/presence/gameData`.
pub fn put_game_data(npid: &str, body: &Bytes) -> Option<Response> {
    with(|s| {
        s.sessions.put_game_data(npid, body);
        StatusCode::NO_CONTENT.into_response()
    })
}

/// `DELETE /v1/users/{npid}/presence/gameStatus`.
pub fn delete_game_status(npid: &str) -> Option<Response> {
    with(|s| {
        s.sessions.clear_game_status(npid);
        StatusCode::NO_CONTENT.into_response()
    })
}

pub async fn get_session(AxPath(id): AxPath<String>) -> Response {
    with(|s| match s.sessions.describe(&id) {
        Ok(v) => (StatusCode::OK, Json(v)).into_response(),
        Err(e) => not_found(e.to_string()),
    })
    .unwrap_or_else(|| not_found("D1_MP is off".into()))
}

async fn session_data(AxPath(id): AxPath<String>) -> Response {
    with(|s| match s.sessions.session_data(&id) {
        Ok(d) => {
            info!("<-- 200 MP sessionData {id} ({}B)", d.len());
            (StatusCode::OK, [("content-type", "application/octet-stream")], d.to_vec()).into_response()
        }
        Err(e) => not_found(e.to_string()),
    })
    .unwrap_or_else(|| not_found("D1_MP is off".into()))
}

async fn get_changeable(AxPath(id): AxPath<String>) -> Response {
    with(|s| match s.sessions.changeable_data(&id) {
        Ok(d) => (StatusCode::OK, [("content-type", "application/octet-stream")], d.to_vec()).into_response(),
        Err(e) => not_found(e.to_string()),
    })
    .unwrap_or_else(|| not_found("D1_MP is off".into()))
}

async fn put_changeable(AxPath(id): AxPath<String>, body: Bytes) -> Response {
    with(|s| match s.sessions.put_changeable_data(&id, &body) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => not_found(e.to_string()),
    })
    .unwrap_or_else(|| not_found("D1_MP is off".into()))
}

async fn join_session(AxPath(id): AxPath<String>, headers: HeaderMap, body: Bytes) -> Response {
    // The joiner's online id if the body names one, else its token.
    let who = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("onlineId").and_then(|o| o.as_str()).map(str::to_owned))
        .unwrap_or_else(|| requester(&headers));
    with(|s| match s.sessions.join(&id, &who) {
        Ok(()) => {
            warn!("<-- 200 MP: {who} joined session {id}");
            (StatusCode::OK, Json(serde_json::json!({"sessionId": id}))).into_response()
        }
        Err(e) => not_found(e.to_string()),
    })
    .unwrap_or_else(|| not_found("D1_MP is off".into()))
}

async fn leave_session(AxPath((id, npid)): AxPath<(String, String)>) -> Response {
    with(|s| match s.sessions.leave(&id, &npid) {
        Ok(ended) => {
            warn!("<-- 204 MP: {npid} left session {id}{}", if ended { " (ended)" } else { "" });
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => not_found(e.to_string()),
    })
    .unwrap_or_else(|| not_found("D1_MP is off".into()))
}

async fn invite(AxPath(id): AxPath<String>, headers: HeaderMap, body: Bytes) -> Response {
    let from = requester(&headers);
    let ct = content_type(&headers);
    with(|s| match s.sessions.invite(&id, &from, ct.as_deref(), &body) {
        Ok(ids) => {
            warn!("<-- 200 MP: invitation(s) {ids:?} for session {id}");
            (StatusCode::OK, Json(serde_json::json!({"invitationId": ids.first()}))).into_response()
        }
        Err(e) => not_found(e.to_string()),
    })
    .unwrap_or_else(|| not_found("D1_MP is off".into()))
}

async fn user_sessions(AxPath(npid): AxPath<String>) -> Response {
    with(|s| (StatusCode::OK, Json(s.sessions.sessions_of(&npid))).into_response()).unwrap_or_else(|| not_found("D1_MP is off".into()))
}

async fn user_invitations(AxPath(npid): AxPath<String>) -> Response {
    with(|s| (StatusCode::OK, Json(s.sessions.invitations_for(&npid))).into_response()).unwrap_or_else(|| not_found("D1_MP is off".into()))
}

async fn invitation_data(AxPath((npid, inv)): AxPath<(String, String)>) -> Response {
    with(|s| match s.sessions.invitation_data(&npid, &inv) {
        Ok(d) => (StatusCode::OK, [("content-type", "application/octet-stream")], d).into_response(),
        Err(e) => not_found(e.to_string()),
    })
    .unwrap_or_else(|| not_found("D1_MP is off".into()))
}

/// The routes only MP serves. Added only when `D1_MP=1`, so with it off these stay the logged
/// 404 gates they were.
pub fn add_routes<S: Clone + Send + Sync + 'static>(r: Router<S>) -> Router<S> {
    if !enabled() {
        return r;
    }
    r.route("/v1/sessions/{id}/sessionData", get(session_data))
        .route("/v1/sessions/{id}/changeableSessionData", get(get_changeable).put(put_changeable))
        .route("/v1/sessions/{id}/members", post(join_session))
        .route("/v1/sessions/{id}/members/{npid}", axum::routing::delete(leave_session))
        .route("/v1/sessions/{id}/invitations", post(invite))
        .route("/v1/users/{npid}/sessions", get(user_sessions))
        .route("/v1/users/{npid}/invitations", get(user_invitations))
        .route("/v1/users/{npid}/invitations/{inv}/invitationData", get(invitation_data))
        .route("/v1/sessions/{id}/sessionImage", put(|| async { StatusCode::NO_CONTENT }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With D1_MP unset every hook declines, so the single-player answers stand. (Runs in its
    /// own process state: `init` is never called here, so the services are absent.)
    #[test]
    fn the_committed_playlists_load_into_the_matchmaker() {
        assert_eq!(playlists_path_for(None), None);
        assert_eq!(playlists_path_for(Some("0")), None);
        let p = playlists_path_for(Some("1")).unwrap();
        let list = d1_mp::matchmaking::parse_playlists(&std::fs::read_to_string(&p).unwrap()).unwrap();
        let n = list.len();
        assert!(n > 0);
        let coop = list.iter().find(|p| matches!(p.kind, d1_mp::matchmaking::PlaylistKind::Cooperative { .. })).unwrap().hash;
        let mut s = d1_mp::Services::default();
        for pl in list {
            s.matchmaker.add_playlist(pl);
        }
        // A cooperative playlist places a fireteam at once: the playlist is known.
        let id = d1_mp::identity::OnlineId::new("o2-test").unwrap();
        s.sign_on(d1_mp::identity::ConnectionId(1), &id, 0).unwrap();
        assert!(s.launch_playlist(d1_mp::identity::ConnectionId(1), coop, None, 0).is_ok(), "{n} playlists");
    }

    #[test]
    fn every_hook_declines_when_mp_is_off() {
        assert!(!enabled());
        assert!(translate(1, &[[0u8; 36]]).is_none());
        assert!(matchmaking_reply(1, &[]).is_none());
        assert!(create_session(&HeaderMap::new(), &Bytes::new()).is_none());
        assert!(update_session("x", &Bytes::new()).is_none());
        drop(ConnGuard(1));
    }
}
