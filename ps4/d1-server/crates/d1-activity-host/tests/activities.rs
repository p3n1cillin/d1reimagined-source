// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The strike, raid, arena and Crucible data under `ps4/fixtures/56578/activities`, checked and
//! run: the data must validate, cover the manifest, and drive the framework from start to end.

use std::collections::BTreeSet;
use std::sync::Arc;

use d1_activity_host::checkpoints::{CheckpointKey, CheckpointStore, MemoryCheckpointStore};
use d1_activity_host::encounter_framework::{
    ActivityKind, ActivityLibrary, ActivityRun, Effect, EndReason, Event, EvidenceKind, Modifiers, RunState, Trigger,
};
use d1_activity_host::pvp::{
    self, CrucibleMatch, MatchEffect, MatchEvent, MatchPhase, PassageOutcome, Timings, TrialsPassage,
};

const NOW: u64 = 1_793_700_000; // 2026-11-03, a Tuesday after the reset

fn lib() -> ActivityLibrary {
    ActivityLibrary::load(&ActivityLibrary::default_dir()).expect("activities data loads")
}

#[test]
fn the_data_is_valid() {
    let lib = lib();
    let problems = lib.validate();
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
}

#[test]
fn every_strike_raid_and_arena_is_defined() {
    let lib = lib();
    let names: BTreeSet<&str> = lib.activities.iter().map(|a| a.name.as_str()).collect();
    for raid in ["Vault of Glass", "Crota's End", "King's Fall", "Wrath of the Machine"] {
        assert!(names.contains(raid), "{raid}");
    }
    for a in ["Prison of Elders", "Challenge of the Elders", "Archon's Forge"] {
        assert!(names.contains(a), "{a}");
    }
    let strikes = lib.activities.iter().filter(|a| a.kind == ActivityKind::Strike).count();
    assert_eq!(strikes, 17, "every strike the manifest names");
    // Every definition says where its encounters come from, and every gap is listed.
    for a in &lib.activities {
        for e in &a.encounters {
            if !e.evidence.kind.is_primary() {
                assert!(!a.needs_evidence.is_empty(), "{}: {} is {:?} but nothing is listed as needed", a.id, e.id, e.evidence.kind);
            }
        }
    }
    // The five restored strike scripts, and the live captures for the others.
    let scripted: BTreeSet<&str> = lib
        .activities
        .iter()
        .filter(|a| a.script.as_ref().is_some_and(|s| !s.start_only_stub))
        .map(|a| a.id.as_str())
        .collect();
    assert_eq!(scripted.len(), 5, "{scripted:?}");
    for s in ["strike-the-will-of-crota", "strike-winters-run", "strike-cerberus-vae-iii", "strike-dust-palace", "raid-crotas-end"] {
        let a = lib.by_id(s).unwrap();
        assert!(a.sequence.is_some(), "{s} runs from its live capture");
        assert!(a.encounters.iter().any(|e| e.evidence.kind == EvidenceKind::LiveCapture), "{s}");
    }
}

#[test]
fn every_capture_replays_through_its_definition() {
    let lib = lib();
    for a in lib.activities.iter().filter(|a| a.sequence.is_some()) {
        let seq = &lib.sequences[a.sequence.as_ref().unwrap()];
        let store = Arc::new(MemoryCheckpointStore::new());
        let (mut run, _) = ActivityRun::start(Arc::clone(a), a.activity_hashes[0], &[1], Modifiers::default(), store, NOW);
        let mut opened = vec![0usize];
        for ev in seq.replay_events() {
            for fx in run.handle(ev) {
                if let Effect::EncounterBegan { index, .. } = fx {
                    opened.push(index);
                }
            }
        }
        // Encounters that open on what the capture reported open in order; the rest (an
        // encounter waiting on the previous one) are allowed to be skipped over.
        assert!(opened.windows(2).all(|w| w[0] < w[1]), "{}: {opened:?}", a.id);
        let reported = a
            .encounters
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(_, e)| matches!(e.begins_on, Trigger::SliceEntered(_) | Trigger::Trigger(_)))
            .count();
        assert_eq!(opened.len() - 1, reported, "{}: every capture-opened encounter opened ({opened:?})", a.id);
        let fx = run.handle(Event::ActivityCompleted);
        assert!(matches!(fx.last(), Some(Effect::ActivityCompleted { .. })), "{}", a.id);
    }
}

#[test]
fn a_scripted_strike_follows_its_phases_and_nightfall_sends_a_wipe_to_orbit() {
    let lib = lib();
    let a = Arc::clone(lib.by_id("strike-the-devils-lair").unwrap());
    let phases = a.script.as_ref().unwrap().phases.clone();
    assert_eq!(phases.len(), 8);
    let store = Arc::new(MemoryCheckpointStore::new());
    let (mut run, _) = ActivityRun::start(Arc::clone(&a), a.activity_hashes[0], &[1, 2], Modifiers::default(), store.clone(), NOW);
    for p in &phases[1..] {
        let fx = run.handle(Event::ScriptPhase(p.clone()));
        assert!(fx.iter().any(|f| matches!(f, Effect::EncounterBegan { id, .. } if id == p)), "{p}");
    }
    run.handle(Event::ActivityCompleted);
    assert_eq!(run.state(), &RunState::Completed);
    assert!(store.is_empty(), "strikes keep no checkpoint");

    // Nightfall, through a playlist row's skulls.
    let nf = lib.catalog.get(1_293_005_540).unwrap(); // Vanguard Nightfall
    let m = Modifiers::of_catalog(nf);
    assert!(m.wipe_returns_to_orbit, "{:?}", m.skulls);
    let (mut run, _) = ActivityRun::start(Arc::clone(&a), a.activity_hashes[0], &[1, 2], m, store, NOW);
    run.handle(Event::ScriptPhase(phases[1].clone()));
    run.handle(Event::PlayerDied(1));
    assert_eq!(run.handle(Event::PlayerDied(2)), vec![Effect::ReturnedToOrbit(EndReason::NightfallWipe)]);
}

#[test]
fn crotas_end_wipes_and_keeps_its_checkpoint_for_the_week() {
    let lib = lib();
    let a = Arc::clone(lib.by_id("raid-crotas-end").unwrap());
    let hash = a.activity_hashes[0];
    let store = Arc::new(MemoryCheckpointStore::new());
    let (mut run, _) = ActivityRun::start(Arc::clone(&a), hash, &[1, 2, 3], Modifiers::default(), store.clone(), NOW);
    // Into the Bridge (the capture's new slice set).
    let Trigger::SliceEntered(bridge) = a.encounters[1].begins_on else { panic!("{:?}", a.encounters[1].begins_on) };
    let fx = run.handle(Event::SliceEntered(bridge));
    assert!(fx.iter().any(|f| matches!(f, Effect::Rewards { encounter, .. } if encounter == "the-abyss")));
    assert!(fx.contains(&Effect::CheckpointSaved { character: 2, index: 1, id: "the-bridge".into() }));
    // The fireteam wipes on the Bridge.
    run.handle(Event::AllPlayersDead);
    assert_eq!(run.state(), &RunState::Wiping { restart: 1 });
    run.handle(Event::WipeFinished);
    assert_eq!(run.current().map(|e| e.id.as_str()), Some("the-bridge"));
    drop(run);
    // Next evening, character 3 leads: the raid starts at the Bridge, and the Abyss is locked.
    let (mut run, fx) = ActivityRun::start(Arc::clone(&a), hash, &[3], Modifiers::default(), store.clone(), NOW + 86_400);
    assert_eq!(fx[0], Effect::CheckpointLoaded { character: 3, index: 1, id: "the-bridge".into() });
    assert!(store.is_looted(CheckpointKey { character: 3, activity_hash: hash }, "the-abyss", d1_activity_host::checkpoints::reset_week(NOW)));
    let fx = run.handle(Event::ActivityCompleted);
    assert!(matches!(fx.last(), Some(Effect::ActivityCompleted { .. })));
    assert!(store.load(CheckpointKey { character: 3, activity_hash: hash }).is_none());
}

#[test]
fn every_crucible_mode_plays_to_an_end() {
    let lib = lib();
    let c = lib.crucible.as_ref().expect("crucible.json");
    for id in pvp::REQUIRED_MODES {
        assert!(c.mode(id).is_some(), "{id}");
    }
    for m in &c.modes {
        let mode = Arc::new(m.clone());
        let mut game = CrucibleMatch::new(Arc::clone(&mode), Timings::default(), 0);
        let n = u64::from(m.max_players);
        for p in 1..=n {
            game.handle(MatchEvent::Join { player: p, team: None });
        }
        game.handle(MatchEvent::Tick(10_000));
        assert!(matches!(game.phase(), MatchPhase::Playing { round: 1, .. }), "{}", m.id);
        let mut ended = false;
        // Player 1 scores against everyone, through every scoring incident the mode has.
        let foes: Vec<u64> = (2..=n).filter(|p| m.is_ffa() || game.team_of(*p) != game.team_of(1)).collect();
        let mut t = 10_000;
        'outer: for _ in 0..10_000 {
            for &f in &foes {
                let mut fx = game.handle(MatchEvent::Kill { killer: Some(1), victim: f, assists: vec![] });
                for s in m.scoring.iter().filter(|s| !s.incident.starts_with("player_")) {
                    fx.extend(game.handle(MatchEvent::Incident { name: s.incident.clone(), player: Some(1), team: game.team_of(1) }));
                }
                if fx.iter().any(|e| matches!(e, MatchEffect::Ended(_))) {
                    ended = true;
                    break 'outer;
                }
            }
            t += 6_000;
            if game.handle(MatchEvent::Tick(t)).iter().any(|e| matches!(e, MatchEffect::Ended(_))) {
                ended = true;
                break;
            }
        }
        assert!(ended, "{} never ended", m.id);
        let r = game.result().unwrap();
        assert!(r.won(1, c.rewards.ffa_winning_places), "{}: player 1 won", m.id);
    }
}

#[test]
fn trials_and_iron_banner_rules_come_from_the_data() {
    let lib = lib();
    let c = lib.crucible.as_ref().unwrap();
    let t = &c.trials;
    assert_eq!((t.wins_for_lighthouse, t.losses_to_end, t.fireteam), (9, 3, 3));
    let trials_row = lib.catalog.get(t.playlists[0]).unwrap();
    assert_eq!((trials_row.min_party, trials_row.max_party), (3, 3), "the manifest's fireteam rule");
    let mut card = TrialsPassage::new([]);
    let mut out = PassageOutcome::Continues;
    for _ in 0..9 {
        out = card.record(true, t);
    }
    assert_eq!(out, PassageOutcome::Complete { flawless: true });
    assert!(!c.iron_banner.playlists.is_empty());
    for h in &c.iron_banner.playlists {
        let p = c.playlist(*h).unwrap();
        assert!(p.level_advantages, "{}", p.name);
        assert_eq!(lib.catalog.get(*h).unwrap().level, 40, "{}", p.name);
    }
    // The eleven restored Crucible roots are maps here, flagged as stubs.
    assert_eq!(c.maps.iter().filter(|m| m.script.is_some()).count(), 11);
}

#[test]
fn the_scoreboard_row_has_bullshcripts_shape() {
    let lib = lib();
    let c = lib.crucible.as_ref().unwrap();
    let mut game = CrucibleMatch::new(Arc::new(c.mode("control").unwrap().clone()), Timings::default(), 0);
    for p in 1..=4 {
        game.handle(MatchEvent::Join { player: p, team: None });
    }
    game.handle(MatchEvent::Tick(10_000));
    let fx = game.handle(MatchEvent::Kill { killer: Some(1), victim: 2, assists: vec![3] });
    let Some(MatchEffect::Scoreboard(st)) = fx.iter().find(|f| matches!(f, MatchEffect::Scoreboard(_))) else { panic!("{fx:?}") };
    let js = serde_json::to_value(st).unwrap();
    for k in ["global", "notifications", "players", "teams"] {
        assert!(js.get(k).is_some(), "{k}");
    }
    let p1 = js["players"].as_array().unwrap().iter().find(|p| p["characterKey"] == "1").unwrap();
    let score = p1["stats"].as_array().unwrap().iter().find(|s| s["nameHash"] == pvp::score_hash()).unwrap();
    assert_eq!(score["value"], 100.0);
}

#[test]
fn playlists_map_onto_the_matchmaker_and_runs_onto_the_reward_call() {
    use d1_activity_host::encounter_framework::{matchmaking_playlists, MatchKind};
    let lib = lib();
    let pls = matchmaking_playlists(&lib);
    let legacy = pls.iter().find(|p| p.name == "Vanguard Legacy").unwrap();
    assert_eq!(legacy.kind, MatchKind::Cooperative { max_players: 3 });
    assert!(legacy.activities.iter().all(|h| lib.by_hash(*h).is_some_and(|a| a.kind == ActivityKind::Strike)));
    let control = pls.iter().find(|p| p.name == "Control").unwrap();
    assert_eq!(control.kind, MatchKind::Versus { teams: 2, team_size: 6, min_players: 6 });
    let rumble = pls.iter().find(|p| p.name == "Rumble").unwrap();
    assert_eq!(rumble.kind, MatchKind::Versus { teams: 6, team_size: 1, min_players: 2 });
    let trials = pls.iter().find(|p| p.name == "Trials of Osiris").unwrap();
    assert_eq!(trials.kind, MatchKind::Versus { teams: 2, team_size: 3, min_players: 3 });

    let a = Arc::clone(lib.by_id("raid-crotas-end").unwrap());
    let store = Arc::new(MemoryCheckpointStore::new());
    let (mut run, _) = ActivityRun::start(Arc::clone(&a), a.activity_hashes[0], &[1, 2], Modifiers::default(), store, NOW);
    assert!(run.outcome().is_none(), "still running");
    run.handle(Event::ActivityCompleted);
    let o = run.outcome().unwrap();
    assert!(o.completed);
    assert_eq!(o.community_page.as_deref(), Some("crotas-end-reprised"));
    assert_eq!(o.characters, [1, 2]);
}
