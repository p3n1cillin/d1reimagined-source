// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The one place stream P touches the player store (stream C's record, made per player by stream
//! O3): [`load`] reads a [`Snapshot`] for the current player (`players::current`: the connection's
//! own, never the knobs' when it has one), [`commit`] writes a handler's changes back and returns
//! the client-encoded objects for the push.
//!
//! Writes go through `PlayerStore` trait methods (`deliver_stack`, `put_item`, `remove_item`,
//! `set_character_attribute`, `add_reputation`, `remove_bounty`, `mint_item_soid`, `save_now`).
//! The profile is the one exception: the trait has no slot writer, so the edited profile object is
//! encoded, pushed, and handed back as delivered (`player_store::note_profile_delivered`), the way
//! every other profile push reaches the store. ⚠ The store adopts it when its decode finishes (a
//! second or two): an op on the same player inside that window starts from the profile before.

use std::collections::BTreeMap;

use d1_bap::investment_ops::Change;
use super::snapshot::{element as el, Snapshot, Touch};

/// The decoded item element template (`D1_VENDOR_ITEM_TEMPLATE`, as a purchase uses), or zeros.
pub fn template() -> Vec<u8> {
    std::env::var_os("D1_VENDOR_ITEM_TEMPLATE").and_then(|p| std::fs::read(p).ok()).filter(|t| t.len() == el::BYTES)
        .unwrap_or_else(|| vec![0; el::BYTES])
}

fn decoded_element(oracle: &str, element: &[u8]) -> Option<Vec<u8>> {
    if element.len() == el::BYTES {
        return Some(element.to_vec());
    }
    crate::vendor::decode_object(oracle, "0x80801732", element, &crate::vendor::unique_name("investment-item"))
        .ok().filter(|e| e.len() == el::BYTES)
}

/// The current player's state, with the elements of `instances` and of every equipped item, and
/// `mint` fresh instance SOIDs reserved. None when the store is off or has no decoded profile.
pub fn load(instances: &[u64], mint: usize) -> anyhow::Result<Snapshot> {
    let names: Vec<String> = crate::tables::tables().progressions.values().map(|p| p.name.clone()).collect();
    let (mut snap, items) = crate::player_store::view(|s| {
        let c = s.chr();
        let a = &s.record.account;
        let snap = Snapshot {
            account_soid: s.record.account_soid,
            character: crate::env_soid("D1_CHARACTER_SOID", 0xD100_0000_0000_0101),
            profile_soid: c.profile_soid,
            class_type: crate::social_hooks::served_character().class.into(),
            profile: c.profile_object.clone().unwrap_or_default(),
            account: a.slots.clone(),
            account_instances: a.slot_instances.clone(),
            attributes: c.attributes.iter().filter(|(k, _)| k.starts_with("investment.")).map(|(k, v)| (k.clone(), v.clone())).collect(),
            bounties: c.bounties.iter().map(|(h, b)| (*h, (b.objectives.clone(), b.completed))).collect(),
            ..Snapshot::default()
        };
        let items: BTreeMap<u64, Vec<u8>> = c.items.iter().map(|i| (i.soid, i.element.clone())).collect();
        (snap, items)
    }).ok_or_else(|| anyhow::anyhow!("the player store is off"))?;
    anyhow::ensure!(!snap.profile.is_empty(), "the store's profile is not decoded yet");
    let oracle = crate::vendor::oracle_path();
    let mut wanted: Vec<u64> = instances.to_vec();
    wanted.extend(snap.equipped());
    for id in wanted {
        if let Some(e) = items.get(&id).and_then(|e| decoded_element(&oracle, e)) {
            snap.elements.insert(id, e);
        }
    }
    crate::player_store::with_store(|s| {
        for n in &names {
            if let Some(r) = s.reputation(n) {
                snap.reputation.insert(n.clone(), r);
            }
        }
        for _ in 0..mint {
            snap.minted.push(s.mint_item_soid());
        }
    });
    Ok(snap)
}

/// The store's curves (bounty XP into the profile).
pub fn curves() -> Option<crate::progression::Curves> {
    crate::player_store::view(|s| s.curves.clone())
}

/// What the push carries, and what to tell the store once it is delivered.
pub struct Committed {
    pub changes: Vec<Change>,
    pub profile: Option<Vec<u8>>,
    pub account: Option<Vec<u8>>,
}

/// Write `after` (a handler's result on `before`) to the store; encode the objects it touched,
/// in its log's order.
pub fn commit(before: &Snapshot, after: &Snapshot, what: &str) -> anyhow::Result<Committed> {
    let oracle = crate::vendor::oracle_path();
    let mut changes = Vec::new();
    let (mut profile, mut account) = (None, None);
    // Encode first: a codec failure leaves the store as it was.
    let mut items = Vec::new();
    for t in &after.log {
        match *t {
            Touch::Account => {
                let refunds = crate::player_store::view(|s| s.record.account.refunds.clone()).unwrap_or_default();
                let p = crate::vendor::account_payload(&oracle, &after.account, &after.account_instances, &refunds)?;
                account = Some(p.clone());
                changes.push(Change::Account(p));
            }
            Touch::Profile => {
                let (p, _) = crate::vendor::encode(&oracle, "0x80801743", &after.profile, &crate::vendor::unique_name("investment-profile"))?;
                profile = Some(p.clone());
                changes.push(Change::Profile { soid: after.profile_soid, payload: p });
            }
            Touch::Item(soid) => {
                let e = after.elements.get(&soid).ok_or_else(|| anyhow::anyhow!("item {soid:#x} has no element"))?;
                let (p, _) = crate::vendor::encode(&oracle, "0x80801732", e, &crate::vendor::unique_name("investment-item"))?;
                items.push((soid, p.clone()));
                changes.push(Change::Item { soid, payload: p });
            }
            Touch::Deleted(soid) => changes.push(Change::Deleted(soid)),
        }
    }
    crate::player_store::with_store(|s| -> anyhow::Result<()> {
        // The account's stacks, by content: what changed between the two snapshots.
        let sum = |m: &BTreeMap<u16, (u16, u32)>| {
            let mut q: BTreeMap<u16, i64> = BTreeMap::new();
            for (c, n) in m.values() {
                *q.entry(*c).or_default() += i64::from(*n);
            }
            q
        };
        let (b, a) = (sum(&before.account), sum(&after.account));
        for c in b.keys().chain(a.keys()).copied().collect::<std::collections::BTreeSet<_>>() {
            let delta = a.get(&c).copied().unwrap_or(0) - b.get(&c).copied().unwrap_or(0);
            if delta != 0 {
                let bucket = crate::content::bucket_of(c).ok_or_else(|| anyhow::anyhow!("content {c} has no bucket"))?;
                s.deliver_stack(c, bucket, delta)?;
            }
        }
        for (soid, p) in &items {
            s.put_item(*soid, p);
        }
        for t in &after.log {
            if let Touch::Deleted(soid) = t {
                s.remove_item(*soid);
            }
        }
        for (k, v) in &after.attributes {
            if before.attributes.get(k) != Some(v) {
                s.set_character_attribute(k, v.clone());
            }
        }
        for (name, n) in &after.reputation_grants {
            s.add_reputation(name, *n);
        }
        for h in &after.bounties_done {
            s.remove_bounty(*h);
        }
        s.save_now(what)
    }).ok_or_else(|| anyhow::anyhow!("the player store is off"))??;
    Ok(Committed { changes, profile, account })
}

/// After the push was written: the store and the vendor state follow the delivered objects.
pub fn delivered(c: &Committed) {
    if let Some(p) = &c.profile {
        crate::vendor::note_profile_pushed(p.clone());
        crate::player_store::note_profile_delivered(p, None);
    }
    if let Some(a) = &c.account {
        crate::player_store::note_account_delivered(a);
    }
}
