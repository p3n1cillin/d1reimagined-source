param(
    [Parameter(Mandatory)][string]$Name,
    # Extra NAME=VALUE server variables (comma-joined), appended to the tutorial set.
    [string]$Extra = '',
    # Optional execute breakpoint (eboot-relative) and register peeks for d1-watchwrite.
    [string]$WatchExec = '',
    [string[]]$Peek = @(),
    [int]$WatchSeconds = 220,
    # Raw d1-watchwrite arguments (e.g. --read ADDR) instead of -WatchExec.
    [string[]]$WatchArgs = @(),
    # Promote a verified full schedule with d1-agr-steps.py --promote-steps first.
    [string]$Steps = 'F:\d1\d1preservation-main\cfw-staging\agr-steps-divide.txt',
    # The served profile. profile-tutorial-t.bin is the unarmed tutorial Guardian (no weapons in the
    # inventory or the roster) who owes the starter Khvostov as pending loot.
    [string]$Profile = 'F:\d1\d1preservation-main\capture-rig\builds\later-56578\profile-tutorial-t.bin',
    # -Geared: the old fixture (geared inventory and roster, hand cannon in hand, no rifle grant).
    [switch]$Geared,
    # Session length passed to d1-lab-served.ps1 (its default is 90 minutes).
    [int]$Minutes = 90,
    # A cold shader/content load can take longer than the old 200-second world wait.
    [ValidateRange(1, 3600)][int]$WorldTimeoutSeconds = 900
    # The host encodes every mission-step list before it listens; a long schedule (25 steps with
    # dropship pilot images) took about 6 minutes in run sense41, past the old fixed 300 s.
    [ValidateRange(1, 3600)][int]$HostTimeoutSeconds = 900
)
# A Guardian Rises tutorial lab boot: title (X), skip the opening Mars
# cinematic in character:prologue (X, then O - left playing it can trip the BLOCK_DETECT watchdog),
# then skip the revival and wake-up cutscenes (the host ends them on incident 1726) and wait for the
# teleport to 112. Keep the emulator window focused: input only reaches the focused window.
$ErrorActionPreference = 'Continue'
$existing = @(Get-Process shadPS4, d1-activity-host, d1-responder, shadnet-server -ErrorAction SilentlyContinue)
if ($existing.Count) { throw "Refusing to replace an active lab run (process ids: $($existing.Id -join ', ')). End that run first." }
if (-not (Test-Path -LiteralPath $Steps -PathType Leaf)) { throw "Missing private mission schedule: $Steps" }
$runStartedAt = Get-Date
Start-Sleep 3
$B = 'F:\d1\d1preservation-main\capture-rig\builds\later-56578'
$D = "F:\d1\d1preservation-main\cfw-staging\$Name"
New-Item -ItemType Directory -Force $D | Out-Null
# Milestones also go to RUN\launcher.log: d1-agr-replay.py waits there for "in gameplay (112)" even
# when nobody redirected this script's output (run divide22 waited forever for a boot.log).
$LaunchLog = "$D\launcher.log"
Set-Content -LiteralPath $LaunchLog -Value "launch $(Get-Date -Format o) $Name"
function Say($m) { $m; Add-Content -LiteralPath $LaunchLog -Value $m }
$L = 'F:\d1\tools\shadPS4\0.19.0\user\log\shad_log.txt'
$pad = 'F:\d1\tools\d1-pad.ps1'
# The Breach props (lights, barrier, rifle) are created by the client only while it holds their
# authority block: every block assigned to machine 3 (the client's machine in bubble 3), not yet
# applied, so the client acknowledges it and keeps it (notes 26 Oct).
$x = "D1_AH_MISSION_STEPS=$Steps,D1_AH_SENSOR_AUTHORITY=3,1,0"
# The live control file: d1-agr-replay.py opens the garage door through it (`clients GROUP PATH`).
# Run 18 was launched without it, and its replay stopped at the garage with the list unconsumed.
if ($Extra -notmatch 'D1_AH_CONTROL_FILE=') {
    $x += ",D1_AH_CONTROL_FILE=F:\d1\d1preservation-main\cfw-staging\ah-control.txt"
}
if (-not $Geared) {
    # The unarmed tutorial: weaponless inventory and peer roster. Claiming the Breach rifle (601)
    # grants the Khvostov (item + profile with the primary slot unlocked) and equips it (peer cards).
    $GD = "$B\grants"
    $x += ",D1_INVENTORY=$B\inventory-tutorial,D1_ROSTER_DIR=$B\rosters-f0-tutorial"
    $x += ",D1_LOOT_GRANT=0x827dbaf0=0xd100000000000401:$GD\khvostov-3598.bin|$GD\profile-tutorial-th.bin"
    $x += ",D1_LOOT_GRANT_ROSTER=$GD\rosters-f0-tutorial-khv"
}
if ($Extra) { $x += ",$Extra" }
Start-Process pwsh -WindowStyle Hidden -ArgumentList @('-NoProfile', '-File', 'F:\d1\tools\d1-lab-served.ps1', '-Name', $Name,
    '-Profile', $Profile, '-Minutes', "$Minutes",
    '-UnlockFlags', '98,308,419,440,834,2668,3391,5021,5998,6029,433,2539,2549,3558,3587,3520,3527,3531,3533,3534,3535,3536,3544,3568,3594,3595,305,2669,2670,3389',
    '-Extra', $x)
function WaitFor($pattern, $seconds) {
    $t = 0
    while ($t -lt $seconds) {
        $client = @(Get-Process shadPS4 -ErrorAction SilentlyContinue | Where-Object { $_.StartTime -ge $runStartedAt })
        if ($client.Count -eq 1 -and (Test-Path $L) -and (Get-Item -LiteralPath $L).LastWriteTime -ge $runStartedAt -and
            (Select-String -Path $L -Pattern $pattern -Quiet)) { return $true }
        Start-Sleep 3; $t += 3
    }
    Say "TIMEOUT waiting for $pattern"; return $false
}
function Skip {
    & $pad -Keys 'Hold:A:80,Hold:D:80' | Out-Null
    & $pad -Keys NumPad2 | Out-Null; Start-Sleep 1; & $pad -Keys NumPad6 | Out-Null
}
function WaitForHost($seconds) {
    $deadline = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $deadline) {
        foreach ($listener in @(Get-NetTCPConnection -LocalPort 4110 -State Listen -ErrorAction SilentlyContinue)) {
            $server = Get-Process -Id $listener.OwningProcess -ErrorAction SilentlyContinue
            if ($server -and $server.StartTime -ge $runStartedAt -and
                $server.Path -eq 'F:\d1\d1preservation-main\d1-server\target\release\d1-activity-host.exe') {
                Say "activity host ready (pid $($server.Id), port 4110)"
                return $true
            }
        }
        Start-Sleep 3
    }
    Say 'TIMEOUT waiting for this run''s activity host on port 4110'
    return $false
}
Start-Sleep 20
if (-not (WaitFor "Entering state 'bootflow:start'" 300)) { exit 2 }
# A cold host can finish its codec setup after the client reaches the title screen.
# Starting the activity first produces Currant and leaves an error dialog waiting for input.
if (-not (WaitForHost $HostTimeoutSeconds)) { exit 2 }
Start-Sleep 6; & $pad -Keys NumPad2 | Out-Null
if (WaitFor "Entering state 'character:prologue'" 300) {
    # A skip can be lost (focus, or pressed before the cinematic accepts input): retry until the
    # client leaves the prologue (run agr-breach12 sat in it for ten minutes).
    for ($k = 0; $k -lt 4; $k++) {
        Start-Sleep 8; Skip
        if (WaitFor "Leaving state 'character:prologue'" 20) { Say 'prologue skipped'; break }
    }
}
$w = $null
if ($WatchExec) {
    $a = @('-I', 'F:\d1\tools\d1-watchwrite.py', '--seconds', "$WatchSeconds", '--max-hits', '60', '--exec', $WatchExec, '--out', "$D\watch.jsonl")
    foreach ($p in $Peek) { $a += @('--peek', $p) }
    $w = Start-Process python -ArgumentList $a -PassThru -WindowStyle Hidden
}
if (-not (WaitFor "Entering state 'activity:in_world'" $WorldTimeoutSeconds)) { exit 2 }
Start-Sleep 8; Skip; Start-Sleep 10; Skip; Say 'cutscenes skipped'
if (-not (WaitFor "transitioning:teleportation' due to completed" 150)) { exit 2 }
Say 'in gameplay (112)'
Start-Sleep 10
& 'F:\d1\tools\d1-shot.ps1' -Out "$D\gameplay.png" | Out-Null
if ($w) { Wait-Process -Id $w.Id -Timeout ($WatchSeconds + 30) -ErrorAction SilentlyContinue }
python -I F:\d1\tools\d1-winguest.py masks 2>&1 | Select-Object -Last 1
Say "run dir: $D"
