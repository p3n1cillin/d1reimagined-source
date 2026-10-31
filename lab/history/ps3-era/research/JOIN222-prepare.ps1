$ErrorActionPreference='Stop'
$rt='F:\d1\<pc>-import\d1xenia\local'
$rpc=Join-Path $rt 'RPCS3_FINAL124_SMOKE001'
$r=Join-Path $rt 'RESEARCH'
. F:\d1\d1xenia\scripts\Final124RunHelpers.ps1
$prior=Get-Content -LiteralPath "$r\JOIN221-independent-closure-verified.json" -Raw | ConvertFrom-Json
if(-not $prior.all_hashes_match -or @($prior.checks).Count -lt 7 -or @($prior.checks | Where-Object exact_owned_alive).Count){throw 'Prior independent closure not verified'}
foreach($check in $prior.checks){if(Get-D1OwnedProcess $check.identity){throw 'Prior owned identity still alive'}}
if(@(Get-CimInstance Win32_Process -Filter "Name='rpcs3.exe'").Count){throw 'Preserve active native client'}
$expected=@{
'rpcs3.exe'='C5BC7F5786835302DAFE3265549E2052D3265DF7025FC997BC6249E3647FE7CF'
'config\custom_configs\config_BLES01857.yml'='22C081E2680A9F81C565860262232AF1296EBEA2831849338BA79BD6E7C2350B'
'config\config.yml'='71C7E08939860F78C12A53D095DDC1CCD1525A46D1493714DEE6F9F85D229AB9'
'd1\d1ppu.txt'='442847E5847064F2D570943381CAA88DFD66E9DC212FFD299D9AC6D2F1EDD4B6'
}
foreach($entry in $expected.GetEnumerator()){if((Get-FileHash -LiteralPath (Join-Path $rpc $entry.Key)).Hash -ne $entry.Value){throw "Unexpected baseline $($entry.Key)"}}
foreach($path in @("$r\JOIN222-before-title.yml","$r\JOIN222-before-d1ppu.txt","$rt\SIGNON_LOCAL\JOIN222-launch.ps1","$rt\SIGNON_LOCAL\FINAL124_JOIN222-control.jsonl")){if(Test-Path -LiteralPath $path){throw 'Preserve existing JOIN222 files'}}
$diff=git -C 'F:\d1\d1xenia' diff
[IO.File]::WriteAllText("$r\JOIN222-tested-source.diff",($diff -join "`n"),[Text.UTF8Encoding]::new($false))
$launch=(Get-Content -LiteralPath "$rt\SIGNON_LOCAL\JOIN221-launch.ps1" -Raw).Replace('JOIN221','JOIN222').Replace('ordinary guarded movement; no guest writes or manual teleport.','recipient-check watches; no movement, guest writes or teleport.')
$trace=@'
# JOIN222: recipient/schema evaluation for the 1504 event at handler 0x5aede4 (synthetic fixture).
5ae120 6 if=m4:4==0x84f68b44 r5:32 r6:128
26a544 6 if=m3:4==0x84f68b44 r4:32 r5:64
26b9b0 6 if=m4:4==0x84f68b44 r3:768
26a3dc 6 if=m3:4==0x84f68b44 r4:64 r5:32
26a00c 12 if=lr==0x26a630 r3:32 r4:32
26a1d8 12 if=lr==0x26a4e4 r3:4 r5:32
5aede4 12 r4:24
2b2a60 12 r3:128
472518 12 r3:24
'@
$trace+="`n"
Copy-Item -LiteralPath "$rpc\config\custom_configs\config_BLES01857.yml" -Destination "$r\JOIN222-before-title.yml"
Copy-Item -LiteralPath "$rpc\d1\d1ppu.txt" -Destination "$r\JOIN222-before-d1ppu.txt"
[IO.File]::WriteAllText("$rt\SIGNON_LOCAL\JOIN222-launch.ps1",$launch,[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText("$r\JOIN222-planned-d1ppu.txt",$trace,[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText("$rpc\d1\d1ppu.txt",$trace,[Text.UTF8Encoding]::new($false))
foreach($marker in 'd1checkpoint','d1depthfp.on'){[IO.File]::WriteAllText("$rpc\d1\$marker",'')}
[ordered]@{utc=[datetime]::UtcNow.ToString('o');source_base=(git -C 'F:\d1\d1xenia' rev-parse HEAD).Trim();source_clean=$false;source_status=@(git -C 'F:\d1\d1xenia' status --short);change='Unchanged source (2770086). Same private unarmed starter, synthetic pending rifle and selector-2 body as JOIN220/221; watches the recipient and schema evaluation (0x5ae120, 0x26a544, 0x26b9b0, 0x26a3dc) before handler 0x5aede4. No production changes or guest writes; no pickup acceptance.';trace=$trace;launch_sha256=(Get-FileHash -LiteralPath "$rt\SIGNON_LOCAL\JOIN222-launch.ps1").Hash}|ConvertTo-Json -Depth 5|Set-Content -LiteralPath "$r\JOIN222-prepare.json" -Encoding utf8
'JOIN222 prepared'



