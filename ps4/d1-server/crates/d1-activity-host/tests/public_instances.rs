// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! Stream G, end to end in one process: two fake clients, each with its own mission host, meet in
//! one public instance of the Cosmodrome's region 56, as the live Restoration capture's client met
//! another player's instance (`notes/ps4-56578-public-instances-2026-11-05.md`). The messages are
//! the decoded struct images the host's state machine sees; their values are synthetic, their
//! layouts the live ones.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use d1_activity_host::messages::ReplicateMembership as M;
use d1_activity_host::open_world::Destination;
use d1_activity_host::public_host::{self, PublicHost, JOIN_FIRETEAM, JOIN_IDENTITY, JOIN_MISSION_HOST, JOIN_SESSION_TYPE};
use d1_activity_host::public_registry::{Arrival, InstanceKey, JoinKind, PublicRegistry};
use d1_activity_host::{ActivityInstance, AhMessage, ClientMessage, InstanceState};

const WORLD: &str = "act_cosmo_m1_dome_a";
const STEPPES: InstanceKey = InstanceKey::Bubble { destination: Destination::Earth, region: 56 };
const FRAME_56: &[u8] = &[0x56, 0x00, 0x01];

fn join(kind: u8, mission_host: u64, fireteam: u64, character: u64) -> ClientMessage {
    let mut img = vec![0u8; 0x100];
    img[..4].copy_from_slice(&(character as u32).to_le_bytes()); // the nonce
    img[JOIN_IDENTITY..JOIN_IDENTITY + 8].copy_from_slice(&character.to_le_bytes());
    img[JOIN_SESSION_TYPE] = kind;
    img[JOIN_MISSION_HOST..JOIN_MISSION_HOST + 8].copy_from_slice(&mission_host.to_le_bytes());
    img[JOIN_FIRETEAM..JOIN_FIRETEAM + 8].copy_from_slice(&fireteam.to_le_bytes());
    ClientMessage::from_struct_image(3, &img).unwrap()
}

/// udt 21 with the TARGET slice-set record naming `region`, and, once the client has joined the
/// region's instance, that instance's session (record +0x14) and host id (+0x98).
fn slice_report(region: u32, token: u8, instance: Option<u64>) -> ClientMessage {
    const TARGET: usize = 0x158;
    let mut img = vec![0u8; 0x220];
    img[TARGET..TARGET + 4].copy_from_slice(&region.to_le_bytes());
    img[TARGET + 4..TARGET + 8].copy_from_slice(&0x2EA8_FB98u32.to_le_bytes());
    img[0x1F8] = token;
    if let Some(host) = instance {
        img[TARGET + 0x14] = 0x4E;
        img[TARGET + 0x18..TARGET + 0x58].fill(0xAB);
        img[TARGET + 0x98..TARGET + 0xA0].copy_from_slice(&host.to_le_bytes());
    }
    ClientMessage::ClientAuthoritativeDataUpdate { struct_image: img }
}

fn instance(id: u64, public: &PublicHost, now: Instant) -> ActivityInstance {
    let mut i = ActivityInstance::new(id, format!("d1ah-{id:016x}"), now);
    i.set_activity_name(WORLD);
    i.set_pah_region(56, 0, 1);
    i.set_host_heartbeat(Duration::from_millis(100));
    i.set_public_host(public.clone());
    i.on_connect(now);
    i
}

fn memberships(out: &[d1_activity_host::AhPush]) -> Vec<&M> {
    out.iter()
        .filter_map(|p| match &p.message {
            AhMessage::ReplicateMembership(m) => Some(m),
            _ => None,
        })
        .collect()
}

fn region_ids(m: &M) -> Vec<u32> {
    (0..M::PAH_REGION_CAPACITY)
        .map(|s| {
            let o = M::PAH_REGIONS + s * M::PAH_REGION_STRIDE;
            u32::from_le_bytes(m.image[o..o + 4].try_into().unwrap())
        })
        .collect()
}

fn frame_in(out: &[d1_activity_host::AhPush]) -> Option<Vec<u8>> {
    out.iter().find_map(|p| match &p.message {
        AhMessage::SensorAuthUpdate { payload } => Some(payload.clone()),
        _ => None,
    })
}

/// The sensor frame a message's own reply carried, or else the first one the heartbeat sends.
fn served_frame(i: &mut ActivityInstance, msg: &ClientMessage, now: Instant) -> Option<Vec<u8>> {
    let out = i.on_client_message(msg, now);
    frame_in(&out).or_else(|| first_sensor_frame(i, now))
}

/// Drive the heartbeat until the instance's sensor frame goes out, and return it.
fn first_sensor_frame(i: &mut ActivityInstance, mut now: Instant) -> Option<Vec<u8>> {
    for _ in 0..20 {
        now += Duration::from_millis(150);
        let _ = i.heartbeat_due(now);
        if let Some(p) = i.sensor_due(now) {
            if let AhMessage::SensorAuthUpdate { payload } = p.message {
                return Some(payload);
            }
        }
    }
    None
}

#[test]
fn two_clients_meet_in_one_public_instance() {
    let t0 = Instant::now();
    let frames: HashMap<_, _> = [(STEPPES, Ok(FRAME_56.to_vec()))].into_iter().collect();
    let registry = Arc::new(Mutex::new(PublicRegistry::new(0xD1A0_0000_0000_1000)));
    let public = PublicHost::new(Arc::new(frames), registry.clone());
    let (alice, bob) = (0xA11CE, 0xB0B);
    let (pah_a, pah_b) = (0xD1A0_0000_0000_0001, 0xD1A0_0000_0000_0002);

    // ── Each client's mission host (the PAH): a mission role, region records served.
    let mut mission_a = instance(pah_a, &public, t0);
    let out = mission_a.on_client_message(&join(3, 0, 0xF1, alice), t0);
    assert_eq!(mission_a.state(), InstanceState::Live);
    assert_eq!(region_ids(memberships(&out)[0])[0], 56, "the mission host tabulates region 56");
    let mut mission_b = instance(pah_b, &public, t0);
    mission_b.on_client_message(&join(3, 0, 0xF2, bob), t0);

    // ── Matchmaking places Alice first (she starts the instance) and Bob into it.
    let a = registry.lock().unwrap().place(STEPPES, Arrival { character: alice, fireteam: 0xF1, fireteam_size: 1 }, t0).unwrap();
    assert_eq!(a.kind, JoinKind::Starts);
    let b = registry.lock().unwrap().place(STEPPES, Arrival { character: bob, fireteam: 0xF2, fireteam_size: 1 }, t0).unwrap();
    assert_eq!((b.host_id, b.kind), (a.host_id, JoinKind::Joins));
    let host = a.host_id;

    // ── Alice's connection to the instance: its join names her mission host.
    let mut inst_a = instance(host, &public, t0);
    let out = inst_a.on_client_message(&join(JoinKind::Starts.session_type(), pah_a, 0, alice), t0);
    assert_eq!(inst_a.state(), InstanceState::Live);
    assert!(inst_a.public_host().unwrap().is_public_instance());
    assert!(region_ids(memberships(&out)[0]).iter().all(|&r| r == u32::MAX), "no region records on a public instance");
    // Nothing is served until she names the region; then region 56's list.
    assert!(first_sensor_frame(&mut inst_a, t0).is_none());
    assert_eq!(served_frame(&mut inst_a, &slice_report(56, 1, None), t0).as_deref(), Some(FRAME_56));

    // ── Alice reports the instance to her mission host, which advertises it (the PAH echo).
    mission_a.on_client_message(&slice_report(56, 1, Some(host)), t0);
    let m = memberships(&mission_a.membership_due(t0).into_iter().collect::<Vec<_>>())[0].clone();
    let rec = &m.image[M::PAH_REGIONS..M::PAH_REGIONS + M::PAH_REGION_STRIDE];
    assert_eq!(&rec[M::PAH_ADVERTISED_HOST..M::PAH_ADVERTISED_HOST + 8], &host.to_le_bytes());
    assert!(registry.lock().unwrap().mark_advertised(host));

    // ── Bob's connection to the same instance: a join (type 2), the same list, no records.
    let mut inst_b = instance(host, &public, t0);
    let out = inst_b.on_client_message(&join(JoinKind::Joins.session_type(), pah_b, 0, bob), t0);
    assert!(region_ids(memberships(&out)[0]).iter().all(|&r| r == u32::MAX));
    assert_eq!(served_frame(&mut inst_b, &slice_report(56, 1, None), t0).as_deref(), Some(FRAME_56));
    {
        let r = registry.lock().unwrap();
        let i = r.get(host).unwrap();
        assert_eq!(i.player_count(), 2);
        assert!(i.advertised);
    }

    // ── Bob leaves the bubble: the slot handover (25 -> 23, 26 -> 24) on the instance, then he
    // leaves the instance and the registry frees his slot.
    let mut abandon = vec![0u8; 0x608];
    abandon[4] = 0x0F;
    abandon[0x604] = 3;
    let out = inst_b.on_client_message(&ClientMessage::Other { user_defined_type: 25, struct_image: abandon.clone() }, t0);
    assert!(out.iter().any(|p| matches!(&p.message, AhMessage::Opaque { user_defined_type: 23, bytes } if bytes[4] == 0x0F)));
    let mut purge = vec![0u8; 0x604];
    purge[0] = 4;
    purge[4] = 0x0F;
    let out = inst_b.on_client_message(&ClientMessage::Other { user_defined_type: 26, struct_image: purge }, t0);
    assert!(out.iter().any(|p| matches!(&p.message, AhMessage::Opaque { user_defined_type: 24, .. })));
    inst_b.on_client_message(&ClientMessage::PeerLeaveRequest { struct_image: vec![0u8; 20] }, t0);
    assert_eq!(registry.lock().unwrap().get(host).unwrap().player_count(), 1);

    // ── The mission hosts never served a public list, and kept their region records.
    assert!(!mission_b.public_host().unwrap().is_public_instance());
    assert_eq!(public_host::role_of_join(&[0u8; 0x100]).is_public_instance(), false);
}

/// The open world of one instance runs once, whichever connection ticks it: the bundles start
/// through the bridge, the public event comes after its quiet spell and rewards everyone who
/// engaged, a chest opens once per character, and a social space registers as a shared instance.
#[test]
fn the_open_world_runs_once_per_instance() {
    use d1_activity_host::open_world::{ActivityOutcome, EventTier, RecordingRewards, SocialSpace};
    use d1_activity_host::public_events::{BridgeCall, EventPhase, EventResult, RecordingBridge};
    use d1_activity_host::public_host::{load_public_table, OpenWorldServices, PLACEHOLDER_LOOT_DELAY};

    let t0 = Instant::now();
    let table = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/56578/public/public-table.txt");
    let lists = load_public_table(&table).unwrap();
    let frames: HashMap<_, _> = lists.iter().map(|l| (l.key, Ok(vec![0x48]))).collect();
    let bridge = Arc::new(Mutex::new(RecordingBridge::default()));
    let rewards = Arc::new(Mutex::new(RecordingRewards::default()));
    let registry = Arc::new(Mutex::new(PublicRegistry::new(0xD1A0_0000_0000_1000)));
    let public = PublicHost::new(Arc::new(frames), registry.clone())
        .with_open_world(&lists, OpenWorldServices { bridge: bridge.clone(), rewards: rewards.clone() });
    let host = 0xD1A0_0000_0000_0010;
    let (alice, bob) = (0xA11CE, 0xB0B);

    // Two connections to one instance of the Cosmodrome's region 48.
    let mut inst_a = instance(host, &public, t0);
    inst_a.on_client_message(&join(3, 0xD1A0_0000_0000_0001, 0, alice), t0);
    inst_a.on_client_message(&slice_report(48, 1, None), t0);
    let mut inst_b = instance(host, &public, t0);
    inst_b.on_client_message(&join(2, 0xD1A0_0000_0000_0002, 0, bob), t0);
    inst_b.on_client_message(&slice_report(48, 1, None), t0);
    let starts = |b: &RecordingBridge| b.calls.iter().filter(|c| matches!(c, BridgeCall::StartBundles { .. })).count();
    assert_eq!(starts(&bridge.lock().unwrap()), 1, "the bundles start once per instance");
    assert_eq!(
        bridge.lock().unwrap().calls[0],
        BridgeCall::StartBundles { host, bundles: vec![0xA2F6_6E1A, 0x358E_BAE1, 0xBDF9_E3C6, 0x00B0_2852] }
    );

    // The region's event bundle rotates; it starts once, though both connections tick.
    let p = inst_a.public_host().unwrap();
    let (events, loot) = p.with_world(|w| (w.events.len(), w.loot.visible(alice, t0 + PLACEHOLDER_LOOT_DELAY).len())).unwrap();
    assert_eq!(events, 1);
    assert_eq!(loot, 15, "8 chests and 7 Thorium nodes");
    let EventPhase::Quiet { until } = p.with_world(|w| w.events[0].phase()).unwrap() else { panic!("quiet first") };
    inst_a.sensor_due(until);
    inst_b.sensor_due(until);
    let events_started = bridge.lock().unwrap().calls.iter().filter(|c| matches!(c, BridgeCall::StartEvent { .. })).count();
    assert_eq!(events_started, 1);

    // Both engage; the script ends it at gold; both are rewarded.
    inst_a.public_host().unwrap().report_engagement(0x358E_BAE1, alice);
    inst_b.public_host().unwrap().report_engagement(0x358E_BAE1, bob);
    inst_a.public_host().unwrap().report_event_finished(0x358E_BAE1, EventResult::Completed(EventTier::Gold), until);
    let granted: Vec<u64> = rewards.lock().unwrap().grants.iter().map(|(c, _, _)| *c).collect();
    assert_eq!(granted, vec![bob, alice]);

    // A chest: once per character, after it shows.
    let chest = |i: &ActivityInstance, who, at| i.public_host().unwrap().open_loot(who, 0x00B0_2852, 0, at);
    assert!(chest(&inst_a, alice, t0).is_err());
    let later = t0 + PLACEHOLDER_LOOT_DELAY;
    chest(&inst_a, alice, later).unwrap();
    assert!(chest(&inst_b, alice, later).is_err(), "the instance is shared, so is what she opened");
    chest(&inst_b, bob, later).unwrap();
    assert!(matches!(rewards.lock().unwrap().grants.last(), Some((c, _, ActivityOutcome::Chest { .. })) if *c == bob));

    // A social space: a mission-type join (type 2 into the Tower) is its shared instance.
    let mut tower = ActivityInstance::new(host + 1, "d1ah-tower", t0);
    tower.set_activity_name("city_tower_default1");
    tower.set_public_host(public.clone());
    tower.on_connect(t0);
    tower.on_client_message(&join(2, 0, 0xF1, alice), t0);
    let r = registry.lock().unwrap();
    assert_eq!(r.get(host + 1).unwrap().key, InstanceKey::Social(SocialSpace::Tower));
    assert_eq!(r.instance_of(alice), Some(host + 1), "arriving in the Tower left the Cosmodrome instance");
}
