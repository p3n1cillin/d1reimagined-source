// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! Stream O: the full-game profile (`ps4/profiles/full-game.env`, #320) through the servers' own
//! knob parsers. With every full-game knob on, the activity host and the responder must agree on
//! each seam between them, and every input the profile names from this checkout must be there.
//! Inputs outside the checkout (the Blam Network checkout, Node, the bundle export, the staging and
//! run folders, the eboot) are the private ones: their paths are not checked.
//!
//! The responder's side of the same profile is `d1-responder`'s `player_store` test
//! `the_full_game_profile_turns_the_store_seams_on`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..")
}

/// A profile's knobs, includes first (later lines win), as `section -> key -> (value, optional)`.
/// The format is lab-base.env's header (`tools/ps4_setup/d1-full-game.ps1` reads it).
fn read_profile(path: &Path, out: &mut HashMap<String, HashMap<String, (String, bool)>>) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut section = String::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(f) = line.strip_prefix("@include ") {
            read_profile(&path.parent().unwrap().join(f.trim()), out);
            continue;
        }
        if let Some(s) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = s.to_string();
            continue;
        }
        let (optional, kv) = match line.strip_prefix('?') {
            Some(rest) => (true, rest),
            None => (false, line),
        };
        let (k, v) = kv.split_once('=').unwrap_or_else(|| panic!("{}: not KEY=VALUE: {raw}", path.display()));
        assert!(!section.is_empty(), "{}: {k} outside a section", path.display());
        out.entry(section.clone()).or_default().insert(k.to_string(), (v.to_string(), optional));
    }
}

/// The locations a value may name. `None`: outside this checkout (private, not checked).
fn location(name: &str, run: &Path) -> Option<PathBuf> {
    let r = repo();
    match name {
        "REPO" => Some(r),
        "FIXTURES" => Some(r.join("ps4/fixtures/56578")),
        "PROFILES" => Some(r.join("ps4/profiles")),
        "RUN" => Some(run.to_path_buf()),
        "STAGING" => Some(run.join("staging")),
        "BIN" => Some(run.join("bin")),
        _ => None,
    }
}

/// `${NAME}` substituted; `private` when the value names a place outside the checkout.
fn resolve(value: &str, launch: &HashMap<String, (String, bool)>, run: &Path, private: &mut bool) -> String {
    let mut out = String::new();
    let mut rest = value;
    while let Some(i) = rest.find("${") {
        out.push_str(&rest[..i]);
        let j = rest[i..].find('}').map(|j| i + j).expect("a closed ${...}");
        let name = &rest[i + 2..j];
        if let Some(p) = location(name, run) {
            out.push_str(&p.to_string_lossy().replace('\\', "/"));
        } else if let Some((v, _)) = launch.get(name) {
            out.push_str(&resolve(v, launch, run, private));
        } else {
            // ROOT, TOOLS, WORKSPACE, SHADPS4, env:NAME: the lab machine's own.
            *private = true;
            out.push_str(&format!("<{name}>"));
        }
        rest = &rest[j + 1..];
    }
    out.push_str(rest);
    out
}

struct Server {
    vars: HashMap<String, String>,
    /// Knobs whose value names a place outside the checkout.
    private: Vec<String>,
}

fn full_game(run: &Path) -> Option<Server> {
    let path = repo().join("ps4/profiles/full-game.env");
    if !path.is_file() {
        eprintln!("SKIP: no {} (#320's profile)", path.display());
        return None;
    }
    let mut p = HashMap::new();
    read_profile(&path, &mut p);
    let launch = p.remove("launch").unwrap_or_default();
    let mut vars = HashMap::new();
    let mut private = Vec::new();
    for (k, (v, _)) in p.remove("server").expect("a [server] section") {
        let mut is_private = false;
        let v = resolve(&v, &launch, run, &mut is_private);
        if is_private {
            private.push(k.clone());
        }
        if !v.is_empty() {
            vars.insert(k, v);
        }
    }
    Some(Server { vars, private })
}

fn run_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("d1-full-game-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Every `[server]` input the profile names from this checkout is there.
#[test]
fn every_checkout_input_exists() {
    let run = run_dir("inputs");
    let Some(s) = full_game(&run) else { return };
    let repo = repo().canonicalize().unwrap().to_string_lossy().replace('\\', "/");
    let root = repo.trim_start_matches("//?/").to_string();
    let mut missing = Vec::new();
    for (k, v) in &s.vars {
        if s.private.contains(k) {
            continue;
        }
        // Each space-separated word that is a path into the checkout.
        for word in v.split(' ') {
            let p = Path::new(word);
            let canon = p.parent().and_then(|d| d.canonicalize().ok()).map(|d| d.to_string_lossy().replace('\\', "/"));
            let inside = canon.as_deref().is_some_and(|d| d.trim_start_matches("//?/").starts_with(&root));
            if p.is_absolute() && inside && !p.exists() {
                missing.push(format!("{k} = {word}"));
            }
        }
    }
    assert!(missing.is_empty(), "the profile names inputs this checkout does not have:\n{}", missing.join("\n"));
    let _ = std::fs::remove_dir_all(&run);
}

/// The pairs of knobs one side writes and the other reads, and the knobs one feature needs.
#[test]
fn the_cross_process_knobs_agree() {
    let run = run_dir("agree");
    let Some(s) = full_game(&run) else { return };
    let v = |k: &str| s.vars.get(k).map(String::as_str);
    let on = |k: &str| v(k) == Some("1");
    let mut problems = Vec::new();
    let mut need = |cond: bool, what: &str| {
        if !cond {
            problems.push(what.to_string());
        }
    };
    // The launched activity: the responder writes it, the host reads it.
    need(v("D1_ACTIVITY_FOLLOW_FILE").is_some() && v("D1_ACTIVITY_FOLLOW_FILE") == v("D1_AH_ACTIVITY_FOLLOW_FILE"), "D1_ACTIVITY_FOLLOW_FILE and D1_AH_ACTIVITY_FOLLOW_FILE name one file");
    need(v("D1_AH_ACTIVITY_TABLE").is_some(), "the host follows the launched activity through D1_AH_ACTIVITY_TABLE");
    // The two command files are different files: each server takes (renames) its own.
    need(v("D1_STORE_CONTROL").is_some() && v("D1_STORE_CONTROL") != v("D1_AH_CONTROL_FILE"), "D1_STORE_CONTROL is not D1_AH_CONTROL_FILE");
    need(v("D1_PROFILE_PUSH_FILE") != v("D1_ACCOUNT_PUSH_FILE"), "the profile and account pushes are different files");
    // Rewards cross the boundary only with the store and its command file.
    need(on("D1_REWARDS") && on("D1_STORE"), "D1_REWARDS=1 with D1_STORE=1");
    // Public instances need one host id per connection.
    need(!on("D1_AH_PUBLIC_HOST") || on("D1_AH_DISTINCT_HOST_IDS"), "D1_AH_PUBLIC_HOST=1 needs D1_AH_DISTINCT_HOST_IDS=1");
    // The script bridge is built inside the sensor setup: without a lifetime phase it never starts.
    if on("D1_AH_SCRIPTS") {
        need(v("D1_AH_SENSOR_PHASE").is_some(), "D1_AH_SCRIPTS=1 needs D1_AH_SENSOR_PHASE (the bridge is set up with the sensor frames)");
        for k in ["D1_AH_SCRIPTS_SIDECAR", "D1_AH_SCRIPTS_ACTIVITIES", "D1_AH_SCRIPTS_INCIDENTS"] {
            need(v(k).is_some(), &format!("D1_AH_SCRIPTS=1 needs {k}"));
        }
    }
    // Shared instances are the activity host's half of multiplayer.
    need(!on("D1_AH_SHARED_INSTANCES") || on("D1_MP"), "D1_AH_SHARED_INSTANCES=1 with D1_MP=1");
    assert!(problems.is_empty(), "full-game.env: {problems:#?}");

    // The forwarder and the checkpoint store resolve from the same knobs the responder reads.
    let forwarder = d1_activity_host::reward_forward::GrantForwarder::from_vars(|k| s.vars.get(k).cloned())
        .expect("grants are forwarded with the full-game knobs");
    assert_eq!(forwarder.path(), Path::new(&s.vars["D1_STORE_CONTROL"]));
    let state = d1_activity_host::checkpoints::state_file_for(forwarder.path());
    assert_eq!(state.file_name().and_then(|n| n.to_str()), Some("store-control.state.json"), "the responder's player_store::state_file_for names the same file");
    let _ = std::fs::remove_dir_all(&run);
}

/// The tables the full-game knobs name load through the host's own loaders.
#[test]
fn the_full_game_tables_load() {
    let run = run_dir("tables");
    let Some(s) = full_game(&run) else { return };
    let v = |k: &str| s.vars.get(k).map(PathBuf::from).unwrap_or_else(|| panic!("full-game.env sets {k}"));
    // #313: the activity framework.
    let lib = d1_activity_host::encounter_framework::ActivityLibrary::load(&v("D1_AH_ACTIVITY_FRAMEWORK")).expect("the activity data");
    assert!(lib.validate().is_empty(), "{:?}", lib.validate());
    // #314: the generated worlds, every listed activity planned, and the PUBLIC checksums.
    let worlds = d1_activity_host::generated::GeneratedWorlds::load(&v("D1_AH_WORLDS")).expect("the worlds");
    let only: Vec<u16> = s.vars["D1_AH_WORLDS_ACTIVITIES"].split(',').map(|i| i.trim().parse().unwrap()).collect();
    let (plans, skipped) = worlds.plans(Some(&only), s.vars.get("D1_AH_WORLDS_PRIVATE_START").is_some_and(|x| x == "1"));
    assert!(skipped.is_empty(), "listed activities without a world: {skipped:?}");
    assert_eq!(plans.len(), only.len());
    let public = d1_activity_host::generated::read_public_checksums(&v("D1_AH_WORLDS")).unwrap();
    assert!(!public.is_empty());
    // #316: every public list loads, a bubble's leads with the players group, and each goes through the entry-only
    // encoder (a recording codec here; the client's own needs the eboot).
    let lists = d1_activity_host::public_host::load_public_table(&v("D1_AH_PUBLIC_TABLE")).expect("the public table");
    assert!(!lists.is_empty());
    // Public bubbles lead with player group 0xEAAF16E2; the social spaces with their own.
    for l in lists.iter().filter(|l| matches!(l.key, d1_activity_host::public_registry::InstanceKey::Bubble { .. })) {
        assert!(d1_activity_host::public_host::players_group_leads(l), "{:?}", l.key);
    }
    let codec: std::sync::Arc<dyn d1_activity_host::codec::AhWireCodec> = std::sync::Arc::new(EntryCount);
    let frames = d1_activity_host::public_host::PublicHost::build_frames(&lists, &d1_activity_host::public_host::entry_frame_encoder(codec, 0));
    for l in &lists {
        assert_eq!(frames[&l.key].as_deref().map(|f| f.len()), Ok(l.clients.len()), "{:?}", l.key);
    }
    let _ = std::fs::remove_dir_all(&run);
}

/// A codec that answers an entry-only frame with one byte per entry (no eboot needed).
struct EntryCount;

impl d1_activity_host::codec::AhWireCodec for EntryCount {
    fn name(&self) -> &'static str {
        "entry-count"
    }
    fn encode(&self, _: &d1_activity_host::messages::AhMessage) -> Result<Vec<u8>, d1_activity_host::AhError> {
        unreachable!()
    }
    fn decode(&self, _: u32, _: &[u8]) -> Result<Vec<u8>, d1_activity_host::AhError> {
        unreachable!()
    }
    fn encode_sensor_entries(&self, frame: &d1_oracle::sensor::SensorEntryFrame) -> Result<(Vec<u8>, usize), d1_activity_host::AhError> {
        assert!(frame.init && frame.authority.is_none(), "an entry-only frame: init, no authority");
        Ok((vec![0; frame.entries.len()], 9))
    }
}

/// The script bridge's configuration builds from the profile (the knobs it reads from the
/// environment), and its rewards can be the forwarder.
#[test]
fn the_script_bridge_configures_from_the_profile() {
    let run = run_dir("scripts");
    let Some(s) = full_game(&run) else { return };
    // This test binary's own environment: only this test sets it.
    for (k, v) in &s.vars {
        if k.starts_with("D1_AH_SCRIPTS") || k.starts_with("D1_AH_ROWS") || k == "D1_AH_WORLDS" || k == "D1_BUNDLES_DIR" {
            std::env::set_var(k, v);
        }
    }
    let base = d1_oracle::sensor::SensorPhaseUpdate {
        gen: 0,
        name_hash: 0xC2DA_CAF7,
        name_i8: 0x10,
        name_sub: 3,
        phase: 3,
        tail: None,
        clients: Vec::new(),
        authority: None,
    };
    let mut c = d1_activity_host::script_bridge::ScriptConfig::from_env(&base)
        .expect("D1_AH_SCRIPTS's inputs")
        .expect("D1_AH_SCRIPTS=1 in full-game.env");
    assert!(!c.activities.is_empty(), "the scripted-activity table names something");
    // Stream K's generated table: cine_reveal, its lists from the world tables, its hand-off.
    if let Some(reveal) = c.activities.get(&165) {
        assert!(!reveal.slice_rows.is_empty(), "the slice lists from D1_AH_WORLDS");
        assert_eq!(reveal.next, Some((0, 0x8AE8_DA69)), "cine_reveal hands off to the Tower");
    }
    let f = d1_activity_host::reward_forward::GrantForwarder::new(run.join("store-control.txt"));
    c.rewards = std::sync::Arc::new(std::sync::Mutex::new(f));
    let _ = std::fs::remove_dir_all(&run);
}
