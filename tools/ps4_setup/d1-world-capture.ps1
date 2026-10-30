param(
    # World name to publish (global_activity_state+0x688).
    [Parameter(Mandatory)][string]$World,
    # 1 = capture the checksum (publish 0); 2 = capture the sensor clients (publish the checksum).
    [ValidateSet(1, 2, 3)][int]$Step = 1,
    # Step 3: the world's sensor group hash (the group holding `i8 0x10 sub 3`).
    [string]$SensorHash = '',
    [string]$Checksum = '',
    [int]$Bubbles = 1
)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\D1Paths.ps1"
# Capture a new world for the served route: step 1 reads the record's PRIVATE checksum and bubble
# count at the client's own comparison (0x299B9A); step 2 publishes them and dumps the world's
# sensor-client list once rung 34 stalls at tasks 9/10. Launch is a Director Private Match.
$S = Get-D1Path Staging
$name = "2026-10-14-world-$World-$Step"
$D = "$S\$name"
New-Item -ItemType Directory -Force $D | Out-Null
Get-Process shadPS4, d1-activity-host, d1-responder, shadnet-server -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep 3
$extra = @()
if ($Step -ge 2) { $extra = @("D1_AH_ACTIVITY_CHECKSUM=$Checksum", "D1_AH_BUBBLE_COUNT=$Bubbles", 'D1_AH_BUBBLE_CHOSEN=all') }
# A separate PowerShell process, NOT Start-Job: booting the servers inside a job made the client's
# BAP connection drop three times during investment sign-in (CHICKEN, error.no_investment_connection).
if ($Step -eq 3) { $extra += "D1_AH_SENSOR_NAME_HASH=$SensorHash" }
$bootArgs = @('-NoProfile', '-File', (Get-D1Path Tools 'd1-lab-served.ps1'), '-Name', $name, '-World', $World)
if ($Step -eq 3) { $bootArgs += @('-SensorClients', "$S\sensor-clients-$World-bundle.txt") }
if ($extra) { $bootArgs += @('-Extra', ($extra -join ',')) }
Start-Process pwsh -ArgumentList $bootArgs -WindowStyle Minimized | Out-Null
& (Get-D1Path Tools 'd1-drive-to-orbit.ps1') -Out $D | Select-Object -Last 1
$watch = Start-Process python -ArgumentList @((Get-D1Path Tools 'd1-watchwrite.py'), '--seconds', '300', '--max-hits', '5', '--exec', '0x299b9a',
    '--peek', 'rcx+rdx*4+0xc', '--peek', 'rcx+0x48', '--out', "$D\cksum.jsonl") -PassThru -WindowStyle Hidden
Start-Sleep 4
& (Get-D1Path Tools 'd1-drive-private-match.ps1') -Out "$D\drive" -NoLaunch | Select-Object -Last 1
& (Get-D1Path Tools 'd1-pad.ps1') -Keys 'Hold:D:400', 'Hold:D:400', 'Hold:S:300' -GapMilliseconds 500 | Out-Null
Start-Sleep 1
& (Get-D1Path Tools 'd1-pad.ps1') -Keys NumPad2 -GapMilliseconds 300 | Out-Null
Start-Sleep 40
if ($Step -eq 3) {
    $f = @(); foreach ($i in 1..6) { Start-Sleep 10; & (Get-D1Path Tools 'd1-shot.ps1') -Out "$D\W$i.png" | Out-Null; $f += "$D\W$i.png" }
    "screens: $($f -join ' ')"
    python (Get-D1Path Tools 'd1-winguest.py') masks 2>&1 | Select-Object -Last 1
}
Get-Content "$D\cksum.jsonl" -ErrorAction SilentlyContinue | Where-Object { $_ -match '"exec"' } | ForEach-Object {
    $j = $_ | ConvertFrom-Json
    "checksum compare: ours $($j.rax)  record $($j.peek.'rcx+rdx*4+0xc')  bubbles $($j.peek.'rcx+0x48')"
}
if ($Step -eq 2) {
    python (Get-D1Path Tools 'd1-winguest.py') masks 2>&1 | Select-Object -Last 1
    python (Get-D1Path Tools 'd1-winguest.py') sensor-clients --all --bundle --out "$S\sensor-clients-$World-bundle.txt" 2>&1 | Select-Object -Last 1
    "sensor list: $S\sensor-clients-$World-bundle.txt"
    Get-Content "$S\sensor-clients-$World-bundle.txt" | Select-Object -Skip 1 | ForEach-Object { ($_ -split ' ')[0] } | Group-Object | ForEach-Object { "  group $($_.Name) x$($_.Count)" }
}
Stop-Process -Id $watch.Id -ErrorAction SilentlyContinue
