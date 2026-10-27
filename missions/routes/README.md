# Experimental traversal routes

Original route descriptions support bounded local diagnostics against privately supplied inputs. They do not establish mission completion or full native acceptance. Inspect each scenario before applying input and stop only processes owned by that test run.

## PS4 build 56578 routes

The following files are lists of world-space `[x, y, z]` waypoints for
`tools/ps4_setup/d1-walk.py`. The walker starts at the nearest point; select the
next route only after checking the current position and scene.

| Route | Points | Coverage |
| --- | ---: | --- |
| `enter_the_wall.json` | 67 | Highway spawn to the Wall's interior, ending near `(354, -796, 14)`. |
| `breach.json` | 77 | Highway through the Wall to the bottom of the Breach platform stairs, near `(386, -802, 16)`. This overlaps the first route. |
| `breach_platform.json` | 15 | Stairs and platform approach, ending near `(401, -780, 19)`. Wait for the mission's Ghost, lights and barrier sequence before the rifle. |
| `breach_corridor.json` | 20 | Rifle position to the end of the east corridor, near `(469, -814, 18)`. |
| `fight_through_the_wall.json` | 54 | Rifle, east corridor, exit-sign room and girder hall to the container-hall frontier, near `(544, -828, 17)`. Includes the corridor route. |
| `wall_container_to_chest.json` | 22 | New supervised trail: north around the containers, beneath the steel braces and along the grating to the shotgun-chest placement near `(535, -737, 17)`. The chest itself is absent in the quiet fixture. Automatic replay pending. |
| `wall_tripmine_corridor.json` | 13 | New supervised trail: down the stairs, west through the low mine corridor and to the next junction near `(473, -722, 15)`. Automatic replay and mine-damage acceptance pending. |

The two new trails were recorded once by supervised pad input on 27 October.
The JSON describes positions only: it does not press crouch or express encounter
conditions. Enter crouch before attempting the tripmine corridor, inspect the
beams and stop before unknown geometry. The native test crouched through it and
stood again near `(490, -722)`; it did not establish mine destruction or damage.
Do not treat these trails as an automatic mission script.

These are recorded traversal aids, not collision maps. Camera calibration and
obstacle recovery can drift: stop a looping or diverging replay and inspect the
scene rather than counting skipped waypoints as progress. The 27 October replay
needed manual alignment at the corridor's north turn near `(466, -814)`.

The Wall stairs need the wider exclusion box `--avoid 330,-837,340,-812`.
Recorded Misadventure drops include the east edge of the exit-sign room near
`(495, -801)`, the south passage near `y = -827`, and the east end of the girder
hall near `x = 549`. These are examples, not an exhaustive safe boundary. Do
not head directly toward the distant objective through the platform's north
edge or assume a visible lower room has a safe landing.

Position tracking addresses belong to one emulator process. Generate fresh
tracking for each boot; after a death, verify the scene and refresh tracking
before resuming. A stale median can remain at the old position while the
Guardian has respawned. The explorer's saved lethal directions help avoid
repeating known drops, but its position-jump death heuristic misses some short
respawns and does not make heading probes or unknown moves safe.

The 26 October journal records the native route discovery. Quiet fixtures omit
Fallen so that navigation can be isolated; enemy encounters and their natural
mission handover require separate validation.
