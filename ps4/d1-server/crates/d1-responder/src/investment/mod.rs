// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! ⭐ Stream P: the investment requests the responder had no answer for, served end to end.
//!
//! | Service | What | Gate | Module |
//! |---|---|---|---|
//! | 406 | the item's state flags (bit 0: the lock) | `D1_SOCIAL=1` | [`item_lock`] |
//! | 402 action 1 on a bounty | the bounty turn-in (gear's 402 stays the dismantle) | `D1_SOCIAL=1` | [`bounties`] |
//! | 801 | activate a talent node (upgrades) | `D1_REWARDS=1` | [`talents`] |
//! | 802 | change a node's step | `D1_REWARDS=1` | [`talents`] |
//! | 804 | infusion | `D1_REWARDS=1` | [`infusion`] |
//! | 904 | packages, turn-ins and other vendor Rewards sales | `D1_SOCIAL=1` | [`packages`] |
//! | 1801, 1802, 1803 | Record Book rewards and unsealing | `D1_REWARDS=1` | [`record_books`] |
//!
//! Every one also needs the player store (`D1_STORE=1`) and a family-4 baseline the client
//! accepted. Without them [`serve`] returns `None` and the responder answers as it did before
//! (the defaults are unchanged).
//!
//! **The live order** (the Tower capture, notes/d1-live-captures-patrol-tower-2026-11-06.md): each
//! exchange is the request, then one family-4 push of the changed objects, then the reply, whose
//! `R.value` is the family-4 version after that push (402's is 0). [`serve`] decodes the request
//! (`d1_bap::investment_ops`), reads the player's state ([`store_io::load`]), runs the handler on a
//! [`snapshot::Snapshot`], commits it ([`store_io::commit`]), and hands back the push and the reply
//! for the connection to write in that order.
//!
//! **A refusal** changes nothing and pushes nothing; the reply carries [`REFUSED`] and the version
//! the client already holds. ⚠ `investment.refusal_status`: which status values the client tells
//! apart is open (every live reply was status 0), so one stand-in value is sent for every refusal;
//! the client checks each request itself before sending it, so a refusal should be rare.

pub mod bounties;
pub mod infusion;
pub mod item_lock;
pub mod packages;
pub mod record_books;
pub mod snapshot;
pub mod store_io;
pub mod talents;

use d1_bap::investment_ops::{self as ops, Request};

/// Why a request was refused (logged; the wire carries [`REFUSED`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    NotFound,
    Invalid,
    NotAvailable,
    NoFunds,
    NoRoom,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub status: Status,
    pub why: String,
}

impl Refusal {
    pub fn new(status: Status, why: impl Into<String>) -> Refusal {
        Refusal { status, why: why.into() }
    }
}

/// The `R.status` of a refusal (stand-in, `investment.refusal_status`).
pub const REFUSED: i8 = 1;

/// Which services this module answers, with their gate on.
pub fn gated(service: u16) -> bool {
    let social = crate::social_hooks::enabled();
    let rewards = crate::rewards::enabled();
    let on = match service {
        402 | 406 | 904 => social,
        801 | 802 | 804 | 1801 | 1802 | 1803 => rewards,
        _ => false,
    };
    on && crate::player_store::enabled()
}

/// The instances a request names, and how many new instance SOIDs it may need.
fn wants(r: &Request) -> (Vec<u64>, usize) {
    match *r {
        Request::ItemFlags { item, .. } => (vec![item.instance], 0),
        Request::ActivateNode { item, .. } | Request::ChangeNodeStep { item, .. } => (vec![item], 0),
        Request::Infuse { target, source, .. } => (vec![target, source], 0),
        Request::VendorInteract { .. } => (Vec::new(), 1),
        _ => (Vec::new(), 0),
    }
}

/// Run a decoded request's handler on `s`.
pub fn handle(s: &mut snapshot::Snapshot, r: &Request, template: &[u8]) -> Result<String, Refusal> {
    match *r {
        Request::ItemFlags { item, flags } => item_lock::set_flags(s, item, flags),
        Request::ActivateNode { item, node } => talents::activate(s, item, node),
        Request::ChangeNodeStep { item, node } => talents::change_step(s, item, node),
        Request::Infuse { target, node, source } => infusion::infuse(s, target, node, source),
        Request::VendorInteract { vendor, a, b, sale } => packages::interact(s, vendor, a, b, sale, template),
        Request::RedeemRecordReward { reward } => record_books::redeem(s, reward),
        Request::UnsealBook { book } => record_books::unseal(s, book),
        Request::ClaimTableReward { entry, row, table } => record_books::claim(s, entry, row, table),
    }
}

/// One served exchange: write `push` (if any) as the next family-4 delta, then `reply`; once the
/// push is written, call [`Served::delivered`].
pub struct Served {
    pub push: Option<(u32, Vec<u8>)>,
    pub reply: Vec<u8>,
    pub what: String,
    committed: Option<store_io::Committed>,
}

impl Served {
    pub fn delivered(&self) {
        if let Some(c) = &self.committed {
            store_io::delivered(c);
        }
    }
}

enum Job {
    Request(Request),
    TurnIn(ops::ItemSelector),
}

fn job(service: u16, body: &[u8]) -> Option<Job> {
    if service == 402 {
        let (item, action) = ops::decode_item_action(body)?;
        if action != bounties::TURN_IN {
            return None;
        }
        // Only a bounty: gear's 402 stays the dismantle (`vendor::on_item_action`).
        let content = crate::player_store::view(|s| {
            let p = s.chr().profile_object.clone()?;
            let snap = snapshot::Snapshot { profile: p, ..Default::default() };
            snap.content_of(item.instance)
        }).flatten()?;
        return bounties::is_bounty(content).then_some(Job::TurnIn(item));
    }
    ops::decode(service, body).map(Job::Request)
}

/// Serve `service`'s request `body` for the current player, if this module answers it.
/// `family4` is the family-4 version the client last accepted (`None`: no baseline yet).
pub fn serve(service: u16, body: &[u8], family4: Option<u32>) -> Option<Served> {
    if !gated(service) {
        return None;
    }
    let version = family4?;
    let job = job(service, body)?;
    let what_req = match &job {
        Job::Request(r) => format!("{r:?}"),
        Job::TurnIn(i) => format!("bounty turn-in {i:?}"),
    };
    // The reply value: the version after the push, except 402's (live: 0).
    let reply_value = |v: u32| if service == 402 { 0 } else { v as i32 };
    let run = || -> Result<(String, Option<store_io::Committed>), Refusal> {
        let (ids, mint) = match &job {
            Job::Request(r) => wants(r),
            Job::TurnIn(i) => (vec![i.instance], 2),
        };
        let before = store_io::load(&ids, mint).map_err(|e| Refusal::new(Status::NotFound, format!("{e:#}")))?;
        let template = store_io::template();
        let mut after = before.clone();
        let what = match &job {
            Job::Request(r) => handle(&mut after, r, &template)?,
            Job::TurnIn(item) => bounties::turn_in(&mut after, *item, store_io::curves().as_ref(), &template)?,
        };
        if after.log.is_empty() && after.attributes == before.attributes {
            return Ok((what, None));
        }
        let c = store_io::commit(&before, &after, &format!("investment {service}: {what}"))
            .map_err(|e| Refusal::new(Status::Invalid, format!("not committed: {e:#}")))?;
        Ok((what, Some(c)))
    };
    let who = crate::players::current();
    let result = tokio::task::block_in_place(|| crate::players::run_as(who, run));
    let account = crate::env_soid("D1_ACCOUNT_SOID", 0xD100_0000_0000_0001);
    Some(match result {
        Ok((what, committed)) => {
            let changes = committed.as_ref().map(|c| c.changes.clone()).unwrap_or_default();
            let e = ops::exchange(account, version, 0, &changes);
            let value = e.push.as_ref().map_or(version, |p| p.0);
            Served { push: e.push, reply: ops::result_body(0, reply_value(value)), what: format!("{what_req}: {what}"), committed }
        }
        Err(r) => Served {
            push: None,
            reply: ops::result_body(REFUSED, reply_value(version)),
            what: format!("{what_req}: REFUSED ({:?}) {}", r.status, r.why),
            committed: None,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use snapshot::tests::{snapshot, with_instance};

    /// The handlers through the one dispatcher, and the live order of what the push would carry.
    #[test]
    fn the_dispatcher_runs_each_request_and_a_refusal_changes_nothing() {
        let mut s = snapshot();
        with_instance(&mut s, 3598, 0x401, vec![0; snapshot::element::BYTES]);
        let lock = Request::ItemFlags { item: ops::ItemSelector { instanced: true, instance: 0x401, index: -1 }, flags: 1 };
        handle(&mut s, &lock, &[]).unwrap();
        assert_eq!(s.log, vec![snapshot::Touch::Profile]);
        let before = s.clone();
        let bad = Request::Infuse { target: 0x401, node: 0, source: 0x401 };
        assert!(handle(&mut s, &bad, &[]).is_err());
        assert_eq!(s, before);
        handle(&mut s, &Request::UnsealBook { book: 1 }, &[]).unwrap();
        assert_eq!(s.log, vec![snapshot::Touch::Profile], "an unseal pushes nothing");
    }

    #[test]
    fn nothing_is_served_with_the_gates_off() {
        // The test environment sets neither D1_SOCIAL, D1_REWARDS nor D1_STORE.
        for id in [402, 406, 801, 802, 804, 904, 1801, 1802, 1803] {
            assert!(serve(id, &[0; 32], Some(5)).is_none(), "{id}");
        }
    }
}
