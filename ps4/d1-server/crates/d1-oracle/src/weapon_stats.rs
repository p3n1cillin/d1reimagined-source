// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Packed weapon investment stats in the peer character's shared appearance block.
//!
//! Build 56578: 0x54DFF0 copies the peer block (+0x30), and 0x912240 supplies its three
//! weapon tables to 0x910C60. 0x28A2F0 expands 16 signed-byte keys into 64 u32 slots;
//! only key -1 is absent. These are separate from inventory instance +0x88/+0x98.

use std::collections::BTreeSet;

use crate::character::{self, EMPTY_CONTENT_INDEX, EMPTY_STAT_KEY, STAT_SLOTS, STAT_STRIDE};
use crate::error::{OracleError, Result};

/// Primary, special, heavy, in the same order as the equipment ordinals.
pub const PEER_TABLES: [usize; 3] = [0x8B8, 0x938, 0x9B8];
pub const MEASURED_EBOOT_SHA256: &str =
    "672f03411c0503dfbcda804cbb9fb27cdd9a8d73a8bcb71bf0c4f688073b0833";

pub fn equipped_index(peer: &[u8], slot: usize) -> Result<u16> {
    if peer.len() != character::PEER_STRUCT_SIZE || slot >= PEER_TABLES.len() {
        return Err(OracleError::Shape(
            "expected a peer character and weapon slot 0..2".into(),
        ));
    }
    let at = character::PEER_BLOCK + character::OFF_EQUIPMENT - character::OFF_STATS_BLOCK
        + character::WEAPON_ORDINALS[slot] * character::EQUIPMENT_STRIDE
        + character::EQUIP_CONTENT_INDEX;
    let index = u16::from_le_bytes(peer[at..at + 2].try_into().unwrap());
    if index == EMPTY_CONTENT_INDEX {
        return Err(OracleError::Shape(
            "the selected weapon slot is empty".into(),
        ));
    }
    Ok(index)
}

/// Replace exactly one table; unused entries must be FF so trailing zeroes cannot erase stat 0.
/// Reject out-of-range and duplicate keys before narrowing to the client's signed-byte key.
pub fn patch_peer(peer: &[u8], slot: usize, stats: &[(u32, u32)]) -> Result<Vec<u8>> {
    equipped_index(peer, slot)?;
    if stats.is_empty() || stats.len() > STAT_SLOTS {
        return Err(OracleError::Shape(format!(
            "weapon needs 1..={STAT_SLOTS} base stats"
        )));
    }
    let mut seen = BTreeSet::new();
    for &(key, _) in stats {
        if key >= 64 || !seen.insert(key) {
            return Err(OracleError::Shape(format!(
                "invalid or duplicate weapon stat key {key}"
            )));
        }
    }
    let base = PEER_TABLES[slot];
    let mut out = peer.to_vec();
    out[base..base + STAT_SLOTS * STAT_STRIDE].fill(0);
    for i in 0..STAT_SLOTS {
        out[base + i * STAT_STRIDE] = EMPTY_STAT_KEY;
    }
    for (i, &(key, value)) in stats.iter().enumerate() {
        let at = base + i * STAT_STRIDE;
        out[at] = key as u8;
        out[at + 4..at + 8].copy_from_slice(&value.to_le_bytes());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer() -> Vec<u8> {
        // Synthetic structure; no owned game bytes in this test.
        vec![0x35; character::PEER_STRUCT_SIZE]
    }

    #[test]
    fn each_slot_changes_only_its_table_and_retains_zero_stat() {
        let before = peer();
        for (slot, &base) in PEER_TABLES.iter().enumerate() {
            let after = patch_peer(&before, slot, &[(0, 37), (20, 44)]).unwrap();
            assert_eq!(&after[..base], &before[..base]);
            assert_eq!(&after[base + 128..], &before[base + 128..]);
            let mut flat = [0; 64];
            for row in after[base..base + 128].chunks_exact(8) {
                if row[0] != EMPTY_STAT_KEY {
                    flat[row[0] as usize] = u32::from_le_bytes(row[4..8].try_into().unwrap());
                }
            }
            assert_eq!((flat[0], flat[20]), (37, 44));
            assert_eq!(after[base + 16], EMPTY_STAT_KEY);
        }
    }

    #[test]
    fn refuses_unsafe_keys_and_shapes() {
        for stats in [
            vec![],
            vec![(64, 1)],
            vec![(255, 1)],
            vec![(0, 1), (0, 2)],
            (0..17).map(|key| (key, 1)).collect(),
        ] {
            assert!(patch_peer(&peer(), 0, &stats).is_err());
        }
        assert!(patch_peer(&peer(), 3, &[(20, 44)]).is_err());
        assert!(patch_peer(&[0; 16], 0, &[(20, 44)]).is_err());
        let mut empty = peer();
        let at = 0x30 + 0x1E8 + 6 * 64 + 8;
        empty[at..at + 2].fill(0xFF);
        assert!(patch_peer(&empty, 0, &[(20, 44)]).is_err());
    }
}
