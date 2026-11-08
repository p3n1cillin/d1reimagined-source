// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! Stream Q: the full-game profile serves every activity with a world
//! (`D1_AH_WORLDS_ACTIVITIES=all`). Before the host listens, main.rs plans each activity from the
//! committed world tables (`ps4/fixtures/56578/worlds/`) and builds two udt 5 frames (teams 0 and
//! -1) for each distinct (world, start slice set, group). This measures what that costs, from the
//! committed tables: the planning always; the frames when the eboot is at hand (`D1_EBOOT`, for the
//! native codec's schemas), skipped otherwise.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use d1_activity_host::generated::{parse_activities, GeneratedWorlds, Plan, LIFETIME_TYPE};

fn worlds_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/56578/worlds")
}

/// main.rs's base parameters for a generated world's frame, as `generated`'s own tests use them.
fn base() -> d1_oracle::sensor::SensorPhaseUpdate {
    d1_oracle::sensor::SensorPhaseUpdate {
        gen: 0,
        name_hash: 0,
        name_i8: LIFETIME_TYPE,
        name_sub: 3,
        phase: 3,
        tail: None,
        clients: Vec::new(),
        authority: None,
    }
}

/// Every plan with a world, once per distinct frame key, as main.rs dedups them.
fn distinct(plans: &[Plan]) -> BTreeMap<(String, u32, u32), &Plan> {
    let mut out = BTreeMap::new();
    for p in plans {
        out.entry((p.world.name.clone(), p.start_region, p.group)).or_insert(p);
    }
    out
}

#[test]
fn all_serves_every_activity_with_a_world() {
    assert_eq!(parse_activities("all"), None);
    assert_eq!(parse_activities(" * "), None);
    assert_eq!(parse_activities("166, 167,x,0"), Some(vec![166, 167, 0]));
    let worlds = GeneratedWorlds::load(&worlds_dir()).expect("the committed world tables");
    let (plans, skipped) = worlds.plans(None, true);
    let with_world: Vec<u16> = worlds.activities.values().filter(|a| a.world.is_some()).map(|a| a.index).collect();
    // Every activity the tables bind is served: none is skipped for want of a start or a lifetime record.
    let skipped_bound: Vec<_> = skipped.iter().filter(|(i, _)| with_world.contains(i)).collect();
    assert!(skipped_bound.is_empty(), "bound activities the host would skip: {skipped_bound:?}");
    assert_eq!(plans.len(), with_world.len());
    // The ones the profile served before (166-186) and the ones stream Q bound from the client's own
    // activity table (King's Fall, Wrath of the Machine, Prison of Elders, Felwinter Peak, the
    // Lighthouse, the Plaguelands) are among them.
    let names: BTreeMap<u16, &str> = plans.iter().map(|p| (p.index, p.world.name.as_str())).collect();
    for (i, w) in [(166, "act_cosmo_m0_5_hangar"), (167, "cine_speaker"), (168, "act_cosmo_m1_dome_a"), (170, "act_cosmo_m2_dome_c")] {
        assert_eq!(names.get(&i).copied(), Some(w), "activity {i}");
    }
    for w in ["raid_epiphany", "raid_promise", "arena_champion", "social_observatory", "mercury_social", "plaguelands_patrol"] {
        assert!(names.values().any(|n| *n == w), "no activity is served {w}");
    }
}

#[test]
fn every_world_plans_within_the_startup_budget() {
    let t = Instant::now();
    let worlds = GeneratedWorlds::load(&worlds_dir()).expect("the committed world tables");
    let loaded = t.elapsed();
    let (plans, _) = worlds.plans(None, true);
    let planned = t.elapsed();
    let keys = distinct(&plans);
    let mut clients = 0usize;
    for p in keys.values() {
        for team in [0, -1] {
            let q = d1_activity_host::sensor_list::params_for_list(&base(), p.group, team, &p.list)
                .unwrap_or_else(|e| panic!("activity {}: {e:#}", p.index));
            assert!(!q.clients.is_empty(), "activity {}", p.index);
            // A generated list carries no values: the native codec covers every frame (main.rs's
            // `native::try_sensor_frame` checks only the clients' value schemas).
            assert!(q.clients.iter().all(|c| c.a_value.is_none() && c.b_value.is_none()), "activity {}", p.index);
            clients += q.clients.len();
        }
    }
    let list_bytes: usize = plans.iter().map(|p| p.list.len()).sum();
    eprintln!(
        "all worlds: {} activities, {} distinct frames, {} clients in them; load {:?}, plan {:?}, params {:?}; \
         plan lists {} KiB",
        plans.len(),
        keys.len() * 2,
        clients,
        loaded,
        planned - loaded,
        t.elapsed() - planned,
        list_bytes / 1024
    );
    // Generous for a debug build on a loaded host; the release host takes a fraction.
    assert!(t.elapsed() < Duration::from_secs(20), "{:?}", t.elapsed());
    // The plans' lists are what the host keeps besides the frames.
    assert!(list_bytes < 16 << 20, "{list_bytes} bytes of plan lists");
}

/// The script bridge's table (`ps4/fixtures/56578/scripts/activities.txt`) lists every bound PvE
/// activity since stream Q; the bridge parses it at startup, with each row's slice lists.
#[test]
fn the_script_bridge_table_parses_within_the_startup_budget() {
    let fixtures = worlds_dir().join("..");
    let t = Instant::now();
    let mut worlds = GeneratedWorlds::load(&worlds_dir()).expect("the committed world tables");
    worlds
        .load_slice_states(&std::fs::read_to_string(fixtures.join("scripts/slice-states.tsv")).expect("slice states"))
        .expect("slice states parse");
    let table = std::fs::read_to_string(fixtures.join("scripts/activities.txt")).expect("the activity table");
    let mut base = base();
    base.phase = 0;
    let (mut n, mut rows) = (0usize, 0usize);
    let mut cache = d1_activity_host::script_bridge::SliceRowsCache::new();
    let mut shared = std::collections::HashSet::new();
    for line in table.lines().map(|l| l.split('#').next().unwrap_or("").trim()).filter(|l| !l.is_empty()) {
        let a = d1_activity_host::script_bridge::ScriptedActivity::parse_cached(
            line,
            &base,
            &|_| Err("lists come from the tables".into()),
            Some(&worlds),
            &mut cache,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        rows += a.rows.len();
        // Each world's slice lists are held once, whichever rows name the world.
        if shared.insert(std::sync::Arc::as_ptr(&a.slice_rows) as usize) {
            rows += a.slice_rows.values().map(Vec::len).sum::<usize>();
        }
        n += 1;
    }
    eprintln!(
        "script bridge table: {n} activities on {} worlds, {rows} rows held with their slice lists, parsed in {:?}",
        shared.len(),
        t.elapsed()
    );
    // About 80 bytes a row: what the bridge keeps for the host's life.
    assert!(rows < 1_000_000, "{rows} rows");
    assert!(n > 400, "{n} activities");
    assert!(t.elapsed() < Duration::from_secs(60), "{:?}", t.elapsed());
}

#[test]
fn every_worlds_frames_build_natively_within_budget() {
    let Ok(schemas) = d1_rsat::schemas() else {
        eprintln!("skipped: no eboot for the native codec's schemas (set D1_EBOOT)");
        return;
    };
    let worlds = GeneratedWorlds::load(&worlds_dir()).expect("the committed world tables");
    let (plans, _) = worlds.plans(None, true);
    let keys = distinct(&plans);
    let t = Instant::now();
    let (mut bytes, mut frames, mut largest) = (0usize, 0usize, 0usize);
    for p in keys.values() {
        for team in [0, -1] {
            let q = d1_activity_host::sensor_list::params_for_list(&base(), p.group, team, &p.list).unwrap();
            let f = d1_activity_host::native::sensor_frame_of(&q)
                .encode(schemas)
                .unwrap_or_else(|e| panic!("activity {} ({} slice set {}): {e}", p.index, p.world.name, p.start_region));
            bytes += f.bytes.len();
            largest = largest.max(f.bytes.len());
            frames += 1;
        }
    }
    eprintln!(
        "all worlds: {frames} frames built natively in {:?}, {} KiB in all, the largest {largest} bytes",
        t.elapsed(),
        bytes / 1024
    );
    assert!(t.elapsed() < Duration::from_secs(60), "{:?}", t.elapsed());
    // The frames stay in memory for the host's life (two per distinct key).
    assert!(bytes < 32 << 20, "{bytes} bytes of frames");
}
