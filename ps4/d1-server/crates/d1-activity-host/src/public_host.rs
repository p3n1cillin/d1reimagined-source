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

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use d1_oracle::sensor::SensorClientRef;

use crate::open_world::{Destination, SocialSpace, PUBLIC_PLAYERS_GROUP};
use crate::public_registry::{InstanceKey, JoinKind, PublicRegistry};

/// `join_request +0x38`: the session type (3 starts an instance, 2 joins one).
pub const JOIN_SESSION_TYPE: usize = 0x38;
/// `join_request +0xC8`: on a public instance, the mission host's id; 0 on a mission host.
pub const JOIN_MISSION_HOST: usize = 0xC8;
/// `join_request +0xD8`: the fireteam id (0 on a public instance).
pub const JOIN_FIRETEAM: usize = 0xD8;
/// `join_request +0x10`: the client's 40-byte identity. Its first 8 bytes key the character here.
pub const JOIN_IDENTITY: usize = 0x10;

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

/// The character key for the registry: the first 8 bytes of the join's identity.
pub fn character_of_join(image: &[u8]) -> u64 {
    u64_at(image, JOIN_IDENTITY)
}

/// The two slice-set records of a udt 21 image: `(CURRENT, TARGET)` region ids, each present only
/// when its name hash is set (the update is a delta).
pub fn slice_regions(image: &[u8]) -> (Option<u32>, Option<u32>) {
    let rec = |base: usize| {
        let index = image.get(base..base + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()) & 0x3FF)?;
        let hash = image.get(base + 4..base + 8).map(|b| u32::from_le_bytes(b.try_into().unwrap()))?;
        (!matches!(hash, 0 | 0x811C_9DC5) && index != 0x3FF).then_some(index)
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

/// The public-host state one activity-host connection carries.
#[derive(Clone)]
pub struct PublicHost {
    /// Every public region's frame, built once at setup (an `Err` names why one could not be).
    frames: Arc<HashMap<InstanceKey, Result<Vec<u8>, String>>>,
    /// Shared by every connection (and, later, by matchmaking).
    registry: Arc<Mutex<PublicRegistry>>,
    role: Option<HostRole>,
    character: u64,
    selected: Option<InstanceKey>,
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
        Self { frames, registry, role: None, character: 0, selected: None }
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
    pub fn on_join(&mut self, join_request: &[u8]) -> HostRole {
        let role = role_of_join(join_request);
        self.role = Some(role);
        self.character = character_of_join(join_request);
        self.selected = None;
        if let HostRole::PublicInstance { kind, mission_host } = role {
            tracing::info!(
                "⭐ public instance: the join names mission host {mission_host:#018x} ({kind:?}); serving no region \
                 records and no mission steps, and waiting for the client's slice set to pick the region's list"
            );
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
        now: Instant,
    ) -> Option<Result<(InstanceKey, Vec<u8>), String>> {
        if !self.is_public_instance() || self.selected.is_some() {
            return None;
        }
        let destination = Destination::from_world_name(world)?;
        let (current, target) = slice_regions(udt21);
        let key = [target, current].into_iter().flatten().map(|region| InstanceKey::Bubble { destination, region })
            .find(|k| self.frames.contains_key(k))?;
        self.selected = Some(key);
        let joined = self.registry.lock().map(|mut r| r.attach(host_id, key, self.character, now));
        tracing::info!("⭐ public instance {host_id:#018x} is {key:?} ({joined:?})");
        Some(match &self.frames[&key] {
            Ok(frame) => Ok((key, frame.clone())),
            Err(e) => Err(format!("{key:?}: {e}")),
        })
    }

    /// The client left this instance (peer leave or lost connection).
    pub fn on_leave(&mut self, now: Instant) {
        if self.selected.take().is_some() {
            if let Ok(mut r) = self.registry.lock() {
                r.leave(self.character, now);
            }
        }
    }
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

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/56578/public")
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
        p.on_join(&join(3, 0, 0xF1, 7));
        assert!(p.on_slice_report("act_cosmo_m1_dome_a", &udt21(None, Some(56)), host, now).is_none());
        // A public instance: an empty report, or one for a region with no list, picks nothing.
        p.on_join(&join(3, 0xD1A0_0000_0000_0001, 0, 7));
        assert!(p.on_slice_report("act_cosmo_m1_dome_a", &udt21(None, None), host, now).is_none());
        assert!(p.on_slice_report("act_cosmo_m1_dome_a", &udt21(Some(104), None), host, now).is_none());
        // The target names region 56: its frame, once, and the registry knows the instance.
        let (key, frame) = p.on_slice_report("act_cosmo_m1_dome_a", &udt21(Some(104), Some(56)), host, now).unwrap().unwrap();
        assert_eq!((key, frame), (steppes, vec![0x56]));
        assert!(p.on_slice_report("act_cosmo_m1_dome_a", &udt21(Some(56), Some(56)), host, now).is_none());
        assert_eq!(p.registry().lock().unwrap().get(host).unwrap().key, steppes);
        p.on_leave(now);
        assert_eq!(p.registry().lock().unwrap().get(host).unwrap().player_count(), 0);
        // Another world's destination does not match Earth's lists, and a list without a frame is named.
        let mut q = host_with(&[(array, Err("no encoder".into()))]);
        q.on_join(&join(3, 0xD1A0_0000_0000_0001, 0, 8));
        assert!(q.on_slice_report("act_mars_m01_valley", &udt21(None, Some(0)), host + 1, now).is_none());
        assert!(q.on_slice_report("act_cosmo_m1_dome_a", &udt21(None, Some(0)), host + 1, now).unwrap().is_err());
    }
}
