# Credits

D1 Reimagined only got this far because other people published their work. This page names them and
says what of theirs the project uses. Licence details are in [LICENSE](LICENSE),
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) and the file headers; this page is about credit.

## aknumbers: the Destiny 1 Preservation Project

Most of the PS4 work here is built on **aknumbers'** Destiny 1 Preservation Project
([gitlab.com/aknumbers/d1preservation](https://gitlab.com/aknumbers/d1preservation), GPL-2.0-or-later).

- The [`ps4/`](ps4/README.md) tree is their source, from the `d1preservation-main` snapshot of
  11 August 2026, with this project's preparation and content-reader fixes on top.
  - It includes the Rust services in `ps4/d1-server` (the shadNet server, the BAP and activity-host
    services, the run harness, and the package and executable tools) and the capture rig.
  - It also includes the tracing and observation tools, Ghidra scripts, loadouts, and the design and
    status documents.
- Files headed `SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project` are theirs.
- aknumbers' published videos and notes on builds 01.29 and 56578 (1.33) also guided the move to
  the final PS4 build.

## Emulators

- **[shadPS4](https://github.com/shadps4-emu/shadPS4)**, by the shadPS4 Emulator Project and its
  contributors (GPL-2.0). It is the emulator Destiny runs on.
  - The project's build, [d1reimagined-shadps4](https://github.com/p3n1cillin/d1reimagined-shadps4),
    is upstream shadPS4 plus the `d1` branch.
  - shadPS4's in-tree shadNet client is the authority the services are written against; see
    [`ps4/d1-server/ATTRIBUTION.md`](ps4/d1-server/ATTRIBUTION.md).
- **shadNet and the rpcsn Project**: the PSN-replacement protocol. Its schema is vendored verbatim in
  `ps4/d1-server/proto/`.
- **[RPCS3](https://github.com/RPCS3/rpcs3)**, by the RPCS3 project and its contributors (GPL-2.0).
  It carried the earlier PS3 route. The `patches/rpcs3-*.patch` files are changes to RPCS3's source,
  and its contributors keep their copyright.
- **[cohaereo](https://github.com/cohaereo)**: the
  [rpcs3-v4nguard](https://github.com/cohaereo/rpcs3-v4nguard) fork (GPL-2.0). Its Destiny network
  and texture-cache work showed the PS3 route was possible ([`notes/ps3-route.md`](notes/ps3-route.md)).

## Destiny research

- **[MontagueM](https://github.com/MontagueM)**: [Charm](https://github.com/MontagueM/Charm)
  (GPL-3.0).
  - Its Destiny 1 tag schemas for activities, bubbles and map data are cited in
    [`notes/public-research.md`](notes/public-research.md).
  - Its `Tiger` library, with a one-line local fix, reads build 56578. `tools/charm_probe` uses it
    to list named tags and activities and to name all 8,609 inventory items
    ([notes](notes/charm-56578.md)).
- **[v4nguard](https://github.com/v4nguard)**: [tiger-pkg](https://github.com/v4nguard/tiger-pkg)
  (MIT). Its `d1_legacy` reader gave the package layout that `scripts/d1_pkg.py` follows.
  [quicktag](https://github.com/v4nguard/quicktag) (GPL-3.0), its package and tag explorer, is built
  locally for browsing build 56578's tags ([related projects](notes/related-projects.md)).
- **[nruble](https://github.com/nruble)**: the [Cryptarch Guide](https://github.com/nruble/cryptarch-guide)
  (GPL-3.0). Its archived D1 manifest gives the Tower vendors' map positions, from which the
  quartermasters' and Banshee-44's world positions are estimated
  ([vendor notes](notes/ps4-56578-vendor-purchase-2026-10-30.md)).
- **[Blam Network](https://github.com/Blam-Network)**: [tiger](https://github.com/Blam-Network/tiger),
  TypeScript libraries for Destiny's Tiger engine (MIT for the code packages).
  - Its `gibbon` reader, with definitions for our exact build `56578.17.04.12.1251.evil`, names
    every row of every activity bundle: `tools/ps4_setup/d1-bundle-rows.mjs` uses it to map each
    sensor index the host serves to its authored name.
  - Its Fukugen Daihon reconstructions of the mission scripts (`enc_breach_m0`,
    `act_cosmo_m0_highway`) show which squads each Breach trigger places.
  - Its dictionary maps bundle hashes to names (`0xFC0E45A3` = `enc_breach_m0`). It also names
    the 2014 build's message ids.
  - Its independent `rsat` codec cross-checks ours byte for byte (`tools/rsat_crosscheck`,
    [notes](notes/rsat-blam-crosscheck.md)).
  - See [`notes/ps4-56578-breach-sequences-2026-10-31.md`](notes/ps4-56578-breach-sequences-2026-10-31.md).
- **[lizardpeter](https://github.com/lizardpeter)**:
  [destiny-1](https://github.com/lizardpeter/destiny-1). It documents the v24 package format and the
  activity, bubble and map graph, cited in the research notes.
- **Bungie engineers' public talks**, used to understand the activity-host model:
  - Justin Truman, "Shared World Shooter: Destiny's Networked Mission Architecture" (GDC 2015).
  - David Aldridge, "I Shot You First: Networking the Gameplay of Halo: Reach" (GDC 2011).
- **[isinternets](https://github.com/isinternets)**: [Dawn](https://github.com/isinternets/Dawn), a
  Destiny 2 server emulator (GPL-3.0). It was read as a reference only, never run or copied. Its
  New Light mission is Destiny 2's remake of A Guardian Rises, and it suggested mechanisms to check
  on 56578 (squads, dropships, checkpoints); see
  [`notes/ps4-56578-continuation-2026-10-27.md`](notes/ps4-56578-continuation-2026-10-27.md).
- **The people behind [RPCS3 issue #12802](https://github.com/RPCS3/rpcs3/issues/12802)**. They
  showed Destiny reaching character select against private servers.

## Other games' revival servers (read for design)

These servers rebuilt online games from the retail client. They were read for their designs (spawn
phases, vendor ledgers, sessions), not copied; see [`notes/similar-projects.md`](notes/similar-projects.md).

- **[Crypto137](https://github.com/Crypto137)** and contributors:
  [MHServerEmu](https://github.com/Crypto137/MHServerEmu) (Marvel Heroes, AGPL-3.0).
- **[NexusForever](https://github.com/NexusForever)**: [NexusForever](https://github.com/NexusForever/NexusForever)
  (WildStar, AGPL-3.0).
- **[spaceninjaserver](https://github.com/spaceninjaserver)**:
  [SpaceNinjaServer](https://github.com/spaceninjaserver/SpaceNinjaServer) (Warframe, AGPL-3.0).
- **[PocketRelay](https://github.com/PocketRelay)**: [PocketRelay](https://github.com/PocketRelay/Server)
  (Mass Effect 3 multiplayer, MIT).
- **[mixutin](https://github.com/mixutin)** and **[SyST3MDeV](https://github.com/SyST3MDeV)**:
  [Dauntless Revived](https://github.com/mixutin/dauntless-revived) and
  [Undaunted](https://github.com/SyST3MDeV/Undaunted) (Dauntless, AGPL-3.0).

## Tools used (not bundled)

- **[maxton](https://github.com/maxton)**: [LibOrbisPkg](https://github.com/maxton/LibOrbisPkg)
  (LGPL-3.0). It reads the PS4 packages and their file system, through a locally fixed build.
- **[SocraticBliss](https://github.com/SocraticBliss)**:
  [ps4_unfself](https://github.com/SocraticBliss/ps4_unfself) (GPL-3.0). It unwraps SELF files.
- **[cohaereo](https://github.com/cohaereo) and nblock**:
  [tiger-pkg](https://github.com/v4nguard/tiger-pkg) (MIT), published by v4nguard. It reads the
  Tiger engine's packages. Its examples (`unpack`, with version `d1_roi`) read build 56578's PS4
  packages, which made the game's UI content readable ([notes](notes/tiger-pkg-56578.md)).
- **The [GoldHEN](https://github.com/GoldHEN/GoldHEN) team**: GoldHEN. Its FTP server decrypts
  SELF/sprx files on read, which made dumping from an owned console straightforward.
- **[Ghidra](https://github.com/NationalSecurityAgency/ghidra)** (Apache-2.0), for analysis, and
  **[Capstone](https://www.capstone-engine.org/)**, for disassembly.
- **[FFmpeg](https://ffmpeg.org/)**, for capture and video work.
- **Oodle, by RAD Game Tools**: the user-supplied decompression library the package readers load. It
  is not included here.
- **Python packages:** [cryptography](https://cryptography.io/), [NumPy](https://numpy.org/),
  [Pillow](https://python-pillow.org/) and Capstone's Python bindings, listed in `requirements.txt`.

## Contributors

- **[p3n1cillin](https://github.com/p3n1cillin)**: project lead.
- **[kaelingray](https://github.com/kaelingray)** wrote:
  - d1-bap wire-boundary tests for matchmaking, character lifecycle, activity-host notifications and
    loot/item grants;
  - Windows CI for the Python tests;
  - the d1-oracle test guards.
- **[DeRveXx](https://github.com/DeRveXx)**: pad testing on the PS4 route, and the trigger deadzone
  for Hall-effect pads (`shadps4-input-kbm.ini`).
- **Dependabot**: dependency updates.
- **AI assistance:** much of the code and documentation was written with AI assistants, under the
  maintainer's direction:
  - Anthropic's Claude. Its commits are authored as Claude or carry `Co-Authored-By` trailers.
  - OpenAI's Codex, for audits and research passes. Its work is named in the commit messages.

## Rights holders

Destiny is © Bungie, Inc., originally published by Activision. D1 Reimagined is an independent
preservation project and isn't affiliated with or endorsed by Bungie, Activision or Sony. No game
files are distributed here.

Missing someone, or got something wrong? Open an issue or a pull request and it will be fixed.
