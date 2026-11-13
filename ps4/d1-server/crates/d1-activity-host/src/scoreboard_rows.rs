// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! # Stream R: the Crucible's two row images, the whole scoreboard and the lifetime's options
//!
//! [`crate::row_encoders`] writes the scoreboard (kind 15) only partly: player row 0's stats into
//! an image the world's list already served. A Crucible match needs every row: each player, each
//! team, the global block and the notifications. This module builds the whole `0x808008D0` image
//! from a [`ScoreboardState`] (BullShcript's shape, [`crate::pvp`]), and the lifetime value's
//! named-value list, where `score_to_win` can ride. Both images go out through the host's one wire
//! codec ([`crate::codec::AhWireCodec`], by way of [`crate::row_encoders::CodecWorker`]); this
//! module encodes nothing itself.
//!
//! ## The scoreboard image (`0x808008D0`, 19,792 bytes)
//!
//! Every offset below is the 56578 descriptor's (`ps4/fixtures/56578/rtti/fields.tsv`, checked by
//! a test), and the player-row layout is the one the post-game run served natively (sense49-51,
//! notes/ps4-56578-postgame-2026-10-29.md):
//!
//! | Offset | Field | Written from |
//! |---|---|---|
//! | `+0` | player count (i32, 5 bits) | the players, at most 16 |
//! | `+0x10 + 0x4A0 * i` | player row `i` (`0x808006E0`) | |
//! | row `+0` | the native player identity (kind `0x123`, 8 bytes) | the player key: the 40-byte identity's `+0x18`, byte-reversed, as the player bundle's `+0x40` |
//! | row `+8` | a bool, meaning open | 0, as the post-game run served it |
//! | row `+0xC` | stat count (i32, 7 bits) | the column table's length |
//! | row `+0x10 + 4 * c` | stat column `c` (f32) | the stat the column table names |
//! | row `+0x110` | rewards count (i32, 3 bits) | 0: rewards reach the client through the profile |
//! | `+0x4A10` | team count (i32, 4 bits), rows at `+0x4A14`, stride 56 (`0x808008A2`) | the teams, at most 12 |
//! | team `+0` | team (i8, 6 bits) | the team id |
//! | team `+1` | a bool, meaning open | 0 |
//! | team `+4` | stat count (i32, 4 bits), stats (f32) at `+8` | the team column table |
//! | `+0x4CB4` | global count (i32, 5 bits), values (f32) at `+0x4CB8` | the global column table |
//! | `+0x4D18` | notification count (i32, 2 bits), rows at `+0x4D1C`, stride 16 (`0x8080099A`) | the notifications, at most 2 |
//! | note `+0` / `+4` / `+0xC` | name hash (u32) / identity (8 bytes) / team (i8) | |
//! | `+0x4D3C` | the display category (raw u32) | FNV-1 of nothing, as the post-game run served it |
//! | `+0x4D40` | `0x8080055B` (i32, raw u64), meaning open | 0 |
//!
//! **The columns are content, and only three are known.** A stat block is a list of f32 slots in
//! the order of the activity mode's scoreboard definition. The post-game run read definition
//! `0x80AA3EE6`'s list (23 stats: `kills` at 0, `precision_kills` at 18, `deaths` at 22, each the
//! FNV-1 of its name). [`Columns::default`] is that list with the 20 unnamed slots left empty;
//! `D1_AH_CRUCIBLE_COLUMNS` replaces it. Which definition each Crucible mode uses, and the team and
//! global lists, are not in the exported content (the strikes/raids/Crucible note): the team list
//! defaults to `score` in slot 0, a **stand-in**, and the global list to nothing.
//!
//! ## The lifetime's named values (`score_to_win`)
//!
//! The client reads `score_to_win` (FNV-1 `0xBE4DB480`) in one place, `0x4001A0`, through the
//! option lookup `0x118870`. That lookup binary-searches a list of `{i32 count; 24-byte entries
//! {u32 name hash; typed value}}` and takes an entry whose typed value has the int type
//! (`[entry+4]`) and whose value is at `[entry+8]`. The 56578 descriptors have exactly one such
//! list type, `0x8080533D` (a 6-bit count and 50 entries of `0x80800D17`: a raw u32 and a kind
//! `0x29` typed value), and it sits in two replicated records: **the lifetime value at `+0x10`**
//! and the global activity state's `0x80800CC5` at `+4` (blob `+0x64`). Which of the two the
//! lookup's manager object returns is not read yet. [`lifetime_with_options`] writes the lifetime's
//! list, because the Crucible driver owns that row; [`crate::messages`] deliberately refuses a
//! non-empty list in the global state.
//!
//! **The int type's tag is a candidate, not a binding.** `0x4001A0` compares the entry's type with
//! the cell behind pointer slot `0x1996058` (relocated to `0x28DBD70`), which the type-cell table at
//! `0x1998030` pairs with name hash `0xA801403A`. No anchored descriptor carries that hash where the
//! 965 body's did (anchor - `0x48`); the nearest descriptor is `0x80800007`, a 4-byte biased i32,
//! with the hash at its anchor - `0x60`. So the tag is configuration (`D1_AH_CRUCIBLE_SCORE_TO_WIN`),
//! and nothing is sent by default.

use crate::pvp::{fnv1, ScoreboardState};

pub const TAG: u32 = crate::row_encoders::TAG_SCOREBOARD;
pub const SIZE: usize = 0x4D50;

pub const PLAYER_COUNT: usize = 0x0;
pub const PLAYER_ROWS: usize = 0x10;
pub const PLAYER_STRIDE: usize = 0x4A0;
pub const PLAYER_CAPACITY: usize = 16;
pub const ROW_IDENTITY: usize = 0x0;
pub const ROW_FLAG: usize = 0x8;
pub const ROW_STAT_COUNT: usize = 0xC;
pub const ROW_STATS: usize = 0x10;
pub const ROW_STAT_CAPACITY: usize = 64;
pub const ROW_REWARD_COUNT: usize = 0x110;

pub const TEAM_COUNT: usize = 0x4A10;
pub const TEAM_ROWS: usize = 0x4A14;
pub const TEAM_STRIDE: usize = 56;
pub const TEAM_CAPACITY: usize = 12;
pub const TEAM_ID: usize = 0x0;
pub const TEAM_STAT_COUNT: usize = 0x4;
pub const TEAM_STATS: usize = 0x8;
pub const TEAM_STAT_CAPACITY: usize = 12;

pub const GLOBAL_COUNT: usize = 0x4CB4;
pub const GLOBAL_VALUES: usize = 0x4CB8;
pub const GLOBAL_CAPACITY: usize = 24;

pub const NOTE_COUNT: usize = 0x4D18;
pub const NOTE_ROWS: usize = 0x4D1C;
pub const NOTE_STRIDE: usize = 16;
pub const NOTE_CAPACITY: usize = 2;

pub const CATEGORY: usize = 0x4D3C;

/// FNV-1 of nothing: "no category", as the post-game run served it.
pub const NONE_HASH: u32 = 0x811C_9DC5;

/// The lifetime value's named-value list (`0x8080533D` at `+0x10`).
pub const LIFETIME_OPTIONS: usize = 0x10;
pub const OPTION_STRIDE: usize = 24;
pub const OPTION_CAPACITY: usize = 50;
/// `score_to_win`, the one option the client reads (`0x4001A0`).
pub const SCORE_TO_WIN: u32 = 0xBE4D_B480;

/// The stat columns, by stat name hash, in each block's order. `None` is a slot the definition
/// has but this table cannot name; it is sent as 0.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Columns {
    pub player: Vec<Option<u32>>,
    pub team: Vec<Option<u32>>,
    pub global: Vec<Option<u32>>,
}

impl Default for Columns {
    /// Definition `0x80AA3EE6`'s 23 player stats as far as they are named (kills 0, precision
    /// kills 18, deaths 22); the team score in team slot 0 (**stand-in**); no global stats.
    fn default() -> Self {
        let mut player = vec![None; 23];
        player[0] = Some(fnv1("kills"));
        player[18] = Some(fnv1("precision_kills"));
        player[22] = Some(fnv1("deaths"));
        Self { player, team: vec![Some(fnv1("score"))], global: Vec::new() }
    }
}

impl Columns {
    /// Parse a column file: three sections `player:`, `team:` and `global:` (a bare list is the
    /// player section), one column per line in order, each a stat name, an FNV-1 hash (`0x…`) or
    /// `-` for an unnamed slot. `#` starts a comment.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut c = Self { player: Vec::new(), team: Vec::new(), global: Vec::new() };
        let mut section = "player";
        let mut saw_team = false;
        for (n, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if let Some(s) = line.strip_suffix(':') {
                section = match s.trim() {
                    "player" => "player",
                    "team" => {
                        saw_team = true;
                        "team"
                    }
                    "global" => "global",
                    other => return Err(format!("line {}: unknown section {other:?}", n + 1)),
                };
                continue;
            }
            let v = if line == "-" {
                None
            } else if let Some(h) = line.strip_prefix("0x") {
                Some(u32::from_str_radix(h, 16).map_err(|e| format!("line {}: {e}", n + 1))?)
            } else {
                Some(fnv1(line))
            };
            match section {
                "player" => c.player.push(v),
                "team" => c.team.push(v),
                _ => c.global.push(v),
            }
        }
        if !saw_team {
            c.team = Self::default().team;
        }
        if c.player.len() > ROW_STAT_CAPACITY || c.team.len() > TEAM_STAT_CAPACITY || c.global.len() > GLOBAL_CAPACITY {
            return Err(format!(
                "too many columns: {} player (max {ROW_STAT_CAPACITY}), {} team (max {TEAM_STAT_CAPACITY}), {} global (max {GLOBAL_CAPACITY})",
                c.player.len(),
                c.team.len(),
                c.global.len()
            ));
        }
        Ok(c)
    }

    /// `D1_AH_CRUCIBLE_COLUMNS` (a file), else the default.
    pub fn from_env() -> Result<Self, String> {
        match std::env::var("D1_AH_CRUCIBLE_COLUMNS") {
            Ok(p) if !p.trim().is_empty() => {
                let text = std::fs::read_to_string(p.trim()).map_err(|e| format!("{}: {e}", p.trim()))?;
                Self::parse(&text)
            }
            _ => Ok(Self::default()),
        }
    }
}

fn put_i32(b: &mut [u8], at: usize, v: i32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn put_f32(b: &mut [u8], at: usize, v: f64) {
    b[at..at + 4].copy_from_slice(&(v as f32).to_le_bytes());
}

fn count(n: usize) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX)
}

/// The whole scoreboard image. `identity_of` gives the 8-byte native identity of a player by its
/// `character_key`; a player it cannot name is left out (the client could not place the row).
/// Rows past a block's capacity are dropped, players first by their order in `state`.
pub fn image(state: &ScoreboardState, columns: &Columns, identity_of: &dyn Fn(&str) -> Option<[u8; 8]>) -> Vec<u8> {
    let mut b = vec![0u8; SIZE];
    let fill = |b: &mut [u8], at: usize, cols: &[Option<u32>], stats: &dyn Fn(u32) -> Option<f64>| {
        for (i, c) in cols.iter().enumerate() {
            let v = c.and_then(stats).unwrap_or(0.0);
            put_f32(b, at + 4 * i, v);
        }
    };
    let players: Vec<_> = state.players.iter().filter_map(|p| identity_of(&p.character_key).map(|id| (p, id))).take(PLAYER_CAPACITY).collect();
    put_i32(&mut b, PLAYER_COUNT, count(players.len()));
    for (i, (p, id)) in players.iter().enumerate() {
        let row = PLAYER_ROWS + PLAYER_STRIDE * i;
        b[row + ROW_IDENTITY..row + ROW_IDENTITY + 8].copy_from_slice(id);
        b[row + ROW_FLAG] = 0;
        put_i32(&mut b, row + ROW_STAT_COUNT, count(columns.player.len()));
        let stat = |h: u32| p.stats.iter().find(|s| s.name_hash == h).map(|s| s.value);
        fill(&mut b, row + ROW_STATS, &columns.player, &stat);
        put_i32(&mut b, row + ROW_REWARD_COUNT, 0);
    }
    let teams: Vec<_> = state.teams.iter().take(TEAM_CAPACITY).collect();
    put_i32(&mut b, TEAM_COUNT, count(teams.len()));
    for (i, t) in teams.iter().enumerate() {
        let row = TEAM_ROWS + TEAM_STRIDE * i;
        b[row + TEAM_ID] = i8::try_from(t.team).unwrap_or(-1) as u8;
        put_i32(&mut b, row + TEAM_STAT_COUNT, count(columns.team.len()));
        let stat = |h: u32| t.stats.iter().find(|s| s.name_hash == h).map(|s| s.value);
        fill(&mut b, row + TEAM_STATS, &columns.team, &stat);
    }
    put_i32(&mut b, GLOBAL_COUNT, count(columns.global.len()));
    let global = |h: u32| state.global.iter().find(|s| s.name_hash == h).map(|s| s.value);
    fill(&mut b, GLOBAL_VALUES, &columns.global, &global);
    let notes: Vec<_> = state.notifications.iter().take(NOTE_CAPACITY).collect();
    put_i32(&mut b, NOTE_COUNT, count(notes.len()));
    for (i, n) in notes.iter().enumerate() {
        let row = NOTE_ROWS + NOTE_STRIDE * i;
        b[row..row + 4].copy_from_slice(&n.name_hash.to_le_bytes());
        if let Some(id) = n.character_key.as_deref().and_then(identity_of) {
            b[row + 4..row + 12].copy_from_slice(&id);
        }
        b[row + 0xC] = i8::try_from(n.team).unwrap_or(-1) as u8;
    }
    b[CATEGORY..CATEGORY + 4].copy_from_slice(&NONE_HASH.to_le_bytes());
    b
}

/// The 8 bytes a player row names a player by: the same kind-`0x123` player key the player bundle
/// carries at A `+0x40` and the cinematic row at A `+0xA0` ([`crate::row_encoders::player_key_of_identity`]:
/// the 40-byte identity's `+0x18`, byte-reversed). The post-game run served codec image
/// `d100000000000201` for native `01020000000000d1` (sense49-51,
/// `ps4/fixtures/56578/postgame/agr-postgame-scoreboard-arcadia-ratio30.draft.txt`). Until run sb1
/// this read `+0x08`, a different field: the screen stayed empty.
pub fn native_identity(identity40: &[u8]) -> Option<[u8; 8]> {
    crate::row_encoders::player_key_of_identity(identity40)
}

/// One named int value: `{u32 name hash; typed value {u32 type tag; i32 value}}`, 24 bytes.
pub fn named_int(name_hash: u32, int_tag: u32, value: i32) -> [u8; OPTION_STRIDE] {
    let mut e = [0u8; OPTION_STRIDE];
    e[0..4].copy_from_slice(&name_hash.to_le_bytes());
    e[4..8].copy_from_slice(&int_tag.to_le_bytes());
    e[8..12].copy_from_slice(&value.to_le_bytes());
    e
}

/// Write `entries` into a lifetime value image's named-value list (`+0x10`), sorted by hash as
/// the client's binary search (`0x118540`) needs. More than 50 entries is refused.
pub fn lifetime_with_options(image: &mut [u8], entries: &[[u8; OPTION_STRIDE]]) -> Result<(), String> {
    if entries.len() > OPTION_CAPACITY {
        return Err(format!("{} named values; the list holds {OPTION_CAPACITY}", entries.len()));
    }
    let end = LIFETIME_OPTIONS + 4 + OPTION_STRIDE * entries.len();
    if image.len() < end {
        return Err(format!("a lifetime image of {} bytes has no room for its list (needs {end})", image.len()));
    }
    let mut sorted = entries.to_vec();
    sorted.sort_by_key(|e| u32::from_le_bytes(e[0..4].try_into().unwrap()));
    put_i32(image, LIFETIME_OPTIONS, count(sorted.len()));
    for (i, e) in sorted.iter().enumerate() {
        let at = LIFETIME_OPTIONS + 4 + OPTION_STRIDE * i;
        image[at..at + OPTION_STRIDE].copy_from_slice(e);
    }
    Ok(())
}

/// What `0x118870` + `0x4001A0` would read from a list written by [`lifetime_with_options`]:
/// the value of `name_hash` when its type is `int_tag`.
pub fn read_option(image: &[u8], name_hash: u32, int_tag: u32) -> Option<i32> {
    let u = |at: usize| image.get(at..at + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap()));
    let n = u(LIFETIME_OPTIONS)? as usize;
    (0..n.min(OPTION_CAPACITY)).find_map(|i| {
        let at = LIFETIME_OPTIONS + 4 + OPTION_STRIDE * i;
        (u(at)? == name_hash && u(at + 4)? == int_tag).then(|| u(at + 8).map(|v| v as i32)).flatten()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pvp::{Notification, PlayerStatsEntry, StatEntry, TeamStatsEntry};

    fn tsv(name: &str) -> String {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/56578/rtti");
        std::fs::read_to_string(dir.join(name)).expect(name)
    }

    /// `(offset, kind, child tag)` of each field of `tag`, and its struct size, from the fixture.
    fn layout(tag: u32) -> (usize, Vec<(usize, u32, u32)>) {
        let t = |s: &str| u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap();
        let size = tsv("schemas.tsv")
            .lines()
            .skip(1)
            .map(|l| l.trim_end_matches('\r').split('\t').map(str::to_string).collect::<Vec<_>>())
            .find(|f| t(&f[0]) == tag)
            .map(|f| f[2].parse().unwrap())
            .unwrap_or_else(|| panic!("{tag:#x} not in schemas.tsv"));
        let fields = tsv("fields.tsv")
            .lines()
            .skip(1)
            .map(|l| l.trim_end_matches('\r').split('\t').map(str::to_string).collect::<Vec<_>>())
            .filter(|f| t(&f[0]) == tag)
            .map(|f| (f[4].parse().unwrap(), f[5].parse().unwrap(), f[6].parse::<u64>().unwrap() as u32))
            .collect();
        (size, fields)
    }

    #[test]
    fn the_offsets_are_the_56578_descriptors() {
        let (size, root) = layout(TAG);
        assert_eq!(size, SIZE);
        let at = |f: &[(usize, u32, u32)], i: usize| f[i].0;
        assert_eq!((at(&root, 0), at(&root, 1), at(&root, 2), at(&root, 3), at(&root, 4)), (PLAYER_COUNT, TEAM_COUNT, GLOBAL_COUNT, NOTE_COUNT, CATEGORY));
        // Players: count @0, the row array @16, rows of 1,184 bytes.
        let (_, players) = layout(root[0].2);
        assert_eq!((players[0].0, players[1].0), (0, PLAYER_ROWS));
        let (row_size, row) = layout(0x8080_06E0);
        assert_eq!(row_size, PLAYER_STRIDE);
        assert_eq!(row.iter().map(|f| f.0).collect::<Vec<_>>(), vec![ROW_IDENTITY, ROW_FLAG, ROW_STAT_COUNT, 272]);
        assert_eq!(row[0].1 & 0xFF, 0x23, "the row names its player by a native identity");
        let (stats_size, stats) = layout(row[2].2);
        assert_eq!((stats[0].0, stats[1].0, stats_size), (0, 4, 4 + 4 * ROW_STAT_CAPACITY));
        assert_eq!(ROW_STATS, ROW_STAT_COUNT + 4);
        assert_eq!(ROW_REWARD_COUNT, 272);
        // Teams: count @0, rows @4 of 56 bytes: team i8 @0, bool @1, stats @4.
        let (_, teams) = layout(root[1].2);
        assert_eq!((teams[0].0, teams[1].0), (0, TEAM_ROWS - TEAM_COUNT));
        let (team_size, team) = layout(0x8080_08A2);
        assert_eq!(team_size, TEAM_STRIDE);
        assert_eq!(team.iter().map(|f| f.0).collect::<Vec<_>>(), vec![TEAM_ID, 1, TEAM_STAT_COUNT]);
        let (tstats_size, tstats) = layout(team[2].2);
        assert_eq!((tstats[1].0, tstats_size), (4, 4 + 4 * TEAM_STAT_CAPACITY));
        // Global: count @0, 24 values @4. Notifications: count @0, two rows of 16 @4.
        let (gsize, global) = layout(root[2].2);
        assert_eq!((global[1].0, gsize), (GLOBAL_VALUES - GLOBAL_COUNT, 4 + 4 * GLOBAL_CAPACITY));
        let (_, notes) = layout(root[3].2);
        assert_eq!(notes[1].0, NOTE_ROWS - NOTE_COUNT);
        let (nsize, note) = layout(0x8080_099A);
        assert_eq!(nsize, NOTE_STRIDE);
        assert_eq!(note.iter().map(|f| f.0).collect::<Vec<_>>(), vec![0, 4, 0xC]);
        // The lifetime value's named-value list: 0x8080533D at +0x10, entries of 24 bytes.
        let (_, lifetime) = layout(crate::row_encoders::TAG_LIFETIME);
        let list = lifetime.iter().find(|f| f.0 == LIFETIME_OPTIONS).expect("a field at +0x10");
        assert_eq!(list.2, 0x8080_533D);
        let (_, nv) = layout(0x8080_533D);
        assert_eq!(nv[1].0, 4, "entries follow the count");
        let (esize, entry) = layout(0x8080_0D17);
        assert_eq!(esize, OPTION_STRIDE);
        assert_eq!((entry[0].0, entry[1].0, entry[1].1), (0, 4, 0x29), "a name hash, then a typed value");
    }

    fn state() -> ScoreboardState {
        let s = |n: &str, v: f64| StatEntry { name_hash: fnv1(n), value: v };
        ScoreboardState {
            global: vec![],
            notifications: vec![Notification { character_key: Some("2".into()), name_hash: 0x1234, team: 1 }],
            players: vec![
                PlayerStatsEntry { character_key: "1".into(), stats: vec![s("kills", 3.0), s("deaths", 1.0), s("score", 300.0)], team: Some(0) },
                PlayerStatsEntry { character_key: "2".into(), stats: vec![s("deaths", 3.0)], team: Some(1) },
                PlayerStatsEntry { character_key: "9".into(), stats: vec![s("kills", 9.0)], team: Some(1) },
            ],
            teams: vec![TeamStatsEntry { stats: vec![s("score", 300.0)], team: 0 }, TeamStatsEntry { stats: vec![], team: 1 }],
        }
    }

    fn ids(k: &str) -> Option<[u8; 8]> {
        match k {
            "1" => Some([1, 0, 0, 0, 0, 0, 0, 0xD1]),
            "2" => Some([2, 0, 0, 0, 0, 0, 0, 0xD1]),
            _ => None,
        }
    }

    #[test]
    fn the_image_carries_every_player_and_team() {
        let b = image(&state(), &Columns::default(), &ids);
        let i = |at: usize| i32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let f = |at: usize| f32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        assert_eq!(b.len(), SIZE);
        assert_eq!(i(PLAYER_COUNT), 2, "a player with no identity is left out");
        let r1 = PLAYER_ROWS + PLAYER_STRIDE;
        assert_eq!(&b[PLAYER_ROWS..PLAYER_ROWS + 8], &[1, 0, 0, 0, 0, 0, 0, 0xD1]);
        assert_eq!(&b[r1..r1 + 8], &[2, 0, 0, 0, 0, 0, 0, 0xD1]);
        assert_eq!(i(PLAYER_ROWS + ROW_STAT_COUNT), 23);
        assert_eq!(f(PLAYER_ROWS + ROW_STATS), 3.0, "kills in column 0");
        assert_eq!(f(PLAYER_ROWS + ROW_STATS + 4 * 22), 1.0, "deaths in column 22");
        assert_eq!(f(r1 + ROW_STATS + 4 * 22), 3.0);
        assert_eq!(i(TEAM_COUNT), 2);
        assert_eq!(b[TEAM_ROWS + TEAM_ID], 0);
        assert_eq!(b[TEAM_ROWS + TEAM_STRIDE + TEAM_ID], 1);
        assert_eq!(i(TEAM_ROWS + TEAM_STAT_COUNT), 1);
        assert_eq!(f(TEAM_ROWS + TEAM_STATS), 300.0, "the team score (stand-in column)");
        assert_eq!(i(GLOBAL_COUNT), 0);
        assert_eq!(i(NOTE_COUNT), 1);
        assert_eq!(&b[NOTE_ROWS..NOTE_ROWS + 4], &0x1234u32.to_le_bytes());
        assert_eq!(&b[NOTE_ROWS + 4..NOTE_ROWS + 12], &[2, 0, 0, 0, 0, 0, 0, 0xD1]);
        assert_eq!(b[NOTE_ROWS + 0xC], 1);
        assert_eq!(&b[CATEGORY..CATEGORY + 4], &NONE_HASH.to_le_bytes());
    }

    #[test]
    fn a_column_file_names_slots_by_name_or_hash() {
        let c = Columns::parse("kills\n-\n0xcfc7f8d0 # deaths\nteam:\nscore\nglobal:\n").unwrap();
        assert_eq!(c.player, vec![Some(fnv1("kills")), None, Some(fnv1("deaths"))]);
        assert_eq!(c.team, vec![Some(fnv1("score"))]);
        assert!(c.global.is_empty());
        let c = Columns::parse("kills\n").unwrap();
        assert_eq!(c.team, Columns::default().team, "no team section keeps the default");
        assert!(Columns::parse(&"kills\n".repeat(65)).is_err());
        assert!(Columns::parse("bogus:\n").is_err());
    }

    #[test]
    fn score_to_win_reads_back_the_way_the_client_looks_it_up() {
        let mut life = crate::row_encoders::lifetime_image(3, None, None);
        let tag = 0x8080_0007;
        lifetime_with_options(&mut life, &[named_int(0x0000_0001, tag, 9), named_int(SCORE_TO_WIN, tag, 15000)]).unwrap();
        assert_eq!(read_option(&life, SCORE_TO_WIN, tag), Some(15000));
        assert_eq!(read_option(&life, SCORE_TO_WIN, 0x8080_0008), None, "another type is not the int");
        assert_eq!(life[0], 3, "the state stays");
        // Sorted for the binary search.
        assert_eq!(u32::from_le_bytes(life[0x14..0x18].try_into().unwrap()), 1);
        assert!(lifetime_with_options(&mut life, &vec![named_int(1, tag, 0); 51]).is_err());
    }

    /// With the owned eboot: both images encode natively (d1-rsat), and decode back to the same
    /// fields. The client's codec path needs the int type registered as a typed value first.
    #[test]
    fn the_images_encode_through_native_rsat() {
        if d1_rsat::schemas().is_err() {
            eprintln!("SKIP the_images_encode_through_native_rsat: needs the owned eboot (D1_EBOOT)");
            return;
        }
        let b = image(&state(), &Columns::default(), &ids);
        let e = d1_rsat::encode(TAG, &b).expect("the scoreboard encodes natively");
        let d = d1_rsat::decode(TAG, &e.payload).expect("and decodes");
        assert_eq!(&d.object[..PLAYER_ROWS + PLAYER_STRIDE * 2], &b[..PLAYER_ROWS + PLAYER_STRIDE * 2], "player rows");
        assert_eq!(&d.object[TEAM_COUNT..TEAM_ROWS + TEAM_STRIDE * 2], &b[TEAM_COUNT..TEAM_ROWS + TEAM_STRIDE * 2], "team rows");
        let mut life = crate::row_encoders::lifetime_image(3, None, None);
        lifetime_with_options(&mut life, &[named_int(SCORE_TO_WIN, 0x8080_0007, 15000)]).unwrap();
        let e = d1_rsat::encode(crate::row_encoders::TAG_LIFETIME, &life).expect("the lifetime with a named int encodes natively");
        let d = d1_rsat::decode(crate::row_encoders::TAG_LIFETIME, &e.payload).expect("and decodes");
        assert_eq!(read_option(&d.object, SCORE_TO_WIN, 0x8080_0007), Some(15000));
    }

    /// The row names the player as the player bundle does: identity `+0x18` byte-reversed. The
    /// post-game fixture's pair: native `01020000000000d1` → codec image `d100000000000201`.
    #[test]
    fn a_player_row_names_the_player_by_the_bundle_key() {
        let mut identity = [0u8; 40];
        for (i, b) in identity.iter_mut().enumerate() {
            *b = 0x80 | i as u8; // every field distinct
        }
        identity[0x18..0x20].copy_from_slice(&[0x01, 0x02, 0, 0, 0, 0, 0, 0xD1]);
        let id = native_identity(&identity).expect("40 bytes");
        assert_eq!(id, [0xD1, 0, 0, 0, 0, 0, 0x02, 0x01]);
        assert_eq!(Some(id), crate::row_encoders::player_key_of_identity(&identity));
        assert_eq!(native_identity(&identity[..0x1F]), None, "a short identity names nobody");
    }
}
