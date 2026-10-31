"""Prepare today's factual private report; no raw logs or instruction files in packages."""
import json
import subprocess
from datetime import datetime, timezone
from pathlib import Path

REPO = Path('F:/d1/d1xenia')
RT = Path('F:/d1/<pc>-import/d1xenia/local')
WORK = Path(__file__).parent
now = datetime.now(timezone.utc).isoformat()
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPO, text=True).strip()
dirty = bool(subprocess.check_output(['git', 'status', '--porcelain'], cwd=REPO, text=True))
assert head == '1d5b7e291220425e81c26197dda039aac9fa259e' and not dirty
new = json.loads((WORK / 'new-evidence-packages.json').read_text())
earlier = json.loads((RT / 'PRIVATE_ENGINEERING/2026-10-09-daily-handoff-receipt.json').read_text())
earlier_ids = [entry.split()[0] for entry in earlier['clip_packages']]
for name, count in [('server-tests.txt', 225), ('script-tests.txt', 176)]:
    result = (WORK / name).read_text(encoding='utf-8-sig')
    assert f'Ran {count} tests' in result and result.rstrip().endswith('OK')
closures = {}
for run in ['JOIN252', 'JOIN253']:
    closure = json.loads((RT / f'RESEARCH/{run}-independent-closure-verified.json').read_text(encoding='utf-8-sig'))
    cleanup = json.loads((RT / f'SIGNON_LOCAL/logs/{run}-cleanup-result.json').read_text(encoding='utf-8-sig'))
    assert closure['all_hashes_match'] and not any(c['exact_owned_alive'] for c in closure['checks'])
    assert cleanup['all_owned_closed'] and not cleanup['errors'] and not cleanup['active_controls']
    closures[run] = {'closed_at': cleanup['at'], 'identities_checked': len(closure['checks']),
                     'baseline_hashes_restored': 3, 'input': cleanup['input'],
                     'shared_rpcn_preserved': cleanup['shared_rpcn_preserved']}

events = [
    {'event_id': 'ui-urgent-suppression-join252', 'timestamp': '2026-10-09T21:18:25+00:00',
     'timestamp_meaning': 'Start of supplied native segment; grant at 21:18:28.314 UTC.',
     'feature': 'opening-mission-notices', 'maturity': 'Partially Working',
     'claims': ['No urgent dialog is visible in the sampled grant sequence.',
                'The retained native watch log has zero dialog-enqueue entries in JOIN252.'],
     'implementation': 'Launch defaults override universe flags 2059, 2065 and 2066 to false; committed as 48c3992.',
     'tested_source': 'bb55871 plus the recorded launch-default change; native worktree dirty.',
     'evidence': [{'package_id': new[0]['package_id'], 'file': 'clip.mp4', 'start_seconds': 0, 'end_seconds': 15,
                   'confidence': 'Moderate: sampled frames and native enqueue watch, one bounded run.'}],
     'comparison_group': 'urgent-dialog-at-rifle-grant', 'comparison_role': 'after',
     'before_package_id': earlier_ids[0],
     'limitations': ['NEW MISSION banner still visible later in this clip.', 'No full-game notice policy acceptance.'],
     'closure': closures['JOIN252']},
    {'event_id': 'ui-objective-banner-join253', 'timestamp': '2026-10-10T00:53:41+00:00',
     'timestamp_meaning': 'Start of supplied native handover segment.',
     'feature': 'opening-mission-objective-presentation', 'maturity': 'Partially Working',
     'claims': ['Sampled Enter the Wall -> Find a Weapon handover has no NEW MISSION banner.'],
     'implementation': 'Mission objectives begin active (state 5) rather than incoming (state 1); committed as 1d5b7e2.',
     'tested_source': '48c3992 plus uncommitted mission objective-state change; native worktree dirty.',
     'evidence': [{'package_id': new[1]['package_id'], 'file': 'clip.mp4', 'start_seconds': 5, 'end_seconds': 25,
                   'confidence': 'Moderate: sampled native video, one handover.'}],
     'limitations': ['Later connection loss prevents acceptance of the post-grant banner in JOIN253.',
                     'This is not mission completion or an end-to-end regression pass.'],
     'closure': closures['JOIN253']},
    {'event_id': 'connection-rejoin-join253', 'timestamp': '2026-10-10T00:55:02.078+00:00',
     'feature': 'activity-connection-stability', 'maturity': 'Known Broken',
     'claims': ['The original activity connection aborts; a new connection appears 55 ms later and requests a host join.',
                'The client returns to initial slice 113 and later hands over to outdoor slice 112 again.'],
     'evidence': [{'file': 'daily-report.md', 'section': 'Remaining failure and next acceptance',
                   'confidence': 'High for reconnect sequence; root cause unknown.'}],
     'limitations': ['Earlier checkpoint reports an approximately 16 s client stall; no causal renderer/backend attribution accepted.',
                     'Native footage of this failure is retained locally but not duplicated in this handoff.']},
    {'event_id': 'ammo-and-aiming-negative-followup', 'timestamp': '2026-10-09T14:00:35+00:00',
     'timestamp_meaning': 'Commit timestamp of d0ffccd documenting JOIN233-235, not a fabricated native event instant.',
     'feature': 'combat-fidelity', 'maturity': 'Known Broken',
     'claims': ['The tested magazine-stat override still displays 12 rounds; reference is 25/184.',
                'Aimed-fire experiments do not establish a Dreg kill; entity-pointer samples are not an accepted enemy list.'],
     'evidence': [{'file': 'daily-report.md', 'section': 'Private engineering summary',
                   'confidence': 'Documented earlier test observations, not rerun today.'}],
     'limitations': ['Failed stat experiment was not retained.', 'Kills, XP, ammunition pickups and later encounters remain unverified.']},
    {'event_id': 'source-validation-20261010', 'timestamp': now, 'feature': 'backend-and-diagnostics',
     'maturity': 'Fully Working within automated test scope',
     'tests': [{'command': 'python -m unittest discover -s server -p test_*.py', 'result': 'PASS', 'count': 225, 'seconds': 14.406},
               {'command': 'python -m unittest discover -s scripts -p test_*.py', 'result': 'PASS', 'count': 176, 'seconds': 9.369}],
     'source_commit': head, 'dirty_worktree': False,
     'limitations': ['Automated tests do not establish native graphics, combat kills, persistence/reload or full-game playability.',
                     'No new native run or final-schema preflight was executed today for this handoff.']},
]

report = f'''# Opening mission: fewer unwanted interruptions

Candidate public update: The opening mission now avoids an unwanted notice during
the rifle pickup and an extra mission banner at the Wall. Testing still shows
lighting problems and a later connection restart. Combat and the first mission
are still being restored.

## Private engineering summary

Reporting date: 2026-10-10, Europe/London. Reporting interval starts at the
previous consolidated handoff, 2026-10-09T12:57:17.9180128Z, and ends at {now}.
Late evidence keeps its actual October 9/10 event dates; these are not all new
tests performed today. Current source master is {head}, clean and verified
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
  {closures['JOIN252']['identities_checked']}/{closures['JOIN253']['identities_checked']}
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

1. {new[0]['package_id']} — JOIN252 untouched native segment, recorded
   2026-10-09T21:18:25Z, {new[0]['duration_seconds']} s, 1280x720, audio present.
   Useful 0-15 s: approach, acquisition toast and rifle without urgent dialog.
   20-27 s still shows the unwanted NEW MISSION banner; do not claim that this
   run fixes it. Average/base recording rates 14.41/30 fps are media properties,
   not gameplay performance measurements. Comparison group
   urgent-dialog-at-rifle-grant; role after. Prior before evidence is referenced
   below. Sampled about every 3 s, not every frame. Moderate visual confidence.

2. {new[1]['package_id']} — JOIN253 untouched native segment, recorded
   2026-10-10T00:53:41Z, {new[1]['duration_seconds']} s, 1280x720, audio present.
   Useful 5-12 s: Enter the Wall; 14-25 s: Find a Weapon without NEW MISSION.
   Average/base recording rates 13.84/30 fps are media properties. Moderate
   confidence for this sampled handover only. No post-grant banner claim.

Each new clip package has two native-frame screenshot candidates. They are
private review candidates, not cleared public images. The whole clips remain
untouched; originals stay locally available. Contact sheets were private
inspection aids and are not delivered as clean screenshots.

Already delivered October 9 report: {earlier['daily_report']}.
Earlier pickup/urgent-dialog evidence: {earlier_ids[0]}.
Earlier rifle/Fallen contacts: {earlier_ids[1]}.
Earlier Guardian Down/respawn: {earlier_ids[2]}.
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
'''
metadata = {
    'recorded_at': now, 'reporting_date': '2026-10-10', 'reporting_timezone': 'Europe/London',
    'daily_update_id': 'destiny1-2026-10-10',
    'reporting_interval_start': '2026-10-09T12:57:17.9180128Z', 'reporting_interval_end': now,
    'development_session': 'opening-mission-ui-order', 'topic': 'opening-mission',
    'feature': 'opening-mission-ui-order', 'event_type': 'MISSION_PROGRESS', 'importance': 'USEFUL',
    'description': 'Consolidated private daily evidence: urgent notice suppression at rifle acquisition and objective handover without the extra mission banner; connection restart, ammunition, aiming and lighting limits remain explicit.',
    'previous_state': 'Unsolicited urgent dialog blocks input after a grant; incoming objectives show an extra mission banner.',
    'current_state': 'One bounded grant sample has no urgent dialog; one Wall handover has no extra mission banner. These are separate native runs with recorded dirty test revisions.',
    'remaining_issue': 'Post-grant banner acceptance, connection restart, magazine 12 versus reference 25/184, enemy kills, early Level Up, lighting, Ghost glow, flicker/distortion and mission completion.',
    'referenced_package_ids': [earlier['daily_report'], *earlier_ids, *[p['package_id'] for p in new]],
    'new_media_included': True, 'new_media_delivery': 'pending at preparation',
    'publication_review_required': True, 'native_pending_at_cutoff': [],
    'tested_runtime_build_sha256': 'c5bc7f5786835302dafe3265549e2052d3265df7025fc997bc6249e3647fe7cf',
    'maturity': 'Partially Working; isolated, sampled native scope only',
}
for name, value in [('daily-report.md', report), ('events.json', json.dumps(events, indent=2)+'\n'),
                    ('metadata.json', json.dumps(metadata, indent=2)+'\n')]:
    path = WORK / name
    assert not path.exists()
    path.write_text(value, encoding='utf-8')
print('Prepared private consolidated report with two new clips and references to previously delivered media.')
