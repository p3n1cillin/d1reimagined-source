// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! # The Crucible: match lifecycle, modes, the scoreboard row, Iron Banner, Trials, rewards
//!
//! **Who owns what.** Crucible combat is peer-to-peer: one console is the *physics host*, as
//! Bungie built it, and the clients simulate every shot. The server owns what that design leaves
//! to it: the **match state** (lobby, start, rounds, the clock, the end), the **scoring**, the
//! **scoreboard row** the clients draw, and the **rewards**. That is this module.
//!
//! **The scoreboard is a script-driven row** (type 15, BullShcript's `scoreboard` item). Its state
//! is a global block, per-team and per-player stat blocks keyed by FNV-1 name hashes, and at most two
//! notifications. Content rules ("dominion rules") add to it when a named incident fires: each rule
//! names the incident, an audience, a scope mask (1 player, 2 team, 4 global) and a little stack
//! program. [`Scoreboard`] reproduces those semantics from Blam Network's BullShcript
//! (`src/host/natives/scoreboard_natives.ts`, `state/items.ts`; MIT, credited in THIRD_PARTY.md),
//! and [`ScoreboardState`] serialises with BullShcript's field names, so the script bridge and a
//! row encoder take it as is.
//!
//! **The rules are data** (`ps4/fixtures/56578/activities/crucible.json`): modes, maps,
//! playlists, Iron Banner and Trials. The game's own scoring lives in its content (the scoreboard
//! definition's rules), which is not exported yet; until it is, each mode's scoring and limits are
//! **stand-ins, labelled so**, written in the content's own rule shape so the export can replace
//! them value for value.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::checkpoints::CharacterId;
use crate::encounter_framework::{Catalog, Evidence};

/// FNV-1 (multiply, then xor): how this build stores every name (BullShcript `name_hash.ts`).
pub fn fnv1(s: &str) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for b in s.bytes() {
        h = h.wrapping_mul(0x0100_0193) ^ u32::from(b);
    }
    h
}

/// The scoreboard's `score` column, shared by a team's and a player's block.
pub fn score_hash() -> u32 {
    fnv1("score")
}

// ---------------------------------------------------------------------------------------------
// Data (crucible.json)
// ---------------------------------------------------------------------------------------------

/// Who a scoring rule credits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Credit {
    Player,
    Team,
    Both,
}

/// One scoring rule: when `incident` fires for a player, add `points` to `stat`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScoringDef {
    pub incident: String,
    pub points: f64,
    #[serde(default = "default_stat")]
    pub stat: String,
    pub credit: Credit,
}

fn default_stat() -> String {
    "score".into()
}

/// What happens when the clock runs out with the score level.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TieRule {
    #[default]
    Draw,
    /// The next point wins.
    SuddenDeath,
    /// Rounds: the team with more players alive takes the round.
    MostAlive,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoundRules {
    /// Round wins that take the match.
    pub wins_needed: u32,
    #[serde(default)]
    pub time_limit_s: Option<u32>,
    /// Players respawn during a round (false: Elimination, revives only).
    #[serde(default)]
    pub respawn: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModeDef {
    pub id: String,
    pub name: String,
    /// 0 for free-for-all.
    pub teams: u8,
    pub team_size: u8,
    pub max_players: u8,
    #[serde(default)]
    pub score_limit: Option<f64>,
    #[serde(default)]
    pub time_limit_s: Option<u32>,
    #[serde(default)]
    pub rounds: Option<RoundRules>,
    /// A lead this large ends the match early.
    #[serde(default)]
    pub mercy_lead: Option<f64>,
    #[serde(default)]
    pub tie: TieRule,
    pub scoring: Vec<ScoringDef>,
    /// What the mode plays for, besides kills: `zones`, `relic`, `sparks`, `crests`.
    #[serde(default)]
    pub objective: Option<String>,
    /// The manifest activity types (playlists) that play this mode.
    #[serde(default)]
    pub type_hashes: Vec<u32>,
    pub evidence: Evidence,
}

impl ModeDef {
    pub fn is_ffa(&self) -> bool {
        self.teams == 0
    }

    /// The mode's scoring as scoreboard rules, in the content's own shape: `var + points`.
    pub fn score_rules(&self) -> Vec<ScoreRule> {
        self.scoring
            .iter()
            .map(|s| ScoreRule {
                incident_name: fnv1(&s.incident),
                audience: fnv1("killing_player"),
                flags: match s.credit {
                    Credit::Player => SCOPE_PLAYER,
                    Credit::Team => SCOPE_TEAM,
                    Credit::Both => SCOPE_PLAYER | SCOPE_TEAM,
                },
                stat_name: fnv1(&s.stat),
                bytecode: vec![OP_PUSH_VAR, 0, OP_PUSH_CONST, 0, OP_ADD, OP_END],
                consts: vec![s.points],
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapDef {
    pub hash: u32,
    pub name: String,
    /// The restored script's `@activity` (all start-only stubs upstream).
    #[serde(default)]
    pub script: Option<String>,
    pub evidence: Evidence,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlaylistDef {
    pub hash: u32,
    pub name: String,
    pub mode: String,
    /// `classic`, `inferno`, `mayhem`, `freelance`, `iron_banner`, `trials`, `doubles` ...
    #[serde(default)]
    pub flavours: Vec<String>,
    /// Iron Banner and Trials: Light and level count in combat.
    #[serde(default)]
    pub level_advantages: bool,
    #[serde(default)]
    pub matchmade: bool,
    pub evidence: Evidence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Boon {
    /// Favor of Osiris: the card starts with one win.
    Favor,
    /// Mercy of Osiris: the first loss is forgiven.
    Mercy,
    /// Boon of Osiris: the first win counts twice.
    Boon,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrialsRules {
    pub mode: String,
    pub fireteam: u8,
    pub wins_for_lighthouse: u32,
    pub losses_to_end: u32,
    /// Win counts that unlock a reward package at the Trials vendor.
    pub reward_wins: Vec<u32>,
    pub boons: Vec<Boon>,
    pub playlists: Vec<u32>,
    pub evidence: Evidence,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IronBannerRules {
    pub playlists: Vec<u32>,
    pub level_advantages: bool,
    pub evidence: Evidence,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RewardRules {
    /// Reputation per completed match, by faction name (`Crucible`, `Iron Banner`).
    pub reputation_completed: BTreeMap<String, i64>,
    /// Added for a win.
    pub reputation_win_bonus: BTreeMap<String, i64>,
    /// Item hashes a post-game drop is drawn from (the manifest's playlist rewards).
    #[serde(default)]
    pub drop_items: Vec<u32>,
    /// Free-for-all: places counted as wins.
    pub ffa_winning_places: u32,
    pub evidence: Evidence,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrucibleData {
    #[serde(default)]
    pub comment: String,
    pub modes: Vec<ModeDef>,
    pub maps: Vec<MapDef>,
    pub playlists: Vec<PlaylistDef>,
    pub iron_banner: IronBannerRules,
    pub trials: TrialsRules,
    pub rewards: RewardRules,
}

/// The modes the Crucible must have.
pub const REQUIRED_MODES: [&str; 10] =
    ["control", "clash", "rumble", "skirmish", "salvage", "rift", "mayhem", "supremacy", "elimination", "trials"];

impl CrucibleData {
    pub fn mode(&self, id: &str) -> Option<&ModeDef> {
        self.modes.iter().find(|m| m.id == id)
    }

    pub fn playlist(&self, hash: u32) -> Option<&PlaylistDef> {
        self.playlists.iter().find(|p| p.hash == hash)
    }

    pub fn validate(&self, catalog: &Catalog) -> Vec<String> {
        let mut out = Vec::new();
        for id in REQUIRED_MODES {
            if self.mode(id).is_none() {
                out.push(format!("crucible: no {id} mode"));
            }
        }
        let mut seen = BTreeSet::new();
        for m in &self.modes {
            if !seen.insert(m.id.as_str()) {
                out.push(format!("crucible: duplicate mode {}", m.id));
            }
            if m.scoring.is_empty() {
                out.push(format!("crucible mode {}: no scoring", m.id));
            }
            if m.score_limit.is_none() && m.time_limit_s.is_none() && m.rounds.is_none() {
                out.push(format!("crucible mode {}: nothing ends it", m.id));
            }
            let size = if m.is_ffa() { m.team_size } else { m.teams * m.team_size };
            if size != m.max_players {
                out.push(format!("crucible mode {}: {} teams of {} is not {} players", m.id, m.teams, m.team_size, m.max_players));
            }
        }
        for p in &self.playlists {
            if self.mode(&p.mode).is_none() {
                out.push(format!("crucible playlist {} ({}): unknown mode {}", p.hash, p.name, p.mode));
            }
            match catalog.get(p.hash) {
                Some(c) if c.category == "crucible_playlist" => {}
                _ => out.push(format!("crucible playlist {} ({}) is not a manifest Crucible playlist", p.hash, p.name)),
            }
        }
        for c in catalog.activities.iter().filter(|c| c.category == "crucible_playlist") {
            if self.playlist(c.hash).is_none() {
                out.push(format!("manifest Crucible playlist {} ({}) has no playlist entry", c.hash, c.name.as_deref().unwrap_or("?")));
            }
        }
        for c in catalog.activities.iter().filter(|c| c.category == "crucible_map") {
            if !self.maps.iter().any(|m| m.hash == c.hash) {
                out.push(format!("manifest Crucible map {} ({}) has no map entry", c.hash, c.name.as_deref().unwrap_or("?")));
            }
        }
        if self.mode(&self.trials.mode).is_none() {
            out.push(format!("trials: unknown mode {}", self.trials.mode));
        }
        for h in self.iron_banner.playlists.iter().chain(&self.trials.playlists) {
            if self.playlist(*h).is_none() {
                out.push(format!("crucible: event playlist {h} has no playlist entry"));
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------
// The scoreboard row (BullShcript's semantics)
// ---------------------------------------------------------------------------------------------

pub const SCOPE_PLAYER: u8 = 1;
pub const SCOPE_TEAM: u8 = 2;
pub const SCOPE_GLOBAL: u8 = 4;

pub const OP_ADD: u8 = 0x01;
pub const OP_SUB: u8 = 0x02;
/// Read as divide (derived stats only; the true op is unknown upstream too).
pub const OP_DIV: u8 = 0x04;
/// Read as max (as above).
pub const OP_MAX: u8 = 0x09;
pub const OP_PUSH_CONST: u8 = 0x34;
pub const OP_PUSH_VAR: u8 = 0x3C;
pub const OP_END: u8 = 0x3E;

/// A content scoring rule (class `ScoreboardRule`).
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ScoreRule {
    pub incident_name: u32,
    pub audience: u32,
    pub flags: u8,
    pub stat_name: u32,
    pub bytecode: Vec<u8>,
    pub consts: Vec<f64>,
}

/// Run a rule's stack program. An unknown op stops it with the top of the stack.
pub fn run_bytecode(code: &[u8], consts: &[f64], var: impl Fn(u8) -> f64) -> f64 {
    let mut st: Vec<f64> = Vec::new();
    let mut i = 0;
    let pop2 = |st: &mut Vec<f64>| {
        let b = st.pop().unwrap_or(0.0);
        let a = st.pop().unwrap_or(0.0);
        (a, b)
    };
    while i < code.len() {
        let op = code[i];
        i += 1;
        match op {
            OP_PUSH_VAR => {
                st.push(var(code.get(i).copied().unwrap_or(0)));
                i += 1;
            }
            OP_PUSH_CONST => {
                st.push(consts.get(usize::from(code.get(i).copied().unwrap_or(0))).copied().unwrap_or(0.0));
                i += 1;
            }
            OP_ADD => {
                let (a, b) = pop2(&mut st);
                st.push(a + b);
            }
            OP_SUB => {
                let (a, b) = pop2(&mut st);
                st.push(a - b);
            }
            OP_MAX => {
                let (a, b) = pop2(&mut st);
                st.push(a.max(b));
            }
            OP_DIV => {
                let (a, b) = pop2(&mut st);
                st.push(if b == 0.0 { 0.0 } else { a / b });
            }
            _ => break,
        }
    }
    st.last().copied().unwrap_or(0.0)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatEntry {
    pub name_hash: u32,
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub character_key: Option<String>,
    pub name_hash: u32,
    /// -1 = every team.
    pub team: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStatsEntry {
    pub character_key: String,
    pub stats: Vec<StatEntry>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub team: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TeamStatsEntry {
    pub stats: Vec<StatEntry>,
    pub team: i32,
}

/// The row's state, with BullShcript's `ScoreboardState` field names.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ScoreboardState {
    pub global: Vec<StatEntry>,
    pub notifications: Vec<Notification>,
    pub players: Vec<PlayerStatsEntry>,
    pub teams: Vec<TeamStatsEntry>,
}

/// The scoreboard row's host-side state.
#[derive(Clone, Debug, Default)]
pub struct Scoreboard {
    global: BTreeMap<u32, f64>,
    notifications: VecDeque<Notification>,
    players: BTreeMap<CharacterId, BTreeMap<u32, f64>>,
    teams: BTreeMap<i32, BTreeMap<u32, f64>>,
    team_of: BTreeMap<CharacterId, i32>,
}

impl Scoreboard {
    pub fn set_team(&mut self, player: CharacterId, team: Option<i32>) {
        match team {
            Some(t) => {
                self.team_of.insert(player, t);
            }
            None => {
                self.team_of.remove(&player);
            }
        }
    }

    /// Run `rule` for `players` (and `team`, for one-team incidents), over every scope its flags
    /// name.
    pub fn apply_rule(&mut self, rule: &ScoreRule, players: &[CharacterId], team: Option<i32>) {
        let run = |cur: f64| run_bytecode(&rule.bytecode, &rule.consts, |_| cur);
        if rule.flags & SCOPE_GLOBAL != 0 {
            let v = run(self.global.get(&rule.stat_name).copied().unwrap_or(0.0));
            self.global.insert(rule.stat_name, v);
        }
        if rule.flags & SCOPE_PLAYER != 0 {
            for p in players {
                let s = self.players.entry(*p).or_default();
                let v = run(s.get(&rule.stat_name).copied().unwrap_or(0.0));
                s.insert(rule.stat_name, v);
            }
        }
        if rule.flags & SCOPE_TEAM != 0 {
            let mut teams: BTreeSet<i32> = team.into_iter().collect();
            teams.extend(players.iter().filter_map(|p| self.team_of.get(p).copied()));
            for t in teams {
                let s = self.teams.entry(t).or_default();
                let v = run(s.get(&rule.stat_name).copied().unwrap_or(0.0));
                s.insert(rule.stat_name, v);
            }
        }
    }

    /// `add_player_score(player, amount, also_team)`.
    pub fn add_player_score(&mut self, player: CharacterId, amount: f64, also_team: bool) {
        *self.players.entry(player).or_default().entry(score_hash()).or_default() += amount;
        if also_team {
            if let Some(t) = self.team_of.get(&player).copied() {
                *self.teams.entry(t).or_default().entry(score_hash()).or_default() += amount;
            }
        }
    }

    pub fn player_stat(&self, player: CharacterId, name_hash: u32) -> f64 {
        self.players.get(&player).and_then(|s| s.get(&name_hash)).copied().unwrap_or(0.0)
    }

    pub fn team_stat(&self, name_hash: u32, team: i32) -> f64 {
        self.teams.get(&team).and_then(|s| s.get(&name_hash)).copied().unwrap_or(0.0)
    }

    pub fn team_score(&self, team: i32) -> f64 {
        self.team_stat(score_hash(), team)
    }

    fn ranked_team_scores(&self) -> Vec<f64> {
        let mut v: Vec<f64> = self.teams.values().map(|s| s.get(&score_hash()).copied().unwrap_or(0.0)).collect();
        v.sort_by(|a, b| b.total_cmp(a));
        v
    }

    /// The top two team scores are equal (0-0 included), or fewer than two teams have scored.
    pub fn is_tied(&self) -> bool {
        let s = self.ranked_team_scores();
        s.len() < 2 || s[0] == s[1]
    }

    /// The leading team and its score; `None` when tied.
    pub fn leading_team(&self) -> Option<(i32, f64)> {
        if self.is_tied() {
            return None;
        }
        self.teams
            .iter()
            .map(|(t, s)| (*t, s.get(&score_hash()).copied().unwrap_or(0.0)))
            .max_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// The client keeps at most two; a third drops the oldest.
    pub fn activate_notification(&mut self, name_hash: u32, team: i32) {
        if self.notifications.len() >= 2 {
            self.notifications.pop_front();
        }
        self.notifications.push_back(Notification { character_key: None, name_hash, team });
    }

    pub fn clear_notifications(&mut self) {
        self.notifications.clear();
    }

    /// The row as the script bridge / encoder take it.
    pub fn state(&self) -> ScoreboardState {
        let entries = |m: &BTreeMap<u32, f64>| m.iter().map(|(&name_hash, &value)| StatEntry { name_hash, value }).collect();
        ScoreboardState {
            global: entries(&self.global),
            notifications: self.notifications.iter().cloned().collect(),
            players: self
                .players
                .iter()
                .map(|(p, s)| PlayerStatsEntry { character_key: p.to_string(), stats: entries(s), team: self.team_of.get(p).copied() })
                .collect(),
            teams: self.teams.iter().map(|(&team, s)| TeamStatsEntry { stats: entries(s), team }).collect(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The match
// ---------------------------------------------------------------------------------------------

/// Milliseconds on the host's clock.
pub type Millis = u64;

#[derive(Clone, Debug, PartialEq)]
pub enum MatchPhase {
    /// Waiting for players.
    Lobby,
    Countdown { starts_at: Millis },
    Playing { round: u32, ends_at: Option<Millis> },
    /// Rounds: between two rounds.
    RoundOver { round: u32, winner: Option<i32>, next_at: Millis },
    /// The clock ran out level; the next point wins.
    SuddenDeath { round: u32 },
    Ended,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MatchEvent {
    Join { player: CharacterId, team: Option<i32> },
    Leave(CharacterId),
    Tick(Millis),
    Kill { killer: Option<CharacterId>, victim: CharacterId, assists: Vec<CharacterId> },
    /// Any other scoring incident (a zone captured, a relic held, a spark scored, a crest taken).
    Incident { name: String, player: Option<CharacterId>, team: Option<i32> },
    Revive(CharacterId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Standing {
    pub player: CharacterId,
    pub team: Option<i32>,
    pub score: f64,
    pub kills: f64,
    pub deaths: f64,
    pub assists: f64,
    /// 1-based.
    pub place: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatchResult {
    /// `None`: a draw, or free-for-all.
    pub winning_team: Option<i32>,
    pub team_scores: BTreeMap<i32, f64>,
    pub round_wins: BTreeMap<i32, u32>,
    pub standings: Vec<Standing>,
    pub mercy: bool,
}

impl MatchResult {
    /// Whether `player` won: their team won, or (free-for-all) they placed in the top
    /// `ffa_places`.
    pub fn won(&self, player: CharacterId, ffa_places: u32) -> bool {
        let Some(s) = self.standings.iter().find(|s| s.player == player) else { return false };
        match (s.team, self.winning_team) {
            (Some(t), Some(w)) => t == w,
            (None, _) => s.place <= ffa_places,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum MatchEffect {
    Phase(MatchPhase),
    Scoreboard(ScoreboardState),
    RoundWon { round: u32, team: Option<i32> },
    Ended(MatchResult),
}

/// Lobby and countdown timings (stand-ins: the client's own pregame timer is content).
#[derive(Clone, Copy, Debug)]
pub struct Timings {
    pub min_players: usize,
    pub countdown_ms: Millis,
    pub round_break_ms: Millis,
}

impl Default for Timings {
    fn default() -> Self {
        Self { min_players: 2, countdown_ms: 10_000, round_break_ms: 5_000 }
    }
}

#[derive(Clone, Debug)]
struct Slot {
    team: Option<i32>,
    alive: bool,
}

/// One Crucible match.
pub struct CrucibleMatch {
    pub mode: Arc<ModeDef>,
    rules: Vec<ScoreRule>,
    timings: Timings,
    phase: MatchPhase,
    players: BTreeMap<CharacterId, Slot>,
    pub scoreboard: Scoreboard,
    round_wins: BTreeMap<i32, u32>,
    now: Millis,
    result: Option<MatchResult>,
}

impl CrucibleMatch {
    pub fn new(mode: Arc<ModeDef>, timings: Timings, now: Millis) -> Self {
        let rules = mode.score_rules();
        Self {
            mode,
            rules,
            timings,
            phase: MatchPhase::Lobby,
            players: BTreeMap::new(),
            scoreboard: Scoreboard::default(),
            round_wins: BTreeMap::new(),
            now,
            result: None,
        }
    }

    pub fn phase(&self) -> &MatchPhase {
        &self.phase
    }

    pub fn result(&self) -> Option<&MatchResult> {
        self.result.as_ref()
    }

    pub fn team_of(&self, p: CharacterId) -> Option<i32> {
        self.players.get(&p).and_then(|s| s.team)
    }

    pub fn round_wins(&self, team: i32) -> u32 {
        self.round_wins.get(&team).copied().unwrap_or(0)
    }

    fn teams(&self) -> Vec<i32> {
        (0..i32::from(self.mode.teams)).collect()
    }

    /// The team with fewest players (lowest id on a tie).
    fn emptiest_team(&self) -> Option<i32> {
        self.teams().into_iter().min_by_key(|t| self.players.values().filter(|s| s.team == Some(*t)).count())
    }

    fn set_phase(&mut self, p: MatchPhase, fx: &mut Vec<MatchEffect>) {
        self.phase = p.clone();
        fx.push(MatchEffect::Phase(p));
    }

    fn start_round(&mut self, round: u32, fx: &mut Vec<MatchEffect>) {
        for s in self.players.values_mut() {
            s.alive = true;
        }
        let limit = match &self.mode.rounds {
            Some(r) => r.time_limit_s,
            None => self.mode.time_limit_s,
        };
        let ends_at = limit.map(|s| self.now + Millis::from(s) * 1000);
        self.set_phase(MatchPhase::Playing { round, ends_at }, fx);
    }

    /// The leading team over every team of the mode (a team with no row yet has 0); `None` when
    /// the top two are level. The board's own [`Scoreboard::leading_team`] counts only teams
    /// with a row, as BullShcript's does.
    fn leader(&self) -> Option<(i32, f64)> {
        let mut s: Vec<(i32, f64)> = self.teams().into_iter().map(|t| (t, self.scoreboard.team_score(t))).collect();
        s.sort_by(|a, b| b.1.total_cmp(&a.1));
        match s.as_slice() {
            [first, second, ..] if first.1 > second.1 => Some(*first),
            [only] => Some(*only),
            _ => None,
        }
    }

    fn playing(&self) -> Option<u32> {
        match self.phase {
            MatchPhase::Playing { round, .. } | MatchPhase::SuddenDeath { round } => Some(round),
            _ => None,
        }
    }

    pub fn handle(&mut self, ev: MatchEvent) -> Vec<MatchEffect> {
        let mut fx = Vec::new();
        if self.phase == MatchPhase::Ended {
            return fx;
        }
        match ev {
            MatchEvent::Join { player, team } => {
                let team = if self.mode.is_ffa() { None } else { team.or_else(|| self.emptiest_team()) };
                let full = self.players.len() >= usize::from(self.mode.max_players);
                if full && !self.players.contains_key(&player) {
                    return fx;
                }
                // A player joining mid-round in a round mode waits for the next round.
                let alive = self.mode.rounds.is_none() || !matches!(self.phase, MatchPhase::Playing { .. });
                self.players.insert(player, Slot { team, alive });
                self.scoreboard.set_team(player, team);
                if self.phase == MatchPhase::Lobby && self.players.len() >= self.timings.min_players {
                    let at = self.now + self.timings.countdown_ms;
                    self.set_phase(MatchPhase::Countdown { starts_at: at }, &mut fx);
                }
            }
            MatchEvent::Leave(p) => {
                self.players.remove(&p);
                match self.phase {
                    MatchPhase::Countdown { .. } if self.players.len() < self.timings.min_players => {
                        self.set_phase(MatchPhase::Lobby, &mut fx)
                    }
                    MatchPhase::Lobby | MatchPhase::Countdown { .. } => {}
                    _ => {
                        if !self.mode.is_ffa() {
                            let staffed: Vec<i32> =
                                self.teams().into_iter().filter(|t| self.players.values().any(|s| s.team == Some(*t))).collect();
                            if staffed.len() <= 1 {
                                self.end(staffed.first().copied(), false, &mut fx);
                                return fx;
                            }
                        } else if self.players.len() <= 1 {
                            self.end(None, false, &mut fx);
                            return fx;
                        }
                        self.check_round_elimination(&mut fx);
                    }
                }
            }
            MatchEvent::Tick(now) => {
                self.now = now;
                match self.phase.clone() {
                    MatchPhase::Countdown { starts_at } if now >= starts_at => {
                        let fresh = self.scoreboard.state();
                        fx.push(MatchEffect::Scoreboard(fresh));
                        self.start_round(1, &mut fx);
                    }
                    MatchPhase::Playing { round, ends_at: Some(end) } if now >= end => self.time_up(round, &mut fx),
                    MatchPhase::RoundOver { round, next_at, .. } if now >= next_at => self.start_round(round + 1, &mut fx),
                    _ => {}
                }
            }
            MatchEvent::Kill { killer, victim, assists } => {
                if self.playing().is_none() {
                    return fx;
                }
                let respawns = self.mode.rounds.as_ref().is_none_or(|r| r.respawn);
                if !respawns {
                    if let Some(s) = self.players.get_mut(&victim) {
                        s.alive = false;
                    }
                }
                // A suicide or an environment kill scores no kill.
                if let Some(k) = killer.filter(|k| *k != victim) {
                    self.score_incident("player_kill", &[k], None);
                }
                self.score_incident("player_death", &[victim], None);
                if !assists.is_empty() {
                    self.score_incident("player_assist", &assists, None);
                }
                fx.push(MatchEffect::Scoreboard(self.scoreboard.state()));
                if !self.check_limits(&mut fx) {
                    self.check_round_elimination(&mut fx);
                }
            }
            MatchEvent::Revive(p) => {
                if self.playing().is_some() {
                    if let Some(s) = self.players.get_mut(&p) {
                        s.alive = true;
                    }
                    self.score_incident("player_revive", &[p], None);
                }
            }
            MatchEvent::Incident { name, player, team } => {
                if self.playing().is_none() {
                    return fx;
                }
                let players: Vec<CharacterId> = player.into_iter().collect();
                if self.score_incident(&name, &players, team) {
                    fx.push(MatchEffect::Scoreboard(self.scoreboard.state()));
                    self.check_limits(&mut fx);
                }
            }
        }
        fx
    }

    /// Apply every rule named `incident`; whether any matched.
    fn score_incident(&mut self, incident: &str, players: &[CharacterId], team: Option<i32>) -> bool {
        let h = fnv1(incident);
        let mut any = false;
        for r in self.rules.iter().filter(|r| r.incident_name == h) {
            self.scoreboard.apply_rule(r, players, team);
            any = true;
        }
        any
    }

    fn standings(&self) -> Vec<Standing> {
        let (score, kills, deaths, assists) = (score_hash(), fnv1("kills"), fnv1("deaths"), fnv1("assists"));
        let mut v: Vec<Standing> = self
            .players
            .iter()
            .map(|(&p, s)| Standing {
                player: p,
                team: s.team,
                score: self.scoreboard.player_stat(p, score),
                kills: self.scoreboard.player_stat(p, kills),
                deaths: self.scoreboard.player_stat(p, deaths),
                assists: self.scoreboard.player_stat(p, assists),
                place: 0,
            })
            .collect();
        v.sort_by(|a, b| b.score.total_cmp(&a.score).then(b.kills.total_cmp(&a.kills)).then(a.deaths.total_cmp(&b.deaths)));
        for (i, s) in v.iter_mut().enumerate() {
            s.place = u32::try_from(i + 1).unwrap_or(u32::MAX);
        }
        v
    }

    fn end(&mut self, winning_team: Option<i32>, mercy: bool, fx: &mut Vec<MatchEffect>) {
        let team_scores = self.teams().into_iter().map(|t| (t, self.scoreboard.team_score(t))).collect();
        let r = MatchResult { winning_team, team_scores, round_wins: self.round_wins.clone(), standings: self.standings(), mercy };
        self.result = Some(r.clone());
        self.set_phase(MatchPhase::Ended, fx);
        fx.push(MatchEffect::Ended(r));
    }

    /// Score limit, mercy and sudden death. Whether the match (or the round) ended.
    fn check_limits(&mut self, fx: &mut Vec<MatchEffect>) -> bool {
        if self.mode.rounds.is_some() {
            return false;
        }
        if self.mode.is_ffa() {
            let top = self.standings().first().map(|s| s.score).unwrap_or(0.0);
            if self.mode.score_limit.is_some_and(|l| top >= l) || matches!(self.phase, MatchPhase::SuddenDeath { .. }) {
                self.end(None, false, fx);
                return true;
            }
            return false;
        }
        if let Some((t, s)) = self.leader() {
            if self.mode.score_limit.is_some_and(|l| s >= l) || matches!(self.phase, MatchPhase::SuddenDeath { .. }) {
                self.end(Some(t), false, fx);
                return true;
            }
            if let Some(m) = self.mode.mercy_lead {
                let second = self.teams().into_iter().filter(|o| *o != t).map(|o| self.scoreboard.team_score(o)).fold(f64::MIN, f64::max);
                if s - second >= m {
                    self.end(Some(t), true, fx);
                    return true;
                }
            }
        }
        false
    }

    /// Rounds: a team with nobody alive loses the round.
    fn check_round_elimination(&mut self, fx: &mut Vec<MatchEffect>) {
        let (Some(rules), Some(round)) = (self.mode.rounds.clone(), self.playing()) else { return };
        if rules.respawn {
            return;
        }
        let alive: Vec<i32> = self
            .teams()
            .into_iter()
            .filter(|t| self.players.values().any(|s| s.team == Some(*t) && s.alive))
            .collect();
        match alive.len() {
            0 => self.round_over(round, None, &rules, fx),
            1 => self.round_over(round, Some(alive[0]), &rules, fx),
            _ => {}
        }
    }

    fn round_over(&mut self, round: u32, winner: Option<i32>, rules: &RoundRules, fx: &mut Vec<MatchEffect>) {
        fx.push(MatchEffect::RoundWon { round, team: winner });
        if let Some(t) = winner {
            let w = self.round_wins.entry(t).or_default();
            *w += 1;
            // The team's round count is its score on the board.
            *self.scoreboard.teams.entry(t).or_default().entry(score_hash()).or_default() += 1.0;
            fx.push(MatchEffect::Scoreboard(self.scoreboard.state()));
            if *w >= rules.wins_needed {
                self.end(Some(t), false, fx);
                return;
            }
        }
        let next_at = self.now + self.timings.round_break_ms;
        self.set_phase(MatchPhase::RoundOver { round, winner, next_at }, fx);
    }

    fn time_up(&mut self, round: u32, fx: &mut Vec<MatchEffect>) {
        if let Some(rules) = self.mode.rounds.clone() {
            let alive_of = |t: i32| self.players.values().filter(|s| s.team == Some(t) && s.alive).count();
            let winner = match self.mode.tie {
                TieRule::MostAlive => {
                    let mut counts: Vec<(i32, usize)> = self.teams().into_iter().map(|t| (t, alive_of(t))).collect();
                    counts.sort_by(|a, b| b.1.cmp(&a.1));
                    (counts.len() >= 2 && counts[0].1 > counts[1].1).then(|| counts[0].0)
                }
                _ => None,
            };
            self.round_over(round, winner, &rules, fx);
            return;
        }
        if self.mode.is_ffa() {
            self.end(None, false, fx);
            return;
        }
        match self.leader() {
            Some((t, _)) => self.end(Some(t), false, fx),
            None if self.mode.tie == TieRule::SuddenDeath => self.set_phase(MatchPhase::SuddenDeath { round }, fx),
            None => self.end(None, false, fx),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Rewards, Trials of Osiris
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PlayerRewards {
    pub won: bool,
    /// Faction name → reputation.
    pub reputation: BTreeMap<String, i64>,
}

/// What `player` earns from `result` on `playlist`.
pub fn rewards_for(result: &MatchResult, player: CharacterId, playlist: &PlaylistDef, rules: &RewardRules) -> PlayerRewards {
    let won = result.won(player, rules.ffa_winning_places);
    let faction = if playlist.flavours.iter().any(|f| f == "iron_banner") { "Iron Banner" } else { "Crucible" };
    let mut rep = BTreeMap::new();
    let base = rules.reputation_completed.get(faction).copied().unwrap_or(0);
    let bonus = if won { rules.reputation_win_bonus.get(faction).copied().unwrap_or(0) } else { 0 };
    if base + bonus != 0 {
        rep.insert(faction.to_owned(), base + bonus);
    }
    PlayerRewards { won, reputation: rep }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PassageOutcome {
    Continues,
    /// Nine wins. `flawless`: no loss on the card, the Lighthouse.
    Complete { flawless: bool },
    /// Three losses.
    Ended,
}

/// A Trials Passage card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrialsPassage {
    pub wins: u32,
    pub losses: u32,
    boons: BTreeSet<Boon>,
    mercy_spent: bool,
    boon_spent: bool,
}

impl TrialsPassage {
    pub fn new(boons: impl IntoIterator<Item = Boon>) -> Self {
        let boons: BTreeSet<Boon> = boons.into_iter().collect();
        let wins = u32::from(boons.contains(&Boon::Favor));
        Self { wins, losses: 0, boons, mercy_spent: false, boon_spent: false }
    }

    pub fn is_over(&self, rules: &TrialsRules) -> bool {
        self.wins >= rules.wins_for_lighthouse || self.losses >= rules.losses_to_end
    }

    /// Record a match.
    pub fn record(&mut self, won: bool, rules: &TrialsRules) -> PassageOutcome {
        if self.is_over(rules) {
            return self.outcome(rules);
        }
        if won {
            self.wins += 1;
            if self.boons.contains(&Boon::Boon) && !self.boon_spent {
                self.boon_spent = true;
                self.wins += 1;
            }
            self.wins = self.wins.min(rules.wins_for_lighthouse);
        } else if self.boons.contains(&Boon::Mercy) && !self.mercy_spent {
            self.mercy_spent = true;
        } else {
            self.losses += 1;
        }
        self.outcome(rules)
    }

    fn outcome(&self, rules: &TrialsRules) -> PassageOutcome {
        if self.wins >= rules.wins_for_lighthouse {
            PassageOutcome::Complete { flawless: self.losses == 0 }
        } else if self.losses >= rules.losses_to_end {
            PassageOutcome::Ended
        } else {
            PassageOutcome::Continues
        }
    }

    /// The reward thresholds reached.
    pub fn rewards_unlocked(&self, rules: &TrialsRules) -> Vec<u32> {
        rules.reward_wins.iter().copied().filter(|w| self.wins >= *w).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encounter_framework::EvidenceKind;

    fn ev() -> Evidence {
        Evidence { kind: EvidenceKind::StandIn, source: String::new() }
    }

    fn kills(points: f64) -> Vec<ScoringDef> {
        vec![
            ScoringDef { incident: "player_kill".into(), points, stat: "score".into(), credit: Credit::Both },
            ScoringDef { incident: "player_kill".into(), points: 1.0, stat: "kills".into(), credit: Credit::Player },
            ScoringDef { incident: "player_death".into(), points: 1.0, stat: "deaths".into(), credit: Credit::Player },
            ScoringDef { incident: "player_assist".into(), points: 1.0, stat: "assists".into(), credit: Credit::Player },
        ]
    }

    fn clash() -> Arc<ModeDef> {
        Arc::new(ModeDef {
            id: "clash".into(),
            name: "Clash".into(),
            teams: 2,
            team_size: 2,
            max_players: 4,
            score_limit: Some(300.0),
            time_limit_s: Some(600),
            rounds: None,
            mercy_lead: Some(250.0),
            tie: TieRule::SuddenDeath,
            scoring: kills(100.0),
            objective: None,
            type_hashes: vec![],
            evidence: ev(),
        })
    }

    fn elimination() -> Arc<ModeDef> {
        Arc::new(ModeDef {
            id: "elimination".into(),
            name: "Elimination".into(),
            teams: 2,
            team_size: 2,
            max_players: 4,
            score_limit: None,
            time_limit_s: None,
            rounds: Some(RoundRules { wins_needed: 2, time_limit_s: Some(120), respawn: false }),
            mercy_lead: None,
            tie: TieRule::MostAlive,
            scoring: kills(100.0),
            objective: None,
            type_hashes: vec![],
            evidence: ev(),
        })
    }

    fn started(mode: Arc<ModeDef>, players: &[CharacterId]) -> CrucibleMatch {
        let mut m = CrucibleMatch::new(mode, Timings::default(), 0);
        for &p in players {
            m.handle(MatchEvent::Join { player: p, team: None });
        }
        m.handle(MatchEvent::Tick(10_000));
        m
    }

    #[test]
    fn fnv1_matches_bullshcript() {
        // FNV-1 of the empty string is the offset basis; "a" is a published test vector.
        assert_eq!(fnv1(""), 0x811C_9DC5);
        assert_eq!(fnv1("a"), 0x050C_5D7E);
    }

    #[test]
    fn bytecode_ops() {
        let c = [OP_PUSH_VAR, 0, OP_PUSH_CONST, 0, OP_ADD, OP_END];
        assert_eq!(run_bytecode(&c, &[100.0], |_| 50.0), 150.0);
        let c = [OP_PUSH_VAR, 0, OP_PUSH_CONST, 1, OP_SUB, OP_END];
        assert_eq!(run_bytecode(&c, &[0.0, 7.0], |_| 10.0), 3.0);
        assert_eq!(run_bytecode(&[OP_PUSH_CONST, 0, OP_PUSH_CONST, 1, OP_DIV], &[1.0, 0.0], |_| 0.0), 0.0);
        assert_eq!(run_bytecode(&[OP_PUSH_CONST, 0, OP_PUSH_CONST, 1, OP_MAX], &[1.0, 4.0], |_| 0.0), 4.0);
        assert_eq!(run_bytecode(&[OP_PUSH_CONST, 0, 0x77, OP_PUSH_CONST, 1], &[9.0, 4.0], |_| 0.0), 9.0, "unknown op stops");
    }

    #[test]
    fn scoreboard_rules_and_notifications() {
        let mut sb = Scoreboard::default();
        sb.set_team(1, Some(0));
        sb.set_team(2, Some(1));
        let rule = ScoreRule {
            incident_name: fnv1("player_kill"),
            audience: 0,
            flags: SCOPE_PLAYER | SCOPE_TEAM | SCOPE_GLOBAL,
            stat_name: score_hash(),
            bytecode: vec![OP_PUSH_VAR, 0, OP_PUSH_CONST, 0, OP_ADD, OP_END],
            consts: vec![100.0],
        };
        assert!(sb.is_tied());
        sb.apply_rule(&rule, &[1], None);
        assert_eq!(sb.team_score(0), 100.0);
        assert_eq!(sb.leading_team(), None, "only one team has a row: still tied");
        sb.add_player_score(2, 50.0, true);
        assert_eq!(sb.leading_team(), Some((0, 100.0)));
        sb.activate_notification(fnv1("domination"), 0);
        sb.activate_notification(fnv1("domination"), 1);
        sb.activate_notification(fnv1("domination"), -1);
        let st = sb.state();
        assert_eq!(st.notifications.len(), 2);
        assert_eq!(st.notifications[0].team, 1, "the oldest dropped");
        assert_eq!(st.global, vec![StatEntry { name_hash: score_hash(), value: 100.0 }]);
        let js = serde_json::to_value(&st).unwrap();
        assert!(js["players"][0]["characterKey"].is_string());
        assert!(js["teams"][0]["stats"][0]["nameHash"].is_number());
        sb.clear_notifications();
        assert!(sb.state().notifications.is_empty());
    }

    #[test]
    fn lobby_countdown_and_start() {
        let mut m = CrucibleMatch::new(clash(), Timings::default(), 0);
        assert_eq!(m.phase(), &MatchPhase::Lobby);
        m.handle(MatchEvent::Join { player: 1, team: None });
        assert_eq!(m.phase(), &MatchPhase::Lobby);
        let fx = m.handle(MatchEvent::Join { player: 2, team: None });
        assert_eq!(fx, vec![MatchEffect::Phase(MatchPhase::Countdown { starts_at: 10_000 })]);
        assert_ne!(m.team_of(1), m.team_of(2), "teams are balanced");
        m.handle(MatchEvent::Leave(2));
        assert_eq!(m.phase(), &MatchPhase::Lobby, "the countdown stops below the minimum");
        m.handle(MatchEvent::Join { player: 2, team: None });
        m.handle(MatchEvent::Join { player: 3, team: None });
        m.handle(MatchEvent::Join { player: 4, team: None });
        assert!(m.handle(MatchEvent::Join { player: 5, team: None }).is_empty(), "full");
        m.handle(MatchEvent::Tick(9_999));
        assert!(matches!(m.phase(), MatchPhase::Countdown { .. }));
        m.handle(MatchEvent::Tick(10_000));
        assert_eq!(m.phase(), &MatchPhase::Playing { round: 1, ends_at: Some(610_000) });
    }

    #[test]
    fn score_limit_ends_the_match_and_the_board_follows() {
        let mut m = started(clash(), &[1, 2, 3, 4]);
        let t1 = m.team_of(1).unwrap();
        let foe = [2, 3, 4].into_iter().find(|p| m.team_of(*p) != Some(t1)).unwrap();
        m.handle(MatchEvent::Kill { killer: Some(1), victim: foe, assists: vec![] });
        m.handle(MatchEvent::Kill { killer: Some(foe), victim: 1, assists: vec![] });
        m.handle(MatchEvent::Kill { killer: Some(1), victim: foe, assists: vec![] });
        // A suicide scores no kill.
        m.handle(MatchEvent::Kill { killer: Some(1), victim: 1, assists: vec![] });
        assert_eq!(m.scoreboard.team_score(t1), 200.0);
        assert_eq!(m.scoreboard.player_stat(1, fnv1("deaths")), 2.0);
        let fx = m.handle(MatchEvent::Kill { killer: Some(1), victim: foe, assists: vec![] });
        let Some(MatchEffect::Ended(r)) = fx.last() else { panic!("{fx:?}") };
        assert_eq!(r.winning_team, Some(t1));
        assert!(!r.mercy);
        assert_eq!(r.standings[0].player, 1);
        assert_eq!(r.standings[0].kills, 3.0);
        assert!(r.won(1, 3) && !r.won(foe, 3));
        assert_eq!(m.phase(), &MatchPhase::Ended);
        assert!(m.handle(MatchEvent::Kill { killer: Some(1), victim: foe, assists: vec![] }).is_empty());
    }

    #[test]
    fn mercy_and_sudden_death() {
        let mut mode = (*clash()).clone();
        mode.score_limit = Some(10_000.0);
        let mut m = started(Arc::new(mode.clone()), &[1, 2]);
        m.handle(MatchEvent::Kill { killer: Some(1), victim: 2, assists: vec![] });
        m.handle(MatchEvent::Kill { killer: Some(1), victim: 2, assists: vec![] });
        let fx = m.handle(MatchEvent::Kill { killer: Some(1), victim: 2, assists: vec![] });
        assert!(matches!(fx.last(), Some(MatchEffect::Ended(r)) if r.mercy));
        // Level at the horn: sudden death, then the next kill wins.
        mode.mercy_lead = None;
        let mut m = started(Arc::new(mode), &[1, 2]);
        m.handle(MatchEvent::Kill { killer: Some(1), victim: 2, assists: vec![] });
        m.handle(MatchEvent::Kill { killer: Some(2), victim: 1, assists: vec![] });
        m.handle(MatchEvent::Tick(610_000));
        assert_eq!(m.phase(), &MatchPhase::SuddenDeath { round: 1 });
        let fx = m.handle(MatchEvent::Kill { killer: Some(2), victim: 1, assists: vec![] });
        assert!(matches!(fx.last(), Some(MatchEffect::Ended(r)) if r.winning_team == m.team_of(2)));
    }

    #[test]
    fn elimination_rounds() {
        let mut m = started(elimination(), &[1, 2, 3, 4]);
        let a = m.team_of(1).unwrap();
        let (mate, foes): (Vec<_>, Vec<_>) = [2, 3, 4].into_iter().partition(|p| m.team_of(*p) == Some(a));
        // Round 1: team a wipes the other.
        m.handle(MatchEvent::Kill { killer: Some(1), victim: foes[0], assists: vec![] });
        let fx = m.handle(MatchEvent::Kill { killer: Some(mate[0]), victim: foes[1], assists: vec![] });
        assert!(fx.contains(&MatchEffect::RoundWon { round: 1, team: Some(a) }));
        assert_eq!(m.round_wins(a), 1);
        assert!(matches!(m.phase(), MatchPhase::RoundOver { round: 1, .. }));
        m.handle(MatchEvent::Tick(15_000));
        assert!(matches!(m.phase(), MatchPhase::Playing { round: 2, .. }));
        // Round 2: a revive keeps team b in it; the clock then runs out with b ahead on players.
        m.handle(MatchEvent::Kill { killer: Some(foes[0]), victim: 1, assists: vec![] });
        m.handle(MatchEvent::Kill { killer: Some(foes[0]), victim: foes[1], assists: vec![] });
        m.handle(MatchEvent::Revive(foes[1]));
        let fx = m.handle(MatchEvent::Tick(15_000 + 120_000));
        assert!(fx.contains(&MatchEffect::RoundWon { round: 2, team: m.team_of(foes[0]) }), "{fx:?}");
        m.handle(MatchEvent::Tick(15_000 + 125_000));
        // Round 3 decides it (two wins needed).
        m.handle(MatchEvent::Kill { killer: Some(1), victim: foes[0], assists: vec![] });
        let fx = m.handle(MatchEvent::Kill { killer: Some(1), victim: foes[1], assists: vec![mate[0]] });
        let Some(MatchEffect::Ended(r)) = fx.last() else { panic!("{fx:?}") };
        assert_eq!(r.winning_team, Some(a));
        assert_eq!(r.round_wins.get(&a), Some(&2));
        assert_eq!(m.scoreboard.team_score(a), 2.0 + 400.0, "round wins plus kill points");
    }

    #[test]
    fn a_team_that_leaves_forfeits() {
        let mut m = started(clash(), &[1, 2]);
        let t1 = m.team_of(1);
        let fx = m.handle(MatchEvent::Leave(2));
        assert!(matches!(fx.last(), Some(MatchEffect::Ended(r)) if r.winning_team == t1));
    }

    #[test]
    fn free_for_all_places() {
        let mut mode = (*clash()).clone();
        mode.teams = 0;
        mode.team_size = 6;
        mode.max_players = 6;
        mode.mercy_lead = None;
        let mut m = started(Arc::new(mode), &[1, 2, 3, 4, 5]);
        assert_eq!(m.team_of(1), None);
        m.handle(MatchEvent::Kill { killer: Some(3), victim: 1, assists: vec![] });
        m.handle(MatchEvent::Kill { killer: Some(3), victim: 2, assists: vec![] });
        let fx = m.handle(MatchEvent::Kill { killer: Some(3), victim: 4, assists: vec![] });
        let Some(MatchEffect::Ended(r)) = fx.last() else { panic!("{fx:?}") };
        assert_eq!(r.standings[0].player, 3);
        assert!(r.won(3, 3));
        assert!(!r.won(4, 3), "4 placed below the top three");
    }

    fn trials() -> TrialsRules {
        TrialsRules {
            mode: "trials".into(),
            fireteam: 3,
            wins_for_lighthouse: 9,
            losses_to_end: 3,
            reward_wins: vec![5, 7, 9],
            boons: vec![Boon::Favor, Boon::Mercy, Boon::Boon],
            playlists: vec![],
            evidence: ev(),
        }
    }

    #[test]
    fn a_flawless_card_reaches_the_lighthouse() {
        let r = trials();
        let mut card = TrialsPassage::new([]);
        for _ in 0..8 {
            assert_eq!(card.record(true, &r), PassageOutcome::Continues);
        }
        assert_eq!(card.rewards_unlocked(&r), [5, 7]);
        assert_eq!(card.record(true, &r), PassageOutcome::Complete { flawless: true });
        assert_eq!(card.rewards_unlocked(&r), [5, 7, 9]);
        assert_eq!(card.record(false, &r), PassageOutcome::Complete { flawless: true }, "a finished card takes no more");
    }

    #[test]
    fn three_losses_end_a_card_and_boons_bend_it() {
        let r = trials();
        let mut card = TrialsPassage::new([]);
        card.record(false, &r);
        card.record(true, &r);
        card.record(false, &r);
        assert_eq!(card.record(false, &r), PassageOutcome::Ended);
        // Mercy forgives the first loss: still flawless.
        let mut card = TrialsPassage::new([Boon::Mercy]);
        card.record(false, &r);
        assert_eq!(card.losses, 0);
        for _ in 0..8 {
            card.record(true, &r);
        }
        assert_eq!(card.record(true, &r), PassageOutcome::Complete { flawless: true });
        // Favor starts at one; Boon doubles the first win.
        let mut card = TrialsPassage::new([Boon::Favor, Boon::Boon]);
        assert_eq!(card.wins, 1);
        card.record(true, &r);
        assert_eq!(card.wins, 3);
        card.record(true, &r);
        assert_eq!(card.wins, 4);
    }

    #[test]
    fn rewards_follow_the_playlist() {
        let mut m = started(clash(), &[1, 2]);
        for _ in 0..3 {
            m.handle(MatchEvent::Kill { killer: Some(1), victim: 2, assists: vec![] });
        }
        let r = m.result().cloned().unwrap();
        let rules = RewardRules {
            reputation_completed: [("Crucible".into(), 75), ("Iron Banner".into(), 100)].into(),
            reputation_win_bonus: [("Crucible".into(), 25), ("Iron Banner".into(), 50)].into(),
            drop_items: vec![],
            ffa_winning_places: 3,
            evidence: ev(),
        };
        let mut pl = PlaylistDef { hash: 1, name: "Clash".into(), mode: "clash".into(), flavours: vec![], level_advantages: false, matchmade: true, evidence: ev() };
        assert_eq!(rewards_for(&r, 1, &pl, &rules).reputation.get("Crucible"), Some(&100));
        assert_eq!(rewards_for(&r, 2, &pl, &rules).reputation.get("Crucible"), Some(&75));
        pl.flavours.push("iron_banner".into());
        let ib = rewards_for(&r, 1, &pl, &rules);
        assert!(ib.won);
        assert_eq!(ib.reputation.get("Iron Banner"), Some(&150));
        assert!(!ib.reputation.contains_key("Crucible"));
    }
}
