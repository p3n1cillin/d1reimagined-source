// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! # Checkpoints and loot lockouts, per character
//!
//! A multi-stage activity remembers two things about a character between runs:
//!
//! * **its checkpoint**: the encounter a raid restarts at. D1 kept it per character and per
//!   activity (each difficulty is its own activity hash) until the weekly reset. Launching the
//!   raid loads the fireteam leader's checkpoint. A full clear spends it, so the next launch
//!   starts from the beginning.
//! * **its loot lockouts**: which encounters already paid out this week. A character's second clear
//!   of an encounter in the same week gives no encounter loot.
//!
//! Strikes, arenas and the Crucible keep neither. Their in-activity restart point lives in the
//! running [`crate::encounter_framework::ActivityRun`], not here.
//!
//! The durable store is stream C's `player_store::PlayerStore` in `d1-responder` (#307), a
//! different process. This module keeps the activity host's side of it:
//!
//! * [`CheckpointStore`], the interface the framework calls;
//! * [`MemoryCheckpointStore`], for tests and for running without the store;
//! * [`ControlFileCheckpointStore`], which reaches the responder's store today through its control
//!   file (`D1_STORE_CONTROL`, polled each second, taken by renaming it `.taken`).
//!
//! ## What crosses to the responder
//!
//! The store keys a checkpoint by the client's **activity index** (`characters/activity-table.tsv`
//! maps the manifest hash to it) and holds a free-form `step`. A save is one control line:
//!
//! ```text
//! checkpoint INDEX ENCOUNTER_INDEX ENCOUNTER_ID week=WEEK
//! ```
//!
//! which `player_store::parse_command` reads as `Command::Checkpoint(INDEX, "ENCOUNTER_INDEX
//! ENCOUNTER_ID week=WEEK")` and applies with `set_checkpoint` + `save_now`. [`format_step`] and
//! [`parse_step`] are the step's two directions. **Asked of the responder side** (not there yet):
//!
//! 1. a clear. Until it exists a spent checkpoint is sent as the step `-`
//!    (`checkpoint INDEX -`), which the responder should treat as `clear_checkpoint(INDEX)`;
//! 2. the loot lockouts (encounters paid this week), which have no store field: they are kept in
//!    the host's mirror only, so they do not survive a host restart;
//! 3. a way to read the checkpoint back at activity start (the BAP side could pass the step in
//!    the activity launch, or the host could read the store's record file). Until then
//!    [`ControlFileCheckpointStore::seed`] takes what the launcher knows.
//!
//! The store holds one character (the one signed on), so only that character's saves are sent;
//! the others stay in the mirror.
//!
//! **The weekly reset** is Tuesday 09:00 UTC, D1's reset time (Bungie's own; the manifest names it
//! `WeeklyReset` on the raid and weekly categories but holds no time). [`reset_week`] numbers the
//! weeks from the first such Tuesday after the Unix epoch.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

/// A character's id, as the character store keys it.
pub type CharacterId = u64;

/// Seconds since the Unix epoch.
pub type UnixSeconds = u64;

/// 1970-01-06 was a Tuesday; 09:00 UTC that day is the first reset after the epoch.
pub const FIRST_RESET_UNIX: UnixSeconds = 5 * 86_400 + 9 * 3_600;
/// One week.
pub const WEEK_SECONDS: UnixSeconds = 7 * 86_400;

/// The reset week `now` falls in. Week 0 began 1970-01-06 09:00 UTC; anything before it is week 0
/// too.
pub fn reset_week(now: UnixSeconds) -> u64 {
    now.saturating_sub(FIRST_RESET_UNIX) / WEEK_SECONDS
}

/// When the reset week `week` began.
pub fn week_start(week: u64) -> UnixSeconds {
    FIRST_RESET_UNIX + week * WEEK_SECONDS
}

/// Which store entry: one character, one activity (one manifest activity hash, so one
/// difficulty).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CheckpointKey {
    pub character: CharacterId,
    pub activity_hash: u32,
}

/// A saved checkpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkpoint {
    pub key: CheckpointKey,
    /// The encounter the activity restarts at (its index in the definition).
    pub encounter_index: usize,
    /// The encounter's id, kept so a reordered definition can be detected rather than silently
    /// loading the wrong encounter.
    pub encounter_id: String,
    /// The reset week it was saved in; a checkpoint from another week is spent.
    pub week: u64,
}

/// The character store's checkpoint interface (stream C implements it durably).
///
/// Every method takes `&self`: the store is shared between activity instances.
pub trait CheckpointStore: Send + Sync {
    /// The checkpoint saved for `key`, whatever its week. Use [`load_current`] to drop a stale
    /// one.
    fn load(&self, key: CheckpointKey) -> Option<Checkpoint>;
    /// Save (replace) the checkpoint for `checkpoint.key`.
    fn save(&self, checkpoint: Checkpoint);
    /// Forget the checkpoint for `key` (a full clear spends it).
    fn clear(&self, key: CheckpointKey);
    /// Record that `key`'s character was paid encounter `encounter_id`'s loot in `week`.
    fn mark_looted(&self, key: CheckpointKey, encounter_id: &str, week: u64);
    /// Whether `key`'s character was already paid `encounter_id`'s loot in `week`.
    fn is_looted(&self, key: CheckpointKey, encounter_id: &str, week: u64) -> bool;
}

/// The checkpoint for `key` if it was saved in `now`'s reset week. A checkpoint from an earlier
/// week is cleared and `None` is returned.
pub fn load_current(store: &dyn CheckpointStore, key: CheckpointKey, now: UnixSeconds) -> Option<Checkpoint> {
    let cp = store.load(key)?;
    if cp.week == reset_week(now) {
        Some(cp)
    } else {
        store.clear(key);
        None
    }
}

/// An in-memory [`CheckpointStore`], for tests and for running without the character store.
#[derive(Default)]
pub struct MemoryCheckpointStore {
    inner: Mutex<MemoryInner>,
}

#[derive(Default)]
struct MemoryInner {
    checkpoints: HashMap<CheckpointKey, Checkpoint>,
    looted: HashSet<(CheckpointKey, String, u64)>,
}

impl MemoryCheckpointStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// How many checkpoints are held (tests).
    pub fn len(&self) -> usize {
        self.inner.lock().expect("checkpoint store poisoned").checkpoints.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl CheckpointStore for MemoryCheckpointStore {
    fn load(&self, key: CheckpointKey) -> Option<Checkpoint> {
        self.inner.lock().expect("checkpoint store poisoned").checkpoints.get(&key).cloned()
    }

    fn save(&self, checkpoint: Checkpoint) {
        self.inner.lock().expect("checkpoint store poisoned").checkpoints.insert(checkpoint.key, checkpoint);
    }

    fn clear(&self, key: CheckpointKey) {
        self.inner.lock().expect("checkpoint store poisoned").checkpoints.remove(&key);
    }

    fn mark_looted(&self, key: CheckpointKey, encounter_id: &str, week: u64) {
        self.inner.lock().expect("checkpoint store poisoned").looted.insert((key, encounter_id.to_owned(), week));
    }

    fn is_looted(&self, key: CheckpointKey, encounter_id: &str, week: u64) -> bool {
        self.inner.lock().expect("checkpoint store poisoned").looted.contains(&(key, encounter_id.to_owned(), week))
    }
}

/// The store's `step` for a checkpoint: `ENCOUNTER_INDEX ENCOUNTER_ID week=WEEK`.
pub fn format_step(cp: &Checkpoint) -> String {
    format!("{} {} week={}", cp.encounter_index, cp.encounter_id, cp.week)
}

/// A store `step` back into (encounter index, encounter id, week); `None` for `-` (spent) or
/// anything else.
pub fn parse_step(step: &str) -> Option<(usize, String, u64)> {
    let mut f = step.split_whitespace();
    let index = f.next()?.parse().ok()?;
    let id = f.next()?.to_owned();
    let week = f.next()?.strip_prefix("week=")?.parse().ok()?;
    f.next().is_none().then_some((index, id, week))
}

/// `characters/activity-table.tsv` (`index<TAB>0xhash`): manifest hash → the client's index.
/// The first index wins where a hash repeats.
pub fn read_activity_table(text: &str) -> HashMap<u32, u16> {
    let mut m = HashMap::new();
    for line in text.lines().skip(1) {
        let mut f = line.split('\t');
        let (Some(i), Some(h)) = (f.next(), f.next()) else { continue };
        let (Ok(i), Ok(h)) = (i.trim().parse::<u16>(), u32::from_str_radix(h.trim().trim_start_matches("0x"), 16)) else { continue };
        m.entry(h).or_insert(i);
    }
    m
}

/// The responder's store through its control file, with a local mirror for reads.
pub struct ControlFileCheckpointStore {
    path: std::path::PathBuf,
    character: CharacterId,
    index_of: HashMap<u32, u16>,
    mirror: MemoryCheckpointStore,
}

impl ControlFileCheckpointStore {
    /// `path` is the responder's `D1_STORE_CONTROL`; `character` the store's signed-on character.
    pub fn new(path: impl Into<std::path::PathBuf>, character: CharacterId, index_of: HashMap<u32, u16>) -> Self {
        Self { path: path.into(), character, index_of, mirror: MemoryCheckpointStore::new() }
    }

    /// Put a checkpoint the launcher read from the store into the mirror (nothing is sent).
    pub fn seed(&self, cp: Checkpoint) {
        self.mirror.save(cp);
    }

    fn send(&self, key: CheckpointKey, step: &str) {
        if key.character != self.character {
            return;
        }
        let Some(index) = self.index_of.get(&key.activity_hash) else {
            tracing::warn!("checkpoint: activity {:#010x} has no client index; not sent to the store", key.activity_hash);
            return;
        };
        let line = format!("checkpoint {index} {step}\n");
        let r = std::fs::OpenOptions::new().create(true).append(true).open(&self.path).and_then(|mut f| {
            use std::io::Write;
            f.write_all(line.as_bytes())
        });
        if let Err(e) = r {
            tracing::warn!("checkpoint: {}: {e}", self.path.display());
        }
    }
}

impl CheckpointStore for ControlFileCheckpointStore {
    fn load(&self, key: CheckpointKey) -> Option<Checkpoint> {
        self.mirror.load(key)
    }

    fn save(&self, checkpoint: Checkpoint) {
        self.send(checkpoint.key, &format_step(&checkpoint));
        self.mirror.save(checkpoint);
    }

    fn clear(&self, key: CheckpointKey) {
        if self.mirror.load(key).is_some() {
            self.send(key, "-");
        }
        self.mirror.clear(key);
    }

    fn mark_looted(&self, key: CheckpointKey, encounter_id: &str, week: u64) {
        self.mirror.mark_looted(key, encounter_id, week);
    }

    fn is_looted(&self, key: CheckpointKey, encounter_id: &str, week: u64) -> bool {
        self.mirror.is_looted(key, encounter_id, week)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: CheckpointKey = CheckpointKey { character: 7, activity_hash: 0x6D7C_C3BC };

    #[test]
    fn weeks_turn_over_on_tuesday_at_nine_utc() {
        // 2026-11-03 was a Tuesday.
        let tue_0859 = 1_793_696_340; // 2026-11-03 08:59:00 UTC
        let tue_0900 = tue_0859 + 60;
        assert_eq!(reset_week(tue_0900), reset_week(tue_0859) + 1);
        assert_eq!(week_start(reset_week(tue_0900)), tue_0900);
        // A week later, the next one.
        assert_eq!(reset_week(tue_0900 + WEEK_SECONDS), reset_week(tue_0900) + 1);
        // Monday night is still the same week as the Tuesday before.
        assert_eq!(reset_week(tue_0900 + 6 * 86_400 + 23 * 3_600), reset_week(tue_0900));
        assert_eq!(reset_week(0), 0);
    }

    #[test]
    fn a_checkpoint_from_last_week_is_spent() {
        let store = MemoryCheckpointStore::new();
        let now = 1_793_700_000;
        store.save(Checkpoint { key: KEY, encounter_index: 2, encounter_id: "deathsinger".into(), week: reset_week(now) - 1 });
        assert!(load_current(&store, KEY, now).is_none());
        assert!(store.is_empty(), "the stale checkpoint is cleared");
        store.save(Checkpoint { key: KEY, encounter_index: 2, encounter_id: "deathsinger".into(), week: reset_week(now) });
        assert_eq!(load_current(&store, KEY, now).map(|c| c.encounter_index), Some(2));
    }

    #[test]
    fn the_control_file_carries_saves_and_clears_for_the_store_character() {
        let dir = std::env::temp_dir().join(format!("d1-cp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("store-control.txt");
        let _ = std::fs::remove_file(&path);
        let table = read_activity_table("index\thash\n15\t0x6d7cc3bc\n16\t0x6d7cc3bf\n");
        assert_eq!(table.get(&0x6D7C_C3BC), Some(&15));
        let store = ControlFileCheckpointStore::new(&path, 7, table);
        let cp = Checkpoint { key: KEY, encounter_index: 1, encounter_id: "the-bridge".into(), week: 2965 };
        store.save(cp.clone());
        store.save(Checkpoint { key: CheckpointKey { character: 8, ..KEY }, ..cp.clone() }); // not the store's
        assert_eq!(store.load(KEY), Some(cp.clone()));
        store.clear(KEY);
        store.clear(KEY); // nothing left to clear: nothing sent
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text, "checkpoint 15 1 the-bridge week=2965\ncheckpoint 15 -\n");
        assert_eq!(parse_step(&format_step(&cp)), Some((1, "the-bridge".into(), 2965)));
        assert_eq!(parse_step("-"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lockouts_are_per_week_and_encounter() {
        let store = MemoryCheckpointStore::new();
        store.mark_looted(KEY, "the-abyss", 10);
        assert!(store.is_looted(KEY, "the-abyss", 10));
        assert!(!store.is_looted(KEY, "the-abyss", 11));
        assert!(!store.is_looted(KEY, "the-bridge", 10));
        let other = CheckpointKey { character: 8, ..KEY };
        assert!(!store.is_looted(other, "the-abyss", 10));
    }
}
