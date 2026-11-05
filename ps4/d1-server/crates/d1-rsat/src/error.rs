// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RsatError {
    /// The image could not be read, or a descriptor did not parse.
    Image(String),
    /// The image is a build with no measured native table. Never encode another build's way.
    Build(String),
    /// No anchored descriptor carries this tag.
    UnknownTag(u32),
    /// ⭐ A schema construct the native codec does not implement. Static: the same tag always
    /// refuses, so a caller can route that schema to the client's own codec at start-up, by
    /// capability, and say so.
    Unsupported { tag: u32, field: usize, what: String },
    /// A construct that is implemented, refused for this image: a count past the schema's
    /// capacity, a read past the image, a typed reference naming a type with no descriptor. The
    /// client would read memory it was not handed. A failure, never a reason to fall back.
    Runtime { tag: u32, what: String },
    /// The wire body is malformed for this schema: it ends before the walk does. The client's
    /// decoders reject the same bodies (`bits <= len * 8`).
    Wire { tag: u32, what: String },
    /// A udt 5 frame could not be assembled or walked (its framing, not one of its objects).
    Frame(String),
    /// The image is longer than the struct; the client would never see the excess.
    BadImage { tag: u32, len: usize, struct_size: usize },
}

impl RsatError {
    /// Whether this is the static refusal: the schema uses a construct the native codec does not
    /// implement. Only this one routes a schema to the client's own codec; a [`Self::Runtime`] or
    /// [`Self::Wire`] failure on an implemented schema is reported, never retried through the
    /// oracle (notes/ps4-56578-native-rsat-plan-2026-11-05.md: no synchronous production fallback).
    pub fn is_unsupported(&self) -> bool {
        matches!(self, Self::Unsupported { .. })
    }
}

impl fmt::Display for RsatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Image(e) => write!(f, "native RSAT: image: {e}"),
            Self::Build(e) => write!(f, "native RSAT: {e}"),
            Self::UnknownTag(t) => write!(f, "native RSAT: no descriptor for tag {t:#010x}"),
            Self::Unsupported { tag, field, what } => {
                write!(f, "native RSAT: tag {tag:#010x} field {field}: {what} is not implemented")
            }
            Self::Runtime { tag, what } => write!(f, "native RSAT: tag {tag:#010x}: {what}"),
            Self::Wire { tag, what } => write!(f, "native RSAT: tag {tag:#010x}: wire: {what}"),
            Self::Frame(e) => write!(f, "native RSAT: sensor frame: {e}"),
            Self::BadImage { tag, len, struct_size } => write!(
                f,
                "native RSAT: tag {tag:#010x}: image is {len} bytes, the struct is {struct_size}"
            ),
        }
    }
}

impl std::error::Error for RsatError {}

pub type Result<T> = std::result::Result<T, RsatError>;
