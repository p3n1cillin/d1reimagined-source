// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! # The multi-stage activity framework: strikes, raids, arenas
//!
//! A strike, a raid, Prison of Elders, Challenge of the Elders and Archon's Forge all run the same
//! way on the server: a fixed list of **encounters**, each opened by something the world reports,
//! some of them **checkpoints**, some of them **darkness zones** where a fireteam that dies
//! together **wipes** and restarts at the last checkpoint. **Modifiers** (the manifest's skulls)
//! change the rules: under Nightfall a wipe returns the fireteam to orbit ("If all players die,
//! your Fireteam is returned to Orbit": the skull's own text in the manifest).
//!
//! Everything about an activity is **data** ([`ActivityDef`], read from
//! `ps4/fixtures/56578/activities/definitions/*.json`). A new activity needs a definition, not
//! code. The runtime is [`ActivityRun`]: it takes [`Event`]s and answers with [`Effect`]s, and it
//! never touches the wire.
//!
//! ## Who drives it
//!
//! * **The script bridge** (streams A1/A2: the BullShcript sidecar). Its `phase` message becomes
//!   [`Event::ScriptPhase`], `all_dead` becomes [`Event::AllPlayersDead`], `activity_complete`
//!   becomes [`Event::ActivityCompleted`]. An encounter that opens on `{"script_phase": NAME}`
//!   follows the restored script's own phase list.
//! * **The host itself**, for activities with no restored script. The client's slice-set reports
//!   (udt 21) become [`Event::SliceEntered`] and its trigger reports ([`crate::state::trigger_report`])
//!   become [`Event::Trigger`]. Encounters described from the live captures open on exactly those
//!   ([`CaptureSequence`], written by `tools/ps4_setup/d1-activity-sequence.py`).
//!
//! ## The checkpoint store
//!
//! A raid's checkpoint persists per character for the week through
//! [`crate::checkpoints::CheckpointStore`]; the run loads the fireteam leader's at start, saves
//! every member's as an encounter completes, and spends it on a full clear.
//!
//! ## Rewards
//!
//! The run decides *that* a reward is due, never what the post-game grants: [`Effect::ActivityCompleted`]
//! (and [`Effect::ReturnedToOrbit`] for a failure) is the call into stream M's
//! `grant_activity_rewards(character, activity_hash, outcome)` on the responder side, once per
//! present character. [`Effect::Rewards`] carries the encounter's loot table (labelled; mostly
//! community stand-ins) and [`Effect::RewardsLocked`] the weekly lockout.
//!
//! ## Evidence
//!
//! Every definition and every encounter says where it came from ([`Evidence`]). Values that are not
//! the game's own data are labelled (`community`, `d1-knowledge`, `stand-in`), and gaps are listed
//! in `needs_evidence` rather than filled in. See notes/ps4-56578-strikes-raids-crucible-2026-11-05.md.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::checkpoints::{self, CharacterId, Checkpoint, CheckpointKey, CheckpointStore, UnixSeconds};

// ---------------------------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------------------------

/// Where a value came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceKind {
    /// Bungie's manifest (the final D1 manifest, or the mobile world database).
    Manifest,
    /// Blam Network's restored 56578 mission scripts (Fukugen Daihon), read locally: identifiers
    /// and structure only, never their text.
    Script,
    /// Bungie's live servers, from the community's Xbox 360 captures (build 51539).
    LiveCapture,
    /// Derived from the game's data by a rule stated in `source` (for example a playlist's
    /// members, which the manifest does not list).
    Inferred,
    /// A community source, named in `source` (for example the Cryptarch guide's loot tables).
    Community,
    /// How D1 played, from the project's knowledge of the game; not from its data.
    D1Knowledge,
    /// A placeholder value so the framework runs; replace it from the game's content.
    StandIn,
    /// Nothing yet: the entry exists so the gap is visible.
    NeedsEvidence,
}

impl EvidenceKind {
    /// The game's own data or live-server behaviour (as opposed to community or placeholder).
    pub fn is_primary(self) -> bool {
        matches!(self, Self::Manifest | Self::Script | Self::LiveCapture)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub kind: EvidenceKind,
    #[serde(default)]
    pub source: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    Strike,
    Raid,
    /// Prison of Elders: rounds against waves, a boss in the last.
    Arena,
    /// Challenge of the Elders: a scored arena.
    ArenaChallenge,
    /// Archon's Forge: a public event inside a patrol (the Plaguelands).
    PublicEvent,
}

/// What persists between runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointPolicy {
    /// Nothing persists; a wipe still restarts at the last checkpoint inside the run.
    #[default]
    InActivity,
    /// The raid rule: the checkpoint and the loot lockouts persist per character until the weekly
    /// reset.
    Weekly,
}

/// What opens an encounter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    /// The previous encounter completed ([`Event::EncounterCompleted`]).
    PreviousComplete,
    /// The script bridge entered this phase (the restored script's `@phase` row name).
    ScriptPhase(String),
    /// The client first reported this slice set (udt 21; `bubble << 3 | state`).
    SliceEntered(u32),
    /// The client crossed this trigger: `0xGROUP/KIND/INDEX` (incident 3403's `trigger_report`).
    Trigger(String),
    /// Only an explicit [`Event::BeginEncounter`] opens it.
    Manual,
}

impl Default for Trigger {
    fn default() -> Self {
        Self::PreviousComplete
    }
}

/// `0xGROUP/KIND/INDEX` → (group, kind, index).
pub fn parse_trigger_name(s: &str) -> Option<(u32, u8, u16)> {
    let mut f = s.split('/');
    let g = f.next()?.trim();
    let g = u32::from_str_radix(g.strip_prefix("0x").or_else(|| g.strip_prefix("0X"))?, 16).ok()?;
    let k = f.next()?.trim().parse().ok()?;
    let i = f.next()?.trim().parse().ok()?;
    f.next().is_none().then_some((g, k, i))
}

/// A reward an encounter or activity pays. `items` are manifest item hashes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RewardDef {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub items: Vec<u32>,
    pub evidence: Evidence,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncounterDef {
    pub id: String,
    pub name: String,
    /// The slice (bubble) it plays in, by name or number, where known.
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub begins_on: Trigger,
    /// A wipe restarts here once the encounter has begun.
    #[serde(default)]
    pub checkpoint: bool,
    /// A darkness zone: players do not respawn, so a fireteam that is all dead wipes. Restored
    /// scripts mark these phases `@hard_wipe`.
    #[serde(default)]
    pub darkness: bool,
    /// The boss, when the encounter is a boss fight.
    #[serde(default)]
    pub boss: Option<String>,
    /// A time limit; running out fails the encounter.
    #[serde(default)]
    pub time_limit_s: Option<u32>,
    #[serde(default)]
    pub rewards: Vec<RewardDef>,
    pub evidence: Evidence,
}

/// The restored script that drives an activity (identifiers only).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptRef {
    /// The `@activity` name.
    pub activity: String,
    /// The `@activity_bundle` (the root's file stem).
    pub bundle: String,
    /// The `@phase` rows in lifetime order.
    #[serde(default)]
    pub phases: Vec<String>,
    /// `true` when the root has only start decorators (no phases): a content hole upstream.
    #[serde(default)]
    pub start_only_stub: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FireteamSize {
    pub min: u8,
    pub max: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActivityDef {
    pub id: String,
    pub name: String,
    pub kind: ActivityKind,
    /// Every manifest activity hash this definition runs (difficulties, Heroic, Nightfall,
    /// playlist entries).
    #[serde(default)]
    pub activity_hashes: Vec<u32>,
    #[serde(default)]
    pub bundle_hashes: Vec<u32>,
    pub fireteam: FireteamSize,
    #[serde(default)]
    pub checkpoints: CheckpointPolicy,
    pub encounters: Vec<EncounterDef>,
    /// Skulls this activity can run with (manifest names: `Heroic`, `Nightfall`, ...).
    #[serde(default)]
    pub modifiers: Vec<String>,
    #[serde(default)]
    pub rewards: Vec<RewardDef>,
    #[serde(default)]
    pub script: Option<ScriptRef>,
    /// A capture-derived sequence under `sequences/` (file stem).
    #[serde(default)]
    pub sequence: Option<String>,
    /// Challenge of the Elders: score thresholds for the end rewards.
    #[serde(default)]
    pub score_thresholds: Vec<i64>,
    pub evidence: Evidence,
    #[serde(default)]
    pub needs_evidence: Vec<String>,
}

impl ActivityDef {
    pub fn encounter_index(&self, id: &str) -> Option<usize> {
        self.encounters.iter().position(|e| e.id == id)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DefinitionFile {
    #[serde(default)]
    pub comment: String,
    pub activities: Vec<ActivityDef>,
}

// ----- The manifest catalog (catalog.json, tools/ps4_setup/d1-activities-export.py) -----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogActivity {
    pub hash: u32,
    #[serde(default)]
    pub name: Option<String>,
    pub category: String,
    #[serde(default)]
    pub type_name: Option<String>,
    #[serde(default)]
    pub bundle_hash: Option<u32>,
    #[serde(default)]
    pub destination: Option<String>,
    #[serde(default)]
    pub level: i32,
    #[serde(default)]
    pub tier: i32,
    #[serde(default)]
    pub min_party: u8,
    #[serde(default)]
    pub max_party: u8,
    #[serde(default)]
    pub max_players: u8,
    #[serde(default)]
    pub matchmade: bool,
    #[serde(default)]
    pub playlist: bool,
    #[serde(default)]
    pub skulls: Vec<Option<String>>,
    #[serde(default)]
    pub rewards: Vec<Vec<u32>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogPlaylist {
    pub hash: u32,
    pub name: String,
    pub category: String,
    #[serde(default)]
    pub members: Vec<u32>,
    pub membership: Evidence,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogSkull {
    pub hash: u32,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Catalog {
    pub activities: Vec<CatalogActivity>,
    #[serde(default)]
    pub playlists: Vec<CatalogPlaylist>,
    #[serde(default)]
    pub skulls: Vec<CatalogSkull>,
}

impl Catalog {
    pub fn get(&self, hash: u32) -> Option<&CatalogActivity> {
        self.activities.iter().find(|a| a.hash == hash)
    }

    /// Every skull name the manifest uses: the scripted-skull table and the skulls activity rows
    /// carry (Heroic, Nightfall and Epic appear only on the rows).
    pub fn skull_names(&self) -> HashSet<&str> {
        self.skulls
            .iter()
            .filter_map(|s| s.name.as_deref())
            .chain(self.activities.iter().flat_map(|a| a.skulls.iter().flatten().map(String::as_str)))
            .collect()
    }
}

// ----- Capture-derived sequences (sequences/*.json) -----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaptureSegment {
    pub index: usize,
    pub begins_on: Trigger,
    pub at_s: f64,
    pub duration_s: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaptureStage {
    pub slice_set: u32,
    pub entered_s: f64,
    #[serde(default)]
    pub groups_first_served: Vec<String>,
    #[serde(default)]
    pub triggers: Vec<String>,
    #[serde(default)]
    pub duration_s: f64,
}

/// One thing the client reported, in time order (a first slice set or a first trigger crossing;
/// cinematic markers are kept in the file but carry neither).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaptureMarker {
    pub t_s: f64,
    #[serde(default)]
    pub slice: Option<u32>,
    #[serde(default)]
    pub trigger: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaptureSequence {
    pub activity_id: String,
    pub name: String,
    pub evidence: serde_json::Value,
    pub host_session_s: f64,
    pub stages: Vec<CaptureStage>,
    #[serde(default)]
    pub markers: Vec<CaptureMarker>,
    pub segments: Vec<CaptureSegment>,
}

impl CaptureSequence {
    /// The live run's reports as framework events, in order: replaying them through an
    /// [`ActivityRun`] walks the definition the way the live run went.
    pub fn replay_events(&self) -> Vec<Event> {
        self.markers
            .iter()
            .filter_map(|m| match (m.slice, &m.trigger) {
                (Some(s), _) => Some(Event::SliceEntered(s)),
                (None, Some(t)) => parse_trigger_name(t).map(|(group, kind, index)| Event::Trigger { group, kind, index }),
                _ => None,
            })
            .collect()
    }

    /// Whether the run this was captured from reported `t`.
    pub fn saw(&self, t: &Trigger) -> bool {
        match t {
            Trigger::SliceEntered(s) => self.stages.iter().any(|st| st.slice_set == *s),
            Trigger::Trigger(name) => self.stages.iter().any(|st| st.triggers.iter().any(|x| x == name)),
            _ => true,
        }
    }
}

// ----- The library: everything under activities/ -----

/// Every activity definition, the catalog they index, the capture sequences and the Crucible data.
#[derive(Clone, Debug, Default)]
pub struct ActivityLibrary {
    pub dir: PathBuf,
    pub catalog: Catalog,
    pub activities: Vec<Arc<ActivityDef>>,
    pub sequences: HashMap<String, CaptureSequence>,
    pub crucible: Option<crate::pvp::CrucibleData>,
}

/// The catalog categories the framework must cover (Crucible ones are [`crate::pvp`]'s).
pub const PVE_CATEGORIES: [&str; 8] =
    ["strike", "nightfall", "weekly_heroic", "strike_playlist", "raid", "arena", "arena_challenge", "patrol_event"];

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> anyhow::Result<T> {
    let text = std::fs::read_to_string(path).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))
}

impl ActivityLibrary {
    /// The fixtures folder: `$D1_ACTIVITIES_DIR`, else `ps4/fixtures/56578/activities` of this
    /// checkout.
    pub fn default_dir() -> PathBuf {
        std::env::var_os("D1_ACTIVITIES_DIR")
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/56578/activities"))
    }

    /// Read `catalog.json`, `definitions/*.json`, `sequences/*.json` and `crucible.json`.
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let catalog: Catalog = read_json(&dir.join("catalog.json"))?;
        let mut activities = Vec::new();
        let mut files: Vec<_> = std::fs::read_dir(dir.join("definitions"))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        files.sort();
        for f in files {
            let file: DefinitionFile = read_json(&f)?;
            activities.extend(file.activities.into_iter().map(Arc::new));
        }
        let mut sequences = HashMap::new();
        if let Ok(rd) = std::fs::read_dir(dir.join("sequences")) {
            for e in rd.filter_map(Result::ok) {
                let p = e.path();
                if p.extension().is_some_and(|x| x == "json") {
                    let seq: CaptureSequence = read_json(&p)?;
                    let stem = p.file_stem().unwrap_or_default().to_string_lossy().into_owned();
                    sequences.insert(stem, seq);
                }
            }
        }
        let crucible = dir.join("crucible.json");
        let crucible = if crucible.exists() { Some(read_json(&crucible)?) } else { None };
        Ok(Self { dir: dir.to_owned(), catalog, activities, sequences, crucible })
    }

    pub fn by_id(&self, id: &str) -> Option<&Arc<ActivityDef>> {
        self.activities.iter().find(|a| a.id == id)
    }

    /// The definition that runs manifest activity `hash` (directly, or as a playlist member).
    pub fn by_hash(&self, hash: u32) -> Option<&Arc<ActivityDef>> {
        self.activities.iter().find(|a| a.activity_hashes.contains(&hash))
    }

    /// Every problem with the data, as sentences. Empty means valid.
    pub fn validate(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut ids = HashSet::new();
        let mut owner: HashMap<u32, &str> = HashMap::new();
        for a in &self.activities {
            if !ids.insert(a.id.as_str()) {
                out.push(format!("{}: duplicate activity id", a.id));
            }
            validate_activity(a, self, &mut out);
            for &h in &a.activity_hashes {
                if let Some(prev) = owner.insert(h, &a.id) {
                    out.push(format!("{}: activity hash {h} is also claimed by {prev}", a.id));
                }
            }
        }
        // Coverage: every PvE row of the manifest must belong to a definition or a playlist.
        let in_playlist: HashSet<u32> = self.catalog.playlists.iter().map(|p| p.hash).collect();
        for c in &self.catalog.activities {
            if PVE_CATEGORIES.contains(&c.category.as_str()) && !owner.contains_key(&c.hash) && !in_playlist.contains(&c.hash) {
                out.push(format!("manifest activity {} ({}, {}) is in no definition or playlist", c.hash,
                    c.name.as_deref().unwrap_or("?"), c.category));
            }
        }
        for p in &self.catalog.playlists {
            for m in &p.members {
                if self.catalog.get(*m).is_none() {
                    out.push(format!("playlist {} ({}): member {m} is not in the catalog", p.hash, p.name));
                }
            }
        }
        if let Some(c) = &self.crucible {
            out.extend(c.validate(&self.catalog));
        }
        out
    }

    /// Per kind: how many definitions, and how many have their encounters from primary evidence
    /// (script or live capture) rather than community or placeholder.
    pub fn coverage(&self) -> BTreeMap<String, (usize, usize)> {
        let mut m: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        for a in &self.activities {
            let e = m.entry(format!("{:?}", a.kind)).or_default();
            e.0 += 1;
            if a.encounters.iter().all(|x| x.evidence.kind.is_primary()) {
                e.1 += 1;
            }
        }
        m
    }
}

fn validate_activity(a: &ActivityDef, lib: &ActivityLibrary, out: &mut Vec<String>) {
    let id = &a.id;
    if a.encounters.is_empty() {
        out.push(format!("{id}: no encounters"));
    }
    if a.fireteam.min == 0 || a.fireteam.min > a.fireteam.max {
        out.push(format!("{id}: fireteam {}..{} is not a range", a.fireteam.min, a.fireteam.max));
    }
    let mut enc_ids = HashSet::new();
    for (i, e) in a.encounters.iter().enumerate() {
        if !enc_ids.insert(e.id.as_str()) {
            out.push(format!("{id}: duplicate encounter id {}", e.id));
        }
        match &e.begins_on {
            Trigger::Trigger(t) if parse_trigger_name(t).is_none() => {
                out.push(format!("{id}/{}: trigger {t:?} is not 0xGROUP/KIND/INDEX", e.id))
            }
            Trigger::ScriptPhase(p) if !a.script.as_ref().is_some_and(|s| s.phases.contains(p)) => {
                out.push(format!("{id}/{}: script phase {p:?} is not in the script's phase list", e.id))
            }
            Trigger::Manual if i == 0 => out.push(format!("{id}/{}: the first encounter cannot be manual", e.id)),
            _ => {}
        }
        if let (Some(seq), t @ (Trigger::SliceEntered(_) | Trigger::Trigger(_))) =
            (a.sequence.as_ref().and_then(|s| lib.sequences.get(s)), &e.begins_on)
        {
            if !seq.saw(t) {
                out.push(format!("{id}/{}: {t:?} never happens in sequence {}", e.id, seq.activity_id));
            }
        }
    }
    if let Some(s) = &a.sequence {
        if !lib.sequences.contains_key(s) {
            out.push(format!("{id}: sequence {s} is not under sequences/"));
        }
    }
    if let Some(s) = &a.script {
        if s.start_only_stub && !s.phases.is_empty() {
            out.push(format!("{id}: script {} is marked a stub but lists phases", s.activity));
        }
    }
    for &h in &a.activity_hashes {
        match lib.catalog.get(h) {
            None => out.push(format!("{id}: activity hash {h} is not in the catalog")),
            Some(c) => {
                let ok = match a.kind {
                    ActivityKind::Strike => matches!(c.category.as_str(), "strike" | "nightfall" | "weekly_heroic" | "strike_playlist"),
                    ActivityKind::Raid => c.category == "raid",
                    ActivityKind::Arena => c.category == "arena",
                    ActivityKind::ArenaChallenge => c.category == "arena_challenge",
                    ActivityKind::PublicEvent => c.category == "patrol_event",
                };
                if !ok {
                    out.push(format!("{id}: activity hash {h} is a {} in the catalog, not a {:?}", c.category, a.kind));
                }
            }
        }
    }
    let known = lib.catalog.skull_names();
    for m in &a.modifiers {
        if !known.contains(m.as_str()) {
            out.push(format!("{id}: modifier {m:?} is no skull the manifest names"));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Modifiers
// ---------------------------------------------------------------------------------------------

/// The skulls a run has, and the rules of theirs the server enforces. The rest (burns, Epic,
/// Juggler, ...) change combat, which the clients simulate; the server only names them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub skulls: Vec<String>,
    /// Nightfall: "If all players die, your Fireteam is returned to Orbit."
    pub wipe_returns_to_orbit: bool,
    /// Heroic: "Enemies appear in greater numbers and are more aggressive." Combat-side; noted so a
    /// run can report it.
    pub heroic: bool,
}

impl Modifiers {
    pub fn from_skulls<'a>(names: impl IntoIterator<Item = &'a str>) -> Self {
        let mut m = Self::default();
        for n in names {
            let n = n.trim();
            if n.is_empty() {
                continue;
            }
            match n.to_ascii_lowercase().as_str() {
                "nightfall" => m.wipe_returns_to_orbit = true,
                "heroic" => m.heroic = true,
                _ => {}
            }
            m.skulls.push(n.to_owned());
        }
        m
    }

    /// The skulls a manifest activity row carries. A Nightfall-typed row
    /// (`ACTIVITY_TYPE_NIGHTFALL`) is a Nightfall even where its row lists only the week's skull
    /// pool (Vanguard Nightfall's 20): the Nightfall skull itself appears on the year-one weekly
    /// rows. A pool row lists every skull the week may draw; the launcher picks the week's.
    pub fn of_catalog(row: &CatalogActivity) -> Self {
        let mut m = Self::from_skulls(row.skulls.iter().flatten().map(String::as_str));
        if row.category == "nightfall" && !m.wipe_returns_to_orbit {
            m.wipe_returns_to_orbit = true;
            m.skulls.push("Nightfall".into());
        }
        m
    }
}

// ---------------------------------------------------------------------------------------------
// Runtime
// ---------------------------------------------------------------------------------------------

/// What the world reported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// The script bridge entered a phase (the `@phase` row name).
    ScriptPhase(String),
    /// The client first reported a slice set.
    SliceEntered(u32),
    /// The client crossed a trigger.
    Trigger { group: u32, kind: u8, index: u16 },
    /// Open an encounter by id (an encounter on [`Trigger::Manual`], or a host override).
    BeginEncounter(String),
    /// The current encounter is done (its boss died, its objective completed).
    EncounterCompleted,
    /// The whole activity is done (the script's `set_activity_complete`).
    ActivityCompleted,
    /// Challenge of the Elders points.
    Score(i64),
    PlayerJoined(CharacterId),
    PlayerLeft(CharacterId),
    PlayerDied(CharacterId),
    /// Revived or respawned.
    PlayerRevived(CharacterId),
    /// The bridge says every player is dead (BullShcript's `all_dead`).
    AllPlayersDead,
    /// The wipe screen is over and the clients have reset (the hard-wipe handshake finished).
    WipeFinished,
    /// The clock.
    Tick(UnixSeconds),
}

/// Why a run ended without completing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EndReason {
    /// A wipe under Nightfall.
    NightfallWipe,
    /// An encounter's time limit ran out.
    TimeLimit(String),
    /// Every player left.
    Abandoned,
}

/// What the run decided.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    EncounterBegan { index: usize, id: String },
    EncounterCompleted { index: usize, id: String },
    /// A wipe will restart here.
    CheckpointReached { index: usize, id: String },
    /// Saved to the character store (raids).
    CheckpointSaved { character: CharacterId, index: usize, id: String },
    /// Loaded from the leader's checkpoint at start.
    CheckpointLoaded { character: CharacterId, index: usize, id: String },
    /// A stored checkpoint named an encounter this definition no longer has at that index.
    CheckpointRejected { character: CharacterId, id: String },
    /// Every player is dead in a darkness zone: show the wipe and reset to `index`.
    Wipe { index: usize, id: String },
    /// The players respawned at the checkpoint after a wipe.
    Respawned { index: usize },
    Rewards { character: CharacterId, encounter: String, items: Vec<u32> },
    /// Already paid this week.
    RewardsLocked { character: CharacterId, encounter: String },
    ActivityCompleted { score: i64 },
    ReturnedToOrbit(EndReason),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunState {
    /// Playing `encounter`; `cleared` once it completed and the next has not opened.
    Running { encounter: usize, cleared: bool },
    /// The wipe screen; waiting for [`Event::WipeFinished`].
    Wiping { restart: usize },
    Completed,
    Ended(EndReason),
}

/// One run of one activity by one fireteam.
pub struct ActivityRun {
    def: Arc<ActivityDef>,
    activity_hash: u32,
    pub modifiers: Modifiers,
    store: Arc<dyn CheckpointStore>,
    now: UnixSeconds,
    /// Present players and whether each is alive.
    players: BTreeMap<CharacterId, bool>,
    leader: CharacterId,
    state: RunState,
    restart_point: usize,
    deadline: Option<UnixSeconds>,
    score: i64,
}

impl ActivityRun {
    /// Start a run. `fireteam[0]` is the leader, whose checkpoint a weekly activity loads.
    pub fn start(
        def: Arc<ActivityDef>,
        activity_hash: u32,
        fireteam: &[CharacterId],
        modifiers: Modifiers,
        store: Arc<dyn CheckpointStore>,
        now: UnixSeconds,
    ) -> (Self, Vec<Effect>) {
        let leader = fireteam.first().copied().unwrap_or_default();
        let mut run = Self {
            def,
            activity_hash,
            modifiers,
            store,
            now,
            players: fireteam.iter().map(|&c| (c, true)).collect(),
            leader,
            state: RunState::Running { encounter: 0, cleared: false },
            restart_point: 0,
            deadline: None,
            score: 0,
        };
        let mut fx = Vec::new();
        let mut first = 0;
        if run.def.checkpoints == CheckpointPolicy::Weekly {
            let key = CheckpointKey { character: leader, activity_hash };
            if let Some(cp) = checkpoints::load_current(run.store.as_ref(), key, now) {
                if run.def.encounters.get(cp.encounter_index).is_some_and(|e| e.id == cp.encounter_id) {
                    first = cp.encounter_index;
                    fx.push(Effect::CheckpointLoaded { character: leader, index: first, id: cp.encounter_id });
                } else {
                    fx.push(Effect::CheckpointRejected { character: leader, id: cp.encounter_id });
                }
            }
        }
        run.begin(first, &mut fx);
        (run, fx)
    }

    pub fn state(&self) -> &RunState {
        &self.state
    }

    pub fn definition(&self) -> &ActivityDef {
        &self.def
    }

    /// The encounter being played (or just cleared).
    pub fn current(&self) -> Option<&EncounterDef> {
        match self.state {
            RunState::Running { encounter, .. } => self.def.encounters.get(encounter),
            RunState::Wiping { restart } => self.def.encounters.get(restart),
            _ => None,
        }
    }

    /// The fireteam leader, whose checkpoint the run loaded.
    pub fn leader(&self) -> CharacterId {
        self.leader
    }

    pub fn restart_point(&self) -> usize {
        self.restart_point
    }

    pub fn score(&self) -> i64 {
        self.score
    }

    pub fn players(&self) -> impl Iterator<Item = (CharacterId, bool)> + '_ {
        self.players.iter().map(|(&c, &a)| (c, a))
    }

    fn begin(&mut self, index: usize, fx: &mut Vec<Effect>) {
        let e = &self.def.encounters[index];
        self.state = RunState::Running { encounter: index, cleared: false };
        fx.push(Effect::EncounterBegan { index, id: e.id.clone() });
        if e.checkpoint || index == 0 {
            self.restart_point = index;
            if e.checkpoint {
                fx.push(Effect::CheckpointReached { index, id: e.id.clone() });
            }
        }
        self.deadline = e.time_limit_s.map(|s| self.now + u64::from(s));
    }

    fn complete(&mut self, index: usize, fx: &mut Vec<Effect>) {
        let def = Arc::clone(&self.def);
        let e = &def.encounters[index];
        fx.push(Effect::EncounterCompleted { index, id: e.id.clone() });
        self.deadline = None;
        let week = checkpoints::reset_week(self.now);
        let weekly = def.checkpoints == CheckpointPolicy::Weekly;
        let items: Vec<u32> = e.rewards.iter().flat_map(|r| r.items.iter().copied()).collect();
        for &c in self.players.keys() {
            let key = CheckpointKey { character: c, activity_hash: self.activity_hash };
            if !e.rewards.is_empty() {
                if weekly && self.store.is_looted(key, &e.id, week) {
                    fx.push(Effect::RewardsLocked { character: c, encounter: e.id.clone() });
                } else {
                    if weekly {
                        self.store.mark_looted(key, &e.id, week);
                    }
                    fx.push(Effect::Rewards { character: c, encounter: e.id.clone(), items: items.clone() });
                }
            }
            if weekly {
                // The checkpoint is the next checkpointed encounter.
                if let Some((j, next)) = def.encounters.iter().enumerate().skip(index + 1).find(|(_, n)| n.checkpoint) {
                    self.store.save(Checkpoint { key, encounter_index: j, encounter_id: next.id.clone(), week });
                    fx.push(Effect::CheckpointSaved { character: c, index: j, id: next.id.clone() });
                }
            }
        }
    }

    fn finish(&mut self, fx: &mut Vec<Effect>) {
        let def = Arc::clone(&self.def);
        if !def.rewards.is_empty() {
            let items: Vec<u32> = def.rewards.iter().flat_map(|r| r.items.iter().copied()).collect();
            for &c in self.players.keys() {
                fx.push(Effect::Rewards { character: c, encounter: String::new(), items: items.clone() });
            }
        }
        if def.checkpoints == CheckpointPolicy::Weekly {
            for &c in self.players.keys() {
                self.store.clear(CheckpointKey { character: c, activity_hash: self.activity_hash });
            }
        }
        self.state = RunState::Completed;
        self.deadline = None;
        fx.push(Effect::ActivityCompleted { score: self.score });
    }

    fn end(&mut self, why: EndReason, fx: &mut Vec<Effect>) {
        self.state = RunState::Ended(why.clone());
        self.deadline = None;
        fx.push(Effect::ReturnedToOrbit(why));
    }

    /// The first encounter after `cur` that `ev` opens.
    fn opened_by(&self, cur: usize, ev: &Event) -> Option<usize> {
        let hit = |t: &Trigger| match (t, ev) {
            (Trigger::ScriptPhase(p), Event::ScriptPhase(q)) => p == q,
            (Trigger::SliceEntered(s), Event::SliceEntered(r)) => s == r,
            (Trigger::Trigger(name), Event::Trigger { group, kind, index }) => {
                parse_trigger_name(name) == Some((*group, *kind, *index))
            }
            _ => false,
        };
        self.def.encounters.iter().enumerate().skip(cur + 1).find(|(_, e)| hit(&e.begins_on)).map(|(i, _)| i)
    }

    fn all_dead(&self) -> bool {
        !self.players.is_empty() && self.players.values().all(|alive| !alive)
    }

    /// Every player is dead. In a darkness zone that is a wipe. Under Nightfall it is the end
    /// anywhere: the skull's text has no darkness condition ("If all players die, your Fireteam is
    /// returned to Orbit"), so it is applied as written.
    fn wipe(&mut self, fx: &mut Vec<Effect>) {
        if !matches!(self.state, RunState::Running { .. }) {
            return;
        }
        if self.modifiers.wipe_returns_to_orbit {
            self.end(EndReason::NightfallWipe, fx);
            return;
        }
        if !self.current().is_some_and(|e| e.darkness) {
            return;
        }
        let restart = self.restart_point;
        self.state = RunState::Wiping { restart };
        self.deadline = None;
        fx.push(Effect::Wipe { index: restart, id: self.def.encounters[restart].id.clone() });
    }

    /// Feed one event; get what the run decided.
    pub fn handle(&mut self, ev: Event) -> Vec<Effect> {
        let mut fx = Vec::new();
        if matches!(self.state, RunState::Completed | RunState::Ended(_)) {
            return fx;
        }
        match ev {
            Event::Tick(now) => {
                self.now = now;
                if let (Some(d), RunState::Running { encounter, cleared: false }) = (self.deadline, &self.state) {
                    if now >= d {
                        let id = self.def.encounters[*encounter].id.clone();
                        self.end(EndReason::TimeLimit(id), &mut fx);
                    }
                }
            }
            Event::PlayerJoined(c) => {
                self.players.insert(c, true);
            }
            Event::PlayerLeft(c) => {
                self.players.remove(&c);
                if self.players.is_empty() {
                    self.end(EndReason::Abandoned, &mut fx);
                } else if self.all_dead() {
                    self.wipe(&mut fx);
                }
            }
            Event::PlayerDied(c) => {
                if let Some(a) = self.players.get_mut(&c) {
                    *a = false;
                }
                if self.all_dead() {
                    self.wipe(&mut fx);
                }
            }
            Event::PlayerRevived(c) => {
                if matches!(self.state, RunState::Running { .. }) {
                    if let Some(a) = self.players.get_mut(&c) {
                        *a = true;
                    }
                }
            }
            Event::AllPlayersDead => {
                for a in self.players.values_mut() {
                    *a = false;
                }
                self.wipe(&mut fx);
            }
            Event::WipeFinished => {
                if let RunState::Wiping { restart } = self.state {
                    for a in self.players.values_mut() {
                        *a = true;
                    }
                    fx.push(Effect::Respawned { index: restart });
                    self.begin(restart, &mut fx);
                }
            }
            Event::Score(p) => {
                if matches!(self.state, RunState::Running { .. }) {
                    self.score += p;
                }
            }
            Event::ActivityCompleted => {
                if let RunState::Running { encounter, cleared } = self.state {
                    if !cleared {
                        self.complete(encounter, &mut fx);
                    }
                    self.finish(&mut fx);
                }
            }
            Event::EncounterCompleted => {
                if let RunState::Running { encounter, cleared: false } = self.state {
                    self.complete(encounter, &mut fx);
                    let next = encounter + 1;
                    if next >= self.def.encounters.len() {
                        self.finish(&mut fx);
                    } else if self.def.encounters[next].begins_on == Trigger::PreviousComplete {
                        self.begin(next, &mut fx);
                    } else {
                        self.state = RunState::Running { encounter, cleared: true };
                    }
                }
            }
            Event::BeginEncounter(id) => {
                if let (RunState::Running { encounter, cleared }, Some(j)) = (self.state.clone(), self.def.encounter_index(&id)) {
                    if j > encounter {
                        self.advance_to(encounter, cleared, j, &mut fx);
                    }
                }
            }
            ev @ (Event::ScriptPhase(_) | Event::SliceEntered(_) | Event::Trigger { .. }) => {
                if let RunState::Running { encounter, cleared } = self.state {
                    if let Some(j) = self.opened_by(encounter, &ev) {
                        self.advance_to(encounter, cleared, j, &mut fx);
                    }
                }
            }
        }
        fx
    }

    /// Open encounter `j`, completing everything from the current one up to it.
    fn advance_to(&mut self, cur: usize, cleared: bool, j: usize, fx: &mut Vec<Effect>) {
        if !cleared {
            self.complete(cur, fx);
        }
        for k in cur + 1..j {
            self.complete(k, fx);
        }
        self.begin(j, fx);
    }
}

/// The host's own reports, as framework events (the activities with no restored script).
pub mod host_events {
    use super::Event;

    /// A trigger report image (incident 3403's body, as [`crate::state::trigger_report`] reads it).
    pub fn from_trigger_report(image: &[u8]) -> Option<Event> {
        crate::state::trigger_report(image).map(|(group, kind, index)| Event::Trigger { group, kind, index })
    }

    /// A slice set the client reported (udt 21 +0xB8); 0 and 0x3FF are "none".
    pub fn from_slice_set(slice_set: u32) -> Option<Event> {
        let s = slice_set & 0x3FF;
        (!matches!(s, 0 | 0x3FF)).then_some(Event::SliceEntered(s))
    }
}

/// `D1_AH_ACTIVITY_FRAMEWORK`: the knob that loads the library at serve time (hook in `main.rs`).
pub const KNOB: &str = "D1_AH_ACTIVITY_FRAMEWORK";

/// A one-line summary of the library for the serve log.
pub fn summary(lib: &ActivityLibrary) -> String {
    let cov: Vec<String> = lib.coverage().into_iter().map(|(k, (n, p))| format!("{k} {n} ({p} primary)")).collect();
    let pvp = lib.crucible.as_ref().map_or(0, |c| c.modes.len());
    format!(
        "{} definitions [{}], {} capture sequences, {} Crucible modes, catalog {} rows",
        lib.activities.len(),
        cov.join(", "),
        lib.sequences.len(),
        pvp,
        lib.catalog.activities.len()
    )
}

/// The ids of every definition without primary evidence for all its encounters.
pub fn needs_evidence(lib: &ActivityLibrary) -> BTreeSet<String> {
    lib.activities
        .iter()
        .filter(|a| !a.encounters.iter().all(|e| e.evidence.kind.is_primary()))
        .map(|a| a.id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoints::{reset_week, MemoryCheckpointStore};

    fn ev(kind: EvidenceKind) -> Evidence {
        Evidence { kind, source: String::new() }
    }

    fn enc(id: &str, begins_on: Trigger, checkpoint: bool, darkness: bool) -> EncounterDef {
        EncounterDef {
            id: id.into(),
            name: id.into(),
            region: None,
            begins_on,
            checkpoint,
            darkness,
            boss: None,
            time_limit_s: None,
            rewards: vec![RewardDef { label: "chest".into(), items: vec![1, 2], evidence: ev(EvidenceKind::StandIn) }],
            evidence: ev(EvidenceKind::StandIn),
        }
    }

    fn raid() -> Arc<ActivityDef> {
        Arc::new(ActivityDef {
            id: "raid-test".into(),
            name: "Test Raid".into(),
            kind: ActivityKind::Raid,
            activity_hashes: vec![42],
            bundle_hashes: vec![],
            fireteam: FireteamSize { min: 1, max: 6 },
            checkpoints: CheckpointPolicy::Weekly,
            encounters: vec![
                enc("abyss", Trigger::PreviousComplete, true, true),
                enc("bridge", Trigger::SliceEntered(32), true, true),
                // A traversal between checkpoints: a wipe here goes back to the bridge.
                enc("corridor", Trigger::PreviousComplete, false, true),
                enc("boss", Trigger::Trigger("0x51d22aa9/29/58".into()), true, true),
            ],
            modifiers: vec![],
            rewards: vec![],
            script: None,
            sequence: None,
            score_thresholds: vec![],
            evidence: ev(EvidenceKind::StandIn),
            needs_evidence: vec![],
        })
    }

    const NOW: UnixSeconds = 1_793_700_000;

    fn begun(fx: &[Effect]) -> Vec<String> {
        fx.iter().filter_map(|f| if let Effect::EncounterBegan { id, .. } = f { Some(id.clone()) } else { None }).collect()
    }

    #[test]
    fn progress_by_slice_trigger_and_completion() {
        let store = Arc::new(MemoryCheckpointStore::new());
        let (mut run, fx) = ActivityRun::start(raid(), 42, &[1, 2], Modifiers::default(), store, NOW);
        assert_eq!(begun(&fx), ["abyss"]);
        // A slice the next encounter does not wait for does nothing.
        assert!(run.handle(Event::SliceEntered(16)).is_empty());
        let fx = run.handle(Event::SliceEntered(32));
        assert_eq!(begun(&fx), ["bridge"]);
        assert!(fx.contains(&Effect::EncounterCompleted { index: 0, id: "abyss".into() }));
        // Completing the bridge opens the corridor at once (previous_complete).
        assert_eq!(begun(&run.handle(Event::EncounterCompleted)), ["corridor"]);
        let fx = run.handle(Event::Trigger { group: 0x51D2_2AA9, kind: 29, index: 58 });
        assert_eq!(begun(&fx), ["boss"]);
        let fx = run.handle(Event::EncounterCompleted);
        assert!(matches!(fx.last(), Some(Effect::ActivityCompleted { .. })));
        assert_eq!(run.state(), &RunState::Completed);
    }

    #[test]
    fn wipe_restarts_at_the_last_checkpoint() {
        let store = Arc::new(MemoryCheckpointStore::new());
        let (mut run, _) = ActivityRun::start(raid(), 42, &[1, 2], Modifiers::default(), store, NOW);
        run.handle(Event::SliceEntered(32));
        run.handle(Event::EncounterCompleted); // into the corridor (not a checkpoint)
        assert_eq!(run.restart_point(), 1);
        assert!(run.handle(Event::PlayerDied(1)).is_empty(), "one death is not a wipe");
        let fx = run.handle(Event::PlayerDied(2));
        assert_eq!(fx, vec![Effect::Wipe { index: 1, id: "bridge".into() }]);
        assert_eq!(run.state(), &RunState::Wiping { restart: 1 });
        // Nothing progresses on the wipe screen.
        assert!(run.handle(Event::EncounterCompleted).is_empty());
        let fx = run.handle(Event::WipeFinished);
        assert_eq!(fx[0], Effect::Respawned { index: 1 });
        assert_eq!(begun(&fx), ["bridge"]);
        assert!(run.players().all(|(_, alive)| alive));
    }

    #[test]
    fn no_wipe_outside_darkness_and_revives_count() {
        let mut def = (*raid()).clone();
        def.encounters[0].darkness = false;
        let store = Arc::new(MemoryCheckpointStore::new());
        let (mut run, _) = ActivityRun::start(Arc::new(def), 42, &[1, 2], Modifiers::default(), store, NOW);
        run.handle(Event::PlayerDied(1));
        run.handle(Event::PlayerDied(2));
        assert!(matches!(run.state(), RunState::Running { encounter: 0, .. }), "players respawn outside darkness");
        run.handle(Event::PlayerRevived(1));
        run.handle(Event::PlayerRevived(2));
        run.handle(Event::SliceEntered(32));
        run.handle(Event::PlayerDied(1));
        assert!(run.handle(Event::PlayerDied(2)).iter().any(|f| matches!(f, Effect::Wipe { .. })));
    }

    #[test]
    fn nightfall_wipe_returns_to_orbit() {
        let store = Arc::new(MemoryCheckpointStore::new());
        let m = Modifiers::from_skulls(["Nightfall", "Arc Burn"]);
        assert!(m.wipe_returns_to_orbit);
        let (mut run, _) = ActivityRun::start(raid(), 42, &[1], m, store, NOW);
        let fx = run.handle(Event::AllPlayersDead);
        assert_eq!(fx, vec![Effect::ReturnedToOrbit(EndReason::NightfallWipe)]);
        assert!(run.handle(Event::WipeFinished).is_empty());
    }

    #[test]
    fn checkpoint_saved_for_every_member_and_loaded_from_the_leader() {
        let store = Arc::new(MemoryCheckpointStore::new());
        let (mut run, _) = ActivityRun::start(raid(), 42, &[1, 2], Modifiers::default(), store.clone(), NOW);
        let fx = run.handle(Event::SliceEntered(32));
        assert!(fx.contains(&Effect::CheckpointSaved { character: 1, index: 1, id: "bridge".into() }));
        assert!(fx.contains(&Effect::CheckpointSaved { character: 2, index: 1, id: "bridge".into() }));
        drop(run); // the fireteam leaves
        // Player 2 leads the next launch, later the same week: it starts at the bridge.
        let (run, fx) = ActivityRun::start(raid(), 42, &[2, 3], Modifiers::default(), store.clone(), NOW + 3_600);
        assert_eq!(fx[0], Effect::CheckpointLoaded { character: 2, index: 1, id: "bridge".into() });
        assert_eq!(run.current().map(|e| e.id.as_str()), Some("bridge"));
        // Player 3 has no checkpoint: leading, they start at the beginning.
        let (run, _) = ActivityRun::start(raid(), 42, &[3], Modifiers::default(), store.clone(), NOW + 3_600);
        assert_eq!(run.current().map(|e| e.id.as_str()), Some("abyss"));
        // After the reset the checkpoint is spent.
        let next_week = checkpoints::week_start(reset_week(NOW) + 1);
        let (run, _) = ActivityRun::start(raid(), 42, &[2], Modifiers::default(), store, next_week);
        assert_eq!(run.current().map(|e| e.id.as_str()), Some("abyss"));
    }

    #[test]
    fn a_full_clear_spends_the_checkpoint_and_loot_locks_for_the_week() {
        let store = Arc::new(MemoryCheckpointStore::new());
        let (mut run, fx) = ActivityRun::start(raid(), 42, &[1], Modifiers::default(), store.clone(), NOW);
        assert!(!fx.iter().any(|f| matches!(f, Effect::CheckpointLoaded { .. })));
        let fx = run.handle(Event::ActivityCompleted);
        assert!(fx.contains(&Effect::Rewards { character: 1, encounter: "abyss".into(), items: vec![1, 2] }));
        assert!(store.load(CheckpointKey { character: 1, activity_hash: 42 }).is_none(), "spent");
        let (mut run, _) = ActivityRun::start(raid(), 42, &[1], Modifiers::default(), store.clone(), NOW + 60);
        let fx = run.handle(Event::EncounterCompleted);
        assert!(fx.contains(&Effect::RewardsLocked { character: 1, encounter: "abyss".into() }));
        // Another difficulty (another hash) is its own lockout.
        let (mut run, _) = ActivityRun::start(raid(), 43, &[1], Modifiers::default(), store, NOW + 60);
        assert!(run.handle(Event::EncounterCompleted).iter().any(|f| matches!(f, Effect::Rewards { .. })));
    }

    #[test]
    fn a_reordered_definition_rejects_a_stale_checkpoint() {
        let store = Arc::new(MemoryCheckpointStore::new());
        store.save(Checkpoint {
            key: CheckpointKey { character: 1, activity_hash: 42 },
            encounter_index: 1,
            encounter_id: "renamed".into(),
            week: reset_week(NOW),
        });
        let (run, fx) = ActivityRun::start(raid(), 42, &[1], Modifiers::default(), store, NOW);
        assert_eq!(fx[0], Effect::CheckpointRejected { character: 1, id: "renamed".into() });
        assert_eq!(run.current().map(|e| e.id.as_str()), Some("abyss"));
    }

    #[test]
    fn script_phases_drive_a_definition() {
        let mut def = (*raid()).clone();
        def.checkpoints = CheckpointPolicy::InActivity;
        def.script = Some(ScriptRef {
            activity: "test".into(),
            bundle: "test".into(),
            phases: vec!["p0".into(), "p1".into(), "p2".into()],
            start_only_stub: false,
        });
        def.encounters = vec![
            enc("p0", Trigger::ScriptPhase("p0".into()), true, false),
            enc("p1", Trigger::ScriptPhase("p1".into()), true, true),
            enc("p2", Trigger::ScriptPhase("p2".into()), true, true),
        ];
        let store = Arc::new(MemoryCheckpointStore::new());
        let (mut run, _) = ActivityRun::start(Arc::new(def), 42, &[1], Modifiers::default(), store.clone(), NOW);
        // A skipped phase completes the one between.
        let fx = run.handle(Event::ScriptPhase("p2".into()));
        assert!(fx.contains(&Effect::EncounterCompleted { index: 1, id: "p1".into() }));
        assert_eq!(begun(&fx), ["p2"]);
        // In-activity checkpoints never reach the store.
        assert!(store.is_empty());
        let fx = run.handle(Event::ActivityCompleted);
        assert!(matches!(fx.last(), Some(Effect::ActivityCompleted { .. })));
    }

    #[test]
    fn time_limits_and_abandonment_end_the_run() {
        let mut def = (*raid()).clone();
        def.encounters[0].time_limit_s = Some(120);
        let store = Arc::new(MemoryCheckpointStore::new());
        let (mut run, _) = ActivityRun::start(Arc::new(def.clone()), 42, &[1], Modifiers::default(), store.clone(), NOW);
        assert!(run.handle(Event::Tick(NOW + 119)).is_empty());
        assert_eq!(run.handle(Event::Tick(NOW + 120)), vec![Effect::ReturnedToOrbit(EndReason::TimeLimit("abyss".into()))]);
        let (mut run, _) = ActivityRun::start(Arc::new(def), 42, &[1, 2], Modifiers::default(), store, NOW);
        run.handle(Event::PlayerLeft(1));
        assert_eq!(run.handle(Event::PlayerLeft(2)), vec![Effect::ReturnedToOrbit(EndReason::Abandoned)]);
    }

    #[test]
    fn trigger_names_parse() {
        assert_eq!(parse_trigger_name("0x51d22aa9/29/58"), Some((0x51D2_2AA9, 29, 58)));
        assert_eq!(parse_trigger_name("51d22aa9/29/58"), None);
        assert_eq!(parse_trigger_name("0x51d22aa9/29"), None);
        assert_eq!(host_events::from_slice_set(0x3FF), None);
        assert_eq!(host_events::from_slice_set(32), Some(Event::SliceEntered(32)));
    }

    #[test]
    fn nightfall_applies_outside_darkness_too() {
        let mut def = (*raid()).clone();
        def.encounters[0].darkness = false;
        let store = Arc::new(MemoryCheckpointStore::new());
        let (mut run, _) = ActivityRun::start(Arc::new(def), 42, &[1, 2], Modifiers::from_skulls(["Nightfall"]), store, NOW);
        run.handle(Event::PlayerDied(1));
        assert_eq!(run.handle(Event::PlayerDied(2)), vec![Effect::ReturnedToOrbit(EndReason::NightfallWipe)]);
    }

    #[test]
    fn modifiers_from_manifest_skulls() {
        let m = Modifiers::from_skulls(["Heroic", "Void Burn"]);
        assert!(m.heroic && !m.wipe_returns_to_orbit);
        assert_eq!(m.skulls, ["Heroic", "Void Burn"]);
    }
}
