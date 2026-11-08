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
//! [`parse_step`] are the step's two directions. The responder side (stream O) also takes:
//!
//! 1. **a clear**: `checkpoint INDEX -` (a spent checkpoint) is `clear_checkpoint(INDEX)`;
//! 2. **the loot lockouts**: `lockout INDEX ENCOUNTER_ID week=WEEK`, kept in the store's record
//!    (`lockouts`), so they survive a restart of either process;
//! 3. **a read-back**: after each batch of commands (and at sign-on) the store writes
//!    `<control>.state.json` ([`state_file_for`]) with the signed-on character, its checkpoints and
//!    its lockouts. [`ControlFileCheckpointStore`] reads it when its mirror has nothing, so a launch
//!    loads the stored checkpoint. [`ControlFileCheckpointStore::seed`] still takes what the
//!    launcher knows.
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

/// The file the responder's store writes back beside its control file (`d1-responder`'s
/// `player_store::state_file_for`): `store-control.txt` -> `store-control.state.json`.
pub fn state_file_for(control: &std::path::Path) -> std::path::PathBuf {
    control.with_extension("state.json")
}

/// What the store's state file says about one character: `(checkpoints by index, lockouts by
/// index then encounter)`.
pub type StoreState = (HashMap<u16, String>, HashMap<u16, HashMap<String, u64>>);

/// Read the store's state file for `character`. `None` when there is no file, it does not parse,
/// or it is another character's.
pub fn read_store_state(path: &std::path::Path, character: CharacterId) -> Option<StoreState> {
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let who = v.get("character")?.as_str()?;
    if u64::from_str_radix(who.trim_start_matches("0x"), 16).ok()? != character {
        return None;
    }
    let index = |k: &String| k.parse::<u16>().ok();
    let checkpoints = v.get("checkpoints").and_then(|c| c.as_object()).map(|m| {
        m.iter().filter_map(|(k, s)| Some((index(k)?, s.as_str()?.to_owned()))).collect()
    });
    let lockouts = v.get("lockouts").and_then(|c| c.as_object()).map(|m| {
        m.iter()
            .filter_map(|(k, l)| {
                let l = l.as_object()?.iter().filter_map(|(e, w)| Some((e.clone(), w.as_u64()?))).collect();
                Some((index(k)?, l))
            })
            .collect()
    });
    Some((checkpoints.unwrap_or_default(), lockouts.unwrap_or_default()))
}

/// The character the store's state file names (the signed-on one).
pub fn store_character(path: &std::path::Path) -> Option<CharacterId> {
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    u64::from_str_radix(v.get("character")?.as_str()?.trim_start_matches("0x"), 16).ok()
}

/// The responder's store through its control file, with a local mirror for reads and the store's
/// state file behind it.
pub struct ControlFileCheckpointStore {
    path: std::path::PathBuf,
    state: std::path::PathBuf,
    character: CharacterId,
    index_of: HashMap<u32, u16>,
    mirror: MemoryCheckpointStore,
}

impl ControlFileCheckpointStore {
    /// `path` is the responder's `D1_STORE_CONTROL`; `character` the store's signed-on character.
    pub fn new(path: impl Into<std::path::PathBuf>, character: CharacterId, index_of: HashMap<u32, u16>) -> Self {
        let path = path.into();
        let state = state_file_for(&path);
        Self { path, state, character, index_of, mirror: MemoryCheckpointStore::new() }
    }

    /// With the store on (`D1_STORE=1`, `D1_STORE_CONTROL`): the store's character from its state
    /// file, else `D1_CHARACTER_SOID` (the responder's default `0xD100_0000_0000_0101`), and the
    /// activity table (`D1_AH_ACTIVITY_TABLE`, else this checkout's `characters/activity-table.tsv`).
    pub fn from_env() -> Option<Self> {
        if !std::env::var("D1_STORE").is_ok_and(|v| v.trim() == "1") {
            return None;
        }
        let path = std::path::PathBuf::from(std::env::var("D1_STORE_CONTROL").ok().filter(|p| !p.trim().is_empty())?);
        let character = store_character(&state_file_for(&path)).unwrap_or_else(|| {
            std::env::var("D1_CHARACTER_SOID")
                .ok()
                .and_then(|v| u64::from_str_radix(v.trim().trim_start_matches("0x"), 16).ok())
                .unwrap_or(0xD100_0000_0000_0101)
        });
        let table = std::env::var("D1_AH_ACTIVITY_TABLE").unwrap_or_else(|_| {
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../../fixtures/56578/characters/activity-table.tsv").to_string()
        });
        let index_of = read_activity_table(&std::fs::read_to_string(&table).ok()?);
        Some(Self::new(path, character, index_of))
    }

    /// The store character this host sends for.
    pub fn character(&self) -> CharacterId {
        self.character
    }

    /// Put a checkpoint the launcher read from the store into the mirror (nothing is sent).
    pub fn seed(&self, cp: Checkpoint) {
        self.mirror.save(cp);
    }

    fn index(&self, key: CheckpointKey) -> Option<u16> {
        if key.character != self.character {
            return None;
        }
        let index = self.index_of.get(&key.activity_hash).copied();
        if index.is_none() {
            tracing::warn!("checkpoint: activity {:#010x} has no client index; not sent to the store", key.activity_hash);
        }
        index
    }

    fn send(&self, key: CheckpointKey, verb: &str, rest: &str) {
        let Some(index) = self.index(key) else { return };
        if let Err(e) = crate::reward_forward::append_line(&self.path, &format!("{verb} {index} {rest}")) {
            tracing::warn!("checkpoint: {}: {e}", self.path.display());
        }
    }

    /// The store's state for this character (read each time: the responder rewrites it).
    fn stored(&self) -> Option<StoreState> {
        read_store_state(&self.state, self.character)
    }
}

impl CheckpointStore for ControlFileCheckpointStore {
    fn load(&self, key: CheckpointKey) -> Option<Checkpoint> {
        if let Some(cp) = self.mirror.load(key) {
            return Some(cp);
        }
        let index = self.index(key)?;
        let (checkpoints, _) = self.stored()?;
        let (encounter_index, encounter_id, week) = parse_step(checkpoints.get(&index)?)?;
        let cp = Checkpoint { key, encounter_index, encounter_id, week };
        self.mirror.save(cp.clone());
        Some(cp)
    }

    fn save(&self, checkpoint: Checkpoint) {
        self.send(checkpoint.key, "checkpoint", &format_step(&checkpoint));
        self.mirror.save(checkpoint);
    }

    fn clear(&self, key: CheckpointKey) {
        if self.load(key).is_some() {
            self.send(key, "checkpoint", "-");
        }
        self.mirror.clear(key);
    }

    fn mark_looted(&self, key: CheckpointKey, encounter_id: &str, week: u64) {
        if !encounter_id.is_empty() && !encounter_id.chars().any(char::is_whitespace) {
            self.send(key, "lockout", &format!("{encounter_id} week={week}"));
        }
        self.mirror.mark_looted(key, encounter_id, week);
    }

    fn is_looted(&self, key: CheckpointKey, encounter_id: &str, week: u64) -> bool {
        if self.mirror.is_looted(key, encounter_id, week) {
            return true;
        }
        let Some(index) = self.index(key) else { return false };
        self.stored().is_some_and(|(_, l)| l.get(&index).and_then(|e| e.get(encounter_id)) == Some(&week))
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

    /// Stream O: lockouts go to the store, and a fresh host reads checkpoints and lockouts back
    /// from the store's state file.
    #[test]
    fn lockouts_are_sent_and_the_store_state_is_read_back() {
        let dir = std::env::temp_dir().join(format!("d1-cp-state-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("store-control.txt");
        let _ = std::fs::remove_file(&path);
        let table = || read_activity_table("index\thash\n15\t0x6d7cc3bc\n");
        let store = ControlFileCheckpointStore::new(&path, 7, table());
        store.mark_looted(KEY, "the-abyss", 2965);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "lockout 15 the-abyss week=2965\n");
        // What the responder writes back (player_store::state_json), for character 7.
        let state = state_file_for(&path);
        assert_eq!(state, dir.join("store-control.state.json"));
        std::fs::write(&state, r#"{"schema":1,"character":"0x0000000000000007","checkpoints":{"15":"1 the-bridge week=2965"},"lockouts":{"15":{"the-abyss":2965}}}"#).unwrap();
        assert_eq!(store_character(&state), Some(7));
        let fresh = ControlFileCheckpointStore::new(&path, 7, table());
        assert_eq!(fresh.load(KEY), Some(Checkpoint { key: KEY, encounter_index: 1, encounter_id: "the-bridge".into(), week: 2965 }));
        assert!(fresh.is_looted(KEY, "the-abyss", 2965));
        assert!(!fresh.is_looted(KEY, "the-abyss", 2966));
        assert!(!fresh.is_looted(KEY, "the-bridge", 2965));
        // Another character's state is not this one's.
        let other = ControlFileCheckpointStore::new(&path, 8, table());
        assert!(other.load(CheckpointKey { character: 8, ..KEY }).is_none());
        // A clear of a read-back checkpoint is sent.
        std::fs::remove_file(&path).unwrap();
        fresh.clear(KEY);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "checkpoint 15 -\n");
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
