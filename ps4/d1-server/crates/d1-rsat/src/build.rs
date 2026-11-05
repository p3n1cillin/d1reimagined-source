// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The few **start-up type cells** a native encode reads, per build.
//!
//! Some leaf encoders name a schema through a pointer global that the game fills while it starts;
//! the file holds zero there, so the tag cannot be read statically. These are the cells
//! `d1_oracle::codec::CODEC_56578.type_cells` records as read live on 56578, restricted to the
//! ones the native encoder implements. A test (`tests/equivalence.rs`) requires each row to equal
//! the oracle's, so the two tables cannot drift.

/// One build's cells.
#[derive(Debug)]
pub struct BuildCells {
    pub name: &'static str,
    pub sha256: &'static str,
    /// Kind `0x29` (`0x102A650`): `mov rax,[0x1996050]; mov edi,[rax]` - the schema of the
    /// reference's `u32` handle. `(pointer slot, tag)`.
    pub typed_ref_handle: (u64, u32),
}

pub const BUILD_56578: BuildCells = BuildCells {
    name: "later-56578",
    sha256: "672f03411c0503dfbcda804cbb9fb27cdd9a8d73a8bcb71bf0c4f688073b0833",
    typed_ref_handle: (0x199_6050, 0x8080_0046),
};

pub fn cells_for(sha256: &str) -> Option<&'static BuildCells> {
    [&BUILD_56578].into_iter().find(|b| b.sha256 == sha256)
}
