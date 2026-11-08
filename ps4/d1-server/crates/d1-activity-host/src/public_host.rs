// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! # The public bubble's own host (stream G)
//!
//! Bungie's "bubble activity host" (GDC 2015): one per public instance, beside each player's
//! mission host. In the live captures every public instance is an activity-host connection of its
//! own, and the client tells it apart from its mission host in its very first message
//! (`notes/ps4-56578-public-instances-2026-11-05.md`):
//!
//! | join request | mission host | public instance |
//! |---|---|---|
//! | `+0x38` session type | 3 (starts) | 3 if the client starts it, 2 if it joins one |
//! | `+0xC8` | 0 | **the mission host's id** |
//! | `+0xD8` fireteam id | the fireteam | 0 |
//!
//! (Social spaces look like mission hosts here: `+0xC8 = 0`, with the fireteam. The Tower was
//! joined, type 2; the Reef started, type 3.)
//!
//! A connection whose join names a mission host is served as that region's public instance:
//!
//! * **no region records.** Every one of the membership's 40 region slots carries region id
//!   `0xFFFFFFFF`, as every live public instance's did (our mission host's "no record" is zeros);
//! * **no mission steps**: by Bungie's rules the mission host does not script a public bubble;
//! * **the region's own sensor list**: player group `0xEAAF16E2` (16 player objects) first, then
//!   the region's bundles (ambient layer, events, chests, resource nodes), as the region's fixture
//!   lists them (`ps4/fixtures/56578/public`). The live list carries **no lifetime-phase record**,
//!   so the frame is an entry-only one ([`FrameEncoder`]);
//! * **the entity-slot handover answered** (udt 25 → 23, 26 → 24), as every live public instance
//!   did when the player left its bubble.
//!
//! Which region the instance is for, the join does not say. The client reports it in its first
//! udt 21 (the slice-set records, CURRENT `+0xB8` and TARGET `+0x158`), 0.7 s after the join in
//! Restoration, and the live host sent the region's full list 1.5 s after that.
//!
//! The hooks in `state.rs` are marked `stream G` and do nothing unless the instance was given a
//! [`PublicHost`] (`D1_AH_PUBLIC_HOST=1` in `main.rs`).

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use d1_oracle::sensor::SensorClientRef;

use crate::open_world::{
    bundle_role, ActivityRewards, BundleRole, Destination, LootField, LootKind, PatrolBoard, SocialSpace,
    PLACEHOLDER_BEACONS, PUBLIC_PLAYERS_GROUP,
};
use crate::public_events::{schedule_for, BubbleScriptBridge, EventNotice, EventResult, PublicEventRunner};
use crate::public_registry::{InstanceKey, JoinKind, PublicRegistry};

/// `join_request +0x38`: the session type (3 starts an instance, 2 joins one).
pub const JOIN_SESSION_TYPE: usize = 0x38;
/// `join_request +0xC8`: on a public instance, the mission host's id; 0 on a mission host.
pub const JOIN_MISSION_HOST: usize = 0xC8;
/// `join_request +0xD8`: the fireteam id (0 on a public instance).
pub const JOIN_FIRETEAM: usize = 0xD8;
/// `join_request +0x10`: the client's 40-byte identity, which keys the character here (stream O2).
pub const JOIN_IDENTITY: usize = 0x10;
/// The identity's size.
pub const JOIN_IDENTITY_SIZE: usize = 40;

/// What a connection is, read from its join request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostRole {
    /// A mission host (or a social space): `+0xC8 = 0`.
    Mission { kind: JoinKind, fireteam: u64 },
    /// A public instance beside `mission_host`.
    PublicInstance { kind: JoinKind, mission_host: u64 },
}

impl HostRole {
    pub fn is_public_instance(self) -> bool {
        matches!(self, Self::PublicInstance { .. })
    }
}

fn u64_at(image: &[u8], at: usize) -> u64 {
    image.get(at..at + 8).map_or(0, |b| u64::from_le_bytes(b.try_into().unwrap()))
}

/// The role a join request asks for.
pub fn role_of_join(image: &[u8]) -> HostRole {
    let kind = if image.get(JOIN_SESSION_TYPE) == Some(&2) { JoinKind::Joins } else { JoinKind::Starts };
    match u64_at(image, JOIN_MISSION_HOST) {
        0 => HostRole::Mission { kind, fireteam: u64_at(image, JOIN_FIRETEAM) },
        mission_host => HostRole::PublicInstance { kind, mission_host },
    }
}

/// The character key for the open world: the first 8 bytes of the join's identity.
pub fn character_of_join(image: &[u8]) -> u64 {
    u64_at(image, JOIN_IDENTITY)
}

/// ⭐ Stream O2: the character for the registry and for rewards: the join's whole 40-byte
/// identity, as stream K's [`crate::rewards::CharacterRef`] (a short image gives what it has).
pub fn identity_of_join(image: &[u8]) -> crate::rewards::CharacterRef {
    let end = (JOIN_IDENTITY + JOIN_IDENTITY_SIZE).min(image.len());
    crate::rewards::CharacterRef { identity: image.get(JOIN_IDENTITY..end).unwrap_or_default().to_vec() }
}

/// The two slice-set records of a udt 21 image: `(CURRENT, TARGET)` region ids. A record is absent
/// when it is all zeros (the update is a delta) or its index is `0x3FF`. Its name hash may be the
/// FNV-1 of the empty name (`0x811C9DC5`): on its new public instance the client's TARGET was
/// region 56 with that hash (run fg6c: `0x38`/`0x811c9dc5`, CURRENT still zero), and treating it as
/// unset left the instance with no list to serve.
pub fn slice_regions(image: &[u8]) -> (Option<u32>, Option<u32>) {
    let rec = |base: usize| {
        let index = image.get(base..base + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()) & 0x3FF)?;
        let hash = image.get(base + 4..base + 8).map(|b| u32::from_le_bytes(b.try_into().unwrap()))?;
        (hash != 0 && index != 0x3FF).then_some(index)
    };
    (rec(0xB8), rec(0x158))
}

/// Make every region slot of a `replicate_membership` image say "no region" (`0xFFFFFFFF`), as a
/// live public instance's membership does.
pub fn clear_region_records(image: &mut [u8]) {
    use crate::messages::ReplicateMembership as M;
    for slot in 0..M::PAH_REGION_CAPACITY {
        let o = M::PAH_REGIONS + slot * M::PAH_REGION_STRIDE;
        let Some(rec) = image.get_mut(o..o + M::PAH_REGION_STRIDE) else { return };
        rec.fill(0);
        rec[0..4].copy_from_slice(&u32::MAX.to_le_bytes());
    }
}

/// The union of entity-slot sets (udt 25's set at `+0x04..+0x604`). The live host answered two 25s
/// that arrived together with one 23 carrying their union (A Guardian Rises, 772 s).
pub fn merge_slot_sets(sets: &[&[u8]]) -> Vec<u8> {
    let len = sets.iter().map(|s| s.len()).max().unwrap_or(0);
    let mut out = vec![0u8; len];
    for s in sets {
        for (o, b) in out.iter_mut().zip(s.iter()) {
            *o |= b;
        }
    }
    out
}

/// One public region's served list, parsed from its fixture.
#[derive(Debug, Clone)]
pub struct PublicRegionList {
    pub key: InstanceKey,
    pub file: String,
    /// The first full list, in served order.
    pub clients: Vec<SensorClientRef>,
    /// Records that appeared later on the live host (event and loot records), not served yet.
    pub later: usize,
    /// Records whose kind no native list has yet, commented out of the fixture.
    pub unread_kinds: usize,
}

impl PublicRegionList {
    pub fn parse(key: InstanceKey, file: &str, text: &str) -> Self {
        let mut clients = Vec::new();
        let (mut later, mut unread_kinds) = (0, 0);
        for line in text.lines() {
            let t = line.trim();
            if t.starts_with("# later") {
                later += 1;
            } else if t.starts_with("# kind") {
                unread_kinds += 1;
            } else if let Some(c) = SensorClientRef::parse_line(t) {
                clients.push(c);
            }
        }
        Self { key, file: file.to_string(), clients, later, unread_kinds }
    }

    /// The sensor groups in order of first appearance, with their record counts.
    pub fn groups(&self) -> Vec<(u32, usize)> {
        let mut out: Vec<(u32, usize)> = Vec::new();
        for c in &self.clients {
            match out.iter_mut().find(|(h, _)| *h == c.name_hash) {
                Some((_, n)) => *n += 1,
                None => out.push((c.name_hash, 1)),
            }
        }
        out
    }
}

/// Load `public-table.txt` (`destination region kind list records` per line) and every list it
/// names, from the table's folder.
pub fn load_public_table(path: &Path) -> Result<Vec<PublicRegionList>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut out = Vec::new();
    for line in text.lines().map(|l| l.split('#').next().unwrap_or("").trim()).filter(|l| !l.is_empty()) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let [place, region, kind, file, ..] = f.as_slice() else {
            return Err(format!("{}: want `destination region kind list`: {line:?}", path.display()));
        };
        let key = match *kind {
            "public" => InstanceKey::Bubble {
                destination: Destination::parse(place).ok_or_else(|| format!("unknown destination {place:?}"))?,
                region: region.parse().map_err(|_| format!("bad region {region:?}"))?,
            },
            "social" => InstanceKey::Social(match *place {
                "tower" => SocialSpace::Tower,
                "reef" => SocialSpace::Reef,
                "felwinter" => SocialSpace::FelwinterPeak,
                other => return Err(format!("unknown social space {other:?}")),
            }),
            other => return Err(format!("unknown kind {other:?}")),
        };
        let list = std::fs::read_to_string(dir.join(file)).map_err(|e| format!("{file}: {e}"))?;
        out.push(PublicRegionList::parse(key, file, &list));
    }
    Ok(out)
}

/// Builds a region's sensor frame from its list. The live public lists have no lifetime-phase
/// record, so this must be an **entry-only** frame: stream A2's `d1_oracle::sensor::SensorEntryFrame`
/// (gen, no authority, `init = 1`, the entries) once it is merged. Built once at setup, never on
/// the send path.
pub type FrameEncoder = Arc<dyn Fn(&[SensorClientRef]) -> Result<Vec<u8>, String> + Send + Sync>;

/// Builds one joining client's frame: an authority table and the entries (run fg6d, F32). The
/// live public instance granted the client its authority blocks and named it in its player
/// record; both depend on the join, so this frame is built when the region is picked.
pub type PersonalEncoder =
    Arc<dyn Fn(Option<d1_oracle::sensor::SensorAuthority>, &[SensorClientRef]) -> Result<Vec<u8>, String> + Send + Sync>;

/// [`entry_frame_encoder`] with an authority table.
pub fn personal_frame_encoder(codec: Arc<dyn crate::codec::AhWireCodec>, gen: u8) -> PersonalEncoder {
    Arc::new(move |authority, clients: &[SensorClientRef]| {
        let frame = d1_oracle::sensor::SensorEntryFrame { gen, authority, init: true, entries: clients.to_vec() };
        codec.encode_sensor_entries(&frame).map(|(bytes, _)| bytes).map_err(|e| format!("personal frame: {e}"))
    })
}

/// A region's list for one joining client: its player record (the players group's record
/// `slot`) names the client's key, as the live instance's did (records 1-15 stayed default).
pub fn personal_clients(list: &[SensorClientRef], identity: &[u8], slot: i16) -> Vec<SensorClientRef> {
    use crate::row_encoders as re;
    let key = re::player_key_of_identity(identity);
    list.iter()
        .cloned()
        .map(|mut c| {
            if let Some(key) = key {
                if c.name_hash == re::PUBLIC_PLAYERS_GROUP && c.name_i8 == re::KIND_PLAYER_BUNDLE && c.name_sub == slot && c.has_a {
                    c.a_value = Some((re::TAG_PLAYER_BUNDLE, re::player_bundle_image(key, false)));
                }
            }
            c
        })
        .collect()
}

/// ⚠ **PLACEHOLDER**: how long after an instance starts its chests and resource nodes show. The
/// restored scripts' notes put the Cosmodrome's at about a minute; nothing here was measured by us.
pub const PLACEHOLDER_LOOT_DELAY: std::time::Duration = std::time::Duration::from_secs(60);

/// What a region's instance runs besides its sensor list: its bundles and its chests and resource
/// nodes, read from the region's list.
#[derive(Debug, Clone)]
pub struct RegionPlan {
    pub bundles: Vec<(u32, BundleRole)>,
    pub loot: Vec<(u32, i16, LootKind)>,
}

impl RegionPlan {
    /// The plan for a list: every group after the players is a bundle; every kind-`0x4` record of a
    /// chest or resource-node bundle is one loot object. Loot inferred only from its shape
    /// ([`BundleRole::LootInferred`]) is left out: whether it is a chest or a node is not known.
    pub fn from_list(list: &PublicRegionList) -> Self {
        let mut kinds: Vec<(u32, BTreeMap<u8, usize>)> = Vec::new();
        for c in &list.clients {
            match kinds.iter_mut().find(|(h, _)| *h == c.name_hash) {
                Some((_, k)) => *k.entry(c.name_i8).or_insert(0) += 1,
                None => kinds.push((c.name_hash, BTreeMap::from([(c.name_i8, 1)]))),
            }
        }
        let bundles: Vec<(u32, BundleRole)> = kinds
            .iter()
            .map(|(h, k)| (*h, bundle_role(*h, k)))
            .filter(|(_, r)| *r != BundleRole::Players)
            .collect();
        let loot = list
            .clients
            .iter()
            .filter(|c| c.name_i8 == 0x4)
            .filter_map(|c| match bundles.iter().find(|(h, _)| *h == c.name_hash)?.1 {
                BundleRole::Chests => Some((c.name_hash, c.name_sub, LootKind::Chest)),
                BundleRole::ResourceNodes => Some((c.name_hash, c.name_sub, LootKind::ResourceNode)),
                _ => None,
            })
            .collect();
        Self { bundles, loot }
    }
}

/// The services an instance's open world calls: stream A2's script bridge and stream M's rewards.
#[derive(Clone)]
pub struct OpenWorldServices {
    pub bridge: Arc<Mutex<dyn BubbleScriptBridge + Send>>,
    pub rewards: Arc<Mutex<dyn ActivityRewards + Send>>,
}

/// One running instance's open world: its event rotations, its loot and its patrol beacons.
#[derive(Debug, Clone)]
pub struct InstanceWorld {
    pub key: InstanceKey,
    pub events: Vec<PublicEventRunner>,
    pub loot: LootField,
    pub patrol: PatrolBoard,
    /// Stream O3: every completed event's incident set, in order
    /// (`public_events::completion_incidents`), for each connection to send its client once.
    pub completions: Vec<&'static [u32]>,
}

/// The completion incidents of the `Ended` notices in `notices`.
fn completions_of(notices: &[EventNotice]) -> impl Iterator<Item = &'static [u32]> + '_ {
    notices.iter().filter_map(|n| match n {
        EventNotice::Ended { result, .. } => Some(crate::public_events::completion_incidents(*result)).filter(|s| !s.is_empty()),
        _ => None,
    })
}

/// The public-host state one activity-host connection carries.
#[derive(Clone)]
pub struct PublicHost {
    /// Every public region's frame, built once at setup (an `Err` names why one could not be).
    frames: Arc<HashMap<InstanceKey, Result<Vec<u8>, String>>>,
    /// Shared by every connection (and, later, by matchmaking).
    registry: Arc<Mutex<PublicRegistry>>,
    role: Option<HostRole>,
    /// The joined client's identity (the registry's key), and its 8-byte key (the open world's).
    identity: crate::rewards::CharacterRef,
    character: u64,
    selected: Option<InstanceKey>,
    /// The open-world layer, when the host was given one ([`Self::with_open_world`]).
    plans: Arc<HashMap<InstanceKey, RegionPlan>>,
    services: Option<OpenWorldServices>,
    /// Every running instance's open world, by host id, shared by every connection.
    worlds: Arc<Mutex<HashMap<u64, InstanceWorld>>>,
    host_id: u64,
    /// Each world's PUBLIC checksum (`worlds.tsv`, stream E), published by a public instance.
    public_checksums: Arc<HashMap<String, u32>>,
    /// Stream O3: how many of the instance's event completions this connection has sent.
    completions_seen: usize,
    /// The last (CURRENT, TARGET) regions a udt 21 named while no list was picked, so each new
    /// pair is logged once (run fg6: the instance picked nothing and said nothing).
    last_report: Option<(Option<u32>, Option<u32>)>,
    /// Each region's list and the per-client encoder ([`Self::with_personal_frames`]); without
    /// them the prebuilt entry-only frame is served.
    lists: Arc<HashMap<InstanceKey, Vec<SensorClientRef>>>,
    personal: Option<PersonalEncoder>,
}

impl std::fmt::Debug for PublicHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PublicHost")
            .field("regions", &self.frames.len())
            .field("role", &self.role)
            .field("selected", &self.selected)
            .finish()
    }
}

impl PublicHost {
    /// Build every list's frame now (see [`FrameEncoder`]).
    pub fn build_frames(lists: &[PublicRegionList], encode: &FrameEncoder) -> HashMap<InstanceKey, Result<Vec<u8>, String>> {
        lists.iter().map(|l| (l.key, encode(&l.clients))).collect()
    }

    pub fn new(frames: Arc<HashMap<InstanceKey, Result<Vec<u8>, String>>>, registry: Arc<Mutex<PublicRegistry>>) -> Self {
        Self {
            frames,
            registry,
            role: None,
            identity: crate::rewards::CharacterRef { identity: Vec::new() },
            character: 0,
            selected: None,
            plans: Arc::new(HashMap::new()),
            services: None,
            worlds: Arc::new(Mutex::new(HashMap::new())),
            host_id: 0,
            public_checksums: Arc::new(HashMap::new()),
            last_report: None,
            completions_seen: 0,
            lists: Arc::new(HashMap::new()),
            personal: None,
        }
    }

    /// Serve each joining client its own frame (F32): the join grant and its player record, from
    /// `lists` through `encode`.
    pub fn with_personal_frames(mut self, lists: &[PublicRegionList], encode: PersonalEncoder) -> Self {
        self.lists = Arc::new(lists.iter().map(|l| (l.key, l.clients.clone())).collect());
        self.personal = Some(encode);
        self
    }

    /// The worlds' PUBLIC checksums (`scenario_record+0x10`, `worlds.tsv`'s `public_checksum`).
    /// A public instance of a listed world publishes it at `global_activity_state+0x5C` instead of
    /// the mission's PRIVATE one: the activity client of a public bubble is `[AC PUBLIC ...]`
    /// (notes/ps4-56578-public-instances-2026-11-05.md). Not measured on the wire yet.
    pub fn with_public_checksums(mut self, checksums: HashMap<String, u32>) -> Self {
        self.public_checksums = Arc::new(checksums);
        self
    }

    /// The PUBLIC checksum of `world`, when the table has it.
    pub fn public_checksum(&self, world: &str) -> Option<u32> {
        self.public_checksums.get(world).copied()
    }

    /// Run each instance's open world: start its bundles through the bridge, rotate its public
    /// events, and keep its loot and patrol beacons, reporting rewards to `services.rewards`. The
    /// worlds are kept per instance and shared by every connection to it.
    pub fn with_open_world(mut self, lists: &[PublicRegionList], services: OpenWorldServices) -> Self {
        self.plans = Arc::new(lists.iter().map(|l| (l.key, RegionPlan::from_list(l))).collect());
        self.services = Some(services);
        self
    }

    /// Look at this connection's instance's open world.
    pub fn with_world<R>(&self, f: impl FnOnce(&InstanceWorld) -> R) -> Option<R> {
        self.worlds.lock().ok()?.get(&self.host_id).map(f)
    }

    fn world_mut<R>(&self, f: impl FnOnce(&mut InstanceWorld, &OpenWorldServices) -> Result<R, String>) -> Result<R, String> {
        let services = self.services.as_ref().ok_or("no open world on this host")?;
        let mut worlds = self.worlds.lock().map_err(|e| e.to_string())?;
        let world = worlds.get_mut(&self.host_id).ok_or("no instance running")?;
        f(world, services)
    }

    /// Start the instance's open world once its place is known (once per instance: a second
    /// connection to the same instance finds it running).
    fn start_world(&mut self, key: InstanceKey, host_id: u64, now: Instant) {
        self.host_id = host_id;
        let (Some(services), Some(plan)) = (self.services.clone(), self.plans.get(&key).cloned()) else { return };
        let Ok(mut worlds) = self.worlds.lock() else { return };
        // Stream O3: a connection that arrives later is sent only the events that end from now on.
        self.completions_seen = worlds.get(&host_id).map_or(0, |w| w.completions.len());
        if worlds.contains_key(&host_id) {
            return;
        }
        let bundles: Vec<u32> = plan.bundles.iter().map(|(h, _)| *h).collect();
        if let Ok(mut b) = services.bridge.lock() {
            if let Err(e) = b.start_bundles(host_id, &bundles) {
                tracing::warn!("⚠ public instance {host_id:#018x}: bundles not started: {e}");
            }
        }
        let events = bundles
            .iter()
            .filter_map(|&b| schedule_for(b))
            .map(|s| PublicEventRunner::new(host_id, s, now))
            .collect();
        let loot = LootField::new(now, plan.loot.iter().map(|&(b, i, k)| (b, i, k, PLACEHOLDER_LOOT_DELAY)));
        let patrol = match key {
            InstanceKey::Bubble { destination, region } => PatrolBoard::new(
                &PLACEHOLDER_BEACONS
                    .iter()
                    .copied()
                    .filter(|&(d, r, _, _)| d == destination && r == region)
                    .collect::<Vec<_>>(),
            ),
            InstanceKey::Social(_) => PatrolBoard::default(),
        };
        worlds.insert(host_id, InstanceWorld { key, events, loot, patrol, completions: Vec::new() });
    }

    /// Advance the instance's event rotations. Called from the host's tick; any connection to the
    /// instance may call it, and a second call at the same time does nothing more.
    pub fn tick(&self, now: Instant) {
        let host = self.host_id;
        let _ = self.world_mut(|world, services| {
            let mut bridge = services.bridge.lock().map_err(|e| e.to_string())?;
            let mut rewards = services.rewards.lock().map_err(|e| e.to_string())?;
            let mut ended = Vec::new();
            for run in &mut world.events {
                for n in run.tick(now, &mut *bridge, &mut *rewards) {
                    tracing::info!("⭐ public instance {host:#018x}: {n:?}");
                    ended.push(n);
                }
            }
            world.completions.extend(completions_of(&ended));
            Ok(())
        });
    }

    /// The bridge reports that `character` engaged a running event of `bundle`.
    pub fn report_engagement(&self, bundle: u32, character: u64) {
        let _ = self.world_mut(|world, _| {
            if let Some(run) = world.events.iter_mut().find(|r| r.bundle() == bundle) {
                run.on_engagement(character);
            }
            Ok(())
        });
    }

    /// The bridge reports that `bundle`'s running event ended.
    pub fn report_event_finished(&self, bundle: u32, result: EventResult, now: Instant) -> Vec<EventNotice> {
        self.world_mut(|world, services| {
            let run = world.events.iter_mut().find(|r| r.bundle() == bundle).ok_or("no such bundle")?;
            let mut rewards = services.rewards.lock().map_err(|e| e.to_string())?;
            let notices = run.on_script_finished(result, now, &mut *rewards);
            world.completions.extend(completions_of(&notices));
            Ok(notices)
        })
        .unwrap_or_default()
    }

    /// Stream O3: the completion incidents of events that ended since this connection last asked
    /// (gold 1918, 3077, 732; silver 1918, 3076), for its client.
    pub fn take_completion_incidents(&mut self) -> Vec<u32> {
        let seen = self.completions_seen;
        let Ok(new) = self.world_mut(|world, _| Ok(world.completions.get(seen..).unwrap_or_default().to_vec())) else { return Vec::new() };
        self.completions_seen += new.len();
        new.into_iter().flatten().copied().collect()
    }

    /// A character opens a chest or harvests a node in this instance.
    pub fn open_loot(&self, character: u64, bundle: u32, index: i16, now: Instant) -> Result<(), String> {
        self.world_mut(|world, services| {
            let mut rewards = services.rewards.lock().map_err(|e| e.to_string())?;
            world.loot.open(character, bundle, index, now, 0, &mut *rewards)
        })
    }

    /// Accept a patrol beacon's mission in this instance.
    pub fn patrol_accept(&self, character: u64, beacon: u32) -> Result<(), String> {
        self.world_mut(|world, _| world.patrol.accept(character, beacon))
    }

    /// A character's patrol progress (a kill, a scan, an item). Returns whether it completed.
    pub fn patrol_progress(&self, character: u64, amount: u32) -> Result<bool, String> {
        self.world_mut(|world, services| {
            let mut rewards = services.rewards.lock().map_err(|e| e.to_string())?;
            world.patrol.progress(character, amount, &mut *rewards)
        })
    }

    pub fn role(&self) -> Option<HostRole> {
        self.role
    }

    pub fn is_public_instance(&self) -> bool {
        self.role.is_some_and(HostRole::is_public_instance)
    }

    pub fn selected(&self) -> Option<InstanceKey> {
        self.selected
    }

    pub fn registry(&self) -> &Arc<Mutex<PublicRegistry>> {
        &self.registry
    }

    /// The client's join: note what this connection is.
    ///
    /// A mission-type join to a social space's world (`world`, the activity's world name) is that
    /// space's shared instance: it is registered under [`InstanceKey::Social`] and its open world
    /// starts, while the world itself is still served by the world table (the Tower's lists).
    pub fn on_join(&mut self, join_request: &[u8], world: &str, host_id: u64, now: Instant) -> HostRole {
        let role = role_of_join(join_request);
        self.role = Some(role);
        self.character = character_of_join(join_request);
        self.identity = identity_of_join(join_request);
        crate::rewards::remember_identity(&self.identity);
        self.selected = None;
        self.host_id = host_id;
        match role {
            HostRole::PublicInstance { kind, mission_host } => tracing::info!(
                "⭐ public instance: the join names mission host {mission_host:#018x} ({kind:?}); serving no region                  records and no mission steps, and waiting for the client's slice set to pick the region's list"
            ),
            HostRole::Mission { kind, .. } => {
                // Stream O3: the player's destination, for the responder's bubble placement.
                if let Some(d) = Destination::from_world_name(world) {
                    if let Ok(mut r) = self.registry.lock() {
                        r.note_mission(&self.identity, d);
                        crate::public_registry::sync_with_responder(&mut r);
                    }
                }
                if let Some(space) = SocialSpace::from_world_name(world) {
                    let key = InstanceKey::Social(space);
                    self.selected = Some(key);
                    let joined = self.registry.lock().ok().map(|mut r| {
                        crate::public_registry::sync_with_responder(&mut r);
                        let j = r.attach(host_id, key, self.identity.clone(), now);
                        crate::public_registry::sync_with_responder(&mut r);
                        j
                    });
                    tracing::info!("⭐ social space {space:?} on host {host_id:#018x} (session {kind:?}, {joined:?})");
                    self.start_world(key, host_id, now);
                }
            }
        }
        role
    }

    /// The client's udt 21 on a public instance: once it names a public region of `world`'s
    /// destination this host has a list for, register the instance and return the region's frame
    /// (once). `Err` is a region with a list whose frame could not be built.
    pub fn on_slice_report(
        &mut self,
        world: &str,
        udt21: &[u8],
        host_id: u64,
        bubbles: Option<usize>,
        now: Instant,
    ) -> Option<Result<(InstanceKey, Vec<u8>), String>> {
        if !self.is_public_instance() || self.selected.is_some() {
            return None;
        }
        let (current, target) = slice_regions(udt21);
        let Some(destination) = Destination::from_world_name(world) else {
            if self.last_report.replace((current, target)) != Some((current, target)) {
                tracing::warn!("⚠ public instance {host_id:#018x}: world {world:?} names no destination; no list for CURRENT {current:?} / TARGET {target:?}");
            }
            return None;
        };
        let found = [target, current].into_iter().flatten().map(|region| InstanceKey::Bubble { destination, region })
            .find(|k| self.frames.contains_key(k));
        let Some(key) = found else {
            if self.last_report.replace((current, target)) != Some((current, target)) {
                let word = |o: usize| udt21.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
                tracing::info!(
                    "public instance {host_id:#018x}: the client's slice sets are CURRENT {current:?} / TARGET {target:?} \
                     ({destination:?}); no public list for either yet. Raw records (index word, name hash): \
                     CURRENT +0xB8 {:x?}/{:x?}, TARGET +0x158 {:x?}/{:x?}, image {} bytes",
                    word(0xB8), word(0xBC), word(0x158), word(0x15C), udt21.len()
                );
            }
            return None;
        };
        self.selected = Some(key);
        let joined = self.registry.lock().ok().map(|mut r| {
            crate::public_registry::sync_with_responder(&mut r);
            let j = r.attach(host_id, key, self.identity.clone(), now);
            crate::public_registry::sync_with_responder(&mut r);
            j
        });
        tracing::info!("⭐ public instance {host_id:#018x} is {key:?} ({joined:?})");
        self.start_world(key, host_id, now);
        // F32: the joining client's own frame, as the live instance served it: the join grant
        // (blocks 0..bubbles and 64 to its machine, seq 1) and its key in the players group's
        // record 0 (its peer slot; ⚠ a second member's slot is not tracked here yet).
        if let (Some(encode), Some(list)) = (self.personal.as_ref(), self.lists.get(&key)) {
            let identity = self.identity.identity.clone();
            let authority = crate::row_encoders::machine_of_identity(&identity)
                .zip(bubbles)
                .map(|(m, n)| d1_oracle::sensor::SensorAuthority::join_grant(m, n));
            let clients = personal_clients(list, &identity, 0);
            match encode(authority.clone(), &clients) {
                Ok(frame) => {
                    tracing::info!(
                        "⭐ public instance {host_id:#018x}: the client's own frame: {}, and its player record (F32)",
                        if authority.is_some() { "the join grant" } else { "no grant (no bubble count)" }
                    );
                    return Some(Ok((key, frame)));
                }
                Err(e) => tracing::warn!("⚠ public instance {host_id:#018x}: the client's own frame not built ({e}); the shared one is served"),
            }
        }
        Some(match &self.frames[&key] {
            Ok(frame) => Ok((key, frame.clone())),
            Err(e) => Err(format!("{key:?}: {e}")),
        })
    }

    /// The client left this instance (peer leave or lost connection).
    pub fn on_leave(&mut self, now: Instant) {
        if self.selected.take().is_some() {
            if let Ok(mut r) = self.registry.lock() {
                r.leave(self.identity.clone(), now);
                crate::public_registry::sync_with_responder(&mut r);
            }
            // The character's patrol ends with its visit; the instance's world runs on.
            let character = self.character;
            let _ = self.world_mut(|world, _| {
                world.patrol.abandon(character);
                Ok(())
            });
        }
    }
}

/// ⭐ **The entry-only encoder** (#319, stream O): a region's list as a
/// `d1_oracle::sensor::SensorEntryFrame` (gen, no authority, `init = 1`, the entries), through the
/// host's wire codec's `encode_sensor_entries`: d1-rsat under `D1_RSAT_NATIVE=1` where it covers
/// the schemas, the client's own codec otherwise. `gen` must equal the client's `world+0x1D`, as
/// for every udt 5 frame (`D1_AH_SENSOR_GEN`, default 0).
pub fn entry_frame_encoder(codec: Arc<dyn crate::codec::AhWireCodec>, gen: u8) -> FrameEncoder {
    Arc::new(move |clients: &[SensorClientRef]| {
        let frame = d1_oracle::sensor::SensorEntryFrame { gen, authority: None, init: true, entries: clients.to_vec() };
        codec.encode_sensor_entries(&frame).map(|(bytes, _)| bytes).map_err(|e| format!("entry-only frame: {e}"))
    })
}

/// The frames a host with no entry-only encoder serves: none, each named.
pub fn encoder_pending() -> FrameEncoder {
    Arc::new(|_| {
        Err("an entry-only sensor frame (no lifetime record) needs stream A2's \
             d1_oracle::sensor::SensorEntryFrame, not merged yet"
            .to_string())
    })
}

/// The players group must lead every public list, as it did on every live public instance.
pub fn players_group_leads(list: &PublicRegionList) -> bool {
    list.groups().first() == Some(&(PUBLIC_PLAYERS_GROUP, 16))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::open_world::{bundle_role, BundleRole};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    /// F32: only the joining client's peer-slot player record gets its key; the rest of the list
    /// is untouched, as on the live instance (records 1-15 stayed default).
    #[test]
    fn the_personal_list_names_the_client_in_its_player_record_only() {
        let list: Vec<SensorClientRef> = [
            "0xeaaf16e2 0xc 0 1 1",
            "0xeaaf16e2 0xc 1 1 1",
            "0x12345678 0xc 0 1 1",
        ]
        .iter()
        .map(|l| SensorClientRef::parse_line(l).unwrap())
        .collect();
        let mut identity = vec![0u8; 40];
        identity[0x18..0x20].copy_from_slice(&0xD100_0000_0000_0201u64.to_le_bytes());
        let out = personal_clients(&list, &identity, 0);
        let (tag, image) = out[0].a_value.clone().unwrap();
        assert_eq!(tag, crate::row_encoders::TAG_PLAYER_BUNDLE);
        assert_eq!(&image[0x40..0x48], &[0xD1, 0, 0, 0, 0, 0, 2, 1]);
        assert!(out[1].a_value.is_none() && out[2].a_value.is_none());
        assert_eq!(personal_clients(&list, &[], 0), list, "no identity: nothing changes");
    }

    /// The live join grant: blocks 0..bubbles and 64, each the machine id, seq 1, not applied.
    #[test]
    fn the_join_grant_covers_the_bubbles_and_the_domain_block() {
        let g = d1_oracle::sensor::SensorAuthority::join_grant([1, 0x22, 3, 4, 5, 6, 7, 8], 22);
        assert_eq!(g.changed, (1u128 << 64) | ((1u128 << 22) - 1));
        let e = |i: usize| &g.image[4 + 12 * i..4 + 12 * i + 12];
        assert_eq!(&e(21)[..8], &[1, 0x22, 3, 4, 5, 6, 7, 8]);
        assert_eq!((e(64)[8], e(64)[9], e(64)[0xA]), (1, 0, 0));
        assert!(e(22).iter().all(|&b| b == 0));
    }

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/56578/public")
    }

    /// Stream O: a public list goes through the client's own codec as an entry-only frame: gen
    /// first, no authority table, the init bit set.
    #[test]
    fn a_public_list_encodes_as_an_entry_only_frame() {
        if !crate::eboot_or_skip("a_public_list_encodes_as_an_entry_only_frame") {
            return;
        }
        let lists = load_public_table(&fixtures().join("public-table.txt")).unwrap();
        let encode = entry_frame_encoder(Arc::new(crate::codec::SchemaCodec), 0);
        let frames = PublicHost::build_frames(&lists[..1], &encode);
        let frame = frames[&lists[0].key].as_ref().expect("the client's codec encodes the list");
        assert_eq!(frame[0], 0, "gen");
        assert_eq!(d1_oracle::sensor::known_stamp_bit(frame), Some(9), "no authority table: the stamp follows gen and its bit");
        assert!(frame.len() > lists[0].clients.len(), "{} bytes for {} entries", frame.len(), lists[0].clients.len());
    }

    #[test]
    fn a_public_instance_knows_its_worlds_public_checksum() {
        let lists = load_public_table(&fixtures().join("public-table.txt")).unwrap();
        let host = PublicHost::new(Arc::new(PublicHost::build_frames(&lists, &encoder_pending())), Arc::new(Mutex::new(PublicRegistry::new(1))))
            .with_public_checksums(HashMap::from([("act_cosmo_m1_dome_a".to_string(), 0xe354_f1ea)]));
        assert_eq!(host.public_checksum("act_cosmo_m1_dome_a"), Some(0xe354_f1ea));
        assert_eq!(host.public_checksum("pvp_moonbase"), None, "an unlisted world keeps the mission's");
    }

    fn join(kind: u8, mission_host: u64, fireteam: u64, character: u64) -> Vec<u8> {
        let mut img = vec![0u8; 0x100];
        img[JOIN_IDENTITY..JOIN_IDENTITY + 8].copy_from_slice(&character.to_le_bytes());
        img[JOIN_SESSION_TYPE] = kind;
        img[JOIN_MISSION_HOST..JOIN_MISSION_HOST + 8].copy_from_slice(&mission_host.to_le_bytes());
        img[JOIN_FIRETEAM..JOIN_FIRETEAM + 8].copy_from_slice(&fireteam.to_le_bytes());
        img
    }

    /// The live join requests' three fields, as the Restoration capture's four joins carry them
    /// (synthetic values, live layout).
    #[test]
    fn a_join_that_names_a_mission_host_is_a_public_instance() {
        let pah = 0xD1A0_0000_0000_0001;
        assert_eq!(role_of_join(&join(3, 0, 0xF1, 1)), HostRole::Mission { kind: JoinKind::Starts, fireteam: 0xF1 });
        assert_eq!(role_of_join(&join(3, pah, 0, 1)), HostRole::PublicInstance { kind: JoinKind::Starts, mission_host: pah });
        assert_eq!(role_of_join(&join(2, pah, 0, 1)), HostRole::PublicInstance { kind: JoinKind::Joins, mission_host: pah });
        // The Tower: a join (type 2) with no mission host is a social space, served as a host of its own.
        assert_eq!(role_of_join(&join(2, 0, 0xF1, 1)), HostRole::Mission { kind: JoinKind::Joins, fireteam: 0xF1 });
        assert_eq!(character_of_join(&join(3, pah, 0, 0xABCD)), 0xABCD);
    }

    #[test]
    fn no_region_records_is_every_slot_terminated() {
        use crate::messages::ReplicateMembership as M;
        let mut img = vec![0u8; M::PAH_REGIONS + M::PAH_REGION_CAPACITY * M::PAH_REGION_STRIDE];
        img[M::PAH_REGIONS..M::PAH_REGIONS + 4].copy_from_slice(&56u32.to_le_bytes());
        img[M::PAH_REGIONS + 0x38] = 1;
        clear_region_records(&mut img);
        for slot in 0..M::PAH_REGION_CAPACITY {
            let o = M::PAH_REGIONS + slot * M::PAH_REGION_STRIDE;
            assert_eq!(&img[o..o + 4], &[0xFF; 4]);
            assert!(img[o + 4..o + M::PAH_REGION_STRIDE].iter().all(|&b| b == 0));
        }
    }

    #[test]
    fn slot_sets_merge_as_a_union() {
        let a = [0b0001u8, 0, 0x80];
        let b = [0b0100u8, 1, 0];
        assert_eq!(merge_slot_sets(&[&a, &b]), vec![0b0101, 1, 0x80]);
    }

    /// ⭐ Every fixture loads, leads with the 16 player objects, and carries no lifetime record
    /// (kind `0x10`), as the live public lists did; Restoration's region 56 has the live groups.
    #[test]
    fn the_public_fixtures_load_with_the_live_groups() {
        let lists = load_public_table(&fixtures().join("public-table.txt")).unwrap();
        let bubbles: Vec<_> = lists.iter().filter(|l| matches!(l.key, InstanceKey::Bubble { .. })).collect();
        assert!(bubbles.len() >= 20, "{} public lists", bubbles.len());
        for d in [Destination::Earth, Destination::Moon, Destination::Venus, Destination::Mars, Destination::Dreadnaught] {
            assert!(bubbles.iter().any(|l| matches!(l.key, InstanceKey::Bubble { destination, .. } if destination == d)), "{d:?}");
        }
        for l in &bubbles {
            assert!(players_group_leads(l), "{}: {:?}", l.file, l.groups().first());
            assert!(l.clients.iter().all(|c| c.name_i8 != 0x10), "{}: a lifetime record", l.file);
            let InstanceKey::Bubble { destination, region } = l.key else { unreachable!() };
            assert!(destination.public_regions_seen().contains(&region), "{}: region {region} never marked public", l.file);
        }
        let steppes = bubbles
            .iter()
            .find(|l| l.key == InstanceKey::Bubble { destination: Destination::Earth, region: 56 })
            .unwrap();
        assert_eq!(
            steppes.groups(),
            vec![(0xEAAF_16E2, 16), (0xD7F6_8DA0, 74), (0xED3C_5B0A, 183), (0x70C1_FC84, 6), (0xF55C_57E4, 6)]
        );
        let social: Vec<_> = lists.iter().filter(|l| matches!(l.key, InstanceKey::Social(_))).collect();
        assert_eq!(social.len(), 2);
        for l in social {
            let players = l.clients.iter().filter(|c| c.name_hash == 0x40AE_BF90 && c.name_i8 == 0xC).count();
            assert_eq!(players, SocialSpace::PLAYER_OBJECTS, "{}", l.file);
        }
        // Region 48's bundles are the ones the restored scripts name.
        let mothyards = bubbles
            .iter()
            .find(|l| l.key == InstanceKey::Bubble { destination: Destination::Earth, region: 48 })
            .unwrap();
        let roles: Vec<BundleRole> = mothyards.groups().iter().map(|&(h, _)| bundle_role(h, &BTreeMap::new())).collect();
        assert_eq!(
            roles,
            vec![BundleRole::Players, BundleRole::Ambient, BundleRole::PublicEvents, BundleRole::ResourceNodes, BundleRole::Chests]
        );
    }

    fn udt21(current: Option<u32>, target: Option<u32>) -> Vec<u8> {
        let mut img = vec![0u8; 0x220];
        for (base, r) in [(0xB8, current), (0x158, target)] {
            if let Some(r) = r {
                img[base..base + 4].copy_from_slice(&r.to_le_bytes());
                img[base + 4..base + 8].copy_from_slice(&0x2EA8_FB98u32.to_le_bytes());
            }
        }
        img
    }

    /// Run fg6c's first udt 21 on the client's new public instance: CURRENT all zeros, TARGET
    /// region 56 (`0x38`) with the empty-name hash. The TARGET counts; the zeroed CURRENT does not.
    #[test]
    fn a_target_with_the_empty_name_hash_still_names_its_region() {
        let mut img = vec![0u8; 0x220];
        img[0x158..0x15C].copy_from_slice(&0x38u32.to_le_bytes());
        img[0x15C..0x160].copy_from_slice(&0x811C_9DC5u32.to_le_bytes());
        assert_eq!(slice_regions(&img), (None, Some(56)));
        img[0x158..0x15C].copy_from_slice(&0x3FFu32.to_le_bytes());
        assert_eq!(slice_regions(&img), (None, None), "0x3FF is no region");
    }

    fn host_with(keys: &[(InstanceKey, Result<Vec<u8>, String>)]) -> PublicHost {
        let frames = keys.iter().cloned().collect::<HashMap<_, _>>();
        PublicHost::new(Arc::new(frames), Arc::new(Mutex::new(PublicRegistry::new(0xD1A0_0000_0000_0100))))
    }

    #[test]
    fn a_public_instance_picks_its_regions_list_from_the_clients_slice_set() {
        let steppes = InstanceKey::Bubble { destination: Destination::Earth, region: 56 };
        let array = InstanceKey::Bubble { destination: Destination::Earth, region: 0 };
        let mut p = host_with(&[(steppes, Ok(vec![0x56])), (array, Err("no encoder".into()))]);
        let now = Instant::now();
        let host = 0xD1A0_0000_0000_0002;
        // A mission host never picks a public list.
        p.on_join(&join(3, 0, 0xF1, 7), "act_cosmo_m1_dome_a", host, now);
        assert!(p.on_slice_report("act_cosmo_m1_dome_a", &udt21(None, Some(56)), host, None, now).is_none());
        // A public instance: an empty report, or one for a region with no list, picks nothing.
        p.on_join(&join(3, 0xD1A0_0000_0000_0001, 0, 7), "act_cosmo_m1_dome_a", host, now);
        assert!(p.on_slice_report("act_cosmo_m1_dome_a", &udt21(None, None), host, None, now).is_none());
        assert!(p.on_slice_report("act_cosmo_m1_dome_a", &udt21(Some(104), None), host, None, now).is_none());
        // The target names region 56: its frame, once, and the registry knows the instance.
        let (key, frame) = p.on_slice_report("act_cosmo_m1_dome_a", &udt21(Some(104), Some(56)), host, None, now).unwrap().unwrap();
        assert_eq!((key, frame), (steppes, vec![0x56]));
        assert!(p.on_slice_report("act_cosmo_m1_dome_a", &udt21(Some(56), Some(56)), host, None, now).is_none());
        assert_eq!(p.registry().lock().unwrap().get(host).unwrap().key, steppes);
        p.on_leave(now);
        assert_eq!(p.registry().lock().unwrap().get(host).unwrap().player_count(), 0);
        // Another world's destination does not match Earth's lists, and a list without a frame is named.
        let mut q = host_with(&[(array, Err("no encoder".into()))]);
        q.on_join(&join(3, 0xD1A0_0000_0000_0001, 0, 8), "act_cosmo_m1_dome_a", host + 1, now);
        assert!(q.on_slice_report("act_mars_m01_valley", &udt21(None, Some(0)), host + 1, None, now).is_none());
        assert!(q.on_slice_report("act_cosmo_m1_dome_a", &udt21(None, Some(0)), host + 1, None, now).unwrap().is_err());
    }
}
