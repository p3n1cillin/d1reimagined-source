// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! ⭐ Stream O3, the multi-client scenario (host side): two fake clients with `D1_MP=1`'s
//! per-connection identities (`0xD101_0000_nnnn_n001` accounts, `...101` characters), first in one
//! strike, then in one public instance, through `ActivityInstance`'s hooks as the listener sets
//! them up under `D1_AH_SHARED_INSTANCES=1`.
//!
//! * **The strike:** both connections name one host, so they share one instance and one script
//!   bridge (integration review, finding 9). The bridge pays each member once at the results
//!   (`script_bridge`'s `a_shared_instance_runs_one_script_for_both_members_and_pays_each_once`
//!   drives a whole run through two handles); here, the two connections hold the same bridge.
//! * **The public instance:** a join naming a mission host runs no mission script and no mission
//!   framework, so it can never grant the mission a second time (finding 10).
//! * **The Tower:** a social-space join registers under the shared instance's host id, so the
//!   second player lands in the first one's Tower (finding 11).
//! * **The registry's snapshot** names each instance's members by account and each mission
//!   host's destination, which the responder needs to answer a bubble search with the placed
//!   instance's advertisement on the searcher's own destination.
//!
//! The responder's half (each player's grant into their own record, once, by full identity) is
//! `d1-responder`'s `two_players_are_each_paid_once_into_their_own_record`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use d1_activity_host::group::{GroupLink, InstanceGroups};
use d1_activity_host::open_world::{Destination, SocialSpace};
use d1_activity_host::public_host::{PublicHost, JOIN_FIRETEAM, JOIN_IDENTITY, JOIN_MISSION_HOST, JOIN_SESSION_TYPE};
use d1_activity_host::public_registry::{InstanceKey, PublicRegistry};
use d1_activity_host::rewards::RecordingRewards;
use d1_activity_host::script_bridge::ScriptConfig;
use d1_activity_host::shared_scripts::SharedScripts;
use d1_activity_host::{ActivityInstance, ClientMessage};

const STRIKE_HOST: u64 = 0xD1A0_0000_0000_0042;
const TOWER_HOST: u64 = 0xD1A0_0000_0000_0077;
const PUBLIC_HOST: u64 = 0xD1A0_0000_0000_1000;
const MISSION_HOST: u64 = 0xD1A0_0000_0000_0001;

/// Player n's identity as `D1_MP=1` allocates it: `{machine, player, account, character, fireteam}`.
fn identity(n: u64) -> [u8; 40] {
    let account = 0xD101_0000_0000_0001 | (n << 12);
    let mut b = [0u8; 40];
    b[0..8].copy_from_slice(&(0x5150_0000 + n).to_le_bytes());
    b[8..16].copy_from_slice(&(0x9150_0000 + n).to_le_bytes());
    b[16..24].copy_from_slice(&account.to_le_bytes());
    b[24..32].copy_from_slice(&(account | 0x100).to_le_bytes());
    b
}

/// A join request naming `host` (`+0x08`), with player n's identity, session type `kind` and the
/// mission host it names (`+0xC8`, 0 for a mission-type join).
fn join(host: u64, n: u64, kind: u8, mission_host: u64) -> ClientMessage {
    let mut img = vec![0u8; 0x100];
    img[..4].copy_from_slice(&(n as u32 + 1).to_le_bytes());
    img[8..16].copy_from_slice(&host.to_le_bytes());
    img[JOIN_IDENTITY..JOIN_IDENTITY + 40].copy_from_slice(&identity(n));
    img[JOIN_SESSION_TYPE] = kind;
    img[JOIN_MISSION_HOST..JOIN_MISSION_HOST + 8].copy_from_slice(&mission_host.to_le_bytes());
    img[JOIN_FIRETEAM..JOIN_FIRETEAM + 8].copy_from_slice(&(0xF17E_0000_0000_0000 + n).to_le_bytes());
    ClientMessage::from_struct_image(3, &img).unwrap()
}

/// A bridge configuration (no sidecar starts: nothing here polls the udt 5 stream).
fn script_config() -> Arc<ScriptConfig> {
    Arc::new(ScriptConfig {
        sidecar: vec!["no-sidecar".into()],
        activities: HashMap::new(),
        gen: 0,
        tail: None,
        incidents: Default::default(),
        incidents_path: None,
        mission_phase: true,
        send_unlisted: false,
        delta_init: false,
        scoreboard_columns: Vec::new(),
        rewards: Arc::new(Mutex::new(RecordingRewards::default())),
        codec: ScriptConfig::host_codec(),
        active_bundles: true,
        start_delay: std::time::Duration::ZERO,
        arrival_cushion: std::time::Duration::ZERO,
        list_retry: None,
    })
}

/// A connection as the listener builds it with shared instances on.
fn connection(own_id: u64, n: u64, groups: &d1_activity_host::group::SharedGroups, scripts: &Arc<SharedScripts>, now: Instant) -> ActivityInstance {
    let mut i = ActivityInstance::new(own_id, format!("d1ah-{own_id:016x}"), now);
    i.set_group(GroupLink::new(groups.clone(), n));
    i.set_shared_scripts(scripts.clone());
    i.set_script_bridge(script_config());
    i.on_connect(now);
    i
}

#[test]
fn two_players_share_one_strike_script_and_a_public_instance_runs_none() {
    let now = Instant::now();
    let groups = InstanceGroups::shared();
    let scripts = SharedScripts::new();

    // The strike: two connections (own ids 0x..42 and 0x..43) name the same host.
    let mut a = connection(STRIKE_HOST, 0, &groups, &scripts, now);
    let mut b = connection(STRIKE_HOST + 1, 1, &groups, &scripts, now);
    assert!(!a.script_handle().unwrap().same_bridge(b.script_handle().unwrap()), "before the join, each its own");
    a.on_client_message(&join(STRIKE_HOST, 1, 3, 0), now);
    b.on_client_message(&join(STRIKE_HOST, 2, 3, 0), now);
    assert_eq!(b.ah_id(), STRIKE_HOST, "B presents the shared instance's host");
    let (ha, hb) = (a.script_handle().unwrap(), b.script_handle().unwrap());
    assert!(ha.same_bridge(hb), "one script bridge for the shared instance");
    assert_eq!(ha.members(), 2);
    assert_eq!(scripts.len(), 1);

    // The public instance: two connections join one bubble host, each naming its mission host.
    let frames: HashMap<_, _> = HashMap::new();
    let registry = Arc::new(Mutex::new(PublicRegistry::new(PUBLIC_HOST)));
    let public = PublicHost::new(Arc::new(frames), registry.clone());
    let mut pa = connection(PUBLIC_HOST, 2, &groups, &scripts, now);
    let mut pb = connection(PUBLIC_HOST + 1, 3, &groups, &scripts, now);
    pa.set_activity_name("act_cosmo_m1_dome_a");
    pb.set_activity_name("act_cosmo_m1_dome_a");
    pa.set_public_host(public.clone());
    pb.set_public_host(public.clone());
    pa.on_client_message(&join(PUBLIC_HOST, 1, 3, MISSION_HOST), now);
    pb.on_client_message(&join(PUBLIC_HOST, 2, 2, MISSION_HOST + 1), now);
    for p in [&pa, &pb] {
        assert!(p.script_handle().is_none(), "a public instance runs no mission script (finding 10)");
        assert!(p.run_driver().is_none(), "and no mission framework, so it grants nothing for the mission");
    }
    assert_eq!(pb.ah_id(), PUBLIC_HOST);
    // The strike's bridge is still the strike's, with its two members.
    assert_eq!(a.script_handle().unwrap().members(), 2);
    drop((pa, pb));
    assert_eq!(scripts.len(), 1, "the public connections' bridges went with them");
    drop(a);
    assert_eq!(b.script_handle().unwrap().members(), 1);
}

#[test]
fn a_tower_join_registers_under_the_shared_instance_host() {
    let now = Instant::now();
    let groups = InstanceGroups::shared();
    let scripts = SharedScripts::new();
    let registry = Arc::new(Mutex::new(PublicRegistry::new(PUBLIC_HOST)));
    let public = PublicHost::new(Arc::new(HashMap::new()), registry.clone());
    let tower = |own: u64, n: u64| {
        let mut i = connection(own, n, &groups, &scripts, now);
        i.set_activity_name("city_tower_default1");
        i.set_public_host(public.clone());
        i
    };
    // Player 1 is in the Tower on host TOWER_HOST; player 2's connection (its own id one higher)
    // joins that host (session type 2), as a fireteam join or a placement would send it.
    let mut a = tower(TOWER_HOST, 0);
    let mut b = tower(TOWER_HOST + 1, 1);
    a.on_client_message(&join(TOWER_HOST, 1, 3, 0), now);
    b.on_client_message(&join(TOWER_HOST, 2, 2, 0), now);
    let r = registry.lock().unwrap();
    let towers: Vec<_> = r.instances().filter(|i| i.key == InstanceKey::Social(SocialSpace::Tower)).collect();
    assert_eq!(towers.len(), 1, "one Tower, not a second instance under B's own id (finding 11)");
    assert_eq!(towers[0].host_id, TOWER_HOST);
    assert_eq!(towers[0].player_count(), 2);
    assert!(r.get(TOWER_HOST + 1).is_none());
}

#[test]
fn the_snapshot_names_members_by_account_and_each_missions_destination() {
    let now = Instant::now();
    let registry = Arc::new(Mutex::new(PublicRegistry::new(PUBLIC_HOST)));
    let public = PublicHost::new(Arc::new(HashMap::new()), registry.clone());
    // Player 1's mission host is on the Moon; player 2 is in the Tower.
    let mut m = ActivityInstance::new(MISSION_HOST, "d1ah-m", now);
    m.set_activity_name("act_moon_m1_cave_a");
    m.set_public_host(public.clone());
    m.on_connect(now);
    m.on_client_message(&join(MISSION_HOST, 1, 3, 0), now);
    let mut t = ActivityInstance::new(TOWER_HOST, "d1ah-t", now);
    t.set_activity_name("city_tower_default1");
    t.set_public_host(public.clone());
    t.on_connect(now);
    t.on_client_message(&join(TOWER_HOST, 2, 3, 0), now);
    let r = registry.lock().unwrap();
    let account = |n: u64| 0xD101_0000_0000_0001u64 | (n << 12);
    assert_eq!(r.mission_destination(account(1)), Some(Destination::Moon), "the mission host's destination");
    assert_eq!(r.mission_destination(account(2)), None, "the Tower is no destination");
    let snap = r.snapshot_json();
    let tower = snap["instances"].as_array().unwrap().iter().find(|i| i["social"] == "Tower").unwrap();
    assert_eq!(tower["accounts"], serde_json::json!([format!("{:#018x}", account(2))]));
    assert_eq!(snap["missions"], serde_json::json!([{"account": format!("{:#018x}", account(1)), "destination": "Moon"}]));
}
