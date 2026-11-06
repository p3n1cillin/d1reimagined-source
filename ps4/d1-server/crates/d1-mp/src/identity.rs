// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! ⭐ **Who is who** when several local players sign on to one server: the shared identity types,
//! and the registry that gives each platform account a stable Bungie account id.
//!
//! ## The ids, and where each comes from
//!
//! | type | what it is | source |
//! |---|---|---|
//! | [`OnlineId`] | the PSN online id (`SceNpOnlineId`, at most 16 bytes) | the client: shadNet sign-in, BAP 23's platform ids, the WebAPI URLs |
//! | [`AccountSoid`] | the Bungie account id (a u64 SOID) | this server, answering BAP 23 (`platform_to_investment_response`) |
//! | [`CharacterSoid`] | a character (u64 SOID) | the player store, answering 501 create-character |
//! | [`PlayerKey`] | `(account, character)`: the player store's key (`account-<soid>.json`, then a character) | both of the above |
//! | [`FireteamId`] | a fireteam | [`crate::fireteam`] |
//! | [`ConnectionId`] | one client's BAP connection | the responder |
//! | [`MemberIdentity`] | the 40-byte identity in an activity-host `join_request` and in each `replicate_membership` member | the client |
//!
//! The 40-byte identity (`0x80800C2D`) is `{u8 machine[8]; u8 player[8]; u64 account;
//! u64 character; u64 fireteam}`, so the activity host sees the same account and character SOIDs
//! the player store is keyed by. [`MemberIdentity::player_key`] is the bridge, and it is why the
//! store's keys and these types must agree: one u64 each, no other encoding.
//!
//! ## Stable account ids for several accounts: the player store's scheme
//!
//! Until now the responder answered every BAP 23 translation with the one `D1_ACCOUNT_SOID`. With
//! two players that is wrong twice: each client would read its fireteam-mate's online id as its own
//! account. [`IdentityRegistry::account_for`] gives each online id its own SOID, by **one scheme,
//! the player store's** (`d1-responder/src/player_store.rs`, `identities.json`, #307):
//!
//! 1. an explicit binding (`D1_MP_ACCOUNTS="<online id>=<soid>,..."`, and the responder's own
//!    `--npid` with `D1_ACCOUNT_SOID`): the first player keeps the responder's knobs;
//! 2. a [`Resolver`]: with the store on (`D1_STORE=1`) the responder resolves through
//!    `player_store::identity_for`, so an online id gets the account `identities.json` filed it
//!    under, the same across restarts and never reused;
//! 3. otherwise the same allocation in memory: a new player gets [`allocated_account`]`(n)` =
//!    `0xD101_0000_nnnn_n001` (the store's `Identity::allocated`), the lowest free `n`.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

/// A Bungie account id (SOID).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AccountSoid(pub u64);

/// A character id (SOID).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CharacterSoid(pub u64);

/// A fireteam, allocated by [`crate::fireteam::FireteamService`]. Never 0: the client's
/// `fireteam_from_player()` treats 0 as "no fireteam" and adds no player record for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FireteamId(pub u64);

/// One client's BAP connection to the responder (the responder numbers them).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnectionId(pub u64);

/// The player store's key: one character of one account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlayerKey {
    pub account: AccountSoid,
    pub character: CharacterSoid,
}

impl fmt::Display for AccountSoid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "account {:#018x}", self.0)
    }
}

impl fmt::Display for FireteamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fireteam {:#x}", self.0)
    }
}

/// A PSN online id. At most 16 bytes (`ORBIS_NP_ONLINEID_MAX_LENGTH`), never empty.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OnlineId(String);

impl OnlineId {
    pub const MAX_LEN: usize = 16;

    pub fn new(s: &str) -> Option<Self> {
        (!s.is_empty() && s.len() <= Self::MAX_LEN && !s.contains('\0')).then(|| Self(s.to_owned()))
    }

    /// The online id at the start of a NUL-padded platform identifier (BAP 23's 36-byte ids,
    /// `SceNpId`): the bytes before the first NUL.
    pub fn from_padded(b: &[u8]) -> Option<Self> {
        let end = b.iter().position(|&c| c == 0).unwrap_or(b.len()).min(Self::MAX_LEN);
        Self::new(std::str::from_utf8(&b[..end]).ok()?)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OnlineId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The 40-byte member identity (`0x80800C2D`) of a `join_request` (at `+0x10`) and of each
/// `replicate_membership` member (at `+0x00`).
///
/// ⚠ `character` is what the client writes at `+0x18`, and run fg2g (6 November) measured it to
/// be the player's **profile** SOID (`0xD100_0000_0000_0201` for the responder's own player,
/// whose character is `...101`). The responder matches it against a player's character or
/// profile (`player_store::Identity::named_by`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MemberIdentity {
    pub machine: [u8; 8],
    pub player: [u8; 8],
    pub account: AccountSoid,
    pub character: CharacterSoid,
    pub fireteam: u64,
}

impl MemberIdentity {
    pub const SIZE: usize = 0x28;
    /// Where the identity sits in a `join_request` struct image.
    pub const IN_JOIN_REQUEST: usize = 0x10;

    pub fn parse(b: &[u8]) -> Option<Self> {
        let b = b.get(..Self::SIZE)?;
        let u = |o: usize| u64::from_le_bytes(b[o..o + 8].try_into().unwrap());
        Some(Self {
            machine: b[0..8].try_into().unwrap(),
            player: b[8..16].try_into().unwrap(),
            account: AccountSoid(u(0x10)),
            character: CharacterSoid(u(0x18)),
            fireteam: u(0x20),
        })
    }

    pub fn from_join_request(image: &[u8]) -> Option<Self> {
        Self::parse(image.get(Self::IN_JOIN_REQUEST..)?)
    }

    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut b = [0u8; Self::SIZE];
        b[0..8].copy_from_slice(&self.machine);
        b[8..16].copy_from_slice(&self.player);
        b[0x10..0x18].copy_from_slice(&self.account.0.to_le_bytes());
        b[0x18..0x20].copy_from_slice(&self.character.0.to_le_bytes());
        b[0x20..0x28].copy_from_slice(&self.fireteam.to_le_bytes());
        b
    }

    pub fn player_key(&self) -> PlayerKey {
        PlayerKey { account: self.account, character: self.character }
    }
}

/// Player `n`'s (n >= 1) account: `0xD101_0000_nnnn_n001`, the player store's
/// `Identity::allocated(n).account` (its character is `...101`, its profile `...201`).
pub fn allocated_account(n: u32) -> AccountSoid {
    AccountSoid(0xD101_0000_0000_0000 | (u64::from(n) << 12) | 0x001)
}

/// Player `n`'s character, `0xD101_0000_nnnn_n101`, as the store allocates it.
pub fn allocated_character(n: u32) -> CharacterSoid {
    CharacterSoid(0xD101_0000_0000_0000 | (u64::from(n) << 12) | 0x101)
}

/// An outside source of accounts: the player store's `identities.json`. `None` falls back to
/// the in-memory allocation.
#[derive(Clone)]
pub struct Resolver(pub std::sync::Arc<dyn Fn(&OnlineId) -> Option<AccountSoid> + Send + Sync>);

impl fmt::Debug for Resolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Resolver")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityError {
    /// The binding spec could not be read: the offending entry.
    BadBinding(String),
    /// Two online ids bound to one account.
    AccountTaken { account: AccountSoid, by: OnlineId },
    /// The connection never signed on.
    UnknownConnection(ConnectionId),
    /// The account already has a live connection (a second client signing on as it).
    AlreadySignedOn { account: AccountSoid, connection: ConnectionId },
}

impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadBinding(e) => write!(f, "account binding {e:?} is not <online id>=<soid>"),
            Self::AccountTaken { account, by } => write!(f, "{account} is already bound to {by}"),
            Self::UnknownConnection(c) => write!(f, "connection {} never signed on", c.0),
            Self::AlreadySignedOn { account, connection } => {
                write!(f, "{account} is already signed on through connection {}", connection.0)
            }
        }
    }
}

impl std::error::Error for IdentityError {}

/// What the server knows about one signed-on player. [`crate::presence`] reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedOn {
    pub connection: ConnectionId,
    pub online_id: OnlineId,
    pub account: AccountSoid,
    /// The character the player is playing, once it chose one (504 login_character).
    pub character: Option<CharacterSoid>,
}

/// Online id ↔ account SOID, and which accounts are signed on through which connection.
#[derive(Debug, Default, Clone)]
pub struct IdentityRegistry {
    resolver: Option<Resolver>,
    by_name: BTreeMap<OnlineId, AccountSoid>,
    by_account: HashMap<AccountSoid, OnlineId>,
    sessions: BTreeMap<ConnectionId, SignedOn>,
}

impl IdentityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolve new online ids through `r` (the player store) before allocating in memory.
    pub fn set_resolver(&mut self, r: Resolver) {
        self.resolver = Some(r);
    }

    /// Bind `online_id` to `account` explicitly (an existing record's id).
    pub fn bind(&mut self, online_id: OnlineId, account: AccountSoid) -> Result<(), IdentityError> {
        if let Some(by) = self.by_account.get(&account) {
            if *by != online_id {
                return Err(IdentityError::AccountTaken { account, by: by.clone() });
            }
            return Ok(());
        }
        if let Some(old) = self.by_name.insert(online_id.clone(), account) {
            self.by_account.remove(&old);
        }
        self.by_account.insert(account, online_id);
        Ok(())
    }

    /// Read `name=soid,name=soid` (soid decimal or `0x` hex), as `D1_MP_ACCOUNTS` carries it.
    pub fn bind_spec(&mut self, spec: &str) -> Result<usize, IdentityError> {
        let mut n = 0;
        for entry in spec.split(',').map(str::trim).filter(|e| !e.is_empty()) {
            let bad = || IdentityError::BadBinding(entry.to_owned());
            let (name, soid) = entry.split_once('=').ok_or_else(bad)?;
            let id = OnlineId::new(name.trim()).ok_or_else(bad)?;
            let soid = soid.trim();
            let v = match soid.strip_prefix("0x").or_else(|| soid.strip_prefix("0X")) {
                Some(h) => u64::from_str_radix(h, 16),
                None => soid.parse(),
            }
            .map_err(|_| bad())?;
            if v == 0 {
                return Err(bad());
            }
            self.bind(id, AccountSoid(v))?;
            n += 1;
        }
        Ok(n)
    }

    /// ⭐ The account SOID for `online_id`: its binding, else the resolver's (the player store),
    /// else the next allocated one. Bound on first use, so the answer never changes within a run.
    pub fn account_for(&mut self, online_id: &OnlineId) -> AccountSoid {
        if let Some(a) = self.by_name.get(online_id) {
            return *a;
        }
        let resolved = self
            .resolver
            .as_ref()
            .and_then(|r| (r.0)(online_id))
            .filter(|a| !self.by_account.contains_key(a));
        let account = match resolved {
            Some(a) => a,
            None => (1..).map(allocated_account).find(|a| !self.by_account.contains_key(a)).expect("u32 players"),
        };
        self.by_name.insert(online_id.clone(), account);
        self.by_account.insert(account, online_id.clone());
        account
    }

    /// The online id an account belongs to, if it has been seen.
    pub fn online_id_of(&self, account: AccountSoid) -> Option<&OnlineId> {
        self.by_account.get(&account)
    }

    /// A client signed on through `connection` as `online_id`. One live connection per account:
    /// a second is refused, as PSN refuses a second sign-in of a live online id.
    pub fn sign_on(&mut self, connection: ConnectionId, online_id: &OnlineId) -> Result<AccountSoid, IdentityError> {
        let account = self.account_for(online_id);
        if let Some(s) = self.sessions.values().find(|s| s.account == account && s.connection != connection) {
            return Err(IdentityError::AlreadySignedOn { account, connection: s.connection });
        }
        self.sessions.insert(
            connection,
            SignedOn { connection, online_id: online_id.clone(), account, character: None },
        );
        Ok(account)
    }

    /// The player chose a character on `connection`.
    pub fn select_character(&mut self, connection: ConnectionId, character: CharacterSoid) -> Result<PlayerKey, IdentityError> {
        let s = self.sessions.get_mut(&connection).ok_or(IdentityError::UnknownConnection(connection))?;
        s.character = Some(character);
        Ok(PlayerKey { account: s.account, character })
    }

    /// The connection closed. Returns who it was.
    pub fn sign_off(&mut self, connection: ConnectionId) -> Option<SignedOn> {
        self.sessions.remove(&connection)
    }

    pub fn session(&self, connection: ConnectionId) -> Option<&SignedOn> {
        self.sessions.get(&connection)
    }

    pub fn connection_of(&self, account: AccountSoid) -> Option<ConnectionId> {
        self.sessions.values().find(|s| s.account == account).map(|s| s.connection)
    }

    pub fn is_online(&self, account: AccountSoid) -> bool {
        self.connection_of(account).is_some()
    }

    pub fn signed_on(&self) -> impl Iterator<Item = &SignedOn> {
        self.sessions.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &str) -> OnlineId {
        OnlineId::new(s).unwrap()
    }

    #[test]
    fn each_online_id_gets_its_own_stable_account() {
        let mut r = IdentityRegistry::new();
        let a = r.account_for(&id("GuardianA"));
        let b = r.account_for(&id("GuardianB"));
        assert_ne!(a, b);
        assert_eq!(r.account_for(&id("GuardianA")), a, "stable within a run");
        // The player store's allocation: 0xD101_0000_nnnn_n001.
        assert_eq!((a, b), (AccountSoid(0xD101_0000_0000_1001), AccountSoid(0xD101_0000_0000_2001)));
        assert_eq!(allocated_character(1), CharacterSoid(0xD101_0000_0000_1101));
        assert_eq!(r.online_id_of(b), Some(&id("GuardianB")));
    }

    #[test]
    fn an_explicit_binding_wins_and_is_exclusive() {
        let mut r = IdentityRegistry::new();
        assert_eq!(r.bind_spec("D1Guardian=0xD100000000000001, Second=16").unwrap(), 2);
        assert_eq!(r.account_for(&id("D1Guardian")), AccountSoid(0xD100_0000_0000_0001));
        assert_eq!(r.account_for(&id("Second")), AccountSoid(16));
        assert!(matches!(r.bind(id("Third"), AccountSoid(16)), Err(IdentityError::AccountTaken { .. })));
        assert!(r.bind_spec("nobody").is_err());
        assert!(r.bind_spec("x=0").is_err());
    }

    #[test]
    fn allocation_skips_taken_ids_and_the_store_resolver_wins() {
        let mut r = IdentityRegistry::new();
        r.bind(id("Squatter"), allocated_account(1)).unwrap();
        assert_eq!(r.account_for(&id("Next")), allocated_account(2));
        // With the store: its answer, the same in every run.
        let store = |n: &OnlineId| (n.as_str() == "Stored").then_some(AccountSoid(0xD101_0000_0000_7001));
        r.set_resolver(Resolver(std::sync::Arc::new(store)));
        assert_eq!(r.account_for(&id("Stored")), AccountSoid(0xD101_0000_0000_7001));
        assert_eq!(r.account_for(&id("Unstored")), allocated_account(3), "the resolver declined");
    }

    #[test]
    fn one_live_connection_per_account() {
        let mut r = IdentityRegistry::new();
        let a = r.sign_on(ConnectionId(1), &id("A")).unwrap();
        r.sign_on(ConnectionId(2), &id("B")).unwrap();
        assert_eq!(
            r.sign_on(ConnectionId(3), &id("A")),
            Err(IdentityError::AlreadySignedOn { account: a, connection: ConnectionId(1) })
        );
        // The same connection signing on again is a refresh, not a second sign-in.
        assert_eq!(r.sign_on(ConnectionId(1), &id("A")), Ok(a));
        let key = r.select_character(ConnectionId(1), CharacterSoid(0x101)).unwrap();
        assert_eq!(key, PlayerKey { account: a, character: CharacterSoid(0x101) });
        assert_eq!(r.sign_off(ConnectionId(1)).unwrap().account, a);
        assert!(!r.is_online(a));
        // After sign-off, a reconnect gets the same account back.
        assert_eq!(r.sign_on(ConnectionId(4), &id("A")), Ok(a));
    }

    #[test]
    fn the_member_identity_round_trips_and_names_the_store_key() {
        let m = MemberIdentity {
            machine: [1, 2, 3, 4, 5, 6, 0, 0],
            player: [9; 8],
            account: AccountSoid(0xD100_0000_0000_0001),
            character: CharacterSoid(0xD100_0000_0000_0201),
            fireteam: 7,
        };
        let mut join = vec![0u8; 0x100];
        join[0x10..0x38].copy_from_slice(&m.to_bytes());
        assert_eq!(MemberIdentity::from_join_request(&join), Some(m));
        assert_eq!(m.player_key().character, CharacterSoid(0xD100_0000_0000_0201));
        assert_eq!(MemberIdentity::parse(&[0u8; 39]), None);
    }

    #[test]
    fn online_ids_are_read_from_padded_platform_ids() {
        let mut b = [0u8; 36];
        b[..6].copy_from_slice(b"Player");
        assert_eq!(OnlineId::from_padded(&b), Some(id("Player")));
        assert_eq!(OnlineId::from_padded(&[0u8; 36]), None);
        assert!(OnlineId::new("seventeen-chars-x").is_none());
    }
}
