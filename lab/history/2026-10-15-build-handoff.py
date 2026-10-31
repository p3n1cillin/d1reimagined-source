"""Prepare today's private inputs for the existing capture report command."""
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess

private = Path(r'lab/history')
repo = Path(r'F:\d1\d1xenia')
runtime = Path(r'F:\d1\d1preservation-main')
pointer = private / '2026-10-15-draft-pointer.json'
if pointer.exists():
    raise SystemExit('Existing daily draft retained; use its pointer, do not duplicate it.')
at = datetime.now(timezone.utc)
def git(*args):
    return subprocess.check_output(['git', '-C', str(repo), *args]).decode().strip()
checkpoint = git('rev-parse', '512c659')
head = git('rev-parse', 'HEAD')
dirty = git('status', '--porcelain')
previous_receipt = json.loads((private / '2026-10-14-daily-handoff-receipt.json').read_text())
previous = previous_receipt['daily_report']
prior = Path(previous_receipt['destination'])
manifest = json.loads((prior / 'hashes.json').read_text())
for name, pin in manifest['files'].items():
    data = (prior / name).read_bytes()
    assert len(data) == pin['size'] and hashlib.sha256(data).hexdigest() == pin['sha256']
prior_meta = json.loads((prior / 'metadata.json').read_text())
tool_hashes = {p: hashlib.sha256((repo / p).read_bytes()).hexdigest()
               for p in prior_meta['handoff_tool_source_sha256']}
assert tool_hashes == prior_meta['handoff_tool_source_sha256'], 'Revalidate changed capture tooling first'

selections = [
    ('2026-10-14-objwp3/enter-the-wall.png', 'Late 14 October objective-path experiment: ENTER THE WALL is visible, with a marker. The launched activity was The Dark Within, mapped to tutorial content; this is not a coherent tutorial or accepted waypoint implementation.'),
    ('2026-10-15-agr9/s2.png', 'A frame of the tutorial opening cinematic over the wrecked cars. Still proof of a rendered cinematic scene, not continuous playback, dialogue, Ghost presence or complete cutscene fidelity.'),
    ('2026-10-15-agr9/s20.png', 'Armed first-person Guardian in daylight facing the Wall after the recorded opening/teleport. Hand cannon is a known incorrect tutorial starting loadout; the objective HUD is missing.'),
    ('2026-10-15-agr17/wall.png', 'Dark interior walkway reached in the recorded pad-driven route. Traversal is source-reported; this still does not prove objective completion, trigger delivery or the route in motion.'),
]
shots = []
for i, (rel, claim) in enumerate(selections, 1):
    p = runtime / 'cfw-staging' / rel
    shots.append({'file': f'screenshots/{i:02d}.png', 'source_path': str(p),
                  'source_modified_at_utc': datetime.fromtimestamp(p.stat().st_mtime, timezone.utc).isoformat(),
                  'source_sha256': hashlib.sha256(p.read_bytes()).hexdigest(), 'bytes': p.stat().st_size,
                  'visual_review': 'inspected for private handoff only', 'claim': claim,
                  'dimensions': [1296, 759], 'publication_approved': False,
                  'suggested_production_crop': {'x': 8, 'y': 31, 'width': 1280, 'height': 720}})

receipts = []
for run in ['20261015-143808-2026-10-15-agr9', '20261015-155733-2026-10-15-agr17', '20261015-161002-2026-10-15-agr18']:
    p = runtime / 'cfw-staging/contained-boot' / run / 'receipt.json'
    r = json.loads(p.read_text(encoding='utf-8-sig'))
    assert r['serversStopped'] is True and r['nativeBootAccepted'] is False
    receipts.append({'reference': p.relative_to(runtime).as_posix(),
                     **{k: r.get(k) for k in ['name', 'startedUtc', 'endedUtc', 'emulatorExitCode',
                        'emulatorExitedByItself', 'serversStopped', 'emulatorFault', 'guestCrashReport',
                        'nativeBootAccepted', 'emulatorSha256', 'clientPatch']}})

report = f'''# 15 October 2026 — tutorial opening and traversal, with objective progression blocked

Candidate public update: The first mission's opening can now reach the starting
area, and movement through the Wall has been exercised. Objective progression,
the starting equipment and lighting still need work.

Private production evidence only. No publication or release is approved.

## Reporting boundary and reproducibility

Reporting date: 2026-10-15, Europe/London. Interval: 2026-10-14T18:00:00Z through
2026-10-15T18:00:00Z (19:00 UK). Late 14 October work is included with its actual
date. Yesterday's immutable package {previous} is referenced, not resent.
Pre-cutoff source: {checkpoint}, master. Observed HEAD at preparation: {head}.
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
'''
for r in receipts:
    report += f"- {r['name']}: {r['startedUtc']} -> {r['endedUtc']}; exit {r['emulatorExitCode']}; servers stopped {r['serversStopped']}.\n"
report += '''
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
'''
for s in shots:
    report += f"\n- `{s['file']}`, original modified {s['source_modified_at_utc']}: {s['claim']}\n"
report += f'''
Each selected original was visually inspected, then byte-pinned and fully decoded
as an image by the existing report command. No video was identified in the
inspected staging search (depth two), and no video/audio is included. Video
frame rate, continuous playback, music and microphone checks are NOT APPLICABLE
to these stills; game sound and full cutscene fidelity remain unverified. No
video is manufactured from intermittent screenshots. No actual paired before/
after footage is supplied. Private transfer is not rights/publication approval.

Earlier package {previous} remains immutable. Its six payloads were re-hashed
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
'''

events = []
for short, feature, claim, maturity in [
    ('6b4f9e4', 'per-activity-world-team', 'World and team follow configured launched-activity rows; not arbitrary client-selected PvP map routing.', 'Experimental'),
    ('b74591b', 'objective-hud-and-bap-reconnect', 'ENTER THE WALL display in a mismatched activity/world experiment; reconnect baseline path repaired.', 'Partially Working'),
    ('359f048', 'earth-world-identification', 'Earth story worlds identified from content; private Dark Within start is a stand-in.', 'Experimental'),
    ('19f51a0', 'renderer-texture-cache', 'Compatible-array expansion retained; severe colour/darkness remains.', 'Partially Working'),
    ('ffd2201', 'tutorial-opening', 'Served revival/wake-up/teleport reaches the daylight tutorial highway with lab interventions retained.', 'Experimental'),
    ('cae6833', 'renderer-volume-attempt', 'Colour-volume attempt worsened gameplay and was reverted.', 'Known Broken / Reverted'),
    ('ba87b1d', 'cinematic-skip-incidents', 'Host can advance tutorial steps on stop/skip incidents with timeout fallback.', 'Partially Working'),
    ('512c659', 'enter-wall-trigger', 'Walking reaches the Wall but the expected trigger report is absent; local ownership is an investigation lead.', 'Known Broken'),
]:
    instant = datetime.fromisoformat(git('show', '-s', '--format=%aI', short)).astimezone(timezone.utc).isoformat()
    events.append({'event_id': 'd1-20261015-' + feature, 'timestamp': instant,
                   'timestamp_basis': 'Recorded source commit time, not an invented test instant.',
                   'source_commit': git('rev-parse', short), 'feature': feature, 'claim': claim,
                   'maturity': maturity, 'verification': 'source-recorded outcomes, not rerun by handoff',
                   'full_game_accepted': False, 'retained_lab_client_patch': True})
events.append({'event_id': 'd1-20261015-inspected-evidence', 'timestamp': at.isoformat(),
               'timestamp_basis': 'Handoff inspection/preparation; original capture modification instants are separate.',
               'feature': 'private-evidence-validation', 'maturity': 'Inspected stills and closed-run receipts only',
               'receipts': receipts,
               'evidence': [{k: v for k, v in s.items() if k != 'source_path'} for s in shots]})
meta = {'recorded_at': at.isoformat(), 'reporting_date': '2026-10-15', 'reporting_timezone': 'Europe/London',
        'reporting_interval_start': '2026-10-14T18:00:00Z', 'reporting_interval_end': '2026-10-15T18:00:00Z',
        'daily_update_id': 'destiny1-2026-10-15', 'development_session': 'tutorial-opening-trigger-investigation',
        'topic': 'tutorial-opening-and-enter-wall-trigger', 'feature': 'first-mission-opening-and-traversal',
        'event_type': 'GENERAL_PROGRESS', 'importance': 'USEFUL',
        'description': 'Private daily evidence: tutorial opening, incident-based skips and pad traversal; objective trigger and lighting still broken. Four inspected stills, no video.',
        'previous_state': 'Armed world entry without a coherent tutorial opening; configured world/activity mismatch.',
        'current_state': 'Served tutorial opening reaches the daylight highway; route reaches the Wall; trigger progression remains unaccepted.',
        'remaining_issue': 'Client-owned trigger report, missing post-teleport objective HUD, authentic loadout, combat, mission progression, public sessions and dark/blue rendering.',
        'referenced_package_ids': [previous], 'new_media_included': True, 'new_screenshot_count': len(shots),
        'new_video_count': 0, 'publication_review_required': True, 'native_boot_accepted': False,
        'retained_lab_patch_synthetic': True, 'tested_source_commit': checkpoint, 'tested_source_branch': 'master',
        'tested_worktree_clean': False, 'worktree_status_at_preparation': dirty,
        'dirty_build_description': 'Unchanged tested capture PNG extension remains local; untracked file 3 excluded. Runtime includes separately built patched emulator/services and private generated fixtures.',
        'handoff_tool_source_sha256': tool_hashes, 'native_run_receipts': receipts,
        'screenshot_evidence': [{k: v for k, v in s.items() if k != 'source_path'} for s in shots],
        'late_delivery': at.hour >= 18, 'post_cutoff_evidence_excluded': 'Anything after 2026-10-15T18:00:00Z is deferred.'}
draft = private / ('daily-2026-10-15-' + at.strftime('%Y%m%dT%H%M%S'))
draft.mkdir()
for name, value in [('events.json', events), ('metadata-input.json', meta), ('selected-screenshots.json', shots)]:
    (draft / name).write_text(json.dumps(value, indent=2) + '\n', encoding='utf-8')
(draft / 'daily-report.md').write_text(report, encoding='utf-8')
cfg = json.loads((private.parent / 'content-capture/config.json').read_text(encoding='utf-8-sig'))
cfg.update(CURRENT_GAME_BUILD='PS4 CUSA00219 owned build 56578.17.04.12.1251.evil; package metadata 01.33; retained synthetic lab patch',
           CURRENT_SESSION='daily-engineering-handoff-2026-10-15', CURRENT_FEATURE='tutorial-opening-and-trigger-investigation')
(draft / 'config.report.json').write_text(json.dumps(cfg, indent=2) + '\n', encoding='utf-8')
pointer.write_text(json.dumps({'draft': str(draft), 'prepared_utc': at.isoformat(),
                              'state': 'PREPARED_WAITING_FOR_SAFE_CHECKPOINT',
                              'reporting_date': '2026-10-15', 'pre_cutoff_source': checkpoint}, indent=2) + '\n', encoding='utf-8')
print(json.dumps({'draft': str(draft), 'report_bytes': len(report.encode()), 'screenshots': len(shots), 'receipt_count': len(receipts)}))
