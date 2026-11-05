// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! # The open-world layer: destinations, social spaces, patrol, loot and rewards (stream G)
//!
//! What a public bubble's own host runs besides its players, per destination, and the interfaces
//! the other streams plug into. The facts come from the live captures, read per activity host
//! (`notes/ps4-56578-public-instances-2026-11-05.md`); everything not read from them is labelled
//! **PLACEHOLDER** where it is defined, and nothing here guesses a value the game would show a
//! player.
//!
//! * [`Destination`] and [`SocialSpace`]: the six open-world destinations and the three social
//!   spaces, with the public regions the live mission hosts marked public on each.
//! * [`bundle_role`]: what each sensor group a live public instance served is for, where Blam
//!   Network's restored scripts name it, or where its shape gives it away (`[I]`).
//! * [`PatrolBoard`]: patrol beacons and the patrol missions they hand out.
//! * [`LootField`]: chests and resource nodes, opened once per character.
//! * [`ActivityRewards`]: `grant_activity_rewards(character, activity_hash, outcome)`, which the
//!   progression stream (M) implements. This module only calls it.
//!
//! **Lost sectors are not a Destiny 1 feature** (they came with Destiny 2), so there is no lost-
//! sector type here: D1's equivalents in a patrol zone are the bubble's chests and its patrol
//! missions, both below.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

/// The open-world destinations of Destiny 1 (Rise of Iron era, PS4 build 56578).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Destination {
    /// Old Russia, the Cosmodrome.
    Earth,
    /// The Ocean of Storms.
    Moon,
    /// The Ishtar Sink.
    Venus,
    /// Meridian Bay.
    Mars,
    /// The Dreadnaught (The Taken King).
    Dreadnaught,
    /// The Plaguelands (Rise of Iron; never on the Xbox 360, so no capture has it).
    Plaguelands,
}

impl Destination {
    pub const ALL: [Destination; 6] = [
        Destination::Earth,
        Destination::Moon,
        Destination::Venus,
        Destination::Mars,
        Destination::Dreadnaught,
        Destination::Plaguelands,
    ];

    /// The name the fixtures use (`public-<name>-r<region>.txt`).
    pub fn name(self) -> &'static str {
        match self {
            Self::Earth => "earth",
            Self::Moon => "moon",
            Self::Venus => "venus",
            Self::Mars => "mars",
            Self::Dreadnaught => "dreadnaught",
            Self::Plaguelands => "plaguelands",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.name() == s.trim())
    }

    /// The destination a world belongs to, from the destination folder its name carries (the
    /// scenario record's `+0x30` is that folder: `cosmo` for every Cosmodrome world). `[I]` for
    /// the Dreadnaught (`hiveship`, The Taken King's mission folder) and the Plaguelands
    /// (`impactsite`, Rise of Iron's); the other four are the folders the restored 56578 mission
    /// scripts and our served worlds use.
    pub fn from_world_name(world: &str) -> Option<Self> {
        let w = world.to_ascii_lowercase();
        let has = |token: &str| w.split(['_', '.', '/', '\\']).any(|t| t == token);
        if has("cosmo") {
            Some(Self::Earth)
        } else if has("moon") {
            Some(Self::Moon)
        } else if has("venus") {
            Some(Self::Venus)
        } else if has("mars") {
            Some(Self::Mars)
        } else if has("hiveship") || has("dreadnaught") {
            Some(Self::Dreadnaught)
        } else if has("impactsite") || has("plaguelands") {
            Some(Self::Plaguelands)
        } else {
            None
        }
    }

    /// The regions (slice-set ids, `8 * bubble + n`) the live mission hosts marked public
    /// (region record `+0x04 = 1`) on this destination, over every capture of a mission there.
    /// The same region on one destination is the same place in every mission: each captured pair
    /// of instances of one region served the same sensor groups. Empty for the Plaguelands.
    pub fn public_regions_seen(self) -> &'static [u32] {
        match self {
            // Restoration, The Dark Within, The Warmind, The Last Array, Siege of the Warmind,
            // The Devils' Lair, The Will of Crota, The Shadow Thief, Winter's Run.
            Self::Earth => &[0, 48, 56, 72, 88, 96, 160],
            // The Dark Beyond, The World's Grave, The Sword of Crota, Shrine of Oryx, Chamber of
            // Night, The Wakening, The Summoning Pits.
            Self::Moon => &[8, 16, 48],
            // A Stranger's Call, Ishtar Collective, Eye of a Gate Lord, The Archive, Patrol Venus.
            Self::Venus => &[32, 72, 80, 120, 160, 224],
            // Exclusion Zone, The Garden's Spire, The Buried City, A Rising Tide, The Black Garden.
            Self::Mars => &[8, 48, 112, 144, 192, 208],
            // Patrol the Dreadnaught.
            Self::Dreadnaught => &[8],
            Self::Plaguelands => &[],
        }
    }
}

/// The social spaces. Each is an activity of its own with its own host (not a bubble of a mission):
/// the client's join request names no mission host (`+0xC8 = 0`) and carries its fireteam.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SocialSpace {
    /// The Tower. Live, the client JOINED a running instance (session type 2): the host's first
    /// stamp was 58,958, about 98 minutes of host clock.
    Tower,
    /// The Reef's Vestian Outpost. Live, the client STARTED it (type 3, first stamp 12).
    Reef,
    /// Felwinter Peak (Rise of Iron; no capture).
    FelwinterPeak,
}

impl SocialSpace {
    pub fn name(self) -> &'static str {
        match self {
            Self::Tower => "tower",
            Self::Reef => "reef",
            Self::FelwinterPeak => "felwinter",
        }
    }

    /// The social space a world is, from its name: `city_tower_*` is the Tower (each seasonal
    /// variant has its own scenario), and the Reef's worlds carry `reef`. Felwinter Peak's world
    /// name has not been read from the client's content yet, so it is never matched here.
    pub fn from_world_name(world: &str) -> Option<Self> {
        let w = world.to_ascii_lowercase();
        if w.starts_with("city_tower") {
            Some(Self::Tower)
        } else if w.split(['_', '.']).any(|t| t == "reef") {
            Some(Self::Reef)
        } else {
            None
        }
    }

    /// Player-object records in the space's player group (`0x40AEBF90`, kind `0xC`): 30 in both
    /// captured spaces. That is how many player objects the space can hold, not how many players
    /// the matchmaker puts in it.
    pub const PLAYER_OBJECTS: usize = 30;

    /// ⚠ **PLACEHOLDER** player cap for one social-space instance. Not read from the game or the
    /// captures (each capture is one player); set to the 16 player objects a public bubble
    /// carries until a source gives the real number.
    pub const PLACEHOLDER_PLAYER_CAP: usize = 16;
}

/// What one sensor group (an activity *bundle*: the restored scripts' `@activity_bundle(HASH)`)
/// on a public host is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleRole {
    /// The bubble's player objects: `0xEAAF16E2` kind `0xC` 0-15 on every public instance, and
    /// `0x40AEBF90` 0-29 in the social spaces.
    Players,
    /// The ambient layer: the bubble's small encounters, its patrol beacons, its music.
    Ambient,
    /// The bubble's public events (one runs at a time).
    PublicEvents,
    /// A Taken invasion (The Taken King's public activity, run like an event).
    TakenInvasion,
    /// Loot chests.
    Chests,
    /// Resource nodes (Spinmetal, Helium Filaments, Relic Iron, Spirit Bloom, Wormspore,
    /// Hadronic Essence, Thorium).
    ResourceNodes,
    /// A social space's vendors and people.
    SocialCast,
    /// `[I]` Loot of some kind: a group of 10 records or fewer, all kind `0x4`, which is the
    /// shape of every chest and resource-node bundle the scripts name. Which of the two is open.
    LootInferred,
    /// Not named and not inferred.
    Unknown,
}

/// Bundles named by the header of Blam Network's restored script for that bundle (used locally
/// only, never copied; the roles are ours, in our own words).
const NAMED_BUNDLES: &[(u32, BundleRole)] = &[
    (0xA2F6_6E1A, BundleRole::Ambient),        // Cosmodrome, around dome A (region 48)
    (0x358E_BAE1, BundleRole::PublicEvents),   // Cosmodrome, region 48
    (0xD2C2_2DCF, BundleRole::PublicEvents),   // Cosmodrome, the Array (region 0)
    (0x00B0_2852, BundleRole::Chests),         // Cosmodrome, region 48
    (0xAF64_2429, BundleRole::Chests),         // Cosmodrome, region 0
    (0xBDF9_E3C6, BundleRole::ResourceNodes),  // Cosmodrome, region 48 (Thorium)
    (0xAFC0_5F47, BundleRole::ResourceNodes),  // Cosmodrome, region 0 (Thorium)
    (0xF800_F659, BundleRole::TakenInvasion),  // Venus, region 72
    (0x9D12_F3F2, BundleRole::TakenInvasion),  // Venus, region 32
    (0x972B_0F9C, BundleRole::TakenInvasion),  // Venus, region 32
    (0x278551AE, BundleRole::TakenInvasion),   // Venus, region 80
    (0x416E_3453, BundleRole::TakenInvasion),  // Venus, region 160
    (0x3B2D_B6DC, BundleRole::TakenInvasion),  // Venus, region 120
    (0xE525_F723, BundleRole::SocialCast),     // the Reef's Vestian Outpost
];

/// The public bubbles' player group, and the social spaces'.
pub const PUBLIC_PLAYERS_GROUP: u32 = 0xEAAF_16E2;
pub const SOCIAL_PLAYERS_GROUP: u32 = 0x40AE_BF90;

/// The role of a bundle, given the record kinds it carries (`kind -> count`).
pub fn bundle_role(hash: u32, kinds: &BTreeMap<u8, usize>) -> BundleRole {
    if hash == PUBLIC_PLAYERS_GROUP || hash == SOCIAL_PLAYERS_GROUP {
        return BundleRole::Players;
    }
    if let Some((_, role)) = NAMED_BUNDLES.iter().find(|(h, _)| *h == hash) {
        return *role;
    }
    let total: usize = kinds.values().sum();
    if total > 0 && total <= 10 && kinds.keys().all(|&k| k == 0x4) {
        return BundleRole::LootInferred;
    }
    BundleRole::Unknown
}

// ── rewards: the one call this layer makes into progression ───────────────────────────────────

/// How an open-world activity ended for one character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityOutcome {
    /// A public event the character took part in. `tier` is what the event's script reported.
    PublicEvent { event_id: u32, completed: bool, tier: Option<EventTier> },
    /// A patrol mission from a beacon.
    PatrolMission { beacon: u32 },
    /// A chest opened.
    Chest { bundle: u32, index: i16 },
    /// A resource node harvested.
    ResourceNode { bundle: u32, index: i16 },
}

/// A public event's completion tier, as Destiny 1 showed it (bronze, silver, gold). The tier is
/// reported by the event's script; this layer never computes one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventTier {
    Bronze,
    Silver,
    Gold,
}

/// ⭐ **The interface stream M (progression) implements.** Every open-world reward goes through
/// this one call: what the character gets, and how it is saved, is M's to decide from the game's
/// own reward data; this layer only reports who finished what.
pub trait ActivityRewards {
    fn grant_activity_rewards(
        &mut self,
        character: u64,
        activity_hash: u32,
        outcome: &ActivityOutcome,
    ) -> Result<(), String>;
}

/// Records every grant, for tests and for a host run with no progression service.
#[derive(Debug, Default)]
pub struct RecordingRewards {
    pub grants: Vec<(u64, u32, ActivityOutcome)>,
}

impl ActivityRewards for RecordingRewards {
    fn grant_activity_rewards(
        &mut self,
        character: u64,
        activity_hash: u32,
        outcome: &ActivityOutcome,
    ) -> Result<(), String> {
        self.grants.push((character, activity_hash, outcome.clone()));
        Ok(())
    }
}

// ── patrol ────────────────────────────────────────────────────────────────────────────────────

/// The kinds of patrol mission a beacon hands out in Destiny 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatrolMissionKind {
    /// Kill a number of enemies.
    Combat,
    /// Go to a place and scan it.
    Scan,
    /// Collect items dropped by enemies.
    Collect,
    /// Kill one high-value target.
    Elimination,
}

/// One patrol beacon, as the bubble's ambient bundle places it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatrolBeacon {
    pub id: u32,
    pub destination: Destination,
    pub region: u32,
    pub kind: PatrolMissionKind,
    /// The patrol mission's activity hash, for the reward call. 0 until read from content.
    pub activity_hash: u32,
    /// How much progress completes it (kills, scans, items); 1 for a scan or a target.
    pub goal: u32,
}

/// ⚠ **PLACEHOLDER beacons.** Not read from the game: one beacon per mission kind on region 48 of
/// the Cosmodrome, where the restored ambient script for that bubble places three beacon objects.
/// The live patrol hosts' own groups (`0x7CF2807B` on Patrol Venus, `0x78708B21` on Patrol the
/// Dreadnaught) carry 60 and 47 objective records and 59 and 22 kind-`0x23` records, `[I]` the
/// patrol missions; the goals and activity hashes come from content once read.
pub const PLACEHOLDER_BEACONS: &[(Destination, u32, PatrolMissionKind, u32)] = &[
    (Destination::Earth, 48, PatrolMissionKind::Combat, 20),
    (Destination::Earth, 48, PatrolMissionKind::Scan, 1),
    (Destination::Earth, 48, PatrolMissionKind::Collect, 10),
];

/// A patrol mission in progress for one character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivePatrol {
    pub beacon: u32,
    pub progress: u32,
}

/// The patrol beacons on offer, and each character's accepted patrol (one at a time, as in D1).
#[derive(Debug, Default)]
pub struct PatrolBoard {
    beacons: BTreeMap<u32, PatrolBeacon>,
    active: BTreeMap<u64, ActivePatrol>,
}

impl PatrolBoard {
    /// A board with the given beacons (`(destination, region, kind, goal)`), numbered from 1.
    pub fn new(beacons: &[(Destination, u32, PatrolMissionKind, u32)]) -> Self {
        let beacons = beacons
            .iter()
            .enumerate()
            .map(|(i, &(destination, region, kind, goal))| {
                let id = i as u32 + 1;
                (id, PatrolBeacon { id, destination, region, kind, activity_hash: 0, goal: goal.max(1) })
            })
            .collect();
        Self { beacons, active: BTreeMap::new() }
    }

    /// The beacons in one bubble.
    pub fn beacons_in(&self, destination: Destination, region: u32) -> Vec<&PatrolBeacon> {
        self.beacons.values().filter(|b| b.destination == destination && b.region == region).collect()
    }

    /// Accept a beacon's mission, replacing any other the character had (D1 kept one patrol at a
    /// time; taking a new beacon dropped the old one).
    pub fn accept(&mut self, character: u64, beacon: u32) -> Result<(), String> {
        if !self.beacons.contains_key(&beacon) {
            return Err(format!("no patrol beacon {beacon}"));
        }
        self.active.insert(character, ActivePatrol { beacon, progress: 0 });
        Ok(())
    }

    pub fn active(&self, character: u64) -> Option<&ActivePatrol> {
        self.active.get(&character)
    }

    /// Count progress (a kill, a scan, an item) and, when the goal is met, finish the mission and
    /// report it. Returns whether it completed.
    pub fn progress(
        &mut self,
        character: u64,
        amount: u32,
        rewards: &mut dyn ActivityRewards,
    ) -> Result<bool, String> {
        let Some(active) = self.active.get_mut(&character) else { return Ok(false) };
        let beacon = &self.beacons[&active.beacon];
        active.progress = active.progress.saturating_add(amount).min(beacon.goal);
        if active.progress < beacon.goal {
            return Ok(false);
        }
        let outcome = ActivityOutcome::PatrolMission { beacon: beacon.id };
        rewards.grant_activity_rewards(character, beacon.activity_hash, &outcome)?;
        self.active.remove(&character);
        Ok(true)
    }

    /// The character left the destination: its patrol is dropped.
    pub fn abandon(&mut self, character: u64) {
        self.active.remove(&character);
    }
}

// ── chests and resource nodes ─────────────────────────────────────────────────────────────────

/// A chest or a resource node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LootKind {
    Chest,
    ResourceNode,
}

/// One loot object in a bubble, named as its sensor record (bundle hash, kind `0x4`, index).
#[derive(Debug, Clone)]
pub struct LootNode {
    pub bundle: u32,
    pub index: i16,
    pub kind: LootKind,
    /// When it shows, measured from the instance's start.
    pub appears_after: Duration,
    /// Characters who opened it. In D1 each player opens a chest once; it is gone for them, not
    /// for the rest of the bubble.
    opened_by: BTreeSet<u64>,
}

/// The loot of one public instance.
#[derive(Debug, Default)]
pub struct LootField {
    started: Option<Instant>,
    nodes: Vec<LootNode>,
}

impl LootField {
    /// ⚠ The appearance delays are the live records' first-appearance offsets where a capture had
    /// them (the loot records of a fresh instance's later list), else the caller's.
    pub fn new(started: Instant, nodes: impl IntoIterator<Item = (u32, i16, LootKind, Duration)>) -> Self {
        Self {
            started: Some(started),
            nodes: nodes
                .into_iter()
                .map(|(bundle, index, kind, appears_after)| LootNode {
                    bundle,
                    index,
                    kind,
                    appears_after,
                    opened_by: BTreeSet::new(),
                })
                .collect(),
        }
    }

    /// The nodes showing for `character` now.
    pub fn visible(&self, character: u64, now: Instant) -> Vec<&LootNode> {
        let Some(t0) = self.started else { return Vec::new() };
        let age = now.saturating_duration_since(t0);
        self.nodes
            .iter()
            .filter(|n| age >= n.appears_after && !n.opened_by.contains(&character))
            .collect()
    }

    /// Open (or harvest) a node for a character, reporting the reward. A node not showing yet, or
    /// already opened by this character, is refused.
    pub fn open(
        &mut self,
        character: u64,
        bundle: u32,
        index: i16,
        now: Instant,
        activity_hash: u32,
        rewards: &mut dyn ActivityRewards,
    ) -> Result<(), String> {
        let age = self.started.map(|t0| now.saturating_duration_since(t0)).unwrap_or_default();
        let node = self
            .nodes
            .iter_mut()
            .find(|n| n.bundle == bundle && n.index == index)
            .ok_or_else(|| format!("no loot {bundle:#010x}/{index}"))?;
        if age < node.appears_after {
            return Err(format!("loot {bundle:#010x}/{index} has not appeared yet"));
        }
        if !node.opened_by.insert(character) {
            return Err(format!("loot {bundle:#010x}/{index} already opened by this character"));
        }
        let outcome = match node.kind {
            LootKind::Chest => ActivityOutcome::Chest { bundle, index },
            LootKind::ResourceNode => ActivityOutcome::ResourceNode { bundle, index },
        };
        rewards.grant_activity_rewards(character, activity_hash, &outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worlds_map_to_their_destination_and_social_space() {
        assert_eq!(Destination::from_world_name("act_cosmo_m1_dome_a"), Some(Destination::Earth));
        assert_eq!(Destination::from_world_name("act_cosmo_m0_highway"), Some(Destination::Earth));
        assert_eq!(Destination::from_world_name("act_mars_m01_valley"), Some(Destination::Mars));
        assert_eq!(Destination::from_world_name("city_tower_default1"), None);
        assert_eq!(SocialSpace::from_world_name("city_tower_default1"), Some(SocialSpace::Tower));
        assert_eq!(SocialSpace::from_world_name("pvp_moonbase"), None);
        assert_eq!(Destination::parse("venus"), Some(Destination::Venus));
        for d in Destination::ALL {
            assert_eq!(Destination::parse(d.name()), Some(d));
        }
        assert!(Destination::Plaguelands.public_regions_seen().is_empty());
        assert!(Destination::Earth.public_regions_seen().contains(&56));
    }

    #[test]
    fn bundle_roles_come_from_the_named_scripts_or_the_loot_shape() {
        let kinds = |pairs: &[(u8, usize)]| pairs.iter().copied().collect::<BTreeMap<u8, usize>>();
        assert_eq!(bundle_role(PUBLIC_PLAYERS_GROUP, &kinds(&[(0xC, 16)])), BundleRole::Players);
        assert_eq!(bundle_role(0x358E_BAE1, &kinds(&[(0x5, 119)])), BundleRole::PublicEvents);
        assert_eq!(bundle_role(0x70C1_FC84, &kinds(&[(0x4, 6)])), BundleRole::LootInferred);
        assert_eq!(bundle_role(0xD7F6_8DA0, &kinds(&[(0x1, 17), (0x4, 19)])), BundleRole::Unknown);
    }

    #[test]
    fn a_patrol_completes_once_its_goal_is_met_and_reports_the_reward() {
        let mut board = PatrolBoard::new(PLACEHOLDER_BEACONS);
        let mut rewards = RecordingRewards::default();
        assert_eq!(board.beacons_in(Destination::Earth, 48).len(), 3);
        let combat = board.beacons_in(Destination::Earth, 48)[0].id;
        let scan = board.beacons_in(Destination::Earth, 48)[1].id;
        board.accept(7, combat).unwrap();
        assert!(!board.progress(7, 19, &mut rewards).unwrap());
        // Taking another beacon drops the first, as in D1.
        board.accept(7, scan).unwrap();
        assert_eq!(board.active(7).unwrap().progress, 0);
        assert!(board.progress(7, 1, &mut rewards).unwrap());
        assert_eq!(rewards.grants, vec![(7, 0, ActivityOutcome::PatrolMission { beacon: scan })]);
        assert!(board.active(7).is_none());
        assert!(board.accept(7, 99).is_err());
    }

    #[test]
    fn a_chest_opens_once_per_character_after_it_appears() {
        let t0 = Instant::now();
        let mut field = LootField::new(t0, [(0x00B0_2852, 0, LootKind::Chest, Duration::from_secs(48))]);
        let mut rewards = RecordingRewards::default();
        assert!(field.visible(1, t0).is_empty());
        assert!(field.open(1, 0x00B0_2852, 0, t0, 0, &mut rewards).is_err(), "not there yet");
        let later = t0 + Duration::from_secs(60);
        assert_eq!(field.visible(1, later).len(), 1);
        field.open(1, 0x00B0_2852, 0, later, 0, &mut rewards).unwrap();
        assert!(field.open(1, 0x00B0_2852, 0, later, 0, &mut rewards).is_err(), "once per character");
        assert!(field.visible(1, later).is_empty());
        assert_eq!(field.visible(2, later).len(), 1, "still there for another player");
        field.open(2, 0x00B0_2852, 0, later, 0, &mut rewards).unwrap();
        assert_eq!(rewards.grants.len(), 2);
    }
}
