// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! ⭐ The udt 5 row table: the host's copy of every sensor row (named entry) in the current world,
//! sent the way the live hosts sent it.
//!
//! The step machine ([`crate::state::MissionStep`]) swaps whole prebuilt lists. A script changes
//! one row at a time, so the script bridge ([`crate::script_bridge`]) keeps the rows here and sends:
//!
//! * **a full list with `init = 1`** at the join and at each world load: every row the world has,
//!   each `fresh`, plus the authority table. The walker clears an entry's pending byte only when a
//!   frame carries it (`0x2D1050..0x2D1079`), and completion fails while any is pending, so the
//!   first frame of a world must name them all (notes/ps4-56578-udt5-stamp-init-2026-11-05.md);
//! * **deltas with `init = 0`**, carrying only the rows that changed, once a full list has gone
//!   out. The `init` latch the full list set persists until the world is set up again; frame
//!   `init = 0` does not clear it. Each entry is still `fresh` with its whole value, as the live
//!   host's decodable deltas were (the cinematic play at stamp 200); the `fresh = 0` partial
//!   overlays the live host also sent are not built;
//! * **a heartbeat** (6 bytes: gen, stamp, `init = 0`, no entry) after 3 s with no other frame,
//!   as the live idle frames (stamps 45, 75, 105, …).
//!
//! Every frame carries the 10 Hz stamp from the first frame of the table, as `D1_AH_SENSOR_STAMP`
//! frames do ([`crate::state::SensorStamp`]).
//!
//! Rows the world's base list does not name are held back by default: a frame naming an entry the
//! client has no sensor client for is untested, and a refused frame would drop every row in it.
//! `D1_AH_ROWS_UNLISTED=1` sends them anyway.
//!
//! The table owns no codec: [`RowTable::plan`] says what the next frame must carry, the caller
//! encodes it (off the connection loop, [`crate::row_encoders::CodecWorker`]) and reports
//! [`RowTable::sent`] or [`RowTable::failed`].

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use d1_oracle::sensor::{SensorAuthority, SensorClientRef};

/// A row's name: `{group hash, type id (the name's i8), type index (its i16)}`, BullShcript's
/// `ItemRef {bundle, typeId, typeIndex}` once the bundle hash is mapped to the live group hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RowKey {
    pub group: u32,
    pub kind: u8,
    pub index: i16,
}

impl std::fmt::Display for RowKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#010x}/{:#x}/{}", self.group, self.kind, self.index)
    }
}

/// A row's value, in the shape a [`SensorClientRef`] carries it. `None` values are the client's
/// own default (`fresh = 1, presence = 0` / `has_b = 0`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub has_a: bool,
    pub has_b: bool,
    pub a: Option<(u32, Vec<u8>)>,
    pub b: Option<(u32, Vec<u8>)>,
}

impl Row {
    pub fn from_client(c: &SensorClientRef) -> (RowKey, Row) {
        (
            RowKey { group: c.name_hash, kind: c.name_i8, index: c.name_sub },
            Row { has_a: c.has_a, has_b: c.has_b, a: c.a_value.clone(), b: c.b_value.clone() },
        )
    }

    pub fn to_client(&self, key: RowKey) -> SensorClientRef {
        SensorClientRef {
            name_hash: key.group,
            name_i8: key.kind,
            name_sub: key.index,
            has_a: self.has_a,
            has_b: self.has_b,
            a_value: self.a.clone(),
            b_value: self.b.clone(),
        }
    }

    /// The same row back at the client's defaults (BullShcript `rows_cleared`).
    pub fn defaulted(&self) -> Row {
        Row { has_a: self.has_a, has_b: self.has_b, a: None, b: None }
    }
}

/// Give `row` the client's shape for its record: whether the entry has an A and a B part. A value
/// the shape has no part for is dropped. F40: run agr13's Breach list sent its 27 squads (kind 1,
/// `has_b`) without the B-presence bit, 2 bits where the step schedule's frame had 3, because the
/// scripts set them before the list named them; no Breach trigger ever reported until the list's
/// shape won.
pub fn reshape(row: &mut Row, has_a: bool, has_b: bool) {
    row.has_a = has_a;
    row.has_b = has_b;
    if !has_a {
        row.a = None;
    }
    if !has_b {
        row.b = None;
    }
}

/// What the next frame must carry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FramePlan {
    pub gen: u8,
    /// A full list (every row, the authority table) rather than a delta.
    pub full: bool,
    pub init: bool,
    pub authority: Option<SensorAuthority>,
    pub entries: Vec<(RowKey, Row)>,
}

impl FramePlan {
    /// The frame in the oracle's shape.
    pub fn to_entry_frame(&self) -> d1_oracle::sensor::SensorEntryFrame {
        d1_oracle::sensor::SensorEntryFrame {
            gen: self.gen,
            authority: self.authority.clone(),
            init: self.init,
            entries: self.entries.iter().map(|(k, r)| r.to_client(*k)).collect(),
        }
    }
}

/// The table. See the module doc.
#[derive(Debug)]
pub struct RowTable {
    gen: u8,
    rows: BTreeMap<RowKey, Row>,
    listed: BTreeSet<RowKey>,
    dirty: BTreeSet<RowKey>,
    authority: Option<SensorAuthority>,
    full_wanted: bool,
    full_delivered: bool,
    in_flight: bool,
    send_unlisted: bool,
    delta_init: bool,
    t0: Option<Instant>,
    last_sent: Option<Instant>,
    held_logged: BTreeSet<RowKey>,
    pub full_frames: u64,
    pub delta_frames: u64,
    pub heartbeats: u64,
}

impl RowTable {
    /// The live idle period, and the heartbeat's.
    pub const HEARTBEAT: Duration = Duration::from_secs(3);

    pub fn new(gen: u8) -> Self {
        Self {
            gen,
            rows: BTreeMap::new(),
            listed: BTreeSet::new(),
            dirty: BTreeSet::new(),
            authority: None,
            full_wanted: false,
            full_delivered: false,
            in_flight: false,
            send_unlisted: false,
            delta_init: false,
            t0: None,
            last_sent: None,
            held_logged: BTreeSet::new(),
            full_frames: 0,
            delta_frames: 0,
            heartbeats: 0,
        }
    }

    /// `D1_AH_ROWS_UNLISTED=1`: send rows the world's base list does not name.
    pub fn set_send_unlisted(&mut self, on: bool) {
        self.send_unlisted = on;
    }

    /// `D1_AH_ROWS_DELTA_INIT=1`: set `init` on deltas too (harmless to the client, which keeps
    /// its latch; a fallback if a world re-setup ever clears the latch unseen).
    pub fn set_delta_init(&mut self, on: bool) {
        self.delta_init = on;
    }

    /// A world loaded: these are all its rows. The next frame is a full list.
    pub fn load_world(&mut self, rows: Vec<(RowKey, Row)>, authority: Option<SensorAuthority>) {
        self.listed = rows.iter().map(|(k, _)| *k).collect();
        self.rows = rows.into_iter().collect();
        self.authority = authority;
        self.dirty.clear();
        self.held_logged.clear();
        self.full_wanted = true;
        self.full_delivered = false;
    }

    /// The client moved to another slice set of the same world, whose list names other rows (the
    /// live host sent the new set's full list after the move). Every value the table holds is
    /// kept, including rows held back until now; a row the new list adds that the table does not
    /// hold yet takes the list's value. `keep` (the lifetime) stays listed. The next frame is a
    /// full list with the current values.
    pub fn relist(&mut self, rows: Vec<(RowKey, Row)>, keep: &[RowKey]) {
        self.listed = rows.iter().map(|(k, _)| *k).chain(keep.iter().copied()).collect();
        for (k, r) in rows {
            match self.rows.get_mut(&k) {
                // The list is the client's own shape for the record (F40).
                Some(held) => reshape(held, r.has_a, r.has_b),
                None => {
                    self.rows.insert(k, r);
                }
            }
        }
        self.dirty.clear();
        self.held_logged.clear();
        self.full_wanted = true;
        self.full_delivered = false;
    }

    /// The client joined again (or set its world up again): send everything once more.
    pub fn request_full(&mut self) {
        self.full_wanted = true;
        self.full_delivered = false;
    }

    pub fn get(&self, key: &RowKey) -> Option<&Row> {
        self.rows.get(key)
    }

    pub fn is_listed(&self, key: &RowKey) -> bool {
        self.listed.contains(key)
    }

    pub fn rows(&self) -> impl Iterator<Item = (&RowKey, &Row)> {
        self.rows.iter()
    }

    /// Set a row. Returns whether it changed (and so will be in the next delta).
    pub fn set(&mut self, key: RowKey, mut row: Row) -> bool {
        // A row the table holds keeps its shape: the encoders take `has_b` from the row they build
        // on, so a value set before the client's list named the record had none (F40).
        if let Some(held) = self.rows.get(&key) {
            reshape(&mut row, held.has_a, held.has_b);
        }
        if self.rows.get(&key) == Some(&row) {
            return false;
        }
        self.rows.insert(key, row);
        if self.listed.contains(&key) || self.send_unlisted {
            self.dirty.insert(key);
        } else if self.held_logged.insert(key) {
            tracing::warn!(
                "udt 5 row {key} is not in the world's list; held back (D1_AH_ROWS_UNLISTED=1 sends it)"
            );
        }
        true
    }

    /// Put a row back at the client's defaults. False if the table has no such row.
    pub fn reset(&mut self, key: RowKey) -> bool {
        match self.rows.get(&key).map(Row::defaulted) {
            Some(row) => self.set(key, row),
            None => false,
        }
    }

    /// Whether a full list has gone out since the world loaded.
    pub fn full_delivered(&self) -> bool {
        self.full_delivered
    }

    /// Rows waiting for the next delta.
    pub fn dirty_len(&self) -> usize {
        self.dirty.len()
    }

    /// The next frame to encode, if one is due and none is being encoded: the full list when one
    /// is wanted, else the changed rows once a full list has gone out.
    pub fn plan(&mut self) -> Option<FramePlan> {
        if self.in_flight {
            return None;
        }
        if self.full_wanted {
            self.full_wanted = false;
            self.dirty.clear();
            self.in_flight = true;
            let entries = self
                .rows
                .iter()
                .filter(|(k, _)| self.send_unlisted || self.listed.contains(k))
                .map(|(k, r)| (*k, r.clone()))
                .collect();
            return Some(FramePlan {
                gen: self.gen,
                full: true,
                init: true,
                authority: self.authority.clone(),
                entries,
            });
        }
        if !self.full_delivered || self.dirty.is_empty() {
            return None;
        }
        let keys = std::mem::take(&mut self.dirty);
        self.in_flight = true;
        Some(FramePlan {
            gen: self.gen,
            full: false,
            init: self.delta_init,
            authority: None,
            entries: keys.into_iter().filter_map(|k| Some((k, self.rows.get(&k)?.clone()))).collect(),
        })
    }

    /// The planned frame went out at `now`.
    pub fn sent(&mut self, plan: &FramePlan, now: Instant) {
        self.in_flight = false;
        self.last_sent = Some(now);
        if plan.full {
            self.full_delivered = true;
            self.full_frames += 1;
        } else {
            self.delta_frames += 1;
        }
    }

    /// The planned frame could not be built: plan it again (its rows' current values).
    pub fn failed(&mut self, plan: FramePlan) {
        self.in_flight = false;
        if plan.full {
            self.full_wanted = true;
        } else {
            self.dirty.extend(plan.entries.into_iter().map(|(k, _)| k));
        }
    }

    /// The 10 Hz stamp at `now`, counted from the table's first call.
    pub fn tick(&mut self, now: Instant) -> u32 {
        let t0 = *self.t0.get_or_insert(now);
        (now.saturating_duration_since(t0).as_millis() / 100) as u32
    }

    /// Stamp an encoded frame for `now`, keeping its `init` bit.
    pub fn stamp(&mut self, frame: &mut [u8], stamp_bit: usize, init: bool, now: Instant) -> Result<(), String> {
        let tick = self.tick(now);
        self.stamp_at(frame, stamp_bit, init, tick)
    }

    /// Stamp an encoded frame with a given tick (a frame that belongs to an earlier one's tick).
    pub fn stamp_at(&mut self, frame: &mut [u8], stamp_bit: usize, init: bool, tick: u32) -> Result<(), String> {
        d1_oracle::sensor::set_frame_stamp(frame, stamp_bit, tick, init).map_err(|e| e.to_string())
    }

    /// The idle heartbeat, when `HEARTBEAT` passed since the last frame (none before the first).
    pub fn heartbeat_due(&mut self, now: Instant) -> Option<Vec<u8>> {
        let last = self.last_sent?;
        if now.saturating_duration_since(last) < Self::HEARTBEAT {
            return None;
        }
        self.last_sent = Some(now);
        self.heartbeats += 1;
        Some(d1_oracle::sensor::heartbeat_frame(self.gen, self.tick(now)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(index: i16) -> RowKey {
        RowKey { group: 0x2FE7_DBE6, kind: 6, index }
    }

    fn row(v: u8) -> Row {
        Row { has_a: true, has_b: false, a: Some((0x8080_254D, vec![v; 4])), b: None }
    }

    fn world() -> RowTable {
        let mut t = RowTable::new(3);
        t.load_world(vec![(key(0), row(1)), (key(1), row(1)), (key(2), row(1))], Some(SensorAuthority::all_to(14, 1)));
        t
    }

    fn names(p: &FramePlan) -> Vec<i16> {
        p.entries.iter().map(|(k, _)| k.index).collect()
    }

    #[test]
    fn the_lists_shape_wins_over_a_value_set_before_it() {
        let k = RowKey { group: 0xfc0e_45a3, kind: 1, index: 36 };
        let mut t = RowTable::new(0);
        t.load_world(Vec::new(), None);
        // The scripts set the squad while no list named it: the encoder gave it no B part.
        t.set(k, Row { has_a: true, has_b: false, a: None, b: None });
        t.relist(vec![(k, Row { has_a: true, has_b: true, a: None, b: None })], &[]);
        assert!(t.get(&k).unwrap().has_b, "the client's list has a B part for squads");
        t.set(k, Row { has_a: true, has_b: false, a: Some((1, vec![1])), b: None });
        let r = t.get(&k).unwrap();
        assert!(r.has_b && r.a == Some((1, vec![1])), "a later value keeps the shape");
        let mut odd = Row { has_a: true, has_b: true, a: Some((1, vec![1])), b: Some((2, vec![2])) };
        reshape(&mut odd, false, false);
        assert_eq!(odd, Row { has_a: false, has_b: false, a: None, b: None }, "a part the shape lacks is dropped");
    }

    #[test]
    fn the_first_frame_is_the_full_list_with_init_and_authority() {
        let mut t = world();
        t.set(key(1), row(2));
        let p = t.plan().expect("full");
        assert!(p.full && p.init && p.authority.is_some() && p.gen == 3);
        assert_eq!(names(&p), [0, 1, 2]);
        assert_eq!(p.entries[1].1, row(2), "the full list carries the current values");
        assert!(t.plan().is_none(), "one frame in flight at a time");
        t.sent(&p, Instant::now());
        assert!(t.plan().is_none(), "nothing changed since the full list");
    }

    #[test]
    fn deltas_carry_only_changed_rows_and_never_precede_the_full_list() {
        let now = Instant::now();
        let mut t = world();
        let full = t.plan().unwrap();
        // Changes while the full list is in flight wait for a delta after it.
        assert!(t.set(key(2), row(5)));
        assert!(!t.set(key(2), row(5)), "an unchanged value is not a change");
        assert!(t.plan().is_none());
        t.sent(&full, now);
        let d = t.plan().expect("delta");
        assert!(!d.full && !d.init && d.authority.is_none());
        assert_eq!(names(&d), [2]);
        assert_eq!(d.entries[0].1, row(5));
        t.sent(&d, now);
        assert!(t.set(key(0), row(7)) && t.set(key(1), row(7)) && t.set(key(0), row(8)));
        let d = t.plan().unwrap();
        assert_eq!(names(&d), [0, 1], "each row once, with its latest value");
        assert_eq!(d.entries[0].1, row(8));
        t.sent(&d, now);
        assert_eq!((t.full_frames, t.delta_frames), (1, 2));
    }

    #[test]
    fn a_failed_frame_is_planned_again() {
        let now = Instant::now();
        let mut t = world();
        let full = t.plan().unwrap();
        t.failed(full);
        let full = t.plan().expect("the full list again");
        assert!(full.full);
        t.sent(&full, now);
        t.set(key(1), row(9));
        let d = t.plan().unwrap();
        t.set(key(2), row(9));
        t.failed(d);
        assert_eq!(names(&t.plan().unwrap()), [1, 2]);
    }

    #[test]
    fn a_reload_or_rejoin_sends_the_full_list_again() {
        let now = Instant::now();
        let mut t = world();
        let p = t.plan().unwrap();
        t.sent(&p, now);
        t.request_full();
        t.set(key(0), row(3));
        let p = t.plan().unwrap();
        assert!(p.full && p.init && names(&p) == [0, 1, 2]);
        t.sent(&p, now);
        t.load_world(vec![(key(9), row(1))], None);
        let p = t.plan().unwrap();
        assert!(p.full && names(&p) == [9] && p.authority.is_none());
    }

    #[test]
    fn rows_the_world_does_not_list_are_held_back_unless_asked() {
        let now = Instant::now();
        let mut t = world();
        let p = t.plan().unwrap();
        t.sent(&p, now);
        assert!(t.set(key(40), row(1)));
        assert!(t.plan().is_none(), "held");
        t.set_send_unlisted(true);
        t.set(key(41), row(1));
        assert_eq!(names(&t.plan().unwrap()), [41]);
    }

    #[test]
    fn a_slice_change_relists_with_current_values_and_releases_held_rows() {
        let now = Instant::now();
        let mut t = world();
        let p = t.plan().unwrap();
        t.sent(&p, now);
        t.set(key(0), row(7));
        t.set(key(40), row(9));
        let p = t.plan().unwrap();
        t.sent(&p, now);
        let lifetime = RowKey { group: 0x4786_C0E0, kind: 0x10, index: 3 };
        t.set(lifetime, row(4));
        t.relist(vec![(key(0), row(1)), (key(40), row(1)), (key(41), row(2))], &[lifetime]);
        let full = t.plan().unwrap();
        assert!(full.full && full.init);
        let got: Vec<(i16, u8)> = full.entries.iter().map(|(k, r)| (k.index, r.a.as_ref().unwrap().1[0])).collect();
        assert_eq!(got, [(0, 7), (40, 9), (41, 2), (3, 4)], "current values, the held row released, the lifetime kept");
        assert!(!t.is_listed(&key(1)), "the old set's other rows are no longer listed");
    }

    #[test]
    fn reset_returns_a_row_to_the_client_defaults() {
        let now = Instant::now();
        let mut t = world();
        let p = t.plan().unwrap();
        t.sent(&p, now);
        assert!(t.reset(key(1)));
        let d = t.plan().unwrap();
        assert_eq!(d.entries, vec![(key(1), Row { has_a: true, has_b: false, a: None, b: None })]);
        assert!(!t.reset(key(77)), "no such row");
    }

    #[test]
    fn the_heartbeat_comes_three_seconds_after_the_last_frame_with_a_10_hz_stamp() {
        let t0 = Instant::now();
        let at = |ms: u64| t0 + Duration::from_millis(ms);
        let mut t = world();
        assert_eq!(t.tick(t0), 0, "the stamp clock starts at the first frame");
        assert!(t.heartbeat_due(at(10_000)).is_none(), "no heartbeat before the first frame");
        let p = t.plan().unwrap();
        t.sent(&p, at(1_000));
        assert!(t.heartbeat_due(at(3_999)).is_none());
        let hb = t.heartbeat_due(at(4_000)).expect("3 s after the full list");
        assert_eq!(hb, d1_oracle::sensor::heartbeat_frame(3, 40));
        assert!(t.heartbeat_due(at(6_999)).is_none());
        assert_eq!(t.heartbeat_due(at(7_000)), Some(d1_oracle::sensor::heartbeat_frame(3, 70)));
        // A delta resets the idle timer.
        t.set(key(0), row(4));
        let d = t.plan().unwrap();
        t.sent(&d, at(8_500));
        assert!(t.heartbeat_due(at(11_000)).is_none());
        assert_eq!(t.heartbeat_due(at(11_500)), Some(d1_oracle::sensor::heartbeat_frame(3, 115)));
        assert_eq!(t.heartbeats, 3);
    }

    #[test]
    fn stamping_writes_the_tick_and_keeps_init() {
        let t0 = Instant::now();
        let mut t = RowTable::new(1);
        t.tick(t0);
        let mut f = d1_oracle::sensor::heartbeat_frame(1, 0);
        t.stamp(&mut f, 9, true, t0 + Duration::from_millis(2_050)).unwrap();
        let mut want = d1_oracle::sensor::heartbeat_frame(1, 20);
        want[5] |= 0x40; // init is bit 41
        assert_eq!(f, want);
    }
}
