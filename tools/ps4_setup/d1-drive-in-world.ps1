param(
    # Folder for screenshots and the driver log.
    [Parameter(Mandatory)][string]$Out,
    [int]$ActivityId = 416,   # 56578: 416 = Elimination (407 is Salvage here; 01.29 numbering differs)
    [int]$InWorldShots = 6,
    [int]$ShotIntervalSeconds = 15,
    # Leave the rung-34 steps to the client: no task-7 suppress byte, no done-mask force. The
    # masks are logged every 20 s for -ObserveSeconds instead.
    [switch]$NoForce,
    [int]$ObserveSeconds = 240
)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\D1Paths.ps1"
# Drives a contained -OperatorInput session of build 56578 from the title screen to
# activity:in_world, the route recorded in notes/ps4-retarget-2026-10-12.md. Three steps are
# SYNTHETIC and are logged as such: the launch poke, the task-7 suppress byte and the rung-34
# done-mask force (d1-winguest.py). Everything else is the client and our servers.
New-Item -ItemType Directory -Force $Out | Out-Null
$log = Join-Path $Out 'driver.log'
$shad = Get-D1Path ShadPS4 'user\log\shad_log.txt'
$guest = Join-Path $PSScriptRoot 'd1-winguest.py'
function Say([string]$m) { $l = "$([DateTime]::UtcNow.ToString('o')) $m"; Add-Content -LiteralPath $log $l; $l }
# Only a log written since this driver started counts: the previous run's log stays on disk until
# the new emulator truncates it, and its stale lines once made the driver press keys at no window.
$driverStart = Get-Date
function WaitFor([string]$pattern, [int]$seconds) {
    $end = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $end) {
        if ((Test-Path $shad) -and (Get-Item $shad).LastWriteTime -gt $driverStart -and
            (Select-String -LiteralPath $shad -Pattern $pattern -Quiet)) { return $true }
        if (Select-String -LiteralPath $shad -Pattern "Entering state 'cleanup' for reason 'error|hitch detected" -Quiet) {
            Say "FAILED while waiting for '$pattern': error state or hitch in the client log"; return $false
        }
        Start-Sleep -Seconds 2
    }
    Say "TIMEOUT waiting for '$pattern'"; return $false
}
if (-not (WaitFor "Entering state 'bootflow:(start|intro_screen_display)'" 300)) { exit 2 }
Start-Sleep 6; & (Join-Path $PSScriptRoot 'd1-pad.ps1') -Keys NumPad2 | ForEach-Object { Say $_ }
if (-not (WaitFor "Entering state 'setup:orbit' for" 600)) { exit 2 }
Start-Sleep 15
Say 'SYNTHETIC launch poke'; python $guest launch --activity-id $ActivityId --confirm | ForEach-Object { Say $_ }
if (-not (WaitFor "Entering state 'activity:initial_slice_set_loading'" 600)) { exit 2 }
Start-Sleep 10
if ($NoForce) {
    $end = (Get-Date).AddSeconds($ObserveSeconds)
    while ((Get-Date) -lt $end) {
        python $guest masks | Select-String 'rung-34' | ForEach-Object { Say $_.ToString() }
        if (Select-String -LiteralPath $shad -Pattern "Entering state 'activity:physics_join'" -Quiet) { Say 'left rung 34 on its own'; break }
        Start-Sleep 20
    }
} else {
Say 'SYNTHETIC task-7 suppress'; python $guest suppress --confirm | ForEach-Object { Say $_ }
Start-Sleep 30
python $guest masks | ForEach-Object { Say $_ }
Say 'SYNTHETIC rung-34 done-mask force'; python $guest masks --force-done --confirm | ForEach-Object { Say $_ }
}
if (-not (WaitFor "Entering state 'activity:in_world'" 300)) { exit 2 }
Say 'IN WORLD'
for ($i = 1; $i -le $InWorldShots; $i++) {
    Start-Sleep -Seconds $ShotIntervalSeconds
    & (Join-Path $PSScriptRoot 'd1-shot.ps1') -Out (Join-Path $Out ('in-world-{0:D2}.png' -f $i)) | ForEach-Object { Say $_ }
}
Say 'driver done'
