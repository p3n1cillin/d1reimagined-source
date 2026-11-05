// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Talent grids: weapon and armour XP, subclass XP, the grid level they reach, and which nodes
//! can be activated.
//!
//! A grid names a progression; the item's (or subclass's) XP runs through that progression's step
//! costs. The client does the same: `0x284630` subtracts per-level costs from the element's XP
//! (`+0x38`) until it underflows (ps4/design/items-and-gear.md section 2, 01.29 reading). A node
//! is available at its steps' `gridLevel`, once its prerequisites are active and no node it is
//! exclusive with is.

use anyhow::{bail, Result};

use super::placeholders as ph;
use super::store::{CharacterId, InstanceId, RewardStore};
use super::tables::{tables, GridDef, ProgressionDef};
use super::Rng;

/// One node's state: the step it rolled (or was given) and whether it is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NodeState {
    pub step: u16,
    pub active: bool,
}

/// An instance's grid: which grid, the XP put into it, and every node's state (in node order).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridState {
    pub grid: u32,
    pub xp: u32,
    pub nodes: Vec<NodeState>,
}

/// `grid.level`: the level `xp` reaches on `progression`, capped by `max_level` (0: no cap).
pub fn level_for_xp(progression: &ProgressionDef, xp: u32, max_level: u32) -> u32 {
    let steps = &progression.step_costs;
    let mut left = u64::from(xp);
    let mut level = 0u32;
    loop {
        if max_level > 0 && level >= max_level {
            break;
        }
        let cost = match steps.get(level as usize) {
            Some(c) => u64::from(*c),
            None if progression.repeat_last_step => match steps.last() {
                Some(c) if *c > 0 => u64::from(*c),
                _ => break,
            },
            None => break,
        };
        if left < cost {
            break;
        }
        left -= cost;
        level += 1;
    }
    level
}

/// The XP needed to reach `level` (`None` when the progression never gets there).
pub fn xp_for_level(progression: &ProgressionDef, level: u32) -> Option<u32> {
    let steps = &progression.step_costs;
    let mut total = 0u64;
    for l in 0..level {
        let cost = match steps.get(l as usize) {
            Some(c) => *c,
            None if progression.repeat_last_step => *steps.last().filter(|c| **c > 0)?,
            None => return None,
        };
        total += u64::from(cost);
    }
    u32::try_from(total).ok()
}

fn grid_def(grid: u32) -> Result<&'static GridDef> {
    match tables().grids.get(&grid) {
        Some(g) => Ok(g),
        None => bail!("talent grid {grid} is not in the manifest table"),
    }
}

fn progression_of(grid: &GridDef) -> Option<&'static ProgressionDef> {
    tables().progressions.get(&grid.progression)
}

/// Why a node cannot be activated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Blocked {
    NoSuchNode,
    AlreadyActive,
    NeedsLevel(u32),
    NeedsNode(u16),
    ExclusiveWith(u16),
}

impl GridState {
    /// A fresh grid for a new instance: random nodes roll their step (`grid.random_roll`), and
    /// `autoUnlocks` nodes available at level 0 XP are active.
    pub fn new(grid: u32, rng: &mut Rng) -> Result<GridState> {
        let g = grid_def(grid)?;
        let nodes = g
            .nodes
            .iter()
            .map(|n| NodeState { step: if n.is_random && n.steps > 1 { rng.range(0, u32::from(n.steps) - 1) as u16 } else { 0 }, active: false })
            .collect();
        let mut s = GridState { grid, xp: 0, nodes };
        s.apply_auto_unlocks()?;
        Ok(s)
    }

    /// The grid level the state's XP reaches.
    pub fn level(&self) -> Result<u32> {
        let g = grid_def(self.grid)?;
        Ok(progression_of(g).map_or(0, |p| level_for_xp(p, self.xp, g.max_grid_level)))
    }

    pub fn can_activate(&self, node: u16) -> Result<std::result::Result<(), Blocked>> {
        let g = grid_def(self.grid)?;
        let level = self.level()?;
        let Some(def) = g.nodes.iter().find(|n| n.index == node) else { return Ok(Err(Blocked::NoSuchNode)) };
        let state = |i: u16| self.nodes.get(i as usize).copied().unwrap_or_default();
        if state(node).active {
            return Ok(Err(Blocked::AlreadyActive));
        }
        if level < def.grid_level {
            return Ok(Err(Blocked::NeedsLevel(def.grid_level)));
        }
        if let Some(p) = def.prerequisites.iter().find(|p| !state(**p).active) {
            return Ok(Err(Blocked::NeedsNode(*p)));
        }
        if let Some(x) = def.exclusive_with.iter().find(|x| state(**x).active) {
            return Ok(Err(Blocked::ExclusiveWith(*x)));
        }
        Ok(Ok(()))
    }

    /// Activate `node` (a player's choice). Its cost is [`ph::NODE_GLIMMER_COST`] (`grid.node_cost`),
    /// which the caller charges.
    pub fn activate(&mut self, node: u16) -> Result<()> {
        match self.can_activate(node)? {
            Ok(()) => {
                self.nodes[node as usize].active = true;
                Ok(())
            }
            Err(b) => bail!("node {node} of grid {} cannot be activated: {b:?}", self.grid),
        }
    }

    /// Activate every available `autoUnlocks` node; returns the nodes activated, in order.
    pub fn apply_auto_unlocks(&mut self) -> Result<Vec<u16>> {
        let g = grid_def(self.grid)?;
        let mut done = Vec::new();
        loop {
            let next = g.nodes.iter().filter(|n| n.auto_unlocks).find(|n| matches!(self.can_activate(n.index), Ok(Ok(()))));
            match next {
                Some(n) => {
                    self.nodes[n.index as usize].active = true;
                    done.push(n.index);
                }
                None => break,
            }
        }
        Ok(done)
    }

    /// Add XP; returns `(levels gained, nodes auto-activated)`.
    pub fn add_xp(&mut self, xp: u32) -> Result<(u32, Vec<u16>)> {
        let before = self.level()?;
        self.xp = self.xp.saturating_add(xp);
        let gained = self.level()? - before;
        Ok((gained, self.apply_auto_unlocks()?))
    }

    /// Nodes that could be activated now.
    pub fn available(&self) -> Result<Vec<u16>> {
        let g = grid_def(self.grid)?;
        Ok(g.nodes.iter().map(|n| n.index).filter(|i| matches!(self.can_activate(*i), Ok(Ok(())))).collect())
    }
}

/// The order a grid's nodes become available as XP rises: `(node, grid level, XP needed)`, by
/// level, then column, then row. Nodes the progression never reaches are left out.
pub fn unlock_order(grid: u32) -> Result<Vec<(u16, u32, u32)>> {
    let g = grid_def(grid)?;
    let Some(p) = progression_of(g) else { return Ok(Vec::new()) };
    let mut nodes: Vec<_> = g
        .nodes
        .iter()
        .filter(|n| g.max_grid_level == 0 || n.grid_level <= g.max_grid_level)
        .filter_map(|n| xp_for_level(p, n.grid_level).map(|xp| (n.grid_level, n.column, n.row, n.index, xp)))
        .collect();
    nodes.sort();
    Ok(nodes.into_iter().map(|(level, _, _, index, xp)| (index, level, xp)).collect())
}

/// `grid.xp_share`: give `xp` to each equipped item (and subclass) that has a grid. Returns
/// `(instance, levels gained, nodes auto-activated)` for each item that changed.
pub fn share_xp(store: &mut dyn RewardStore, character: CharacterId, xp: u32) -> Result<Vec<(InstanceId, u32, Vec<u16>)>> {
    let share = (u64::from(xp) * u64::from(ph::XP_SHARE_PER_MILLE) / 1000) as u32;
    let mut out = Vec::new();
    for id in store.equipped(character) {
        let Some(inst) = store.item_mut(id) else { continue };
        let Some(grid) = inst.grid.as_mut() else { continue };
        let (levels, nodes) = grid.add_xp(share)?;
        out.push((id, levels, nodes));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prog(steps: &[u32], repeat: bool) -> ProgressionDef {
        ProgressionDef { hash: 0, identifier: String::new(), scope: 0, repeat_last_step: repeat, step_costs: steps.to_vec() }
    }

    #[test]
    fn level_subtracts_step_costs_in_order() {
        let p = prog(&[0, 3570, 4284], true);
        assert_eq!(level_for_xp(&p, 0, 0), 1);
        assert_eq!(level_for_xp(&p, 3569, 0), 1);
        assert_eq!(level_for_xp(&p, 3570, 0), 2);
        assert_eq!(level_for_xp(&p, 3570 + 4284, 0), 3);
        assert_eq!(level_for_xp(&p, 3570 + 4284 * 5, 0), 7);
        assert_eq!(level_for_xp(&p, u32::MAX, 10), 10);
        let once = prog(&[0, 100, 200], false);
        assert_eq!(level_for_xp(&once, 1_000_000, 0), 3);
        for level in 0..12 {
            let xp = xp_for_level(&p, level).unwrap();
            assert_eq!(level_for_xp(&p, xp, 0), level.max(1), "level {level}");
            if xp > 0 {
                assert_eq!(level_for_xp(&p, xp - 1, 0), level - 1);
            }
        }
        assert_eq!(xp_for_level(&once, 4), None);
    }

    #[test]
    fn character_level_40_takes_the_manifest_curve() {
        let p = tables().progressions.values().find(|p| p.identifier == "character_level").unwrap();
        assert_eq!(p.step_costs.len(), 40);
        let total = xp_for_level(p, 40).unwrap();
        assert_eq!(level_for_xp(p, total, 0), 40);
        assert_eq!(level_for_xp(p, total - 1, 0), 39);
    }

    /// The Khvostov's grid order: available nodes appear exactly at their level, and every node
    /// reached has its prerequisites active before it.
    #[test]
    fn grid_unlocks_follow_level_and_prerequisites() {
        let t = tables();
        let mut checked = 0;
        for d in t.items.values().filter(|d| d.talent_grid != 0 && d.instanced).take(400) {
            let g = &t.grids[&d.talent_grid];
            let Some(p) = t.progressions.get(&g.progression) else { continue };
            let order = unlock_order(g.hash).unwrap();
            assert!(order.windows(2).all(|w| w[0].1 <= w[1].1 && w[0].2 <= w[1].2), "grid {} order", g.hash);
            let mut s = GridState::new(g.hash, &mut Rng::new(u64::from(d.hash))).unwrap();
            for (node, level, xp) in order {
                if s.nodes[node as usize].active {
                    continue;
                }
                if xp > s.xp {
                    let below = level_for_xp(p, xp - 1, g.max_grid_level);
                    if below < level {
                        s.xp = xp - 1;
                        assert_eq!(s.can_activate(node).unwrap(), Err(Blocked::NeedsLevel(level)), "grid {} node {node}", g.hash);
                    }
                    s.add_xp(xp - s.xp).unwrap();
                }
                let def = &g.nodes[node as usize];
                match s.can_activate(node).unwrap() {
                    Ok(()) => {
                        assert!(def.prerequisites.iter().all(|q| s.nodes[*q as usize].active));
                        s.activate(node).unwrap();
                        assert!(def.exclusive_with.iter().all(|x| !s.nodes[*x as usize].active));
                    }
                    Err(Blocked::NeedsNode(q)) => assert!(def.prerequisites.contains(&q)),
                    Err(Blocked::ExclusiveWith(x)) => assert!(def.exclusive_with.contains(&x)),
                    Err(b) => panic!("grid {} node {node}: {b:?} at level {}", g.hash, s.level().unwrap()),
                }
            }
            checked += 1;
        }
        assert!(checked > 100, "{checked} grids checked");
    }

    #[test]
    fn random_nodes_roll_within_their_steps() {
        let t = tables();
        let g = t.grids.values().find(|g| g.nodes.iter().any(|n| n.is_random && n.steps > 3)).unwrap();
        let mut seen = std::collections::HashSet::new();
        for seed in 0..200 {
            let s = GridState::new(g.hash, &mut Rng::new(seed)).unwrap();
            for (n, st) in g.nodes.iter().zip(&s.nodes) {
                assert!(st.step < n.steps.max(1));
                if !n.is_random {
                    assert_eq!(st.step, 0);
                } else if n.steps > 3 {
                    seen.insert((n.index, st.step));
                }
            }
        }
        assert!(seen.len() > 3, "random steps never varied");
    }

    #[test]
    fn exclusive_nodes_exclude_each_other() {
        let t = tables();
        // Grids in hash order (the table is a HashMap), and only a pair whose first node the fresh
        // state can activate: a grid's defaults may already hold the pair's other node.
        let mut grids: Vec<_> = t.grids.values().collect();
        grids.sort_by_key(|g| g.hash);
        let (g, a, b, xp) = grids
            .into_iter()
            .find_map(|g| {
                g.nodes.iter().find_map(|n| {
                    let x = *n.exclusive_with.first()?;
                    let other = &g.nodes[x as usize];
                    let p = t.progressions.get(&g.progression)?;
                    let xp = xp_for_level(p, n.grid_level.max(other.grid_level))?;
                    (n.prerequisites.is_empty() && other.prerequisites.is_empty() && !n.auto_unlocks && !other.auto_unlocks
                        && other.exclusive_with.contains(&n.index)
                        && (g.max_grid_level == 0 || n.grid_level.max(other.grid_level) <= g.max_grid_level))
                        .then_some((g, n.index, x, xp))
                        .filter(|&(g, a, _, xp)| {
                            let mut s = GridState::new(g.hash, &mut Rng::new(1)).unwrap();
                            s.xp = xp;
                            matches!(s.can_activate(a), Ok(Ok(())))
                        })
                })
            })
            .unwrap();
        let mut s = GridState::new(g.hash, &mut Rng::new(1)).unwrap();
        s.xp = xp;
        s.activate(a).unwrap();
        assert_eq!(s.can_activate(b).unwrap(), Err(Blocked::ExclusiveWith(a)));
    }
}
