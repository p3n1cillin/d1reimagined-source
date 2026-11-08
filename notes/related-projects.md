# Related projects and what each gives this one (1 November 2026)

On 1 November the user asked to pull down, build and use other people's projects where they help,
with credit in [CREDITS.md](../CREDITS.md). These were checked out and, where useful, built outside
this repository (under `F:\d1\third-party`). Licences are their own: GPL code is used as a tool or a
reference, not copied in.

| Project | Licence | What it gives us | Status here |
| --- | --- | --- | --- |
| [Blam Network's tiger](https://github.com/Blam-Network/tiger) | MIT (code packages) | Definitions for our exact build 56578. The `gibbon` tag reader names every activity bundle row (sensor index). The Fukugen Daihon reconstructions of the mission scripts give the Breach's waves per trigger. Its `rsat` codec and `dictionary` message names are checked against ours; the `bullshcript` VM is a further candidate. | Built with Node 22. `tools/ps4_setup/d1-bundle-rows.mjs` uses it ([Breach notes](ps4-56578-breach-sequences-2026-10-31.md)); `tools/rsat_crosscheck` matches our codec 26 of 28 ([notes](rsat-blam-crosscheck.md)). |
| [tiger-pkg](https://github.com/v4nguard/tiger-pkg) | MIT | Reads our PS4 packages as `d1_roi` (needs your own `oo2core_3_win64.dll`). | Built; UI packages extracted ([notes](tiger-pkg-56578.md)). |
| [quicktag](https://github.com/v4nguard/quicktag) | GPL-3.0 | A graphical package and tag explorer for D1/D2. | Built (`quicktag.exe`); interactive, not scripted. |
| [Cryptarch Guide](https://github.com/nruble/cryptarch-guide) | GPL-3.0 | An archived D1 manifest (vendors, sales, items, unlock flags, activity bundles). | Its vendor map positions estimate the quartermasters' and Banshee's world positions ([vendor notes](ps4-56578-vendor-purchase-2026-10-30.md)). |
| [Charm](https://github.com/MontagueM/Charm) | GPL-3.0 | A Tiger datamining tool, mostly Destiny 2. Its D1 schemas are cited in [public research](public-research.md). | Built with a portable .NET 7 SDK. Its `Tiger` library reads 56578 after a one-line fix ([patch](../patches/charm-d1-strategy-null-guard.patch)); [`tools/charm_probe`](../tools/charm_probe/README.md) names 643 tags, 57 activities and 8,609 items ([notes](charm-56578.md)). |
| [DestinyDataminingDocs](https://github.com/MontagueM/DestinyDataminingDocs) | GPL-3.0 | Engine overview and tag format write-ups. | Read; background for tag work. |
| [Alkahest](https://github.com/cohaereo/alkahest) | GPL-3.0 | A Destiny 2 asset viewer that recreates the renderer. | Built. Its version selection is hard-coded to Destiny 2, but the tiger-pkg underneath lists 1,114 named tags on 56578. |
| [d1-manifest-build](https://github.com/DestinyItemManager/d1-manifest-build) | none stated | DIM's scripts that fetch the D1 manifest from Bungie's API. | Not run: needs a Bungie API key; the Cryptarch Guide's archive covers the same data. |
| [Dawn](https://github.com/isinternets/Dawn) | GPL-3.0 | A Destiny 2 server emulator; its New Light mission mirrors A Guardian Rises. | Read-only reference, already credited. |
| [Destiny 1 Preservation Project](https://gitlab.com/aknumbers/d1preservation) | GPL-2.0-or-later | The base of `ps4/`. | Already the foundation, credited. |

## Blam Network's mission scripts: what each part's licence allows (5 November 2026)

aknumbers (Destiny 1 Preservation Project) advised using Codie's (Blam Network's) mission-script work in full. Its two parts differ:

- **`packages/bullshcript`: MIT.** A best-effort reimplementation of BungieScript and EncounterScript, Destiny's mission-script languages: a compiler, decompiler and runtime VM. Usable with credit. It could replace our hand-written step schedules with the missions' own logic.
- **`packages/fukugen-daihon`: no licence.** About 1,190 restored D1 mission scripts, for builds from the 2013 alpha to release. Its README says the alpha's scripts were *decompiled from a leaked `release_internal` Bungie build*; the later builds' scripts are reconstructions.
  - Unlicensed means all rights reserved by its authors, and the decompiled scripts are Bungie's own code. Our rules allow neither ([licensing](../ps4/design/licensing.md), rule 2).
  - We already rely on it: the Breach's wave and squad data comes from its `enc_breach_m0` reconstruction (`tools/ps4_setup/d1-breach-waves.py`, `d1-member-states.py`).
  - **Update, the same day:** the project owner decided to use it in full, as a local external input only ([licensing](../ps4/design/licensing.md), rule 2's exception). Nothing from it is vendored or redistributed, and a licence is still being sought. It includes a restored set for **our exact build**, `v56578_17_04_12_1251_evil`: 380 files covering A Guardian Rises, the reveal and Tower cinematics, The Speaker and The Awoken, Restoration and the Tower.

## Codie (craftycodie) and Blam Network: what is public, and what each licence allows (5 November 2026)

Codie runs Blam Network and wrote the Xbox 360 plugin that recorded the live captures. Their public repositories, checked through each one's licence file or package manifest. Code without a licence is not read ([licensing](../ps4/design/licensing.md), rule 2); only its README was.

| Repository | Licence | What it is | Use here |
| --- | --- | --- | --- |
| [tiger](https://github.com/Blam-Network/tiger) `bullshcript` | MIT | BungieScript/EncounterScript compiler, decompiler and runtime VM | Usable with credit: the way to run missions from their own scripts instead of our step lists |
| tiger `gibbon` | MIT | Tag reader with definitions for build 56578 | In use (`tools/ps4_setup/d1-bundle-rows.mjs`) |
| tiger `rsat` (and [rsat](https://github.com/Blam-Network/rsat)) | MIT | RSAT codec | In use as a cross-check (`tools/rsat_crosscheck`); a reference for the native codec plan |
| tiger `datamine` | MIT | Bungie's event and datamine log format | Usable: event logs in the game's own format |
| tiger `poodle` | MIT | Oodle LZH decompressor for early Destiny packages | Not needed for 56578 |
| [cstruct](https://github.com/craftycodie/cstruct) | MIT | Declarative packed binary readers and writers (TypeScript) | Available; checked out under `third-party` |
| tiger `fukugen-daihon`, `dictionary`, `bullshcript-debugger` | none | Restored mission scripts (part decompiled from a leaked alpha), the name dictionary, a debugger | Used locally as an external input, never vendored (rule 2's exception) |
| [tiger_test_harness](https://github.com/Blam-Network/tiger_test_harness) | `UNLICENSED` (all rights reserved) | A test server for D1 builds from the 2013 alpha to 41375 (launch): sign-on, BAP, and an activity host running BullShcript mission scripts | Read and run locally as a reference (rule 2's exception), never vendored. Its README calls it a test harness, "not meant for production or as a foundation". |
| [Datamine-Explorer](https://github.com/craftycodie/Datamine-Explorer) | none | Desktop and browser viewer for Bungie event logs and datamine files | Its released app (or its browser version) may be run as an external tool (rule 4) |

The rest of both accounts is Halo, Minecraft and other projects.

Servers rebuilt for other games (Mass Effect 3 and others; no AGPL code is used or read) are
surveyed in [similar projects](similar-projects.md).

Searches that found nothing usable: a public reverse-engineered D1 server other than the above.
The shadPS4 compatibility tracker has two reports for CUSA00219 (issues 317 and 994); they were not
commented on.
