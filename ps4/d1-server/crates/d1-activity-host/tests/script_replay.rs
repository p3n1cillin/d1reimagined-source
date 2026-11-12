// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! ⭐ A deterministic offline replay of Destiny's own mission scripts on the game's own content,
//! through the script bridge: `tools/script-host/tools/replay.mjs` runs an activity on its world's
//! private content export under the coverage driver and writes the sidecar's transcript; a fake
//! sidecar replays the transcript's answers, one tick of them per bridge tick, into the bridge as
//! the committed activity table configures it (`ps4/fixtures/56578/scripts/activities.txt`, the
//! generated world tables and their slice states). The bridge's frames are checked, and two runs
//! must plan the same frames.
//!
//! Private inputs, all required, or the test skips and says why: `D1_SCRIPTS_DIR` (the 56578
//! scripts, unlicensed, local only), `D1_BUNDLES_DIR` (the export, game data, outside the
//! repository), Node (`D1_NODE`, else `node`) and a built BullShcript (`D1_TIGER_DIR`). The
//! transcript is written to a temporary folder and removed; nothing is committed.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use d1_activity_host::generated::GeneratedWorlds;
use d1_activity_host::incident_catalog::IncidentCatalog;
use d1_activity_host::messages::ClientMessage;
use d1_activity_host::rewards::{ActivityOutcome, RecordingRewards};
use d1_activity_host::row_encoders::{CodecDone, CodecJob, CodecWorker, EncodedFrame};
use d1_activity_host::rows::FramePlan;
use d1_activity_host::script_bridge::{
    parse_sidecar_line, HostMsg, LinkEvent, ScriptBridge, ScriptConfig, ScriptedActivity, SidecarLink, SidecarMsg,
};

fn repo() -> PathBuf {
    // crates/d1-activity-host -> crates -> d1-server -> ps4 -> the repository.
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..")
}

/// The private inputs (Node, the content export), or why the test cannot run.
fn inputs() -> Result<(String, String), String> {
    let dir = |k: &str| {
        std::env::var(k).ok().filter(|d| Path::new(d).is_dir()).ok_or_else(|| format!("set {k}"))
    };
    dir("D1_SCRIPTS_DIR")?;
    let bundles = dir("D1_BUNDLES_DIR")?;
    let node = std::env::var("D1_NODE").unwrap_or_else(|_| "node".into());
    std::process::Command::new(&node).arg("--version").output().map_err(|e| format!("{node}: {e} (set D1_NODE)"))?;
    Ok((node, bundles))
}

/// The sidecar's answers, one segment per script tick (segment 0: the answers to `start`).
fn transcript(node: &str, bundles: &str, activity: &str, seconds: u32) -> Vec<Vec<SidecarMsg>> {
    let out = std::env::temp_dir().join(format!("d1-replay-{activity}-{}.jsonl", std::process::id()));
    let status = std::process::Command::new(node)
        .arg(repo().join("tools/script-host/tools/replay.mjs"))
        .args([activity, "--bundles", bundles, "--seconds", &seconds.to_string(), "--out"])
        .arg(&out)
        .status()
        .expect("node");
    assert!(status.success(), "replay.mjs {activity}");
    let text = std::fs::read_to_string(&out).expect("transcript");
    let _ = std::fs::remove_file(&out);
    let mut segments = vec![Vec::new()];
    let mut ticked = false;
    for line in text.lines() {
        let v: serde_json::Value = serde_json::from_str(line).expect("a transcript line");
        if !ticked && v.get("in").and_then(|m| m.get("type")).and_then(|t| t.as_str()) == Some("tick") {
            // The first tick: what came before answered start and the spawn.
            ticked = true;
            segments.push(Vec::new());
        }
        let Some(m) = v.get("out") else { continue };
        let Some(msg) = parse_sidecar_line(&m.to_string()).expect("a protocol 1 line") else { continue };
        let done = matches!(msg, SidecarMsg::TickDone { .. });
        segments.last_mut().unwrap().push(msg);
        if done {
            segments.push(Vec::new());
        }
    }
    segments
}

/// A sidecar that answers with a transcript: `loaded` (content, not stand-ins), the start segment
/// at `start`, then one segment per `tick`.
struct Replay {
    segments: VecDeque<Vec<SidecarMsg>>,
    out: VecDeque<LinkEvent>,
    sent: Arc<Mutex<Vec<HostMsg>>>,
}

impl SidecarLink for Replay {
    fn send(&mut self, msg: &HostMsg) -> Result<(), String> {
        self.sent.lock().unwrap().push(msg.clone());
        let release = match msg {
            HostMsg::Hello { .. } => {
                self.out.push_back(LinkEvent::Msg(SidecarMsg::Loaded {
                    stand_in: false,
                    stub: false,
                    drives_lifetime: true,
                    compile_errors: Vec::new(),
                }));
                false
            }
            HostMsg::Start | HostMsg::Tick { .. } => true,
            _ => false,
        };
        if release {
            for m in self.segments.pop_front().unwrap_or_default() {
                self.out.push_back(LinkEvent::Msg(m));
            }
        }
        Ok(())
    }

    fn poll(&mut self) -> Option<LinkEvent> {
        self.out.pop_front()
    }
}

struct Run {
    frames: Vec<FramePlan>,
    incidents: Vec<Vec<u8>>,
    rewards: Arc<Mutex<RecordingRewards>>,
    sent: Arc<Mutex<Vec<HostMsg>>>,
    activity: ScriptedActivity,
    /// The regions the bridge sent the client to, in order.
    regions: Vec<u32>,
}

/// One pass of the transcript through a bridge configured from the committed table.
fn run(index: u16, segments: &[Vec<SidecarMsg>], secs: u32) -> Run {
    let fixtures = repo().join("ps4/fixtures/56578");
    let mut worlds = GeneratedWorlds::load(&fixtures.join("worlds")).expect("world tables");
    worlds
        .load_slice_states(&std::fs::read_to_string(fixtures.join("scripts/slice-states.tsv")).expect("slice states"))
        .expect("slice states parse");
    let table = std::fs::read_to_string(fixtures.join("scripts/activities.txt")).expect("the activity table");
    let line = table
        .lines()
        .find(|l| l.split_whitespace().next() == Some(&index.to_string()))
        .unwrap_or_else(|| panic!("no row for {index}"));
    let base = d1_oracle::sensor::SensorPhaseUpdate {
        gen: 0, name_hash: 0, name_i8: 0x10, name_sub: 3, phase: 0, tail: None, clients: Vec::new(), authority: None,
    };
    let activity = ScriptedActivity::parse_with(line, &base, &|_| Err("lists come from the tables".into()), Some(&worlds))
        .expect("the committed row parses");
    let frames = Arc::new(Mutex::new(Vec::new()));
    let f = frames.clone();
    let rewards = Arc::new(Mutex::new(RecordingRewards::default()));
    let config = ScriptConfig {
        sidecar: vec!["replay".into()],
        activities: HashMap::from([(index, activity.clone())]),
        gen: 0,
        tail: None,
        incidents: IncidentCatalog::load(&fixtures.join("incidents-56578.tsv")).expect("catalogue"),
        incidents_path: None,
        mission_phase: false,
        send_unlisted: false,
        delta_init: false,
        scoreboard_columns: Vec::new(),
        rewards: rewards.clone(),
        codec: Arc::new(move || {
            let f = f.clone();
            CodecWorker::inline(Box::new(move |job: &CodecJob| match job {
                CodecJob::Frame(plan) => {
                    f.lock().unwrap().push(plan.clone());
                    let mut bytes = d1_oracle::sensor::heartbeat_frame(plan.gen, 0);
                    bytes.push(plan.entries.len() as u8);
                    CodecDone::Frame(Ok(EncodedFrame { bytes, stamp_bit: 9 }))
                }
                CodecJob::Object { .. } => CodecDone::Object(Ok((725, vec![0xCC; 91]))),
            }))
        }),
        active_bundles: true,
        start_delay: Duration::ZERO,
        arrival_cushion: Duration::ZERO,
        list_retry: None,
        world_loot: Vec::new(),
        device_rules: Vec::new(),
    };
    let sent = Arc::new(Mutex::new(Vec::new()));
    let (s, segs) = (sent.clone(), segments.to_vec());
    let mut bridge = ScriptBridge::with_launcher(
        Arc::new(config),
        Box::new(move |_| {
            Ok(Box::new(Replay { segments: segs.clone().into(), out: VecDeque::new(), sent: s.clone() }) as Box<dyn SidecarLink>)
        }),
    );
    let mut struct_image = vec![0u8; 0x200];
    struct_image[0x10..0x38].fill(7);
    let mut t = Instant::now();
    bridge.on_client_message(&ClientMessage::JoinRequest { struct_image }, index, 0xD1A0_0000_0000_0042, t);
    let mut incidents = Vec::new();
    let mut regions = Vec::new();
    let end = t + Duration::from_secs(u64::from(secs));
    while t < end {
        t += Duration::from_millis(100);
        let _ = bridge.sensor_due(t);
        while let Some(i) = bridge.take_incident() {
            incidents.push(i);
        }
        // The client goes where the host sends it and reports its slice set (udt 21 +0xB8).
        if let Some(region) = bridge.take_region_request() {
            regions.push(region);
            let mut image = vec![0u8; 0x220];
            image[0xB8..0xBC].copy_from_slice(&region.to_le_bytes());
            image[0xBC..0xC0].copy_from_slice(&0x1234_5678u32.to_le_bytes());
            bridge.on_client_message(&ClientMessage::ClientAuthoritativeDataUpdate { struct_image: image }, index, 0xD1A0_0000_0000_0042, t);
        }
    }
    let frames = frames.lock().unwrap().clone();
    Run { frames, incidents, rewards, sent, activity, regions }
}

fn lifetimes(frames: &[FramePlan]) -> Vec<i8> {
    let mut v: Vec<i8> = frames
        .iter()
        .filter_map(|p| p.entries.iter().find(|(k, _)| k.kind == 0x10).and_then(|(_, r)| r.a.as_ref()).map(|(_, i)| i[0] as i8))
        .collect();
    v.dedup();
    v
}

#[test]
fn cine_reveal_replays_from_its_own_scripts_on_content() {
    let (node, bundles) = match inputs() {
        Ok(i) => i,
        Err(why) => return eprintln!("SKIP: {why} (the 56578 scripts, the content export and Node are private inputs)"),
    };
    let segments = transcript(&node, &bundles, "cine_reveal", 120);
    let a = run(165, &segments, 130);
    let b = run(165, &segments, 130);
    assert_eq!(a.frames, b.frames, "the replay is deterministic");
    let seq = lifetimes(&a.frames);
    assert_eq!(&seq[seq.len() - 3..], [4, 6, 8], "the reveal ends 4 -> 6 -> 8 ({seq:?})");
    assert!(seq.contains(&1), "pregame first ({seq:?})");
    // Content rows reach the wire: the reveal's two cinematic records, by the game's indices.
    let cinematics: std::collections::BTreeSet<i16> = a
        .frames
        .iter()
        .skip(1)
        .flat_map(|p| p.entries.iter())
        .filter(|(_, r)| r.a.as_ref().is_some_and(|(_, i)| i.get(8) == Some(&1)))
        .filter(|(k, _)| k.group == 0x2FE7_DBE6 && k.kind == 6)
        .map(|(k, _)| k.index)
        .collect();
    assert_eq!(cinematics.into_iter().collect::<Vec<_>>(), [0, 1], "both cinematic records played (+8 set)");
    // Phase regions came from content: 25 for the Traveler, 26 for the Tower (no regions= in the table).
    assert_eq!(a.activity.phase_slices.get(&0x77A6_931D), Some(&25));
    assert_eq!(a.activity.phase_slices.get(&0xD77D_27FF), Some(&26));
    // The client loads 25, the live start region (the start slice set from content), so the only
    // move is phase 1's to 26 (unit test: "phase 0 is region 25, the start slice set the client
    // loaded"). This replay needs private inputs, so CI skips it; it still expected a move to 25.
    assert_eq!(a.regions, [26], "the Traveler in 25 (loaded), then the Tower cinematic in 26");
    assert!(a.frames.iter().filter(|p| p.full).count() >= 2, "26's own list goes out in full after the move");
    assert!(a.sent.lock().unwrap().iter().any(|m| matches!(m, HostMsg::ActiveBundles { .. })), "26's bundles made active");
    // The completion: one envelope, the cinematic chain, granted once as Completed, and the hand-off.
    assert_eq!(a.incidents.len(), 1, "one activity_complete");
    assert_eq!(u32::from_le_bytes(a.incidents[0][0..4].try_into().unwrap()), 4, "cinematic-only: four incidents");
    let r = a.rewards.lock().unwrap();
    assert_eq!(r.grants.len(), 1);
    assert_eq!((r.grants[0].1, r.grants[0].2), (0x6EAF_0959, ActivityOutcome::Completed));
    assert_eq!(r.hand_offs.iter().map(|h| (h.2, h.3)).collect::<Vec<_>>(), [(0, 0x8AE8_DA69)], "cine_reveal hands off to the Tower");
    assert!(a.sent.lock().unwrap().iter().any(|m| matches!(m, HostMsg::Tick { .. })));
}

#[test]
fn a_guardian_rises_replays_from_its_own_scripts_on_content() {
    let (node, bundles) = match inputs() {
        Ok(i) => i,
        Err(why) => return eprintln!("SKIP: {why} (the 56578 scripts, the content export and Node are private inputs)"),
    };
    let segments = transcript(&node, &bundles, "act_cosmo_m0_highway", 300);
    let a = run(164, &segments, 310);
    let b = run(164, &segments, 310);
    assert_eq!(a.frames, b.frames, "the replay is deterministic");
    let seq = lifetimes(&a.frames);
    assert!(seq.contains(&1) && seq.contains(&4), "pregame, then the intro's cinematic hold ({seq:?})");
    assert!(a.frames.first().is_some_and(|p| p.full), "the full list first");
    // Every row a delta carries is one the client's current list names (unlisted rows are held
    // back); each slice set the client moves to sends its own list in full first.
    let mut listed = std::collections::HashSet::new();
    let mut deltas = Vec::new();
    for p in &a.frames {
        if p.full {
            listed = p.entries.iter().map(|(k, _)| *k).collect();
        } else {
            for (k, _) in &p.entries {
                assert!(listed.contains(k), "{k:?} is not in the list the client holds");
                deltas.push(*k);
            }
        }
    }
    assert!(!a.regions.is_empty(), "the scripts' phases moved the client ({:?})", a.regions);
    let kinds: std::collections::BTreeSet<u8> = deltas.iter().map(|k| k.kind).collect();
    assert!(kinds.len() >= 3, "the scripts moved several kinds of content row ({kinds:?})");
    assert!(a.incidents.is_empty(), "AGR does not complete in five scripted minutes under the driver");
    assert!(a.rewards.lock().unwrap().grants.is_empty());
}

/// Every row of the committed activity table parses against the committed world tables: its
/// `@generated` list, its lifetime group, its manifest hashes and, where it names one, its next
/// activity. No private input is needed.
#[test]
fn the_committed_activity_table_parses_for_every_story_cinematic_and_quest_activity() {
    let fixtures = repo().join("ps4/fixtures/56578");
    let mut worlds = GeneratedWorlds::load(&fixtures.join("worlds")).expect("world tables");
    worlds
        .load_slice_states(&std::fs::read_to_string(fixtures.join("scripts/slice-states.tsv")).expect("slice states"))
        .expect("slice states parse");
    let table = std::fs::read_to_string(fixtures.join("scripts/activities.txt")).expect("the activity table");
    let base = d1_oracle::sensor::SensorPhaseUpdate {
        gen: 0, name_hash: 0, name_i8: 0x10, name_sub: 3, phase: 0, tail: None, clients: Vec::new(), authority: None,
    };
    let mut n = 0;
    for line in table.lines().map(|l| l.split('#').next().unwrap_or("").trim()).filter(|l| !l.is_empty()) {
        let a = ScriptedActivity::parse_with(line, &base, &|_| Err("lists come from the tables".into()), Some(&worlds))
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(!a.rows.is_empty(), "{}: an empty list", a.activity);
        assert_eq!(a.lifetime.kind, 0x10);
        assert!(a.slice_rows.contains_key(&a.start.expect("start=")), "{}: its start set's list", a.activity);
        n += 1;
    }
    assert!(n >= 100, "{n} activities");
    let reveal = table.lines().find(|l| l.starts_with("165 ")).expect("cine_reveal's row");
    let a = ScriptedActivity::parse_with(reveal, &base, &|_| Err(String::new()), Some(&worlds)).unwrap();
    assert_eq!((a.activity.as_str(), a.start, a.next), ("cine_reveal", Some(25), Some((0, 0x8AE8_DA69))));
    assert_eq!(a.end, d1_activity_host::row_encoders::EndKind::Cinematic);
    let h = a.hashes.expect("the completion body's hashes");
    assert_eq!((h.activity, h.kind, h.kind2), (0x6EAF_0959, 0x6489_99F4, 0x4D78_882E), "Story, then STORY_CINEMATIC, as the live bodies");
}
