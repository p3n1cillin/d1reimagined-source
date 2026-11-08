// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! ⭐ **Native RSAT behind the codec trait**, opt-in with `D1_RSAT_NATIVE=1`.
//!
//! [`NativeCodec`] is an [`AhWireCodec`]: a drop-in for [`SchemaCodec`] that encodes and decodes
//! every activity-host message whose schema closure `d1-rsat` covers **without running guest
//! code**, and hands the rest to [`SchemaCodec`] (the client's own codec under Unicorn). The choice
//! is by **schema capability**, made the same way for every message of a udt: a static
//! [`d1_rsat::RsatError::Unsupported`] routes the udt to the oracle, logged once; a native failure
//! on a covered schema (a count past a capacity, a short body) is reported as an error and never
//! retried through the oracle (notes/ps4-56578-native-rsat-plan-2026-11-05.md, stage 1).
//!
//! udt 5 frames go the same way: [`try_sensor_frame`] assembles a `sensor_auth_update` natively
//! when every value it carries has a covered schema, and records the frame's stamp offset so a
//! connection loop can restamp it without decoding it. [`SchemaCodec::build_sensor`] calls it
//! first when the knob is set.
//!
//! The byte-identity evidence is `d1-rsat`'s differential tests against `d1-oracle` (every pinned
//! udt, every fixture sensor value, whole frames, served profiles and items). With the knob unset
//! nothing here runs and the host is exactly as before.

use std::collections::BTreeSet;
use std::sync::{Mutex, OnceLock};

use crate::codec::{AhWireCodec, SchemaCodec};
use crate::messages::AhMessage;
use crate::AhError;

/// The knob's name.
pub const KNOB: &str = "D1_RSAT_NATIVE";

/// Whether `D1_RSAT_NATIVE=1` is set (read once per process).
pub fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var(KNOB).is_ok_and(|v| v.trim() == "1"))
}

/// The native schemas for this process's eboot, or why there are none (another build, no image).
fn schemas() -> Result<&'static d1_rsat::Schemas, String> {
    d1_rsat::schemas().map_err(|e| e.to_string())
}

/// Say once per (udt, direction) that it is on the client's codec, and why.
fn note_oracle(udt: u32, dir: &str, why: &str) {
    static SEEN: Mutex<BTreeSet<(u32, String)>> = Mutex::new(BTreeSet::new());
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    if seen.insert((udt, dir.to_string())) {
        tracing::info!("native RSAT: udt {udt} {dir} stays on the client's codec: {why}");
    }
}

fn failed(udt: u32, e: d1_rsat::RsatError) -> AhError {
    AhError::SchemaCodec { user_defined_type: udt, detail: e.to_string() }
}

/// How each pinned udt is served under the knob, for the start-up log:
/// `(udt, encode native?, decode native?, reason when not)`.
pub fn coverage() -> Vec<(u32, bool, bool, String)> {
    let Ok(s) = schemas() else { return Vec::new() };
    let Ok(all) = d1_oracle::activity_host::schemas().as_ref() else { return Vec::new() };
    all.values()
        .map(|a| {
            let e = s.check(a.tag);
            let d = s.check_decode(a.tag);
            let why = e.as_ref().err().or(d.as_ref().err()).map(|e| e.to_string()).unwrap_or_default();
            (a.user_defined_type, e.is_ok(), d.is_ok(), why)
        })
        .collect()
}

/// ⭐ The drop-in. A unit struct like [`SchemaCodec`], so `Arc::new(NativeCodec)` replaces it.
#[derive(Debug, Default, Clone, Copy)]
pub struct NativeCodec;

impl NativeCodec {
    /// The schema tag a udt is coded with, when the native codec covers it in `decode` or encode
    /// direction; `None` routes the udt to [`SchemaCodec`].
    fn native_tag(udt: u32, decode: bool) -> Option<(&'static d1_rsat::Schemas, u32)> {
        let dir = if decode { "decode" } else { "encode" };
        let s = match schemas() {
            Ok(s) => s,
            Err(why) => {
                note_oracle(udt, dir, &why);
                return None;
            }
        };
        // An unpinned udt has no tag: the client's codec gives the named `SchemaCodecMissing`.
        let tag = d1_oracle::activity_host::tag_for_udt(udt).ok()?;
        let cap = if decode { s.check_decode(tag) } else { s.check(tag) };
        match cap {
            Ok(()) => Some((s, tag)),
            Err(e) => {
                note_oracle(udt, dir, &e.to_string());
                None
            }
        }
    }
}

impl AhWireCodec for NativeCodec {
    fn name(&self) -> &'static str {
        "native RSAT (d1-rsat, no guest code), the client's own codec for schemas it does not cover"
    }

    fn encode(&self, msg: &AhMessage) -> Result<Vec<u8>, AhError> {
        let udt = msg.user_defined_type();
        crate::protocol::check_sendable(udt)?;
        if let AhMessage::SensorAuthUpdate { payload } = msg {
            return Ok(payload.clone());
        }
        let Some((s, tag)) = Self::native_tag(udt, false) else { return SchemaCodec.encode(msg) };
        let image = msg.try_struct_bytes()?;
        let size = s.get(tag).map_err(|e| failed(udt, e))?.struct_size as usize;
        if image.len() != size {
            // As the oracle: never pad or truncate a struct the client reads.
            return Err(AhError::SchemaCodec {
                user_defined_type: udt,
                detail: format!("struct image is {} bytes, the schema's struct is {size} ({size:#x})", image.len()),
            });
        }
        let enc = d1_rsat::Encoder::new(s);
        let e = if d1_oracle::activity_host::is_replicated(udt) {
            enc.encode_replicated(tag, &image)
        } else {
            enc.encode(tag, &image)
        };
        e.map(|e| e.payload).map_err(|e| failed(udt, e))
    }

    fn decode(&self, user_defined_type: u32, wire: &[u8]) -> Result<Vec<u8>, AhError> {
        self.decode_seeded(user_defined_type, wire, &[])
    }

    fn decode_seeded(&self, udt: u32, wire: &[u8], seed: &[u8]) -> Result<Vec<u8>, AhError> {
        crate::protocol::check_receivable(udt)?;
        if d1_oracle::activity_host::is_raw_send(udt).is_some() {
            return Ok(wire.to_vec());
        }
        let Some((s, tag)) = Self::native_tag(udt, true) else {
            return SchemaCodec.decode_seeded(udt, wire, seed);
        };
        let dec = d1_rsat::Decoder::new(s);
        let d = if d1_oracle::activity_host::is_replicated(udt) {
            dec.decode_replicated(tag, wire, seed)
        } else if !seed.is_empty() {
            return Err(AhError::SchemaCodec {
                user_defined_type: udt,
                detail: "not replicated: a seed image is meaningless for it".into(),
            });
        } else {
            dec.decode(tag, wire)
        };
        d.map(|d| d.object).map_err(|e| failed(udt, e))
    }

    /// ── D1_AH_SCRIPTS ── The script bridge's entry-only frames, natively when every value schema
    /// is covered; otherwise the client's own codec, as [`try_sensor_frame`] routes.
    fn encode_sensor_entries(
        &self,
        frame: &d1_oracle::sensor::SensorEntryFrame,
    ) -> Result<(Vec<u8>, usize), AhError> {
        match try_entry_frame(frame) {
            Some(r) => r.map_err(|detail| AhError::SchemaCodec { user_defined_type: crate::protocol::UDT_SENSOR_AUTH_UPDATE, detail }),
            None => crate::codec::oracle_sensor_entries(frame),
        }
    }

    /// ── D1_AH_SCRIPTS ── An incident body, natively when its schema is covered.
    fn encode_object(&self, tag: u32, image: &[u8]) -> Result<(u32, Vec<u8>), AhError> {
        let covered = enabled().then(schemas).and_then(Result::ok).filter(|s| s.check(tag).is_ok());
        match covered {
            Some(s) => d1_rsat::Encoder::new(s)
                .encode(tag, image)
                .map(|e| (e.bits as u32, e.payload))
                .map_err(|e| failed(18, e)),
            None => crate::codec::oracle_object(tag, image),
        }
    }
}

/// ── D1_AH_SCRIPTS ── An entry-only udt 5 frame natively, under the same conditions as
/// [`try_sensor_frame`]. `None`: build it with the client's codec.
pub fn try_entry_frame(f: &d1_oracle::sensor::SensorEntryFrame) -> Option<Result<(Vec<u8>, usize), String>> {
    use d1_rsat::sensor::{Entry, Frame, NameRef};
    if !enabled() {
        return None;
    }
    let s = schemas().ok()?;
    let covered = f
        .entries
        .iter()
        .flat_map(|c| [c.a_value.as_ref().filter(|_| c.has_a), c.b_value.as_ref().filter(|_| c.has_b)])
        .flatten()
        .all(|(tag, _)| s.check(*tag).is_ok());
    if !covered {
        return None;
    }
    let frame = Frame {
        gen: f.gen,
        authority: f.authority.as_ref().map(|a| (a.changed, a.image.clone())),
        stamp: 0,
        init: f.init,
        entries: f
            .entries
            .iter()
            .map(|c| Entry {
                name: NameRef { hash: c.name_hash, i8: c.name_i8, sub: c.name_sub },
                has_a: c.has_a,
                a_value: c.a_value.clone(),
                has_b: c.has_b,
                b_value: c.b_value.clone(),
            })
            .collect(),
        pad: true,
    };
    Some(frame.encode(s).map_err(|e| e.to_string()).map(|b| {
        d1_oracle::sensor::remember_stamp_bit(&b.bytes, b.stamp_bit as usize);
        (b.bytes, b.stamp_bit as usize)
    }))
}

/// The host's udt 5 frame as `d1-rsat` assembles it: the lifetime record first, then every client.
pub fn sensor_frame_of(p: &d1_oracle::sensor::SensorPhaseUpdate) -> d1_rsat::sensor::Frame {
    use d1_rsat::sensor::{Entry, Frame, NameRef, TAG_SENSOR_VALUE};
    let mut entries = vec![Entry {
        name: NameRef { hash: p.name_hash, i8: p.name_i8, sub: p.name_sub },
        has_a: true,
        a_value: Some((TAG_SENSOR_VALUE, p.value_image())),
        has_b: false,
        b_value: None,
    }];
    entries.extend(p.clients.iter().map(|c| Entry {
        name: NameRef { hash: c.name_hash, i8: c.name_i8, sub: c.name_sub },
        has_a: c.has_a,
        a_value: c.a_value.clone(),
        has_b: c.has_b,
        b_value: c.b_value.clone(),
    }));
    Frame {
        gen: p.gen,
        authority: p.authority.as_ref().map(|a| (a.changed, a.image.clone())),
        stamp: 0,
        init: true,
        entries,
        // `SensorPhaseUpdate::encode`'s rule: the audited single-entry frame stays unpadded.
        pad: !p.clients.is_empty() || p.authority.is_some(),
    }
}

/// ⭐ Build a udt 5 frame natively, when the knob is set and every value schema is covered.
/// `None`: build it with the client's codec (knob off, another build, or a value schema the native
/// codec does not cover: logged once per tag). The stamp offset is recorded for restamping.
pub fn try_sensor_frame(p: &d1_oracle::sensor::SensorPhaseUpdate) -> Option<Result<Vec<u8>, String>> {
    if !enabled() {
        return None;
    }
    let s = match schemas() {
        Ok(s) => s,
        Err(why) => {
            note_oracle(crate::protocol::UDT_SENSOR_AUTH_UPDATE, "frame", &why);
            return None;
        }
    };
    let values = p
        .clients
        .iter()
        .flat_map(|c| [c.a_value.as_ref().filter(|_| c.has_a), c.b_value.as_ref().filter(|_| c.has_b)])
        .flatten();
    for (tag, _) in values {
        if let Err(e) = s.check(*tag) {
            static SEEN: Mutex<BTreeSet<u32>> = Mutex::new(BTreeSet::new());
            if SEEN.lock().unwrap_or_else(|e| e.into_inner()).insert(*tag) {
                tracing::info!("native RSAT: udt 5 frames carrying {tag:#010x} stay on the client's codec: {e}");
            }
            return None;
        }
    }
    Some(sensor_frame_of(p).encode(s).map_err(|e| e.to_string()).map(|b| {
        d1_oracle::sensor::remember_stamp_bit(&b.bytes, b.stamp_bit as usize);
        b.bytes
    }))
}

/// The stamp offset of a built frame, natively (no codec worker). `None` with the knob unset.
pub fn sensor_stamp_bit(frame: &[u8]) -> Option<Result<usize, String>> {
    if !enabled() {
        return None;
    }
    let s = schemas().ok()?;
    Some(d1_rsat::sensor::stamp_bit(s, frame).map(|b| b as usize).map_err(|e| e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eboot() -> bool {
        let ok = schemas().is_ok();
        if !ok {
            eprintln!("SKIP: no owned 56578 executable with a native RSAT table; set D1_EBOOT");
        }
        ok
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
    }

    /// ⭐ The drop-in against the codec it replaces: the same bytes for every hot message, and the
    /// same image back from the same body, through the trait the listener holds.
    #[test]
    fn the_native_codec_matches_the_schema_codec_through_the_trait() {
        if !eboot() {
            return;
        }
        let native: &dyn AhWireCodec = &NativeCodec;
        let oracle: &dyn AhWireCodec = &SchemaCodec;
        let mut checked = 0;
        for udt in [0u32, 1, 2, 4, 11, 23, 24] {
            let Ok(size) = SchemaCodec::struct_size(udt) else { continue };
            for first in [0u8, 1] {
                let mut bytes = vec![0u8; size];
                bytes[0] = first;
                let msg = AhMessage::Opaque { user_defined_type: udt, bytes };
                let (n, o) = (native.encode(&msg), oracle.encode(&msg));
                assert_eq!(n.is_ok(), o.is_ok(), "udt {udt} first {first}: {n:?} / {o:?}");
                if let (Ok(n), Ok(o)) = (n, o) {
                    assert_eq!(n, o, "udt {udt} first {first}: encode");
                    checked += 1;
                }
            }
        }
        assert!(checked >= 8, "only {checked} images compared");
        // Decode the captured join_request through both.
        let wire = hex(d1_oracle::activity_host::CAPTURED_JOIN_REQUEST);
        assert_eq!(native.decode(3, &wire).unwrap(), oracle.decode(3, &wire).unwrap());
        // A replicated decode over a seed, and the dynamic-flags udt on the oracle's refusal.
        let size = SchemaCodec::struct_size(21).unwrap();
        let seed: Vec<u8> = (0..size).map(|i| (i % 7) as u8 & 1).collect();
        assert_eq!(native.decode_seeded(21, &[0x00], &seed).unwrap(), seed);
    }

    /// The audited lifetime frame natively is the frame the oracle builds, and its stamp offset is
    /// known without a codec worker.
    #[test]
    fn the_audited_frame_is_built_natively() {
        if !eboot() {
            return;
        }
        let p = d1_oracle::sensor::SensorPhaseUpdate {
            gen: 0,
            name_hash: 0xC2DA_CAF7,
            name_i8: 0x10,
            name_sub: 3,
            phase: 3,
            tail: None,
            clients: Vec::new(),
            authority: Some(d1_oracle::sensor::SensorAuthority::all_to(14, 1)),
        };
        let s = schemas().unwrap();
        let native = sensor_frame_of(&p).encode(s).unwrap();
        assert_eq!(native.bytes, p.encode().unwrap());
        assert_eq!(d1_rsat::sensor::stamp_bit(s, &native.bytes).unwrap(), native.stamp_bit);
    }
}
