param([Parameter(Mandatory)][string]$Screenshot,[Parameter(Mandatory)][string]$Observed)
$ErrorActionPreference='Stop'
$rt='F:\d1\<pc>-import\d1xenia\local'
$rpc=Join-Path $rt 'RPCS3_FINAL124_SMOKE001'
. 'F:\d1\d1xenia\scripts\Final124RunHelpers.ps1'
$checkpoint=Get-Content -LiteralPath (Join-Path $rt 'SIGNON_LOCAL\logs\JOIN167-capture-checkpoint.json') -Raw | ConvertFrom-Json
if([DateTime]::UtcNow -ge ([DateTime]$checkpoint.deadline_utc).ToUniversalTime().AddSeconds(-15)){throw 'Bounded capture deadline too close'}
$identity=$checkpoint.client
$client=Get-D1OwnedProcess -Identity $identity
if(-not $client){throw 'Owned client unavailable'}
$hash=(Get-FileHash -LiteralPath $identity.executable).Hash.ToLowerInvariant()
if($hash -ne 'cc73a62d8a079fca2ca3e995647c468485bb6095c548a95596de4e73a6c90397'){throw 'Unexpected build'}
$layer=@($client.Modules | Where-Object ModuleName -EQ 'VkLayer_khronos_validation.dll' | Select-Object ModuleName,FileName)
if($layer.Count -ne 1){throw 'Core layer not verified'}
$item=Get-Item -LiteralPath $Screenshot
if($item.LastWriteTimeUtc -lt [DateTime]::UtcNow.AddSeconds(-90) -or $item.Length -le 0){throw 'Inspected screenshot is not fresh and complete'}
$expectedShots=[IO.Path]::GetFullPath((Join-Path $rpc 'screenshots\BLES01857'))+[IO.Path]::DirectorySeparatorChar
if(-not $item.FullName.StartsWith($expectedShots,[StringComparison]::OrdinalIgnoreCase)){throw 'Screenshot outside current native title folder'}
$marker=Join-Path $rpc 'd1\d1rsxcapture.on'
if(Test-Path -LiteralPath $marker){throw 'Capture marker already exists'}
$request=[ordered]@{at=[DateTime]::UtcNow.ToString('o');client=$identity;build=$hash;core_layer=$layer;inspected_screenshot=$item.FullName;screenshot_at=$item.LastWriteTimeUtc.ToString('o');screenshot_sha256=(Get-FileHash -LiteralPath $item.FullName).Hash.ToLowerInvariant();observed=$Observed;target='Next complete native RSX frame, which may differ from inspected screenshot';graphics_acceptance='NOT ACCEPTED';scope='Private diagnostic after actual image inspection; game bytes stay outside repos/content packages'}
$path=Join-Path $rt 'SIGNON_LOCAL\logs\JOIN167-rsx-capture-request.json'
$stream=[IO.File]::Open($path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
try{$bytes=[Text.UTF8Encoding]::new($false).GetBytes(($request|ConvertTo-Json -Depth 6));$stream.Write($bytes,0,$bytes.Length)}finally{$stream.Dispose()}
if(-not (Get-D1OwnedProcess -Identity $identity)){throw 'Owned client closed before marker'}
$stream=[IO.File]::Open($marker,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
$stream.Dispose()
[pscustomobject]@{at=[DateTime]::UtcNow.ToString('o');marker_created=$true;expected_pause_after_success=$true}|ConvertTo-Json
