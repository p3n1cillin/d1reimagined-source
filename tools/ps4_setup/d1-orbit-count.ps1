param([Parameter(Mandatory)][string]$Out, [Parameter(Mandatory)][string]$Addrs)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\D1Paths.ps1"
# Title -> orbit with ad-hoc entry/branch counters installed BEFORE orbit loads (D1_COUNT_ANY),
# so build-once code at orbit entry is counted. Reads at orbit, leaves the hooks for a later read.
New-Item -ItemType Directory -Force $Out | Out-Null
$log = Join-Path $Out 'driver.log'
$shad = Get-D1Path ShadPS4 'user\log\shad_log.txt'
function Say([string]$m) { $l = "$([DateTime]::UtcNow.ToString('o')) $m"; Add-Content -LiteralPath $log $l; $l }
# Only a log written since this driver started counts: the previous run's log stays on disk until
# the new emulator truncates it, and its stale lines once made the driver press keys at no window.
$driverStart = Get-Date
function WaitFor([string]$pattern, [int]$seconds) {
    $end = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $end) {
        if ((Test-Path $shad) -and (Get-Item $shad).LastWriteTime -gt $driverStart -and
            (Select-String -LiteralPath $shad -Pattern $pattern -Quiet)) { return $true }
        Start-Sleep -Seconds 2
    }
    Say "TIMEOUT waiting for '$pattern'" | Write-Host; return $false
}
if (-not (WaitFor "Entering state 'bootflow:(start|intro_screen_display)'" 300)) { exit 2 }
$env:D1_COUNT_ANY = '1'; $env:D1_COUNT_ADDRS = $Addrs
for ($try = 0; $try -lt 3; $try++) { $o = python (Join-Path $PSScriptRoot 'd1-wincount.py') install 2>&1; $o | ForEach-Object { Say $_.ToString() }; if ($o -match 'hooked') { break }; Start-Sleep 3 }
Start-Sleep 6; & (Join-Path $PSScriptRoot 'd1-pad.ps1') -Keys NumPad2 | ForEach-Object { Say $_ }
if (-not (WaitFor "Entering state 'setup:orbit' for" 600)) { exit 2 }
Start-Sleep 20
python (Join-Path $PSScriptRoot 'd1-wincount.py') read | ForEach-Object { Say $_ }
# Session 0 must be valid (state 4..10) before anyone pokes a launch: the world-controller state can
# reach setup:orbit a moment before the session accessor does (14 Oct, 'session state 0 is outside 4..10').
$sessEnd = (Get-Date).AddSeconds(60)
while ((Get-Date) -lt $sessEnd) { python (Join-Path $PSScriptRoot 'd1-winguest.py') session *> $null; if ($LASTEXITCODE -eq 0) { break }; Start-Sleep 2 }
Say 'AT ORBIT'
