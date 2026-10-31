$ErrorActionPreference='Stop'
$rt='F:\d1\<pc>-import\d1xenia\local'
$rpc=Join-Path $rt 'RPCS3_FINAL124_SMOKE001'
$r=Join-Path $rt 'RESEARCH'
. F:\d1\d1xenia\scripts\Final124RunHelpers.ps1
$prior=Get-Content -LiteralPath "$r\JOIN250-independent-closure-verified.json" -Raw | ConvertFrom-Json
if(-not $prior.all_hashes_match -or @($prior.checks).Count -lt 1 -or @($prior.checks | Where-Object exact_owned_alive).Count){throw 'Prior independent closure not verified'}
foreach($check in $prior.checks){if(Get-D1OwnedProcess $check.identity){throw 'Prior owned identity still alive'}}
if(@(Get-CimInstance Win32_Process -Filter "Name='rpcs3.exe'").Count){throw 'Preserve active native client'}
$expected=@{
'rpcs3.exe'='C5BC7F5786835302DAFE3265549E2052D3265DF7025FC997BC6249E3647FE7CF'
'config\custom_configs\config_BLES01857.yml'='22C081E2680A9F81C565860262232AF1296EBEA2831849338BA79BD6E7C2350B'
'config\config.yml'='71C7E08939860F78C12A53D095DDC1CCD1525A46D1493714DEE6F9F85D229AB9'
'd1\d1ppu.txt'='442847E5847064F2D570943381CAA88DFD66E9DC212FFD299D9AC6D2F1EDD4B6'
}
foreach($entry in $expected.GetEnumerator()){if((Get-FileHash -LiteralPath (Join-Path $rpc $entry.Key)).Hash -ne $entry.Value){throw "Unexpected baseline $($entry.Key)"}}
foreach($path in @("$r\JOIN251-before-title.yml","$r\JOIN251-before-d1ppu.txt","$rt\SIGNON_LOCAL\JOIN251-launch.ps1","$rt\SIGNON_LOCAL\FINAL124_JOIN251-control.jsonl")){if(Test-Path -LiteralPath $path){throw 'Preserve existing JOIN251 files'}}
$diff=git -C 'F:\d1\d1xenia' diff
[IO.File]::WriteAllText("$r\JOIN251-tested-source.diff",($diff -join "`n"),[Text.UTF8Encoding]::new($false))
$launch=(Get-Content -LiteralPath "$rt\SIGNON_LOCAL\JOIN244-launch.ps1" -Raw).Replace('JOIN244','JOIN251').Replace(';--world-loot;F:\d1\d1xenia\missions\a_guardian_rises.json''',';--world-loot;F:\d1\d1xenia\missions\a_guardian_rises.json;--universe-flag-values;3390=1,309=1,2857=1''').Replace('no private overlay or control event;','no private overlay or control event; universe flag 2857 false (urgent dialog expression test);')
if(-not $launch.Contains('2857=1')){throw 'flag override not applied'}
$trace=@'
# JOIN251: universe 2857 = false; urgent entry selection and enqueue.
48a2e0 30 r3:16 r4:64
753430 20 r3:16
c5be34 20 if=r4==2857 r3:4
'@
$trace+="`n"
Copy-Item -LiteralPath "$rpc\config\custom_configs\config_BLES01857.yml" -Destination "$r\JOIN251-before-title.yml"
Copy-Item -LiteralPath "$rpc\d1\d1ppu.txt" -Destination "$r\JOIN251-before-d1ppu.txt"
[IO.File]::WriteAllText("$rt\SIGNON_LOCAL\JOIN251-launch.ps1",$launch,[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText("$r\JOIN251-planned-d1ppu.txt",$trace,[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText("$rpc\d1\d1ppu.txt",$trace,[Text.UTF8Encoding]::new($false))
foreach($marker in 'd1checkpoint','d1depthfp.on'){[IO.File]::WriteAllText("$rpc\d1\$marker",'')}
[ordered]@{utc=[datetime]::UtcNow.ToString('o');source_base=(git -C 'F:\d1\d1xenia' rev-parse HEAD).Trim();source_clean=$false;source_status=@(git -C 'F:\d1\d1xenia' status --short);change='Universe flag override 2857=1 (false). JOIN250 traced the urgent dialog to the MOTD selector 0x48a570 (entry 0x70092668, unlock expression at +0x1c, time slot 12, cooldown 300 s) and the enqueue in 0x86ae1c; if content indexes map linearly (0x525->0xb1f) the urgent index 0x52f is runtime 0xb29. The drive also reads the live expression. Read-only watches. Same mission drive. Committed source bb55871. No control lines or guest writes.';trace=$trace;launch_sha256=(Get-FileHash -LiteralPath "$rt\SIGNON_LOCAL\JOIN251-launch.ps1").Hash}|ConvertTo-Json -Depth 5|Set-Content -LiteralPath "$r\JOIN251-prepare.json" -Encoding utf8
'JOIN251 prepared'




























