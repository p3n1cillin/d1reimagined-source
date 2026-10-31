# Opening mission: fewer unwanted interruptions

Candidate public update: The opening mission now avoids an unwanted notice during
the rifle pickup and an extra mission banner at the Wall. Testing still shows
lighting problems and a later connection restart. Combat and the first mission
are still being restored.

## Private engineering summary

Reporting date: 2026-10-10, Europe/London. Reporting interval starts at the
previous consolidated handoff, 2026-10-09T12:57:17.9180128Z, and ends at 2026-10-10T18:10:53.856326+00:00.
Late evidence keeps its actual October 9/10 event dates; these are not all new
tests performed today. Current source master is 1d5b7e291220425e81c26197dda039aac9fa259e, clean and verified
against origin before packaging. Runtime under the native tests was the existing
01.24 BLES01857 C5 renderer, SHA-256
c5bc7f5786835302dafe3265549e2052d3265df7025fc997bc6249e3647fe7cf.

The urgent dialog interrupted controls after a successful rifle grant. Static
and read-only native inspection located the actual MOTD/urgent selectors:
universe flags 2059, 2065 and 2066. Earlier content-index and character-flag
overrides were negative experiments, not fixes. The launch default now marks
the three resolved runtime flags false. JOIN252's native trace has zero dialog
enqueues, and sampled game frames show the Khvostov toast and first-person rifle
without the dialog. The change was tested on bb55871 with an uncommitted launch
default change and documentation edits, then committed as 48c3992.

Incoming objective state 1 caused NEW MISSION / The Taken King: Prelude. The
mission now begins each objective in active state 5; the delayed waypoint step
stays. JOIN253 sampled video shows Enter the Wall then Find a Weapon without
that banner. The native run used 48c3992 plus the uncommitted mission change,
subsequently committed as 1d5b7e2. Current HEAD alone is not presented as the
original clean native-test revision. The post-grant banner is not accepted in
JOIN253 because that run reconnects before reaching the pickup.

Earlier late work: JOIN233-235's stat-array experiment still displayed a
12-round magazine and was not retained. JOIN236-240 show the urgent dialog
blocking the rifle trigger until dismissed. Aimed-fire attempts did not prove
a kill; hook-derived entity pointers are not a reliable enemy list. Recorder
handling was corrected after experiments showed an inactive game-window shortcut
and later an already-active recorder being toggled off. These are documented
observations, not additional native tests performed for this report.

## Tests and maturity

Fresh checks on current clean HEAD:

- PASS: `python -m unittest discover -s server -p 'test_*.py'` — 225 tests, 14.406 s.
- PASS: `python -m unittest discover -s scripts -p 'test_*.py'` — 176 tests, 9.369 s.
- PASS: both newly supplied original segments fully decode through the existing
  capture packager, contain audio, and retain their source SHA-256.
- PASS: earlier independent native closure receipts for JOIN252/253 verify
  7/7
  exact identities closed and three baseline hashes restored per run. Cleanup
  receipts retain XInput and shared RPCN, no active markers and no cleanup errors.
- NOT RUN for this handoff: a new native session, final-schema preflight,
  persistence/reload, a combat-kill acceptance test or a combined graphics test.

Notice suppression and objective presentation are Partially Working in the
bounded native scope. The automated tests are Fully Working within their own
scope. Ammunition fidelity, confirmed kills, first-person lighting and connection
stability remain Known Broken or unverified. Local service stand-ins and isolated
native successes do not establish a complete game or multiplayer-ready backend.

## Evidence index

1. <lab-pc>-20261010T180802-6a22199448334c9d9b7c118ae9ddbfb6 — JOIN252 untouched native segment, recorded
   2026-10-09T21:18:25Z, 30.66 s, 1280x720, audio present.
   Useful 0-15 s: approach, acquisition toast and rifle without urgent dialog.
   20-27 s still shows the unwanted NEW MISSION banner; do not claim that this
   run fixes it. Average/base recording rates 14.41/30 fps are media properties,
   not gameplay performance measurements. Comparison group
   urgent-dialog-at-rifle-grant; role after. Prior before evidence is referenced
   below. Sampled about every 3 s, not every frame. Moderate visual confidence.

2. <lab-pc>-20261010T180808-5edaf0838d814fb9b5895bd5a98f1515 — JOIN253 untouched native segment, recorded
   2026-10-10T00:53:41Z, 30.85 s, 1280x720, audio present.
   Useful 5-12 s: Enter the Wall; 14-25 s: Find a Weapon without NEW MISSION.
   Average/base recording rates 13.84/30 fps are media properties. Moderate
   confidence for this sampled handover only. No post-grant banner claim.

Each new clip package has two native-frame screenshot candidates. They are
private review candidates, not cleared public images. The whole clips remain
untouched; originals stay locally available. Contact sheets were private
inspection aids and are not delivered as clean screenshots.

Already delivered October 9 report: <lab-pc>-20261009T125718-1d066e3db5a04e1aa2f57c03b33c0c2b.
Earlier pickup/urgent-dialog evidence: <lab-pc>-20261009T125544-09cb974a0e1e437c81a2a894a944a91a.
Earlier rifle/Fallen contacts: <lab-pc>-20261009T125545-28ab7c371d894cb198306a0e9bbf0b63.
Earlier Guardian Down/respawn: <lab-pc>-20261009T125546-9759fa314de647ed8276a6f0b481dcba.
Reference these identities without resending their media. Their isolated native
pickup and Fallen/death results are background, not new achievements today.

## Remaining failure and next acceptance

JOIN253 activity log records an abort at 2026-10-10T00:55:02.078Z, a new
connection at 00:55:02.133Z, and a host-join request at 00:55:03.133Z.
The client returns to initial slice113 before another outdoor handover. The
engineering checkpoint reports an approximately 16-second client stall before
this; its cause is unresolved. The later 01:00:30 UTC disconnect is planned
run shutdown, not the earlier failure. Do not attribute the abort to the new
banner state merely because both occurred in this run.

Next engineering priority: retain the same activity connection through the
Breach and rifle pickup, then use actual squad-member actors or manual inputs
to accept a visible enemy kill. Require input -> hit/health/death -> encounter
progression, with trace and continuous footage. Verify post-grant objective
order and notice absence without dismissing dialogs. Reference ammunition is
25/184; the restored rifle still shows 12. XP, grimoire, ammo pickups, encounter
placement, Ghost illumination, first-person darkness, flicker/distortion, Nav
crash, early Level Up coach, later encounters, mission completion, rewards,
Tower and broader backend/multiplayer remain open.

## Privacy, audio and rights

All files are private production inputs. Both segments show emulator
recording-start/save notices with a recording path around 0-5 s. Inspect and
exclude or redact those notices before release; no automatic visual clearance
is claimed. The Level Up coach remains out of order throughout. Native capture
contains game audio and no microphone input; music was not audited or cleared.
No reference video is copied in this handoff. The existing user-supplied first
mission walkthrough supports behaviour comparisons, not publication rights.
No raw game assets, binaries, backend/native logs, credentials or instruction
files are included. No final video editing, narration, rendering or publishing
is performed here. Downstream privacy/rights review remains required.

## Handoff state at preparation

Two new media packages are durably queued; this consolidated report is queued
using the existing report command. Full media decoding and local hashes pass.
Transfer, destination hash verification and ready transition are recorded only
after they happen in the separate private receipt. No October 9 ready package
is modified. Delivery failure retains the outbox for retry. Report preparation
does not pause or complete the full engineering goal.
