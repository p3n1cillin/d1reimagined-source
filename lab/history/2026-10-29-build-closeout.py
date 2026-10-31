"""Consolidate today's two earlier ready snapshots without changing/resending them.
Uses the existing capture report command and durable outbox. No lab input.
"""
from pathlib import Path, PurePosixPath
from datetime import datetime, timezone
import json, subprocess, sys, hashlib
P=Path(__file__).parent
sys.path.insert(0,r'F:\d1\d1xenia\tools\content_capture')
from capture import Capture,lock,check_text
DATE='2026-10-29'
POINTER=P/(DATE+'-draft-pointer.json')
FIRST='<lab-pc>-20261029T153911-dcb6eb8933d040b4bd6ea1ad58b6980a'
TOWER='<lab-pc>-20261029T173936-f34dc117b28f4e4fb9325930a78e2c7b'
def sha(path):
    h=hashlib.sha256()
    with path.open('rb') as f:
        for chunk in iter(lambda:f.read(1024*1024),b''):h.update(chunk)
    return h.hexdigest()
def git(root,*args):return subprocess.check_output(['git','-C',str(root),*args],text=True).strip()
with lock(P/(DATE+'-delivery.lock'),timeout=1):
    if POINTER.exists():print(POINTER.read_text());raise SystemExit(0)
    cfg=json.loads((P.parent/'content-capture/config.json').read_text())
    cap=Capture(P.parent/'content-capture/config.json')
    parent=Path(cfg['CONTENT_NAS_PATH'])/'incoming'/cap.machine/'2026/10/29'
    assert parent.is_dir()
    # The first is an earlier daily snapshot; the second is a Tower event with
    # the same package_kind. A new immutable correction is required to replace
    # stale destination/spinner claims, not another media release/edition.
    for ready in parent.glob('*.ready'):
        m=json.loads((ready/'metadata.json').read_text())
        if m.get('reporting_date')==DATE and m.get('package_kind')=='engineering-report' and m['clip_id'] not in (FIRST,TOWER):
            cap.verify(ready)
            print(json.dumps({'state':'EXISTING_LATER_DAILY_REPORT_VERIFIED','destination':str(ready)}));raise SystemExit(0)
    verified=[]
    for identity in (FIRST,TOWER):
        ready=parent/(identity+'.ready')
        manifest=json.loads((ready/'hashes.json').read_text())
        expected=set(manifest['files'])|{'hashes.json'}
        actual={x.relative_to(ready).as_posix() for x in ready.rglob('*') if x.is_file()}
        assert actual==expected
        total=0
        for relative,pin in manifest['files'].items():
            q=PurePosixPath(relative);assert not q.is_absolute() and '..' not in q.parts
            payload=ready/relative;assert not payload.is_symlink()
            assert payload.stat().st_size==pin['size']>0 and sha(payload)==pin['sha256'],relative
            total+=pin['size']
        verified.append({'clip_id':identity,'payloads':len(manifest['files']),'bytes':total,
          'manifest_sha256':sha(ready/'hashes.json'),'all_payload_hashes_match':True,
          'new_decode':'NOT RUN; prior metadata records full decode','media_resent':False})
    stamp=datetime.now(timezone.utc).isoformat()
    source=Path(r'F:\d1\public-work\d1rs-claude')
    reporting=Path(r'F:\d1\public-work\d1reimagined-sense')
    lab_source={'commit':git(source,'rev-parse','HEAD'),'branch':git(source,'branch','--show-current'),
                'dirty':git(source,'status','--porcelain')}
    inspected={'commit':git(reporting,'rev-parse','HEAD'),'branch':git(reporting,'branch','--show-current'),
               'dirty':git(reporting,'status','--porcelain')}
    draft=P/(DATE+'-daily-closeout-'+datetime.now(timezone.utc).strftime('%H%M%S'));draft.mkdir()
    report=f'''# 29 October 2026 — from the hangar ending to a populated Tower diagnostic

Candidate public update: The opening mission now reaches its closing cinematic
and results screens. Continue can take the Guardian into the Tower, where movement
and initial character spawning have been tested. The intervening story sequence,
vendor interactions and accurate rewards still need work.

PRIVATE production input only. This report approves no publication or video edit.

## One consolidated edition and immutable evidence

Reporting date2026-10-29, Europe/London. Coverage2026-10-28T19:00:00Z through
2026-10-29T19:00:00Z; prepared{stamp}. Post-cutoff outcomes retain their dates
and roll into the next eligible edition. This is the consolidated closeout for
the same reporting day, not an additional release or invented episode.

This new identity supersedes the earlier15:39 textual daily snapshot
{FIRST}. Its files remain immutable and its media remains evidence. The17:39
Tower snapshot {TOWER} is referenced as a supporting event. Both older packages
were independently checked against EVERY payload's size/SHA256 and the manifest;
their verification counts are in metadata. None of their media is copied here.
Do not queue three editions from these three related package identities.

Corrections to earlier text: the Tower did load later today; the ring described
as a loading spinner was demonstrated to be the menu cursor. The empty equipment
Advancement list does not mean the900 character-XP delta was lost. These corrections
replace the earlier snapshot's open-issue wording without changing its files.

## Source/build scope and actual changes

Build: owned PS4 CUSA00219 build56578, contained emulator/host/backend lab with
existing private patches and mission schedules. Tests span different revisions;
no one commit alone reproduces all private inputs. Earlier evidence metadata
pins tested commits bd2addb7b3318385ceb1d8ff02af51e4223c6a09 and
56ff331fb3c0bd0fb05879af6da86c10ec992290.

At report preparation the lab-owner checkout was {lab_source['commit']}, branch
{lab_source['branch']}, status{lab_source['dirty'] or 'clean'}. Its local134a36c
hangar KnownWorld commit was NOT pushed/served/tested as a destination. Keep it
distinct from the tested Tower host and from the table bypass. The reporting
checkout is {inspected['commit']}, branch{inspected['branch']}, status
{inspected['dirty'] or 'clean'}. Automatic report-tool Git fields refer to its
legacy capture checkout; explicit metadata supplies these separate source pins.

1. Hangar/outro: the first outro occurred28Oct20:13:09–20:14:33Z, after the prior
daily cutoff. That early test used the diagnostic six-member cohort, not all
eight. On29Oct the authored encounter/task references released the two hidden
squads11/12 into reachable firing areas; the operator observed them move and
die. In sense53 the full explicit5–12 cohort reached exact zero at12:19:05Z.
The assignment/task timing is still a diagnostic reconstruction, not the
recovered original mission script. Preserve that distinction when describing
the now-reachable eight-squad clear.

2. Ending/results: the server-driven region105 and cinematic cast/play sequence
plays the full approximately84s outro without skip input. Post-game lifetime
phase support and the authority-frame padding fix allow ACTIVITY REWARDS and
MISSION SUMMARY to appear. Scoreboard rows must arrive before entering phase7.
Rows now show the test Guardian, mission title/location/time and reward display.
Counts50 kills/15 precision/2 deaths are SYNTHETIC; the30% ratio follows them,
not a measured real-session total. A cinematic's completion plus these diagnostic
results is not full reference-accurate mission/reward/persistence acceptance.

3. Continue: a lab-inherited completion/unlock state selected an alternate
sign-out branch. The corrected profile gate selects ordinary recall. The live
single-player control requires a roughly3s hold; a tap starts then aborts on
release. Earlier blanket seven-second advice was superseded by the live result.
The sense57 monitor records native countdown17:33:47.816, handler31/goal27 at
17:33:50.874, desired165 at17:33:53.884, BAP42/follow165 at17:33:54.24, and AH
publish/world-row/join165 at17:33:55.848. No per-hit or input fix is inferred
from a codec round trip alone.

4. Tower entry: sense55 initially used the new AH instance's default First Light
world because165 had no matching world-table row. An initial Tower attempt then
stalled because the template sensor namespace/lifetime index was wrong. PR85
allows the list's owned lifetime row to specify its subindex; the Tower's common
group is40aebf90, lifetime sub2, not the mission's4786c0e0/sub3. The operator
reported105/105 host tests passing. With its OWN live100-client pending list,
sense57 entered Tower North: sense update17:34:03Z, entry incidents17:34:11Z,
location banner and Guardian standing with HUD, still present at+120s.

IMPORTANT: serving Tower on published activity165 is a LABELLED BYPASS of165's
unimplemented cinematic stage. Content identifies165→activity0/Tower, but does
not establish that165's own world is Tower. The original intervening cinematic
and automatic authentic chain remain open. act_cosmo_m0_5_hangar is only an
owned-content-derived candidate; its local KnownWorld is not a validated165 fix.

5. Tower movement and population: the operator later walked through the courtyard
and interior and rotated the camera. Default squad images yielded an empty
Tower. At17:52 a live diagnostic copied a hangar squad activation image to15
Tower North squads, and each changed0→1; civilians and a Frame appeared. This
is inferred activation, not an authentic Tower quest/calendar/vendor schedule.
At17:58 the Frame was identified on screen as Patches99-40, Research Assistant,
with Approach prompt. Approach did not open a vendor screen or send vendor904;
whether that NPC is a transactional vendor is unresolved. Do not call it the
Postmaster or claim working buying/selling. The actual Postmaster is in another
regional group. Other regions/vendors still need their own resident pending lists.

6. Static population work recovered all15 North squad names and authored variant
ordering. A revised UNSERVED diagnostic enables separate normal civilian slots
while retaining zero for alternate slots; that is an explicit reconstruction
choice, not original desired counts. Parser/schema controls accepted99 non-lifetime
clients and15 typed squads, encoding2184B for team0 and team-1. These controls
prove encodability, not live acceptance of the revised population draft.

7. XP/rewards: the collector must seed from activity-start state before XP rises.
The sense56/57 live cached post-game data reads initiallevel1/fraction1.0,
finallevel2/fraction2.0, gained900, remaining0; collector0,900,900. Thus character
XP was recorded. The empty Upgrades list is separate EQUIPPED-item advancement.
Static owned UI graphs bind that list to equipment-upgrade rows and list count;
no direct character-XP binding was found in the inspected default body. A missing
character bar is not proved to be a max-level40 gate. The exact selected live
Advancement body/visibility path remains to capture before Continue destroys it.
The18:52 bounded tree probe saw3 current screens/712nodes/0errors/0pending and no
post_game_data, consistent with already having left post-game. It does not test
the absent Advancement screen retroactively. Do not replay XP as a guessed fix.

The jumpship is displayed as a reward in sense54 and was sent as an owned item
in a later completion push. Inventory/equipping/persistence acceptance was NOT
established by the inspected handoffs. Display and ownership are distinct claims.

## Tests, maturity and limits

- PASS within recorded scope: full eight-member diagnostic cohort clear,
  natural outro completion, results display, ordinary hold departure and Tower
  spawn via the explicit bypass. These are operator native/log/capture results.
- PASS, operator-reported focused host suite for PR85:105/105. No new broad
  suite rerun by this reporting pass; do not aggregate unrelated historical counts.
- PASS, static parser/codec controls for the revised Tower draft, both teams;
  bounded current-tree probe, and independent earlier-package payload hashes.
- Corrected/closed: the alleged persistent loading spinner is the menu cursor,
  demonstrated by stick movement; no spinner workaround is needed.
- Partially Working: complete diagnostic mission flow, results/rewards, Tower
  entry/movement and basic NPC activation. Their original authored state is not
  fully reproduced; statistics/gear remain synthetic in places.
- Experimental: hidden-squad assignment, revised Tower variant counts and165
  hangar-world candidate. The revised Tower/hangar drafts were not served by
  the static researcher. Codec PASS is never live-world/cinematic PASS.
- NOT ESTABLISHED: original165 cinematic/automatic Tower chain, working vendor
  transaction screens, correct quest population, visible character XP bar,
  authentic mission statistics, ship equipment persistence or full stock-game
  acceptance. No performance improvement is inferred from smoother footage.

## Evidence index — reuse earlier packages

Package{FIRST}, original daily-report/events index:
- clips/clip01.mp4,0–80s: Dock13 arrival/traversal, source12:02:00–12:03:20Z.
- clips/clip02.mp4,0–85s: final hangar fight and full cohort clear;12:17:45–12:19:10Z.
- clips/clip03.mp4,0–95s: outro,12:19:45–12:21:20Z, cinematic starts12:19:49.7,
  completes12:21:14.2. No skip input.
- clips/clip04.mp4,0–74.834s: rewards/Mission Summary/Advancement in sense54,
 13:12:48–13:14:03Z. Reward display only in this capture, not later item grant.
- clips/clip05.mp4,0–60s: Continue and spawn into the LUNAR FALLBACK world,
 15:09:08–15:10:08Z. Do not edit the story to imply this clip arrived in Tower.
- screenshots01–08: original package index describes exact subjects/times.
  Original capture metadata:1920×1080,H264/AAC, approximately29.4–29.89fps.

Package{TOWER}:
- clips/clip.mp4,0–70s: separate sense57 Tower entry after Continue. Original
  metadata:1920×1080,H264/AAC,28.74averagefps; game stereo48kHz, no microphone.
- screenshots01–03: original Tower-entry candidates. Use its own events/report
  index for source timecodes, not this package's transfer timestamp.

Both earlier metadata sets report full decode at packaging. This pass verified
all sizes/hashes, not a new playback/audio/full-decode or clean-image review.
The later17:52 NPC activation/17:58 Approach observations are operator evidence
in private sense57 captures, NOT shown by the earlier17:39 Tower-entry clip.
Their media is not selected/reviewed/copied by this closeout. No matched
before/after pair or new screenshot crop is supplied. The report's new payloads
are text/JSON only. No raw game content, secrets, packets or instruction file.

## Privacy/audio/rights and next engineering priority

Existing media includes the game HUD, project test character and possible game
music; original report mentions a capture border requiring production review.
No microphone was reported. Reassess privacy, picture/audio, music/rights and
cropping before use; hash integrity is not publication clearance. Public copy
must omit private identifiers, implementation details and model/provider names.
No production editing or publishing has been done here.

Next: owner-controlled165 hangar diagnostic using its own pending list/lifetime,
while preserving validated Tower0 and other world rows; first accept real
scenario readiness/state36/spawn, then separately verify cinematic/cast/cues
and chain165→0. A parked empty hangar alone cannot disprove the candidate.
Continue authentic Tower population/vendor data and selected-Advancement UI
tracing; no guessed XP replay, global NPC generation bump or unlock-all buff.

Transfer uses the existing content-capture report command, durable local outbox,
NAS hash verification and uploading→ready rename. Receipt records exact timings,
capacity and pending outbox. All older ready directories remain untouched. Active
lab ownership/input is unchanged; post-cutoff engineering resumes independently.
'''
    events=[
      {'event_id':'d1-20261029-full-cohort','timestamp':'2026-10-29T12:19:05Z','feature':'hangar-encounter','maturity':'Partially Working','claim':'Full5–12 cohort clears after inferred task assignment; not recovered authentic script.','referenced_package_id':FIRST,'evidence_file':'clips/clip02.mp4'},
      {'event_id':'d1-20261029-outro','timestamp':'2026-10-29T12:21:14.2Z','feature':'outro-and-results','maturity':'Partially Working','claim':'Outro completes; diagnostic scoreboard/rewards follow.','referenced_package_id':FIRST,'evidence_file':'clips/clip03.mp4'},
      {'event_id':'d1-20261029-tower','timestamp':'2026-10-29T17:34:11Z','feature':'tower-entry','maturity':'Partially Working','claim':'Tower spawn via labelled activity165 bypass; original intervening cinematic unimplemented.','referenced_package_id':TOWER,'evidence_file':'clips/clip.mp4'},
      {'event_id':'d1-20261029-npc','timestamp':'2026-10-29T17:52:00Z','timestamp_basis':'operator minute-level observation','feature':'tower-diagnostic-population','maturity':'Experimental','claim':'15 North squad counts0→1 after copied activation image; revised variant-count draft remains unserved.','evidence_scope':'operator private captures, not included'},
      {'event_id':'d1-20261029-xp-binding','timestamp':'2026-10-29T18:52:00Z','timestamp_basis':'bounded probe minute-level observation','feature':'advancement-investigation','maturity':'Experimental','claim':'XP900 cached; equipment list separate; current UI no longer post-game. No proved character-bar predicate.'}]
    meta={'recorded_at':stamp,'reporting_date':DATE,'reporting_timezone':'Europe/London',
      'reporting_interval_start':'2026-10-28T19:00:00Z','reporting_interval_end':'2026-10-29T19:00:00Z',
      'daily_update_id':'d1-engineering-2026-10-29','supersedes_clip_id':FIRST,
      'correction_reason':'Consolidate already-delivered daily and Tower event; correct stale destination/spinner wording and add pre-cutoff NPC/XP findings without resending media.',
      'referenced_package_ids':[FIRST,TOWER],'referenced_integrity':verified,
      'feature':'daily-consolidated-closeout','event_type':'GENERAL_PROGRESS','importance':'MAJOR',
      'description':'Consolidated29Oct private closeout: diagnostic full-cohort/outro/results, Tower bypass arrival and NPC activation; original165 stage/vendors remain open.',
      'previous_state':'Earlier same-day snapshot ended at lunar fallback and misidentified the menu cursor as loading.',
      'current_state':'Tower entry/movement and diagnostic population observed; XP cached; authentic intervening cinematic and vendor transactions unresolved.',
      'remaining_issue':'Authentic165 world/cinematic/chain, original encounter/population states, vendor screens, character-bar presentation and reward persistence.',
      'lab_owner_source_snapshot':lab_source,'reporting_source_snapshot':inspected,
      'media_resent':False,'new_media_count':0,'new_decode':'NOT RUN; no new media',
      'late_evidence_rule':'Only events before19:00Z included; later outcomes roll forward with actual timestamps.',
      'publication_review_required':True}
    cfg.update(CURRENT_GAME_BUILD='PS4 CUSA00219 build56578',CURRENT_SESSION='daily-2026-10-29-closeout',CURRENT_FEATURE='daily-consolidated-closeout',CURRENT_DEVBLOG=None)
    for name,value in (('config.report.json',cfg),('metadata-input.json',meta),('events.json',events),('reference-integrity.json',verified)):
        (draft/name).write_text(json.dumps(value,indent=2)+'\n',encoding='utf-8')
    (draft/'daily-report.md').write_text(report,encoding='utf-8')
    for name in ('metadata-input.json','events.json','daily-report.md'):check_text((draft/name).read_text())
    job={'state':'PREPARED_WAITING_FOR_SAFE_CHECKPOINT','draft':str(draft),'prepared_at':stamp,'reporting_date':DATE,'supersedes_clip_id':FIRST}
    POINTER.write_text(json.dumps(job,indent=2)+'\n',encoding='utf-8');print(json.dumps(job))
