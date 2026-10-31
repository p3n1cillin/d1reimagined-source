$ErrorActionPreference='Stop'
$rt='F:\d1\<pc>-import\d1xenia\local'
$rpc=Join-Path $rt 'RPCS3_FINAL124_SMOKE001'
$r=Join-Path $rt 'RESEARCH'
$prior=Get-Content -LiteralPath "$r\JOIN216-independent-closure-verified.json" -Raw|ConvertFrom-Json
if(-not $prior.all_hashes_match -or @($prior.checks).Count -ne 9 -or @($prior.checks | Where-Object exact_owned_alive).Count){throw 'Prior independent closure not verified'}
if(@(Get-CimInstance Win32_Process -Filter "Name='rpcs3.exe'").Count){throw 'Preserve active native client'}
$expected=@{
'rpcs3.exe'='C5BC7F5786835302DAFE3265549E2052D3265DF7025FC997BC6249E3647FE7CF'
'config\custom_configs\config_BLES01857.yml'='22C081E2680A9F81C565860262232AF1296EBEA2831849338BA79BD6E7C2350B'
'config\config.yml'='71C7E08939860F78C12A53D095DDC1CCD1525A46D1493714DEE6F9F85D229AB9'
'd1\d1ppu.txt'='442847E5847064F2D570943381CAA88DFD66E9DC212FFD299D9AC6D2F1EDD4B6'
}
foreach($entry in $expected.GetEnumerator()){if((Get-FileHash -LiteralPath (Join-Path $rpc $entry.Key)).Hash -ne $entry.Value){throw "Unexpected baseline $($entry.Key)"}}
foreach($p in @("$r\JOIN217-before-title.yml","$r\JOIN217-before-d1ppu.txt","$rt\SIGNON_LOCAL\JOIN217-launch.ps1","$rt\SIGNON_LOCAL\FINAL124_JOIN217-control.jsonl")){if(Test-Path -LiteralPath $p){throw 'Preserve existing JOIN217 files'}}
Copy-Item -LiteralPath "$rpc\config\custom_configs\config_BLES01857.yml" -Destination "$r\JOIN217-before-title.yml"
Copy-Item -LiteralPath "$rpc\d1\d1ppu.txt" -Destination "$r\JOIN217-before-d1ppu.txt"
$launch=(Get-Content -LiteralPath "$rt\SIGNON_LOCAL\JOIN216-launch.ps1" -Raw).Replace('JOIN216','JOIN217')
$old='--peer-family;--character-gear;starter:01.24:titan,@F:\d1\<pc>-import\d1xenia\local\RESEARCH\JOIN217-khvostov-gear.json'
$new='--peer-family;--character-gear;starter:01.24:titan;--character-overlay;@F:\d1\<pc>-import\d1xenia\local\RESEARCH\JOIN217-loot-overlay.json'
if(-not $launch.Contains($old)){throw 'Unexpected launch template'}
$launch=$launch.Replace($old,$new).Replace('bounded seven-minute checkpoint','bounded fifteen-minute checkpoint').Replace('JOIN217: production mission, natural Breach crossing and deeper exploration with inspected owned inputs; no guest writes or manual teleport.','JOIN217: unarmed starter plus two private synthetic pending-loot records; production mission, owned ordinary inputs; no guest writes or manual teleport.')
[IO.File]::WriteAllText("$rt\SIGNON_LOCAL\JOIN217-launch.ps1",$launch,[Text.UTF8Encoding]::new($false))
$trace=@'
# JOIN217: read-only loot prerequisites; no equipped rifle fixture.
2b36f8 24 if=m4:2==0xe0e r4:56 r6:48
2b3240 24 if=m5:2==0xe0e r5:56 r6:48
2b279c 8 r3:32
2b2a60 8 r3:32
2b2f74 8 r3:48
472518 8 r3:24
214478 8 r3+0x110:16
13f068 8 if=lr==0x2145d0
'@
$trace+="`n"
[IO.File]::WriteAllText("$r\JOIN217-planned-d1ppu.txt",$trace,[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText("$rpc\d1\d1ppu.txt",$trace,[Text.UTF8Encoding]::new($false))
foreach($marker in 'd1checkpoint','d1depthfp.on'){[IO.File]::WriteAllText("$rpc\d1\$marker",'')}
$diff=git -C 'F:\d1\d1xenia' diff
[IO.File]::WriteAllText("$r\JOIN217-tested-source.diff",($diff -join "`n"),[Text.UTF8Encoding]::new($false))
[ordered]@{utc=[datetime]::UtcNow.ToString('o');source_base=(git -C 'F:\d1\d1xenia' rev-parse HEAD).Trim();source_clean=$false;source_status=@(git -C 'F:\d1\d1xenia' status --short);change='Backend ownership fixes and new read-only loot probe under test; no mission/renderer change. Unarmed starter with two synthetic loot records, sources3/5. No position cache, native grant or pickup acceptance claimed.';trace=$trace;overlay_sha256=(Get-FileHash -LiteralPath "$r\JOIN217-loot-overlay.json").Hash;launch_sha256=(Get-FileHash -LiteralPath "$rt\SIGNON_LOCAL\JOIN217-launch.ps1").Hash}|ConvertTo-Json -Depth 5|Set-Content -LiteralPath "$r\JOIN217-prepare.json" -Encoding utf8
'JOIN217 prepared'
