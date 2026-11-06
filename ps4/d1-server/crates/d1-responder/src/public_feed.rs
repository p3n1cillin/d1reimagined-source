// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! ⭐ Stream O2: the activity host's public instances, seen from the responder's matchmaker.
//!
//! The public-instance registry (stream G, `d1_activity_host::public_registry`) lives in the
//! activity host, and the client's 42 bubble searches are answered here. With
//! `D1_AH_PUBLIC_INSTANCES_FILE` set for both servers, the host writes its instances to that file
//! and applies `reserve HOST SLOTS` lines from `<file>.reserve`. [`FilePublicInstances`] is
//! `d1_mp::matchmaking::PublicInstances` over that pair, so a slot-1 search runs
//! `place_in_public` (the registry's own emptiest-first rule) and holds the fireteam's slots in
//! the chosen instance before it arrives ([`on_bubble_search`]).
//!
//! **The 43 answer (stream O3).** Which advertisement belongs to which host instance is not in
//! the advertisement's bytes, but who sent it is: the client in a public instance advertises it
//! (the mission host advertised the instance 1.2-1.5 s after the join in every capture). The
//! host's snapshot names each instance's members by account (`accounts`), so a placed search is
//! answered with an advertisement whose owner is a member of the chosen instance, and with none
//! when the placement starts a new instance ([`on_bubble_search_by`];
//! `d1_mp::matchmaking::Directory::handle_placed`).
//!
//! **The destination (stream O3).** The bubble id is a region (`0x8001_0038` is region 56) and
//! carries no destination, so a region id two destinations share would match both. The host's
//! snapshot also names each mission host's player and its destination (`missions`), and a search
//! is placed only among the searcher's destination's instances. A searcher the host has not seen
//! on a mission host is placed among every destination's, as before.

use std::path::{Path, PathBuf};

use d1_mp::matchmaking::{place_in_public, PublicInstance, PublicInstances, PublicPlacement};

pub const KNOB: &str = "D1_AH_PUBLIC_INSTANCES_FILE";

/// The host's reservation file beside the snapshot (`public_registry::reservations_file_for`).
pub fn reservations_file_for(snapshot: &Path) -> PathBuf {
    let mut s = snapshot.as_os_str().to_owned();
    s.push(".reserve");
    PathBuf::from(s)
}

/// One instance of the host's snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostInstance {
    pub region: Option<u32>,
    /// The destination's name as the host writes it (`Earth`), None for a social space.
    pub destination: Option<String>,
    /// Its members' accounts (each identity's `+0x10`).
    pub accounts: Vec<u64>,
    pub instance: PublicInstance,
}

/// The host's snapshot, with reservations written back to it.
#[derive(Debug, Clone)]
pub struct FilePublicInstances {
    pub instances: Vec<HostInstance>,
    /// Each mission host's player (account) and its destination, from the snapshot's `missions`.
    pub missions: Vec<(u64, String)>,
    /// Place only among this destination's instances (the searcher's), when set.
    pub destination: Option<String>,
    reserve: PathBuf,
}

impl FilePublicInstances {
    /// Read the snapshot at `path` (an unreadable or missing file is no instances).
    pub fn read(path: &Path) -> Self {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        Self { instances: parse_snapshot(&text), missions: parse_missions(&text), destination: None, reserve: reservations_file_for(path) }
    }

    /// The destination the host last saw `account` on a mission host in.
    pub fn destination_of(&self, account: u64) -> Option<&str> {
        self.missions.iter().rev().find(|(a, _)| *a == account).map(|(_, d)| d.as_str())
    }

    /// The members of instance `host`.
    pub fn accounts_of(&self, host: u64) -> Vec<u64> {
        self.instances.iter().find(|i| i.instance.host == host).map(|i| i.accounts.clone()).unwrap_or_default()
    }

    pub fn from_env() -> Option<Self> {
        std::env::var_os(KNOB).filter(|p| !p.is_empty()).map(|p| Self::read(Path::new(&p)))
    }
}

fn hex_u64(v: &serde_json::Value) -> Option<u64> {
    u64::from_str_radix(v.as_str()?.trim_start_matches("0x"), 16).ok()
}

/// The snapshot's `missions`: `[{"account": "0x..", "destination": "Earth"}]`.
pub fn parse_missions(text: &str) -> Vec<(u64, String)> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { return Vec::new() };
    v.get("missions")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|r| Some((hex_u64(r.get("account")?)?, r.get("destination")?.as_str()?.to_string())))
        .collect()
}

/// The instances in the host's `snapshot_json` text.
pub fn parse_snapshot(text: &str) -> Vec<HostInstance> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { return Vec::new() };
    let num = |r: &serde_json::Value, k: &str| r.get(k).and_then(serde_json::Value::as_u64);
    v.get("instances")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let host = u64::from_str_radix(r.get("host")?.as_str()?.trim_start_matches("0x"), 16).ok()?;
            Some(HostInstance {
                region: num(r, "region").map(|x| x as u32),
                destination: r.get("destination").and_then(serde_json::Value::as_str).map(str::to_string),
                accounts: r.get("accounts").and_then(serde_json::Value::as_array).into_iter().flatten().filter_map(hex_u64).collect(),
                instance: PublicInstance {
                    host,
                    players: num(r, "players")? as usize,
                    reserved: num(r, "reserved").unwrap_or(0) as usize,
                    capacity: num(r, "capacity")? as usize,
                    started_ms: num(r, "started_ms").unwrap_or(0),
                },
            })
        })
        .collect()
}

impl PublicInstances for FilePublicInstances {
    /// The instances of the region the bubble id names (its low 10 bits: Restoration's region 56
    /// was searched as `0x8001_0038`), on the searcher's destination when it is known
    /// ([`Self::destination`]): the bubble id itself carries none.
    fn instances(&self, bubble: u32) -> Vec<PublicInstance> {
        let region = bubble & 0x3FF;
        self.instances
            .iter()
            .filter(|i| i.region == Some(region))
            .filter(|i| self.destination.as_ref().is_none_or(|d| i.destination.as_ref() == Some(d)))
            .map(|i| i.instance)
            .collect()
    }

    fn reserve(&mut self, host: u64, slots: usize) -> bool {
        let Some(i) = self.instances.iter_mut().find(|i| i.instance.host == host) else { return false };
        if i.instance.room() < slots {
            return false;
        }
        let line = format!("reserve {host:#x} {slots}\n");
        let wrote = std::fs::OpenOptions::new().create(true).append(true).open(&self.reserve).and_then(|mut f| {
            use std::io::Write;
            f.write_all(line.as_bytes())
        });
        if let Err(e) = wrote {
            tracing::warn!("⚠ public placement: {}: {e}", self.reserve.display());
            return false;
        }
        i.instance.reserved += slots;
        true
    }
}

/// A slot-1 search for `bubble` by a fireteam needing `slots`: place it in the host's public
/// instances. `None` when the feed is off.
#[allow(dead_code)] // the account-less form; the responder places through `on_bubble_search_by`
pub fn on_bubble_search(bubble: u32, slots: usize) -> Option<PublicPlacement> {
    let mut feed = FilePublicInstances::from_env()?;
    Some(place_feed(&mut feed, bubble, slots, None).0)
}

/// Stream O3: [`on_bubble_search`] for the searcher `account`, among its destination's instances.
/// Returns the accounts whose advertisements answer the search: the placed instance's members (an
/// empty list when a new instance starts). `None` when the feed is off.
pub fn on_bubble_search_by(bubble: u32, slots: usize, account: Option<u64>) -> Option<Vec<u64>> {
    let mut feed = FilePublicInstances::from_env()?;
    Some(place_feed(&mut feed, bubble, slots, account).1)
}

/// The placement and the accounts that answer it, on `feed`.
pub fn place_feed(feed: &mut FilePublicInstances, bubble: u32, slots: usize, account: Option<u64>) -> (PublicPlacement, Vec<u64>) {
    feed.destination = account.and_then(|a| feed.destination_of(a)).map(str::to_string);
    let placement = place_in_public(bubble, slots, feed);
    let owners = match placement {
        PublicPlacement::Join { host } => feed.accounts_of(host),
        PublicPlacement::StartNew => Vec::new(),
    };
    tracing::warn!(
        "      ⭐ public placement for bubble {bubble:#010x} ({slots} slot(s){}): {placement:?}, answered with {} member(s)' advertisement(s)",
        feed.destination.as_ref().map(|d| format!(", {d}")).unwrap_or_default(),
        owners.len()
    );
    (placement, owners)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNAPSHOT: &str = r#"{"instances": [
        {"host": "0xd1a0000000000100", "region": 56, "destination": "Earth", "social": null, "players": 4, "reserved": 0, "capacity": 6, "started_ms": 0},
        {"host": "0xd1a0000000000101", "region": 56, "destination": "Earth", "social": null, "players": 1, "reserved": 0, "capacity": 6, "started_ms": 5},
        {"host": "0xd1a0000000000102", "region": null, "destination": null, "social": "Tower", "players": 2, "reserved": 0, "capacity": 16, "started_ms": 9}
    ]}"#;

    /// Region 56 on two destinations, members by account, and where each searcher's mission is.
    const SHARED_REGION: &str = r#"{"instances": [
        {"host": "0xd1a0000000000200", "region": 56, "destination": "Earth", "accounts": ["0xd101000000001001"], "players": 1, "reserved": 0, "capacity": 6, "started_ms": 0},
        {"host": "0xd1a0000000000201", "region": 56, "destination": "Moon", "accounts": ["0xd101000000002001", "0xd101000000003001"], "players": 2, "reserved": 0, "capacity": 6, "started_ms": 1}
    ], "missions": [
        {"account": "0xd101000000004001", "destination": "Moon"},
        {"account": "0xd101000000005001", "destination": "Earth"}
    ]}"#;

    #[test]
    fn a_search_is_placed_on_its_own_destination_and_answered_by_the_instances_members() {
        let dir = std::env::temp_dir().join(format!("d1-resp-feed-dest-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("public-instances.json");
        std::fs::write(&path, SHARED_REGION).unwrap();
        let _ = std::fs::remove_file(reservations_file_for(&path));
        let mut feed = FilePublicInstances::read(&path);
        assert_eq!(feed.destination_of(0xD101_0000_0000_4001), Some("Moon"));
        // On the Moon the fuller Moon instance is the only one; Earth's emptier one is not offered.
        let (p, owners) = place_feed(&mut feed, 0x8001_0038, 1, Some(0xD101_0000_0000_4001));
        assert_eq!(p, PublicPlacement::Join { host: 0xD1A0_0000_0000_0201 });
        assert_eq!(owners, vec![0xD101_0000_0000_2001, 0xD101_0000_0000_3001]);
        let (p, owners) = place_feed(&mut feed, 0x8001_0038, 1, Some(0xD101_0000_0000_5001));
        assert_eq!(p, PublicPlacement::Join { host: 0xD1A0_0000_0000_0200 });
        assert_eq!(owners, vec![0xD101_0000_0000_1001]);
        // A searcher the host has not seen: every destination's, the emptiest (Earth's) first.
        let (p, _) = place_feed(&mut feed, 0x8001_0038, 1, None);
        assert_eq!(p, PublicPlacement::Join { host: 0xD1A0_0000_0000_0200 });
        // No room on the Moon for 5: a new instance, so no advertisement answers.
        let (p, owners) = place_feed(&mut feed, 0x8001_0038, 5, Some(0xD101_0000_0000_4001));
        assert_eq!((p, owners), (PublicPlacement::StartNew, Vec::new()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bubble_search_places_into_the_hosts_emptiest_instance_and_holds_its_slots() {
        let dir = std::env::temp_dir().join(format!("d1-resp-feed-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("public-instances.json");
        std::fs::write(&path, SNAPSHOT).unwrap();
        let _ = std::fs::remove_file(reservations_file_for(&path));
        let mut feed = FilePublicInstances::read(&path);
        assert_eq!(feed.instances.len(), 3);
        assert_eq!(feed.instances(0x8001_0038).len(), 2, "region 56's two instances; the Tower has no region");
        let p = place_in_public(0x8001_0038, 3, &mut feed);
        assert_eq!(p, PublicPlacement::Join { host: 0xD1A0_0000_0000_0101 }, "the emptiest first");
        assert_eq!(std::fs::read_to_string(reservations_file_for(&path)).unwrap(), "reserve 0xd1a0000000000101 3\n");
        // That instance now holds 1 + 3; a fireteam of 3 fits neither instance.
        assert_eq!(place_in_public(0x8001_0038, 3, &mut feed), PublicPlacement::StartNew);
        assert_eq!(place_in_public(0x8001_0030, 1, &mut feed), PublicPlacement::StartNew, "no instance of region 48");
        assert!(parse_snapshot("not json").is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
