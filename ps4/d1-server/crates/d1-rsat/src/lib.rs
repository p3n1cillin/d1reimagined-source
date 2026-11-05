// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! `d1-rsat` - **native RSAT**: the client's schema-driven object encoder, in Rust.
//!
//! Stage 1 of `notes/ps4-56578-native-rsat-plan-2026-11-05.md`. The activity host has run the
//! client's own codec under Unicorn (`d1-oracle`) for every message it sends. That is right as a
//! reference and wrong as a production path: each call holds the connection loop. This crate
//! encodes the same bytes without executing guest code, from the same owned eboot's descriptors:
//!
//! * [`schema`] loads every anchored descriptor once, with the columns the walker reads;
//! * [`encode`] is the walker `0x101C220` and the leaf encoders it dispatches to;
//! * [`decode`] is the decode walker `0x101EB30`, its leaf decoders and the replicated wrapper;
//! * [`bits`] is the client's bitstream accumulator, down to how an overflowing value spills;
//! * [`sensor`] assembles udt 5 `sensor_auth_update` frames around natively encoded objects.
//!
//! `d1-oracle` stays the authority: `tests/equivalence.rs` requires byte- and bit-identical
//! output against it for every activity-host message the host encodes, for generated images, and
//! for decodes of both codecs' bytes. Anything the native walk does not implement returns
//! [`RsatError::Unsupported`], statically per schema, so a caller routes that schema to the oracle
//! by capability and says so; [`RsatError::Runtime`] and [`RsatError::Wire`] are failures of one
//! image or body and are reported, not retried.
//!
//! This crate links no Unicorn and runs no guest code. It still needs the owned eboot: the
//! descriptors are read from it at start-up, never shipped.

pub mod bits;
pub mod build;
pub mod decode;
pub mod encode;
pub mod error;
pub mod schema;
pub mod sensor;

use std::sync::OnceLock;

pub use decode::{Decoded, Decoder};
pub use encode::{Encoded, Encoder};
pub use error::{Result, RsatError};
pub use schema::Schemas;

/// The selected build's schemas, loaded at most once per process.
pub fn schemas() -> Result<&'static Schemas> {
    static S: OnceLock<std::result::Result<Schemas, RsatError>> = OnceLock::new();
    S.get_or_init(Schemas::load_default).as_ref().map_err(Clone::clone)
}

/// Encode one object body with the process-wide schemas.
pub fn encode(tag: u32, image: &[u8]) -> Result<Encoded> {
    Encoder::new(schemas()?).encode(tag, image)
}

/// Encode one replicated full-state body with the process-wide schemas.
pub fn encode_replicated(tag: u32, image: &[u8]) -> Result<Encoded> {
    Encoder::new(schemas()?).encode_replicated(tag, image)
}

/// Decode one object body with the process-wide schemas.
pub fn decode(tag: u32, wire: &[u8]) -> Result<Decoded> {
    Decoder::new(schemas()?).decode(tag, wire)
}

/// Decode one replicated body over `seed` (empty: the zero image) with the process-wide schemas.
pub fn decode_replicated(tag: u32, wire: &[u8], seed: &[u8]) -> Result<Decoded> {
    Decoder::new(schemas()?).decode_replicated(tag, wire, seed)
}

/// The static capability check for one schema tag, with the process-wide schemas.
pub fn check(tag: u32) -> Result<()> {
    schemas()?.check(tag)
}

#[cfg(test)]
pub(crate) mod test_inputs {
    /// Tests that need the owned eboot skip, saying why, when it is absent.
    pub(crate) fn selected_eboot() -> bool {
        let path = d1_eboot::image::default_eboot();
        if std::path::Path::new(&path).is_file() {
            true
        } else {
            eprintln!("SKIP: selected game executable is absent; set D1_EBOOT to an owned image");
            false
        }
    }
}
