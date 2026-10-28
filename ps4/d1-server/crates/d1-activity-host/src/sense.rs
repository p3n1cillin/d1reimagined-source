// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! ⭐ Live squad counts from the client's udt 6 `sensor_sense_update` (56578), opt-in.
//!
//! `D1_AH_SENSE_SCHEMAS=KIND:TAG,...` (hex or decimal, e.g. `1:0x80800357,3:0x80800830`) turns it
//! on. The decoder is [`d1_oracle::sensor_sense`]: it needs a B tag for **every** kind the world
//! sends and refuses a packet that names an unmapped kind, so list all of the world's kinds (read
//! them with `d1-winguest.py sensor-clients --all --types`).
//!
//! The squad B image (tag [`SQUAD_B_TAG`], 160 bytes) carries the squad's live-member count at
//! `+0x0C`. Run sense35 (28 Oct 2026) measured it natively: Divide squad 17 went 1 -> 0 at the
//! moment the client reported the Vandal kill (incidents 2389/3056/2462/2512/3246/3856), and the
//! other eleven squads stayed at 1. Only changes the decoder actually observed at `+0x0C` are
//! reported, so a zero-seeded image whose count was never sent is not a kill.
//!
//! The decoder runs Unicorn (`!Send`), so it lives on its own thread: the connection submits raw
//! payloads without waiting and drains the counts on its next frame, which keeps the heartbeat and
//! the codec worker off this path.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, OnceLock};

/// The squad sense B image (kind 1 in the Divide's group `0x9243DFFB`).
pub const SQUAD_B_TAG: u32 = 0x8080_0357;
/// The live-member count in the squad B image (native writer `0x105330`).
pub const LIVE_COUNT_AT: usize = 0x0C;

/// One observed change of a squad's live-member count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SquadCount {
    pub group: u32,
    pub kind: u8,
    pub index: i16,
    pub live: i32,
}

/// Parse `KIND:TAG,...`. Kinds and tags take `0x` hex or decimal.
pub fn parse_schemas(s: &str) -> Option<HashMap<u8, u32>> {
    let num = |t: &str| {
        let t = t.trim();
        t.strip_prefix("0x").map_or_else(|| t.parse::<u32>().ok(), |h| u32::from_str_radix(h, 16).ok())
    };
    let mut out = HashMap::new();
    for pair in s.split(',').filter(|p| !p.trim().is_empty()) {
        let (k, t) = pair.split_once(':')?;
        out.insert(u8::try_from(num(k)?).ok()?, num(t)?);
    }
    (!out.is_empty()).then_some(out)
}

/// The squad counts a decoded frame changed.
pub fn squad_counts(frame: &d1_oracle::sensor_sense::SenseFrame) -> Vec<SquadCount> {
    frame
        .entries
        .iter()
        .filter(|e| e.tag == SQUAD_B_TAG)
        .filter(|e| e.changed_offsets.iter().any(|o| (LIVE_COUNT_AT..LIVE_COUNT_AT + 4).contains(o)))
        .filter_map(|e| {
            let b = e.image.get(LIVE_COUNT_AT..LIVE_COUNT_AT + 4)?;
            Some(SquadCount {
                group: e.name.group,
                kind: e.name.kind,
                index: e.name.index,
                live: i32::from_le_bytes(b.try_into().ok()?),
            })
        })
        .collect()
}

struct SenseWorker {
    tx: Mutex<Sender<Vec<u8>>>,
    out: Mutex<Receiver<Vec<SquadCount>>>,
}

fn worker() -> Option<&'static SenseWorker> {
    static W: OnceLock<Option<SenseWorker>> = OnceLock::new();
    W.get_or_init(|| {
        let spec = std::env::var("D1_AH_SENSE_SCHEMAS").ok()?;
        let Some(schemas) = parse_schemas(&spec) else {
            tracing::warn!("D1_AH_SENSE_SCHEMAS={spec:?} is not KIND:TAG,...; squad counts are off");
            return None;
        };
        let (tx, rx) = channel::<Vec<u8>>();
        let (otx, orx) = channel::<Vec<SquadCount>>();
        std::thread::Builder::new()
            .name("d1-ah-sense".into())
            .spawn(move || {
                let mut decoder = match d1_oracle::sensor_sense::SenseDecoder::new(schemas) {
                    Ok(d) => d,
                    Err(e) => {
                        tracing::warn!("udt 6 sense decoder unavailable, squad counts are off: {e}");
                        return;
                    }
                };
                let (mut ok, mut failed) = (0u64, 0u64);
                while let Ok(payload) = rx.recv() {
                    match decoder.decode(&payload) {
                        Ok(frame) => {
                            ok += 1;
                            let counts = squad_counts(&frame);
                            if !counts.is_empty() && otx.send(counts).is_err() {
                                return;
                            }
                        }
                        // A refused packet commits nothing; say so sparingly (1st, 2nd, 4th, ...).
                        Err(e) => {
                            failed += 1;
                            if failed.is_power_of_two() {
                                tracing::warn!("udt 6 sense packet {} not decoded ({failed} of {}): {e}",
                                    ok + failed, ok + failed);
                            }
                        }
                    }
                }
            })
            .ok()?;
        tracing::info!("udt 6 squad counts on (D1_AH_SENSE_SCHEMAS={spec})");
        Some(SenseWorker { tx: Mutex::new(tx), out: Mutex::new(orx) })
    })
    .as_ref()
}

/// Queue one raw udt 6 payload for decoding. A no-op unless `D1_AH_SENSE_SCHEMAS` is set.
pub fn submit(payload: &[u8]) {
    if let Some(w) = worker() {
        if let Ok(tx) = w.tx.lock() {
            let _ = tx.send(payload.to_vec());
        }
    }
}

/// Every squad count decoded since the last call, in order.
pub fn drain() -> Vec<SquadCount> {
    let Some(w) = worker() else { return Vec::new() };
    let Ok(rx) = w.out.lock() else { return Vec::new() };
    rx.try_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schemas_take_hex_and_decimal_and_refuse_junk() {
        let s = parse_schemas("1:0x80800357, 0x1c:0x808005C3,3:2155874352").unwrap();
        assert_eq!(s[&1], 0x8080_0357);
        assert_eq!(s[&0x1C], 0x8080_05C3);
        assert_eq!(s[&3], 2_155_874_352);
        assert!(parse_schemas("").is_none());
        assert!(parse_schemas("1=0x80800357").is_none());
        assert!(parse_schemas("300:1").is_none());
    }
}
