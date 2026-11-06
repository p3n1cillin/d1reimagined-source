// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! ⭐ Stream O3: **which player a piece of work is for**, so one responder serves several players
//! (integration review, finding 4).
//!
//! With `D1_MP=1` every BAP connection signs on as its own player (`mp::translate`). Before this,
//! every connection was still served the knobs' SOIDs (`D1_ACCOUNT_SOID`, `D1_CHARACTER_SOID`,
//! `D1_PROFILE_SOID`) and the one player store's profile, so two players were the same player to
//! the client and to the grants. Now:
//!
//! * **Each connection has a full identity**: account, character and profile SOIDs from the player
//!   store's scheme. With the store on (`D1_STORE=1`) they are `identities.json`'s; without it the
//!   same allocation in memory (`player_store::Identity::allocated`, `0xD101_0000_nnnn_n001/101/201`).
//!   The responder's own player (`--npid`, the store's `D1_STORE_PLAYER`) keeps the knobs, so a
//!   one-player run serves exactly what it served before.
//! * **The connection's task carries it.** The BAP connection task runs inside [`CONNECTION`]'s
//!   scope; work it hands to a thread carries the player across ([`current`], [`run_as`]).
//!   `crate::env_soid` answers the three identity knobs from it, so every id the connection loop
//!   serves is the connection's own, and `player_store` serves and edits that player's record.
//! * **Off by default.** With `D1_MP` unset nothing is registered, [`current`] is always `None`,
//!   and every answer is the knobs' as before.
//!
//! `[I]`: a connection is known only once it signed on, at its first BAP 23 (the client translates
//! itself first; no other message names its online id on this connection). Anything served before
//! that is the knobs'. A second BAP connection of one client that never sends 23 is served the
//! knobs too.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::player_store::Identity;

tokio::task_local! {
    /// The BAP connection a task serves (`main.rs` scopes each connection's task with it).
    pub static CONNECTION: u64;
}

thread_local! {
    /// The player a thread works for ([`run_as`]): a vendor op or a command run off the
    /// connection's task.
    static PLAYER: Cell<Option<Identity>> = const { Cell::new(None) };
}

fn registry() -> &'static Mutex<HashMap<u64, Identity>> {
    static R: OnceLock<Mutex<HashMap<u64, Identity>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

/// The connections that sent `queuez_register`, in order (the per-player primary, [`is_primary`]).
fn registered() -> &'static Mutex<Vec<u64>> {
    static R: OnceLock<Mutex<Vec<u64>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

/// The responder's own player: the knobs (`D1_ACCOUNT_SOID`, `D1_CHARACTER_SOID`,
/// `D1_PROFILE_SOID`).
pub fn primary() -> Identity {
    Identity::from_env()
}

/// `conn` signed on as `identity` (`mp::translate`).
pub fn register(conn: u64, identity: Identity) {
    if let Ok(mut r) = registry().lock() {
        r.insert(conn, identity);
    }
}

/// `conn` closed (`mp::ConnGuard`).
pub fn forget(conn: u64) {
    if let Ok(mut r) = registry().lock() {
        r.remove(&conn);
    }
    if let Ok(mut r) = registered().lock() {
        r.retain(|c| *c != conn);
    }
}

/// The identity `conn` signed on as, if it did.
pub fn of_connection(conn: u64) -> Option<Identity> {
    registry().lock().ok()?.get(&conn).copied()
}

/// The connections of the player with this identity (several when a client opens more than one).
pub fn connections_of(identity: &Identity) -> Vec<u64> {
    let Ok(r) = registry().lock() else { return Vec::new() };
    let mut v: Vec<u64> = r.iter().filter(|(_, i)| *i == identity).map(|(c, _)| *c).collect();
    v.sort_unstable();
    v
}

/// The player this work is for: the thread's ([`run_as`]), else the connection task's, else
/// `None` (the knobs' player; also always `None` with `D1_MP` off, since nothing registers).
pub fn current() -> Option<Identity> {
    PLAYER.with(Cell::get).or_else(|| CONNECTION.try_with(|c| *c).ok().and_then(of_connection))
}

/// Whether the work is for a player other than the knobs' one.
pub fn current_is_other() -> bool {
    current().is_some_and(|i| i != primary())
}

/// The SOID `var` names for the current player, when the current player has its own: the
/// identity knobs only (`crate::env_soid` asks this first).
pub fn soid_for(var: &str) -> Option<u64> {
    let id = current()?;
    match var {
        "D1_ACCOUNT_SOID" => Some(id.account),
        "D1_CHARACTER_SOID" | "D1_CREATED_CHARACTER_SOID" => Some(id.character),
        "D1_PROFILE_SOID" => Some(id.profile),
        _ => None,
    }
}

/// Run `f` as `who` on this thread (`None`: as the knobs' player), then restore the thread's.
pub fn run_as<T>(who: Option<Identity>, f: impl FnOnce() -> T) -> T {
    let before = PLAYER.with(|p| p.replace(who));
    struct Restore(Option<Identity>);
    impl Drop for Restore {
        fn drop(&mut self) {
            PLAYER.with(|p| p.set(self.0));
        }
    }
    let _restore = Restore(before);
    f()
}

/// `conn` sent `queuez_register`.
pub fn note_register(conn: u64) {
    if let Ok(mut r) = registered().lock() {
        r.retain(|c| *c != conn);
        r.push(conn);
    }
}

/// With `D1_MP=1`, whether `conn` is its player's primary BAP connection: the last of that
/// player's connections to send `queuez_register` (each client has its own, as the single-player
/// primary is). `global` is the single-player primary, used when no player has registered.
pub fn is_primary(conn: u64, global: u64) -> bool {
    let Some(me) = of_connection(conn) else { return conn == global };
    let Ok(r) = registered().lock() else { return conn == global };
    match r.iter().rev().find(|c| of_connection(**c) == Some(me)) {
        Some(c) => *c == conn,
        None => conn == global,
    }
}

/// The full identity of a signed-on account: the store's (`identities.json`, by online id) when it
/// is on, else the allocation scheme: the knobs' for the responder's own account, and
/// [`Identity::allocated`]`(n)` for `0xD101_0000_nnnn_n001`. An account bound by hand
/// (`D1_MP_ACCOUNTS`) outside both gets `+0x100` / `+0x200` (a labelled stand-in).
pub fn identity_for(online_id: &str, account: u64) -> Identity {
    if let Some(cfg) = crate::player_store::Config::from_env() {
        match crate::player_store::identity_for(&cfg.dir, online_id, primary()) {
            Ok(i) if i.account == account => return i,
            Ok(i) => tracing::warn!("⚠ MP: '{online_id}' is {:#x} in identities.json but signed on as {account:#x}; using the scheme", i.account),
            Err(e) => tracing::warn!("⚠ MP: identities.json for '{online_id}': {e:#}; using the scheme"),
        }
    }
    derived(account)
}

/// [`identity_for`] without the store.
pub fn derived(account: u64) -> Identity {
    let p = primary();
    if account == p.account {
        return p;
    }
    if account >> 48 == 0xD101 && account & 0xFFF == 0x001 {
        let n = ((account >> 12) & 0xF_FFFF_FFFF) as u32;
        let a = Identity::allocated(n);
        if a.account == account {
            return a;
        }
    }
    Identity { account, character: account.wrapping_add(0x100), profile: account.wrapping_add(0x200) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identities_follow_the_store_scheme() {
        let a = Identity::allocated(3);
        assert_eq!(derived(a.account), a);
        assert_eq!((a.account, a.character, a.profile), (0xD101_0000_0000_3001, 0xD101_0000_0000_3101, 0xD101_0000_0000_3201));
        assert_eq!(derived(primary().account), primary(), "the responder's own player keeps the knobs");
        let odd = derived(0x1234_0000);
        assert_eq!((odd.character, odd.profile), (0x1234_0100, 0x1234_0200));
    }

    #[test]
    fn a_connections_task_and_a_thread_carry_its_player() {
        let a = Identity::allocated(41);
        let b = Identity::allocated(42);
        register(9001, a);
        register(9002, b);
        assert_eq!(current(), None, "no connection, no thread player");
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let seen = rt.block_on(CONNECTION.scope(9001, async { (current(), soid_for("D1_CHARACTER_SOID")) }));
        assert_eq!(seen, (Some(a), Some(a.character)));
        assert_eq!(run_as(Some(b), || soid_for("D1_PROFILE_SOID")), Some(b.profile));
        assert_eq!(soid_for("D1_ITEM_SOID_BASE"), None);
        assert_eq!(current(), None, "run_as restores");
        // Each player's primary is its own last registered connection.
        register(9003, a);
        note_register(9001);
        note_register(9002);
        assert!(is_primary(9001, 0) && is_primary(9002, 0));
        note_register(9003);
        assert!(!is_primary(9001, 0) && is_primary(9003, 0) && is_primary(9002, 0));
        assert_eq!(connections_of(&a), vec![9001, 9003]);
        for c in [9001, 9002, 9003] {
            forget(c);
        }
        assert!(is_primary(7, 7), "an unknown connection: the single-player primary");
    }
}
