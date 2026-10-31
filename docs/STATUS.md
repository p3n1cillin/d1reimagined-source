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
| The whole mission, reproducible | Everything a run serves is in the repository: the characters, the mission schedule and its sensor lists, the post-game pushes and the vendor setup ([`ps4/fixtures/56578`](../ps4/fixtures/56578/README.md)). [`lab/tower-route/ending-run.ps1`](../lab/tower-route/README.md) plays A Guardian Rises from boot to Tower North with nobody at the pad. Run sense76 (31 October) did it from the repository alone. | The unattended fight relies on memory reads of the running client. |
| Subclass abilities | The served Guardian (a Titan Striker) has its grenade, melee and super: the ability rows and perks come from the subclass talent grid ([#134](https://github.com/p3n1cillin/d1reimagined-source/issues/134)). Seen on the Moon: an arc grenade and melee, and sprint at 1.63× walking. The Guardian's helmet shows in the Tower now that the wear-helmet flag is served ([#128](https://github.com/p3n1cillin/d1reimagined-source/issues/128), run sense77). | Other subclasses, and choosing nodes. |
| Post-game and CONTINUE | Rewards, 900 XP and the completion flag reach the post-game screens; holding △ CONTINUE launches the next activity ([29 October](../notes/ps4-56578-continue-tower-2026-10-29.md), [post-game](../notes/ps4-56578-postgame-2026-10-29.md)). | The next mission's own world: the Tower stands in for it. |
| Tower North | Loads populated: civilians, Patches and Xûr in their authored places ([evening notes](../notes/ps4-56578-tower-evening-2026-10-29.md)). The Guardian stays as long as there is input. The idle return to orbit is the client's own 15-minute limit, which the server can now raise at sign-on (`D1_INACTIVITY_MS`, [idle timer](../notes/ps4-56578-idle-timer-2026-10-31.md)). Published as the Tower's own activity, it now loads as the social space: its menus load, the Armsday objective shows, and vendors show an "Approach" prompt. That works once two served unlock flags (rows 308 and 3391), which the Tower's unlock rule requires to be clear, are no longer served. Vendor screens open (Executor Hideo, Eva Levante), and with Glimmer served an emblem can be bought: the purchase request goes out, and the Glimmer and the emblem arrive once pushed ([purchase notes](../notes/ps4-56578-vendor-purchase-2026-10-30.md)). | Selling and refunds (903, 905), and purchases priced in marks. The next mission's own world (activity 165). The plaza and hangar regions. |
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

- Vendors: the server applies purchases (investment 901) by itself; selling (903) and refunds (905) are next ([notes](../notes/ps4-56578-vendor-purchase-2026-10-30.md)).
- Play or replay the whole mission: [`lab/tower-route`](../lab/tower-route/README.md), or `tools/ps4_setup/d1-agr-run.ps1` to take the pad yourself.
- The next mission's world (activity 165), the Tower's plaza and hangar regions, and ammo and stat fidelity.
- The emulator: stability and rendering in [d1reimagined-shadps4](https://github.com/p3n1cillin/d1reimagined-shadps4).
- [60 fps](https://github.com/p3n1cillin/d1reimagined-source/issues/112): the cap is lifted (`ps4/patches/CUSA00219-56578-lab-orbit-60hz.xml` with the emulator's `D1_FLIP_RATE=0`) at full game speed; heavy scenes run 34–38 fps, limited by the emulator's per-draw cost. Next: render interpolation above the tick rate, and the draw cost.
- [PC controls](https://github.com/p3n1cillin/d1reimagined-source/issues/113): the emulator fork has a detour framework; the menu cursor follows the PC mouse (`D1_MOUSE_CURSOR=1`), the camera turns with it in gameplay (`D1_MOUSE_LOOK=1`), the buttons click menus and fire and aim (`D1_MOUSE_BUTTONS=1`), and the game's own PC keyboard layer reads the keyboard (`D1_KBM=1`; W walks with W unmapped in the pad config). The engine's own keys are a developer layout (E reloads, R switches weapons); `D1_KBM_MAP` rebinds them, and `tools/ps4_setup/shadps4-input-kbm.ini` keeps the pad config from doubling them. Next: a default PC layout, aim assist, recentering and recoil at the mapped sites, and raw input instead of cursor re-centring.

Open a focused issue with the observed failure, source commit, proposed scope and validation, or ask on the
[Discord](https://d1reimagined.com/discord) first. Bring your own authorised inputs for native work, and never
upload them.
