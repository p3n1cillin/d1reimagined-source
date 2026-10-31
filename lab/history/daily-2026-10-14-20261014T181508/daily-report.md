# 14 October 2026 — activity launch and an armed Guardian on served data

Candidate public update: The map can now open activities with an armed character,
and a crash when leaving an activity has been addressed. Mission encounters and
complete gameplay remain unfinished.

Private production evidence only. No publication, edit, narration or release is
approved by this package.

## Dates, source and evidence boundary

Reporting date 2026-10-14, Europe/London. Interval:
2026-10-13T18:00:00Z through 2026-10-14T18:00:00Z (19:00 UK cutoff).
This includes the previous evening's late evidence at its original date, not as
work newly done today. The 13 October report is immutable and referenced below.
The pre-cutoff gameplay record is frozen at 575358ba33b96acf5e666b8d60c1edb54dfc3d09 (master). The observed
worktree at preparation is 080ba131ca9b8fc4dfb3f17a6df29c51a23804cb; it contains a pre-existing untracked file `3`
and uncommitted handoff-tool changes listed in the delivery appendix. Those are
not a claim that a clean commit alone reproduces the native binaries/fixtures.

Active game tests take priority. This handoff sends no input, reads/writes no
live game memory and stops no emulator or server. Post-cutoff automatic-team and
subsequent world-table work is excluded and rolls to the next eligible report.
Delivery may occur after the cutoff at a safe checkpoint.

## What changed since the previous report

**13 October after cutoff:** the recorded investigation moved from forced spawn
to serving the pending sensor/replica and player-bundle data. The body and camera
could then be created without the former spawn/getter/reject/fade overrides.
Later gear fixtures made armour and the emblem render, while weapons were still
missing. These are late 13 October observations, carried into this edition.

**Weapon creation:** the apparent absence of a weapon-creation trigger was a
data problem. The client's player-loadout path does run. A previously zeroed
u16 in each equipment element selects a weapon object-definition record. Zero
selected the same non-weapon object for all three slots. Reading the per-item
definition and serving the correct indices (12, 13, 39 for the staged loadout)
makes the descriptors resolve to weapon objects. The regenerated roster change
is five bytes. The source record reports two R2 presses lowering the magazine
3 to 1 with recoil in armed4. The inspected original still shows the hand cannon
and HUD value 1/35; it does not independently prove the input sequence, damage,
reload, special/heavy use or combat correctness. This is a geared lab fixture,
not the authentic first-mission starting equipment or progression.

**Activity-end crash:** this corrects the previous report's provisional missing
entity-backpointer explanation. The Activity Host re-sent its full entity-index
grant on every state refresh, although the client consumes those bits as new
free slots. A live slot could therefore be allocated twice; freeing one record
left another live record pointing through an unset index. The repair sends an
empty grant on refresh, preserving the initial allocation. The source record
reports 68 tests passing, including a regression that refresh re-grants no indices.
In refreshfix it observes distinct slots 1–16, the same cleanup/free sequence
without the old fault, and the same process answering reads more than an hour
later. These are recorded results, not reruns by this handoff. Treat this as
acceptance of the reproduced refresh/teardown case, not a guarantee of general
emulator stability.

**Director and launch:** build-correct sparse unlock pairs and dense profile
completion flags open the Director and its activity nodes through served data.
Private Match can be selected and launched with ordinary pad input, spawning
an armed Guardian without the old launch poke, forced task mask, team poke,
suppress loop, spawn write or unfade. The responder records the launched activity
hash; the Activity Host maps it to the later build's activity index per join.
This avoids the old 01.29 default being mistaken for the later build's activity.
The world itself remains a host setting; map selection is not generally followed.

**World coverage:** per-world checksum and sensor-list capture supports a
configured Bannerfall world and the Cosmodrome highway. Bannerfall is very dark;
the cause is unresolved. These are bounded world-entry observations. The host
does not serve encounters and the worlds remain empty of accepted mission logic.

**Story UI to world:** served Earth flags make The Dark Within focusable and
launchable. The early dome_a guess stalled on a public start region that was
not connected. Excluding that region produced LOBSTER; checksum-probe failures
produced deliberate LIME. The successful test configures the highway world;
its retail-correct association with this story and initial region still need
verification. Its player-bundle team must match the story player's -1 rather than a Private Match
player's 0. With the corrected served bundle, dw-highway2 goes through Earth →
The Dark Within → Restart → Launch to an armed Cosmodrome scene. Its driver and
walking sequence are source-reported; the included original shows the final
scene. This does not complete The Dark Within or A Guardian Rises.

**Retained lab interventions:** “no runtime memory writes” on the served route
does not mean an unmodified retail client. All inspected relevant run receipts
retain the synthetic lab-orbit patch (job-helper guard, local manifest setting,
diagnostics and the recorded setup/voice changes). nativeBootAccepted remains
false. The network, profile, roster, flags and activity authority are local
experimental implementations. No stock-client, production-service or full-game
acceptance is claimed.

## Validation, failures and maturity

| Result | Evidence and tested scope | Maturity |
| --- | --- | --- |
| SOURCE-REPORTED PASS | Equipment-object selector traced from per-item definition through loadout descriptors; served roster produces armed first person. armed4 records two shots (3 → 1). | Partially Working: bounded weapon creation/use, not combat or first-mission fidelity. |
| SOURCE-REPORTED PASS | Entity refresh now emits an empty delta; 68 lab tests pass including a dedicated regression. refreshfix survives the formerly crashing cleanup. | Partially Working: reproduced teardown fix. |
| SOURCE-REPORTED PASS | served5: Director → Private Match → armed spawn without runtime state pokes; follow: AH selects the launched activity instead of the reference-build default. | Experimental native lab behavior. |
| VISUALLY CHECKED | Screenshot 01: hand cannon and ammo 1/35; 02: focusable Earth story node; 03: armed Cosmodrome scene. Originals inspected for this private transfer and kept unchanged. | Still-image proof only; no motion/firing sequence or timing claim. |
| RECEIPT CHECKED | dw-highway2 starts 2026-10-14T17:52:14.7259529Z, ends 2026-10-14T17:58:11.0381214Z; owned-server cleanup true, emulator exit -1, no listed emulator fault or guest crash report. | Bounded closed run. Exit -1 is not asserted to be a normal clean client exit. nativeBootAccepted false. |
| KNOWN FAILURE | Two early served world loads faulted inside Zydis while the emulator decoded an unreadable guest instruction address. A fork guard records/refuses that access; no later reproduction is not a root-cause fix. | Open stability risk; no inferred crash-free rate. |
| KNOWN FAILURE | Intermittent incomplete Director despite matching flag-plane bytes; reboot recovered it. Cause unpinned. CHICKEN occurred in the retired job-hosted boot; separate-process boot worked, but the exact cause is unproven. | Known Broken / Experimental. |
| KNOWN FAILURE | Public-region entry, selected-map routing and the cinematic-world black screen remain unresolved. | Planned/partial broader content. |
| NOT RUN | Mission objectives/encounters, AI damage/kills, rewards, persistence/reconnect, completion, correct starter equipment, matchmade playlists and multiplayer synchronization. | Unaccepted. |

Do not turn a successful codec, manifest, screenshot or server response into a
claim that a whole mission works. The historical claim “the client never tries
to create weapons” is superseded by today's traced selector bug. The prior
teardown explanation was provisional; today's refresh-grant trace is stronger.
The old ready report is not modified.

## Evidence index and media handling

All three included PNGs are untouched window captures, 1296×759. They include
the emulator title and slivers of desktop/log text. They are private diagnostic
originals requiring downstream review/redaction. A possible game-viewport crop
is x=8, y=31, width=1280, height=720; no crop or other edit was made here.
No upscaling, captions, overlays, narration or designed imagery was added.

1. `screenshots/01.png` ← armed4/fired.png, original modification time
   2026-10-14T08:18:53.914971+00:00. Proves the visible weapon/HUD; not the
   two-shot sequence or enemy interaction.
2. `screenshots/02.png` ← earth/e3.png, original modification time
   2026-10-14T15:37:17.464414+00:00. Proves Earth nodes and The Dark Within's
   Resume/Restart UI; not a completed activity.
3. `screenshots/03.png` ← dw-highway2/W8.png, original modification time
   2026-10-14T17:56:40.762145+00:00. Proves an armed rendered Cosmodrome scene;
   walking and the preceding UI sequence are supported by the source record,
   not by this still alone.

Each original is fully decoded as an image, measured and pinned by size/SHA-256
before transfer. No video or audio is included. No usable closed PS4 video was
identified in the inspected staging scope, so video decode, frame rate, audio,
music and microphone checks are NOT APPLICABLE here. This is not fabricated
video footage, an end-to-end recording or a controlled before/after comparison.
Capture failure/absence does not invalidate the separately scoped test results.
Rights/publication approval remains downstream; private transfer is not clearance.

Private source references (not copied into the package):
- notes/ps4-56578-blockers-2026-10-14.md at 575358ba33b96acf5e666b8d60c1edb54dfc3d09;
- notes/ps4-retarget-2026-10-12.md for late 13 October context;
- cfw-staging/2026-10-14-refreshfix/ for the slot trace and local images;
- cfw-staging/2026-10-14-served5/ for the served Director/launch sequence;
- cfw-staging/contained-boot/20261014-175212-2026-10-14-dw-highway2/receipt.json.

Previous handoff: <lab-pc>-20261013T181812-ae94e24c41d043fda43b6500d0e35f5c. Its NAS payloads were re-hashed against its
immutable manifest at preparation. No earlier images or clips are resent.
These three images are new evidence for this reporting interval.

## Next engineering priority and concrete acceptance

Keep the activity launch and normal spawn route as the baseline. Continue into
the actual PvE mission layer: correct starting location/checkpoint, objectives
and waypoints, encounter creation, combat and server-owned progression. Prove
one complete objective/encounter sequence against the owned-game reference,
then reload/reconnect to the same checkpoint. Do not substitute an empty loaded
world or a different activity for mission completion.

Also reproduce activity return/relaunch after the entity-grant repair, trace the
unreadable-guest-address fault if it returns, and fix public-region connection
and per-launch world choice. Verify the per-activity team follow-up in its next
eligible edition; its current post-cutoff run is not counted here.

The original first mission, broader PvE content, social destinations, inventory
semantics, rewards and multiplayer are still open. Still images cannot establish
that flicker, distorted characters, dark lighting or Ghost glow are repaired.

## Delivery appendix — process evidence after cutoff, not new gameplay

The existing report command was extended narrowly to accept explicitly inspected
PNG originals. It copies them unchanged, image-decodes them and binds transfer
permission to their exact manifest hashes. Video and unreviewed additions remain
held; sensitive-information and publication-review flags remain true. The 85-test
capture suite passes, including replacement-image rejection, binary-image transfer
and the original unreviewed-video hold. Actual FFmpeg checks pass for these three
originals and reject an invalid PNG. These delivery checks occurred after cutoff
and are not presented as additional 14 October gameplay progress.

The changed, tested files are tools/content_capture/capture.py, test_capture.py
and README.md; they remain local/uncommitted, and nothing is published by this
handoff. Their byte fingerprints are in private context metadata. No unrelated
working files are changed. No policy/instruction file, credential, raw trace,
decrypted game input, binary, firmware or extracted catalogue is included.

One consolidated package, three new screenshots, zero clips/audio, no reused
media. Actual ready identity, timestamps, NAS capacity and per-file re-hashes
are recorded in the private delivery receipt. If transfer fails, the durable
outbox retains the same package for retry. Ready packages are immutable.
Engineering continues independently of production review.
