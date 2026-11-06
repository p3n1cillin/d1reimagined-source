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
//! What it does not do yet: the 43 answer still lists the advertisements the clients sent (the
//! directory's own ranking); which advertisement belongs to which host instance is not known from
//! the advertisement bytes. The placement holds the slots and is logged.

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
    pub instance: PublicInstance,
}

/// The host's snapshot, with reservations written back to it.
#[derive(Debug, Clone)]
pub struct FilePublicInstances {
    pub instances: Vec<HostInstance>,
    reserve: PathBuf,
}

impl FilePublicInstances {
    /// Read the snapshot at `path` (an unreadable or missing file is no instances).
    pub fn read(path: &Path) -> Self {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        Self { instances: parse_snapshot(&text), reserve: reservations_file_for(path) }
    }

    pub fn from_env() -> Option<Self> {
        std::env::var_os(KNOB).filter(|p| !p.is_empty()).map(|p| Self::read(Path::new(&p)))
    }
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
    /// was searched as `0x8001_0038`). ⚠ The destination is not in the bubble id, so a region id
    /// two destinations share matches both.
    fn instances(&self, bubble: u32) -> Vec<PublicInstance> {
        let region = bubble & 0x3FF;
        self.instances.iter().filter(|i| i.region == Some(region)).map(|i| i.instance).collect()
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
pub fn on_bubble_search(bubble: u32, slots: usize) -> Option<PublicPlacement> {
    let mut feed = FilePublicInstances::from_env()?;
    let placement = place_in_public(bubble, slots, &mut feed);
    tracing::warn!("      ⭐ public placement for bubble {bubble:#010x} ({slots} slot(s)): {placement:?}");
    Some(placement)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNAPSHOT: &str = r#"{"instances": [
        {"host": "0xd1a0000000000100", "region": 56, "destination": "Earth", "social": null, "players": 4, "reserved": 0, "capacity": 6, "started_ms": 0},
        {"host": "0xd1a0000000000101", "region": 56, "destination": "Earth", "social": null, "players": 1, "reserved": 0, "capacity": 6, "started_ms": 5},
        {"host": "0xd1a0000000000102", "region": null, "destination": null, "social": "Tower", "players": 2, "reserved": 0, "capacity": 16, "started_ms": 9}
    ]}"#;

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
