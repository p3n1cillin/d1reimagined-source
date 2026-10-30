// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Offline, observational decoder for build 56578's raw udt 6 `sensor_sense_update`.
//!
//! The envelope was traced from the native sender `0x299CB0`, through its caller
//! `0x2D0500`, to writer `0x2D0640`. Names and replicated B bodies are decoded by
//! the owned client's existing codecs. B tags must come from native sensor-client
//! metadata; the envelope does not carry them. See `notes/ps4-sensor-sense.md`.
//!
//! Cached images are **observed overlays**, not authoritative snapshots. A capture
//! can omit the baseline and intermediate updates. Neither a zero byte nor a field
//! change establishes a kill. This module does not drive activity-host progression.

use std::collections::HashMap;

use serde::Serialize;

use crate::error::{OracleError, Result};
use crate::object::{Decoded, ObjectOracle};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct SenseName {
    pub group: u32,
    pub kind: u8,
    pub index: i16,
}

#[derive(Debug, Serialize)]
pub struct SenseEntry {
    pub name: SenseName,
    pub tag: u32,
    pub revision: u32,
    pub body_bits: u32,
    /// True only if this decoder has an earlier observation of this name in this generation.
    /// It does not establish that the capture contains the sender's complete baseline.
    pub reused_observation: bool,
    pub changed_offsets: Vec<usize>,
    pub image: Vec<u8>,
}

#[derive(Debug, Serialize)]
pub struct SenseFrame {
    /// `!sender_state[0x62B]`, not part of the generation byte. Meaning remains unpromoted.
    pub initial_flag: bool,
    pub generation: u8,
    pub padding_bits: usize,
    pub entries: Vec<SenseEntry>,
}

#[derive(Clone, Default)]
struct Observations {
    generation: Option<u8>,
    images: HashMap<SenseName, Vec<u8>>,
}

trait SenseCodec {
    fn name(&mut self, payload: &[u8]) -> Result<Decoded>;
    fn size(&mut self, tag: u32) -> Result<usize>;
    fn body(&mut self, tag: u32, payload: &[u8], seed: &[u8]) -> Result<Decoded>;
}

struct NativeCodec {
    name: ObjectOracle,
    bodies: HashMap<u32, ObjectOracle>,
}

impl NativeCodec {
    fn oracle(&mut self, tag: u32) -> Result<&mut ObjectOracle> {
        if let std::collections::hash_map::Entry::Vacant(entry) = self.bodies.entry(tag) {
            entry.insert(ObjectOracle::with_types(
                tag,
                crate::sensor::TYPED_VALUE_TYPES,
            )?);
        }
        Ok(self.bodies.get_mut(&tag).unwrap())
    }
}

impl SenseCodec for NativeCodec {
    fn name(&mut self, payload: &[u8]) -> Result<Decoded> {
        self.name.decode(payload)
    }
    fn size(&mut self, tag: u32) -> Result<usize> {
        Ok(self.oracle(tag)?.struct_size())
    }
    fn body(&mut self, tag: u32, payload: &[u8], seed: &[u8]) -> Result<Decoded> {
        self.oracle(tag)?.decode_replicated(payload, seed)
    }
}

/// One ordered capture from one connection. Construct a new instance for another run.
pub struct SenseDecoder {
    /// Explicit `kind -> B tag` observations, supplied by the caller, never inferred from payload.
    schemas: HashMap<u8, u32>,
    codec: NativeCodec,
    observations: Observations,
}

impl SenseDecoder {
    pub fn new(schemas: HashMap<u8, u32>) -> Result<Self> {
        if d1_build::selected().id != "later-56578" {
            return Err(OracleError::Shape(
                "udt 6 framing is measured only for later-56578".into(),
            ));
        }
        if schemas.is_empty() {
            return Err(OracleError::Shape(
                "supply native-observed kind:B-tag mappings".into(),
            ));
        }
        Ok(Self {
            schemas,
            codec: NativeCodec {
                name: ObjectOracle::new(crate::sensor::TAG_NAME_REF)?,
                bodies: HashMap::new(),
            },
            observations: Observations::default(),
        })
    }

    /// Decode transactionally. A bad packet does not commit any of its overlays.
    pub fn decode(&mut self, payload: &[u8]) -> Result<SenseFrame> {
        decode_frame(
            payload,
            &self.schemas,
            &mut self.codec,
            &mut self.observations,
        )
    }
}

struct Bits<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl Bits<'_> {
    fn read(&mut self, count: usize) -> Result<u64> {
        self.require(count)?;
        let mut value = 0;
        for bit in self.position..self.position + count {
            value = (value << 1) | u64::from((self.bytes[bit / 8] >> (7 - bit % 8)) & 1);
        }
        self.position += count;
        Ok(value)
    }

    fn require(&self, count: usize) -> Result<()> {
        if count > self.bytes.len() * 8 - self.position {
            return Err(OracleError::Shape(format!(
                "truncated udt 6 at bit {}: need {count} bits",
                self.position
            )));
        }
        Ok(())
    }

    /// The native codec reads from a byte boundary; retain the enclosing bit position ourselves.
    fn remainder(&self) -> Vec<u8> {
        let count = self.bytes.len() * 8 - self.position;
        let mut output = vec![0; count.div_ceil(8)];
        for bit in 0..count {
            let source = self.position + bit;
            output[bit / 8] |= ((self.bytes[source / 8] >> (7 - source % 8)) & 1) << (7 - bit % 8);
        }
        output
    }

    fn advance(&mut self, count: u32) -> Result<()> {
        self.require(count as usize)?;
        self.position += count as usize;
        Ok(())
    }
}

fn decode_frame<C: SenseCodec>(
    payload: &[u8],
    schemas: &HashMap<u8, u32>,
    codec: &mut C,
    observations: &mut Observations,
) -> Result<SenseFrame> {
    let mut bits = Bits {
        bytes: payload,
        position: 0,
    };
    let initial_flag = bits.read(1)? != 0;
    let generation = bits.read(8)? as u8;
    if bits.read(1)? != 0 {
        return Err(OracleError::Shape("udt 6 reserved bit 9 is nonzero".into()));
    }
    let mut pending = if observations.generation == Some(generation) {
        observations.clone()
    } else {
        Observations {
            generation: Some(generation),
            images: HashMap::new(),
        }
    };
    let mut entries = Vec::new();
    while bits.read(1)? != 0 {
        bits.require(55)?;
        let name = codec.name(&bits.remainder())?;
        if name.bits != 55 || name.object.len() < 8 {
            return Err(OracleError::Shape(
                "udt 6 name did not decode to the measured 55-bit reference".into(),
            ));
        }
        bits.advance(name.bits)?;
        let name = SenseName {
            group: u32::from_le_bytes(name.object[..4].try_into().unwrap()),
            kind: name.object[4],
            index: i16::from_le_bytes(name.object[6..8].try_into().unwrap()),
        };
        let tag = *schemas.get(&name.kind).ok_or_else(|| {
            OracleError::Shape(format!(
                "no native-observed B tag for kind {:#x} ({:#x}/{})",
                name.kind, name.group, name.index
            ))
        })?;
        let size = codec.size(tag)?;
        let reused_observation = pending.images.contains_key(&name);
        let seed = pending
            .images
            .get(&name)
            .cloned()
            .unwrap_or_else(|| vec![0; size]);
        let decoded = codec.body(tag, &bits.remainder(), &seed)?;
        if decoded.bits == 0 || decoded.object.len() != size {
            return Err(OracleError::Shape(
                "invalid udt 6 replicated body shape".into(),
            ));
        }
        bits.advance(decoded.bits)?;
        let revision = bits.read(32)? as u32;
        let changed_offsets = seed
            .iter()
            .zip(&decoded.object)
            .enumerate()
            .filter_map(|(offset, (before, after))| (before != after).then_some(offset))
            .collect();
        pending.images.insert(name, decoded.object.clone());
        entries.push(SenseEntry {
            name,
            tag,
            revision,
            body_bits: decoded.bits,
            reused_observation,
            changed_offsets,
            image: decoded.object,
        });
    }
    let padding_bits = payload.len() * 8 - bits.position;
    if padding_bits >= 8 || bits.read(padding_bits)? != 0 {
        return Err(OracleError::Shape(
            "udt 6 has extra bytes or nonzero trailing padding".into(),
        ));
    }
    *observations = pending;
    Ok(SenseFrame {
        initial_flag,
        generation,
        padding_bits,
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Deliberately synthetic codecs. These tests check envelope bounds and overlay isolation;
    // native codec validation is separate and requires the user's owned build.
    struct FakeCodec;
    impl SenseCodec for FakeCodec {
        fn name(&mut self, payload: &[u8]) -> Result<Decoded> {
            let mut bits = Bits {
                bytes: payload,
                position: 0,
            };
            let mut object = vec![0; 8];
            object[0] = bits.read(8)? as u8;
            object[4] = 1;
            Ok(Decoded {
                tag: 0,
                bits: 55,
                object,
            })
        }
        fn size(&mut self, _: u32) -> Result<usize> {
            Ok(2)
        }
        fn body(&mut self, tag: u32, payload: &[u8], seed: &[u8]) -> Result<Decoded> {
            let mut bits = Bits {
                bytes: payload,
                position: 0,
            };
            let mut object = seed.to_vec();
            if bits.read(1)? != 0 {
                object[0] = bits.read(8)? as u8;
            }
            Ok(Decoded {
                tag,
                bits: bits.position as u32,
                object,
            })
        }
    }
    fn push(bits: &mut Vec<u8>, value: u64, width: usize) {
        bits.extend((0..width).rev().map(|i| ((value >> i) & 1) as u8));
    }
    fn frame(generation: u8, entries: &[(u8, Option<u8>, u32)]) -> Vec<u8> {
        let mut bits = vec![1];
        push(&mut bits, u64::from(generation), 8);
        bits.push(0);
        for &(name, value, revision) in entries {
            bits.push(1);
            push(&mut bits, u64::from(name), 8);
            push(&mut bits, 0, 47);
            bits.push(u8::from(value.is_some()));
            if let Some(value) = value {
                push(&mut bits, u64::from(value), 8);
            }
            push(&mut bits, u64::from(revision), 32);
        }
        bits.push(0);
        let mut bytes = vec![0; bits.len().div_ceil(8)];
        for (i, bit) in bits.into_iter().enumerate() {
            bytes[i / 8] |= bit << (7 - i % 8);
        }
        bytes
    }
    fn decode(bytes: &[u8], state: &mut Observations) -> Result<SenseFrame> {
        decode_frame(bytes, &HashMap::from([(1, 7)]), &mut FakeCodec, state)
    }
    #[test]
    fn multiple_entries_and_overlay_are_isolated_by_name_and_generation() {
        let mut state = Observations::default();
        let first = decode(&frame(3, &[(8, Some(12), 1), (9, Some(41), 7)]), &mut state).unwrap();
        assert_eq!(first.entries.len(), 2);
        assert!(first.initial_flag);
        assert_eq!(first.generation, 3);
        let next = decode(&frame(3, &[(8, None, 2), (9, None, 8)]), &mut state).unwrap();
        assert_eq!(next.entries[0].image, [12, 0]);
        assert_eq!(next.entries[1].image, [41, 0]);
        assert!(next.entries[0].reused_observation);
        assert!(next.entries[0].changed_offsets.is_empty());
        let changed = decode(&frame(4, &[(8, None, 1)]), &mut state).unwrap();
        assert_eq!(changed.entries[0].image, [0, 0]);
        assert!(!changed.entries[0].reused_observation);
    }
    #[test]
    fn malformed_packet_does_not_commit_a_partial_overlay() {
        let mut state = Observations::default();
        decode(&frame(0, &[(8, Some(12), 1)]), &mut state).unwrap();
        let mut bad = frame(0, &[(8, Some(99), 2), (9, Some(6), 1)]);
        bad.truncate(bad.len() - 2);
        assert!(decode(&bad, &mut state).is_err());
        let next = decode(&frame(0, &[(8, None, 3)]), &mut state).unwrap();
        assert_eq!(next.entries[0].image, [12, 0]);
    }
    #[test]
    fn rejects_missing_schema_reserved_bit_extra_bytes_and_dirty_padding() {
        let mut state = Observations::default();
        let valid = frame(0, &[(8, Some(12), 1)]);
        assert!(decode_frame(&valid, &HashMap::new(), &mut FakeCodec, &mut state).is_err());
        let mut reserved = valid.clone();
        reserved[1] |= 0x40;
        assert!(decode(&reserved, &mut state).is_err());
        let mut extra = valid.clone();
        extra.push(0);
        assert!(decode(&extra, &mut state).is_err());
        let mut dirty = valid;
        *dirty.last_mut().unwrap() |= 1;
        assert!(decode(&dirty, &mut state).is_err());
    }
    #[test]
    fn rejects_every_truncated_prefix() {
        let packet = frame(0, &[(8, Some(12), 1)]);
        for end in 0..packet.len() {
            assert!(
                decode(&packet[..end], &mut Observations::default()).is_err(),
                "prefix {end}"
            );
        }
    }
}
