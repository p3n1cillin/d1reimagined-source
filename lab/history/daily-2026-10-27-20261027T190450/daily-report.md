# 27 October 2026 - the opening mission reaches and crosses the Divide

Candidate public update: The opening mission can now replay automatically into
the Divide. The Guardian can reach its far gate and encounter Fallen there.
Combat balance and the next section remain under development.

Private production input only. Publication and final-video work are not approved.

## Reporting window and source checkpoint

Reporting date: 2026-10-27, Europe/London (GMT). Coverage is the reporting window
2026-10-26T19:00:00Z through 2026-10-27T19:00:00Z. Verification/preparation time is
2026-10-27T19:04:50.083456+00:00. Native tests begun after the cutoff retain their actual dates and roll
into the next eligible edition. This is the one consolidated daily engineering
report; the earlier 16:39 package is referenced as an event, without its media.

The public source checkpoint is 7cc9a8eb1aee5d2c9d06f6641ccfe5d794c25adc, branch ps4/divide-gate.
Its observed status is clean. PR40 (merge 1a515db) and
PR41 (merge 0326421) are merged; their configured GitHub Python, Rust Linux/Windows,
input and automation checks were independently read as successful today.
The later gate/slice-104 journal is commit7cc9a8e on its own branch.
Automatic report-tool Git fields describe the legacy capture checkout; the
explicit tested/reporting source fields in metadata identify the public source.
Runtime native tests used privately supplied owned build56578 and existing lab
patches. No stock-client or complete-game acceptance is implied.

## Engineering progress and actual native evidence

1. The objective presentation fix from PR38 is retained. The authored title,
description and marker are selected without the unwanted mission banner.
The verified schedule is promoted to stable private filenames; its final step
waits for the natural slice96 crossing and carries the local authority handover.

2. The temporary route scripts were consolidated into the canonical replay,
with a longer gameplay wait, settling/scanning controls, bounded movement,
rifle tolerance1.0, an actual trigger35 postcondition and host acceptance for
the garage control. Early run18 failed tracking and later stalled on the Wall
route; it stopped rather than sending subsequent route input. These failures
are not erased by the later success. Subsequent engineering hardened the
replay/control defaults and recorded the successful fresh run19.

The run19 record reports every leg from spawn to the Divide completed without
intervention. Its replay log ends with completed garage and fan routes and
DIVIDE REACHED. Independent host-log checks confirm trigger35 at
16:20:26.446658Z and the natural slice96 crossing at16:32:32.690354Z, followed
immediately by scheduled step16. The source journal records the correct
FIGHT THROUGH THE FALLEN presentation. The later geared run20 independently
reports trigger35 at18:07:29.194376Z and slice96/step16 at18:21:41.349762Z.

3. The Divide's owned firing-area container was identified and compared with
loaded native memory:18 name/bounds records,15 fa_ label strings, three prop
positions and selected code blocks matched. These areas are not a collision
map. Later traversal established the southern low route through the area
centres to the exit, instead of the walled starter pocket. A 57-point trail
was recorded, but replaying its exploration zig-zags still drifts into the
pocket; the area-centre chain is the presently reliable diagnostic approach.
The earlier assertion that a chasm prevented crossing is superseded by this
successful low route. The exact cause of the earlier apparent death remains
an inference, not a proved fall or renderer fault.

4. Ground Fallen do engage near the exit. The source journal records visible
damage, a Vandal nameplate and a Guardian-down screen naming a Wire Rifle.
Run20's explorer log independently ends at(146.12,-608.11), near the gate.
The journal records a later walk through the open doorway to(140.1,-609.1),
and a target transition toward slice104 before a death/respawn. Targeting104
is not evidence that the next region loaded or that Find the Ship completed.
There is no new verified enemy kill or mission completion in this report.

5. Steering now has an optional native-facing input alongside the older
movement-derived heading. The source record reports agreement with a clean
forward move and a successful partial route replay. This improves a specific
steering case; it does not make the full57-point trail reliably replayable.

Gantry and exit-gate creation diagnostics earlier today produced live entities
through host-served states. They did not establish the staged ship flyover,
authored animation sequence or a fix for the starter obstruction. Prop positions
place the gate near the far exit, away from that obstruction.

## Validation and limits

- PASS: the earlier local script suite,199 tests; server suite225 tests ran,
  with224 passed and one skipped. Later focused replay failure controls:12 passed.
  These counts identify actual earlier runs, not a newly rerun final aggregate.
- PASS: configured CI on the final heads of merged PR40 and PR41, checked today.
- PASS: native log postconditions and bounded replay/explorer log checks above.
- PASS: selected owned firing-area/prop/code comparisons against native memory.
- NOT FULLY PASSING: the earlier broad oracle library attempt was40 passed,
  seven failed: six lacked the reference executable and one lacked the separate
  build-specific item table. No substitute executable or waived input is claimed.
- NOT ACCEPTED or still unverified: full retail/stock boot acceptance, full mission completion, verified
  enemy kills, persistence/reconnect, performance benchmark, combat balance,
  correct25/184 ammunition, correct time progression and staged flyover.

The tutorial rifle grant remains a known investigation input: content index3598,
level40. A private one-variable level1 candidate was encoded through the selected
client codec; the level40 baseline reproduced the served bytes and both variants
round-tripped identically. The candidate was not served in the preparation test
and has no native ammo acceptance. Empty per-instance stat arrays are measured
content data for this item; they must not be filled with invented values. The
separate translated weapon-stat pairs still need investigation.

## Maturity and next acceptance

Unattended spawn-to-Divide replay and the scheduled objective handover are working
in the tested contained lab scope. The crossing and exit fight are partially
working: the route is known but its long recorded trail is not robust, and
Vandals/Dregs still kill the Guardian quickly, including the geared fixture.
The full mission, combat balancing, drop staging and the next region remain open.

Next: aim at visible enemy nameplates/body positions rather than radar alone;
establish a verified kill and encounter completion. Preserve the low area-centre
crossing, then verify the actual slice104 entry and authored Find the Ship handover.
Test the starter weapon-level candidate as a controlled separate experiment.
Keep garage opening tied to its eventual encounter result, and verify time/night
and ship sequencing independently.

## Evidence index - reuse the earlier package

Referenced event package: <lab-pc>-20261027T163920-483d90b903ad47e69f966f0c5fb71f27. Its9 payloads,226254428 bytes, were
independently re-hashed against its existing manifest today. Manifest SHA256:
a7896a5c4ee74cd55f7b8a9cc3b7e61afc7fbd58e917fa76ee3508f1840d546c. It remains immutable. None of its media is resent.

- clips/clip01.mp4: garage event16:29:40Z at offset5s; fan-tunnel event16:30:30Z
  at offset50s, as indexed by its existing events.json.
- clips/clip02.mp4: Divide entry16:32:39Z at offset19s; night scene16:33:10Z
  at offset50s. These show the opening milestone, not the later crossing/fight.
- clips/clip03.mp4: Nav Mode objective16:34:34Z at offset6s.
- screenshots/01.png through03.png: existing candidates in that earlier package.

The earlier event report's 'no Divide fight' and 'crossing not found' limitations
describe its earlier checkpoint and are superseded by the later facts above.
Do not edit that ready package or present its early limitations as current.

This consolidated package includes text/JSON only. It adds no new video, audio
or screenshot. Media integrity was checked; playback/audio/full-decode were not
rerun by this handoff, and no new visual review or publication approval is claimed.
The earlier metadata states game audio/no microphone and requires manual review;
production must inspect picture, music/audio, rights and privacy before using it.
Private diagnostic screenshots elsewhere include desktop/remote-session overlays
and are not supplied as clean public imagery. There is no new before/after clip
or measured performance comparison.

## Delivery and active engineering

Use the existing content-capture report command and durable local outbox, with
per-payload NAS verification and the uploading-to-ready transition. Transfer times,
free capacity, hashes and pending state are recorded in the separate private
receipt. No instruction file, owned executable, key, extracted asset or raw run
trace is packaged. No final video is edited and nothing is published.

The native game remains active in run20. This handoff sends no input and stops
no process. A brief read-only safe-checkpoint inventory is required before the
small text-only transfer; active route/combat probes are allowed to finish.
Post-cutoff test results are reserved for the next eligible daily report.
