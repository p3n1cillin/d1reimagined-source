// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Shared by the differential tests: the inputs gate and the schema-driven image generator.
#![allow(dead_code)]

use d1_oracle::activity_host;
use d1_rsat::schema::{Descriptor, Field, Schemas, KIND_COMPOSITE, KIND_TYPED_REF};
use d1_rsat::RsatError;

/// The schemas, or `None` (with the reason printed) when the owned executable is absent or is a
/// build without a measured native table.
pub fn inputs() -> Option<&'static Schemas> {
    let path = d1_eboot::image::default_eboot();
    if !std::path::Path::new(&path).is_file() {
        eprintln!("SKIP: selected game executable is absent; set D1_EBOOT to an owned image");
        return None;
    }
    match d1_rsat::schemas() {
        Ok(s) => Some(s),
        Err(RsatError::Build(why)) => {
            eprintln!("SKIP: {why}");
            None
        }
        Err(e) => panic!("the owned executable is present but its schemas did not load: {e}"),
    }
}

// ── the schema-driven image generator ──────────────────────────────────────────────────────────

pub struct Gen<'a> {
    s: &'a Schemas,
    state: u64,
}

impl<'a> Gen<'a> {
    pub fn new(s: &'a Schemas, seed: u64) -> Self {
        Self { s, state: seed ^ 0x9E37_79B9_7F4A_7C15 }
    }

    pub fn next(&mut self) -> u64 {
        // splitmix64
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next() % n
        }
    }

    /// A count in `0..=max`, biased to the boundaries.
    fn count(&mut self, max: i64) -> i64 {
        match self.below(5) {
            0 => 0,
            1 => max.min(1),
            2 => max,
            _ => self.below(max as u64 + 1) as i64,
        }
    }

    /// One image for `tag`. `raw`: random bytes everywhere before the counts are set.
    pub fn image(&mut self, tag: u32, raw: bool) -> Vec<u8> {
        let d = self.s.get(tag).unwrap();
        let mut img = vec![0u8; d.struct_size as usize];
        if raw {
            for b in img.iter_mut() {
                *b = self.next() as u8;
            }
        } else {
            self.walk(d, &mut img, 0, Pass::Leaves, 0);
        }
        self.walk(d, &mut img, 0, Pass::Counts, 0);
        img
    }

    /// Null every typed reference in `img`, over every array's whole capacity.
    pub fn null_refs(&mut self, tag: u32, img: &mut [u8]) {
        let d = self.s.get(tag).unwrap();
        self.walk(d, img, 0, Pass::Refs, 0);
    }

    fn walk(&mut self, d: &Descriptor, img: &mut [u8], obj: i64, pass: Pass, depth: usize) {
        assert!(depth < 32);
        if let Some(e) = &d.element {
            for i in 0..i64::from(d.capacity) {
                let at = obj + i64::from(e.offset) * i;
                self.field(e, img, obj, at, pass, depth);
            }
        } else {
            for f in &d.fields {
                self.field(f, img, obj, obj + i64::from(f.offset), pass, depth);
            }
        }
    }

    fn field(&mut self, f: &Field, img: &mut [u8], parent: i64, at: i64, pass: Pass, depth: usize) {
        if f.kind == KIND_COMPOSITE {
            let child = self.s.get(f.child).unwrap();
            if pass == Pass::Counts && f.count_source() == 1 {
                let max = if child.is_array() { i64::from(child.capacity) } else { child.fields.len() as i64 };
                let c = self.count(max) as i32;
                put(img, parent + i64::from(f.count_offset()), &c.to_le_bytes());
            }
            self.walk(child, img, at, pass, depth + 1);
            return;
        }
        if f.kind == KIND_TYPED_REF {
            // Null references: a present one names a type the oracle's registry may not hold. The
            // present case has its own test.
            if pass != Pass::Leaves {
                put(img, at, &u32::MAX.to_le_bytes());
            }
            return;
        }
        if pass != Pass::Leaves {
            return;
        }
        let width = f.width().min(64);
        let wire = if width == 64 { self.next() } else { self.below(1u64 << width) };
        let raw = wire.wrapping_sub(u64::from(f.bias()));
        match f.kind {
            2 => put(img, at, &[self.below(2) as u8]),
            3 | 7 => put(img, at, &[raw as u8]),
            4 | 8 => put(img, at, &(raw as u16).to_le_bytes()),
            5 | 9 => put(img, at, &(raw as u32).to_le_bytes()),
            6 | 10 => {
                let raw = wire.wrapping_sub(i64::from(f.bias() as i32) as u64);
                put(img, at, &raw.to_le_bytes())
            }
            11 => put(img, at, &(self.next() as u32).to_le_bytes()),
            0x15..=0x17 => {
                let v = if self.below(3) == 0 { u32::MAX } else { self.next() as u32 };
                put(img, at, &v.to_le_bytes())
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pass {
    Leaves,
    Counts,
    /// Only null every typed reference (handle `-1`), so a hand-made image never names type 0.
    Refs,
}

pub fn put(img: &mut [u8], at: i64, bytes: &[u8]) {
    let at = at as usize;
    if at + bytes.len() <= img.len() {
        img[at..at + bytes.len()].copy_from_slice(bytes);
    }
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Every pinned activity-host udt, by tag, from the oracle's own static derivation.
pub fn ah_schemas() -> Vec<(u32, u32, usize)> {
    activity_host::schemas()
        .as_ref()
        .expect("the AH schemas derive on the owned executable")
        .values()
        .map(|s| (s.user_defined_type, s.tag, s.struct_size))
        .collect()
}

