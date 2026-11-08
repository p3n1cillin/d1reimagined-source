// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! ⭐ **Matchmaking**: the session directory the client talks to over messages 42/43, the
//! configuration replies, and the server-side placement rules for playlists and public bubbles.
//!
//! ## How the live services matchmade (the private captures, 1,011 exchanges)
//!
//! Matchmaking in Destiny is **client-driven over a directory**. A client that hosts a session
//! advertises it (kind 2) and gets an id back; it updates the advertisement as slots fill and
//! deletes it (kind 3) when it leaves. A client looking for a session searches (kind 1) every
//! ~5 s, and the service answers with at most one candidate: the advertised session and its
//! host's name, verbatim, and a score. Both run per **slot**: slot 0 is the activity (a strike
//! searches and advertises under the strike's activity hash), slot 1 a public bubble (under the
//! bubble's id). In The Devil's Lair the client advertised its own strike session with 2 open
//! slots and searched at the same time, so a solo player is in its own instance at once and
//! strangers join it in progress, which is how a playlist fills. A bubble search asks for 3
//! slots, the fireteam's reservation (Bungie's GDC 2015 talk: each player reserves public-bubble
//! slots for absent fireteam members).
//!
//! [`Directory`] is that service. It never looks inside a session: what the client advertised is
//! what a searcher gets back.
//!
//! ## What is ours, not theirs (stand-ins, labelled)
//!
//! * **Which candidate.** The live answer carried one candidate; the rule that chose it is not on
//!   the wire. Ours is the one placement rule every path uses (integration review, finding 8): the
//!   **emptiest** advertisement (the most open slots) that still has room for the searcher, oldest
//!   first ([`rank_fill`]), the rule the public-instance registry records as Bungie's stated design
//!   ([`emptiest_first`]). A public-bubble search whose placement the activity host's registry
//!   decides answers with that instance's advertisement ([`Directory::handle_placed`]).
//! * **The score** (`f3`, 0.906-1.0 live) and `f5` (0.99 or 1.0): `[U]` meaning. We send
//!   `1.0 - 0.005 * open slots` and 1.0.
//! * **The search level** (`f3.2`): bubble searches always got 400; activity searches got 100,
//!   then 200 from about 10 s and 300 from about 15 s, and 400 from about 20 s of the search's own
//!   elapsed time (`f13`). [`RelaxationRule`] reproduces that from the elapsed time; the step and
//!   the ceiling are its parameters, not constants, and the 5 s step matches the configuration's
//!   own `f4.1.4 = 5` / `f4.1.7 = 5` [I].
//! * **Playlists and Crucible**: the live client's search keys are activity hashes, so the
//!   playlist's choice of activity happened before the directory. [`Matchmaker`] makes that choice
//!   and groups fireteams; its rules are ours and documented on it. Crucible teams are **not**
//!   assigned here: the client's own `team_picker` (`0x417380`) balances them peer-side and keeps
//!   fireteam-mates together; the service only groups fireteams into one match and checks that
//!   they can be split into teams at all.

use std::collections::{BTreeMap, HashMap};

use d1_bap::matchmaking::{
    MatchmakingCoreRequest, MatchmakingCoreResponse, MM_KIND_DELETE_ADVERTISEMENT,
    MM_KIND_MATCHMAKING_CONFIGURATION, MM_KIND_SESSION_SEARCH, MM_KIND_UPDATE_ADVERTISEMENT,
};
use d1_bap::matchmaking_sessions::{
    decode_advertisement, decode_delete, decode_search, delete_acknowledged, encode_search_results,
    ProtoTree, SearchCandidate, SessionAttributes, SLOT_BUBBLE,
};
use d1_bap::XetrovDecodeError;

use crate::identity::{ConnectionId, FireteamId};

/// How the `f3.2` level of an activity search rises with the search's elapsed time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelaxationRule {
    pub step_ms: u64,
    pub unit: u64,
    pub max_level: u64,
}

impl Default for RelaxationRule {
    fn default() -> Self {
        Self { step_ms: 5_000, unit: 100, max_level: 4 }
    }
}

impl RelaxationRule {
    pub fn level(&self, slot: u32, elapsed_ms: Option<u64>) -> u64 {
        if slot == SLOT_BUBBLE {
            return self.unit * self.max_level;
        }
        let steps = elapsed_ms.unwrap_or(0) / self.step_ms.max(1);
        self.unit * steps.clamp(1, self.max_level)
    }
}

/// One advertised session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvertisedSession {
    pub id: u64,
    pub owner: ConnectionId,
    /// `f4` and `f5` of the request: which directory it lives in.
    pub slot: u32,
    pub set_id: u32,
    /// `f11.1`, the activity hash (the FNV basis for "unset" on bubble ads).
    pub activity_hash: u32,
    pub configuration_hash: u32,
    pub attributes: Option<SessionAttributes>,
    /// Verbatim from the advertisement; handed back to searchers.
    pub session: Vec<u8>,
    pub name: Vec<u8>,
    pub created_ms: u64,
    pub updated_ms: u64,
}

impl AdvertisedSession {
    /// The slots still open, if the advertiser said.
    pub fn open_slots(&self) -> Option<u32> {
        self.attributes.as_ref().and_then(|a| a.slots)
    }
}

/// What a search wanted, for [`rank_fill`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Want {
    pub slot: u32,
    pub set_id: u32,
    /// Slots the searcher needs (`f7` of its attributes; 1 when it gave none).
    pub needed: u32,
}

/// ⭐ The fill rule: sessions in the same directory that have room for `want.needed`, emptiest
/// first (most open slots), then oldest: the same rule as [`emptiest_first`] (integration review,
/// finding 8; before, this ranked the fullest first while the registry placed the emptiest).
pub fn rank_fill<'a>(
    ads: impl Iterator<Item = &'a AdvertisedSession>,
    want: Want,
    exclude: &[ConnectionId],
) -> Vec<&'a AdvertisedSession> {
    let mut v: Vec<_> = ads
        .filter(|a| a.slot == want.slot && a.set_id == want.set_id)
        .filter(|a| !exclude.contains(&a.owner))
        .filter(|a| !a.session.is_empty())
        .filter(|a| a.open_slots().is_some_and(|open| open >= want.needed))
        .collect();
    v.sort_by_key(|a| (std::cmp::Reverse(a.open_slots().unwrap_or(0)), a.created_ms, a.id));
    v
}

/// The configurations to answer kind 4 with, by matchmaking content hash (`f11.2`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConfigTable {
    by_content: HashMap<u32, Vec<u8>>,
    default: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError(pub String);

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "matchmaking configuration file: {}", self.0)
    }
}

impl std::error::Error for ConfigError {}

fn json_u64(v: &serde_json::Value) -> Option<u64> {
    match v {
        serde_json::Value::Number(n) => n.as_u64(),
        serde_json::Value::String(s) => match s.strip_prefix("0x") {
            Some(h) => u64::from_str_radix(h, 16).ok(),
            None => s.parse().ok(),
        },
        _ => None,
    }
}

/// `[[field, value], ...]` where a value is an integer or hex string (varint), `{"f32": x}`, or
/// a nested list (a message).
fn json_tree(v: &serde_json::Value) -> Result<Vec<ProtoTree>, ConfigError> {
    let items = v.as_array().ok_or_else(|| ConfigError(format!("expected a list, got {v}")))?;
    items
        .iter()
        .map(|item| {
            let pair = item.as_array().filter(|p| p.len() == 2).ok_or_else(|| ConfigError(format!("expected [field, value], got {item}")))?;
            let field = pair[0].as_u64().and_then(|f| u32::try_from(f).ok()).filter(|&f| f > 0)
                .ok_or_else(|| ConfigError(format!("bad field number {}", pair[0])))?;
            let value = &pair[1];
            if value.is_array() {
                return Ok(ProtoTree::Message(field, json_tree(value)?));
            }
            if let Some(f) = value.get("f32").and_then(serde_json::Value::as_f64) {
                return Ok(ProtoTree::Fixed32(field, f as f32));
            }
            json_u64(value).map(|n| ProtoTree::Varint(field, n)).ok_or_else(|| ConfigError(format!("bad value {value}")))
        })
        .collect()
}

impl ConfigTable {
    /// Read the fixture format (`ps4/fixtures/56578/matchmaking/configurations.json`).
    pub fn from_json(text: &str) -> Result<Self, ConfigError> {
        let v: serde_json::Value = serde_json::from_str(text).map_err(|e| ConfigError(e.to_string()))?;
        let list = v.get("configurations").and_then(|c| c.as_array()).ok_or_else(|| ConfigError("no \"configurations\" list".into()))?;
        let mut t = Self::default();
        for c in list {
            let body = ProtoTree::encode_all(&json_tree(c.get("f4").ok_or_else(|| ConfigError("a configuration without \"f4\"".into()))?)?);
            if c.get("default").and_then(serde_json::Value::as_bool) == Some(true) {
                t.default = Some(body.clone());
            }
            for h in c.get("content_hashes").and_then(|h| h.as_array()).into_iter().flatten() {
                let h = json_u64(h).and_then(|h| u32::try_from(h).ok()).ok_or_else(|| ConfigError(format!("bad content hash {h}")))?;
                t.by_content.insert(h, body.clone());
            }
        }
        Ok(t)
    }

    /// The `f4` body for a content hash: its own, the default, or `None` (the caller then sends
    /// the minimal presence-only configuration).
    pub fn for_content(&self, content_hash: u32) -> Option<&[u8]> {
        self.by_content.get(&content_hash).or(self.default.as_ref()).map(Vec::as_slice)
    }

    pub fn is_empty(&self) -> bool {
        self.by_content.is_empty() && self.default.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectoryError {
    Decode(XetrovDecodeError),
    /// A request kind outside 1-4.
    UnknownKind(Option<u32>),
}

impl From<XetrovDecodeError> for DirectoryError {
    fn from(e: XetrovDecodeError) -> Self {
        Self::Decode(e)
    }
}

impl std::fmt::Display for DirectoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Decode(e) => write!(f, "message 42 did not decode: {e}"),
            Self::UnknownKind(k) => write!(f, "message 42 kind {k:?} is not 1-4"),
        }
    }
}

impl std::error::Error for DirectoryError {}

/// What a request did, for the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectoryAction {
    Searched { slot: u32, set_id: u32, needed: u32, found: Vec<u64>, level: u64 },
    Advertised { id: u64, new: bool, open_slots: Option<u32> },
    Deleted { id: u64, existed: bool },
    Configured { content_hash: u32, from_table: bool },
}

/// A reply: the 43 body (after its header) and what happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryReply {
    pub body: Vec<u8>,
    pub action: DirectoryAction,
}

/// ⭐ The session directory behind messages 42/43, shared by every connected client.
#[derive(Debug, Clone)]
pub struct Directory {
    ads: BTreeMap<u64, AdvertisedSession>,
    next_id: u64,
    pub relaxation: RelaxationRule,
    pub configurations: ConfigTable,
    /// How many candidates a search returns. The live service returned one.
    pub max_candidates: usize,
}

impl Default for Directory {
    fn default() -> Self {
        Self { ads: BTreeMap::new(), next_id: 1, relaxation: RelaxationRule::default(), configurations: ConfigTable::default(), max_candidates: 1 }
    }
}

impl Directory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn advertisements(&self) -> impl Iterator<Item = &AdvertisedSession> {
        self.ads.values()
    }

    pub fn advertisement(&self, id: u64) -> Option<&AdvertisedSession> {
        self.ads.get(&id)
    }

    /// ⭐ Answer one message 42 from `conn`. `exclude` are connections whose sessions the
    /// searcher must not be offered (its own fireteam's: they are already together).
    /// `default_needed` is the slots a search needs when it does not say (activity searches never
    /// did): the searcher's fireteam size, so a fireteam is never offered a session it cannot fit.
    pub fn handle(&mut self, conn: ConnectionId, body: &[u8], exclude: &[ConnectionId], default_needed: u32, now_ms: u64) -> Result<DirectoryReply, DirectoryError> {
        self.handle_placed(conn, body, exclude, default_needed, now_ms, &mut |_| None)
    }

    /// [`Self::handle`], with a placement for searches: `place(want)` returns `None` to rank every
    /// advertisement as [`rank_fill`] does, or `Some(owners)` to offer only advertisements owned by
    /// those connections (the members of the instance a server-side placement chose; an empty list
    /// offers none, so the searcher starts its own instance). Integration review, #327's open item:
    /// a public-bubble search is answered with the instance the registry placed it in.
    pub fn handle_placed(
        &mut self,
        conn: ConnectionId,
        body: &[u8],
        exclude: &[ConnectionId],
        default_needed: u32,
        now_ms: u64,
        place: &mut dyn FnMut(&Want) -> Option<Vec<ConnectionId>>,
    ) -> Result<DirectoryReply, DirectoryError> {
        let req = MatchmakingCoreRequest::decode(body)?;
        let nonce = req.nonce.unwrap_or(0);
        let slot = req.search_slot.unwrap_or(0);
        let set_id = req.matchmaking_set_id.unwrap_or(0);
        match req.request_kind {
            Some(MM_KIND_SESSION_SEARCH) => {
                let params = decode_search(body)?.unwrap_or_default();
                let attrs = params.attributes.unwrap_or_default();
                let needed = attrs.slots.unwrap_or(default_needed).max(1);
                let mut exclude = exclude.to_vec();
                exclude.push(conn);
                let want = Want { slot, set_id, needed };
                let owners = place(&want);
                let found: Vec<&AdvertisedSession> = rank_fill(self.ads.values(), want, &exclude)
                    .into_iter()
                    .filter(|a| owners.as_ref().is_none_or(|o| o.contains(&a.owner)))
                    .take(self.max_candidates)
                    .collect();
                let candidates: Vec<SearchCandidate> = found
                    .iter()
                    .map(|a| SearchCandidate {
                        session: a.session.clone(),
                        name: a.name.clone(),
                        score: (1.0 - 0.005 * a.open_slots().unwrap_or(0) as f32).clamp(0.0, 1.0),
                        score_5: 1.0,
                    })
                    .collect();
                let level = self.relaxation.level(slot, attrs.elapsed_ms);
                let results = encode_search_results(&candidates, level).expect("max_candidates is capped below the descriptor's 50");
                let body = MatchmakingCoreResponse { kind: Some(MM_KIND_SESSION_SEARCH), nonce: Some(nonce), search_results: Some(results), ..Default::default() }.encode();
                Ok(DirectoryReply { body, action: DirectoryAction::Searched { slot, set_id, needed, found: found.iter().map(|a| a.id).collect(), level } })
            }
            Some(MM_KIND_UPDATE_ADVERTISEMENT) => {
                let ad = decode_advertisement(body)?.unwrap_or_default();
                let activity_hash = req.matchmaking_key.map(|k| k.activity_hash).unwrap_or(0);
                let existing = ad.id.filter(|id| self.ads.get(id).is_some_and(|a| a.owner == conn));
                let (id, new) = match existing {
                    Some(id) => (id, false),
                    None => {
                        // A repeated id we do not hold (a restart) is re-registered under that id
                        // if it is free, so the client's view stays true; otherwise a fresh one.
                        let id = match ad.id.filter(|id| *id != 0 && !self.ads.contains_key(id)) {
                            Some(id) => id,
                            None => {
                                while self.ads.contains_key(&self.next_id) {
                                    self.next_id += 1;
                                }
                                self.next_id
                            }
                        };
                        self.next_id = self.next_id.max(id + 1);
                        self.ads.insert(id, AdvertisedSession {
                            id, owner: conn, slot, set_id, activity_hash, configuration_hash: 0,
                            attributes: None, session: Vec::new(), name: Vec::new(), created_ms: now_ms, updated_ms: now_ms,
                        });
                        (id, true)
                    }
                };
                let rec = self.ads.get_mut(&id).expect("just ensured");
                rec.slot = slot;
                rec.set_id = set_id;
                if activity_hash != 0 {
                    rec.activity_hash = activity_hash;
                }
                if let Some(h) = ad.configuration_hash.filter(|h| *h != 0) {
                    rec.configuration_hash = h;
                }
                if ad.attributes.is_some() {
                    rec.attributes = ad.attributes;
                }
                if let Some(s) = ad.session {
                    rec.session = s;
                }
                if let Some(n) = ad.name {
                    rec.name = n;
                }
                rec.updated_ms = now_ms;
                let open_slots = rec.open_slots();
                let body = MatchmakingCoreResponse::advertisement_acknowledged(nonce, id).encode();
                Ok(DirectoryReply { body, action: DirectoryAction::Advertised { id, new, open_slots } })
            }
            Some(MM_KIND_DELETE_ADVERTISEMENT) => {
                let id = decode_delete(body)?.unwrap_or(0);
                let existed = self.ads.get(&id).is_some_and(|a| a.owner == conn);
                if existed {
                    self.ads.remove(&id);
                }
                Ok(DirectoryReply { body: delete_acknowledged(nonce), action: DirectoryAction::Deleted { id, existed } })
            }
            Some(MM_KIND_MATCHMAKING_CONFIGURATION) => {
                let content_hash = req.matchmaking_key.map(|k| k.matchmaking_content_hash).unwrap_or(0);
                let (rsp, from_table) = match self.configurations.for_content(content_hash) {
                    Some(f4) => (MatchmakingCoreResponse { kind: Some(MM_KIND_MATCHMAKING_CONFIGURATION), nonce: Some(nonce), configuration: Some(f4.to_vec()), ..Default::default() }, true),
                    None => (MatchmakingCoreResponse::minimal_configuration(MM_KIND_MATCHMAKING_CONFIGURATION, nonce), false),
                };
                Ok(DirectoryReply { body: rsp.encode(), action: DirectoryAction::Configured { content_hash, from_table } })
            }
            other => Err(DirectoryError::UnknownKind(other)),
        }
    }

    /// A connection closed: its advertisements go with it.
    pub fn drop_connection(&mut self, conn: ConnectionId) -> usize {
        let before = self.ads.len();
        self.ads.retain(|_, a| a.owner != conn);
        before - self.ads.len()
    }

    /// Drop advertisements not refreshed for `max_age_ms` (the live client refreshed every ~300 s).
    pub fn expire(&mut self, now_ms: u64, max_age_ms: u64) -> usize {
        let before = self.ads.len();
        self.ads.retain(|_, a| now_ms.saturating_sub(a.updated_ms) < max_age_ms);
        before - self.ads.len()
    }
}

// ─────────────────────────── server-side placement ───────────────────────────

/// A public bubble instance, as the public-instance registry (stream G, `d1-activity-host`) sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicInstance {
    /// Its activity host id.
    pub host: u64,
    /// Players in it now.
    pub players: usize,
    /// Slots held for absent fireteam members of players already in it.
    pub reserved: usize,
    /// Its size (the standard bubble is 6: GDC 2015).
    pub capacity: usize,
    /// When it started, for the oldest-first tie-break.
    pub started_ms: u64,
}

impl PublicInstance {
    pub fn room(&self) -> usize {
        self.capacity.saturating_sub(self.players + self.reserved)
    }
}

/// ⭐ **The interface stream G implements**: the public instances of one bubble, and taking a
/// reservation in one.
pub trait PublicInstances {
    /// Every live public instance of `bubble` (the bubble id the client searches with, `f5` of a
    /// slot-1 search, e.g. `0x8001_0038`).
    fn instances(&self, bubble: u32) -> Vec<PublicInstance>;
    /// Hold `slots` in `host` for an arriving fireteam. `false` if it no longer has room.
    fn reserve(&mut self, host: u64, slots: usize) -> bool;
    /// The hosts to try for a fireteam reserving `reservation` slots, best first. The registry
    /// that owns the instances owns the rule: stream G's registry (`d1-activity-host`,
    /// `PublicRegistry::placement_order`) overrides this with its own. The default is the same
    /// rule, [`emptiest_first`].
    fn placement_order(&self, bubble: u32, reservation: usize) -> Vec<u64> {
        emptiest_first(&self.instances(bubble), reservation)
    }
}

/// ⭐ The public placement rule: the **emptiest** instance with room for the whole reservation,
/// the oldest first on a tie (then the lowest host id). This is stream G's registry's rule, which
/// it records as Bungie's stated design (`d1-activity-host/src/public_registry.rs`). It replaces the "fullest first"
/// fill this module had: the live captures do not show which one the live matchmaker used, and
/// the two rules must not disagree (`notes/ps4-56578-integration-wiring-2026-11-05.md`).
pub fn emptiest_first(instances: &[PublicInstance], reservation: usize) -> Vec<u64> {
    let mut v: Vec<&PublicInstance> = instances.iter().filter(|i| i.room() >= reservation.max(1)).collect();
    v.sort_by_key(|i| (i.players + i.reserved, i.started_ms, i.host));
    v.into_iter().map(|i| i.host).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicPlacement {
    /// Join this host (the client's activity-host request names it; a join of type 2).
    Join { host: u64 },
    /// No instance has room: start a standalone instance that strangers can join later (the
    /// airlock's fallback, GDC 2015).
    StartNew,
}

/// ⭐ Place a fireteam in a public bubble with room for its whole reservation (`reservation`, at
/// least its size: the fireteam's cap, so absent members can follow). The order is the registry's
/// ([`PublicInstances::placement_order`]; [`emptiest_first`] by default), so the matchmaker and
/// the registry place by one rule.
pub fn place_in_public(bubble: u32, reservation: usize, registry: &mut dyn PublicInstances) -> PublicPlacement {
    for host in registry.placement_order(bubble, reservation) {
        if registry.reserve(host, reservation.max(1)) {
            return PublicPlacement::Join { host };
        }
    }
    PublicPlacement::StartNew
}

/// An in-memory [`PublicInstances`], for tests and until stream G's registry is wired.
#[derive(Debug, Clone, Default)]
pub struct InMemoryPublicInstances {
    pub by_bubble: BTreeMap<u32, Vec<PublicInstance>>,
}

impl PublicInstances for InMemoryPublicInstances {
    fn instances(&self, bubble: u32) -> Vec<PublicInstance> {
        self.by_bubble.get(&bubble).cloned().unwrap_or_default()
    }

    fn reserve(&mut self, host: u64, slots: usize) -> bool {
        for i in self.by_bubble.values_mut().flatten() {
            if i.host == host {
                if i.room() < slots {
                    return false;
                }
                i.reserved += slots;
                return true;
            }
        }
        false
    }
}

/// How a playlist fills.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaylistKind {
    /// Strikes (and other cooperative playlists): instances of up to `max_players`. A fireteam is
    /// placed at once, in an open instance of the playlist if one has room, else its own, which
    /// then stays open for later arrivals (join in progress).
    Cooperative { max_players: usize },
    /// Crucible: `teams` teams of up to `team_size`. A match forms once `min_players` are grouped
    /// (or it is full) and every fireteam fits on one team.
    Versus { teams: usize, team_size: usize, min_players: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Playlist {
    pub hash: u32,
    pub kind: PlaylistKind,
    /// The activities it rotates through (content: the playlist's activity list).
    pub activities: Vec<u32>,
}

/// ⭐ Stream O2: the playlist table the responder loads at start-up
/// (`ps4/fixtures/56578/activities/matchmaking-playlists.tsv`), written by the activity host from
/// the activity framework (`encounter_framework::matchmaking_playlists`), which the responder
/// does not load. One row per playlist:
///
/// ```text
/// hash  kind  max_or_teams  team_size  min_players  activities  name
/// 0x…   coop  3             -          -            0x…,0x…     Vanguard Strikes
/// 0x…   versus 2            6          6            0x…,…       Control
/// ```
pub fn format_playlists(rows: &[(Playlist, String)]) -> String {
    let mut out = String::from("hash\tkind\tmax_or_teams\tteam_size\tmin_players\tactivities\tname\n");
    for (p, name) in rows {
        let (kind, a, b, c) = match p.kind {
            PlaylistKind::Cooperative { max_players } => ("coop", max_players.to_string(), "-".to_string(), "-".to_string()),
            PlaylistKind::Versus { teams, team_size, min_players } => ("versus", teams.to_string(), team_size.to_string(), min_players.to_string()),
        };
        let acts: Vec<String> = p.activities.iter().map(|h| format!("{h:#010x}")).collect();
        let name: String = name.chars().map(|c| if c == '\t' || c == '\n' { ' ' } else { c }).collect();
        out += &format!("{:#010x}\t{kind}\t{a}\t{b}\t{c}\t{}\t{name}\n", p.hash, if acts.is_empty() { "-".into() } else { acts.join(",") });
    }
    out
}

/// Read [`format_playlists`]' table (`#` lines are comments).
pub fn parse_playlists(text: &str) -> Result<Vec<Playlist>, String> {
    let hex = |s: &str| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).map_err(|e| format!("{s:?}: {e}"));
    let num = |s: &str| s.trim().parse::<usize>().map_err(|e| format!("{s:?}: {e}"));
    let mut out = Vec::new();
    for line in text.lines().filter(|l| !l.starts_with('#') && !l.starts_with("hash\t") && !l.trim().is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 6 {
            return Err(format!("{line:?}: hash kind max_or_teams team_size min_players activities [name]"));
        }
        let kind = match f[1] {
            "coop" => PlaylistKind::Cooperative { max_players: num(f[2])? },
            "versus" => PlaylistKind::Versus { teams: num(f[2])?, team_size: num(f[3])?, min_players: num(f[4])? },
            k => return Err(format!("{line:?}: kind {k:?} is not coop or versus")),
        };
        let activities = if f[5] == "-" { Vec::new() } else { f[5].split(',').map(hex).collect::<Result<_, _>>()? };
        out.push(Playlist { hash: hex(f[0])?, kind, activities });
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TicketId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ticket {
    pub id: TicketId,
    pub fireteam: FireteamId,
    pub size: usize,
    pub playlist: u32,
    pub queued_ms: u64,
}

/// A match the matchmaker formed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchFormed {
    pub playlist: u32,
    pub activity_hash: u32,
    /// The instance's key (a matchmaker match id; the activity host id once one is assigned).
    pub match_id: u64,
    /// Fireteams in it, in arrival order. Teams are the client's `team_picker`'s to decide.
    pub fireteams: Vec<FireteamId>,
    pub players: usize,
    /// For a cooperative match, whether this placement joined an instance already running.
    pub joined_in_progress: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchmakerError {
    UnknownPlaylist(u32),
    /// The fireteam is larger than the playlist's instances or teams.
    TooLarge { size: usize, max: usize },
    AlreadyQueued(FireteamId),
}

impl std::fmt::Display for MatchmakerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownPlaylist(h) => write!(f, "no playlist {h:#010x}"),
            Self::TooLarge { size, max } => write!(f, "a fireteam of {size} cannot enter (max {max})"),
            Self::AlreadyQueued(id) => write!(f, "{id} is already matchmaking"),
        }
    }
}

impl std::error::Error for MatchmakerError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OpenMatch {
    formed: MatchFormed,
    capacity: usize,
}

/// Whether fireteams of these sizes can be split into `teams` teams of at most `team_size` without
/// splitting a fireteam (first-fit decreasing; exact for the sizes D1 uses).
pub fn teams_feasible(sizes: &[usize], teams: usize, team_size: usize) -> bool {
    let mut s = sizes.to_vec();
    s.sort_unstable_by(|a, b| b.cmp(a));
    let mut bins = vec![0usize; teams];
    'next: for x in s {
        for b in bins.iter_mut() {
            if *b + x <= team_size {
                *b += x;
                continue 'next;
            }
        }
        return false;
    }
    true
}

/// ⭐ Playlist matchmaking: tickets in, matches out.
#[derive(Debug, Clone, Default)]
pub struct Matchmaker {
    playlists: BTreeMap<u32, Playlist>,
    queue: Vec<Ticket>,
    open: Vec<OpenMatch>,
    next_ticket: u64,
    next_match: u64,
    /// Rotation position per playlist (the choice of activity; the live rule is unknown).
    rotation: HashMap<u32, usize>,
}

impl Matchmaker {
    pub fn new() -> Self {
        Self { next_ticket: 1, next_match: 1, ..Self::default() }
    }

    pub fn add_playlist(&mut self, p: Playlist) {
        self.playlists.insert(p.hash, p);
    }

    pub fn queued(&self) -> &[Ticket] {
        &self.queue
    }

    fn max_fireteam(kind: &PlaylistKind) -> usize {
        match kind {
            PlaylistKind::Cooperative { max_players } => *max_players,
            PlaylistKind::Versus { team_size, .. } => *team_size,
        }
    }

    fn next_activity(&mut self, playlist: u32) -> u32 {
        let p = &self.playlists[&playlist];
        if p.activities.is_empty() {
            return playlist;
        }
        let i = self.rotation.entry(playlist).or_insert(0);
        let a = p.activities[*i % p.activities.len()];
        *i += 1;
        a
    }

    /// A fireteam asks to play `playlist`. A cooperative playlist places it at once (returned);
    /// a versus playlist queues it until [`Self::poll`] forms a match.
    pub fn request(&mut self, fireteam: FireteamId, size: usize, playlist: u32, now_ms: u64) -> Result<(TicketId, Option<MatchFormed>), MatchmakerError> {
        let p = self.playlists.get(&playlist).ok_or(MatchmakerError::UnknownPlaylist(playlist))?;
        let max = Self::max_fireteam(&p.kind);
        if size == 0 || size > max {
            return Err(MatchmakerError::TooLarge { size, max });
        }
        if self.queue.iter().any(|t| t.fireteam == fireteam) || self.open.iter().any(|m| m.formed.fireteams.contains(&fireteam)) {
            return Err(MatchmakerError::AlreadyQueued(fireteam));
        }
        let id = TicketId(self.next_ticket);
        self.next_ticket += 1;
        match p.kind.clone() {
            PlaylistKind::Cooperative { max_players } => {
                // Fill: the emptiest open instance of this playlist that has room, the oldest on a
                // tie (the one placement rule, integration review finding 8).
                let pick = self
                    .open
                    .iter_mut()
                    .filter(|m| m.formed.playlist == playlist && m.formed.players + size <= m.capacity)
                    .min_by_key(|m| (m.formed.players, m.formed.match_id));
                if let Some(m) = pick {
                    m.formed.fireteams.push(fireteam);
                    m.formed.players += size;
                    let mut f = m.formed.clone();
                    f.joined_in_progress = true;
                    return Ok((id, Some(f)));
                }
                let activity_hash = self.next_activity(playlist);
                let formed = MatchFormed { playlist, activity_hash, match_id: self.next_match, fireteams: vec![fireteam], players: size, joined_in_progress: false };
                self.next_match += 1;
                self.open.push(OpenMatch { formed: formed.clone(), capacity: max_players });
                Ok((id, Some(formed)))
            }
            PlaylistKind::Versus { .. } => {
                self.queue.push(Ticket { id, fireteam, size, playlist, queued_ms: now_ms });
                Ok((id, None))
            }
        }
    }

    /// Withdraw a queued ticket, or take a fireteam out of the instance it was placed in.
    pub fn cancel(&mut self, fireteam: FireteamId, size: usize) {
        self.queue.retain(|t| t.fireteam != fireteam);
        for m in &mut self.open {
            if let Some(i) = m.formed.fireteams.iter().position(|f| *f == fireteam) {
                m.formed.fireteams.remove(i);
                m.formed.players = m.formed.players.saturating_sub(size);
            }
        }
        self.open.retain(|m| !m.formed.fireteams.is_empty());
    }

    /// Form what versus matches can be formed now: oldest tickets first, a match is the largest
    /// group (up to `teams * team_size`) whose fireteams fit into teams, once it reaches
    /// `min_players`.
    pub fn poll(&mut self, _now_ms: u64) -> Vec<MatchFormed> {
        let mut out = Vec::new();
        let hashes: Vec<u32> = self.playlists.keys().copied().collect();
        for playlist in hashes {
            let PlaylistKind::Versus { teams, team_size, min_players } = self.playlists[&playlist].kind.clone() else { continue };
            loop {
                let mut tickets: Vec<Ticket> = self.queue.iter().filter(|t| t.playlist == playlist).cloned().collect();
                tickets.sort_by_key(|t| (t.queued_ms, t.id));
                let mut chosen: Vec<Ticket> = Vec::new();
                for t in tickets {
                    let mut sizes: Vec<usize> = chosen.iter().map(|c| c.size).collect();
                    sizes.push(t.size);
                    if teams_feasible(&sizes, teams, team_size) {
                        chosen.push(t);
                    }
                }
                let players: usize = chosen.iter().map(|t| t.size).sum();
                if players < min_players.max(1) {
                    break;
                }
                let activity_hash = self.next_activity(playlist);
                self.queue.retain(|t| !chosen.iter().any(|c| c.id == t.id));
                out.push(MatchFormed { playlist, activity_hash, match_id: self.next_match, fireteams: chosen.iter().map(|t| t.fireteam).collect(), players, joined_in_progress: false });
                self.next_match += 1;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn playlist_tables_round_trip() {
        let rows = vec![
            (Playlist { hash: 0x1234_5678, kind: PlaylistKind::Cooperative { max_players: 3 }, activities: vec![1, 2] }, "Vanguard\tStrikes".to_string()),
            (Playlist { hash: 0x9ABC_DEF0, kind: PlaylistKind::Versus { teams: 2, team_size: 6, min_players: 6 }, activities: vec![] }, "Control".to_string()),
        ];
        let text = format_playlists(&rows);
        assert_eq!(parse_playlists(&text).unwrap(), rows.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>());
        assert!(text.contains("Vanguard Strikes"), "a tab in a name does not split the row");
        assert!(parse_playlists("0x1\tsolo\t1\t-\t-\t-\n").is_err());
    }

    use super::*;
    use d1_bap::matchmaking::{MatchmakingCoreRequest, MatchmakingKey};
    use d1_bap::matchmaking_sessions::decode_search_results;

    // Synthetic request bodies in the captured shapes (no capture bytes).
    fn varint(out: &mut Vec<u8>, field: u32, mut v: u64) {
        let mut tag = u64::from(field) << 3;
        loop { let b = (tag & 0x7F) as u8; tag >>= 7; if tag == 0 { out.push(b); break } out.push(b | 0x80) }
        loop { let b = (v & 0x7F) as u8; v >>= 7; if v == 0 { out.push(b); return } out.push(b | 0x80) }
    }
    fn bytes(out: &mut Vec<u8>, field: u32, b: &[u8]) {
        out.push(((field << 3) | 2) as u8);
        let mut n = b.len() as u64;
        loop { let x = (n & 0x7F) as u8; n >>= 7; if n == 0 { out.push(x); break } out.push(x | 0x80) }
        out.extend_from_slice(b);
    }
    fn base(kind: u32, nonce: u64, slot: u32, set: u32) -> Vec<u8> {
        MatchmakingCoreRequest {
            version: Some(13), request_kind: Some(kind), nonce: Some(nonce), search_slot: Some(slot), matchmaking_set_id: Some(set),
            matchmaking_key: Some(MatchmakingKey { activity_hash: set, matchmaking_content_hash: 0, skill_bucket_hash: None }),
            ..Default::default()
        }.encode()
    }
    fn advertise(nonce: u64, id: Option<u64>, slot: u32, set: u32, open: Option<u32>, who: &[u8]) -> Vec<u8> {
        let mut b = base(2, nonce, slot, set);
        let mut attrs = Vec::new();
        varint(&mut attrs, 3, 1);
        if let Some(o) = open { varint(&mut attrs, 7, u64::from(o)); }
        let mut sess = Vec::new();
        bytes(&mut sess, 1, &[who[0]; 60]);
        let mut desc = Vec::new();
        bytes(&mut desc, 1, if open.is_some() { &attrs } else { &[] });
        bytes(&mut desc, 2, &sess);
        bytes(&mut desc, 3, who);
        let mut f8 = Vec::new();
        if let Some(id) = id { varint(&mut f8, 1, id); }
        bytes(&mut f8, 2, &desc);
        varint(&mut f8, 3, 0xF1B7_AD41);
        bytes(&mut b, 8, &f8);
        b
    }
    fn search(nonce: u64, slot: u32, set: u32, needed: Option<u32>, elapsed: u64) -> Vec<u8> {
        let mut b = base(1, nonce, slot, set);
        let mut attrs = Vec::new();
        varint(&mut attrs, 3, 3);
        if let Some(n) = needed { varint(&mut attrs, 7, u64::from(n)); }
        varint(&mut attrs, 13, elapsed);
        let mut f7 = Vec::new();
        bytes(&mut f7, 1, &attrs);
        varint(&mut f7, 2, 0xF1B7_AD41);
        bytes(&mut b, 7, &f7);
        b
    }
    fn delete(nonce: u64, id: u64) -> Vec<u8> {
        let mut b = base(3, nonce, 0, 0);
        let mut f9 = Vec::new();
        varint(&mut f9, 1, id);
        bytes(&mut b, 9, &f9);
        b
    }

    const STRIKE: u32 = 0xFBFD_F050;
    const BUBBLE: u32 = 0x8001_0038;
    const A: ConnectionId = ConnectionId(1);
    const B: ConnectionId = ConnectionId(2);
    const C: ConnectionId = ConnectionId(3);

    fn ack_id(body: &[u8]) -> u64 {
        MatchmakingCoreResponse::decode(body).unwrap().advertisement_ack.unwrap()
    }

    #[test]
    fn a_strike_session_is_advertised_found_and_deleted() {
        let mut d = Directory::new();
        // A registers (no attributes yet), then updates with 2 open slots: the live sequence.
        let r = d.handle(A, &advertise(1, None, 0, STRIKE, None, b"Alpha"), &[], 1, 0).unwrap();
        let id = ack_id(&r.body);
        assert!(matches!(r.action, DirectoryAction::Advertised { new: true, .. }));
        // Not matchable before it says how many slots are open.
        let s = d.handle(B, &search(2, 0, STRIKE, None, 0), &[], 1, 1).unwrap();
        assert!(matches!(&s.action, DirectoryAction::Searched { found, .. } if found.is_empty()));
        let r = d.handle(A, &advertise(3, Some(id), 0, STRIKE, Some(2), b"Alpha"), &[], 1, 2).unwrap();
        assert_eq!(ack_id(&r.body), id, "an update is acknowledged with the same id");
        // B finds it: A's session and name, verbatim.
        let s = d.handle(B, &search(4, 0, STRIKE, None, 0), &[], 1, 3).unwrap();
        let (cands, level) = decode_search_results(&s.body).unwrap().unwrap();
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].name, b"Alpha");
        assert_eq!(cands[0].session, d.advertisement(id).unwrap().session);
        assert_eq!(level, Some(100));
        // A does not find its own session.
        let s = d.handle(A, &search(5, 0, STRIKE, None, 0), &[], 1, 4).unwrap();
        assert!(matches!(&s.action, DirectoryAction::Searched { found, .. } if found.is_empty()));
        // Delete, acknowledged with an empty f5; gone from searches.
        let del = d.handle(A, &delete(6, id), &[], 1, 5).unwrap();
        assert_eq!(&del.body[del.body.len() - 2..], &[0x2A, 0x00]);
        assert_eq!(del.action, DirectoryAction::Deleted { id, existed: true });
        let s = d.handle(B, &search(7, 0, STRIKE, None, 0), &[], 1, 6).unwrap();
        assert!(matches!(&s.action, DirectoryAction::Searched { found, .. } if found.is_empty()));
    }

    #[test]
    fn a_search_fills_the_emptiest_session_with_room_and_honours_the_reservation() {
        let mut d = Directory::new();
        d.handle(A, &advertise(1, None, SLOT_BUBBLE, BUBBLE, Some(5), b"A"), &[], 1, 0).unwrap();
        d.handle(B, &advertise(2, None, SLOT_BUBBLE, BUBBLE, Some(3), b"B"), &[], 1, 1).unwrap();
        d.handle(C, &advertise(3, None, SLOT_BUBBLE, 0x8001_0030, Some(1), b"C"), &[], 1, 2).unwrap();
        // Needs 3 (a fireteam's reservation): A (5 open) is emptier than B (3 open).
        let s = d.handle(ConnectionId(9), &search(4, SLOT_BUBBLE, BUBBLE, Some(3), 0), &[], 1, 3).unwrap();
        let (c, level) = decode_search_results(&s.body).unwrap().unwrap();
        assert_eq!(c[0].name, b"A");
        assert_eq!(level, Some(400), "bubble searches were always answered at 400");
        // Needs 4: only A has room.
        let s = d.handle(ConnectionId(9), &search(5, SLOT_BUBBLE, BUBBLE, Some(4), 0), &[], 1, 4).unwrap();
        assert_eq!(decode_search_results(&s.body).unwrap().unwrap().0[0].name, b"A");
        // Needs 6: nobody; the client then starts its own instance.
        let s = d.handle(ConnectionId(9), &search(6, SLOT_BUBBLE, BUBBLE, Some(6), 0), &[], 1, 5).unwrap();
        assert!(decode_search_results(&s.body).unwrap().unwrap().0.is_empty());
        // Excluding A (the searcher's own fireteam) offers B.
        let s = d.handle(ConnectionId(9), &search(7, SLOT_BUBBLE, BUBBLE, Some(3), 0), &[A], 1, 6).unwrap();
        assert_eq!(decode_search_results(&s.body).unwrap().unwrap().0[0].name, b"B");
    }

    #[test]
    fn a_placed_search_offers_only_the_placed_instances_advertisement() {
        let mut d = Directory::new();
        d.handle(A, &advertise(1, None, SLOT_BUBBLE, BUBBLE, Some(5), b"A"), &[], 1, 0).unwrap();
        d.handle(B, &advertise(2, None, SLOT_BUBBLE, BUBBLE, Some(3), b"B"), &[], 1, 1).unwrap();
        // The registry placed the searcher in B's instance, although A's is emptier.
        let s = d.handle_placed(ConnectionId(9), &search(3, SLOT_BUBBLE, BUBBLE, Some(1), 0), &[], 1, 2, &mut |w| {
            assert_eq!((w.slot, w.set_id, w.needed), (SLOT_BUBBLE, BUBBLE, 1));
            Some(vec![B])
        }).unwrap();
        assert_eq!(decode_search_results(&s.body).unwrap().unwrap().0[0].name, b"B");
        // A new instance: no candidate, so the client starts its own.
        let s = d.handle_placed(ConnectionId(9), &search(4, SLOT_BUBBLE, BUBBLE, Some(1), 0), &[], 1, 3, &mut |_| Some(Vec::new())).unwrap();
        assert!(decode_search_results(&s.body).unwrap().unwrap().0.is_empty());
        // No placement: the directory's own ranking.
        let s = d.handle_placed(ConnectionId(9), &search(5, SLOT_BUBBLE, BUBBLE, Some(1), 0), &[], 1, 4, &mut |_| None).unwrap();
        assert_eq!(decode_search_results(&s.body).unwrap().unwrap().0[0].name, b"A");
    }

    #[test]
    fn the_activity_search_level_rises_with_its_elapsed_time() {
        let r = RelaxationRule::default();
        // The live pairs (elapsed ms -> level) from The Summoning Pits and The Devil's Lair.
        for (elapsed, level) in [(1, 100), (5_200, 100), (10_404, 200), (15_033, 300), (20_241, 400), (60_000, 400)] {
            assert_eq!(r.level(0, Some(elapsed)), level, "{elapsed} ms");
        }
        assert_eq!(r.level(SLOT_BUBBLE, Some(0)), 400);
    }

    #[test]
    fn only_the_owner_updates_or_deletes_and_a_closed_connection_takes_its_ads() {
        let mut d = Directory::new();
        let id = ack_id(&d.handle(A, &advertise(1, None, 0, STRIKE, Some(2), b"A"), &[], 1, 0).unwrap().body);
        // B repeating A's id gets an advertisement of its own.
        let other = ack_id(&d.handle(B, &advertise(2, Some(id), 0, STRIKE, Some(1), b"B"), &[], 1, 1).unwrap().body);
        assert_ne!(other, id);
        assert_eq!(d.advertisement(id).unwrap().name, b"A");
        assert_eq!(d.handle(B, &delete(3, id), &[], 1, 2).unwrap().action, DirectoryAction::Deleted { id, existed: false });
        assert!(d.advertisement(id).is_some());
        assert_eq!(d.drop_connection(A), 1);
        assert!(d.advertisement(id).is_none());
        // A stale advertisement expires.
        assert_eq!(d.expire(1_000_000, 900_000), 1);
    }

    #[test]
    fn configurations_come_from_the_fixture_by_content_hash() {
        let mut d = Directory::new();
        d.configurations = ConfigTable::from_json(include_str!("../../../../fixtures/56578/matchmaking/configurations.json")).unwrap();
        let req = |content: u32| MatchmakingCoreRequest {
            version: Some(13), request_kind: Some(4), nonce: Some(8),
            matchmaking_key: Some(MatchmakingKey { activity_hash: 1, matchmaking_content_hash: content, skill_bucket_hash: None }),
            ..Default::default()
        }.encode();
        let strike = MatchmakingCoreResponse::decode(&d.handle(A, &req(0xF502_DD07), &[], 1, 0).unwrap().body).unwrap();
        let social = MatchmakingCoreResponse::decode(&d.handle(A, &req(0x5EC0_25C8), &[], 1, 0).unwrap().body).unwrap();
        let other = d.handle(A, &req(0x1234), &[], 1, 0).unwrap();
        assert!(matches!(other.action, DirectoryAction::Configured { from_table: true, .. }), "the default configuration");
        let f4 = strike.configuration.unwrap();
        // f4.1.1 is the configuration's own hash, which strike searches then quote in f7.2.
        assert_eq!(&f4[..8], &[0x0A, 0x19, 0x08, 0xC1, 0xDA, 0xDE, 0x8D, 0x0F]);
        assert_ne!(Some(f4), social.configuration);
        // No table: the minimal presence-only configuration, as before.
        let mut bare = Directory::new();
        let r = bare.handle(A, &req(0xF502_DD07), &[], 1, 0).unwrap();
        assert_eq!(MatchmakingCoreResponse::decode(&r.body).unwrap().configuration, Some(vec![0x0A, 0x00]));
    }

    fn inst(host: u64, players: usize, reserved: usize, started_ms: u64) -> PublicInstance {
        PublicInstance { host, players, reserved, capacity: 6, started_ms }
    }

    #[test]
    fn public_fill_takes_the_emptiest_instance_with_room_for_the_reservation() {
        let mut reg = InMemoryPublicInstances::default();
        reg.by_bubble.insert(BUBBLE, vec![inst(10, 1, 2, 0), inst(11, 1, 0, 5), inst(12, 4, 0, 1)]);
        // A fireteam reserving 3: 12 has only 2 left; 11 (1 held) is emptier than 10 (3 held).
        assert_eq!(place_in_public(BUBBLE, 3, &mut reg), PublicPlacement::Join { host: 11 });
        // 10 and 11 now both hold 3 and have 3 left; 10 started first.
        assert_eq!(place_in_public(BUBBLE, 3, &mut reg), PublicPlacement::Join { host: 10 });
        // Both are full; a solo player still fits in 12.
        assert_eq!(place_in_public(BUBBLE, 1, &mut reg), PublicPlacement::Join { host: 12 });
        assert_eq!(place_in_public(BUBBLE, 3, &mut reg), PublicPlacement::StartNew);
        assert_eq!(place_in_public(0x8001_0030, 1, &mut reg), PublicPlacement::StartNew);
    }

    /// A registry that places by its own rule is asked for the order.
    #[test]
    fn the_registry_owns_the_placement_order() {
        struct Reversed(InMemoryPublicInstances);
        impl PublicInstances for Reversed {
            fn instances(&self, bubble: u32) -> Vec<PublicInstance> {
                self.0.instances(bubble)
            }
            fn reserve(&mut self, host: u64, slots: usize) -> bool {
                self.0.reserve(host, slots)
            }
            fn placement_order(&self, bubble: u32, reservation: usize) -> Vec<u64> {
                let mut v = emptiest_first(&self.instances(bubble), reservation);
                v.reverse();
                v
            }
        }
        let mut reg = Reversed(InMemoryPublicInstances::default());
        reg.0.by_bubble.insert(BUBBLE, vec![inst(10, 1, 0, 0), inst(11, 3, 0, 0)]);
        assert_eq!(place_in_public(BUBBLE, 1, &mut reg), PublicPlacement::Join { host: 11 });
        assert_eq!(emptiest_first(&reg.instances(BUBBLE), 1), vec![10, 11]);
        assert_eq!(emptiest_first(&reg.instances(BUBBLE), 3), vec![10], "11 has 2 left");
    }

    fn ft(n: u64) -> FireteamId {
        FireteamId(crate::fireteam::FIRETEAM_ID_BASE | n)
    }

    #[test]
    fn a_strike_playlist_fills_instances_to_three_keeping_fireteams_whole() {
        let mut m = Matchmaker::new();
        m.add_playlist(Playlist { hash: 0xAAAA, kind: PlaylistKind::Cooperative { max_players: 3 }, activities: vec![STRIKE, 0x0CA4_36DA] });
        let (_, a) = m.request(ft(1), 1, 0xAAAA, 0).unwrap();
        let a = a.unwrap();
        assert!(!a.joined_in_progress);
        let (_, b) = m.request(ft(2), 2, 0xAAAA, 1).unwrap();
        let b = b.unwrap();
        assert_eq!((b.match_id, b.players, b.joined_in_progress), (a.match_id, 3, true));
        // Full: the next fireteam starts a new instance, on the next strike in the rotation.
        let (_, c) = m.request(ft(3), 2, 0xAAAA, 2).unwrap();
        let c = c.unwrap();
        assert_ne!(c.match_id, a.match_id);
        assert_ne!(c.activity_hash, a.activity_hash);
        assert_eq!(m.request(ft(4), 4, 0xAAAA, 3), Err(MatchmakerError::TooLarge { size: 4, max: 3 }));
        assert_eq!(m.request(ft(1), 1, 0xAAAA, 3), Err(MatchmakerError::AlreadyQueued(ft(1))));
        // ft(2) leaves; its two slots open again for a pair.
        m.cancel(ft(2), 2);
        let (_, d) = m.request(ft(5), 2, 0xAAAA, 4).unwrap();
        // c (2 of 3) has no room for a pair; a (1 of 3) does.
        assert_eq!(d.unwrap().match_id, a.match_id);
    }

    #[test]
    fn a_crucible_match_forms_at_its_minimum_with_fireteams_that_fit_on_teams() {
        let mut m = Matchmaker::new();
        m.add_playlist(Playlist { hash: 0xC0C0, kind: PlaylistKind::Versus { teams: 2, team_size: 3, min_players: 6 }, activities: vec![0x1111] });
        for (i, size) in [(1, 3), (2, 3), (3, 2), (4, 1)] {
            let (_, now) = m.request(ft(i), size, 0xC0C0, i).unwrap();
            assert!(now.is_none(), "versus playlists queue");
        }
        let formed = m.poll(10);
        assert_eq!(formed.len(), 1);
        // The two threes fill both teams; the pair and the solo wait for the next match.
        assert_eq!(formed[0].fireteams, vec![ft(1), ft(2)]);
        assert_eq!(formed[0].players, 6);
        assert_eq!(m.queued().len(), 2);
        assert!(m.poll(11).is_empty(), "3 players are below the minimum");
        let (_, _) = m.request(ft(5), 3, 0xC0C0, 12).unwrap();
        let next = m.poll(13);
        assert_eq!(next[0].fireteams, vec![ft(3), ft(4), ft(5)]);
        assert_eq!(m.request(ft(6), 4, 0xC0C0, 14), Err(MatchmakerError::TooLarge { size: 4, max: 3 }));
    }

    #[test]
    fn team_feasibility_keeps_fireteams_whole() {
        assert!(teams_feasible(&[3, 3], 2, 3));
        assert!(!teams_feasible(&[2, 2, 2], 2, 3));
        assert!(teams_feasible(&[2, 1, 2, 1], 2, 3));
        assert!(teams_feasible(&[6, 6], 2, 6));
    }
}
