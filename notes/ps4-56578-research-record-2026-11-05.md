# Research record: resources for the full game (5 November 2026)

This is the research phase of the full-game operation. Four research agents and Codex searched for about an hour. This record lists:
- each resource found;
- its licence;
- the subsystem it can improve;
- whether it was integrated, kept as a reference or rejected.

Licences follow [licensing](../ps4/design/licensing.md). Rule 2's exception covers Blam Network's unlicensed material only: it is read and run locally and never copied here. Downloads and clones live outside the repository, under `F:\d1\third-party\trawl-2026-11-05\` (each folder has a `SOURCES.md`). Bungie's game data stays a local input.

## Highest-leverage findings

1. **Restored mission scripts for our exact build.**
   - Blam Network's `fukugen-daihon` has a set for 56578 (`v56578_17_04_12_1251_evil`): 380 files, reconstructed from retail PS4 2.3.1 sessions.
   - 30 activities are written missions (169 phases); 29 are start-only stubs. That includes all 11 Crucible maps, the Vault of Glass and several story missions.
   - It covers A Guardian Rises, the reveal and Tower cinematics, the Awoken cinematics, Restoration, the Tower and the Reef.
   - The MIT `bullshcript` compiler and VM can run them. Its nearest target is PS4 01.29 (`v52553`), one patch before ours.
   - Status: **integrating.** The script sidecar and host bridge are workstreams A1 and A2. Unlicensed: used locally only.
2. **Bungie's final D1 manifest** (DIM's archived copy, MIT repository, Bungie's data, so a local input only). It has 8,609 items, the same count as 56578's content. It carries:
   - the exact XP curve (level 40 = 346,000) and the rank curves;
   - 95 vendors with prices, currencies, refund policy (Eververse: 15 minutes) and reset offsets (daily 09:00 UTC, weekly Tuesday 09:00, Xûr Friday 09:00);
   - mission order and recommended levels;
   - Grimoire, objectives, talent grids and unlock flags.

   Status: **integrating** in the player-store workstream (C).
3. **Bungie's own service architecture**, from Michael Williams, "Exploring Services Architecture at Bungie", GDC 2021.
   - The services: SignOn and its queue, BAP, an in-memory WorldServer that saves every 5 minutes and on important events, Claims, and pub-sub character updates.
   - Activity hosts over TCP and bubble hosts over UDP, run by proxies.
   - D1 characters stored as attribute rows.

   Status: **design input** for the player store (C).
4. **`activity_complete` is incident 965** in 56578's content. The lifetime states map to served phases as pregame 1, active 3, cinematic hold 4, results 6, and 8 at the end (Codex, #304).
5. **Bungie's renderer talks** (Tatarchuk, GDC 2015 and 2017, and SIGGRAPH 2013 and 2014) explain D1's per-draw cost:
   - many small command buffers per view and render stage;
   - 3,100–9,000 shader techniques per frame;
   - each piece of gear drawn about 5 times;
   - redundant scope re-binds at the start of each command buffer.

   Status: **design input** for the emulator per-draw workstream (F).

## Resources by subsystem

### Mission scripting and activity hosting
| Resource | Licence | Use |
|---|---|---|
| [Blam Network tiger](https://github.com/Blam-Network/tiger) `bullshcript` (82c57cb) | MIT | Integrating: the script sidecar (A1) |
| tiger `fukugen-daihon` (56578 set) | none (rule 2 exception) | Local input for A1/A2; never vendored |
| [tiger_test_harness](https://github.com/Blam-Network/tiger_test_harness) | UNLICENSED (rule 2 exception) | Reference for how a runtime drives a client; read only |
| Bungie GDC 2015 "Shared World Shooter" | public talk | Reference: [notes](d1-gdc2015-activity-hosts.md) |
| Bungie GDC 2021 "Exploring Services Architecture" | public talk | Reference: the service topology |
| [Magnetesim/d1-standin](https://github.com/Magnetesim/d1-standin) (PS3 stand-in for RPCS3) | AGPL-3.0 | **Rejected**: not read (rule 1) |
| [leoxcho/ProjectDestiny-Research](https://github.com/leoxcho/ProjectDestiny-Research) | none | README only; ask the author |

### Protocol and traffic
| Resource | Licence | Use |
|---|---|---|
| [Blam Network rsat](https://github.com/Blam-Network/rsat) and tiger `rsat` | MIT | Cross-check, and the reference for the native codec (B) |
| [kallsyms/d1-re](https://github.com/kallsyms/d1-re): PS3 sign-on and BAP captures, decrypted | none | README only; ask the author for a licence |
| [isinternets/d2-research](https://github.com/isinternets/d2-research) (D2: BAP, replication, player state) | none at the top level | README only |
| Demonware servers: elliot1377/mw2-ps3-server (MIT), iw4x bde (GPL-3.0), madden20-online (GPL-2.0, "local EA server + patched shadPS4") | as listed | Reference for Demonware stubs and the local-server-with-shadPS4 pattern |
| open-bitdemon-emulator, openpak/demonware (AGPL); demonware-server (PolyForm Shield) | not compatible | **Rejected** |

### Content, packages and tags
| Resource | Licence | Use |
|---|---|---|
| tiger `gibbon` (tag reader with 56578 definitions) | MIT | In use; A1 exports bundle tables with it |
| [v4nguard/destinypkgtool](https://github.com/v4nguard/destinypkgtool) and [tiger-parse](https://github.com/v4nguard/tiger-parse) | MIT | Cloned (bundled game files and key blob removed); can be adapted for package and tag readers |
| [Kalivins/tiger-offline-reader](https://github.com/Kalivins/tiger-offline-reader) (D2 encounter extraction) | MIT | Cloned; reference for encounter data |
| [cra0kalo/DestinyPKGTool](https://github.com/cra0kalo/DestinyPKGTool) | GPL-2.0 | Cloned (game files removed); reference |
| craftycodie/cstruct | MIT | Cloned |
| [sskorri/Destiny-Activity-Loader](https://github.com/sskorri/Destiny-Activity-Loader) (337-activity Xbox 360 catalogue) | Apache-2.0 | Cloned; reference |
| MontagueM Destiny1Things (map exports) | Bungie assets | **Rejected** |

### Game data (local inputs; only facts committed)
| Resource | Licence | Use |
|---|---|---|
| DIM's archived D1 manifest | MIT repo, Bungie data | Player store (C): XP, ranks, vendors, resets |
| [nruble/cryptarch-guide](https://github.com/nruble/cryptarch-guide) data (39 manifest tables; per-activity rewards and weekly limits) | GPL-3.0 | Player store (C); community-collated, so labelled stand-ins |
| [DestinyDevs/BungieNetPlatform](https://github.com/DestinyDevs/BungieNetPlatform) (D1 OpenAPI, definitions) | MIT | Cloned; field names and shapes |
| Bungie patch notes (2.0.0 to 2.6.0.2, destiny.bungie.org archive) | Bungie, so cite only | Light caps and engram rules per patch |
| nmlorg/destiny-db (manifest history, 2014–2016) | none | README only |

### Emulator and rendering
| Resource | Licence | Use |
|---|---|---|
| Tatarchuk, GDC 2015 multithreaded renderer; GDC 2017 TFX shader pipeline; SIGGRAPH 2013 and 2014 decks; I3D 2015 keynote | public decks | Design input for F (per-draw cost) and for lighting and material fidelity |
| Genova, GDC 2015 "Multithreading the Entire Destiny Engine" | public talk | Frame pacing, core affinity, stalls at destination changes |
| shadPS4 issues #317, #964, #994 (CUSA00219 crash at eboot `0xFAAF4`, still in 0.19.0) | public | Our fork is past it; upstream baseline |

## What's still not public, and the new obstacles
- **Not found publicly:**
  - drop rates and engram decrypt pools;
  - dismantle amounts;
  - Light weighting;
  - public-event schedules;
  - Xûr's weekly history.

  These were server-side. Placeholders must stay labelled and isolated.
- **Gaps in the 56578 scripts:** 29 activities are start-only stubs, including the Crucible and the Vault of Glass.
- **Gaps in BullShcript:** the runtime's natives are for 01.29. Several 56578 types are missing (`cinematic_midmission`, for one).
- **Host work the scripts need:**
  - a row table that sends only changed entries;
  - row encoders for each type;
  - a 56578 incident catalogue (Codex is on this);
  - sense decoding beyond squad counts.

  Stream A2 covers the table, the encoders and the bridge.
- **Contacts that would unlock more:** Codie (a licence for Fukugen Daihon and the harness), kallsyms (PS3 captures), leoxcho.

## Workstreams fed by this research
- **A1:** the script sidecar.
- **A2:** the host script bridge.
- **B:** the native codec (from #301).
- **C:** the player store, with manifest progression.
- **E:** worlds for every activity.
- **F:** emulator per-draw cost.
- **Codex:** 165 from the scripts (#304) and the incident catalogue.

Each stream ends in a draft PR. The integrated native test comes after the build-out.
