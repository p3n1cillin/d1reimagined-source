// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! 801 (activate a talent node) and 802 (change a node's step), on the item's own element.
//!
//! The client sends them from `cui::c_talent_activation_request` slot 3 `0xA43860`, after its own
//! checks: 801 after `0x284E10` (the node exists, its level, prerequisites, exclusive set, the
//! step's requirement list and the cost), 802 after `0x286130` (the node has another step to
//! select and the step's unlock). The server repeats what it can from content:
//!
//! - the grid is the item definition's (`content -> item hash -> talentGridHash`, the manifest's
//!   node table in `rewards::tables`); the node index is the grid's node index;
//! - the grid level from the element's XP (`+0x38`) through the grid's progression;
//! - prerequisites and exclusive sets as `rewards::talent_grid::GridState` checks them;
//! - the cost: `rewards::placeholders::node_cost` (labelled stand-in, one live 801).
//!
//! **Node-state bytes** (`+0x5A`, one per node): written as the client's readers read them
//! (`0x284E10`, `0x285050`): 0 = not active; a fixed node's byte counts its activated steps (the
//! next step to activate is the byte's value); a random node is active with byte 1, its step taken
//! from the step bytes (`+0x7E`, `byte % steps`). ⚠ Decoded from the readers, never seen served:
//! `item.node_state` stays a placeholder until a served item shows it.
//!
//! **802** selects `node` within its exclusive set (D1: switching between unlocked perks is free).
//! The node must have been active before: the server remembers each item's switched-off nodes in
//! the attribute `investment.grid.<instance>` (`{node: byte}`), since the element's bytes cannot
//! say it. The live 802 (#328) spent nothing and changed the item (38 B) and the profile (20 B);
//! what the profile's 20 bytes are is open, so only the item is pushed.

use serde_json::{json, Value};

use super::snapshot::{element as el, Snapshot};
use super::{Refusal, Status};
use crate::rewards::placeholders as ph;
use crate::rewards::tables::{tables, GridDef, ItemDef};
use crate::rewards::talent_grid::{level_for_xp, Blocked, GridState, NodeState};

const SUBCLASS_BUCKET: u32 = 3_284_755_031;
const WEAPON_BUCKETS: [u32; 3] = [1_498_876_634, 2_465_295_065, 953_998_645];

pub(crate) fn item_def(content: u16) -> Option<&'static ItemDef> {
    tables().items.get(&crate::content::item_hash(content)?)
}

fn grid_of(def: &ItemDef) -> Option<&'static GridDef> {
    tables().grids.get(&def.talent_grid)
}

/// The grid state the element's bytes say (random nodes numbered in node order for the step bytes).
pub(crate) fn grid_state(e: &[u8], g: &GridDef) -> GridState {
    let mut random = 0usize;
    let nodes = g.nodes.iter().map(|n| {
        let state = e.get(el::NODE_STATES + n.index as usize).copied().unwrap_or(0);
        let step = if n.is_random && n.steps > 1 {
            let k = random;
            random += 1;
            (e.get(el::STEPS + k % el::STEP_COUNT).copied().unwrap_or(0) as u16) % n.steps
        } else {
            u16::from(state.saturating_sub(1)).min(n.steps.saturating_sub(1))
        };
        NodeState { step, active: state > 0 }
    }).collect();
    GridState { grid: g.hash, xp: el::u32_at(e, el::XP), nodes }
}

/// The 801 cost of a node on this item (labelled stand-in, `grid.node_cost`).
pub fn node_cost(def: &ItemDef) -> Vec<(u32, u32)> {
    use crate::rewards::currencies::{ARMOR_MATERIALS, GLIMMER, WEAPON_PARTS};
    if def.bucket == SUBCLASS_BUCKET {
        return Vec::new();
    }
    let material = if WEAPON_BUCKETS.contains(&def.bucket) { WEAPON_PARTS } else { ARMOR_MATERIALS };
    vec![(GLIMMER, ph::NODE_GLIMMER_COST), (material, ph::NODE_MATERIAL_COST)]
}

/// Charge `costs` (item hashes) from the snapshot; refuse without changing anything if short.
pub(crate) fn charge(s: &mut Snapshot, costs: &[(u32, u32)]) -> Result<(), Refusal> {
    let mut resolved = Vec::new();
    for &(hash, n) in costs {
        let c = crate::content::item_index(hash).ok_or_else(|| Refusal::new(Status::Invalid, format!("cost item {hash} has no 56578 content index")))?;
        if s.quantity(c) < n {
            return Err(Refusal::new(Status::NoFunds, format!("needs {n} of content {c}, has {}", s.quantity(c))));
        }
        resolved.push((c, n));
    }
    for (c, n) in resolved {
        s.take(c, n).map_err(|e| Refusal::new(Status::NoFunds, e))?;
    }
    Ok(())
}

fn grid_attr(item: u64) -> String {
    format!("investment.grid.{item:#x}")
}

struct Target {
    element: Vec<u8>,
    def: &'static ItemDef,
    grid: &'static GridDef,
}

fn target(s: &Snapshot, item: u64) -> Result<Target, Refusal> {
    let content = s.content_of(item).ok_or_else(|| Refusal::new(Status::NotFound, format!("instance {item:#x} is not held")))?;
    let element = s.elements.get(&item).cloned().ok_or_else(|| Refusal::new(Status::NotFound, format!("instance {item:#x} has no element")))?;
    let def = item_def(content).ok_or_else(|| Refusal::new(Status::Invalid, format!("content {content} is not in the item table")))?;
    let grid = grid_of(def).ok_or_else(|| Refusal::new(Status::Invalid, format!("content {content} has no talent grid")))?;
    Ok(Target { element, def, grid })
}

/// 801: activate `node`.
pub fn activate(s: &mut Snapshot, item: u64, node: i8) -> Result<String, Refusal> {
    let t = target(s, item)?;
    let n = u16::try_from(node).ok().and_then(|i| t.grid.nodes.iter().find(|d| d.index == i))
        .ok_or_else(|| Refusal::new(Status::Invalid, format!("grid {} has no node {node}", t.grid.hash)))?;
    let at = el::NODE_STATES + n.index as usize;
    if n.index as usize >= el::NODE_STATE_COUNT {
        return Err(Refusal::new(Status::Invalid, format!("node {} is past the element's 36 node states", n.index)));
    }
    let state = t.element[at];
    let g = grid_state(&t.element, t.grid);
    // A fixed node with more steps takes its next one; anything else must not be active yet.
    let next_step = !n.is_random && state > 0 && u16::from(state) < n.steps;
    if !next_step {
        match g.can_activate(n.index).map_err(|e| Refusal::new(Status::Invalid, e.to_string()))? {
            Ok(()) => {}
            Err(Blocked::NeedsLevel(l)) => return Err(Refusal::new(Status::NotAvailable, format!("node {} needs grid level {l}", n.index))),
            Err(b) => return Err(Refusal::new(Status::NotAvailable, format!("node {}: {b:?}", n.index))),
        }
    } else {
        let level = tables().progressions.get(&t.grid.progression).map_or(0, |p| level_for_xp(p, g.xp, t.grid.max_grid_level));
        if level < n.grid_level {
            return Err(Refusal::new(Status::NotAvailable, format!("node {} needs grid level {}", n.index, n.grid_level)));
        }
    }
    let costs = node_cost(t.def);
    charge(s, &costs)?;
    let mut e = t.element;
    e[at] = if n.is_random { 1 } else { state.saturating_add(1) };
    s.set_element(item, e);
    Ok(format!("node {} of grid {} -> state {} (cost {costs:?}, grid.node_cost stand-in)", n.index, t.grid.hash, if n.is_random { 1 } else { state + 1 }))
}

/// 802: make `node` the selected one of its exclusive set.
pub fn change_step(s: &mut Snapshot, item: u64, node: i8) -> Result<String, Refusal> {
    let t = target(s, item)?;
    let n = u16::try_from(node).ok().and_then(|i| t.grid.nodes.iter().find(|d| d.index == i))
        .ok_or_else(|| Refusal::new(Status::Invalid, format!("grid {} has no node {node}", t.grid.hash)))?;
    if n.index as usize >= el::NODE_STATE_COUNT {
        return Err(Refusal::new(Status::Invalid, format!("node {} is past the element's 36 node states", n.index)));
    }
    let mut kept: serde_json::Map<String, Value> = s.attribute(&grid_attr(item)).and_then(Value::as_object).cloned().unwrap_or_default();
    let mut e = t.element;
    let at = el::NODE_STATES + n.index as usize;
    if e[at] > 0 {
        return Err(Refusal::new(Status::NotAvailable, format!("node {} is already selected", n.index)));
    }
    let Some(byte) = kept.remove(&n.index.to_string()).and_then(|v| v.as_u64()) else {
        return Err(Refusal::new(Status::NotAvailable, format!("node {} was never activated (801 first)", n.index)));
    };
    for &x in &n.exclusive_with {
        let xa = el::NODE_STATES + x as usize;
        if x as usize >= el::NODE_STATE_COUNT || e[xa] == 0 {
            continue;
        }
        kept.insert(x.to_string(), json!(e[xa]));
        e[xa] = 0;
    }
    e[at] = byte.clamp(1, 255) as u8;
    s.set_element(item, e);
    s.set_attribute(&grid_attr(item), Value::Object(kept));
    Ok(format!("node {} selected in grid {} (free)", n.index, t.grid.hash))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::investment::snapshot::tests::{snapshot, with_instance};
    use crate::rewards::currencies::{GLIMMER, WEAPON_PARTS};

    /// A weapon whose grid has a node available at grid level 0 and a pair of exclusive nodes.
    fn weapon() -> Option<(u16, &'static GridDef, u16, (u16, u16))> {
        let t = tables();
        let mut defs: Vec<_> = t.items.values().filter(|d| d.instanced && d.talent_grid != 0 && WEAPON_BUCKETS.contains(&d.bucket)).collect();
        defs.sort_by_key(|d| d.hash);
        defs.into_iter().find_map(|d| {
            let content = crate::content::item_index(d.hash)?;
            crate::content::bucket_of(content)?;
            let g = t.grids.get(&d.talent_grid)?;
            let free = g.nodes.iter().find(|n| !n.is_random && !n.auto_unlocks && n.grid_level == 0 && n.prerequisites.is_empty() && n.exclusive_with.is_empty() && (n.index as usize) < 36)?;
            let pair = g.nodes.iter().find_map(|n| {
                let x = *n.exclusive_with.first()?;
                let o = &g.nodes[x as usize];
                (!n.is_random && !o.is_random && n.prerequisites.is_empty() && o.prerequisites.is_empty() && n.grid_level == 0 && o.grid_level == 0
                    && !n.auto_unlocks && !o.auto_unlocks && (n.index as usize) < 36 && (x as usize) < 36).then_some((n.index, x))
            })?;
            Some((content, g, free.index, pair))
        })
    }

    fn funded(content: u16, soid: u64) -> Snapshot {
        let mut s = snapshot();
        let parts = crate::content::item_index(WEAPON_PARTS).unwrap();
        s.give(crate::content::item_index(GLIMMER).unwrap(), 1_000, 25_000).unwrap();
        s.give(parts, 25, 200).unwrap();
        with_instance(&mut s, content, soid, vec![0; el::BYTES]);
        s.log.clear();
        s
    }

    #[test]
    fn an_801_charges_the_stand_in_and_sets_the_node() {
        let Some((content, _, node, _)) = weapon() else { panic!("no weapon with a free level-0 node in the tables") };
        let mut s = funded(content, 0x401);
        let what = activate(&mut s, 0x401, node as i8).unwrap();
        assert!(what.contains("stand-in"), "{what}");
        assert_eq!(s.quantity(869), 500, "500 Glimmer spent");
        assert_eq!(s.quantity(crate::content::item_index(WEAPON_PARTS).unwrap()), 15, "10 Weapon Parts spent");
        assert_eq!(s.elements[&0x401][el::NODE_STATES + node as usize], 1);
        // The live 801's push: account (Glimmer), profile (materials), then the item.
        assert_eq!(s.log, vec![super::super::snapshot::Touch::Account, super::super::snapshot::Touch::Profile, super::super::snapshot::Touch::Item(0x401)]);
        // Not again (a single-step node), and nothing is spent by the refusal.
        let before = s.clone();
        let r = activate(&mut s, 0x401, node as i8);
        if r.is_err() {
            assert_eq!(s, before);
        }
    }

    #[test]
    fn a_short_wallet_or_a_missing_node_changes_nothing() {
        let Some((content, g, node, _)) = weapon() else { panic!("no weapon") };
        let mut s = snapshot();
        with_instance(&mut s, content, 0x401, vec![0; el::BYTES]);
        let before = s.clone();
        assert_eq!(activate(&mut s, 0x401, node as i8).unwrap_err().status, Status::NoFunds);
        assert_eq!(s, before);
        assert_eq!(activate(&mut s, 0x401, g.nodes.len() as i8 + 1).unwrap_err().status, Status::Invalid);
        assert_eq!(activate(&mut s, 0x999, node as i8).unwrap_err().status, Status::NotFound);
    }

    #[test]
    fn an_802_switches_between_exclusive_nodes_for_free() {
        let Some((content, _, _, (a, b))) = weapon() else { panic!("no weapon") };
        let mut s = funded(content, 0x401);
        activate(&mut s, 0x401, a as i8).unwrap();
        // b is exclusive with the active a: the client would not send an 801 for it, nor do we take one.
        assert!(activate(&mut s, 0x401, b as i8).is_err());
        // b was never active, so it cannot be selected yet.
        assert_eq!(change_step(&mut s, 0x401, b as i8).unwrap_err().status, Status::NotAvailable);
        // Switch a off by selecting it again: refused (it is selected).
        assert!(change_step(&mut s, 0x401, a as i8).is_err());
        // Simulate b unlocked earlier and switched off (the attribute 802 keeps), then switch to it.
        s.set_attribute(&grid_attr(0x401), json!({ b.to_string(): 1 }));
        let glimmer = s.quantity(869);
        s.log.clear();
        change_step(&mut s, 0x401, b as i8).unwrap();
        assert_eq!(s.quantity(869), glimmer, "802 is free");
        let e = &s.elements[&0x401];
        assert_eq!((e[el::NODE_STATES + b as usize], e[el::NODE_STATES + a as usize]), (1, 0));
        assert_eq!(s.log, vec![super::super::snapshot::Touch::Item(0x401)], "the item only");
        // And back to a.
        change_step(&mut s, 0x401, a as i8).unwrap();
        assert_eq!(s.elements[&0x401][el::NODE_STATES + a as usize], 1);
    }

    #[test]
    fn subclass_nodes_are_free() {
        let t = tables();
        let d = t.items.values().find(|d| d.bucket == SUBCLASS_BUCKET && d.talent_grid != 0).unwrap();
        assert!(node_cost(d).is_empty());
    }
}
