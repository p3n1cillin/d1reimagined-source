// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! `d1-activity-host` - run the Activity Host, or run its controls.
//!
//! ⭐⭐ **The named gate this used to stop at is OPEN (2026-08-08).** `serve` accepts the
//! connection, completes the BAP handshake, decodes BAP message 8 **through the client's own
//! schema codec** (`FUN_002A4090`, run under Unicorn by `d1-oracle`), runs the state machine, and
//! frames `join_result` and `global_activity_state` back. See `d1_activity_host::codec`.
//!
//! ⚠ It needs the **pinned eboot at runtime** and refuses to listen without it, because a listener
//! that can only produce named gates is worse than one that will not start.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use clap::{Parser, Subcommand};

use d1_activity_host::codec::{AhWireCodec, RawStructCodec, SchemaCodec};
use d1_activity_host::listener::{serve, AhBind};
use d1_activity_host::state::ActivityInstance;

#[derive(Parser)]
#[command(name = "d1-activity-host", about = "The Activity Host role for the D1 preservation lab")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run every control. **Exit 2 means a control failed and the output may not be cited.**
    Controls,
    /// Listen for the client's Activity-Host connection.
    Serve {
        /// Address to listen on. This is what BAP message 17 must hand out.
        #[arg(long, default_value = "0.0.0.0:4110")]
        bind: SocketAddr,
        /// The activity-host id to present. Must be non-zero.
        #[arg(long, default_value_t = d1_activity_host::registry::FIRST_ACTIVITY_HOST_ID)]
        ah_id: u64,
        /// ⛔ Use the raw-struct codec. **FALSIFIED on the live client 2026-08-08** - it is kept
        /// only as the negative control that the schema encoding is not a tautology. Do not use it
        /// to try to reach an activity.
        #[arg(long)]
        raw_struct_codec: bool,
        /// ⭐ The activity name published in `global_activity_state+0x688`. Rung 31 reads it, and
        /// it should match the `act_*` world the change-world command forced.
        ///
        /// ⚠ `capture-rig/run-signon-experiment.sh` launches this binary with a fixed argument
        /// list, so **`D1_AH_ACTIVITY` is the way to set it from a run** without editing the
        /// launcher. The flag wins if both are given.
        #[arg(long)]
        activity: Option<String>,
    },
    /// Print the 49 `user_defined_type` ids with their direction.
    Ids,
}

/// The Elimination index for the selected build. 01.29's activity table has it at 407
/// ([`ActivityInstance::DEFAULT_ACTIVITY_INDEX`]); build 56578's private-match band moved by
/// +7..+9 (`ps4_investment_globals_0131_6` blocks 182/187), which puts Elimination at 416 and
/// Salvage at 407.
fn default_activity_index() -> u16 {
    match d1_build::selected().id.as_str() {
        "later-56578" => 416,
        _ => ActivityInstance::DEFAULT_ACTIVITY_INDEX,
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Controls => std::process::exit(d1_activity_host::controls::main()),
        Command::Ids => {
            for id in 0..d1_activity_host::protocol::AH_MESSAGE_COUNT as u32 {
                println!(
                    "{id:3}  {:<14}  {}",
                    format!("{:?}", d1_activity_host::protocol::direction(id).unwrap()),
                    d1_activity_host::protocol::name_or_unknown(id)
                );
            }
            Ok(())
        }
        Command::Serve { bind, ah_id, raw_struct_codec, activity } => {
            tracing_subscriber::fmt().with_env_filter("info").init();
            if ah_id == 0 {
                anyhow::bail!(
                    "activity-host id 0 is the `no activity host` sentinel - FUN_00594320 issues \
                     nothing while it reads zero"
                );
            }
            let codec: Arc<dyn AhWireCodec> = if raw_struct_codec {
                tracing::warn!(
                    "⛔ RAW-STRUCT CODEC: FALSIFIED on the live client 2026-08-08 (run \
                     20260808-210910-AH-RAWCODEC) - the pushes went out and the client rejected \
                     them, ending in error.starting_private_activity_instance_timed_out_waiting_\
                     for_join_response. It is kept only as the negative control. You almost \
                     certainly want the default."
                );
                Arc::new(RawStructCodec)
            } else {
                // ⭐ Build the oracles BEFORE listening. The first use of a udt costs ~2 s (29 MB
                // image + 70,682 relocations + the pinned object control), and rung 30's failure
                // mode is a TIMEOUT - paying that inside the join handler would be self-inflicted.
                // It is also where a missing eboot must surface: at launch, not at the join.
                let started = Instant::now();
                let mut failed = 0usize;
                for (udt, r) in SchemaCodec::prewarm(&SchemaCodec::HOT_PATH) {
                    match r {
                        Ok(n) => tracing::info!(
                            "codec ready: udt {udt} ({}) struct {n:#x}",
                            d1_activity_host::protocol::name_or_unknown(udt)
                        ),
                        Err(e) => {
                            failed += 1;
                            tracing::warn!("** NAMED GATE ** codec NOT ready: {e}");
                        }
                    }
                }
                if failed == SchemaCodec::HOT_PATH.len() {
                    anyhow::bail!(
                        "the schema codec could not be built for ANY hot-path message. It runs the \
                         client's own FUN_002A4090 under Unicorn, so it needs the pinned eboot: set \
                         D1_EBOOT, or make dumps/patch-01.29-CUSA00219/eboot.bin readable. Refusing \
                         to serve a listener that can only produce named gates."
                    );
                }
                tracing::info!(
                    "schema codec warm in {:?} - the CLIENT'S OWN FUN_002A4090/FUN_002A4140, \
                     join_request 0x{:08X} / join_result 0x{:08X}",
                    started.elapsed(),
                    d1_activity_host::codec::TAG_JOIN_REQUEST,
                    d1_activity_host::codec::TAG_JOIN_RESULT
                );
                // ⚠ Same rule as `prewarm`, one call later: this walks the descriptor tree (~2 s
                // cold) and the first message that would trigger it lazily is the join_request.
                let undecodable = d1_activity_host::codec::warm_undecodable_set();
                tracing::info!(
                    "⭐ not-decodable by construction: udt {undecodable:?} - a DYNAMIC FLAGS \
                     descriptor (kind 0x22) whose bitset comes from a runtime pool a static image \
                     does not have. Both are client→AH with no client handler, so nothing waits \
                     on them."
                );
                // ⭐⭐ The oracle's bump arena, in the run log, at startup.
                //
                // Run `20260809-120451-MOONBASE-FRESH` died of `scratch heap exhausted` thirteen
                // minutes in and nothing in its log had ever named the arena, so the failure had
                // to be reasoned back from `error.private_activity_host_host_crashed`. These five
                // lines cost nothing and make the resource visible before it matters. The invariant
                // to read them against: **`high-water` must never move again** for the life of the
                // process, because every codec call rewinds to its scaffolding mark
                // (`d1_oracle::schema::SchemaHarness::begin_call`).
                for a in SchemaCodec::arena_stats() {
                    tracing::info!("oracle arena {}", a.line());
                }
                Arc::new(SchemaCodec)
            };
            tracing::warn!(
                "⛔ CEILING: a stub Activity Host yields a LOADED BUT EMPTY world - no spawns, no \
                 encounters, no objectives. The mission logic is cloud-side content."
            );
            let activity = activity
                .or_else(|| std::env::var("D1_AH_ACTIVITY").ok().filter(|s| !s.is_empty()))
                .unwrap_or_else(|| {
                    ActivityInstance::DEFAULT_ACTIVITY_NAME.to_string()
                });
            if activity.len() >= d1_activity_host::messages::GlobalActivityState::NAME_CAPACITY {
                anyhow::bail!(
                    "--activity {activity:?} does not fit the char[0x28] at \
                     global_activity_state+0x688; a truncated name is a DIFFERENT activity"
                );
            }
            tracing::info!(
                "publishing activity name {activity:?} at global_activity_state+0x{:03X} \
                 (rung 31 reads it; FUN_01057B80 sets ac+0x130 |= 0x40 only on a successful decode)",
                d1_activity_host::messages::GlobalActivityState::NAME
            );
            // ⭐⭐ THE FIELD RUNG 33 GATES ON. `FUN_0078C770` reads a u16 at
            // `global_activity_state+0x680`; `FUN_007A7420` hands it to `FUN_0054F410` *and* to
            // `activity_table[idx]` for the name it prints. Every run so far sent 0.
            let activity_index = std::env::var("D1_AH_ACTIVITY_INDEX")
                .ok()
                .and_then(|s| s.parse::<u16>().ok())
                .unwrap_or_else(default_activity_index);
            if activity_index == 0 {
                tracing::warn!(
                    "⚠ D1_AH_ACTIVITY_INDEX WAS EXPLICITLY SET TO 0 at \
                     global_activity_state+0x{:03X} (the default is now \
                     {} = ELIMINATION) - the client will \
                     evaluate `activity_table[0]` = HASH(0x8AE8DA69) = \"Tower\", whose \
                     prerequisites are unmet, and rung 33 will emit \
                     `activity_failed_to_load_missing_dlc` regardless of which activity was \
                     poked. That is runs 20260809-054417 and 20260809-075545, exactly. Unset the \
                     variable to get the default back; 0 is a bisect lever, not a configuration.",
                    d1_activity_host::messages::GlobalActivityState::ACTIVITY_INDEX,
                    ActivityInstance::DEFAULT_ACTIVITY_INDEX
                );
            } else {
                tracing::info!(
                    "⭐ publishing activity INDEX {activity_index} at \
                     global_activity_state+0x{:03X} - rung 33 (FUN_0078C770 -> FUN_007A7420 -> \
                     FUN_0054F410) evaluates THIS number's prerequisites, and prints \
                     activity_table[{activity_index}]'s name hash. It is NOT derived from the \
                     activity name; they are adjacent, independent fields.",
                    d1_activity_host::messages::GlobalActivityState::ACTIVITY_INDEX
                );
            }
            // ⭐⭐ WHAT THAT INDEX MEANS FOR `Multiplayer Team`, said ONCE, here.
            //
            // The index is a MODE, and the mode carries a gametype whose team-assignment byte
            // decides which of the client's two non-matchmaking assigners runs - or whether either
            // does. 407's is COPY-FROM-FIRETEAM, whose source is never written on the 393-413 band,
            // so `activities:team_picker:dump` prints `Multiplayer Team=-1` however well everything
            // else goes. That failure is silent by construction: nothing refuses and nothing errors.
            // ⇒ `d1_activity_host::fireteam`.
            let team = d1_activity_host::fireteam::receipt(activity_index);
            if team.is_warning {
                tracing::warn!("{}", team.message);
            } else {
                tracing::info!("{}", team.message);
            }
            // ⭐⭐ THE LEASE. Run `20260809-054417-RUNG33-BYPASS` established the session and then
            // went silent; the client tore it down after 20,626 ms with
            // ⭐⭐ The activity CHECKSUM at global_activity_state+0x5C. `FUN_00299100` compares it
            // against content and aborts the world transition on a mismatch - measured 3/3 in
            // 20260809-084932-RUNG33-PERMS6 as `transition_error: activity checksum mismatch`.
            //
            // ⭐⭐ THE CHECKSUM, THE BUBBLE COUNT AND THE CHOSEN BUBBLES ARE ONE FACT: THE WORLD.
            // All three are read out of the scenario record of the world the client ACTUALLY
            // LOADED, so they are derived from `activity` (the MAP name) via
            // `d1_activity_host::worlds`, and an explicit override that contradicts a measured
            // world is reported ONCE, here, instead of surfacing three rungs later as a gate
            // failure that names the field and never the cause.
            let parse_radix = |s: String| -> Option<u32> {
                let t = s.trim();
                let (r, base) = match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                    Some(h) => (h, 16),
                    None => (t, 10),
                };
                u32::from_str_radix(r, base).ok()
            };
            let checksum_override =
                std::env::var("D1_AH_ACTIVITY_CHECKSUM").ok().and_then(&parse_radix);
            let count_override = std::env::var("D1_AH_BUBBLE_COUNT").ok().and_then(&parse_radix);
            // Resolved WITHOUT the chosen set first, because `D1_AH_BUBBLE_CHOSEN=all` has to
            // expand against the resolved count. `worlds::resolve` is pure, so calling it twice is
            // free and keeps one source for every derivation.
            let counted =
                d1_activity_host::worlds::resolve(Some(&activity), checksum_override, count_override, None);
            match counted.world {
                Some(w) => tracing::info!(
                    "⭐⭐ world {:?} is a MEASURED scenario record (tag {:#010x}): checksum \
                     {:#010x} at +0x0C, {} bubble(s) at +0x48, choosable {:?}. Rung 33 compares \
                     our blob against THIS record - not against the activity index, which cannot \
                     change any of it.",
                    w.name, w.scenario_tag, w.checksum, w.bubble_count, w.chosen
                ),
                None => tracing::warn!(
                    "⚠ world {activity:?} has NO captured scenario record in \
                     d1_activity_host::worlds, so the checksum / bubble count / chosen bubbles \
                     CANNOT be derived and fall back to the pre-2026-08-10 zeros unless you set \
                     all three by hand. A world's record is resident only while it loads: capture \
                     it with `capture-rig/scenario-checksum.py watch --hz 500` (⚠ --hz 50 MISSES \
                     the ~100 ms window and reads like the world never loaded) and add a row."
                ),
            }
            let activity_checksum = counted.checksum;
            if activity_checksum == 0 {
                tracing::warn!(
                    "⚠ activity checksum 0 at global_activity_state+0x{:02X} - FUN_00299100 \
                     compares this against the content registry and, on a mismatch, calls \
                     `(*param_2)(5, 0, \"activity checksum mismatch\")`, which surfaces as \
                     `world_controller:transition_error` and ABORTS the world transition. That is \
                     runs 20260809-084932 (3 of 3). The world then changes to the EMPTY STRING and \
                     rung 33 reports \"we don't have an active activity\". A measured world \
                     supplies this by itself now, so 0 means either an UNMEASURED world name or an \
                     explicit D1_AH_ACTIVITY_CHECKSUM=0 - both of which are bisect levers, not \
                     configurations.",
                    d1_activity_host::messages::GlobalActivityState::CHECKSUM
                );
            } else {
                tracing::info!(
                    "⭐ publishing activity CHECKSUM {activity_checksum:#010x} at \
                     global_activity_state+0x{:02X} - FUN_00299100 must find this equal to the \
                     content's own value or the world transition aborts.",
                    d1_activity_host::messages::GlobalActivityState::CHECKSUM
                );
            }
            // ⭐⭐ The bubble count at global_activity_state+0x10. FUN_002A0320 runs right after
            // the checksum passes and requires this to equal the world's scenario bubble count
            // (measured live: orbit = 16). It also sets the 0xFF "unchosen" bytes at +0x14.
            let bubble_count = counted.bubble_count;
            let cap = d1_activity_host::messages::GlobalActivityState::BUBBLE_STATE_CAPACITY as u32;
            if bubble_count == 0 {
                tracing::warn!(
                    "⚠ bubble count 0 at global_activity_state+0x{:02X} - FUN_002A0320 compares this \
                     against the scenario record's +0x48 (the world's bubble count) and, when ours \
                     is 0, calls `(*cb)(6, 0, \"invalid host activity state\")`, aborting the world \
                     transition. That is run 20260809-084932's NEW wall after the checksum was \
                     fixed. A measured world supplies this by itself now (pvp_moonbase = 1, orbit \
                     = 16), so 0 means an UNMEASURED world name or an explicit \
                     D1_AH_BUBBLE_COUNT=0.",
                    d1_activity_host::messages::GlobalActivityState::BUBBLE_COUNT
                );
            } else if bubble_count > cap {
                anyhow::bail!(
                    "D1_AH_BUBBLE_COUNT={bubble_count} exceeds the {cap}-byte bubble-state field at \
                     global_activity_state+0x{:02X}; a count that large cannot be marked unchosen \
                     in one blob and would over-run the struct.",
                    d1_activity_host::messages::GlobalActivityState::BUBBLE_STATE
                );
            } else {
                tracing::info!(
                    "⭐ publishing bubble count {bubble_count} at global_activity_state+0x{:02X} \
                     (and {bubble_count} bubble-state bytes at +0x{:02X}, whose chosen/unchosen \
                     split the next line reports) - FUN_002A0320 must find this equal to the \
                     scenario record's +0x48 or the world transition aborts.",
                    d1_activity_host::messages::GlobalActivityState::BUBBLE_COUNT,
                    d1_activity_host::messages::GlobalActivityState::BUBBLE_STATE
                );
            }
            // ⭐⭐ WHICH bubbles are CHOSEN (`0x00` at +0x14); the rest stay `0xFF` (unchosen).
            // This is rung 34's gate. FUN_002A0320's unchosen branch calls
            // `FUN_00343020(ctx, bubble*8, 0xFFFFFFFF, 0)`, which writes -1 into the slice-set
            // registry slot - i.e. it UNREGISTERS the region. Rung 34's FUN_0077E440 then starts a
            // `transitioning:initial_slice_set` on a region id read from content and FUN_00344A50
            // returns state 5 ⇒ `Region 'PRV00.00 (HASH: …)' cannot be found!` ⇒
            // `error.sstm_slice_not_found`. Accepts `0`, `0-4,9-14`, or `all`.
            //
            // ⚠⚠ THREE STATES, AND ONLY THE FIRST MAY BE DEFAULTED. `Err` = unset ⇒ the measured
            // world's choosable set. `Ok("")` = EXPLICITLY empty ⇒ all-`0xFF`, which is the LOBSTER
            // / `error.sstm_slice_not_found` negative control and must stay reachable. `Ok(spec)` =
            // exactly what was asked for. Collapsing the first two would delete a control.
            let chosen_override: Option<Vec<usize>> = match std::env::var("D1_AH_BUBBLE_CHOSEN") {
                Err(_) => None,
                Ok(spec) => Some({
                    let t = spec.trim();
                    if t.eq_ignore_ascii_case("all") {
                        (0..bubble_count as usize).collect()
                    } else if t.is_empty() {
                        Vec::new()
                    } else {
                        let mut v = Vec::new();
                        for part in t.split(',').map(str::trim).filter(|p| !p.is_empty()) {
                            let (lo, hi) = match part.split_once('-') {
                                Some((a, b)) => (a.trim(), b.trim()),
                                None => (part, part),
                            };
                            let (lo, hi) = match (lo.parse::<usize>(), hi.parse::<usize>()) {
                                (Ok(a), Ok(b)) if a <= b => (a, b),
                                _ => anyhow::bail!(
                                    "D1_AH_BUBBLE_CHOSEN={spec:?}: {part:?} is not a bubble index \
                                     or an ascending `lo-hi` range. Use e.g. `0`, `0-4,9-14`, `all`."
                                ),
                            };
                            v.extend(lo..=hi);
                        }
                        v.sort_unstable();
                        v.dedup();
                        v
                    }
                }),
            };
            // ⭐⭐ THE FULL RESOLUTION, and the ONE place a cross-world misconfiguration is named.
            let resolved = d1_activity_host::worlds::resolve(
                Some(&activity),
                checksum_override,
                count_override,
                chosen_override,
            );
            for d in &resolved.disagreements {
                tracing::warn!("⛔ OVERRIDE CONTRADICTS THE MEASURED WORLD: {d}");
            }
            let bubble_chosen = resolved.chosen.clone();
            if let Some(&bad) = bubble_chosen.iter().find(|&&i| i >= bubble_count as usize) {
                anyhow::bail!(
                    "D1_AH_BUBBLE_CHOSEN names bubble {bad} but D1_AH_BUBBLE_COUNT is \
                     {bubble_count}; the client only walks `[0, count)` bubble-state bytes, so that \
                     index would be published and never read - a knob that silently does nothing."
                );
            }
            if bubble_chosen.is_empty() {
                tracing::warn!(
                    "⚠ NO CHOSEN BUBBLES - publishing ALL {bubble_count} bubbles as \
                     0xFF 'unchosen' at global_activity_state+0x{:02X}. That PASSES rung 33 and \
                     then guarantees rung 34 fails: FUN_002A0320's unchosen branch calls \
                     FUN_00343020(ctx, bubble*8, -1, 0), which UNREGISTERS the slice-set region, \
                     and FUN_0077E440 then cannot find it - \
                     `Region 'PRV00.00 (HASH: …)' cannot be found!` = \
                     `error.sstm_slice_not_found`, run 20260809's rung-34 wall. A measured world \
                     supplies this by itself now (pvp_moonbase: `0`; orbit: `0-4,9-14`), so an \
                     empty set means an UNMEASURED world name or an explicit \
                     `D1_AH_BUBBLE_CHOSEN=`, which is the LOBSTER negative control.",
                    d1_activity_host::messages::GlobalActivityState::BUBBLE_STATE
                );
            } else {
                tracing::info!(
                    "⭐ publishing bubbles {bubble_chosen:?} as CHOSEN (0x00) at \
                     global_activity_state+0x{:02X}; the other {} stay 0xFF. A chosen bubble is \
                     what REGISTERS its slice-set region for rung 34. ⚠ Choosing a bubble whose \
                     scenario sub-bubble count is 0 is the loud \
                     `activity client bubble state index was valid, but state itself was invalid`.",
                    d1_activity_host::messages::GlobalActivityState::BUBBLE_STATE,
                    bubble_count as usize - bubble_chosen.len()
                );
            }
            // `error.private_activity_host_host_crashed`. The number is the client's own.
            let heartbeat_ms = std::env::var("D1_AH_HEARTBEAT_MS")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(ActivityInstance::HOST_HEARTBEAT_MS);
            if heartbeat_ms >= d1_activity_host::state::HOST_SILENCE_LIMIT_MS {
                anyhow::bail!(
                    "D1_AH_HEARTBEAT_MS={heartbeat_ms} is at or above the client's own host-crash \
                     timeout of {} ms (FUN_0075E440: `20000 + 500 <= now - conn+0x1EF8`). That is \
                     not a heartbeat, it is a countdown, and it reproduces run \
                     20260809-054417-RUNG33-BYPASS exactly.",
                    d1_activity_host::state::HOST_SILENCE_LIMIT_MS
                );
            }
            tracing::info!(
                "⭐ host heartbeat every {heartbeat_ms} ms - the client declares \
                 `error.private_activity_host_host_crashed` after {} ms of OUR silence \
                 (FUN_0075E440 reads FUN_0029CA00(ac) = now - conn+0x1EF8, the AH connection's \
                 last-received time). udt 15's keepalive is the client talking to US and is NOT \
                 what holds the session open. D1_AH_HEARTBEAT_MS overrides.",
                d1_activity_host::state::HOST_SILENCE_LIMIT_MS
            );
            // ⭐⭐ RUNG 34 - the PAH-tabulated region record in replicate_membership+0x2E48.
            // `FUN_007775B0` copies a record into `sstm+0x490` only when its region id equals the
            // PENDING region (`sstm+0x138`, measured 0 for a one-bubble PvP map) AND its per-peer
            // token byte at `+0x38 + peer` equals the transition token (`sstm+0x1D0`).
            // `FUN_0077E5A0` advances that token on EVERY transition (`t = t + 1; if (t == 0) t =
            // 1;`), so it is 1 on the first slice-set transition after a boot.
            //
            // ⛔ **REFUTED, in place: the token is NOT stale-per-transition.** It RESETS on the
            // fallback - the client printed `token advanced to 001` on *both* transitions of a
            // two-poke run and `sstm+0x1D0` reads 0 at rest. So `1` is correct for EVERY poke, not
            // just the first after a boot, which is what makes defaulting it sound rather than
            // optimistic. ⚠ `D1_AH_SLICE_TOKEN=0` is preserved as the explicit "publish a ZEROED
            // PAH array" bisect lever - the pre-2026-08-09 behaviour and rung 34 task 8's negative
            // control. Do not collapse it into the default.
            let parse_u32 = |v: String| -> Option<u32> {
                let t = v.trim().to_string();
                let (r, base) = match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                    Some(h) => (h.to_string(), 16),
                    None => (t, 10),
                };
                u32::from_str_radix(&r, base).ok()
            };
            let slice_token = std::env::var("D1_AH_SLICE_TOKEN")
                .ok()
                .and_then(&parse_u32)
                .or(Some(u32::from(ActivityInstance::DEFAULT_SLICE_TOKEN)));
            let slice_region =
                std::env::var("D1_AH_SLICE_REGION").ok().and_then(&parse_u32).unwrap_or(0);
            let slice_peer = std::env::var("D1_AH_SLICE_PEER")
                .ok()
                .and_then(&parse_u32)
                .unwrap_or(0) as usize;
            match slice_token {
                Some(t) if t <= u8::MAX as u32 && t != 0 => tracing::info!(
                    "⭐ D1_AH_SLICE_TOKEN={t} D1_AH_SLICE_REGION={slice_region} \
                     D1_AH_SLICE_PEER={slice_peer} - publishing one PAH-tabulated region record at \
                     replicate_membership+0x{:04X}. This is the ONLY thing between us and rung 34 \
                     task 8: without it FUN_0077F6B0 prints \"we do not yet have valid \
                     host-tabulated data for it. Waiting.\" forever. ⭐ The token RESETS on the \
                     fallback (sstm+0x1D0 reads 0 at rest and the client printed `token advanced \
                     to 001` on both transitions of a two-poke run), so {} is correct for every \
                     poke, not only the first after a boot.",
                    d1_activity_host::messages::ReplicateMembership::PAH_REGIONS,
                    ActivityInstance::DEFAULT_SLICE_TOKEN
                ),
                Some(0) => tracing::warn!(
                    "⚠ D1_AH_SLICE_TOKEN=0 - publishing a ZEROED PAH tabulated region array at \
                     replicate_membership+0x{:04X}. FUN_007775B0 will match nothing, sstm+0x490 \
                     stays -1, and rung 34 task 8 parks forever on 'Region … is ready for switch, \
                     but we do not yet have valid host-tabulated data for it. Waiting.' That is \
                     every run before 2026-08-09. This is the BISECT LEVER, not a configuration - \
                     unset the variable to get the default token \
                     {} back. ⇒ historicaldocs/design/rung34-task7.md",
                    d1_activity_host::messages::ReplicateMembership::PAH_REGIONS,
                    ActivityInstance::DEFAULT_SLICE_TOKEN
                ),
                Some(t) => tracing::warn!(
                    "⚠ D1_AH_SLICE_TOKEN={t} is not a valid transition token - FUN_0077E5A0 keeps \
                     it in 1..=255 (it skips 0 on wrap). Ignoring; rung 34 will park."
                ),
                None => {}
            }
            let pah_region = slice_token
                .filter(|t| *t != 0 && *t <= u8::MAX as u32)
                .map(|t| (slice_region, slice_peer, t as u8));

            // ⭐⭐ RUNG 34→spawn: the udt-5 `sensor_auth_update` that writes the activity lifetime
            // phase (`record+0x1AC`) the player-spawn gate reads. Off unless `D1_AH_SENSOR_PHASE`
            // is set - this is `synthetic-minimal` and its post-pass dispatch is `[U]` until the
            // live watch on `record+0x1AC` confirms it. ⇒ design/activity-lifetime-state.md.
            let mut sensor_phase_file = None;
            let mut sensor_team_frames: Option<(Vec<u8>, Vec<u8>, std::collections::HashSet<u16>)> = None;
            let mut world_table: std::collections::HashMap<u16, d1_activity_host::state::ServedWorld> = std::collections::HashMap::new();
            let mut mission_steps: std::collections::HashMap<u16, Vec<d1_activity_host::state::MissionStep>> = std::collections::HashMap::new();
            let mut control_builder: Option<d1_activity_host::state::ControlBuilder> = None;
            let sensor_payload = match std::env::var("D1_AH_SENSOR_PHASE")
                .ok()
                .and_then(|s| s.trim().parse::<i8>().ok())
            {
                Some(phase) => {
                    // ⛔ Per-load and NOT derivable server-side (the server cannot read the
                    // client's globals table). The default is the measured in-world pvp_moonbase
                    // group-0 hash; a different map needs `D1_AH_SENSOR_NAME_HASH` set from
                    // `d1-replay sensor-name <snapshot>`. gen must equal the client's `world+0x1D`.
                    let name_hash = std::env::var("D1_AH_SENSOR_NAME_HASH")
                        .ok()
                        .and_then(&parse_u32)
                        .unwrap_or(0xC2DA_CAF7);
                    let name_sub = std::env::var("D1_AH_SENSOR_NAME_SUB")
                        .ok()
                        .and_then(|s| s.trim().parse::<i16>().ok())
                        .unwrap_or(3);
                    let name_i8 = std::env::var("D1_AH_SENSOR_NAME_I8")
                        .ok()
                        .and_then(&parse_u32)
                        .unwrap_or(0x10) as u8;
                    let gen = std::env::var("D1_AH_SENSOR_GEN")
                        .ok()
                        .and_then(&parse_u32)
                        .unwrap_or(0) as u8;
                    let tail = match (
                        std::env::var("D1_AH_SENSOR_TAIL_ID").ok().and_then(&parse_u32),
                        std::env::var("D1_AH_SENSOR_TAIL_HASH").ok().and_then(&parse_u32),
                    ) {
                        (Some(id), Some(h)) => Some((id, h)),
                        _ => None,
                    };
                    // ⭐ `D1_AH_SENSOR_CLIENTS=<file>`: the world's other pending sensor entity
                    // clients (`hash i8 sub has_a has_b` per line, read off the live client with
                    // `d1-winguest.py sensor-clients`). Without them the terminator 0x2D1600
                    // rejects the frame (build 56578: 96 of 97 pending), so the phase never lands.
                    let clients: Vec<d1_oracle::sensor::SensorClientRef> =
                        match std::env::var("D1_AH_SENSOR_CLIENTS") {
                            Ok(path) => match std::fs::read_to_string(&path) {
                                Ok(text) => text
                                    .lines()
                                    .filter_map(d1_oracle::sensor::SensorClientRef::parse_line)
                                    .filter(|c| {
                                        !(c.name_hash == name_hash
                                            && c.name_i8 == name_i8
                                            && c.name_sub == name_sub)
                                    })
                                    .collect(),
                                Err(e) => {
                                    tracing::error!("⛔ D1_AH_SENSOR_CLIENTS={path}: {e}");
                                    Vec::new()
                                }
                            },
                            Err(_) => Vec::new(),
                        };
                    let params = d1_oracle::sensor::SensorPhaseUpdate {
                        gen,
                        name_hash,
                        name_i8,
                        name_sub,
                        phase,
                        tail,
                        clients,
                        // ⭐ D1_AH_SENSOR_AUTHORITY=<machine>[,<seq>[,<applied>]]: every authority block to
                        // that machine (see `SensorAuthority`). Unset = no authority table.
                        authority: std::env::var("D1_AH_SENSOR_AUTHORITY").ok().and_then(|v| {
                            let mut it = v.split(',').map(|s| s.trim().parse::<u32>().ok());
                            let machine = it.next()??;
                            let seq = it.next().flatten().unwrap_or(1);
                            let applied = it.next().flatten().unwrap_or(1) != 0;
                            tracing::info!("⭐ D1_AH_SENSOR_AUTHORITY: all 65 authority blocks to machine {machine}, seq {seq}, applied {applied}");
                            Some(d1_oracle::sensor::SensorAuthority::all_to_with(machine as u8, seq as u16, applied))
                        }),
                    };
                    // `D1_AH_SENSOR_PHASE_FILE` + `D1_AH_SENSOR_PHASES=3,4,5`: one frame per listed
                    // phase, chosen per send from the file's content (state.rs).
                    if let (Ok(path), Ok(list)) = (
                        std::env::var("D1_AH_SENSOR_PHASE_FILE"),
                        std::env::var("D1_AH_SENSOR_PHASES"),
                    ) {
                        let mut frames = std::collections::BTreeMap::new();
                        for p in list.split(',').filter_map(|s| s.trim().parse::<i8>().ok()) {
                            let mut each = params.clone();
                            each.phase = p;
                            match d1_activity_host::codec::SchemaCodec::build_sensor(&each) {
                                Ok(b) => {
                                    frames.insert(p, b);
                                }
                                Err(e) => tracing::error!("⛔ sensor frame for phase {p}: {e}"),
                            }
                        }
                        tracing::info!(
                            "udt-5 sensor phase file {path}: frames for phases {:?}",
                            frames.keys().collect::<Vec<_>>()
                        );
                        sensor_phase_file = Some((std::path::PathBuf::from(path), frames));
                    }
                    // ⭐ D1_AH_TEAM0_ACTIVITIES=410,416: build the frame twice, with the type-12
                    // bundle's A+0x38 (the player's team) = 0 and = -1; the instance picks one per
                    // join from the launched activity (Private Match 0, story -1).
                    if let Ok(list) = std::env::var("D1_AH_TEAM0_ACTIVITIES") {
                        let set: std::collections::HashSet<u16> =
                            list.split(',').filter_map(|s| s.trim().parse().ok()).collect();
                        let with_team = |team: i32| {
                            let mut p = params.clone();
                            for c in p.clients.iter_mut() {
                                if let Some((0x8080_22A5, img)) = c.a_value.as_mut() {
                                    if img.len() >= 0x3C {
                                        img[0x38..0x3C].copy_from_slice(&team.to_le_bytes());
                                    }
                                }
                            }
                            d1_activity_host::codec::SchemaCodec::build_sensor(&p)
                        };
                        match (with_team(0), with_team(-1)) {
                            (Ok(a), Ok(b)) => {
                                tracing::info!("⭐ per-activity spawn team armed: team 0 for {set:?}, -1 otherwise");
                                sensor_team_frames = Some((a, b, set));
                            }
                            (Err(e), _) | (_, Err(e)) => tracing::error!("⛔ D1_AH_TEAM0_ACTIVITIES: {e}"),
                        }
                    }
                    // ⭐ D1_AH_WORLD_TABLE=<file>: `activity_index world sensor_group sensor_file` per
                    // line. Each world must be a known world (checksum, bubbles, chosen); its sensor
                    // frame is built here for both spawn teams, and the instance swaps world at join.
                    if let Ok(path) = std::env::var("D1_AH_WORLD_TABLE") {
                        let text = std::fs::read_to_string(&path)
                            .map_err(|e| anyhow::anyhow!("D1_AH_WORLD_TABLE {path}: {e}"))?;
                        for line in text.lines().map(|l| l.split('#').next().unwrap_or("").trim()).filter(|l| !l.is_empty()) {
                            let f: Vec<&str> = line.split_whitespace().collect();
                            if f.len() != 4 && f.len() != 5 {
                                anyhow::bail!("D1_AH_WORLD_TABLE line {line:?}: want `index world sensor_group sensor_file [start_region]`");
                            }
                            let start_region = match f.get(4) {
                                Some(r) => Some(parse_u32(r.to_string())
                                    .ok_or_else(|| anyhow::anyhow!("D1_AH_WORLD_TABLE: bad start region {r:?}"))?),
                                None => None,
                            };
                            let index: u16 = f[0].parse()?;
                            let Some(known) = d1_activity_host::worlds::lookup(f[1]) else {
                                anyhow::bail!("D1_AH_WORLD_TABLE: world {:?} has no captured scenario record", f[1]);
                            };
                            let group = parse_u32(f[2].to_string())
                                .ok_or_else(|| anyhow::anyhow!("D1_AH_WORLD_TABLE: bad sensor group {:?}", f[2]))?;
                            let clients: Vec<d1_oracle::sensor::SensorClientRef> = std::fs::read_to_string(f[3])
                                .map_err(|e| anyhow::anyhow!("D1_AH_WORLD_TABLE {}: {e}", f[3]))?
                                .lines()
                                .filter_map(d1_oracle::sensor::SensorClientRef::parse_line)
                                .filter(|c| !(c.name_hash == group && c.name_i8 == name_i8 && c.name_sub == name_sub))
                                .collect();
                            let build = |team: i32| {
                                let mut p = params.clone();
                                p.name_hash = group;
                                p.clients = clients.clone();
                                for c in p.clients.iter_mut() {
                                    if let Some((0x8080_22A5, img)) = c.a_value.as_mut() {
                                        if img.len() >= 0x3C {
                                            img[0x38..0x3C].copy_from_slice(&team.to_le_bytes());
                                        }
                                    }
                                }
                                d1_activity_host::codec::SchemaCodec::build_sensor(&p)
                            };
                            let (team0, team_minus1) = (build(0)?, build(-1)?);
                            tracing::info!(
                                "⭐ world table: index {index} -> {} ({} sensor clients, group {group:#010x})",
                                known.name, clients.len()
                            );
                            world_table.insert(index, d1_activity_host::state::ServedWorld {
                                name: known.name.to_string(),
                                checksum: known.checksum,
                                bubble_count: known.bubble_count,
                                chosen: known.chosen.to_vec(),
                                start_region,
                                team0,
                                team_minus1,
                            });
                        }
                    }
                    // ⭐ D1_AH_MISSION_STEPS=<file>: `activity_index when sensor_group sensor_file
                    // [region]` per line, `when` = N | +N | @T|N (see `StepWhen`). Each step's frame
                    // is prebuilt for both spawn teams.
                    if let Ok(path) = std::env::var("D1_AH_MISSION_STEPS") {
                        let text = std::fs::read_to_string(&path)
                            .map_err(|e| anyhow::anyhow!("D1_AH_MISSION_STEPS {path}: {e}"))?;
                        for line in text.lines().map(|l| l.split('#').next().unwrap_or("").trim()).filter(|l| !l.is_empty()) {
                            let f: Vec<&str> = line.split_whitespace().collect();
                            if !(4..=6).contains(&f.len()) {
                                anyhow::bail!("D1_AH_MISSION_STEPS line {line:?}: want `index seconds sensor_group sensor_file [region] [incident=TYPE:BODYHEX]`");
                            }
                            // ⭐ incident=TYPE:BODYHEX: a host incident sent when the step fires.
                            let mut incident = None;
                            let mut rest: Vec<&str> = Vec::new();
                            for t in &f[4..] {
                                if let Some(v) = t.strip_prefix("incident=") {
                                    let (ty, hex) = v.split_once(':')
                                        .ok_or_else(|| anyhow::anyhow!("D1_AH_MISSION_STEPS: bad incident {v:?} (want TYPE:BODYHEX)"))?;
                                    let body = (0..hex.len()).step_by(2)
                                        .map(|k| hex.get(k..k + 2).and_then(|b| u8::from_str_radix(b, 16).ok()))
                                        .collect::<Option<Vec<u8>>>()
                                        .ok_or_else(|| anyhow::anyhow!("D1_AH_MISSION_STEPS: bad incident body {hex:?}"))?;
                                    incident = Some((ty.parse::<u32>()?, body));
                                } else {
                                    rest.push(t);
                                }
                            }
                            let index: u16 = f[0].parse()?;
                            let when = d1_activity_host::state::StepWhen::parse(f[1])
                                .ok_or_else(|| anyhow::anyhow!("D1_AH_MISSION_STEPS: bad step time {:?} (want N, +N, @T|N, %S|N or ^G/K/I|N)", f[1]))?;
                            let group = parse_u32(f[2].to_string())
                                .ok_or_else(|| anyhow::anyhow!("D1_AH_MISSION_STEPS: bad sensor group {:?}", f[2]))?;
                            let region = match rest.first() {
                                Some(r) => Some(parse_u32(r.to_string())
                                    .ok_or_else(|| anyhow::anyhow!("D1_AH_MISSION_STEPS: bad region {r:?}"))?),
                                None => None,
                            };
                            let clients: Vec<d1_oracle::sensor::SensorClientRef> = std::fs::read_to_string(f[3])
                                .map_err(|e| anyhow::anyhow!("D1_AH_MISSION_STEPS {}: {e}", f[3]))?
                                .lines()
                                .filter_map(d1_oracle::sensor::SensorClientRef::parse_line)
                                .filter(|c| !(c.name_hash == group && c.name_i8 == name_i8 && c.name_sub == name_sub))
                                .collect();
                            let build = |team: i32| {
                                let mut p = params.clone();
                                p.name_hash = group;
                                p.clients = clients.clone();
                                for c in p.clients.iter_mut() {
                                    if let Some((0x8080_22A5, img)) = c.a_value.as_mut() {
                                        if img.len() >= 0x3C {
                                            img[0x38..0x3C].copy_from_slice(&team.to_le_bytes());
                                        }
                                    }
                                }
                                d1_activity_host::codec::SchemaCodec::build_sensor(&p)
                            };
                            let (team0, team_minus1) = (build(0)?, build(-1)?);
                            tracing::info!(
                                "⭐ mission step: index {index} {when:?} -> {} ({} sensor clients){}",
                                f[3],
                                clients.len(),
                                region.map(|r| format!(", region {r}")).unwrap_or_default()
                            );
                            mission_steps.entry(index).or_default().push(d1_activity_host::state::MissionStep {
                                when,
                                team0,
                                team_minus1,
                                region,
                                incident,
                            });
                        }
                    }
                    // `clients GROUP PATH` control lines (D1_AH_CONTROL_FILE) build a frame the way a
                    // mission step does.
                    {
                        let params = params.clone();
                        control_builder = Some(d1_activity_host::state::ControlBuilder(std::sync::Arc::new(
                            move |group: u32, path: &str, team: i32| -> Result<Vec<u8>, String> {
                                let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
                                let clients: Vec<d1_oracle::sensor::SensorClientRef> = text
                                    .lines()
                                    .filter_map(d1_oracle::sensor::SensorClientRef::parse_line)
                                    .filter(|c| !(c.name_hash == group && c.name_i8 == name_i8 && c.name_sub == name_sub))
                                    .collect();
                                let mut p = params.clone();
                                p.name_hash = group;
                                p.clients = clients;
                                // `@authority MACHINE[,SEQ[,APPLIED]]` in the list replaces
                                // D1_AH_SENSOR_AUTHORITY for this frame: the local machine changes
                                // with the slice set (3 in the Breach, 12 in the Divide).
                                if let Some(v) = text.lines().find_map(|l| l.trim().strip_prefix("@authority ")) {
                                    let mut it = v.split(',').map(|s| s.trim().parse::<u32>().ok());
                                    let machine = it.next().flatten().ok_or("@authority needs a machine")?;
                                    let seq = it.next().flatten().unwrap_or(1);
                                    let applied = it.next().flatten().unwrap_or(1) != 0;
                                    tracing::info!("⭐ control: all 65 authority blocks to machine {machine}, seq {seq}, applied {applied}");
                                    p.authority = Some(d1_oracle::sensor::SensorAuthority::all_to_with(machine as u8, seq as u16, applied));
                                }
                                for c in p.clients.iter_mut() {
                                    if let Some((0x8080_22A5, img)) = c.a_value.as_mut() {
                                        if img.len() >= 0x3C {
                                            img[0x38..0x3C].copy_from_slice(&team.to_le_bytes());
                                        }
                                    }
                                }
                                d1_activity_host::codec::SchemaCodec::build_sensor(&p).map_err(|e| e.to_string())
                            },
                        )));
                    }
                    match d1_activity_host::codec::SchemaCodec::build_sensor(&params) {
                        Ok(bytes) => {
                            tracing::info!(
                                "⭐⭐ D1_AH_SENSOR_PHASE={phase} - armed a udt-5 sensor_auth_update \
                                 ({} bytes, {} extra sensor clients) to write activity lifetime phase {phase} into \
                                 record+0x1AC (name {{hash {name_hash:#010x}, i8 {name_i8:#x}, sub \
                                 {name_sub}}}, gen {gen}). Sent ONCE after the instance reaches \
                                 Live. ⚠ synthetic-minimal: the post-pass dispatch is [U] until a \
                                 live watch on record+0x1AC confirms it, and gen MUST equal the \
                                 client's world+0x1D or the applier silently skips the frame.",
                                bytes.len(),
                                params.clients.len()
                            );
                            Some(bytes)
                        }
                        Err(e) => {
                            tracing::error!(
                                "⛔ D1_AH_SENSOR_PHASE={phase} set but the sensor frame could not \
                                 be built: {e}. Not arming - the run proceeds without it."
                            );
                            None
                        }
                    }
                }
                None => None,
            };

            // ⭐ `D1_AH_SENSOR_REPEAT_S`: re-send the armed sensor frame every N seconds instead of
            // once. The once-after-Live send lands during world load before the content record
            // exists and the applier drops it silently (task #4); repeating is how one copy
            // arrives after the world is resident. No effect unless the sensor is armed.
            let sensor_repeat_s: Option<u64> = std::env::var("D1_AH_SENSOR_REPEAT_S")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|s| *s > 0);
            if let Some(s) = sensor_repeat_s {
                tracing::info!("udt-5 sensor repeat armed: every {s} s while Live");
            }

            // ⭐ D1_AH_ACTIVITY_FOLLOW_FILE + D1_AH_ACTIVITY_TABLE: publish the activity the client
            // launched (the responder writes its hash from the kind-4 matchmaking key) instead of a
            // fixed index. The table is `index<TAB>hash` per row (`d1-activitytable.py`).
            let activity_follow: Option<(std::path::PathBuf, std::collections::HashMap<u32, u16>)> =
                match (
                    std::env::var("D1_AH_ACTIVITY_FOLLOW_FILE").ok().filter(|s| !s.is_empty()),
                    std::env::var("D1_AH_ACTIVITY_TABLE").ok().filter(|s| !s.is_empty()),
                ) {
                    (Some(file), Some(table)) => {
                        let text = std::fs::read_to_string(&table)
                            .map_err(|e| anyhow::anyhow!("D1_AH_ACTIVITY_TABLE {table}: {e}"))?;
                        let mut map = std::collections::HashMap::new();
                        for line in text.lines().filter(|l| !l.starts_with('#') && !l.starts_with("index")) {
                            let mut f = line.split('\t');
                            let (Some(i), Some(h)) = (f.next(), f.next()) else { continue };
                            let h = h.trim();
                            let hash = match h.strip_prefix("0x") {
                                Some(x) => u32::from_str_radix(x, 16)?,
                                None => h.parse::<u32>()?,
                            };
                            // First index wins: a hash listed twice keeps its lowest index.
                            map.entry(hash).or_insert(i.trim().parse::<u16>()?);
                        }
                        tracing::info!(
                            "⭐ activity follow armed: {} hashes from {table}, launched activity read from {file} on each join",
                            map.len()
                        );
                        Some((std::path::PathBuf::from(file), map))
                    }
                    (None, None) => None,
                    _ => anyhow::bail!("D1_AH_ACTIVITY_FOLLOW_FILE and D1_AH_ACTIVITY_TABLE go together"),
                };

            let session_id = format!("d1ah-{ah_id:016x}");
            let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
            rt.block_on(serve(
                AhBind::Listen(bind),
                move || {
                    let mut i = ActivityInstance::new(ah_id, session_id.clone(), Instant::now());
                    i.set_activity_name(activity.clone());
                    i.set_activity_index(activity_index);
                    if let Some((file, map)) = activity_follow.clone() {
                        i.set_activity_follow(file, map);
                    }
                    i.set_activity_checksum(activity_checksum);
                    i.set_bubble_count(bubble_count);
                    i.set_bubble_chosen(bubble_chosen.clone());
                    if let Some((r, p, t)) = pah_region {
                        i.set_pah_region(r, p, t);
                    }
                    if let Some(bytes) = sensor_payload.clone() {
                        i.set_sensor_payload(bytes);
                        if let Some((a, b, set)) = sensor_team_frames.clone() {
                            i.set_sensor_team_frames(a, b, set);
                        }
                        i.set_world_table(world_table.clone());
                        i.set_mission_steps(mission_steps.clone());
                        if let Ok(path) = std::env::var("D1_AH_CONTROL_FILE") {
                            i.set_control_file(path.into());
                            if let Some(b) = control_builder.clone() {
                                i.set_control_builder(b);
                            }
                        }
                        // ⭐ D1_AH_TIME_BASE_MS=<ms>|now: the world clock base at blob +0x520.
                        if let Ok(v) = std::env::var("D1_AH_TIME_BASE_MS") {
                            let ms = if v.trim() == "now" {
                                std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|d| d.as_millis() as u64)
                                    .unwrap_or(0)
                            } else {
                                v.trim().parse().unwrap_or(0)
                            };
                            tracing::info!("⭐ D1_AH_TIME_BASE_MS={v} - serving world clock base {ms} ms at blob +0x520");
                            i.set_time_base_ms(ms);
                        }
                        if let Some(s) = sensor_repeat_s {
                            i.set_sensor_repeat(std::time::Duration::from_secs(s));
                        }
                        if let Some((path, frames)) = sensor_phase_file.clone() {
                            i.set_sensor_phase_file(path, frames);
                        }
                    }
                    i.set_host_heartbeat(std::time::Duration::from_millis(heartbeat_ms));
                    i
                },
                codec,
            ))
        }
    }
}
