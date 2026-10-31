$ErrorActionPreference='Stop'
. 'F:\d1\d1xenia\scripts\Final124RunHelpers.ps1'
$env:D1_LOCAL='F:\d1\<pc>-import\d1xenia\local'
$env:D1_RPCS3=Join-Path $env:D1_LOCAL 'RPCS3_FINAL124_SMOKE001'
$client=(Read-D1ProcessManifest (Join-Path $env:D1_LOCAL 'SIGNON_LOCAL\logs\FINAL124_JOIN182-processes.json')).processes | Where-Object tag -EQ 'rpcs3'
$receipt=Join-Path $env:D1_LOCAL 'RESEARCH\JOIN182-visual-sampling.jsonl'
$until=[DateTime]::UtcNow.AddSeconds(190)
while ([DateTime]::UtcNow -lt $until) {
    if (-not (Get-D1OwnedProcess $client)) {throw 'Exact JOIN182 client closed'}
    $at=[DateTime]::UtcNow.ToString('o')
    $shot=& 'F:\d1\d1xenia\scripts\Invoke-RpcsStep.ps1' -ProcessId $client.id -WaitSeconds 0 | Select-Object -Last 1
    [ordered]@{at=$at;shot=$shot;scope='Native screenshot sampling; not continuous video or full audio acceptance'} | ConvertTo-Json -Compress | Add-Content -LiteralPath $receipt
    $shot
    Start-Sleep -Seconds 5
}
