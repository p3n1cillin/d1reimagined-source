# One unattended A Guardian Rises run to the Tower: boot -> replay -> chain (Divide, gate, Dock 13, full
# hangar clear) -> outro -> XP + completion + ship + scoreboard + phase 7 -> post-game tour -> CONTINUE
# (3 s hold) -> 165 -> Tower North with its squads and vendors, recorded with game audio.
#   ending-run.ps1 -Name sense73 [-WorldTable <file>] [-Postgame postgame5.sh] [-NoContinue] [-NoRecord] [-NoVendors]
#     [-ClientPatch <xml>] [-ExtraEnv D1_INACTIVITY_MS=86400000] [-BreachFallen] [-JunctionFallen] [-Waves] [-Steps <schedule>]
# The defaults recreate run sense73 (31 Oct 2026). Served fixtures come from ps4/fixtures/56578
# (D1_FIXTURES); run output goes to the staging folder (D1_STAGING). The lab must already be stopped.
# The served host/responder are whatever is in the served target; build and swap them separately,
# with the lab stopped.
param(
    [Parameter(Mandatory)][string]$Name,
    [string]$Scoreboard = (& "$PSScriptRoot\..\..\tools\ps4_setup\D1Paths.ps1" Fixtures 'postgame\agr-postgame-scoreboard-arcadia-ratio30.draft.txt'),
    [string]$Postgame = 'postgame5.sh',
    # The host world table: rows 0 and 165 serve the Tower with its own (squad-populated) list.
    [string]$WorldTable = (& "$PSScriptRoot\..\..\tools\ps4_setup\D1Paths.ps1" Fixtures 'mission\world-table-tower.txt'),
    # The roster cards served after the Breach rifle claim (the loot grant).
    [string]$LootRoster = (& "$PSScriptRoot\..\..\tools\ps4_setup\D1Paths.ps1" Fixtures 'characters\roster-def1200-helm-saint14'),
    # Unlock flags as run sense73 served them, 308 and 3391 included; it reached the Tower through 165.
    # Run sense66 cleared both and reached the social Tower instead (D1_AH_ACTIVITY_REMAP=165:0; notes/
    # ps4-56578-tower-social-vendors-2026-10-30.md): pass that list and -ExtraEnv to do the same.
    [string]$UnlockFlags = '98,308,419,440,834,2668,3391,5021,5998,6029,433,2539,2549,3558,3587,3520,3527,3531,3533,3534,3535,3536,3544,3568,3594,3595,305,2669,2670,3389',
    # The d1-oracle build the responder encodes vendor purchases with.
    [string]$Oracle = (& "$PSScriptRoot\..\..\tools\ps4_setup\D1Paths.ps1" Workspace 'd1-server\target\release\d1-oracle.exe'),
    # Stop at the post-game screens instead of holding CONTINUE.
    [switch]$NoContinue,
    # Leave out the account, Glimmer and vendor purchase serving.
    [switch]$NoVendors,
    # Extra D1_KEY=VALUE server variables, comma-joined, appended after the defaults (later wins).
    [string]$ExtraEnv = '',
    # The client patch (empty: d1-lab-served.ps1's default, lab-orbit.xml); e.g. the
    # ps4/patches/CUSA00219-56578-lab-orbit-idle-probe.xml inactivity probe.
    [string]$ClientPatch = '',
    [switch]$NoRecord,
    # After the rifle, serve the Breach Fallen (mission/agr-b-graveyard-fallen.txt) and fight them
    # before walking on (d1-agr-replay.py --breach-fallen).
    [switch]$BreachFallen,
    # After the junction, serve the squads by the junction and the garage (mission/
    # agr-b-graveyard-junction.txt) and fight them on the way to the garage (--junction-fallen).
    [switch]$JunctionFallen,
    # Serve the Breach by its own triggers instead (mission/agr-steps-divide-waves.txt, made with
    # tools/ps4_setup/d1-breach-waves.py; issue #185) and fight what each wave places (--waves).
    [switch]$Waves,
    # Serve another mission schedule (its lists beside it), e.g. a staged experiment.
    [string]$Steps = '',
    [string]$Bash = 'C:\Program Files\Git\bin\bash.exe'
)
$ErrorActionPreference = 'Stop'
$Repo = (Resolve-Path "$PSScriptRoot\..\..").Path
. "$Repo\tools\ps4_setup\D1Paths.ps1"
$S = $PSScriptRoot -replace '\\', '/'
$RepoFwd = $Repo -replace '\\', '/'
$C = Get-D1Path Staging
$Fix = Get-D1Path Fixtures   # not $F: PowerShell names ignore case, and the loop below uses $f
$Cf = $C -replace '\\', '/'
# Stop the previous lab yourself first (by exact PID); this script only refuses to start over a running one.
if (Get-Process d1-activity-host, d1-responder, shadnet-server, shadPS4 -ErrorAction SilentlyContinue) { throw 'lab still running' }
if (-not $env:D1_AUDIO_TAP) { $env:D1_AUDIO_TAP = Get-D1Path ShadPS4 '..\audio-tap' }
foreach ($f in "$C\profile-push.bin", "$C\profile-push.item", "$C\profile-push.soid", "$env:D1_AUDIO_TAP\rec.flag") { if (Test-Path $f) { Remove-Item -LiteralPath $f -Force } }
New-Item -ItemType Directory -Force "$C\$Name" | Out-Null
Copy-Item "$Fix\mission\breach-prop3-only.txt" "$C\$Name\"
$extra = "D1_LOOT_GRANT_ROSTER=$LootRoster,D1_AH_CAPTURE_CLIENT_UPDATES=2048,D1_AH_CAPTURE_CLIENT_UPDATES_AFTER_S=900,D1_AH_WORLD_PERIOD_MS=33.333333,D1_AH_SENSE_SCHEMAS=1:0x80800357,2:0x808005D9,3:0x80800830,4:0x80800387,0xC:0x80802C30,0x18:0x8080048A,0x1C:0x808005C3,0x27:0x80802B18,D1_PROFILE_PUSH_FILE=$C\profile-push.bin"
if ($WorldTable) { $extra += ",D1_AH_WORLD_TABLE=$WorldTable" }
if (-not $NoVendors) {
    # Glimmer 25,000 (slot 308), Silver 500 (309) and Legendary Marks 200 (310) in the account; the
    # responder applies vendor purchases itself (notes/ps4-56578-vendor-purchase-2026-10-30.md).
    $V = "$Fix\vendor"
    $extra += ",D1_ACCOUNT_FILE=$V\account-currencies.bin,D1_ACCOUNT_PUSH_FILE=$C\account-push.bin"
    $extra += ",D1_VENDOR_SALES=$V\vendor-sales.tsv,D1_VENDOR_PROFILE_OBJ=$V\vendor-profile.obj,D1_VENDOR_ITEM_TEMPLATE=$V\vendor-item-template.obj"
    $extra += ",D1_VENDOR_ACCOUNT_SLOTS=308:869:25000,309:873:500,310:4548:200,D1_ORACLE=$Oracle"
}
if ($ExtraEnv) { $extra += ",$ExtraEnv" }
$agr = @('-NoProfile', '-File', "$Repo\tools\ps4_setup\d1-agr-run.ps1", '-Name', $Name, '-Minutes', '300', '-UnlockFlags', $UnlockFlags, '-Extra', $extra)
if ($ClientPatch) { $agr += '-ClientPatch', (Resolve-Path $ClientPatch).Path }
if ($Steps) {
    # With -Waves, a staged copy of the waves schedule (e.g. with the Ghost's states added).
    $agr += '-Steps', (Resolve-Path $Steps).Path
}
if ($Waves) {
    if ($BreachFallen -or $JunctionFallen) { throw '-Waves replaces -BreachFallen and -JunctionFallen' }
    if (-not $Steps) { $agr += '-Steps', (Join-Path $Fix 'mission\agr-steps-divide-waves.txt' -Resolve) }
    # Ten more lists for the host to build at start (about 40 s each with their squad states):
    # sense105's host was ready around the default 900 s limit.
    $agr += '-HostTimeoutSeconds', '1800'
}
Start-Process pwsh -WindowStyle Hidden -ArgumentList $agr -RedirectStandardOutput "$C\$Name\boot.log" -RedirectStandardError "$C\$Name\boot-error.log" | Out-Null
$Fallen = if ($BreachFallen) { ' --breach-fallen ' + ((Join-Path $Fix 'mission\agr-b-graveyard-fallen.txt' -Resolve) -replace '\\', '/') } else { '' }
if ($Waves) { $Fallen += ' --waves' }
if ($JunctionFallen) { $Fallen += ' --junction-fallen ' + ((Join-Path $Fix 'mission\agr-b-graveyard-junction.txt' -Resolve) -replace '\\', '/') }
$jobs = @(
    "until ls -d $Cf/contained-boot/*-$Name >/dev/null 2>&1; do sleep 5; done; H=`$(ls -d $Cf/contained-boot/*-$Name)/activity-host.log; R=$Cf/$Name; cd $RepoFwd && python -u -I tools/ps4_setup/d1-agr-replay.py --run-dir `$R --host-log `$H --garage-clients `$R/breach-prop3-only.txt --boot-timeout 1800$Fallen > `$R/replay-console.log 2> `$R/replay-error.log; grep -q 'DIVIDE REACHED' `$R/replay-console.log && bash $S/chain2.sh $Name > `$R/chain.log 2>&1",
    "bash $S/$Postgame $Name $($Scoreboard -replace '\\','/') > $Cf/$Name/postgame.log 2>&1"
)
# The keep-alive starts in gameplay: an early press can carry the client past the title before the
# activity host listens (about 9 minutes), and it then fails to reach it (run sense75). With the idle
# limit raised (-ExtraEnv D1_INACTIVITY_MS=...) it isn't needed, and its presses can't land on the
# post-game screens (run sense90 left them by itself before the CONTINUE hold).
if ($ExtraEnv -notmatch 'D1_INACTIVITY_MS=') {
    $jobs += "until grep -q 'in gameplay' $Cf/$Name/launcher.log 2>/dev/null; do sleep 5; done; exec $RepoFwd/tools/ps4_setup/d1-idle-guard.sh"
}
if (-not $NoRecord) { $jobs += "bash $S/capture51.sh $Name > $Cf/$Name/capture.log 2>&1" }
# continue-hold.sh waits for the capture tour to finish; with -NoRecord there is none to wait for.
if (-not $NoContinue) { $jobs += "bash $S/continue-hold.sh $Name$(if ($NoRecord) { ' --no-record' }) > $Cf/$Name/continue.log 2>&1" }
# Start-Process joins ArgumentList unquoted, so the script must be one quoted argument (the jobs hold no double quotes).
foreach ($j in $jobs) { Start-Process $Bash -WindowStyle Hidden -ArgumentList "-c `"$j`"" | Out-Null }
"$Name launched at $(Get-Date -Format HH:mm:ss); logs in $C\$Name (replay-console, chain, postgame, capture, continue)"
