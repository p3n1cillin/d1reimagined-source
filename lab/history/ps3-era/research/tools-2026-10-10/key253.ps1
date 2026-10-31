param([string]$Keys='F12',[int]$HoldMs=150)
$ErrorActionPreference='Stop'
Set-Location F:\d1\d1xenia
. .\scripts\Final124RunHelpers.ps1
$m=Read-D1ProcessManifest 'F:\d1\<pc>-import\d1xenia\local\SIGNON_LOCAL\logs\FINAL124_JOIN253-processes.json'
$client=$m.processes | Where-Object tag -EQ 'rpcs3'
$shots='F:\d1\<pc>-import\d1xenia\local\RPCS3_FINAL124_SMOKE001\screenshots\BLES01857'
$before=(Get-ChildItem $shots -File | Sort-Object LastWriteTimeUtc | Select-Object -Last 1).FullName
& .\scripts\Send-RpcsKey.ps1 -Keys $Keys -HoldMs $HoldMs -ProcessId $client.id -ProcessIdentity $client | Out-Null
"$([DateTime]::UtcNow.ToString('o')) sent $Keys $HoldMs"
if ($Keys -eq 'F12') { for($i=0;$i -lt 40;$i++){ Start-Sleep -Milliseconds 500; $now=(Get-ChildItem $shots -File | Sort-Object LastWriteTimeUtc | Select-Object -Last 1).FullName; if($now -ne $before){ Start-Sleep -Milliseconds 700; $now; break } } }

































