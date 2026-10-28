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
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TryRecvError};

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
        let (kind, tag) = (u8::try_from(num(k)?).ok()?, num(t)?);
        if out.insert(kind, tag).is_some_and(|old| old != tag) { return None; }
    }
    (!out.is_empty()).then_some(out)
}

/// The squad counts a decoded frame changed.
pub fn squad_counts(frame: &d1_oracle::sensor_sense::SenseFrame) -> Vec<SquadCount> {
    frame
        .entries
        .iter()
        .filter(|e| e.tag == SQUAD_B_TAG && e.name.kind == 1 && e.name.index >= 0)
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

/// One successfully decoded frame, tied to the connection/world epoch that queued it.
#[derive(Debug)]
pub struct SenseUpdate {
    pub epoch: u64,
    pub generation: u8,
    pub counts: Vec<SquadCount>,
}

/// Evidence for an explicitly authored cohort. Zero seeds and negative counts cannot clear it.
#[derive(Debug, Default)]
pub struct SquadObservations {
    epoch: u64,
    generation: Option<u8>,
    ready: bool,
    counts: HashMap<(u32, i16), (i32, bool)>,
}

impl SquadObservations {
    pub fn epoch(&self) -> u64 { self.epoch }
    pub fn reset(&mut self) {
        self.epoch = self.epoch.checked_add(1).expect("sense epoch exhausted");
        self.generation = None;
        self.ready = false;
        self.counts.clear();
    }
    pub fn set_ready(&mut self, ready: bool) { self.ready = ready; }
    pub fn apply(&mut self, update: SenseUpdate) {
        if update.epoch != self.epoch { return; }
        if self.generation != Some(update.generation) {
            self.counts.clear();
            self.generation = Some(update.generation);
        }
        for c in update.counts {
            if c.kind != 1 || c.index < 0 { continue; }
            let key = (c.group, c.index);
            if c.live < 0 {
                self.counts.remove(&key);
                continue;
            }
            let e = self.counts.entry(key).or_insert((0, false));
            if e.0 != c.live {
                tracing::info!("⭐ squad {:#010x}/0x1/{} live members {} -> {}", c.group, c.index, e.0, c.live);
            }
            *e = (c.live, e.1 || c.live > 0);
        }
    }
    pub fn cleared(&self, group: u32, cohort: &[i16]) -> bool {
        self.ready && !cohort.is_empty() && cohort.iter().all(|i| {
            self.counts.get(&(group, *i)) == Some(&(0, true))
        })
    }
}

/// Bounded, connection-owned decoder. No global baselines or result receiver.
/// On a lost/refused delta, fail closed until the next connection/world epoch. Reconstructing
/// a decoder from later deltas would invent a baseline. Dropping this object closes its queues.
pub struct SenseWorker {
    epoch: u64,
    tx: SyncSender<Vec<u8>>,
    out: Receiver<Result<SenseUpdate, String>>,
    pending: usize,
    failed: bool,
}

impl SenseWorker {
    pub fn from_env(epoch: u64) -> Option<Self> {
        let spec = std::env::var("D1_AH_SENSE_SCHEMAS").ok()?;
        let Some(schemas) = parse_schemas(&spec) else {
            tracing::warn!("D1_AH_SENSE_SCHEMAS={spec:?} is not unambiguous KIND:TAG,...; squad counts are off");
            return None;
        };
        let (tx, rx) = sync_channel::<Vec<u8>>(32);
        let (otx, out) = sync_channel(32);
        std::thread::Builder::new().name("d1-ah-sense".into()).spawn(move || {
            let mut decoder = match d1_oracle::sensor_sense::SenseDecoder::new(schemas) {
                Ok(d) => d,
                Err(e) => { let _ = otx.send(Err(e.to_string())); return; }
            };
            while let Ok(payload) = rx.recv() {
                let update = decoder.decode(&payload).map(|frame| SenseUpdate {
                    epoch, generation: frame.generation, counts: squad_counts(&frame),
                }).map_err(|e| e.to_string());
                let failed = update.is_err();
                if otx.send(update).is_err() || failed { return; }
            }
        }).ok()?;
        tracing::info!("udt 6 squad decoder created for epoch {epoch}");
        Some(Self { epoch, tx, out, pending: 0, failed: false })
    }
    pub fn epoch(&self) -> u64 { self.epoch }
    pub fn submit(&mut self, payload: &[u8]) {
        if self.failed { return; }
        // Bound both queue depth and payload memory. Never discard a delta and keep trusting
        // the baseline: an overload poisons this epoch's kill evidence instead.
        if payload.len() > 64 * 1024 || self.tx.try_send(payload.to_vec()).is_err() {
            self.fail("sense input unavailable/full/oversized; HOLD until next world or connection");
        } else {
            self.pending += 1;
        }
    }
    fn fail(&mut self, reason: &str) {
        if !self.failed { tracing::warn!("squad evidence invalidated: {reason}"); }
        self.failed = true;
    }
    pub fn drain(&mut self) -> Vec<SenseUpdate> {
        let mut updates = Vec::new();
        if self.failed { return updates; }
        loop {
            match self.out.try_recv() {
                Ok(Ok(update)) => {
                    self.pending = self.pending.saturating_sub(1);
                    updates.push(update);
                }
                Ok(Err(e)) => { self.fail(&e); break; }
                Err(TryRecvError::Disconnected) => { self.fail("sense decoder exited"); break; }
                Err(TryRecvError::Empty) => break,
            }
        }
        updates
    }
    pub fn ready(&self) -> bool { !self.failed && self.pending == 0 }
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
        assert!(parse_schemas("1:1,1:2").is_none());
        assert_eq!(parse_schemas("1:1,1:1").unwrap()[&1], 1);
    }

    fn count(group: u32, index: i16, live: i32) -> SquadCount {
        SquadCount { group, kind: 1, index, live }
    }

    fn apply(o: &mut SquadObservations, generation: u8, counts: Vec<SquadCount>) {
        o.apply(SenseUpdate { epoch: o.epoch(), generation, counts });
        o.set_ready(true);
    }

    #[test]
    fn cohort_requires_every_member_alive_then_exactly_zero() {
        let mut o = SquadObservations::default();
        apply(&mut o, 1, vec![count(1,5,1), count(1,5,0), count(1,6,0)]);
        assert!(!o.cleared(1, &[5,6]), "zero-seeded or unseen members hold");
        apply(&mut o, 1, vec![count(2,6,1), count(2,6,0), SquadCount { kind: 2, ..count(1,6,1) }]);
        assert!(!o.cleared(1, &[5,6]), "other groups/kinds cannot admit a member");
        apply(&mut o, 1, vec![count(1,6,1), count(1,6,-1), count(1,6,0)]);
        assert!(!o.cleared(1, &[5,6]), "invalid counts invalidate admission");
        apply(&mut o, 1, vec![count(1,6,2), count(1,6,0), count(1,99,1)]);
        assert!(o.cleared(1, &[5,6]), "unrelated squads are outside the authored cohort");
        assert!(!o.cleared(1, &[]));
        o.set_ready(false);
        assert!(!o.cleared(1, &[5,6]), "pending/error decoder status holds");
    }

    #[test]
    fn generation_and_epoch_reset_admission_and_ignore_queued_old_results() {
        let mut o = SquadObservations::default();
        apply(&mut o, 1, vec![count(1,5,1), count(1,5,0)]);
        assert!(o.cleared(1, &[5]));
        apply(&mut o, 2, vec![count(1,5,0)]);
        assert!(!o.cleared(1, &[5]), "new generation cannot reuse alive history");
        apply(&mut o, 2, vec![count(1,5,1), count(1,5,0)]);
        let old = o.epoch();
        o.reset();
        o.apply(SenseUpdate { epoch: old, generation: 2, counts: vec![count(1,5,1), count(1,5,0)] });
        o.set_ready(true);
        assert!(!o.cleared(1, &[5]), "stale async result cannot revive old evidence");
        let independent = SquadObservations::default();
        assert!(!independent.cleared(1, &[5]));
    }

    fn fake_worker() -> (SenseWorker, Receiver<Vec<u8>>, SyncSender<Result<SenseUpdate, String>>) {
        let (tx, rx) = sync_channel(1);
        let (otx, out) = sync_channel(4);
        (SenseWorker { epoch: 7, tx, out, pending: 0, failed: false }, rx, otx)
    }

    #[test]
    fn worker_backlog_and_any_lost_or_refused_delta_hold_the_gate() {
        let (mut w, rx, otx) = fake_worker();
        w.submit(&[1]);
        assert!(!w.ready());
        assert_eq!(rx.try_recv().unwrap(), vec![1]);
        otx.send(Ok(SenseUpdate { epoch: 7, generation: 1, counts: vec![] })).unwrap();
        assert_eq!(w.drain().len(), 1);
        assert!(w.ready());
        w.submit(&[2]);
        w.submit(&[3]); // bounded input queue is full: do not trust partial baseline
        assert!(!w.ready());
        assert!(w.drain().is_empty());
        let (mut w, _rx, otx) = fake_worker();
        otx.send(Err("refused delta".into())).unwrap();
        assert!(w.drain().is_empty());
        assert!(!w.ready());
        let (mut w, _rx, otx) = fake_worker();
        drop(otx);
        w.drain();
        assert!(!w.ready(), "dead worker cannot look caught up");
    }

    #[test]
    fn changed_count_bytes_are_required_not_a_zero_seed_or_wrong_kind() {
        use d1_oracle::sensor_sense::{SenseEntry, SenseFrame, SenseName};
        let entry = |kind, live: i32, changed_offsets| {
            let mut image = vec![0; 160];
            image[12..16].copy_from_slice(&live.to_le_bytes());
            SenseEntry { name: SenseName { group: 1, kind, index: 5 }, tag: SQUAD_B_TAG,
                revision: 0, body_bits: 1, reused_observation: false, changed_offsets, image }
        };
        let f = SenseFrame { initial_flag: true, generation: 1, padding_bits: 0,
            entries: vec![entry(1,0,vec![]), entry(2,1,vec![12]), entry(1,1,vec![12])] };
        assert_eq!(squad_counts(&f), vec![count(1,5,1)]);
    }
}
