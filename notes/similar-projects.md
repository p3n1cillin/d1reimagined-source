# Servers rebuilt for other games, and what each suggests for ours (1 November 2026)

The user asked about projects similar to this one: servers rebuilt from the retail client for
online games that were shut down. 29 were surveyed. The five most useful were checked out (with
`--depth 1`, under `F:\d1\third-party`) and read for their designs. None was built or run; the two
nearest need their game's data to start.

Licences decide use:
- GPL and AGPL code is read for design and never copied.
- MIT code (PocketRelay here) could be adapted, with credit in [CREDITS.md](../CREDITS.md).

The Destiny-specific tools are in [related projects](related-projects.md).

## Read in depth

| Project (game) | Licence | Language | What it suggests for D1 |
| --- | --- | --- | --- |
| [MHServerEmu](https://github.com/Crypto137/MHServerEmu) (Marvel Heroes, including a PS4 client) | AGPL-3.0 | C# | The closest model of our activity host: the server interprets the client's own data (populations, encounter phases, mission conditions and actions, wave states, loot tables). `PopulationManager.SpawnEncounterPhase` fires each phase once (a bitmask) and queues its squads; `SpawnScheduler` always places critical objects and retries failures; markers are reserved so squads don't collide. `MetaStateWaveInstance`, `MetaStatePopulationMaintain` and `MetaStateMissionSequencer` are waves, a kept population and chained encounters. |
| [NexusForever](https://github.com/NexusForever/NexusForever) (WildStar) | AGPL-3.0 | C# | Spawn rows tied to public-event phases. Saves write only changed fields, and a player's packets pause during a save (no item duplication). Instances come from a "map lock" (solo, match, residence). Vendor requests carry only an index into the server's list; sales go to a buyback list. Its bugs are a checklist too: buying never checks for a free slot, and selling trusts the client's quantity. |
| [SpaceNinjaServer / OpenWF](https://github.com/spaceninjaserver/SpaceNinjaServer) (Warframe) | AGPL-3.0 with Commons Clause | TypeScript | The looter-shooter economy. Every action returns an inventory change list. Weekly content and vendor stock come from the day or week number, so nothing is stored (the Xur and weekly rotation case). Reward seeds are issued by the server, and data snapshots are kept per client build. |
| [PocketRelay](https://github.com/PocketRelay/Server) (Mass Effect 3 multiplayer) | MIT | Rust | Reusable with credit: a typed handler router that logs unknown commands; matchmaking by rule set into player-hosted games with host migration; a UDP tunnel for players behind NAT. |
| [Dauntless Revived](https://github.com/mixutin/dauntless-revived), from [Undaunted](https://github.com/SyST3MDeV/Undaunted) (Dauntless) | AGPL-3.0 | C++, TypeScript | The inventory ledger treats a repeated transaction id as a retry and replays the stored answer, so nothing is charged twice. Unknown routes get a strict 404, because guessed empty replies crashed the client. Its social space is one long-running process and each hunt its own. |

## Worth reading later

These were not checked out:

| Project | Licence | Why |
| --- | --- | --- |
| [ACE](https://github.com/ACEmulator/ACE) (Asheron's Call) | AGPL-3.0 | Data-driven spawn generators; tiered loot |
| [SWGEmu Core3](https://github.com/swgemu/Core3) | AGPL-3.0 | Lua quest scripts; lairs that spawn waves when attacked |
| [LandSandBoat](https://github.com/LandSandBoat/server) (FFXI) | GPL-3.0 | Lua mission scripts; spawn groups |
| [newserv](https://github.com/fuzziqersoftware/newserv) (Phantasy Star Online) | MIT | Server-side drop tables; quest handling |
| [Horizon](https://github.com/Horizon-Private-Server/horizon-server) (Medius, PS2/PS3) | MIT | Sony-era lobbies and game lists |
| [ds3os](https://github.com/TLeonardUK/ds3os) (Dark Souls 2/3) | MIT | Brokering summons between players |
| [Northstar Atlas](https://github.com/R2Northstar/Atlas) (Titanfall 2) | MIT | A modern master server |
| [shadp2p](https://github.com/Wozzardman/shadp2p) | GPL-2.0 | A shadNet fork for Bloodborne co-op; the only other PS4 game we found online over shadNet |

Nothing public was found for Anthem, The Division, MAG, Killzone 3, Resistance 2 or Warhawk.
OpenSpy has no licence, so it can be read only.

## What it means for our open problems

- **Waves.** Our trigger-driven Breach ([`d1-breach-waves.py`](ps4-56578-breach-sequences-2026-10-31.md))
  is already MHServerEmu's phase model in miniature: a trigger fires a phase once and places its
  squads. Their scheduler adds two things we lack: retrying a placement that fails, and keeping
  critical squads apart from ambient ones.
- **Missions.** Our hand-written step schedules could become data: conditions (slice reached,
  incident, trigger, squads cleared) and actions (arm, place, play, advance). The host's `StepWhen`
  is the start of the condition half.
- **Vendors.** Price everything on the server and take only an index from the client. Charge and
  grant in one step, keyed by the request, so a retried request is answered again rather than
  charged twice. Check for a free slot before buying. Put sales on a buyback list (902
  `vendor_buyback_item` in [Blam's 2014 message names](rsat-blam-crosscheck.md)).
- **Public sessions.** Keep one instance registry keyed by activity, and fill the emptiest public
  instance (the Tower) until it is full before making another.
- **Unknown requests.** Log them, and don't guess an answer; Dauntless shows that a wrong empty
  reply can crash the client.
