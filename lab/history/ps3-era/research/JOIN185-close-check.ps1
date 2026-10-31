$ErrorActionPreference='Stop'
. 'F:\d1\d1xenia\scripts\Final124RunHelpers.ps1'
$rt='F:\d1\<pc>-import\d1xenia\local'
$logs=Join-Path $rt 'SIGNON_LOCAL\logs'
$rpc=Join-Path $rt 'RPCS3_FINAL124_SMOKE001'
$identities=@((Read-D1ProcessManifest (Join-Path $logs 'FINAL124_JOIN185-processes.json')).processes)
foreach($name in 'FINAL124_JOIN185-proxy-process.json','JOIN185-session-probe-process.json') {
    $identities+=Get-Content -LiteralPath (Join-Path $logs $name) -Raw | ConvertFrom-Json
}
$identities+=(Get-Content -LiteralPath (Join-Path $rt 'RESEARCH\JOIN185-recorder-owned-process.json') -Raw | ConvertFrom-Json).recorder
$identities+=(Get-Content -LiteralPath (Join-Path $rt 'RESEARCH\JOIN185-performance-probe-process.json') -Raw | ConvertFrom-Json)
$identities+=(Get-Content -LiteralPath (Join-Path $rt 'RESEARCH\JOIN185-performance-probe-2-process.json') -Raw | ConvertFrom-Json)
$checks=@($identities | ForEach-Object {[pscustomobject]@{identity=$_;exact_owned_alive=[bool](Get-D1OwnedProcess $_)}})
$files=@(
    @{name='title';path=(Join-Path $rpc 'config\custom_configs\config_BLES01857.yml');expected='22C081E2680A9F81C565860262232AF1296EBEA2831849338BA79BD6E7C2350B'},
    @{name='global';path=(Join-Path $rpc 'config\config.yml');expected='71C7E08939860F78C12A53D095DDC1CCD1525A46D1493714DEE6F9F85D229AB9'},
    @{name='watch';path=(Join-Path $rpc 'd1\d1ppu.txt');expected='442847E5847064F2D570943381CAA88DFD66E9DC212FFD299D9AC6D2F1EDD4B6'})
$hashes=@($files | ForEach-Object {$actual=(Get-FileHash -LiteralPath $_.path).Hash;[pscustomobject]@{name=$_.name;sha256=$actual;unchanged=($actual -eq $_.expected)}})
$out=[ordered]@{at=[DateTime]::UtcNow.ToString('o');checks=$checks;all_exact_owned_closed=(-not($checks.exact_owned_alive -contains $true));configuration=$hashes;all_hashes_match=(-not($hashes.unchanged -contains $false))}
$out | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $rt 'RESEARCH\JOIN185-independent-closed.json') -Encoding utf8
$out | ConvertTo-Json -Depth 8


