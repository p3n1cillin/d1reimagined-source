$ErrorActionPreference='Stop'
$rt='F:\d1\<pc>-import\d1xenia\local'
$rpc=Join-Path $rt 'RPCS3_FINAL124_SMOKE001'
$r=Join-Path $rt 'RESEARCH'
. F:\d1\d1xenia\scripts\Final124RunHelpers.ps1
$prior=Get-Content -LiteralPath "$r\JOIN220-independent-closure-verified.json" -Raw | ConvertFrom-Json
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
foreach($path in @("$r\JOIN221-before-title.yml","$r\JOIN221-before-d1ppu.txt","$rt\SIGNON_LOCAL\JOIN221-launch.ps1","$rt\SIGNON_LOCAL\FINAL124_JOIN221-control.jsonl")){if(Test-Path -LiteralPath $path){throw 'Preserve existing JOIN221 files'}}
$diff=git -C 'F:\d1\d1xenia' diff
[IO.File]::WriteAllText("$r\JOIN221-tested-source.diff",($diff -join "`n"),[Text.UTF8Encoding]::new($false))
$launch=(Get-Content -LiteralPath "$rt\SIGNON_LOCAL\JOIN220-launch.ps1" -Raw).Replace('JOIN220-loot-overlay.json','JOIN221-loot-overlay.json').Replace('JOIN220','JOIN221').Replace('ordinary guarded movement; no guest writes or manual teleport.','queue-gate watches; no movement, guest writes or teleport.')
$trace=@'
# JOIN221: queue-submission gates for the explicit selector-2 host incident (synthetic fixture).
27d744 8 if=r3==25 r5:128
12f9d4 40 if=lr==0x27d774
bee3a8 40 if=lr==0x27d798
1db184 40 if=lr==0x27d7b8 r3:32 r4:32
bee268 40 if=lr==0x27d7d8
1f6ec8 40 if=lr==0x27d864
99e380 40 if=lr==0x27d8a4
1f6f88 40 if=lr==0x27d8ac r3:32
5aede4 12 r4:24
2b2a60 12 r3:128
472518 12 r3:24
'@
$trace+="`n"
Copy-Item -LiteralPath "$rpc\config\custom_configs\config_BLES01857.yml" -Destination "$r\JOIN221-before-title.yml"
Copy-Item -LiteralPath "$rpc\d1\d1ppu.txt" -Destination "$r\JOIN221-before-d1ppu.txt"
[IO.File]::WriteAllText("$rt\SIGNON_LOCAL\JOIN221-launch.ps1",$launch,[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText("$r\JOIN221-planned-d1ppu.txt",$trace,[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText("$rpc\d1\d1ppu.txt",$trace,[Text.UTF8Encoding]::new($false))
foreach($marker in 'd1checkpoint','d1depthfp.on'){[IO.File]::WriteAllText("$rpc\d1\$marker",'')}
[ordered]@{utc=[datetime]::UtcNow.ToString('o');source_base=(git -C 'F:\d1\d1xenia' rev-parse HEAD).Trim();source_clean=$false;source_status=@(git -C 'F:\d1\d1xenia' status --short);change='Committed host-incident control (2770086). Same private unarmed starter, synthetic pending rifle and selector-2 body as JOIN220; watches each queue-submission gate (0x12f9d4, serialize 0x1db184, 0xbee268, alloc 0x1f6ec8, commit 0x1f6f88). No production changes or guest writes; no pickup acceptance.';trace=$trace;launch_sha256=(Get-FileHash -LiteralPath "$rt\SIGNON_LOCAL\JOIN221-launch.ps1").Hash}|ConvertTo-Json -Depth 5|Set-Content -LiteralPath "$r\JOIN221-prepare.json" -Encoding utf8
'JOIN221 prepared'

