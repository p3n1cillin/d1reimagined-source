$env:VK_LAYER_GPUAV_ENABLE='0'
$env:VK_LAYER_GPUAV_SAFE_MODE='0'
$env:VK_INSTANCE_LAYERS='VK_LAYER_KHRONOS_validation'
$env:VK_LAYER_PATH='F:\d1\<pc>-import\dev\validation-runtime-1.4.341.1'
$ErrorActionPreference='Stop'
$rt='F:\d1\<pc>-import\d1xenia\local'
Set-Location -LiteralPath 'F:\d1\d1xenia'
. .\scripts\Final124RunHelpers.ps1
$env:D1_LOCAL=$rt
$env:D1_RPCS3="$rt\RPCS3_FINAL124_SMOKE001"
$rpc=$env:D1_RPCS3
$logs=Join-Path $rt 'SIGNON_LOCAL\logs'
$control="$rt\SIGNON_LOCAL\FINAL124_JOIN186-control.jsonl"
New-Item -ItemType File -Path $control -ErrorAction Stop | Out-Null
$proxy='--ah-bubble-states;-1,0=0,3=0,14=0;--ah-zone;113:0x811c9dc5;--ah-world-state;113:1;--ah-roster;0;--ah-bubble-reset;14:1;--ah-sim-player;0x4786c0e0:5:0x0a3d4a2c8fbea6ba:1;--ah-game-mode;0x4786c0e0:3:3;--ah-baselines;@'+"$rt\SIGNON_LOCAL\baselines_highway_0124_all.txt"+';--ah-heartbeat;1;--ah-sim-clock;30;--ah-group-switch;--ah-mission;F:\d1\<pc>-import\d1xenia\local\RESEARCH\JOIN186-mission-candidate.json;--ah-control;'+$control
$completed=$false
try {
    & .\scripts\Invoke-Final124Join.ps1 -Name FINAL124_JOIN186 -Local $rt -DataRepo 'F:\d1\d1xenia-data' -BapExtra '--peer-family;--character-gear;starter:01.24:titan' -ProxyExtra $proxy -AfterMatchSeconds 0 -SkipOpeningCinematic
    $completed=$true
    # Preserve the completed conformance interval, then inspect actual UI/game inputs.
    $manifest=Read-D1ProcessManifest (Join-Path $logs 'FINAL124_JOIN186-processes.json')
    $client=$manifest.processes | Where-Object tag -EQ 'rpcs3'
    Copy-D1SharedFileSnapshot -Source (Join-Path $rpc 'log\RPCS3.log') -Destination (Join-Path $logs 'JOIN186-baseline-native.log') | Out-Null
    $shot=(Get-ChildItem -LiteralPath (Join-Path $rpc 'screenshots\BLES01857') -File | Sort-Object LastWriteTimeUtc | Select-Object -Last 1).FullName
    $captureDir=Join-Path $rpc 'captures'
    $waitStarted=[DateTime]::UtcNow
    $deadline=$waitStarted.AddSeconds(420)
    [ordered]@{at=$waitStarted.ToString('o');client=$client;last_native_screenshot=$shot;deadline_utc=$deadline.ToString('o');scope='Owned mission/transition test with explicit inspected inputs; any RSX capture must be armed separately.'} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $logs 'JOIN186-capture-checkpoint.json') -Encoding utf8
    $probeExe='%USERPROFILE%\AppData\Local\Programs\Python\Python314\python.exe'
    $probeArgs=@('F:\d1\d1xenia\scripts\rpcs3_final124_session.py','--manifest',(Join-Path $logs 'FINAL124_JOIN186-processes.json'),'--output',(Join-Path $rt 'RESEARCH\JOIN186-session-timeline.jsonl'),'--seconds','800','--interval','0.25')
    $probe=Start-Process -FilePath $probeExe -ArgumentList @($probeArgs | ForEach-Object {ConvertTo-D1ProcessArgument $_}) -WindowStyle Hidden -RedirectStandardOutput (Join-Path $logs 'JOIN186-session-probe.log') -RedirectStandardError (Join-Path $logs 'JOIN186-session-probe.err') -PassThru
    $probeIdentity=Get-D1ProcessIdentity -Process $probe -Tag 'session-probe' -Executable $probeExe
    $probeIdentity | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $logs 'JOIN186-session-probe-process.json') -Encoding utf8
    'Ready for owned mission/transition inputs and native image inspection; bounded seven-minute checkpoint'
    $captureSaved=$false
    while ([DateTime]::UtcNow -lt $deadline) {
        if (Test-Path -LiteralPath (Join-Path $logs 'JOIN186-finish.on')) { break }
        if (-not (Get-D1OwnedProcess $client)) {throw 'Exact client closed at capture checkpoint'}
        $files=@(Get-ChildItem -LiteralPath $captureDir -Filter '*.rrc.gz' -File -ErrorAction SilentlyContinue | Where-Object {$_.Length -gt 0 -and $_.LastWriteTimeUtc -ge $waitStarted})
        if ($files.Count) {
            $capture=$files | Sort-Object LastWriteTimeUtc | Select-Object -Last 1
            $native=Join-Path $logs 'JOIN186-after-rsx-capture-native.log'
            Copy-D1SharedFileSnapshot -Source (Join-Path $rpc 'log\RPCS3.log') -Destination $native | Out-Null
            $success=@(Select-String -LiteralPath $native -Pattern ('Capture successful:.*'+[regex]::Escape($capture.Name)))
            if ($success.Count -eq 1) {
                [ordered]@{at=[DateTime]::UtcNow.ToString('o');client=$client;build=(Get-FileHash -LiteralPath $client.executable).Hash.ToLowerInvariant();file=$capture.FullName;bytes=$capture.Length;sha256=(Get-FileHash -LiteralPath $capture.FullName).Hash.ToLowerInvariant();native_success_line=$success[0].Line;capture_saved=$true;pause_expected=$true;full_parse='NOT RUN';replay='NOT RUN';graphics='NOT ACCEPTED';scope='Private native RSX diagnostic frame; successful native capture pauses the emulator. Captured state must be inspected separately.'} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $logs 'JOIN186-rsx-capture-result.json') -Encoding utf8
                $captureSaved=$true
                break
            }
        }
        Start-Sleep -Seconds 2
    }
    if (-not $captureSaved) {
        [ordered]@{at=[DateTime]::UtcNow.ToString('o');capture_saved=$false;scope='Bounded diagnostic checkpoint ended without a verified RSX capture; no capture or graphics success claimed.'} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $logs 'JOIN186-rsx-capture-result.json') -Encoding utf8
    }

} finally {
    $errors=[Collections.Generic.List[string]]::new()
    if ($probeIdentity) {
        try { Stop-D1OwnedProcess -Identity $probeIdentity } catch { $errors.Add('Probe cleanup: '+$_.Exception.Message) }
    }
    $owned=Read-D1ProcessManifest (Join-Path $logs 'FINAL124_JOIN186-processes.json')
    $identities=@($owned.processes)
    $proxyManifest=Join-Path $logs 'FINAL124_JOIN186-proxy-process.json'
    if (Test-Path -LiteralPath $proxyManifest) { $identities+=Get-Content -LiteralPath $proxyManifest -Raw | ConvertFrom-Json }
    $client=$identities | Where-Object tag -EQ 'rpcs3'
    try {
        if ($client) {
            $p=Get-D1OwnedProcess $client
            $layer=if ($p) { $p.Modules | Where-Object ModuleName -EQ 'VkLayer_khronos_validation.dll' | Select-Object ModuleName,FileName } else { $null }
            [ordered]@{at=[DateTime]::UtcNow.ToString('o');client_alive=[bool]$p;layer=$layer} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $logs 'JOIN186-final-layer.json') -Encoding utf8
            $source=Join-Path $rpc 'log\RPCS3.log'
            $started=[DateTime]::new([long]$client.startTimeUtcTicks,[DateTimeKind]::Utc)
            if ((Get-Item -LiteralPath $source).LastWriteTimeUtc -lt $started) { throw 'Shared log predates this exact client.' }
            Copy-D1SharedFileSnapshot -Source $source -Destination (Join-Path $logs 'JOIN186-pre-stop-native.log') | Out-Null
        }
    } catch { $errors.Add('Final native evidence: '+$_.Exception.Message) }
    try { & .\scripts\Invoke-Final124Join.ps1 -Name FINAL124_JOIN186 -Local $rt -DataRepo 'F:\d1\d1xenia-data' -Stop }
    catch {
        $errors.Add('Stack cleanup: '+$_.Exception.Message)
        foreach ($identity in $identities) {
            try { Stop-D1OwnedProcess $identity } catch { $errors.Add('Owned cleanup: '+$_.Exception.Message) }
        }
    }
    try {
        Copy-Item -LiteralPath (Join-Path $rt 'RESEARCH\JOIN186-before-title.yml') -Destination (Join-Path $rpc 'config\custom_configs\config_BLES01857.yml') -Force
        Copy-Item -LiteralPath (Join-Path $rt 'RESEARCH\JOIN186-before-d1ppu.txt') -Destination (Join-Path $rpc 'd1\d1ppu.txt') -Force
        foreach ($controlName in 'd1checkpoint','d1descriptor','d1descriptorring','d1depthfp.on','d1fp.on','d1rsxcapture.on') {
            $marker=Join-Path $rpc ('d1\'+$controlName)
            if (Test-Path -LiteralPath $marker) { Move-Item -LiteralPath $marker -Destination (Join-Path $rpc ('d1\'+$controlName+'.disabled-after-JOIN186')) -ErrorAction Stop }
        }
    } catch { $errors.Add('Controls restoration: '+$_.Exception.Message) }
    try { & .\scripts\Set-PadConfig.ps1 -Mode xinput -Rpcs3Dir $rpc }
    catch { $errors.Add('Pad restoration: '+$_.Exception.Message) }
    $remaining=@($identities | Where-Object { Get-D1OwnedProcess $_ })
    $receipt=[ordered]@{
        at=[DateTime]::UtcNow.ToString('o');fixture_completed=$completed;
        all_owned_closed=($remaining.Count -eq 0);
        title_unchanged=((Get-FileHash -LiteralPath (Join-Path $rpc 'config\custom_configs\config_BLES01857.yml')).Hash -eq '22C081E2680A9F81C565860262232AF1296EBEA2831849338BA79BD6E7C2350B');
        watch_unchanged=((Get-FileHash -LiteralPath (Join-Path $rpc 'd1\d1ppu.txt')).Hash -eq '442847E5847064F2D570943381CAA88DFD66E9DC212FFD299D9AC6D2F1EDD4B6');
        input=((Get-Content -LiteralPath (Join-Path $rpc 'config\input_configs\global\Default.yml') -TotalCount 2)[1].Trim());
        active_controls=@(Get-ChildItem -LiteralPath (Join-Path $rpc 'd1') -File | Where-Object { $_.Length -eq 0 -and ($_.Name -notmatch '\.' -or $_.Name -in 'd1depthfp.on','d1fp.on','d1rsxcapture.on') } | ForEach-Object Name);
        shared_rpcn_preserved=(Test-Path -LiteralPath (Join-Path $rpc 'config\rpcn.yml'));errors=@($errors.ToArray())
    }
    $receipt | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $logs 'JOIN186-cleanup-result.json') -Encoding utf8
    if ($errors.Count -or $remaining.Count -or -not $receipt.title_unchanged -or -not $receipt.watch_unchanged -or $receipt.active_controls.Count) { throw 'Automatic cleanup requires investigation; see private receipt.' }
}

