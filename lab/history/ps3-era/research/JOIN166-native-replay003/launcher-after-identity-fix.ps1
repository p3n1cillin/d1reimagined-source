$ErrorActionPreference='Stop'
$rt='F:\d1\<pc>-import\d1xenia\local'
$rpc=Join-Path $rt 'RPCS3_FINAL124_SMOKE001'
$evidence=Join-Path $rt 'RESEARCH\JOIN166-native-replay004'
. 'F:\d1\d1xenia\scripts\Final124RunHelpers.ps1'
$env:D1_LOCAL=$rt
$env:D1_RPCS3=$rpc
$env:VK_LAYER_GPUAV_ENABLE='0'
$env:VK_LAYER_GPUAV_SAFE_MODE='0'
$env:VK_INSTANCE_LAYERS='VK_LAYER_KHRONOS_validation'
$env:VK_LAYER_PATH='F:\d1\<pc>-import\dev\validation-runtime-1.4.341.1'
if(Get-CimInstance Win32_Process -Filter "Name='rpcs3.exe'"){throw 'RPCS3 is already running'}
if(Test-Path -LiteralPath $evidence){throw 'Replay evidence directory already exists'}
$saved=Get-Content -LiteralPath (Join-Path $rt 'SIGNON_LOCAL\logs\JOIN166-rsx-capture-result.json') -Raw|ConvertFrom-Json
if(-not $saved.capture_saved -or (Get-FileHash -LiteralPath $saved.file).Hash.ToLowerInvariant() -ne $saved.sha256){throw 'Capture identity failed'}
$exe=Join-Path $rpc 'rpcs3.exe'
$replaySha=(Get-FileHash -LiteralPath $exe).Hash.ToLowerInvariant()
if($replaySha -ne 'cc73a62d8a079fca2ca3e995647c468485bb6095c548a95596de4e73a6c90397'){throw 'Unexpected context/display-repaired replay executable'}
$global=Join-Path $rpc 'config\config.yml'
$title=Join-Path $rpc 'config\custom_configs\config_BLES01857.yml'
if((Get-FileHash -LiteralPath $title).Hash -ne '22C081E2680A9F81C565860262232AF1296EBEA2831849338BA79BD6E7C2350B'){throw 'Unexpected tested title config'}
foreach($name in 'd1shot.on','d1depthfp.on','d1rsxcapture.on','d1descriptorring','d1descriptor','d1fp.on','d1trace.on'){
    if(Test-Path -LiteralPath (Join-Path $rpc ('d1\'+$name))){throw "Unexpected active marker $name"}
}
New-Item -ItemType Directory -Path $evidence|Out-Null
$backup=Join-Path $evidence 'global-before.yml'
Copy-Item -LiteralPath $global -Destination $backup
$beforeSha=(Get-FileHash -LiteralPath $global).Hash
$identity=$null
$shots=[Collections.Generic.List[object]]::new()
$errors=[Collections.Generic.List[string]]::new()
$started=[DateTime]::UtcNow
try {
    Copy-Item -LiteralPath $title -Destination $global -Force
    if((Get-FileHash -LiteralPath $global).Hash -ne (Get-FileHash -LiteralPath $title).Hash){throw 'Replay settings copy failed'}
    New-Item -ItemType File -Path (Join-Path $rpc 'd1\d1depthfp.on')|Out-Null
    $args=@('--no-gui','--rsx-capture',$saved.file)|ForEach-Object {ConvertTo-D1ProcessArgument $_}
    $process=Start-Process -FilePath $exe -ArgumentList $args -WorkingDirectory $rpc -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $evidence 'stdout.txt') -RedirectStandardError (Join-Path $evidence 'stderr.txt')
    $identity=Get-D1ProcessIdentity -Process $process -Tag rpcs3 -Executable $exe
    # Windows may expose StartTime before Process.Path. Wait for the complete
    # exact identity rather than mistaking that startup transition for exit.
    $identityDeadline=[DateTime]::UtcNow.AddSeconds(5)
    while(-not(Get-D1OwnedProcess -Identity $identity)){
        $process.Refresh()
        if($process.HasExited){throw "Replay process exited with code $($process.ExitCode)"}
        if([DateTime]::UtcNow -ge $identityDeadline){throw 'Replay executable identity did not become available'}
        Start-Sleep -Milliseconds 50
    }
    [ordered]@{at=$started.ToString('o');identity=$identity;capture_sha256=$saved.sha256;capture_build=$saved.build;build=$replaySha;change='Replay main-memory allocation excludes null address';global_before_sha256=$beforeSha;global_replay_sha256=(Get-FileHash -LiteralPath $global).Hash;depth_candidate=$true;ring=$false;scope='Private captured UI/control frame only; no live PPU/SPU/backend replay.'}|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $evidence 'process.json') -Encoding utf8
    $deadline=$started.AddSeconds(120)
    while([DateTime]::UtcNow -lt $started.AddSeconds(20)){
        if(-not(Get-D1OwnedProcess -Identity $identity)){throw 'Replay process exited during startup'}
        Start-Sleep -Seconds 1
    }
    for($sample=1;$sample -le 2;$sample++){
        $owned=Get-D1OwnedProcess -Identity $identity
        if(-not $owned){throw 'Owned replay unavailable'}
        $layer=@($owned.Modules|Where-Object ModuleName -EQ 'VkLayer_khronos_validation.dll'|Select-Object ModuleName,FileName)
        if($layer.Count -ne 1){throw 'Replay core validation layer not verified'}
        $shotStarted=[DateTime]::UtcNow
        $stream=[IO.File]::Open((Join-Path $rpc 'd1\d1shot.on'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
        $stream.Dispose()
        $shot=$null
        while([DateTime]::UtcNow -lt $shotStarted.AddSeconds(15) -and [DateTime]::UtcNow -lt $deadline){
            $shot=Get-ChildItem -LiteralPath (Join-Path $rpc 'screenshots') -Filter 'screenshot_*.png' -File|Where-Object {$_.Length -gt 0 -and $_.LastWriteTimeUtc -ge $shotStarted}|Sort-Object LastWriteTimeUtc|Select-Object -Last 1
            if($shot){break}
            if(-not(Get-D1OwnedProcess -Identity $identity)){throw 'Owned replay exited before screenshot'}
            Start-Sleep -Milliseconds 500
        }
        if(-not $shot){throw 'No fresh native replay screenshot'}
        $shots.Add([ordered]@{sample=$sample;path=$shot.FullName;at=$shot.LastWriteTimeUtc.ToString('o');bytes=$shot.Length;sha256=(Get-FileHash -LiteralPath $shot.FullName).Hash.ToLowerInvariant();core_layer=$layer})
        if($sample -eq 1){Start-Sleep -Seconds 10}
    }
} catch {
    $errors.Add($_.Exception.Message)
} finally {
    if($identity){
        try {
            $native=Join-Path $rpc 'log\RPCS3.log'
            if((Get-Item -LiteralPath $native).LastWriteTimeUtc -lt [DateTime]::new([long]$identity.startTimeUtcTicks,[DateTimeKind]::Utc)){throw 'Native log predates owned replay'}
            Copy-D1SharedFileSnapshot -Source $native -Destination (Join-Path $evidence 'pre-stop-native.log')|Out-Null
        } catch {$errors.Add('Log snapshot: '+$_.Exception.Message)}
        try {Stop-D1OwnedProcess -Identity $identity} catch {$errors.Add('Owned stop: '+$_.Exception.Message)}
    }
    try {
        Copy-Item -LiteralPath $backup -Destination $global -Force
        foreach($name in 'd1shot.on','d1depthfp.on'){
            $marker=Join-Path $rpc ('d1\'+$name)
            if(Test-Path -LiteralPath $marker){Move-Item -LiteralPath $marker -Destination (Join-Path $evidence ($name+'.disabled'))}
        }
    } catch {$errors.Add('Config/marker restoration: '+$_.Exception.Message)}
    $closed=($null -eq $identity -or -not(Get-D1OwnedProcess -Identity $identity))
    $restored=((Get-FileHash -LiteralPath $global).Hash -eq $beforeSha)
    [ordered]@{at=[DateTime]::UtcNow.ToString('o');identity=$identity;capture_sha256=$saved.sha256;capture_build=$saved.build;build=$replaySha;screenshots=@($shots.ToArray());native_full_parse=if($shots.Count){'Native parser reached rendered output; visual inspection pending'}else{'NOT VERIFIED'};visual_comparison='NOT RUN';graphics='NOT ACCEPTED';owned_closed=$closed;global_restored=$restored;title_unchanged=((Get-FileHash -LiteralPath $title).Hash -eq '22C081E2680A9F81C565860262232AF1296EBEA2831849338BA79BD6E7C2350B');errors=@($errors.ToArray())}|ConvertTo-Json -Depth 7|Set-Content -LiteralPath (Join-Path $evidence 'receipt.json') -Encoding utf8
    if($errors.Count -or -not $closed -or -not $restored){throw 'Private replay requires inspection; see receipt'}
}
'Replay complete; inspect private native screenshots and receipt'
