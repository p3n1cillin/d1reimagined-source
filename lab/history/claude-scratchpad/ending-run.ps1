# One unattended A Guardian Rises run to the Tower: boot -> replay -> chain (Divide, gate, Dock 13, full
# hangar clear) -> outro -> XP + completion + ship + scoreboard + phase 7 -> post-game tour -> CONTINUE
# (3 s hold) -> 165 -> Tower North with its squads, recorded with game audio.
#   ending-run.ps1 -Name sense58 [-WorldTable <file>] [-Postgame postgame5.sh] [-NoContinue] [-NoRecord]
# The lab must already be stopped. The served host/responder are whatever is in the served
# target; build and swap them separately, with the lab stopped.
param(
    [Parameter(Mandatory)][string]$Name,
    [string]$Scoreboard = 'F:/d1/d1preservation-main/cfw-staging/agr-postgame-scoreboard-arcadia-ratio30.draft.txt',
    [string]$Postgame = 'postgame5.sh',
    # The host world table: rows 0 and 165 serve the Tower with its own (squad-populated) list.
    [string]$WorldTable = 'F:\d1\d1preservation-main\cfw-staging\world-table-sense58.txt',
    # Stop at the post-game screens instead of holding CONTINUE.
    [switch]$NoContinue,
    # Extra D1_KEY=VALUE server variables, comma-joined, appended after the defaults (later wins).
    [string]$ExtraEnv = '',
    [switch]$NoRecord
)
$ErrorActionPreference = 'Stop'
$S = $PSScriptRoot -replace '\\', '/'
$C = 'F:\d1\d1preservation-main\cfw-staging'
# Stop the previous lab yourself first (by exact PID); this script only refuses to start over a running one.
if (Get-Process d1-activity-host, d1-responder, shadnet-server, shadPS4 -ErrorAction SilentlyContinue) { throw 'lab still running' }
foreach ($f in "$C\profile-push.bin", "$C\profile-push.item", "$C\profile-push.soid", 'F:\d1\tools\shadPS4\audio-tap\rec.flag') { if (Test-Path $f) { Remove-Item -LiteralPath $f -Force } }
New-Item -ItemType Directory -Force "$C\$Name" | Out-Null
Copy-Item "$C\sense35\breach-prop3-only.txt" "$C\$Name\"
$env:D1_AUDIO_TAP = 'F:\d1\tools\shadPS4\audio-tap'
$extra = 'D1_LOOT_GRANT_ROSTER=lab/history\2026-10-28-character-stats\roster-defense1200-attack200-claude,D1_AH_CAPTURE_CLIENT_UPDATES=2048,D1_AH_CAPTURE_CLIENT_UPDATES_AFTER_S=900,D1_AH_WORLD_PERIOD_MS=33.333333,D1_AH_SENSE_SCHEMAS=1:0x80800357,2:0x808005D9,3:0x80800830,4:0x80800387,0xC:0x80802C30,0x18:0x8080048A,0x1C:0x808005C3,0x27:0x80802B18,D1_PROFILE_PUSH_FILE=F:\d1\d1preservation-main\cfw-staging\profile-push.bin'
if ($WorldTable) { $extra += ",D1_AH_WORLD_TABLE=$WorldTable" }
if ($ExtraEnv) { $extra += ",$ExtraEnv" }
Start-Process pwsh -WindowStyle Hidden -ArgumentList @('-NoProfile', '-File', 'F:\d1\public-work\d1rs-claude\tools\ps4_setup\d1-agr-run.ps1', '-Name', $Name, '-Minutes', '300', '-Extra', $extra) -RedirectStandardOutput "$C\$Name\boot.log" -RedirectStandardError "$C\$Name\boot-error.log" | Out-Null
$bash = 'C:\Program Files\Git\bin\bash.exe'
$jobs = @(
    "until tasklist | grep -q shadPS4; do sleep 5; done; exec /f/d1/tools/d1-idle-guard.sh",
    "until ls -d F:/d1/d1preservation-main/cfw-staging/contained-boot/*-$Name >/dev/null 2>&1; do sleep 5; done; H=`$(ls -d F:/d1/d1preservation-main/cfw-staging/contained-boot/*-$Name)/activity-host.log; R=F:/d1/d1preservation-main/cfw-staging/$Name; cd F:/d1/public-work/d1rs-claude && python -u -I tools/ps4_setup/d1-agr-replay.py --run-dir `$R --host-log `$H --garage-clients `$R/breach-prop3-only.txt --boot-timeout 1800 > `$R/replay-console.log 2> `$R/replay-error.log; grep -q 'DIVIDE REACHED' `$R/replay-console.log && bash $S/chain2.sh $Name > `$R/chain.log 2>&1",
    "bash $S/$Postgame $Name $Scoreboard > $($C -replace '\\','/')/$Name/postgame.log 2>&1"
)
if (-not $NoRecord) { $jobs += "bash $S/capture51.sh $Name > $($C -replace '\\','/')/$Name/capture.log 2>&1" }
# continue-hold.sh waits for the capture tour to finish, so it needs the recording.
if (-not $NoContinue -and -not $NoRecord) { $jobs += "bash $S/continue-hold.sh $Name > $($C -replace '\\','/')/$Name/continue.log 2>&1" }
# Start-Process joins ArgumentList unquoted, so the script must be one quoted argument (the jobs hold no double quotes).
foreach ($j in $jobs) { Start-Process $bash -WindowStyle Hidden -ArgumentList "-c `"$j`"" | Out-Null }
"$Name launched at $(Get-Date -Format HH:mm:ss); logs in $C\$Name (replay-console, chain, postgame, capture, continue)"
