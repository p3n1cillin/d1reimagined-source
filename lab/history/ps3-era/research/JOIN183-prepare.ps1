$ErrorActionPreference='Stop'
$rt='F:\d1\<pc>-import\d1xenia\local'
$rpc=Join-Path $rt 'RPCS3_FINAL124_SMOKE001'
$research=Join-Path $rt 'RESEARCH'
$closed=Get-Content -LiteralPath (Join-Path $research 'JOIN182-independent-closed.json') -Raw | ConvertFrom-Json
if(-not $closed.all_exact_owned_closed -or -not $closed.all_hashes_match){throw 'Prior cleanup is not verified'}
if(@(Get-CimInstance Win32_Process -Filter "Name='rpcs3.exe'").Count){throw 'A native client is live; preserve it'}
$build=(Get-FileHash -LiteralPath (Join-Path $rpc 'rpcs3.exe')).Hash.ToLowerInvariant()
if($build -ne 'c5bc7f5786835302dafe3265549e2052d3265df7025fc997bc6249e3647fe7cf'){throw 'Unexpected native executable'}
$title=(Get-FileHash -LiteralPath (Join-Path $rpc 'config\custom_configs\config_BLES01857.yml')).Hash
$watch=(Get-FileHash -LiteralPath (Join-Path $rpc 'd1\d1ppu.txt')).Hash
if($title -ne '22C081E2680A9F81C565860262232AF1296EBEA2831849338BA79BD6E7C2350B' -or $watch -ne '442847E5847064F2D570943381CAA88DFD66E9DC212FFD299D9AC6D2F1EDD4B6'){throw 'Unexpected original configuration'}
$wrapper=Join-Path $rt 'SIGNON_LOCAL\JOIN183-launch.ps1'
foreach($path in $wrapper,(Join-Path $research 'JOIN183-before-title.yml'),(Join-Path $research 'JOIN183-before-d1ppu.txt'),(Join-Path $research 'JOIN183-mission-marker-candidate.json')){if(Test-Path -LiteralPath $path){throw 'Preserve existing JOIN183 preparation'}}
foreach($marker in 'd1checkpoint','d1depthfp.on','d1descriptor','d1descriptorring','d1fp.on','d1rsxcapture.on'){
    if(Test-Path -LiteralPath (Join-Path $rpc ('d1\'+$marker))){throw 'Unexpected active diagnostic marker'}
}
Copy-Item -LiteralPath 'F:\d1\d1xenia\missions\a_guardian_rises.json' -Destination (Join-Path $research 'JOIN183-mission-marker-candidate.json')
Copy-Item -LiteralPath (Join-Path $rpc 'config\custom_configs\config_BLES01857.yml') -Destination (Join-Path $research 'JOIN183-before-title.yml')
Copy-Item -LiteralPath (Join-Path $rpc 'd1\d1ppu.txt') -Destination (Join-Path $research 'JOIN183-before-d1ppu.txt')
$text=(Get-Content -LiteralPath (Join-Path $rt 'SIGNON_LOCAL\JOIN182-launch.ps1') -Raw).Replace('JOIN182','JOIN183').Replace('AddSeconds(840)','AddSeconds(420)').Replace('bounded fourteen-minute','bounded seven-minute')
[IO.File]::WriteAllText($wrapper,$text,[Text.UTF8Encoding]::new($false))
$trace=@'
# JOIN183: bounded cinematic dispatch only; static call-site returns verified.
580928 4 r3:448
425654 4 if=lr==0x580a3c r3:4
425120 4 if=lr==0x42567c r3:4
42478c 4 if=lr==0x425694 r3:32
291870 4 if=lr==0x424874 r3:32
'@
$trace+="`n"
[IO.File]::WriteAllText((Join-Path $research 'JOIN183-planned-d1ppu.txt'),$trace,[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText((Join-Path $rpc 'd1\d1ppu.txt'),$trace,[Text.UTF8Encoding]::new($false))
foreach($marker in 'd1checkpoint','d1depthfp.on'){[IO.File]::WriteAllText((Join-Path $rpc ('d1\'+$marker)),'')}
$receipt=[ordered]@{at=[DateTime]::UtcNow.ToString('o');build=$build;source_base='7bedf8e5df854bac79633baf242cc6110957a5f8';source_clean=$true;change='Bounded read-only cinematic dispatch watches; production mission/codec unchanged.';candidate_sha256=(Get-FileHash -LiteralPath (Join-Path $research 'JOIN183-mission-marker-candidate.json')).Hash.ToLowerInvariant();trace=$trace;scope='C5 native diagnostics; no cinematic timing/cast changes, guest writes or playback acceptance claimed.'}
$receipt | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $research 'JOIN183-prepare.json') -Encoding utf8
$wrapper
