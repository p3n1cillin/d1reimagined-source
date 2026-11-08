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
    /// Kind 0x23's two identity schemas (0x102A450 / 0x102D280).
    pub identity: [(u64, u32); 2],
    pub identity_selector_fn: u64,
    /// Kind 0x26 selector schemas and the lazy tables built by 0x1C0630/0x1C0740.
    pub atom_selectors: [(u64, u32); 2],
    pub atom_primary: &'static [(u64, u32)],
    pub atom_secondary: &'static [(u64, u32)],
    /// Kind 0x0D's decoded fourth SIMD lane (0x102C8D0).
    pub position_w: u64,
    pub quant_half: u64,
}

pub const BUILD_56578: BuildCells = BuildCells {
    name: "later-56578",
    sha256: "672f03411c0503dfbcda804cbb9fb27cdd9a8d73a8bcb71bf0c4f688073b0833",
    typed_ref_handle: (0x199_6050, 0x8080_0046),
    identity: [(0x191_A240, 0x8080_04AC), (0x197_E160, 0x8080_1932)],
    identity_selector_fn: 0x25_CC10,
    atom_selectors: [(0x197_4C00, 0x8080_40F7), (0x197_4BE0, 0x8080_424E)],
    atom_primary: &[
        (0x197_4C18, 0x8080_0547),
        (0x197_4C20, 0x8080_047D),
        (0x197_4C28, 0x8080_0716),
        (0x197_4C30, 0x8080_040B),
        (0x197_4C38, 0x8080_0842),
        (0x197_4C10, 0x8080_0335),
        (0x197_4C40, 0x8080_079F),
        (0x197_4C08, 0x8080_051E),
        (0x197_4C48, 0x8080_0661),
        (0x197_4C50, 0x8080_08D3),
    ],
    atom_secondary: &[
        (0x197_4BE8, 0x8080_07B3),
        (0x197_4BF0, 0x8080_0453),
        (0x197_4BF8, 0x8080_046B),
    ],
    position_w: 0x162_7134,
    quant_half: 0x161_0430,
};

pub fn cells_for(sha256: &str) -> Option<&'static BuildCells> {
    [&BUILD_56578].into_iter().find(|b| b.sha256 == sha256)
}
