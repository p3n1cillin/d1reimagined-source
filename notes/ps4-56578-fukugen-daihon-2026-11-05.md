# PS4 56578: Fukugen Daihon and the reveal sequence (5 November 2026)

Activity 165's reconstructed script separates **activity completion**, **cinematic completion**,
**mission-phase selection**, and **lifetime shutdown**. Our schedule previously represented only
cinematic rows, a region move, and shutdown. The updated schedule waits for the second slice before
the four-second delay, and identifies the missing completion operation explicitly. **It is still
an incomplete host approximation, not a retail implementation.**

This investigation read local sources and owned 56578 content offline. No game, lab, input, capture,
or watchpoint session was started, and no new in-game result is claimed. Earlier runs are described
in [cine-live13/15](ps4-56578-cine-live13-2026-11-05.md); that note does not establish their emulator
commit. A future native result must name both build 56578 and the emulator commit.

## Sources and permissions

Credit: Codie and [Blam Network's Tiger project](https://github.com/Blam-Network/tiger).
Paths below use `<tiger>` for the local Tiger checkout and `<harness>` for the separate local
`Blam-Network_tiger_test_harness` checkout. `<scripts>` means
`<tiger>/packages/fukugen-daihon/destiny1/v56578_17_04_12_1251_evil`.

The maintainer's 5 November decision permits local reference use of all this material. BullShcript,
Gibbon, RSAT, Datamine and Poodle are MIT; adaptations must retain their notices and credit. The
harness is UNLICENSED, and sources without a licence remain local read/run references. This note
describes their behavior in our own words; no harness code or script text is reproduced. The alpha
scripts are leaked-build decompilations and must not be quoted. The v56578 scripts are Codie's
reconstruction for our target build, not recovered Bungie source. Their comments also identify
retail 2.3.1 captures as reconstruction evidence: that is not a native PS4 56578 execution result.

Principal references:

- `<scripts>/es/cosmo/cine_reveal/cine_reveal.es` and its two `.esp` mission files.
- `<scripts>/es/utility_scripts/cinematic_midmission.esu` and
  `<scripts>/bs/activities/common/{story_common,portal_common,tower_common}.bs`.
- `<tiger>/packages/bullshcript/{LANGUAGE,NATIVES,INCIDENTS}.md`;
  `src/host/natives/{native_support,sequence_natives}.ts` and
  `src/host/build/instance_building.ts` within that package.
- `<harness>/src/activity-host-proxy/script-host/{compiled-scripts,script-source,runtime,item-sink}.ts`;
  `versions/v41375_14_07_03_1501_live/{sensor-auth,incident}.ts` under `activity-host-proxy`;
  `src/content/versions/v41375_14_07_03_1501_live/readers/incident-names.ts`.

## Lifetime numbers: distinguish the script, decoded value, and wire bits

Our fixture `@phase` is the **decoded signed lifetime value**, stored in the first byte of the
`0x808008A3` image. It is not the mission-phase index. Its four-bit wire representation is biased
by one: decoded 3 encodes as 4. The existing [sensor encoder](../ps4/d1-server/crates/d1-oracle/src/sensor.rs)
already performs that conversion; do not apply the bias again in a fixture.

BullShcript's internal model and the older harness's adapter are separate number spaces:

| Meaning | BullShcript runtime | Older live-build harness value | Current cinematic fixture `@phase` |
|---|---:|---:|---:|
| Initial/unset host state | no script member mapped | 0 | 0 |
| `pregame` | 0 | 1 | 1 |
| `countdown` | 1 | 2 | not needed by this reveal sequence |
| `active` (ordinary play) | 2 | 3 | ordinary mission spawn value 3 |
| Cinematic hold (runtime's descriptive name) | 3 | 4 | 4 |
| Results (`end_game`'s first write) | 5 | 6 | 6 |
| Ended (`end_game`'s subsequent write) | 6 | 7 | existing fixtures use 8; exact correspondence unresolved |

The older adapter adds one to runtime states. That explains pregame, ordinary active play,
cinematic hold, and results, but **does not explain our final 8**. There is no basis for changing
56578's end fixture to 7 merely because an older-build harness does so. Nor is there evidence here
for assigning names to every unused value. The observed capture order decoded with our tools was
0, 1, 4, cinematic play, 6, 8; see [the timeline](d1-live-speaker-timeline-2026-11-05.md).
Those captures came from Xbox 360 build 51539, as [their provenance](d1-live-captures-2026-11-05.md)
records. Native 56578 cine-live13 established that serving the sequence can play and leave the
activity; it did not establish every enum name or validate every intermediate state.

**Update (open-items pass, same day):** the 52 live captures and the 56578 consumers of the value
resolve the end: gameplay activities go `6`, then `7` (the post-game screen) about 30 s later;
cinematic-only activities go `6, 8` in one tick, and 8 is the client's own end-of-activity
transition. The completion incident chain and its `0x80800595` body are decoded too. See
[the wire catalogue](ps4-56578-wire-catalog-2026-11-05.md#activity-lifetime-completion-965-scoreboard-and-public-events).

## What completion sends, and what remains unknown

`set_activity_complete()` calls BullShcript's completion callback. The harness treats this as the
semantic `activity_complete` incident, but **does not emit that incident**: its runtime explicitly
leaves host-incident encoding unresolved. It also delays its BAP/profile completion until lifetime
end, to avoid launching the next activity while the outro is still playing. Therefore neither its
callback nor a profile completion is evidence for an already implemented wire payload.

The owned **56578** startup-root content resolves its incident registry to tag `0x81656598`, class
`0x80800659`. Its incident list has 3902 rows, with a 32-byte row stride. Resolving the semantic
FNV-1 names in that list gives:

| Name | FNV-1 name hash | 56578 client incident index |
|---|---|---:|
| `activity_complete` | `0x42660690` | **965** |
| `cinematic_finished` | `0x3C03F059` | 891 |
| `cinematic_skip` | `0x7352DAFE` | 1726 |
| `cinematic_started` | `0xAECA3822` | 2681 |

This is a content lookup, with three independently known cinematic indices as positive controls,
not an offset applied to another build's ID. The registry was read locally through MIT Gibbon's
56578 package reader; no registry bytes or extracted table were added here. A future native
completion encoder should resolve the index from that build's registry rather than assume 965
across builds.

**The 56578 metadata body, any accompanying incident types, and their construction remain open.**
The supplied prefix of the pre-cinematic host udt 18 is insufficient to establish them. A diagnostic
decode of that prefix padded with synthetic zeroes reports a multi-incident envelope, not a sole
965 notification; padding is not evidence for the missing fields. The original envelope is from
51539, whose IDs differ. Its approximately half-second lead before the cinematic is consistent
with the reconstruction's completion point, but timing alone does not identify its contents.
Do not paste that envelope, use its ID unchanged, or substitute empty metadata.

`end_game()` is a separate lifetime operation in BullShcript: write results, disable joining, start
the lifetime timer, then write ended and stop the scripts after the optional delay (zero by default).
No incident is emitted by that native implementation. For 56578 our supported approximation remains
the existing 6 then 8 lists after the final cinematic. Whether native retail shutdown sends other
messages, and how its final state relates to the older model's 7, still require build-specific proof.

## The state-2 swap needs mission-phase identity

The reconstruction runs the Traveler cinematic in the first mission phase, then the Tower cinematic
in the second. Both belong to plaza, bubble 3. The owned 56578 `cine_reveal` scenario-client tag
`0x80C3E00A` independently has three valid states for that bubble, so state 2 is a state of the same
bubble, not a new bubble numbered 2. Region encoding is `bubble << 3 | state`: the first and second
cinematic states are 25 and 26 respectively. The state-name hashes in that scenario match
FNV-1(`cine_106_trv_phase`) = `0x77A6931D` at state 1 and FNV-1(`cine_107_twr_phase`) =
`0xD77D27FF` at state 2; the bubble hash matches FNV-1(`plaza`). This independently connects
the reconstructed names to the owned target content.

The older harness advances the lifetime's **mission phase index, phase name hash, and bubble** when
it starts the next mission. Its content layer resolves the phase's bubble state. That is distinct
from `lifetime.set_state(...)` and a PAH region request.

Our [SensorPhaseUpdate](../ps4/d1-server/crates/d1-oracle/src/sensor.rs) currently writes NONE/empty
defaults for all three mission fields. The 56578 descriptor for `0x808008A3`, inspected with our
existing descriptor reader, locates them at decoded image offsets `+0x04`, `+0x08`, `+0x0C` after
the state fields; types are 32-bit integer, name hash, 32-bit integer. The harness supplies the
semantic interpretation; descriptor geometry alone does not prove the interpretation in 56578.

The exact host extension still needed is an optional mission-phase tuple on `SensorPhaseUpdate`,
populated by the sensor-list parser, encoded through the existing object codec, and carried in every
subsequent frame. For example, a future `@mission-phase INDEX NAME BUBBLE` directive could resolve
the name with the existing FNV-1 helper. For reveal it must select index 0 / `cine_106_trv_phase` /
plaza, then index 1 / `cine_107_twr_phase` / plaza, resolving hashes and bubble indices from content.
The phase index 1 must **not** be confused with bubble state 2 or lifetime state 4. Do not add that
directive to a fixture before the parser supports it: unknown directives are currently ignored.

The existing region-26 request and membership/slot handover remain useful, but cannot be claimed
to replace those lifetime fields. The earlier native list contained only `/6/0` in state 1; `/6/1`
still needs an exact-build binding check in state 2, rather than an assertion that it is absent
from the entire activity.

## Updated schedule and its limits

[cine-reveal-steps.txt](../ps4/fixtures/56578/mission/cine-reveal-steps.txt) now expresses the supported
operations in this order:

1. Initial lifetime 0, then pregame 1, then Traveler play at 4.
2. On cinematic-finished 891, stop the first row and request region 26, keeping lifetime 4.
3. Wait for client CURRENT slice 26 using the existing `%26` gate, replacing the single-run 20 s wait.
4. Wait four seconds; republish the idle second-cinematic list at the **missing completion slot**.
5. Half a second later, play the Tower cinematic at lifetime 4.
6. On finished 891, stop both rows and publish 6; one second later publish 8.

Step 4 does **not** mark completion: the fixture says so. Once the metadata is derived, the existing
mission-step `incident=TYPE:BODYHEX` option can send it at that point, through
[incident_image / incident_due](../ps4/d1-server/crates/d1-activity-host/src/state.rs) and the existing
AH schema codec. `incident TYPE BODYHEX` control lines provide the same transport for private
experiments. `BODYHEX` there is metadata inside the decoded incident envelope, not a complete udt-18
wire message. Multiple incident IDs would require extending the single-type step/image interface.
Runtime-dependent metadata needs a semantic builder, not a captured static body.

The 60-second start cushion, half-second separation and one-second shutdown gap are diagnostic
schedule choices, not newly proven retail constants. Existing 900-second event/slice fallbacks can
advance without their events and remain another limit; an actual script adapter must hold or fail
explicitly on missing completion/arrival evidence. This change neither fixes the missing completion
payload nor implements mission-phase replication, join-flag changes, or lifetime-timer replication.

## What the v56578 script folder covers

This local snapshot contains 172 `.es`, 169 `.esp`, 36 `.esu`, and three common `.bs` files. File
presence is not proof of a working activity. In particular, several requested cinematic roots are
only launch metadata. Paths in the table are relative to `<scripts>/es`; a root's companion `.esp`
files supply its phases where listed.

| Activity or group | Local path | Present coverage |
|---|---|---|
| Reveal, 165 | `cosmo/cine_reveal/` | Pregame and two cinematic phases, completion before the second cinematic, shutdown after it |
| The Speaker, 167 | `cinematic_trv/cine_speaker/` | Start-slice declaration only; its comment's Restoration label is inconsistent with our activity identification |
| The Awoken, 186; A Key Awaits | `cinematic_queen/act_cinematic_queen_{101,102}/` | Start declarations; 102 names a cinematic phase, but neither has mission logic |
| Hangar; ending cinematic | `cinematic_trv/{cine_hangar,cine_end}/` | Hangar has one mission; ending root is start metadata only |
| A Guardian Rises, 164 | `cosmo/act_cosmo_m0_highway/` | Seven phases, separate highway/Breach/graveyard/hangar encounters, plus audio/music bundle |
| Dome A | `cosmo/act_cosmo_m1_dome_a/` | Three phases, scan, basement waves, upper-floor wizard, exit; reconstruction notes its waits/rules are provisional |
| Other Cosmodrome story | `cosmo/act_cosmo_m0_5_hangar/`, `act_cosmo_m2_dome_c/`, `act_cosmo_m4_dome_c/` | Three, five, four missions respectively, with area encounters |
| Later Cosmodrome missions/quests | `cosmo/{mission_elevator,mission_pact,quest_bonzo_dlc01,quest_infidel}/` | Five, five, two, four missions respectively, plus companion encounter/audio bundles |
| Cosmodrome strikes | `cosmo/{cosmo_portal1_activity,cosmo_portal_2}/` | Portal1 has eight missions and strike encounters/audio; portal2 is start metadata |
| Cosmodrome placeholders | `cosmo/{act_cosmo_m5_refinery,act_cosmo_m6_rasputin}/` | Start metadata only |
| Tower | `city_tower/` | Fireteam-bind startup for default/Iron Banner/Queen variants; military/plaza ambient roots also present; no vendor service implementation implied |
| Dreadnaught/impact site | `hiveship/mission_touchdown/`, `impactsite/mission_darksnow/` | Six and seven mission files respectively; the latter uses an unresolved root name |
| Mars story | `mars/act_mars_{m01_valley,m05_warbase,m06_subway,m07_lowergarden}/` | Six, nine, eight, ten phases; penthouse is start metadata only |
| Mars strikes | `mars/{mars_portal3_activity,mars_portal_1,mars_portal_2}/` | Portal3 has six missions and audio/area encounters; portal1/2 are start metadata |
| Moon story | `moon/act_moon_{m1_launchtubes,m3_lowercitadel,m5_strength}/` | Nine, five, eight phases; uppercitadel/reactor/selection roots are start metadata |
| Moon strikes | `moon/{moon_portal_2,strike_vampire}/` | Six and eight phases, with companion scripts |
| Venus story/quests | `venus/{act_venus_m01_engineering,act_venus_m07_northern_tides,mission_pursuit,mission_traitor,quest_safari}/` | Six, five, five, six, five missions; southern_tides/vault/ketch roots are start metadata |
| Venus strike/raid placeholders | `venus/{strike_1_activity,venus_portal_2,venus_raid_1}/` | Strike1 has five mission files and audio/area encounters; portal2/raid1 are start metadata |
| Reef and PvP | `reef/social_landing/`, `pvp_*/` | Start metadata, including eleven PvP map roots; no reconstructed match rules implied |
| Shared helpers | `utility_scripts/`; `<scripts>/bs/activities/common/` | Cinematics, objective/interaction/trigger helpers, encounters, events; common binders attach lifetime to its timer |

## How the harness runs them, and a path for our host

The harness chooses a build-specific script folder, compiles the reconstructed language in memory
or loads a precompiled set, and links bundle roots to mission and archetype scripts. Bundle rows and
their schemas bind script names to runtime objects. A cooperative VM advances startup threads,
sleeps, predicates and phases on the host clock. Runtime item-change callbacks update an item sink;
its build adapter serializes dirty auth rows. Client incidents and sensor reports feed the model
back: cinematic finishes, trigger firings, squad counts and interaction state unblock threads.
The lifetime's current phase is host state, preserved across bubble-local runtime rebuilds.

**Prefer a local TypeScript sidecar first.** Use BullShcript's MIT compiler/runtime directly, with
its own notices; write our adapter from its documented/public interfaces. Do not transplant the
unlicensed harness. Keep reconstructed scripts and compiled script/game-derived artifacts outside
the repository, selected by environment variables. Start with reveal, then AGR, then Dome A.
BullShcript currently exposes older build adapters, so a 56578 adapter and exact-build row types,
schemas and bindings must be validated; selecting an older adapter by proximity is not sufficient.

The sidecar should receive content-derived bundle/row definitions and an ordered stream of client
observations. It should emit semantic changes over local IPC: cinematic state, mission-phase tuple,
lifetime state/timer/join policy, and completion requests. Our Rust host retains the sole RSAT and
BAP encoding boundary. Derive live timestamps and per-instance identities in that boundary; do not
send precomputed capture payloads. Apply commands atomically/in order and stop progression when the
sidecar exits, bindings are unresolved, or required client evidence is unavailable. Profile/BAP
completion must remain separate from the pre-outro script notification.

Porting the MIT VM to Rust is a later option: reproduce its bytecode semantics, closures/upvalues,
cooperative threads, sleeps, restart/stop behavior, native dispatch and phase ownership, and compare
semantic event streams against the TypeScript runtime on synthetic scripts. That is substantially
more work than an adapter. No unlicensed reconstructed script text is needed in those tests.

The adapter must back these native families (names from `NATIVES.md`; the scope grows by activity):

| Scope | Required natives and observations |
|---|---|
| Reveal and common binder | `get_lifetime_sensor`, `get_lifetime_timer`, `import_only_bind`, `set_state`, `allow_join_in_progress`, `end_game`, `set_activity_complete`; cinematic `start`, `is_done`; timer `reset`, `start`, `stop`, `is_expired` |
| Scheduling/phase engine | VM `sleep`, `sleep_until`, thread restart/stop and ordered mission `init`/`main`/`complete`; phase-to-bubble/state binding; `get_current_activity_time` and client readiness |
| AGR and Dome A | Squad `place`, `remove`, `alive_count`, `set_objective`, `set_task`; player-objective `activate`, `set_active`, `set_complete`, `clear`, `is_active`, `nav_point_activate`, `nav_point_clear`; AI-objective `add_squad`, `reset`; player-trigger `enable`, `disable`, `is_triggered`; sequence `start`, `is_done`; interaction and device state as named in each phase |
| Broader missions | `all_players_are_ready`, `get_player_count`, `get_player_sensor`, fireteam `t30_hack_bind_host`; loot `enable`, `get_count`, `reset`; activity-flag `activate`, `clear`, `is_active`; object filters, encounter activation, hard-wipe/checkpoint state, dialogue/audio sequences and `set_time_of_day` |

Names in this plan are required interfaces, not a claim that all are wired today. Cinematic-finished
891 should resolve to a particular bound row, not just satisfy any outstanding cinematic wait;
likewise squad/trigger evidence must retain group, index, world epoch and ordering.

## Validation and remaining work

Offline AH codec controls passed against the owned 56578 ELF using an existing prebuilt oracle.
The existing descriptor reader found all listed lifetime fields with no layout problems. Gibbon's
registry lookup reproduced the three known cinematic IDs before resolving completion 965; its
scenario reader independently found the three plaza states. The diagnostic prefix decode is
explicitly incomplete and supplies no metadata proof. No heavy build or native run was performed.

The seven existing tests in `tests/test_cine_watch.py` passed. A separate lightweight fixture check
validated all nine schedule rows, list paths, arrival-before-delay ordering, and unchanged active
directives/row data in every referenced list. These are offline checks of the supported approximation;
they do not validate a completion incident or the omitted mission-phase fields.

Before calling 165 retail-like: derive the complete incident semantics/body for 56578; add and
control mission-phase replication, join policy and timer writes; resolve the final-8 discrepancy;
verify `/6/1`'s state-2 binding; then have the operator run the two cinematics and the next-activity
transition. An updated schedule alone cannot close those gates.
