"""Prepare a private factual daily draft; does not publish or control a game."""
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess

private = Path(r'lab/history')
repo = Path(r'F:\d1\d1xenia')
runtime = Path(r'F:\d1\d1preservation-main')
created = datetime.now(timezone.utc)
draft = private / ('daily-2026-10-13-' + created.strftime('%Y%m%dT%H%M%S'))
draft.mkdir()
head = subprocess.check_output(['git','-C',str(repo),'rev-parse','HEAD']).decode().strip()
dirty = bool(subprocess.check_output(['git','-C',str(repo),'status','--porcelain']).strip())
previous = '<lab-pc>-20261012T201422-024b4f583aaa4b579f3e1357e1528b9d'
prior = Path(r'Y:\incoming\<lab-pc>\2026\10\12') / (previous+'.ready')
manifest = json.loads((prior/'hashes.json').read_text(encoding='utf-8'))
prior_checks = []
for name, pin in manifest['files'].items():
    path = prior/name
    sha = hashlib.sha256(path.read_bytes()).hexdigest()
    match = path.stat().st_size == pin['size'] and sha == pin['sha256']
    if not match: raise RuntimeError('Previous handoff verification failed')
    prior_checks.append({'file':name,'matches':match,'sha256':sha,'size':pin['size']})
forced = json.loads((runtime/'cfw-staging/contained-boot/20261013-165142-later-56578-forced-spawn/receipt.json').read_text(encoding='utf-8-sig'))
content = json.loads((runtime/'cfw-staging/2026-10-12/later-56578-content-validation-v2/acceptance.json').read_text(encoding='utf-8-sig'))
assert content['tableAndStoredBlockVerificationExit'] == 0 and content['manifestExit'] == 0
assert forced['emulatorExitedByItself'] and forced['emulatorExitCode'] == -1073741819
evidence = {
    'source_note': 'notes/ps4-retarget-2026-10-12.md',
    'content_receipt': 'cfw-staging/2026-10-12/later-56578-content-validation-v2/acceptance.json',
    'forced_spawn_receipt': 'cfw-staging/contained-boot/20261013-165142-later-56578-forced-spawn/receipt.json',
    'local_screenshots': []
}
for name in ['64-unfaded.png','65-moved.png']:
    image = runtime/'cfw-staging/2026-10-13-op'/name
    evidence['local_screenshots'].append({
        'path':str(image.relative_to(runtime)).replace('\\','/'),
        'file_modified_utc':datetime.fromtimestamp(image.stat().st_mtime,timezone.utc).isoformat(),
        'size':image.stat().st_size,'sha256':hashlib.sha256(image.read_bytes()).hexdigest(),
        'transferred':False,'visual_review_by_this_handoff':False})

report = f'''# 13 October 2026 — an experimental PS4 scene, with a confirmed crash

Candidate public update: An experimental run now reaches orbit and displays a game
scene. It still depends on temporary interventions and ends in a crash; stable
play remains unfinished.

This is private production evidence, not approved publication copy or a release.

## Reporting boundary

Reporting timezone: Europe/London. Date: 2026-10-13. Reporting begins after the
previous report's 2026-10-12T20:14:08Z evidence endpoint. The daily cutoff is
2026-10-13T18:00:00Z (19:00 UK). Source checkpoint at draft preparation: {head},
master; dirty worktree: {str(dirty).lower()}. The source note is a cumulative
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
{forced['startedUtc']}, end {forced['endedUtc']}, emulator exit code
{forced['emulatorExitCode']} (access violation), clean owned-server shutdown and
no guest crash report. Static analysis maps the null dereference to the entity
definition's vfunc 19, matching the documented structural failure of the older
forced path. The missing backpointer explanation is an engineering diagnosis,
not a verified repair. The receipt keeps nativeBootAccepted false.

## Tests and maturity

| Result | Actual scenario and scope | Maturity |
| --- | --- | --- |
| PASS | Test-D1BuildContent.ps1, completed {content['atUtc']}: all 2,119 extracted files match SHA-256; d1-pkg verify --blocks reports 2,105 packages / 964,151 stored-block hashes, zero failures. Acceptance receipt and verification tail were inspected for this handoff. | Fully Working: isolated content validation only. |
| PASS | Regenerated English manifest: 1,392 packages, 71,003 bytes, SHA-256 {content['manifestSha256']}. It exceeds the cap measured on 01.29; the later-build cap is not independently established. | Fully Working: reproducible selected manifest; native fidelity unaccepted. |
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

1. {evidence['source_note']}: cumulative build-specific observations, controls,
   bounded runs, failures, hypotheses and next measurement.
2. {evidence['content_receipt']}: closed content-only acceptance; actual UTC
   completion is 13 October even though its parent directory is dated 12 October.
3. {evidence['forced_spawn_receipt']}: closed native failure, exact build/patch
   identity and owned-child shutdown.
4. {evidence['local_screenshots'][0]['path']}: local still for the documented
   unfaded scene. Timestamp {evidence['local_screenshots'][0]['file_modified_utc']}.
5. {evidence['local_screenshots'][1]['path']}: local still after documented input.
   Timestamp {evidence['local_screenshots'][1]['file_modified_utc']}.

No clips, audio or screenshots are included in this text-only package. Timecodes
are not applicable. Local screenshot pointers are not transferable media files;
their existence/hashes were checked, but visual, crop, rights and audio reviews
were NOT RUN here. Windows frame captures may expose desktop edges and internal
identifiers. No recording or media decode pass is claimed. No before/after video
pair was recreated. No prior package's media is resent.

Earlier package reference: {previous}. Its three payloads were re-hashed against
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
'''

events = [
 {'event_id':'d1-20261013-full-owned-content','timestamp':content['atUtc'],
  'feature':'content-validation','maturity':'Fully Working (isolated content only)',
  'claim':'All 2,105 package tables and 964,151 stored-block hashes pass; native boot remains unaccepted.',
  'tests':{'files':2119,'packages':2105,'blocks':964151,'failures':0,'receipt_inspected':True},
  'source_reference':evidence['content_receipt'],'media':[]},
 {'event_id':'d1-20261013-orbit-and-protocol','timestamp':'2026-10-13T13:01:10Z',
  'timestamp_basis':'Run name start; duration and outcome from the engineering record.',
  'feature':'local-native-lab-orbit','maturity':'Experimental','synthetic_ancestor':True,
  'claim':'A bounded lab run reaches orbit; Director content and natural gameplay remain missing.',
  'source_reference':evidence['source_note'],'verification':'source-reported; not rerun','media':[]},
 {'event_id':'d1-20261013-forced-spawn-failure','timestamp':forced['endedUtc'],
  'started_at':forced['startedUtc'],'feature':'spawn-and-rendering','maturity':'Experimental / Known Broken',
  'synthetic_ancestor':True,'claim':'Documented forced scene/input response followed by confirmed access violation.',
  'tests':{'emulator_exit_code':forced['emulatorExitCode'],'servers_stopped':forced['serversStopped'],
           'native_boot_accepted':False,'closed_receipt_inspected':True},
  'local_evidence':evidence['local_screenshots'],'media':[]},
 {'event_id':'d1-20261013-served-spawn-measurement','timestamp':'2026-10-13T17:36:14Z',
  'timestamp_basis':'Recorded source commit time; not a completed-run timestamp.',
  'feature':'activity-lifetime-phase','maturity':'Experimental / Known Broken',
  'claim':'Snapshot records 97 world-container entries, no lifetime-class record; served phase still absent and outer-loop gate unresolved.',
  'active_run':'20261013-173323-later-56578-worldset','run_complete':False,
  'source_reference':evidence['source_note'],'verification':'source-reported committed snapshot; subsequent live evidence excluded','media':[]}
]
meta = {
 'recorded_at':created.isoformat(),'reporting_date':'2026-10-13','reporting_timezone':'Europe/London',
 'reporting_interval_start':'2026-10-12T20:14:08Z','reporting_interval_end':'2026-10-13T18:00:00Z',
 'daily_update_id':'destiny1-2026-10-13','development_session':'later-56578-native-lab-and-served-spawn',
 'topic':'experimental-native-scene-and-confirmed-spawn-crash','feature':'served-activity-world-and-player-spawn',
 'event_type':'GENERAL_PROGRESS','importance':'USEFUL',
 'description':'Private daily evidence: verified later-build content, synthetic orbit/scene, confirmed forced-spawn crash and unresolved served lifetime phase. Game remains unfinished.',
 'previous_state':'Exact-build preparation lacked a runnable client; earlier native later-build runs stalled before orbit.',
 'current_state':'The later owned build reaches synthetic orbit/in-world; forced scene/control is documented but crashes; served spawn remains blocked.',
 'remaining_issue':'Natural world-content and player spawn, entity-definition crash, Director content and complete gameplay.',
 'referenced_package_ids':[previous],'new_media_included':False,
 'media_decode':'NOT APPLICABLE: text-only report','publication_review_required':True,
 'tested_source_commit':head,'tested_source_branch':'master','tested_worktree_clean':not dirty,
 'native_boot_accepted':False,'synthetic_lab_client_observed':True,
 'active_native_test_excluded':'20261013-173323-later-56578-worldset',
 'cutoff_late_evidence_rule':'Only pre-cutoff recorded measurements included; later live-run evidence goes to the next eligible edition.',
 'late_delivery':created.hour >= 18
}
for filename, value in [('events.json',events),('metadata-input.json',meta),
                        ('previous-package-verification.json',prior_checks)]:
    (draft/filename).write_text(json.dumps(value,indent=2)+'\n',encoding='utf-8')
(draft/'daily-report.md').write_text(report,encoding='utf-8')
(private/'2026-10-13-draft-pointer.json').write_text(json.dumps({
    'draft':str(draft),'prepared_utc':created.isoformat(),'state':'WAITING_FOR_NATIVE_SAFE_CHECKPOINT',
    'active_native_pid':3612,'reporting_date':'2026-10-13'},indent=2)+'\n',encoding='utf-8')
print(json.dumps({'draft':str(draft),'source_head':head,'dirty':dirty,'report_bytes':(draft/'daily-report.md').stat().st_size}))
