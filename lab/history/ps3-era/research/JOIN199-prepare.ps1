$ErrorActionPreference='Stop'
$taskRt='F:\d1\<pc>-import\d1xenia\local'
$taskRpc=Join-Path $taskRt 'RPCS3_FINAL124_SMOKE001'
$taskResearch=Join-Path $taskRt 'RESEARCH'
$taskCleanup=Get-Content -LiteralPath "$taskRt\SIGNON_LOCAL\logs\JOIN198-cleanup-result.json" -Raw | ConvertFrom-Json
if(-not $taskCleanup.all_owned_closed -or $taskCleanup.errors.Count -or $taskCleanup.active_controls.Count){throw 'Prior cleanup requires attention'}
if(@(Get-CimInstance Win32_Process -Filter "Name='rpcs3.exe'").Count){throw 'Preserve active native client'}
if((Get-FileHash -LiteralPath "$taskRpc\rpcs3.exe").Hash.ToLowerInvariant() -ne 'c5bc7f5786835302dafe3265549e2052d3265df7025fc997bc6249e3647fe7cf'){throw 'Unexpected native executable'}
if((Get-FileHash -LiteralPath "$taskRpc\config\custom_configs\config_BLES01857.yml").Hash -ne '22C081E2680A9F81C565860262232AF1296EBEA2831849338BA79BD6E7C2350B'){throw 'Unexpected title configuration'}
if((Get-FileHash -LiteralPath "$taskRpc\d1\d1ppu.txt").Hash -ne '442847E5847064F2D570943381CAA88DFD66E9DC212FFD299D9AC6D2F1EDD4B6'){throw 'Unexpected watch configuration'}
foreach($taskPath in "$taskResearch\JOIN199-before-title.yml","$taskResearch\JOIN199-before-d1ppu.txt","$taskRt\SIGNON_LOCAL\JOIN199-launch.ps1","$taskRt\SIGNON_LOCAL\FINAL124_JOIN199-control.jsonl"){
    if(Test-Path -LiteralPath $taskPath){throw 'Preserve existing JOIN199 files'}
}
& '%USERPROFILE%\AppData\Local\Programs\Python\Python314\python.exe' "$taskResearch\JOIN199-candidate.py"
if($LASTEXITCODE -ne 0){throw 'Private overlay verification failed'}
Copy-Item -LiteralPath "$taskRpc\config\custom_configs\config_BLES01857.yml" -Destination "$taskResearch\JOIN199-before-title.yml"
Copy-Item -LiteralPath "$taskRpc\d1\d1ppu.txt" -Destination "$taskResearch\JOIN199-before-d1ppu.txt"
$taskText=(Get-Content -LiteralPath "$taskRt\SIGNON_LOCAL\JOIN198-launch.ps1" -Raw).Replace('JOIN198','JOIN199').Replace('AddSeconds(1500)','AddSeconds(900)')
$taskText=$taskText.Replace("-BapExtra '--peer-family;--character-gear;starter:01.24:titan'","-BapExtra '--peer-family;--character-gear;starter:01.24:titan;--character-overlay;@F:\d1\<pc>-import\d1xenia\local\RESEARCH\JOIN199-ghost-appearance-overlay.json'")
if($taskText -match 'JOIN193' -or $taskText -notmatch 'character-overlay'){throw 'Wrong wrapper candidate'}
[IO.File]::WriteAllText("$taskRt\SIGNON_LOCAL\JOIN199-launch.ps1",$taskText,[Text.UTF8Encoding]::new($false))
$taskTrace=@'
# JOIN199: appearance=42, arrangement=2706 plus private Ghost channel 15/cosmetic index 1; distinguish unit, override, cache.
71ec90 16 r3:48
71edcc 16 r1+0x74:4 r26:12
71ed90 16 r1+0x84:8
2a65a8 16 if=lr==0x71ee38 r3:4
42fa70 8 r20:824
71ee68 16 r1+0x7c:4 r23:4
2d184c 8 r5:260
2d1880 8 r1+0x70:4
42fae0 8 r20:824
c67e8 12 if=lr==0x42fa78 r3:12
42b51c 4 r3:208
'@
$taskTrace+="`n"
[IO.File]::WriteAllText("$taskResearch\JOIN199-planned-d1ppu.txt",$taskTrace,[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText("$taskRpc\d1\d1ppu.txt",$taskTrace,[Text.UTF8Encoding]::new($false))
foreach($taskMarker in 'd1checkpoint','d1depthfp.on'){[IO.File]::WriteAllText("$taskRpc\d1\$taskMarker",'')}
[ordered]@{at_utc=[DateTime]::UtcNow.ToString('o');source_base=(git -C 'F:\d1\d1xenia' rev-parse HEAD).Trim();source_clean=(-not(git -C 'F:\d1\d1xenia' status --porcelain));change='Private peer/select equipped Ghost slot +0xa=42; Ghost channel 15/cosmetic index 1 candidate; Ghost arrangement 2706 or renderer change.';overlay_sha256=(Get-FileHash -LiteralPath "$taskResearch\JOIN199-ghost-appearance-overlay.json").Hash.ToLowerInvariant();trace=$taskTrace} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath "$taskResearch\JOIN199-prepare.json" -Encoding utf8
