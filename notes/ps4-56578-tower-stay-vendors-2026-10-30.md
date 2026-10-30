# PS4 56578: staying in the Tower, and where vendor screens stop (30 October 2026)

Run sense63 on the owned 56578 image, served from `main` (including #102). Private lists, captures and media
stay outside the repository. Times are UTC.

## The Tower no longer sends the Guardian to orbit

The return to orbit was the client's own inactivity timeout (`peer_leave_request` reason `0x4ca36d9a`,
`error.inactivity_timed_out`; [29 October](ps4-56578-tower-evening-2026-10-29.md)). The client takes the
older of two timestamps, the last button press and the last change of stick direction, against a limit of
15 minutes in PvE activities (60 in social activities), with 60 seconds' grace. Walking alone never resets
the button timestamp, which is why the earlier walking keep-alive was still kicked.

With a keep-alive that also presses a button each minute, sense63 stayed in Tower North for almost two
hours (10:07–12:03). A window of 23 minutes (10:42–11:05) had nothing but the keep-alive. As a control,
both the keep-alive and the lab's five-minute idle press were then stopped. The client sent the Guardian to
orbit exactly 15 minutes after the last button press (12:03:47, reason `0x4ca36d9a`). Until then the only
`peer_leave_request` in the run was the CONTINUE departure (`0x03cfbb27`).

## Where a vendor screen stops

Pressing □ at the emblem kiosk ("Access Emblems", vendor `0xc4c8e846`) was traced live with execute
breakpoints:

| Step | Code | Result |
|---|---|---|
| Vendor dialogue action runs | `0x5c5a00` (reads the vendor and screen hashes at definition `+0x280`/`+0x284`) | hit |
| Its entity gate | `0x5c5a41` | passed |
| `ui_launch_vendor_screen` body | `0xa96110` | entered |
| Vendor found in the investment vendor table | `0x58ab80` → `0xa9618a` | passed (index 40; the table is content `80a5ffcf`, 124 rows) |
| Screen created for hash `0xc2ddef93` | `0x97ec70` → `0x983000` → `0x97faf0` → `0xa8dcb0` | **null** |
| `904 vendor_interact` sent | `0x552ef0` | not reached |

So no request reaches the server because the client cannot build the vendor screen, not because of
missing server data. The screen lookup (`0xac6190`) searches the loaded UI packages. Screen `0xc2ddef93`
is registered in package `0x8085a000`, but with **no variants**: a name with no definition. The same holds
for the default vendor screen `0x0d89e1c7`, for 7 of that package's 17 screens, and for 20 of
`0x80830000`'s 42.

Which UI packages are loaded is chosen every frame (`0xac4ca0`) from a mask table indexed by the world
controller's UI context (`+0x80`). The masks are `0x1 0x5 0x2 0x1a 0x2a 0xaa 0x6a 0x12a 0x22a 0x42a`. In
the Tower the context is 7 (`0x12a`: four packages). Contexts 5 to 9 share a core and differ by one bit,
that is one context-specific package each.

**What sets the context.** A write watch on the world controller (`[0x26ca0b0]`) through that
inactivity return caught `+0x80` going 7 → 5 (orbit) in `0x7887d0`. That function logs
"world_controller:ui_stage: Stage-loading goal changed from '%s' to '%s'". Its neighbour `+0x84` is the
substage (43 in the Tower, then 19, 18, 34 while leaving). The stage updater (`0x788e80`) picks the
loading stage from how the activity was launched. A byte in the launch descriptor (`0x545ce0`) gives the
matchmaking type, and its log strings name the cases: "NOT matchmade" (cinematic UI), "**TOWER
matchmade** (special tower matchmaking UI)" and plain matchmaking. A byte at `+0xa9` of the activity's
definition (`0x545230`) is checked too.

**Working hypothesis:** the vendor screens' definitions are in the package of the Tower's own context.
CONTINUE launches the next story activity (165, through matchmaking message 42), and the host serves
Tower North as that activity's world, so the client never enters the Tower's context. The next tests are to read the
launch descriptor's matchmaking type during CONTINUE, to compare activity 165's definition with the
Tower's own activity, and to launch the Tower from orbit.

## Also

- The hangar fight passed again: the lab bot's tools now finish within nested time limits and release
  every key afterwards. A tool killed mid-press had left a button held, and with a later press that cast the
  Super.
- Tested, no new findings: the vendor table is content, not server data. The hashes read live match the
  owned table exactly.
