$ErrorActionPreference='Stop'
$rt='F:\d1\<pc>-import\d1xenia\local'
. 'F:\d1\d1xenia\scripts\Final124RunHelpers.ps1'
$manifest=Read-D1ProcessManifest (Join-Path $rt 'SIGNON_LOCAL\logs\FINAL124_JOIN183-processes.json')
$client=$manifest.processes | Where-Object tag -EQ 'rpcs3'
if(-not(Get-D1OwnedProcess $client)){throw 'Exact client is closed'}
& 'F:\d1\d1xenia\scripts\Send-RpcsKey.ps1' -Keys X -HoldMs 400 -ProcessId 22436 -ProcessIdentity $client
$watch=Join-Path $rt 'RPCS3_FINAL124_SMOKE001\d1\d1ppu.txt'
$planned=Join-Path $rt 'RESEARCH\JOIN183-planned-d1ppu.txt'
if((Get-FileHash -LiteralPath $watch).Hash -ne (Get-FileHash -LiteralPath $planned).Hash){throw 'Watch changed; preserve it'}
$additional=@'
42c060 2 if=r3==0x60393a20 r3:208
42c060 2 if=r3==0x60396c00 r3:208
42afec 2 if=r3==0x60393a20 r3:208
42afec 2 if=r3==0x60396c00 r3:208
42b51c 2 if=r3==0x60393a20 r3:208 r4:192
42b51c 2 if=r3==0x60396c00 r3:208 r4:192
'@
[IO.File]::AppendAllText($watch,$additional+"`n",[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText((Join-Path $rt 'RESEARCH\JOIN183-followup-d1ppu.txt'),[IO.File]::ReadAllText($watch),[Text.UTF8Encoding]::new($false))
[ordered]@{at=[DateTime]::UtcNow.ToString('o');client=$client;action='Dismiss inspected Urgent Message with Cross; append read-only per-instance dispatch watches after matching installed watch bytes.'} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $rt 'RESEARCH\JOIN183-dismiss-and-watch.json') -Encoding utf8
