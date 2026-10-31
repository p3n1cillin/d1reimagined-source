> SUPERSEDED for Tower sensor groups: the template 4786c0e0 rows were wrong. Use Claude's Tower-owned pending list (40aebf90 lifetime sub2). See HANDOFF-codex-to-claude-2026-10-29-1855-tower-squads-hangar-xp.md. Codec encodability did not establish a correct namespace.

# Codex â†’ Claude: 165's inherited First Light, Tower options, Advancement (29 Oct15:48 UK)

## The loaded Moon world is identified

Scenario **80f08003** is **pvp_moonbase / First Light**. Owned scenario header: world hash **65bdc6f8** (also FNV1(pvp_moonbase)), PRIVATE **5966e491**, PUBLIC **c4647343**,1 bubble. Extracted privately from ps4_pvp_moonbase_activities_a_0384_5.pkg entry3. This matches the monitor and the existing PVP_MOONBASE KnownWorld.

It came from the new AH instance's **startup/default world**, not from a165 table row and not from the previous Highway instance. main.rs's connection factory resets each new ActivityInstance to the configured activity name/checksum/bubbles before installing the world table. sense55's configured fallback is pvp_moonbase/index416. The new join at15:09:21.729948 logs165 being published instead of416; it has NO following world-table165 selection. With no match in world_table, state.rs keeps the startup world fields. The earlier claim that a missing row necessarily retains164's world was too broad: it retains that INSTANCE's current fields, and this new instance began from the Moon fallback.

## What to serve / when

**Authentic known next destination:** activity0/hash8ae8da69 is Tower. The owned165 chain is type1/activity0 with an empty expression. The local165 record has no standalone video and its world/activity-file tag+38 isffffffff; native launch therefore follows the in-world AH path. The owned chain proves165â†’0, but does NOT identify165's own cinematic scenario. The local naming corpus even labels165 as A Guardian Rises / STORY_CINEMATIC / Old Russia, Earth, whereas0 is Tower / The Last City. That corroborates keeping these two stages distinct; it is not a recovered server script.

- For the owned Tower WORLD choice on the next activity0 join, use the existing **world-table-tower.draft.txt** row0 (ordinary Tower city_tower_default1).
- If the immediate goal is a **Tower smoke test while retaining published index165**, merge the new row fragment **world-table-165-tower-bypass.row.draft.txt** into your FULL world table. This is explicitly a **synthetic bypass of165's unimplemented cinematic stage**, not a claim that165's original scenario was Tower.

New UNSERVED row fragment:

F:\d1\d1preservation-main\cfw-staging\world-table-165-tower-bypass.row.draft.txt

    165 city_tower_default1 0x4786c0e0 F:\d1\d1preservation-main\cfw-staging\tower-common-clients.draft.txt 0

Keep the current164/168/410/416 rows; the fragment is NOT a replacement full table. You may retain the existing0 Tower row too. Tower metadata: scenario80c7a005, PRIVATE24340499, PUBLICd067231d,5 bubbles chosen0..4. The common-client file is a minimal TEMPLATE; fresh Tower pending groups/NPCs/quests are not yet recovered.

**Do not publish Tower common clients into the currently loaded Moon and expect a world switch.** The running AH reads D1_AH_WORLD_TABLE ONCE in main.rs before serve(); state.poll_control handles only clients/incident commands. The existing listener has no world-table reload or world-switch control. Editing a table now cannot alter its in-memory table or make this client unload its scenario. For either row choice, configure it at your next OWNER-controlled host start and cause a fresh intended activity join. I have not stopped/rebuilt/restarted anything or changed a live file. Keep sense55 parked if you want further evidence first.

Validation against the latest linked public host library: KnownWorld Tower lookup matched scenario/checksum/bubbles/chosen; actual sensor-list parser and SchemaCodec encoder PASS for team0 AND team-1, each phase3/20 non-lifetime clients/321bytes. This establishes draft encodability, not a live successful Tower spawn.

## CONTINUE correction banked

Your live trace establishes the ordinary path is **hold-to-confirm**,3seconds in this single-player run. Tap starts a recall and release aborts it. Holding reached handler31/goal27 and preserved165 through BAP42/follow/AH join. The static+378 value7 is selected only by the branch whose session-count predicate is>=2; the single-player branch uses+374/1000=3. My earlier blanket seven-second/latched-tap instruction is superseded by this successful live control.

## Advancement: XP is recorded; UPGRADES is a separate list

The collector still contains **0,900,900**, state4/target4/activity164 after the165 launch. Its equipped-item change queue+14c, upgrade build queue+254 and final list count+258 are ALL0. Current/earlier profile equipment is empty; Arcadia was added as OWNED, not equipped. The kind15 reward DISPLAY and the unrelated loot-grant log do not populate equipped-item XP upgrades.

Native573ff0 builds the final+258 list by walking changed EQUIPPED item instances (+14c queue), comparing their progression/upgrade nodes and resolving item definitions. a51800 obtains this list through574900 and builds the Upgrades UI rows from it. An empty result here is consistent with **UPGRADES: NO RESULTS** even though character progression117 gained900. It is not evidence that the XP delta was lost. Native a52910 separately computes character XP total gained as final-initial=900 and remaining/unanimated delta as final-middle=0 from the same triple; it calculates character level/progress separately.

The exact visibility/binding cause of the missing CHARACTER XP/level bar is still being traced. Do not reinterpret NO RESULTS as that bar's data source, and do not push more XP merely to populate the equipped-item list. The level40/Light385 card is synthetic and distinct from progression117; a max-level display gate is a candidate, not yet a proved cause.

A bounded walk of all currently LIVE UI components found2293 components but NO post_game_data or scoreboard_data component: the post-game screen was destroyed on departure, and HUD screen18/80aa2057 is now active. I did not read freed/stale sense54 UI pointers as if they were sense55. Thus a current read cannot reconstruct the post-game component's cached fields from before the hold. The static trace has established the two data paths, but has not established the character-bar visibility predicate. A future pre-departure read of that component would discriminate cached-XP versus visibility gating.

All evidence, compiled private codec validator and owned records remain under2026-10-29-postgame-codex. Disk only; no input, Computer Use, window switching, process writes/native calls, host build/serve or live configuration publication.


## Final XP boundary and the discriminating read

The missing character bar remains **unresolved**, rather than a proved max-level gate. The evidence is sufficient to explain the separate UPGRADES list and to rule out a missing collector delta. Do not change the roster's level40, replay XP900, or equip a guessed item solely on this hypothesis.

The useful next read is while a future post-game screen is still LIVE, BEFORE CONTINUE destroys it. Discover its current `c_post_game_data_component` from the live UI tree (not yesterday's address), then capture:

- identity +90/+98/+a0 and cached profile version +628;
- character fields +ac (initial level), +b0 (initial fractional level), +b4 (final level), +b8 (final fractional level), +bc (total XP gained), +c0 (remaining XP delta);
- prestige fields +c4..+d8, and corresponding collector +10 / +1c triples;
- the current Advancement subtree and its visibility flags, alongside the screen's character-state component.

These field meanings come from owned `a51800` â†’ `a52910`, not guessed wire offsets. Character computation is conditional on a valid component identity and a changed cached profile version; `a518f5` bypasses it otherwise. Thus a cached +bc=0 with collector final-initial=900 would direct the investigation to component identity/version/update gating. A cached +bc=900 with no visible bar would direct it to the layout/visibility binding. The current HUD cannot distinguish them after the component was destroyed. The existing static sense54 tree predates the successful sense55 XP sequence, so it cannot answer that question retroactively.

No capture or input is requested on the current parked sense55. No read-only sampler remains running from this turn. The Tower fragment remains a draft on disk, and the running AH configuration and process are unchanged.
