// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! ⭐⭐ **The known-world scenario table - the values rung 33 compares OUR blob against.**
//!
//! ## Why this table exists at all
//!
//! Rung 33 (`setup:activity_world_transition`) is a **sequence of blob ↔ scenario-record field
//! checks**, and every one of them is a property of the **WORLD**, not of the activity:
//!
//! | gate | rule | fn |
//! |---|---|---|
//! | checksum    | `blob+0x5C` **==** `scenario_record+0x0C` (PRIVATE; `+0x10` is PUBLIC) | `FUN_00299100` |
//! | bubble count| `blob+0x10` **==** `scenario_record+0x48`                             | `FUN_002A0320` |
//! | bubble state| `blob+0x14[i]` = `0x00` chosen / `0xFF` unchosen, per bubble          | `FUN_002A0320` |
//!
//! ⛔⛔ **The failure this table is here to make impossible.** Those three values and the activity
//! NAME are four knobs that must all describe *the same world*, and until now they were four
//! independent environment variables. Publishing one world's checksum while naming another
//! re-fails gate 1 - *the client compares against the record of the world it **actually loaded***,
//! not the one we claim - and the symptom is `world_controller:transition_error: activity checksum
//! mismatch`, which reads exactly like "the checksum value is wrong" rather than "the checksum
//! belongs to a different world". Deriving all three from the **name** collapses four knobs into
//! one and makes the disagreement a loud startup warning instead of a run-time gate.
//!
//! ⚠ **A launch is still TWO values, and this table only carries one of them.** The MODE - the
//! `u16` index into the client's 540-entry activity table at `global_activity_state+0x680` - is
//! **not** a property of the world and is **nowhere derived from it** (`0x80800CC5` field `[7]
//! @+0x620` u16 and field `[10] @+0x628` `char[0x28]` are two adjacent, independent fields; two
//! derivations agree). So [`crate::state::ActivityInstance::DEFAULT_ACTIVITY_INDEX`] is a separate
//! constant on purpose, and this table must never grow one.
//!
//! ## Provenance - `[C]` `observed-live`, and how each row was taken
//!
//! A world's scenario record is resident **only while that world is loading**, in the single
//! scenario slot `0x5A9`. Both rows were captured with the two-poke procedure in `CLAUDE.md`:
//! publish the target world with any checksum (gate 1 fails on purpose) while
//! `capture-rig/scenario-checksum.py watch --hz 500` samples the slot, then re-fire with the
//! captured values.
//!
//! ⚠⚠ `--hz 50` **misses the ~100 ms window** and reads like "the world never loaded" - a false
//! negative that costs an hour. `[C]`, measured both ways.
//!
//! ⚠ **Offline derivation (corrected 29 Oct 2026).** This note used to say the rows could not be
//! derived offline. They can: each world's owned `<world>:scenario_client` package record carries
//! the PRIVATE checksum at `+0x0C`, PUBLIC at `+0x10` and the bubble count at `+0x48`, and its
//! package symbol binds the world name (`+0x08` is FNV-1 of that name; `+0x30` is the
//! *destination* folder, e.g. `cosmo`, which is why a name/`+0x30` equality check rejects valid
//! records). Read from the 56578 packages, `orbit`, `act_cosmo_m0_highway` and
//! `act_cosmo_m1_dome_a` reproduce the live-captured rows below exactly, including orbit's five
//! zero-sub-bubble bubbles (Codex). A live capture is still the acceptance test for a new row;
//! [`lookup`] still returns `None` for a world nobody has read, rather than guessing - an invented
//! checksum is indistinguishable on the wire from a stale one.

/// One world's scenario-record facts, as rung 33 reads them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KnownWorld {
    /// The NUL-terminated ASCII name published at `global_activity_state+0x688`.
    pub name: &'static str,
    /// The scenario record's content tag, for the run log. Not sent; it is how the live capture
    /// identified which record it had, and it makes a stale row falsifiable against a new capture.
    pub scenario_tag: u32,
    /// `scenario_record+0x0C` - the **PRIVATE** checksum. `FUN_00299100` indexes
    /// `record + 0xC + ac[3]*4` where `ac[3]` is the index into `{"PRIVATE","PUBLIC"}` that the
    /// activity client prints in its own `[AC PRIVATE CURRENT …]` line ⇒ **0**. `+0x10` is PUBLIC
    /// and is the wrong one.
    pub checksum: u32,
    /// `scenario_record+0x48` - the world's bubble count. ⭐ A PvP map is ONE bubble; orbit is 16.
    pub bubble_count: u32,
    /// Which bubbles may legally be marked **CHOSEN** (`0x00`).
    ///
    /// ⚠ `0x00` is not universally legal: `FUN_002A0320` rejects a chosen bubble whose scenario
    /// sub-bubble flag is clear with *"activity client bubble state index was valid, but state
    /// itself was invalid"*. Measured live on `orbit` (16/16): bubbles 5-8 and 15 have sub-bubble
    /// count 0, which is why a blanket `0x00` failed there.
    ///
    /// ⛔ And leaving a bubble `0xFF` is **not** the safe default it looks like: the unchosen
    /// branch calls `FUN_00343020(ctx, bubble*8, -1, 0)`, which **UNREGISTERS** the slice-set
    /// region, so an all-`0xFF` blob passes rung 33 and guarantees rung 34 fails with
    /// `error.sstm_slice_not_found` ("LOBSTER").
    pub chosen: &'static [usize],
}

/// ⭐ `pvp_moonbase` - **First Light**, the map the rung-36 walk was taken on.
///
/// One bubble, bubble 0 chosen. Every value below is from the live capture of run
/// `20260809`'s scenario slot during the moonbase load.
pub const PVP_MOONBASE: KnownWorld = KnownWorld {
    name: "pvp_moonbase",
    scenario_tag: 0x80F0_8003,
    checksum: 0x5966_E491,
    bubble_count: 1,
    chosen: &[0],
};

/// `orbit` - the world the client is already in when an activity is launched.
///
/// ⭐ Kept because it is the **only** row with an independent second derivation: `orbit`'s record is
/// resident at rest, so `+0x0C = 0x33F044D3` was read directly rather than through the 100 ms
/// window, and `D1_AH_ACTIVITY=orbit D1_AH_ACTIVITY_INDEX=407 D1_AH_ACTIVITY_CHECKSUM=0x33f044d3`
/// is the combination that made `Activity checksum mismatch!` disappear on the live client. It is
/// this table's positive control against the moonbase row being a transcription.
pub const ORBIT: KnownWorld = KnownWorld {
    name: "orbit",
    scenario_tag: 0x80B5_2003,
    checksum: 0x33F0_44D3,
    bubble_count: 16,
    // Bubbles 5-8 and 15 have sub-bubble count 0 and CANNOT be chosen. Measured live, 16/16.
    chosen: &[0, 1, 2, 3, 4, 9, 10, 11, 12, 13, 14],
};

/// `pvp_city_defense` - **Bannerfall**, captured on build 56578 (run `2026-10-14-citydef`).
///
/// Read at the comparison itself rather than through the 100 ms record window: an exec breakpoint
/// on 56578's `0x299B9A` (`cmp eax, [rcx + rdx*4 + 0xC]`, the build's `FUN_00299100`) logs the
/// record's PRIVATE checksum while the client checks ours (`d1-watchwrite.py --exec 0x299b9a --peek
/// "rcx+rdx*4+0xc"`). PUBLIC was `0xC4647343`, `+0x48` = 1. The positive control is the follow-up
/// run: publishing this value made the same breakpoint read ours == the record's, rung 34 completed
/// and the Guardian spawned armed (with Bannerfall's own sensor-client list).
///
/// ⚠ `scenario_tag` was not captured by this method and is 0.
pub const PVP_CITY_DEFENSE: KnownWorld = KnownWorld {
    name: "pvp_city_defense",
    scenario_tag: 0,
    checksum: 0x929D_89ED,
    bubble_count: 1,
    chosen: &[0],
};

/// `act_cosmo_m0_highway` - the Cosmodrome's first story world, captured on build 56578 the same
/// way as [`PVP_CITY_DEFENSE`] (run `2026-10-14-world-act_cosmo_m0_highway`): PRIVATE checksum
/// `0x8FE07299`, 22 bubbles. Served with every bubble CHOSEN, this world's own sensor-client list
/// and its sensor group `0x4786C0E0` (`D1_AH_SENSOR_NAME_HASH`), rung 34 completed and the Guardian
/// spawned armed and walked. The first PvE world on the served route.
///
/// ⚠ All 22 bubbles were chosen without a sub-bubble check (orbit's rule); this one passed.
pub const ACT_COSMO_M0_HIGHWAY: KnownWorld = KnownWorld {
    name: "act_cosmo_m0_highway",
    scenario_tag: 0,
    checksum: 0x8FE0_7299,
    bubble_count: 22,
    chosen: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21],
};

/// ⭐ `act_cosmo_m1_dome_a` = **The Dark Within** (56578 activity index 168). The world was named
/// from the game's own content, not guessed: its scenario's string library holds the mission's
/// Ghost lines (Skywatch array station, the Hive Wizard "from the Moon"), and its first phase is
/// `cosmo_m1_phase00_dome_a`. PRIVATE checksum `0x42FE77A5`, 22 bubbles (run
/// `2026-10-14-world-act_cosmo_m1_dome_a-1`). Bubbles 0, 6, 7, 11, 12 and 20 are PUBLIC, so it
/// cannot start in region 0 without a public session. Served with start region 8 (slice set 0 of
/// bubble 1; see `GlobalActivityState::START_REGION`), the Guardian spawns armed at a Skywatch
/// array station (run `2026-10-14-dw-domea4`). ⚠ That is a private stand-in: the mission's own
/// opening lines are in the public Skywatch/Steppes space, and its first phase,
/// `cosmo_m1_phase00_dome_a`, is bubble 8's set 64.
pub const ACT_COSMO_M1_DOME_A: KnownWorld = KnownWorld {
    name: "act_cosmo_m1_dome_a",
    scenario_tag: 0,
    checksum: 0x42FE_77A5,
    bubble_count: 22,
    chosen: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21],
};

/// `city_tower_default1` - the ordinary **Tower** (not an event variant: queen, harvest, srl, fol,
/// crimson and chalice have their own scenarios and checksums). Read offline from the owned 56578
/// scenario-client record `0x80C7A005` (`ps4_city_tower_activities_act_023d_5.pkg`): `+0x08` =
/// FNV-1("city_tower_default1") `0x545AEC79`, PRIVATE `0x24340499`, PUBLIC `0xD067231D`, 5 bubbles
/// with one sub-bubble each, so all five may be chosen. Needed after A Guardian Rises, whose
/// completion chains (via activity 165) toward the Tower (activity index 0).
///
/// ⚠ Not yet live-captured: the first spawn region (0 is an inference) and its sensor-client list
/// are separate test gates.
pub const CITY_TOWER_DEFAULT1: KnownWorld = KnownWorld {
    name: "city_tower_default1",
    scenario_tag: 0x80C7_A005,
    checksum: 0x2434_0499,
    bubble_count: 5,
    chosen: &[0, 1, 2, 3, 4],
};

/// `act_cosmo_m0_5_hangar` - an authored hangar world of the opening (scenario `0x80CB802B`),
/// read offline from owned 56578 content: FNV-1 name `0x1200A031`, PRIVATE `0x31429B32`, PUBLIC
/// `0xE354F1EA` (the Highway's PUBLIC too), 22 bubbles with one sub-bubble each.
///
/// ⚠ A candidate for activity 165 (the STORY_CINEMATIC between A Guardian Rises and the Tower),
/// not a recovered binding: the client's 165 record names no world. Its sensor list must be the
/// client's own pending list, read once the world is resident.
pub const ACT_COSMO_M0_5_HANGAR: KnownWorld = KnownWorld {
    name: "act_cosmo_m0_5_hangar",
    scenario_tag: 0x80CB_802B,
    checksum: 0x3142_9B32,
    bubble_count: 22,
    chosen: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21],
};

/// Every world whose scenario record has been captured or read from owned content.
pub const KNOWN_WORLDS: &[KnownWorld] = &[
    PVP_MOONBASE,
    ORBIT,
    PVP_CITY_DEFENSE,
    ACT_COSMO_M0_HIGHWAY,
    ACT_COSMO_M1_DOME_A,
    CITY_TOWER_DEFAULT1,
    ACT_COSMO_M0_5_HANGAR,
];

/// The world an Activity Host publishes when nothing overrides it.
///
/// ⭐ `pvp_moonbase` / activity **407 = ELIMINATION** is the *only* configuration this project has
/// ever walked to `activity:in_world` on, and it is what `runs/snapshots/rung36-*.snap` were taken
/// under. The previous default - `act_cosmo_m0_highway`, checksum 0, bubble count 0, bubbles
/// unset - published **every documented rung-33/34 wall at once**, and had already silently shipped
/// in runs that meant to use the good set but inherited it from an unexported shell variable.
pub const DEFAULT_WORLD: KnownWorld = PVP_MOONBASE;

/// The captured facts for `name`, or `None` if this world has never been measured.
///
/// ⛔ Deliberately **not** a guess. An invented checksum is byte-indistinguishable on the wire from
/// a stale one, and both surface as the same `activity checksum mismatch`; returning `None` lets
/// the caller keep the old loud "checksum 0" warning, which at least names itself.
pub fn lookup(name: &str) -> Option<&'static KnownWorld> {
    KNOWN_WORLDS.iter().find(|w| w.name == name)
}

/// What the Activity Host will actually publish, after the world table and any overrides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// The world name published at `global_activity_state+0x688`.
    pub name: String,
    /// The table row `name` matched, if any. `None` = an unmeasured world; the three values below
    /// then fall back to the pre-2026-08-10 zeros, which are loud rather than wrong-and-quiet.
    pub world: Option<&'static KnownWorld>,
    pub checksum: u32,
    pub bubble_count: u32,
    pub chosen: Vec<usize>,
    /// ⭐⭐ **Every override that CONTRADICTS the named world's measured value**, in words.
    ///
    /// This is the whole point of the type. Each entry is a configuration that will produce a
    /// rung-33 gate failure whose message names the *field* and never the *cause* - so it is
    /// reported once, loudly, at startup, where it is cheap.
    pub disagreements: Vec<String>,
}

/// ⭐⭐ Derive the three rung-33 scenario values from the **world name**, letting explicit
/// overrides win and reporting every one that contradicts a measured world.
///
/// `None` for an override means *the caller did not ask* (derive it). `Some(v)` means the caller
/// asked for exactly `v`, including `Some(0)` and `Some(vec![])` - both of which are deliberate,
/// documented negative controls (`D1_AH_ACTIVITY_CHECKSUM=0` reproduces every pre-2026-08-09 run;
/// `D1_AH_BUBBLE_CHOSEN=` publishes all-`0xFF` and reproduces LOBSTER). Collapsing "unset" and
/// "set to the falsy value" would silently delete both.
pub fn resolve(
    name: Option<&str>,
    checksum: Option<u32>,
    bubble_count: Option<u32>,
    chosen: Option<Vec<usize>>,
) -> Resolved {
    let name = name.unwrap_or(DEFAULT_WORLD.name).to_string();
    let world = lookup(&name);
    let mut disagreements = Vec::new();
    if let Some(w) = world {
        if let Some(c) = checksum {
            if c != w.checksum {
                disagreements.push(format!(
                    "D1_AH_ACTIVITY_CHECKSUM={c:#010x} but {}'s measured scenario_record+0x0C is \
                     {:#010x} (tag {:#010x}) - FUN_00299100 compares against the record of the \
                     world the client ACTUALLY LOADED, so this publishes one world's checksum \
                     while naming another and re-fails gate 1 as `activity checksum mismatch`",
                    w.name, w.checksum, w.scenario_tag
                ));
            }
        }
        if let Some(n) = bubble_count {
            if n != w.bubble_count {
                disagreements.push(format!(
                    "D1_AH_BUBBLE_COUNT={n} but {}'s measured scenario_record+0x48 is {} - \
                     FUN_002A0320 requires equality and reports the mismatch as \
                     `activity client bubble count mismatch`",
                    w.name, w.bubble_count
                ));
            }
        }
        if let Some(sel) = chosen.as_ref() {
            let illegal: Vec<usize> =
                sel.iter().copied().filter(|i| !w.chosen.contains(i)).collect();
            if !illegal.is_empty() {
                disagreements.push(format!(
                    "D1_AH_BUBBLE_CHOSEN names bubble(s) {illegal:?}, which {} cannot legally \
                     have CHOSEN (its choosable set is {:?}) - FUN_002A0320 rejects a chosen \
                     bubble whose scenario sub-bubble flag is clear with `activity client bubble \
                     state index was valid, but state itself was invalid`",
                    w.name, w.chosen
                ));
            }
            if sel.is_empty() && w.bubble_count > 0 {
                disagreements.push(format!(
                    "D1_AH_BUBBLE_CHOSEN is EXPLICITLY EMPTY - all {} of {}'s bubbles publish as \
                     0xFF `unchosen`. That PASSES rung 33 and then UNREGISTERS every slice-set \
                     region, so rung 34 fails with error.sstm_slice_not_found (\"LOBSTER\"). This \
                     is a negative control, not a configuration",
                    w.bubble_count, w.name
                ));
            }
        }
    }
    Resolved {
        checksum: checksum.or_else(|| world.map(|w| w.checksum)).unwrap_or(0),
        bubble_count: bubble_count.or_else(|| world.map(|w| w.bubble_count)).unwrap_or(0),
        chosen: chosen.or_else(|| world.map(|w| w.chosen.to_vec())).unwrap_or_default(),
        name,
        world,
        disagreements,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_known_world_chooses_distinct_bubbles_in_range() {
        for w in KNOWN_WORLDS {
            assert!(w.chosen.iter().all(|&b| (b as u32) < w.bubble_count), "{}", w.name);
            let mut seen = w.chosen.to_vec();
            seen.dedup();
            assert_eq!(seen.len(), w.chosen.len(), "{}", w.name);
        }
    }

    #[test]
    fn the_tower_is_known_by_its_world_name() {
        let t = lookup("city_tower_default1").expect("tower row");
        assert_eq!((t.scenario_tag, t.checksum, t.bubble_count), (0x80C7_A005, 0x2434_0499, 5));
        assert!(lookup("city_tower").is_none(), "the destination folder is not a world name");
    }

    #[test]
    fn the_hangar_is_known_by_its_world_name() {
        let h = lookup("act_cosmo_m0_5_hangar").expect("hangar row");
        assert_eq!((h.scenario_tag, h.checksum, h.bubble_count), (0x80CB_802B, 0x3142_9B32, 22));
        assert_ne!(h.checksum, ACT_COSMO_M0_HIGHWAY.checksum, "PRIVATE differs from the Highway");
    }
}
