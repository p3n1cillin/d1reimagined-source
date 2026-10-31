$ErrorActionPreference='Stop'
$rt='F:\d1\<pc>-import\d1xenia\local'
$rpc=Join-Path $rt 'RPCS3_FINAL124_SMOKE001'
$logs=Join-Path $rt 'SIGNON_LOCAL\logs'
$private=Join-Path $rt 'PRIVATE_ENGINEERING'
. 'F:\d1\d1xenia\scripts\Final124RunHelpers.ps1'
function Read-Json([string]$path){ Get-Content -LiteralPath $path -Raw | ConvertFrom-Json }
function Save-NewJson([string]$path,$value){
    $stream=[IO.File]::Open($path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
    try{$bytes=[Text.UTF8Encoding]::new($false).GetBytes(($value|ConvertTo-Json -Depth 12));$stream.Write($bytes,0,$bytes.Length)}finally{$stream.Dispose()}
}
function Shot-Receipt([string]$path,[string]$observation){
    $item=Get-Item -LiteralPath $path
    [ordered]@{path=$item.FullName;at=$item.LastWriteTimeUtc.ToString('o');bytes=$item.Length;sha256=(Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant();actually_inspected=$true;observed=$observation}
}
$summary=Read-Json (Join-Path $logs 'FINAL124_JOIN168-summary.json')
$cleanup=Read-Json (Join-Path $logs 'JOIN168-cleanup-result.json')
$capture=Read-Json (Join-Path $logs 'JOIN168-rsx-capture-result.json')
$replay=Read-Json (Join-Path $rt 'RESEARCH\JOIN168-native-replay001\receipt.json')
$identities=@((Read-Json (Join-Path $logs 'FINAL124_JOIN168-processes.json')).processes)
$identities+=Read-Json (Join-Path $logs 'FINAL124_JOIN168-proxy-process.json')
$identities+=$replay.identity
if(@($identities|Where-Object{Get-D1OwnedProcess -Identity $_}).Count){throw 'An owned process remains live'}
if(-not $summary.completed -or -not $summary.keptStack -or -not $summary.returnedAfterUserProfileReply){throw 'Native profile handoff not accepted'}
if(-not $cleanup.all_owned_closed -or $cleanup.errors.Count -or -not $replay.owned_closed -or -not $replay.global_restored -or $replay.errors.Count){throw 'Cleanup not verified'}
if((Get-FileHash -LiteralPath (Join-Path $rpc 'config\config.yml')).Hash -ne '71C7E08939860F78C12A53D095DDC1CCD1525A46D1493714DEE6F9F85D229AB9'){throw 'Original global configuration mismatch'}
$now=[DateTime]::UtcNow.ToString('o')
$shots=@(
    Shot-Receipt (Join-Path $rpc 'screenshots\BLES01857\BLES01857_screenshot_2026_10_07_15_14_44.png') 'Outdoor cinematic behind urgent maintenance message.'
    Shot-Receipt (Join-Path $rpc 'screenshots\BLES01857\BLES01857_screenshot_2026_10_07_15_15_39.png') 'Loading after owned Cross dismisses urgent maintenance dialog.'
    Shot-Receipt (Join-Path $rpc 'screenshots\BLES01857\BLES01857_screenshot_2026_10_07_15_16_14.png') 'Unobstructed outdoor cinematic: rusted cars, trees, sky and distant structures. No movement/combat/character/equipment/full-mission acceptance.'
)
$replayShots=@(foreach($shot in $replay.screenshots){Shot-Receipt $shot.path 'Loading UI with small lower-right loading symbol; not the earlier outdoor reference. Matching outdoor replay and black/green reproduction unverified.'})
Save-NewJson (Join-Path $logs 'JOIN168-visual-inspection.json') ([ordered]@{at=$now;native_screenshots=$shots;replay_screenshots=$replayShots;native_capture=$capture;replay_parser='PASS: native parser reaches rendered output';outdoor_frame_match='NOT VERIFIED';capture_content='Replayed loading UI; no direct live screenshot of the successfully captured frame';corruption_reproduction='NOT VERIFIED';combined_graphics='NOT ACCEPTED';recording_full_decode='NOT RUN';scope='The inspected outdoor reference precedes the requested next-frame capture by about 24 seconds. A different replayed scene alone does not identify a replay defect.'})
Save-NewJson (Join-Path $rt 'RESEARCH\JOIN168-native-replay001\visual-inspection.json') ([ordered]@{at=$now;screenshots=$replayShots;native_full_parse='PASS: native parser reaches rendered output';scope='Private loading UI control; original process.json requested outdoor-control scope is superseded by this actual visual inspection.';matching_outdoor_frame='NOT VERIFIED';graphics='NOT ACCEPTED'})
foreach($item in @(@{path=(Join-Path $logs 'JOIN168-pre-stop-native.log');output=(Join-Path $logs 'JOIN168-native-acceptance.json');scope='Completed 60-second baseline and explicit next-frame capture checkpoint; successful native capture deliberately pauses. Native handoff verified, combined graphics/gameplay not accepted.'},@{path=(Join-Path $rt 'RESEARCH\JOIN168-native-replay001\pre-stop-native.log');output=(Join-Path $rt 'RESEARCH\JOIN168-native-replay001\native-log-inspection.json');scope='Bounded native replay of this one capture. Actual images show loading UI; matching outdoor scene not accepted.'})){
    $raw=Get-Content -LiteralPath $item.path -Raw
    $ids=@([regex]::Matches($raw,'VUID-[A-Za-z0-9_-]+|UNASSIGNED-Threading-[A-Za-z0-9_-]+')|ForEach-Object Value|Sort-Object -Unique)
    Save-NewJson $item.output ([ordered]@{at=$now;build=$capture.build;source=$item.path;source_sha256=(Get-FileHash -LiteralPath $item.path).Hash.ToLowerInvariant();validation_ids=$ids;device_lost=[regex]::Matches($raw,'VK_ERROR_DEVICE_LOST').Count;last_native_line=(Get-Content -LiteralPath $item.path -Tail 1);scope=$item.scope})
}
$olderShots=@(
    Shot-Receipt (Join-Path $rpc 'screenshots\BLES01857\BLES01857_screenshot_2026_10_07_14_54_40.png') 'Outdoor cinematic behind urgent message; inspected earlier in this continuation.'
    Shot-Receipt (Join-Path $rpc 'screenshots\BLES01857\BLES01857_screenshot_2026_10_07_14_56_01.png') 'Final baseline image shows loading; inspected earlier in this continuation.'
    Shot-Receipt (Join-Path $rt 'RESEARCH\JOIN167-evidence-inspection-final\59b08572014c4a849b5319b42e8e2be1-01.png') 'Video QA: loading with native overlays; inspected earlier in this continuation.'
    Shot-Receipt (Join-Path $rt 'RESEARCH\JOIN167-evidence-inspection-final\59b08572014c4a849b5319b42e8e2be1-07.png') 'Video QA: loading with native red diagnostic line; inspected earlier in this continuation.'
    Shot-Receipt (Join-Path $rt 'RESEARCH\JOIN167-evidence-inspection-final\d3125c6c2ffa4ed58771a882c913713d-offset0.3.png') 'Video QA at 0.3 seconds: loading with Contacting Destiny Servers; inspected earlier in this continuation.'
)
Save-NewJson (Join-Path $logs 'JOIN167-visual-inspection.json') ([ordered]@{at=$now;images=$olderShots;inner_summary=(Read-Json (Join-Path $logs 'FINAL124_JOIN167-summary.json'));outer_cleanup=(Read-Json (Join-Path $logs 'JOIN167-cleanup-result.json'));capture_saved=$false;scope='Outer fixture sampling return is not inner observer success. Incomplete observer was forcibly removed after sixty-second wait, closing its owned stack. Shutdown errors do not establish a renderer crash.';combined_graphics='NOT ACCEPTED'})
$state=@(Get-ChildItem -LiteralPath (Join-Path $rt 'content-capture\state') -File|ForEach-Object{Read-Json $_.FullName})
$pending=@($state|Where-Object status -in 'WAITING_FOR_NAS','FAILED_RETRYING','TRANSFERRING','VERIFYING').Count
$status=Read-Json (Join-Path $rt 'content-capture\machine-status.json')
$existing=Read-Json (Join-Path $private '2026-10-07-heartbeat-existing-handoff-1419.json')
$monitor=Read-Json (Join-Path $rt 'content-capture\monitor-process.json')
Save-NewJson (Join-Path $private '2026-10-07-heartbeat-checkpoint-1423.json') ([ordered]@{checked_at=$now;automation_id='daily-engineering-handoff';trigger_timestamp='2026-10-06T19:57:30.649Z';current_reporting_date='2026-10-07';timezone='Europe/London';october6_handoff_complete=$true;previous_ready_reverified=$existing;new_package_created=$false;media_resent=$false;previous_ready_modified=$false;pending_outbox=$pending;state_counts=@($state|Group-Object status|ForEach-Object{@{status=$_.Name;count=$_.Count}});capture_status_at=$status.updated_at;nas_connected=$status.NAS_CONNECTED;nas_available_bytes=$status.AVAILABLE_NAS_SPACE;monitor_identity=$monitor;monitor_alive=[bool](Get-D1OwnedProcess -Identity $monitor);safe_checkpoint_reached=$true;all_test_owned_closed=$true;native_run='FINAL124_JOIN168';native_cleanup=$cleanup;native_replay_cleanup=$replay;next_handoff='October 7 near 19:00 Europe/London, next safe checkpoint. Current local time is earlier than cutoff; do not duplicate the completed October 6 package.';late_evidence='Late October 6 and October 7 events retain actual dates for next eligible consolidated edition; reference prior packages without media resends.';next_engineering='Align captured frame and inspected reference; preserve actual loading replay outcome. Obtain useful outdoor control/fault pair and combined character/world/mission evidence.'})
$prefix=@'
## Current safe engineering checkpoint — CHECKPOINT_TIME

JOIN168 and its native replay are CLOSED, with no owned test process remaining. The active whole-game goal remains incomplete. This continuation has progressed: replay boot/display repairs and the short-join ownership repair are pushed; isolated run/join fixtures pass 173/19 checks, and short-join retention now has native verification. No new live-game graphics repair or full mission acceptance.

Source master 7b159d776b1a53a9afccd0cdbd9b50b08da13df7 and data main de956e57482d73deeb25c3372b19cb8652845b13 are pushed with exact remote heads verified and clean worktrees. Tested launcher source is 09dc8097ff6173c3c59d5af09a4642190d6118f7 (later change is documentation); renderer build cc73a62d8a079fca2ca3e995647c468485bb6095c548a95596de4e73a6c90397 changes only native capture replay relative to prior live-game renderer. Public October 7/readme/capture notes now record the actual JOIN168 scope. Imported unrelated work remains preserved.

JOIN167 exec13395 exit1: baseline180 sampling returns, but background observer is forcibly removed after its sixty-second join wait before eight-minute observation completes. Inner completed=false/keptStack=false; exact client closes before capture checkpoint. Outer fixture_completed=true describes only sampling return. Owned cleanup 13:57:08 UTC passes. Fatal debugger disconnect and native recorder trailer failure occur during owned shutdown; do not label a renderer crash. No marker/capture. Thirteen untouched indexed original hashes match; one unindexed failed original retained. Last two raw clips fully decode 33.3/2.39 seconds, 1280x720 with audio, reported average/base 26.9/30 and 29.73/30, not game FPS. Three actually inspected QA samples show loading (short clip includes Contacting Destiny Servers). Detailed visual/cancellation receipt now saved; older ACTIVE167 statements below are historical.

Runner 09dc809 adds opt-in ReturnAfterUserProfileReply: exact logged profile reply, ready stack and launcher success end observation successfully while retaining ownership. Join requires completed job, receives errors and rechecks exact client. Default full observations remain unchanged. Actual fixtures: run tests 14:04:16–14:06:20 UTC, 173 PASS/exit0; join capture/handoff tests 14:05:43–14:06:02 UTC, 19 PASS/exit0. These use isolated children/mock protocol responses, not game acceptance.

JOIN168 exec66152 exit0, exact client16096/ticks639269790474122438. Native summary at14:12:53 UTC records completed=true/keptStack=true/returnedAfterUserProfileReply=true, one actual profile reply. Same Titan/highway fixture, title read/write surfaces and depth/checkpoint ON, ring/text/FP OFF; shorter60s baseline, no20-menu sequence. Exact client/core loaded at14:14:47 capture checkpoint; runner retained it. Actual15_14_44 image: outdoor cinematic behind urgent dialog. Owned X Cross posted14:15:35, then15_15_39 loading;15_16_14 actual unobstructed outdoor cars/trees/sky/structures. No character/equipment/traversal/combat/fullmission claim.

Marker armed14:16:38.4708439 UTC against actually inspected outdoor reference (about24s earlier). Native capture succeeds14:16:39, 2524052 bytes SHA f17d48b7ec856e2cf8335d78a37a672d3ae9beaa430dafa32735fa32d34479dc, private runtime captures/BLES01857_20261007151639_capture.rrc.gz. Capture pauses intentionally, not a spontaneous stall. Gzip and prefix/reference checks PASS:1tile/246memory/197data/1display/11812commands,74240opaque register tail. Native full parser later reaches rendered output. Live cleanup14:16:44.0135439 UTC: original title/watch, XInput, controls[], RPCN, errors[]. Native last5:54.403783, zero validation IDs or device loss; scope short-run only.

Native replay001 exact10156/ticks639269794648452920, same cc73 build, actual native --no-gui --rsx-capture. Globals temporarily matched tested title (clear-title replay path), core loaded. Both actual15_18_05/15_18_16 images show LOADING UI, not the earlier outdoor reference. Captured next frame may differ; no direct live screenshot of successful captured frame. Outdoor match/corruption reproduction NOT VERIFIED; do not infer a replay defect from scene mismatch alone. Source process.json requested outdoor-control scope is superseded by actual visual-inspection receipt. Native last31.796560, zero validation IDs/device loss, alias warning still unproved. Replay closes14:18:17.3201609, original global71c7.../title restored, errors[]. Receipts RESEARCH/JOIN168-native-replay001 and logs/JOIN168-visual-inspection.json, JOIN168-native-acceptance.json. Captures/game bytes stay private and outside production packages. JOIN168 raw originals remain local; final full decoding/QA not yet run.

JOIN166 inactivity UI remains the accepted single control replay: two actual RGB-identical images a285953822c4c2b281d5a485b9672ece946d74e99360261223558794d909b47e. This does not establish outdoor/character/mission fidelity. Current build/PDB differs from historical JOIN163 symbols; do not rerun that unwind with new symbols. Do not broaden SPU reads or change appearance based on an unproved alias/generation hypothesis.

Next engineering: align the actual captured scene with a fresh reference (capture request/image are separated by human inspection/tool latency), then obtain comparable useful outdoor control and fault replay. Keep real character/equipment and outdoor/mission acceptance together. Consider dismissed urgent dialog before waiting for native mission-arrival gate; memory in_world is not visible gameplay. Review JOIN168 raw originals with existing helper before production evidence claims. No current native stack/build work needs interruption.

Heartbeat is an old October6 trigger. October6 consolidated ready package's three payloads reverified today without modification; pending outbox0, NAS healthy. No new report/package/media resend/publication. Current October7 local time precedes19:00 target; next eligible reporting edition near19:00 safe checkpoint preserves late Oct6/Oct7 dates and references prior event IDs. Detailed audit PRIVATE_ENGINEERING/2026-10-07-heartbeat-checkpoint-1423.json. Automation remains useful/active; no user action required.

Everything below is historical; old active handles, pending native handoff and pending replay statements are superseded.

'@
$prefix=$prefix.Replace('CHECKPOINT_TIME',$now)
foreach($name in '2026-10-07-progress.md','HANDOFF-2026-10-06-CLAUDE.md'){
    $path=Join-Path $private $name
    $old=[IO.File]::ReadAllText($path)
    [IO.File]::WriteAllText($path,($prefix+"`r`n"+$old),[Text.UTF8Encoding]::new($false))
}
'Private checkpoint recorded; existing handoff unchanged, current daily cutoff not yet due.'
