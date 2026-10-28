// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Execute the selected 56578 client's base-investment calculation on owned data.
//!
//! A declared zero primary stat is a request to evaluate its level curve. Copying the zero
//! into an appearance table loses that calculation. 0x26F7E0 walks definition +0x50 and
//! delegates each matching row to 0x281A20. Content-interface lookup is substituted and the
//! scoped scalar interpreter corrects the bundled engine's VEX arithmetic. Curve sampling,
//! equipment weighting, scaling and integer conversion still follow the client's instructions.
//! This does not add talent-grid contributions or infer an unimplemented progression rank.

use crate::art_arrangement::base_stat_list;
use crate::character::{self, EMPTY_STAT_KEY, STAT_SLOTS, STAT_STRIDE};
use crate::error::{OracleError, Result};
use crate::harness::{Harness, Stub};
use crate::{inventory, oodle::Content};

pub const STAT_TABLE_TAG: u32 = 0x80A5_E244;
pub const BASE_STAT_WRAPPER: u64 = 0x26F7E0;
pub const PEER_CHARACTER_TABLE: usize = 0x838;

fn shape(s: impl Into<String>) -> OracleError {
    OracleError::Shape(s.into())
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(b[at..at + 2].try_into().unwrap())
}

/// One selected-image engine with the complete owned stat-definition entry in its heap.
pub struct NativeItemStat {
    harness: Harness,
    scalar: crate::scalar_float::ScalarFloat,
    rows: u64,
    row_count: usize,
}

impl NativeItemStat {
    pub fn open(content: &mut Content) -> Result<Self> {
        let (image, mut harness) = Harness::open_default()?;
        if image.sha256() != crate::weapon_stats::MEASURED_EBOOT_SHA256 {
            return Err(shape("base-stat execution is measured only on build 56578"));
        }
        let (_, _, class, data) = content.entry_bytes_for_tag(STAT_TABLE_TAG)?;
        if class != 0x8080_17EC {
            return Err(shape("owned stat-definition entry has the wrong class"));
        }
        let (base, row_count) = stat_rows(&data)?;
        let scalar = crate::scalar_float::ScalarFloat::install(
            harness.uc(),
            &[
                (0x26F7E0, 0x26F8C3),
                (0x281A20, 0x281B99),
                (0x1017530, 0x101774D),
                (0x915E0, 0x915ED),
            ],
        )?;
        let table = harness.alloc(data.len() as u64)?;
        harness.write(table, &data)?;
        let interface = harness.alloc(8)?;
        let vtable = harness.alloc(0x600)?;
        harness.write_u64(interface, vtable)?;
        harness.write_u64(vtable + 0x4C8, 0x25B6E0)?;
        harness.stub(0x25A550, Stub::Return(interface));
        Ok(Self {
            harness,
            scalar,
            rows: table + base as u64,
            row_count,
        })
    }

    /// `None` means this definition declares no contribution for the selected key.
    /// Instance state is required even for an explicit non-zero base row: it prevents a caller
    /// from silently presenting this base-only calculation as a complete rolled/talented item.
    pub fn base_value(
        &mut self,
        definition: &[u8],
        instance: &[u8],
        key: u8,
    ) -> Result<Option<u32>> {
        validate_instance(instance)?;
        if key as usize >= self.row_count {
            return Err(shape(format!(
                "stat key {key} is outside the owned stat-definition table"
            )));
        }
        let pairs = base_stat_list(definition)?.unwrap_or_default();
        let matches = pairs.iter().filter(|(k, _)| *k == key as u32).count();
        if matches == 0 {
            return Ok(None);
        }
        if matches != 1 {
            return Err(shape(
                "duplicate base-stat declarations require explicit investigation",
            ));
        }
        let d = self.harness.alloc(definition.len() as u64)?;
        self.harness.write(d, definition)?;
        let i = self.harness.alloc(instance.len() as u64)?;
        self.harness.write(i, instance)?;
        self.harness
            .stub(0x25B6E0, Stub::Return(self.rows + key as u64 * 16));
        // Rank zero follows from the refused progression/XP state below. Selector zero is the
        // native primary instance level at +0x18. The wrapper supplies its own scalar of 1.
        let result = self
            .harness
            .call(BASE_STAT_WRAPPER, &[d, i, 0, 0, key as u64]);
        self.scalar.check()?;
        let value = result? as u32;
        if (value as i32) < 0 {
            return Err(shape("native base-stat result is negative"));
        }
        Ok(Some(value))
    }
}

#[derive(Debug)]
pub struct DerivedStat {
    pub peer: Vec<u8>,
    pub total: u32,
    /// `(equipment slot, content index, instance SOID, derived base contribution)`.
    pub contributions: Vec<(usize, u16, u64, u32)>,
}

/// Derive one base stat from the peer's actual equipment links, not filename order.
/// Character Light is deliberately excluded: summing item Light is not its aggregation rule.
pub fn derive_peer_stat(
    peer: &[u8],
    inventory_dir: &std::path::Path,
    table: usize,
    key: u8,
    content: &mut Content,
    indices: &crate::IndexTable,
) -> Result<DerivedStat> {
    use crate::art_arrangement;
    use std::collections::BTreeMap;
    if peer.len() != character::PEER_STRUCT_SIZE {
        return Err(shape("expected a peer character"));
    }
    let slots: Vec<usize> = if table == PEER_CHARACTER_TABLE && (1..=7).contains(&key) {
        (0..character::EQUIPMENT_SLOTS).collect()
    } else if let Some(weapon) = crate::weapon_stats::PEER_TABLES
        .iter()
        .position(|base| *base == table)
    {
        vec![character::WEAPON_ORDINALS[weapon]]
    } else {
        return Err(shape(
            "character derivation supports keys 1..7; choose a measured table",
        ));
    };
    let mut instances = BTreeMap::new();
    for item in inventory::read_staged(inventory_dir)? {
        if instances.insert(item.soid, item).is_some() {
            return Err(shape("duplicate inventory instance SOID"));
        }
    }
    let mut calculator = NativeItemStat::open(content)?;
    let mut item_codec = crate::ObjectOracle::new(inventory::TAG)?;
    let mut contributions = Vec::new();
    let mut total = 0u32;
    for slot in slots {
        let at = character::PEER_BLOCK + character::OFF_EQUIPMENT - character::OFF_STATS_BLOCK
            + slot * character::EQUIPMENT_STRIDE;
        let soid = u64::from_le_bytes(peer[at..at + 8].try_into().unwrap());
        let ci = u16_at(peer, at + character::EQUIP_CONTENT_INDEX);
        if ci == character::EMPTY_CONTENT_INDEX || soid == 0 {
            continue;
        }
        let definition = art_arrangement::resolve(content, ci, indices)?;
        if definition.equip_slot != Some(slot as u8) {
            return Err(shape(format!(
                "definition {ci} does not belong to equipped slot {slot}"
            )));
        }
        let (_, _, _, bytes) = content.entry_bytes_for_tag(definition.world_tag)?;
        if !base_stat_list(&bytes)?
            .unwrap_or_default()
            .iter()
            .any(|(k, _)| *k == key as u32)
        {
            continue;
        }
        let item = instances.get(&soid).ok_or_else(|| {
            shape(format!(
                "equipped instance {soid:#x} is absent from inventory"
            ))
        })?;
        if item.content_index != ci {
            return Err(shape(
                "peer and inventory disagree about an equipped content index",
            ));
        }
        let wire = std::fs::read(&item.file).map_err(|e| shape(e.to_string()))?;
        let decoded = item_codec.decode(&wire)?;
        let value = calculator
            .base_value(&bytes, &decoded.object, key)?
            .ok_or_else(|| shape("base declaration disappeared during derivation"))?;
        total = total
            .checked_add(value)
            .ok_or_else(|| shape("derived stat sum overflow"))?;
        contributions.push((slot, ci, soid, value));
    }
    if contributions.is_empty() {
        return Err(shape("no equipped definition declares this base stat"));
    }
    let peer = replace_peer_stat(peer, table, key, total)?;
    Ok(DerivedStat {
        peer,
        total,
        contributions,
    })
}

fn validate_instance(b: &[u8]) -> Result<()> {
    if b.len() != inventory::STRUCT_SIZE {
        return Err(shape("expected a decoded inventory instance"));
    }
    let level = i32::from_le_bytes(
        b[inventory::OFF_LEVEL..inventory::OFF_LEVEL + 4]
            .try_into()
            .unwrap(),
    );
    if !(0..=inventory::MAX_PLAUSIBLE_LEVEL).contains(&level) {
        return Err(shape("instance level is outside the established bound"));
    }
    if u16_at(b, inventory::OFF_TALENT_GRID) != inventory::EMPTY_INDEX
        || b[inventory::OFF_NODE_STATES..inventory::OFF_FLAG_86]
            .iter()
            .any(|v| *v != 0)
    {
        return Err(shape(
            "base-only calculation refuses active or undecoded talent state",
        ));
    }
    if u16_at(b, inventory::OFF_PROGRESSION_ORDINAL) != inventory::EMPTY_INDEX
        || b[inventory::OFF_PROGRESS_POINTS..inventory::OFF_PROGRESSION_ORDINAL]
            .iter()
            .any(|v| *v != 0)
    {
        return Err(shape(
            "native progression rank must be derived before calculating this instance",
        ));
    }
    Ok(())
}

fn stat_rows(b: &[u8]) -> Result<(usize, usize)> {
    if b.len() < 0x30 || u64::from_le_bytes(b[..8].try_into().unwrap()) != b.len() as u64 {
        return Err(shape("invalid stat-definition entry length"));
    }
    let count = i64::from_le_bytes(b[8..16].try_into().unwrap());
    let rel = i64::from_le_bytes(b[16..24].try_into().unwrap());
    let base = 16i64
        .checked_add(rel)
        .and_then(|v| v.checked_add(16))
        .ok_or_else(|| shape("stat-definition row offset overflow"))?;
    if count != 36
        || base < 0
        || (base as usize)
            .checked_add(count as usize * 16)
            .is_none_or(|end| end > b.len())
    {
        return Err(shape("wrong stat-definition count or escaping rows"));
    }
    let base = base as usize;
    for (key, hash) in [(0, 2_391_494_160u32), (1, 3_897_883_278), (11, 368_428_387)] {
        let at = base + key * 16;
        if u32::from_le_bytes(b[at..at + 4].try_into().unwrap()) != hash {
            return Err(shape("stat-definition row identity mismatch"));
        }
    }
    Ok((base, count as usize))
}

/// Update or append one packed key, preserving every unrelated entry and decoded byte.
pub fn replace_peer_stat(peer: &[u8], table: usize, key: u8, value: u32) -> Result<Vec<u8>> {
    let allowed = [PEER_CHARACTER_TABLE, 0x8B8, 0x938, 0x9B8];
    if peer.len() != character::PEER_STRUCT_SIZE || !allowed.contains(&table) || key >= 64 {
        return Err(shape("invalid peer, table or stat key"));
    }
    let matches: Vec<_> = (0..STAT_SLOTS)
        .filter(|i| peer[table + i * STAT_STRIDE] == key)
        .collect();
    if matches.len() > 1 {
        return Err(shape(
            "duplicate existing packed keys would overwrite the replacement",
        ));
    }
    let index = matches
        .first()
        .copied()
        .or_else(|| (0..STAT_SLOTS).find(|i| peer[table + i * STAT_STRIDE] == EMPTY_STAT_KEY))
        .ok_or_else(|| shape("packed stat table has no free slot"))?;
    let mut out = peer.to_vec();
    let at = table + index * STAT_STRIDE;
    out[at..at + STAT_STRIDE].fill(0);
    out[at] = key;
    out[at + 4..at + 8].copy_from_slice(&value.to_le_bytes());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacing_one_key_keeps_the_other_three_tables_and_light() {
        let mut before = vec![0x35; character::PEER_STRUCT_SIZE];
        for i in 0..STAT_SLOTS {
            before[PEER_CHARACTER_TABLE + i * 8] = EMPTY_STAT_KEY;
        }
        before[PEER_CHARACTER_TABLE] = 0;
        before[PEER_CHARACTER_TABLE + 4..PEER_CHARACTER_TABLE + 8]
            .copy_from_slice(&385u32.to_le_bytes());
        let after = replace_peer_stat(&before, PEER_CHARACTER_TABLE, 1, 37).unwrap();
        let slot = PEER_CHARACTER_TABLE + 8;
        assert_eq!(&after[..slot], &before[..slot]);
        assert_eq!(&after[slot + 8..], &before[slot + 8..]);
        assert_eq!(
            replace_peer_stat(&after, PEER_CHARACTER_TABLE, 1, 37).unwrap(),
            after
        );
    }

    #[test]
    fn duplicate_and_full_tables_are_refused() {
        let mut peer = vec![0; character::PEER_STRUCT_SIZE];
        assert!(replace_peer_stat(&peer, PEER_CHARACTER_TABLE, 0, 4).is_err());
        for i in 0..STAT_SLOTS {
            peer[PEER_CHARACTER_TABLE + i * 8] = i as u8;
        }
        assert!(replace_peer_stat(&peer, PEER_CHARACTER_TABLE, 20, 4).is_err());
        assert!(replace_peer_stat(&peer, PEER_CHARACTER_TABLE, 64, 4).is_err());
        assert!(replace_peer_stat(&peer, 0, 1, 4).is_err());
    }

    #[test]
    fn unknown_talents_and_progression_cannot_be_silently_ignored() {
        let mut item = vec![0; inventory::STRUCT_SIZE];
        item[inventory::OFF_TALENT_GRID..inventory::OFF_TALENT_GRID + 2].fill(0xFF);
        item[inventory::OFF_PROGRESSION_ORDINAL..inventory::OFF_PROGRESSION_ORDINAL + 2].fill(0xFF);
        assert!(validate_instance(&item).is_ok());
        item[inventory::OFF_NODE_STATES] = 1;
        assert!(validate_instance(&item).is_err());
        item[inventory::OFF_NODE_STATES] = 0;
        item[inventory::OFF_PROGRESS_POINTS] = 1;
        assert!(validate_instance(&item).is_err());
        assert!(stat_rows(&[0; 16]).is_err());
    }
}
