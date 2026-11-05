// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! 56578 real32 quantization (0x917C0/0x91850), endpoint-preserving arm (0, 1).
//! Keep each f32 operation separate: the client uses scalar SSE, without fused arithmetic.

fn cvttss2si(v: f32) -> i32 {
    // x86's indefinite result on NaN/overflow, unlike Rust's saturating `as`.
    if !(-2147483648.0..2147483648.0).contains(&v) {
        i32::MIN
    } else {
        v as i32
    }
}

pub(crate) fn encode(v: f32, lo: f32, hi: f32, bits: u32) -> u32 {
    let count = 1i32.wrapping_shl(bits);
    if v == lo {
        return 0;
    }
    if v == hi {
        return count.wrapping_sub(1) as u32;
    }
    let last = count.wrapping_sub(2);
    let step = (hi - lo) / last as f32;
    let code = cvttss2si((v - lo) / step).wrapping_add(1);
    code.max(1).min(last) as u32
}

pub(crate) fn decode(code: u32, lo: f32, hi: f32, bits: u32, half: f32) -> f32 {
    let count = 1i32.wrapping_shl(bits);
    if code == 0 {
        return lo;
    }
    if code == count.wrapping_sub(1) as u32 {
        return hi;
    }
    let step = (hi - lo) / count.wrapping_sub(2) as f32;
    let center = step * half;
    let value = code.wrapping_sub(1) as i32 as f32 * step;
    value + (center + lo)
}
