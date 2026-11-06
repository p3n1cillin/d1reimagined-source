// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! # Stream R: a Crucible match driven by the clients' own reports
//!
//! [`crate::pvp`] (#313) has the match: lobby, countdown, play, rounds, sudden death, mercy,
//! forfeits and results, the BullShcript-shaped scoreboard, and the Trials and Iron Banner rules.
//! Nothing fed it. [`MatchDriver`] does, for one matchmade instance, and [`MatchRegistry`] keeps one
//! driver per instance (activity host id) for every connection in it:
//!
//! * **It starts** when a Crucible map's instance forms: stream H's
//!   [`d1_mp::matchmaking::MatchFormed`] (its `match_id` is the instance's host id) names the
//!   playlist, the map and the fireteams; without one, the launch's activity table names the
//!   playlist (`playlist=` in `D1_AH_CRUCIBLE_ACTIVITIES`, else `D1_AH_CRUCIBLE_PLAYLIST`).
//! * **It advances on the clients' incidents** (udt 18), read by name hash through the 56578
//!   catalogue ([`incidents`]). The PvE captures show what a client reports about combat: every
//!   kill goes out as one chain led by `any_kill` (734) and `kill` (1825), all selector 1, with one
//!   metadata body; the dying player's `player_died` (2079); `player_resurrected` (3615). The
//!   Crucible adds `suicide`, `betrayal`, `assist_damage` and the zone captures, which the
//!   catalogue names. [`classify`] turns one udt 18 into [`Report`]s.
//! * **Who did it.** ⚠ **STAND-IN ATTRIBUTION.** The kill body is selector 1's metadata: the
//!   worldserver kill metadata class (`c_incident_metadata_worldserver_kill_interface`, vptr
//!   `0x190AF20`) names the type `0x80800AAB` (its slot 3 reads cell `0x26AAD40`, paired with
//!   name hash `0x3240684C`, which `0x80800AAB`'s descriptor carries), two `0x80800E70`
//!   participants at `+0x18` and `+0x58` whose `+0x28` is the activity player id (the rifle-pickup
//!   runs). Neither codec decodes a live body yet: d1-rsat lacks field kind `0x19`, and the
//!   client's codec faults (`0x7201F`) on the live bodies. So the driver credits **the reporting
//!   connection**: a `kill` chain to the reporter as killer, a `player_died` to the reporter as
//!   victim, and pairs a kill with a death from another team inside [`DriverConfig::pair_window_ms`].
//!   This is right only if each console reports its own player's kills and deaths; if the physics
//!   host reports every kill, it is wrong, and the note's evidence list says what settles it.
//! * **Rounds** (Elimination, Trials) end from the deaths themselves: the clients report no round
//!   end, so [`crate::pvp`]'s alive count decides.
//! * **The lifetime** goes 1 (pregame) at the instance's start, 2 (countdown, BullShcript's mapping,
//!   never seen in PvE; [`DriverConfig::countdown_lifetime`]), 3 (play), 6 (results: the final
//!   scoreboard first, in its own frame), 7 (post-game, 30 s after 6 as in 36 of 39 live
//!   activities) and 8 (the client's own end-of-activity transition: for a playlist, the rematch
//!   decision `0x75E600`), after [`DriverConfig::rematch_ms`], a stand-in.
//! * **Rewards** are granted once per player present at the results, through the host's one
//!   rewards trait ([`ActivityRewards`]; the forwarder under `D1_REWARDS=1`), as
//!   [`ActivityOutcome::Crucible`] with the Iron Banner faction and the Trials card applied.
//! * **The physics host.** Combat is peer-to-peer and the server simulates none of it: this module
//!   only reads reports. The instance's physics host is elected by [`crate::group`] (the listener
//!   turns shared instances on under `D1_AH_CRUCIBLE`); the driver records who holds it and logs a
//!   migration.
//!
//! **Placeholders.** Every score limit, time limit, mercy lead, round count, point value,
//! reputation amount, the countdown, the round break, the pairing window and the rematch delay is
//! a labelled stand-in (`crucible.json`'s evidence, [`DriverConfig`]). The notes say what a captured
//! match would settle (notes/ps4-56578-crucible-wire-2026-11-06.md).

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use crate::checkpoints::CharacterId;
use crate::pvp::{
    self, Boon, CrucibleData, CrucibleMatch, MatchEffect, MatchEvent, MatchPhase, MatchResult, Millis, ModeDef, PassageOutcome,
    PlaylistDef, PlayerStatsEntry, ScoreboardState, StatEntry, TeamStatsEntry, Timings, TrialsPassage,
};
use crate::rewards::{ActivityOutcome, ActivityRewards, CharacterRef};

/// Turns the driver on (`1`). Off by default: nothing changes with it unset.
pub const KNOB: &str = "D1_AH_CRUCIBLE";

/// Whether [`KNOB`] is set.
pub fn enabled() -> bool {
    std::env::var(KNOB).is_ok_and(|v| v.trim() == "1")
}

/// The client incidents the driver reads, by FNV-1 name hash (the 56578 catalogue's names; a test
/// checks each against `ps4/fixtures/56578/incidents-56578.tsv`).
pub mod incidents {
    /// 734, selector 1: leads every kill chain.
    pub const ANY_KILL: u32 = 0x324E_9A20;
    /// 1825, selector 1.
    pub const KILL: u32 = 0x790C_2A95;
    /// 2079, selector 1: the dying player's console.
    pub const PLAYER_DIED: u32 = 0x88CD_EB67;
    /// 3615, selector 13.
    pub const PLAYER_RESURRECTED: u32 = 0xEB1F_EE95;
    /// 643, selector 1.
    pub const SUICIDE: u32 = 0x2CF1_784F;
    /// 1435, selector 1: a kill of a teammate.
    pub const BETRAYAL: u32 = 0x6255_08BD;
    /// 2695, selector 1.
    pub const ASSIST_DAMAGE: u32 = 0xAFF7_735C;
    /// Control's captures, selector 6: `pvp_dominion_zone_captured` (774), `capture` (1338),
    /// `pvp_zone_captured_a` / `_b` / `_c` (2378, 2376, 2377).
    pub const ZONE_CAPTURED: [u32; 5] = [0x34FD_8483, 0x5AA6_AA6D, 0x9AB1_702B, 0x9AB1_7028, 0x9AB1_7029];
}

/// What one client report says about the match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Report {
    /// The reporter killed someone.
    Kill,
    /// The reporter died.
    Died,
    /// The reporter killed themselves.
    Suicide,
    /// The reporter killed a teammate.
    Betrayal,
    /// The reporter assisted a kill.
    Assist,
    /// The reporter was revived.
    Revived,
    /// The reporter captured a zone.
    ZoneCaptured,
}

/// One udt 18 (an incident chain, by name hash) as reports. A chain is one event: a suicide or a
/// betrayal replaces the plain kill its chain also names; a zone capture counts once.
pub fn classify(names: &[u32]) -> Vec<Report> {
    use incidents::*;
    let has = |h: u32| names.contains(&h);
    let mut out = Vec::new();
    if has(SUICIDE) {
        out.push(Report::Suicide);
    } else if has(BETRAYAL) {
        out.push(Report::Betrayal);
    } else if has(KILL) || has(ANY_KILL) {
        out.push(Report::Kill);
    }
    if has(PLAYER_DIED) {
        out.push(Report::Died);
    }
    if has(ASSIST_DAMAGE) {
        out.push(Report::Assist);
    }
    if has(PLAYER_RESURRECTED) {
        out.push(Report::Revived);
    }
    if names.iter().any(|h| ZONE_CAPTURED.contains(h)) {
        out.push(Report::ZoneCaptured);
    }
    out
}

/// The faction a match's reputation goes to, with its progression name in the manifest
/// (`ps4/fixtures/56578/progression/progression-curves.tsv`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Faction {
    Crucible,
    IronBanner,
}

impl Faction {
    /// The name `crucible.json`'s reward rules use.
    pub fn name(self) -> &'static str {
        match self {
            Self::Crucible => "Crucible",
            Self::IronBanner => "Iron Banner",
        }
    }

    /// The progression the store adds the reputation to.
    pub fn progression(self) -> &'static str {
        match self {
            Self::Crucible => "faction_pvp",
            Self::IronBanner => "faction_event_iron_banner",
        }
    }
}

/// Where a Trials card stands after a match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardState {
    Continues,
    /// Nine wins; `flawless`: no loss, the Lighthouse.
    Complete { flawless: bool },
    /// Three losses.
    Ended,
}

/// A Trials card after a match, and what this match changed on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrialsCard {
    pub wins: u8,
    pub losses: u8,
    pub state: CardState,
    /// Wins and losses this match added (a Boon win adds two; a Mercy loss adds none).
    pub wins_added: u8,
    pub losses_added: u8,
    /// Reward thresholds (5, 7, 9 wins) the card has reached.
    pub rewards_reached: u8,
}

/// ⭐ A Crucible match's outcome for one player, as [`ActivityOutcome::Crucible`] carries it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrucibleOutcome {
    /// The playlist's manifest hash (the activity the grant names: rewards belong to the playlist).
    pub playlist: u32,
    pub won: bool,
    pub mercy: bool,
    /// 1-based place on the scoreboard.
    pub place: u8,
    pub faction: Faction,
    /// Reputation for [`Self::faction`] (stand-in amounts, `crucible.json`).
    pub reputation: u32,
    /// Iron Banner and Trials: Light and level count in combat. The physics host applies it; the
    /// server only records it.
    pub level_advantages: bool,
    pub trials: Option<TrialsCard>,
}

/// Every player's Trials card (in memory: the player store has no card field yet). A card that is
/// over is replaced by a fresh one at the next match (stand-in for buying a new card).
#[derive(Debug, Default)]
pub struct TrialsCards {
    cards: HashMap<CharacterId, TrialsPassage>,
    boons: HashMap<CharacterId, Vec<Boon>>,
}

impl TrialsCards {
    /// The boons the next fresh card for `who` starts with (the vendor's sale).
    pub fn set_boons(&mut self, who: CharacterId, boons: Vec<Boon>) {
        self.boons.insert(who, boons);
    }

    pub fn card(&self, who: CharacterId) -> Option<&TrialsPassage> {
        self.cards.get(&who)
    }

    /// Record one match on `who`'s card.
    pub fn record(&mut self, who: CharacterId, won: bool, rules: &pvp::TrialsRules) -> TrialsCard {
        let boons = self.boons.get(&who).cloned().unwrap_or_default();
        let card = self.cards.entry(who).or_insert_with(|| TrialsPassage::new(boons.clone()));
        if card.is_over(rules) {
            tracing::info!("crucible: {who:#x}'s Trials card is over; a fresh card starts (stand-in for a new card)");
            *card = TrialsPassage::new(boons);
        }
        let (w0, l0) = (card.wins, card.losses);
        let state = match card.record(won, rules) {
            PassageOutcome::Continues => CardState::Continues,
            PassageOutcome::Complete { flawless } => CardState::Complete { flawless },
            PassageOutcome::Ended => CardState::Ended,
        };
        let small = |n: u32| u8::try_from(n).unwrap_or(u8::MAX);
        TrialsCard {
            wins: small(card.wins),
            losses: small(card.losses),
            state,
            wins_added: small(card.wins - w0),
            losses_added: small(card.losses - l0),
            rewards_reached: small(card.rewards_unlocked(rules).len() as u32),
        }
    }
}

/// Stand-in timings and the knobs of the driver.
#[derive(Clone, Debug)]
pub struct DriverConfig {
    /// The countdown before play (stand-in: the client's pregame timer is content).
    pub countdown_ms: Millis,
    /// Between two rounds (stand-in).
    pub round_break_ms: Millis,
    /// A kill and a death reported this close together are one kill (stand-in).
    pub pair_window_ms: Millis,
    /// 6 → 7: 30 s, as 36 of the 39 live activities that reached 7.
    pub postgame_ms: Millis,
    /// 7 → 8 (stand-in: no live PvP capture).
    pub rematch_ms: Millis,
    /// Serve 2 for the countdown (BullShcript's mapping; never seen in PvE). Off: 1 holds.
    pub countdown_lifetime: bool,
    /// The int typed value's tag for `score_to_win` (`scoreboard_rows`: a candidate, not a
    /// binding). `None`: no option is sent.
    pub score_to_win_tag: Option<u32>,
    /// The Trials ticket progressions' prefix (`.wins`, `.losses`), the manifest's names.
    pub trials_ticket: String,
}

impl Default for DriverConfig {
    fn default() -> Self {
        Self {
            countdown_ms: 10_000,
            round_break_ms: 5_000,
            pair_window_ms: 1_000,
            postgame_ms: 30_000,
            rematch_ms: 30_000,
            countdown_lifetime: true,
            score_to_win_tag: None,
            trials_ticket: "r1_s4_tickets.pvp.trials_of_osiris".into(),
        }
    }
}

impl DriverConfig {
    /// `D1_AH_CRUCIBLE_COUNTDOWN_LIFETIME=0` keeps 1 through the countdown;
    /// `D1_AH_CRUCIBLE_SCORE_TO_WIN=TAG` sends `score_to_win` with that int type tag;
    /// `D1_AH_CRUCIBLE_REMATCH_MS`, `D1_AH_CRUCIBLE_COUNTDOWN_MS`, `D1_AH_CRUCIBLE_TRIALS_TICKET`.
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Self {
        let mut c = Self::default();
        let num = |v: &str| {
            let v = v.trim();
            v.strip_prefix("0x").map_or_else(|| v.parse::<u64>().ok(), |h| u64::from_str_radix(h, 16).ok())
        };
        if var("D1_AH_CRUCIBLE_COUNTDOWN_LIFETIME").is_some_and(|v| v.trim() == "0") {
            c.countdown_lifetime = false;
        }
        c.score_to_win_tag = var("D1_AH_CRUCIBLE_SCORE_TO_WIN").and_then(|v| num(&v)).and_then(|v| u32::try_from(v).ok());
        if let Some(v) = var("D1_AH_CRUCIBLE_REMATCH_MS").and_then(|v| num(&v)) {
            c.rematch_ms = v;
        }
        if let Some(v) = var("D1_AH_CRUCIBLE_COUNTDOWN_MS").and_then(|v| num(&v)) {
            c.countdown_ms = v;
        }
        if let Some(v) = var("D1_AH_CRUCIBLE_TRIALS_TICKET").filter(|v| !v.trim().is_empty()) {
            c.trials_ticket = v.trim().to_string();
        }
        c
    }
}

/// What forms an instance's match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchSetup {
    /// The instance's activity host id.
    pub host: u64,
    /// The map (the activity the clients launched).
    pub map: u32,
    pub playlist: u32,
    /// Players the match waits for before its countdown (`MatchFormed::players`); `None`: two.
    pub expected_players: Option<usize>,
    /// The fireteams in arrival order (`MatchFormed::fireteams`).
    pub fireteams: Vec<u64>,
}

impl MatchSetup {
    pub fn from_formed(f: &d1_mp::matchmaking::MatchFormed) -> Self {
        Self {
            host: f.match_id,
            map: f.activity_hash,
            playlist: f.playlist,
            expected_players: Some(f.players),
            fireteams: f.fireteams.iter().map(|t| t.0).collect(),
        }
    }
}

/// `crucible-match HOST PLAYLIST MAP PLAYERS [FIRETEAM,...]`: a formed match handed across the
/// process boundary (the responder runs stream H's matchmaker), one line per match in
/// `D1_AH_CRUCIBLE_MATCHES`.
pub fn parse_match_line(line: &str) -> Result<MatchSetup, String> {
    let f: Vec<&str> = line.split_whitespace().collect();
    let num = |s: &str| {
        s.strip_prefix("0x").map_or_else(|| s.parse::<u64>().ok(), |h| u64::from_str_radix(h, 16).ok()).ok_or_else(|| format!("{line:?}: {s:?} is not a number"))
    };
    let ["crucible-match", host, playlist, map, players, rest @ ..] = f.as_slice() else {
        return Err(format!("{line:?}: want `crucible-match HOST PLAYLIST MAP PLAYERS [FIRETEAM,...]`"));
    };
    let u32_of = |s: &str| num(s).and_then(|v| u32::try_from(v).map_err(|_| format!("{line:?}: {s} is not 32-bit")));
    let fireteams = match rest {
        [] => Vec::new(),
        [list] => list.split(',').map(&num).collect::<Result<_, _>>()?,
        _ => return Err(format!("{line:?}: the fireteams are one comma-separated field")),
    };
    Ok(MatchSetup {
        host: num(host)?,
        playlist: u32_of(playlist)?,
        map: u32_of(map)?,
        expected_players: Some(usize::try_from(num(players)?).map_err(|e| e.to_string())?),
        fireteams,
    })
}

/// One player of the match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub character: CharacterRef,
    /// The membership slot ([`crate::group`]), when the instance is shared.
    pub slot: Option<usize>,
    /// The fireteam its join names (`join_request` properties `+0xA0`).
    pub fireteam: u64,
    pub team: Option<i32>,
    pub present: bool,
}

impl Member {
    pub fn new(character: CharacterRef, slot: Option<usize>, fireteam: u64) -> Self {
        Self { character, slot, fireteam, team: None, present: true }
    }

    pub fn id(&self) -> CharacterId {
        crate::activity_driver::character_id(&self.character)
    }
}

/// What the driver publishes for every connection of the instance to put on its udt 5, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum Publish {
    /// The lifetime value, and `score_to_win` when it is sent.
    Lifetime { state: i8, score_to_win: Option<(u32, i32)> },
    Scoreboard(ScoreboardState),
}

#[derive(Clone, Copy, Debug)]
struct PendingKill {
    killer: CharacterId,
    kind: Report,
    at: Millis,
}

/// What every driver in the process shares.
pub struct Shared {
    pub data: Arc<CrucibleData>,
    pub config: DriverConfig,
    pub rewards: Option<Arc<Mutex<dyn ActivityRewards + Send>>>,
    pub trials: Mutex<TrialsCards>,
}

impl Shared {
    pub fn new(data: Arc<CrucibleData>, config: DriverConfig, rewards: Option<Arc<Mutex<dyn ActivityRewards + Send>>>) -> Arc<Self> {
        Arc::new(Self { data, config, rewards, trials: Mutex::new(TrialsCards::default()) })
    }
}

/// ⭐ One instance's match.
pub struct MatchDriver {
    pub setup: MatchSetup,
    shared: Arc<Shared>,
    playlist: PlaylistDef,
    mode: Arc<ModeDef>,
    m: CrucibleMatch,
    members: BTreeMap<CharacterId, Member>,
    now: Millis,
    lifetime: i8,
    results_at: Option<Millis>,
    postgame_at: Option<Millis>,
    kills: VecDeque<PendingKill>,
    deaths: VecDeque<(CharacterId, Millis)>,
    assists: VecDeque<(CharacterId, Millis)>,
    published: Vec<Publish>,
    last_board: Option<ScoreboardState>,
    grants: Vec<(CharacterId, CrucibleOutcome)>,
    physics_host: Option<CharacterId>,
    serial: u64,
}

/// Each match gets its own number, so a connection can tell a rematch on the same host.
static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl std::fmt::Debug for MatchDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MatchDriver").field("host", &self.setup.host).field("mode", &self.mode.id).field("phase", self.m.phase()).finish()
    }
}

impl MatchDriver {
    /// A match of `setup`'s playlist, in its lobby (lifetime 1, pregame).
    pub fn new(setup: MatchSetup, shared: Arc<Shared>, now: Millis) -> Result<Self, String> {
        let playlist = shared.data.playlist(setup.playlist).cloned().ok_or_else(|| format!("{:#010x} is not a Crucible playlist", setup.playlist))?;
        let mode = Arc::new(shared.data.mode(&playlist.mode).cloned().ok_or_else(|| format!("playlist {} plays unknown mode {}", playlist.name, playlist.mode))?);
        if !shared.data.maps.iter().any(|m| m.hash == setup.map) {
            tracing::warn!("crucible: {:#010x} is not one of the Crucible maps; the match runs anyway", setup.map);
        }
        let min_players = setup.expected_players.unwrap_or(2).clamp(1, usize::from(mode.max_players).max(1));
        let timings = Timings { min_players, countdown_ms: shared.config.countdown_ms, round_break_ms: shared.config.round_break_ms };
        let m = CrucibleMatch::new(Arc::clone(&mode), timings, now);
        tracing::info!(
            "⭐ crucible: host {:#018x} plays {} ({}) on {:#010x}; waits for {min_players} player(s)",
            setup.host, playlist.name, mode.name, setup.map
        );
        let mut d = Self {
            setup,
            shared,
            playlist,
            mode,
            m,
            members: BTreeMap::new(),
            now,
            lifetime: 0,
            results_at: None,
            postgame_at: None,
            kills: VecDeque::new(),
            deaths: VecDeque::new(),
            assists: VecDeque::new(),
            published: Vec::new(),
            last_board: None,
            grants: Vec::new(),
            physics_host: None,
            serial: SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        };
        d.set_lifetime(1);
        Ok(d)
    }

    pub fn mode(&self) -> &ModeDef {
        &self.mode
    }

    pub fn playlist(&self) -> &PlaylistDef {
        &self.playlist
    }

    pub fn phase(&self) -> &MatchPhase {
        self.m.phase()
    }

    pub fn lifetime(&self) -> i8 {
        self.lifetime
    }

    /// This match's number (a rematch on the same host has a new one).
    pub fn serial(&self) -> u64 {
        self.serial
    }

    /// Rounds `team` has won.
    pub fn round_wins(&self, team: i32) -> u32 {
        self.m.round_wins(team)
    }

    pub fn result(&self) -> Option<&MatchResult> {
        self.m.result()
    }

    pub fn members(&self) -> impl Iterator<Item = &Member> {
        self.members.values()
    }

    pub fn member(&self, id: CharacterId) -> Option<&Member> {
        self.members.get(&id)
    }

    /// Every grant made at the results: `(character, outcome)`.
    pub fn grants(&self) -> &[(CharacterId, CrucibleOutcome)] {
        &self.grants
    }

    /// Everything published so far, in order; a connection keeps its own cursor.
    pub fn published(&self) -> &[Publish] {
        &self.published
    }

    /// The last lifetime and scoreboard published: what a connection joining now must be sent.
    pub fn current(&self) -> Vec<Publish> {
        let mut v = Vec::new();
        if let Some(b) = &self.last_board {
            v.push(Publish::Scoreboard(b.clone()));
        }
        v.push(self.lifetime_publish(self.lifetime));
        v
    }

    /// The match has run its course (8 published).
    pub fn over(&self) -> bool {
        self.lifetime == 8
    }

    pub fn physics_host(&self) -> Option<CharacterId> {
        self.physics_host
    }

    fn score_to_win(&self) -> Option<(u32, i32)> {
        let tag = self.shared.config.score_to_win_tag?;
        // ⚠ STAND-IN values (crucible.json): the mode's score limit, or a round mode's wins.
        let v = match (&self.mode.rounds, self.mode.score_limit) {
            (Some(r), _) => f64::from(r.wins_needed),
            (None, Some(l)) => l,
            (None, None) => return None,
        };
        Some((tag, v as i32))
    }

    fn lifetime_publish(&self, state: i8) -> Publish {
        Publish::Lifetime { state, score_to_win: self.score_to_win() }
    }

    fn set_lifetime(&mut self, state: i8) {
        if self.lifetime == state {
            return;
        }
        tracing::info!("⭐ crucible: host {:#018x} lifetime {} -> {state}", self.setup.host, self.lifetime);
        self.lifetime = state;
        let p = self.lifetime_publish(state);
        self.published.push(p);
    }

    /// The scoreboard as the clients are sent it: every player and every team of the mode, each
    /// with a row, whether or not they have scored (BullShcript's state lists only those with a
    /// stat).
    pub fn scoreboard(&self) -> ScoreboardState {
        let mut s = self.m.scoreboard.state();
        for m in self.members.values() {
            let key = m.id().to_string();
            if !s.players.iter().any(|p| p.character_key == key) {
                s.players.push(PlayerStatsEntry { character_key: key, stats: Vec::new(), team: m.team });
            }
        }
        for p in &mut s.players {
            if p.stats.is_empty() {
                p.stats = vec![StatEntry { name_hash: pvp::score_hash(), value: 0.0 }];
            }
        }
        for t in 0..i32::from(self.mode.teams) {
            if !s.teams.iter().any(|e| e.team == t) {
                s.teams.push(TeamStatsEntry { stats: vec![StatEntry { name_hash: pvp::score_hash(), value: 0.0 }], team: t });
            }
        }
        s.teams.sort_by_key(|t| t.team);
        s
    }

    fn publish_board(&mut self) {
        let b = self.scoreboard();
        if self.last_board.as_ref() != Some(&b) {
            self.last_board = Some(b.clone());
            self.published.push(Publish::Scoreboard(b));
        }
    }

    /// A team for `fireteam`: its fireteam-mates' team, else the team with fewest players.
    /// ⚠ Teams are the client's `team_picker`'s, peer-side (stream H); this keeps fireteams
    /// together the way the matchmaker grouped them, and the scoreboard rows carry it.
    fn team_for(&self, fireteam: u64) -> Option<i32> {
        if self.mode.is_ffa() {
            return None;
        }
        if fireteam != 0 {
            if let Some(t) = self.members.values().find(|m| m.fireteam == fireteam && m.team.is_some()).and_then(|m| m.team) {
                return Some(t);
            }
        }
        (0..i32::from(self.mode.teams)).min_by_key(|t| self.members.values().filter(|m| m.present && m.team == Some(*t)).count())
    }

    /// A player joined the instance (or joined again).
    pub fn join(&mut self, mut member: Member, now: Millis) {
        self.advance_clock(now);
        let id = member.id();
        if let Some(m) = self.members.get_mut(&id) {
            m.present = true;
            m.slot = member.slot.or(m.slot);
            tracing::info!("crucible: {id:#x} is back in host {:#018x}", self.setup.host);
            let fx = self.m.handle(MatchEvent::Join { player: id, team: m.team });
            self.apply(fx);
            self.publish_board();
            return;
        }
        member.team = self.team_for(member.fireteam);
        tracing::info!("crucible: {id:#x} joins host {:#018x} on team {:?}", self.setup.host, member.team);
        let team = member.team;
        self.members.insert(id, member);
        let fx = self.m.handle(MatchEvent::Join { player: id, team });
        // The match may have put a full mode's player elsewhere; keep what it decided.
        if let Some(m) = self.members.get_mut(&id) {
            m.team = self.m.team_of(id).or(team);
        }
        self.apply(fx);
        self.publish_board();
    }

    /// A player left (its connection closed). A match in play may end by forfeit.
    pub fn leave(&mut self, id: CharacterId, now: Millis) {
        self.advance_clock(now);
        let Some(m) = self.members.get_mut(&id) else { return };
        if !m.present {
            return;
        }
        m.present = false;
        tracing::info!("crucible: {id:#x} left host {:#018x}", self.setup.host);
        if self.physics_host == Some(id) {
            self.physics_host = None;
        }
        if self.lifetime < 6 {
            let fx = self.m.handle(MatchEvent::Leave(id));
            self.apply(fx);
        }
    }

    /// The instance's physics host ([`crate::group`]'s election), as a member's character.
    pub fn set_physics_host(&mut self, id: Option<CharacterId>) {
        if self.physics_host != id {
            tracing::info!("⭐ crucible: host {:#018x} physics host {:?} -> {:?} (combat runs on that console; the server simulates none)", self.setup.host, self.physics_host, id);
            self.physics_host = id;
        }
    }

    /// One udt 18 from `reporter`'s connection, as name hashes.
    pub fn report(&mut self, reporter: CharacterId, names: &[u32], now: Millis) {
        self.advance_clock(now);
        if !self.members.contains_key(&reporter) {
            tracing::warn!("crucible: a report from {reporter:#x}, who is not in host {:#018x}; dropped", self.setup.host);
            return;
        }
        for r in classify(names) {
            match r {
                Report::Kill | Report::Suicide | Report::Betrayal => {
                    self.kills.push_back(PendingKill { killer: reporter, kind: r, at: now });
                }
                Report::Died => self.deaths.push_back((reporter, now)),
                Report::Assist => self.assists.push_back((reporter, now)),
                Report::Revived => {
                    let fx = self.m.handle(MatchEvent::Revive(reporter));
                    self.apply(fx);
                }
                Report::ZoneCaptured => {
                    let team = self.members.get(&reporter).and_then(|m| m.team);
                    let fx = self.m.handle(MatchEvent::Incident { name: "zone_captured".into(), player: Some(reporter), team });
                    self.apply(fx);
                }
            }
        }
        self.pair();
    }

    /// Pair pending kills with pending deaths.
    fn pair(&mut self) {
        let team = |s: &Self, id: CharacterId| s.members.get(&id).and_then(|m| m.team);
        let mut i = 0;
        while i < self.deaths.len() {
            let (victim, _) = self.deaths[i];
            let found = self.kills.iter().position(|k| match k.kind {
                Report::Suicide => k.killer == victim,
                Report::Betrayal => k.killer != victim && team(self, k.killer) == team(self, victim),
                _ => k.killer != victim && (self.mode.is_ffa() || team(self, k.killer) != team(self, victim)),
            });
            match found {
                Some(k) => {
                    let k = self.kills.remove(k).expect("found");
                    self.deaths.remove(i);
                    let killer = (k.kind == Report::Kill).then_some(k.killer);
                    self.kill(killer, victim);
                }
                None => i += 1,
            }
        }
    }

    fn kill(&mut self, killer: Option<CharacterId>, victim: CharacterId) {
        let team = |s: &Self, id: CharacterId| s.members.get(&id).and_then(|m| m.team);
        let assists: Vec<CharacterId> = match killer {
            Some(k) => {
                let kt = team(self, k);
                let mine: Vec<CharacterId> =
                    self.assists.iter().map(|a| a.0).filter(|a| *a != k && *a != victim && (self.mode.is_ffa() || team(self, *a) == kt)).collect();
                self.assists.retain(|a| !mine.contains(&a.0));
                mine
            }
            None => Vec::new(),
        };
        let fx = self.m.handle(MatchEvent::Kill { killer, victim, assists });
        self.apply(fx);
    }

    /// Unpaired reports past the window: a kill nobody died for still scores the kill; a death
    /// nobody claimed is a death with no killer.
    fn flush(&mut self, now: Millis) {
        let window = self.shared.config.pair_window_ms;
        while let Some(k) = self.kills.front().copied().filter(|k| now.saturating_sub(k.at) >= window) {
            self.kills.pop_front();
            if k.kind == Report::Kill {
                let fx = self.m.handle(MatchEvent::Incident { name: "player_kill".into(), player: Some(k.killer), team: None });
                self.apply(fx);
            }
        }
        while let Some((victim, _)) = self.deaths.front().copied().filter(|d| now.saturating_sub(d.1) >= window) {
            self.deaths.pop_front();
            let fx = self.m.handle(MatchEvent::Kill { killer: None, victim, assists: Vec::new() });
            self.apply(fx);
        }
        self.assists.retain(|a| now.saturating_sub(a.1) < window);
    }

    fn advance_clock(&mut self, now: Millis) {
        self.now = self.now.max(now);
    }

    /// The clock: pairing, the match's own timers, and the end's phases.
    pub fn tick(&mut self, now: Millis) {
        self.advance_clock(now);
        let now = self.now;
        self.flush(now);
        let fx = self.m.handle(MatchEvent::Tick(now));
        self.apply(fx);
        if let (Some(at), 6) = (self.results_at, self.lifetime) {
            if now >= at + self.shared.config.postgame_ms {
                self.set_lifetime(7);
                self.postgame_at = Some(now);
            }
        }
        if let (Some(at), 7) = (self.postgame_at, self.lifetime) {
            if now >= at + self.shared.config.rematch_ms {
                self.set_lifetime(8);
            }
        }
    }

    fn apply(&mut self, fx: Vec<MatchEffect>) {
        let mut board = false;
        for e in fx {
            match e {
                MatchEffect::Phase(p) => match p {
                    MatchPhase::Lobby => self.set_lifetime(1),
                    MatchPhase::Countdown { .. } => self.set_lifetime(if self.shared.config.countdown_lifetime { 2 } else { 1 }),
                    MatchPhase::Playing { .. } | MatchPhase::RoundOver { .. } | MatchPhase::SuddenDeath { .. } => {
                        if board {
                            self.publish_board();
                            board = false;
                        }
                        self.set_lifetime(3)
                    }
                    MatchPhase::Ended => {}
                },
                MatchEffect::Scoreboard(_) => board = true,
                MatchEffect::RoundWon { round, team } => {
                    tracing::info!("⭐ crucible: host {:#018x} round {round} to {team:?}", self.setup.host);
                }
                MatchEffect::Ended(r) => {
                    // The final board goes out before the results: 0xAB9B80 reads the UI store at 6.
                    self.publish_board();
                    board = false;
                    self.finish(&r);
                }
            }
        }
        if board {
            self.publish_board();
        }
    }

    /// The results: lifetime 6, and each present player's grant, once.
    fn finish(&mut self, r: &MatchResult) {
        if self.results_at.is_some() {
            return;
        }
        tracing::info!(
            "⭐ crucible: host {:#018x} ends: winner {:?}, scores {:?}, rounds {:?}{}",
            self.setup.host, r.winning_team, r.team_scores, r.round_wins, if r.mercy { ", mercy" } else { "" }
        );
        self.results_at = Some(self.now);
        self.set_lifetime(6);
        self.grant(r);
    }

    fn grant(&mut self, r: &MatchResult) {
        let data = Arc::clone(&self.shared.data);
        let faction = if self.playlist.flavours.iter().any(|f| f == "iron_banner") || data.iron_banner.playlists.contains(&self.playlist.hash) {
            Faction::IronBanner
        } else {
            Faction::Crucible
        };
        let trials = self.playlist.flavours.iter().any(|f| f == "trials") || data.trials.playlists.contains(&self.playlist.hash);
        let present: Vec<Member> = self.members.values().filter(|m| m.present).cloned().collect();
        for m in &present {
            let id = m.id();
            let pr = pvp::rewards_for(r, id, &self.playlist, &data.rewards);
            let rep = pr.reputation.get(faction.name()).copied().unwrap_or(0).max(0);
            let place = r.standings.iter().find(|s| s.player == id).map_or(0, |s| s.place);
            let card = trials.then(|| self.shared.trials.lock().map(|mut t| t.record(id, pr.won, &data.trials)).ok()).flatten();
            let o = CrucibleOutcome {
                playlist: self.playlist.hash,
                won: pr.won,
                mercy: r.mercy,
                place: u8::try_from(place).unwrap_or(u8::MAX),
                faction,
                reputation: u32::try_from(rep).unwrap_or(0),
                level_advantages: self.playlist.level_advantages,
                trials: card,
            };
            self.grants.push((id, o));
        }
        let Some(rewards) = self.shared.rewards.clone() else {
            tracing::info!("crucible: host {:#018x}: {} outcome(s), no rewards service; nothing granted", self.setup.host, self.grants.len());
            return;
        };
        let Ok(mut rw) = rewards.lock() else {
            tracing::warn!("crucible: the rewards service lock is poisoned");
            return;
        };
        for (m, (_, o)) in present.iter().zip(self.grants.iter()) {
            if let Err(e) = rw.grant_activity_rewards(&m.character, self.playlist.hash, &ActivityOutcome::Crucible(*o)) {
                tracing::warn!("crucible: rewards for {:?}: {e}", m.character);
            }
        }
        tracing::info!("⭐ crucible: host {:#018x}: granted {} player(s)", self.setup.host, self.grants.len());
    }
}

/// ⭐ Every Crucible instance in the process, by activity host id.
pub struct MatchRegistry {
    shared: Arc<Shared>,
    matches: HashMap<u64, MatchDriver>,
    formed: HashMap<u64, MatchSetup>,
}

pub type SharedRegistry = Arc<Mutex<MatchRegistry>>;

impl MatchRegistry {
    pub fn new(shared: Arc<Shared>) -> Self {
        Self { shared, matches: HashMap::new(), formed: HashMap::new() }
    }

    pub fn shared(shared: Arc<Shared>) -> SharedRegistry {
        Arc::new(Mutex::new(Self::new(shared)))
    }

    pub fn config(&self) -> &Shared {
        &self.shared
    }

    /// Stream H formed a match: its instance (`match_id`, the host id) plays this.
    pub fn formed(&mut self, setup: MatchSetup) {
        tracing::info!("crucible: match formed for host {:#018x}: playlist {:#010x} on {:#010x}", setup.host, setup.playlist, setup.map);
        self.formed.insert(setup.host, setup);
    }

    pub fn get(&self, host: u64) -> Option<&MatchDriver> {
        self.matches.get(&host)
    }

    pub fn get_mut(&mut self, host: u64) -> Option<&mut MatchDriver> {
        self.matches.get_mut(&host)
    }

    /// `member` joined `host`, an instance of Crucible map `map`. The first join starts the match:
    /// from the formed match if one names the host, else `playlist` (the launch's). A join after a
    /// finished match (8) starts the next one on the same host (a rematch).
    pub fn join(&mut self, host: u64, map: u32, playlist: Option<u32>, member: Member, now: Millis) -> Result<(), String> {
        if self.matches.get(&host).is_some_and(|d| d.over()) {
            self.matches.remove(&host);
        }
        if !self.matches.contains_key(&host) {
            let setup = match self.formed.get(&host) {
                Some(s) => s.clone(),
                None => MatchSetup {
                    host,
                    map,
                    playlist: playlist.ok_or_else(|| format!("host {host:#018x}: no formed match and no playlist for map {map:#010x}"))?,
                    expected_players: None,
                    fireteams: Vec::new(),
                },
            };
            let d = MatchDriver::new(setup, Arc::clone(&self.shared), now)?;
            self.matches.insert(host, d);
        }
        self.matches.get_mut(&host).expect("just made").join(member, now);
        Ok(())
    }

    pub fn report(&mut self, host: u64, reporter: CharacterId, names: &[u32], now: Millis) {
        if let Some(d) = self.matches.get_mut(&host) {
            d.report(reporter, names, now);
        }
    }

    pub fn leave(&mut self, host: u64, who: CharacterId, now: Millis) {
        if let Some(d) = self.matches.get_mut(&host) {
            d.leave(who, now);
            if d.members().all(|m| !m.present) && (d.over() || d.lifetime() < 3) {
                tracing::info!("crucible: host {host:#018x} is empty; its match is dropped");
                self.matches.remove(&host);
            }
        }
    }

    pub fn tick(&mut self, host: u64, now: Millis) {
        if let Some(d) = self.matches.get_mut(&host) {
            d.tick(now);
        }
    }

    /// Read new `crucible-match` lines from `path` past byte `offset`; returns the new offset.
    pub fn poll_formed_file(&mut self, path: &std::path::Path, offset: u64) -> u64 {
        let Ok(text) = std::fs::read(path) else { return offset };
        let start = usize::try_from(offset).unwrap_or(usize::MAX).min(text.len());
        let end = text[start..].iter().rposition(|b| *b == b'\n').map_or(start, |p| start + p + 1);
        for line in String::from_utf8_lossy(&text[start..end]).lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            match parse_match_line(line) {
                Ok(s) => self.formed(s),
                Err(e) => tracing::warn!("crucible: {}: {e}", path.display()),
            }
        }
        end as u64
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::rewards::RecordingRewards;

    pub(crate) fn data() -> Arc<CrucibleData> {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/56578/activities/crucible.json");
        Arc::new(serde_json::from_str(&std::fs::read_to_string(p).expect("crucible.json")).expect("crucible.json parses"))
    }

    /// A playlist of `mode` with no event flavour (lowest hash, for a stable choice).
    pub(crate) fn playlist_of(d: &CrucibleData, mode: &str, flavour: Option<&str>) -> u32 {
        let mut v: Vec<&PlaylistDef> = d
            .playlists
            .iter()
            .filter(|p| p.mode == mode)
            .filter(|p| match flavour {
                Some(f) => p.flavours.iter().any(|x| x == f),
                None => !p.flavours.iter().any(|x| x == "iron_banner" || x == "trials"),
            })
            .collect();
        v.sort_by_key(|p| p.hash);
        v.first().unwrap_or_else(|| panic!("a {mode} playlist ({flavour:?})")).hash
    }

    pub(crate) fn identity(n: u64, fireteam: u64) -> CharacterRef {
        let mut b = vec![0u8; 40];
        b[..8].copy_from_slice(&(0x1000 + n).to_le_bytes());
        b[8..16].copy_from_slice(&(0x5000 + n).to_le_bytes());
        b[0x10..0x18].copy_from_slice(&(0xD101_0000_0000_0001 + (n << 12)).to_le_bytes());
        b[0x18..0x20].copy_from_slice(&(0xD100_0000_0000_0100 + n).to_le_bytes());
        b[0x20..0x28].copy_from_slice(&fireteam.to_le_bytes());
        CharacterRef { identity: b }
    }

    pub(crate) fn id(n: u64) -> CharacterId {
        0xD100_0000_0000_0100 + n
    }

    fn shared(rewards: &Arc<Mutex<RecordingRewards>>) -> Arc<Shared> {
        let r: Arc<Mutex<dyn ActivityRewards + Send>> = rewards.clone();
        Shared::new(data(), DriverConfig::default(), Some(r))
    }

    fn map(d: &CrucibleData) -> u32 {
        let mut m: Vec<u32> = d.maps.iter().map(|m| m.hash).collect();
        m.sort_unstable();
        m[0]
    }

    fn lifetimes(d: &MatchDriver) -> Vec<i8> {
        d.published().iter().filter_map(|p| if let Publish::Lifetime { state, .. } = p { Some(*state) } else { None }).collect()
    }

    #[test]
    fn the_incidents_are_the_catalogues() {
        let cat = crate::incident_catalog::IncidentCatalog::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/56578/incidents-56578.tsv"),
        )
        .expect("the committed catalogue");
        use incidents::*;
        for (name, hash, index) in [
            ("any_kill", ANY_KILL, 734),
            ("kill", KILL, 1825),
            ("player_died", PLAYER_DIED, 2079),
            ("player_resurrected", PLAYER_RESURRECTED, 3615),
            ("suicide", SUICIDE, 643),
            ("betrayal", BETRAYAL, 1435),
            ("assist_damage", ASSIST_DAMAGE, 2695),
            ("pvp_dominion_zone_captured", ZONE_CAPTURED[0], 774),
            ("capture", ZONE_CAPTURED[1], 1338),
            ("pvp_zone_captured_a", ZONE_CAPTURED[2], 2378),
            ("pvp_zone_captured_b", ZONE_CAPTURED[3], 2376),
            ("pvp_zone_captured_c", ZONE_CAPTURED[4], 2377),
        ] {
            assert_eq!(crate::pvp::fnv1(name), hash, "{name}");
            assert_eq!(cat.index_of_name(name), Some(index), "{name}");
            assert_eq!(cat.name_hash(index), Some(hash), "{name}");
        }
    }

    #[test]
    fn a_chain_is_one_event() {
        use incidents::*;
        // The live PvE kill chain: any_kill, kill, then qualifiers.
        assert_eq!(classify(&[ANY_KILL, KILL, 0x9B60_AB3F, 0xA3B5_012E]), vec![Report::Kill]);
        assert_eq!(classify(&[PLAYER_DIED]), vec![Report::Died]);
        assert_eq!(classify(&[ANY_KILL, SUICIDE, PLAYER_DIED]), vec![Report::Suicide, Report::Died]);
        assert_eq!(classify(&[ANY_KILL, KILL, BETRAYAL]), vec![Report::Betrayal]);
        assert_eq!(classify(&[ZONE_CAPTURED[0], ZONE_CAPTURED[1], ZONE_CAPTURED[2]]), vec![Report::ZoneCaptured]);
        assert_eq!(classify(&[ASSIST_DAMAGE]), vec![Report::Assist]);
        assert_eq!(classify(&[PLAYER_RESURRECTED]), vec![Report::Revived]);
        assert!(classify(&[0x1234_5678]).is_empty());
    }

    #[test]
    fn a_kill_and_a_death_pair_and_the_rest_flush_alone() {
        let rewards = Arc::new(Mutex::new(RecordingRewards::default()));
        let d = data();
        let mut m = MatchDriver::new(
            MatchSetup { host: 7, map: map(&d), playlist: playlist_of(&d, "control", None), expected_players: Some(2), fireteams: vec![] },
            shared(&rewards),
            0,
        )
        .unwrap();
        m.join(Member::new(identity(1, 11), Some(0), 11), 0);
        m.join(Member::new(identity(2, 22), Some(1), 22), 0);
        assert_ne!(m.member(id(1)).unwrap().team, m.member(id(2)).unwrap().team, "two fireteams, two teams");
        m.tick(10_000);
        assert_eq!(m.lifetime(), 3);
        use incidents::*;
        m.report(id(1), &[ANY_KILL, KILL], 10_100);
        m.report(id(2), &[PLAYER_DIED], 10_300);
        let sb = &m.m.scoreboard;
        assert_eq!(sb.player_stat(id(1), pvp::fnv1("kills")), 1.0);
        assert_eq!(sb.player_stat(id(2), pvp::fnv1("deaths")), 1.0);
        // A kill with no death: scored at the window's end; a death alone: a death, no killer.
        m.report(id(2), &[ANY_KILL, KILL], 11_000);
        m.tick(11_500);
        assert_eq!(m.m.scoreboard.player_stat(id(2), pvp::fnv1("kills")), 0.0, "still pairing");
        m.tick(12_100);
        assert_eq!(m.m.scoreboard.player_stat(id(2), pvp::fnv1("kills")), 1.0);
        m.report(id(1), &[PLAYER_DIED], 13_000);
        m.tick(14_100);
        assert_eq!(m.m.scoreboard.player_stat(id(1), pvp::fnv1("deaths")), 1.0);
        assert_eq!(m.m.scoreboard.player_stat(id(2), pvp::fnv1("kills")), 1.0, "nobody claimed it");
        // A suicide is a death with no killer.
        m.report(id(1), &[SUICIDE, PLAYER_DIED], 15_000);
        assert_eq!(m.m.scoreboard.player_stat(id(1), pvp::fnv1("deaths")), 2.0);
        assert_eq!(m.m.scoreboard.player_stat(id(1), pvp::fnv1("kills")), 1.0, "the suicide scores no kill");
        // A report from outside the instance is dropped.
        m.report(id(9), &[ANY_KILL, KILL], 15_500);
    }

    #[test]
    fn a_formed_match_line_round_trips() {
        let s = parse_match_line("crucible-match 0xd1a0000000000042 0x1234 0x5678 12 0x11,0x22").unwrap();
        assert_eq!(s, MatchSetup { host: 0xD1A0_0000_0000_0042, playlist: 0x1234, map: 0x5678, expected_players: Some(12), fireteams: vec![0x11, 0x22] });
        assert!(parse_match_line("crucible-match 1 2").is_err());
        let f = d1_mp::matchmaking::MatchFormed {
            playlist: 1,
            activity_hash: 2,
            match_id: 3,
            fireteams: vec![d1_mp::identity::FireteamId(4)],
            players: 6,
            joined_in_progress: false,
        };
        assert_eq!(MatchSetup::from_formed(&f), MatchSetup { host: 3, map: 2, playlist: 1, expected_players: Some(6), fireteams: vec![4] });
    }

    #[test]
    fn the_end_runs_6_then_7_then_8_and_grants_once() {
        let rewards = Arc::new(Mutex::new(RecordingRewards::default()));
        let d = data();
        let mut m = MatchDriver::new(
            MatchSetup { host: 7, map: map(&d), playlist: playlist_of(&d, "control", None), expected_players: Some(2), fireteams: vec![] },
            shared(&rewards),
            0,
        )
        .unwrap();
        m.join(Member::new(identity(1, 11), Some(0), 11), 0);
        m.join(Member::new(identity(2, 22), Some(1), 22), 0);
        m.tick(10_000);
        // A forfeit ends it: the other team leaves.
        m.leave(id(2), 20_000);
        assert_eq!(m.lifetime(), 6);
        assert_eq!(rewards.lock().unwrap().grants.len(), 1, "only the player present");
        m.tick(49_999);
        assert_eq!(m.lifetime(), 6);
        m.tick(50_000);
        assert_eq!(m.lifetime(), 7);
        m.tick(80_000);
        assert_eq!(m.lifetime(), 8);
        assert_eq!(lifetimes(&m), vec![1, 2, 3, 6, 7, 8]);
        assert_eq!(rewards.lock().unwrap().grants.len(), 1, "once");
    }
}
