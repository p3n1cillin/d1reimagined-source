$ErrorActionPreference='Stop'
$rt='F:\d1\<pc>-import\d1xenia\local'
$rpc=Join-Path $rt 'RPCS3_FINAL124_SMOKE001'
$r=Join-Path $rt 'RESEARCH'
. F:\d1\d1xenia\scripts\Final124RunHelpers.ps1
$prior=Get-Content -LiteralPath "$r\JOIN243-independent-closure-verified.json" -Raw | ConvertFrom-Json
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
foreach($path in @("$r\JOIN244-before-title.yml","$r\JOIN244-before-d1ppu.txt","$rt\SIGNON_LOCAL\JOIN244-launch.ps1","$rt\SIGNON_LOCAL\FINAL124_JOIN244-control.jsonl")){if(Test-Path -LiteralPath $path){throw 'Preserve existing JOIN244 files'}}
$diff=git -C 'F:\d1\d1xenia' diff
[IO.File]::WriteAllText("$r\JOIN244-tested-source.diff",($diff -join "`n"),[Text.UTF8Encoding]::new($false))
$launch=(Get-Content -LiteralPath "$rt\SIGNON_LOCAL\JOIN226-launch.ps1" -Raw).Replace('--character-overlay;@F:\d1\<pc>-import\d1xenia\local\RESEARCH\JOIN226-loot-overlay.json','--world-loot;F:\d1\d1xenia\missions\a_guardian_rises.json').Replace('JOIN226','JOIN244').Replace('unarmed starter with one synthetic pending rifle; explicit catalog selector2 incident test;','unarmed starter; mission-announced starter rifle (--world-loot), no private overlay or control event;')
$trace=@'
# JOIN244: dialog manager queue calls and investment-side callers (urgent dialog source).
19cd24 24 r3:32 r4:32
19cdb0 24 r3:32 r4:32
19d04c 24 r3:32 r4:32
19c604 24 r3:32
455008 12 r3:32
4551c8 12 r3:32
456410 24 r3:32 r4:32
457480 12 r3:32
'@
$trace+="`n"
Copy-Item -LiteralPath "$rpc\config\custom_configs\config_BLES01857.yml" -Destination "$r\JOIN244-before-title.yml"
Copy-Item -LiteralPath "$rpc\d1\d1ppu.txt" -Destination "$r\JOIN244-before-d1ppu.txt"
[IO.File]::WriteAllText("$rt\SIGNON_LOCAL\JOIN244-launch.ps1",$launch,[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText("$r\JOIN244-planned-d1ppu.txt",$trace,[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText("$rpc\d1\d1ppu.txt",$trace,[Text.UTF8Encoding]::new($false))
foreach($marker in 'd1checkpoint','d1depthfp.on'){[IO.File]::WriteAllText("$rpc\d1\$marker",'')}
[ordered]@{utc=[datetime]::UtcNow.ToString('o');source_base=(git -C 'F:\d1\d1xenia' rev-parse HEAD).Trim();source_clean=$false;source_status=@(git -C 'F:\d1\d1xenia' status --short);change='Read-only trace of the dialog manager queue calls (0x19cd24, 0x19cdb0, 0x19d04c, 0x19c604) and investment callers, to find what raises the urgent dialog at world entry and after the grant. Committed source bb55871. No control lines or guest writes.';trace=$trace;launch_sha256=(Get-FileHash -LiteralPath "$rt\SIGNON_LOCAL\JOIN244-launch.ps1").Hash}|ConvertTo-Json -Depth 5|Set-Content -LiteralPath "$r\JOIN244-prepare.json" -Encoding utf8
'JOIN244 prepared'




























