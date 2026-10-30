# Contributor checkpoint — 30 October 2026

The active work is PS4 CUSA00219 executable build 56578 (package 01.33; 01.29 remains the address
reference) on [the project's shadPS4 build](https://github.com/p3n1cillin/d1reimagined-shadps4). The
retained PS3 implementation targets BLES01857 01.24. **The complete game is not playable on either route.**
The results below come from bounded local runs with privately supplied inputs, not from an installable
release.

## Where the PS4 route stands

| Area | Observed | Still open |
| --- | --- | --- |
| Executable and content | Owned build 56578 (ELF SHA-256 `672f03411c0503dfbcda804cbb9fb27cdd9a8d73a8bcb71bf0c4f688073b0833`), selected through `ps4/capture-rig/builds.json`. 1,647 package tables and 701,164 block hashes pass. | Each contributor supplies their own inputs. Don't apply 01.29 offsets unchanged. |
| A Guardian Rises | Runs from the highway through The Breach, the Divide and Dock 13 to the hangar fight and the outro cinematic. The host drives each step from the client's own reports ([Dock 13](../notes/ps4-56578-dock13-2026-10-28.md), [outro](../notes/ps4-56578-outro-2026-10-28.md)). | Unattended combat, dialogue and tutorial fidelity; some cutscenes are skipped. |
| Rifle, ammo and stats | Service 601 and the profile deltas deliver and equip the starter rifle; it fires and kills. | Ammo and item-stat fidelity. Attack and Defense come from the item level curves ([#51](https://github.com/p3n1cillin/d1reimagined-source/pull/51)). |
| Post-game and CONTINUE | Rewards, 900 XP and the completion flag reach the post-game screens; holding △ CONTINUE launches the next activity ([29 October](../notes/ps4-56578-continue-tower-2026-10-29.md), [post-game](../notes/ps4-56578-postgame-2026-10-29.md)). | The next mission's own world: the Tower stands in for it. |
| Tower North | Loads populated: civilians, Patches and Xûr in their authored places ([evening notes](../notes/ps4-56578-tower-evening-2026-10-29.md)). The Guardian stays as long as there is input; the idle return to orbit is the client's own 15-minute timer ([30 October](../notes/ps4-56578-tower-stay-vendors-2026-10-30.md)). | Vendor screens: the client reaches its screen code but has no definition for the screen in the loaded UI packages, probably because it is in a story context rather than the Tower's own. The plaza and hangar regions. |
| Renderer | The fork's colour-LUT cache fix improves shadow detail and the lit Breach. | Wider scene coverage, characters, combined stability. |
| Contained local servers | The owned-run harness and local services support every run above. | Keep the Windows guest's containment and exact process ownership; test each launcher change separately. |

PS3: local sign-in, revival, waypoints, rifle grant and Dreg attacks have bounded native evidence. Still
open: activity disconnects, objective handover, ammo, kills, completion, and the renderer (flicker,
distorted characters, dark Breach, Ghost glow).

Later notes supersede earlier hypotheses; the dated journals in [`notes/`](../notes) keep the history.
Neither portable tests nor a quiet route replay establishes a completed mission.

## Places to start

No game files needed:

- [Good first issues](https://github.com/p3n1cillin/d1reimagined-source/labels/good%20first%20issue): small, well-scoped tasks, for example the remaining [d1-bap round-trip tests](https://github.com/p3n1cillin/d1reimagined-source/issues/4).
- Tests and CI: the workspace tests CI can't run yet (d1-oracle, d1-activity-host, d1-live's process tests) and portable build instructions.
- Review the Windows/WSL platform contract, the package ownership resolver and the recovery adapters with synthetic inputs. Keep the crypto, hash and build-identity gates intact.

With your own inputs:

- Vendors: find what the client needs before a Tower vendor screen opens. The vendor table itself comes from content, so the gap is likely server data.
- The next mission's world (activity 165), the Tower's plaza and hangar regions, and ammo and stat fidelity.
- The emulator: stability and rendering in [d1reimagined-shadps4](https://github.com/p3n1cillin/d1reimagined-shadps4).

Open a focused issue with the observed failure, source commit, proposed scope and validation, or ask on the
[Discord](https://d1reimagined.com/discord) first. Bring your own authorised inputs for native work, and never
upload them.
