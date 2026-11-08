# Servers rebuilt for other games, and what each suggests for ours (1 November 2026)

The user asked about projects similar to this one: servers rebuilt from the retail client for online games that were shut down. 29 were surveyed. The most useful were checked out (with `--depth 1`) and read for their designs. None was built or run.

Licences decide use:
- MIT code (PocketRelay here) may be adapted, with credit in [CREDITS.md](../CREDITS.md).
- GPL code is read for design.
- AGPL code is not used **and not read**, not even as a source of facts ([licensing](../ps4/design/licensing.md), rule 1). AGPL is network copyleft: anyone who runs modified AGPL code for players over a network must offer them its source. That is no fit for the project's servers, or for an anti-cheat.

**A correction (5 November 2026).** Four of the five projects read on 1 November were AGPL-3.0: MHServerEmu (Marvel Heroes), NexusForever (WildStar), SpaceNinjaServer (Warframe) and Dauntless Revived / Undaunted (Dauntless). Reading them broke the rule above. No code from them is in this repository, and nothing here was written from them; the server's design came from the D1 client and the live captures. Their summaries are removed from this note and [CREDITS.md](../CREDITS.md), the local checkouts are quarantined, and they are not consulted again. AGPL projects are also gone from the reading list below.

The Destiny-specific tools are in [related projects](related-projects.md).

## Read in depth

| Project (game) | Licence | Language | What it suggests for D1 |
| --- | --- | --- | --- |
| [PocketRelay](https://github.com/PocketRelay/Server) (Mass Effect 3 multiplayer) | MIT | Rust | Reusable with credit: a typed handler router that logs unknown commands; matchmaking by rule set into player-hosted games with host migration; a UDP tunnel for players behind NAT. |

## Worth reading later

These were not checked out:

| Project | Licence | Why |
| --- | --- | --- |
| [LandSandBoat](https://github.com/LandSandBoat/server) (FFXI) | GPL-3.0 | Lua mission scripts; spawn groups |
| [newserv](https://github.com/fuzziqersoftware/newserv) (Phantasy Star Online) | MIT | Server-side drop tables; quest handling |
| [Horizon](https://github.com/Horizon-Private-Server/horizon-server) (Medius, PS2/PS3) | MIT | Sony-era lobbies and game lists |
| [ds3os](https://github.com/TLeonardUK/ds3os) (Dark Souls 2/3) | MIT | Brokering summons between players |
| [Northstar Atlas](https://github.com/R2Northstar/Atlas) (Titanfall 2) | MIT | A modern master server |
| [shadp2p](https://github.com/Wozzardman/shadp2p) | GPL-2.0 | A shadNet fork for Bloodborne co-op; the only other PS4 game we found online over shadNet |

Nothing public was found for Anthem, The Division, MAG, Killzone 3, Resistance 2 or Warhawk. OpenSpy has no licence, so it can't be used.

## Our own design principles for the open problems

General server practice, not taken from any of the projects above:

- **Missions as data.** Our hand-written step schedules could become data: conditions (slice reached, incident, trigger, squads cleared) and actions (arm, place, play, advance). The host's `StepWhen` is the start of the condition half. The Destiny-specific source for the content is the client's own mission scripts and Blam Network's Fukugen Daihon reconstructions ([related projects](related-projects.md)).
- **Vendors.** Price everything on the server and take only an index from the client. Charge and grant in one step, keyed by the request, so a retried request is answered again rather than charged twice. Check for a free slot before buying. Put sales on a buyback list (902 `vendor_buyback_item` in [Blam's 2014 message names](rsat-blam-crosscheck.md)).
- **Public sessions.** Bungie's own design applies: one bubble host per public instance ([GDC 2015](d1-gdc2015-activity-hosts.md)).
- **Unknown requests.** Log them, and don't guess an answer: a wrong empty reply can crash a client (PocketRelay's router logs unknown commands for this reason).
