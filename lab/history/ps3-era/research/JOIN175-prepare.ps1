$ErrorActionPreference='Stop'
$rt='F:\d1\<pc>-import\d1xenia\local'
$rpc=Join-Path $rt 'RPCS3_FINAL124_SMOKE001'
$research=Join-Path $rt 'RESEARCH'
. 'F:\d1\d1xenia\scripts\Final124RunHelpers.ps1'
$old=Read-D1ProcessManifest -Path (Join-Path $rt 'SIGNON_LOCAL\logs\FINAL124_JOIN174-processes.json')
foreach ($identity in $old.processes) { if (Get-D1OwnedProcess -Identity $identity) { throw 'Prior owned stack is live' } }
$proxy=Get-Content -LiteralPath (Join-Path $rt 'SIGNON_LOCAL\logs\FINAL124_JOIN174-proxy-process.json') -Raw | ConvertFrom-Json
if(Get-D1OwnedProcess -Identity $proxy){throw 'Prior proxy is live'}
if (@(Get-CimInstance Win32_Process -Filter "Name='rpcs3.exe'").Count) { throw 'A native client is live; preserve it' }
$build=(Get-FileHash -LiteralPath (Join-Path $rpc 'rpcs3.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
if ($build -ne 'c5bc7f5786835302dafe3265549e2052d3265df7025fc997bc6249e3647fe7cf') { throw 'Unexpected runtime build' }
$watch=Get-Content -LiteralPath (Join-Path $research 'JOIN175-planned-d1ppu.txt') -Raw
$wrapper=Join-Path $rt 'SIGNON_LOCAL\JOIN175-launch.ps1'
if(Test-Path -LiteralPath $wrapper){throw 'Preserve existing wrapper'}
foreach($marker in 'd1checkpoint','d1depthfp.on') {
    if(Test-Path -LiteralPath (Join-Path $rpc ('d1\'+$marker))){throw 'Unexpected active control'}
}
foreach ($pair in @(@('config\custom_configs\config_BLES01857.yml','JOIN175-before-title.yml'),@('d1\d1ppu.txt','JOIN175-before-d1ppu.txt'))) {
    $target=Join-Path $research $pair[1]
    if (Test-Path -LiteralPath $target) { throw 'Preserve existing preparation backup' }
    Copy-Item -LiteralPath (Join-Path $rpc $pair[0]) -Destination $target
}
$text=(Get-Content -LiteralPath (Join-Path $rt 'SIGNON_LOCAL\JOIN174-launch.ps1') -Raw).Replace('JOIN174','JOIN175')
[IO.File]::WriteAllText($wrapper,$text,[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText((Join-Path $rpc 'd1\d1ppu.txt'),$watch,[Text.UTF8Encoding]::new($false))
foreach($marker in 'd1checkpoint','d1depthfp.on') { [IO.File]::WriteAllText((Join-Path $rpc ('d1\'+$marker)),'') }
[ordered]@{at=[datetime]::UtcNow.ToString('o');build=$build;native_build_not_changed=$true;trace=$watch;previous_owned_closed=$true;scope='Same Titan/highway fixture. Capture initialized type-21 body and factory-3 header encoding; no protocol or graphics change. Planned return-helper hook must emit before interpretation.'} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $research 'JOIN175-prepare.json') -Encoding utf8
$wrapper
