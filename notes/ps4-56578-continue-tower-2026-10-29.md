# PS4 56578: from the post-game screens to the Tower (29 October 2026)

Runs sense54–sense57 on the owned 56578 image. Host and responder built from `main` (#81, #85). Private lists, captures and media stay outside the repository. Times are UTC.

## Result

After A Guardian Rises, the client now leaves the post-game screens on **△ CONTINUE**, launches the next story activity (165), and arrives in **Tower North** (`city_tower_default1`) with the Guardian standing in the courtyard and the HUD up (sense57, 17:34). Four server-side pieces get it there, in order.

| Step | What the server sends | What the client does |
|---|---|---|
| 1 | While the profile's current activity is still `ffff`: a profile delta with activity 164, **completed 0**, progression 117 = 0 | `0x56f185` sees the activity change and calls `0x574470`: the post-game XP collector goes to state 0 → target 2 (collecting) for activity 164 |
| 2 | Later in the mission: the same delta with progression 117 = 900 | `0x5747b0` records the sample only in collector states 2–3: XP triple 0 / 900 / 900 |
| 3 | After the outro: activity 164 **completed 1**, XP 900, and the completion flag **sub673 = 1** (with an item for the reward, #81) | The collector finalises (4/4, gained 900). Selection moves to 165. CONTINUE takes the ordinary recall branch |
| 4 | A world-table row for 165 with **the Tower's own sensor list** | The client leaves, launches 165 (kind-4 BAP 42, hash `6eaf0959`), joins, loads the Tower and spawns |

## Findings along the way

- **The XP collector needs a start before completion.** Introducing activity 164 with completed 1 in a single delta seeds and finalises the collector at once, so it records no gain (0/0/0 or 900/900/900, depending on order). An earlier "active" delta (completed 0) puts it in target 2, where XP changes are sampled.
- **CONTINUE's branch is a quest flag, not the input route.** `0xabd740` takes the ordinary leader recall only when `0x78e2a0` is false. That function tests unlock index 2838 (hash `2c39ab77`), which is profile completion sub673 (`+0x53c0+673 = +0x5661`). At **2** (inherited from the launch setup) the press arms the alternate path: holding past 3000 ms sends **505 signout_character**, and a shorter press does nothing. At **1** the ordinary path runs. The input route (`+0x114 == 37`) passes on its own.
- **The ordinary recall is hold-to-confirm** in a single-player session: the recall counter counts 3 → 2 → 1 while △ is held, and releasing early cancels it. Then `handler+31` fires (goal 27, reason 0), the client sends `peer_leave_request`, reconnects, and requests 165. The seven-second value at config `+0x378` applies only with two or more session members.
- **A missing world row means the host's startup world.** With no row for 165, a new activity instance kept its configured default, so the client loaded First Light (`pvp_moonbase`, scenario `80f08003`) instead (sense55).
- **Each world brings its own sensor groups.** With a Tower row and a template list in the Highway's common group `4786c0e0`, the Tower scenario loaded but the client sat at world state 34 on the loading screen, never answering the host's udt-5 frames (sense56). Read from the client, the Tower's pending list is **100 clients** in three groups:
  - `40aebf90`: the Tower common group, including 30 sim-player slots and the lifetime record at **kind 0x10, sub 2**;
  - `8d3f1166`: kinds 1, 2, 3 and 0x28;
  - `f2d88bd0`: two kind-6 cast rows.
  Served under the host's fixed lifetime sub 3, only 1 of the 100 was delivered. #85 lets the list's own lifetime row set the sub, and with it the client sends its sense update, reports incidents 566/3615/2014, and spawns (sense57).
- **The ring under the scoreboard row is the menu cursor**, not a loading indicator: it moves with the left stick.
- **The Advancement tab's missing XP bar is a display binding, not missing data.** Read before CONTINUE, the live `c_post_game_data_component` holds level 1 → 2 and XP gained 900 (`+0xbc`), with profile version `+0x628` = 5. "Upgrades: no results" is a separate list of equipped-item changes, empty because nothing was equipped.

## Not working yet

- **165's own world.** The owned record has no video and world tag `ffffffff`. The run maps 165 straight to the Tower, which skips that stage. This is a stand-in, not a recovered binding.
- **The Tower beyond arrival.** The `8d3f1166` world rows are served with default values, and the Tower's characters, vendors and movement are untested.
- The Advancement character bar's visibility, and the synthetic scoreboard values.
