# Full-game coverage matrix (6 November 2026)

One row per player-facing system of Destiny 1 as it stood in Year 3 (Rise of Iron), against what
this project serves for PS4 build 56578. It is the reference for the integrated native test and
for the fixing phase after it. Every system D1 had is listed, built or not.

**Nothing below is working in-game.** About twenty build streams merged on 5–6 November (#304–#327).
Each was built and unit-tested on its own behind an off-by-default knob. None has run in the game,
the emulator or the lab since. The integrated native test ([full-game test](full-game-test.md),
[`full-game.env`](../ps4/profiles/full-game.env)) is the first time they run, and run together.
Where a system was seen in a native run before the build-out, the Tested column says
"baseline native" and names the run. That run used the older, step-driven servers, so the new
code path for that system has still not run.

**Pending merge:** #328 (live patrol, public events and Tower captures, `captures/patrol-tower`) and
stream O3 (`integrate/multiplayer-fixes`, in progress, not pushed). Rows that depend on them say so.
Fork PRs #7 (draw cost) and #8 (PC client) in
[d1reimagined-shadps4](https://github.com/p3n1cillin/d1reimagined-shadps4) are drafts, and the
deployed emulator (98542a7, fork #6) has neither.

## How to read it

| Column | Values |
|---|---|
| Built | **yes**: the server or emulator side exists for the whole system as D1 had it. **partial**: some of it. **no**: nothing yet. |
| Source of truth | Where the system's values come from. **verified**: the game's own data, checked (the owned 56578 executable or content, or Bungie's manifest and patch notes). **capture**: the private Xbox 360 live captures (build 51539; [notes](../notes/d1-live-captures-2026-11-05.md)). **community**: community collation or D1 knowledge (the Cryptarch guide, DIM). **placeholder**: a labelled stand-in. **n/a**: no game values involved. A row can name several. |
| Knob | What switches it on in `full-game.env`. "lab-base" means the base session serves it (`lab-base.env`). "–" means no knob. |
| PR(s) | Source-repository PRs (#NNN), fork PRs ("fork #N") or streams. "earlier" is work before #304. |
| Tested | **unit**: unit and offline tests only. **baseline native**: seen in a native run with the pre-build-out servers. **none**. |

## 1. Sign-on and characters

| System | Built | Source of truth | Knob | PR(s) | Tested | Known gaps and evidence needed |
|---|---|---|---|---|---|---|
| Platform sign-on (shadNet PSN, auth, content manifest, BAP sign-in) | yes | verified (the owned build's manifest and codecs) | lab-base | earlier | baseline native (every lab run) | Contained lab network only. |
| Character select and the served Guardian | yes | verified (family 3/4 objects); the Guardian is a lab fixture (Titan, level 40, Light 385) | lab-base; `D1_STORE=1` serves it from the store after the first sign-on | earlier, #307 | baseline native (fixtures); store path unit | The store's first serve matches the fixtures byte for byte in a unit test only. The character screen shows a black silhouette, and Intellect, Discipline and Strength read 0. |
| Character creation and deletion (501, 502, 504) | partial | verified: 501's layout, each class's preload lists; placeholder: the starting grant | `D1_SOCIAL=1`; `D1_STARTING_KIT` unset (empty) | #308, #309, #318, #322 | unit | No run has created a character. A preload list is not a proven server grant: a native create with the exported kit, or a capture of a creation, settles it. |
| Three characters per account | partial | verified (record schema) | `D1_STORE=1` | #307 | unit | One character is served. Account-owned reward counters are kept per character (integration review 26). |
| A new Guardian at level 1 in starter gear | partial | verified (level and item curves) | not in full-game (lab-base serves level 40) | earlier, #51 | baseline native (level1-run2, 3 Nov; [notes](../notes/ps4-56578-level1-2026-11-02.md)) | Not the default. Post-game pushes still carry Light 385. The full-game store seeds from the geared Guardian. |
| Appearance (501 identity, 506 change) | partial | verified (layout); the selectors' meanings are unknown | – | #308, #318 | unit (codec only) | 506 is not served. Palette, face and marking selectors are unknown. |

## 2. Client, emulator and PC

| System | Built | Source of truth | Knob | PR(s) | Tested | Known gaps and evidence needed |
|---|---|---|---|---|---|---|
| Emulator boot and runtime (fork `d1`) | yes | n/a | – | fork #6 (deployed 98542a7) | baseline native | Load on the host machine freezes the client (6–24 s gaps, the game's block detector; [lab stalls](../notes/ps4-56578-lab-stalls-2026-11-05.md)). Nothing heavy may run beside a test. |
| Fatal-path stability: exception handlers, clean exit, hitch halt | yes | verified (the game's handlers and its `ud2` halt, read from the executable) | `?D1_EXCEPTION_HANDLERS=1`, `?D1_CLEAN_EXIT=1`, `?D1_HITCH_HALT=0` | fork #8 (draft) | unit | Not in the deployed emulator, so the launcher drops these knobs. Needs the boot-log lines in [the full-game test](full-game-test.md#fork-8-the-pc-client). |
| Renderer fidelity | partial | n/a | – | earlier (colour-LUT cache fix) | baseline native | Characters and wider scene coverage; combined stability. |
| Per-draw CPU cost and frame rate | partial | n/a (the saving is an estimate) | `?D1_DRAW_FAST=1`, `?D1_DRAW_STATS=1`, `?D1_GPU_WAIT_YIELD=1` | fork #7 (draft), #112 | baseline native: 34–38 fps in heavy scenes; #7 unit | Estimated 5–15% of the command processor's time. Needs the A/B in [the full-game test](full-game-test.md#fork-7-the-draw-path). |
| Frame-rate targets and resolution (native Settings → Display) | yes | verified (the game's own settings screen) | lab-base | fork #6 | baseline native ([settings](../notes/ps4-56578-native-video-settings-2026-11-03.md), [rates](../notes/ps4-56578-frame-rate-2026-11-02.md)) | The added labels are English only. A no-jump ledge drop and other weapons' cadence are unchecked. |
| One-command full-game launcher and profiles | yes | n/a | the profile | #320, #323, #327 | unit (profile tests, dry run) | Never launched for a full run. |
| An installable release | no | – | – | – | none | Contributors supply their own build and inputs. |

## 3. Orbit and the Director

| System | Built | Source of truth | Knob | PR(s) | Tested | Known gaps and evidence needed |
|---|---|---|---|---|---|---|
| Orbit (load, Set Destination, announcements) | yes | verified | lab-base | earlier | baseline native (cine-live13 arrived in orbit with "Crucible Unlocked") | – |
| Director rules: launch, join and visibility (653 programs) | partial | verified (all 653 launch programs and 28 shared groups) | `D1_SOCIAL=1` | #309, #322 | unit | The server-side predictor reads every unlock value as zero. What the Director lists still follows the served unlock flags. |
| Activity → world binding | partial | verified (the client's own activity table names each activity's world: 477 generated); capture (57 cross-checked, all agreeing with the client's table) | `D1_AH_WORLDS`; `D1_AH_WORLDS_ACTIVITIES=all` | #314, stream Q | unit (`tests/all_worlds.rs`: every bound activity plans; 330 frames build natively in 0.2 s) | 534 of 653 activities have a world. The 119 without one name none: 82 playlists, 24 Private Match modes, 10 credits and cinematics, three rows naming 2014 bundles with no scenario. 235 starts are stand-ins (the first private slice set). Generated lists carry default values. None has been served in the lab; the hand rows (164, 165, 168, 0, 410, 416) win. [worlds note](../notes/ps4-56578-worlds-2026-11-05.md). |
| Launch requirements (fireteam size) | partial | verified (`minParty`/`maxParty`) | `D1_MP=1` | #312 | unit | – |
| Insertion points | partial | verified (`insertion-points.tsv`) | – | #314 | unit | The host sends no transition name, so the client always takes `default`. |
| Story progression: CONTINUE and the next activity | partial | verified (profile field [6]); 2014 table (`next=`) | `D1_STORE=1`, `D1_REWARDS=1` | earlier, #324, #327 | baseline native: CONTINUE 164 → 165 ([29 Oct](../notes/ps4-56578-continue-tower-2026-10-29.md)); the 165 → Tower hand-off unit | After 165, ours went to orbit, where retail went to the Tower. The hand-off is wired but unseen. Only 165 has a `next=`. |
| Ships and sparrows (vehicle bucket) | partial | verified | lab-base | earlier | baseline native (bucket equips) | Sparrow use in patrol has not been run. |

## 4. Story missions and cinematics

| System | Built | Source of truth | Knob | PR(s) | Tested | Known gaps and evidence needed |
|---|---|---|---|---|---|---|
| Missions driven by Destiny's own scripts (sidecar and bridge) | partial | restored 56578 scripts (Fukugen Daihon; a local input, unlicensed) on verified content | `D1_AH_SCRIPTS=1`, `D1_AH_SCRIPTS_SIDECAR`, `D1_AH_SCRIPTS_ACTIVITIES`, `D1_SCRIPTS_DIR`, `D1_BUNDLES_DIR` | #310, #319, #324, #327 | unit (a deterministic replay of 164 and 165) | The bridge's table lists every bound PvE activity (473: story, cinematic, quest, strike, raid, arena); 250 name a world no restored script starts, which the sidecar serves as a start-only stub on its content bundles (stream Q). Timer, hard-wipe and object-filter rows are refused. One bridge per connection, so a fireteam runs N copies (review 9, 10). BullShcript answers natives with 01.29 semantics. [Story missions](../notes/ps4-56578-story-missions-2026-11-05.md). |
| Missions driven by step schedules (the earlier route) | partial (164 and 165 only) | capture and memory reads | lab-base (`D1_AH_MISSION_STEPS`) | earlier | baseline native (sense116: all of A Guardian Rises; cine-live13: 165) | Perching Fallen, the Ghost's performances and dialogue fidelity; some cutscenes skipped. |
| Completion and post-game (965, lifetime 6 → 7/8, reward screens) | partial | verified (965 body decoded, #318); capture (lifetime order); placeholder (A Guardian Rises' 900 XP, the counters and session id) | `D1_REWARDS=1`, `D1_STORE=1` | #318, #324, #327 | baseline native (AGR post-game, [29 Oct](../notes/ps4-56578-postgame-2026-10-29.md)); scripted path unit | The scripted grant now runs the store's completion (review 6, fixed in #327), unseen. `D1_AH_SCRIPTS_MISSION_PHASE` stays off (meaning unknown). |
| Enemies and squads | partial | verified (squad states, cells) | lab-base; script bridge | earlier, #324 | baseline native (the Breach's waves) | Combat runs on the clients. Squad task and roster fields have no map. |
| Every mission and cinematic | – | see below | – | – | – | – |

The 78 story, cinematic and quest missions with a bound world (201 activities), from the generated
[`scripts/activities.txt`](../ps4/fixtures/56578/scripts/activities.txt) and stream K's offline
replay. "Phases" is phases reached of phases written in 600 s of synthetic play, which measures
the driver, not a player. Every row loads in the default full-game profile
(`D1_AH_WORLDS_ACTIVITIES=all`, stream Q). "none (start-only stub)": no restored script starts the
world, so the sidecar serves it on its content bundles alone; "restored (not replayed)": a script
exists but stream K's replay did not cover the world.

| Mission | Index | Kind | World | World evidence | Script | Phases, end (offline) | Native |
|---|---|---|---|---|---|---|---|
| A Guardian Rises | 164 | story | `act_cosmo_m0_highway` | cross-checked | written | 5/7, no end | baseline (step-driven) |
| A Guardian Rises (the reveal) | 165 | cinematic | `cine_reveal` | cross-checked | written | 2/2, ends | baseline (step-driven) |
| Restoration | 166, 606 | story | `act_cosmo_m0_5_hangar` | cross-checked | written | 1/3, no end | black screen (sense125) |
| The Speaker | 167 | cinematic | `cine_speaker` | cross-checked | stub | – | none |
| The Dark Within | 168, 169, 607 | story | `act_cosmo_m1_dome_a` | cross-checked | written | 1/3, no end | none |
| The Warmind | 170, 171, 608 | story | `act_cosmo_m2_dome_c` | cross-checked | written | 3/5, no end | none |
| The Last Array | 172, 173, 520, 578 | story | `act_cosmo_m4_dome_c` | cross-checked | written | 1/4, no end | none |
| The Dark Beyond | 174, 175, 609 | story | `act_moon_m1_launchtubes` | cross-checked | written | 2/9, no end | none |
| The World's Grave | 176, 177, 610 | story | `act_moon_m3_lowercitadel` | cross-checked | written | 2/5, no end | none |
| The Sword of Crota | 178, 179, 521, 579 | story | `act_moon_m4_uppercitadel` | cross-checked | stub | – | none |
| Chamber of Night | 180, 181, 522, 580 | story | `act_moon_m5_strength` | cross-checked | written | 7/8, no end | none |
| Shrine of Oryx | 182, 183, 523, 581 | story | `act_moon_m6_reactor` | cross-checked | stub | – | none |
| A Stranger's Call | 184, 185, 616 | story | `act_venus_m01_engineering` | cross-checked | written | 4/6, no end | none |
| The Awoken | 186 | cinematic | `act_cinematic_queen_101` | cross-checked | stub | – | none |
| Ishtar Collective | 187, 188, 617 | story | `act_venus_m02_southern_tides` | cross-checked | stub | – | none |
| The Archive | 189, 190, 524, 582 | story | `act_venus_m05_vault` | cross-checked | stub | – | none |
| Scourge of Winter | 191, 192, 525, 583 | story | `act_venus_m06_ketch` | cross-checked | stub | – | none |
| Eye of a Gate Lord | 193, 194, 526, 584 | story | `act_venus_m07_northern_tides` | cross-checked | written | 1/5, no end | none |
| A Key Awaits | 195 | cinematic | `act_cinematic_queen_102` | cross-checked | stub | – | none |
| Exclusion Zone | 196, 197, 618 | story | `act_mars_m01_valley` | cross-checked | written | 3/6, no end | none |
| The Buried City | 198, 199, 527, 585 | story | `act_mars_m03_penthouse` | cross-checked | stub | – | none |
| The Garden's Spire | 200, 201, 528, 586 | story | `act_mars_m05_warbase` | cross-checked | written | 3/9, no end | none |
| A Rising Tide | 202, 203, 529, 587 | story | `act_mars_m06_subway` | cross-checked | written | 2/8, no end | none |
| The Black Garden | 204, 205, 530, 588 | story | `act_mars_m07_lowergarden` | cross-checked | written | 4/10, no end | none |
| A Hero's Return | 206 | cinematic | `cine_end` | cross-checked | stub | – | none |
| Fist of Crota | 207, 208, 611 | story | `quest_bonzo_dlc01` | cross-checked | written | 2/2, no end | none |
| Siege of the Warmind | 209, 210, 531, 589 | story | `act_cosmo_m6_rasputin` | cross-checked | stub | – | none |
| The Wakening | 211, 212, 532, 590 | story | `act_moon_m7_selection` | cross-checked | stub | – | none |
| Ritual of Sacrifice | 213, 612 | quest | `quest_screw_dlc01` | generated | none (start-only stub) | – | none |
| A Kell Rising | 214, 215, 613 | story | `mission_pursuit` | cross-checked | written | 5/5, ends | none |
| The Ruling House | 216, 217, 533, 591 | story | `mission_pact` | cross-checked | written | 5/5, ends (one thread failure: a BullShcript hash collision) | none |
| Queen's Ransom | 218, 219, 534, 592 | story | `mission_traitor` | cross-checked | written | 2/6, no end | none |
| The Silent Fang | 220, 221, 614 | story | `quest_infidel` | cross-checked | written | 3/4, no end | none |
| The Archon-Slayer | 222 | quest | `quest_nightcrawler` | generated | none (start-only stub) | – | none |
| Wolves' Gambit | 223, 224, 615 | story | `quest_safari` | cross-checked | written | 3/5, no end | none |
| Tenebrous Tunnels | 225 | quest | `quest_apollo` | generated | none (start-only stub) | – | none |
| Shadows at Twilight | 226 | quest (solo) | `pvp_city_defense_carbon` | generated | none (start-only stub) | – | none |
| Entropy's Pinnacle | 227, 228, 535 | quest | `quest_catalyst` | generated | none (start-only stub) | – | none |
| Hangar Stakes | 229 | credits | `cine_hangar` | cross-checked | written | 1/1, ends | none |
| Lost to Light | 230, 238, 239, 537, 594 | cinematic | `mission_deja` | cross-checked | none | – | none |
| The Coming War | 234, 235, 236, 536, 593 | cinematic | `mission_darksnow` | cross-checked | written | 7/7, ends | none |
| Dread Patrol | 237 | quest | `quest_deadflower` | cross-checked | none | – | none |
| Summoner's Circle | 240 | story | `mission_destroyer` | generated | none (start-only stub) | – | none |
| Cayde's Stash | 241, 242, 538, 595 | story | `mission_elevator` | cross-checked | written | 4/5, no end | none |
| Shadow Call | 243 | quest (solo) | `quest_helium` | generated | none (start-only stub) | – | none |
| The Prime's Path | 244, 245, 539 | quest | `quest_hydrogen` | generated | none (start-only stub) | – | none |
| Knock Knock | 246 | quest | `quest_lithium` | generated | none (start-only stub) | – | none |
| A Ranger Found | 247 | quest (solo) | `mission_marathon_hunter` | generated | none (start-only stub) | – | none |
| Bound by Fire | 248 | quest (solo) | `mission_marathon_titan` | generated | none (start-only stub) | – | none |
| Bottle the Arc | 249 | quest (solo) | `mission_marathon_warlock` | generated | none (start-only stub) | – | none |
| Fear's Embrace | 250, 251, 252 | quest | `mission_microwave` | generated | none (start-only stub) | – | none |
| Blood in the Garden | 253 | quest | `quest_nitrogen` | generated | none (start-only stub) | – | none |
| Enemy of My Enemy | 254, 255, 540, 596 | story | `mission_omen` | cross-checked | none | – | none |
| A New Den | 256, 257, 541 | quest | `quest_oxygen` | generated | none (start-only stub) | – | none |
| A Spark in Shadow | 258 | quest (solo) | `quest_paper` | generated | none (start-only stub) | – | none |
| Paradox | 259, 260, 542 | story | `mission_paradox` | generated | none (start-only stub) | – | none |
| Blighted Coven | 261, 262, 543 | quest | `quest_peekaboo` | generated | none (start-only stub) | – | none |
| The First Firewall | 263 | quest | `quest_radium` | generated | none (start-only stub) | – | none |
| A March of Fire | 264 | quest (solo) | `quest_rock` | generated | none (start-only stub) | – | none |
| A Ranger Lost | 265 | quest (solo) | `quest_scissors` | generated | none (start-only stub) | – | none |
| Kings of Decay | 266, 267, 544 | quest | `quest_sightline` | generated | none (start-only stub) | – | none |
| Last Rites | 268, 269, 545, 597 | story | `mission_souvenir` | cross-checked | none | – | none |
| Outbound Signal | 270, 271, 272, 546 | story | `mission_toaster` | generated | none (start-only stub) | – | none |
| Regicide | 273, 274, 547, 598 | story | `mission_topknot` | cross-checked | none | – | none |
| The Dreadnaught | 275, 276, 548, 599 | story | `mission_touchdown` | cross-checked | written | 4/6, no end | none |
| The Promethean Code | 277, 278, 279, 549, 600 | story | `mission_turing` | cross-checked | none | – | none |
| Pretender to the Throne | 339 | quest | `mission_rook` | generated | none (start-only stub) | – | none |
| - | 382 | cinematic | `cinematic_peace` | generated | none (start-only stub) | – | none |
| - | 384 | cinematic | `cinematic_fall` | generated | none (start-only stub) | – | none |
| King of the Mountain | 387, 388, 601 | story | `mission_pug` | generated | none (start-only stub) | – | none |
| The Walls Come Down | 389, 390, 602 | story | `mission_beagle` | generated | none (start-only stub) | – | none |
| The Plaguelands | 391, 392, 603 | story | `mission_hound` | generated | none (start-only stub) | – | none |
| Download Complete | 393, 394, 604 | story | `mission_pitbull` | generated | none (start-only stub) | – | none |
| The Iron Tomb | 395, 396, 605 | story | `mission_mutt` | generated | none (start-only stub) | – | none |
| Bad Blood | 457 | quest | `mission_word` | generated | none (start-only stub) | – | none |
| A Khvostov Rising | 458 | quest | `mission_memory` | generated | none (start-only stub) | – | none |
| A Symbol of Honor | 459 | quest | `mission_research` | generated | none (start-only stub) | – | none |
| Beauty in Delivery | 460 | quest | `mission_fireworks` | generated | none (start-only stub) | – | none |

**No world (15 story-kind activities; built: no).** Their definitions in the client's own activity
table name no world: the five story playlists (Earth and Moon, Mars and Venus, Dark Champions, The
Taken War, The Iron Lords), Oryx Triumphant, King of the Mountain's cinematic (386), the Taken King
and Rise of Iron preludes, Destiny Credits, The Dawning, Age of Triumph (2), Destiny 2 Pre-Order and
one unnamed cinematic (233). They play a video or launch another activity.

## 5. Patrol and public events

| System | Built | Source of truth | Knob | PR(s) | Tested | Known gaps and evidence needed |
|---|---|---|---|---|---|---|
| Patrol zones (a mission host beside public instances) | partial | capture (Patrol Venus, the Dreadnaught and the Cosmodrome cross-checked); verified (the client's activity table binds the Moon, Mars and the Plaguelands) | `D1_AH_PUBLIC_HOST=1`, `D1_AH_PUBLIC_TABLE`, `D1_AH_WORLDS_ACTIVITIES=all` | #316, #323, #327, #328, stream Q | unit | Patrol Cosmodrome (8), a 2014 row whose `cosmo_patrols_1` has no scenario, stays unbound (93 is the Taken King's). The Plaguelands patrol (424) starts in a stand-in. |
| Public instances (region lists, joins, the bubble swap, the slot handover) | partial | capture (every region's groups and timings) | `D1_AH_PUBLIC_HOST=1`, `D1_AH_DISTINCT_HOST_IDS=1`, `D1_AH_PUBLIC_INSTANCES_FILE` | #316, #323, #327 | unit | The entry-only lists carry default values and have never been served. The PUBLIC checksum is the client's own index reading, not measured. The authority reset (udt 27–31, 42) and peer reservations (12, 13) are not served. The registry never enforces its cap and forgets no player on a TCP drop (review 23). [Public instances](../notes/ps4-56578-public-instances-2026-11-05.md). |
| Restoration's public start | partial | capture | `D1_AH_WORLDS_PRIVATE_START=1` serves a private stand-in | #314, #316 | baseline native: a black screen (sense125) | Try the public start with `-Set D1_AH_WORLDS_PRIVATE_START=`. |
| Patrol beacons | partial | capture: six live beacons by region, accept and `bounty_complete` incidents (#328, pending); placeholder: goals | `D1_AH_PUBLIC_HOST=1` | #316, #328 | unit | Beacon rewards past the Glimmer cap and their XP are unread. Accept incidents have no names. |
| Public events | partial | capture: first event 485–494 s after the instance starts, tier incident sets (#328, pending); placeholder: later spells, time limits, event activity hashes | `D1_AH_PUBLIC_HOST=1`, `D1_AH_SCRIPTS=1` | #316, #324, #328 | unit | An event starts only once a scripted run exists in the bubble, and the open world's bridge has no join, so it may never start (review 20). Event rewards would pay as activity completions (review 21). |
| Taken spawn cycles and champions | no | capture (177-s cycles on the bubble's host, #328) | – | #328 (note only) | none | Not built. |
| Chests and resource nodes | partial | placeholder (60 s per character) | `D1_AH_PUBLIC_HOST=1` | #316 | unit | Chest grants are refused (activity hash 0, review 21). |
| Loot pickups (601) | yes | verified | lab-base | earlier | baseline native (the starter rifle) | Open-world drop tables are not modelled. |
| Court of Oryx | no | – | – | – | none | No world, script or capture. |

## 6. Strikes, Nightfall and Heroic

| System | Built | Source of truth | Knob | PR(s) | Tested | Known gaps and evidence needed |
|---|---|---|---|---|---|---|
| Strike runs (encounters, checkpoints, darkness zones, wipes) | partial | restored scripts (5 strikes); capture (4); placeholder (8: one encounter each) | `D1_AH_ACTIVITY_FRAMEWORK` | #313, #327 | unit | O2 drives `ActivityRun` from the wire; never run. "A respawn after a wipe ends it" is a stand-in. [Strikes, raids, Crucible](../notes/ps4-56578-strikes-raids-crucible-2026-11-05.md). |
| Strike worlds | yes | verified (the client's activity table: 230 generated); capture (9 cross-checked) | `D1_AH_WORLDS` | #314, stream Q | unit | Every one of the 239 strike rows (heroic, Nightfall, weekly and playlist copies) has a world and loads in the default full-game profile. The client's table moved 60 rows off the 2014 worlds (the Taken King's `mars_portal_*_com01`, `venus_portal_2_redux`, `strike_oyster`, `strike_lobster`, `strike_dolphin_dlc03`). Starts for uncaptured strikes are stand-ins. |
| The 17 strikes | partial | scripts: The Devils' Lair, The Summoning Pits, The Nexus, The Undying Mind, The Shadow Thief. Capture: The Will of Crota, Winter's Run, Cerberus Vae III, Dust Palace. Neither: Fallen S.A.B.E.R., Shield Brothers, The Sunless Cell, Blighted Chalice, Echo Chamber, Sepiks Perfected, The Wretched Eye, The Abomination Heist | as above | #313 | unit | The eight with neither are one encounter, `needs-evidence`. Capture-derived checkpoints and bosses are guesses. |
| Strike playlists (Vanguard Legacy, Taken War, SIVA Crisis and Heroics) | partial | verified (manifest rows); members inferred and labelled | `D1_MP=1`, `D1_AH_ACTIVITY_FRAMEWORK` | #312, #313, #327 | unit | The year-one weekly rows name no strike. |
| Nightfall | partial | verified (the skull's own text: a wipe returns to orbit) | `D1_AH_ACTIVITY_FRAMEWORK` | #313 | unit | Which strike and skulls run each week was server-side: no rotation is built. |
| Heroic modifiers and skulls | partial | verified (skull definitions) | as above | #313 | unit | Noted on the run only; the clients simulate combat. |
| Strike rewards (Marks, playlist page, end-of-strike engrams) | partial | community (Cryptarch guide pages); placeholder (drop chances) | `D1_REWARDS=1` | #311, #313 | unit | The host never sends the reward page or the engram flag (review 24). |

## 7. Raids

| System | Built | Source of truth | Knob | PR(s) | Tested | Known gaps and evidence needed |
|---|---|---|---|---|---|---|
| Raid framework: weekly checkpoints, the leader's checkpoint, loot lockouts | yes | community (D1's rules) | `D1_AH_ACTIVITY_FRAMEWORK`, `D1_STORE_CONTROL` | #313, #323, #327 | unit | Checkpoints and lockouts reach the store and are read back at launch; never run. |
| Vault of Glass | partial | world generated (the client's table and the 2014 table agree, start 120); script a start-only stub; encounters community | as above | #313, #314 | none | No capture. |
| Crota's End | partial | capture (encounter sequence; world cross-checked); encounters community | as above | #313, #314 | none | Which segment is which encounter is our reading. |
| King's Fall | partial | world verified (the client's table: `raid_epiphany`); encounters community | as above | #313, stream Q | none | No script (a start-only stub on content) and no capture; the start is a stand-in (slice set 0). |
| Wrath of the Machine | partial | world verified (the client's table: `raid_promise`); encounters community | as above | #313, stream Q | none | No script (a start-only stub on content) and no capture; the start is a stand-in (slice set 0). |
| Raid loot | partial | community (Cryptarch loot tables, item hashes only) | `D1_REWARDS=1` | #311, #313 | unit | – |
| Difficulties and Age of Triumph challenges | no | – | – | – | none | Needs evidence. |

## 8. Prison of Elders, Challenge of the Elders and Archon's Forge

| System | Built | Source of truth | Knob | PR(s) | Tested | Known gaps and evidence needed |
|---|---|---|---|---|---|---|
| Prison of Elders (and Prison of Elders: Taken) | partial | world verified (the client's table: `arena_champion`, Taken `arena_goldfish`); community (five rounds, the last a boss) | `D1_AH_ACTIVITY_FRAMEWORK` | #313, stream Q | unit | Waves and bosses need evidence. The two arena playlists (332, 333, 334) name no world. |
| Challenge arenas (Skolas's Revenge and the others) | partial | world verified (the client's table: `arena_champion`, Challenge of the Elders `arena_angelfish`); community | as above | #313, stream Q | unit | As above. |
| Challenge of the Elders (scored rounds) | partial | placeholder (threshold 30,000) | as above | #313 | unit | Scoring needs evidence. |
| Archon's Forge | partial | community, `needs-evidence` (no manifest row of its own) | as above | #313 | unit | The Plaguelands patrol (424) now has its world (`plaguelands_patrol`, the client's table). Rise of Iron was never on the 360, so no capture. Tiers, timer and SIVA offerings are unknown. |

## 9. Crucible, Iron Banner and Trials

| System | Built | Source of truth | Knob | PR(s) | Tested | Known gaps and evidence needed |
|---|---|---|---|---|---|---|
| Match flow (lobby, countdown, rounds, sudden death, mercy, results) | partial | placeholder (every limit) | `D1_AH_ACTIVITY_FRAMEWORK` | #313, #326 | unit (every mode plays to an end) | No `CrucibleMatch` is driven from the wire. Combat is peer to peer on a physics host. |
| Modes (Control, Zone Control, Clash, Mayhem, Rumble, Skirmish, Doubles, Salvage, Rift, Supremacy, Rumble Supremacy, Elimination, Trials, Combined Arms, Sparrow Racing, Arsenal) | partial | verified: team and party sizes; placeholder: score limits, time limits, points, mercy, rounds | as above | #313, #326 | unit | The client reads only `score_to_win`; no matchmade playlist sets it in content, and `time_limit` is server-side ([limits](../notes/ps4-56578-strikes-raids-crucible-2026-11-05.md#needs-evidence)). A capture of a Crucible match settles it. |
| Scoreboard (row 15) | partial | verified: layout, 23 exported scoreboard programs; placeholder: which program a mode uses | `D1_AH_SCRIPTS_SCOREBOARD_COLUMNS` (unset) | #313, #322 | unit | Other players' rows are not encoded. |
| Maps (36) | partial | verified (map rows); worlds: 35 of 60 activities bound by the client's table; the 24 Private Match modes and one unnamed row name none | `D1_AH_WORLDS_ACTIVITIES=all` | #314, stream Q | baseline native: `pvp_city_defense` (Twilight Gap) loaded and spawned armed (citydef, [14 Oct](../notes/ps4-56578-blockers-2026-10-14.md)); `pvp_moonbase` is the lab's boot world | The rotation is a stand-in (every map). |
| Playlists and matchmaking (86) | partial | verified (manifest playlists); placeholder (`min_players`) | `D1_MP=1`, `D1_AH_ACTIVITY_FRAMEWORK` | #312, #313, #327 | unit | Teams are assigned peer-side. A match can form with one team empty (review 22). |
| Private matches | partial | lab hand rows (First Light) | lab-base | earlier | none | – |
| Iron Banner | partial | verified: level 40, level advantages, its progression | as above | #307, #313 | unit | The Iron Banner Tower (`city_tower_iron_banner`) has no world. |
| Trials of Osiris (cards, boons, the Lighthouse) | partial | community (nine wins, three losses, the boons); verified (passage progressions, fireteam of 3) | as above | #307, #313 | unit | The Lighthouse (40) has its world (`mercury_social`, public slice sets only, so it starts public). Trials could form from one fireteam (review 22). |
| Crucible rewards | partial | verified (playlist reward items); placeholder (drop chances) | `D1_REWARDS=1` | #311, #313 | unit | – |
| Skill matchmaking (BAP 34/35) | no | – | – | #308 (shape only) | none | Field meanings unknown. |

## 10. The Tower, the Reef and Felwinter Peak

| System | Built | Source of truth | Knob | PR(s) | Tested | Known gaps and evidence needed |
|---|---|---|---|---|---|---|
| Tower North | partial | memory read and capture (hand world table) | lab-base (`world-table-tower.txt`) | earlier, #328 (pending: the full 252-record list) | baseline native: populated, vendors open ([evening](../notes/ps4-56578-tower-evening-2026-10-29.md)) | The plaza and hangar regions. The generated table starts in slice set 24 (the plaza); the hand table uses 0 and wins. |
| Other Guardians in the Tower (shared social space) | partial | capture | `D1_AH_SHARED_INSTANCES=1`, `D1_AH_PUBLIC_HOST=1` | #312, #316 | unit | A join registers under the wrong host id (review 11, still open after #327). The cap is a placeholder (16); the live Tower showed at most 5 peers. |
| Tower set pieces (the Fan, the Jukebox) | no | capture (client incidents only, #328) | – | – | none | No services message is involved. |
| The Reef (Vestian Outpost) | partial | capture (world cross-checked, a reference list) | `D1_AH_WORLDS_ACTIVITIES=all` | #314, #316 | none | Never served. |
| Felwinter Peak (Iron Temple, 398) | partial | world verified (the client's table: `social_observatory`; the Iron Banner and Festival copies `social_observatory_iron_banner`, `fotl_observatory`) | `D1_AH_WORLDS_ACTIVITIES=all` | stream Q | none | Never served; the start is a stand-in. 383 is the `cinematic_efrideet` cinematic row. |
| Idle return to orbit | yes | verified (the client's own 15-minute limit) | lab-base (`D1_INACTIVITY_MS`) | earlier | baseline native | – |

## 11. Vendors

| System | Built | Source of truth | Knob | PR(s) | Tested | Known gaps and evidence needed |
|---|---|---|---|---|---|---|
| Buying (901) | yes | verified (sales, costs, required levels); capture confirms nine prices (#328) | lab-base; `D1_SOCIAL=1` | earlier, #309, #327 | baseline native (an emblem from Eva, [30 Oct](../notes/ps4-56578-vendor-purchase-2026-10-30.md)); social path unit | Three live prices differ (Outfitter sale 71, the Speaker's 106, Gunsmith 374). |
| Stock and resets | partial | verified (reset schedules, quantities); placeholder (which sales a refresh picks) | `D1_SOCIAL_STOCK=0` (off) | #307, #309 | unit | Not enforced: the client lists the whole catalogue and asks for no stock (#328). |
| Xûr | partial | verified (Friday reset); placeholder (the 48-hour stay) | `D1_SOCIAL=1` | #309 | baseline native (placed in Tower North) | His weekly stock history is not public. |
| Eververse refunds (905, 15 minutes) | partial | verified (67 of 123 sales; per sale since #327) | `D1_REFUND_POLICIES` | #307, #309, #327 | unit | 905 is decoded, not applied. The 901 stamp's unit (seconds) is assumed. No screen has sent one. |
| Silver purchases (Eververse) | no | – | – | – | none | Real-money currency; no store. |
| Selling (903) | yes (D1 refused it) | verified (`Unsellable`) | `D1_SOCIAL=1` | #309 | unit | Never seen on the wire. |
| Buyback (902) | partial | placeholder (read as "take back") | `D1_SOCIAL=1` | #309 | unit | No static sender of 902 in 56578: probably unused. |
| Cryptarch decryption | partial | verified (a "Decode Engram" sale); community and placeholder (the outcome pool) | `D1_SOCIAL=1`, `D1_REWARDS=1` | #309, #311 | unit | #328's seven live decrypts give the push's shape. |
| Bounties (pick-up, turn-in) | partial | verified (objectives, rewards); capture: turn-in is 402 action 1 with 10 item XP updates (#328) | `D1_SOCIAL=1` | #309, #328 | unit | The 402 turn-in is not wired yet. |
| Faction and rank-up packages | partial | capture: 904 at the vendor, contents into the inventory (#328) | `D1_SOCIAL=1` | #309 | unit | 904 is not served. The service routes packages to Lost Items, which the capture contradicts. |
| Reputation and ranks | partial | verified (curves and runtime indices) | `D1_STORE=1` | #307, #326 | unit | Whether the client shows the rank from the progress field alone. Daily and weekly fields are never reset. |
| Gunsmith (Armsday orders, weapon tests) | partial | verified (5 of 25 orders, 2,500 Glimmer); placeholder (one order a week) | `D1_SOCIAL=1` | #309 | unit | The weapon test is a 904, not served. |
| Speaker's exchange, faction donations, Shipwright, Outfitter, kiosks | partial | verified (sale costs match the capture); capture (kiosks are free 901 sales) | `D1_SOCIAL=1` | #309 | unit | The Shipwright's freebie is a 904. |
| Postmaster | partial | verified (vendor rows 20 and 36); placeholder (Lost Items is bucket 0x10) | `D1_SOCIAL=1` | #309 | unit | The claim path (601 or 405) is unknown. |

## 12. Inventory and the vault

| System | Built | Source of truth | Knob | PR(s) | Tested | Known gaps and evidence needed |
|---|---|---|---|---|---|---|
| Equip (403) | yes | verified (exotic and class rules) | lab-base; `D1_SOCIAL=1` | earlier, #309 | baseline native ([1 Nov](../notes/ps4-56578-equip-2026-11-01.md)) | – |
| Unequip (404) | partial | placeholder (the next item takes the slot) | `D1_SOCIAL=1` | #309 | unit | No run has sent one. |
| Transfer and the vault (405) | partial | verified (vault row 26, buckets and sizes); capture (405's bytes, #328) | `D1_SOCIAL=1` | #309, #323 | unit | The direction is inferred from where the item is. Never sent natively. |
| Dismantle (402) | partial | capture: per-tier yields (#328, pending); placeholder: basic and exotic | `D1_DISMANTLE_PLACEHOLDER=1` | earlier, #309, #328 | baseline native: the item is removed (sense125); yields unit | Three yield tables disagree (review 27). |
| Item lock (406) | no | verified (bit 0 is the lock) | – | #318 | none | Not served. |
| Buckets beyond weapons, armour, subclass, vehicle and Ghost | partial | verified (39 buckets, 29 named) | lab-base | earlier, #309 | baseline native (partial) | Emotes, emblems, shaders, horns, artifacts and consumables are not exercised. |
| New item instances (element writer) | partial | verified (offsets); placeholder (node states, level for a Light) | `D1_REWARDS=1` | #311, #327 | unit | Level, grid, node states and stat positions are the template's. |

## 13. Loot, engrams and Light

| System | Built | Source of truth | Knob | PR(s) | Tested | Known gaps and evidence needed |
|---|---|---|---|---|---|---|
| Completion rewards (the activity's reward list) | yes | verified (manifest) | `D1_REWARDS=1`, `D1_STORE=1` | #311, #323, #327 | unit | Granted from scripted runs and `ActivityRun` only. Every item has a content index since #327. |
| End-of-activity engram drops | partial | placeholder (Rare 400‰, Legendary 100‰, Exotic 10‰) | `D1_REWARDS=1`, `D1_REWARDS_SEED` | #311 | unit | The fixed seed rolls the same engrams every completion (review 29). |
| Engram decryption | partial | verified (bucket, Uncommon → materials at 40); community (pools, class, minimum tier); placeholder (upgrade chance, fallback pool) | `D1_REWARDS=1` | #311 | unit | – |
| Drop Light | partial | verified (caps 400 and 385, at-or-above rule, Rare 365, Exotic 400); community (Legendary 390, the basis); placeholder (spread, slow zone) | `D1_REWARDS=1` | #311 | unit | – |
| Character Light | partial | community (DIM's weights) | – | #311 | unit | Read the client's own sum (`0x265EF0`). The lab Guardian is Light 385 throughout. |
| Infusion (804) | partial | verified (the client's formula and rates); placeholder (no cost) | – | #311, #318, #322 | unit | 804 is not served, and no reply or push was seen. |
| Personal loot per member | yes | community | `D1_REWARDS=1` | #311 | unit | With two players, grants pay player 1 (review 4; O3 in progress). |
| Post-game reward screens | yes | verified | lab-base | earlier | baseline native | – |

## 14. Talent grids and subclasses

| System | Built | Source of truth | Knob | PR(s) | Tested | Known gaps and evidence needed |
|---|---|---|---|---|---|---|
| Subclass abilities | partial | verified (subclass grid ability rows and perks) | lab-base | earlier (#134) | baseline native (the Striker's grenade, melee and super) | The other eight subclasses have not been seen. |
| Choosing nodes (801, 802) | partial | verified (grid rules); capture (one node: 500 Glimmer, 10 Weapon Parts, 10 materials; #328) | `D1_REWARDS=1` | #311 | unit | 801 and 802 are not served. The node cost placeholder (free) contradicts the capture. |
| Item and subclass XP, grid levels | partial | verified (step costs); placeholder (XP share) | `D1_REWARDS=1` | #311 | unit | The node-state bytes are not decoded. |

## 15. Grimoire, Record Books, quests and bounties

| System | Built | Source of truth | Knob | PR(s) | Tested | Known gaps and evidence needed |
|---|---|---|---|---|---|---|
| Grimoire (cards, score 5,610) | partial | verified (cards, points, ranks); placeholder (what unlocks a card) | `D1_REWARDS=1` | #311, #327 | unit | No wire structure is decoded and the client has no request for it. |
| Record Books (1801, 1802) | partial | verified (books, records, rewards); placeholder (1,000 points a record) | `D1_REWARDS=1` | #311, #318 | unit | 1801 and 1802 are decoded, not served. |
| Quests | partial | verified (steps and questline order); community (advancing) | `D1_REWARDS=1`, `D1_SOCIAL=1` | #309, #311 | unit | Quest objectives' wire state is unknown. |
| Bounties | partial | see §11 | `D1_SOCIAL=1` | #309 | unit | See §11. |

## 16. Currencies

| System | Built | Source of truth | Knob | PR(s) | Tested | Known gaps and evidence needed |
|---|---|---|---|---|---|---|
| Glimmer | yes | verified (cap 25,000, account slot 308); capture (the cap holds) | lab-base; `D1_REWARDS=1` | earlier, #311, #327 | baseline native (shown and spent) | – |
| Legendary Marks | partial | verified (cap 200, slot 310); placeholder (100 a week) | `D1_REWARDS=1` | #311, #327 | unit | Reach the client only since #327. |
| Vanguard and Crucible Marks (profile slots 278, 279) | partial | verified | lab-base | earlier | baseline native (in the inventory header) | Purchases priced in marks are untested. |
| Silver Dust | partial | verified (cap 250, dismantle 1 or 5) | `D1_REWARDS=1` | #311 | unit | – |
| Materials (planetary, Weapon Parts, Armor Materials, Motes of Light, Strange Coins, Ascendant) | partial | verified (caps 200); capture (amounts, #328); community (per-character scope) | `D1_REWARDS=1` | #311, #327 | unit | – |
| Silver | no | – | – | – | none | See §11. |

## 17. Fireteams, matchmaking, multiple players and P2P

| System | Built | Source of truth | Knob | PR(s) | Tested | Known gaps and evidence needed |
|---|---|---|---|---|---|---|
| Per-player identity (BAP 23) | partial | placeholder (the own-id rule is a guess) | `D1_MP=1` | #312, #327, O3 (in progress) | unit | Every connection gets the same SOIDs until O3 lands (review 4). |
| Several players on one responder | partial | – | `D1_MP=1` | O3 (in progress) | none | One player per store process; QueueZ goes only to the primary BAP connection. |
| Fireteams (invite, join, leave, kick, leader) on the PSN Session WebAPI | partial | verified (party sizes); our design | `D1_MP=1` | #312 | unit (fake clients) | Never with two clients. The party invite blob is unknown. |
| Matchmaking directory (42/43) and relaxation | partial | capture (51539 shapes and timings); placeholder (search scores; the 56578 fields in `f7`/`f8` are inferred) | `D1_MP=1`, `D1_MM_CONFIG_FILE` | #312, #327 | unit | The 43 answer lists the clients' own advertisements. Fullest-first against emptiest-first (review 8). |
| Public placement across processes | partial | Bungie's stated design (emptiest first) | `D1_AH_PUBLIC_INSTANCES_FILE` | #323, #327 | unit | The live rule is not in the captures. |
| Join in progress | partial | capture (a strike advertised open slots) | `D1_MP=1` | #312 | unit | – |
| Shared activity instances (membership, physics-host election, entity indices) | partial | capture (500 indices a slot) | `D1_AH_SHARED_INSTANCES=1` | #312 | unit | The per-peer token sits at the configured index. No reconcile on a host migration. Scripts run per connection (review 9). |
| P2P game sessions between consoles | partial | – | – | #312 | none | Two emulators have never shared a session. |
| Friends and presence (BAP 36/37) | partial | verified (shapes only) | – | #308 | none | Relay only; the 360 captures carry none. |
| Voice and text chat | no | – | – | – | none | Platform functions. |
| Reporting a player (1501) | no | – | – | – | none | – |

## 18. Saving

| System | Built | Source of truth | Knob | PR(s) | Tested | Known gaps and evidence needed |
|---|---|---|---|---|---|---|
| Player store (account, characters, inventory, currencies, completions, flags) | yes | Bungie's GDC 2021 design (save on important events, else every 5 minutes) | `D1_STORE=1`, `D1_STORE_DIR` | #307, #326, #327 | unit (crash safety included) | Never across two native sign-ons. One process owns the store since #327 (review 2). A refused social op loses the side store (review 12). |
| Reward overlay (instances, grids, Grimoire, objectives, counters) | yes | – | `D1_REWARDS=1`, `D1_STORE=1` | #327 | unit | – |
| Raid checkpoints and loot lockouts | yes | community | `D1_STORE_CONTROL` | #313, #323 | unit | – |
| Vault instances, refund records, vendor resets | yes | verified | `D1_STORE=1` | #307, #323 | unit | – |
| The command file between the servers | yes | – | `D1_STORE_CONTROL` | #323 | unit | A late writer's line can still be lost (review 30). |

## 19. Network codec coverage

| System | Built | Source of truth | Knob | PR(s) | Tested | Known gaps and evidence needed |
|---|---|---|---|---|---|---|
| BAP envelopes and sign-on messages | yes | verified | lab-base | earlier | baseline native | – |
| Investment services served (901, 402, 403, 601 and the sign-on set) | yes | verified | lab-base | earlier | baseline native | 702's purpose is open. |
| Investment services decoded, not seen on the wire (903, 902, 404, 405, 905) | partial | verified (layouts) | `D1_SOCIAL=1` | #308, #309, #318 | unit | 405's bytes from #328's capture. |
| Investment services not served (801, 802, 804, 904, 406, 1801–1803, 1501) | no | verified (layouts) | – | #308, #318 | none | 904 and 801 are in #328's capture. |
| QueueZ pushes (BAP 123) | partial | verified (families 2–4) | lab-base | earlier | baseline native | Family 6 (fireteam) unknown. |
| Activity-host udts (33 schema-coded) | yes | verified (byte-identical to the client's codec, 1,089 images) | `D1_RSAT_NATIVE=1` | #317, #321 | unit | Raw udts 6, 8 and 15 are passed verbatim. No comparison against captures and no native run. |
| udt 5 sensor lists (10 value schemas) | partial | verified (66 of 66 fixture lists native) | `D1_RSAT_NATIVE=1` | #317, #321 | unit | No production delta encoder. World-bounds position compression is not native. The sensor cache key ignores the codec (review 18). |
| udt 5 stamp, init and heartbeats | yes | verified (static analysis) | `D1_AH_SENSOR_STAMP=1` | #298 | unit | No stamp-age timeout found, which is not proof of none ([note](../notes/ps4-56578-udt5-stamp-init-2026-11-05.md)). |
| Script row kinds (sequence, actor, machine, trigger group) | partial | verified (56578 layouts, round-tripped) | `D1_AH_SCRIPTS=1` | #324 | unit | Timer, hard-wipe globals and object filter are refused. |
| Incidents (udt 18) | yes | verified (the full catalogue and the 51539 mapping) | `D1_AH_SCRIPTS_INCIDENTS` | #306 | unit | The host reads at most 8 of a 25-incident envelope (review 31). |
| Lifetime and completion 965 | partial | verified (layouts); capture (order) | – | #318 | unit | Lifetime values 2, 5 and 9, the seven counters and the session id are unknown. |
| Public-event objective deltas | no | – | – | #318 | none | Not decoded. |
| Slot handover (udt 23–26) | yes | capture | on for public instances | earlier, #316 | unit | – |
| Authority reset (27–31, 42), peer reservations (12, 13) | no | capture | – | – | none | – |
| The responder's codec | partial | verified (the client's own codec) | – | earlier | baseline native | The responder still runs `d1-oracle` (Unicorn). |

## 20. Audio, video and input

| System | Built | Source of truth | Knob | PR(s) | Tested | Known gaps and evidence needed |
|---|---|---|---|---|---|---|
| Audio output pacing | partial | n/a | `?D1_AUDIO_SYNC=1` | fork #8 (draft) | baseline native (upstream pacing); #8 unit | Upstream allows up to 341 ms of lag. Needs the long-run check in fork #8's list. |
| In-engine cinematics | yes | verified (no Bink or AvPlayer) | – | earlier | baseline native (cine-live13) | 165's second cinematic showed a loading screen until the arrival (cine-live15). Dialogue sync is unchecked. |
| Pad input | yes | n/a | – | earlier | baseline native | – |
| PC keyboard and mouse | partial | n/a | `?D1_MOUSE_CURSOR`, `?D1_MOUSE_BUTTONS`, `?D1_MOUSE_LOOK` (deployed); `?D1_PC_LAYOUT=1` (fork #8) | #113, fork #8 | baseline native (cursor, buttons, look); the layout unit | No mouse aim assist (as Destiny 2 on PC). Raw look replaces cursor re-centring in #8. |
| Pad hot-plug and keyboard fallback | partial | n/a | `?D1_PAD_KEEP_KEYBOARD=1` | fork #8 | unit | – |
| Languages other than English (the added settings labels) | partial | n/a | – | fork #6 | none | – |

## Summary counts

Counted over the 146 system rows above, not the per-mission table. A row can name several
sources, so the source counts overlap.

| | Rows |
|---|---:|
| Built: yes | 30 |
| Built: partial | 101 |
| Built: no | 15 |
| Source names verified game data | 77 |
| Source names a live capture | 32 |
| Source names community data | 19 |
| Source names a labelled placeholder | 32 |
| Tested: a baseline native run (before the build-out; some of them failures) | 35 |
| Tested: unit only | 86 |
| Tested: none | 25 |
| Tested natively since the build-out | **0** |

Missions: 78 story, cinematic and quest missions (201 activities) have a world, and 15 story-kind
activities name none. Of 653 activities, 534 have a world (stream Q). Rewards rules alone ([rewards note](../notes/ps4-56578-rewards-and-growth-2026-11-05.md)):
20 verified, 14 community, 19 placeholder.

## What the integrated native test must check first

In order. Each step needs the ones before it. Compare each against `-Profile baseline` on the same
emulator build.

1. **The launch.** `d1-full-game.ps1 -DryRun`, then a run: every `[server]` knob reads SET in the
   responder's table, and `manifest.json` lists only the expected `dropped` knobs. Only one
   responder opens the store.
2. **Sign-on from the store.** The store's first serve reaches character select and orbit with the
   same Guardian as baseline.
3. **The native codec and the stamp.** Under `D1_RSAT_NATIVE=1` and `D1_AH_SENSOR_STAMP=1`, the boot
   world and Tower North load and stay connected through an idle stretch, with no native codec
   error. Compare slow-turn warnings against baseline.
4. **Tower North as a social space** with the public host on: it loads populated, vendors open,
   and the join registers one shared instance.
5. **Vendor and inventory ops through the social path and the store:** a purchase (901), an equip
   (403), a dismantle (402) and the first native vault transfer (405). Each is pushed back and shows
   in the client.
6. **Persistence.** Sign off, start a second run with the same store: everything from step 5 is
   still there, and `account-<soid>.json`'s `history` names it.
7. **The reveal, script-driven.** `D1_AH_ACTIVITY_REMAP=164:165`: the bridge sends lifetime
   0 → 1 → 4 → 6 → 8 and both cinematics play. The grant and the completion reach the store once
   (sub 41, no double XP).
8. **The hand-off.** After 165 the client launches the Tower, not orbit.
9. **Rewards into the wallet.** A grant changes Glimmer and Legendary Marks (slot 310) in the
   client. Two vendor ops inside one push round trip lose no XP (review 3).
10. **A generated world.** Restoration (166) from the Director, with the private start: a world
    loads, not the black screen of sense125. Then The Dark Within (168) and The Speaker (167); the
    sidecar answers every tick.
11. **A Guardian Rises, script-driven, end to end:** 164 to its post-game, the CONTINUE gate (subs
    40/673) set by the scripted completion.
12. **A public instance.** Restoration's public start (`-Set D1_AH_WORLDS_PRIVATE_START=`): the
    region-56 instance's entry-only list is served and its slot freed on leave.
13. **The emulator PRs.** Deploy an integration build of fork #7 and #8 and run their A/B: the boot
    log lines, the PC layout, audio sync and the draw-stats medians.
14. **A strike** through the framework (add one to `D1_AH_WORLDS_ACTIVITIES`, for example The
    Devils' Lair): encounters open, a checkpoint is saved, Nightfall's wipe returns to orbit.
15. **Two clients** (after O3): two identities, one fireteam, one shared instance, grants to the
    right player.
16. **A Crucible playlist or private match** on a generated map: the match state and the
    scoreboard row.

## Placeholders that need new evidence

| Placeholder | Where | What would settle it |
|---|---|---|
| Crucible score limits, time limits, points, mercy and round counts | `activities/crucible.json` | A capture of a matchmade Crucible match of each mode, or a whole-package scan for what references the 46 preset option tags |
| Which scoreboard program each mode uses | `crucible.json`, `scoreboard-definitions.json` | The same capture (its row 15), or the content that references the 23 definitions |
| `min_players` and the map rotation | `encounter_framework.rs` | A capture of a playlist's matchmaking |
| Encounters of the eight strikes with neither script nor capture | `definitions/strikes.json` | Captures of those strikes, or Codie's scripts for them |
| Checkpoints and darkness zones of capture-derived strikes | `definitions/strikes.json` | A capture with a wipe, or the scripts |
| Raid encounter lists, difficulties and Age of Triumph challenges | `definitions/raids.json` | Raid captures (none for King's Fall or Wrath of the Machine on the 360), or the raids' scripts |
| Prison of Elders and Challenge of the Elders waves, scoring, threshold 30,000 | `definitions/arenas.json` | Arena captures, or the arena scripts |
| Archon's Forge tiers and timer | `definitions/arenas.json` | A PS4 capture of Rise of Iron, or its scripts |
| Weekly Heroic and Nightfall strike and skull rotation | `catalog.json` | A history of the weekly rotation (community archives), checked against a capture |
| "A respawn after a wipe ends it" | `activity_driver.rs` | An eboot read of the hard-wipe handshake, or a capture of a wipe |
| Engram drop chances, the Light spread and slow zone, decrypt upgrades, the fallback pool | `rewards/placeholders.rs` | Counts from recordings or PS4 captures of Year 3 play (the 360 captures are Year 1) |
| Legendary Marks weekly cap (100) | `rewards/placeholders.rs` | Rise of Iron patch notes or a capture that reaches the cap |
| Dismantle yields: basic and exotic, and Marks for armour (2) | `inventory_ops.rs`, `rewards/placeholders.rs` | #328 settles uncommon, rare and legendary (3 Marks for armour too); a capture dismantling basic and exotic gear settles the rest |
| Uncommon engram → material amount | `rewards/placeholders.rs` | A capture of a level-40 pickup |
| Talent node costs and random rolls | `rewards/placeholders.rs` | #328 shows one 801's cost; more 801s across item tiers in a capture |
| Item XP share | `rewards/placeholders.rs` | #328's 10 item updates on a turn-in fit "every equipped grid item"; a decoded 56578 item object would confirm it |
| Node-state bytes, level for a Light, stat rolls | `item_instance.rs` | Serve an instance with one node toggled and read it back; read the level curve (`d1-oracle character derived-stat`) |
| Character Light weights | `rewards` (community) | An eboot read of `0x265EF0` against a known loadout |
| Infusion cost | `rewards/placeholders.rs` | A capture of an 804, and its push |
| Grimoire card triggers, Record Book points | `rewards` | A capture of an account with Grimoire and Record Book progress, or the eboot's readers |
| A Guardian Rises' 900 XP and the other stand-in completion rows | `progression/activity-rewards.tsv` | A capture of each mission's completion push (XP is in the profile's later fields, which need a 51539 executable's descriptors to read) |
| The 901 stamp's unit | `player_store.rs` | #328 shows it rising about one a second: a run that buys twice, timed |
| Which sales a stock refresh picks; one Armsday order a week; Xûr's 48 hours | `vendors/rotation.rs` | A history of vendor stock (community archives); the "Max Orders Placed" number from content |
| 902 as "take back" | `social_hooks.rs` | Nothing sends it in 56578: drop it unless a sender is found |
| Lost Items bucket (0x10 or 0x14), the Postmaster claim path | `inventory_ops.rs` | A capture or native run of a Postmaster claim |
| The unequip replacement; the transfer direction | `inventory_ops.rs` | A native run of each from the character screen |
| Character creation grant | `starting-kit.tsv` | A native create, or a capture of one |
| Public-event later spells and time limits; event activity hashes | `public_events.rs` | A capture that stays through two events in one instance; content for the hashes |
| Patrol beacon goals | `open_world.rs` | The beacons' content rows, or the incidents' names |
| Chest and node respawn (60 s); empty-instance retire (300 s) | `open_world.rs`, `public_registry.rs` | A capture that revisits a chest |
| Social-space cap (16) | `open_world.rs` | A capture in a busy Tower, or the names of the matchmaking configuration's numbers |
| Matchmaking search scores; the `f7`/`f8` field numbers; BAP 23's own-id rule | `d1-mp` | A PS4 capture of matchmaking, or an eboot read of the search encoder |
| The completion body's counters and session id; the mission-phase tuple | `completion.rs`, `script_bridge.rs` | An eboot read of the counter names, or a PS4 capture |
| Generated list values and five stand-in starts | `worlds/` | Memory reads or captures of each world |
| Light 385 on the lab Guardian and its post-game pushes | `lab-base.env`, fixtures | Serve the level-1 Guardian by default once the route allows it |
