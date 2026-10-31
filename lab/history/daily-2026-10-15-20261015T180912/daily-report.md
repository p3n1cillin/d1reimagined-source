# 15 October 2026 — tutorial opening and traversal, with objective progression blocked

Candidate public update: The first mission's opening can now reach the starting
area, and movement through the Wall has been exercised. Objective progression,
the starting equipment and lighting still need work.

Private production evidence only. No publication or release is approved.

## Reporting boundary and reproducibility

Reporting date: 2026-10-15, Europe/London. Interval: 2026-10-14T18:00:00Z through
2026-10-15T18:00:00Z (19:00 UK). Late 14 October work is included with its actual
date. Yesterday's immutable package <lab-pc>-20261014T193305-e7a2a8daaa4a4f7d9e50a3f454ddd3b4 is referenced, not resent.
Pre-cutoff source: 512c659de0db2ff2673c6b1dee87cad975b83301, master. Observed HEAD at preparation: 512c659de0db2ff2673c6b1dee87cad975b83301.
Dirty worktree: the three unchanged, previously tested content-capture PNG files
and the pre-existing untracked file `3`; none of that file's contents is included.
Generated fixtures, built local services and a separately patched emulator are
additional runtime inputs; the commit alone is not a complete reproducer.

Active target is owned PS4 CUSA00219 build 56578.17.04.12.1251.evil (package
metadata 01.33; the repack title displays 01.00). This is not an exact 01.29
acceptance claim. Relevant receipts retain the synthetic lab-orbit client patch
and nativeBootAccepted=false. Normal served-data operation without the old
runtime spawn/launch writes does not imply an unmodified retail client.
The handoff itself sends no pad input, touches no game memory, stops no process,
and does not start a native test. Later evidence rolls to the next edition.

## Engineering progress and corrections

**Late 14 October — activity follow, objective display and reconnect:** the host
can choose configured worlds and spawn-bundle teams per launched activity. A
served objective produced ENTER THE WALL on the HUD. The included original also
shows a marker; its correct target binding and full navigation are not accepted.
Crucially, that experiment launched The Dark Within while serving tutorial-world
objects at a default Skywatch spawn. It proved the objective display path, not
the actual tutorial. The user identified this content mismatch. Do not reuse it
as a correct first-mission demonstration or as proof of mission completion.

The reconnect investigation identified another concrete CHICKEN path: the first
BAP connection could abort, but account baselines were pushed only to connection
zero. The responder now selects the latest queuez_register connection. This
addresses that missing-baseline path, not every possible CHICKEN/network fault.
The orbit driver also previously read stale log lines; the boot script clears
the old emulator log. These are source-recorded changes, not fresh reruns here.

**Late 14 October / early 15 October — correct content and start regions:**
scenario objectives and Ghost text identify the Earth story worlds. The Dark
Within uses dome_a; the highway world belongs to A Guardian Rises. A private
region-8 stand-in can load the former, but does not reproduce its retail start.
Later region analysis identifies slice-set IDs (8*bubble + set), including
tutorial intro 113 and gameplay 112. Other Earth stories start in shared public
bubbles and still need a second public activity session. Loading a private
stand-in is not acceptance of those missions' correct starts.

**15 October — tutorial opening:** a synthetic geared profile with two completion
flags makes the client auto-launch activity 164, A Guardian Rises, from orbit.
This is not accepted character creation or a real new-character profile flow.
Correct intro-region sensor data, an absent-object sentinel instead of a zero
reference, and the player's identity in the cinematic cast allow the revival
cinematic to play. Serving the first-person wake-up and a host teleport with an
advanced transition token then reaches gameplay region 112 facing the Wall.
Run agr9 records revival -> wake-up -> teleport -> daylight highway without
runtime memory writes. Included stills directly show a cinematic scene and the
final first-person view; motion, complete cast/Ghost presence, audio, dialogue
and reference-perfect timing have not been independently checked by this handoff.

The first implementation used fixed 70/45/5-second steps. The later host reacts
to cinematic-stop or player-skip incidents (891/1726), retaining time fallbacks.
The source records agr17 reaching the highway at about +43 seconds with both
cutscenes skipped, versus about +120 seconds for the timed sequence. These are
recorded functional timings, not new measurements or an accepted whole sequence.
tools/ps4_setup/d1-agr-steps.py writes the incident-driven step files.

**Traversal and the current blocker:** tools/ps4_setup/d1-walk.py uses pad input
and read-only position tracking to follow the existing PS3 breadcrumb route in
the same coordinates. The source records agr17 inside the Wall and agr18 at
waypoint 66/67. Neither run yielded the expected Enter-the-Wall trigger incident.
An initial arm-clock hypothesis was tested with a future arm time and still
failed. The trigger's sensor owner is the local machine, unlike the host-owned
objective/sequence/cinematic records. Client-owned reporting/subscription is the
next lead, not a proven root cause. Movement incident 3403 is not accepted as the
objective event. The correct post-teleport run also lacks the objective HUD.

**Renderer work, partial and one reverted attempt:** source logs traced severe
darkness to texture-cache overlap failures that discard rendered images. The
retained patch expands compatible arrays/mip levels instead of recreating them.
Source image samples improved in one scenario, but no controlled before/after
pair or performance benchmark is supplied here. Colour and shadow rendering
remain visibly wrong; the included Wall interior is extremely dark. Serving the
world clock changes the sun/shadows but does not repair that issue.
Keeping a 3D colour-volume image through 2D overlap requests removed log errors
yet made gameplay nearly black in agr12. That attempt was reverted. Zero overlap
errors did not establish rendering correctness. Trace image writes/uploads or a
bounded frame capture before another cache change. No general lighting, flicker,
character distortion, Ghost-glow or renderer-stability fix is accepted here.

## Verification and maturity

| Check | Result and actual scope | Maturity |
| --- | --- | --- |
| agr9 opening | SOURCE-REPORTED: served cinematic states and host teleport reach the tutorial highway. Stills independently inspected. | Experimental / Partially Working; geared synthetic profile and patched client. |
| agr17 skip and traversal | SOURCE-REPORTED: skip incidents advance the host and the pad route reaches inside the Wall. | Partial; no continuous clip, objective progression unaccepted. |
| agr18 trigger probe | SOURCE-REPORTED FAIL: future arm time and completed route still produce no expected trigger incident. Local ownership is an investigation lead. | Known Broken. |
| Objective display | VISUALLY CHECKED: late-14-Oct ENTER THE WALL and marker are visible in the mismatched story/world experiment. | Isolated HUD path only; missing after correct tutorial teleport. |
| Closed native receipts | Independently read agr9/17/18 start/end, patched-emulator identity, serversStopped=true, no listed fault, guestCrashReport=null. All exit -1 and nativeBootAccepted=false. | Closed bounded runs; -1 is not asserted to be a normal clean client exit. |
| Colour-volume cache attempt | SOURCE-REPORTED FAIL / REVERTED after near-black gameplay despite removing overlap errors. | Known Broken; existing partial cache fix remains. |
| Full gameplay | NOT ACCEPTED: authentic unarmed start/rifle pickup, enemy AI, damage/kills, encounter sequence, mission completion, rewards and reload/reconnect persistence. | Missing/unverified. |

Native receipt times (UTC; never inferred from folder names):
- 2026-10-15-agr9: 2026-10-15T14:38:12.3804693Z -> 2026-10-15T14:48:32.4545335Z; exit -1; servers stopped True.
- 2026-10-15-agr17: 2026-10-15T15:57:35.6590531Z -> 2026-10-15T16:09:58.6481540Z; exit -1; servers stopped True.
- 2026-10-15-agr18: 2026-10-15T16:10:04.3303561Z -> 2026-10-15T16:18:49.3785147Z; exit -1; servers stopped True.

Reproduction paths recorded by the engineer: d1-lab-served.ps1 with the tutorial
profile, region-specific sensor lists and D1_AH_MISSION_STEPS; d1-agr-steps.py
--r113 LIST --r112 LIST --objectives LIST --out-dir DIR; skip requests by pad;
then d1-walk.py over missions/routes/enter_the_wall.json. The private generated
inputs are not copied here. The historical runs are not rerun for this report.
No new gameplay unit-test count is claimed. Capture tooling is byte-identical
to yesterday's 85-test checkpoint; those tests are not relabelled as today's
gameplay tests. Today's image decode and NAS checks run during packaging/delivery.

## Evidence, privacy and media limitations

Four new original PNGs, not footage. All are 1296x759 window captures with an
emulator/operator title and some desktop/log edges. They are private diagnostics,
not clean approved public images. Downstream review/redaction is required. A
possible viewport crop is x=8,y=31,w=1280,h=720; no crop, colour correction,
upscaling, captions, narration or other edit was performed here. The cinematic
has its own letterbox bars inside the viewport. The dark image must not be
brightened and presented as an actual rendering fix.

- `screenshots/01.png`, original modified 2026-10-14T20:49:28.450056+00:00: Late 14 October objective-path experiment: ENTER THE WALL is visible, with a marker. The launched activity was The Dark Within, mapped to tutorial content; this is not a coherent tutorial or accepted waypoint implementation.

- `screenshots/02.png`, original modified 2026-10-15T14:42:48.189762+00:00: A frame of the tutorial opening cinematic over the wrecked cars. Still proof of a rendered cinematic scene, not continuous playback, dialogue, Ghost presence or complete cutscene fidelity.

- `screenshots/03.png`, original modified 2026-10-15T14:45:56.889764+00:00: Armed first-person Guardian in daylight facing the Wall after the recorded opening/teleport. Hand cannon is a known incorrect tutorial starting loadout; the objective HUD is missing.

- `screenshots/04.png`, original modified 2026-10-15T16:09:07.107582+00:00: Dark interior walkway reached in the recorded pad-driven route. Traversal is source-reported; this still does not prove objective completion, trigger delivery or the route in motion.

Each selected original was visually inspected, then byte-pinned and fully decoded
as an image by the existing report command. No video was identified in the
inspected staging search (depth two), and no video/audio is included. Video
frame rate, continuous playback, music and microphone checks are NOT APPLICABLE
to these stills; game sound and full cutscene fidelity remain unverified. No
video is manufactured from intermittent screenshots. No actual paired before/
after footage is supplied. Private transfer is not rights/publication approval.

Earlier package <lab-pc>-20261014T193305-e7a2a8daaa4a4f7d9e50a3f454ddd3b4 remains immutable. Its six payloads were re-hashed
at preparation and none is resent. These four original images were not in it.
No raw trace, credential, extracted game input, executable or instruction file
is included. Source references are notes/ps4-56578-blockers-2026-10-14.md,
notes/ps4-56578-blockers-2026-10-15.md, the named tools and the receipt references
in private context metadata. Source notes contain hypotheses superseded by later
sections; the corrected scope is set out above.

## Next engineering checkpoint and handoff state

Prioritize the first mission's actual client-owned trigger path: determine the
report/authority/subscription required for Enter the Wall, then establish one
natural, exactly-once objective transition with its HUD/waypoint. Repeat with
both watched and skipped cinematics, and verify the checkpoint after reload/
reconnect. Preserve correct activity/world/region/cast identity. Remove the
geared starter loadout for an authentic unarmed opening before claiming first-
mission fidelity. Combat, encounters, rifle grant, rewards and the rest of the
mission remain unaccepted. Shared public-session entry and renderer diagnostics
are additional open work; avoid masking trigger failure with time-based progress.

One consolidated private report uses the existing durable outbox and verified
uploading-to-ready transition. Actual identity, transfer times, all hashes,
capacity and outbox state are in today's private delivery receipt. Failed
transfer retains the same local package for retry; ready inputs are immutable.
No final video, public upload, repository push or site change is made by this
heartbeat. Engineering can continue without any production response.
