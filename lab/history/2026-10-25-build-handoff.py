"""Private checkpoint report inputs; reuse capture.py report and the durable outbox."""
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess

private = Path(r'lab/history')
public = Path(r'F:\d1\public-work\d1reimagined-source')
source = Path(r'F:\d1\d1xenia')
pointer = private / '2026-10-25-draft-pointer.json'
if pointer.exists():
    raise SystemExit('Reuse the existing daily draft; do not create another package.')
at = datetime.now(timezone.utc)
instant = at.isoformat()
def git(root, *args):
    return subprocess.check_output(['git', '-C', str(root), *args]).decode().strip()
checkpoint = git(public, 'rev-parse', 'HEAD')
assert checkpoint.startswith('d4f9cda'), 'Source moved: review the new checkpoint before reporting'
implementation = git(public, 'rev-parse', '19150dc')
repo_status = git(public, 'status', '--porcelain')
legacy_head = git(source, 'rev-parse', 'HEAD')
legacy_status = git(source, 'status', '--porcelain')
changed = ['ps4/d1-server/crates/d1-activity-host/src/listener.rs', 'ps4/design/client-update-capture.md']
pins = {name: hashlib.sha256((public / name).read_bytes()).hexdigest() for name in changed}
runtime_listener = Path(r'F:\d1\d1preservation-main\d1-server\crates\d1-activity-host\src\listener.rs')
runtime_sha = hashlib.sha256(runtime_listener.read_bytes()).hexdigest()
assert runtime_sha == pins[changed[0]] == '3628d502ee2cef564c9568988483e145538542b1c7c9a8bf25510ec35aa38352'
run = Path(r'F:\d1\d1preservation-main\cfw-staging\contained-boot\20261025-174422-2026-10-25-agr-cap')
native = json.loads((run / 'receipt.json').read_text(encoding='utf-8-sig'))
assert native['serversStopped'] and native['endedUtc'].startswith('2026-10-25T18:00:56')
native_summary = {k: native.get(k) for k in ['name', 'startedUtc', 'endedUtc', 'emulatorExitCode',
    'emulatorExitedByItself', 'serversStopped', 'emulatorFault', 'guestCrashReport',
    'nativeBootAccepted', 'emulatorSha256', 'clientPatch']}
shot = Path(r'F:\d1\d1preservation-main\cfw-staging\2026-10-25-agr-cap\stuck.png')
shots = [{'file': 'screenshots/01.png', 'source_path': str(shot),
    'source_modified_at_utc': datetime.fromtimestamp(shot.stat().st_mtime, timezone.utc).isoformat(),
    'source_sha256': hashlib.sha256(shot.read_bytes()).hexdigest(), 'bytes': shot.stat().st_size,
    'dimensions': [1296, 759], 'visual_review': 'inspected for private handoff only',
    'claim': 'Visible WEASEL error after the diagnostic probe; no proof of the cause or mission completion.',
    'publication_approved': False, 'suggested_production_crop': {'x': 8, 'y': 31, 'width': 1280, 'height': 720}}]
prior_receipt = json.loads((private / '2026-10-15-daily-handoff-receipt.json').read_text())
prior_id = prior_receipt['daily_report']
prior = Path(prior_receipt['destination'])
manifest = json.loads((prior / 'hashes.json').read_text())
for name, expected in manifest['files'].items():
    data = (prior / name).read_bytes()
    assert len(data) == expected['size'] and hashlib.sha256(data).hexdigest() == expected['sha256']
old_meta = json.loads((prior / 'metadata.json').read_text())
tool_hashes = {name: hashlib.sha256((source / name).read_bytes()).hexdigest()
               for name in old_meta['handoff_tool_source_sha256']}
assert tool_hashes == old_meta['handoff_tool_source_sha256'], 'Report tool changed: validate before use'
test_log = (private / '2026-10-25-client-update-capture-tests-final.log').read_text(encoding='utf-8-sig')
assert '7 passed; 0 failed' in test_log
full_log = (private / '2026-10-25-activity-host-lib-tests-final.log').read_text(encoding='utf-8-sig')
assert '70 passed; 9 failed' in full_log and 'eboot' in full_log

report = f'''# 25 October 2026 — Wall-objective diagnostics and a failed native probe

Candidate public update: Investigation of the first mission's stalled objective
has narrowed the next debugging step. The latest test also hit a connection
error; there is no newly completed mission or verified gameplay improvement.

Private production evidence only; no publication or final video is approved.

## Date, coverage and the early checkpoint

Reporting date is 2026-10-25, Europe/London. This is an EARLY checkpoint handoff,
observed through {instant}, not a claim to cover future tests through 19:00.
UK clocks changed today: the preceding daily cutoff (24 October, 19:00 BST) was
2026-10-24T18:00:00Z; today's target (19:00 GMT) is 2026-10-25T19:00:00Z. The full
scheduled window is 25 hours. This report covers only its observed prefix.
Later results retain their actual dates and enter the next eligible handoff.
The last locally verified package is from 15 October, {prior_id}; missing dates
are not invented or retrospectively filled. Its seven payloads were re-hashed
today; none of that older media is resent or presented as new progress.

No native game was initially active. A concurrent engineering workflow then
deployed and launched a diagnostic test. This handoff waited until the run ended
and an empty native inventory was observed at 18:01:01 UTC. It stopped no process,
sent no pad input and touched no live game memory. Engineering took priority.

## Source state and what changed today

The legacy private checkout still points to {legacy_head}. Its last gameplay
notes were initially dated 15 October; the new probe below is dated 25 October. The
former private Git remote returned Repository not found; no hosting, visibility
or credentials were changed in response. This does not block the local/NAS work.

The public-source checkout was observed advancing through the 20 October unified
repository update. Its relevant Activity Host/BAP/oracle Rust inputs are unchanged
by that fast-forward. During preparation a concurrent workflow committed the
tested diagnostic implementation as {implementation}; remote HEAD was observed
at that commit. The reporting snapshot is {checkpoint}, which adds the new
investigation note. This heartbeat issued no commit or push command and
did not undo concurrent work. Source state is pinned below rather than assuming
the old 15 October checkout is current. The 20 October consolidation is historical
context, not new gameplay achieved on 25 October.

**Concrete engineering change:** the last useful next step was to observe what
client-owned state changes near Enter the Wall. Existing logs capture every
udt-18 incident but only the first payload of other message types per connection.
A single logged udt-21/47 vector cannot prove that later state never changes.

The listener now has opt-in, bounded diagnostic capture for udt 21 (client
authoritative data) and 47 (high water). D1_AH_CAPTURE_CLIENT_UPDATES=N requests
extra captures, hard-capped at 256 per connection across both types. An optional
D1_AH_CAPTURE_CLIENT_UPDATES_AFTER_S delay opens the observation window after the
first received 21/47 payload, so early counters need not consume the budget.
The first sample of each type inside the window establishes a fresh baseline;
subsequent identical samples are skipped. Comparison storage is at most 2 KiB per
type. Oversized data is not retained, remains eligible within the budget and is
explicitly truncated in diagnostic logs. Normal first-payload and every-incident
logging remains in place; diagnostics default off.

This is observability, not a trigger fix. It does not change authority, advance
an objective or emit an acknowledgement. In particular udt 21 remains fire and
forget; udt 47 is not assumed to be a player-trigger event. No generated raw
payload/log is included in this production input.

Changed source: ps4/d1-server/crates/d1-activity-host/src/listener.rs and
ps4/design/client-update-capture.md. Their exact SHA-256 values are in private
context metadata. The public checkout status at preparation is
{repo_status if repo_status else '(clean, at the concurrently committed checkpoint)'}.
The concurrent workflow mirrored the tested source (runtime SHA {runtime_sha}),
rebuilt the runtime host around 17:44 UTC and exercised it in the contained run.
That supersedes the initial assumption of pending deployment. The legacy
checkout's pre-existing three content-capture changes
and untracked file 3 are preserved and are not a gameplay change.

## Tests actually run, and limits

From the public checkout's ps4/d1-server directory, with the existing d1-env.ps1
build environment sourced:

- PASS: cargo test -p d1-activity-host --lib client_update_capture --offline.
  Final run: 7 passed, 0 failed, 73 filtered out. Tests exercise disabled/default
  behavior, both watched types, unchanged samples, delayed window baselines,
  shared hard budget, bounded/truncated oversized payloads and the actual
  notification path producing no replies or pushes.
- NOT PASSING: cargo test -p d1-activity-host --lib --offline. The full attempt
  with the initial six diagnostic tests finished 70 passed and 9 failed. All
  nine failures require the separately supplied pinned eboot/schema-codec input
  and reported its missing path. This is not a full-suite pass. The later added
  seventh diagnostic test passed in the final targeted run; the complete
  resulting 80-test suite was not rerun. These limits are not hidden by using a
  different game executable or suppressing failures.
- Initial targeted compilation failed because libclang was not on the build
  environment. Sourcing the existing d1-env.ps1 supplied the installed toolchain;
  compilation then succeeded. That recovered setup failure is not a game fix.
- PASS: git diff --check before the concurrent commit. The private delivery
  adapter's Python syntax check passed after correcting zero-screenshot receipts
  to say NOT APPLICABLE and preserving the report's early/late delivery status.
- NOT ACCEPTED: decoded trigger behavior, objective handover, combat,
  checkpoint/reconnect persistence, correct loadout and full mission completion.
  No lighting comparison, performance benchmark or video/audio acceptance ran.

Private originals of test output remain local. Only this sanitized outcome is
handed over; no raw trace, local credential or external client data is copied.

## Native probe and the updated investigation lead

Run agr-cap starts {native['startedUtc']} and ends {native['endedUtc']}.
The owned target is PS4 CUSA00219 56578.17.04.12.1251.evil, with the pinned
patched emulator and synthetic lab-orbit client patch. The receipt records
serversStopped=true, exit -1, emulatorFault=[], guestCrashReport=null and
nativeBootAccepted=false. These do not establish successful gameplay, a normal
clean client exit, an unmodified client or exact 01.29 acceptance.

The run used diagnostic count 256, delay 60 seconds. Independent log indexing
finds two CAPTURED udt-21 payloads, two udt-47 payloads and seven udt-18 records.
Additional entries may be the fresh window baselines; do not call them four
proved state changes or decoded trigger reports. They establish that the
diagnostic observation path ran, not that the objective progressed.

The new 25 October section appended to the historically named
notes/ps4-56578-blockers-2026-10-15.md reports sparse 21/47 updates and no trigger
report. Its next lead is objective activation: objective 3 never activates
client-side (no HUD), so local mission logic does not arm the client-owned
trigger. Trace the objective-state apply handler and its gates next. This is a
source-recorded lead, not a proved universal exclusion of those message types.

The probe ended at WEASEL, independently visible in the included original PNG.
The engineer records error.private_activity_host_host_crashed after an emulator
stall around 17:54–17:56 UTC. Independent host-log indexing still finds heartbeat
records in minutes 17:54, 17:55 and 17:56. Distinguish client/emulator silence
from an asserted Activity Host process crash; this is not a fully proven root
cause or a repaired stability/network fault.

## Maturity and current gameplay baseline

| Claim | Maturity and scope |
| --- | --- |
| Bounded client-update diagnostics | Unit Tested / Experimental; exercised in one contained probe, not accepted as decoded mission logic. |
| WEASEL failure | Visually checked; source-recorded stall context and independently checked host heartbeat presence. |
| Tutorial opening, skip handling and pad traversal | Historical 15 October partial lab results, referenced only. No repeat acceptance today. |
| Enter-the-Wall progress | Known Broken / Unaccepted. Trigger owner/report path remains under investigation; instrumentation does not itself repair it. |
| Lighting, authentic unarmed start/rifle pickup, encounters, combat, rewards and reload/reconnect progression | Still open or unverified; no new acceptance claimed. |
| Complete first mission, stock-client boot and full game | Not accepted. Historical native receipts retain the synthetic lab patch and nativeBootAccepted=false. |

## Evidence and handoff state

One new original PNG, screenshots/01.png, 1296x759, source modified
{shots[0]['source_modified_at_utc']}, proves the visible WEASEL dialog only.
It is a private diagnostic window capture with emulator/operator title and
desktop/log edges, not an approved clean public image. A possible viewport crop
is x=8,y=31,w=1280,h=720; no crop, colour correction or other edit was performed.
The existing report command image-decodes and pins the unchanged copied original.
No video/audio is included; playback, frame rate, microphone/music and audio
checks are NOT APPLICABLE. Older originals are not repeated to fill an update,
and video is not manufactured from stills. Unit tests do not prove game playback.
There is no before/after footage or performance benchmark for this change.

All production inputs remain private and require downstream review. No source
policy/instruction, extracted game input, executable, firmware, raw trace or
ready package is edited or copied into this package. The previous report is
referenced by identity. The existing capture report command, durable outbox,
per-payload verification and uploading-to-ready NAS transition handle delivery;
its actual identity, times, capacity and pending state are recorded separately
in today's private receipt. Receipt completion means this observed checkpoint
was delivered, not that the future scheduled daily window was already observed.

## Next engineering acceptance

Keep the correct tutorial activity, intro/gameplay regions and cast baseline.
Trace why the served objective does not become active client-side after the
teleport. Establish the objective handler's gate before more trigger-state guesses
or speculative udt-21 replies. Do not replace trigger completion with a timer. Then
verify one natural, exactly-once objective/waypoint handover, both cinematic skip
paths and reload/reconnect state. The full schema tests still need their correct
pinned fixture. Work proceeds independently of production review.
'''

events = [
    {'event_id': 'd1-20261025-client-update-diagnostics', 'timestamp': instant,
     'timestamp_basis': 'Checkpoint inspection/test verification; source commit time is separately recorded.',
     'source_commit': implementation, 'feature': 'client-owned-trigger-observability',
     'maturity': 'Unit Tested / Experimental; one bounded native exercise, no mission acceptance',
     'claim': 'Default-off bounded 21/47 diagnostic capture, with a delayed observation baseline; no protocol replies/authority changes.',
     'tests': {'targeted': {'passed': 7, 'failed': 0},
               'full_attempt_before_seventh_test': {'passed': 70, 'failed': 9, 'reason': 'missing pinned eboot input'},
               'native_mission_progression': 'NOT ACCEPTED'},
     'full_game_accepted': False, 'changed_source_sha256': pins},
    {'event_id': 'd1-20261025-native-weasel', 'timestamp': native['endedUtc'],
     'started_at': native['startedUtc'], 'feature': 'native-diagnostic-probe',
     'maturity': 'Known Failure / Experimental', 'native_receipt': native_summary,
     'claim': 'Diagnostic probe ends with a visible WEASEL error; no mission completion.',
     'evidence': [{k: v for k, v in s.items() if k != 'source_path'} for s in shots]},
]
meta = {'recorded_at': instant, 'reporting_date': '2026-10-25', 'reporting_timezone': 'Europe/London',
        'reporting_interval_start': '2026-10-24T18:00:00Z', 'reporting_interval_end': instant,
        'scheduled_cutoff_utc': '2026-10-25T19:00:00Z', 'reporting_window_complete': False,
        'early_checkpoint': True, 'late_delivery': False,
        'daily_update_id': 'destiny1-2026-10-25', 'development_session': 'bounded-client-update-diagnostics',
        'topic': 'wall-trigger-observability-and-validation-limits', 'feature': 'bounded-client-update-diagnostics',
        'event_type': 'GENERAL_PROGRESS', 'importance': 'USEFUL',
        'description': 'Early private checkpoint: seven targeted diagnostic tests pass; full schema checks lack their pinned input. Contained native probe ends in WEASEL; one original failure screenshot, no video.',
        'previous_state': 'Only the first 21/47 payload was visible per connection; no later-state diagnostic window.',
        'current_state': 'Bounded diagnostics unit-tested and exercised; objective activation is the next lead, no gameplay fix accepted.',
        'remaining_issue': 'Actual client-owned trigger decoding, objective progression, correct loadout, combat, persistence, lighting and required schema-test fixture.',
        'referenced_package_ids': [prior_id], 'new_media_included': True, 'new_screenshot_count': 1, 'new_video_count': 0,
        'publication_review_required': True, 'native_boot_accepted': False,
        'tested_source_commit': implementation, 'reporting_snapshot_commit': checkpoint,
        'tested_source_branch': git(public, 'branch', '--show-current'),
        'tested_worktree_clean': not bool(repo_status), 'changed_source_sha256': pins,
        'dirty_build_description': 'Concurrent workflow committed the tested diagnostic source and deployed a matching runtime copy. Legacy report-tool changes/untracked file 3 are preserved.',
        'legacy_source_commit': legacy_head, 'legacy_worktree_status': legacy_status,
        'handoff_tool_source_sha256': tool_hashes, 'runtime_listener_sha256': runtime_sha,
        'native_run_receipt': native_summary,
        'native_validation': 'Diagnostic exercise only; mission progression not accepted',
        'screenshot_evidence': [{k: v for k, v in s.items() if k != 'source_path'} for s in shots],
        'post_checkpoint_evidence': 'Not observed by this early package; preserve actual instants in the next eligible handoff.'}
draft = private / ('daily-2026-10-25-' + at.strftime('%Y%m%dT%H%M%S'))
draft.mkdir()
for name, value in [('events.json', events), ('metadata-input.json', meta), ('selected-screenshots.json', shots)]:
    (draft / name).write_text(json.dumps(value, indent=2) + '\n', encoding='utf-8')
(draft / 'daily-report.md').write_text(report, encoding='utf-8')
cfg = json.loads((private.parent / 'content-capture/config.json').read_text(encoding='utf-8-sig'))
cfg.update(CURRENT_GAME_BUILD='PS4 CUSA00219 owned 56578.17.04.12.1251.evil; synthetic lab patch; native progression not accepted',
           CURRENT_SESSION='daily-engineering-handoff-2026-10-25', CURRENT_FEATURE='bounded-client-update-diagnostics')
(draft / 'config.report.json').write_text(json.dumps(cfg, indent=2) + '\n', encoding='utf-8')
pointer.write_text(json.dumps({'draft': str(draft), 'prepared_utc': instant,
                              'state': 'PREPARED_WAITING_FOR_SAFE_CHECKPOINT', 'reporting_date': '2026-10-25',
                              'pre_cutoff_source': checkpoint}, indent=2) + '\n', encoding='utf-8')
print(json.dumps({'draft': str(draft), 'report_bytes': len(report.encode()), 'new_screenshots': 1,
                  'coverage_end': instant, 'target_cutoff': meta['scheduled_cutoff_utc']}))
