# After RUN LONG1 showed the real loop, this sweeps the STUN reply variants *with the
# title screen driven*, looking for the one thing that matters now: the first TCP
# connection to the BAP stand-in at 127.1.1.127:12850.
#
# Why this is the right signal: the client signs on, fetches the manifest, and then
# goes back to STUN and stays there. Demonware will not dial BAP until the NAT punch
# completes (network_bap_nat_punch_queue / waiting_for_bap_refresh), and the punch is
# what the unaccepted STUN reply prevents. So "did BAP see a connection" is the
# end-to-end outcome, and unlike the field values it is unambiguous.
#
#   pwsh -NoProfile -File stun-after-signon.ps1 -Variants map,binary,mirror -BootSeconds 480
param(
    [string]$Variants = 'typeversion,mirror,map',
    [int]$BootSeconds = 480,
    [int]$PressSeconds = 150,     # stop pressing once sign-on has clearly happened
    [string]$Local = '<D1_ROOT>\main-pc-import\d1xenia\local',
    [string]$DataRepo = '<D1_ROOT>\d1xenia-data'
)

$ErrorActionPreference = 'Continue'
$repo = '<D1_ROOT>\d1xenia'
$rpcs3dir = "$Local\RPCS3_FINAL124_SMOKE001"
$usrdir = "$Local\01_WORKING\ps3_destiny\PS3_GAME\USRDIR"
$logs = "$Local\SIGNON_LOCAL\logs"
$env:D1_LOCAL = $Local
$pwsh = (Get-Command pwsh).Source

# argument string per variant, so only the reply changes between runs
$stunArgFor = @{
    'typeversion' = '--shape;typeversion;--reply-type;15;--reply-version;2'
    'binary'     = '--shape;binary;--reply-index;0'
    'map'        = '--shape;map;--fill;all'
    'mirror'     = '--shape;mirror;--mirror-fill;port'
    'index'      = '--shape;index;--reply-index;0'
    'none'       = '--reply-index;-1'          # control: never answer
}

function Stop-Stack {
    Get-Process rpcs3 -ErrorAction SilentlyContinue |
        Stop-Process -Force -Confirm:$false -ErrorAction SilentlyContinue
    Get-CimInstance Win32_Process -Filter "Name like 'python%'" -ErrorAction SilentlyContinue |
        Where-Object { $_.CommandLine -match 'dns_sinkhole|stun_stub|signon_probe|lobby_probe|bap_server|rpcs3_gdb_daemon' } |
        ForEach-Object { Stop-Process -Id $_.ProcessId -Force -Confirm:$false -ErrorAction SilentlyContinue }
    Start-Sleep 4
}

$results = @()
foreach ($v in ($Variants -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ })) {
    $name = "PUNCH_$v"
    Write-Host "=== $name  ($($stunArgFor[$v]))  $(Get-Date -Format HH:mm:ss) ===" -ForegroundColor Cyan
    Stop-Stack

    $a = @(
        '-NoProfile', '-File', "$repo\scripts\Start-FinalBuildSmoke.ps1",
        '-DataRepo', $DataRepo, '-Rpcs3Dir', $rpcs3dir, '-GameUsrdir', $usrdir,
        '-Name', $name, '-Run', '-StunArgs', $stunArgFor[$v]
    )
    Start-Process -FilePath $pwsh -ArgumentList $a -WorkingDirectory $repo -WindowStyle Hidden `
        -RedirectStandardOutput "$logs\$name-launch.log" -RedirectStandardError "$logs\$name-launch.err" | Out-Null

    $deadline = (Get-Date).AddSeconds($BootSeconds)
    $nextPress = (Get-Date).AddSeconds(45)
    $presses = 0; $signon = 0; $bap = 0; $probes = 0
    $bapHit = $false
    while ((Get-Date) -lt $deadline) {
        Start-Sleep 10
        $bf = "$logs\$name-bap.log"
        if (Test-Path $bf) {
            $bap = @(Get-Content $bf -ErrorAction SilentlyContinue).Count
            if ($bap -gt 1) { $bapHit = $true }
        }
        $gf = "$logs\$name-signon.log"
        if (Test-Path $gf) {
            $signon = @(Get-Content $gf -ErrorAction SilentlyContinue |
                        Select-String -NotMatch 'listening on').Count
        }
        $sf = "$logs\$name-stun.log"
        if (Test-Path $sf) { $probes = @(Get-Content $sf -ErrorAction SilentlyContinue | Select-String '<- ').Count }

        if ($bapHit) {
            Write-Host "  BAP CONNECTED after $presses presses" -ForegroundColor Green
            break
        }
        # press Cross until sign-on is clearly done, then stop so the run is not
        # confused by input on later screens
        if ($presses -lt 12 -and $signon -lt 5 -and (Get-Date) -ge $nextPress) {
            & "$repo\scripts\Send-RpcsKey.ps1" -Keys X -HoldMs 150 | Out-Null
            $presses++
            $nextPress = (Get-Date).AddSeconds(10)
        }
    }

    $verdict = if ($bapHit) { 'BAP CONNECTED' } else { 'no BAP' }
    Write-Host ("  -> {0}: presses={1} signon={2} probes={3} bap-log-lines={4}" -f $verdict, $presses, $signon, $probes, $bap) -ForegroundColor Yellow
    if (Test-Path $bf) { Get-Content $bf -ErrorAction SilentlyContinue | Select-Object -First 8 | ForEach-Object { "     $_" } }
    $results += [pscustomobject]@{ Variant = $v; Verdict = $verdict; Presses = $presses; Signon = $signon; Probes = $probes; BapLines = $bap }
    Stop-Stack
}

Write-Host ''
Write-Host '=== NAT punch sweep (BAP connection is the signal) ===' -ForegroundColor Cyan
$results | Format-Table -AutoSize | Out-String -Width 200 | Write-Host
$results | Export-Csv "$logs\stun-after-signon.csv" -NoTypeInformation
