$ErrorActionPreference='Stop'
$rt='F:\d1\<pc>-import\d1xenia\local'
$rpc=Join-Path $rt 'RPCS3_FINAL124_SMOKE001'
$r=Join-Path $rt 'RESEARCH'
$cleanup=Get-Content -LiteralPath "$rt\SIGNON_LOCAL\logs\JOIN200-cleanup-result.json" -Raw|ConvertFrom-Json
if(-not $cleanup.all_owned_closed -or $cleanup.errors.Count -or $cleanup.active_controls.Count){throw 'Prior cleanup failed'}
if(@(Get-CimInstance Win32_Process -Filter "Name='rpcs3.exe'").Count){throw 'Preserve active native client'}
if((Get-FileHash -LiteralPath "$rpc\rpcs3.exe").Hash.ToLowerInvariant() -ne 'c5bc7f5786835302dafe3265549e2052d3265df7025fc997bc6249e3647fe7cf'){throw 'Unexpected native executable'}
if((Get-FileHash -LiteralPath "$rpc\config\custom_configs\config_BLES01857.yml").Hash -ne '22C081E2680A9F81C565860262232AF1296EBEA2831849338BA79BD6E7C2350B'){throw 'Unexpected title config'}
if((Get-FileHash -LiteralPath "$rpc\config\config.yml").Hash -ne '71C7E08939860F78C12A53D095DDC1CCD1525A46D1493714DEE6F9F85D229AB9'){throw 'Unexpected global config'}
if((Get-FileHash -LiteralPath "$rpc\d1\d1ppu.txt").Hash -ne '442847E5847064F2D570943381CAA88DFD66E9DC212FFD299D9AC6D2F1EDD4B6'){throw 'Unexpected watch config'}
foreach($p in @("$r\JOIN201-before-title.yml","$r\JOIN201-before-d1ppu.txt","$rt\SIGNON_LOCAL\JOIN201-launch.ps1","$rt\SIGNON_LOCAL\FINAL124_JOIN201-control.jsonl")) {
    if(Test-Path -LiteralPath $p){throw 'Preserve existing JOIN201 files'}
}
Copy-Item -LiteralPath "$rpc\config\custom_configs\config_BLES01857.yml" -Destination "$r\JOIN201-before-title.yml"
Copy-Item -LiteralPath "$rpc\d1\d1ppu.txt" -Destination "$r\JOIN201-before-d1ppu.txt"
$launch=(Get-Content -LiteralPath "$rt\SIGNON_LOCAL\JOIN200-launch.ps1" -Raw).Replace('JOIN200','JOIN201')
$launch=$launch.Replace(';--character-gear;starter:01.24:titan',';--character-gear;@F:\d1\<pc>-import\d1xenia\local\RESEARCH\JOIN201-titan-ghost-gear.json')
if($launch -match 'character-overlay' -or $launch -notmatch 'JOIN201-titan-ghost-gear'){throw 'Expected gear file, no overlay'}
[IO.File]::WriteAllText("$rt\SIGNON_LOCAL\JOIN201-launch.ps1",$launch,[Text.UTF8Encoding]::new($false))
$trace=@'
# JOIN201: art-record Ghost appearance 0 and dye 15/2493; verify unit/cache resolution and Nav null component.
71ec90 12 r3:48
71edcc 12 r1+0x74:4
2d1880 8 r1+0x70:4
42fae0 4 r20:824
1fb3c8 8 if=r4==0x0 if=lr==0x2ff330
'@
$trace+="`n"
[IO.File]::WriteAllText("$r\JOIN201-planned-d1ppu.txt",$trace,[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText("$rpc\d1\d1ppu.txt",$trace,[Text.UTF8Encoding]::new($false))
foreach($marker in 'd1checkpoint','d1depthfp.on'){[IO.File]::WriteAllText("$rpc\d1\$marker",'')}
$diff=git -C 'F:\d1\d1xenia' diff -- server/bap_server.py server/test_bap_server.py
[IO.File]::WriteAllText("$r\JOIN201-tested-source.diff",($diff -join "`n"),[Text.UTF8Encoding]::new($false))
[ordered]@{utc=[datetime]::UtcNow.ToString('o');source_base=(git -C 'F:\d1\d1xenia' rev-parse HEAD).Trim();source_clean=$true;
    server_sha256=(Get-FileHash -LiteralPath 'F:\d1\d1xenia\server\bap_server.py').Hash;
    tested_source_diff_sha256=(Get-FileHash -LiteralPath "$r\JOIN201-tested-source.diff").Hash;
    change='Private gear file: Titan starter kit plus Generalist appearance 0 and dye 15/2493 from its art record; committed server, unchanged renderer and mission.';
    trace=$trace}|ConvertTo-Json -Depth 5|Set-Content -LiteralPath "$r\JOIN201-prepare.json" -Encoding utf8
