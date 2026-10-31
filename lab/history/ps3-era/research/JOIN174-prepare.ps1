$ErrorActionPreference='Stop'
$rt='F:\d1\<pc>-import\d1xenia\local'
$rpc=Join-Path $rt 'RPCS3_FINAL124_SMOKE001'
$research=Join-Path $rt 'RESEARCH'
. 'F:\d1\d1xenia\scripts\Final124RunHelpers.ps1'
$old=Read-D1ProcessManifest -Path (Join-Path $rt 'SIGNON_LOCAL\logs\FINAL124_JOIN173-processes.json')
foreach ($identity in $old.processes) { if (Get-D1OwnedProcess -Identity $identity) { throw 'Prior owned stack is live' } }
$clients=@(Get-CimInstance Win32_Process -Filter "Name='rpcs3.exe'")
if ($clients.Count) { throw 'A native client is live; preserve it' }
foreach ($pair in @(@('config\custom_configs\config_BLES01857.yml','JOIN174-before-title.yml'),@('d1\d1ppu.txt','JOIN174-before-d1ppu.txt'))) {
    $target=Join-Path $research $pair[1]
    if (Test-Path -LiteralPath $target) { throw 'Preserve existing preparation backup' }
    Copy-Item -LiteralPath (Join-Path $rpc $pair[0]) -Destination $target
}
$build=(Get-FileHash -LiteralPath (Join-Path $rpc 'rpcs3.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
if ($build -ne 'c5bc7f5786835302dafe3265549e2052d3265df7025fc997bc6249e3647fe7cf') { throw 'Unexpected runtime build' }
$watch=@'
# JOIN174: direct record allocation and consumer of serialized47-byte channel17 record
48e7f0 80 if=r5==0x15
4797b8 80 if=lr==0x47a1fc if=m6+1:4==0x15 r6:13 r8:13 r3+0x114:20
d0fcc 80 if=lr==0x479ae0 if=r25==0x11 if=m24+1:4==0x15 r24:13 r26+0x114:20 r3:68
2bb60 80 if=lr==0x479d14 if=m5+1:4==0x15 r5:13 r26+0x114:20 r1+0x70:24
82db0 20 if=lr==0x47a76c if=m26+4:4==0x11 if=m26+0xc:4==47 r26:24 r26+0x18:88 r28+0x114:20
c03b60 80 if=lr==0x47a208 if=r24==0 if=m30+1:4==0x15 r30:13
'@
[IO.File]::WriteAllText((Join-Path $research 'JOIN174-planned-d1ppu.txt'),$watch,[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText((Join-Path $rpc 'd1\d1ppu.txt'),$watch,[Text.UTF8Encoding]::new($false))
foreach ($marker in 'd1checkpoint','d1depthfp.on') {
    $path=Join-Path $rpc ('d1\'+$marker)
    if (Test-Path -LiteralPath $path) { throw 'Unexpected existing active control' }
    [IO.File]::WriteAllText($path,'')
}
$wrapper=Join-Path $rt 'SIGNON_LOCAL\JOIN174-launch.ps1'
if (Test-Path -LiteralPath $wrapper) { throw 'Preserve existing wrapper' }
$text=(Get-Content -LiteralPath (Join-Path $rt 'SIGNON_LOCAL\JOIN173-launch.ps1') -Raw).Replace('JOIN173','JOIN174')
[IO.File]::WriteAllText($wrapper,$text,[Text.UTF8Encoding]::new($false))
[ordered]@{at=[datetime]::UtcNow.ToString('o');build=$build;native_build_not_changed=$true;trace=$watch;previous_owned_closed=$true;scope='Same Titan/highway fixture; replaces outer return-helper trace with actual direct record allocation and queue consumer. Serialized record47-byte filter selects channel17 length, not decoded target. No protocol/graphics repair.'} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $research 'JOIN174-prepare.json') -Encoding utf8
$wrapper
