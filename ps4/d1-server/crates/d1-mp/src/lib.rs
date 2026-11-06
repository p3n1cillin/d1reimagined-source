// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! **Multiplayer services for one local server**: several independently connected players, each
//! with a stable identity, in fireteams, matchmaking into activities and public instances, and
//! setting up their peer-to-peer sessions.
//!
//! Every module is a pure state machine: requests in, replies and events out, time passed in as
//! milliseconds. Nothing here opens a socket, so every multi-client scenario is a unit test with
//! fake clients (`tests/scenarios.rs`). The responder holds one [`Services`] for all its
//! connections, behind `D1_MP=1` (off by default).
//!
//! | module | Bungie's service (GDC 2021, "Exploring Services Architecture at Bungie") | wire |
//! |---|---|---|
//! | [`identity`] | accounts, sign-on | BAP 23/24 account-id translation |
//! | [`fireteam`] | fireteams, invites, presence | the client's fireteam session; PSN invitations |
//! | [`matchmaking`] | matchmaking, locator | BAP 42/43 |
//! | [`platform_session`] | the platform's session service | PSN Session WebAPI (`/v1/sessions`) |
//!
//! Several clients in one activity instance (shared membership, peer slots, physics-host election)
//! live with the activity host, in `d1-activity-host/src/group.rs`. The design note is
//! notes/ps4-56578-multiplayer-2026-11-05.md.

pub mod fireteam;
pub mod identity;
pub mod matchmaking;
pub mod platform_session;

use fireteam::{ActivityParty, ActivityRef, FireteamError, FireteamRules, FireteamService};
use identity::{AccountSoid, ConnectionId, FireteamId, IdentityError, IdentityRegistry, OnlineId};
use matchmaking::{ConfigTable, Directory, DirectoryError, DirectoryReply, MatchFormed, Matchmaker, MatchmakerError};
use platform_session::PlatformSessions;

/// What other players (and the client's roster) can see of a player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presence {
    pub account: AccountSoid,
    pub online_id: Option<OnlineId>,
    pub online: bool,
    pub fireteam: Option<FireteamId>,
    pub fireteam_size: usize,
    pub leader: bool,
    /// The activity the player's fireteam is in.
    pub activity_hash: Option<u32>,
    /// Whether another player may join them now ("join fireteam").
    pub joinable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MpError {
    Identity(IdentityError),
    Fireteam(FireteamError),
    Matchmaking(DirectoryError),
    Matchmaker(MatchmakerError),
    /// The connection never signed on.
    NotSignedOn(ConnectionId),
    Config(String),
}

impl std::fmt::Display for MpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Identity(e) => e.fmt(f),
            Self::Fireteam(e) => e.fmt(f),
            Self::Matchmaking(e) => e.fmt(f),
            Self::Matchmaker(e) => e.fmt(f),
            Self::NotSignedOn(c) => write!(f, "connection {} never signed on", c.0),
            Self::Config(e) => f.write_str(e),
        }
    }
}

impl std::error::Error for MpError {}

impl From<IdentityError> for MpError {
    fn from(e: IdentityError) -> Self {
        Self::Identity(e)
    }
}
impl From<FireteamError> for MpError {
    fn from(e: FireteamError) -> Self {
        Self::Fireteam(e)
    }
}
impl From<DirectoryError> for MpError {
    fn from(e: DirectoryError) -> Self {
        Self::Matchmaking(e)
    }
}
impl From<MatchmakerError> for MpError {
    fn from(e: MatchmakerError) -> Self {
        Self::Matchmaker(e)
    }
}

/// ⭐ Every service, shared by every connection of one server.
#[derive(Debug, Clone)]
pub struct Services {
    pub identity: IdentityRegistry,
    pub fireteams: FireteamService,
    pub directory: Directory,
    pub matchmaker: Matchmaker,
    pub sessions: PlatformSessions,
}

impl Default for Services {
    fn default() -> Self {
        Self::new(FireteamRules::default())
    }
}

impl Services {
    pub fn new(rules: FireteamRules) -> Self {
        Self {
            identity: IdentityRegistry::new(),
            fireteams: FireteamService::new(rules),
            directory: Directory::new(),
            matchmaker: Matchmaker::new(),
            sessions: PlatformSessions::new(),
        }
    }

    /// Configure from the environment (the responder calls this once):
    ///
    /// * `D1_MP_ACCOUNTS` — `<online id>=<soid>,...` explicit account bindings;
    /// * `D1_MM_CONFIG_FILE` — the matchmaking configurations (`ps4/fixtures/56578/matchmaking/configurations.json`);
    /// * `D1_MP_FIRETEAM_CAP` — the cap with no activity selected (default 3);
    /// * `D1_MP_RECONNECT_GRACE_MS` — how long a dropped member keeps their slot (default 120000).
    ///
    /// `primary` binds the responder's own `--npid` to `D1_ACCOUNT_SOID`, so a single-player
    /// record keeps its id.
    pub fn from_env(primary: Option<(&str, u64)>) -> Result<Self, MpError> {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        let mut rules = FireteamRules::default();
        if let Some(v) = var("D1_MP_FIRETEAM_CAP") {
            rules.default_cap = v.trim().parse().map_err(|_| MpError::Config(format!("D1_MP_FIRETEAM_CAP={v}")))?;
        }
        if let Some(v) = var("D1_MP_RECONNECT_GRACE_MS") {
            rules.reconnect_grace_ms = v.trim().parse().map_err(|_| MpError::Config(format!("D1_MP_RECONNECT_GRACE_MS={v}")))?;
        }
        let mut s = Self::new(rules);
        if let Some((npid, soid)) = primary {
            if let Some(id) = OnlineId::new(npid) {
                s.identity.bind(id, AccountSoid(soid))?;
            }
        }
        if let Some(spec) = var("D1_MP_ACCOUNTS") {
            s.identity.bind_spec(&spec)?;
        }
        if let Some(path) = var("D1_MM_CONFIG_FILE") {
            let text = std::fs::read_to_string(&path).map_err(|e| MpError::Config(format!("D1_MM_CONFIG_FILE {path}: {e}")))?;
            s.directory.configurations = ConfigTable::from_json(&text).map_err(|e| MpError::Config(e.to_string()))?;
        }
        Ok(s)
    }

    /// A client signed on through `conn` as `online_id`: its account, and its fireteam (a new one
    /// of its own, or the one it dropped out of within the grace).
    pub fn sign_on(&mut self, conn: ConnectionId, online_id: &OnlineId, now_ms: u64) -> Result<(AccountSoid, FireteamId), MpError> {
        let account = self.identity.sign_on(conn, online_id)?;
        let fireteam = self.fireteams.sign_on(account, None, now_ms);
        Ok((account, fireteam))
    }

    /// The connection closed: the player is marked away in its fireteam and its advertised
    /// sessions are withdrawn.
    pub fn sign_off(&mut self, conn: ConnectionId, now_ms: u64) {
        self.directory.drop_connection(conn);
        if let Some(s) = self.identity.sign_off(conn) {
            if let Some(t) = self.fireteams.fireteam_of(s.account) {
                let (id, size) = (t.id, t.size());
                self.matchmaker.cancel(id, size);
            }
            self.fireteams.sign_off(s.account, now_ms);
        }
    }

    /// Periodic housekeeping: expire dropped members and stale advertisements.
    pub fn tick(&mut self, now_ms: u64) -> Vec<MatchFormed> {
        self.fireteams.tick(now_ms);
        self.directory.expire(now_ms, 15 * 60 * 1000);
        self.matchmaker.poll(now_ms)
    }

    /// ⭐ BAP 23: translate the platform ids a client asks about into account SOIDs, each its own.
    /// The ids are the 36-byte padded platform identifiers d1-bap decodes.
    pub fn translate_accounts(&mut self, platform_ids: &[[u8; 36]]) -> Vec<([u8; 36], u64)> {
        platform_ids
            .iter()
            .filter_map(|p| OnlineId::from_padded(p).map(|id| (*p, self.identity.account_for(&id).0)))
            .collect()
    }

    fn account_of(&self, conn: ConnectionId) -> Result<AccountSoid, MpError> {
        self.identity.session(conn).map(|s| s.account).ok_or(MpError::NotSignedOn(conn))
    }

    /// ⭐ BAP 42 from `conn`: the 43 body. Sessions advertised by the searcher's own fireteam are
    /// not offered back to it.
    pub fn matchmaking(&mut self, conn: ConnectionId, body: &[u8], now_ms: u64) -> Result<DirectoryReply, MpError> {
        self.matchmaking_placed(conn, body, now_ms, &mut |_, _| None)
    }

    /// [`Self::matchmaking`] with a server-side placement: `place(want, searcher)` returns `None`
    /// (the directory ranks every advertisement) or `Some(accounts)`, the members of the instance
    /// the placement chose, so only their advertisements are offered (an empty list offers none:
    /// the searcher starts its own instance). `searcher` is the connection's account, if it signed
    /// on. The responder places public-bubble searches in the activity host's registry this way.
    pub fn matchmaking_placed(
        &mut self,
        conn: ConnectionId,
        body: &[u8],
        now_ms: u64,
        place: &mut dyn FnMut(&matchmaking::Want, Option<AccountSoid>) -> Option<Vec<AccountSoid>>,
    ) -> Result<DirectoryReply, MpError> {
        let searcher = self.identity.session(conn).map(|s| s.account);
        let team = searcher.and_then(|a| self.fireteams.fireteam_of(a));
        let exclude: Vec<ConnectionId> = team
            .map(|t| t.accounts().into_iter().filter_map(|a| self.identity.connection_of(a)).collect())
            .unwrap_or_default();
        let size = team.map_or(1, |t| t.size()) as u32;
        let identity = &self.identity;
        let mut owners = |w: &matchmaking::Want| {
            place(w, searcher).map(|accounts| accounts.into_iter().filter_map(|a| identity.connection_of(a)).collect())
        };
        Ok(self.directory.handle_placed(conn, body, &exclude, size, now_ms, &mut owners)?)
    }

    /// The leader launches `activity` with the fireteam: the size check, then the record that the
    /// fireteam is in it (so members can join it in progress). Returns the members to bring.
    pub fn launch_activity(&mut self, leader: ConnectionId, activity: ActivityRef) -> Result<Vec<AccountSoid>, MpError> {
        let account = self.account_of(leader)?;
        self.fireteams.set_activity(account, activity)?;
        Ok(self.fireteams.fireteam_of(account).map(|t| t.accounts()).unwrap_or_default())
    }

    /// The leader enters a playlist with the fireteam. A cooperative playlist places it at once.
    pub fn launch_playlist(&mut self, leader: ConnectionId, playlist: u32, party: Option<ActivityParty>, now_ms: u64) -> Result<Option<MatchFormed>, MpError> {
        let account = self.account_of(leader)?;
        let team = self.fireteams.fireteam_of(account).ok_or(FireteamError::NotSignedOn(account))?;
        if team.leader != account {
            return Err(FireteamError::NotLeader { account, leader: team.leader }.into());
        }
        let (id, size) = (team.id, team.size());
        if let Some(p) = party {
            self.fireteams.check_launch(id, p)?;
        }
        Ok(self.matchmaker.request(id, size, playlist, now_ms)?.1)
    }

    pub fn presence(&self, account: AccountSoid) -> Presence {
        let team = self.fireteams.fireteam_of(account);
        let cap = team.map(|t| t.cap(self.fireteams.rules())).unwrap_or(0);
        Presence {
            account,
            online_id: self.identity.online_id_of(account).cloned(),
            online: self.identity.is_online(account),
            fireteam: team.map(|t| t.id),
            fireteam_size: team.map_or(0, |t| t.size()),
            leader: team.is_some_and(|t| t.leader == account),
            activity_hash: team.and_then(|t| t.activity).map(|a| a.activity_hash),
            joinable: team.is_some_and(|t| {
                t.policy == fireteam::JoinPolicy::Open
                    && t.size() < cap
                    && t.activity.is_none_or(|a| a.host.is_none() || a.joinable_in_progress)
            }),
        }
    }
}
