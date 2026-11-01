# Related projects and what each gives this one (1 November 2026)

On 1 November the user asked to pull down, build and use other people's projects where they help,
with credit in [CREDITS.md](../CREDITS.md). These were checked out and, where useful, built outside
this repository (under `F:\d1\third-party`). Licences are their own: GPL code is used as a tool or a
reference, not copied in.

| Project | Licence | What it gives us | Status here |
| --- | --- | --- | --- |
| [Blam Network's tiger](https://github.com/Blam-Network/tiger) | MIT (code packages) | Definitions for our exact build 56578. The `gibbon` tag reader names every activity bundle row (sensor index). The Fukugen Daihon reconstructions of the mission scripts give the Breach's waves per trigger. The `rsat` codec, `dictionary` hashes and `bullshcript` VM are further candidates. | Built with Node 22. `tools/ps4_setup/d1-bundle-rows.mjs` uses it ([Breach notes](ps4-56578-breach-sequences-2026-10-31.md)). |
| [tiger-pkg](https://github.com/v4nguard/tiger-pkg) | MIT | Reads our PS4 packages as `d1_roi` (needs your own `oo2core_3_win64.dll`). | Built; UI packages extracted ([notes](tiger-pkg-56578.md)). |
| [quicktag](https://github.com/v4nguard/quicktag) | GPL-3.0 | A graphical package and tag explorer for D1/D2. | Built (`quicktag.exe`); interactive, not scripted. |
| [Cryptarch Guide](https://github.com/nruble/cryptarch-guide) | GPL-3.0 | An archived D1 manifest (vendors, sales, items, unlock flags, activity bundles). | Its vendor map positions estimate the quartermasters' and Banshee's world positions ([vendor notes](ps4-56578-vendor-purchase-2026-10-30.md)). |
| [Charm](https://github.com/MontagueM/Charm) | GPL-3.0 | A Tiger datamining tool, mostly Destiny 2. Its D1 schemas are cited in [public research](public-research.md). | Not built: needs the .NET SDK, and its strength is D2 assets. |
| [DestinyDataminingDocs](https://github.com/MontagueM/DestinyDataminingDocs) | GPL-3.0 | Engine overview and tag format write-ups. | Read; background for tag work. |
| [Alkahest](https://github.com/cohaereo/alkahest) | GPL-3.0 | A Destiny 2 asset viewer that recreates the renderer. | Not used: Destiny 2 only. |
| [d1-manifest-build](https://github.com/DestinyItemManager/d1-manifest-build) | none stated | DIM's scripts that fetch the D1 manifest from Bungie's API. | Not run: needs a Bungie API key; the Cryptarch Guide's archive covers the same data. |
| [Dawn](https://github.com/isinternets/Dawn) | GPL-3.0 | A Destiny 2 server emulator; its New Light mission mirrors A Guardian Rises. | Read-only reference, already credited. |
| [Destiny 1 Preservation Project](https://gitlab.com/aknumbers/d1preservation) | GPL-2.0-or-later | The base of `ps4/`. | Already the foundation, credited. |

Searches that found nothing usable: a public reverse-engineered D1 server other than the above.
The shadPS4 compatibility tracker has two reports for CUSA00219 (issues 317 and 994); they were not
commented on.
