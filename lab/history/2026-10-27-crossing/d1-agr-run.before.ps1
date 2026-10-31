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
    [string]$Steps = 'F:\d1\d1preservation-main\cfw-staging\agr-steps-breach13.txt',
    # The served profile. profile-tutorial-t.bin is the unarmed tutorial Guardian (no weapons in the
    # inventory or the roster) who owes the starter Khvostov as pending loot.
    [string]$Profile = 'F:\d1\d1preservation-main\capture-rig\builds\later-56578\profile-tutorial-t.bin',
    # -Geared: the old fixture (geared inventory and roster, hand cannon in hand, no rifle grant).
    [switch]$Geared,
    # Session length passed to d1-lab-served.ps1 (its default is 90 minutes).
    [int]$Minutes = 90
)
# A Guardian Rises (the tutorial) end to end, unattended: boot, title (X), skip the opening Mars
# cinematic in character:prologue (X, then O - left playing it can trip the BLOCK_DETECT watchdog),
# then skip the revival and wake-up cutscenes (the host ends them on incident 1726) and wait for the
# teleport to 112. Keep the emulator window focused: input only reaches the focused window.
$ErrorActionPreference = 'Continue'
Get-Process shadPS4, d1-activity-host, d1-responder, shadnet-server -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep 3
$B = 'F:\d1\d1preservation-main\capture-rig\builds\later-56578'
$D = "F:\d1\d1preservation-main\cfw-staging\$Name"
New-Item -ItemType Directory -Force $D | Out-Null
$L = 'F:\d1\tools\shadPS4\0.19.0\user\log\shad_log.txt'
$pad = 'F:\d1\tools\d1-pad.ps1'
# The Breach props (lights, barrier, rifle) are created by the client only while it holds their
# authority block: every block assigned to machine 3 (the client's machine in bubble 3), not yet
# applied, so the client acknowledges it and keeps it (notes 26 Oct).
$x = "D1_AH_MISSION_STEPS=$Steps,D1_AH_SENSOR_AUTHORITY=3,1,0"
if (-not $Geared) {
    # The unarmed tutorial: weaponless inventory and peer roster. Claiming the Breach rifle (601)
    # grants the Khvostov (item + profile with the primary slot unlocked) and equips it (peer cards).
    $GD = "$B\grants"
    $x += ",D1_INVENTORY=$B\inventory-tutorial,D1_ROSTER_DIR=$B\rosters-f0-tutorial"
    $x += ",D1_LOOT_GRANT=0x827dbaf0=0xd100000000000401:$GD\khvostov-3598.bin|$GD\profile-tutorial-th.bin"
    $x += ",D1_LOOT_GRANT_ROSTER=$GD\rosters-f0-tutorial-khv"
}
if ($Extra) { $x += ",$Extra" }
Start-Process pwsh -WindowStyle Minimized -ArgumentList @('-NoProfile', '-File', 'F:\d1\tools\d1-lab-served.ps1', '-Name', $Name,
    '-Profile', $Profile, '-Minutes', "$Minutes",
    '-UnlockFlags', '98,308,419,440,834,2668,3391,5021,5998,6029,433,2539,2549,3558,3587,3520,3527,3531,3533,3534,3535,3536,3544,3568,3594,3595,305,2669,2670,3389',
    '-Extra', $x)
function WaitFor($pattern, $seconds) {
    $t = 0
    while ($t -lt $seconds) {
        if ((Test-Path $L) -and (Select-String -Path $L -Pattern $pattern -Quiet)) { return $true }
        Start-Sleep 3; $t += 3
    }
    "TIMEOUT waiting for $pattern"; return $false
}
function Skip { & $pad -Keys NumPad2 | Out-Null; Start-Sleep 1; & $pad -Keys NumPad6 | Out-Null }
Start-Sleep 20
if (-not (WaitFor "Entering state 'bootflow:start'" 300)) { exit 2 }
Start-Sleep 6; & $pad -Keys NumPad2 | Out-Null
if (WaitFor "Entering state 'character:prologue'" 300) {
    # A skip can be lost (focus, or pressed before the cinematic accepts input): retry until the
    # client leaves the prologue (run agr-breach12 sat in it for ten minutes).
    for ($k = 0; $k -lt 4; $k++) {
        Start-Sleep 8; Skip
        if (WaitFor "Leaving state 'character:prologue'" 20) { 'prologue skipped'; break }
    }
}
$w = $null
if ($WatchExec) {
    $a = @('-I', 'F:\d1\tools\d1-watchwrite.py', '--seconds', "$WatchSeconds", '--max-hits', '60', '--exec', $WatchExec, '--out', "$D\watch.jsonl")
    foreach ($p in $Peek) { $a += @('--peek', $p) }
    $w = Start-Process python -ArgumentList $a -PassThru -WindowStyle Hidden
}
if (WaitFor "Entering state 'activity:in_world'" 200) { Start-Sleep 8; Skip; Start-Sleep 10; Skip; 'cutscenes skipped' }
if (WaitFor "transitioning:teleportation' due to completed" 150) { 'in gameplay (112)' }
Start-Sleep 10
& 'F:\d1\tools\d1-shot.ps1' -Out "$D\gameplay.png" | Out-Null
if ($w) { Wait-Process -Id $w.Id -Timeout ($WatchSeconds + 30) -ErrorAction SilentlyContinue }
python -I F:\d1\tools\d1-winguest.py masks 2>&1 | Select-Object -Last 1
"run dir: $D"
