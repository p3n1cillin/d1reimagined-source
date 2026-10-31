$ErrorActionPreference='Stop'
$rt='F:\d1\<pc>-import\d1xenia\local'
$rpc=Join-Path $rt 'RPCS3_FINAL124_SMOKE001'
$r=Join-Path $rt 'RESEARCH'
. F:\d1\d1xenia\scripts\Final124RunHelpers.ps1
$prior=Get-Content -LiteralPath "$r\JOIN218-independent-closure-verified.json" -Raw | ConvertFrom-Json
if(-not $prior.all_hashes_match -or @($prior.checks).Count -ne 9 -or @($prior.checks | Where-Object exact_owned_alive).Count){throw 'Prior independent closure not verified'}
foreach($check in $prior.checks){if(Get-D1OwnedProcess $check.identity){throw 'Prior owned identity still alive'}}
if(@(Get-CimInstance Win32_Process -Filter "Name='rpcs3.exe'").Count){throw 'Preserve active native client'}
$expected=@{
'rpcs3.exe'='C5BC7F5786835302DAFE3265549E2052D3265DF7025FC997BC6249E3647FE7CF'
'config\custom_configs\config_BLES01857.yml'='22C081E2680A9F81C565860262232AF1296EBEA2831849338BA79BD6E7C2350B'
'config\config.yml'='71C7E08939860F78C12A53D095DDC1CCD1525A46D1493714DEE6F9F85D229AB9'
'd1\d1ppu.txt'='442847E5847064F2D570943381CAA88DFD66E9DC212FFD299D9AC6D2F1EDD4B6'
}
foreach($entry in $expected.GetEnumerator()){if((Get-FileHash -LiteralPath (Join-Path $rpc $entry.Key)).Hash -ne $entry.Value){throw "Unexpected baseline $($entry.Key)"}}
foreach($path in @("$r\JOIN219-before-title.yml","$r\JOIN219-before-d1ppu.txt","$rt\SIGNON_LOCAL\JOIN219-launch.ps1","$rt\SIGNON_LOCAL\FINAL124_JOIN219-control.jsonl")){if(Test-Path -LiteralPath $path){throw 'Preserve existing JOIN219 files'}}
if(@(git -C 'F:\d1\d1xenia' status --porcelain).Count){throw 'Record changed source before launch'}
$launch=(Get-Content -LiteralPath "$rt\SIGNON_LOCAL\JOIN218-launch.ps1" -Raw).Replace('JOIN218','JOIN219')
$trace=@'
# JOIN219: client incident context and body selection, read-only.
12f2e8 12 r4:96
1cb0b0 12 r4:64 r4+0x130:24
1caf20 16 r4:12 r5:24 r6:64
26ba98 12 r3:24
5aea30 12 r4:64 r4+0x130:24
5ae7a4 12 r4:12 r5:24
5aede4 12 r4:24
2b279c 12 r3:32
2b2a60 12 r3:32
2b2f74 12 r3:48
472518 12 r3:24
'@
$trace+="`n"
Copy-Item -LiteralPath "$rpc\config\custom_configs\config_BLES01857.yml" -Destination "$r\JOIN219-before-title.yml"
Copy-Item -LiteralPath "$rpc\d1\d1ppu.txt" -Destination "$r\JOIN219-before-d1ppu.txt"
[IO.File]::WriteAllText("$rt\SIGNON_LOCAL\JOIN219-launch.ps1",$launch,[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText("$r\JOIN219-planned-d1ppu.txt",$trace,[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText("$rpc\d1\d1ppu.txt",$trace,[Text.UTF8Encoding]::new($false))
foreach($marker in 'd1checkpoint','d1depthfp.on'){[IO.File]::WriteAllText("$rpc\d1\$marker",'')}
[ordered]@{utc=[datetime]::UtcNow.ToString('o');source_base=(git -C 'F:\d1\d1xenia' rev-parse HEAD).Trim();source_clean=$true;source_status=@();change='Native incoming incident interface and action-body lookup; unmodified starter, no mission/renderer change, echo or incident injection.';trace=$trace;launch_sha256=(Get-FileHash -LiteralPath "$rt\SIGNON_LOCAL\JOIN219-launch.ps1").Hash}|ConvertTo-Json -Depth 5|Set-Content -LiteralPath "$r\JOIN219-prepare.json" -Encoding utf8
'JOIN219 prepared'
