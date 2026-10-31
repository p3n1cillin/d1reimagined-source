$ErrorActionPreference='Stop'
$rt='F:\d1\<pc>-import\d1xenia\local'
$rpc=Join-Path $rt 'RPCS3_FINAL124_SMOKE001'
$research=Join-Path $rt 'RESEARCH'
$closed=Get-Content -LiteralPath (Join-Path $research 'JOIN184-independent-closed.json') -Raw | ConvertFrom-Json
if(-not $closed.all_exact_owned_closed -or -not $closed.all_hashes_match){throw 'Prior cleanup is not verified'}
if(@(Get-CimInstance Win32_Process -Filter "Name='rpcs3.exe'").Count){throw 'A native client is live; preserve it'}
$build=(Get-FileHash -LiteralPath (Join-Path $rpc 'rpcs3.exe')).Hash.ToLowerInvariant()
if($build -ne 'c5bc7f5786835302dafe3265549e2052d3265df7025fc997bc6249e3647fe7cf'){throw 'Unexpected native executable'}
$title=(Get-FileHash -LiteralPath (Join-Path $rpc 'config\custom_configs\config_BLES01857.yml')).Hash
$watch=(Get-FileHash -LiteralPath (Join-Path $rpc 'd1\d1ppu.txt')).Hash
if($title -ne '22C081E2680A9F81C565860262232AF1296EBEA2831849338BA79BD6E7C2350B' -or $watch -ne '442847E5847064F2D570943381CAA88DFD66E9DC212FFD299D9AC6D2F1EDD4B6'){throw 'Unexpected original configuration'}
$wrapper=Join-Path $rt 'SIGNON_LOCAL\JOIN185-launch.ps1'
foreach($path in $wrapper,(Join-Path $research 'JOIN185-before-title.yml'),(Join-Path $research 'JOIN185-before-d1ppu.txt'),(Join-Path $research 'JOIN185-mission-candidate.json')){if(Test-Path -LiteralPath $path){throw 'Preserve existing JOIN185 preparation'}}
foreach($marker in 'd1checkpoint','d1depthfp.on','d1descriptor','d1descriptorring','d1fp.on','d1rsxcapture.on'){
    if(Test-Path -LiteralPath (Join-Path $rpc ('d1\'+$marker))){throw 'Unexpected active diagnostic marker'}
}
& '%USERPROFILE%\AppData\Local\Programs\Python\Python314\python.exe' (Join-Path $research 'JOIN185-candidate.py')
if ($LASTEXITCODE -ne 0) {throw 'Candidate creation failed'}
Copy-Item -LiteralPath (Join-Path $rpc 'config\custom_configs\config_BLES01857.yml') -Destination (Join-Path $research 'JOIN185-before-title.yml')
Copy-Item -LiteralPath (Join-Path $rpc 'd1\d1ppu.txt') -Destination (Join-Path $research 'JOIN185-before-d1ppu.txt')
$text=(Get-Content -LiteralPath (Join-Path $rt 'SIGNON_LOCAL\JOIN184-launch.ps1') -Raw).Replace('JOIN184','JOIN185')
$text=$text.Replace('-AfterMatchSeconds 60','-AfterMatchSeconds 5').Replace('JOIN185-mission-marker-candidate.json','JOIN185-mission-candidate.json'); [IO.File]::WriteAllText($wrapper,$text,[Text.UTF8Encoding]::new($false))
$trace=@'
# JOIN185: bounded cinematic dispatch only; static call-site returns verified.
580928 4 r3:448
425654 4 if=lr==0x580a3c r3:4
425120 4 if=lr==0x42567c r3:4
42478c 4 if=lr==0x425694 r3:32
42c060 96 r3:208
42afec 96 r3:208
42b51c 256 r3:32 r4:192
'@
$trace+="`n"
[IO.File]::WriteAllText((Join-Path $research 'JOIN185-planned-d1ppu.txt'),$trace,[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText((Join-Path $rpc 'd1\d1ppu.txt'),$trace,[Text.UTF8Encoding]::new($false))
foreach($marker in 'd1checkpoint','d1depthfp.on'){[IO.File]::WriteAllText((Join-Path $rpc ('d1\'+$marker)),'')}
$receipt=[ordered]@{at=[DateTime]::UtcNow.ToString('o');build=$build;source_base='7bedf8e5df854bac79633baf242cc6110957a5f8';source_clean=$false;change='Private mission removes fixed revival stops/teleport for authored-completion diagnosis; production mission unchanged.';candidate_sha256=(Get-FileHash -LiteralPath (Join-Path $research 'JOIN185-mission-candidate.json')).Hash.ToLowerInvariant();probe_sha256=(Get-FileHash -LiteralPath 'F:\d1\d1xenia\scripts\rpcs3_final124_performance.py').Hash.ToLowerInvariant();trace=$trace;scope='C5 native diagnostics; manual handover must be labelled, no production playback acceptance claimed.'}
$receipt | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $research 'JOIN185-prepare.json') -Encoding utf8
$wrapper

