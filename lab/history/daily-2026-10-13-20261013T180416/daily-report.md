# 13 October 2026 — an experimental PS4 scene, with a confirmed crash

Candidate public update: An experimental run now reaches orbit and displays a game
scene. It still depends on temporary interventions and ends in a crash; stable
play remains unfinished.

This is private production evidence, not approved publication copy or a release.

## Reporting boundary

Reporting timezone: Europe/London. Date: 2026-10-13. Reporting begins after the
previous report's 2026-10-12T20:14:08Z evidence endpoint. The daily cutoff is
2026-10-13T18:00:00Z (19:00 UK). Source checkpoint at draft preparation: 67055ec47ad05e54787de8d78837ca66531dbc7a,
master; dirty worktree: false. The source note is a cumulative
engineering record. Dates below identify the actual test instants, not this
report's preparation time. No tests were rerun during this handoff.

The worldset native test was still running when the draft was prepared. It is
left untouched: no stop, input, memory write, build or video job is performed by
this handoff. The current live run cannot be counted as a completed passing run.
Delivery waits for its safe checkpoint; any new post-cutoff measurements belong
to the next eligible edition. This report covers already recorded evidence only.

## What changed

On 12 October the user changed the PS4 target from the unavailable exact 01.29
executable to the owned repack's actual executable, build
56578.17.04.12.1251.evil. Package metadata reports 01.33, but the build registry
does not infer an independently accepted APP_VER from that field. The 01.29
addresses remain a reference, never automatically valid on the later build.

The full repack extraction completed on 12 October at 21:49:37 UTC: 2,119 game
files, 52,738,749,800 bytes and 2,105 Bungie packages. That event follows the
previous report's endpoint and therefore belongs to this edition. The entry-table
metadata extraction retained 17 unencrypted sce_sys entries. Original inputs
and all extracted game files stay private and are not in this package.

The new client initially faulted before the title screen in the graphics heartbeat.
A recorded synthetic null-worker guard allows it to continue. Redirect and
Windows guest-IP changes enable local OAuth, SignOn and content transfer. The
experimental unsigned-manifest setting, skipped voice/profile gates and subsequent
in-memory interventions are explicitly synthetic ancestors. None proves an
unmodified client works.

Build-specific codec, schema and Activity-Host work corrected account/profile
checksums, completion-flag layout and changed unlock ordinals. Family 0/2 baseline
answers remove repeated subscription-validation complaints in a bounded orbit run.
Ten permission/DLC flags do not populate the Director: it remains empty.

An experimental forced-spawn route uses the hybrid roster and Activity Host
membership. The roster makes the character task finish without its former
suppress poke and creates the player slot. Further code hooks, a forced completion
mask, spawn-word write and fade/suppress overrides produce a first-person scene
on pvp_moonbase. The engineering record reports two seconds of left-stick input
changing the rendered view. Two still images exist locally; they are not continuous
video proof and were not independently visually reviewed in this handoff.

The forced-spawn run then crashes. The closed receipt records start
2026-10-13T16:51:45.5962672Z, end 2026-10-13T16:57:30.6168039Z, emulator exit code
-1073741819 (access violation), clean owned-server shutdown and
no guest crash report. Static analysis maps the null dereference to the entity
definition's vfunc 19, matching the documented structural failure of the older
forced path. The missing backpointer explanation is an engineering diagnosis,
not a verified repair. The receipt keeps nativeBootAccepted false.

## Tests and maturity

| Result | Actual scenario and scope | Maturity |
| --- | --- | --- |
| PASS | Test-D1BuildContent.ps1, completed 2026-10-13T07:27:24.4177702Z: all 2,119 extracted files match SHA-256; d1-pkg verify --blocks reports 2,105 packages / 964,151 stored-block hashes, zero failures. Acceptance receipt and verification tail were inspected for this handoff. | Fully Working: isolated content validation only. |
| PASS | Regenerated English manifest: 1,392 packages, 71,003 bytes, SHA-256 50f2b13866773d7143f65218c3205179147406ac3d32d50c536028d9de515bb0. It exceeds the cap measured on 01.29; the later-build cap is not independently established. | Fully Working: reproducible selected manifest; native fidelity unaccepted. |
| SOURCE-REPORTED PASS | Re-derived 56578 oracle object/message controls, vectors, Activity Host 34/34, sensor 11/11, roster 18/18, character 20/20, inventory 9/9, profile 12/12, Activity Host server 112/112. These are the source note's actual results, not reruns by this handoff. | Partially Working: tested codec/protocol scope. |
| SOURCE-REPORTED PARTIAL | Item-index controls 4/5; remaining check needs the absent catalogue. Earlier static eboot controls 21 pass / 40 fail are retained, not overwritten by later successes. Reference-dependent oracle/server unit failure sets remain explicit. | Experimental; incomplete full-suite acceptance. |
| SOURCE-REPORTED PASS | Orbit run 20261013-130110-later-56578-lab-orbit-c ends at its 14-minute limit without fault; answer-fam02 run stays in orbit for 12 minutes with zero subscription-validation complaints. Synthetic client patches remain active. | Experimental native lab behavior. |
| SOURCE-REPORTED PARTIAL | Launch-poke-b reaches activity:in_world, later returns cleanly to orbit on inactivity timeout; Windows lock prevents an in-world visual claim. Watched rerun is black because no player/camera is constructed. | Experimental; state transition is not scene/playability acceptance. |
| FAIL | Forced-spawn run displays a scene and a documented input response, then access-violates and exits by itself; stopped servers verified by its closed receipt. | Known Broken: stability. Experimental: synthetic scene/control. |
| SOURCE-REPORTED FAIL | Served udt-5 phase frames reach handler/decoder/walker, but post-pass/setter counters remain zero and lifetime phase remains -1. Worldset snapshot shows 97 entries in three groups with no lifetime-class record. Outer-loop gating remains unresolved. These counters do not prove an equivalent 01.29 run passed. | Known Broken: served spawn; cause partly inferred. |
| NOT RUN | Natural Director activity launch, unmodified spawn, mission objectives/combat, progression, inventory persistence, fireteams and complete-game acceptance. | Planned or unaccepted. |

The later build uses an isolated lab adapter and Windows-native local services.
The forced-spawn receipt records all its containment/input identity checks passing,
including the exact executable, applied redirects and no default lab route.
That is scoped configuration evidence, not an independent full-network packet audit.
Earlier loopback-only transport and failed/unapplied patch runs remain failures.
Lock-screen hitch runs are confounded and cannot be used as protocol-fault proof.

## Evidence index

The source note and receipts are source references only; raw logs, binaries,
fixture byte streams, keys and native inputs are deliberately not copied.

1. notes/ps4-retarget-2026-10-12.md: cumulative build-specific observations, controls,
   bounded runs, failures, hypotheses and next measurement.
2. cfw-staging/2026-10-12/later-56578-content-validation-v2/acceptance.json: closed content-only acceptance; actual UTC
   completion is 13 October even though its parent directory is dated 12 October.
3. cfw-staging/contained-boot/20261013-165142-later-56578-forced-spawn/receipt.json: closed native failure, exact build/patch
   identity and owned-child shutdown.
4. cfw-staging/2026-10-13-op/64-unfaded.png: local still for the documented
   unfaded scene. Timestamp 2026-10-13T16:56:49.775241+00:00.
5. cfw-staging/2026-10-13-op/65-moved.png: local still after documented input.
   Timestamp 2026-10-13T16:57:04.765959+00:00.

No clips, audio or screenshots are included in this text-only package. Timecodes
are not applicable. Local screenshot pointers are not transferable media files;
their existence/hashes were checked, but visual, crop, rights and audio reviews
were NOT RUN here. Windows frame captures may expose desktop edges and internal
identifiers. No recording or media decode pass is claimed. No before/after video
pair was recreated. No prior package's media is resent.

Earlier package reference: <lab-pc>-20261012T201422-024b4f583aaa4b579f3e1357e1528b9d. Its three payloads were re-hashed against
the immutable NAS manifest during preparation and match. The previous Oct 12
daily report already references the Oct 11 edition; do not resend that chain.

## Remaining work and acceptance

Highest-impact engineering task: make the served activity/world-content route
establish the objects needed by the lifetime phase and normal player spawn.
First trace the walker's outer container-loop bound and post-pass preconditions,
then implement the missing server/content behavior using the existing codecs.
The worldset run is active at draft preparation and its later evidence is pending.
Do not force it to end for this handoff.

Acceptance requires a new recorded, bounded run with task 9/10 completing on
served data, the lifetime setter accepting the phase, a normally constructed
player/camera, reproducible input response and clean shutdown, without the
forced completion mask, reject NOP, spawn write or manual unfade. Director
content remains a separate missing gate. Entity-definition crash repair must
be reproduced in the same scenario and must not be inferred from quiet logs.

The whole game remains unfinished. No unmodified PS4 playability, first mission,
stable world gameplay or multiplayer milestone is claimed.

## Handoff state and private handling

One consolidated text-only report for 13 October. New footage: zero. Existing
event media resent: zero. Decode: NOT APPLICABLE. Actual transfer, NAS space,
full-payload re-hash and ready transition will be recorded in the private receipt.
The durable outbox preserves this report if transfer fails. Ready packages are
immutable; this report never changes an older package. Late live-test evidence
is excluded from this edition and must retain its original timestamps later.

All payloads are private production inputs and require review. No policy file,
internal instruction text, credential, personal account or game payload is
included. A private handoff does not approve publication, narration, editing,
video release, website updates or membership. Engineering continues independently.

Delivery checkpoint: the existing native launcher ended at 2026-10-13T18:03:32.9946970Z and its closed receipt records owned-server cleanup. This handoff did not stop it. Its post-cutoff results remain excluded and roll to the next eligible edition.
