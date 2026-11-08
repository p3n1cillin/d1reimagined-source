// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! Multi-client scenarios against one [`Services`], with fake in-process clients. Each fake
//! client speaks the wire the real one does (message 42 bodies in the captured shapes, BAP 23
//! platform ids) and holds only what a client would: its connection, its online id, the ids the
//! service gave it.

use d1_bap::matchmaking::{MatchmakingCoreRequest, MatchmakingCoreResponse, MatchmakingKey};
use d1_bap::matchmaking_sessions::{decode_search_results, SLOT_ACTIVITY, SLOT_BUBBLE};
use d1_mp::fireteam::{ActivityParty, ActivityRef, FireteamError, FireteamRules};
use d1_mp::identity::{AccountSoid, ConnectionId, OnlineId};
use d1_mp::matchmaking::{place_in_public, InMemoryPublicInstances, Playlist, PlaylistKind, PublicInstance, PublicPlacement};
use d1_mp::Services;

fn varint(out: &mut Vec<u8>, field: u32, mut v: u64) {
    let mut tag = u64::from(field) << 3;
    loop { let b = (tag & 0x7F) as u8; tag >>= 7; if tag == 0 { out.push(b); break } out.push(b | 0x80) }
    loop { let b = (v & 0x7F) as u8; v >>= 7; if v == 0 { out.push(b); return } out.push(b | 0x80) }
}

fn bytes(out: &mut Vec<u8>, field: u32, b: &[u8]) {
    let mut tag = u64::from(field) << 3 | 2;
    loop { let x = (tag & 0x7F) as u8; tag >>= 7; if tag == 0 { out.push(x); break } out.push(x | 0x80) }
    let mut n = b.len() as u64;
    loop { let x = (n & 0x7F) as u8; n >>= 7; if n == 0 { out.push(x); break } out.push(x | 0x80) }
    out.extend_from_slice(b);
}

/// A fake client: what a real one holds.
struct Client {
    conn: ConnectionId,
    name: &'static str,
    account: AccountSoid,
    nonce: u64,
    advertisement: Option<u64>,
}

impl Client {
    fn sign_on(s: &mut Services, conn: u64, name: &'static str, now: u64) -> Self {
        let (account, _) = s.sign_on(ConnectionId(conn), &OnlineId::new(name).unwrap(), now).unwrap();
        Self { conn: ConnectionId(conn), name, account, nonce: conn << 32, advertisement: None }
    }

    fn base(&mut self, kind: u32, slot: u32, set: u32) -> Vec<u8> {
        self.nonce += 1;
        MatchmakingCoreRequest {
            version: Some(13),
            request_kind: Some(kind),
            nonce: Some(self.nonce),
            search_slot: Some(slot),
            matchmaking_set_id: Some(set),
            matchmaking_key: Some(MatchmakingKey { activity_hash: set, matchmaking_content_hash: 0, skill_bucket_hash: None }),
            ..Default::default()
        }
        .encode()
    }

    /// Advertise (or update) the session this client hosts, with `open` slots.
    fn advertise(&mut self, s: &mut Services, slot: u32, set: u32, open: u32, now: u64) -> u64 {
        let mut body = self.base(2, slot, set);
        let mut attrs = Vec::new();
        varint(&mut attrs, 3, 1);
        varint(&mut attrs, 7, u64::from(open));
        let mut session = Vec::new();
        bytes(&mut session, 1, format!("session-of-{}", self.name).as_bytes());
        let mut desc = Vec::new();
        bytes(&mut desc, 1, &attrs);
        bytes(&mut desc, 2, &session);
        bytes(&mut desc, 3, self.name.as_bytes());
        let mut f8 = Vec::new();
        if let Some(id) = self.advertisement {
            varint(&mut f8, 1, id);
        }
        bytes(&mut f8, 2, &desc);
        bytes(&mut body, 8, &f8);
        let r = s.matchmaking(self.conn, &body, now).unwrap();
        let id = MatchmakingCoreResponse::decode(&r.body).unwrap().advertisement_ack.unwrap();
        self.advertisement = Some(id);
        id
    }

    /// Search; returns the names of the candidates' hosts.
    fn search(&mut self, s: &mut Services, slot: u32, set: u32, needed: Option<u32>, now: u64) -> Vec<String> {
        let mut body = self.base(1, slot, set);
        let mut attrs = Vec::new();
        varint(&mut attrs, 3, 3);
        if let Some(n) = needed {
            varint(&mut attrs, 7, u64::from(n));
        }
        varint(&mut attrs, 13, 0);
        let mut f7 = Vec::new();
        bytes(&mut f7, 1, &attrs);
        bytes(&mut body, 7, &f7);
        let r = s.matchmaking(self.conn, &body, now).unwrap();
        let (cands, _) = decode_search_results(&r.body).unwrap().unwrap();
        cands.iter().map(|c| String::from_utf8(c.name.clone()).unwrap()).collect()
    }

    fn padded_id(name: &str) -> [u8; 36] {
        let mut b = [0u8; 36];
        b[..name.len()].copy_from_slice(name.as_bytes());
        b
    }
}

const STRIKE: u32 = 0xFBFD_F050;
const BUBBLE: u32 = 0x8001_0038;

fn services() -> Services {
    let mut s = Services::new(FireteamRules::default());
    s.identity.bind(OnlineId::new("Alpha").unwrap(), AccountSoid(0xD100_0000_0000_0001)).unwrap();
    s
}

#[test]
fn every_player_gets_its_own_account_and_bap_23_says_so() {
    let mut s = services();
    let a = Client::sign_on(&mut s, 1, "Alpha", 0);
    let b = Client::sign_on(&mut s, 2, "Bravo", 0);
    assert_eq!(a.account, AccountSoid(0xD100_0000_0000_0001), "the bound single-player account keeps its id");
    assert_ne!(a.account, b.account);
    // Alpha's client asks about both players: two different accounts, not its own twice.
    let t = s.translate_accounts(&[Client::padded_id("Alpha"), Client::padded_id("Bravo")]);
    assert_eq!(t.iter().map(|(_, soid)| *soid).collect::<Vec<_>>(), vec![a.account.0, b.account.0]);
    // A second client signing on as Alpha is refused.
    assert!(s.sign_on(ConnectionId(3), &OnlineId::new("Alpha").unwrap(), 0).is_err());
}

#[test]
fn fireteam_lifecycle_with_presence() {
    let mut s = services();
    let a = Client::sign_on(&mut s, 1, "Alpha", 0);
    let b = Client::sign_on(&mut s, 2, "Bravo", 0);
    let c = Client::sign_on(&mut s, 3, "Charlie", 0);
    let d = Client::sign_on(&mut s, 4, "Delta", 0);
    let inv = s.fireteams.invite(a.account, b.account, 1).unwrap();
    s.fireteams.accept(inv, b.account).unwrap();
    let p = s.presence(a.account);
    assert!(p.online && p.leader && p.joinable);
    assert_eq!(p.fireteam_size, 2);
    assert_eq!(s.presence(b.account).fireteam, p.fireteam);
    // Charlie joins Alpha directly (an open fireteam): three, the cap, and no longer joinable.
    s.fireteams.join_member(c.account, a.account).unwrap();
    assert!(!s.presence(a.account).joinable);
    assert!(matches!(s.fireteams.join_member(d.account, a.account), Err(FireteamError::Full { cap: 3, .. })));
    // Alpha leaves: Bravo, the earliest after Alpha, leads; Alpha is alone again.
    s.fireteams.leave(a.account).unwrap();
    assert!(s.presence(b.account).leader);
    assert_eq!(s.presence(b.account).fireteam_size, 2);
    assert_eq!(s.presence(a.account).fireteam_size, 1);
}

#[test]
fn a_strike_fills_by_matchmaking_and_honours_fireteam_size() {
    let mut s = services();
    let mut a = Client::sign_on(&mut s, 1, "Alpha", 0);
    let mut b = Client::sign_on(&mut s, 2, "Bravo", 0);
    let mut c = Client::sign_on(&mut s, 3, "Charlie", 0);
    let d = Client::sign_on(&mut s, 4, "Delta", 0);
    // Charlie and Delta are a fireteam of two.
    let inv = s.fireteams.invite(c.account, d.account, 0).unwrap();
    s.fireteams.accept(inv, d.account).unwrap();
    // Alpha, solo, starts the strike and advertises its session with 2 open slots.
    a.advertise(&mut s, SLOT_ACTIVITY, STRIKE, 2, 1);
    // Bravo finds it.
    assert_eq!(b.search(&mut s, SLOT_ACTIVITY, STRIKE, None, 2), vec!["Alpha"]);
    // Bravo joined; Alpha updates to 1 open slot.
    a.advertise(&mut s, SLOT_ACTIVITY, STRIKE, 1, 3);
    // Charlie's fireteam of two needs two slots: not offered Alpha's session.
    assert!(c.search(&mut s, SLOT_ACTIVITY, STRIKE, None, 4).is_empty(), "never a session the fireteam cannot fit");
    // So Charlie hosts its own; a later solo searcher is sent to the fuller of the two with room.
    c.advertise(&mut s, SLOT_ACTIVITY, STRIKE, 1, 5);
    let mut e = Client::sign_on(&mut s, 5, "Echo", 0);
    assert_eq!(e.search(&mut s, SLOT_ACTIVITY, STRIKE, None, 6), vec!["Alpha"], "equal room: the oldest");
    // Delta, in Charlie's fireteam, is never offered Charlie's own session.
    let mut d = d;
    let found = d.search(&mut s, SLOT_ACTIVITY, STRIKE, Some(1), 7);
    assert_eq!(found, vec!["Alpha"]);
}

#[test]
fn a_public_bubble_search_respects_the_reservation_and_the_registry_fills() {
    let mut s = services();
    let mut a = Client::sign_on(&mut s, 1, "Alpha", 0);
    let mut b = Client::sign_on(&mut s, 2, "Bravo", 0);
    a.advertise(&mut s, SLOT_BUBBLE, BUBBLE, 2, 0);
    assert!(b.search(&mut s, SLOT_BUBBLE, BUBBLE, Some(3), 1).is_empty(), "3 reserved slots do not fit in 2");
    a.advertise(&mut s, SLOT_BUBBLE, BUBBLE, 5, 2);
    assert_eq!(b.search(&mut s, SLOT_BUBBLE, BUBBLE, Some(3), 3), vec!["Alpha"]);
    // The same rule over the activity host's public instances (stream G's registry).
    let mut reg = InMemoryPublicInstances::default();
    reg.by_bubble.insert(BUBBLE, vec![PublicInstance { host: 0xD1A0_0000_0000_0002, players: 1, reserved: 2, capacity: 6, started_ms: 0 }]);
    assert_eq!(place_in_public(BUBBLE, 3, &mut reg), PublicPlacement::Join { host: 0xD1A0_0000_0000_0002 });
    assert_eq!(place_in_public(BUBBLE, 3, &mut reg), PublicPlacement::StartNew);
}

#[test]
fn leave_and_reconnect() {
    let mut s = services();
    let a = Client::sign_on(&mut s, 1, "Alpha", 0);
    let mut b = Client::sign_on(&mut s, 2, "Bravo", 0);
    let inv = s.fireteams.invite(a.account, b.account, 0).unwrap();
    s.fireteams.accept(inv, b.account).unwrap();
    let team = s.presence(a.account).fireteam;
    b.advertise(&mut s, SLOT_BUBBLE, BUBBLE, 4, 0);
    // Bravo's connection drops: its advertisement goes, its fireteam slot stays.
    s.sign_off(b.conn, 1_000);
    assert_eq!(s.directory.advertisements().count(), 0);
    assert!(!s.presence(b.account).online);
    assert_eq!(s.presence(a.account).fireteam_size, 2);
    s.tick(30_000);
    // Bravo reconnects on a new connection within the grace: same account, same fireteam.
    let b2 = Client::sign_on(&mut s, 7, "Bravo", 40_000);
    assert_eq!(b2.account, b.account);
    assert_eq!(s.presence(b.account).fireteam, team);
    assert!(s.presence(b.account).online);
    // Dropped again and gone past the grace: the slot is released.
    s.sign_off(b2.conn, 50_000);
    s.tick(50_000 + s.fireteams.rules().reconnect_grace_ms);
    assert_eq!(s.presence(a.account).fireteam_size, 1);
    let b3 = Client::sign_on(&mut s, 8, "Bravo", 400_000);
    assert_ne!(s.presence(b3.account).fireteam, team);
}

#[test]
fn launches_check_the_party_and_a_raid_takes_six() {
    let mut s = services();
    let clients: Vec<Client> = (1..=7).map(|i| Client::sign_on(&mut s, i, ["A", "B", "C", "D", "E", "F", "G"][i as usize - 1], 0)).collect();
    let lead = &clients[0];
    let raid = ActivityRef { activity_hash: 0x6D7C_C3BC, party: Some(ActivityParty { min_party: 1, max_party: 6 }), host: None, joinable_in_progress: true };
    s.launch_activity(lead.conn, raid).unwrap();
    for c in &clients[1..6] {
        s.fireteams.join_member(c.account, lead.account).unwrap();
    }
    assert!(matches!(s.fireteams.join_member(clients[6].account, lead.account), Err(FireteamError::Full { cap: 6, .. })));
    let launched = s.launch_activity(lead.conn, ActivityRef { host: Some(0xD1A0_0000_0000_0009), ..raid }).unwrap();
    assert_eq!(launched.len(), 6);
    // Six cannot launch a strike playlist (3).
    s.matchmaker.add_playlist(Playlist { hash: 0x5757, kind: PlaylistKind::Cooperative { max_players: 3 }, activities: vec![STRIKE] });
    assert!(s.launch_playlist(lead.conn, 0x5757, Some(ActivityParty { min_party: 1, max_party: 3 }), 0).is_err());
    // Only the leader launches.
    assert!(s.launch_playlist(clients[1].conn, 0x5757, None, 0).is_err());
}

#[test]
fn a_crucible_playlist_groups_whole_fireteams() {
    let mut s = services();
    s.matchmaker.add_playlist(Playlist { hash: 0xC0C0, kind: PlaylistKind::Versus { teams: 2, team_size: 3, min_players: 6 }, activities: vec![0x1111] });
    let names = ["A", "B", "C", "D", "E", "F"];
    let c: Vec<Client> = (0..6).map(|i| Client::sign_on(&mut s, i as u64 + 1, names[i], 0)).collect();
    // A+B+C and D+E, and F solo.
    for (lead, m) in [(0, 1), (0, 2), (3, 4)] {
        let inv = s.fireteams.invite(c[lead].account, c[m].account, 0).unwrap();
        s.fireteams.accept(inv, c[m].account).unwrap();
    }
    for lead in [0, 3, 5] {
        assert_eq!(s.launch_playlist(c[lead].conn, 0xC0C0, None, 1).unwrap(), None, "queued");
    }
    let formed = s.tick(2);
    assert_eq!(formed.len(), 1);
    assert_eq!(formed[0].players, 6);
    assert_eq!(formed[0].fireteams.len(), 3);
    // A queued fireteam that disconnects leaves the queue.
    s.launch_playlist(c[0].conn, 0xC0C0, None, 3).unwrap();
    s.sign_off(c[0].conn, 4);
    assert!(s.matchmaker.queued().is_empty());
}
