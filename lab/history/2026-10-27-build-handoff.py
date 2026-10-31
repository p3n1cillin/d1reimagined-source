"""One consolidated private daily report; referenced milestone media is not copied."""
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess
import sys

PRIVATE = Path(__file__).parent
PUBLIC = Path(r'F:\d1\public-work\d1reimagined-source')
LEGACY = Path(r'F:\d1\d1xenia')
DATE = '2026-10-27'
POINTER = PRIVATE / (DATE + '-draft-pointer.json')
sys.path.insert(0, str(LEGACY / 'tools/content_capture'))
from capture import Capture, lock, check_text


def git(*args):
    return subprocess.check_output(['git', '-C', str(PUBLIC), *args], text=True).strip()


with lock(PRIVATE / (DATE + '-delivery.lock'), timeout=1):
    if POINTER.exists():
        print(POINTER.read_text())
        raise SystemExit(0)
    at = datetime.now(timezone.utc)
    stamp = at.isoformat()
    config_path = PRIVATE.parent / 'content-capture/config.json'
    cap = Capture(config_path)
    parent = Path(cap.cfg['CONTENT_NAS_PATH']) / 'incoming' / cap.machine / '2026/10/27'
    for ready in parent.glob('*.ready'):
        meta = json.loads((ready/'metadata.json').read_text(encoding='utf-8-sig'))
        if meta.get('package_kind') == 'engineering-report' and meta.get('reporting_date') == DATE:
            cap.verify(ready)
            print(json.dumps({'state': 'EXISTING_DAILY_REPORT_VERIFIED', 'destination': str(ready)}))
            raise SystemExit(0)
    snapshot = git('rev-parse', 'HEAD')
    if not snapshot.startswith('7cc9a8e'):
        raise RuntimeError('Source advanced: review the new checkpoint before preparing the daily report')
    status = git('status', '--porcelain')
    earlier = '<lab-pc>-20261027T163920-483d90b903ad47e69f966f0c5fb71f27'
    event_package = parent / (earlier + '.ready')
    manifest = json.loads((event_package/'hashes.json').read_text())
    for relative, pin in manifest['files'].items():
        data = (event_package/relative).read_bytes()
        if not data or len(data) != pin['size'] or hashlib.sha256(data).hexdigest() != pin['sha256']:
            raise RuntimeError('Earlier event package integrity check failed')
    event_manifest_sha = hashlib.sha256((event_package/'hashes.json').read_bytes()).hexdigest()
    tool_hashes = {name: hashlib.sha256((LEGACY/name).read_bytes()).hexdigest() for name in (
        'tools/content_capture/capture.py', 'tools/content_capture/test_capture.py',
        'tools/content_capture/README.md')}
    report = f'''# 27 October 2026 - the opening mission reaches and crosses the Divide

Candidate public update: The opening mission can now replay automatically into
the Divide. The Guardian can reach its far gate and encounter Fallen there.
Combat balance and the next section remain under development.

Private production input only. Publication and final-video work are not approved.

## Reporting window and source checkpoint

Reporting date: 2026-10-27, Europe/London (GMT). Coverage is the reporting window
2026-10-26T19:00:00Z through 2026-10-27T19:00:00Z. Verification/preparation time is
{stamp}. Native tests begun after the cutoff retain their actual dates and roll
into the next eligible edition. This is the one consolidated daily engineering
report; the earlier 16:39 package is referenced as an event, without its media.

The public source checkpoint is {snapshot}, branch {git('branch', '--show-current')}.
Its observed status is {status if status else 'clean'}. PR40 (merge 1a515db) and
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
- NOT RUN: full retail/stock boot acceptance, full mission completion, verified
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

Referenced event package: {earlier}. Its9 payloads,226254428 bytes, were
independently re-hashed against its existing manifest today. Manifest SHA256:
{event_manifest_sha}. It remains immutable. None of its media is resent.

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
'''
    events = [
        {'event_id': 'd1-20261027-objective-replay', 'timestamp': '2026-10-27T16:32:32.690354Z',
         'feature': 'opening-mission-replay', 'maturity': 'Working within tested contained lab scope',
         'claim': 'Natural slice96 crossing and scheduled step16; source-recorded unattended route.',
         'evidence': {'referenced_package_id': earlier, 'files': ['clips/clip01.mp4','clips/clip02.mp4','clips/clip03.mp4']},
         'full_mission_complete': False},
        {'event_id': 'd1-20261027-low-divide-route', 'timestamp': '2026-10-27T18:32:06Z',
         'timestamp_basis': 'Source journal checkpoint; explorer terminal position independently checked.',
         'feature': 'divide-crossing-and-exit-fight', 'maturity': 'Partially Working',
         'claim': 'Low area-centre route reaches exit gate; hostile engagement/death recorded; no verified kill.',
         'next_region_accepted': False},
        {'event_id': 'd1-20261027-report-verification', 'timestamp': stamp,
         'feature': 'daily-evidence-integrity', 'maturity': 'Verified private report integrity',
         'claim': 'Earlier event package payload hashes verified; media not resent or newly decoded.',
         'referenced_package_id': earlier, 'payload_count': 9,
         'manifest_sha256': event_manifest_sha},
    ]
    meta = {
        'recorded_at': stamp, 'reporting_date': DATE, 'reporting_timezone': 'Europe/London',
        'reporting_interval_start': '2026-10-26T19:00:00Z', 'reporting_interval_end': '2026-10-27T19:00:00Z',
        'scheduled_cutoff_utc': '2026-10-27T19:00:00Z', 'late_delivery': False,
        'daily_update_id': 'destiny1-2026-10-27', 'topic': 'opening-mission-divide-and-combat-frontier',
        'feature': 'opening-mission-divide-and-combat-frontier', 'event_type': 'GENERAL_PROGRESS',
        'importance': 'USEFUL', 'description': 'Consolidated daily evidence: unattended opening replay and objective, low Divide crossing and exit combat frontier. Three earlier clips referenced without resending media.',
        'previous_state': 'Opening replay and objective handover under validation; Divide crossing unresolved.',
        'current_state': 'Unattended replay verified; low crossing reaches gate; enemies engage, but kill/next-region completion unverified.',
        'remaining_issue': 'Aiming and survivability, ammo, encounter completion, slice104 entry, drop staging, clock and flyover.',
        'tested_source_commit': git('rev-parse', '1a515db'), 'reporting_snapshot_commit': snapshot,
        'tested_source_branch': 'ps4/divide-crossing', 'reporting_snapshot_branch': git('branch','--show-current'),
        'tested_worktree_clean': not bool(status),
        'dirty_build_description': 'Native tests preceded their final source commits. This report pins committed journals and independently checked logs; it does not claim every final source aggregate was rerun locally.',
        'referenced_package_ids': [earlier], 'earlier_media_resent': False,
        'new_media_included': False, 'new_screenshot_count': 0, 'new_video_count': 0,
        'earlier_manifest_sha256': event_manifest_sha, 'earlier_payload_integrity': 'PASS:9payloads',
        'new_video_decode': 'NOT APPLICABLE: no new media', 'earlier_video_decode_this_handoff': 'NOT RUN',
        'publication_review_required': True, 'native_boot_accepted': False,
        'handoff_tool_source_sha256': tool_hashes,
        'post_cutoff_evidence': 'Reserved for the next eligible edition; no report updates to ready packages.',
    }
    draft = PRIVATE / ('daily-' + DATE + '-' + at.strftime('%Y%m%dT%H%M%S'))
    draft.mkdir()
    for name, value in [('metadata-input.json',meta),('events.json',events),('selected-screenshots.json',[])]:
        (draft/name).write_text(json.dumps(value,indent=2)+'\n',encoding='utf-8')
    (draft/'daily-report.md').write_text(report,encoding='utf-8')
    cfg = dict(cap.cfg, CURRENT_GAME_BUILD='PS4 CUSA00219 owned56578; existing synthetic lab patches; full mission not accepted',
               CURRENT_SESSION='daily-engineering-handoff-2026-10-27',
               CURRENT_FEATURE='opening-mission-divide-and-combat-frontier')
    (draft/'config.report.json').write_text(json.dumps(cfg,indent=2)+'\n',encoding='utf-8')
    for name in ('metadata-input.json','events.json','daily-report.md'):
        check_text((draft/name).read_text(encoding='utf-8'))
    job = {'draft':str(draft),'prepared_utc':stamp,'state':'PREPARED_WAITING_FOR_SAFE_CHECKPOINT',
           'reporting_date':DATE,'pre_cutoff_source':snapshot}
    POINTER.write_text(json.dumps(job,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(job))
