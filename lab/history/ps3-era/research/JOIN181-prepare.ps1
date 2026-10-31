$ErrorActionPreference='Stop'
$rt='F:\d1\<pc>-import\d1xenia\local'
$rpc=Join-Path $rt 'RPCS3_FINAL124_SMOKE001'
$research=Join-Path $rt 'RESEARCH'
. 'F:\d1\d1xenia\scripts\Final124RunHelpers.ps1'
$closed=Get-Content -LiteralPath (Join-Path $research 'JOIN180-independent-closed.json') -Raw | ConvertFrom-Json
if(-not $closed.all_exact_owned_closed -or -not $closed.all_hashes_match){throw 'Prior cleanup is not verified'}
if(@(Get-CimInstance Win32_Process -Filter "Name='rpcs3.exe'").Count){throw 'A native client is live; preserve it'}
$build=(Get-FileHash -LiteralPath (Join-Path $rpc 'rpcs3.exe')).Hash.ToLowerInvariant()
if($build -ne 'c5bc7f5786835302dafe3265549e2052d3265df7025fc997bc6249e3647fe7cf'){throw 'Unexpected native executable'}
$title=(Get-FileHash -LiteralPath (Join-Path $rpc 'config\custom_configs\config_BLES01857.yml')).Hash
$watchHash=(Get-FileHash -LiteralPath (Join-Path $rpc 'd1\d1ppu.txt')).Hash
if($title -ne '22C081E2680A9F81C565860262232AF1296EBEA2831849338BA79BD6E7C2350B' -or $watchHash -ne '442847E5847064F2D570943381CAA88DFD66E9DC212FFD299D9AC6D2F1EDD4B6'){throw 'Unexpected configuration before test'}
$trace=@'
# JOIN181: bounded read-only objective and marker materialization watches.
3a68c0 64 r3:16
57b4ec 24 r3:48
57b3cc 24 if=lr==0x57b730 r3:48 r5:64
'@
$trace+="`n"
$wrapper=Join-Path $rt 'SIGNON_LOCAL\JOIN181-launch.ps1'
foreach($path in @($wrapper,(Join-Path $research 'JOIN181-before-title.yml'),(Join-Path $research 'JOIN181-before-d1ppu.txt'))){if(Test-Path -LiteralPath $path){throw 'Preserve existing test preparation'}}
foreach($marker in 'd1checkpoint','d1depthfp.on','d1descriptor','d1descriptorring','d1fp.on','d1rsxcapture.on'){
 if(Test-Path -LiteralPath (Join-Path $rpc ('d1\'+$marker))){throw 'Unexpected active diagnostic marker'}
}
Copy-Item -LiteralPath (Join-Path $rpc 'config\custom_configs\config_BLES01857.yml') -Destination (Join-Path $research 'JOIN181-before-title.yml')
Copy-Item -LiteralPath (Join-Path $rpc 'd1\d1ppu.txt') -Destination (Join-Path $research 'JOIN181-before-d1ppu.txt')
$text=(Get-Content -LiteralPath (Join-Path $rt 'SIGNON_LOCAL\JOIN180-launch.ps1') -Raw).Replace('JOIN180','JOIN181')
$old='--ah-mission;F:\d1\d1xenia\missions\a_guardian_rises.json'
$new='--ah-mission;F:\d1\<pc>-import\d1xenia\local\RESEARCH\JOIN181-mission-marker-candidate.json'
if(-not $text.Contains($old)){throw 'Existing launcher mission string changed'}
$text=$text.Replace($old,$new)
[IO.File]::WriteAllText($wrapper,$text,[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText((Join-Path $research 'JOIN181-planned-d1ppu.txt'),$trace,[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText((Join-Path $rpc 'd1\d1ppu.txt'),$trace,[Text.UTF8Encoding]::new($false))
foreach($marker in 'd1checkpoint','d1depthfp.on'){[IO.File]::WriteAllText((Join-Path $rpc ('d1\'+$marker)),'')}
$receipt=[ordered]@{at=[datetime]::UtcNow.ToString('o');source_commit='87c7e83acfcffd48d1ff2a1bef09f550aa81d6ae';data_commit='7893dea856fdd3cebd4467b89654636a86007426';source_clean=$true;build=$build;mission_candidate_sha256=(Get-FileHash -LiteralPath (Join-Path $research 'JOIN181-mission-marker-candidate.json')).Hash.ToLowerInvariant();trace=$trace;scope='Private native marker-registration/visibility candidate. Same C5, backend and fixture. No renderer build, guest writes, casting/timing change or presentation success claimed.'}
$receipt | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $research 'JOIN181-prepare.json') -Encoding utf8
$wrapper
